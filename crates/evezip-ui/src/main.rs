mod app;
mod cli;
mod format;
mod preview;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use evezip_core::{ArchiveEngine, Browser, JobQueue, Location};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let comando = match cli::parse(&args) {
        Ok(c) => c,
        Err(uso) => {
            eprintln!("{uso}");
            std::process::exit(2);
        }
    };

    let engine = match evezip_engine::Engine::locate() {
        Ok(e) => Arc::new(e) as Arc<dyn ArchiveEngine>,
        Err(e) => {
            eprintln!("EveZip: {e}. Rode scripts/fetch-7zz.sh.");
            std::process::exit(1);
        }
    };

    match comando {
        cli::Cli::Extrair { archive, dest } => extrair_headless(&engine, archive, dest),
        cli::Cli::Abrir(caminho) => abrir_ui(engine, caminho),
    }
}

/// Extração sem UI: progresso percentual em stderr, exit code ≠ 0 em erro.
fn extrair_headless(engine: &Arc<dyn ArchiveEngine>, archive: PathBuf, dest: Option<PathBuf>) {
    let dest = dest
        .unwrap_or_else(|| archive.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());
    let mut ultimo = 0u8;
    let r = engine.extract(
        &archive,
        &dest,
        None,
        None,
        &mut |p| {
            if p != ultimo {
                ultimo = p;
                eprint!("\r{p}%");
            }
        },
        &evezip_engine::CancelToken::new(),
    );
    eprintln!();
    match r {
        Ok(()) => println!("extraído em {}", dest.display()),
        Err(e) => {
            eprintln!("erro: {e}");
            std::process::exit(1);
        }
    }
}

/// Abre a janela principal, opcionalmente já posicionada num archive ou pasta.
fn abrir_ui(engine: Arc<dyn ArchiveEngine>, caminho: Option<PathBuf>) {
    let inicial = match caminho {
        Some(p) if Browser::is_archive_file(&p.to_string_lossy()) => {
            Location::Archive { archive: p, inner: String::new() }
        }
        Some(p) => Location::Disk(p),
        None => Location::Disk(dirs_home().unwrap_or_else(|| PathBuf::from("/"))),
    };
    let app = app::App::new(Arc::clone(&engine), inicial).expect("falha ao criar janela");

    // Mapa id → descrição do job, usado para preencher `JobRow.descricao`
    // quando o evento `Started` chega (ele só carrega o id). Instância única,
    // dona do processo `main`, compartilhada (via Arc<Mutex<_>>, pois a
    // thread-ponte abaixo precisa ser `Send`) entre os callbacks da UI
    // (`submeter`) e a thread-ponte (`aplicar_evento`).
    let descricoes: Arc<Mutex<HashMap<evezip_core::JobId, String>>> = Arc::default();

    // Fila de jobs + thread-ponte: eventos do worker (thread separada) são
    // encaminhados para o thread da UI via `upgrade_in_event_loop`.
    let (tx, rx) = mpsc::channel::<evezip_core::JobEvent>();
    let queue = Arc::new(JobQueue::new(engine, tx));
    app.instalar_fila(queue, Arc::clone(&descricoes));

    let weak = app.window_weak();
    std::thread::spawn(move || {
        for ev in rx {
            let weak = weak.clone();
            let descricoes = Arc::clone(&descricoes);
            let _ = weak.upgrade_in_event_loop(move |w| app::aplicar_evento(&w, &descricoes, ev));
        }
    });

    app.run().expect("event loop");
    preview::limpar_temp();
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
