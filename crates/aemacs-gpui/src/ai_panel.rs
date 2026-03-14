use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::loader::load_file;
use aemacs_ai::mcp::{ToolHost, ToolRegistry, validate_path};
use aemacs_ai::{AIBackend, Conversation, PersonaRegistry};
use aemacs_core::{Editor, command::Command, mode::Mode};
use async_trait::async_trait;
use gpui::prelude::*;
use gpui::{
    App, AsyncApp, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, WeakEntity, Window,
    div, px, relative, rgb,
};
use regex::Regex;
use std::path::Path;
use std::sync::Arc;

use crate::ai_utils::{AgentEvent, spawn_agent_task};
use crate::editor_view::render_editor_view;
use crate::input_handler::resolve_key_command;

// --- Modal Bridge (ACO-023) ---

use aemacs_ai::rag::KnowledgeBase;

pub enum HostRequest {
    Approval {
        description: String,
        responder: futures::channel::oneshot::Sender<bool>,
    },
    UserPrompt {
        question: String,
        responder: futures::channel::oneshot::Sender<String>,
    },
}

pub struct GuiHost {
    pub request_tx: async_channel::Sender<HostRequest>,
    pub event_tx: tokio::sync::broadcast::Sender<aemacs_core::bus::SystemEvent>,
    pub agent_id: String,
}

#[async_trait]
impl ToolHost for GuiHost {
    async fn ask_approval(&self, description: &str) -> bool {
        let (tx, rx) = futures::channel::oneshot::channel();
        let _ = self
            .request_tx
            .send(HostRequest::Approval {
                description: description.to_string(),
                responder: tx,
            })
            .await;
        rx.await.unwrap_or(false)
    }

    async fn ask_user(&self, question: &str) -> String {
        let (tx, rx) = futures::channel::oneshot::channel();
        let _ = self
            .request_tx
            .send(HostRequest::UserPrompt {
                question: question.to_string(),
                responder: tx,
            })
            .await;
        rx.await.unwrap_or_default()
    }

    fn get_agent_id(&self) -> String {
        self.agent_id.clone()
    }

    fn report_progress(&self, tool_name: String, is_running: bool) {
        let signal = aemacs_core::signals::ToolProgressSignal {
            tool_name,
            is_running,
        };
        if let Ok(payload) = serde_json::to_string(&signal) {
            let _ = self.event_tx.send(aemacs_core::bus::SystemEvent::Signal {
                source: "Specialist".to_string(),
                event_type: "ToolProgress".to_string(),
                payload,
            });
        }
    }

    async fn emit_signal(&self, event_type: String, payload: String) -> anyhow::Result<()> {
        let event = aemacs_core::bus::SystemEvent::Signal {
            source: format!("Agent:{}", self.agent_id),
            event_type,
            payload,
        };
        let _ = self.event_tx.send(event);
        Ok(())
    }
}

pub struct AiPanel {
    pub input_editor: Entity<Editor>,
    pub input_list_state: gpui::ListState,
    pub focus_handle: FocusHandle,
    pub message_scroll_handle: gpui::ScrollHandle,
    pub input_scroll_handle: gpui::ScrollHandle,
    messages: Vec<ChatMessage>,
    backend: OpenAICompatibleBackend,
    host_tx: async_channel::Sender<HostRequest>, // Store TX for later ToolRegistry integration
    event_tx: tokio::sync::broadcast::Sender<aemacs_core::bus::SystemEvent>,
    tasks: Vec<aemacs_core::task::Task>, // Store current plan (ACO-034)
    selected_model_index: usize,
    selected_context: u32,
    pub kb: Arc<KnowledgeBase>,
    pub registry: Arc<ToolRegistry>,
    pub persona_registry: Arc<PersonaRegistry>,
    pub active_persona_name: Option<String>,
    pub conversation: Conversation,
    pub proactive_mode: bool,
    pub width: f32,
    pub is_maximized: bool,
    pub current_action: Option<String>,
    pub status: CognitiveStatus,
    pub shimmer_offset: f32,
    pub window_handle: gpui::AnyWindowHandle,
    pub available_models: Vec<&'static aemacs_ai::models::ModelDefinition>,
    pub available_personas: Vec<String>,
    last_cursor_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CognitiveStatus {
    Idle,
    Thinking,
    #[allow(dead_code)]
    Streaming,
    Errored(String),
}

struct ChatMessage {
    role: String,
    content: String,
    parsed_blocks: Vec<MarkdownBlock>,
}

#[derive(Clone, Debug)]
enum MarkdownBlock {
    Paragraph(String),
    Code { language: String, content: String },
    Header { level: usize, content: String },
}

impl ChatMessage {
    fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        let parsed_blocks = parse_markdown_blocks(&content);
        Self {
            role: role.into(),
            content,
            parsed_blocks,
        }
    }

    fn update_content(&mut self, new_content: String) {
        self.content = new_content;
        self.parsed_blocks = parse_markdown_blocks(&self.content);
    }
}

fn parse_markdown_blocks(text: &str) -> Vec<MarkdownBlock> {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};
    let parser = Parser::new(text);
    let mut blocks = Vec::new();
    let mut current_text = String::new();
    let mut in_code_block = false;
    let mut current_language = String::new();
    let mut current_header_level = 0;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Fenced(lang))) => {
                if !current_text.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
                    current_text.clear();
                }
                in_code_block = true;
                current_language = lang.to_string();
            }
            Event::Start(Tag::CodeBlock(pulldown_cmark::CodeBlockKind::Indented)) => {
                if !current_text.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
                    current_text.clear();
                }
                in_code_block = true;
                current_language = String::new();
            }
            Event::End(TagEnd::CodeBlock) => {
                blocks.push(MarkdownBlock::Code {
                    language: current_language.clone(),
                    content: current_text.trim_end().to_string(),
                });
                current_text.clear();
                in_code_block = false;
            }
            Event::Start(Tag::Heading { level, .. }) => {
                if !current_text.is_empty() {
                    blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
                    current_text.clear();
                }
                current_header_level = level as usize;
            }
            Event::End(TagEnd::Heading(_)) => {
                blocks.push(MarkdownBlock::Header {
                    level: current_header_level,
                    content: current_text.trim().to_string(),
                });
                current_text.clear();
                current_header_level = 0;
            }
            Event::Text(t) => {
                current_text.push_str(&t);
            }
            Event::Code(t) => {
                current_text.push('`');
                current_text.push_str(&t);
                current_text.push('`');
            }
            Event::SoftBreak | Event::HardBreak => {
                current_text.push('\n');
            }
            _ => {}
        }
    }

    if !current_text.is_empty() {
        if in_code_block {
            blocks.push(MarkdownBlock::Code {
                language: current_language,
                content: current_text.trim_end().to_string(),
            });
        } else {
            blocks.push(MarkdownBlock::Paragraph(current_text.trim().to_string()));
        }
    }

    blocks
}

impl AiPanel {
    pub fn new(
        cx: &mut App,
        window_handle: gpui::AnyWindowHandle,
        host_tx: async_channel::Sender<HostRequest>,
        kb: Arc<KnowledgeBase>,
        registry: Arc<ToolRegistry>,
        persona_registry: Arc<PersonaRegistry>,
        bus: aemacs_core::bus::EventBus,
        available_models: Vec<&'static aemacs_ai::models::ModelDefinition>,
    ) -> Entity<Self> {
        let backend = OpenAICompatibleBackend::new("http://localhost:11434/v1", None);
        let model_name = available_models
            .first()
            .map(|m| m.name)
            .unwrap_or("hermes3:8b-llama3.1-q4_K_M");
        let initial_context = available_models
            .first()
            .map(|m| m.max_context)
            .unwrap_or(8192);

        let panel = cx.new(|cx| {
            let input_editor = cx.new(|_cx| {
                let mut editor = Editor::new();
                let _ = editor.run(aemacs_core::command::Command::EnterMode(
                    aemacs_core::mode::Mode::Insert,
                ));
                editor
            });
            let input_list_state = gpui::ListState::new(0, gpui::ListAlignment::Top, px(10.0));
            let focus_handle = cx.focus_handle();
            let message_scroll_handle = gpui::ScrollHandle::new();
            let input_scroll_handle = gpui::ScrollHandle::new();

            AiPanel {
                input_editor,
                input_list_state,
                focus_handle,
                message_scroll_handle,
                input_scroll_handle,
                messages: vec![ChatMessage::new(
                    "System",
                    "AI System Online. Waiting for input...",
                )],
                backend: backend.clone(),
                host_tx,
                event_tx: bus.tx.clone(),
                tasks: Vec::new(),
                selected_model_index: 0,
                selected_context: initial_context,
                kb,
                registry,
                persona_registry,
                active_persona_name: None,
                conversation: Conversation::new(model_name),
                proactive_mode: true,
                width: 400.0,
                is_maximized: false,
                current_action: None,
                status: CognitiveStatus::Idle,
                shimmer_offset: 0.0,
                window_handle,
                available_models,
                available_personas: Vec::new(),
                last_cursor_line: 0,
            }
        });

        // Initialize personas (ACO-005)
        let persona_registry = panel.read(cx).persona_registry.clone();
        let panel_weak_init = panel.downgrade();
        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                let personas = persona_registry.list_personas().await;
                let _ = cx.update(|app: &mut App| {
                    if let Some(panel) = panel_weak_init.upgrade() {
                        panel.update(app, |this, cx| {
                            this.available_personas = personas;
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();

        // ACO-026 & ACO-030: Spawn the Triage Router Listener
        let panel_weak = panel.downgrade();
        let mut rx = bus.subscribe();

        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok(event) = rx.recv().await {
                    match event {
                        aemacs_core::bus::SystemEvent::Signal { source, event_type, payload } => {
                                                    let _ = cx.update(|app: &mut App| {
                                                        if let Some(panel) = panel_weak.upgrade() {
                                    panel.update(app, |this, cx| {
                                        // ACO-030: Handle Tool Progress
                                        if event_type == "ToolProgress" {
                                            if let Ok(progress) = serde_json::from_str::<aemacs_core::signals::ToolProgressSignal>(&payload) {
                                                if progress.is_running {
                                                    this.current_action = Some(format!("Executing {}...", progress.tool_name));
                                                } else {
                                                    this.current_action = None;
                                                }
                                                cx.notify();
                                            }
                                            return;
                                        }

                                        if !this.proactive_mode {
                                            return;
                                        }

                                        // Check if AI is currently generating
                                        if let Some(last_msg) = this.messages.last() {
                                            if last_msg.role == "AI" && last_msg.content.is_empty() {
                                                return; // Drop signal if busy
                                            }
                                        }

                                        if event_type == "BufferModified" {
                                            let observation = format!(
                                                "Observation (from {}): The following file was just modified:\n{}",
                                                source, payload
                                            );

                                            this.messages.push(ChatMessage::new(
                                                "System",
                                                "👀 I noticed you changed a file. Let me look..."
                                            ));

                                            this.conversation.add_message(aemacs_ai::Message::user(observation));

                                            // Ensure we have an active persona, default to marjin for refactoring
                                            if this.active_persona_name.is_none() {
                                                let persona_registry = this.persona_registry.clone();
                                                cx.spawn(|panel: WeakEntity<Self>, cx: &mut AsyncApp| {
                                                    let mut cx = cx.clone();
                                                    async move {
                                                        if let Some(persona) = persona_registry.get_persona("marjin").await {
                                                            let _ = panel.update(&mut cx, |this, cx: &mut Context<Self>| {
                                                                this.active_persona_name = Some("marjin".to_string());
                                                                this.conversation.set_persona(persona);
                                                                this.trigger_ai_response(cx);
                                                                cx.notify();
                                                            });
                                                        }
                                                    }
                                                }).detach();
                                            } else {
                                                this.trigger_ai_response(cx);
                                            }
                                        }
                                    });
                                }
                            });
                        }
                        _ => {} // Ignore other events for now
                    }
                }
            }
        }).detach();

        // ACO-033: Backend Health Check
        let backend_check = backend.clone();
        let panel_weak_check = panel.downgrade();

        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                // Run the health check on the Tokio worker pool
                let check_result = aemacs_core::runtime::Tokio::spawn(&cx, async move {
                    backend_check.health_check().await
                });

                // Wait for the task to finish if it was successfully spawned
                if let Ok(task) = check_result {
                    if let Ok(Err(_)) = task.await {
                        let _ = cx.update(|app: &mut App| {
                            if let Some(panel) = panel_weak_check.upgrade() {
                                let _ = panel.update(app, |this, cx| {
                                    this.messages.push(ChatMessage::new(
                                        "System",
                                        "❌ [BACKEND OFFLINE] Ollama is not responding at http://localhost:11434/v1. Specialists are currently disabled."
                                    ));
                                    cx.notify();
                                });
                            }
                        });
                    }
                }
            }
        }).detach();

        panel
    }

    /// Updates the local task list (ACO-034)
    pub fn update_tasks(&mut self, tasks: Vec<aemacs_core::task::Task>, cx: &mut Context<Self>) {
        self.tasks = tasks;
        cx.notify();
    }

    /// Handles a programmatic persona switch from the dispatcher (ACO-010).
    pub fn handoff_persona(
        &mut self,
        name: String,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let name_lower = name.to_lowercase();
        if let Some(persona) =
            futures::executor::block_on(self.persona_registry.get_persona(&name_lower))
        {
            self.active_persona_name = Some(name_lower);
            self.conversation.set_persona(persona);

            self.messages.push(ChatMessage::new(
                "System",
                format!("Programmatic handoff to: {}", name.to_uppercase()),
            ));

            if let Some(msg) = message {
                self.messages.push(ChatMessage::new("User", msg.clone()));
                self.conversation.add_message(aemacs_ai::Message::user(msg));
                // Automatically trigger the new agent if a message was provided
                self.trigger_ai_response(cx);
            }
            cx.notify();
        }
    }

    fn trigger_ai_response(&mut self, cx: &mut Context<Self>) {
        // Prepare AI Message Placeholder
        self.messages.push(ChatMessage::new("AI", ""));
        self.status = CognitiveStatus::Thinking;

        // ACO-040: Start recursive animation loop
        self.animate_pulse(cx);

        let registry = self.registry.clone();
        let host = GuiHost {
            request_tx: self.host_tx.clone(),
            event_tx: self.event_tx.clone(),
            agent_id: self
                .active_persona_name
                .clone()
                .unwrap_or_else(|| "global".to_string()),
        };
        let backend = self.backend.clone();
        let conversation = self.conversation.clone();

        // Spawn Agent Task
        spawn_agent_task(
            cx,
            backend,
            registry,
            self.kb.clone(),
            host,
            conversation,
            |panel, cx, event| {
                match event {
                    AgentEvent::StreamChunk(chunk) => {
                        panel.status = CognitiveStatus::Streaming;
                        if let Some(last_msg) = panel.messages.last_mut() {
                            if last_msg.role == "AI" {
                                last_msg.update_content(last_msg.content.clone() + &chunk);
                            }
                        }
                        panel
                            .message_scroll_handle
                            .set_offset(gpui::point(px(0.0), px(999999.0)));
                    }
                    AgentEvent::ToolStarted(name) => {
                        panel.current_action = Some(format!("Executing {}...", name));
                    }
                    AgentEvent::ToolFinished(_name, _success) => {
                        panel.current_action = None;
                    }
                    AgentEvent::Result(updated_conv) => {
                        panel.status = CognitiveStatus::Idle;
                        panel.conversation = updated_conv;

                        // ACO-025: Synchronize UI messages with the updated conversation
                        // This ensures tool outputs are visible in the message area.
                        let conv_messages = panel.conversation.messages();
                        if conv_messages.len() > panel.messages.len() {
                            for i in panel.messages.len()..conv_messages.len() {
                                let m = &conv_messages[i];
                                let role = match m.role {
                                    aemacs_ai::Role::System => "System",
                                    aemacs_ai::Role::User => "User",
                                    aemacs_ai::Role::Assistant => "AI",
                                    aemacs_ai::Role::Tool => "Tool",
                                };
                                let content = match &m.content {
                                    aemacs_ai::Content::Text(t) => t.clone(),
                                    _ => String::new(),
                                };
                                panel.messages.push(ChatMessage::new(role, content));
                            }
                        }
                    }
                    AgentEvent::Error(e) => {
                        panel.status = CognitiveStatus::Errored(e.to_string());
                        if let Some(last_msg) = panel.messages.last_mut() {
                            if last_msg.role == "AI" {
                                last_msg.role = "System".to_string();
                                last_msg.update_content(format!("❌ AI Error: {}", e));
                            }
                        }
                        log::error!("AI Task Failed: {}", e);
                    }
                }
                cx.notify();
            },
        )
        .detach();
    }

    /// Schedules the next frame of the cognition pulse animation.
    fn animate_pulse(&mut self, cx: &mut Context<Self>) {
        if matches!(self.status, CognitiveStatus::Idle) {
            return;
        }

        let window_handle = self.window_handle;
        let handle = cx.entity().downgrade();

        let _ = cx.update_window(window_handle, move |_, window, _cx| {
            window.on_next_frame(move |_, cx| {
                if let Some(panel) = handle.upgrade() {
                    let _ = panel.update(cx, |this, cx| {
                        // Adjust speed based on status
                        let delta = match this.status {
                            CognitiveStatus::Thinking => 0.015,
                            _ => 0.025,
                        };
                        this.shimmer_offset = (this.shimmer_offset + delta) % 1.0;
                        cx.notify();
                        this.animate_pulse(cx);
                    });
                }
            });
        });
    }

    fn render_agent_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_persona = self.active_persona_name.clone();

        div()
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x5c6370))
                    .child("ACTIVE AGENT"),
            )
            .child(div().flex().flex_wrap().gap(px(4.0)).children(
                self.available_personas.iter().enumerate().map(|(i, name)| {
                    let is_selected = active_persona.as_ref() == Some(name);
                    let name_clone = name.clone();
                    div()
                        .id(i)
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded_md()
                        .border_1()
                        .border_color(if is_selected {
                            rgb(0xbd93f9)
                        } else {
                            rgb(0x3e4451)
                        })
                        .bg(if is_selected {
                            rgb(0x282c34)
                        } else {
                            rgb(0x21252b)
                        })
                        .text_color(if is_selected {
                            rgb(0xffffff)
                        } else {
                            rgb(0xabb2bf)
                        })
                        .text_size(px(11.0))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            let name_lower = name_clone.clone();
                            let persona_registry = this.persona_registry.clone();

                            cx.spawn(|panel: WeakEntity<Self>, cx: &mut AsyncApp| {
                                let mut cx = cx.clone();
                                async move {
                                    if let Some(persona) =
                                        persona_registry.get_persona(&name_lower).await
                                    {
                                        let _ = panel.update(
                                            &mut cx,
                                            |this, cx: &mut Context<Self>| {
                                                this.active_persona_name = Some(name_lower);
                                                this.conversation.set_persona(persona);
                                                cx.notify();
                                            },
                                        );
                                    }
                                }
                            })
                            .detach();
                        }))
                        .child(name.to_uppercase())
                }),
            ))
    }

    fn render_model_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_index = self.selected_model_index;

        div()
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x5c6370))
                    .child("MODEL MATRIX"),
            )
            .child(div().flex().flex_wrap().gap(px(4.0)).children(
                self.available_models.iter().enumerate().map(|(i, model)| {
                    let is_selected = i == selected_index;

                    // Role colors (Sacred Palette)
                    let role_color = match model.role {
                        aemacs_ai::models::ModelRole::Logic => rgb(0x61afef), // Sapphire
                        aemacs_ai::models::ModelRole::Creative => rgb(0xd19a66), // Amber
                        aemacs_ai::models::ModelRole::Roleplay => rgb(0x98c379), // Emerald
                    };

                    div()
                        .id(("model", i))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_x(px(4.0))
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded_md()
                        .border_1()
                        .border_color(if is_selected {
                            rgb(0xbd93f9)
                        } else {
                            rgb(0x3e4451)
                        })
                        .bg(if is_selected {
                            rgb(0x282c34)
                        } else {
                            rgb(0x21252b)
                        })
                        .text_color(if is_selected {
                            rgb(0xffffff)
                        } else {
                            rgb(0xabb2bf)
                        })
                        .text_size(px(11.0))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.selected_model_index = i;
                            if let Some(m) = this.available_models.get(i) {
                                this.conversation.set_model(m.name);
                                this.selected_context = m.max_context;
                                this.conversation.set_context_window(this.selected_context);
                            }
                            cx.notify();
                        }))
                        .child(div().size(px(6.0)).rounded_full().bg(role_color))
                        .child(model.label)
                }),
            ))
            .child({
                let model = self.available_models.get(selected_index);
                div().when_some(model, |this, model| {
                    this.p(px(8.0))
                        .bg(rgb(0x181a1f))
                        .rounded_md()
                        .mt(px(4.0))
                        .flex_col()
                        .gap_y(px(2.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0xffffff))
                                .child(format!(
                                    "{} | VRAM: {:.1} GB | Context: {}k",
                                    model.label,
                                    model.base_vram_gb,
                                    model.max_context / 1024
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(0xabb2bf))
                                .child(model.model_description),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .italic()
                                .text_color(rgb(0x5c6370))
                                .child(model.license_constraints),
                        )
                })
            })
    }

    fn render_context_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_model = &aemacs_ai::models::MODELS[self.selected_model_index];
        let current_context = self.selected_context;

        div()
            .flex()
            .flex_col()
            .gap_y(px(4.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x5c6370))
                    .child("CONTEXT WINDOW"),
            )
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    aemacs_ai::models::CONTEXT_OPTIONS
                        .iter()
                        .filter(|&&opt| opt <= selected_model.max_context)
                        .map(|&opt| {
                            let is_selected = opt == current_context;
                            div()
                                .id(("context", opt as usize))
                                .px(px(6.0))
                                .py(px(2.0))
                                .rounded_md()
                                .border_1()
                                .border_color(if is_selected {
                                    rgb(0xbd93f9)
                                } else {
                                    rgb(0x3e4451)
                                })
                                .bg(if is_selected {
                                    rgb(0x282c34)
                                } else {
                                    rgb(0x21252b)
                                })
                                .text_color(if is_selected {
                                    rgb(0xffffff)
                                } else {
                                    rgb(0xabb2bf)
                                })
                                .text_size(px(11.0))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.selected_context = opt;
                                    this.conversation.set_context_window(opt);
                                    cx.notify();
                                }))
                                .child(if opt >= 1024 {
                                    format!("{}k", opt / 1024)
                                } else {
                                    opt.to_string()
                                })
                        }),
                ),
            )
    }

    fn render_vram_estimate(&self) -> impl IntoElement {
        let model = &aemacs_ai::models::MODELS[self.selected_model_index];
        let estimate =
            model.base_vram_gb + (self.selected_context as f32 / 1024.0) * model.kv_rate_gb_per_1k;

        div()
            .text_size(px(10.0))
            .text_color(rgb(0xbd93f9))
            .italic()
            .child(format!("Estimated VRAM: {:.2} GB", estimate))
    }

    fn render_cognition_pulse(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let status = &self.status;
        let is_active = !matches!(status, CognitiveStatus::Idle);

        let (color, shimmer_width, intensity) = match status {
            CognitiveStatus::Idle => (
                gpui::rgba(0x1c315eff),
                Default::default(),
                Default::default(),
            ), // Dormant cobalt
            CognitiveStatus::Thinking => (gpui::rgba(0xffbf00ff), 0.4, 0.8), // Radiating amber
            CognitiveStatus::Streaming => (gpui::rgba(0x00ff7fff), 0.8, 1.0), // Vibrant emerald
            CognitiveStatus::Errored(_) => (gpui::rgba(0xdc143cff), 0.2, 1.0), // Jagged crimson
        };

        // Simulated Sine-wave pulsing for opacity (intensity)
        let pulse_opacity = (self.shimmer_offset * std::f32::consts::PI * 2.0).sin() * 0.3 + 0.7;
        let final_opacity = intensity * pulse_opacity;

        div()
            .h(px(8.0)) // The Height of Sanctity
            .w_full()
            .when(is_active, |this| {
                this.bg(gpui::rgba(0x181a1fff)).child(
                    div()
                        .size_full()
                        .bg(color)
                        .opacity(final_opacity)
                        .relative()
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(relative(self.shimmer_offset))
                                .w(relative(shimmer_width))
                                .bg(gpui::rgba(0xffffffff))
                                .opacity(0.4), // The "Heat" core
                        ),
                )
            })
            .when(!is_active, |this| this.bg(gpui::transparent_black()))
    }

    fn handle_input_keydown(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let mode = self.input_editor.read(cx).mode;

        if keystroke.key == "enter" {
            if mode == Mode::Normal || !keystroke.modifiers.shift {
                self.send_message(cx);
                return;
            } else {
                self.input_editor.update(cx, |editor, _| {
                    let _ = editor.run(Command::InsertNewline);
                });
                cx.notify();
                return;
            }
        }

        if let Some(cmd) = resolve_key_command(keystroke, mode) {
            self.input_editor.update(cx, |editor, _cx| {
                let _ = editor.run(cmd);
            });
            cx.notify();
        }
    }

    fn send_message(&mut self, cx: &mut Context<Self>) {
        let raw_text = self.input_editor.read(cx).buffer.text();
        if raw_text.trim().is_empty() {
            return;
        }

        let mut text = raw_text.clone();
        let mut switched = false;

        // --- Statute Lexer (@) (ACO-018) ---
        // Catch single '@' but not '@@'
        // Rust's regex doesn't support lookbehind, so we match '@' and check manually or use a specific pattern
        let statute_regex = Regex::new(r"@([^@\s]+)").unwrap();
        let statute_matches: Vec<_> = statute_regex
            .find_iter(&text)
            .filter(|m| {
                let start = m.start();
                // Ensure it's not preceded by another '@'
                if start > 0 && text.as_bytes()[start - 1] == b'@' {
                    return false;
                }
                true
            })
            .collect();

        if statute_matches.len() > 1 {
            self.messages.push(ChatMessage::new(
                "System",
                "❌ Multiple statutes detected! You can only load one profile (@) at a time. Use @@ for attaching multiple source files."
            ));
            cx.notify();
            return;
        }

        if let Some(mat) = statute_matches.first() {
            let path_str = mat.as_str().trim_start_matches('@');
            match validate_path(path_str) {
                Ok(path) => {
                    match load_file(&path) {
                        Ok(part) => {
                            if let aemacs_ai::ContentPart::Text {
                                text: profile_content,
                            } = part
                            {
                                self.conversation.set_profile(profile_content);
                                self.messages.push(ChatMessage::new(
                                    "System",
                                    format!("📖 Profile loaded: {}", path_str),
                                ));
                                // Strip the tag from the final message text
                                let start = mat.start();
                                let end = mat.end();
                                text.replace_range(start..end, "");
                                text = text.trim().to_string();
                            } else {
                                self.messages.push(ChatMessage::new(
                                    "System",
                                    format!("⚠️ Profile at '{}' is not a text file.", path_str),
                                ));
                            }
                        }
                        Err(e) => {
                            self.messages.push(ChatMessage::new(
                                "System",
                                format!("⚠️ Failed to load profile: {} ({})", path_str, e),
                            ));
                        }
                    }
                }
                Err(e) => {
                    self.messages.push(ChatMessage::new(
                        "System",
                        format!("⚠️ Invalid profile path: {} ({})", path_str, e),
                    ));
                }
            }
        }

        // --- Material Lexer (@@) (ACO-019) ---
        let material_regex = Regex::new(r"@@([^@\s]+)").unwrap();
        let material_matches: Vec<(usize, usize, String)> = material_regex
            .find_iter(&text)
            .map(|m| (m.start(), m.end(), m.as_str().to_string()))
            .collect();
        let mut attachments = Vec::new();

        for (start, end, mat_str) in material_matches.iter().rev() {
            let path_str = mat_str.trim_start_matches("@@");
            match validate_path(path_str) {
                Ok(path) => {
                    match load_file(&path) {
                        Ok(part) => {
                            if let aemacs_ai::ContentPart::Text { text: file_content } = part {
                                attachments.push((path_str.to_string(), file_content));
                                // Strip the tag
                                text.replace_range(*start..*end, "");
                            } else {
                                self.messages.push(ChatMessage::new(
                                    "System",
                                    format!("⚠️ Material at '{}' is not a text file.", path_str),
                                ));
                            }
                        }
                        Err(e) => {
                            self.messages.push(ChatMessage::new(
                                "System",
                                format!("⚠️ Failed to load material: {} ({})", path_str, e),
                            ));
                        }
                    }
                }
                Err(e) => {
                    self.messages.push(ChatMessage::new(
                        "System",
                        format!("⚠️ Invalid material path: {} ({})", path_str, e),
                    ));
                }
            }
        }

        if !attachments.is_empty() {
            text = text.trim().to_string();
            text.push_str("\n\n---\n### ATTACHED CONTEXT\n");
            for (path, content) in attachments {
                let ext = Path::new(&path)
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                text.push_str(&format!(
                    "#### File: {}\n```{}\n{}\n```\n",
                    path, ext, content
                ));
            }
        }

        // --- Slash Command Interceptor (ACO-005/006) ---
        if text.starts_with('/') {
            let (cmd, remainder) = text.split_once(' ').unwrap_or((text.as_str(), ""));
            let agent_name = cmd.trim_start_matches('/').to_lowercase();

            if let Some(_) =
                futures::executor::block_on(self.persona_registry.get_persona(&agent_name))
            {
                self.active_persona_name = Some(agent_name);
                text = remainder.to_string();
                switched = true;
            } else {
                // ACO-006: Lexical Guard
                self.messages.push(ChatMessage::new(
                    "System",
                    format!(
                        "⚠️ Unknown agent: /{}. Type a valid specialist name.",
                        agent_name
                    ),
                ));
                self.input_editor.update(cx, |editor, _| {
                    editor.buffer.content = ropey::Rope::new();
                    editor.mode = Mode::Normal;
                });
                cx.notify();
                return;
            }
        }

        // 1. Add User/System Message
        if !text.trim().is_empty() {
            self.messages.push(ChatMessage::new("User", text.clone()));
        } else if switched {
            let persona_display = self
                .active_persona_name
                .as_deref()
                .unwrap_or("UNKNOWN")
                .to_uppercase();
            self.messages.push(ChatMessage::new(
                "System",
                format!("Agent switched to: {}", persona_display),
            ));
        } else {
            return;
        }

        // 2. Clear Input
        self.input_editor.update(cx, |editor, _| {
            editor.clear();
            editor.mode = Mode::Normal;
        });

        if text.trim().is_empty() {
            cx.notify();
            return;
        }

        // 3. Update persistent conversation
        if let Some(persona_name) = &self.active_persona_name {
            if let Some(persona) =
                futures::executor::block_on(self.persona_registry.get_persona(persona_name))
            {
                self.conversation.set_persona(persona);
            }
        } else {
            self.conversation.clear_persona();
        }
        self.conversation
            .add_message(aemacs_ai::Message::user(text));

        // Auto-scroll to the bottom so the user's message is immediately visible
        self.message_scroll_handle
            .set_offset(gpui::point(px(0.0), px(999999.0)));

        cx.notify();

        self.trigger_ai_response(cx);
    }
}

impl Render for AiPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.input_editor.read(cx);
        let line_count = editor.line_count();
        let (line, col) = editor.cursor_position();
        let cursor_line = line.saturating_sub(1);

        let current_count = self.input_list_state.item_count();

        if current_count != line_count {
            // Structural change: full redraw needed
            self.input_list_state.splice(0..current_count, line_count);
        } else {
            // Typing change: partial redraw
            if cursor_line == self.last_cursor_line {
                self.input_list_state
                    .splice(cursor_line..cursor_line + 1, 1);
            } else {
                let min_line = std::cmp::min(cursor_line, self.last_cursor_line);
                let max_line = std::cmp::max(cursor_line, self.last_cursor_line);
                if min_line != max_line {
                    self.input_list_state.splice(min_line..min_line + 1, 1);
                    self.input_list_state.splice(max_line..max_line + 1, 1);
                }
            }
        }
        self.last_cursor_line = cursor_line;

        let bg = rgb(0x21252b);
        let input_bg = rgb(0x282c34);

        let root_div = div().flex().flex_col();

        let root_div = if self.is_maximized {
            root_div.w_full()
        } else {
            root_div.w(px(self.width))
        };

        root_div
            .size_full()
            .min_h_0() // CRITICAL: Stop the root panel from blowing out
            .bg(bg)
            .overflow_hidden() // Enforce strict bounding box to prevent layout blowouts
            .border_l_1()
            .border_color(rgb(0x181a1f))
            .child(
                div()
                    .flex_shrink_0() // Ensure header doesn't shrink
                    .w_full()
                    .p(px(10.0))
                    .border_b_1()
                    .border_color(rgb(0x181a1f))
                    .flex()
                    .flex_col()
                    .gap_y(px(8.0))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Æmacs Mesh"),
                            )
                            .child(
                                div()
                                    .id("maximize_btn")
                                    .cursor_pointer()
                                    .text_color(rgb(0xabb2bf))
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.is_maximized = !this.is_maximized;
                                        cx.notify();
                                    }))
                                    .child(if self.is_maximized { "[ _ ]" } else { "[ + ]" }),
                            ),
                    )
                    .child(self.render_agent_selector(cx))
                    .child(self.render_model_selector(cx))
                    .child(self.render_context_selector(cx))
                    .child(self.render_vram_estimate()),
            )
            .when(!self.tasks.is_empty(), |this| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .w_full()
                        .max_h(px(400.0))
                        .border_b_1()
                        .border_color(rgb(0x181a1f))
                        .p(px(10.0))
                        .flex()
                        .flex_col()
                        .gap_y(px(4.0))
                        .id("task_board")
                        .overflow_y_scroll()
                        .children(self.tasks.iter().map(|task| {
                            let icon = match task.status {
                                aemacs_core::task::TaskStatus::Pending => "[ ]",
                                aemacs_core::task::TaskStatus::InProgress => "[>]",
                                aemacs_core::task::TaskStatus::Completed => "[x]",
                                aemacs_core::task::TaskStatus::Failed => "[!]",
                            };
                            let color = match task.status {
                                aemacs_core::task::TaskStatus::Pending => rgb(0x5c6370),
                                aemacs_core::task::TaskStatus::InProgress => rgb(0x61afef),
                                aemacs_core::task::TaskStatus::Completed => rgb(0x98c379),
                                aemacs_core::task::TaskStatus::Failed => rgb(0xe06c75),
                            };
                            div()
                                .flex()
                                .gap_x(px(8.0))
                                .text_size(px(12.0))
                                .text_color(color)
                                .child(div().child(icon))
                                .child(div().child(task.description.clone()))
                        })),
                )
            })
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .min_h_0() // Crucial for allowing flex children to shrink and scroll
                    .flex_col()
                    .gap_y(px(10.0))
                    .p(px(10.0))
                    .id("message_area")
                    .overflow_y_scroll()
                    .track_scroll(&self.message_scroll_handle)
                    .children(self.messages.iter().map(|msg| {
                        let is_user = msg.role == "User";
                        let is_tool = msg.role == "Tool";
                        let is_error = msg.content.contains("🛠️ TOOL_ERROR");

                        if is_tool {
                            // ACO-025: Visual Loom - Specialist Pill
                            return div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_x(px(8.0))
                                .px(px(8.0))
                                .py(px(4.0))
                                .h(px(24.0))
                                .rounded_full()
                                .border_1()
                                .border_color(if is_error {
                                    rgb(0xe06c75) // Error Red
                                } else {
                                    rgb(0x61afef) // Sapphire Blue
                                })
                                .bg(rgb(0x1e1e1e))
                                .child(div().text_size(px(12.0)).child("🛠️"))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(if is_error {
                                            rgb(0xe06c75)
                                        } else {
                                            rgb(0xabb2bf)
                                        })
                                        .child("Specialist action performed"),
                                )
                                .into_any_element();
                        }

                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0x5c6370))
                                    .child(msg.role.clone()),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .p(px(8.0))
                                    .rounded_md()
                                    .bg(if is_user {
                                        rgb(0x3e4451)
                                    } else {
                                        rgb(0x282c34)
                                    })
                                    .text_color(if is_error {
                                        rgb(0xe06c75)
                                    } else {
                                        rgb(0xabb2bf)
                                    })
                                    .flex()
                                    .flex_col()
                                    .gap_y(px(4.0))
                                    .children(msg.parsed_blocks.iter().map(|block| {
                                        match block {
                                            MarkdownBlock::Paragraph(text) => div()
                                                .w_full()
                                                .child(text.clone())
                                                .into_any_element(),
                                            MarkdownBlock::Code { language, content } => div()
                                                .w_full()
                                                .bg(rgb(0x1e1e1e))
                                                .p(px(6.0))
                                                .rounded_sm()
                                                .border_1()
                                                .border_color(rgb(0x3e4451))
                                                .overflow_x_hidden()
                                                .child(
                                                    div()
                                                        .text_color(rgb(0x61afef))
                                                        .text_size(px(10.0))
                                                        .child(language.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xabb2bf))
                                                        .font_family("monospace")
                                                        .child(content.clone()),
                                                )
                                                .into_any_element(),
                                            MarkdownBlock::Header { level, content } => {
                                                let size = match level {
                                                    1 => 18.0,
                                                    2 => 16.0,
                                                    3 => 14.0,
                                                    _ => 12.0,
                                                };
                                                div()
                                                    .w_full()
                                                    .text_size(px(size))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child(content.clone())
                                                    .into_any_element()
                                            }
                                        }
                                    })),
                            )
                            .into_any_element()
                    })),
            )
            .child(self.render_cognition_pulse(cx))
            .child(
                // ACO-030: Progress Signal line
                div().px(px(10.0)).h(px(16.0)).flex().items_center().child(
                    if let Some(action) = &self.current_action {
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x61afef))
                            .italic()
                            .child(action.clone())
                    } else {
                        div()
                    },
                ),
            )
            .child(
                // Input Area
                div()
                    .id("input_area")
                    .flex_shrink_0()
                    .h(px(100.0)) // Set a fixed initial height or use flex rules properly
                    .max_h(px(200.0))
                    .bg(input_bg)
                    .border_t_1()
                    .border_color(rgb(0x181a1f))
                    .p(px(8.0))
                    .overflow_x_hidden()
                    .track_focus(&self.focus_handle)
                    .on_click(cx.listener(|this, _, window, cx| {
                        window.focus(&this.focus_handle, cx);
                    }))
                    .on_key_down(cx.listener(Self::handle_input_keydown))
                    .child({
                        let editor = self.input_editor.read(cx);
                        render_editor_view(editor, self.input_list_state.clone(), true)
                    }),
            )
    }
}

mod tests {
    use super::*;
    use regex::Regex;

    /// This quest verifies that our regex can distinguish between the Holy Statute (@)
    /// and the Raw Materials (@@). It ensures that '@@' is never accidentally
    /// identified as a single-@ statute.
    #[test]
    fn test_statute_differentiation_logic() {
        let statute_regex = Regex::new(r"@([^@\s]+)").unwrap();
        let find_statute_indices = |text: &str| {
            statute_regex
                .find_iter(text)
                .filter(|m: &regex::Match| {
                    let start = m.start();
                    if start > 0 && text.as_bytes()[start - 1] == b'@' {
                        return false;
                    }
                    true
                })
                .map(|m: regex::Match| (m.start(), m.end()))
                .collect::<Vec<(usize, usize)>>()
        };

        // Case 1: Multiple statutes (The Law-Confusion Dragon)
        let text1 = "@file1.md @file2.md build the forge";
        assert_eq!(find_statute_indices(text1).len(), 2);

        // Case 2: Mixed materials and statutes
        let text2 = "@@src/main.rs @profile.md";
        let matches2 = find_statute_indices(text2);
        assert_eq!(matches2.len(), 1);
        assert_eq!(&text2[matches2[0].0..matches2[0].1], "@profile.md");

        // Case 3: Pure materials (Should be ignored by this lexer)
        let text3 = "@@src/main.rs @@src/lib.rs";
        assert_eq!(find_statute_indices(text3).len(), 0);
    }

    /// This quest verifies that the Material Lexer (@@) can find multiple
    /// attachments within a single string of intent.
    #[test]
    fn test_material_collection_logic() {
        let material_regex = Regex::new(r"@@([^@\s]+)").unwrap();
        let text = "Analyze @@src/main.rs and @@crates/core/lib.rs please.";
        let matches: Vec<_> = material_regex.find_iter(text).collect();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].as_str(), "@@src/main.rs");
        assert_eq!(matches[1].as_str(), "@@crates/core/lib.rs");
    }

    /// The Ultimate Quest! This test verifies the full transformation of a complex
    /// mixed input. It ensures that statutes and materials are correctly extracted
    /// and that ALL tags are stripped from the final payload sent to the specialists.
    #[test]
    fn test_mixed_lexer_transformation() {
        let statute_regex = Regex::new(r"@([^@\s]+)").unwrap();
        let material_regex = Regex::new(r"@@([^@\s]+)").unwrap();

        let mut text = "@ai/profiles/rust.md @@src/rag.rs @@src/mcp.rs please analyze.".to_string();

        // 1. Extract Statute
        let statute_matches: Vec<_> = statute_regex
            .find_iter(&text)
            .filter(|m: &regex::Match| {
                let start = m.start();
                if start > 0 && text.as_bytes()[start - 1] == b'@' {
                    return false;
                }
                true
            })
            .collect();

        assert_eq!(statute_matches.len(), 1);

        // 2. Extract Materials
        let material_matches: Vec<_> = material_regex.find_iter(&text).collect();
        assert_eq!(material_matches.len(), 2);

        // 3. Execute Stripping (Simulating the reverse-iteration logic used in send_message)
        let mut all_tags: Vec<_> = statute_matches
            .iter()
            .map(|m: &regex::Match| (m.start(), m.end()))
            .collect();
        all_tags.extend(
            material_matches
                .iter()
                .map(|m: &regex::Match| (m.start(), m.end())),
        );
        all_tags.sort_by_key(|k| k.0);

        for (start, end) in all_tags.iter().rev() {
            text.replace_range(start..end, "");
        }

        let cleaned_text = text.trim();
        assert_eq!(
            cleaned_text, "please analyze.",
            "The final intent must be pure and free of metadata tags!"
        );
    }

    #[test]
    fn test_visual_loom_pill_logic() {
        // Quest: Verify that the UI correctly identifies tool roles and errors

        // Case 1: Successful tool
        let msg_ok = ChatMessage::new("Tool", "Result of read_file...");
        let is_tool_ok = msg_ok.role == "Tool";
        let is_error_ok = msg_ok.content.contains("🛠️ TOOL_ERROR");

        assert!(is_tool_ok, "Should identify 'Tool' role.");
        assert!(
            !is_error_ok,
            "Should not identify error in successful result."
        );

        // Case 2: Failed tool
        let msg_err = ChatMessage::new("Tool", "🛠️ TOOL_ERROR: [File not found]");
        let is_tool_err = msg_err.role == "Tool";
        let is_error_err = msg_err.content.contains("🛠️ TOOL_ERROR");

        assert!(is_tool_err, "Should identify 'Tool' role.");
        assert!(
            is_error_err,
            "Should identify error when TOOL_ERROR prefix is present."
        );
    }
}
