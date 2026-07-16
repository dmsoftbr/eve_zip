//! Wrapper do binário 7zz: única camada que conhece o 7-Zip.
pub mod cancel;
pub mod entry;
pub mod error;
pub mod locate;
pub mod ops;
pub mod parse_list;
pub mod parse_progress;
pub mod runner;

pub use cancel::CancelToken;
pub use entry::ArchiveEntry;
pub use error::EngineError;
pub use ops::{CreateOptions, Engine, Format};
