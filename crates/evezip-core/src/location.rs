use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Disk(PathBuf),
    Archive { archive: PathBuf, inner: String },
}

impl Location {
    pub fn parent(&self) -> Option<Location> {
        match self {
            Location::Disk(p) => p.parent().map(|pp| Location::Disk(pp.to_path_buf())),
            Location::Archive { archive, inner } => {
                if inner.is_empty() {
                    let dir = archive.parent().unwrap_or(archive.as_path());
                    Some(Location::Disk(dir.to_path_buf()))
                } else {
                    let novo = match inner.rsplit_once('/') {
                        Some((pai, _)) => pai.to_string(),
                        None => String::new(),
                    };
                    Some(Location::Archive {
                        archive: archive.clone(),
                        inner: novo,
                    })
                }
            }
        }
    }

    pub fn display(&self) -> String {
        match self {
            Location::Disk(p) => p.display().to_string(),
            Location::Archive { archive, inner } => {
                if inner.is_empty() {
                    format!("{}/", archive.display())
                } else {
                    format!("{}/{}/", archive.display(), inner)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parent_dentro_do_archive() {
        let l = Location::Archive {
            archive: PathBuf::from("/x/a.7z"),
            inner: "src/sub".into(),
        };
        match l.parent().unwrap() {
            Location::Archive { inner, .. } => assert_eq!(inner, "src"),
            _ => panic!(),
        }
    }

    #[test]
    fn parent_da_raiz_do_archive_volta_ao_disco() {
        let l = Location::Archive {
            archive: PathBuf::from("/x/a.7z"),
            inner: String::new(),
        };
        match l.parent().unwrap() {
            Location::Disk(p) => assert_eq!(p, PathBuf::from("/x")),
            _ => panic!(),
        }
    }

    #[test]
    fn parent_de_disco() {
        assert!(matches!(
            Location::Disk(PathBuf::from("/x/y")).parent(),
            Some(Location::Disk(_))
        ));
        assert!(Location::Disk(PathBuf::from("/")).parent().is_none());
    }

    #[test]
    fn display_de_archive() {
        let l = Location::Archive {
            archive: PathBuf::from("/x/a.7z"),
            inner: "src".into(),
        };
        assert_eq!(l.display(), "/x/a.7z/src/");
    }
}
