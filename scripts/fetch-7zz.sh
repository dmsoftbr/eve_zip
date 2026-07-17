#!/usr/bin/env bash
# Baixa o binário oficial do 7-Zip (7zz) para vendor/7zz/<plataforma>/.
set -euo pipefail
VERSION=2501
BASE="https://www.7-zip.org/a"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

os="$(uname -s)"; arch="$(uname -m)"
case "$os" in
  Darwin)
    plat="macos"; file="7z${VERSION}-mac.tar.xz" ;;
  Linux)
    case "$arch" in
      x86_64)  plat="linux-x64";   file="7z${VERSION}-linux-x64.tar.xz" ;;
      aarch64) plat="linux-arm64"; file="7z${VERSION}-linux-arm64.tar.xz" ;;
      *) echo "arch não suportada: $arch" >&2; exit 1 ;;
    esac ;;
  *)
    echo "Windows: baixe ${BASE}/7z${VERSION}-x64.exe, extraia 7z.exe e 7z.dll" >&2
    echo "para vendor/7zz/windows-x64/ (use 7zr.exe ou um 7-Zip existente)." >&2
    exit 1 ;;
esac

dest="$ROOT/vendor/7zz/$plat"
[ -x "$dest/7zz" ] && { echo "já existe: $dest/7zz"; exit 0; }
mkdir -p "$dest"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
curl -fsSL "$BASE/$file" -o "$tmp/$file"
tar -xJf "$tmp/$file" -C "$tmp"
cp "$tmp/7zz" "$dest/7zz"
chmod +x "$dest/7zz"
echo "ok: $dest/7zz"
"$dest/7zz" | head -2
