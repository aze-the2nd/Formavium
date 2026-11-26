//! Referenzbeispiel: Modul „Materialliste“ in einer Anleitung.

use std::collections::BTreeMap;

use crate::model::{
    Align, Column, Element, ElementKind, FieldDef, FieldKind, Id, ModuleDef, ModuleInstance, Part,
    PartContent, Project, Row, Text, UnitPlacement, Value,
};

pub const SAMPLE_IMAGE: &str = "bilder/beispiel.png";
/// Beispielbild für neu angelegte Übungsdokumente.
pub const SAMPLE_PNG: &[u8] = include_bytes!("../../formavium-cli/assets/beispiel.png");

pub fn materialliste() -> ModuleDef {
    let col = |id: &str, header: &str, unit: Option<&str>, placement, align| Column {
        id: Id::from(id),
        header: Text::Literal(header.into()),
        unit: unit.map(Into::into),
        unit_placement: placement,
        align,
        width: None,
        latex: false,
    };
    ModuleDef {
        id: Id::from("materialliste"),
        version: 1,
        name: "Materialliste".into(),
        description: "Überschrift, Tabelle mit Menge und Länge, daneben ein Bild.".into(),
        preamble: String::new(),
        fields: vec![
            FieldDef {
                id: Id::from("titel"),
                label: "Titel".into(),
                kind: FieldKind::Text,
                default: Some(Value::Text("Materialliste".into())),
            },
            FieldDef {
                id: Id::from("zeilen"),
                label: "Tabellenwerte".into(),
                kind: FieldKind::Table,
                default: None,
            },
            FieldDef {
                id: Id::from("foto"),
                label: "Foto".into(),
                kind: FieldKind::Image,
                default: None,
            },
        ],
        body: vec![
            Element {
                id: Id::from("kopf"),
                kind: ElementKind::Heading {
                    level: 1,
                    text: Text::Field(Id::from("titel")),
                },
            },
            Element {
                id: Id::from("anordnung"),
                kind: ElementKind::Row {
                    children: vec![
                        Element {
                            id: Id::from("tabelle"),
                            kind: ElementKind::Table {
                                columns: vec![
                                    col(
                                        "bez",
                                        "Bezeichnung",
                                        None,
                                        UnitPlacement::Header,
                                        Align::Left,
                                    ),
                                    col(
                                        "menge",
                                        "Menge",
                                        Some("Stk."),
                                        UnitPlacement::Value,
                                        Align::Right,
                                    ),
                                    col(
                                        "laenge",
                                        "Länge",
                                        Some("mm"),
                                        UnitPlacement::Header,
                                        Align::Right,
                                    ),
                                ],
                                rows: Id::from("zeilen"),
                                caption: None,
                                latex: None,
                            },
                        },
                        Element {
                            id: Id::from("bild"),
                            kind: ElementKind::Image {
                                path: Id::from("foto"),
                                width: 1.0,
                            },
                        },
                    ],
                },
            },
        ],
    }
}

pub fn row(id: &str, cells: &[(&str, &str)]) -> Row {
    Row {
        id: Id::from(id),
        cells: cells
            .iter()
            .map(|(k, v)| (Id::from(*k), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        merged: Default::default(),
    }
}

/// Anleitung mit zwei Instanzen der Materialliste und freiem LaTeX.
pub fn anleitung() -> Project {
    let mut p = Project::new(
        "Montageanleitung",
        "\\documentclass[a4paper,11pt]{article}\n\\usepackage{fontspec}\n\\usepackage[margin=2.5cm]{geometry}",
    );
    let def = materialliste();
    let module = def.reference();
    p.add_module(def);

    let intro = p.fresh_id("p");
    p.parts.push(Part {
        id: intro,
        content: PartContent::Latex {
            source: "\\section*{Montageanleitung}\nDiese Anleitung beschreibt den Aufbau. Für die Last gilt $F = m \\cdot g$.\n".into(),
        },
    });

    let mut first = ModuleInstance::new(module.clone());
    first.required = true;
    first.values.insert(
        "titel".into(),
        Value::Text("Material für Schritt 1 & 2".into()),
    );
    first.values.insert(
        "zeilen".into(),
        Value::Table(vec![
            row(
                "r1",
                &[("bez", "Schraube M4"), ("menge", "12"), ("laenge", "20")],
            ),
            row(
                "r2",
                &[("bez", "Winkel_A"), ("menge", "4"), ("laenge", "12,5")],
            ),
        ]),
    );
    first
        .values
        .insert("foto".into(), Value::Image(SAMPLE_IMAGE.into()));
    let first_id = p.fresh_id("p");
    p.parts.push(Part {
        id: first_id,
        content: PartContent::Module(first),
    });

    let middle = p.fresh_id("p");
    p.parts.push(Part {
        id: middle,
        content: PartContent::Latex {
            source: "\\subsection*{Hinweis}\nAlle Maße in Millimetern.\n".into(),
        },
    });

    let mut second = ModuleInstance::new(module);
    second.values.insert(
        "titel".into(),
        Value::Text("Ersatzteile (10 % Reserve)".into()),
    );
    second.values.insert(
        "zeilen".into(),
        Value::Table(vec![row("r1", &[("bez", "Schraube M4"), ("menge", "2")])]),
    );
    let second_id = p.fresh_id("p");
    p.parts.push(Part {
        id: second_id,
        content: PartContent::Module(second),
    });
    p
}
