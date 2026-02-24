use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::{Conversation, mcp::ToolHost};
use aemacs_core::{Editor, command::Command, mode::Mode};
use async_trait::async_trait;
use gpui::{
    App, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, PromptLevel, WeakEntity, Window,
    div, px, rgb,
};
use gpui::{AppContext, prelude::*};

use crate::ai_utils::{StreamEvent, spawn_chat_stream};
use crate::editor_view::render_editor_view;
use crate::input_handler::resolve_key_command;

// --- Modal Bridge (ACO-023) ---

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
    request_tx: async_channel::Sender<HostRequest>,
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
}

struct ChatMessage {
    role: String,
    content: String,
}

impl AiPanel {
    pub fn new(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let input_editor = cx.new(|_cx| Editor::new());
            let focus_handle = cx.focus_handle();
            let (tx, rx) = async_channel::unbounded::<HostRequest>();

            // Spawn the Listener for HostRequests (ACO-023 Bridge)
            cx.spawn(
                |_this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| async move {
                    while let Ok(request) = rx.recv().await {
                        match request {
                            HostRequest::Approval {
                                description,
                                responder,
                            } => {
                                let prompt_future = cx.prompt(
                                    PromptLevel::Warning,
                                    "AI Permission Request",
                                    Some(&description),
                                    &["Approve", "Deny"],
                                );

                                let answer = prompt_future.await.unwrap_or(1usize); // Default to Deny
                                let _ = responder.send(answer == 0);
                            }
                            HostRequest::UserPrompt {
                                question,
                                responder,
                            } => {
                                let prompt_future = cx.prompt(
                                    PromptLevel::Info,
                                    "AI Question",
                                    Some(&question),
                                    &["Acknowledge"],
                                );

                                let _ = prompt_future.await;
                                let _ = responder.send("Acknowledged".to_string());
                            }
                        }
                    }
                },
            )
            .detach();

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
                host_tx: tx,
            }
        })
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

        // 4. Construct Request via Builder
        let request = Conversation::new("mistral")
            .with_system("You are a helpful assistant embedded in Æmacs.")
            .with_user(text)
            .build();

        let backend = self.backend.clone();

        // 5. Fire and Forget Stream
        // We pass a closure that handles updates
        spawn_chat_stream(cx, backend, request, |panel, cx, event| {
            match event {
                StreamEvent::Chunk(token) => {
                    if let Some(last_msg) = panel.messages.last_mut() {
                        if last_msg.role == "AI" {
                            last_msg.content.push_str(&token);
                        }
                    }
                }
                StreamEvent::Error(e) => {
                    if let Some(last_msg) = panel.messages.last_mut() {
                        last_msg.content.push_str(&format!("\n[Error: {}]", e));
                    }
                }
                StreamEvent::Done => {
                    // Optional cleanup
                }
            }
            cx.notify();
        })
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
