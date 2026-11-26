//! Zuordnung zwischen angezeigtem PDF und Dokument über SyncTeX.

use formavium_core::latex::{line_start, Origin};
use formavium_core::Generated;
use formavium_render::{SyncBox, SyncMap};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};

/// Ziel einer Markierung: ein Teil, ein Feld oder eine Zelle.
#[derive(Clone, Debug, Deserialize)]
pub struct Target {
    pub part: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub row: Option<String>,
    #[serde(default)]
    pub column: Option<String>,
}

/// Rechteck im PDF: Seite, links, oben, Breite, Höhe (PDF-Punkte).
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct Rect(pub u32, pub f32, pub f32, pub f32, pub f32);

fn line_end(text: &str, start: usize) -> usize {
    text[start..].find('\n').map_or(text.len(), |i| start + i)
}

/// Spaltenindex einer Zelle in ihrer Tabellenzeile: Anzahl der `&` vor ihr,
/// die nicht zum Inhalt einer Zelle gehören.
fn column_index(g: &Generated, start: usize, line_from: usize) -> usize {
    let inside = |pos: usize| {
        g.spans
            .iter()
            .any(|s| matches!(s.origin, Origin::Cell { .. }) && s.start <= pos && pos < s.end)
    };
    g.text[line_from..start]
        .char_indices()
        .filter(|(i, c)| *c == '&' && !inside(line_from + i))
        .count()
}

fn origin_json(origin: &Origin) -> Json {
    match origin {
        Origin::Preamble => json!({ "kind": "preamble" }),
        Origin::Latex { part } => json!({ "kind": "latex", "part": part }),
        Origin::Module { part, .. } => json!({ "kind": "module", "part": part }),
        Origin::Field { part, path } => json!({ "kind": "field", "part": part, "path": path }),
        Origin::Cell {
            part,
            path,
            row,
            column,
        } => json!({ "kind": "cell", "part": part, "path": path, "row": row, "column": column }),
    }
}

/// Was an einer Stelle des PDFs steht.
pub fn locate(g: &Generated, map: &SyncMap, page: u32, x: f32, y: f32) -> Option<Json> {
    let hit = map.hit(page, x, y)?;
    let mut line = hit.line;
    // Tabellenzeile: die k-te Zellbox ist die k-te Spalte. Die letzte Zelle
    // trägt die Folgezeile, deshalb auch die Zeile davor prüfen.
    let mut origin = None;
    if hit.hbox {
        for candidate in [hit.line, hit.line.saturating_sub(1)] {
            let cells = map.cells(page, candidate);
            if cells.len() < 2 {
                continue;
            }
            let Some(k) = cells
                .iter()
                .position(|b| (b.x - hit.x).abs() < 0.01 && (b.w - hit.w).abs() < 0.01)
            else {
                continue;
            };
            let Some(from) = line_start(&g.text, candidate) else {
                continue;
            };
            let to = line_end(&g.text, from);
            origin = g
                .spans
                .iter()
                .filter(|s| s.start >= from && s.start <= to)
                .filter(|s| matches!(s.origin, Origin::Cell { .. }))
                .find(|s| column_index(g, s.start, from) == k)
                .map(|s| &s.origin);
            if origin.is_some() {
                line = candidate;
                break;
            }
        }
    }
    let from = line_start(&g.text, line)?;
    let to = line_end(&g.text, from);
    // Sonst ein Eingabefeld auf dieser Zeile (etwa der Text einer
    // Überschrift hinter `\section{`), zuletzt der Teil.
    let origin = origin
        .or_else(|| {
            g.spans
                .iter()
                .filter(|s| s.start <= to && s.end >= from)
                .filter(|s| matches!(s.origin, Origin::Field { .. } | Origin::Cell { .. }))
                .min_by_key(|s| s.end - s.start)
                .map(|s| &s.origin)
        })
        .or_else(|| g.origin_at_line(line))?;
    let mut out = origin_json(origin);
    out["line"] = json!(line);
    out["offset"] = json!(formavium_core::latex::byte_to_utf16(&g.text, from));
    Some(out)
}

fn matches(origin: &Origin, t: &Target) -> bool {
    match origin {
        Origin::Field { part, path } => {
            part.as_str() == t.part && t.row.is_none() && t.path.as_deref() == Some(path.as_str())
        }
        Origin::Cell {
            part,
            path,
            row,
            column,
        } => {
            part.as_str() == t.part
                && t.path.as_deref() == Some(path.as_str())
                && t.row.as_deref() == Some(row.as_str())
                && t.column.as_deref().is_none_or(|c| c == column.as_str())
        }
        _ => false,
    }
}

/// Umgebendes Rechteck je Seite.
fn union(boxes: &[&SyncBox]) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for b in boxes {
        let (x0, y0, x1, y1) = (b.x, b.y - b.h, b.x + b.w, b.y + b.d);
        match out.iter_mut().find(|r| r.0 == b.page) {
            Some(r) => {
                let (rx1, ry1) = (r.1 + r.3, r.2 + r.4);
                r.1 = r.1.min(x0);
                r.2 = r.2.min(y0);
                r.3 = rx1.max(x1) - r.1;
                r.4 = ry1.max(y1) - r.2;
            }
            None => out.push(Rect(b.page, x0, y0, x1 - x0, y1 - y0)),
        }
    }
    out
}

/// Stellen im PDF, an denen ein Ziel steht.
pub fn highlight(g: &Generated, map: &SyncMap, targets: &[Target]) -> Vec<Rect> {
    let mut rects = Vec::new();
    for t in targets {
        if t.path.is_none() {
            // Ganzer Teil.
            let Some(range) = g.part_range(&t.part.as_str().into()) else {
                continue;
            };
            let (l0, l1) = (
                g.line_of(range.start),
                g.line_of(range.end.saturating_sub(1)),
            );
            let boxes: Vec<&SyncBox> = map
                .boxes
                .iter()
                .filter(|b| b.hbox && b.w > 0.0 && b.line >= l0 && b.line <= l1)
                .collect();
            rects.extend(union(&boxes));
            continue;
        }
        for span in g.spans.iter().filter(|s| matches(&s.origin, t)) {
            let line = g.line_of(span.start);
            let last = g.line_of(span.end.saturating_sub(1).max(span.start));
            if matches!(span.origin, Origin::Cell { .. }) && line == last {
                let from = line_start(&g.text, line).unwrap_or(0);
                let k = column_index(g, span.start, from);
                let pages: Vec<u32> = {
                    let mut p: Vec<u32> = map
                        .boxes
                        .iter()
                        .filter(|b| b.line == line)
                        .map(|b| b.page)
                        .collect();
                    p.dedup();
                    p
                };
                let mut found = false;
                for page in pages {
                    let cells = map.cells(page, line);
                    if cells.len() > 1 {
                        if let Some(b) = cells.get(k) {
                            rects.extend(union(&[b]));
                            found = true;
                        }
                    }
                }
                if found {
                    continue;
                }
            }
            let boxes: Vec<&SyncBox> = map
                .boxes
                .iter()
                .filter(|b| b.hbox && b.w > 0.0 && b.line >= line && b.line <= last)
                .collect();
            // Nur die innersten Boxen, nicht die ganze Textzeile drumherum.
            let inner: Vec<&SyncBox> = boxes
                .iter()
                .copied()
                .filter(|b| {
                    !boxes.iter().any(|o| {
                        !std::ptr::eq(*o, *b)
                            && o.page == b.page
                            && o.w < b.w
                            && o.x >= b.x
                            && o.x + o.w <= b.x + b.w + 0.01
                    })
                })
                .collect();
            rects.extend(union(&inner));
        }
    }
    rects
}

/// Für synchrones Scrollen und Seitenmarken: je Zeile die erste Stelle
/// im PDF (Seite, Oberkante), dazu die Zeilenbereiche der Teile.
pub fn overview(g: &Generated, map: &SyncMap) -> Json {
    let total = g.line_of(g.text.len()) as usize;
    let mut first: Vec<Option<(u32, f32)>> = vec![None; total + 2];
    for b in &map.boxes {
        let Some(slot) = first.get_mut(b.line as usize) else {
            continue;
        };
        let top = b.y - b.h;
        match slot {
            Some((p, y)) if (*p, *y) <= (b.page, top) => {}
            _ => *slot = Some((b.page, top)),
        }
    }
    let lines: Vec<Json> = first
        .iter()
        .map(|s| match s {
            Some((p, y)) => json!([p, (y * 10.0).round() / 10.0]),
            None => Json::Null,
        })
        .collect();
    let parts: Vec<Json> = g
        .part_ranges
        .iter()
        .map(|r| {
            json!([
                r.part,
                g.line_of(r.start),
                g.line_of(r.end.saturating_sub(1).max(r.start))
            ])
        })
        .collect();
    json!({ "pages": map.pages(), "lines": lines, "parts": parts })
}
