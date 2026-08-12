//! Content-addressed disk cache for image attachments.
//!
//! Images picked via the attach button are hashed and copied into
//! `~/.operon/media/<sha256>.<ext>` so repeat uploads dedupe and the GUI has
//! a stable path to reference for chip rendering / later history replay.

use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Pending attachment representation for the prompt composer.
#[derive(Debug, Clone)]
pub enum PendingAttachment {
    Image {
        cached_path: PathBuf,
        media_type: String,
        base64_data: String,
        display_name: String,
    },
    File {
        path: PathBuf,
        display_name: String,
    },
}

/// Hashes and copies an image attachment into `~/.operon/media/<sha256>.<ext>`.
/// Skips copying if the file already exists in the cache (deduplication).
pub fn cache_image(source_path: &Path) -> anyhow::Result<PathBuf> {
    let bytes = fs::read(source_path)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();

    let media_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not locate home directory"))?
        .join(".operon")
        .join("media");

    fs::create_dir_all(&media_dir)?;

    let target_filename = format!("{hash}{ext}");
    let target_path = media_dir.join(target_filename);

    if !target_path.exists() {
        fs::write(&target_path, &bytes)?;
    }

    Ok(target_path)
}

/// Sniffs file magic bytes to determine if the path is a supported image:
/// PNG, JPEG, GIF, or WEBP.
pub fn is_image_mime(path: &Path) -> bool {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => return false,
    };

    if bytes.len() >= 8 && &bytes[0..8] == &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return true;
    }

    if bytes.len() >= 3 && &bytes[0..3] == &[0xFF, 0xD8, 0xFF] {
        return true;
    }

    if bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        return true;
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return true;
    }

    false
}

/// Reads the file, base64 encodes it, and determines its MIME type via magic bytes.
pub fn encode_base64(path: &Path) -> anyhow::Result<(String, String)> {
    let bytes = fs::read(path)?;

    let media_type = if bytes.len() >= 8
        && &bytes[0..8] == &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
    {
        "image/png"
    } else if bytes.len() >= 3 && &bytes[0..3] == &[0xFF, 0xD8, 0xFF] {
        "image/jpeg"
    } else if bytes.len() >= 6 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        "image/gif"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        anyhow::bail!("Unsupported image format: magic bytes do not match PNG, JPEG, GIF, or WEBP");
    };

    let base64_data = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok((media_type.to_string(), base64_data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_image_mime_sniffs_magic_bytes() {
        let temp_dir = tempfile::tempdir().unwrap();

        // 1. PNG magic bytes
        let png_path = temp_dir.path().join("test.png");
        fs::write(
            &png_path,
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01],
        )
        .unwrap();
        assert!(is_image_mime(&png_path));

        // 2. JPEG magic bytes
        let jpg_path = temp_dir.path().join("test.jpg");
        fs::write(&jpg_path, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00]).unwrap();
        assert!(is_image_mime(&jpg_path));

        // 3. GIF magic bytes
        let gif_path = temp_dir.path().join("test.gif");
        fs::write(&gif_path, b"GIF89a_header_data").unwrap();
        assert!(is_image_mime(&gif_path));

        // 4. WEBP magic bytes
        let webp_path = temp_dir.path().join("test.webp");
        let mut webp_bytes = Vec::new();
        webp_bytes.extend_from_slice(b"RIFF");
        webp_bytes.extend_from_slice(&[0; 4]);
        webp_bytes.extend_from_slice(b"WEBP");
        fs::write(&webp_path, &webp_bytes).unwrap();
        assert!(is_image_mime(&webp_path));

        // 5. Plain text file (not an image)
        let txt_path = temp_dir.path().join("notes.txt");
        fs::write(&txt_path, b"Hello world text file").unwrap();
        assert!(!is_image_mime(&txt_path));
    }

    #[test]
    fn test_encode_base64_returns_correct_media_type() {
        let temp_dir = tempfile::tempdir().unwrap();
        let png_path = temp_dir.path().join("test.png");
        let sample_bytes = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x11, 0x22];
        fs::write(&png_path, sample_bytes).unwrap();

        let (media_type, base64_data) = encode_base64(&png_path).unwrap();
        assert_eq!(media_type, "image/png");
        assert_eq!(
            base64_data,
            base64::engine::general_purpose::STANDARD.encode(sample_bytes)
        );
    }
}
