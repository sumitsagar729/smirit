//! Runs scan + index in the background and tells the UI how it's going.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crossbeam_channel::{Sender, unbounded};
use serde::Serialize;
use smriti_core::{IndexProgress, Library};
use tauri::{AppHandle, Emitter};

pub const STATUS_EVENT: &str = "index-status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Idle,
    Scanning,
    Indexing,
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
}

pub struct Indexer {
    wake: Sender<()>,
    status: Arc<Mutex<IndexStatus>>,
    cancel: Arc<AtomicBool>,
}

impl Indexer {
    pub fn start(app: AppHandle, lib: Arc<Library>) -> Self {
        let (wake, rx) = unbounded::<()>();
        let status =
            Arc::new(Mutex::new(IndexStatus { phase: Phase::Idle, total: 0, done: 0, failed: 0, generation: 0 }));
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
                // First run at start-up, then whenever woken.
                loop {
                    while rx.try_recv().is_ok() {} // coalesce repeated wake-ups
                    set(&|s| s.phase = Phase::Scanning);
                    match lib.scan_all(&stop) {
                        Ok(r) if r.added + r.changed + r.missing + r.restored > 0 => set(&|s| s.generation += 1),
                        Ok(_) => {}
                        Err(e) => tracing::error!(error = %e, "scan failed"),
                    }

                    let last = Mutex::new(Instant::now());
                    let on_progress = |p: IndexProgress| {
                        let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                        if last.elapsed() >= Duration::from_millis(400) {
                            *last = Instant::now();
                            set(&|s| {
                                s.phase = Phase::Indexing;
                                s.total = p.total;
                                s.done = p.done;
                                s.failed = p.failed;
                                s.generation += 1;
                            });
                        }
                    };
                    match lib.run_index(workers, &stop, &on_progress) {
                        Ok(p) if p.total > 0 => set(&|s| {
                            s.total = p.total;
                            s.done = p.done;
                            s.failed = p.failed;
                            s.generation += 1;
                        }),
                        Ok(_) => {}
                        Err(e) => tracing::error!(error = %e, "indexing failed"),
                    }
                    set(&|s| s.phase = Phase::Idle);

                    if stop.load(Ordering::Relaxed) || rx.recv().is_err() {
                        break;
                    }
                }
            })
            .expect("spawn indexer thread");

        Self { wake, status, cancel }
    }

    /// Asks for a rescan; cheap to call repeatedly.
    pub fn wake(&self) {
        let _ = self.wake.send(());
    }

    pub fn status(&self) -> IndexStatus {
        *self.status.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn shutdown(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.wake.send(());
    }
}
