//! Face alignment for SFace: a similarity transform (rotation, uniform scale,
//! translation) that maps the five landmarks onto the ArcFace 112x112 template.

use image::RgbImage;

pub const SIZE: u32 = 112;

/// Where the landmarks should land in the 112x112 crop (same order as YuNet).
const TEMPLATE: [[f32; 2]; 5] =
    [[38.2946, 51.6963], [73.5318, 51.5014], [56.0252, 71.7366], [41.5493, 92.3655], [70.7299, 92.2041]];

/// Least-squares similarity transform src -> dst, as complex numbers:
/// z' = a z + b, with a = sum(conj(p) q) / sum(|p|^2) over centred points.
/// Returns (a_re, a_im, b_re, b_im).
fn similarity(src: &[[f32; 2]; 5], dst: &[[f32; 2]; 5]) -> (f32, f32, f32, f32) {
    let mean = |pts: &[[f32; 2]; 5]| {
        let (x, y) = pts.iter().fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
        (x / 5.0, y / 5.0)
    };
    let (mpx, mpy) = mean(src);
    let (mqx, mqy) = mean(dst);
    let (mut num_re, mut num_im, mut den) = (0.0f32, 0.0f32, 0.0f32);
    for (p, q) in src.iter().zip(dst) {
        let (px, py) = (p[0] - mpx, p[1] - mpy);
        let (qx, qy) = (q[0] - mqx, q[1] - mqy);
        // conj(p) * q
        num_re += px * qx + py * qy;
        num_im += px * qy - py * qx;
        den += px * px + py * py;
    }
    if den <= f32::EPSILON {
        return (1.0, 0.0, mqx - mpx, mqy - mpy);
    }
    let (ar, ai) = (num_re / den, num_im / den);
    // b = mean_q - a * mean_p
    (ar, ai, mqx - (ar * mpx - ai * mpy), mqy - (ar * mpy + ai * mpx))
}

/// Warps the face into a 112x112 crop with bilinear sampling (black outside the image).
pub fn align(img: &RgbImage, landmarks: &[[f32; 2]; 5]) -> RgbImage {
    let (ar, ai, br, bi) = similarity(landmarks, &TEMPLATE);
    // Inverse map: z = (z' - b) / a
    let n = ar * ar + ai * ai;
    let (ir, ii) = (ar / n, -ai / n);
    let (w, h) = (img.width() as i64, img.height() as i64);
    let px = |x: i64, y: i64| -> [f32; 3] {
        if x < 0 || y < 0 || x >= w || y >= h {
            [0.0; 3]
        } else {
            let p = img.get_pixel(x as u32, y as u32).0;
            [p[0] as f32, p[1] as f32, p[2] as f32]
        }
    };
    RgbImage::from_fn(SIZE, SIZE, |u, v| {
        let (dx, dy) = (u as f32 - br, v as f32 - bi);
        let sx = dx * ir - dy * ii;
        let sy = dx * ii + dy * ir;
        let (x0, y0) = (sx.floor(), sy.floor());
        let (fx, fy) = (sx - x0, sy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let (p00, p10, p01, p11) = (px(x0, y0), px(x0 + 1, y0), px(x0, y0 + 1), px(x0 + 1, y0 + 1));
        let mut out = [0u8; 3];
        for c in 0..3 {
            let top = p00[c] * (1.0 - fx) + p10[c] * fx;
            let bottom = p01[c] * (1.0 - fx) + p11[c] * fx;
            out[c] = (top * (1.0 - fy) + bottom * fy).round().clamp(0.0, 255.0) as u8;
        }
        image::Rgb(out)
    })
}

/// Variance of the Laplacian of the grey crop: low means blurry.
pub fn sharpness(crop: &RgbImage) -> f32 {
    let (w, h) = (crop.width() as usize, crop.height() as usize);
    let grey: Vec<f32> =
        crop.pixels().map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).collect();
    let (mut sum, mut sq, mut n) = (0.0f64, 0.0f64, 0.0f64);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let l = (grey[i - 1] + grey[i + 1] + grey[i - w] + grey[i + w] - 4.0 * grey[i]) as f64;
            sum += l;
            sq += l * l;
            n += 1.0;
        }
    }
    if n == 0.0 { 0.0 } else { (sq / n - (sum / n).powi(2)) as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_maps_onto_itself() {
        let (ar, ai, br, bi) = similarity(&TEMPLATE, &TEMPLATE);
        assert!((ar - 1.0).abs() < 1e-5 && ai.abs() < 1e-5 && br.abs() < 1e-3 && bi.abs() < 1e-3);
    }

    #[test]
    fn recovers_scale_rotation_and_shift() {
        // src = template scaled by 2, rotated 90 degrees, shifted by (100, 50).
        let src = TEMPLATE.map(|[x, y]| [100.0 - 2.0 * y, 50.0 + 2.0 * x]);
        let (ar, ai, br, bi) = similarity(&src, &TEMPLATE);
        for (s, t) in src.iter().zip(TEMPLATE) {
            let (x, y) = (ar * s[0] - ai * s[1] + br, ai * s[0] + ar * s[1] + bi);
            assert!((x - t[0]).abs() < 1e-2 && (y - t[1]).abs() < 1e-2, "{x},{y} vs {t:?}");
        }
    }

    #[test]
    fn flat_crop_has_no_sharpness() {
        assert_eq!(sharpness(&RgbImage::from_pixel(112, 112, image::Rgb([90, 90, 90]))), 0.0);
    }
}
