use aemacs_core::{Editor, command::Command, runtime};
use anyhow::Result;
use gpui::prelude::*;
use gpui::{
    App,
    Application,
    Bounds,
    Context,
    Entity,
    FocusHandle,
    IntoElement,
    KeyDownEvent, // Keystroke entfernt, da unused
    Window,
    WindowBounds,
    WindowOptions,
    div,
    px,
    rgb,
    size,
};
use log::info;

pub fn init() -> Result<()> {
    info!("🎨 [GPUI] Initializing Graphics Engine...");
    Ok(())
}

pub struct Workspace {
    editor: Entity<Editor>,
    focus_handle: FocusHandle,
}

impl Workspace {
    pub fn build(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let editor = cx.new(|_cx| Editor::new());
            let focus_handle = cx.focus_handle();

            Workspace {
                editor,
                focus_handle,
            }
        })
    }

    fn render_welcome(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .bg(rgb(0x282c34))
            .child(
                div()
                    .text_xl()
                    .text_color(rgb(0xabb2bf))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("Welcome to Æmacs"),
            )
            .child(
                div()
                    .mt(px(10.0))
                    .text_sm()
                    .text_color(rgb(0x5c6370))
                    .child("The Iron Core is ready. Start typing..."),
            )
    }

    fn render_editor(&self, content: String) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .bg(rgb(0x282c34))
            .text_color(rgb(0xabb2bf))
            .p(px(16.0))
            .child(content)
    }

    fn handle_keydown(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;

        let command: Option<Command> = if keystroke.key == "backspace" {
            Some(Command::Backspace)
        } else if keystroke.key == "delete" {
            Some(Command::Delete)
        } else if keystroke.key == "enter" {
            Some(Command::InsertNewline)
        } else if keystroke.key == "left" {
            Some(Command::MoveLeft)
        } else if keystroke.key == "right" {
            Some(Command::MoveRight)
        } else if keystroke.key == "up" {
            Some(Command::MoveUp)
        } else if keystroke.key == "down" {
            Some(Command::MoveDown)
        } else if let Some(text) = &keystroke.key_char {
            if !keystroke.modifiers.platform
                && !keystroke.modifiers.control
                && !keystroke.modifiers.function
            {
                Some(Command::Insert(text.clone()))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(cmd) = command {
            self.editor.update(cx, |editor, _cx| {
                if let Err(e) = editor.run(cmd) {
                    log::error!("Editor command failed: {}", e);
                }
            });
            cx.notify();
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let is_empty = editor.buffer.len_chars() == 0;
        let content = editor.buffer.text();

        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_keydown))
            .size_full()
            .bg(rgb(0x282c34))
            .child(if is_empty {
                self.render_welcome().into_any_element()
            } else {
                self.render_editor(content).into_any_element()
            })
    }
}

pub fn run_app() {
    Application::new().run(|cx: &mut App| {
        runtime::init(cx);
        info!("🚀 [RUNTIME] Tokio Bridge active.");

        let bounds = Bounds::centered(None, size(px(800.), px(600.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Maximized(bounds)),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("Æmacs".into()),
                appears_transparent: true,
                traffic_light_position: Some(gpui::Point::new(px(8.0), px(8.0))),
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let view = Workspace::build(cx);

            // FIX: Borrow Checker Appeasement 🕯️
            // 1. We get the handle and clone it (Immutable borrow of `view` via `cx` ends here)
            let focus_handle = view.read(cx).focus_handle.clone();

            // 2. Now `cx` is free again for a Mutable Borrow
            window.focus(&focus_handle, cx);

            view
        })
        .unwrap();

        cx.activate(true);
    });
}
