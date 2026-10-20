//! Smriti core: everything the app knows about a photo library.
//!
//! The UI never touches files or SQL directly; it calls into this crate through
//! the desktop shell's commands.

pub mod error;
pub mod faces;
pub mod filename_date;
pub mod geo;
pub mod id;
pub mod indexer;
pub mod library;
pub mod media;
pub mod metadata;
pub mod migrations;
pub mod people;
pub mod places;
pub mod scan;
pub mod search;
pub mod sources;
pub mod thumbs;
pub mod timeline;

pub use error::{Error, Result};
pub use id::Id;
pub use indexer::IndexProgress;
pub use library::{Library, LibraryStats};
pub use media::Kind;
pub use people::{AssetFace, FaceGroup, FaceRef, Person, PersonRef, PersonUpdate, RecognizeReport};
pub use places::{Home, PlaceSummary, PlacesReport, Stop, Trip};
pub use scan::ScanReport;
pub use search::{AgeFilter, Chip, Query, Suggestion};
pub use sources::Source;
