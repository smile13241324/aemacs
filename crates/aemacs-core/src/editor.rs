use crate::buffer::Buffer;
use crate::command::Command;
use crate::mode::Mode;
use crate::selection::Selection;
use ropey::Rope;

/// A snapshot of the editor state at a specific point in time.
#[derive(Debug, Clone)]
struct Snapshot {
    content: Rope,
    selections: Vec<Selection>,
}

/// The engine that manages the interaction between the buffer and the user.
#[derive(Debug)]
pub struct Editor {
    pub buffer: Buffer,
    pub selections: Vec<Selection>,
    pub mode: Mode,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            selections: vec![Selection::point(0)],
            mode: Mode::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from_file(path: std::path::PathBuf) -> anyhow::Result<Self> {
        let buffer = Buffer::from_file(path)?;
        Ok(Self {
            buffer,
            selections: vec![Selection::point(0)],
            mode: Mode::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    /// Reloads the editor buffer from disk. Clears undo/redo stacks to prevent invalid history.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.buffer.reload()?;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    /// Saves the current state to the undo history.
    /// MUST be called before any modification (insert/delete).
    fn save_snapshot(&mut self) {
        let snapshot = Snapshot {
            content: self.buffer.content.clone(),
            selections: self.selections.clone(),
        };
        self.undo_stack.push(snapshot);

        // When we make a new change, the "redo" branch (the future) is invalidated.
        self.redo_stack.clear();
    }

    /// Reverts the last action.
    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            // 1. Save current state to redo stack (so we can go back to the future)
            let current_state = Snapshot {
                content: self.buffer.content.clone(),
                selections: self.selections.clone(),
            };
            self.redo_stack.push(current_state);

            // 2. Restore the snapshot
            self.buffer.content = snapshot.content;
            self.selections = snapshot.selections;

            // Dirty state handling is complex (we might have undone to a clean state),
            // but for now, let's keep it simple.
            self.buffer.dirty = true;
        }
    }

    /// Re-applies the last undone action.
    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            // 1. Save current state to undo stack
            let current_state = Snapshot {
                content: self.buffer.content.clone(),
                selections: self.selections.clone(),
            };
            self.undo_stack.push(current_state);

            // 2. Restore the snapshot
            self.buffer.content = snapshot.content;
            self.selections = snapshot.selections;
        }
    }

    /// Inserts a character (or string) at all cursor positions.
    /// Handles Multi-Cursor logic by processing usually from right to left (simulated here simpler).
    pub fn insert(&mut self, text: &str) {
        self.save_snapshot();
        let text_len = text.chars().count(); // Grapheme count approximation

        // 1. We must sort selections to process from back to front?
        // Actually, for a naive implementation, let's process them and track the offset shift.
        // BUT: The safest standard way is: Sort descending by start index.
        // This prevents index invalidation for subsequent edits.
        self.selections.sort_by(|a, b| b.start().cmp(&a.start()));

        for selection in &mut self.selections {
            let start = selection.start();
            let end = selection.end();

            // Step A: If we have a range selection, delete it first.
            if start != end {
                self.buffer.content.remove(start..end);
            }

            // Step B: Insert the new text at the start position.
            self.buffer.content.insert(start, text);
            self.buffer.dirty = true;

            // Step C: Update the cursor to be at the end of inserted text.
            // Anchor moves to new head usually, or stays?
            // Standard behavior: Cursor is collapsed to end of insertion.
            let new_pos = start + text_len;
            *selection = Selection::point(new_pos);
        }
    }

    /// Moves all cursors one character to the right.
    /// If a selection exists, it collapses the cursor to the end of the selection.
    pub fn move_right(&mut self) {
        let max_len = self.buffer.len_chars();

        for selection in &mut self.selections {
            if !selection.is_empty() {
                // Case A: Collapse selection to the right end
                *selection = Selection::point(selection.end());
            } else {
                // Case B: Move cursor right (clamp at EOF)
                let new_pos = std::cmp::min(selection.head + 1, max_len);
                *selection = Selection::point(new_pos);
            }
            // Memory clear, as user explicitly moves horizontally
            selection.wanted_column = None;
        }
    }

    /// Moves all cursors one character to the left.
    /// If a selection exists, it collapses the cursor to the start of the selection.
    pub fn move_left(&mut self) {
        for selection in &mut self.selections {
            if !selection.is_empty() {
                // Case A: Collapse selection to the left start
                *selection = Selection::point(selection.start());
            } else {
                // Case B: Move cursor left (clamp at 0)
                let new_pos = selection.head.saturating_sub(1);
                *selection = Selection::point(new_pos);
            }
            // Memory clear, as user explicitly moves horizontally
            selection.wanted_column = None;
        }
    }

    /// Deletes the character before the cursor (Backspace) or the active selection.
    pub fn backspace(&mut self) {
        self.save_snapshot();

        // CRITICAL: Sort descending!
        // If we delete at index 10, index 50 shifts to 49.
        // If we delete at index 50 first, index 10 stays at 10.
        self.selections.sort_by(|a, b| b.start().cmp(&a.start()));

        for selection in &mut self.selections {
            let start = selection.start();
            let end = selection.end();

            if start != end {
                // Case A: Delete Selection
                self.buffer.content.remove(start..end);
                *selection = Selection::point(start);
                self.buffer.dirty = true;
            } else if start > 0 {
                // Case B: Simple Backspace (delete char before)
                self.buffer.content.remove(start - 1..start);
                *selection = Selection::point(start - 1);
                self.buffer.dirty = true;
            }
            // Case C: Start == 0 -> Do nothing (can't backspace at start of file)
        }
    }

    /// Saves the current buffer to disk.
    pub fn save(&mut self) -> anyhow::Result<()> {
        self.buffer.save()
    }

    /// Sets the file path for the current buffer (e.g. "Save As").
    pub fn set_path(&mut self, path: std::path::PathBuf) {
        self.buffer.path = Some(path);
    }

    /// Returns the primary cursor (usually the last one added or the "main" one).
    pub fn primary_cursor(&self) -> Selection {
        *self.selections.first().unwrap_or(&Selection::point(0))
    }

    /// Executes a single abstract command.
    /// This is the main entry point for Keybindings and AI Agents.
    pub fn run(&mut self, cmd: Command) -> anyhow::Result<()> {
        match cmd {
            // Navigation
            Command::MoveRight => self.move_right(),
            Command::MoveLeft => self.move_left(),
            Command::MoveUp => self.move_vertical(-1),
            Command::MoveDown => self.move_vertical(1),

            // Editing
            Command::Insert(text) => self.insert(&text),
            Command::InsertNewline => self.insert_newline(),
            Command::Backspace => self.backspace(),
            Command::Delete => self.delete(),

            // System
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::Save => self.save()?,

            // Mode Switching
            Command::EnterMode(new_mode) => self.mode = new_mode,
        }
        Ok(())
    }

    /// Moves cursors vertically (lines).
    /// dir: -1 for Up, +1 for Down.
    fn move_vertical(&mut self, dir: i32) {
        for selection in &mut self.selections {
            // 1. Where are we?
            let current_char_idx = selection.head;
            let current_line_idx = self.buffer.content.char_to_line(current_char_idx);

            // 2. Where do we want to go?
            let target_line_idx = if dir < 0 {
                if current_line_idx == 0 {
                    continue;
                } // At top nothing to do
                current_line_idx - 1
            } else {
                let last_line = self.buffer.content.len_lines() - 1;
                if current_line_idx >= last_line {
                    continue;
                } // At bottom nothing to do
                current_line_idx + 1
            };

            // 3. Column memory calculation
            let current_line_start = self.buffer.content.line_to_char(current_line_idx);
            let column = if let Some(wanted) = selection.wanted_column {
                wanted
            } else {
                // We remember the column for future moves
                let col = current_char_idx - current_line_start;
                selection.wanted_column = Some(col);
                col
            };

            // 4. Calculate target position
            let target_line_start = self.buffer.content.line_to_char(target_line_idx);
            let target_line_end = self.buffer.content.line_to_char(target_line_idx + 1);

            // The length of the target line (excluding the newline at the end, if present)
            // Ropey counts \n as part of the line. We don't want to jump PAST the \n.
            let target_line_len = target_line_end - target_line_start;

            // Small hack: If line is not empty and ends with \n, subtract one.
            // (For a real implementation we need more precise grapheme checks, but for core it's enough).
            let clamp_limit = if target_line_len > 0 {
                target_line_len - 1
            } else {
                0
            };

            let new_col = std::cmp::min(column, clamp_limit);
            let new_pos = target_line_start + new_col;

            *selection = Selection::point(new_pos);
            // IMPORTANT: Keep the memory!
            selection.wanted_column = Some(column);
        }
    }

    /// Forward delete (Delete key).
    pub fn delete(&mut self) {
        self.save_snapshot();
        self.selections.sort_by(|a, b| b.start().cmp(&a.start()));

        let max_len = self.buffer.len_chars();

        for selection in &mut self.selections {
            let start = selection.start();
            let end = selection.end();

            if start != end {
                // Case A: Delete Selection
                self.buffer.content.remove(start..end);
                *selection = Selection::point(start);
                self.buffer.dirty = true;
            } else if start < max_len {
                // Case B: Simple Delete (delete char at cursor)
                self.buffer.content.remove(start..start + 1);
                // Cursor stays where it is (text shifts left)
                *selection = Selection::point(start);
                self.buffer.dirty = true;
            }
        }
    }

    /// Inserts a newline at all cursor positions.
    pub fn insert_newline(&mut self) {
        // Platform independent internal representation is \n.
        // GPUI handles rendering properly.
        self.insert("\n");
    }

    /// Returns the current cursor position as (1-based Line, 1-based Column).
    pub fn cursor_position(&self) -> (usize, usize) {
        let head = self.primary_cursor().head;
        let line_idx = self.buffer.content.char_to_line(head);
        let line_start = self.buffer.content.line_to_char(line_idx);
        let col_idx = head - line_start;

        // Return 1-based for UI
        (line_idx + 1, col_idx + 1)
    }

    /// Returns the total number of lines
    pub fn line_count(&self) -> usize {
        self.buffer.content.len_lines()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::KeymapRegistry;

    #[test]
    fn test_editor_insert_basic() {
        let mut editor = Editor::new();
        editor.insert("H");
        editor.insert("i");

        assert_eq!(editor.buffer.text(), "Hi");
        assert_eq!(editor.primary_cursor().head, 2);
    }

    #[test]
    fn test_editor_replace_selection() {
        let mut editor = Editor::new();
        editor.insert("Hello"); // Cursor at 5

        // Select "ello" (1 to 5)
        editor.selections = vec![Selection::new(1, 5)];

        editor.insert("i"); // Should replace "ello" with "i" -> "Hi"

        assert_eq!(editor.buffer.text(), "Hi");
        assert_eq!(editor.primary_cursor().head, 2);
    }

    #[test]
    fn test_multi_cursor_insert() {
        let mut editor = Editor::new();
        editor.insert("aa"); // "aa"

        // Cursors at 0 and 1 (before first 'a' and before second 'a')
        // We set manually for test.
        // Wait: "aa" -> indices: 0, 1, 2.
        // We put cursors at 1 (between a and a) and 2 (end).
        editor.selections = vec![Selection::point(1), Selection::point(2)];

        // Insert "b" -> Expect "aba" ... wait.
        // Buffer: "a a"
        // Cursor 1 at 1: inserts 'b' -> "ab a" (Cursor moves to 2)
        // Cursor 2 at 2 (originally): Since we process back-to-front:
        // 1. Process Cursor at 2: "a a" -> "a ab"
        // 2. Process Cursor at 1: "a ab" -> "ab ab"

        editor.insert("b");

        assert_eq!(editor.buffer.text(), "abab");
    }

    #[test]
    fn test_movement_and_bounds() {
        let mut editor = Editor::new();
        editor.insert("abc"); // Cursor is at 3

        // Try moving right past end
        editor.move_right();
        assert_eq!(editor.primary_cursor().head, 3); // Should stay at 3

        // Move left
        editor.move_left();
        assert_eq!(editor.primary_cursor().head, 2); // Between b and c

        // Move all the way to start
        editor.move_left();
        editor.move_left();
        editor.move_left(); // Extra move
        assert_eq!(editor.primary_cursor().head, 0); // Should stick at 0
    }

    #[test]
    fn test_backspace_complex() {
        let mut editor = Editor::new();
        editor.insert("Hello World");
        // Text: "Hello World", Cursor at 11

        // 1. Delete "d"
        editor.backspace();
        assert_eq!(editor.buffer.text(), "Hello Worl");

        // 2. Create selection "Wor" (indices 6 to 9)
        // "H e l l o   W o r l"
        //  0 1 2 3 4 5 6 7 8 9
        editor.selections = vec![Selection::new(6, 9)];

        // 3. Backspace on selection -> should delete "Wor"
        editor.backspace();
        assert_eq!(editor.buffer.text(), "Hello l");
        assert_eq!(editor.primary_cursor().head, 6); // Cursor should be where "W" was
    }

    #[test]
    fn test_undo_redo() {
        let mut editor = Editor::new();

        // 1. Type "A"
        editor.insert("A");
        assert_eq!(editor.buffer.text(), "A");

        // 2. Type "B" -> "AB"
        editor.insert("B");
        assert_eq!(editor.buffer.text(), "AB");

        // 3. Undo -> Should be "A"
        editor.undo();
        assert_eq!(editor.buffer.text(), "A");

        // 4. Undo -> Should be empty ""
        editor.undo();
        assert_eq!(editor.buffer.text(), "");

        // 5. Redo -> Should be "A"
        editor.redo();
        assert_eq!(editor.buffer.text(), "A");

        // 6. Type "C" -> "AC" (This kills the "B" future in redo stack)
        editor.insert("C");
        assert_eq!(editor.buffer.text(), "AC");

        // 7. Redo should do nothing (stack cleared)
        editor.redo();
        assert_eq!(editor.buffer.text(), "AC");

        // 8. Undo -> "A"
        editor.undo();
        assert_eq!(editor.buffer.text(), "A");
    }

    #[test]
    fn test_editor_persistence() -> anyhow::Result<()> {
        // 1. Setup Temp File
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("aemacs_save_test.txt");

        // Ensure cleanup from previous runs
        let _ = std::fs::remove_file(&file_path);

        // 2. Create Editor, Type something, Save
        {
            let mut editor = Editor::new();
            editor.set_path(file_path.clone());
            editor.insert("Hello Saved World");
            assert!(editor.buffer.dirty); // Should be dirty

            editor.save()?; // <--- THE ACTION
            assert!(!editor.buffer.dirty); // Should be clean
        }

        // 3. Check file on disk (Independent verification)
        let content = std::fs::read_to_string(&file_path)?;
        assert_eq!(content, "Hello Saved World");

        // 4. Load back into new Editor
        let editor2 = Editor::from_file(file_path.clone())?;
        assert_eq!(editor2.buffer.text(), "Hello Saved World");

        // Cleanup
        let _ = std::fs::remove_file(file_path);

        Ok(())
    }

    #[test]
    fn test_command_pipeline() -> anyhow::Result<()> {
        let mut editor = Editor::new();

        // Thats basically a "macro" or what the AI would generate:
        let script = vec![
            Command::Insert("Hello".to_string()), // Write Hello
            Command::MoveLeft,                    // Go back (between l and o)
            Command::Backspace,                   // Delete l -> "Helo"
            Command::Insert("p".to_string()),     // Write p -> "Helpo" ... wait
            Command::Undo,                        // Undo Insert "p" -> "Helo"
            Command::Undo,                        // Undo Backspace -> "Hello"
        ];

        // Execute
        for cmd in script {
            editor.run(cmd)?;
        }

        assert_eq!(editor.buffer.text(), "Hello");
        Ok(())
    }

    #[test]
    fn test_vertical_movement_memory() {
        let mut editor = Editor::new();
        // Setup a "Sandwich" structure:
        // Long line
        // Short line
        // Long line
        editor.insert("0123456789");
        editor.insert_newline();
        editor.insert("01");
        editor.insert_newline();
        editor.insert("0123456789");

        // Reset cursor to start
        editor.selections[0] = Selection::point(0);

        // Move to character '5' in the first line
        for _ in 0..5 {
            editor.move_right();
        }
        // Cursor is now at index 5 (on the '5')

        // 1. Move DOWN to the short line
        editor.move_vertical(1);
        // Expectation: Should be capped at the end of the short line (at '1' or after),
        // because the line is too short for column 5.
        // Index Check: Line 0 has 11 chars ("0123456789\n").
        // Line 1 has 3 chars ("01\n").
        // Cursor should snap to the end of visible text in line 1.

        // 2. Move DOWN again to the third (long) line
        editor.move_vertical(1);

        // MAGIC: Should be back at '5'!
        // The editor remembered we actually wanted column 5.
        let current_pos = editor.primary_cursor().head;

        // Math verification:
        // Line 0 length: 11 chars ("0123456789\n")
        // Line 1 length: 3 chars ("01\n")
        // Line 2 Start Index: 11 + 3 = 14.
        // Target Index: 14 + 5 = 19.
        assert_eq!(current_pos, 19);

        // Check content at cursor to be 100% sure
        let char_at_cursor = editor.buffer.content.char(current_pos);
        assert_eq!(char_at_cursor, '5');
    }

    #[test]
    fn test_modal_editing_workflow() -> anyhow::Result<()> {
        let mut editor = Editor::new();
        let keymap = KeymapRegistry::new(); // Loads Vim Defaults

        // 1. Start: Normal Mode
        // User presses 'j'. In an empty buffer, 'j' (MoveDown) does nothing visible,
        // but crucially: IT MUST NOT INSERT "j".
        let input = "j";
        if let Some(cmd) = keymap.resolve(editor.mode, input) {
            editor.run(cmd)?;
        } else if editor.mode == crate::mode::Mode::Insert {
            // Only in Insert Mode do we treat unbound keys as text!
            editor.run(Command::Insert(input.to_string()))?;
        } else {
            // Normal Mode + Unbound Key = Ignore (or beep in UI later)
        }

        // Assertion: Buffer must remain empty!
        assert_eq!(editor.buffer.text(), "");

        // 2. User presses 'ü' (Unbound key in Normal Mode)
        // Should strictly be ignored.
        let input = "ü";
        if let Some(cmd) = keymap.resolve(editor.mode, input) {
            editor.run(cmd)?;
        } else if editor.mode == crate::mode::Mode::Insert {
            editor.run(Command::Insert(input.to_string()))?;
        }

        // Assertion: Still empty. No "ü" inserted.
        assert_eq!(editor.buffer.text(), "");

        // 3. Switch to Insert Mode ('i')
        let cmd = keymap.resolve(editor.mode, "i").unwrap();
        editor.run(cmd)?;
        assert_eq!(editor.mode, crate::mode::Mode::Insert);

        // 4. Typing ("h", "i")
        // Now we are in Insert Mode. 'h' is NOT bound in Insert Mode.
        // Logic: resolve returns None -> Fallback to Insert(char).
        let inputs = vec!["h", "i"];
        for key in inputs {
            if let Some(cmd) = keymap.resolve(editor.mode, key) {
                editor.run(cmd)?;
            } else if editor.mode == crate::mode::Mode::Insert {
                // FALLBACK: Allow insertion
                editor.run(Command::Insert(key.to_string()))?;
            }
        }
        assert_eq!(editor.buffer.text(), "hi");

        // 5. Back to Normal Mode ('esc')
        let cmd = keymap.resolve(editor.mode, "esc").unwrap();
        editor.run(cmd)?;
        assert_eq!(editor.mode, crate::mode::Mode::Normal);

        // 6. Test Command Execution in Normal Mode ('x' -> Delete)
        // Cursor is at end ("hi"). Move left once to be on 'i'.
        editor.run(Command::MoveLeft)?;

        let cmd = keymap.resolve(editor.mode, "x").unwrap();
        editor.run(cmd)?; // Should delete 'i'

        // Verify delete happened
        assert_eq!(editor.buffer.text(), "h");

        // 7. Test Undo ('u')
        let cmd = keymap.resolve(editor.mode, "u").unwrap();
        editor.run(cmd)?;

        // Should be restored to "hi"
        assert_eq!(editor.buffer.text(), "hi");

        Ok(())
    }
}
