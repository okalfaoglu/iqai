#!/usr/bin/env bash
# Prod sunucu: çalışma ağacını GitHub origin/main ile birebir eşitler.
# Yerel commitlenmemiş değişiklikleri ve çoğu untracked dosyayı SİLER.
# config.json .gitignore'daysa dokunulmaz; yine de önce yedek alın.
#
# Kullanım: ./scripts/git-prod-reset.sh
# Onay istemeden:  IQAI_YES=1 ./scripts/git-prod-reset.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ "${IQAI_YES:-}" != "1" ]]; then
  echo "UYARI: origin/main'e hard reset + git clean -fd uygulanacak."
  echo "Yerel prod değişiklikleri kaybolur. config.json genelde korunur (.gitignore)."
  read -r -p "Devam? [y/N] " c
  if [[ "$c" != "y" && "$c" != "Y" ]]; then
    echo "İptal."
    exit 1
  fi
fi

# pull'un şikayet ettiği untracked çakışmaları (repoda aynı yol varsa) kaldırır
rm -f crates/iqai-core/src/elliott_fusion.rs \
      docs/ELLIOTT_CODE_REVIEW_AND_PLAN.md \
      docs/PINE_EW_SMC_FUSION_PORT_ANALYSIS.md \
      2>/dev/null || true

git fetch origin main
git reset --hard origin/main
git clean -fd

echo "== Son commit =="
git log -1 --oneline
echo "Şimdi: cargo build --release ... && sudo systemctl restart iqai-stack.service"
