//! Bibliothek, LaTeX-Module mit Feldern, Listen und Modulversionen.

use std::collections::BTreeMap;

use formavium_core::latex::{field_refs, segments, Origin, Segment};
use formavium_core::library::{slug, validate_module, Library, Source};
use formavium_core::*;

fn text_field(id: &str, default: &str) -> FieldDef {
    FieldDef {
        id: Id::from(id),
        label: id.into(),
        kind: FieldKind::Text,
        default: Some(Value::Text(default.into())),
    }
}

fn latex_module(id: &str, source: &str, fields: Vec<FieldDef>) -> ModuleDef {
    ModuleDef {
        id: Id::from(id),
        version: 1,
        name: id.into(),
        description: String::new(),
        preamble: String::new(),
        fields,
        body: vec![Element {
            id: Id::from("inhalt"),
            kind: ElementKind::Latex {
                source: source.into(),
            },
        }],
    }
}

fn doc_with(def: ModuleDef) -> (Workspace, Id) {
    let mut ws = Workspace::new(Project::new("T", "\\documentclass{article}"));
    let r = ws.revision();
    let (_, id) = ws
        .apply(
            r,
            Command::InsertModule {
                index: 0,
                def,
                requires: Vec::new(),
            },
        )
        .unwrap();
    (ws, id.unwrap())
}

fn apply(ws: &mut Workspace, cmd: Command) {
    let r = ws.revision();
    ws.apply(r, cmd).unwrap();
}

#[test]
fn field_marks_are_parsed_and_plain_macros_left_alone() {
    let src = "A \\feld{titel} B \\rohfeld{formel} \\bildfeld[width=3cm]{foto}\\feldx{y} \\bildfeld{logo} \\feld{";
    assert_eq!(
        segments(src),
        vec![
            Segment::Text("A "),
            Segment::Field("titel"),
            Segment::Text(" B "),
            Segment::Raw("formel"),
            Segment::Text(" "),
            Segment::Image {
                options: "[width=3cm]",
                field: "foto"
            },
            Segment::Text("\\feldx{y} "),
            Segment::Image {
                options: "",
                field: "logo"
            },
            Segment::Text(" \\feld{"),
        ]
    );
    assert_eq!(field_refs(src), ["titel", "formel", "foto", "logo"]);
}

#[test]
fn latex_module_fields_are_escaped_and_editable_from_source() {
    let def = latex_module(
        "kasten",
        "\\fbox{\\feld{text}} $\\rohfeld{formel}$",
        vec![
            text_field("text", "50 % & mehr"),
            text_field("formel", "a_1^2"),
        ],
    );
    let (mut ws, part) = doc_with(def);
    let g = generate(ws.project());
    assert!(
        g.text.contains("\\fbox{50 \\% \\& mehr} $a_1^2$"),
        "{}",
        g.text
    );
    // Der maskierte Text ist dem Feld zugeordnet, die Rohformel nicht.
    let at = g.text.find("50 \\%").unwrap();
    assert_eq!(
        g.origin_at(at),
        Some(&Origin::Field {
            part: part.clone(),
            path: "text".into()
        })
    );
    let at = g.text.find("a_1").unwrap();
    assert!(matches!(g.origin_at(at), Some(Origin::Module { .. })));

    apply(
        &mut ws,
        Command::SetValue {
            part,
            path: "text".into(),
            value: Value::Text("neu".into()),
        },
    );
    assert!(generate(ws.project()).text.contains("\\fbox{neu}"));
}

#[test]
fn image_fields_in_latex_become_graphics_or_a_placeholder() {
    let def = latex_module(
        "bild",
        "\\bildfeld[width=0.5\\linewidth]{foto}",
        vec![FieldDef {
            id: Id::from("foto"),
            label: "Foto".into(),
            kind: FieldKind::Image,
            default: None,
        }],
    );
    let (mut ws, part) = doc_with(def);
    let g = generate(ws.project());
    assert!(g.text.contains("\\fbox"), "{}", g.text);
    assert!(g.text.contains("\\usepackage{graphicx}"));
    assert!(g.resources.is_empty());

    apply(
        &mut ws,
        Command::SetValue {
            part,
            path: "foto".into(),
            value: Value::Image("bilder/a.png".into()),
        },
    );
    let g = generate(ws.project());
    assert!(g
        .text
        .contains("\\includegraphics[width=0.5\\linewidth]{bilder/a.png}"));
    assert!(g.resources.contains("bilder/a.png"));
}

#[test]
fn graphics_in_free_latex_are_collected_as_resources() {
    let mut p = Project::new(
        "T",
        "\\documentclass{article}\n\\usepackage{graphicx}\n\\newcommand{\\logo}{\\includegraphics[height=1cm]{bilder/logo.pdf}}",
    );
    let id = p.fresh_id("p");
    p.parts.push(Part {
        id,
        content: PartContent::Latex {
            source: "\\includegraphics{bilder/foto.jpg} \\includegraphics{../geheim.png} \\includegraphics{ohne-endung}\n".into(),
        },
    });
    let g = generate(&p);
    let found: Vec<&str> = g.resources.iter().map(String::as_str).collect();
    assert_eq!(found, ["bilder/foto.jpg", "bilder/logo.pdf"]);
}

#[test]
fn lists_render_items_and_map_them_to_cells() {
    let def = builtin::modules()
        .into_iter()
        .find(|m| m.id.as_str() == "nummerierte-liste")
        .unwrap();
    let (mut ws, part) = doc_with(def);
    let g = generate(ws.project());
    assert!(
        g.text.contains(
            "\\begin{enumerate}\n\\item Erster Schritt\n\\item Zweiter Schritt\n\\end{enumerate}"
        ),
        "{}",
        g.text
    );
    let at = g.text.find("Zweiter").unwrap();
    assert!(
        matches!(g.origin_at(at), Some(Origin::Cell { column, .. }) if column.as_str() == "text")
    );

    apply(
        &mut ws,
        Command::InsertRow {
            part: part.clone(),
            path: "punkte".into(),
            index: 2,
        },
    );
    assert!(generate(ws.project())
        .text
        .contains("\\item \n\\end{enumerate}"));

    // Eine leere Liste erzeugt keine Umgebung.
    apply(
        &mut ws,
        Command::SetValue {
            part,
            path: "punkte".into(),
            value: Value::Table(Vec::new()),
        },
    );
    assert!(!generate(ws.project()).text.contains("enumerate"));
}

#[test]
fn fixed_width_columns_wrap_and_load_array() {
    let def = builtin::modules()
        .into_iter()
        .find(|m| m.id.as_str() == "aenderungshistorie")
        .unwrap();
    let (ws, _) = doc_with(def);
    let g = generate(ws.project());
    assert!(
        g.text
            .contains("{|l|l|>{\\raggedright\\arraybackslash}p{0.500\\linewidth}|l|}"),
        "{}",
        g.text
    );
    assert!(g.text.contains("\\usepackage{array}"));
}

#[test]
fn builtin_library_is_valid_and_self_contained() {
    let lib = Library::new(None);
    let modules = lib.modules();
    assert!(modules.len() >= 20);
    for entry in &modules {
        validate_module(&entry.def).unwrap_or_else(|e| panic!("{}: {e}", entry.def.id));
        assert_eq!(entry.source, Source::Builtin);
        lib.closure(&entry.def, &[]).unwrap();
    }
    let mut ids: Vec<&str> = modules.iter().map(|m| m.def.id.as_str()).collect();
    ids.sort_unstable();
    let count = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), count, "Kennungen müssen eindeutig sein");

    for template in lib.templates() {
        let g = generate(&template.project);
        assert!(g.notices.is_empty(), "{}: {:?}", template.id, g.notices);
        assert!(g.text.contains("\\begin{document}"));
    }
}

#[test]
fn saving_a_module_creates_versions_only_for_changes() {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::new(Some(dir.path().to_owned()));
    let mut def = latex_module(
        "pruefschritt",
        "\\feld{text}",
        vec![text_field("text", "a")],
    );
    def.name = "Prüfschritt".into();

    let first = lib.save_module(&def).unwrap();
    assert_eq!(first.version, 1);
    assert!(dir.path().join("module/pruefschritt@1.json").is_file());
    // Unverändert: keine neue Version.
    assert_eq!(lib.save_module(&def).unwrap().version, 1);

    def.fields[0].default = Some(Value::Text("b".into()));
    let second = lib.save_module(&def).unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(lib.latest_version("pruefschritt"), Some(2));
    assert_eq!(lib.module(&first.reference()).unwrap(), first);

    let own: Vec<_> = lib
        .modules()
        .into_iter()
        .filter(|m| m.source == Source::User)
        .collect();
    assert_eq!(own.len(), 2);

    lib.delete_module(&first.reference()).unwrap();
    assert!(lib.module(&first.reference()).is_none());

    // Ohne Ordner lässt sich nichts speichern.
    assert!(Library::new(None).save_module(&def).is_err());
}

#[test]
fn invalid_modules_are_rejected_with_a_reason() {
    let mut def = latex_module("x", "\\feld{fehlt}", vec![]);
    assert!(validate_module(&def).unwrap_err().contains("fehlt"));
    def.body = vec![];
    def.name = " ".into();
    assert!(validate_module(&def).unwrap_err().contains("Namen"));
    assert_eq!(
        slug("Prüfbericht – Übertrager (2025)"),
        "pruefbericht-uebertrager-2025"
    );
    assert_eq!(slug("  "), "unbenannt");
}

#[test]
fn updating_a_module_changes_all_uses_and_prunes_stale_values() {
    let def = latex_module(
        "kopf",
        "\\feld{a} \\feld{b}",
        vec![text_field("a", "A"), text_field("b", "B")],
    );
    let (mut ws, part) = doc_with(def.clone());
    apply(
        &mut ws,
        Command::SetValue {
            part: part.clone(),
            path: "b".into(),
            value: Value::Text("eigener Wert".into()),
        },
    );
    let mut changed = def.clone();
    changed.fields.truncate(1);
    changed.body[0].kind = ElementKind::Latex {
        source: "[\\feld{a}]".into(),
    };
    apply(&mut ws, Command::UpdateModule { def: changed });
    let p = ws.project();
    assert!(generate(p).text.contains("[A]"));
    assert_eq!(p.instance(&part).unwrap().values, BTreeMap::new());

    // Rückgängig stellt Definition und Wert wieder her.
    ws.undo().unwrap();
    assert!(generate(ws.project()).text.contains("A eigener Wert"));
}

#[test]
fn switching_to_a_new_version_keeps_values_and_drops_the_old_definition() {
    let v1 = latex_module("kopf", "\\feld{a}", vec![text_field("a", "A")]);
    let (mut ws, part) = doc_with(v1.clone());
    apply(
        &mut ws,
        Command::SetValue {
            part: part.clone(),
            path: "a".into(),
            value: Value::Text("bleibt".into()),
        },
    );
    let mut v2 = latex_module(
        "kopf",
        "\\feld{a} / \\feld{neu}",
        vec![text_field("a", "A"), text_field("neu", "N")],
    );
    v2.version = 2;
    apply(
        &mut ws,
        Command::UseModuleVersion {
            id: Id::from("kopf"),
            from: 1,
            def: v2.clone(),
            requires: Vec::new(),
        },
    );
    let p = ws.project();
    assert_eq!(p.modules, vec![v2]);
    assert_eq!(p.instance(&part).unwrap().module.version, 2);
    assert!(generate(p).text.contains("bleibt / N"));
}

#[test]
fn free_latex_part_becomes_a_module_instance() {
    let mut p = Project::new("T", "\\documentclass{article}");
    let id = p.fresh_id("p");
    p.parts.push(Part {
        id: id.clone(),
        content: PartContent::Latex {
            source: "\\section{Alt}\n".into(),
        },
    });
    let mut ws = Workspace::new(p);
    let def = latex_module(
        "abschnitt-alt",
        "\\section{\\feld{t}}",
        vec![text_field("t", "Alt")],
    );
    apply(
        &mut ws,
        Command::ConvertPartToModule {
            part: id.clone(),
            def,
        },
    );
    assert!(ws.project().instance(&id).is_some());
    assert!(generate(ws.project()).text.contains("\\section{Alt}"));
}

#[test]
fn templates_are_saved_with_images_and_without_foreign_values() {
    let lib_dir = tempfile::tempdir().unwrap();
    let doc_dir = tempfile::tempdir().unwrap();
    let lib = Library::new(Some(lib_dir.path().to_owned()));
    std::fs::create_dir_all(doc_dir.path().join("bilder")).unwrap();
    std::fs::write(
        doc_dir.path().join("bilder/beispiel.png"),
        sample::SAMPLE_PNG,
    )
    .unwrap();
    std::fs::write(doc_dir.path().join("bilder/logo.png"), sample::SAMPLE_PNG).unwrap();

    let mut project = sample::anleitung();
    project.title = "Firmenbericht".into();
    project.preamble.push_str(
        "\n\\usepackage{graphicx}\n\\newcommand{\\logo}{\\includegraphics{bilder/logo.png}}",
    );
    let resources = generate(&project).resources;
    assert!(resources.contains("bilder/logo.png") && resources.contains("bilder/beispiel.png"));

    let id = lib
        .save_template(&project, doc_dir.path(), &resources, false)
        .unwrap();
    assert_eq!(id, "firmenbericht");
    let saved = lib.template(&id).unwrap();
    assert_eq!(saved.source, Source::User);
    // Eingaben sind geleert, das Logo des Vorspanns bleibt, das Foto nicht.
    assert!(saved
        .project
        .parts
        .iter()
        .all(|p| !matches!(&p.content, PartContent::Module(i) if !i.values.is_empty())));
    let template_dir = lib_dir.path().join("vorlagen/firmenbericht");
    assert!(template_dir.join("bilder/logo.png").is_file());
    assert!(!template_dir.join("bilder/beispiel.png").exists());

    // Neues Dokument aus der Vorlage: Arbeitsdatei und Bilder.
    let target = doc_dir.path().join("neu/neu.fvm");
    let wanted = generate(&saved.project).resources;
    saved.instantiate(&target, &wanted).unwrap();
    assert!(target.is_file());
    assert!(target.with_file_name("bilder").join("logo.png").is_file());

    // Mit Inhalten: Werte und alle Bilder bleiben.
    project.title = "Mit Inhalt".into();
    let id = lib
        .save_template(&project, doc_dir.path(), &resources, true)
        .unwrap();
    assert!(lib_dir
        .path()
        .join("vorlagen")
        .join(&id)
        .join("bilder/beispiel.png")
        .is_file());

    // Mitgelieferte Vorlagen lassen sich nicht überschreiben.
    project.title = "Bericht".into();
    assert!(lib
        .save_template(&project, doc_dir.path(), &resources, true)
        .is_err());
    lib.delete_template("firmenbericht").unwrap();
    assert!(lib.template("firmenbericht").is_none());
    assert!(lib.delete_template("../x").is_err());
}
