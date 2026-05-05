use std::collections::HashMap;

use crate::{command::Command, mode::Mode};

/// Stores the mapping between keystrokes and commands for each mode.
#[derive(Debug, Default)]
pub struct KeymapRegistry {
    maps: HashMap<(Mode, String), Command>,
}

impl KeymapRegistry {
    /// Initializes a new `KeymapRegistry` populated with default Vim-like bindings.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_defaults();
        registry
    }

    /// Registers the standard Vim-like bindings for Normal and Insert modes.
    fn register_defaults(&mut self) {
        // --- NORMAL MODE ---
        // Navigation
        self.bind(Mode::Normal, "h", Command::MoveLeft);
        self.bind(Mode::Normal, "j", Command::MoveDown);
        self.bind(Mode::Normal, "k", Command::MoveUp);
        self.bind(Mode::Normal, "l", Command::MoveRight);

        // Editing / Delete
        self.bind(Mode::Normal, "x", Command::Delete);

        // Mode Switching
        self.bind(Mode::Normal, "i", Command::EnterMode(Mode::Insert));

        // Undo/Redo
        self.bind(Mode::Normal, "u", Command::Undo);
        self.bind(Mode::Normal, "ctrl-r", Command::Redo);

        // --- INSERT MODE ---
        // Escape to Normal
        self.bind(Mode::Insert, "esc", Command::EnterMode(Mode::Normal));
        self.bind(Mode::Insert, "backspace", Command::Backspace);
        self.bind(Mode::Insert, "enter", Command::InsertNewline);
    }

    /// Maps a specific key sequence to an architectural command within a given mode.
    pub fn bind(&mut self, mode: Mode, key: &str, cmd: Command) {
        self.maps.insert((mode, key.to_string()), cmd);
    }

    /// Attempts to translate a raw input string into a structured editor command based on the current mode.
    ///
    /// Returns `Some(Command)` if a match is found in the registry.
    /// Returns `None` if no binding exists, which the input handler typically interprets as a signal
    /// to insert the character literally (if in Insert mode) or ignore it (if in Normal mode).
    #[must_use]
    pub fn resolve(&self, mode: Mode, input: &str) -> Option<Command> {
        self.maps.get(&(mode, input.to_string())).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::Command, mode::Mode};

    #[test]
    fn test_keymap_resolution_quest() {
        // QUEST: Verify that default and custom keys resolve correctly.
        let mut registry = KeymapRegistry::new();

        // 1. Check Default (Normal Mode)
        assert_eq!(registry.resolve(Mode::Normal, "h"), Some(Command::MoveLeft));
        assert_eq!(registry.resolve(Mode::Normal, "i"), Some(Command::EnterMode(Mode::Insert)));

        // 2. Check Default (Insert Mode)
        assert_eq!(registry.resolve(Mode::Insert, "esc"), Some(Command::EnterMode(Mode::Normal)));

        // 3. Custom Chord Binding
        // Even if we don't have a sequence state machine yet, we can bind "fd" as a single string.
        registry.bind(Mode::Insert, "fd", Command::EnterMode(Mode::Normal));
        assert_eq!(registry.resolve(Mode::Insert, "fd"), Some(Command::EnterMode(Mode::Normal)));

        // 4. Modal Overlap Check
        // Bind 'x' in Insert mode to something else
        registry.bind(Mode::Insert, "x", Command::Backspace);
        assert_eq!(registry.resolve(Mode::Normal, "x"), Some(Command::Delete));
        assert_eq!(registry.resolve(Mode::Insert, "x"), Some(Command::Backspace));

        // 5. Shadow Check (Unbound keys)
        assert_eq!(
            registry.resolve(Mode::Normal, "z"),
            None,
            "The 'Shadow-Check' failed! Unbound key returned a command!"
        );
    }
}
