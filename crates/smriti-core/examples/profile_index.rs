//! Times each indexing step on a sample of files:
//! `cargo run --release -p smriti-core --example profile_index -- <folder> [n]`

use std::time::{Duration, Instant};

use nom_exif::MediaParser;
use smriti_core::{Kind, metadata, thumbs};

fn main() {
    let mut args = std::env::args().skip(1);
    let folder = args.next().expect("folder");
    let n: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);
    let files: Vec<_> = walkdir::WalkDir::new(&folder)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("jpg")))
        .take(n)
        .map(|e| e.into_path())
        .collect();

    let out = std::env::temp_dir().join("smriti-profile");
    std::fs::create_dir_all(&out).unwrap();
    let mut parser = MediaParser::new();
    let (mut t_meta, mut t_read, mut t_thumb) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    for (i, f) in files.iter().enumerate() {
        let t = Instant::now();
        let m = metadata::read(&mut parser, f, Kind::Photo, 0, None);
        t_meta += t.elapsed();

        let t = Instant::now();
        let _ = std::fs::read(f).map(|b| b.len());
        t_read += t.elapsed();

        let t = Instant::now();
        thumbs::make_photo(f, None, m.orientation, &out.join(format!("{i}.webp"))).ok();
        t_thumb += t.elapsed();
    }
    let per = |d: Duration| d.as_secs_f64() * 1000.0 / files.len() as f64;
    println!(
        "{} files: metadata {:.0} ms, full read {:.0} ms, thumbnail {:.0} ms (per file)",
        files.len(),
        per(t_meta),
        per(t_read),
        per(t_thumb)
    );
}
