//! Headless indexer for benchmarks and debugging:
//! `cargo run --release -p smriti-core --bin smriti-index -- <library-dir> [folder ...]`

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use smriti_core::{Error, IndexProgress, Library};

fn main() -> anyhow::Result<()> {
    tracing_subscriber_init();
    let mut args = std::env::args().skip(1);
    let lib_dir = args.next().ok_or_else(|| anyhow::anyhow!("usage: smriti-index <library-dir> [folder ...]"))?;
    let lib = Library::open(&lib_dir)?;
    for folder in args {
        match lib.add_source(&folder) {
            Ok(s) => println!("added {}", s.path),
            Err(Error::SourceAlreadyAdded(_)) => {}
            Err(e) => return Err(e.into()),
        }
    }

    let cancel = AtomicBool::new(false);
    let t = Instant::now();
    let scan = lib.scan_all(&cancel)?;
    println!("scan: {scan:?} in {:.1}s", t.elapsed().as_secs_f32());

    let workers = std::env::var("SMRITI_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get().saturating_sub(1).max(1)));
    let t = Instant::now();
    let last = Mutex::new(Instant::now());
    let report = |p: IndexProgress| {
        let mut last = last.lock().unwrap();
        if last.elapsed().as_secs() >= 10 {
            *last = Instant::now();
            let n = p.done + p.failed;
            let rate = n as f32 / t.elapsed().as_secs_f32();
            println!("{n}/{} ({} failed) {rate:.1}/s", p.total, p.failed);
        }
    };
    let p = lib.run_index(workers, &cancel, &report)?;
    let secs = t.elapsed().as_secs_f32();
    println!("index: {p:?} in {secs:.0}s ({:.1}/s, {workers} workers)", (p.done + p.failed) as f32 / secs.max(0.1));
    println!("{:?}", lib.stats()?);
    Ok(())
}

fn tracing_subscriber_init() {
    // Keep the bench output readable; warnings only.
    let _ = std::env::var("RUST_LOG");
}
