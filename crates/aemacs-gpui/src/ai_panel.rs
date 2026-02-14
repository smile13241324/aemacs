use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::{AIBackend, AIRequest, Conversation, Message, Role}; // Add Conversation
use aemacs_core::{Editor, command::Command, mode::Mode};
use gpui::prelude::*;
use gpui::{App, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Window, div, px, rgb};

use crate::ai_utils::{StreamEvent, spawn_chat_stream};
use crate::editor_view::render_editor_view;
use crate::input_handler::resolve_key_command;

pub struct AiPanel {
    pub input_editor: Entity<Editor>,
    pub focus_handle: FocusHandle,
    messages: Vec<ChatMessage>,
    backend: OpenAICompatibleBackend,
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
                    // .overflow_y(Overflow::Scroll) // Still waiting for fix
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
