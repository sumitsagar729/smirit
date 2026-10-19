use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("database migration failed: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("folder not found: {}", .0.display())]
    FolderNotFound(PathBuf),

    #[error("{} is already part of the library", .0.display())]
    SourceAlreadyAdded(PathBuf),

    #[error("{} overlaps with an existing source ({})", .new.display(), .existing.display())]
    SourceOverlaps { new: PathBuf, existing: PathBuf },

    #[error("the library folder itself cannot be added as a source")]
    SourceIsLibrary,

    #[error("not found: {0}")]
    NotFound(String),
}

impl Error {
    /// Stable machine-readable code for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Db(_) => "db",
            Error::Migration(_) => "migration",
            Error::Io(_) => "io",
            Error::FolderNotFound(_) => "folder_not_found",
            Error::SourceAlreadyAdded(_) => "source_already_added",
            Error::SourceOverlaps { .. } => "source_overlaps",
            Error::SourceIsLibrary => "source_is_library",
            Error::NotFound(_) => "not_found",
        }
    }
}
