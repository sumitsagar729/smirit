//! Grid thumbnails: WebP, about 440 px tall (sharp on 2x displays), stored as
//! `thumbs/<id / 1000>/<id>.webp` so no folder holds more than 1,000 files.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use fast_image_resize::{self as fr, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageReader, RgbImage};

use crate::Id;

pub const THUMB_HEIGHT: u32 = 440;
const MAX_WIDTH: u32 = 1100;
const WEBP_QUALITY: f32 = 78.0;

pub fn thumb_path(thumbs_dir: &Path, id: Id) -> PathBuf {
    thumbs_dir.join(format!("{:04}", id.0 / 1000)).join(format!("{}.webp", id.0))
}

#[derive(Debug, thiserror::Error)]
pub enum ThumbError {
    #[error("cannot decode image: {0}")]
    Decode(String),
    #[error("ffmpeg is not available")]
    NoFfmpeg,
    #[error("ffmpeg could not read a frame")]
    VideoFrame,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Decodes, orients, resizes and writes the thumbnail. Returns its size.
/// Uses `data` (the file's bytes) when the caller already read it.
pub fn make_photo(
    src: &Path,
    data: Option<&[u8]>,
    orientation: Option<u16>,
    dest: &Path,
) -> Result<(u32, u32), ThumbError> {
    let img = decode_photo(src, data, THUMB_HEIGHT)?;
    write(img, orientation.unwrap_or(1), dest)
}

/// Writes the thumbnail for an image that is already upright.
pub fn write_oriented(img: RgbImage, dest: &Path) -> Result<(u32, u32), ThumbError> {
    write(DynamicImage::ImageRgb8(img), 1, dest)
}

/// Decodes a photo with its short side at least `min_short` pixels where the
/// format allows a cheaper reduced-size decode (JPEG), else at full size.
pub fn decode_photo(src: &Path, data: Option<&[u8]>, min_short: u32) -> Result<DynamicImage, ThumbError> {
    let owned;
    let bytes = match data {
        Some(d) => d,
        None => {
            owned = std::fs::read(src)?;
            &owned
        }
    };
    let is_jpeg = src
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "jpg" | "jpeg" | "jfif"));
    match is_jpeg.then(|| decode_jpeg_scaled(bytes, min_short)).flatten() {
        Some(img) => Ok(img),
        None => ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()?
            .decode()
            .map_err(|e| ThumbError::Decode(e.to_string())),
    }
}

/// JPEGs can be decoded straight at 1/2, 1/4 or 1/8 size by skipping DCT
/// coefficients: a 12 MP phone photo decodes several times faster this way.
/// Picks the smallest scale whose short side still covers `min_short`.
/// Returns None (caller falls back to a full decode) for anything unusual.
fn decode_jpeg_scaled(bytes: &[u8], min_short: u32) -> Option<DynamicImage> {
    use jpeg_decoder::{Decoder, PixelFormat};
    let mut d = Decoder::new(Cursor::new(bytes));
    d.read_info().ok()?;
    let info = d.info()?;
    let (w, h) = (info.width as f64, info.height as f64);
    let f = (min_short as f64 / w.min(h)).min(1.0);
    let (sw, sh) = d.scale((w * f).ceil() as u16, (h * f).ceil() as u16).ok()?;
    let px = d.decode().ok()?;
    match d.info()?.pixel_format {
        PixelFormat::RGB24 => RgbImage::from_raw(sw as u32, sh as u32, px).map(DynamicImage::ImageRgb8),
        PixelFormat::L8 => image::GrayImage::from_raw(sw as u32, sh as u32, px).map(DynamicImage::ImageLuma8),
        _ => None, // CMYK / 16-bit: rare, let the general decoder handle colour conversion
    }
}

/// Grabs a frame 10% into the video (skips black intros) with FFmpeg.
pub fn make_video(
    ffmpeg: Option<&Path>,
    src: &Path,
    duration_ms: Option<u64>,
    dest: &Path,
) -> Result<(u32, u32), ThumbError> {
    let ffmpeg = ffmpeg.ok_or(ThumbError::NoFfmpeg)?;
    let at = duration_ms.map(|d| d as f64 / 1000.0 * 0.1).unwrap_or(1.0).min(30.0);
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-v", "error", "-ss", &format!("{at:.2}"), "-i"])
        .arg(src)
        .args(["-frames:v", "1", "-vf", &format!("scale=-2:{THUMB_HEIGHT}"), "-f", "image2pipe", "-c:v", "png", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output()?;
    if out.stdout.is_empty() {
        return Err(ThumbError::VideoFrame);
    }
    let img = image::load(Cursor::new(out.stdout), image::ImageFormat::Png).map_err(|_| ThumbError::VideoFrame)?;
    // FFmpeg already applied the container's rotation.
    write(img, 1, dest)
}

fn write(img: DynamicImage, orientation: u16, dest: &Path) -> Result<(u32, u32), ThumbError> {
    let rgb = img.into_rgb8();
    let (w, h) = (rgb.width(), rgb.height());
    if w == 0 || h == 0 {
        return Err(ThumbError::Decode("empty image".into()));
    }

    // Target size in the stored (unrotated) frame, so the *displayed* height is THUMB_HEIGHT.
    let rotated = matches!(orientation, 5..=8);
    let (disp_w, disp_h) = if rotated { (h, w) } else { (w, h) };
    let scale = (THUMB_HEIGHT as f64 / disp_h as f64).min(MAX_WIDTH as f64 / disp_w as f64).min(1.0);
    let (tw, th) = (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1));

    let small = if (tw, th) == (w, h) {
        rgb
    } else {
        let src = fr::images::Image::from_vec_u8(w, h, rgb.into_raw(), fr::PixelType::U8x3)
            .map_err(|e| ThumbError::Decode(e.to_string()))?;
        let mut dst = fr::images::Image::new(tw, th, fr::PixelType::U8x3);
        Resizer::new()
            .resize(&src, &mut dst, &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(fr::FilterType::Lanczos3)))
            .map_err(|e| ThumbError::Decode(e.to_string()))?;
        RgbImage::from_raw(tw, th, dst.into_vec()).expect("buffer size matches")
    };
    let oriented = orient(DynamicImage::ImageRgb8(small), orientation).into_rgb8();

    let encoded = webp::Encoder::from_rgb(oriented.as_raw(), oriented.width(), oriented.height()).encode(WEBP_QUALITY);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("tmp");
    std::fs::write(&tmp, &*encoded)?;
    std::fs::rename(&tmp, dest)?;
    Ok((oriented.width(), oriented.height()))
}

/// Applies an EXIF orientation (1–8) to pixels.
pub fn orient(img: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// FFmpeg from `SMRITI_FFMPEG`, next to the app executable, or on PATH.
pub fn find_ffmpeg() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(p) = std::env::var_os("SMRITI_FFMPEG").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    if let Some(p) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(exe))).filter(|p| p.is_file())
    {
        return Some(p);
    }
    std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|d| d.join(exe)).find(|p| p.is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_is_scaled_to_thumb_height() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("a.png");
        RgbImage::from_pixel(2000, 1000, image::Rgb([200, 120, 40])).save(&src).unwrap();
        let dest = thumb_path(dir.path(), Id(1234));
        assert_eq!(make_photo(&src, None, None, &dest).unwrap(), (880, THUMB_HEIGHT));
        assert!(dest.ends_with("0001/1234.webp") || dest.ends_with("0001\\1234.webp"));
        assert!(dest.is_file());
    }

    #[test]
    fn exif_rotation_swaps_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("r.png");
        RgbImage::new(1200, 900).save(&src).unwrap(); // stored landscape, displayed portrait
        let (w, h) = make_photo(&src, None, Some(6), &dir.path().join("r.webp")).unwrap();
        assert_eq!(h, THUMB_HEIGHT);
        assert!(w < h);
    }

    #[test]
    fn large_jpeg_uses_scaled_decode() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("big.jpg");
        RgbImage::from_fn(4000, 3000, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90])).save(&src).unwrap();
        let img = decode_jpeg_scaled(&std::fs::read(&src).unwrap(), THUMB_HEIGHT).expect("baseline RGB JPEG decodes");
        // 1/4 scale (1000x750) is the smallest that keeps the short side >= 440.
        assert_eq!((img.width(), img.height()), (1000, 750));
        assert_eq!(make_photo(&src, None, None, &dir.path().join("big.webp")).unwrap(), (587, THUMB_HEIGHT));
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("s.png");
        RgbImage::new(300, 200).save(&src).unwrap();
        assert_eq!(make_photo(&src, None, None, &dir.path().join("s.webp")).unwrap(), (300, 200));
    }
}
