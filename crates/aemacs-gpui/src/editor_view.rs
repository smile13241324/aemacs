use aemacs_core::{Editor, mode::Mode};
use gpui::prelude::*;
use gpui::{IntoElement, ListState, div, list, px, rgb, rgba};
/// Renders the core editor text area, including line gutters, text content, and cursor styling.
/// It uses a high-performance virtualized list to efficiently display extremely large buffers.
///
/// # Parameters
/// * `editor`: A reference to the active Editor engine state.
/// * `list_state`: The GPUI `ListState` controlling the scroll position and item rendering.
/// * `wrap`: Whether text lines should soft-wrap at the view boundary.
pub(crate) fn render_editor_view(editor: &Editor, list_state: ListState, wrap: bool) -> impl IntoElement {
    let theme_bg = rgb(0x282c34);
    let text_color = rgb(0xabb2bf);
    let cursor_pos = editor.cursor_position();
    let cursor_line_idx = cursor_pos.0.saturating_sub(1);
    let cursor_col_idx = cursor_pos.1.saturating_sub(1);

    let (cursor_bg, is_block, _has_shadow) = match editor.mode {
        Mode::Normal => (rgb(0xd19a66), true, false),
        Mode::Insert => (rgb(0x98c379), false, false),
        Mode::Visual => (rgba(0x3e445180), true, true),
    };

    let buffer_clone = editor.buffer.clone();

    // GPUI calls this closure only for items currently in the viewport.
    let list_element = list(list_state, move |line_idx, _window, _cx| {
        let line_text = buffer_clone.content.line(line_idx).to_string();

        if wrap {
            let mut display_text = line_text;
            if line_idx == cursor_line_idx {
                let chars: Vec<char> = display_text.chars().collect();
                let len = chars.len();
                let safe_col = std::cmp::min(cursor_col_idx, len);

                let mut new_chars = chars;
                if is_block {
                    new_chars.insert(safe_col, '█');
                } else {
                    new_chars.insert(safe_col, '|');
                }
                display_text = new_chars.into_iter().collect();
            }

            div()
                .w_full()
                .whitespace_normal()
                .font_family("Fira Code")
                .text_size(px(14.0))
                .text_color(text_color)
                .child(display_text)
                .into_any_element()
        } else if line_idx == cursor_line_idx {
            let chars: Vec<char> = line_text.chars().collect();
            let len = chars.len();
            let safe_col = std::cmp::min(cursor_col_idx, len);

            let pre_text: String = chars.iter().take(safe_col).collect();
            let cursor_char_str =
                if safe_col < len && chars[safe_col] != ' ' && chars[safe_col] != '\n' {
                    chars[safe_col].to_string()
                } else {
                    " ".to_string()
                };
            let post_text: String = chars.iter().skip(safe_col + 1).collect();

            div()
                .flex()
                .flex_row()
                .h(px(20.0))
                .whitespace_nowrap()
                .font_family("Fira Code")
                .text_size(px(14.0))
                .text_color(text_color)
                .child(pre_text)
                .child(
                    div()
                        .child(cursor_char_str)
                        .bg(if is_block {
                            cursor_bg
                        } else {
                            rgba(0x00000000)
                        })
                        .text_color(if is_block { rgb(0x282c34) } else { text_color })
                        .when(!is_block, |this| this.border_l_2().border_color(cursor_bg)),
                )
                .child(post_text)
                .into_any_element()
        } else {
            div()
                .h(px(20.0))
                .whitespace_nowrap()
                .font_family("Fira Code")
                .text_size(px(14.0))
                .text_color(text_color)
                .child(line_text)
                .into_any_element()
        }
    })
    .w_full()
    .h_full();

    div()
        .size_full()
        .flex_1() // Ensure the outer div takes up the remaining space
        .bg(theme_bg)
        .when(!wrap, |this| this.pl(px(16.0)).pt(px(16.0)))
        .child(list_element)
}
