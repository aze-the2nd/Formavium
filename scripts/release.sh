#!/bin/sh
# Bereitet ein Release vor: setzt die Versionsnummer überall, aktualisiert
# die Lock-Dateien im Container, committet und legt das Tag an. Das Pushen
# des Tags startet den Release-Build auf GitHub.
#
#   sh scripts/release.sh 0.3.0
#   git push origin main v0.3.0
set -eu

VERSION=${1:?Aufruf: sh scripts/release.sh <Version, z. B. 0.3.0>}
echo "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "Version muss die Form 1.2.3 haben"; exit 1; }
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

if [ -n "$(git status --porcelain)" ]; then
  echo "Es gibt nicht committete Änderungen. Bitte zuerst committen."
  exit 1
fi
if git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null; then
  echo "Das Tag v$VERSION existiert bereits."
  exit 1
fi

# Eine Versionsnummer an drei Stellen: Cargo-Workspace, Tauri, Oberfläche.
sed -i -E "0,/^version = \"[0-9.]+\"/s//version = \"$VERSION\"/" Cargo.toml
sed -i -E "0,/\"version\": \"[0-9.]+\"/s//\"version\": \"$VERSION\"/" app/src-tauri/tauri.conf.json
sed -i -E "0,/\"version\": \"[0-9.]+\"/s//\"version\": \"$VERSION\"/" app/package.json

# Lock-Dateien folgen der neuen Version (Cargo.lock, package-lock.json).
MSYS_NO_PATHCONV=1 docker compose exec -T dev sh -c \
  'cargo update --workspace --quiet && cd app && npm install --package-lock-only --no-fund --no-audit >/dev/null'

git add -A
git commit -q -m "Version $VERSION"
git tag "v$VERSION"
echo "Version $VERSION ist committet und als v$VERSION markiert."
echo "Veröffentlichen mit:  git push origin main v$VERSION"
