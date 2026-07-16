use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Cli {
    Abrir(Option<PathBuf>),
    Extrair {
        archive: PathBuf,
        dest: Option<PathBuf>,
    },
}

/// Destino padrão de `evezip x <archive>` quando `-o` não é informado.
///
/// `Path::parent()` de um nome relativo "bare" (ex.: "bare.7z") retorna
/// `Some("")` (path vazio), não `None` — então um simples
/// `.unwrap_or(Path::new("."))` nunca aciona o fallback e o 7zz recebe
/// `-o` com destino vazio ("Too short switch: -o"). Trata explicitamente
/// o parent vazio como ".".
pub fn destino_padrao(archive: &Path, dest: Option<PathBuf>) -> PathBuf {
    dest.unwrap_or_else(|| match archive.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    })
}

/// args SEM o argv[0].
pub fn parse(args: &[String]) -> Result<Cli, String> {
    match args {
        [] => Ok(Cli::Abrir(None)),
        [cmd, resto @ ..] if cmd == "x" => match resto {
            [archive] => Ok(Cli::Extrair {
                archive: archive.into(),
                dest: None,
            }),
            [archive, flag, dest] if flag == "-o" => Ok(Cli::Extrair {
                archive: archive.into(),
                dest: Some(dest.into()),
            }),
            _ => Err("uso: evezip x <archive> [-o <destino>]".into()),
        },
        [caminho] => Ok(Cli::Abrir(Some(caminho.into()))),
        _ => Err("uso: evezip [<archive>] | evezip x <archive> [-o <destino>]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sem_args_abre_ui() {
        assert_eq!(parse(&[]).unwrap(), Cli::Abrir(None));
    }

    #[test]
    fn com_caminho_abre_no_archive() {
        assert_eq!(
            parse(&v(&["/x/a.7z"])).unwrap(),
            Cli::Abrir(Some("/x/a.7z".into()))
        );
    }

    #[test]
    fn x_extrai() {
        assert_eq!(
            parse(&v(&["x", "/x/a.7z"])).unwrap(),
            Cli::Extrair {
                archive: "/x/a.7z".into(),
                dest: None
            }
        );
        assert_eq!(
            parse(&v(&["x", "/x/a.7z", "-o", "/out"])).unwrap(),
            Cli::Extrair {
                archive: "/x/a.7z".into(),
                dest: Some("/out".into())
            }
        );
    }

    #[test]
    fn x_sem_archive_da_erro() {
        assert!(parse(&v(&["x"])).is_err());
        assert!(parse(&v(&["x", "/a.7z", "-o"])).is_err());
    }

    #[test]
    fn destino_padrao_bare_usa_diretorio_atual() {
        assert_eq!(destino_padrao(Path::new("a.7z"), None), PathBuf::from("."));
        assert_eq!(
            destino_padrao(Path::new("bare.7z"), None),
            PathBuf::from(".")
        );
    }

    #[test]
    fn destino_padrao_com_dir_usa_parent() {
        assert_eq!(
            destino_padrao(Path::new("dir/a.7z"), None),
            PathBuf::from("dir")
        );
        assert_eq!(
            destino_padrao(Path::new("/x/a.7z"), None),
            PathBuf::from("/x")
        );
    }

    #[test]
    fn destino_padrao_explicito_passa_direto() {
        assert_eq!(
            destino_padrao(Path::new("bare.7z"), Some(PathBuf::from("/out"))),
            PathBuf::from("/out")
        );
    }
}
