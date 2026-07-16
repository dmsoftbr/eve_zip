use std::path::{Path, PathBuf};

use evezip_engine::{ArchiveEntry, CancelToken, CreateOptions, EngineError};

pub trait ArchiveEngine: Send + Sync + 'static {
    fn list(
        &self,
        archive: &Path,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, EngineError>;
    fn extract(
        &self,
        archive: &Path,
        dest: &Path,
        entries: Option<&[String]>,
        password: Option<&str>,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError>;
    fn create(
        &self,
        archive: &Path,
        inputs: &[PathBuf],
        options: &CreateOptions,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError>;
    fn test(
        &self,
        archive: &Path,
        password: Option<&str>,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError>;
}

impl ArchiveEngine for evezip_engine::Engine {
    fn list(
        &self,
        archive: &Path,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, EngineError> {
        evezip_engine::Engine::list(self, archive, password)
    }
    fn extract(
        &self,
        archive: &Path,
        dest: &Path,
        entries: Option<&[String]>,
        password: Option<&str>,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        evezip_engine::Engine::extract(self, archive, dest, entries, password, on_progress, cancel)
    }
    fn create(
        &self,
        archive: &Path,
        inputs: &[PathBuf],
        options: &CreateOptions,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        evezip_engine::Engine::create(self, archive, inputs, options, on_progress, cancel)
    }
    fn test(
        &self,
        archive: &Path,
        password: Option<&str>,
        on_progress: &mut dyn FnMut(u8),
        cancel: &CancelToken,
    ) -> Result<(), EngineError> {
        evezip_engine::Engine::test(self, archive, password, on_progress, cancel)
    }
}
