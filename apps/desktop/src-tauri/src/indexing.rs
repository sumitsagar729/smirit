//! Runs scan, index and people recognition in the background and tells the
//! UI how it's going.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, unbounded};
use serde::Serialize;
use smriti_core::{IndexProgress, Library};
use tauri::{AppHandle, Emitter};

pub const STATUS_EVENT: &str = "index-status";

/// Recognition runs between chunks of this many items, so names spread to
/// new photos while a long first index is still going.
const CHUNK: u32 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Scanning,
    Indexing,
    Recognizing,
}

#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub phase: Phase,
    pub total: u32,
    pub done: u32,
    pub failed: u32,
    /// Bumped whenever new items became visible, so the UI knows to refetch.
    pub generation: u32,
    /// Bumped after recognition, so people screens refetch.
    pub people_generation: u32,
}

/// Applies a change to the shared status and tells the UI.
type SetStatus<'a> = dyn Fn(&dyn Fn(&mut IndexStatus)) + Sync + 'a;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Wake {
    Rescan,
    Recognize,
}

pub struct Indexer {
    wake: Sender<Wake>,
    status: Arc<Mutex<IndexStatus>>,
    cancel: Arc<AtomicBool>,
}

impl Indexer {
    pub fn start(app: AppHandle, lib: Arc<Library>) -> Self {
        let (wake, rx) = unbounded::<Wake>();
        let status = Arc::new(Mutex::new(IndexStatus {
            phase: Phase::Idle,
            total: 0,
            done: 0,
            failed: 0,
            generation: 0,
            people_generation: 0,
        }));
        let cancel = Arc::new(AtomicBool::new(false));
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get().saturating_sub(1).max(1));

        let st = status.clone();
        let stop = cancel.clone();
        std::thread::Builder::new()
            .name("indexer".into())
            .spawn(move || {
                let set = |f: &dyn Fn(&mut IndexStatus)| {
                    let snapshot = {
                        let mut s = st.lock().unwrap_or_else(|e| e.into_inner());
                        f(&mut s);
                        *s
                    };
                    let _ = app.emit(STATUS_EVENT, snapshot);
                };
                let recognize = || {
                    set(&|s| s.phase = Phase::Recognizing);
                    match lib.recognize() {
                        Ok(_) => set(&|s| s.people_generation += 1),
                        Err(e) => tracing::error!(error = %e, "recognition failed"),
                    }
                };

                // First run at start-up does everything; later wake-ups say what's needed.
                let mut reason = Wake::Rescan;
                loop {
                    // Coalesce repeated wake-ups; a rescan request wins.
                    while let Ok(w) = rx.try_recv() {
                        if w == Wake::Rescan {
                            reason = Wake::Rescan;
                        }
                    }
                    if reason == Wake::Rescan {
                        set(&|s| s.phase = Phase::Scanning);
                        match lib.scan_all(&stop) {
                            Ok(r) if r.added + r.changed + r.missing + r.restored > 0 => set(&|s| s.generation += 1),
                            Ok(_) => {}
                            Err(e) => tracing::error!(error = %e, "scan failed"),
                        }
                        index_all(&lib, workers, &stop, &set, &recognize);
                    } else {
                        recognize();
                    }
                    set(&|s| s.phase = Phase::Idle);

                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    match rx.recv() {
                        Ok(w) => reason = w,
                        Err(_) => break,
                    }
                }
            })
            .expect("spawn indexer thread");

        Self { wake, status, cancel }
    }

    /// Asks for a rescan; cheap to call repeatedly.
    pub fn wake(&self) {
        let _ = self.wake.send(Wake::Rescan);
    }

    /// Asks for recognition to run again (after naming or correcting faces).
    pub fn recognize(&self) {
        let _ = self.wake.send(Wake::Recognize);
    }

    pub fn status(&self) -> IndexStatus {
        *self.status.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn shutdown(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.wake.send(Wake::Rescan);
    }
}

/// Indexes the whole queue in chunks, recognising people after each chunk.
fn index_all(lib: &Library, workers: usize, stop: &AtomicBool, set: &SetStatus, recognize: &dyn Fn()) {
    let total = lib.stats().map(|s| s.pending_jobs).unwrap_or(0);
    if total == 0 {
        return;
    }
    let (mut done, mut failed) = (0u32, 0u32);
    set(&|s| {
        s.phase = Phase::Indexing;
        s.total = total;
        s.done = 0;
        s.failed = 0;
    });
    while !stop.load(Ordering::Relaxed) {
        let last = Mutex::new(Instant::now());
        let (base_done, base_failed) = (done, failed);
        let on_progress = |p: IndexProgress| {
            let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
            if last.elapsed() >= Duration::from_millis(400) {
                *last = Instant::now();
                set(&|s| {
                    s.phase = Phase::Indexing;
                    s.total = total.max(base_done + base_failed + p.total);
                    s.done = base_done + p.done;
                    s.failed = base_failed + p.failed;
                    s.generation += 1;
                });
            }
        };
        match lib.run_index_upto(workers, CHUNK, stop, &on_progress) {
            Ok(p) if p.total > 0 => {
                done += p.done;
                failed += p.failed;
                set(&|s| {
                    s.done = done;
                    s.failed = failed;
                    s.generation += 1;
                });
                if lib.models_dir().is_some() {
                    recognize();
                }
            }
            Ok(_) => break,
            Err(e) => {
                tracing::error!(error = %e, "indexing failed");
                break;
            }
        }
    }
}
