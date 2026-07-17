//! Domínio do EveZip: árvore de archive, navegação unificada, fila de jobs, config.

pub mod browser;
pub mod config;
pub mod engine_trait;
pub mod jobs;
pub mod location;
pub mod tree;

pub use browser::{Browser, Row};
pub use config::Config;
pub use engine_trait::ArchiveEngine;
pub use jobs::{JobEvent, JobId, JobKind, JobQueue, JobSpec};
pub use location::Location;
pub use tree::ArchiveTree;
