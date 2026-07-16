mod app;
mod format;

use std::sync::Arc;

use evezip_core::Location;

fn main() {
    let engine = match evezip_engine::Engine::locate() {
        Ok(e) => Arc::new(e),
        Err(e) => {
            eprintln!("EveZip: {e}. Rode scripts/fetch-7zz.sh.");
            std::process::exit(1);
        }
    };
    let inicial = Location::Disk(
        dirs_home().unwrap_or_else(|| std::path::PathBuf::from("/")),
    );
    let app = app::App::new(engine, inicial).expect("falha ao criar janela");
    app.run().expect("event loop");
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}
