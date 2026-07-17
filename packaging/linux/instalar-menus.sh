#!/usr/bin/env bash
# Instala os menus de contexto "Comprimir/Extrair com EveZip" nos gerenciadores
# de arquivos do Linux presentes (Dolphin/KDE, Nemo/Cinnamon, Nautilus/GNOME,
# Thunar/XFCE) + associação de arquivo + ícone. Tudo no diretório do usuário
# (sem sudo).
#
# Pré-requisito: o binário `evezip` precisa estar no PATH. Se não estiver, passe
# o caminho dele como argumento que eu crio um symlink em ~/.local/bin:
#   ./instalar-menus.sh /opt/evezip/evezip
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
share="${XDG_DATA_HOME:-$HOME/.local/share}"

# --- binário evezip no PATH ---------------------------------------------------
if ! command -v evezip >/dev/null 2>&1; then
  if [ $# -ge 1 ] && [ -x "$1" ]; then
    mkdir -p "$HOME/.local/bin"
    ln -sf "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")" "$HOME/.local/bin/evezip"
    echo "symlink criado: ~/.local/bin/evezip -> $1"
    case ":$PATH:" in *":$HOME/.local/bin:"*) : ;; *)
      echo "AVISO: ~/.local/bin não está no PATH. Adicione ao seu shell:"
      echo "  export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
    esac
  else
    echo "AVISO: 'evezip' não está no PATH. Os menus só funcionarão quando ele"
    echo "       estiver acessível. Rode de novo passando o caminho do binário:"
    echo "       ./instalar-menus.sh /caminho/para/evezip"
  fi
fi

# --- ícone --------------------------------------------------------------------
icondir="$share/icons/hicolor/256x256/apps"
mkdir -p "$icondir"
cp "$DIR/evezip-256.png" "$icondir/evezip.png"
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
  gtk-update-icon-cache -f -t "$share/icons/hicolor" >/dev/null 2>&1 || true

# --- associação de arquivo (.desktop) ----------------------------------------
appsdir="$share/applications"
mkdir -p "$appsdir"
cp "$DIR/evezip.desktop" "$appsdir/evezip.desktop"
command -v update-desktop-database >/dev/null 2>&1 && \
  update-desktop-database "$appsdir" >/dev/null 2>&1 || true

instalou=()

# --- KDE / Dolphin ------------------------------------------------------------
if command -v dolphin >/dev/null 2>&1 || [ -d "$share/kio" ]; then
  km="$share/kio/servicemenus"
  mkdir -p "$km"
  cp "$DIR/menus/kde/evezip-comprimir.desktop" "$km/"
  cp "$DIR/menus/kde/evezip-extrair.desktop" "$km/"
  chmod +x "$km/evezip-comprimir.desktop" "$km/evezip-extrair.desktop" 2>/dev/null || true
  command -v kbuildsycoca6 >/dev/null 2>&1 && kbuildsycoca6 >/dev/null 2>&1 || \
    { command -v kbuildsycoca5 >/dev/null 2>&1 && kbuildsycoca5 >/dev/null 2>&1 || true; }
  instalou+=("KDE/Dolphin")
fi

# --- Cinnamon / Nemo ----------------------------------------------------------
if command -v nemo >/dev/null 2>&1 || [ -d "$share/nemo" ]; then
  nm="$share/nemo/actions"
  mkdir -p "$nm"
  cp "$DIR/menus/nemo/evezip-comprimir.nemo_action" "$nm/"
  cp "$DIR/menus/nemo/evezip-extrair.nemo_action" "$nm/"
  instalou+=("Cinnamon/Nemo")
fi

# --- GNOME / Nautilus (scripts) ----------------------------------------------
if command -v nautilus >/dev/null 2>&1 || [ -d "$share/nautilus" ]; then
  ns="$share/nautilus/scripts"
  mkdir -p "$ns"
  cp "$DIR/menus/nautilus/Comprimir com EveZip" "$ns/"
  cp "$DIR/menus/nautilus/Extrair com EveZip" "$ns/"
  chmod +x "$ns/Comprimir com EveZip" "$ns/Extrair com EveZip"
  instalou+=("GNOME/Nautilus (submenu Scripts)")
fi

# --- XFCE / Thunar ------------------------------------------------------------
if command -v thunar >/dev/null 2>&1 || [ -f "$HOME/.config/Thunar/uca.xml" ]; then
  uca="$HOME/.config/Thunar/uca.xml"
  mkdir -p "$(dirname "$uca")"
  if [ ! -f "$uca" ]; then
    { echo '<?xml version="1.0" encoding="UTF-8"?>'; echo '<actions>';
      cat "$DIR/menus/thunar-actions.xml"; echo '</actions>'; } > "$uca"
    instalou+=("XFCE/Thunar")
  elif ! grep -q "EveZip" "$uca"; then
    # insere nossas ações antes de </actions>
    tmp="$(mktemp)"
    awk -v f="$DIR/menus/thunar-actions.xml" '
      /<\/actions>/ { while ((getline l < f) > 0) print l; close(f) }
      { print }' "$uca" > "$tmp" && mv "$tmp" "$uca"
    instalou+=("XFCE/Thunar")
  else
    instalou+=("XFCE/Thunar (já presente)")
  fi
fi

echo ""
if [ ${#instalou[@]} -eq 0 ]; then
  echo "Nenhum gerenciador de arquivos suportado detectado."
  echo "Os arquivos estão em packaging/linux/menus/ para instalação manual."
else
  echo "Menus instalados para: ${instalou[*]}"
  echo "Reinicie o gerenciador de arquivos (ex.: 'nautilus -q', 'nemo -q') se preciso."
fi
