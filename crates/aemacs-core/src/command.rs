/// The fundamental atomic actions the editor can perform.
/// This is the "language" that both the user (via keys) and the AI (via intent) speak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Move the primary cursor one character to the right.
    MoveRight,
    /// Move the primary cursor one character to the left.
    MoveLeft,
    /// Move the primary cursor one line up.
    MoveUp,
    /// Move the primary cursor one line down.
    MoveDown,

    /// Insert a literal string at the current cursor position.
    Insert(String),
    /// Insert a newline character at the current cursor position.
    InsertNewline,
    /// Delete the character before the current cursor position.
    Backspace,
    /// Delete the character at or after the current cursor position.
    Delete,

    /// Revert the last modification to the buffer.
    Undo,
    /// Re-apply the last undone modification.
    Redo,
    /// Physically write the current buffer state to disk.
    Save,

    /// Switch the editor to a different operational mode.
    EnterMode(crate::mode::Mode),
}
