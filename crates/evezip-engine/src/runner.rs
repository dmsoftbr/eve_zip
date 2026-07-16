use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::cancel::CancelToken;
use crate::error::EngineError;
use crate::parse_progress::parse_progress;

/// Executa o 7zz, transmite progresso e mapeia o resultado para EngineError.
pub fn run_7zz(
    bin: &Path,
    args: &[OsString],
    on_progress: &mut dyn FnMut(u8),
    cancel: &CancelToken,
) -> Result<String, EngineError> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");
    let child = Arc::new(Mutex::new(child));
    let terminou = Arc::new(AtomicBool::new(false));

    // Watcher: mata o processo se o token for cancelado.
    let watcher = {
        let child = Arc::clone(&child);
        let cancel = cancel.clone();
        let terminou = Arc::clone(&terminou);
        std::thread::spawn(move || loop {
            if terminou.load(Ordering::SeqCst) {
                return;
            }
            if cancel.is_cancelled() {
                let _ = child.lock().unwrap().kill();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        })
    };

    // stderr em thread própria para não travar o pipe.
    let stderr_handle = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    // Lê stdout em blocos; tokens separados por '\r'/'\n' podem conter progresso.
    let mut saida = String::new();
    let mut parcial = String::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = stdout.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let texto = String::from_utf8_lossy(&buf[..n]);
        saida.push_str(&texto);
        for ch in texto.chars() {
            if ch == '\r' || ch == '\n' {
                if let Some(p) = parse_progress(&parcial) {
                    on_progress(p);
                }
                parcial.clear();
            } else {
                parcial.push(ch);
            }
        }
    }
    if let Some(p) = parse_progress(&parcial) {
        on_progress(p);
    }

    let status = child.lock().unwrap().wait()?;
    terminou.store(true, Ordering::SeqCst);
    let _ = watcher.join();
    let stderr_texto = stderr_handle.join().unwrap_or_default();

    if cancel.is_cancelled() {
        return Err(EngineError::Cancelado);
    }
    match status.code() {
        Some(0) => Ok(saida),
        Some(code) => Err(mapear_erro(code, &stderr_texto)),
        None => Err(EngineError::Cancelado), // morto por sinal
    }
}

fn mapear_erro(exit_code: i32, stderr: &str) -> EngineError {
    let s = stderr.to_lowercase();
    if s.contains("wrong password") {
        return EngineError::SenhaIncorreta;
    }
    if s.contains("no space left") || s.contains("not enough space") {
        return EngineError::SemEspacoEmDisco;
    }
    for marcador in [
        "headers error",
        "data error",
        "crc failed",
        "unexpected end of archive",
        "cannot open the file as archive",
        "is not archive",
    ] {
        if s.contains(marcador) {
            return EngineError::ArchiveCorrompido(stderr.trim().to_string());
        }
    }
    EngineError::Falha {
        exit_code,
        stderr: stderr.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locate::find_7zz;

    fn args(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn sucesso_retorna_stdout() {
        let bin = find_7zz().unwrap();
        let out = run_7zz(&bin, &args(&["i"]), &mut |_| {}, &CancelToken::new()).unwrap();
        assert!(out.contains("7-Zip"));
    }

    #[test]
    fn falha_mapeia_para_erro_tipado() {
        let bin = find_7zz().unwrap();
        let err = run_7zz(
            &bin,
            &args(&["l", "-slt", "/caminho/que/nao/existe.7z"]),
            &mut |_| {},
            &CancelToken::new(),
        )
        .unwrap_err();
        assert!(matches!(err, EngineError::Falha { .. } | EngineError::ArchiveCorrompido(_)));
    }

    #[test]
    fn mapeamento_de_stderr() {
        assert!(matches!(
            mapear_erro(2, "ERROR: Data Error in encrypted file. Wrong password?"),
            EngineError::SenhaIncorreta
        ));
        assert!(matches!(
            mapear_erro(2, "Cannot open encrypted archive. Wrong password?"),
            EngineError::SenhaIncorreta
        ));
        assert!(matches!(
            mapear_erro(2, "ERROR: CRC Failed : a.txt"),
            EngineError::ArchiveCorrompido(_)
        ));
        assert!(matches!(
            mapear_erro(2, "Headers Error"),
            EngineError::ArchiveCorrompido(_)
        ));
        assert!(matches!(
            mapear_erro(2, "ERROR: No space left on device"),
            EngineError::SemEspacoEmDisco
        ));
        assert!(matches!(
            mapear_erro(1, "algum aviso"),
            EngineError::Falha { exit_code: 1, .. }
        ));
    }
}
