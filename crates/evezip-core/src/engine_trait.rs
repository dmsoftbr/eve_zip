use std::path::Path;

use evezip_engine::{ArchiveEntry, EngineError};

pub trait ArchiveEngine: Send + Sync + 'static {
    fn list(&self, archive: &Path, password: Option<&str>) -> Result<Vec<ArchiveEntry>, EngineError>;
}

impl ArchiveEngine for evezip_engine::Engine {
    fn list(&self, archive: &Path, password: Option<&str>) -> Result<Vec<ArchiveEntry>, EngineError> {
        evezip_engine::Engine::list(self, archive, password)
    }
}
