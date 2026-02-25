use aemacs_ai::Conversation;
use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::mcp::{ToolHost, ToolRegistry};
use aemacs_ai::PersonaRegistry;
use aemacs_core::{Editor, command::Command, mode::Mode};
use async_trait::async_trait;
use gpui::prelude::*;
use gpui::{App, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Window, div, px, rgb};
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
}

pub struct AiPanel {
    pub input_editor: Entity<Editor>,
    pub focus_handle: FocusHandle,
    messages: Vec<ChatMessage>,
    backend: OpenAICompatibleBackend,
    host_tx: async_channel::Sender<HostRequest>, // Store TX for later ToolRegistry integration
    tasks: Vec<aemacs_core::task::Task>,         // Store current plan (ACO-034)
    selected_model_index: usize,
    selected_context: u32,
    pub kb: Arc<KnowledgeBase>,
    pub registry: Arc<ToolRegistry>,
    pub persona_registry: Arc<PersonaRegistry>,
    pub active_persona_name: Option<String>,
    pub conversation: Conversation,
}

struct ChatMessage {
    role: String,
    content: String,
}

impl AiPanel {
    pub fn new(
        cx: &mut App,
        host_tx: async_channel::Sender<HostRequest>,
        kb: Arc<KnowledgeBase>,
        registry: Arc<ToolRegistry>,
        persona_registry: Arc<PersonaRegistry>,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let input_editor = cx.new(|_cx| Editor::new());
            let focus_handle = cx.focus_handle();

            // Default to local Ollama instance for now
            let backend = OpenAICompatibleBackend::new("http://localhost:11434/v1", None);
            let model_name = aemacs_ai::models::MODELS[0].name;

            AiPanel {
                input_editor,
                focus_handle,
                messages: vec![ChatMessage {
                    role: "System".to_string(),
                    content: "AI System Online. Waiting for input...".to_string(),
                }],
                backend,
                host_tx,
                tasks: Vec::new(),
                selected_model_index: 0,
                selected_context: aemacs_ai::models::MODELS[0].max_context,
                kb,
                registry,
                persona_registry,
                active_persona_name: None,
                conversation: Conversation::new(model_name),
            }
        })
    }

    /// Updates the local task list (ACO-034)
    pub fn update_tasks(&mut self, tasks: Vec<aemacs_core::task::Task>, cx: &mut Context<Self>) {
        self.tasks = tasks;
        cx.notify();
    }

    /// Handles a programmatic persona switch from the dispatcher (ACO-010).
    pub fn handoff_persona(&mut self, name: String, message: Option<String>, cx: &mut Context<Self>) {
        let name_lower = name.to_lowercase();
        if let Some(persona) =
            futures::executor::block_on(self.persona_registry.get_persona(&name_lower))
        {
            self.active_persona_name = Some(name_lower);
            self.conversation.set_persona(persona);

            self.messages.push(ChatMessage {
                role: "System".to_string(),
                content: format!("Programmatic handoff to: {}", name.to_uppercase()),
            });

            if let Some(msg) = message {
                self.messages.push(ChatMessage {
                    role: "User".to_string(),
                    content: msg.clone(),
                });
                self.conversation.add_message(aemacs_ai::Message::user(msg));
                // Automatically trigger the new agent if a message was provided
                self.trigger_ai_response(cx);
            }
            cx.notify();
        }
    }

    fn trigger_ai_response(&mut self, cx: &mut Context<Self>) {
        // Prepare AI Message Placeholder
        self.messages.push(ChatMessage {
            role: "AI".to_string(),
            content: "".to_string(),
        });

        let registry = self.registry.clone();
        let host = GuiHost {
            request_tx: self.host_tx.clone(),
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
                    AgentEvent::Result(result) => {
                        if let Some(last_msg) = panel.messages.last_mut() {
                            if last_msg.role == "AI" {
                                last_msg.content = result.clone();
                            }
                        }
                        panel
                            .conversation
                            .add_message(aemacs_ai::Message::assistant(result));
                    }
                    AgentEvent::Error(e) => {
                        if let Some(last_msg) = panel.messages.last_mut() {
                            if last_msg.role == "AI" {
                                last_msg.role = "System".to_string();
                                last_msg.content = format!("❌ AI Error: {}", e);
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

    fn render_agent_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let personas = futures::executor::block_on(self.persona_registry.list_personas());
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
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    personas.into_iter().enumerate().map(|(i, name)| {
                        let is_selected = active_persona.as_ref() == Some(&name);
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
                                this.active_persona_name = Some(name_clone.clone());
                                cx.notify();
                            }))
                            .child(name.to_uppercase())
                    }),
                ),
            )
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
                    .child("MODEL"),
            )
            .child(
                div().flex().flex_wrap().gap(px(4.0)).children(
                    aemacs_ai::models::MODELS
                        .iter()
                        .enumerate()
                        .map(|(i, model)| {
                            let is_selected = i == selected_index;
                            div()
                                .id(("model", i))
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
                                    let model_name = aemacs_ai::models::MODELS[i].name;
                                    this.conversation.set_model(model_name);
                                    // Reset context to max for new model
                                    this.selected_context =
                                        aemacs_ai::models::MODELS[i].max_context;
                                    this.conversation.set_context_window(this.selected_context);
                                    cx.notify();
                                }))
                                .child(model.label)
                        }),
                ),
            )
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

    fn handle_input_keydown(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let mode = self.input_editor.read(cx).mode;

        if keystroke.key == "enter" {
            if mode == Mode::Normal {
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
                self.messages.push(ChatMessage {
                    role: "System".to_string(),
                    content: format!("⚠️ Unknown agent: /{}. Type a valid specialist name.", agent_name),
                });
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
            self.messages.push(ChatMessage {
                role: "User".to_string(),
                content: text.clone(),
            });
        } else if switched {
            let persona_display = self.active_persona_name.as_deref().unwrap_or("UNKNOWN").to_uppercase();
            self.messages.push(ChatMessage {
                role: "System".to_string(),
                content: format!("Agent switched to: {}", persona_display),
            });
        } else {
            return;
        }

        // 2. Clear Input
        self.input_editor.update(cx, |editor, _| {
            editor.buffer.content = ropey::Rope::new();
            editor.mode = Mode::Normal;
        });

        if text.trim().is_empty() {
            cx.notify();
            return;
        }

        // 3. Prepare AI Message Placeholder
        self.messages.push(ChatMessage {
            role: "AI".to_string(),
            content: "".to_string(),
        });

        // 4. Update persistent conversation
        if let Some(persona_name) = &self.active_persona_name {
            if let Some(persona) =
                futures::executor::block_on(self.persona_registry.get_persona(persona_name))
            {
                self.conversation.set_persona(persona);
            }
        } else {
            self.conversation.clear_persona();
        }
        self.conversation.add_message(aemacs_ai::Message::user(text));

        cx.notify();

        self.trigger_ai_response(cx);
    }
}

impl Render for AiPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = rgb(0x21252b);
        let input_bg = rgb(0x282c34);
        let text_color = rgb(0xabb2bf);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg)
            .border_l_1()
            .border_color(rgb(0x181a1f))
            .child(
                div()
                    .p(px(10.0))
                    .border_b_1()
                    .border_color(rgb(0x181a1f))
                    .flex()
                    .flex_col()
                    .gap_y(px(8.0))
                    .child(self.render_agent_selector(cx))
                    .child(self.render_model_selector(cx))
                    .child(self.render_context_selector(cx))
                    .child(self.render_vram_estimate()),
            )
            .when(!self.tasks.is_empty(), |this| {
                this.child(
                    div()
                        .flex_grow()
                        .max_h(px(300.0))
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
                    .flex_1()
                    .flex_col()
                    .gap_y(px(10.0))
                    .p(px(10.0))
                    .id("message_area")
                    .overflow_y_scroll()
                    .children(self.messages.iter().map(|msg| {
                        let is_user = msg.role == "User";
                        let is_error = msg.content.starts_with("❌");
                        
                        div()
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
                                        text_color
                                    })
                                    .child(msg.content.clone()),
                            )
                    })),
            )
            .child(
                // Input Area
                div()
                    .h(px(100.0))
                    .bg(input_bg)
                    .border_t_1()
                    .border_color(rgb(0x181a1f))
                    .p(px(8.0))
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(Self::handle_input_keydown))
                    .child({
                        let editor = self.input_editor.read(cx);
                        render_editor_view(editor)
                    }),
            )
    }
}
