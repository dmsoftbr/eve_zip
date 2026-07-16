mod app;
mod format;
mod preview;

use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use evezip_core::{ArchiveEngine, JobQueue, Location};

fn main() {
    let engine = match evezip_engine::Engine::locate() {
        Ok(e) => Arc::new(e) as Arc<dyn ArchiveEngine>,
        Err(e) => {
            eprintln!("EveZip: {e}. Rode scripts/fetch-7zz.sh.");
            std::process::exit(1);
        }
    };
    let inicial = Location::Disk(
        dirs_home().unwrap_or_else(|| std::path::PathBuf::from("/")),
    );
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

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}
