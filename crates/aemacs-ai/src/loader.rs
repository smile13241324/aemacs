use crate::models::{ContentPart, ImageUrl};
use anyhow::{Context, Result};
use base64::prelude::*;
use mime_guess::from_path;
use std::fs;
use std::path::Path;

/// Loads a file from the physical disk and translates it into a format suitable for the AI model.
/// It automatically detects MIME types to distinguish between text documents and image assets.
pub fn load_file(path: impl AsRef<Path>) -> Result<ContentPart> {
    let path = path.as_ref();
    let mime = from_path(path).first_or_octet_stream();

    if mime.type_() == "image" {
        let bytes = fs::read(path).with_context(|| format!("Failed to read image: {path:?}"))?;
        let b64 = BASE64_STANDARD.encode(&bytes);
        let url = format!("data:{mime};base64,{b64}");
        Ok(ContentPart::ImageUrl {
            image_url: ImageUrl { url },
        })
    } else {
        // Default to Text.
        // In a real system, we should check for binary content to avoid dumping garbage.
        // For now, we assume if it's not an image, it's text context (code, logs, etc).
        let text = fs::read_to_string(path)
            .with_context(|| format!("Failed to read text file: {path:?}"))?;
        Ok(ContentPart::Text { text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_file_gatekeeper_audit_quest() -> Result<()> {
        // --- 1. The Scroll of Truth (Valid UTF-8) ---
        let mut text_file = NamedTempFile::new()?;
        let content = "The Iron Core is solid.";
        text_file.write_all(content.as_bytes())?;

        let result = load_file(text_file.path())?;
        if let ContentPart::Text { text } = result {
            assert_eq!(text, content);
        } else {
            panic!("The 'Scroll-of-Truth' was mistaken for an image!");
        }

        // --- 2. The Corruption-Beast (Invalid UTF-8) ---
        // Note: Unless it has an image extension, our logic currently tries read_to_string and fails.
        let mut binary_file = NamedTempFile::new()?;
        binary_file.write_all(&[0, 159, 146, 150])?;

        let result = load_file(binary_file.path());
        assert!(
            result.is_err(),
            "The 'Corruption-Beast' was allowed to pass! Invalid UTF-8 should fail read_to_string."
        );

        // --- 3. The Void-Check (Missing File) ---
        let result = load_file("/tmp/non_existent_file_9999");
        assert!(
            result.is_err(),
            "The 'Void-Check' failed! Missing file should return an error."
        );

        Ok(())
    }
}
