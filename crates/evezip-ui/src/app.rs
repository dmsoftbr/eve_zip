use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Model, ModelRc, StandardListViewItem, VecModel, Weak};

use evezip_core::{
    ArchiveEngine, Browser, JobEvent, JobId, JobKind, JobQueue, JobSpec, Location, Row,
};

use crate::format::tamanho_humano;

slint::include_modules!();

pub struct App {
    pub window: AppWindow,
    pub state: Rc<std::cell::RefCell<State>>,
}

pub struct State {
    pub location: Location,
    pub browser: Browser,
    pub rows: Vec<Row>,
    pub ultimo_clique: Option<(i32, Instant)>,
    pub senha_do_archive: Option<String>, // preenchida pelo fluxo de senha (Task 13)
    /// Guarda contra diálogos empilhados: true enquanto um `CreateDialog` ou
    /// `PasswordDialog` está aberto. Todo caminho que dispara um desses
    /// diálogos deve checar essa flag antes de abrir outro, e todo caminho
    /// que fecha um diálogo (confirmar-sucesso, confirmar-erro-que-esconde,
    /// cancelar) deve resetá-la — um `true` vazado trava a UI para sempre.
    pub dialogo_aberto: bool,
}

impl State {
    /// Getter usado pelo preview (Task 14): expõe o `Arc<dyn ArchiveEngine>`
    /// do `Browser` sem expor o `Browser` inteiro.
    pub fn browser_engine(&self) -> Arc<dyn ArchiveEngine> {
        self.browser.engine()
    }
}

pub fn linhas_para_modelo(rows: &[Row]) -> ModelRc<ModelRc<StandardListViewItem>> {
    let linhas: Vec<ModelRc<StandardListViewItem>> = rows
        .iter()
        .map(|r| {
            let nome = if r.is_dir {
                format!("📁 {}", r.name)
            } else {
                format!("📄 {}", r.name)
            };
            let celulas: Vec<StandardListViewItem> = vec![
                StandardListViewItem::from(nome.as_str()),
                StandardListViewItem::from(
                    if r.is_dir {
                        "—".to_string()
                    } else {
                        tamanho_humano(r.size)
                    }
                    .as_str(),
                ),
                StandardListViewItem::from(
                    if r.packed_size == 0 {
                        "".to_string()
                    } else {
                        tamanho_humano(r.packed_size)
                    }
                    .as_str(),
                ),
                StandardListViewItem::from(r.modified.as_str()),
            ];
            ModelRc::new(VecModel::from(celulas))
        })
        .collect();
    ModelRc::new(VecModel::from(linhas))
}

impl App {
    pub fn new(
        engine: Arc<dyn ArchiveEngine>,
        inicial: Location,
    ) -> Result<App, slint::PlatformError> {
        let window = AppWindow::new()?;
        let state = Rc::new(std::cell::RefCell::new(State {
            location: inicial,
            browser: Browser::new(engine),
            rows: Vec::new(),
            ultimo_clique: None,
            senha_do_archive: None,
            dialogo_aberto: false,
        }));
        let app = App { window, state };
        app.recarregar();
        app.instalar_callbacks();
        Ok(app)
    }

    pub fn recarregar(&self) {
        let mut s = self.state.borrow_mut();
        let senha = s.senha_do_archive.clone();
        let loc = s.location.clone();
        match s.browser.list(&loc, senha.as_deref()) {
            Ok(rows) => {
                self.window.set_caminho(loc.display().into());
                self.window
                    .set_status(format!("{} itens", rows.len()).into());
                self.window.set_rows(linhas_para_modelo(&rows));
                s.rows = rows;
            }
            Err(e) => {
                self.window.set_status(format!("Erro: {e}").into());
            }
        }
    }

    fn instalar_callbacks(&self) {
        let state = Rc::clone(&self.state);
        let weak = self.window.as_weak();

        // Duplo-clique manual: dois cliques na mesma linha em < 400ms.
        self.window.on_linha_click({
            let state = Rc::clone(&state);
            let weak = weak.clone();
            move |row| {
                let agora = Instant::now();
                let duplo = {
                    let mut s = state.borrow_mut();
                    let duplo = matches!(
                        s.ultimo_clique,
                        Some((r, t)) if r == row && agora.duration_since(t) < Duration::from_millis(400)
                    );
                    s.ultimo_clique = if duplo { None } else { Some((row, agora)) };
                    duplo
                };
                if !duplo {
                    return;
                }
                let (loc, nova) = {
                    let s = state.borrow();
                    let Some(r) = s.rows.get(row as usize) else { return };
                    // Arquivo dentro de um archive: preview via app padrão do
                    // SO em vez de navegar (Task 14). `browser.enter` devolve
                    // a própria location nesse caso (dead-end), então
                    // intercepta antes de chegar lá.
                    if let Location::Archive { archive, inner } = &s.location {
                        if !r.is_dir {
                            let entrada = if inner.is_empty() {
                                r.name.clone()
                            } else {
                                format!("{inner}/{}", r.name)
                            };
                            let archive = archive.clone();
                            let senha = s.senha_do_archive.clone();
                            let eng = s.browser_engine();
                            let res = crate::preview::abrir_preview(
                                &eng,
                                &archive,
                                &entrada,
                                senha.as_deref(),
                            );
                            if let (Err(e), Some(w)) = (res, weak.upgrade()) {
                                w.set_status(format!("Erro no preview: {e}").into());
                            }
                            return;
                        }
                    }
                    let nova = s.browser.enter(&s.location, &r.name, r.is_dir);
                    (s.location.clone(), nova)
                };
                if nova != loc {
                    let senha = state.borrow().senha_do_archive.clone();
                    let resultado = state.borrow_mut().browser.list(&nova, senha.as_deref());
                    match resultado {
                        Ok(_) => {
                            // Navegação válida: troca de location e, se saiu do
                            // archive atual (ou foi para o disco), esquece a senha.
                            let manter_senha = mesmo_archive(&loc, &nova);
                            {
                                let mut s = state.borrow_mut();
                                s.location = nova;
                                if !manter_senha {
                                    s.senha_do_archive = None;
                                }
                            }
                            if let Some(w) = weak.upgrade() {
                                recarregar_janela(&w, &state);
                            }
                        }
                        Err(evezip_engine::EngineError::SenhaNecessaria) => {
                            pedir_senha_e_navegar(&state, &weak, nova, "Este archive exige senha.");
                        }
                        Err(evezip_engine::EngineError::SenhaIncorreta) => {
                            pedir_senha_e_navegar(&state, &weak, nova, "Senha incorreta, tente novamente.");
                        }
                        Err(e) => {
                            // Navegação falhou (ex.: archive corrompido): NÃO navega
                            // e NÃO recarrega — a tabela continua mostrando o
                            // diretório atual; só a status bar é atualizada.
                            if let Some(w) = weak.upgrade() {
                                w.set_status(format!("Erro: {e}").into());
                            }
                        }
                    }
                }
            }
        });

        self.window.on_subir({
            let state = Rc::clone(&state);
            let weak = weak.clone();
            move || {
                let pai = state.borrow().location.parent();
                if let Some(p) = pai {
                    state.borrow_mut().location = p;
                    if let Some(w) = weak.upgrade() {
                        recarregar_janela(&w, &state);
                    }
                }
            }
        });

        // extrair/testar ganham corpo em `instalar_fila` (chamado pelo main.rs
        // após a fila de jobs existir). adicionar ganha corpo na Task 13.
        self.window.on_extrair(|| {});
        self.window.on_adicionar(|| {});
        self.window.on_testar(|| {});
    }

    pub fn window_weak(&self) -> Weak<AppWindow> {
        self.window.as_weak()
    }

    /// Liga extrair/testar/cancelar-job à fila de jobs. Chamado pelo `main.rs`
    /// depois de `App::new`, uma vez que a fila (e a ponte de eventos) exista.
    /// Substitui os `on_extrair`/`on_testar` vazios instalados em
    /// `instalar_callbacks`.
    pub fn instalar_fila(
        &self,
        queue: Arc<JobQueue>,
        descricoes: Arc<Mutex<HashMap<JobId, String>>>,
        kinds: Arc<Mutex<HashMap<JobId, JobKind>>>,
    ) {
        let state = Rc::clone(&self.state);
        let weak = self.window.as_weak();
        let q = Arc::clone(&queue);
        let descricoes1 = Arc::clone(&descricoes);
        let kinds1 = Arc::clone(&kinds);

        self.window.on_extrair(move || {
            let (archive, senha, entrada) = {
                let s = state.borrow();
                match &s.location {
                    Location::Archive { archive, inner } => {
                        // Extrai a linha selecionada (se houver); senão, o archive inteiro.
                        let linha = weak.upgrade().map(|w| w.get_linha_atual()).unwrap_or(-1);
                        let sel = if linha >= 0 {
                            s.rows.get(linha as usize).map(|r| {
                                let base = if inner.is_empty() {
                                    r.name.clone()
                                } else {
                                    format!("{inner}/{}", r.name)
                                };
                                vec![base]
                            })
                        } else {
                            None
                        };
                        (archive.clone(), s.senha_do_archive.clone(), sel)
                    }
                    Location::Disk(dir) => {
                        // No disco: extrai o archive selecionado na tabela.
                        let linha = weak.upgrade().map(|w| w.get_linha_atual()).unwrap_or(-1);
                        let Some(r) = (linha >= 0)
                            .then(|| s.rows.get(linha as usize))
                            .flatten()
                            .filter(|r| Browser::is_archive_file(&r.name))
                        else {
                            if let Some(w) = weak.upgrade() {
                                w.set_status("Selecione um archive para extrair".into());
                            }
                            return;
                        };
                        (dir.join(&r.name), None, None)
                    }
                }
            };
            let Some(dest) = rfd::FileDialog::new()
                .set_title("Extrair para...")
                .pick_folder()
            else {
                return;
            };
            submeter(
                &q,
                &descricoes1,
                &kinds1,
                JobSpec {
                    descricao: format!(
                        "Extrair {}",
                        archive
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    ),
                    kind: JobKind::Extract {
                        archive,
                        dest,
                        entries: entrada,
                        password: senha,
                    },
                },
            );
        });

        let q = Arc::clone(&queue);
        let state2 = Rc::clone(&self.state);
        let descricoes2 = Arc::clone(&descricoes);
        let kinds2 = Arc::clone(&kinds);
        self.window.on_testar(move || {
            let s = state2.borrow();
            if let Location::Archive { archive, .. } = &s.location {
                submeter(
                    &q,
                    &descricoes2,
                    &kinds2,
                    JobSpec {
                        descricao: format!("Testar {}", archive.display()),
                        kind: JobKind::Test {
                            archive: archive.clone(),
                            password: s.senha_do_archive.clone(),
                        },
                    },
                );
            }
        });

        let q = Arc::clone(&queue);
        let descricoes3 = Arc::clone(&descricoes);
        let kinds3 = Arc::clone(&kinds);
        let state3 = Rc::clone(&self.state);
        let weak4 = self.window.as_weak();
        self.window.on_adicionar(move || {
            // Guarda contra diálogos empilhados: se um CreateDialog/PasswordDialog
            // já está aberto (janela principal continua interativa), ignora o
            // disparo em vez de abrir outro por cima.
            if state3.borrow().dialogo_aberto {
                if let Some(w) = weak4.upgrade() {
                    w.set_status("Feche o diálogo aberto primeiro.".into());
                }
                return;
            }

            // 1. Escolher entradas (arquivos a compactar).
            let Some(inputs) = rfd::FileDialog::new()
                .set_title("Arquivos para compactar")
                .pick_files()
            else {
                return;
            };

            // 2. Abrir o diálogo de opções de criação.
            let dlg = CreateDialog::new().expect("dialog");
            state3.borrow_mut().dialogo_aberto = true;
            let dlg_weak = dlg.as_weak();
            let q = Arc::clone(&q);
            let descricoes = Arc::clone(&descricoes3);
            let kinds = Arc::clone(&kinds3);
            let dir_atual = match &state3.borrow().location {
                Location::Disk(d) => d.clone(),
                Location::Archive { archive, .. } => archive
                    .parent()
                    .unwrap_or(std::path::Path::new("/"))
                    .to_path_buf(),
            };

            dlg.on_cancelar({
                let w = dlg_weak.clone();
                let state3 = Rc::clone(&state3);
                move || {
                    state3.borrow_mut().dialogo_aberto = false;
                    if let Some(d) = w.upgrade() {
                        let _ = d.hide();
                    }
                }
            });

            let state_confirmar = Rc::clone(&state3);
            dlg.on_confirmar(move |formato, nivel, senha, criptografar_nomes| {
                use evezip_engine::{CreateOptions, Format};
                let (fmt, ext) = match formato.as_str() {
                    "zip" => (Format::Zip, "zip"),
                    "tar" => (Format::Tar, "tar"),
                    "tar.gz" => (Format::TarGz, "tar.gz"),
                    _ => (Format::SevenZ, "7z"),
                };
                let sugestao = format!("novo.{ext}");
                let Some(destino) = rfd::FileDialog::new()
                    .set_title("Salvar archive como...")
                    .set_directory(&dir_atual)
                    .set_file_name(&sugestao)
                    .save_file()
                else {
                    // Diálogo de destino cancelado: o CreateDialog continua
                    // aberto, então a flag permanece true.
                    return;
                };
                // O 7zz (via evezip-engine::ops::create) rejeita senha em
                // tar/tar.gz com `OpcaoNaoSuportada` — o diálogo já desabilita
                // os campos de senha/nomes para esses formatos, mas isso é
                // reforçado aqui em defesa de profundidade (o campo poderia
                // reter texto de uma seleção anterior de formato).
                let eh_tar = matches!(fmt, Format::Tar | Format::TarGz);
                let password = if eh_tar {
                    None
                } else {
                    (!senha.is_empty()).then(|| senha.to_string())
                };
                let encrypt_names = criptografar_nomes && fmt == Format::SevenZ;
                submeter(
                    &q,
                    &descricoes,
                    &kinds,
                    JobSpec {
                        descricao: format!(
                            "Criar {}",
                            destino
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default()
                        ),
                        kind: JobKind::Create {
                            archive: destino,
                            inputs: inputs.clone(),
                            options: CreateOptions {
                                format: fmt,
                                level: nivel as u8,
                                password,
                                encrypt_names,
                            },
                        },
                    },
                );
                state_confirmar.borrow_mut().dialogo_aberto = false;
                if let Some(d) = dlg_weak.upgrade() {
                    let _ = d.hide();
                }
            });
            let _ = dlg.show();
        });

        let q_senha = Arc::clone(&queue);
        let descricoes_senha = Arc::clone(&descricoes);
        let kinds_senha = Arc::clone(&kinds);
        let state_senha = Rc::clone(&self.state);
        let weak_senha = self.window.as_weak();
        self.window.on_pedir_senha_job(move |id, incorreta| {
            pedir_senha_e_reenviar_job(
                &q_senha,
                &descricoes_senha,
                &kinds_senha,
                &state_senha,
                &weak_senha,
                id as JobId,
                incorreta,
            );
        });

        let q = Arc::clone(&queue);
        self.window.on_cancelar_job(move |id| q.cancel(id as JobId));
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }
}

/// Duas locations apontam para o mesmo archive (independente do `inner`)?
/// Usado para decidir se a senha do archive deve ser mantida ao navegar
/// entre diretórios internos do mesmo archive, ou esquecida ao trocar de
/// archive (ou voltar ao disco).
fn mesmo_archive(a: &Location, b: &Location) -> bool {
    matches!(
        (a, b),
        (Location::Archive { archive: a1, .. }, Location::Archive { archive: a2, .. })
            if a1 == a2
    )
}

/// Abre o `PasswordDialog` para uma navegação que falhou com `SenhaNecessaria`
/// ou `SenhaIncorreta`, e tenta `list` de novo com a senha digitada. Sucesso:
/// grava a senha em `state.senha_do_archive`, navega para `nova` e recarrega a
/// janela. `SenhaIncorreta` de novo: reexibe o diálogo com nova mensagem
/// (permite repetir quantas vezes o usuário quiser). Qualquer outro erro:
/// mostra na status bar e fecha o diálogo.
fn pedir_senha_e_navegar(
    state: &Rc<std::cell::RefCell<State>>,
    weak: &slint::Weak<AppWindow>,
    nova: Location,
    mensagem: &str,
) {
    // Guarda contra diálogos empilhados: se um diálogo já está aberto (ex.:
    // outra navegação com senha em curso), ignora este disparo.
    if state.borrow().dialogo_aberto {
        if let Some(w) = weak.upgrade() {
            w.set_status("Feche o diálogo aberto primeiro.".into());
        }
        return;
    }
    let dlg = PasswordDialog::new().expect("dialog");
    dlg.set_mensagem(mensagem.into());
    state.borrow_mut().dialogo_aberto = true;
    let dlg_weak = dlg.as_weak();
    dlg.on_cancelar({
        let w = dlg_weak.clone();
        let state = Rc::clone(state);
        move || {
            state.borrow_mut().dialogo_aberto = false;
            if let Some(d) = w.upgrade() {
                let _ = d.hide();
            }
        }
    });
    let state = Rc::clone(state);
    let weak = weak.clone();
    dlg.on_confirmar(move |senha| {
        let resultado = {
            let mut s = state.borrow_mut();
            s.browser.list(&nova, Some(senha.as_str())).map(|_| ())
        };
        match resultado {
            Ok(()) => {
                {
                    let mut s = state.borrow_mut();
                    s.senha_do_archive = Some(senha.to_string());
                    s.location = nova.clone();
                    s.dialogo_aberto = false;
                }
                if let Some(w) = weak.upgrade() {
                    recarregar_janela(&w, &state);
                }
                if let Some(d) = dlg_weak.upgrade() {
                    let _ = d.hide();
                }
            }
            Err(evezip_engine::EngineError::SenhaIncorreta) => {
                // Diálogo permanece aberto para nova tentativa: flag continua
                // true. Limpa o campo de senha para não reexibir a senha errada.
                if let Some(d) = dlg_weak.upgrade() {
                    d.set_mensagem("Senha incorreta, tente novamente.".into());
                    d.set_senha_texto("".into());
                }
            }
            Err(e) => {
                state.borrow_mut().dialogo_aberto = false;
                if let Some(w) = weak.upgrade() {
                    w.set_status(format!("Erro: {e}").into());
                }
                if let Some(d) = dlg_weak.upgrade() {
                    let _ = d.hide();
                }
            }
        }
    });
    let _ = dlg.show();
}

fn nome_de(p: &std::path::Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// Abre o `PasswordDialog` para um job de extração/teste que falhou por senha
/// (`SenhaNecessaria`/`SenhaIncorreta`) e, na confirmação, reenvia o MESMO job
/// com a senha digitada. Guarda a senha em `state.senha_do_archive` para
/// operações seguintes. O reprompt é natural: se a senha estiver errada, o novo
/// job falha de novo e `aplicar_evento` dispara este fluxo outra vez (agora com
/// `incorreta = true`). Só é chamado para `Extract`/`Test` (garantido em
/// `aplicar_evento`).
fn pedir_senha_e_reenviar_job(
    q: &Arc<JobQueue>,
    descricoes: &Arc<Mutex<HashMap<JobId, String>>>,
    kinds: &Arc<Mutex<HashMap<JobId, JobKind>>>,
    state: &Rc<std::cell::RefCell<State>>,
    weak: &Weak<AppWindow>,
    id: JobId,
    incorreta: bool,
) {
    // Recupera o molde do job falho e o remove do mapa PRIMEIRO — assim a
    // entrada nunca vaza, mesmo que não seja possível abrir o diálogo agora.
    // O reenvio cria um id novo, que `submeter` registra de novo.
    let Some(kind) = kinds.lock().unwrap().remove(&id) else {
        return;
    };
    // Guarda contra diálogos empilhados: se já há um diálogo aberto (ex.: uma
    // senha de navegação em curso), avisa e não empilha outro. O job fica
    // marcado "aguardando senha"; o usuário refaz a ação depois de fechar o
    // diálogo aberto (um novo job então pede a senha normalmente).
    if state.borrow().dialogo_aberto {
        if let Some(w) = weak.upgrade() {
            w.set_status("Feche o diálogo de senha aberto e tente novamente.".into());
        }
        return;
    }

    let dlg = PasswordDialog::new().expect("dialog");
    dlg.set_mensagem(if incorreta {
        "Senha incorreta, tente novamente.".into()
    } else {
        "Este archive exige senha.".into()
    });
    state.borrow_mut().dialogo_aberto = true;
    let dlg_weak = dlg.as_weak();

    dlg.on_cancelar({
        let w = dlg_weak.clone();
        let state = Rc::clone(state);
        move || {
            state.borrow_mut().dialogo_aberto = false;
            if let Some(d) = w.upgrade() {
                let _ = d.hide();
            }
        }
    });

    let q = Arc::clone(q);
    let descricoes = Arc::clone(descricoes);
    let kinds = Arc::clone(kinds);
    let state = Rc::clone(state);
    dlg.on_confirmar(move |senha| {
        let senha = senha.to_string();
        // Injeta a senha no molde do job e monta uma descrição fresca (a antiga
        // já foi removida em `aplicar_evento`).
        let (novo_kind, descricao) = match &kind {
            JobKind::Extract {
                archive,
                dest,
                entries,
                ..
            } => (
                JobKind::Extract {
                    archive: archive.clone(),
                    dest: dest.clone(),
                    entries: entries.clone(),
                    password: Some(senha.clone()),
                },
                format!("Extrair {}", nome_de(archive)),
            ),
            JobKind::Test { archive, .. } => (
                JobKind::Test {
                    archive: archive.clone(),
                    password: Some(senha.clone()),
                },
                format!("Testar {}", nome_de(archive)),
            ),
            // Create nunca chega aqui (aplicar_evento só dispara p/ Extract/Test).
            outro => (outro.clone(), "Reenviar".to_string()),
        };
        // Guarda a senha para operações seguintes (otimista; se errada, o
        // reprompt a substitui na próxima falha).
        state.borrow_mut().senha_do_archive = Some(senha);
        submeter(
            &q,
            &descricoes,
            &kinds,
            JobSpec {
                descricao,
                kind: novo_kind,
            },
        );
        state.borrow_mut().dialogo_aberto = false;
        if let Some(d) = dlg_weak.upgrade() {
            let _ = d.hide();
        }
    });
    let _ = dlg.show();
}

/// Abre um caminho vindo do SO (Finder / `open` no macOS): navega até o archive
/// (ou entra na pasta). Se o archive exigir senha, dispara o fluxo de senha.
/// Arquivos comuns são ignorados.
pub fn abrir_caminho_externo(
    state: &Rc<std::cell::RefCell<State>>,
    weak: &Weak<AppWindow>,
    path: std::path::PathBuf,
) {
    let nova = if Browser::is_archive_file(&path.to_string_lossy()) {
        Location::Archive {
            archive: path,
            inner: String::new(),
        }
    } else if path.is_dir() {
        Location::Disk(path)
    } else {
        return;
    };

    let senha = state.borrow().senha_do_archive.clone();
    let resultado = state.borrow_mut().browser.list(&nova, senha.as_deref());
    match resultado {
        Ok(_) => {
            {
                let mut s = state.borrow_mut();
                s.location = nova;
                s.senha_do_archive = None;
            }
            if let Some(w) = weak.upgrade() {
                recarregar_janela(&w, state);
            }
        }
        Err(evezip_engine::EngineError::SenhaNecessaria) => {
            pedir_senha_e_navegar(state, weak, nova, "Este archive exige senha.");
        }
        Err(evezip_engine::EngineError::SenhaIncorreta) => {
            pedir_senha_e_navegar(state, weak, nova, "Senha incorreta, tente novamente.");
        }
        Err(e) => {
            if let Some(w) = weak.upgrade() {
                w.set_status(format!("Erro: {e}").into());
            }
        }
    }
}

/// Recarga usada de dentro dos callbacks (sem &self).
pub fn recarregar_janela(w: &AppWindow, state: &Rc<std::cell::RefCell<State>>) {
    let mut s = state.borrow_mut();
    let senha = s.senha_do_archive.clone();
    let loc = s.location.clone();
    match s.browser.list(&loc, senha.as_deref()) {
        Ok(rows) => {
            w.set_caminho(loc.display().into());
            w.set_status(format!("{} itens", rows.len()).into());
            w.set_rows(linhas_para_modelo(&rows));
            s.rows = rows;
        }
        Err(e) => w.set_status(format!("Erro: {e}").into()),
    }
}

/// Único ponto de submissão de jobs na UI: registra a descrição em `descricoes`
/// e submete o job na fila. Todo `q.submit(spec)` de um callback da UI deve
/// passar por aqui.
///
/// Invariante estrutural: esta função só é chamada a partir do thread da UI
/// (dentro de um callback Slint, ex.: `on_extrair`/`on_testar`). Isso garante
/// que a inserção no mapa aconteça antes de o event loop processar o evento
/// `Started` correspondente — que também só é tratado no thread da UI, via
/// `Weak::upgrade_in_event_loop` (ver `aplicar_evento`/`main.rs`). Sem essa
/// garantia (ex.: se `submeter` fosse chamada de outra thread), haveria uma
/// corrida entre o insert e o `Started` chegando primeiro.
pub fn submeter(
    q: &JobQueue,
    descricoes: &Mutex<HashMap<JobId, String>>,
    kinds: &Mutex<HashMap<JobId, JobKind>>,
    spec: JobSpec,
) -> JobId {
    let descricao = spec.descricao.clone();
    let kind = spec.kind.clone();
    let id = q.submit(spec);
    descricoes.lock().unwrap().insert(id, descricao);
    kinds.lock().unwrap().insert(id, kind);
    id
}

/// Aplica um `JobEvent` ao modelo `jobs` da janela. Roda no thread da UI
/// (agendado via `Weak::upgrade_in_event_loop` a partir da thread-ponte em
/// `main.rs`).
pub fn aplicar_evento(
    w: &AppWindow,
    descricoes: &Mutex<HashMap<JobId, String>>,
    kinds: &Mutex<HashMap<JobId, JobKind>>,
    ev: JobEvent,
) {
    type Mudanca = Box<dyn Fn(&mut JobRow)>;

    let modelo = w.get_jobs();
    let mut jobs: Vec<JobRow> = modelo.iter().collect();

    // Se um job de extração/teste falhou por senha, dispara o fluxo de reprompt
    // depois de atualizar o modelo (id-do-job, senha-estava-incorreta). Fica
    // fora de qualquer lock de mutex para não colidir com o handler que roda
    // sincronamente ao invocar o callback.
    let mut precisa_senha: Option<(i32, bool)> = None;

    let (id, mudanca): (JobId, Mudanca) = match ev {
        JobEvent::Started(id) => {
            let descricao = descricoes
                .lock()
                .unwrap()
                .get(&id)
                .cloned()
                .unwrap_or_default();
            jobs.push(JobRow {
                id: id as i32,
                descricao: descricao.into(),
                progresso: 0,
                estado: "executando".into(),
            });
            (id, Box::new(|_: &mut JobRow| {}))
        }
        JobEvent::Progress(id, p) => (id, Box::new(move |j: &mut JobRow| j.progresso = p as i32)),
        JobEvent::Done(id) => {
            descricoes.lock().unwrap().remove(&id);
            kinds.lock().unwrap().remove(&id);
            (
                id,
                Box::new(|j: &mut JobRow| {
                    j.progresso = 100;
                    j.estado = "concluído".into();
                }),
            )
        }
        JobEvent::Failed(id, e) => {
            descricoes.lock().unwrap().remove(&id);
            // Falha por senha em extração/teste: mantém o kind no mapa para o
            // handler reenviar o job com a senha digitada. Outros erros (ou
            // kinds que não sejam Extract/Test) limpam o mapa e só mostram o erro.
            let incorreta = matches!(e, evezip_engine::EngineError::SenhaIncorreta);
            let por_senha = matches!(
                e,
                evezip_engine::EngineError::SenhaNecessaria
                    | evezip_engine::EngineError::SenhaIncorreta
            );
            let kind_reenviavel = matches!(
                kinds.lock().unwrap().get(&id),
                Some(JobKind::Extract { .. } | JobKind::Test { .. })
            );
            if por_senha && kind_reenviavel {
                precisa_senha = Some((id as i32, incorreta));
                (
                    id,
                    Box::new(|j: &mut JobRow| j.estado = "aguardando senha".into()),
                )
            } else {
                kinds.lock().unwrap().remove(&id);
                let msg = slint::SharedString::from(format!("erro: {e}"));
                (id, Box::new(move |j: &mut JobRow| j.estado = msg.clone()))
            }
        }
        JobEvent::Cancelled(id) => {
            descricoes.lock().unwrap().remove(&id);
            kinds.lock().unwrap().remove(&id);
            (id, Box::new(|j: &mut JobRow| j.estado = "cancelado".into()))
        }
    };
    for j in jobs.iter_mut() {
        if j.id == id as i32 {
            mudanca(j);
        }
    }
    w.set_jobs(ModelRc::new(VecModel::from(jobs)));

    // Nenhum lock está retido aqui: seguro invocar o callback (que roda
    // sincronamente no thread da UI e pode abrir o PasswordDialog).
    if let Some((id, incorreta)) = precisa_senha {
        w.invoke_pedir_senha_job(id, incorreta);
    }
}

#[cfg(test)]
mod jobs_tests {
    //! Testa `submeter`/`descricoes` sem UI: são funções livres que não
    //! dependem da janela Slint (só `aplicar_evento` precisa de `AppWindow`,
    //! que exige uma janela real — fora do escopo automatizável aqui).
    use super::*;
    use evezip_engine::{ArchiveEntry, CancelToken, CreateOptions, EngineError};
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::mpsc;

    struct EngineFalso;
    impl ArchiveEngine for EngineFalso {
        fn list(&self, _: &Path, _: Option<&str>) -> Result<Vec<ArchiveEntry>, EngineError> {
            Ok(vec![])
        }
        fn extract(
            &self,
            _: &Path,
            _: &Path,
            _: Option<&[String]>,
            _: Option<&str>,
            _: &mut dyn FnMut(u8),
            _: &CancelToken,
        ) -> Result<(), EngineError> {
            Ok(())
        }
        fn create(
            &self,
            _: &Path,
            _: &[PathBuf],
            _: &CreateOptions,
            _: &mut dyn FnMut(u8),
            _: &CancelToken,
        ) -> Result<(), EngineError> {
            Ok(())
        }
        fn test(
            &self,
            _: &Path,
            _: Option<&str>,
            _: &mut dyn FnMut(u8),
            _: &CancelToken,
        ) -> Result<(), EngineError> {
            Ok(())
        }
    }

    #[test]
    fn submeter_registra_descricao_no_mapa() {
        let (tx, rx) = mpsc::channel();
        let q = JobQueue::new(Arc::new(EngineFalso), tx);
        let descricoes: Mutex<HashMap<JobId, String>> = Mutex::new(HashMap::new());
        let kinds: Mutex<HashMap<JobId, JobKind>> = Mutex::new(HashMap::new());
        let id = submeter(
            &q,
            &descricoes,
            &kinds,
            JobSpec {
                descricao: "Testar a.7z".into(),
                kind: JobKind::Test {
                    archive: PathBuf::from("/x/a.7z"),
                    password: None,
                },
            },
        );

        // `submeter` insere nos dois mapas antes de retornar — não depende de
        // nenhum evento chegar para isso ser verdade.
        assert_eq!(
            descricoes.lock().unwrap().get(&id).cloned(),
            Some("Testar a.7z".to_string())
        );
        assert!(
            matches!(kinds.lock().unwrap().get(&id), Some(JobKind::Test { .. })),
            "kind do job deveria estar registrado"
        );

        // Consome os eventos (Started + terminal) para não vazar a thread do
        // worker (join implícito via drop do sender ao sair de escopo já é
        // suficiente para o teste, mas drenar deixa a intenção clara).
        let started = rx.recv().unwrap();
        assert!(matches!(started, JobEvent::Started(i) if i == id));
        let _ = rx.recv();
    }
}
