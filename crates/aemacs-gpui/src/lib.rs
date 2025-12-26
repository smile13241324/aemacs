use aemacs_core::{Editor, command::Command, mode::Mode, runtime};
use anyhow::Result;
use gpui::prelude::*;
use gpui::{
    App, Application, Bounds, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, Window,
    WindowBounds, WindowOptions, div, px, rgb, rgba, size,
};
use log::info;
use std::time::{Duration, Instant};

pub fn init() -> Result<()> {
    info!("🎨 [GPUI] Initializing Graphics Engine...");
    Ok(())
}

pub struct Workspace {
    editor: Entity<Editor>,
    focus_handle: FocusHandle,
    last_key: Option<(String, Instant)>,
}

impl Workspace {
    pub fn build(cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let editor = cx.new(|_cx| Editor::new());
            let focus_handle = cx.focus_handle();

            Workspace {
                editor,
                focus_handle,
                last_key: None,
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

    fn render_editor(&self, editor: &Editor, _cx: &App) -> impl IntoElement {
        let theme_bg = rgb(0x282c34);
        let text_color = rgb(0xabb2bf);
        let cursor_pos = editor.cursor_position();
        let cursor_line_idx = cursor_pos.0 - 1;
        let cursor_col_idx = cursor_pos.1 - 1;

        let (cursor_bg, is_block, has_shadow) = match editor.mode {
            Mode::Normal => (rgb(0xd19a66), true, false),  // Orange
            Mode::Insert => (rgb(0x98c379), false, false), // Green
            Mode::Visual => (rgba(0x3e445180), true, true), // Grey Shadow
        };

        let line_count = editor.line_count();

        let lines_view = div()
            .flex()
            .flex_col()
            .size_full()
            .font_family("Fira Code")
            .text_size(px(14.0))
            .text_color(text_color)
            .children((0..line_count).map(|line_idx| {
                let line_text = editor.buffer.content.line(line_idx).to_string();

                if line_idx == cursor_line_idx {
                    let chars: Vec<char> = line_text.chars().collect();
                    let len = chars.len();
                    let safe_col = std::cmp::min(cursor_col_idx, len);

                    let pre_text: String = chars.iter().take(safe_col).collect();
                    let cursor_char_str = if safe_col < len && chars[safe_col] != '\n' {
                        chars[safe_col].to_string()
                    } else {
                        " ".to_string()
                    };
                    let post_text: String = chars.iter().skip(safe_col + 1).collect();

                    div()
                        .h(px(20.0))
                        .flex()
                        .flex_row()
                        .whitespace_nowrap()
                        .child(pre_text)
                        .child(
                            div()
                                .child(cursor_char_str)
                                .text_color(if is_block && !has_shadow {
                                    rgb(0x282c34)
                                } else {
                                    text_color
                                })
                                .bg(if is_block {
                                    cursor_bg
                                } else {
                                    rgba(0x00000000)
                                })
                                .when(has_shadow, |this| this.shadow_sm())
                                .when(!is_block, |this| this.border_l_2().border_color(cursor_bg)),
                        )
                        .child(post_text)
                        .into_any_element()
                } else {
                    div()
                        .h(px(20.0))
                        .whitespace_nowrap()
                        .child(line_text)
                        .into_any_element()
                }
            }));

        div()
            .flex()
            .size_full()
            .bg(theme_bg)
            .pl(px(16.0))
            .pt(px(16.0))
            .child(lines_view)
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
        } else if keystroke.key == "escape" {
            Some(Command::EnterMode(Mode::Normal))
        } else if let Some(text) = &keystroke.key_char {
            if !keystroke.modifiers.platform
                && !keystroke.modifiers.control
                && !keystroke.modifiers.function
            {
                let current_mode = self.editor.read(cx).mode;
                if current_mode == Mode::Normal && text == "i" {
                    Some(Command::EnterMode(Mode::Insert))
                } else if current_mode == Mode::Normal {
                    None
                } else {
                    self.last_key = Some((text.clone(), current_time));
                    Some(Command::Insert(text.clone()))
                }
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
                                    .map(|i| {
                                        div().child(i.to_string()).h(px(20.0)).into_any_element()
                                    })
                                    .collect()
                            }),
                    )
                    .child(if is_empty {
                        self.render_welcome().into_any_element()
                    } else {
                        self.render_editor(editor, cx).into_any_element()
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
                    .child(
                        div()
                            .text_color(status_fg)
                            .child(format!("Ln {}, Col {}", line, col)),
                    )
                    .child(div().text_color(gutter_text).child("UTF-8")),
            )
    }
}

pub fn run_app() {
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
            let view = Workspace::build(cx);
            let focus_handle = view.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
            view
        })
        .unwrap();

        cx.activate(true);
    });
}
