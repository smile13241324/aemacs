use anyhow::{Context, Result};
use ropey::Rope;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::path::PathBuf;

/// The central data structure for text editing.
/// Backed by a Rope (Tree-based string) for O(log n) insert/delete.
#[derive(Debug, Clone)]
pub struct Buffer {
    /// The actual text content.
    pub content: Rope,
    /// The file path (None if it's a scratch buffer/unsaved).
    pub path: Option<PathBuf>,
    /// Dirty flag: Has the buffer been modified since load/save?
    pub dirty: bool,
}

impl Buffer {
    /// Creates a new, empty scratch buffer.
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
            path: None,
            dirty: false,
        }
    }

    /// Loads a buffer from a file on disk.
    ///
    /// # Errors
    /// Returns an error if the file cannot be opened or read.
    pub fn from_file(path: PathBuf) -> Result<Self> {
        // Reasoning: Use BufReader for better I/O performance on large files.
        let file =
            File::open(&path).with_context(|| format!("Failed to open file at {:?}", path))?;

        let reader = BufReader::new(file);

        // Ropey loads efficiently from a reader (chunks).
        let content = Rope::from_reader(reader)?;

        Ok(Self {
            content,
            path: Some(path),
            dirty: false,
        })
    }

    /// Returns the length of the buffer in characters (graphemes).
    pub fn len_chars(&self) -> usize {
        self.content.len_chars()
    }

    /// Debug helper: Returns the full content as a String.
    /// WARNING: Allocates memory. Do not use for huge files in hot paths.
    pub fn text(&self) -> String {
        self.content.to_string()
    }

    /// Saves the buffer content to disk atomically.
    /// 1. Writes to a temporary file.
    /// 2. Renames temporary file to target file.
    pub fn save(&mut self) -> Result<()> {
        let path = self
            .path
            .as_ref()
            .context("Cannot save a buffer with no path (Scratchpad)")?;

        // 1. Create a temp file in the same directory (ensures same filesystem for rename)
        let mut temp_path = path.clone();
        if let Some(filename) = path.file_name() {
            let mut new_name = std::ffi::OsString::from(".");
            new_name.push(filename);
            new_name.push(".tmp");
            temp_path.set_file_name(new_name);
        } else {
            // Fallback if weird path
            temp_path.set_extension("tmp");
        }

        {
            let file = File::create(&temp_path)?;
            let mut writer = BufWriter::new(file);

            // Ropey streams the chunks directly to the writer. Zero extra allocation.
            self.content.write_to(&mut writer)?;
            // Ensure everything is flushed to disk hardware
            writer.into_inner()?.sync_all()?;
        }

        // 2. Atomic Swap
        std::fs::rename(&temp_path, path)?;

        // 3. Mark as clean
        self.dirty = false;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.len_chars(), 0);
        assert_eq!(buf.text(), "");
        assert_eq!(buf.path, None);
        assert!(!buf.dirty);
    }

    #[test]
    fn test_load_from_file() -> Result<()> {
        // 1. Setup: We create a temporary file with content
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("aemacs_test_buffer.txt");

        {
            let mut file = File::create(&file_path)?;
            write!(file, "Hello Æmacs Core!")?;
        }

        // 2. Action: We load the file into our buffer
        let buf = Buffer::from_file(file_path.clone())?;

        // 3. Assertion: Check content correctness
        assert_eq!(buf.text(), "Hello Æmacs Core!");
        assert_eq!(buf.len_chars(), 17);
        assert_eq!(buf.path, Some(file_path.clone()));
        assert!(!buf.dirty);

        // Cleanup (Best Effort)
        let _ = std::fs::remove_file(file_path);

        Ok(())
    }
}
