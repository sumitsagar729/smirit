//! What counts as a photo or video, and which folders are never indexed.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Photo,
    Video,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Photo => "photo",
            Kind::Video => "video",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "photo" => Some(Kind::Photo),
            "video" => Some(Kind::Video),
            _ => None,
        }
    }
}

/// Classifies a file by extension. RAW and HEIC are listed so they are found
/// and dated now; their thumbnails arrive with dedicated decoders later.
pub fn classify(path: &Path) -> Option<(Kind, &'static str)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" | "jfif" => (Kind::Photo, "image/jpeg"),
        "png" => (Kind::Photo, "image/png"),
        "webp" => (Kind::Photo, "image/webp"),
        "gif" => (Kind::Photo, "image/gif"),
        "bmp" => (Kind::Photo, "image/bmp"),
        "tif" | "tiff" => (Kind::Photo, "image/tiff"),
        "heic" | "heif" => (Kind::Photo, "image/heic"),
        "cr2" | "cr3" | "nef" | "arw" | "dng" | "raf" | "orf" | "rw2" => (Kind::Photo, "image/x-raw"),
        "mp4" | "m4v" => (Kind::Video, "video/mp4"),
        "mov" => (Kind::Video, "video/quicktime"),
        "3gp" => (Kind::Video, "video/3gpp"),
        "mkv" => (Kind::Video, "video/x-matroska"),
        "avi" => (Kind::Video, "video/x-msvideo"),
        "wmv" => (Kind::Video, "video/x-ms-wmv"),
        "webm" => (Kind::Video, "video/webm"),
        "mts" | "m2ts" => (Kind::Video, "video/mp2t"),
        _ => return None,
    })
}

/// Formats the built-in decoders can thumbnail directly.
pub fn decodable_photo(mime: &str) -> bool {
    matches!(mime, "image/jpeg" | "image/png" | "image/webp" | "image/gif" | "image/bmp" | "image/tiff")
}

/// Directory names skipped during scans (case-insensitive).
pub fn ignored_dir(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with('.')
        || matches!(
            n.as_str(),
            "node_modules" | "$recycle.bin" | "system volume information" | "@eadir" | "thumbs" | "__macosx"
        )
}

/// Files below this size are icons, stickers or broken copies.
pub const MIN_FILE_BYTES: u64 = 10 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_formats_case_insensitively() {
        assert_eq!(classify(Path::new("IMG_1.JPG")), Some((Kind::Photo, "image/jpeg")));
        assert_eq!(classify(Path::new("a/b/VID.Mp4")), Some((Kind::Video, "video/mp4")));
        assert_eq!(classify(Path::new("x.CR2")).map(|c| c.0), Some(Kind::Photo));
        assert_eq!(classify(Path::new("notes.pdf")), None);
        assert_eq!(classify(Path::new("no_extension")), None);
    }

    #[test]
    fn skips_system_and_hidden_dirs() {
        assert!(ignored_dir(".git"));
        assert!(ignored_dir("$RECYCLE.BIN"));
        assert!(!ignored_dir("Diwali 2021"));
    }
}
