use std::sync::{Arc, Mutex};
use std::time::Duration;

use formavium_core::{generate, sample, Command, Workspace};
use formavium_render::*;

fn ok(revision: u64, bytes: &[u8]) -> RenderOutcome {
    RenderOutcome {
        revision,
        result: Ok(RenderedPdf::new(bytes.to_vec())),
        warnings: vec![],
        duration: Duration::ZERO,
        log: String::new(),
    }
}

fn failed(revision: u64) -> RenderOutcome {
    RenderOutcome {
        revision,
        result: Err(CompileFailure {
            kind: FailureKind::Compile,
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                file: Some("main.tex".into()),
                line: Some(3),
                message: "Undefined control sequence".into(),
            }],
        }),
        warnings: vec![],
        duration: Duration::ZERO,
        log: String::new(),
    }
}

#[test]
fn failure_keeps_last_good_pdf_marked_as_stale() {
    let mut p = Preview::default();
    assert_eq!(p.status(), PreviewStatus::Empty);
    assert!(p.accept(ok(2, b"pdf-2")));
    assert!(p.accept(failed(3)));
    assert_eq!(
        p.status(),
        PreviewStatus::Stale {
            shown: 2,
            failed: 3
        }
    );
    assert_eq!(p.shown().unwrap().pdf.bytes.as_slice(), b"pdf-2");
    assert_eq!(p.failure().unwrap().diagnostics[0].line, Some(3));
    assert!(p.accept(ok(4, b"pdf-4")));
    assert_eq!(p.status(), PreviewStatus::Current { revision: 4 });
}

#[test]
fn first_failure_has_no_pdf() {
    let mut p = Preview::default();
    p.accept(failed(1));
    assert_eq!(p.status(), PreviewStatus::Failed { failed: 1 });
    let dir = tempfile::tempdir().unwrap();
    assert!(p.save_pdf(&dir.path().join("x.pdf")).is_err());
}

#[test]
fn late_older_result_is_ignored() {
    let mut p = Preview::default();
    assert!(p.accept(ok(5, b"neu")));
    assert!(!p.accept(ok(4, b"alt")));
    assert!(!p.accept(failed(3)));
    assert_eq!(p.status(), PreviewStatus::Current { revision: 5 });
}

#[test]
fn save_pdf_writes_exactly_the_shown_bytes() {
    let mut p = Preview::default();
    p.accept(ok(7, b"%PDF-angezeigt"));
    p.accept(failed(8));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.pdf");
    let shown = p.save_pdf(&path).unwrap();
    assert_eq!(shown.revision, 7);
    assert_eq!(std::fs::read(&path).unwrap(), b"%PDF-angezeigt");
}

/// Engine, die bei Revision 1 langsam ist und Aufrufe protokolliert.
struct FakeEngine {
    calls: Arc<Mutex<Vec<u64>>>,
}

impl Engine for FakeEngine {
    fn compile(&self, req: &RenderRequest) -> RenderOutcome {
        self.calls.lock().unwrap().push(req.revision);
        if req.revision == 1 {
            std::thread::sleep(Duration::from_millis(200));
        }
        ok(req.revision, req.main_tex.as_bytes())
    }
}

fn req(revision: u64) -> RenderRequest {
    RenderRequest {
        revision,
        main_tex: format!("rev {revision}"),
        resources: vec![],
    }
}

#[test]
fn only_latest_pending_revision_waits() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (renderer, rx) = Renderer::start(FakeEngine {
        calls: Arc::clone(&calls),
    });
    renderer.request(req(1));
    std::thread::sleep(Duration::from_millis(50)); // Revision 1 läuft
    for r in 2..=6 {
        renderer.request(req(r));
    }
    let mut preview = Preview::default();
    let first = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let second = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!((first.revision, second.revision), (1, 6));
    preview.accept(first);
    preview.accept(second);
    assert_eq!(preview.status(), PreviewStatus::Current { revision: 6 });
    assert_eq!(*calls.lock().unwrap(), vec![1, 6]);
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
}

#[test]
fn older_request_does_not_replace_newer_pending() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let (renderer, rx) = Renderer::start(FakeEngine {
        calls: Arc::clone(&calls),
    });
    renderer.request(req(1));
    std::thread::sleep(Duration::from_millis(50));
    renderer.request(req(4));
    renderer.request(req(3));
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap().revision, 4);
}

// --- Tests mit echter Tectonic-Engine (übersprungen, falls nicht vorhanden) ---

fn tectonic() -> Option<TectonicEngine> {
    let exe = locate_tectonic();
    let ok = std::process::Command::new(&exe)
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("Tectonic nicht gefunden, Test übersprungen");
        return None;
    }
    Some(TectonicEngine {
        executable: exe,
        ..Default::default()
    })
}

fn sample_request(ws: &Workspace, dir: &std::path::Path) -> RenderRequest {
    RenderRequest::from_generated(ws.revision(), &generate(ws.project()), dir)
}

fn project_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("bilder")).unwrap();
    std::fs::write(
        dir.path().join(sample::SAMPLE_IMAGE),
        include_bytes!("../../formavium-cli/assets/beispiel.png"),
    )
    .unwrap();
    dir
}

#[test]
fn tectonic_renders_sample_and_reports_error_location() {
    let Some(engine) = tectonic() else { return };
    let dir = project_dir();
    let mut ws = Workspace::new(sample::anleitung());
    let mut preview = Preview::default();

    let good = engine.compile(&sample_request(&ws, dir.path()));
    assert!(good.result.is_ok(), "{}", good.log);
    assert!(good.result.as_ref().unwrap().bytes.starts_with(b"%PDF"));
    preview.accept(good);

    let part = ws.project().parts[0].id.clone();
    let r = ws.revision();
    ws.apply(
        r,
        Command::SetLatex {
            part: part.clone(),
            source: "Text\n\\gibtsnicht\n".into(),
        },
    )
    .unwrap();
    let generated = generate(ws.project());
    let bad = engine.compile(&RenderRequest::from_generated(
        ws.revision(),
        &generated,
        dir.path(),
    ));
    let failure = bad.result.as_ref().unwrap_err().clone();
    assert_eq!(failure.kind, FailureKind::Compile);
    let line = failure.diagnostics[0].line.expect("Zeilenangabe");
    assert_eq!(
        generated.origin_at_line(line),
        Some(&formavium_core::Origin::Latex { part })
    );
    preview.accept(bad);
    assert!(matches!(preview.status(), PreviewStatus::Stale { .. }));
}

#[test]
fn tectonic_output_is_reproducible() {
    let Some(engine) = tectonic() else { return };
    let dir = project_dir();
    let ws = Workspace::new(sample::anleitung());
    let a = engine
        .compile(&sample_request(&ws, dir.path()))
        .result
        .unwrap();
    let b = engine
        .compile(&sample_request(&ws, dir.path()))
        .result
        .unwrap();
    assert_eq!(
        a.sha256, b.sha256,
        "bytegleiche PDFs bei festem Build-Datum"
    );
}

#[test]
fn tectonic_timeout_kills_process() {
    let Some(mut engine) = tectonic() else { return };
    engine.timeout = Duration::from_millis(1500);
    let tex = "\\documentclass{article}\\begin{document}\\def\\x{\\x}\\x\\end{document}\n";
    let out = engine.compile(&RenderRequest {
        revision: 1,
        main_tex: tex.into(),
        resources: vec![],
    });
    assert_eq!(out.result.unwrap_err().kind, FailureKind::Timeout);
    assert!(out.duration < Duration::from_secs(5));
}
