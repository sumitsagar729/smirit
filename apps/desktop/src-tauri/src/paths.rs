use std::path::{Path, PathBuf};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Where the library (database, thumbnails, models) lives.
/// `SMRITI_LIBRARY_DIR` overrides the default `%LOCALAPPDATA%\Smriti`.
pub fn library_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SMRITI_LIBRARY_DIR") {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("app", "Smriti", "Smriti")
        .map(|d| d.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("Smriti"))
}

/// Logs to stderr and to daily-rotated files in `<library>/logs`.
pub fn init_logging(library_dir: &Path) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,smriti=debug"));
    let logs = library_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let file = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("smriti")
        .filename_suffix("log")
        .max_log_files(14)
        .build(logs)
        .ok();

    let registry = tracing_subscriber::registry().with(filter).with(fmt::layer().with_writer(std::io::stderr));
    match file {
        Some(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            registry.with(fmt::layer().with_ansi(false).with_writer(writer)).init();
            Some(guard)
        }
        None => {
            registry.init();
            None
        }
    }
}
