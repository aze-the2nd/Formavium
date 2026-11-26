//! SyncTeX: Zuordnung zwischen Stellen im PDF und Zeilen von `main.tex`.
//!
//! Tectonic schreibt mit `--synctex` eine gepackte Textdatei, die für jede
//! Box auf jeder Seite die Quellzeile und die Lage nennt. Formavium nutzt
//! sie für Klicks in der Vorschau, die Markierung der gewählten Stelle,
//! synchrones Scrollen und Seitenmarken im visuellen Editor.

use std::io::Read;

/// Skalierte Punkte je PostScript-Punkt (TeX: 65536 sp/pt, 72,27 pt/in).
const SP_PER_BP: f32 = 65536.0 * 72.27 / 72.0;

/// Eine Box oder ein Punkt auf einer Seite. Lage in PDF-Punkten, Ursprung
/// oben links; `y` ist die Grundlinie, `h` die Höhe darüber, `d` die Tiefe
/// darunter. Punkte (Leerraum, Kerning, Formeln) haben die Breite 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyncBox {
    pub page: u32,
    pub line: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub d: f32,
    /// Horizontale Box (`(` oder `h`): Zeile, Tabellenzelle, Wort.
    pub hbox: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncMap {
    pub boxes: Vec<SyncBox>,
}

impl SyncMap {
    /// Liest die gepackte SyncTeX-Datei.
    pub fn from_gz(bytes: &[u8]) -> std::io::Result<Self> {
        let mut text = String::new();
        flate2::read::GzDecoder::new(bytes).read_to_string(&mut text)?;
        Ok(Self::parse(&text))
    }

    /// Liest den entpackten Text. Nur Einträge zu `main.tex` zählen.
    pub fn parse(text: &str) -> Self {
        let mut main_tag: Option<u32> = None;
        let mut unit = 1.0f32;
        let mut page = 0u32;
        let mut boxes = Vec::new();
        let mut content = false;
        for line in text.lines() {
            if !content {
                if let Some(rest) = line.strip_prefix("Input:") {
                    let (tag, path) = rest.split_once(':').unwrap_or((rest, ""));
                    if path.ends_with("main.tex") && main_tag.is_none() {
                        main_tag = tag.parse().ok();
                    }
                } else if let Some(u) = line.strip_prefix("Unit:") {
                    unit = u.trim().parse().unwrap_or(1.0);
                } else if line.starts_with("Content:") {
                    content = true;
                }
                continue;
            }
            let Some(kind) = line.chars().next() else {
                continue;
            };
            let rest = &line[kind.len_utf8()..];
            if kind == '{' {
                page = rest.parse().unwrap_or(page);
                continue;
            }
            if !matches!(kind, '(' | '[' | 'h' | 'v' | 'x' | 'k' | 'g' | '$') {
                continue;
            }
            let Some(record) = parse_record(rest) else {
                continue;
            };
            if Some(record.0) != main_tag || page == 0 {
                continue;
            }
            let (_, line_no, nums) = record;
            let scale = unit / SP_PER_BP;
            let get = |i: usize| nums.get(i).copied().unwrap_or(0) as f32 * scale;
            let dims = matches!(kind, '(' | '[' | 'h' | 'v');
            boxes.push(SyncBox {
                page,
                line: line_no,
                x: get(0),
                y: get(1),
                w: if dims { get(2) } else { 0.0 },
                h: if dims { get(3) } else { 0.0 },
                d: if dims { get(4) } else { 0.0 },
                hbox: matches!(kind, '(' | 'h'),
            });
        }
        SyncMap { boxes }
    }

    pub fn pages(&self) -> u32 {
        self.boxes.iter().map(|b| b.page).max().unwrap_or(0)
    }

    /// Kleinste horizontale Box, die den Punkt enthält, sonst die Box mit
    /// dem geringsten Abstand auf derselben Seite.
    pub fn hit(&self, page: u32, x: f32, y: f32) -> Option<&SyncBox> {
        let on_page = || self.boxes.iter().filter(move |b| b.page == page);
        let inside = on_page()
            .filter(|b| b.hbox && b.w > 0.0)
            .filter(|b| x >= b.x && x <= b.x + b.w && y >= b.y - b.h - 1.0 && y <= b.y + b.d + 1.0)
            .min_by(|a, b| (a.w * (a.h + a.d)).total_cmp(&(b.w * (b.h + b.d))));
        inside.or_else(|| on_page().min_by(|a, b| distance(a, x, y).total_cmp(&distance(b, x, y))))
    }

    /// Horizontale Boxen einer Zeile auf einer Seite, von links nach rechts.
    /// Boxen, die eine andere enthalten (eine ganze Tabellenzeile), fehlen.
    pub fn cells(&self, page: u32, line: u32) -> Vec<&SyncBox> {
        let mut all: Vec<&SyncBox> = self
            .boxes
            .iter()
            .filter(|b| b.page == page && b.line == line && b.hbox && b.w > 0.0)
            .collect();
        all.sort_by(|a, b| a.x.total_cmp(&b.x));
        let inner: Vec<&SyncBox> = all
            .iter()
            .copied()
            .filter(|b| {
                !all.iter().any(|o| {
                    !std::ptr::eq(*o, *b)
                        && o.x >= b.x - 0.01
                        && o.x + o.w <= b.x + b.w + 0.01
                        && o.w < b.w
                        && (o.y - b.y).abs() < 0.5
                })
            })
            .collect();
        inner
    }
}

fn distance(b: &SyncBox, x: f32, y: f32) -> f32 {
    let dx = if x < b.x {
        b.x - x
    } else if x > b.x + b.w {
        x - b.x - b.w
    } else {
        0.0
    };
    let dy = if y < b.y - b.h {
        b.y - b.h - y
    } else if y > b.y + b.d {
        y - b.y - b.d
    } else {
        0.0
    };
    dx + dy * 4.0
}

/// `tag,line:x,y[:w,h,d]` → (tag, line, Zahlen).
fn parse_record(rest: &str) -> Option<(u32, u32, Vec<i64>)> {
    let mut parts = rest.split(':');
    let (tag, line) = parts.next()?.split_once(',')?;
    let mut nums = Vec::new();
    for group in parts {
        for n in group.split(',') {
            nums.push(n.trim().parse().ok()?);
        }
    }
    Some((tag.parse().ok()?, line.parse().ok()?, nums))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "SyncTeX Version:1\nInput:1:/tmp/job/main.tex\nInput:2:/bundle/article.cls\nOutput:pdf\nMagnification:1000\nUnit:1\nX Offset:0\nY Offset:0\nContent:\n!305\n{1\n[1,10:4736286,4736286:30000000,40000000,0\n(1,15:5612566,11500221:12418882,623900,267389\n(1,14:5612566,11500221:4915613,623900,267389\ng1,14:5612566,11500221\n(1,14:10528179,11500221:2999570,623900,267389\n(1,14:13527749,11500221:4503699,623900,267389\n(2,3:0,0:100,100,0\n]\n}1\n{2\n(1,40:5612566,5000000:2000000,623900,267389\n}2\nInput:3:/x.sty\n";

    #[test]
    fn parses_boxes_of_main_file_in_points() {
        let map = SyncMap::parse(SAMPLE);
        assert_eq!(map.pages(), 2);
        assert!(
            map.boxes.iter().all(|b| b.line != 3),
            "andere Dateien zählen nicht"
        );
        let first = map.boxes.iter().find(|b| b.line == 14 && b.hbox).unwrap();
        assert!((first.x - 85.32).abs() < 0.05, "{}", first.x);
        assert!((first.y - 174.82).abs() < 0.05, "{}", first.y);
    }

    #[test]
    fn finds_cells_and_hits() {
        let map = SyncMap::parse(SAMPLE);
        let cells = map.cells(1, 14);
        assert_eq!(cells.len(), 3);
        assert!(cells[0].x < cells[1].x && cells[1].x < cells[2].x);
        // Klick in die zweite Zelle.
        let hit = map.hit(1, cells[1].x + 2.0, cells[1].y - 2.0).unwrap();
        assert_eq!(hit.line, 14);
        assert_eq!(hit.x, cells[1].x);
        // Klick daneben: nächste Box auf der Seite.
        assert!(map.hit(2, 10.0, 10.0).is_some());
        assert!(map.hit(9, 10.0, 10.0).is_none());
    }
}
