//! PDF ↔ Dokument über SyncTeX mit echtem Tectonic.

use std::time::{Duration, Instant};

use formavium_session::Session;
use serde_json::{json, Value};

fn tectonic_ok() -> bool {
    std::process::Command::new(formavium_render::locate_tectonic())
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

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

#[test]
fn pdf_positions_map_to_cells_and_back() {
    if !tectonic_ok() {
        eprintln!("Tectonic fehlt, Test übersprungen");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    s.call(
        "new_document",
        &json!({ "path": dir.path().join("a.fvm"), "template": "montageanleitung" }),
    )
    .unwrap();
    assert_eq!(wait_preview(&mut s)["status"], "current");

    // Übersicht: Teile mit Zeilen, Zeilen mit Seitenlage.
    let sync = s.call("sync", &json!({})).unwrap();
    assert_eq!(sync["pages"], 1);
    assert_eq!(sync["current"], true);
    let parts = sync["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 4);
    let first_line = parts[1][1].as_u64().unwrap() as usize;
    assert!(sync["lines"].as_array().unwrap()[first_line..]
        .iter()
        .any(|l| !l.is_null()));

    // Zelle „Menge“ der Zeile r2 im ersten Modul markieren …
    let rects = s
        .call(
            "highlight",
            &json!({ "targets": [{ "part": "p2", "path": "zeilen", "row": "r2", "column": "menge" }] }),
        )
        .unwrap();
    let rect = rects[0].as_array().unwrap();
    assert_eq!(rect[0], 1);
    let (x, y, w, h) = (
        rect[1].as_f64().unwrap(),
        rect[2].as_f64().unwrap(),
        rect[3].as_f64().unwrap(),
        rect[4].as_f64().unwrap(),
    );
    assert!(w > 5.0 && w < 120.0 && h > 5.0 && h < 30.0, "{rect:?}");

    // … und ein Klick in ihre Mitte führt zur selben Zelle zurück.
    let hit = s
        .call(
            "locate",
            &json!({ "page": 1, "x": x + w / 2.0, "y": y + h / 2.0 }),
        )
        .unwrap();
    assert_eq!(hit["kind"], "cell", "{hit}");
    assert_eq!(hit["part"], "p2");
    assert_eq!(hit["row"], "r2");
    assert_eq!(hit["column"], "menge");

    // Feld: die Überschrift des zweiten Moduls.
    let rects = s
        .call(
            "highlight",
            &json!({ "targets": [{ "part": "p4", "path": "titel" }] }),
        )
        .unwrap();
    assert!(!rects.as_array().unwrap().is_empty());
    let r = rects[0].as_array().unwrap();
    let hit = s
        .call(
            "locate",
            &json!({ "page": 1, "x": r[1].as_f64().unwrap() + 2.0, "y": r[2].as_f64().unwrap() + r[4].as_f64().unwrap() / 2.0 }),
        )
        .unwrap();
    assert_eq!(hit["part"], "p4", "{hit}");
    assert_eq!(hit["kind"], "field", "{hit}");
}
