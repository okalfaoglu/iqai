#!/usr/bin/env bash
# GitHub'a gönder: tüm değişiklikleri stage + commit + origin main push
# Kullanım:
#   ./scripts/git-push.sh "commit mesajı"
#   ./scripts/git-push.sh   # son commit'i tekrar push eder (değişiklik yoksa)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MSG="${1:-}"

# Yanlışlıkla oluşmuş "dosya:line" isimli çöpleri temizle (varsa)
rm -f \
  "crates/iqai-core/src/elliott_detector.rs:239:8" \
  "crates/iqai-core/src/q_radar_analysis.rs:262:19" \
  2>/dev/null || true

# config.json repoda olmamalı (.gitignore); yanlışlıkla eklenmişse uyar
if git ls-files --error-unmatch config.json &>/dev/null; then
  echo "HATA: config.json git'te takip ediliyor. Kaldır: git rm --cached config.json"
  exit 1
fi

echo "== Durum =="
git status -sb

if [[ -z "$MSG" ]]; then
  if git diff --quiet && git diff --cached --quiet; then
    echo "== Değişiklik yok; mevcut branch'i push ediyorum =="
    git push origin main
    exit 0
  fi
  echo "Kullanım: $0 \"commit mesajı\""
  exit 1
fi

git add -A

if git diff --cached --quiet; then
  echo "Commit edilecek bir şey yok (belki sadece ignored dosyalar)."
  git push origin main
  exit 0
fi

git commit -m "$MSG"
echo "== Push origin main =="
git push origin main
echo "Tamam."
