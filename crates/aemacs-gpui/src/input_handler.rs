use aemacs_core::{command::Command, mode::Mode};
use gpui::Keystroke;
/// Translates a raw GPUI keystroke into a high-level editor command.
/// This function implements the primary modal editing logic, distinguishing between
/// movement, editing, and mode switching based on the current state of the editor.
///
/// Note: Standard keys like 'Enter' are handled at the view level to allow for
/// context-sensitive behavior (e.g., newline in editor vs send in chat).
pub(crate) fn resolve_key_command(keystroke: &Keystroke, mode: Mode) -> Option<Command> {
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
            }
            // Insert Mode: Type text
            return Some(Command::Insert(text.clone()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use gpui::Modifiers;

    use super::*;

    #[test]
    fn test_input_handler_handshake_quest() {
        // --- 1. Mode Switching ---
        let i_key = Keystroke {
            modifiers: Modifiers::default(),
            key: "i".to_string(),
            key_char: Some("i".to_string()),
        };
        assert_eq!(
            resolve_key_command(&i_key, Mode::Normal),
            Some(Command::EnterMode(Mode::Insert))
        );

        let esc_key = Keystroke {
            modifiers: Modifiers::default(),
            key: "escape".to_string(),
            key_char: None,
        };
        assert_eq!(
            resolve_key_command(&esc_key, Mode::Insert),
            Some(Command::EnterMode(Mode::Normal))
        );

        // --- 2. Navigation ---
        let left_key =
            Keystroke { modifiers: Modifiers::default(), key: "left".to_string(), key_char: None };
        assert_eq!(resolve_key_command(&left_key, Mode::Normal), Some(Command::MoveLeft));

        // --- 3. Text Insertion ---
        let a_key = Keystroke {
            modifiers: Modifiers::default(),
            key: "a".to_string(),
            key_char: Some("a".to_string()),
        };
        // In Normal mode, 'a' doesn't have a command yet (it's not 'i')
        assert_eq!(resolve_key_command(&a_key, Mode::Normal), None);
        // In Insert mode, 'a' should be inserted
        assert_eq!(
            resolve_key_command(&a_key, Mode::Insert),
            Some(Command::Insert("a".to_string()))
        );

        // --- 4. Modifiers (Safety Check) ---
        let ctrl_i = Keystroke {
            modifiers: Modifiers { control: true, ..Default::default() },
            key: "i".to_string(),
            key_char: Some("i".to_string()),
        };
        // Ctrl-i should NOT trigger Insert mode
        assert_eq!(resolve_key_command(&ctrl_i, Mode::Normal), None);
    }
}
