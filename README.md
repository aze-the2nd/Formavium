# Formavium – document engineering

Lokale Desktop-Anwendung für strukturierte, modulbasierte LaTeX-Dokumente:
wiederverwendbare Module einmal gestalten, beliebig oft ausfüllen, als PDF
ausgeben. Läuft offline und ohne Adminrechte.

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
  - *Tabelle:* bedient wie eine Tabellenkalkulation. Zellen wählen,
    Bereiche ziehen oder mit Umschalt+Pfeil markieren, Tippen überschreibt,
    Entf leert, Strg+C/V kopiert und setzt Blöcke ein (auch aus Excel oder
    Word). Rechtsklick verbindet Zellen, macht eine Spalte zur LaTeX-Spalte
    (Indizes, Formeln, Farben) oder richtet sie aus. Beim Überfahren einer
    Zeilen- oder Spaltengrenze erscheint ein Plus zum Einfügen. Drei
    Bearbeitungsarten, umschaltbar im Kopf der Ansicht: direkt in der Zelle,
    Raster mit Spaltenbuchstaben und Eingabezeile, Zelleditor mit Vorschau.
    Ergänzte, entfernte und umbenannte Spalten gelten nur für diese
    Verwendung des Moduls.
  - *Liste:* Eingabe legt den nächsten Punkt an, Rücktaste im leeren Punkt
    entfernt ihn.
  - *Bild:* anklicken, hineinziehen oder mit Strg+V einfügen.
  - *Abschnitte:* am Griff ziehen oder mit Strg+↑/↓ verschieben, Plus
    zwischen den Abschnitten fügt ein.
- **PDF:** Ein Klick springt zur Stelle im Editor (SyncTeX), die gewählte
  Stelle des Editors ist im PDF markiert. Bei Fehlern bleibt der letzte gute
  Stand sichtbar und ist als älter markiert; Diagnosen springen zur Stelle.
  „Als PDF speichern“ schreibt genau die angezeigten Bytes.
- **Ansichten:** Kacheln am Griff umordnen, Trennlinien ziehen, mit
  Strg+1/2/3 ein- und ausblenden. Die Anordnung bleibt erhalten. Die
  Ansichten scrollen synchron; der visuelle Editor zeigt, wo im PDF eine
  neue Seite beginnt.
- **Nerd-Modus:** Ein freies LaTeX-Dokument ist reiner Quelltext mit
  PDF-Vorschau, ohne Module und ohne Schutz (Vorlage „Freies LaTeX“ oder
  Datei → Als freies LaTeX weiterführen, eine Kopie). Geführte Dokumente
  bleiben geschützt. Die Vervollständigung im Quelltext kennt LaTeX-Befehle
  und die Befehle der eigenen Vorlagen; Strg+Umschalt+L schlägt sie nach.
- **Befehle:** Menüleiste und Befehlspalette (Strg+K) zeigen dieselben
  Befehle. Die Palette findet auch Module, Vorlagen und zuletzt geöffnete
  Dokumente. **F1** zeigt alle Tastenkürzel.

## Module, Vorlagen und Bibliothek

Die Bibliothek besteht aus mitgelieferten, neutralen Bausteinen (Überschrift,
Text, Liste, Tabelle, Kenndaten, Messwerte, Bild, Titelblock, …) und dem
Ordner des Anwenders, `Dokumente/Formavium/Bibliothek`:

```text
Bibliothek/
  module/<kennung>@<version>.json     ein Modul je Datei und Version
  vorlagen/<kennung>/vorlage.fvm      eine Vorlage ist ein Dokument
  vorlagen/<kennung>/bilder/…         mit seinen Bildern
```

`FORMAVIUM_LIBRARY` zeigt auf einen anderen Ordner, etwa ein gemeinsames
Laufwerk. Firmenspezifische Module und Vorlagen gehören dorthin, nicht in
dieses Repository.

- **Modul einfügen** (Strg+M): Auswahl mit Suche, UND-Filtern nach Inhalt
  und Aufbau-Miniaturen.
- **Modul-Werkstatt** (Doppelklick auf den Modulkopf): bearbeitet die
  Definition im Dokument. Jede Änderung wirkt sofort auf visuelle Ansicht
  und PDF. „In Bibliothek speichern“ legt eine neue Version an.
- **LaTeX wird Modul:** Rechtsklick auf einen freien LaTeX-Abschnitt, „In
  Modul umwandeln“. In der Werkstatt Text markieren und zum Eingabefeld
  machen.
- **Versionen:** Ein Dokument trägt seine Modulstände mit und bleibt ohne
  Bibliothek lesbar. Gibt es dort eine neuere Version, zeigt das Modul es an
  und übernimmt sie auf Wunsch (rückgängig möglich).
- **Vorlagen:** „Neu aus Vorlage“ (Strg+N) und „Als Vorlage speichern“, auf
  Wunsch mit geleerten Eingaben.

Feldmarken im LaTeX-Quelltext eines Moduls:

| Marke | Bedeutung |
|---|---|
| `\feld{name}` | Text des Anwenders, automatisch maskiert |
| `\rohfeld{name}` | LaTeX des Anwenders, unverändert (Formeln, Sonderzeichen) |
| `\bildfeld[optionen]{name}` | Bild, ohne Auswahl ein Platzhalter |

Braucht ein Modul eigene Pakete oder Befehle, gehören sie in seinen
**Vorspann** (Modul-Werkstatt). Mit `\providecommand` definiert, kann eine
Vorlage sie vorgeben, und das Modul funktioniert in jedem Dokument.

## Updates und Feedback

- **Auto-Update:** Die installierte App sucht beim Start im neuesten
  [Release](https://github.com/aze-the2nd/Formavium/releases) nach einer
  höheren Version. Ist noch kein Dokument offen, lädt und installiert sie sie
  sofort und startet neu. Sonst erscheint ein Hinweis in der Statusleiste.
  Ohne Netz geschieht nichts. Das Update-Paket ist signiert, die App prüft
  die Signatur mit dem eingebauten öffentlichen Schlüssel.
- **Feedback** (Strg+Umschalt+F): Fehler oder Idee landen als Issue in
  diesem Repository. Ohne weitere Einrichtung öffnet sich das ausgefüllte
  Formular im Browser. Mit einem hinterlegten Zugriffstoken (Fine-grained,
  nur dieses Repository, „Issues: Read and write“) legt die App das Issue
  direkt an. Das Token liegt in der Windows-Anmeldeinformationsverwaltung.
  Technische Angaben enthalten nie Dokumentinhalte oder Pfade.

## Release

```sh
sh scripts/release.sh 0.3.0        # Version setzen, committen, Tag anlegen
git push origin main v0.3.0        # startet den Release-Build auf GitHub
```

Der Workflow `.github/workflows/release.yml` baut im selben Container wie
lokal, führt die Tests aus und veröffentlicht:

| Datei | Zweck |
|---|---|
| `Formavium_<v>_x64-setup.exe` (ca. 28 MiB) | schlanker Installer, lädt WebView2 bei Bedarf nach; Grundlage des Auto-Updates |
| `Formavium_<v>_x64-offline-setup.exe` (ca. 310 MiB) | mit WebView2-Offline-Installer, für Rechner ohne Netz |
| `latest.json`, `*.sig` | Update-Beschreibung und Signatur |
| `release-manifest.json`, `SHA256SUMS` | Versionen und Prüfsummen |

Voraussetzung ist das Repository-Secret `TAURI_SIGNING_PRIVATE_KEY` mit dem
privaten Signaturschlüssel. Geht der Schlüssel verloren, können installierte
Apps keine Updates mehr annehmen.

Lokal baut `docker compose exec dev sh scripts/build-windows.sh` dieselben
Pakete nach `dist/windows/` (ohne Schlüssel in der Umgebung ohne Signatur).

Das Skript kompiliert jedes Korpusdokument aus `docker/tex-corpus/` und jede
mitgelieferte Vorlage und prüft danach mit `--only-cached`, dass alle
TeX-Ressourcen offline vorliegen. Beim ersten Start kopiert die App das Paket
nach `%LOCALAPPDATA%\com.formavium.app\tectonic-cache` und rendert danach
ausschließlich offline (`FORMAVIUM_ALLOW_NETWORK=1` erlaubt das Nachladen
fehlender Pakete). Eigene Vorlagen können deshalb nur Pakete nutzen, die im
Korpus vorkommen.

Hinweis für Vorlagen: Tectonic findet Schriften aus dem Bundle über den
Dateinamen (`\setmainfont{texgyreheros}[Extension=.otf, …]`), nicht über
den Systemnamen („TeX Gyre Heros“).

## Aufbau

| Teil | Inhalt |
|---|---|
| `crates/formavium-core` | Dokumentmodell (`.fvm`, Schema 1), Module mit Feldern, Tabellen, Listen, Bildern, Anordnung, Untermodulen und LaTeX mit Feldmarken, lokale Spaltenanpassungen je Verwendung, Revisionen mit Rückgängig, sicheres Speichern mit Konflikterkennung, LaTeX-Erzeugung mit Maskierung und Positionszuordnung, Abbildung von Quelltextänderungen auf Felder/Zellen/Teile, Bibliothek mit Versionen und Vorlagen, LaTeX-Export |
| `crates/formavium-render` | Tectonic im eigenen Prozess (`--untrusted`, festes Build-Datum, Zeitlimit, Abbruch), Diagnosen, Warteschlange „nur jüngste Revision wartet“, Vorschauzustand mit letztem gutem PDF |
| `crates/formavium-session` | Sitzung eines offenen Dokuments: eine Schnittstelle (`call`, `pdf`, `resource`) für Desktop-Hülle und Dev-Bridge; `formavium-devbridge` stellt sie per HTTP bereit |
| `crates/formavium-cli` | `formavium new / templates / library / info / latex / render / export / set / set-latex / move-column` |
| `app/` | Svelte-5-Oberfläche: Quelltext (CodeMirror 6), visueller Editor, PDF-Vorschau (PDF.js) als verschiebbare Kacheln, Menüleiste, Befehlspalette, Modulauswahl und Werkstatt |
| `app/src-tauri` | Tauri-2-Hülle: Sitzungsbefehle, Dateidialoge, Updates, Feedback |

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
(das Projekt liegt unter `/work`, Beispiele unter `/work/out`, die eigene
Bibliothek unter `/work/out/bibliothek`). Die Dev-Bridge liest und schreibt
beliebige Pfade und ist nur an `127.0.0.1` im Container gebunden; nach außen
ist nur Vite auf `127.0.0.1:5173` freigegeben.

### Kommandozeile

```sh
docker compose exec dev sh -c 'cargo build && mkdir -p out && cd out &&
  /target/debug/formavium new bericht/bericht.fvm bericht &&
  /target/debug/formavium render bericht/bericht.fvm'
```

`CARGO_TARGET_DIR` liegt im Container unter `/target` (Volume). Der
Tectonic-Cache liegt im Volume `tectonic-cache`. Der erste Lauf lädt die
benötigten TeX-Ressourcen, danach funktioniert `formavium render --offline`.

Zieh-Gesten der Oberfläche nutzen Zeiger-Ereignisse statt HTML5-Drag-and-Drop,
damit Datei-Drops aus dem Explorer (Bilder) im selben Fenster funktionieren.

## Offen

- Die Installer sind noch nicht systematisch geprüft (frische VM,
  Standardbenutzer, ohne Netz, mit und ohne WebView2-Runtime).
- Keine Code-Signatur (SmartScreen-Warnung beim Start des Installers).
- Untermodule in der Werkstatt, Modulupdates mit Abgleich lokaler Änderungen.
- Vollständige Referenz der LaTeX-Befehle in der App.

## Lizenz

MIT, siehe [LICENSE](LICENSE).
