#!/usr/bin/env bash
# Roda os gates do CI (fmt/clippy/test) para LINUX num container Docker, sem
# gastar minutos de GitHub Actions. Usa um CARGO_TARGET_DIR separado para não
# colidir com os artefatos do macOS em target/.
#
# Pré-requisito: Docker (ex.: OrbStack) rodando.
# Uso: bash scripts/verify-linux.sh
set -euo pipefail
cd "$(dirname "$0")/.."

IMG="rust:bookworm"

docker run --rm \
  -v "$PWD":/work -w /work \
  -v evezip-cargo-registry:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/work/target-linux \
  "$IMG" bash -c '
set -e
echo "== apt: dependências de UI (Slint + rfd) =="
apt-get update -qq
apt-get install -y --no-install-recommends \
  libgtk-3-dev libxkbcommon-dev libfontconfig-dev \
  libgl1-mesa-dev libegl1-mesa-dev \
  libxcb1-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libwayland-dev curl xz-utils >/dev/null
echo "== clippy: instala componente =="
rustup component add clippy rustfmt >/dev/null 2>&1 || true
echo "== vendoriza 7zz (linux) =="
./scripts/fetch-7zz.sh
echo "== fmt =="
cargo fmt --check
echo "== clippy -D warnings =="
cargo clippy --workspace --all-targets -- -D warnings
echo "== test =="
cargo test --workspace
echo "== LINUX OK =="
'
