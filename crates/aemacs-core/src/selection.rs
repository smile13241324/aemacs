use std::cmp::{max, min};

/// Represents a cursor or a text selection in the buffer.
///
/// A selection is defined by an `anchor` (fixed point) and a `head` (moving point).
/// If anchor == head, it is a simple caret (cursor).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The start point of the selection (where the user started selecting).
    pub anchor: usize,
    /// The end point of the selection (where the active cursor is).
    pub head: usize,
    /// Stores the visual column we WANT to be in during vertical movement.
    /// If None, we recalculate it from the current head.
    pub wanted_column: Option<usize>,
}

impl Selection {
    /// Creates a new cursor (point) at the specified character index.
    #[must_use]
    pub const fn point(pos: usize) -> Self {
        Self {
            anchor: pos,
            head: pos,
            wanted_column: None, // Reset memory
        }
    }

    /// Creates a new selection spanning from the anchor to the head.
    #[must_use]
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head, wanted_column: None }
    }

    /// Returns true if this selection represents a single point (cursor) rather than a range.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Returns the absolute starting character index of the selection (the minimum of anchor and head).
    #[must_use]
    pub fn start(&self) -> usize {
        min(self.anchor, self.head)
    }

    /// Returns the absolute ending character index of the selection (the maximum of anchor and head).
    #[must_use]
    pub fn end(&self) -> usize {
        max(self.anchor, self.head)
    }

    /// Swaps the anchor and head of the selection, effectively reversing its direction.
    /// This operation resets the `wanted_column` memory.
    #[must_use]
    pub const fn swap(&self) -> Self {
        Self { anchor: self.head, head: self.anchor, wanted_column: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_basics() {
        let sel = Selection::new(10, 20); // Left to Right
        assert_eq!(sel.start(), 10);
        assert_eq!(sel.end(), 20);
        assert!(!sel.is_empty());

        let rev_sel = Selection::new(20, 10); // Right to Left
        assert_eq!(rev_sel.start(), 10); // Start is still 10!
        assert_eq!(rev_sel.end(), 20);
        assert!(!rev_sel.is_empty());

        let cursor = Selection::point(5);
        assert!(cursor.is_empty());
        assert_eq!(cursor.anchor, 5);
        assert_eq!(cursor.head, 5);
    }
}
