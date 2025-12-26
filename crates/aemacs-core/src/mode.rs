use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
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
