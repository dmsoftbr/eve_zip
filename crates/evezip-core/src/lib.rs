//! Domínio do EveZip: árvore de archive, navegação unificada, fila de jobs, config.

pub mod browser;
pub mod engine_trait;
pub mod location;
pub mod tree;

pub use browser::{Browser, Row};
pub use engine_trait::ArchiveEngine;
pub use location::Location;
pub use tree::ArchiveTree;
