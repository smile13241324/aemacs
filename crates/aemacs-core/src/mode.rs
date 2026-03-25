use std::fmt;

/// Defines the operational modes of the editor, determining how input events are interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Mode {
    /// Standard navigation mode where keys trigger architectural commands (e.g., movement, deletion).
    #[default]
    Normal,
    /// Direct text entry mode where most keys result in character insertion into the buffer.
    Insert,
    /// Selection mode where movement keys expand the active selection range.
    Visual,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Insert => write!(f, "INSERT"),
            Mode::Visual => write!(f, "VISUAL"),
        }
    }
}
