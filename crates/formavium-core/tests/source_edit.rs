use formavium_core::sample::anleitung;
use formavium_core::source_edit::{plan_source_change, SourceChange, SourcePlan};
use formavium_core::*;

/// Ersetzt das `n`-te Vorkommen von `needle` im erzeugten Text.
fn change_at(g: &Generated, needle: &str, nth: usize, replacement: &str) -> SourceChange {
    let from = g.text.match_indices(needle).nth(nth).expect(needle).0;
    SourceChange {
        from,
        to: from + needle.len(),
        insert: replacement.into(),
    }
}

fn edited(g: &Generated, c: &SourceChange) -> String {
    format!("{}{}{}", &g.text[..c.from], c.insert, &g.text[c.to..])
}

/// Wendet eine Änderung an und liefert den neu erzeugten Text.
fn apply(ws: &mut Workspace, c: &SourceChange, allow_convert: bool) -> Result<String, String> {
    let g = generate(ws.project());
    let cmd = match plan_source_change(ws.project(), &g, c) {
        SourcePlan::Apply(cmd) => cmd,
        SourcePlan::Convert { command, .. } if allow_convert => command,
        SourcePlan::Convert { modules, .. } => return Err(format!("convert {modules:?}")),
        SourcePlan::Reject(m) => return Err(m),
    };
    let r = ws.revision();
    ws.apply(r, cmd).map_err(|e| e.to_string())?;
    Ok(generate(ws.project()).text)
}

fn ids(p: &Project) -> Vec<Id> {
    p.parts.iter().map(|x| x.id.clone()).collect()
}

#[test]
fn free_latex_edit_roundtrips_exactly() {
    let mut ws = Workspace::new(anleitung());
    let g = generate(ws.project());
    let c = change_at(&g, "beschreibt den Aufbau", 0, "erklärt die Montage");
    let expected = edited(&g, &c);
    assert_eq!(apply(&mut ws, &c, false).unwrap(), expected);
}

#[test]
fn field_edit_updates_exactly_that_value() {
    let p = anleitung();
    let id = ids(&p)[1].clone();
    let mut ws = Workspace::new(p);
    let g = generate(ws.project());
    let c = change_at(&g, "Schritt 1 \\& 2", 0, "Schritt 1 \\& 2 und 3");
    let expected = edited(&g, &c);
    assert_eq!(apply(&mut ws, &c, false).unwrap(), expected);
    let p = ws.project();
    assert_eq!(
        p.instance(&id).unwrap().values["titel"],
        Value::Text("Material für Schritt 1 & 2 und 3".into())
    );
}

#[test]
fn typed_special_characters_in_field_stay_content() {
    let p = anleitung();
    let id = ids(&p)[1].clone();
    let mut ws = Workspace::new(p);
    let g = generate(ws.project());
    let c = change_at(&g, "Schritt 1 \\& 2", 0, "50% & \\input{x}");
    let text = apply(&mut ws, &c, false).unwrap();
    assert!(
        text.contains("\\section{Material für 50\\% \\& \\textbackslash{}input\\{x\\}}"),
        "{text}"
    );
    assert_eq!(
        ws.project().instance(&id).unwrap().values["titel"],
        Value::Text("Material für 50% & \\input{x}".into())
    );
}

#[test]
fn cell_edit_changes_one_cell_only() {
    let mut ws = Workspace::new(anleitung());
    let g = generate(ws.project());
    // „Schraube M4“ kommt in beiden Instanzen vor; nur die erste ändern.
    let c = change_at(&g, "Schraube M4", 0, "Schraube M5");
    let text = apply(&mut ws, &c, false).unwrap();
    assert_eq!(text, edited(&g, &c));
    assert_eq!(text.matches("Schraube M4").count(), 1);
}

#[test]
fn typing_into_empty_cell() {
    let mut ws = Workspace::new(anleitung());
    let g = generate(ws.project());
    // Zweite Instanz: Länge ist leer: "Schraube M4 & 2~Stk. &  \\".
    let needle = "Schraube M4 & 2~Stk. & ";
    let anchor = g.text.find(needle).unwrap() + needle.len();
    let c = SourceChange {
        from: anchor,
        to: anchor,
        insert: "8".into(),
    };
    let text = apply(&mut ws, &c, false).unwrap();
    assert!(text.contains("2~Stk. & 8 \\\\"), "{text}");
}

#[test]
fn identical_modules_keep_identity() {
    let mut p = anleitung();
    let a = ids(&p)[1].clone();
    let b = ids(&p)[3].clone();
    let copy = p.instance(&a).unwrap().clone();
    *p.instance_mut(&b).unwrap() = copy;
    let mut ws = Workspace::new(p);
    let g = generate(ws.project());
    let c = change_at(&g, "Winkel\\_A", 1, "Winkel\\_B");
    apply(&mut ws, &c, false).unwrap();
    let p = ws.project();
    let cell = |id: &Id| match &p.instance(id).unwrap().values["zeilen"] {
        Value::Table(rows) => rows[1].cells[&Id::from("bez")].clone(),
        _ => unreachable!(),
    };
    assert_eq!(cell(&a), "Winkel_A");
    assert_eq!(cell(&b), "Winkel_B");
}

#[test]
fn structure_edit_requires_conversion_and_keeps_text() {
    let p = anleitung();
    let module = ids(&p)[1].clone();
    let mut ws = Workspace::new(p);
    let g = generate(ws.project());
    let c = change_at(&g, "\\hline", 0, "\\hline\\hline");
    assert_eq!(
        apply(&mut ws, &c, false),
        Err(format!("convert {:?}", vec![module.clone()]))
    );
    let text = apply(&mut ws, &c, true).unwrap();
    assert_eq!(
        text,
        edited(&g, &c),
        "eingegebener Text bleibt vollständig erhalten"
    );
    assert!(ws.project().part(&module).is_none());
    // Andere Module behalten ihre Identität.
    assert!(ws.project().instance(&ids(&anleitung())[3]).is_some());
    // Rückgängig stellt das Modul wieder her.
    ws.undo().unwrap();
    assert!(ws.project().instance(&module).is_some());
}

#[test]
fn generated_frame_is_not_editable() {
    let mut ws = Workspace::new(anleitung());
    let g = generate(ws.project());
    let c = change_at(&g, "\\begin{document}", 0, "\\begin{dokument}");
    assert!(apply(&mut ws, &c, true).unwrap_err().contains("erzeugt"));
    let c = change_at(&g, "\\usepackage{graphicx}", 0, "");
    assert!(apply(&mut ws, &c, true).is_err());
}

#[test]
fn preamble_is_editable() {
    let mut ws = Workspace::new(anleitung());
    let g = generate(ws.project());
    let c = change_at(&g, "11pt", 0, "12pt");
    assert_eq!(apply(&mut ws, &c, false).unwrap(), edited(&g, &c));
}

#[test]
fn typing_on_blank_line_after_module_creates_free_part() {
    let p = anleitung();
    let n = p.parts.len();
    let mut ws = Workspace::new(p);
    let g = generate(ws.project());
    let module_range = g.part_ranges[1].clone();
    let pos = module_range.end - 1; // Leerzeile hinter dem Modul
    let c = SourceChange {
        from: pos,
        to: pos,
        insert: "Zwischentext".into(),
    };
    let text = apply(&mut ws, &c, false).unwrap();
    assert_eq!(ws.project().parts.len(), n + 1);
    assert!(
        matches!(&ws.project().parts[2].content, PartContent::Latex { source } if source == "Zwischentext")
    );
    assert!(
        text.contains("\\par\n\nZwischentext\n\\subsection*{Hinweis}"),
        "{text}"
    );
}

#[test]
fn deletion_across_parts_converts_affected_parts_only() {
    let p = anleitung();
    let first = ids(&p)[0].clone();
    let module = ids(&p)[1].clone();
    let second = ids(&p)[3].clone();
    let mut ws = Workspace::new(p);
    let g = generate(ws.project());
    let from = g.text.find("Für die Last").unwrap();
    let to = g.text.find("\\begin{tabular}").unwrap();
    let c = SourceChange {
        from,
        to,
        insert: String::new(),
    };
    let text = apply(&mut ws, &c, true).unwrap();
    assert_eq!(text, edited(&g, &c));
    let p = ws.project();
    assert!(p.part(&first).is_none() && p.part(&module).is_none());
    assert!(p.instance(&second).is_some());
}
