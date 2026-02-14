use aemacs_core::{Editor, mode::Mode};
use gpui::prelude::*;
use gpui::{IntoElement, div, px, rgb, rgba};

pub fn render_editor_view(editor: &Editor) -> impl IntoElement {
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
                let cursor_char_str = if safe_col < len && chars[safe_col] != ' ' {
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
