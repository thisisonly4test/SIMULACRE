//! Face forge. Pulls GAN-generated portraits from thispersondoesnotexist.com
//! (or any compatible endpoint), re-encodes them locally and serves them from
//! the vault — the browser never talks to the third party.
//!
//! Re-encoding does three things: strips every metadata block, normalises the
//! size to 512×512, and gives each face a content-addressed id (SHA-256), so
//! the same picture reused by two legends is detected.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const DEFAULT_SOURCE: &str = "https://thispersondoesnotexist.com/random-person.jpeg";
pub const SIZE: u32 = 512;
pub const MAX_UPLOAD: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FaceError {
    #[error("face source unreachable: {0}")]
    Fetch(String),
    #[error("not an image: {0}")]
    Decode(String),
    #[error("storage: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize)]
pub struct Face {
    pub id: String,
    pub url: String,
    pub source: String,
}

pub use simulacre_core::service::is_valid_face_id as is_valid_id;

pub fn path_for(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.jpg"))
}

/// Decode → centre-crop to square → resize → JPEG. Returns (id, bytes).
pub fn normalize(raw: &[u8]) -> Result<(String, Vec<u8>), FaceError> {
    let img = image::load_from_memory(raw).map_err(|e| FaceError::Decode(e.to_string()))?;
    let (w, h) = (img.width(), img.height());
    if w < 64 || h < 64 {
        return Err(FaceError::Decode(format!("too small ({w}×{h})")));
    }
    let side = w.min(h);
    let square = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let resized = DynamicImage::ImageRgb8(square.resize_exact(SIZE, SIZE, FilterType::Lanczos3).to_rgb8());
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(Cursor::new(&mut out), 88);
    enc.encode_image(&resized).map_err(|e| FaceError::Decode(e.to_string()))?;
    let id = hex::encode(&Sha256::digest(&out)[..16]);
    Ok((id, out))
}

pub async fn save(dir: &Path, id: &str, bytes: &[u8]) -> Result<(), FaceError> {
    let path = path_for(dir, id);
    if !tokio::fs::try_exists(&path).await? {
        let tmp = dir.join(format!(".{id}.tmp"));
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, &path).await?;
    }
    Ok(())
}

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("simulacre/", env!("CARGO_PKG_VERSION"), " (+local OSINT persona workbench)"))
        .build()
        .expect("http client")
}

pub async fn fetch(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, FaceError> {
    let res = client.get(url).header("Accept", "image/jpeg,image/*").send().await.map_err(|e| FaceError::Fetch(e.to_string()))?;
    if !res.status().is_success() {
        return Err(FaceError::Fetch(format!("HTTP {}", res.status())));
    }
    if let Some(ct) = res.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok())
        && !ct.starts_with("image/")
    {
        return Err(FaceError::Fetch(format!("unexpected content-type {ct}")));
    }
    let bytes = res.bytes().await.map_err(|e| FaceError::Fetch(e.to_string()))?;
    if bytes.len() > MAX_UPLOAD {
        return Err(FaceError::Fetch("image too large".into()));
    }
    Ok(bytes.to_vec())
}

pub fn sniff_ok(raw: &[u8]) -> bool {
    matches!(image::guess_format(raw), Ok(ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn sample_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 128]));
        let mut out = Vec::new();
        DynamicImage::ImageRgb8(img).write_to(&mut Cursor::new(&mut out), ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn normalize_squares_and_hashes() {
        let (id, jpg) = normalize(&sample_png(300, 200)).unwrap();
        assert!(is_valid_id(&id));
        let back = image::load_from_memory(&jpg).unwrap();
        assert_eq!((back.width(), back.height()), (SIZE, SIZE));
        let (id2, _) = normalize(&sample_png(300, 200)).unwrap();
        assert_eq!(id, id2, "content-addressed");
    }

    #[test]
    fn rejects_garbage_and_tiny() {
        assert!(normalize(b"not an image").is_err());
        assert!(normalize(&sample_png(10, 10)).is_err());
        assert!(!sniff_ok(b"GIF89a"));
        assert!(sniff_ok(&sample_png(80, 80)));
    }

    #[test]
    fn id_validation_blocks_traversal() {
        assert!(!is_valid_id("../../etc/passwd"));
        assert!(!is_valid_id("ABCDEF0123456789ABCDEF0123456789"));
        assert!(is_valid_id("abcdef0123456789abcdef0123456789"));
    }
}
