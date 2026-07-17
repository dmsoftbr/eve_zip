#!/usr/bin/env bash
# Espelho local do CI (.github/workflows/ci.yml): roda os MESMOS gates antes do
# push, para não gastar minutos de GitHub Actions com push quebrado.
#
# Gates (idênticos ao ci.yml): cargo fmt --check, clippy -D warnings,
# cargo test --workspace. Não há gate exclusivo de CI.
#
# Uso:
#   bash scripts/verify.sh        # roda todos os gates
#   git push                      # o hook .githooks/pre-push roda isto sozinho
#   git push --no-verify          # pula o gate (emergência, por sua conta)
set -uo pipefail
cd "$(dirname "$0")/.."

# Localiza o cargo. Em máquinas onde o rustup foi instalado via Homebrew, o
# `cargo` não fica no PATH — cai no `rustup which` ou em ~/.cargo/bin.
CARGO="$(command -v cargo || true)"
if [ -z "$CARGO" ] && command -v rustup >/dev/null 2>&1; then
  CARGO="$(rustup which cargo 2>/dev/null || true)"
fi
if [ -z "$CARGO" ] && [ -x "$HOME/.cargo/bin/cargo" ]; then
  CARGO="$HOME/.cargo/bin/cargo"
fi
if [ -z "$CARGO" ]; then
  echo "✗ cargo não encontrado. Instale o Rust: https://rustup.rs"
  exit 1
fi

# Pré-requisito dos testes de integração do engine: o binário 7zz vendorizado.
# Best-effort — se faltar e não der para baixar, os testes de engine avisam.
if ! ls vendor/7zz/*/7zz >/dev/null 2>&1; then
  echo "▶ Vendorizando 7zz (scripts/fetch-7zz.sh)…"
  bash scripts/fetch-7zz.sh || echo "  aviso: fetch-7zz falhou; testes de engine podem falhar"
fi

fail=0
run() {
  local title="$1"
  shift
  echo ""
  echo "▶ ${title}"
  if "$@"; then
    echo "✓ ${title}"
  else
    echo "✗ ${title} FALHOU"
    fail=1
  fi
}

run "Formatação (cargo fmt --check)" "$CARGO" fmt --check
run "Lints (clippy -D warnings)" "$CARGO" clippy --workspace --all-targets -- -D warnings
run "Testes (cargo test --workspace)" "$CARGO" test --workspace

echo ""
if [ "$fail" -ne 0 ]; then
  echo "✗ Verificação local FALHOU — corrija antes do push (ou 'git push --no-verify' por sua conta)."
  exit 1
fi
echo "✓ Tudo verde localmente — seguro para push."
