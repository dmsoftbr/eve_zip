# EveZip

Gerenciador de archives estilo 7-Zip (macOS), em Rust + Slint, com o binário
oficial `7zz` embutido como motor. Workspace com 3 crates: `evezip-engine`
(wrapper do 7zz), `evezip-core` (árvore de archive, navegação, fila de jobs,
config) e `evezip-ui` (janela Slint + CLI).

## Antes de dar push: rode a verificação local

O CI (GitHub Actions) roda em minutos pagos. Para não gastar minutos com push
quebrado, **as mesmas checagens do CI rodam localmente**:

```bash
bash scripts/verify.sh
```

Gates (idênticos a `.github/workflows/ci.yml`): `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

Um **git hook de pre-push** roda `scripts/verify.sh` automaticamente a cada
`git push` e bloqueia se algo falhar. Para ativar (uma vez, após clonar):

```bash
bash scripts/setup-hooks.sh     # ou: git config core.hooksPath .githooks
```

Bypass de emergência (por sua conta): `git push --no-verify`.

## Toolchain

- Rust estável via `rustup`. Nesta máquina o `cargo` não está no PATH (rustup
  instalado via Homebrew só linka `rustup`); o `verify.sh` acha o cargo via
  `rustup which cargo`. Para usar direto no shell: `rustup default stable` e
  ponha `~/.rustup/toolchains/stable-*/bin` (ou `~/.cargo/bin`) no PATH.
- Os testes de integração do engine executam o `7zz` vendorizado. Baixe-o com
  `bash scripts/fetch-7zz.sh` (o `verify.sh` faz isso sozinho se faltar).

## Empacotamento (macOS)

- `bash scripts/package-macos.sh` gera `target/EveZip.app` (com ícone, assinado
  ad-hoc, com o 7zz embutido) e `target/EveZip.dmg`.
- Quick Actions do Finder ("Comprimir/Extrair com EveZip"):
  `bash packaging/macos/instalar-quick-action.sh`.

## CI

`.github/workflows/ci.yml` roda **só no macOS** (alvo suportado), em PRs e push
na `main`, com `concurrency` (um push novo cancela o run anterior). Linux/Windows
saíram da matriz porque o app ainda não tem suporte de 1ª classe lá.
