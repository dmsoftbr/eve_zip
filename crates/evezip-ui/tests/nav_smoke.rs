//! Smoke test da Task 11 (navegar para dentro de archives) sem UI.
//!
//! O ambiente de execução não permite ver a janela (sem screenshot), então
//! este teste reproduz — usando Browser/Location/Engine reais, sem mocks —
//! exatamente os dois cenários da verificação manual do brief:
//!   1. duplo-clique num .7z válido navega raiz → subdiretório → volta ao disco;
//!   2. duplo-clique num .7z corrompido NÃO navega e o erro fica disponível
//!      para a status bar (endurecimento do Step 2 em `on_linha_click`).

use std::path::PathBuf;
use std::sync::Arc;

use evezip_core::{Browser, Location};
use evezip_engine::{CancelToken, CreateOptions, Engine, Format};

fn sandbox(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("evezip-ui-nav-smoke-{nome}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("conteudo/sub")).unwrap();
    std::fs::write(d.join("conteudo/a.txt"), "ola").unwrap();
    std::fs::write(d.join("conteudo/sub/b.txt"), "mundo").unwrap();
    d
}

/// Espelha o endurecimento do Step 2 em `on_linha_click` (crates/evezip-ui/src/app.rs):
/// só troca de location se `browser.list` da nova location suceder; senão,
/// permanece na location atual (o erro fica disponível para a status bar).
fn tentar_navegar(
    browser: &mut Browser,
    atual: &Location,
    nova: Location,
    senha: Option<&str>,
) -> Location {
    if browser.list(&nova, senha).is_ok() {
        nova
    } else {
        atual.clone()
    }
}

#[test]
fn navega_para_dentro_do_archive_e_volta_ao_disco() {
    let d = sandbox("ok");
    let eng = Engine::locate().unwrap();
    let arq = d.join("teste.7z");
    let opts = CreateOptions {
        format: Format::SevenZ,
        level: 1,
        password: None,
        encrypt_names: false,
    };
    eng.create(
        &arq,
        &[d.join("conteudo")],
        &opts,
        &mut |_| {},
        &CancelToken::new(),
    )
    .unwrap();

    let mut browser = Browser::new(Arc::new(eng));
    let no_disco = Location::Disk(d.clone());

    // Duplo-clique em "teste.7z" no disco: enter() resolve para a raiz do archive.
    let alvo = browser.enter(&no_disco, "teste.7z", false);
    let loc_raiz = tentar_navegar(&mut browser, &no_disco, alvo, None);
    assert!(matches!(&loc_raiz, Location::Archive { inner, .. } if inner.is_empty()));

    let raiz = browser.list(&loc_raiz, None).unwrap();
    assert_eq!(raiz.len(), 1);
    assert_eq!(raiz[0].name, "conteudo");
    assert!(raiz[0].is_dir);

    // Duplo-clique em "conteudo".
    let alvo2 = browser.enter(&loc_raiz, "conteudo", true);
    let loc_conteudo = tentar_navegar(&mut browser, &loc_raiz, alvo2, None);
    let itens = browser.list(&loc_conteudo, None).unwrap();
    let nomes: Vec<&str> = itens.iter().map(|r| r.name.as_str()).collect();
    assert!(nomes.contains(&"a.txt"), "{nomes:?}");
    assert!(nomes.contains(&"sub"), "{nomes:?}");
    let a = itens.iter().find(|r| r.name == "a.txt").unwrap();
    assert_eq!(a.size, 3); // "ola" tem 3 bytes — coluna Tamanho correta

    // Duplo-clique em "sub".
    let alvo3 = browser.enter(&loc_conteudo, "sub", true);
    let loc_sub = tentar_navegar(&mut browser, &loc_conteudo, alvo3, None);
    let sub_itens = browser.list(&loc_sub, None).unwrap();
    assert_eq!(sub_itens.len(), 1);
    assert_eq!(sub_itens[0].name, "b.txt");

    // ⬆ na raiz do archive volta para o diretório do disco onde ele está.
    let pai = loc_raiz.parent().unwrap();
    assert_eq!(pai, Location::Disk(d));
}

#[test]
fn archive_corrompido_nao_navega() {
    let d = sandbox("corrompido");
    std::fs::write(d.join("falso.7z"), "nao sou um archive").unwrap();
    let eng = Engine::locate().unwrap();
    let mut browser = Browser::new(Arc::new(eng));

    let no_disco = Location::Disk(d.clone());

    // Duplo-clique em "falso.7z": enter() já resolve para Location::Archive
    // (a validação de fato só acontece no list, como no `on_linha_click`).
    let alvo = browser.enter(&no_disco, "falso.7z", false);
    assert_ne!(alvo, no_disco);

    let erro = browser.list(&alvo, None).unwrap_err();
    let msg = format!("Erro: {erro}"); // é isso que vai para a status bar
    assert!(
        msg.starts_with("Erro: archive corrompido"),
        "mensagem inesperada na status bar: {msg:?}"
    );

    // Endurecimento do Step 2: como `list` falhou, a location NÃO deve mudar.
    let loc_final = tentar_navegar(&mut browser, &no_disco, alvo, None);
    assert_eq!(
        loc_final, no_disco,
        "não deveria navegar para o archive corrompido"
    );
}
