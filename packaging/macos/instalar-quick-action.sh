#!/usr/bin/env bash
# Instala as Quick Actions "Comprimir com EveZip" e "Extrair com EveZip" no
# menu de contexto do Finder. Pré-requisito: EveZip.app em /Applications.
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$HOME/Library/Services"
for wf in "Comprimir com EveZip" "Extrair com EveZip"; do
  rm -rf "$HOME/Library/Services/$wf.workflow"
  cp -R "$DIR/$wf.workflow" "$HOME/Library/Services/$wf.workflow"
  echo "instalada: $wf"
done
/System/Library/CoreServices/pbs -flush || true
/System/Library/CoreServices/pbs -update || true
echo "ok. Clique com o botão direito num arquivo no Finder → 'Comprimir com EveZip'"
echo "   ou num archive → 'Extrair com EveZip' (extrai na pasta, em subpasta numerada)."
