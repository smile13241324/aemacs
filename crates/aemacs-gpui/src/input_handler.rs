use aemacs_core::{command::Command, mode::Mode};
use gpui::Keystroke;

/// Maps a GPUI keystroke to an abstract Editor Command.
///
/// This handles standard Vim-like navigation and mode switching.
/// Note: 'Enter' is explicitly NOT handled here, as it is context-dependent
/// (e.g. Newline in Editor vs Send in Chat).
pub fn resolve_key_command(keystroke: &Keystroke, mode: Mode) -> Option<Command> {
    if keystroke.key == "backspace" {
        return Some(Command::Backspace);
    }
    if keystroke.key == "delete" {
        return Some(Command::Delete);
    }
    if keystroke.key == "left" {
        return Some(Command::MoveLeft);
    }
    if keystroke.key == "right" {
        return Some(Command::MoveRight);
    }
    if keystroke.key == "up" {
        return Some(Command::MoveUp);
    }
    if keystroke.key == "down" {
        return Some(Command::MoveDown);
    }
    if keystroke.key == "escape" {
        return Some(Command::EnterMode(Mode::Normal));
    }

    // Text Input & Mode Switching
    if let Some(text) = &keystroke.key_char {
        // Filter out control sequences
        if !keystroke.modifiers.platform
            && !keystroke.modifiers.control
            && !keystroke.modifiers.function
        {
            if mode == Mode::Normal {
                if text == "i" {
                    return Some(Command::EnterMode(Mode::Insert));
                }
                // Other Normal mode keys would go here (h,j,k,l, etc)
                // For now, return None
                return None;
            } else {
                // Insert Mode: Type text
                return Some(Command::Insert(text.clone()));
            }
        }
    }

    None
}
