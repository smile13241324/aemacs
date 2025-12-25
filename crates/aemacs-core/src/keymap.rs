use crate::command::Command;
use crate::mode::Mode;
use std::collections::HashMap;

/// Stores the mapping between keystrokes and commands for each mode.
#[derive(Debug, Default)]
pub struct KeymapRegistry {
    maps: HashMap<(Mode, String), Command>,
}

impl KeymapRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_defaults();
        registry
    }

    /// Registers the standard Vim-like bindings.
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

    pub fn bind(&mut self, mode: Mode, key: &str, cmd: Command) {
        self.maps.insert((mode, key.to_string()), cmd);
    }

    /// Resolves an input to a command.
    /// Returns None if no binding exists (which usually means: insert literal char if in insert mode).
    pub fn resolve(&self, mode: Mode, input: &str) -> Option<Command> {
        self.maps.get(&(mode, input.to_string())).cloned()
    }
}
