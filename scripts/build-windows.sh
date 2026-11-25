#!/bin/sh
# Baut den Windows-Installer (x64, NSIS, Installation für den aktuellen
# Benutzer) im Dev-Container per Cross-Kompilierung.
#
#   docker compose exec dev sh scripts/build-windows.sh
#
# Ergebnis unter dist/windows/: Installer, Release-Manifest, SHA256SUMS.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TECTONIC_VERSION=0.17.0
TRIPLE=x86_64-pc-windows-gnu
DL=/target/downloads
SEED=/target/seed-cache
TAURI=$ROOT/app/src-tauri
OUT=$ROOT/dist/windows
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}

step() { printf '\n== %s\n' "$*"; }

step "Tectonic $TECTONIC_VERSION für Windows"
mkdir -p "$DL" "$TAURI/binaries"
ZIP=$DL/tectonic-$TECTONIC_VERSION-x86_64-pc-windows-msvc.zip
if [ ! -f "$ZIP" ]; then
  curl -fsSL -o "$ZIP.part" \
    "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40$TECTONIC_VERSION/tectonic-$TECTONIC_VERSION-x86_64-pc-windows-msvc.zip"
  mv "$ZIP.part" "$ZIP"
fi
unzip -o -q "$ZIP" tectonic.exe -d "$DL/tectonic-win"
cp "$DL/tectonic-win/tectonic.exe" "$TAURI/binaries/tectonic-$TRIPLE.exe"
echo "DLL-Abhängigkeiten von tectonic.exe:"
x86_64-w64-mingw32-objdump -p "$TAURI/binaries/tectonic-$TRIPLE.exe" | sed -n 's/^\tDLL Name: /  /p'

step "Offline-Ressourcenpaket aus dem Testkorpus"
rm -rf "$SEED"
mkdir -p "$SEED"
WORK=$(mktemp -d)
cp "$ROOT"/docker/tex-corpus/*.tex "$WORK/"
# Das Übungsdokument der App gehört ebenfalls dazu.
cargo build -q -p formavium-cli
"$TARGET_DIR/debug/formavium" new "$WORK/sample/anleitung.fvm" >/dev/null
"$TARGET_DIR/debug/formavium" latex "$WORK/sample/anleitung.fvm" > "$WORK/sample/main.tex"
for doc in "$WORK"/*.tex "$WORK/sample/main.tex"; do
  (cd "$(dirname "$doc")" && TECTONIC_CACHE_DIR=$SEED tectonic -X compile "$(basename "$doc")" --untrusted >/dev/null 2>&1) \
    || { echo "Korpusdokument fehlgeschlagen: $doc"; exit 1; }
done
# Gegenprobe: alles muss ohne Nachladen kompilieren.
for doc in "$WORK"/*.tex "$WORK/sample/main.tex"; do
  (cd "$(dirname "$doc")" && TECTONIC_CACHE_DIR=$SEED tectonic -X compile "$(basename "$doc")" --untrusted --only-cached >/dev/null 2>&1) \
    || { echo "Offline-Gegenprobe fehlgeschlagen: $doc"; exit 1; }
  echo "  ok  $(basename "$doc")"
done
rm -rf "$WORK"
SEED_HASH=$( (cd "$ROOT/docker/tex-corpus" && cat *.tex; echo "$TECTONIC_VERSION") | sha256sum | cut -c1-16)
SEED_ID="tectonic-$TECTONIC_VERSION-$SEED_HASH"
echo "$SEED_ID" > "$SEED/SEED_ID"
rm -rf "$TAURI/resources/tex-cache"
mkdir -p "$TAURI/resources"
cp -r "$SEED" "$TAURI/resources/tex-cache"
echo "  $SEED_ID, $(du -sh "$SEED" | cut -f1), $(find "$SEED" -type f | wc -l) Dateien"

step "Oberfläche und Installer"
cd "$ROOT/app"
npm install --no-fund --no-audit >/dev/null
npx tauri build --target "$TRIPLE" --bundles nsis

step "Release-Manifest"
INSTALLER=$(ls -t "$TARGET_DIR/$TRIPLE/release/bundle/nsis/"*-setup.exe | head -1)
rm -rf "$OUT"
mkdir -p "$OUT"
cp "$INSTALLER" "$OUT/"
APP_VERSION=$(node -p "require('$TAURI/tauri.conf.json').version")
sha() { sha256sum "$1" | cut -d' ' -f1; }
cat > "$OUT/release-manifest.json" <<EOF
{
  "product": "Formavium",
  "version": "$APP_VERSION",
  "target": "$TRIPLE",
  "installer": { "file": "$(basename "$INSTALLER")", "sha256": "$(sha "$INSTALLER")" },
  "app": { "file": "formavium-app.exe", "sha256": "$(sha "$TARGET_DIR/$TRIPLE/release/formavium-app.exe")" },
  "engine": { "name": "tectonic", "version": "$TECTONIC_VERSION", "sha256": "$(sha "$TAURI/binaries/tectonic-$TRIPLE.exe")" },
  "tex_resources": { "id": "$SEED_ID", "files": $(find "$SEED" -type f | wc -l), "bytes": $(du -sb "$SEED" | cut -f1) },
  "webview2": "offlineInstaller",
  "built": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
(cd "$OUT" && sha256sum ./*.exe release-manifest.json > SHA256SUMS)
ls -la "$OUT"
