use std::path::PathBuf;

use crate::error::EngineError;

/// Ordem de busca: env EVEZIP_7ZZ → diretório do executável → vendor/ (dev) → PATH.
pub fn find_7zz() -> Result<PathBuf, EngineError> {
    if let Ok(p) = std::env::var("EVEZIP_7ZZ") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Ok(p);
        }
    }

    let names: &[&str] = if cfg!(windows) {
        &["7z.exe", "7zz.exe"]
    } else {
        &["7zz"]
    };

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for n in names {
                let p = dir.join(n);
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    // Desenvolvimento: vendor/ na raiz do workspace.
    let plat = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => "macos",
        ("linux", "x86_64") => "linux-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("windows", _) => "windows-x64",
        _ => "",
    };
    if !plat.is_empty() {
        let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../vendor/7zz")
            .join(plat);
        for n in names {
            let p = vendor.join(n);
            if p.exists() {
                return Ok(p);
            }
        }
    }

    // PATH
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            for n in names {
                let p = dir.join(n);
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    Err(EngineError::BinarioNaoEncontrado)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // find_7zz() lê a variável de ambiente global EVEZIP_7ZZ. Os testes abaixo
    // fazem set_var/remove_var nela, e o Rust roda testes em paralelo por padrão,
    // então sem essa serialização há uma corrida latente entre eles (flake em CI).
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn respeita_variavel_de_ambiente() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let fake = std::env::temp_dir().join("evezip-fake-7zz");
        std::fs::write(&fake, b"").unwrap();
        std::env::set_var("EVEZIP_7ZZ", &fake);
        assert_eq!(find_7zz().unwrap(), fake);
        std::env::remove_var("EVEZIP_7ZZ");
    }

    #[test]
    fn encontra_binario_vendorizado() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("EVEZIP_7ZZ");
        // pré-condição: scripts/fetch-7zz.sh já rodou
        let p = find_7zz().expect("rode scripts/fetch-7zz.sh antes dos testes");
        assert!(p.exists());
    }
}
