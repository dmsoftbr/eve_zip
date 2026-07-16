//! Preview de um arquivo dentro de um archive: extrai a entrada única para um
//! diretório temporário estável (`evezip-preview-<pid>`) e abre com o app
//! padrão do SO. `limpar_temp` remove esse diretório (chamado ao sair, em
//! `main.rs`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use evezip_core::ArchiveEngine;
use evezip_engine::{CancelToken, EngineError};

fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("evezip-preview-{}", std::process::id()))
}

pub fn abrir_preview(
    engine: &Arc<dyn ArchiveEngine>,
    archive: &Path,
    entry_path: &str,
    password: Option<&str>,
) -> Result<(), EngineError> {
    // Defesa em profundidade (zip-slip): mesmo que o 7zz recuse a extração
    // de uma entrada ".." em si, `dest.join(entry_path)` abaixo é o caminho
    // que de fato abrimos — se `entry_path` contiver traversal, o alvo
    // poderia apontar para fora do diretório temporário de preview.
    if !evezip_engine::ops::entrada_segura(entry_path) {
        return Err(EngineError::CaminhoInvalido(entry_path.to_string()));
    }
    let dest = temp_root();
    std::fs::create_dir_all(&dest)?;
    engine.extract(
        archive,
        &dest,
        Some(&[entry_path.to_string()]),
        password,
        &mut |_| {},
        &CancelToken::new(),
    )?;
    let alvo = dest.join(entry_path);
    // Segunda camada, best-effort: se ambos canonicalizarem, o alvo precisa
    // continuar dentro do destino. Se `alvo` não existir (extração falhou
    // silenciosamente por algum motivo), não há o que canonicalizar — a
    // checagem acima já barrou o caso hostil conhecido.
    if let (Ok(alvo_canon), Ok(dest_canon)) = (alvo.canonicalize(), dest.canonicalize()) {
        if !alvo_canon.starts_with(&dest_canon) {
            return Err(EngineError::CaminhoInvalido(entry_path.to_string()));
        }
    }
    if std::env::var_os("EVEZIP_PREVIEW_NO_OPEN").is_none() {
        abrir_com_app_padrao(&alvo)?;
    }
    Ok(())
}

pub fn limpar_temp() {
    let _ = std::fs::remove_dir_all(temp_root());
}

fn abrir_com_app_padrao(p: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut cmd = { let mut c = std::process::Command::new("open"); c.arg(p); c };
    #[cfg(target_os = "linux")]
    let mut cmd = { let mut c = std::process::Command::new("xdg-open"); c.arg(p); c };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]).arg(p);
        c
    };
    cmd.spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use evezip_engine::{ArchiveEntry, CreateOptions};
    use std::sync::Mutex;

    struct EngineGravador {
        extraidos: Mutex<Vec<String>>,
    }

    impl ArchiveEngine for EngineGravador {
        fn list(&self, _: &Path, _: Option<&str>) -> Result<Vec<ArchiveEntry>, EngineError> {
            Ok(vec![])
        }
        fn extract(
            &self, _: &Path, dest: &Path, entries: Option<&[String]>, _: Option<&str>,
            _: &mut dyn FnMut(u8), _: &CancelToken,
        ) -> Result<(), EngineError> {
            let e = entries.unwrap()[0].clone();
            let alvo = dest.join(&e);
            std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
            std::fs::write(&alvo, "conteudo").unwrap();
            self.extraidos.lock().unwrap().push(e);
            Ok(())
        }
        fn create(&self, _: &Path, _: &[PathBuf], _: &CreateOptions, _: &mut dyn FnMut(u8), _: &CancelToken) -> Result<(), EngineError> { Ok(()) }
        fn test(&self, _: &Path, _: Option<&str>, _: &mut dyn FnMut(u8), _: &CancelToken) -> Result<(), EngineError> { Ok(()) }
    }

    #[test]
    fn extrai_entrada_unica_para_temp() {
        // Só valida a extração; abrir o app padrão é ignorado se EVEZIP_PREVIEW_NO_OPEN=1.
        std::env::set_var("EVEZIP_PREVIEW_NO_OPEN", "1");
        let eng: Arc<dyn ArchiveEngine> = Arc::new(EngineGravador { extraidos: Mutex::new(vec![]) });
        abrir_preview(&eng, Path::new("/x/a.7z"), "sub/nota.txt", None).unwrap();
        assert!(temp_root().join("sub/nota.txt").exists());
        limpar_temp();
        assert!(!temp_root().exists());
    }

    #[test]
    fn rejeita_entry_path_com_traversal_antes_de_extrair() {
        // Zip-slip: uma entrada "../../../foo" nunca deve chegar a extract()
        // nem a dest.join() — o preview tem que barrar antes.
        std::env::set_var("EVEZIP_PREVIEW_NO_OPEN", "1");
        let eng: Arc<dyn ArchiveEngine> = Arc::new(EngineGravador { extraidos: Mutex::new(vec![]) });
        let err = abrir_preview(&eng, Path::new("/x/a.7z"), "../../../etc/passwd", None).unwrap_err();
        assert!(matches!(err, EngineError::CaminhoInvalido(_)), "{err:?}");
        limpar_temp();
    }
}
