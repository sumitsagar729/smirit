//! Face analysis: YuNet finds faces and landmarks, SFace turns each aligned
//! face into a 128-number embedding. Similar embeddings (cosine) mean the
//! same person. Both models run on the CPU through ONNX Runtime.

pub mod align;
pub mod detect;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use image::{RgbImage, imageops};
use ort::environment::Environment;
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;

use detect::{Detection, Head, STRIDES};

pub const DETECTOR_FILE: &str = "face_detection_yunet_2023mar.onnx";
pub const RECOGNIZER_FILE: &str = "face_recognition_sface_2021dec.onnx";
pub const EMBEDDING_DIM: usize = 128;
/// Bump when detection or embedding changes so old results get redone.
pub const PIPELINE_VERSION: i64 = 1;

/// Images are decoded with their short side at least this long for faces
/// (a 12 MP photo decodes at 1/4 scale, about 1000 x 750).
pub const DECODE_SHORT_SIDE: u32 = 720;
/// The detector takes a fixed 640 x 640 input; images are letterboxed into it.
const DETECT_SIZE: u32 = 640;
const MIN_SCORE: f32 = 0.75;
const NMS_IOU: f32 = 0.3;
/// Faces smaller than this (detector pixels) are too small to recognise.
const MIN_FACE_PX: f32 = 12.0;
pub const CROP_SIZE: u32 = 160;

#[derive(Debug, thiserror::Error)]
#[error("face analysis failed: {0}")]
pub struct FaceError(String);

fn err(e: impl std::fmt::Display) -> FaceError {
    FaceError(e.to_string())
}

/// Both model files, if present in `dir`.
pub fn model_paths(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let (d, r) = (dir.join(DETECTOR_FILE), dir.join(RECOGNIZER_FILE));
    (d.is_file() && r.is_file()).then_some((d, r))
}

#[derive(Debug, Clone)]
pub struct AnalyzedFace {
    /// x, y, width, height as fractions of the (oriented) image.
    pub bbox: [f32; 4],
    pub landmarks: [[f32; 2]; 5],
    pub score: f32,
    /// 0..1 from size, sharpness and how frontal the face is.
    pub quality: f32,
    /// Unit-length SFace embedding.
    pub embedding: Vec<f32>,
    /// Square crop for the UI, CROP_SIZE pixels.
    pub crop: RgbImage,
}

/// One ONNX Runtime environment per process, shared by every session.
static ENVIRONMENT: LazyLock<Result<Environment, String>> =
    LazyLock::new(|| ort::init().with_name("smriti").with_telemetry(false).build().map_err(|e| e.to_string()));

/// One per worker thread: ONNX Runtime sessions need `&mut` to run.
pub struct FaceEngine {
    detector: Session,
    recognizer: Session,
}

impl FaceEngine {
    pub fn load(models_dir: &Path) -> Result<Self, FaceError> {
        let (det, rec) =
            model_paths(models_dir).ok_or_else(|| err(format!("models not found in {}", models_dir.display())))?;
        let session = |path: &Path| -> Result<Session, FaceError> {
            let env = ENVIRONMENT.as_ref().map_err(err)?;
            Session::builder(env)
                .map_err(err)?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(err)?
                .with_intra_threads(1)
                .map_err(err)?
                .commit_from_file(path)
                .map_err(err)
        };
        Ok(Self { detector: session(&det)?, recognizer: session(&rec)? })
    }

    /// Finds, aligns and embeds every face in an already-oriented image.
    pub fn analyze(&mut self, img: &RgbImage) -> Result<Vec<AnalyzedFace>, FaceError> {
        let (w, h) = img.dimensions();
        if w < 32 || h < 32 {
            return Ok(Vec::new());
        }
        let (detections, scale) = self.detect(img)?;
        let mut out = Vec::with_capacity(detections.len());
        for d in detections {
            if d.bbox[2].min(d.bbox[3]) < MIN_FACE_PX {
                continue;
            }
            // Back to full-image pixels: align from the sharpest pixels we have.
            let landmarks = d.landmarks.map(|[x, y]| [x / scale, y / scale]);
            let bbox = d.bbox.map(|v| v / scale);
            let aligned = align::align(img, &landmarks);
            let embedding = self.embed(&aligned)?;
            out.push(AnalyzedFace {
                bbox: [bbox[0] / w as f32, bbox[1] / h as f32, bbox[2] / w as f32, bbox[3] / h as f32],
                landmarks: landmarks.map(|[x, y]| [x / w as f32, y / h as f32]),
                score: d.score,
                quality: quality(&d, &aligned),
                embedding,
                crop: crop(img, bbox),
            });
        }
        Ok(out)
    }

    /// Letterboxes the image into the detector's fixed square input. Returns
    /// detections in input pixels and the image-to-input scale.
    fn detect(&mut self, img: &RgbImage) -> Result<(Vec<Detection>, f32), FaceError> {
        let (w, h) = img.dimensions();
        let scale = (DETECT_SIZE as f32 / w.max(h) as f32).min(1.0);
        let small;
        let fitted = if scale < 1.0 {
            let (sw, sh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
            small = shrink(img, sw.min(DETECT_SIZE), sh.min(DETECT_SIZE))?;
            &small
        } else {
            img
        };
        let side = DETECT_SIZE as usize;
        let plane = side * side;
        // NCHW, BGR, raw 0..255, zero padding right and bottom.
        let mut data = vec![0f32; 3 * plane];
        for (x, y, p) in fitted.enumerate_pixels() {
            let i = y as usize * side + x as usize;
            data[i] = p[2] as f32;
            data[plane + i] = p[1] as f32;
            data[2 * plane + i] = p[0] as f32;
        }
        let input = Tensor::from_array(([1usize, 3, side, side], data)).map_err(err)?;
        let outputs = self.detector.run(ort::inputs![input]).map_err(err)?;
        let get = |name: String| -> Result<&[f32], FaceError> {
            let v = outputs.get(&name).ok_or_else(|| err(format!("detector has no output {name}")))?;
            Ok(v.try_extract_tensor::<f32>().map_err(err)?.1)
        };
        let mut heads = Vec::with_capacity(3);
        for stride in STRIDES {
            heads.push(Head {
                stride,
                cls: get(format!("cls_{stride}"))?,
                obj: get(format!("obj_{stride}"))?,
                bbox: get(format!("bbox_{stride}"))?,
                kps: get(format!("kps_{stride}"))?,
            });
        }
        Ok((detect::nms(detect::decode(&heads, side, side, MIN_SCORE), NMS_IOU), scale))
    }

    fn embed(&mut self, aligned: &RgbImage) -> Result<Vec<f32>, FaceError> {
        let n = (align::SIZE * align::SIZE) as usize;
        // NCHW, RGB, raw 0..255.
        let mut data = vec![0f32; 3 * n];
        for (i, p) in aligned.pixels().enumerate() {
            data[i] = p[0] as f32;
            data[n + i] = p[1] as f32;
            data[2 * n + i] = p[2] as f32;
        }
        let input = Tensor::from_array(([1usize, 3, align::SIZE as usize, align::SIZE as usize], data)).map_err(err)?;
        let outputs = self.recognizer.run(ort::inputs![input]).map_err(err)?;
        let (_, v) = outputs[0].try_extract_tensor::<f32>().map_err(err)?;
        if v.len() != EMBEDDING_DIM {
            return Err(err(format!("embedding has {} values", v.len())));
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
        Ok(v.iter().map(|x| x / norm).collect())
    }
}

/// SIMD resize (several times faster than `image`'s for big DSLR frames).
fn shrink(img: &RgbImage, w: u32, h: u32) -> Result<RgbImage, FaceError> {
    use fast_image_resize::{self as fr, ResizeAlg, ResizeOptions, Resizer};
    let src = fr::images::ImageRef::new(img.width(), img.height(), img.as_raw(), fr::PixelType::U8x3).map_err(err)?;
    let mut dst = fr::images::Image::new(w, h, fr::PixelType::U8x3);
    Resizer::new()
        .resize(&src, &mut dst, &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(fr::FilterType::Bilinear)))
        .map_err(err)?;
    RgbImage::from_raw(w, h, dst.into_vec()).ok_or_else(|| err("resize produced a wrong-sized buffer"))
}

/// Size, sharpness and pose, each 0..1, blended. Tiny, blurry or profile
/// faces embed poorly, so grouping only trusts the better ones.
fn quality(d: &Detection, aligned: &RgbImage) -> f32 {
    let size = ((d.bbox[2].min(d.bbox[3]) - MIN_FACE_PX) / 48.0).clamp(0.0, 1.0);
    let sharp = (align::sharpness(aligned) / 250.0).clamp(0.0, 1.0);
    let [re, le, nose, ..] = d.landmarks;
    let eye_dist = ((le[0] - re[0]).powi(2) + (le[1] - re[1]).powi(2)).sqrt().max(1.0);
    let yaw = (nose[0] - (re[0] + le[0]) / 2.0).abs() / eye_dist;
    let frontal = (1.0 - yaw / 0.6).clamp(0.0, 1.0);
    let confidence = ((d.score - MIN_SCORE) / (1.0 - MIN_SCORE)).clamp(0.0, 1.0);
    0.35 * size + 0.25 * sharp + 0.25 * frontal + 0.15 * confidence
}

/// A square crop around the face with some margin, for People screens.
fn crop(img: &RgbImage, bbox: [f32; 4]) -> RgbImage {
    let side = bbox[2].max(bbox[3]) * 1.5;
    let (cx, cy) = (bbox[0] + bbox[2] / 2.0, bbox[1] + bbox[3] / 2.0);
    let side = side.min(img.width().min(img.height()) as f32).max(1.0);
    let x = (cx - side / 2.0).clamp(0.0, img.width() as f32 - side);
    let y = (cy - side / 2.0).clamp(0.0, img.height() as f32 - side);
    let region = imageops::crop_imm(img, x as u32, y as u32, side as u32, side as u32).to_image();
    imageops::resize(&region, CROP_SIZE, CROP_SIZE, imageops::FilterType::CatmullRom)
}

/// `faces/<id / 1000>/<id>.webp`, like thumbnails.
pub fn crop_path(faces_dir: &Path, id: crate::Id) -> PathBuf {
    faces_dir.join(format!("{:04}", id.0 / 1000)).join(format!("{}.webp", id.0))
}

/// Cosine similarity of two unit vectors.
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub fn from_blob(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_round_trip() {
        let v = vec![0.25f32, -1.5, 3.0];
        assert_eq!(from_blob(&to_blob(&v)), v);
    }

    #[test]
    fn crop_stays_inside_the_image() {
        let img = RgbImage::from_pixel(300, 200, image::Rgb([10, 20, 30]));
        let c = crop(&img, [250.0, 150.0, 80.0, 80.0]);
        assert_eq!(c.dimensions(), (CROP_SIZE, CROP_SIZE));
    }

    /// Runs only where the models are available (`scripts/fetch-models.ps1`).
    #[test]
    fn engine_finds_nothing_in_a_blank_image() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
        if model_paths(&dir).is_none() {
            return;
        }
        let mut engine = FaceEngine::load(&dir).unwrap();
        let faces = engine.analyze(&RgbImage::from_pixel(640, 480, image::Rgb([128, 128, 128]))).unwrap();
        assert!(faces.is_empty());
    }
}
