//! YuNet output decoding: anchor-free heads at strides 8, 16 and 32, each
//! giving class and objectness scores, a box and five landmarks per cell.
//! Mirrors OpenCV's `FaceDetectorYN` post-processing.

/// A face in detector-input pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    /// x, y, width, height.
    pub bbox: [f32; 4],
    /// Right eye, left eye, nose tip, right and left mouth corners (as seen in the image: subject's right first).
    pub landmarks: [[f32; 2]; 5],
    pub score: f32,
}

pub const STRIDES: [usize; 3] = [8, 16, 32];

/// Raw head outputs for one stride, flattened row-major over the grid.
pub struct Head<'a> {
    pub stride: usize,
    pub cls: &'a [f32],
    pub obj: &'a [f32],
    pub bbox: &'a [f32],
    pub kps: &'a [f32],
}

/// Decodes every head for a `pad_w` x `pad_h` input, keeping cells scoring at least `min_score`.
pub fn decode(heads: &[Head<'_>], pad_w: usize, pad_h: usize, min_score: f32) -> Vec<Detection> {
    let mut out = Vec::new();
    for h in heads {
        let s = h.stride as f32;
        let cols = pad_w / h.stride;
        let rows = pad_h / h.stride;
        let cells = (rows * cols).min(h.cls.len()).min(h.obj.len());
        for idx in 0..cells {
            let score = (h.cls[idx].clamp(0.0, 1.0) * h.obj[idx].clamp(0.0, 1.0)).sqrt();
            if score < min_score {
                continue;
            }
            let (r, c) = ((idx / cols) as f32, (idx % cols) as f32);
            let b = &h.bbox[idx * 4..idx * 4 + 4];
            let (cx, cy) = ((c + b[0]) * s, (r + b[1]) * s);
            let (w, hh) = (b[2].exp() * s, b[3].exp() * s);
            let k = &h.kps[idx * 10..idx * 10 + 10];
            let mut landmarks = [[0.0; 2]; 5];
            for (n, lm) in landmarks.iter_mut().enumerate() {
                *lm = [(k[2 * n] + c) * s, (k[2 * n + 1] + r) * s];
            }
            out.push(Detection { bbox: [cx - w / 2.0, cy - hh / 2.0, w, hh], landmarks, score });
        }
    }
    out
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = (a[0] + a[2]).min(b[0] + b[2]);
    let y2 = (a[1] + a[3]).min(b[1] + b[3]);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a[2] * a[3] + b[2] * b[3] - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// Greedy non-maximum suppression, best score first.
pub fn nms(mut dets: Vec<Detection>, threshold: f32) -> Vec<Detection> {
    dets.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut keep: Vec<Detection> = Vec::new();
    for d in dets {
        if keep.iter().all(|k| iou(&k.bbox, &d.bbox) < threshold) {
            keep.push(d);
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_single_cell() {
        // 32x32 input at stride 8 -> 4x4 grid; light up cell (row 1, col 2).
        let mut cls = vec![0.0; 16];
        let mut obj = vec![0.0; 16];
        let mut bbox = vec![0.0; 64];
        let kps = vec![0.5; 160];
        cls[6] = 0.81;
        obj[6] = 1.0;
        bbox[24..28].copy_from_slice(&[0.5, 0.5, 0.0, 0.0]); // centre of the cell, 8x8
        let heads = [Head { stride: 8, cls: &cls, obj: &obj, bbox: &bbox, kps: &kps }];
        let d = decode(&heads, 32, 32, 0.5);
        assert_eq!(d.len(), 1);
        assert!((d[0].score - 0.9).abs() < 1e-6);
        assert_eq!(d[0].bbox, [16.0, 8.0, 8.0, 8.0]);
        assert_eq!(d[0].landmarks[0], [20.0, 12.0]);
    }

    #[test]
    fn nms_drops_overlapping_boxes() {
        let d = |x: f32, score| Detection { bbox: [x, 0.0, 10.0, 10.0], landmarks: [[0.0; 2]; 5], score };
        let kept = nms(vec![d(0.0, 0.8), d(1.0, 0.9), d(50.0, 0.7)], 0.3);
        assert_eq!(kept.iter().map(|k| k.score).collect::<Vec<_>>(), vec![0.9, 0.7]);
    }
}
