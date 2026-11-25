/// Eine Meldung der Engine mit Datei und Zeile, sofern bekannt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
}

/// Liest Diagnosen aus der Ausgabe der Tectonic-CLI.
///
/// Erwartetes Format: `error: main.tex:5: Undefined control sequence` bzw.
/// `warning: …`. Allgemeine Fehlerzeilen ohne Ort werden übernommen, wenn
/// keine ortsbezogene Meldung vorliegt.
pub fn parse_tectonic_output(output: &str) -> Vec<Diagnostic> {
    let mut located = Vec::new();
    let mut general = Vec::new();
    for line in output.lines() {
        let (severity, rest) = if let Some(r) = line.strip_prefix("error: ") {
            (Severity::Error, r)
        } else if let Some(r) = line.strip_prefix("warning: ") {
            (Severity::Warning, r)
        } else {
            continue;
        };
        match split_location(rest) {
            Some((file, line, message)) => located.push(Diagnostic {
                severity,
                file: Some(file.to_owned()),
                line: Some(line),
                message: message.trim().to_owned(),
            }),
            None => general.push(Diagnostic {
                severity,
                file: None,
                line: None,
                message: rest.trim().to_owned(),
            }),
        }
    }
    let has_located_error = located.iter().any(|d| d.severity == Severity::Error);
    if has_located_error {
        // Folgemeldungen wie „the XeTeX engine had an unrecoverable error“ weglassen.
        general.retain(|d| d.severity == Severity::Warning);
    }
    located.extend(general);
    // Tectonic meldet manche Warnungen doppelt.
    let mut seen = std::collections::HashSet::new();
    located.retain(|d| seen.insert((d.severity, d.file.clone(), d.line, d.message.clone())));
    located
}

fn split_location(rest: &str) -> Option<(&str, u32, &str)> {
    let mut parts = rest.splitn(3, ':');
    let file = parts.next()?;
    let line = parts.next()?.trim().parse().ok()?;
    let message = parts.next()?;
    if file.is_empty() || file.contains(' ') {
        return None;
    }
    Some((file, line, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_located_error_and_drops_followups() {
        let out = "note: Running TeX ...\nerror: main.tex:5: Undefined control sequence\n\
                   error: something bad happened inside XeTeX; its output follows:\n\
                   error: the XeTeX engine had an unrecoverable error\n\
                   warning: main.tex:9: Overfull \\hbox (1.0pt too wide) in paragraph\n";
        let d = parse_tectonic_output(out);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].line, Some(5));
        assert_eq!(d[0].message, "Undefined control sequence");
        assert_eq!(d[1].severity, Severity::Warning);
    }

    #[test]
    fn keeps_general_error_without_location() {
        let d = parse_tectonic_output("error: failed to open input file \"main.tex\"\n");
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].file, None);
    }
}
