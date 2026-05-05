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

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    /// Creates a new, empty scratch buffer with no associated physical path.
    ///
    /// # Examples
    ///
    /// ```
    /// use aemacs_core::buffer::Buffer;
    /// let buf = Buffer::new();
    /// assert_eq!(buf.len_chars(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
            path: None,
            dirty: false,
        }
    }

    /// Loads a buffer from a file on disk at the specified path.
    /// It uses a buffered reader for efficient I/O performance on large files.
    ///
    /// # Errors
    /// Returns an error if the file cannot be opened, read, or if the content is not valid UTF-8.
    pub fn from_file(path: PathBuf) -> Result<Self> {
        // Reasoning: Use BufReader for better I/O performance on large files.
        let file =
            File::open(&path).with_context(|| format!("Failed to open file at {path:?}"))?;

        let reader = BufReader::new(file);

        // Ropey loads efficiently from a reader (chunks).
        let content = Rope::from_reader(reader)?;

        Ok(Self {
            content,
            path: Some(path),
            dirty: false,
        })
    }

    /// Reloads the buffer content directly from disk, effectively discarding any unsaved changes.
    /// Does nothing if the buffer has no associated path.
    ///
    /// # Errors
    /// Returns an error if the underlying file has been removed or is no longer accessible.
    pub fn reload(&mut self) -> Result<()> {
        if let Some(path) = &self.path {
            let file =
                File::open(path).with_context(|| format!("Failed to open file at {path:?}"))?;
            let reader = BufReader::new(file);
            self.content = Rope::from_reader(reader)?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Returns the total number of characters (Unicode scalar values) currently in the buffer.
    #[must_use] 
    pub fn len_chars(&self) -> usize {
        self.content.len_chars()
    }

    /// Returns the entire content of the buffer as a standard Rust String.
    ///
    /// # Warning
    /// This method clones and allocates the entire buffer into memory.
    /// It should be avoided for extremely large files in performance-critical paths.
    #[must_use] 
    pub fn text(&self) -> String {
        self.content.to_string()
    }

    /// Saves the current buffer content to its physical file on disk.
    /// The operation is performed atomically by writing to a temporary file and then renaming it.
    ///
    /// # Errors
    /// Returns an error if the buffer has no path, or if disk I/O fails during writing or renaming.
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
