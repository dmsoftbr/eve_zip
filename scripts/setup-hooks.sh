#!/usr/bin/env bash
# Ativa o git hook de pre-push (roda scripts/verify.sh antes de cada push).
# Rode uma vez após clonar o repositório.
set -euo pipefail
cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
echo "ok: pre-push ativado (core.hooksPath = .githooks). 'git push' agora roda scripts/verify.sh."
