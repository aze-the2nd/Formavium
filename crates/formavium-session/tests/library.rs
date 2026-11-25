//! Bibliothek über die Sitzungsschnittstelle: Vorlagen, Module einfügen,
//! in der Werkstatt ändern, veröffentlichen und aktualisieren.

use formavium_session::Session;
use serde_json::{json, Value};

fn rev(doc: &Value) -> u64 {
    doc["revision"].as_u64().unwrap()
}

fn edit(s: &mut Session, doc: &Value, command: Value) -> Value {
    s.call("edit", &json!({ "base": rev(doc), "command": command }))
        .unwrap()
}

#[test]
fn library_flow_from_template_to_published_module() {
    let lib = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    s.set_library_dir(Some(lib.path().to_owned()));

    // Mitgelieferte Bausteine und Vorlagen.
    let library = s.call("library", &json!({})).unwrap();
    assert_eq!(library["dir"], json!(lib.path()));
    let modules = library["modules"].as_array().unwrap();
    assert!(modules.len() >= 20);
    assert!(modules
        .iter()
        .all(|m| m["source"] == "builtin" && m["latest"] == true));
    let table = modules.iter().find(|m| m["id"] == "messwerte").unwrap();
    assert_eq!(table["kinds"], json!(["text", "table"]));
    assert_eq!(table["outline"][1], json!({ "kind": "table", "cols": 6 }));
    let templates: Vec<&str> = library["templates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        templates,
        ["leer", "bericht", "datenblatt", "montageanleitung"]
    );

    // Neues Dokument aus einer Vorlage, im eigenen Ordner.
    let doc = s
        .call(
            "new_document",
            &json!({ "path": docs.path().join("pruefung.fvm"), "template": "bericht" }),
        )
        .unwrap();
    assert_eq!(
        doc["path"],
        json!(docs.path().join("pruefung").join("pruefung.fvm"))
    );
    assert_eq!(doc["parts"].as_array().unwrap().len(), 8);
    assert_eq!(doc["parts"][0]["name"], "Titelblock");
    // LaTeX-Modul: Eingabefelder als Formular.
    let block = &doc["parts"][0]["body"][0];
    assert_eq!(block["kind"], "latex");
    assert_eq!(block["fields"][0]["label"], "Titel");
    assert_eq!(block["fields"][0]["value"], "Bericht");
    assert_eq!(
        doc["parts"][0]["status"],
        json!({ "local": false, "newer": null })
    );

    // Modul aus der Bibliothek einfügen.
    let doc = s
        .call(
            "insert_module",
            &json!({ "base": rev(&doc), "index": 1, "module": { "id": "aufzaehlung", "version": 1 } }),
        )
        .unwrap();
    assert_eq!(doc["parts"][1]["body"][0]["kind"], "list");
    assert_eq!(
        doc["parts"][1]["body"][0]["items"][1]["text"],
        "Zweiter Punkt"
    );
    assert!(doc["source"]
        .as_str()
        .unwrap()
        .contains("\\begin{itemize}\n\\item Erster Punkt"));

    // Neues Modul anlegen und in der Werkstatt ausbauen.
    let r = s
        .call(
            "new_module",
            &json!({ "base": rev(&doc), "index": 2, "name": "Prüfschritt" }),
        )
        .unwrap();
    assert_eq!(r["module"], json!({ "id": "pruefschritt", "version": 1 }));
    let doc = &r["doc"];
    assert_eq!(doc["parts"][2]["status"]["local"], true);
    let mut def = s
        .call("module_def", &json!({ "module": r["module"] }))
        .unwrap();
    def["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "norm", "label": "Norm", "kind": "text",
                      "default": { "type": "text", "value": "EN 61558" } }));
    def["body"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "zeile", "kind": "latex", "source": "\\textbf{Norm:} \\feld{norm}" }));
    let doc = edit(&mut s, doc, json!({ "op": "update_module", "def": def }));
    assert!(doc["source"]
        .as_str()
        .unwrap()
        .contains("\\textbf{Norm:} EN 61558"));

    // Ungültige Definitionen werden mit Begründung abgelehnt.
    let mut broken = def.clone();
    broken["body"][2]["source"] = json!("\\feld{gibtsnicht}");
    let err = s
        .call(
            "edit",
            &json!({ "base": rev(&doc), "command": { "op": "update_module", "def": broken } }),
        )
        .unwrap_err();
    assert!(err.contains("gibtsnicht"), "{err}");

    // Veröffentlichen: erste Version in der eigenen Bibliothek.
    let r = s
        .call(
            "publish_module",
            &json!({ "base": rev(&doc), "module": { "id": "pruefschritt", "version": 1 } }),
        )
        .unwrap();
    assert_eq!(r["module"]["version"], 1);
    assert_eq!(r["doc"]["parts"][2]["status"]["local"], false);
    assert!(lib.path().join("module/pruefschritt@1.json").is_file());

    // Weitere Änderung, erneut veröffentlichen: Version 2, Dokument zieht mit.
    def["name"] = json!("Prüfschritt mit Norm");
    let doc = edit(
        &mut s,
        &r["doc"],
        json!({ "op": "update_module", "def": def }),
    );
    assert_eq!(doc["parts"][2]["status"]["local"], true);
    let r = s
        .call(
            "publish_module",
            &json!({ "base": rev(&doc), "module": { "id": "pruefschritt", "version": 1 } }),
        )
        .unwrap();
    assert_eq!(r["module"]["version"], 2);
    assert_eq!(r["doc"]["parts"][2]["module"], "pruefschritt@2");
    assert_eq!(r["doc"]["parts"][2]["name"], "Prüfschritt mit Norm");

    // Dokument als Vorlage speichern und daraus ein neues anlegen.
    s.call("save", &json!({})).unwrap();
    let saved = s
        .call(
            "save_template",
            &json!({ "title": "Prüfbericht Labor", "description": "Mit Prüfschritt", "keep_values": false }),
        )
        .unwrap();
    assert_eq!(saved["id"], "pruefbericht-labor");
    let library = s.call("library", &json!({})).unwrap();
    let own = library["templates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == "pruefbericht-labor")
        .unwrap();
    assert_eq!(own["source"], "user");
    assert_eq!(own["description"], "Mit Prüfschritt");
    let doc = s
        .call(
            "new_document",
            &json!({ "path": docs.path().join("zweites.fvm"), "template": "pruefbericht-labor" }),
        )
        .unwrap();
    assert_eq!(doc["title"], "Prüfbericht Labor");
    assert_eq!(doc["parts"][2]["module"], "pruefschritt@2");

    s.call("delete_template", &json!({ "id": "pruefbericht-labor" }))
        .unwrap();
    assert!(!lib.path().join("vorlagen/pruefbericht-labor").exists());
}

#[test]
fn documents_learn_about_newer_library_versions() {
    let lib = tempfile::tempdir().unwrap();
    let docs = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    s.set_library_dir(Some(lib.path().to_owned()));

    // Erstes Dokument: Modul anlegen und als Version 1 veröffentlichen.
    let doc = s
        .call(
            "new_document",
            &json!({ "path": docs.path().join("a.fvm"), "template": "leer" }),
        )
        .unwrap();
    let r = s
        .call(
            "new_module",
            &json!({ "base": rev(&doc), "index": 1, "name": "Kopf" }),
        )
        .unwrap();
    let r = s
        .call(
            "publish_module",
            &json!({ "base": rev(&r["doc"]), "module": r["module"] }),
        )
        .unwrap();
    s.call("save", &json!({})).unwrap();
    let old_path = r["doc"]["path"].clone();

    // Zweites Dokument entwickelt das Modul weiter (Version 2).
    let doc = s
        .call(
            "new_document",
            &json!({ "path": docs.path().join("b.fvm"), "template": "leer" }),
        )
        .unwrap();
    let doc = s
        .call(
            "insert_module",
            &json!({ "base": rev(&doc), "index": 1, "module": { "id": "kopf", "version": 1 } }),
        )
        .unwrap();
    let mut def = s
        .call(
            "module_def",
            &json!({ "module": { "id": "kopf", "version": 1 } }),
        )
        .unwrap();
    def["fields"][0]["default"]["value"] = json!("Neuer Vorgabetitel");
    let doc = edit(&mut s, &doc, json!({ "op": "update_module", "def": def }));
    s.call(
        "publish_module",
        &json!({ "base": rev(&doc), "module": { "id": "kopf", "version": 1 } }),
    )
    .unwrap();

    // Das erste Dokument behält Version 1 und erfährt von Version 2.
    let doc = s.call("open", &json!({ "path": old_path })).unwrap();
    assert_eq!(doc["parts"][1]["module"], "kopf@1");
    assert_eq!(
        doc["parts"][1]["status"],
        json!({ "local": false, "newer": 2 })
    );
    assert!(doc["source"].as_str().unwrap().contains("\\section{Kopf}"));

    // Übernahme auf Wunsch, rückgängig möglich.
    let doc = s
        .call(
            "upgrade_module",
            &json!({ "base": rev(&doc), "module": { "id": "kopf", "version": 1 } }),
        )
        .unwrap();
    assert_eq!(doc["parts"][1]["module"], "kopf@2");
    assert!(doc["source"]
        .as_str()
        .unwrap()
        .contains("\\section{Neuer Vorgabetitel}"));
    let doc = s.call("undo", &json!({})).unwrap();
    assert_eq!(doc["parts"][1]["module"], "kopf@1");
}

#[test]
fn free_latex_becomes_a_module() {
    let docs = tempfile::tempdir().unwrap();
    let mut s = Session::default();
    let doc = s
        .call(
            "new_document",
            &json!({ "path": docs.path().join("m.fvm"), "template": "montageanleitung" }),
        )
        .unwrap();
    assert_eq!(doc["parts"][0]["kind"], "latex");
    let r = s
        .call(
            "part_to_module",
            &json!({ "base": rev(&doc), "part": "p1", "name": "Einleitung" }),
        )
        .unwrap();
    let part = &r["doc"]["parts"][0];
    assert_eq!(part["kind"], "module");
    assert_eq!(part["id"], "p1");
    assert_eq!(part["status"]["local"], true);
    // Der erzeugte Quelltext bleibt gleich, bis auf die Leerzeile, die auf
    // jedes Modul folgt.
    let lines = |v: &Value| -> Vec<String> {
        let text = v["doc"]["source"]
            .as_str()
            .or(v["source"].as_str())
            .unwrap();
        text.lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(lines(&r), lines(&doc));
    // Module lassen sich nicht umwandeln.
    assert!(s
        .call(
            "part_to_module",
            &json!({ "base": rev(&r["doc"]), "part": "p2", "name": "x" })
        )
        .is_err());
    // Ohne Bibliotheksordner lässt sich nichts veröffentlichen.
    assert!(s
        .call(
            "publish_module",
            &json!({ "base": rev(&r["doc"]), "module": r["module"] })
        )
        .is_err());
}
