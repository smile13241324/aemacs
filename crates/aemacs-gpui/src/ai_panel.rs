use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::loader::load_file;
use aemacs_ai::mcp::{ToolHost, ToolRegistry, validate_path};
use aemacs_ai::{Conversation, PersonaRegistry};
use aemacs_core::{Editor, command::Command, mode::Mode};
use async_trait::async_trait;
use gpui::prelude::*;
use gpui::{App, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Window, div, px, rgb};
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
    pub proactive_mode: bool,
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
        bus: aemacs_core::bus::EventBus,
    ) -> Entity<Self> {
        let panel = cx.new(|cx| {
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
                proactive_mode: true,
            }
        });

        // ACO-026: Spawn the Triage Router Listener
        let panel_weak = panel.downgrade();
        let rx = bus.rx.clone();

        cx.spawn(|cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                while let Ok(event) = rx.recv().await {
                    match event {
                        aemacs_core::bus::SystemEvent::Signal { source, event_type, payload } => {
                            let _ = cx.update(|app: &mut gpui::App| {
                                if let Some(panel) = panel_weak.upgrade() {
                                    panel.update(app, |this, cx| {
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

                                            this.messages.push(ChatMessage {
                                                role: "System".to_string(),
                                                content: "👀 I noticed you changed a file. Let me look...".to_string(),
                                            });

                                            this.conversation.add_message(aemacs_ai::Message::user(observation));
                                            
                                            // Ensure we have an active persona, default to marjin for refactoring
                                            if this.active_persona_name.is_none() {
                                                this.active_persona_name = Some("marjin".to_string());
                                                if let Some(persona) = futures::executor::block_on(this.persona_registry.get_persona("marjin")) {
                                                    this.conversation.set_persona(persona);
                                                }
                                            }

                                            cx.notify();
                                            this.trigger_ai_response(cx);
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
            .child(div().flex().flex_wrap().gap(px(4.0)).children(
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
            self.messages.push(ChatMessage {
                role: "System".to_string(),
                content: "❌ Multiple statutes detected! You can only load one profile (@) at a time. Use @@ for attaching multiple source files.".to_string(),
            });
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
                                self.messages.push(ChatMessage {
                                    role: "System".to_string(),
                                    content: format!("📖 Profile loaded: {}", path_str),
                                });
                                // Strip the tag from the final message text
                                let start = mat.start();
                                let end = mat.end();
                                text.replace_range(start..end, "");
                                text = text.trim().to_string();
                            } else {
                                self.messages.push(ChatMessage {
                                    role: "System".to_string(),
                                    content: format!(
                                        "⚠️ Profile at '{}' is not a text file.",
                                        path_str
                                    ),
                                });
                            }
                        }
                        Err(e) => {
                            self.messages.push(ChatMessage {
                                role: "System".to_string(),
                                content: format!("⚠️ Failed to load profile: {} ({})", path_str, e),
                            });
                        }
                    }
                }
                Err(e) => {
                    self.messages.push(ChatMessage {
                        role: "System".to_string(),
                        content: format!("⚠️ Invalid profile path: {} ({})", path_str, e),
                    });
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
                                self.messages.push(ChatMessage {
                                    role: "System".to_string(),
                                    content: format!(
                                        "⚠️ Material at '{}' is not a text file.",
                                        path_str
                                    ),
                                });
                            }
                        }
                        Err(e) => {
                            self.messages.push(ChatMessage {
                                role: "System".to_string(),
                                content: format!(
                                    "⚠️ Failed to load material: {} ({})",
                                    path_str, e
                                ),
                            });
                        }
                    }
                }
                Err(e) => {
                    self.messages.push(ChatMessage {
                        role: "System".to_string(),
                        content: format!("⚠️ Invalid material path: {} ({})", path_str, e),
                    });
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
                self.messages.push(ChatMessage {
                    role: "System".to_string(),
                    content: format!(
                        "⚠️ Unknown agent: /{}. Type a valid specialist name.",
                        agent_name
                    ),
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
            let persona_display = self
                .active_persona_name
                .as_deref()
                .unwrap_or("UNKNOWN")
                .to_uppercase();
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
        self.conversation
            .add_message(aemacs_ai::Message::user(text));

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
                                    .text_color(if is_error { rgb(0xe06c75) } else { text_color })
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

#[cfg(test)]
mod tests {
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
                .filter(|m| {
                    let start = m.start();
                    if start > 0 && text.as_bytes()[start - 1] == b'@' {
                        return false;
                    }
                    true
                })
                .map(|m| (m.start(), m.end()))
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
            .filter(|m| {
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
            .map(|m| (m.start(), m.end()))
            .collect();
        all_tags.extend(material_matches.iter().map(|m| (m.start(), m.end())));
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
}
