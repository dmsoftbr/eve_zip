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
    // Acumula bytes crus e só decodifica UTF-8 por token completo (ou no fim, para
    // a saída inteira) — assim um caractere multibyte (ex.: nome de arquivo
    // acentuado) partido entre dois chunks de 4096 bytes nunca vira U+FFFD.
    //
    // Todo o corpo do loop fica dentro de uma closure que retorna Result: se o
    // `read` falhar no meio da leitura, o `?` sai da closure (não da função), o
    // que garante que a limpeza abaixo (kill em caso de erro, wait, marcar
    // `terminou`, join das threads) sempre rode antes de qualquer erro ser
    // propagado para o chamador.
    let leitura: std::io::Result<Vec<u8>> = (|| {
        let mut saida = Vec::new();
        let mut parcial: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = stdout.read(&mut buf)?;
            if n == 0 {
                break;
            }
            saida.extend_from_slice(&buf[..n]);
            for &b in &buf[..n] {
                if b == b'\r' || b == b'\n' {
                    if !parcial.is_empty() {
                        let token = String::from_utf8_lossy(&parcial);
                        if let Some(p) = parse_progress(&token) {
                            on_progress(p);
                        }
                        parcial.clear();
                    }
                } else {
                    parcial.push(b);
                }
            }
        }
        if !parcial.is_empty() {
            let token = String::from_utf8_lossy(&parcial);
            if let Some(p) = parse_progress(&token) {
                on_progress(p);
            }
        }
        Ok(saida)
    })();

    // Se a leitura do stdout falhou, o processo pode ainda estar rodando; tenta
    // matá-lo antes de seguir para a limpeza (ignora erro: pode já ter morrido).
    if leitura.is_err() {
        let _ = child.lock().unwrap().kill();
    }

    // Invariante: ao sair do loop acima por EOF (n == 0), o 7zz já fechou o pipe
    // de stdout, o que só acontece quando o processo está encerrando. Por isso
    // este wait() tende a retornar quase instantaneamente e não fica preso
    // segurando o lock do child — o que deixaria o watcher sem conseguir
    // adquirir o lock para matar o processo em caso de cancelamento.
    let status_resultado = child.lock().unwrap().wait();

    terminou.store(true, Ordering::SeqCst);
    let _ = watcher.join();
    let stderr_texto = stderr_handle.join().unwrap_or_default();

    // Só agora, com child aguardado/finalizado, watcher e stderr_handle
    // encerrados, propagamos qualquer erro pendente da leitura ou do wait.
    let saida_bytes = leitura?;
    let status = status_resultado?;

    if cancel.is_cancelled() {
        return Err(EngineError::Cancelado);
    }
    match status.code() {
        Some(0) => Ok(String::from_utf8_lossy(&saida_bytes).into_owned()),
        Some(code) => Err(mapear_erro(code, &stderr_texto)),
        // Morte por sinal (crash/OOM-kill) que não veio do nosso watcher (já
        // tratado acima via cancel.is_cancelled()) deve virar um erro mapeado,
        // não Cancelado.
        None => Err(mapear_erro(-1, &stderr_texto)),
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

    #[test]
    fn cancelamento_encerra_processo_de_execucao_longa() {
        let bin = find_7zz().unwrap();
        let cancel = CancelToken::new();
        let cancel_watcher = cancel.clone();

        let inicio = std::time::Instant::now();
        let handle = std::thread::spawn(move || {
            run_7zz(&bin, &args(&["b", "-mmt1"]), &mut |_| {}, &cancel_watcher)
        });

        std::thread::sleep(std::time::Duration::from_millis(300));
        cancel.cancel();

        let resultado = handle.join().expect("thread do run_7zz não deve entrar em pânico");
        let decorrido = inicio.elapsed();

        assert!(
            matches!(resultado, Err(EngineError::Cancelado)),
            "esperava Err(Cancelado), obteve {resultado:?}"
        );
        assert!(
            decorrido < std::time::Duration::from_secs(15),
            "cancelamento deveria ser rápido (child.kill()), mas levou {decorrido:?}"
        );
    }
}
