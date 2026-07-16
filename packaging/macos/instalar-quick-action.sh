#!/usr/bin/env bash
# Instala a Quick Action "Comprimir com EveZip" no menu de contexto do Finder.
# Pré-requisito: EveZip.app instalado em /Applications.
set -euo pipefail
ORIGEM="$(cd "$(dirname "$0")" && pwd)/Comprimir com EveZip.workflow"
DESTINO="$HOME/Library/Services/Comprimir com EveZip.workflow"
rm -rf "$DESTINO"
mkdir -p "$HOME/Library/Services"
cp -R "$ORIGEM" "$DESTINO"
/System/Library/CoreServices/pbs -flush || true
/System/Library/CoreServices/pbs -update || true
echo "ok: Quick Action instalada. Clique com o botão direito num arquivo no Finder"
echo "    → 'Comprimir com EveZip' (pode estar em 'Ações Rápidas' ou 'Serviços')."
