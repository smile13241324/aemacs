use crate::models::{ContentPart, ImageUrl};
use anyhow::{Context, Result};
use base64::prelude::*;
use mime_guess::from_path;
use std::fs;
use std::path::Path;

pub fn load_file(path: impl AsRef<Path>) -> Result<ContentPart> {
    let path = path.as_ref();
    let mime = from_path(path).first_or_octet_stream();

    if mime.type_() == "image" {
        let bytes = fs::read(path).with_context(|| format!("Failed to read image: {:?}", path))?;
        let b64 = BASE64_STANDARD.encode(&bytes);
        let url = format!("data:{};base64,{}", mime, b64);
        Ok(ContentPart::ImageUrl {
            image_url: ImageUrl { url },
        })
    } else {
        // Default to Text.
        // In a real system, we should check for binary content to avoid dumping garbage.
        // For now, we assume if it's not an image, it's text context (code, logs, etc).
        let text = fs::read_to_string(path)
            .with_context(|| format!("Failed to read text file: {:?}", path))?;
        Ok(ContentPart::Text { text })
    }
}
