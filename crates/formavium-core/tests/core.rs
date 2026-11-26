use std::fs;

use formavium_core::catalog::{content_kinds, matches_all, CatalogError, ContentKind};
use formavium_core::io::{self, IoError};
use formavium_core::latex::Origin;
use formavium_core::sample::{anleitung, materialliste, row};
use formavium_core::*;

fn part_ids(p: &Project) -> Vec<Id> {
    p.parts.iter().map(|x| x.id.clone()).collect()
}

#[test]
fn title_with_ampersand_and_percent_is_content() {
    let g = generate(&anleitung());
    assert!(g.text.contains("Material für Schritt 1 \\& 2"));
    assert!(g.text.contains("Ersatzteile (10 \\% Reserve)"));
    assert!(g.text.contains("Winkel\\_A"));
}

#[test]
fn generated_source_has_no_formavium_metadata() {
    let g = generate(&anleitung());
    assert!(!g.text.contains("materialliste"));
    assert!(!g.text.to_lowercase().contains("formavium"));
}

#[test]
fn free_latex_is_kept_verbatim_across_edits() {
    let p = anleitung();
    let ids = part_ids(&p);
    let mut ws = Workspace::new(p);
    let formula = "\\[ \\int_0^1 x^2\\,dx = \\tfrac{1}{3} \\]\n";
    let r = ws.revision();
    ws.apply(
        r,
        Command::SetLatex {
            part: ids[2].clone(),
            source: formula.into(),
        },
    )
    .unwrap();
    let r = ws.revision();
    ws.apply(
        r,
        Command::SetValue {
            part: ids[1].clone(),
            path: "titel".into(),
            value: Value::Text("Neu".into()),
        },
    )
    .unwrap();
    assert!(generate(ws.project()).text.contains(formula));
}

#[test]
fn instances_are_independent() {
    let p = anleitung();
    let ids = part_ids(&p);
    let mut ws = Workspace::new(p);
    let r = ws.revision();
    ws.apply(
        r,
        Command::SetValue {
            part: ids[3].clone(),
            path: "titel".into(),
            value: Value::Text("Zweite".into()),
        },
    )
    .unwrap();
    let p = ws.project();
    assert_eq!(
        p.effective_value(p.instance(&ids[3]).unwrap(), "titel"),
        Some(&Value::Text("Zweite".into()))
    );
    assert_eq!(
        p.effective_value(p.instance(&ids[1]).unwrap(), "titel"),
        Some(&Value::Text("Material für Schritt 1 & 2".into()))
    );
}

#[test]
fn default_value_applies_until_set() {
    let mut p = anleitung();
    let id = part_ids(&p)[3].clone();
    p.instance_mut(&id).unwrap().values.remove("titel");
    assert_eq!(
        p.effective_value(p.instance(&id).unwrap(), "titel"),
        Some(&Value::Text("Materialliste".into()))
    );
}

#[test]
fn move_column_moves_header_values_and_unit_in_one_instance() {
    let p = anleitung();
    let ids = part_ids(&p);
    let mut ws = Workspace::new(p);
    let r = ws.revision();
    ws.apply(
        r,
        Command::MoveColumn {
            part: ids[1].clone(),
            table: "tabelle".into(),
            from: 2,
            to: 0,
        },
    )
    .unwrap();
    let text = generate(ws.project()).text;
    assert!(
        text.contains("Länge [mm] & Bezeichnung & Menge \\\\"),
        "{text}"
    );
    assert!(text.contains("20 & Schraube M4 & 12~Stk. \\\\"));
    // Zweite Instanz behält ihren Aufbau.
    assert!(text.contains("Bezeichnung & Menge & Länge [mm] \\\\"));
}

#[test]
fn rejects_wrong_value_kind_and_unknown_field() {
    let p = anleitung();
    let id = part_ids(&p)[1].clone();
    let mut ws = Workspace::new(p);
    let r = ws.revision();
    let e = ws.apply(
        r,
        Command::SetValue {
            part: id.clone(),
            path: "foto".into(),
            value: Value::Text("x".into()),
        },
    );
    assert!(matches!(e, Err(EditError::WrongValueKind { .. })));
    let e = ws.apply(
        r,
        Command::SetValue {
            part: id,
            path: "gibtsnicht".into(),
            value: Value::Text("x".into()),
        },
    );
    assert!(matches!(e, Err(EditError::UnknownField(_))));
    assert_eq!(
        ws.revision(),
        r,
        "fehlgeschlagene Bearbeitung erzeugt keine Revision"
    );
}

#[test]
fn stale_base_revision_is_rejected() {
    let p = anleitung();
    let id = part_ids(&p)[0].clone();
    let mut ws = Workspace::new(p);
    let r = ws.revision();
    ws.apply(
        r,
        Command::SetLatex {
            part: id.clone(),
            source: "a".into(),
        },
    )
    .unwrap();
    let e = ws.apply(
        r,
        Command::SetLatex {
            part: id,
            source: "b".into(),
        },
    );
    assert_eq!(
        e,
        Err(EditError::StaleRevision {
            base: r,
            current: r + 1
        })
    );
}

#[test]
fn undo_redo_create_new_revisions() {
    let p = anleitung();
    let original = p.clone();
    let id = part_ids(&p)[0].clone();
    let mut ws = Workspace::new(p);
    let r1 = ws.revision();
    let (r2, _) = ws
        .apply(
            r1,
            Command::SetLatex {
                part: id,
                source: "x".into(),
            },
        )
        .unwrap();
    let r3 = ws.undo().unwrap();
    assert!(r3 > r2);
    assert_eq!(ws.project(), &original);
    let r4 = ws.redo().unwrap();
    assert!(r4 > r3);
    assert_ne!(ws.project(), &original);
}

#[test]
fn inserted_parts_get_fresh_ids_never_reused() {
    let p = anleitung();
    let module = p.modules[0].reference();
    let mut ws = Workspace::new(p);
    let r = ws.revision();
    let (r, a) = ws
        .apply(
            r,
            Command::InsertPart {
                index: 0,
                content: PartContent::Module(ModuleInstance::new(module.clone())),
            },
        )
        .unwrap();
    let a = a.unwrap();
    let (r, _) = ws
        .apply(r, Command::RemovePart { part: a.clone() })
        .unwrap();
    let (_, b) = ws
        .apply(
            r,
            Command::InsertPart {
                index: 0,
                content: PartContent::Module(ModuleInstance::new(module)),
            },
        )
        .unwrap();
    assert_ne!(a, b.unwrap());
}

#[test]
fn diagnostics_map_lines_to_fields_and_parts() {
    let p = anleitung();
    let ids = part_ids(&p);
    let g = generate(&p);
    let line_of =
        |needle: &str| g.text[..g.text.find(needle).unwrap()].matches('\n').count() as u32 + 1;
    assert_eq!(
        g.origin_at_line(line_of("\\section*{Montage")),
        Some(&Origin::Latex {
            part: ids[0].clone()
        })
    );
    assert_eq!(
        g.origin_at_line(line_of("Schraube M4 & 12")),
        Some(&Origin::Cell {
            part: ids[1].clone(),
            path: "zeilen".into(),
            row: "r1".into(),
            column: "bez".into(),
            index: 0,
        })
    );
    let title = g.text.find("Material für").unwrap();
    assert_eq!(
        g.origin_at(title),
        Some(&Origin::Field {
            part: ids[1].clone(),
            path: "titel".into()
        })
    );
    assert_eq!(g.origin_at_line(1), Some(&Origin::Preamble));
}

#[test]
fn required_fields_produce_notices_but_do_not_block() {
    let mut p = anleitung();
    let id = part_ids(&p)[1].clone();
    let inst = p.instance_mut(&id).unwrap();
    inst.values.remove("foto");
    inst.values.insert("titel".into(), Value::Text("  ".into()));
    let g = generate(&p);
    assert_eq!(g.notices.len(), 2, "{:?}", g.notices);
    assert!(g.text.contains("\\end{document}"));
}

#[test]
fn unsafe_image_path_is_not_emitted() {
    let mut p = anleitung();
    let id = part_ids(&p)[1].clone();
    p.instance_mut(&id)
        .unwrap()
        .values
        .insert("foto".into(), Value::Image("../../geheim.png".into()));
    let g = generate(&p);
    assert!(!g.text.contains("geheim"));
    assert!(g.resources.is_empty());
    assert_eq!(g.notices.len(), 1);
}

#[test]
fn save_and_reopen_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.fvm");
    let p = anleitung();
    let stamp = io::save(&path, &p, None).unwrap();
    let (loaded, stamp2) = io::load(&path).unwrap();
    assert_eq!(loaded, p);
    assert_eq!(stamp, stamp2);
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "keine temporären Dateien übrig: {names:?}");
}

#[test]
fn external_change_is_detected_before_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.fvm");
    let p = anleitung();
    let stamp = io::save(&path, &p, None).unwrap();
    let mut other = p.clone();
    other.title = "Extern geändert".into();
    io::save(&path, &other, None).unwrap();
    let e = io::save(&path, &p, Some(&stamp));
    assert!(matches!(e, Err(IoError::Conflict { .. })));
    assert_eq!(io::load(&path).unwrap().0.title, "Extern geändert");
}

#[test]
fn newer_schema_is_refused_without_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("neu.fvm");
    let mut json: serde_json::Value = serde_json::from_slice(&io::to_bytes(&anleitung())).unwrap();
    json["schema"] = (SCHEMA_VERSION + 1).into();
    let bytes = serde_json::to_vec(&json).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        io::load(&path),
        Err(IoError::UnsupportedSchema { .. })
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
}

#[test]
fn rejects_non_formavium_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.fvm");
    fs::write(&path, b"{\"schema\":1}").unwrap();
    assert!(matches!(io::load(&path), Err(IoError::Invalid { .. })));
}

#[test]
fn export_contains_sources_and_resources() {
    let src = tempfile::tempdir().unwrap();
    fs::create_dir_all(src.path().join("bilder")).unwrap();
    fs::write(src.path().join(sample::SAMPLE_IMAGE), b"png").unwrap();
    let out = tempfile::tempdir().unwrap();
    export::export_latex(&anleitung(), src.path(), out.path()).unwrap();
    assert!(out.path().join("main.tex").is_file());
    assert_eq!(
        fs::read(out.path().join(sample::SAMPLE_IMAGE)).unwrap(),
        b"png"
    );
    assert!(out.path().join("LIESMICH.txt").is_file());
}

fn wrapper(inner: ModuleRef) -> ModuleDef {
    ModuleDef {
        id: Id::from("bericht"),
        version: 1,
        name: "Bericht".into(),
        description: String::new(),
        preamble: String::new(),
        fields: vec![],
        body: vec![
            Element {
                id: Id::from("t"),
                kind: ElementKind::Paragraph {
                    text: Text::Literal("Einleitung".into()),
                },
            },
            Element {
                id: Id::from("sub"),
                kind: ElementKind::Module { module: inner },
            },
        ],
    }
}

#[test]
fn and_filter_counts_nested_content() {
    let inner = materialliste();
    let outer = wrapper(inner.reference());
    let defs = [inner.clone(), outer.clone()];
    let lookup = |r: &ModuleRef| defs.iter().find(|d| d.reference() == *r);
    let kinds = content_kinds(&outer, &lookup).unwrap();
    assert!(matches_all(
        &kinds,
        &[ContentKind::Image, ContentKind::Table]
    ));
    assert!(!matches_all(&kinds, &[ContentKind::Latex]));
}

#[test]
fn cycles_are_detected() {
    let mut a = wrapper(ModuleRef {
        id: Id::from("bericht"),
        version: 1,
    });
    a.id = Id::from("bericht");
    let defs = [a.clone()];
    let lookup = |r: &ModuleRef| defs.iter().find(|d| d.reference() == *r);
    assert!(matches!(
        content_kinds(&a, &lookup),
        Err(CatalogError::Cycle(_))
    ));
}

#[test]
fn nested_module_fields_use_paths() {
    let inner = materialliste();
    let outer = wrapper(inner.reference());
    let mut p = Project::new("T", "\\documentclass{article}");
    p.add_module(inner);
    p.add_module(outer.clone());
    let mut ws = Workspace::new(p);
    let r = ws.revision();
    let (r, id) = ws
        .apply(
            r,
            Command::InsertPart {
                index: 0,
                content: PartContent::Module(ModuleInstance::new(outer.reference())),
            },
        )
        .unwrap();
    let id = id.unwrap();
    ws.apply(
        r,
        Command::SetValue {
            part: id.clone(),
            path: "sub/titel".into(),
            value: Value::Text("Innen".into()),
        },
    )
    .unwrap();
    let r = ws.revision();
    ws.apply(
        r,
        Command::SetValue {
            part: id.clone(),
            path: "sub/zeilen".into(),
            value: Value::Table(vec![row("r1", &[("bez", "Mutter")])]),
        },
    )
    .unwrap();
    let r = ws.revision();
    ws.apply(
        r,
        Command::MoveColumn {
            part: id.clone(),
            table: "sub/tabelle".into(),
            from: 0,
            to: 1,
        },
    )
    .unwrap();
    let g = generate(ws.project());
    assert!(g.text.contains("\\section{Innen}"));
    assert!(g.text.contains("Menge & Bezeichnung & Länge [mm]"));
    let pos = g.text.find("Innen").unwrap();
    assert_eq!(
        g.origin_at(pos),
        Some(&Origin::Field {
            part: id,
            path: "sub/titel".into()
        })
    );
}
