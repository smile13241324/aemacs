use aemacs_ai::Conversation;
use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::mcp::{ToolHost, ToolRegistry};
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
    ) -> Entity<Self> {
        cx.new(|cx| {
            let input_editor = cx.new(|_cx| Editor::new());
            let focus_handle = cx.focus_handle();

            // Default to local Ollama instance for now
            let backend = OpenAICompatibleBackend::new("http://localhost:11434/v1", None);

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
            }
        })
    }

    /// Updates the local task list (ACO-034)
    pub fn update_tasks(&mut self, tasks: Vec<aemacs_core::task::Task>, cx: &mut Context<Self>) {
        self.tasks = tasks;
        cx.notify();
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
                                    // Reset context to max for new model
                                    this.selected_context =
                                        aemacs_ai::models::MODELS[i].max_context;
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
        let text = self.input_editor.read(cx).buffer.text();
        if text.trim().is_empty() {
            return;
        }

        // 1. Add User Message
        self.messages.push(ChatMessage {
            role: "User".to_string(),
            content: text.clone(),
        });

        // 2. Clear Input
        self.input_editor.update(cx, |editor, _| {
            editor.buffer.content = ropey::Rope::new();
            editor.mode = Mode::Normal;
        });

        // 3. Prepare AI Message Placeholder
        self.messages.push(ChatMessage {
            role: "AI".to_string(),
            content: "".to_string(),
        });

        cx.notify();

        // 4. Setup Agent Loop
        let model_name = aemacs_ai::models::MODELS[self.selected_model_index].name;
        let conversation = Conversation::new(model_name)
            .with_system("You are a helpful assistant embedded in Æmacs.")
            .with_context_window(self.selected_context)
            .with_user(text);

        let registry = self.registry.clone();

        let host = GuiHost {
            request_tx: self.host_tx.clone(),
        };

        let backend = self.backend.clone();

        // 5. Spawn Agent Task
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
                                last_msg.content = result;
                            }
                        }
                    }
                    AgentEvent::Error(e) => {
                        if let Some(last_msg) = panel.messages.last_mut() {
                            last_msg.content.push_str(&format!("\n[Error: {}]", e));
                        }
                    }
                }
                cx.notify();
            },
        )
        .detach();
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
                                    .text_color(text_color)
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
