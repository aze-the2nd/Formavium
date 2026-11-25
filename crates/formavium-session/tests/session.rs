use std::time::{Duration, Instant};

use formavium_session::Session;
use serde_json::{json, Value};

fn wait_preview(s: &mut Session) -> Value {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let p = s.call("preview", &json!({})).unwrap();
        if !p["pending"].as_bool().unwrap() {
            return p;
        }
        assert!(Instant::now() < deadline, "Rendern dauert zu lange");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn utf16_index(text: &str, needle: &str) -> usize {
    let byte = text.find(needle).expect(needle);
    text[..byte].encode_utf16().count()
}

#[test]
fn full_session_flow() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("anleitung.fvm");
    let mut s = Session::default();
    let doc = s.call("new_sample", &json!({ "path": path })).unwrap();
    // Das Dokument bekommt einen eigenen Ordner mit seinem Namen.
    let path = dir.path().join("anleitung").join("anleitung.fvm");
    assert_eq!(doc["path"], json!(path));
    assert!(path.with_file_name("bilder").join("beispiel.png").exists());
    assert_eq!(doc["parts"].as_array().unwrap().len(), 4);
    assert_eq!(doc["parts"][1]["kind"], "module");
    assert_eq!(
        doc["parts"][1]["body"][1]["children"][0]["columns"][2]["header"],
        "Länge"
    );

    let tectonic_ok = std::process::Command::new(formavium_render::locate_tectonic())
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());

    // Visuelle Bearbeitung: Titel der zweiten Instanz.
    let rev = doc["revision"].as_u64().unwrap();
    let doc = s
        .call(
            "edit",
            &json!({ "base": rev, "command": { "op": "set_value", "part": "p4", "path": "titel",
                     "value": { "type": "text", "value": "Ersatz – Größe Ø" } } }),
        )
        .unwrap();
    assert!(doc["source"]
        .as_str()
        .unwrap()
        .contains("\\section{Ersatz – Größe Ø}"));
    assert!(doc["dirty"].as_bool().unwrap());

    // Quelltextbearbeitung mit UTF-16-Positionen hinter Nicht-ASCII-Zeichen.
    let src = doc["source"].as_str().unwrap();
    let from = utf16_index(src, "Ø}") + 1;
    let rev = doc["revision"].as_u64().unwrap();
    let r = s
        .call(
            "source_edit",
            &json!({ "base": rev, "from": from, "to": from, "insert": " 8" }),
        )
        .unwrap();
    assert_eq!(r["outcome"], "applied");
    assert!(r["doc"]["source"]
        .as_str()
        .unwrap()
        .contains("\\section{Ersatz – Größe Ø 8}"));

    // Strukturänderung verlangt Bestätigung.
    let src = r["doc"]["source"].as_str().unwrap();
    let rev = r["doc"]["revision"].as_u64().unwrap();
    let at = utf16_index(src, "\\hline");
    let args = json!({ "base": rev, "from": at, "to": at + 6, "insert": "\\hline\\hline" });
    let r = s.call("source_edit", &args).unwrap();
    assert_eq!(r["outcome"], "needs_conversion");
    let mut args = args;
    args["allow_convert"] = json!(true);
    let r = s.call("source_edit", &args).unwrap();
    assert_eq!(r["converted"], true);
    let doc = s.call("undo", &json!({})).unwrap();
    assert_eq!(doc["parts"][1]["kind"], "module");

    // Speichern und erneut öffnen.
    s.call("save", &json!({})).unwrap();
    let mut s2 = Session::default();
    let reopened = s2.call("open", &json!({ "path": path })).unwrap();
    assert_eq!(reopened["source"], doc["source"]);
    assert_eq!(reopened["dirty"], false);

    if !tectonic_ok {
        eprintln!("Tectonic fehlt, Renderteil übersprungen");
        return;
    }
    let p = wait_preview(&mut s);
    assert_eq!(p["status"], "current", "{p}");
    let (rev, bytes) = s.pdf().unwrap();
    assert_eq!(Some(rev), p["shown_revision"].as_u64());
    assert!(bytes.starts_with(b"%PDF"));

    // Fehler: letzter Stand bleibt sichtbar, Stelle wird benannt.
    let doc = s.call("state", &json!({})).unwrap();
    let rev = doc["revision"].as_u64().unwrap();
    s.call(
        "edit",
        &json!({ "base": rev, "command": { "op": "set_latex", "part": "p3", "source": "\\kaputt\n" } }),
    )
    .unwrap();
    let p = wait_preview(&mut s);
    assert_eq!(p["status"], "stale", "{p}");
    assert_eq!(p["diagnostics"][0]["part"], "p3");
    assert_eq!(p["diagnostics_current"], true);

    let out = dir.path().join("ausgabe.pdf");
    let saved = s.call("save_pdf", &json!({ "path": out })).unwrap();
    assert_eq!(saved["sha256"], p["sha256"]);
    assert_eq!(std::fs::read(&out).unwrap(), *bytes);
}

#[test]
fn folder_named_like_the_document_is_used_as_is() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Anleitung").join("anleitung.fvm");
    let mut s = Session::default();
    let doc = s.call("new_sample", &json!({ "path": path })).unwrap();
    assert_eq!(doc["path"], json!(path));
}

#[test]
fn images_are_imported_from_bytes_and_served_for_display() {
    use base64::Engine as _;
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    let doc = s
        .call("new_sample", &json!({ "path": dir.path().join("a.fvm") }))
        .unwrap();
    let image = &doc["parts"][1]["body"][1]["children"][1];
    assert_eq!(image["value"], "bilder/beispiel.png");
    assert_eq!(image["width"], 1.0);
    let (mime, bytes) = s.resource("bilder/beispiel.png").unwrap();
    assert_eq!(mime, "image/png");
    assert_eq!(&bytes[1..4], b"PNG");
    // Nichts außerhalb des Projektordners.
    assert!(s.resource("../a.fvm").is_none());
    assert!(s.resource("a.fvm").is_none());

    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let r = s
        .call(
            "import_image_data",
            &json!({ "name": "Foto Montage (1).PNG", "data": b64(&bytes) }),
        )
        .unwrap();
    assert_eq!(r["path"], "bilder/foto-montage--1-.png");
    let again = s
        .call(
            "import_image_data",
            &json!({ "name": "Foto Montage (1).PNG", "data": b64(&bytes) }),
        )
        .unwrap();
    assert_eq!(again["path"], "bilder/foto-montage--1--2.png");
    assert_eq!(s.resource("bilder/foto-montage--1-.png").unwrap().1, bytes);

    // Falscher Inhalt und fremde Formate werden abgelehnt.
    let err = s
        .call(
            "import_image_data",
            &json!({ "name": "x.png", "data": b64(b"kein bild") }),
        )
        .unwrap_err();
    assert!(err.contains("PNG"), "{err}");
    assert!(s
        .call(
            "import_image_data",
            &json!({ "name": "x.svg", "data": b64(b"<svg/>") })
        )
        .is_err());

    // Tabellenbefehle der Oberfläche.
    let rev = doc["revision"].as_u64().unwrap();
    let doc = s
        .call(
            "edit",
            &json!({ "base": rev, "command": { "op": "insert_column", "part": "p2",
                     "table": "tabelle", "index": 3, "header": "Hinweis" } }),
        )
        .unwrap();
    let table = &doc["parts"][1]["body"][1]["children"][0];
    assert_eq!(table["columns"][3]["header"], "Hinweis");
    assert_eq!(table["columns"][1]["align"], "right");
    let rev = doc["revision"].as_u64().unwrap();
    let doc = s
        .call(
            "edit",
            &json!({ "base": rev, "command": { "op": "insert_row", "part": "p2",
                     "path": "zeilen", "index": 2 } }),
        )
        .unwrap();
    let rows = doc["parts"][1]["body"][1]["children"][0]["rows"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2]["cells"], json!(["", "", "", ""]));
}
