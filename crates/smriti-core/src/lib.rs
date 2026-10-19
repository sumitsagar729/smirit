//! Smriti core: everything the app knows about a photo library.
//!
//! The UI never touches files or SQL directly; it calls into this crate through
//! the desktop shell's commands.

pub mod error;
pub mod id;
pub mod library;
pub mod migrations;
pub mod sources;

pub use error::{Error, Result};
pub use id::Id;
pub use library::{Library, LibraryStats};
pub use sources::Source;
