use aemacs_core::{Editor, command::Command, mode::Mode, runtime};
use anyhow::Result;
use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Window,
    WindowBounds, WindowOptions, div, px, rgb, size,
};
use log::info;
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod ai_panel;
use ai_panel::AiPanel;

mod editor_view;
use editor_view::render_editor_view;

mod input_handler;
use input_handler::resolve_key_command;

mod ai_utils;

pub fn init() -> Result<()> {
    info!("🎨 [GPUI] Initializing Graphics Engine...");
    Ok(())
}

pub struct Workspace {
    editor: Entity<Editor>,
    ai_panel: Entity<AiPanel>,
    focus_handle: FocusHandle,
    last_key: Option<(String, Instant)>,
    show_ai: bool,
}

impl Workspace {
    pub fn build(cx: &mut App, file_path: Option<PathBuf>) -> Entity<Self> {
        cx.new(|cx| {
            let editor = cx.new(|_cx| {
                if let Some(path) = file_path {
                    match Editor::from_file(path) {
                        Ok(ed) => {
                            log::info!("📂 [Workspace] File loaded successfully.");
                            ed
                        }
                        Err(e) => {
                            log::error!("⚠️ [Workspace] Failed to load file: {}", e);
                            Editor::new() // Fallback: Leerer Buffer
                        }
                    }
                } else {
                    Editor::new()
                }
            });

            let ai_panel = AiPanel::new(cx);
            let focus_handle = cx.focus_handle();

            Workspace {
                editor,
                ai_panel,
                focus_handle,
                last_key: None,
                show_ai: true, // Default to visible for testing
            }
        })
    }

    fn render_welcome(&self) -> impl IntoElement {
        let bg_color = rgb(0x282c34);
        let accent_color = rgb(0xbd93f9);
        let key_hint_bg = rgb(0x3e4451);

        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .bg(bg_color)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_y(px(20.0))
                    .child(div().text_size(px(64.0)).child("🦀"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_x(px(10.0))
                            .child(
                                div()
                                    .text_size(px(48.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xe0e0e0))
                                    .child("Æmacs"),
                            )
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded_md()
                                    .bg(accent_color)
                                    .text_color(rgb(0x282c34))
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("PRE-ALPHA"),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(40.0))
                    .flex()
                    .gap_x(px(20.0))
                    .text_sm()
                    .text_color(rgb(0xabb2bf))
                    .child(self.render_key_hint("Type", "Insert Text", key_hint_bg.into()))
                    .child(self.render_key_hint("i", "Insert Mode", key_hint_bg.into()))
                    .child(self.render_key_hint("ESC", "Normal Mode", key_hint_bg.into())),
            )
    }

    fn render_key_hint(&self, key: &str, desc: &str, bg: gpui::Hsla) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_x(px(8.0))
            .child(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .bg(bg)
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0x1e222a))
                    .font_family("Fira Code")
                    .child(key.to_string()),
            )
            .child(desc.to_string())
    }

    fn handle_keydown(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        let current_time = Instant::now();

        if self.editor.read(cx).mode == Mode::Insert {
            if let Some((last_char, last_time)) = &self.last_key {
                if last_char == "f" && keystroke.key == "d" {
                    if current_time.duration_since(*last_time) < Duration::from_millis(250) {
                        self.editor.update(cx, |editor, _| editor.backspace());

                        self.editor.update(cx, |editor, _| {
                            if let Err(e) = editor.run(Command::EnterMode(Mode::Normal)) {
                                log::error!("Failed to switch mode: {}", e);
                            }
                        });

                        self.last_key = None;
                        cx.notify();
                        return;
                    }
                }
            }
        }

        let current_mode = self.editor.read(cx).mode;

        // 1. Special Handling: Enter -> Newline
        let command = if keystroke.key == "enter" {
            Some(Command::InsertNewline)
        } else {
            // 2. Delegate to shared logic
            resolve_key_command(keystroke, current_mode)
        };

        if let Some(text) = &keystroke.key_char {
            // Update last key for special combos (like fd)
            // Only if it was a text input
            self.last_key = Some((text.clone(), current_time));
        }

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
        let (line, col) = editor.cursor_position();
        let mode_name = format!("{:?}", editor.mode).to_uppercase();

        let bg_color = rgb(0x282c34);
        let gutter_bg = rgb(0x21252b);
        let gutter_text = rgb(0x495162);
        let status_bg = rgb(0x21252b);
        let status_fg = rgb(0x9da5b4);

        let mode_color = match editor.mode {
            Mode::Normal => rgb(0xd19a66), // Orange
            Mode::Insert => rgb(0x98c379), // Green
            Mode::Visual => rgb(0x3e4451), // Visual Grey
        };

        // Main Editor Area
        let main_view = div()
            .flex()
            .flex_1()
            .flex_row()
            .child(
                div()
                    .w(px(50.0))
                    .bg(gutter_bg)
                    .flex()
                    .flex_col()
                    .items_end()
                    .pr(px(8.0))
                    .pt(px(16.0))
                    .text_size(px(14.0))
                    .font_family("Fira Code")
                    .text_color(gutter_text)
                    .children(if is_empty {
                        vec![div().child("~").into_any_element()]
                    } else {
                        (1..=editor.line_count())
                            .map(|i| div().child(i.to_string()).h(px(20.0)).into_any_element())
                            .collect()
                    }),
            )
            .child(if is_empty {
                self.render_welcome().into_any_element()
            } else {
                render_editor_view(editor).into_any_element()
            });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg_color)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_keydown))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .flex_row()
                    .child(main_view)
                    .when(self.show_ai, |this| {
                        this.child(
                            div()
                                .w(px(350.0)) // AI Panel Width
                                .border_l_1()
                                .border_color(rgb(0x181a1f))
                                .child(self.ai_panel.clone()),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(30.0))
                    .bg(status_bg)
                    .border_t_1()
                    .border_color(rgb(0x181a1f))
                    .flex()
                    .items_center()
                    .px(px(10.0))
                    .gap_x(px(10.0))
                    .text_size(px(12.0))
                    .font_family("Fira Code")
                    .child(
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .rounded_sm()
                            .bg(mode_color)
                            .text_color(rgb(0x282c34))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(mode_name),
                    )
                    .child(div().text_color(status_fg).child("buffer-1.rs"))
                    .child(div().flex_1())
                    .child(div().text_color(rgb(0xff5555)).child(if self.show_ai {
                        "AI: ON"
                    } else {
                        "AI: OFF"
                    }))
                    .child(
                        div()
                            .text_color(status_fg)
                            .child(format!("Ln {}, Col {}", line, col)),
                    )
                    .child(div().text_color(gutter_text).child("UTF-8")),
            )
    }
}

pub fn run_app(file_to_open: Option<PathBuf>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let handle = runtime.handle().clone();

    Application::new().run(move |cx: &mut App| {
        runtime::init_from_handle(cx, handle);
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
            let view = Workspace::build(cx, file_to_open);
            let focus_handle = view.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
            view
        })
        .unwrap();

        cx.activate(true);
    });
}
