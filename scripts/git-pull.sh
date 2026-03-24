#!/usr/bin/env bash
# GitHub'dan çek: origin/main ile birleştir (geliştirme makinesi için)
# Kullanım: ./scripts/git-pull.sh
#
# Prod'da yerel değişiklik / untracked çakışması varsa: ./scripts/git-prod-reset.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "== git fetch origin main =="
git fetch origin main

echo "== git pull origin main =="
git pull origin main

echo "== Son commit =="
git log -1 --oneline
echo "Tamam."
