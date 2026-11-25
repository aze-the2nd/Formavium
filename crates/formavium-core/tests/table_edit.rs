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
    assert!(text.contains("Winkel\\_A & 8~Stk. & 12,5 \\\\\nSchraube M4"));
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
    assert!(text.contains("\\begin{tabular}{llrr}"));
    // Zweite Instanz und Moduldefinition bleiben unverändert.
    assert!(text.contains("Bezeichnung & Menge & Länge [mm] \\\\"));
    assert!(text.contains("\\begin{tabular}{lrr}"));
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
