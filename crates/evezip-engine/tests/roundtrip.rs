use std::path::PathBuf;

use evezip_engine::cancel::CancelToken;
use evezip_engine::ops::{CreateOptions, Engine, Format};
use evezip_engine::EngineError;

fn sandbox(nome: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("evezip-test-{nome}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("in/sub")).unwrap();
    std::fs::write(d.join("in/a.txt"), "conteúdo A").unwrap();
    std::fs::write(d.join("in/sub/b.txt"), "conteúdo B").unwrap();
    d
}

fn roundtrip(format: Format, ext: &str) {
    let d = sandbox(ext);
    let eng = Engine::locate().unwrap();
    let arq = d.join(format!("out.{ext}"));

    let opts = CreateOptions { format, level: 5, password: None, encrypt_names: false };
    eng.create(&arq, &[d.join("in")], &opts, &mut |_| {}, &CancelToken::new()).unwrap();
    assert!(arq.exists());

    let entradas = eng.list(&arq, None).unwrap();
    let caminhos: Vec<_> = entradas.iter().map(|e| e.path.replace('\\', "/")).collect();
    assert!(caminhos.iter().any(|p| p.ends_with("in/a.txt")), "{caminhos:?}");
    assert!(caminhos.iter().any(|p| p.ends_with("in/sub/b.txt")), "{caminhos:?}");

    let dest = d.join("out-dir");
    eng.extract(&arq, &dest, None, None, &mut |_| {}, &CancelToken::new()).unwrap();
    assert_eq!(std::fs::read_to_string(dest.join("in/a.txt")).unwrap(), "conteúdo A");

    eng.test(&arq, None, &mut |_| {}, &CancelToken::new()).unwrap();
}

#[test]
fn roundtrip_zip() { roundtrip(Format::Zip, "zip"); }

#[test]
fn roundtrip_7z() { roundtrip(Format::SevenZ, "7z"); }

#[test]
fn roundtrip_tar() { roundtrip(Format::Tar, "tar"); }

#[test]
fn roundtrip_targz() { roundtrip(Format::TarGz, "tar.gz"); }

#[test]
fn sete_z_com_senha_e_nomes_criptografados() {
    let d = sandbox("7z-senha");
    let eng = Engine::locate().unwrap();
    let arq = d.join("out.7z");

    let opts = CreateOptions {
        format: Format::SevenZ,
        level: 5,
        password: Some("s3nha!".into()),
        encrypt_names: true,
    };
    eng.create(&arq, &[d.join("in")], &opts, &mut |_| {}, &CancelToken::new()).unwrap();

    // Sem senha: precisa de senha (nomes criptografados nem listam).
    let err = eng.list(&arq, None).unwrap_err();
    assert!(matches!(err, EngineError::SenhaNecessaria), "{err:?}");

    // Senha errada: incorreta.
    let err = eng.list(&arq, Some("errada")).unwrap_err();
    assert!(matches!(err, EngineError::SenhaIncorreta), "{err:?}");

    // Senha certa: extrai.
    let dest = d.join("out-dir");
    eng.extract(&arq, &dest, None, Some("s3nha!"), &mut |_| {}, &CancelToken::new()).unwrap();
    assert_eq!(std::fs::read_to_string(dest.join("in/a.txt")).unwrap(), "conteúdo A");
}

#[test]
fn extracao_seletiva() {
    let d = sandbox("seletivo");
    let eng = Engine::locate().unwrap();
    let arq = d.join("out.zip");
    let opts = CreateOptions { format: Format::Zip, level: 5, password: None, encrypt_names: false };
    eng.create(&arq, &[d.join("in")], &opts, &mut |_| {}, &CancelToken::new()).unwrap();

    let dest = d.join("so-um");
    eng.extract(&arq, &dest, Some(&["in/a.txt".into()]), None, &mut |_| {}, &CancelToken::new())
        .unwrap();
    assert!(dest.join("in/a.txt").exists());
    assert!(!dest.join("in/sub/b.txt").exists());
}

#[test]
fn extrai_rar_se_houver_fixture() {
    // Não é possível GERAR rar (proprietário). Fixture opcional em tests/fixtures/sample.rar
    // contendo um arquivo "hello.txt" com o texto "hello rar".
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.rar");
    if !fixture.exists() {
        eprintln!("fixture rar ausente — teste pulado");
        return;
    }
    let eng = Engine::locate().unwrap();
    let dest = std::env::temp_dir().join("evezip-test-rar-out");
    let _ = std::fs::remove_dir_all(&dest);
    eng.extract(&fixture, &dest, None, None, &mut |_| {}, &CancelToken::new()).unwrap();
    assert_eq!(std::fs::read_to_string(dest.join("hello.txt")).unwrap().trim(), "hello rar");
}
