mod app;
mod cli;
mod format;
mod preview;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use evezip_core::{ArchiveEngine, Browser, Config, JobQueue, Location};

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
    let dest = cli::destino_padrao(&archive, dest);
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
    let config = Config::load();
    let inicial = match caminho {
        Some(p) if Browser::is_archive_file(&p.to_string_lossy()) => Location::Archive {
            archive: p,
            inner: String::new(),
        },
        Some(p) => Location::Disk(p),
        // Sem caminho na linha de comando: retoma a última pasta usada se ela
        // ainda existir; caso contrário, abre no home.
        None => Location::Disk(pasta_inicial(&config)),
    };
    let app = app::App::new(Arc::clone(&engine), inicial).expect("falha ao criar janela");

    // Mapa id → descrição do job, usado para preencher `JobRow.descricao`
    // quando o evento `Started` chega (ele só carrega o id). Instância única,
    // dona do processo `main`, compartilhada (via Arc<Mutex<_>>, pois a
    // thread-ponte abaixo precisa ser `Send`) entre os callbacks da UI
    // (`submeter`) e a thread-ponte (`aplicar_evento`).
    let descricoes: Arc<Mutex<HashMap<evezip_core::JobId, String>>> = Arc::default();

    // Mapa id → JobKind: guarda o "molde" de cada job para poder reenviá-lo com
    // a senha digitada quando ele falha por senha (SenhaNecessaria/Incorreta).
    // JobKind é `Send` (só caminhos e opções), então acompanha `descricoes`
    // pela thread-ponte e é limpo em `aplicar_evento` nos eventos terminais.
    let kinds: Arc<Mutex<HashMap<evezip_core::JobId, evezip_core::JobKind>>> = Arc::default();

    // Fila de jobs + thread-ponte: eventos do worker (thread separada) são
    // encaminhados para o thread da UI via `upgrade_in_event_loop`.
    let (tx, rx) = mpsc::channel::<evezip_core::JobEvent>();
    let queue = Arc::new(JobQueue::new(engine, tx));
    app.instalar_fila(
        Arc::clone(&queue),
        Arc::clone(&descricoes),
        Arc::clone(&kinds),
    );

    let weak = app.window_weak();
    std::thread::spawn(move || {
        for ev in rx {
            let weak = weak.clone();
            let descricoes = Arc::clone(&descricoes);
            let kinds = Arc::clone(&kinds);
            let _ = weak
                .upgrade_in_event_loop(move |w| app::aplicar_evento(&w, &descricoes, &kinds, ev));
        }
    });

    app.run().expect("event loop");

    // Persiste a pasta atual (última usada) para a próxima abertura.
    let dir_atual = pasta_de(&app.state.borrow().location);
    let mut config = config;
    config.ultima_pasta = Some(dir_atual);
    let _ = config.save();

    preview::limpar_temp();
}

/// Pasta em que a navegação está agora: o próprio diretório no disco, ou o
/// diretório que contém o archive aberto.
fn pasta_de(location: &Location) -> PathBuf {
    match location {
        Location::Disk(dir) => dir.clone(),
        Location::Archive { archive, .. } => archive
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs_home().unwrap_or_else(|| PathBuf::from("/"))),
    }
}

/// Pasta inicial ao abrir sem argumento: a última usada, se ainda existir;
/// senão, o home.
fn pasta_inicial(config: &Config) -> PathBuf {
    match &config.ultima_pasta {
        Some(p) if p.is_dir() => p.clone(),
        _ => dirs_home().unwrap_or_else(|| PathBuf::from("/")),
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
