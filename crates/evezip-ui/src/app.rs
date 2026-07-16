use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, ModelRc, StandardListViewItem, VecModel};

use evezip_core::{ArchiveEngine, Browser, Location, Row};

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
}

pub fn linhas_para_modelo(rows: &[Row]) -> ModelRc<ModelRc<StandardListViewItem>> {
    let linhas: Vec<ModelRc<StandardListViewItem>> = rows
        .iter()
        .map(|r| {
            let nome = if r.is_dir { format!("📁 {}", r.name) } else { format!("📄 {}", r.name) };
            let celulas: Vec<StandardListViewItem> = vec![
                StandardListViewItem::from(nome.as_str()),
                StandardListViewItem::from(
                    if r.is_dir { "—".to_string() } else { tamanho_humano(r.size) }.as_str(),
                ),
                StandardListViewItem::from(
                    if r.packed_size == 0 { "".to_string() } else { tamanho_humano(r.packed_size) }
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
    pub fn new(engine: Arc<dyn ArchiveEngine>, inicial: Location) -> Result<App, slint::PlatformError> {
        let window = AppWindow::new()?;
        let state = Rc::new(std::cell::RefCell::new(State {
            location: inicial,
            browser: Browser::new(engine),
            rows: Vec::new(),
            ultimo_clique: None,
            senha_do_archive: None,
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
                    let nova = s.browser.enter(&s.location, &r.name, r.is_dir);
                    (s.location.clone(), nova)
                };
                if nova != loc {
                    let ok = {
                        let mut s = state.borrow_mut();
                        let senha = s.senha_do_archive.clone();
                        s.browser.list(&nova, senha.as_deref()).is_ok()
                    };
                    if ok {
                        state.borrow_mut().location = nova;
                    }
                    if let Some(w) = weak.upgrade() {
                        recarregar_janela(&w, &state); // mostra o erro na status bar se falhou
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

        // extrair/adicionar/testar ganham corpo nas Tasks 12–13.
        self.window.on_extrair(|| {});
        self.window.on_adicionar(|| {});
        self.window.on_testar(|| {});
    }

    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
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
