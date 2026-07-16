use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum Cli {
    Abrir(Option<PathBuf>),
    Extrair { archive: PathBuf, dest: Option<PathBuf> },
}

/// args SEM o argv[0].
pub fn parse(args: &[String]) -> Result<Cli, String> {
    match args {
        [] => Ok(Cli::Abrir(None)),
        [cmd, resto @ ..] if cmd == "x" => match resto {
            [archive] => Ok(Cli::Extrair { archive: archive.into(), dest: None }),
            [archive, flag, dest] if flag == "-o" => {
                Ok(Cli::Extrair { archive: archive.into(), dest: Some(dest.into()) })
            }
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
        assert_eq!(parse(&v(&["/x/a.7z"])).unwrap(), Cli::Abrir(Some("/x/a.7z".into())));
    }

    #[test]
    fn x_extrai() {
        assert_eq!(
            parse(&v(&["x", "/x/a.7z"])).unwrap(),
            Cli::Extrair { archive: "/x/a.7z".into(), dest: None }
        );
        assert_eq!(
            parse(&v(&["x", "/x/a.7z", "-o", "/out"])).unwrap(),
            Cli::Extrair { archive: "/x/a.7z".into(), dest: Some("/out".into()) }
        );
    }

    #[test]
    fn x_sem_archive_da_erro() {
        assert!(parse(&v(&["x"])).is_err());
        assert!(parse(&v(&["x", "/a.7z", "-o"])).is_err());
    }
}
