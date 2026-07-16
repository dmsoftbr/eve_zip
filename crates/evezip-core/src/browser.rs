use std::path::{Path, PathBuf};
use std::sync::Arc;

use evezip_engine::EngineError;

use crate::engine_trait::ArchiveEngine;
use crate::location::Location;
use crate::tree::ArchiveTree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub size: u64,
    pub packed_size: u64,
    pub modified: String,
    pub is_dir: bool,
}

pub struct Browser {
    engine: Arc<dyn ArchiveEngine>,
    cache: Option<(PathBuf, ArchiveTree)>,
}

const EXTENSOES: &[&str] = &["zip", "7z", "rar", "tar", "gz", "tgz"];

impl Browser {
    pub fn new(engine: Arc<dyn ArchiveEngine>) -> Self {
        Browser { engine, cache: None }
    }

    pub fn is_archive_file(name: &str) -> bool {
        let lower = name.to_lowercase();
        EXTENSOES.iter().any(|e| lower.ends_with(&format!(".{e}")))
    }

    pub fn list(&mut self, loc: &Location, password: Option<&str>) -> Result<Vec<Row>, EngineError> {
        match loc {
            Location::Disk(dir) => {
                let mut rows = Vec::new();
                for item in std::fs::read_dir(dir)? {
                    let item = item?;
                    let meta = item.metadata()?;
                    let name = item.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue; // ocultos fora da v1
                    }
                    rows.push(Row {
                        name,
                        size: if meta.is_dir() { 0 } else { meta.len() },
                        packed_size: 0,
                        modified: String::new(),
                        is_dir: meta.is_dir(),
                    });
                }
                rows.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
                Ok(rows)
            }
            Location::Archive { archive, inner } => {
                let precisa_carregar =
                    !matches!(&self.cache, Some((p, _)) if p == archive);
                if precisa_carregar {
                    let entradas = self.engine.list(archive, password)?;
                    self.cache = Some((archive.clone(), ArchiveTree::build(&entradas)));
                }
                let (_, tree) = self.cache.as_ref().unwrap();
                tree.list_dir(inner).ok_or_else(|| {
                    EngineError::Falha { exit_code: -1, stderr: format!("caminho não existe no archive: {inner}") }
                })
            }
        }
    }

    pub fn enter(&self, loc: &Location, name: &str, is_dir: bool) -> Location {
        match loc {
            Location::Disk(dir) => {
                let alvo = dir.join(name);
                if is_dir {
                    Location::Disk(alvo)
                } else if Self::is_archive_file(name) {
                    Location::Archive { archive: alvo, inner: String::new() }
                } else {
                    loc.clone()
                }
            }
            Location::Archive { archive, inner } => {
                if is_dir {
                    let novo = if inner.is_empty() { name.to_string() } else { format!("{inner}/{name}") };
                    Location::Archive { archive: archive.clone(), inner: novo }
                } else {
                    loc.clone() // arquivo dentro de archive: preview (Task 14), não navegação
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evezip_engine::ArchiveEntry;
    use std::sync::Mutex;

    struct EngineFalso {
        chamadas: Mutex<u32>,
    }

    impl ArchiveEngine for EngineFalso {
        fn list(&self, _a: &Path, _p: Option<&str>) -> Result<Vec<ArchiveEntry>, EngineError> {
            *self.chamadas.lock().unwrap() += 1;
            Ok(vec![ArchiveEntry {
                path: "src/main.rs".into(),
                size: 10,
                packed_size: 5,
                modified: None,
                is_dir: false,
                encrypted: false,
            }])
        }
    }

    #[test]
    fn lista_archive_com_cache() {
        let falso = Arc::new(EngineFalso { chamadas: Mutex::new(0) });
        let mut b = Browser::new(falso.clone());
        let loc = Location::Archive { archive: PathBuf::from("/x/a.7z"), inner: String::new() };
        let raiz = b.list(&loc, None).unwrap();
        assert_eq!(raiz[0].name, "src");
        let dentro = Location::Archive { archive: PathBuf::from("/x/a.7z"), inner: "src".into() };
        let sub = b.list(&dentro, None).unwrap();
        assert_eq!(sub[0].name, "main.rs");
        assert_eq!(*falso.chamadas.lock().unwrap(), 1); // uma listagem só, resto do cache
    }

    #[test]
    fn detecta_extensoes_de_archive() {
        assert!(Browser::is_archive_file("a.ZIP"));
        assert!(Browser::is_archive_file("b.tar.gz"));
        assert!(!Browser::is_archive_file("c.txt"));
    }

    #[test]
    fn enter_em_dir_de_disco_e_em_archive() {
        let falso = Arc::new(EngineFalso { chamadas: Mutex::new(0) });
        let b = Browser::new(falso);
        let disco = Location::Disk(PathBuf::from("/x"));
        assert_eq!(b.enter(&disco, "sub", true), Location::Disk(PathBuf::from("/x/sub")));
        assert_eq!(
            b.enter(&disco, "a.7z", false),
            Location::Archive { archive: PathBuf::from("/x/a.7z"), inner: String::new() }
        );
    }
}
