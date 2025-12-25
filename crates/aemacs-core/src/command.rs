/// The fundamental atomic actions the editor can perform.
/// This is the "language" that both the user (via keys) and the AI (via intent) speak.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // Navigation
    MoveRight,
    MoveLeft,
    MoveUp,
    MoveDown,

    // Editing
    Insert(String),
    InsertNewline,
    Backspace,
    Delete,

    // System
    Undo,
    Redo,
    Save,

    // Mode Switching
    EnterMode(crate::mode::Mode),
}
