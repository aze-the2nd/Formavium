//! Tabellenbearbeitung über Kennungen: Zellen, Zeilen und lokale Spalten.

use formavium_core::sample::anleitung;
use formavium_core::*;

fn workspace() -> (Workspace, Id, Id) {
    let p = anleitung();
    let first = p.parts[1].id.clone();
    let second = p.parts[3].id.clone();
    (Workspace::new(p), first, second)
}

fn apply(ws: &mut Workspace, cmd: Command) -> Option<Id> {
    let r = ws.revision();
    ws.apply(r, cmd).unwrap().1
}

fn rows(ws: &Workspace, part: &Id) -> Vec<Row> {
    let p = ws.project();
    match p.effective_value(p.instance(part).unwrap(), "zeilen") {
        Some(Value::Table(r)) => r.clone(),
        other => panic!("keine Tabelle: {other:?}"),
    }
}

#[test]
fn cell_and_row_commands_work_on_ids() {
    let (mut ws, first, _) = workspace();
    apply(
        &mut ws,
        Command::SetCell {
            part: first.clone(),
            path: "zeilen".into(),
            row: Id::from("r2"),
            column: Id::from("menge"),
            value: "8".into(),
        },
    );
    let new = apply(
        &mut ws,
        Command::InsertRow {
            part: first.clone(),
            path: "zeilen".into(),
            index: 1,
        },
    )
    .unwrap();
    let ids: Vec<Id> = rows(&ws, &first).into_iter().map(|r| r.id).collect();
    assert_eq!(ids, [Id::from("r1"), new.clone(), Id::from("r2")]);
    assert_eq!(rows(&ws, &first)[2].cells[&Id::from("menge")], "8");

    apply(
        &mut ws,
        Command::MoveRow {
            part: first.clone(),
            path: "zeilen".into(),
            row: Id::from("r2"),
            to: 0,
        },
    );
    apply(
        &mut ws,
        Command::RemoveRow {
            part: first.clone(),
            path: "zeilen".into(),
            row: new,
        },
    );
    let ids: Vec<Id> = rows(&ws, &first).into_iter().map(|r| r.id).collect();
    assert_eq!(ids, [Id::from("r2"), Id::from("r1")]);
    let text = generate(ws.project()).text;
    assert!(text.contains("Winkel\\_A & 8~Stk. & 12,5 \\\\\n\\hline\nSchraube M4"));
}

#[test]
fn new_row_ids_do_not_collide_with_existing_rows() {
    let (mut ws, first, _) = workspace();
    for _ in 0..12 {
        apply(
            &mut ws,
            Command::InsertRow {
                part: first.clone(),
                path: "zeilen".into(),
                index: 0,
            },
        );
    }
    let mut ids: Vec<Id> = rows(&ws, &first).into_iter().map(|r| r.id).collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
}

#[test]
fn row_commands_reject_unknown_targets() {
    let (mut ws, first, _) = workspace();
    let r = ws.revision();
    assert_eq!(
        ws.apply(
            r,
            Command::RemoveRow {
                part: first.clone(),
                path: "zeilen".into(),
                row: Id::from("fehlt"),
            }
        ),
        Err(EditError::UnknownRow(Id::from("fehlt")))
    );
    assert!(matches!(
        ws.apply(
            r,
            Command::InsertRow {
                part: first.clone(),
                path: "titel".into(),
                index: 0,
            }
        ),
        Err(EditError::WrongValueKind { .. })
    ));
    assert_eq!(
        ws.apply(
            r,
            Command::InsertRow {
                part: first,
                path: "zeilen".into(),
                index: 9,
            }
        ),
        Err(EditError::OutOfRange(9))
    );
}

#[test]
fn local_column_exists_only_in_one_instance() {
    let (mut ws, first, _) = workspace();
    let col = apply(
        &mut ws,
        Command::InsertColumn {
            part: first.clone(),
            table: "tabelle".into(),
            index: 1,
            header: "Norm & Güte".into(),
        },
    )
    .unwrap();
    apply(
        &mut ws,
        Command::SetCell {
            part: first.clone(),
            path: "zeilen".into(),
            row: Id::from("r1"),
            column: col.clone(),
            value: "DIN 912".into(),
        },
    );
    let text = generate(ws.project()).text;
    assert!(
        text.contains("Bezeichnung & Norm \\& Güte & Menge & Länge [mm] \\\\"),
        "{text}"
    );
    assert!(text.contains("Schraube M4 & DIN 912 & 12~Stk. & 20 \\\\"));
    assert!(text.contains("\\begin{tabular}{|l|l|r|r|}"));
    // Zweite Instanz und Moduldefinition bleiben unverändert.
    assert!(text.contains("Bezeichnung & Menge & Länge [mm] \\\\"));
    assert!(text.contains("\\begin{tabular}{|l|r|r|}"));
    let def = &ws.project().modules[0];
    assert_eq!(def, &formavium_core::sample::materialliste());

    // Verschieben wirkt auch auf die lokale Spalte.
    apply(
        &mut ws,
        Command::MoveColumn {
            part: first.clone(),
            table: "tabelle".into(),
            from: 1,
            to: 3,
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains("Bezeichnung & Menge & Länge [mm] & Norm \\& Güte \\\\"));

    // Eine weitere Spalte landet an der sichtbaren Position.
    apply(
        &mut ws,
        Command::InsertColumn {
            part: first,
            table: "tabelle".into(),
            index: 0,
            header: "Pos.".into(),
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains("Pos. & Bezeichnung & Menge & Länge [mm] & Norm \\& Güte \\\\"));
}

#[test]
fn removing_a_column_drops_its_values_and_keeps_the_last_one() {
    let (mut ws, first, _) = workspace();
    apply(
        &mut ws,
        Command::RemoveColumn {
            part: first.clone(),
            table: "tabelle".into(),
            column: Id::from("menge"),
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains("Schraube M4 & 20 \\\\"), "{text}");
    assert!(rows(&ws, &first)
        .iter()
        .all(|r| !r.cells.contains_key(&Id::from("menge"))));

    apply(
        &mut ws,
        Command::RemoveColumn {
            part: first.clone(),
            table: "tabelle".into(),
            column: Id::from("laenge"),
        },
    );
    let r = ws.revision();
    assert_eq!(
        ws.apply(
            r,
            Command::RemoveColumn {
                part: first.clone(),
                table: "tabelle".into(),
                column: Id::from("bez"),
            }
        ),
        Err(EditError::LastColumn)
    );
    // Rückgängig stellt Spalte und Werte wieder her.
    ws.undo().unwrap();
    ws.undo().unwrap();
    assert!(generate(ws.project())
        .text
        .contains("Schraube M4 & 12~Stk. & 20 \\\\"));
}

#[test]
fn column_header_can_be_renamed_locally() {
    let (mut ws, first, _) = workspace();
    apply(
        &mut ws,
        Command::SetColumnHeader {
            part: first,
            table: "tabelle".into(),
            column: Id::from("laenge"),
            header: "Schaftlänge".into(),
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains("Bezeichnung & Menge & Schaftlänge [mm] \\\\"));
    assert!(text.contains("Bezeichnung & Menge & Länge [mm] \\\\"));
}

#[test]
fn local_columns_survive_save_and_load() {
    let (mut ws, first, _) = workspace();
    apply(
        &mut ws,
        Command::InsertColumn {
            part: first,
            table: "tabelle".into(),
            index: 3,
            header: "Hinweis".into(),
        },
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.fvm");
    formavium_core::io::save(&path, ws.project(), None).unwrap();
    let (loaded, _) = formavium_core::io::load(&path).unwrap();
    assert_eq!(&loaded, ws.project());
    // Dokumente ohne lokale Spalten enthalten das Feld nicht.
    let plain = serde_json::to_string(&anleitung()).unwrap();
    assert!(!plain.contains("\"columns\":{"));
}

#[test]
fn pasted_block_fills_cells_adds_rows_and_drops_overflow() {
    let (mut ws, first, _) = workspace();
    // Spalten in Anzeigereihenfolge umstellen: Länge, Bezeichnung, Menge.
    apply(
        &mut ws,
        Command::MoveColumn {
            part: first.clone(),
            table: "tabelle".into(),
            from: 2,
            to: 0,
        },
    );
    // Block ab Zeile 2, Spalte 2 (Bezeichnung): zwei Spalten breit passt,
    // die dritte Spalte des Blocks liegt jenseits der Tabelle.
    apply(
        &mut ws,
        Command::PasteCells {
            part: first.clone(),
            table: "tabelle".into(),
            row: 1,
            column: 1,
            cells: vec![
                vec!["Winkel B".into(), "6".into(), "zu viel".into()],
                vec!["Mutter M4".into(), "12".into()],
                vec!["Scheibe".into()],
            ],
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains("20 & Schraube M4 & 12~Stk. \\\\"), "{text}");
    assert!(text.contains("12,5 & Winkel B & 6~Stk. \\\\"));
    assert!(text.contains(" & Mutter M4 & 12~Stk. \\\\"));
    assert!(text.contains(" & Scheibe &  \\\\"));
    assert!(!text.contains("zu viel"));
    assert_eq!(rows(&ws, &first).len(), 4);

    // Ein Schritt im Verlauf.
    ws.undo().unwrap();
    assert_eq!(rows(&ws, &first).len(), 2);

    let r = ws.revision();
    assert_eq!(
        ws.apply(
            r,
            Command::PasteCells {
                part: first,
                table: "tabelle".into(),
                row: 9,
                column: 0,
                cells: vec![vec!["x".into()]],
            }
        ),
        Err(EditError::OutOfRange(9))
    );
}

#[test]
fn merged_cells_render_as_multirow_and_survive_row_edits() {
    let (mut ws, first, _) = workspace();
    // Dritte Zeile anlegen, dann „Menge“ über Zeile 1 und 2 verbinden.
    apply(
        &mut ws,
        Command::InsertRow {
            part: first.clone(),
            path: "zeilen".into(),
            index: 2,
        },
    );
    apply(
        &mut ws,
        Command::MergeCells {
            part: first.clone(),
            table: "tabelle".into(),
            rows: vec![Id::from("r1"), Id::from("r2")],
            columns: vec![Id::from("menge")],
            merge: true,
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains(r"\usepackage{multirow}"), "{text}");
    assert!(
        text.contains(
            "Schraube M4 & \\multirow{2}{*}{12~Stk.} & 20 \\\\\n\\cline{1-1}\n\\cline{3-3}\nWinkel\\_A &  & 12,5 \\\\\n\\hline"
        ),
        "{text}"
    );
    // Der Wert der unteren Zelle ist geleert, Einfügen überspringt sie.
    assert!(!rows(&ws, &first)[1].cells.contains_key(&Id::from("menge")));
    apply(
        &mut ws,
        Command::PasteCells {
            part: first.clone(),
            table: "tabelle".into(),
            row: 1,
            column: 1,
            cells: vec![vec!["99".into()]],
        },
    );
    assert!(!rows(&ws, &first)[1].cells.contains_key(&Id::from("menge")));

    // Zeile innerhalb des Blocks einfügen: Sie gehört zum Block.
    apply(
        &mut ws,
        Command::InsertRow {
            part: first.clone(),
            path: "zeilen".into(),
            index: 1,
        },
    );
    assert!(generate(ws.project())
        .text
        .contains(r"\multirow{3}{*}{12~Stk.}"));

    // Oberste Zeile entfernen: Die nächste übernimmt Inhalt und Block.
    apply(
        &mut ws,
        Command::RemoveRow {
            part: first.clone(),
            path: "zeilen".into(),
            row: Id::from("r1"),
        },
    );
    let text = generate(ws.project()).text;
    assert!(text.contains(r"\multirow{2}{*}{12~Stk.}"), "{text}");
    assert!(rows(&ws, &first)[0].merged.is_empty());

    // Trennen hebt den ganzen Block auf.
    let top = rows(&ws, &first)[0].id.clone();
    apply(
        &mut ws,
        Command::MergeCells {
            part: first.clone(),
            table: "tabelle".into(),
            rows: vec![top],
            columns: vec![Id::from("menge")],
            merge: false,
        },
    );
    let text = generate(ws.project()).text;
    assert!(!text.contains("multirow"), "{text}");
}

#[test]
fn latex_columns_are_not_escaped_and_take_source_edits_verbatim() {
    let (mut ws, first, _) = workspace();
    apply(
        &mut ws,
        Command::SetColumnStyle {
            part: first.clone(),
            table: "tabelle".into(),
            column: Id::from("bez"),
            align: Some(Align::Center),
            latex: Some(true),
        },
    );
    apply(
        &mut ws,
        Command::SetCell {
            part: first.clone(),
            path: "zeilen".into(),
            row: Id::from("r1"),
            column: Id::from("bez"),
            value: "$U_E$ und mehr".into(),
        },
    );
    let g = generate(ws.project());
    assert!(g.text.contains("{|c|r|r|}"), "{}", g.text);
    assert!(g.text.contains("$U_E$ und mehr & 12~Stk."));
    // Tippen im Quelltext: Text der Zelle unverändert übernehmen.
    let at = g.text.find("$U_E$").unwrap();
    let plan = formavium_core::source_edit::plan_source_change(
        ws.project(),
        &g,
        &formavium_core::source_edit::SourceChange {
            from: at + 5,
            to: at + 5,
            insert: r"\,V".into(),
        },
    );
    let formavium_core::source_edit::SourcePlan::Apply(cmd) = plan else {
        panic!("{plan:?}");
    };
    apply(&mut ws, cmd);
    assert_eq!(
        rows(&ws, &first)[0].cells[&Id::from("bez")],
        r"$U_E$\,V und mehr"
    );
}

#[test]
fn module_preamble_is_emitted_once_before_the_document() {
    let mut p = Project::new("T", r"\documentclass{article}");
    for id in ["a", "b"] {
        p.add_module(ModuleDef {
            id: Id::from(id),
            version: 1,
            name: id.into(),
            description: String::new(),
            preamble: r"\providecommand{\jsMarke}{X}".into(),
            fields: vec![],
            body: vec![Element {
                id: Id::from("x"),
                kind: ElementKind::Latex {
                    source: r"\jsMarke".into(),
                },
            }],
        });
    }
    let text = generate(&p).text;
    assert_eq!(text.matches(r"\providecommand{\jsMarke}{X}").count(), 1);
    assert!(text.find("jsMarke").unwrap() < text.find(r"\begin{document}").unwrap());
}
