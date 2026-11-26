//! Mitgelieferte, neutrale Bibliothek: Kleinstbausteine, Module und Vorlagen.
//!
//! Die Bausteine sind bewusst allgemein gehalten (Überschrift, Text, Liste,
//! Tabelle, Bild, Titelblock …). Firmenspezifische Module und Vorlagen
//! gehören in den Bibliotheksordner des Anwenders.

use std::collections::BTreeMap;

use crate::model::{
    Align, Column, Element, ElementKind, FieldDef, FieldKind, Id, ModuleDef, ModuleInstance, Part,
    PartContent, Project, Row, Text, UnitPlacement, Value,
};
use crate::sample;

/// Vorspann der mitgelieferten Vorlagen.
pub const PREAMBLE: &str = "\\documentclass[a4paper,11pt]{article}\n\\usepackage{fontspec}\n\\usepackage[ngerman]{babel}\n\\usepackage[margin=2.5cm]{geometry}\n\\setlength{\\parindent}{0pt}\n\\setlength{\\parskip}{0.5\\baselineskip}";

// --- Kurzschreibweisen ---

fn text(id: &str, label: &str, default: &str) -> FieldDef {
    FieldDef {
        id: Id::from(id),
        label: label.into(),
        kind: FieldKind::Text,
        default: Some(Value::Text(default.into())),
    }
}

fn image(id: &str, label: &str) -> FieldDef {
    FieldDef {
        id: Id::from(id),
        label: label.into(),
        kind: FieldKind::Image,
        default: None,
    }
}

fn rows(id: &str, label: &str, default: Vec<Row>) -> FieldDef {
    FieldDef {
        id: Id::from(id),
        label: label.into(),
        kind: FieldKind::Table,
        default: Some(Value::Table(default)),
    }
}

/// Zeilen mit Kennungen `z1`, `z2` … und Zellwerten je Spalte.
fn data(lines: &[&[(&str, &str)]]) -> Vec<Row> {
    lines
        .iter()
        .enumerate()
        .map(|(i, cells)| Row {
            id: Id(format!("z{}", i + 1)),
            cells: cells
                .iter()
                .map(|(k, v)| (Id::from(*k), (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            merged: Default::default(),
        })
        .collect()
}

fn el(id: &str, kind: ElementKind) -> Element {
    Element {
        id: Id::from(id),
        kind,
    }
}

fn heading(id: &str, level: u8, field: &str) -> Element {
    el(
        id,
        ElementKind::Heading {
            level,
            text: Text::Field(Id::from(field)),
        },
    )
}

fn paragraph(id: &str, field: &str) -> Element {
    el(
        id,
        ElementKind::Paragraph {
            text: Text::Field(Id::from(field)),
        },
    )
}

fn col(id: &str, header: &str, align: Align) -> Column {
    Column {
        id: Id::from(id),
        header: Text::Literal(header.into()),
        unit: None,
        unit_placement: UnitPlacement::Header,
        align,
        width: None,
        latex: false,
    }
}

/// Spalte mit fester Breite (Anteil der Zeilenbreite) und Zeilenumbruch.
fn wide(id: &str, header: &str, width: f32) -> Column {
    Column {
        width: Some(width),
        ..col(id, header, Align::Left)
    }
}

fn table(id: &str, rows: &str, columns: Vec<Column>) -> Element {
    el(
        id,
        ElementKind::Table {
            columns,
            rows: Id::from(rows),
            caption: None,
        },
    )
}

fn list(id: &str, items: &str, ordered: bool) -> Element {
    el(
        id,
        ElementKind::List {
            items: Id::from(items),
            ordered,
        },
    )
}

fn latex(id: &str, source: &str) -> Element {
    el(
        id,
        ElementKind::Latex {
            source: source.into(),
        },
    )
}

fn module(
    id: &str,
    name: &str,
    description: &str,
    fields: Vec<FieldDef>,
    body: Vec<Element>,
) -> ModuleDef {
    ModuleDef {
        id: Id::from(id),
        version: 1,
        name: name.into(),
        description: description.into(),
        preamble: String::new(),
        fields,
        body,
    }
}

const PICTURE: &str = "\\begin{center}\n\\bildfeld[width=0.8\\linewidth]{bild}\\par\\smallskip\n{\\small \\feld{unterschrift}}\n\\end{center}";

/// Alle mitgelieferten Module.
pub fn modules() -> Vec<ModuleDef> {
    vec![
        // --- Kleinstbausteine ---
        module(
            "ueberschrift",
            "Überschrift",
            "Nummerierte Abschnittsüberschrift.",
            vec![text("titel", "Titel", "Überschrift")],
            vec![heading("kopf", 1, "titel")],
        ),
        module(
            "zwischenueberschrift",
            "Zwischenüberschrift",
            "Überschrift der zweiten Ebene.",
            vec![text("titel", "Titel", "Zwischenüberschrift")],
            vec![heading("kopf", 2, "titel")],
        ),
        module(
            "text",
            "Textabsatz",
            "Fließtext. Leerzeilen trennen Absätze.",
            vec![text("text", "Text", "Text")],
            vec![paragraph("absatz", "text")],
        ),
        module(
            "abschnitt",
            "Abschnitt",
            "Überschrift mit Fließtext.",
            vec![
                text("titel", "Titel", "Abschnitt"),
                text("text", "Text", "Text"),
            ],
            vec![heading("kopf", 1, "titel"), paragraph("absatz", "text")],
        ),
        module(
            "aufzaehlung",
            "Aufzählung",
            "Liste mit Aufzählungspunkten.",
            vec![rows(
                "punkte",
                "Punkte",
                data(&[&[("text", "Erster Punkt")], &[("text", "Zweiter Punkt")]]),
            )],
            vec![list("liste", "punkte", false)],
        ),
        module(
            "nummerierte-liste",
            "Nummerierte Liste",
            "Schritte in fester Reihenfolge.",
            vec![rows(
                "punkte",
                "Schritte",
                data(&[&[("text", "Erster Schritt")], &[("text", "Zweiter Schritt")]]),
            )],
            vec![list("liste", "punkte", true)],
        ),
        module(
            "hinweis",
            "Hinweiskasten",
            "Hervorgehobener Hinweis im Rahmen.",
            vec![
                text("titel", "Titel", "Hinweis"),
                text("text", "Text", "Text des Hinweises."),
            ],
            vec![latex(
                "kasten",
                "\\par\\smallskip\\noindent\\fbox{\\parbox{\\dimexpr\\linewidth-2\\fboxsep-2\\fboxrule\\relax}{\\textbf{\\feld{titel}}\\par\\smallskip \\feld{text}}}\\par\\smallskip",
            )],
        ),
        module(
            "bild",
            "Bild mit Unterschrift",
            "Zentriertes Bild mit Bildunterschrift.",
            vec![
                image("bild", "Bild"),
                text("unterschrift", "Bildunterschrift", "Bildunterschrift"),
            ],
            vec![latex("abbildung", PICTURE)],
        ),
        module(
            "bilder-nebeneinander",
            "Zwei Bilder nebeneinander",
            "Zwei Bilder mit je einer Unterschrift.",
            vec![
                image("links", "Bild links"),
                text("text-links", "Unterschrift links", "Links"),
                image("rechts", "Bild rechts"),
                text("text-rechts", "Unterschrift rechts", "Rechts"),
            ],
            vec![el(
                "paar",
                ElementKind::Row {
                    children: vec![
                        latex(
                            "bild-links",
                            "\\centering\\bildfeld[width=\\linewidth]{links}\\par\\smallskip{\\small \\feld{text-links}}",
                        ),
                        latex(
                            "bild-rechts",
                            "\\centering\\bildfeld[width=\\linewidth]{rechts}\\par\\smallskip{\\small \\feld{text-rechts}}",
                        ),
                    ],
                },
            )],
        ),
        module(
            "formel",
            "Formel",
            "Abgesetzte Formel in LaTeX-Schreibweise.",
            vec![text("formel", "Formel (LaTeX)", "U = R \\cdot I")],
            vec![latex("gleichung", "\\[ \\rohfeld{formel} \\]")],
        ),
        module(
            "seitenumbruch",
            "Seitenumbruch",
            "Beginnt eine neue Seite.",
            vec![],
            vec![latex("umbruch", "\\newpage")],
        ),
        // --- Tabellen ---
        module(
            "tabelle",
            "Tabelle",
            "Freie Tabelle. Spalten lassen sich im Dokument ergänzen, umbenennen und entfernen.",
            vec![rows(
                "zeilen",
                "Tabellenwerte",
                data(&[&[("a", ""), ("b", ""), ("c", "")]]),
            )],
            vec![table(
                "tabelle",
                "zeilen",
                vec![
                    col("a", "Spalte 1", Align::Left),
                    col("b", "Spalte 2", Align::Left),
                    col("c", "Spalte 3", Align::Left),
                ],
            )],
        ),
        module(
            "kenndaten",
            "Kenndaten",
            "Eigenschaften mit Wert und Einheit.",
            vec![
                text("titel", "Titel", "Kenndaten"),
                rows(
                    "zeilen",
                    "Kenndaten",
                    data(&[&[("eigenschaft", ""), ("wert", ""), ("einheit", "")]]),
                ),
            ],
            vec![
                heading("kopf", 2, "titel"),
                table(
                    "tabelle",
                    "zeilen",
                    vec![
                        col("eigenschaft", "Eigenschaft", Align::Left),
                        col("wert", "Wert", Align::Right),
                        col("einheit", "Einheit", Align::Left),
                    ],
                ),
            ],
        ),
        module(
            "messwerte",
            "Messwerttabelle",
            "Messgrößen mit Sollwert, Istwert und Bewertung.",
            vec![
                text("titel", "Titel", "Messwerte"),
                rows(
                    "zeilen",
                    "Messwerte",
                    data(&[&[
                        ("groesse", ""),
                        ("bedingung", ""),
                        ("soll", ""),
                        ("ist", ""),
                        ("einheit", ""),
                        ("bewertung", ""),
                    ]]),
                ),
            ],
            vec![
                heading("kopf", 2, "titel"),
                table(
                    "tabelle",
                    "zeilen",
                    vec![
                        col("groesse", "Messgröße", Align::Left),
                        col("bedingung", "Bedingung", Align::Left),
                        col("soll", "Soll", Align::Right),
                        col("ist", "Ist", Align::Right),
                        col("einheit", "Einheit", Align::Left),
                        col("bewertung", "Bewertung", Align::Left),
                    ],
                ),
            ],
        ),
        module(
            "stueckliste",
            "Stückliste",
            "Positionen mit Bezeichnung, Menge und Hinweis.",
            vec![
                text("titel", "Titel", "Stückliste"),
                rows(
                    "zeilen",
                    "Positionen",
                    data(&[&[("pos", "1"), ("bez", ""), ("menge", ""), ("hinweis", "")]]),
                ),
            ],
            vec![
                heading("kopf", 2, "titel"),
                table(
                    "tabelle",
                    "zeilen",
                    vec![
                        col("pos", "Pos.", Align::Right),
                        col("bez", "Bezeichnung", Align::Left),
                        Column {
                            unit: Some("Stk.".into()),
                            unit_placement: UnitPlacement::Value,
                            ..col("menge", "Menge", Align::Right)
                        },
                        wide("hinweis", "Hinweis", 0.35),
                    ],
                ),
            ],
        ),
        module(
            "aenderungshistorie",
            "Änderungshistorie",
            "Versionen des Dokuments mit Datum, Änderung und Bearbeiter.",
            vec![
                text("titel", "Titel", "Änderungshistorie"),
                rows(
                    "zeilen",
                    "Änderungen",
                    data(&[&[
                        ("version", "1.0"),
                        ("datum", ""),
                        ("aenderung", "Erstausgabe"),
                        ("name", ""),
                    ]]),
                ),
            ],
            vec![
                heading("kopf", 1, "titel"),
                table(
                    "tabelle",
                    "zeilen",
                    vec![
                        col("version", "Version", Align::Left),
                        col("datum", "Datum", Align::Left),
                        wide("aenderung", "Änderung", 0.5),
                        col("name", "Name", Align::Left),
                    ],
                ),
            ],
        ),
        // --- Dokumentrahmen ---
        module(
            "titelblock",
            "Titelblock",
            "Dokumenttitel mit Untertitel, Nummer, Datum und Ersteller.",
            vec![
                text("titel", "Titel", "Dokumenttitel"),
                text("untertitel", "Untertitel", "Untertitel"),
                text("nummer", "Dokumentnummer", ""),
                text("datum", "Datum", ""),
                text("ersteller", "Ersteller", ""),
            ],
            vec![latex(
                "block",
                "{\\LARGE\\bfseries \\feld{titel}\\par}\n\\smallskip\n{\\large \\feld{untertitel}\\par}\n\\medskip\n\\begin{tabular}{@{}ll}\nDokument-Nr.: & \\feld{nummer} \\\\\nDatum: & \\feld{datum} \\\\\nErsteller: & \\feld{ersteller} \\\\\n\\end{tabular}\n\\par\\medskip\\hrule\\medskip",
            )],
        ),
        module(
            "ergebnis",
            "Ergebnis",
            "Zusammenfassung mit hervorgehobenem Gesamtergebnis.",
            vec![
                text("titel", "Titel", "Ergebnis"),
                text("text", "Zusammenfassung", "Zusammenfassung der Ergebnisse."),
                text("urteil", "Gesamtergebnis", "bestanden"),
            ],
            vec![
                heading("kopf", 1, "titel"),
                paragraph("absatz", "text"),
                latex("urteil", "\\textbf{Gesamtergebnis: \\feld{urteil}}\\par"),
            ],
        ),
        module(
            "unterschriften",
            "Unterschriftenfeld",
            "Erstellt, geprüft und freigegeben mit Name und Datum.",
            vec![
                text("erstellt", "Erstellt von", ""),
                text("geprueft", "Geprüft von", ""),
                text("freigegeben", "Freigegeben von", ""),
                text("datum", "Datum", ""),
            ],
            vec![latex(
                "felder",
                "\\par\\bigskip\\noindent\n\\begin{tabular}{@{}p{0.3\\linewidth}p{0.3\\linewidth}p{0.3\\linewidth}@{}}\nErstellt & Geprüft & Freigegeben \\\\[1.6cm]\n\\rule{\\linewidth}{0.4pt} & \\rule{\\linewidth}{0.4pt} & \\rule{\\linewidth}{0.4pt} \\\\\n\\feld{erstellt} & \\feld{geprueft} & \\feld{freigegeben} \\\\\n\\feld{datum} & \\feld{datum} & \\feld{datum} \\\\\n\\end{tabular}\\par",
            )],
        ),
        sample::materialliste(),
    ]
}

fn find(id: &str) -> ModuleDef {
    modules()
        .into_iter()
        .find(|m| m.id.as_str() == id)
        .unwrap_or_else(|| panic!("mitgeliefertes Modul {id} fehlt"))
}

/// Dokument aus mitgelieferten Modulen, je Modul mit optionalen Werten.
fn compose(title: &str, description: &str, parts: &[(&str, &[(&str, &str)])]) -> Project {
    let mut p = Project::new(title, PREAMBLE);
    p.description = description.into();
    for (module, values) in parts {
        let def = find(module);
        let mut inst = ModuleInstance::new(def.reference());
        for (field, value) in *values {
            inst.values
                .insert((*field).into(), Value::Text((*value).into()));
        }
        p.add_module(def);
        let id = p.fresh_id("p");
        p.parts.push(Part {
            id,
            content: PartContent::Module(inst),
        });
    }
    p
}

/// Mitgelieferte Vorlagen mit ihrer Kennung.
pub fn templates() -> Vec<(&'static str, Project)> {
    let mut sample = sample::anleitung();
    sample.description =
        "Übungsdokument: zwei Materiallisten mit Bild, dazwischen freies LaTeX.".into();
    vec![
        (
            "leer",
            compose(
                "Neues Dokument",
                "Leeres Dokument mit einer Überschrift und einem Textabsatz.",
                &[("abschnitt", &[("titel", "Überschrift")])],
            ),
        ),
        (
            "bericht",
            compose(
                "Bericht",
                "Titelblock, Änderungshistorie, Abschnitte, Messwerte, Ergebnis und Unterschriften.",
                &[
                    ("titelblock", &[("titel", "Bericht"), ("untertitel", "Gegenstand des Berichts")]),
                    ("aenderungshistorie", &[]),
                    ("abschnitt", &[("titel", "Aufgabenstellung")]),
                    ("abschnitt", &[("titel", "Durchführung")]),
                    ("messwerte", &[]),
                    ("bild", &[]),
                    ("ergebnis", &[]),
                    ("unterschriften", &[]),
                ],
            ),
        ),
        (
            "datenblatt",
            compose(
                "Datenblatt",
                "Titelblock, Beschreibung, Kenndaten, Bild und Hinweise.",
                &[
                    ("titelblock", &[("titel", "Datenblatt"), ("untertitel", "Bezeichnung des Bauteils")]),
                    ("abschnitt", &[("titel", "Beschreibung")]),
                    ("kenndaten", &[]),
                    ("bild", &[]),
                    ("hinweis", &[]),
                    ("aenderungshistorie", &[]),
                ],
            ),
        ),
        ("montageanleitung", sample),
        ("freies-latex", free_latex()),
    ]
}

/// Nerd-Modus: ein leeres, vollständiges LaTeX-Dokument ohne Module.
fn free_latex() -> Project {
    let mut p = Project::new(
        "Freies LaTeX",
        "\\documentclass[a4paper,11pt]{article}\n\\usepackage{fontspec}\n\\usepackage[ngerman]{babel}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{graphicx}\n\\usepackage{amsmath}\n\n\\begin{document}\n\n\\section{Einleitung}\nHier steht dein Text. Formeln wie $U = R \\cdot I$ funktionieren direkt.\n\n\\end{document}\n",
    );
    p.free = true;
    p.description =
        "Nerd-Modus: das ganze Dokument als LaTeX-Quelltext, ohne Module und ohne Schutz.".into();
    p
}

/// Bilder der mitgelieferten Vorlagen.
pub fn resource(rel: &str) -> Option<&'static [u8]> {
    (rel == sample::SAMPLE_IMAGE).then_some(sample::SAMPLE_PNG)
}
