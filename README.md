# Formavium – document engineering

Lokale Desktop-Anwendung für strukturierte, modulbasierte LaTeX-Dokumente:
wiederverwendbare Module einmal gestalten, beliebig oft ausfüllen, als PDF
ausgeben. Läuft offline und ohne Adminrechte.

## Aufbau

| Teil | Inhalt |
|---|---|
| `crates/formavium-core` | Dokumentmodell (`.fvm`, Schema 1), Module mit Feldern, Tabellen, Bildern, Anordnung und Untermodulen, lokale Spaltenanpassungen je Verwendung, Revisionen mit Rückgängig, sicheres Speichern mit Konflikterkennung, LaTeX-Erzeugung mit Maskierung und Positionszuordnung, Abbildung von Quelltextänderungen auf Felder/Zellen/Teile, LaTeX-Export, Inhaltsfilter |
| `crates/formavium-render` | Tectonic im eigenen Prozess (`--untrusted`, festes Build-Datum, Zeitlimit, Abbruch), Diagnosen, Warteschlange „nur jüngste Revision wartet“, Vorschauzustand mit letztem gutem PDF |
| `crates/formavium-session` | Sitzung eines offenen Dokuments: eine Schnittstelle (`call`, `pdf`, `resource`) für Desktop-Hülle und Dev-Bridge; `formavium-devbridge` stellt sie per HTTP bereit |
| `crates/formavium-cli` | `formavium new / info / latex / render / export / set / set-latex / move-column` |
| `app/` | Svelte-5-Oberfläche: Quelltext (CodeMirror 6), visueller Editor, PDF-Vorschau (PDF.js) als verschiebbare Kacheln |
| `app/src-tauri` | Tauri-2-Hülle: drei Befehle (`call`, `pdf`, `resource`) und Dateidialoge |

## Entwicklung im Container

```sh
docker compose up -d --build          # Rust 1.91, Node 22, Tectonic 0.17.0, WebKitGTK
docker compose exec dev cargo test --workspace
docker compose exec dev cargo clippy --workspace --all-targets
```

### Oberfläche im Browser

Die Windows-App lässt sich im Linux-Container nicht anzeigen. Dieselbe
Oberfläche läuft deshalb im Browser gegen den echten Kern:

```sh
docker compose exec -d dev sh -c 'cargo build -p formavium-session && /target/debug/formavium-devbridge'
docker compose exec -d dev sh -c 'cd app && npm install && npm run dev'
```

Danach <http://localhost:5173> öffnen. Dateipfade sind Pfade im Container
(das Projekt liegt unter `/work`, Beispiele unter `/work/out`). Die Dev-Bridge
liest und schreibt beliebige Pfade und ist nur an `127.0.0.1` im Container
gebunden; nach außen ist nur Vite auf `127.0.0.1:5173` freigegeben.

### Kommandozeile

```sh
docker compose exec dev sh -c 'cargo build && mkdir -p out && cd out &&
  /target/debug/formavium new anleitung.fvm && /target/debug/formavium render anleitung.fvm'
```

`CARGO_TARGET_DIR` liegt im Container unter `/target` (Volume). Der
Tectonic-Cache liegt im Volume `tectonic-cache`. Der erste Lauf lädt die
benötigten TeX-Ressourcen, danach funktioniert `formavium render --offline`.

## Bedienung

Ein Dokument liegt in einem eigenen Ordner mit seinem Namen
(`anleitung/anleitung.fvm`), Bilder daneben unter `bilder/`. Beim Anlegen
entsteht dieser Ordner von selbst.

- **Quelltext:** reines LaTeX. Tippen in einem Feld oder einer Tabellenzelle
  ändert genau diesen Wert; Sonderzeichen bleiben Inhalt. Eine Änderung an der
  Modulstruktur wandelt den Abschnitt nach Rückfrage in freies LaTeX um.
- **Visuell:** Module sehen aus wie im Dokument und werden direkt bearbeitet.
  Es gibt kaum Schaltflächen: Einfügestellen erscheinen beim Überfahren,
  alles Weitere liegt im Rechtsklick-Menü und auf Tastenkürzeln.
  - *Tabelle:* Plus unter der letzten Zeile, am linken Rand zwischen Zeilen
    und über einer Spaltengrenze. Spalten am Kopf ziehen, Doppelklick benennt
    um. Ergänzte, entfernte und umbenannte Spalten gelten nur für diese
    Verwendung des Moduls.
  - *Bild:* anklicken, hineinziehen oder mit Strg+V einfügen.
  - *Abschnitte:* am Griff ziehen oder mit Strg+↑/↓ verschieben, Plus
    zwischen den Abschnitten fügt ein.
- **PDF:** bei Fehlern bleibt der letzte gute Stand sichtbar und ist als
  älter markiert; Diagnosen springen zur Stelle. „Als PDF speichern“ schreibt
  genau die angezeigten Bytes.
- **Ansichten:** Kacheln am Griff umordnen, Trennlinien ziehen, mit
  Strg+1/2/3 ein- und ausblenden. Die Anordnung bleibt erhalten.
- **Strg+Z / Strg+Y** wirken auf das Dokument, über alle Ansichten.
  **F1** zeigt alle Tastenkürzel.

Zieh-Gesten nutzen Zeiger-Ereignisse statt HTML5-Drag-and-Drop, damit
Datei-Drops aus dem Explorer (Bilder) im selben Fenster funktionieren.

## Windows-Installer

```sh
docker compose exec dev sh scripts/build-windows.sh
```

Cross-Build im Container (Target `x86_64-pc-windows-gnu`, MinGW-w64,
`makensis` 3.10). Ergebnis unter `dist/windows/`:

| Bestandteil | Größe | Anmerkung |
|---|---:|---|
| `formavium-app.exe` | 21 MB | Tauri-Hülle, Oberfläche eingebettet |
| `tectonic.exe` 0.17.0 | 51 MB | offizielle Release, nur System-DLLs |
| `tex-cache/` | 51 MB | vorbefüllter Tectonic-Cache aus `docker/tex-corpus/` |
| WebView2-Offline-Installer | 294 MB | läuft nur, wenn die Runtime fehlt |
| **Installer gesamt** | **308 MiB** | NSIS, aktueller Benutzer, keine Adminrechte |

Das Skript kompiliert jedes Korpusdokument und prüft danach mit
`--only-cached`, dass alle Ressourcen offline vorliegen. Beim ersten Start
kopiert die App das Paket nach `%LOCALAPPDATA%\com.formavium.app\tectonic-cache`
und rendert danach ausschließlich offline (`FORMAVIUM_ALLOW_NETWORK=1`
erlaubt das Nachladen fehlender Pakete). `release-manifest.json` und
`SHA256SUMS` halten Versionen und Prüfsummen fest.

Hinweis für Vorlagen: Tectonic findet Schriften aus dem Bundle über den
Dateinamen (`\setmainfont{texgyrepagella}[Extension=.otf, …]`), nicht über
den Systemnamen („TeX Gyre Pagella“).

## Offen

- Der Installer ist noch nicht systematisch geprüft (frische VM,
  Standardbenutzer, ohne Netz, mit und ohne WebView2-Runtime).
- Keine Code-Signatur (SmartScreen-Warnung beim Start des Installers).
- Modulbibliothek, Vorlagen, Tabellenimport und Modulupdates.

## Lizenz

MIT, siehe [LICENSE](LICENSE).
