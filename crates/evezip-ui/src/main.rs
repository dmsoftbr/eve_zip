mod app;
mod format;

use std::sync::{mpsc, Arc};

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

    // Fila de jobs + thread-ponte: eventos do worker (thread separada) são
    // encaminhados para o thread da UI via `upgrade_in_event_loop`.
    let (tx, rx) = mpsc::channel::<evezip_core::JobEvent>();
    let queue = Arc::new(JobQueue::new(engine, tx));
    app.instalar_fila(queue);

    let weak = app.window_weak();
    std::thread::spawn(move || {
        for ev in rx {
            let weak = weak.clone();
            let _ = weak.upgrade_in_event_loop(move |w| app::aplicar_evento(&w, ev));
        }
    });

    app.run().expect("event loop");
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}
