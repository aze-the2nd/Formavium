#!/bin/sh
# Baut die Windows-Installer (x64, NSIS, Installation für den aktuellen
# Benutzer) im Dev-Container per Cross-Kompilierung.
#
#   docker compose exec dev sh scripts/build-windows.sh
#
# Ergebnis unter dist/windows/:
#   Formavium_<v>_x64-setup.exe          schlank, lädt WebView2 bei Bedarf nach.
#                                        Dieses Paket nutzt auch das Auto-Update.
#   Formavium_<v>_x64-offline-setup.exe  mit WebView2-Offline-Installer, für
#                                        Rechner ohne Netz.
#   latest.json, *.sig                   Update-Beschreibung und Signatur, nur
#                                        mit TAURI_SIGNING_PRIVATE_KEY.
#   release-manifest.json, SHA256SUMS
#
# FORMAVIUM_SKIP_OFFLINE=1 überspringt das große Offline-Paket.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TECTONIC_VERSION=0.17.0
TRIPLE=x86_64-pc-windows-gnu
REPO=aze-the2nd/Formavium
DL=/target/downloads
SEED=/target/seed-cache
TAURI=$ROOT/app/src-tauri
OUT=$ROOT/dist/windows
TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target}
BUNDLE=$TARGET_DIR/$TRIPLE/release/bundle/nsis

step() { printf '\n== %s\n' "$*"; }

step "Tectonic $TECTONIC_VERSION für Windows"
mkdir -p "$DL" "$TAURI/binaries"
ZIP=$DL/tectonic-$TECTONIC_VERSION-x86_64-pc-windows-msvc.zip
if [ ! -f "$ZIP" ]; then
  curl -fsSL --retry 5 --retry-all-errors -o "$ZIP.part" \
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

cd "$ROOT/app"
npm install --no-fund --no-audit >/dev/null
APP_VERSION=$(node -p "require('$TAURI/tauri.conf.json').version")
SETUP=Formavium_${APP_VERSION}_x64-setup.exe
OFFLINE=Formavium_${APP_VERSION}_x64-offline-setup.exe
rm -rf "$OUT" "$BUNDLE"
mkdir -p "$OUT"

# Mit Signaturschlüssel entstehen die Update-Artefakte (Signatur des Installers).
if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  SIGNED=true
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
else
  SIGNED=false
  echo "Hinweis: TAURI_SIGNING_PRIVATE_KEY fehlt, es entsteht kein Update-Paket."
fi

step "Installer (schlank, Grundlage für Updates)"
npx tauri build --target "$TRIPLE" --bundles nsis --config \
  "{\"bundle\":{\"createUpdaterArtifacts\":$SIGNED,\"windows\":{\"webviewInstallMode\":{\"type\":\"embedBootstrapper\"}}}}"
cp "$BUNDLE/$SETUP" "$OUT/"
if [ "$SIGNED" = true ]; then
  cp "$BUNDLE/$SETUP.sig" "$OUT/"
  # Beschreibung, die die installierte App beim Start abruft.
  SETUP="$SETUP" VERSION="$APP_VERSION" REPO="$REPO" OUT="$OUT" node -e '
    const fs = require("fs");
    const { SETUP, VERSION, REPO, OUT } = process.env;
    fs.writeFileSync(`${OUT}/latest.json`, JSON.stringify({
      version: VERSION,
      notes: `Formavium ${VERSION}. Änderungen: https://github.com/${REPO}/releases/tag/v${VERSION}`,
      pub_date: new Date().toISOString(),
      platforms: {
        "windows-x86_64": {
          signature: fs.readFileSync(`${OUT}/${SETUP}.sig`, "utf8"),
          url: `https://github.com/${REPO}/releases/download/v${VERSION}/${SETUP}`,
        },
      },
    }, null, 2) + "\n");
  '
fi

if [ "${FORMAVIUM_SKIP_OFFLINE:-}" != 1 ]; then
  step "Installer (offline, mit WebView2)"
  rm -rf "$BUNDLE"
  npx tauri build --target "$TRIPLE" --bundles nsis --config \
    '{"bundle":{"createUpdaterArtifacts":false,"windows":{"webviewInstallMode":{"type":"offlineInstaller"}}}}'
  cp "$BUNDLE/$SETUP" "$OUT/$OFFLINE"
fi

step "Release-Manifest"
sha() { sha256sum "$1" | cut -d' ' -f1; }
installer() { [ -f "$OUT/$1" ] && printf '{ "file": "%s", "sha256": "%s", "bytes": %s }' "$1" "$(sha "$OUT/$1")" "$(stat -c %s "$OUT/$1")" || printf null; }
cat > "$OUT/release-manifest.json" <<EOF
{
  "product": "Formavium",
  "version": "$APP_VERSION",
  "target": "$TRIPLE",
  "installer": $(installer "$SETUP"),
  "installer_offline": $(installer "$OFFLINE"),
  "update_signed": $SIGNED,
  "app": { "file": "formavium-app.exe", "sha256": "$(sha "$TARGET_DIR/$TRIPLE/release/formavium-app.exe")" },
  "engine": { "name": "tectonic", "version": "$TECTONIC_VERSION", "sha256": "$(sha "$TAURI/binaries/tectonic-$TRIPLE.exe")" },
  "tex_resources": { "id": "$SEED_ID", "files": $(find "$SEED" -type f | wc -l), "bytes": $(du -sb "$SEED" | cut -f1) },
  "built": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
(cd "$OUT" && sha256sum ./*.exe release-manifest.json > SHA256SUMS)
ls -la "$OUT"
