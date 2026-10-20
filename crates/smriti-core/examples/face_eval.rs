//! Measures face detection recall, speed and identity separation against a
//! labelled set: TSV rows of `path, x, y, w, h (fractions), person, date`.
//!
//! cargo run --release -p smriti-core --example face_eval -- <labels.tsv> <models-dir>

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use nom_exif::MediaParser;
use smriti_core::faces::{self, FaceEngine};
use smriti_core::{Kind, metadata, thumbs};

struct Label {
    bbox: [f32; 4],
    who: String,
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let x1 = a[0].max(b[0]);
    let y1 = a[1].max(b[1]);
    let x2 = (a[0] + a[2]).min(b[0] + b[2]);
    let y2 = (a[1] + a[3]).min(b[1] + b[3]);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    inter / (a[2] * a[3] + b[2] * b[3] - inter)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (tsv, models) = (&args[1], Path::new(&args[2]));
    let mut photos: BTreeMap<String, Vec<Label>> = BTreeMap::new();
    for line in std::fs::read_to_string(tsv).unwrap().lines() {
        let c: Vec<&str> = line.split('\t').collect();
        let b = [c[1], c[2], c[3], c[4]].map(|v| v.parse::<f32>().unwrap());
        photos.entry(c[0].to_string()).or_default().push(Label { bbox: b, who: c[5].to_string() });
    }

    let mut engine = FaceEngine::load(models).unwrap();
    let mut parser = MediaParser::new();
    let (mut labelled, mut found, mut detected_total) = (0, 0, 0);
    let (mut t_decode, mut t_faces) = (0.0, 0.0);
    let mut matched: Vec<(String, usize, Vec<f32>, f32)> = Vec::new(); // who, photo index, embedding, quality
    for (pi, (path, labels)) in photos.iter().enumerate() {
        let p = Path::new(path);
        let Ok(bytes) = std::fs::read(p) else { continue };
        let t = Instant::now();
        let data = bytes::Bytes::from(bytes);
        let meta = metadata::read(&mut parser, p, Kind::Photo, 0, Some(&data));
        let img = thumbs::decode_photo(p, Some(&data), faces::DECODE_SHORT_SIDE).unwrap();
        let img = thumbs::orient(img, meta.orientation.unwrap_or(1)).into_rgb8();
        t_decode += t.elapsed().as_secs_f64();
        let t = Instant::now();
        let fs = engine.analyze(&img).unwrap();
        t_faces += t.elapsed().as_secs_f64();
        detected_total += fs.len();
        for l in labels {
            labelled += 1;
            if let Some(f) = fs.iter().max_by(|a, b| iou(&a.bbox, &l.bbox).total_cmp(&iou(&b.bbox, &l.bbox)))
                && iou(&f.bbox, &l.bbox) >= 0.3
            {
                found += 1;
                matched.push((l.who.clone(), pi, f.embedding.clone(), f.quality));
            }
        }
    }
    let n = photos.len() as f64;
    println!(
        "photos {} | labelled faces {labelled} | found {found} ({:.1}%) | detected {detected_total}",
        n,
        100.0 * found as f64 / labelled as f64
    );
    println!("per photo: decode {:.0} ms, faces {:.0} ms", 1000.0 * t_decode / n, 1000.0 * t_faces / n);

    // Pairwise similarity, different photos only.
    let (mut same, mut diff) = (Vec::new(), Vec::new());
    for i in 0..matched.len() {
        for j in i + 1..matched.len() {
            if matched[i].1 == matched[j].1 {
                continue;
            }
            let s = faces::similarity(&matched[i].2, &matched[j].2);
            if matched[i].0 == matched[j].0 { same.push(s) } else { diff.push(s) }
        }
    }
    same.sort_by(f32::total_cmp);
    diff.sort_by(f32::total_cmp);
    let pct = |v: &[f32], q: f64| v[((v.len() - 1) as f64 * q) as usize];
    println!("same person  p10 {:.3} p50 {:.3} p90 {:.3}", pct(&same, 0.1), pct(&same, 0.5), pct(&same, 0.9));
    println!("different    p50 {:.3} p99 {:.3} p99.9 {:.3}", pct(&diff, 0.5), pct(&diff, 0.99), pct(&diff, 0.999));
    for t in [0.3, 0.35, 0.4, 0.45, 0.5] {
        let tar = same.iter().filter(|&&s| s >= t).count() as f64 / same.len() as f64;
        let far = diff.iter().filter(|&&s| s >= t).count() as f64 / diff.len() as f64;
        println!(
            "  threshold {t:.2}: same-person pairs accepted {:.1}%, different-person pairs accepted {:.3}%",
            100.0 * tar,
            100.0 * far
        );
    }

    // Identification, leave one out: mean of the top 3 similarities per person.
    for (min, margin) in [(0.38, 0.04), (0.42, 0.04), (0.45, 0.05)] {
        let (mut right, mut wrong, mut undecided) = (0, 0, 0);
        for (i, (who, pi, e, _)) in matched.iter().enumerate() {
            let mut per: BTreeMap<&str, Vec<f32>> = BTreeMap::new();
            for (j, (w2, pj, e2, _)) in matched.iter().enumerate() {
                if i != j && pi != pj {
                    per.entry(w2).or_default().push(faces::similarity(e, e2));
                }
            }
            let mut scores: Vec<(f32, &str)> = per
                .into_iter()
                .map(|(w, mut v)| {
                    v.sort_by(|a, b| b.total_cmp(a));
                    (v.iter().take(3).sum::<f32>() / v.len().min(3) as f32, w)
                })
                .collect();
            scores.sort_by(|a, b| b.0.total_cmp(&a.0));
            let (best, second) = (scores[0], scores.get(1).map_or(0.0, |s| s.0));
            if best.0 < min || best.0 - second < margin {
                undecided += 1;
            } else if best.1 == who {
                right += 1;
            } else {
                wrong += 1;
            }
        }
        println!("identify min {min} margin {margin}: right {right}, wrong {wrong}, undecided {undecided}");
    }
    let mut q: Vec<f32> = matched.iter().map(|m| m.3).collect();
    q.sort_by(f32::total_cmp);
    println!("quality p10 {:.2} p50 {:.2} p90 {:.2}", pct(&q, 0.1), pct(&q, 0.5), pct(&q, 0.9));
}
