use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::diagnostics::{parse_tectonic_output, Diagnostic, Severity};
use crate::{CompileFailure, Engine, FailureKind, RenderOutcome, RenderRequest, RenderedPdf};

/// Tectonic-CLI in einem eigenen Prozess mit festen Argumenten.
///
/// Die Prozessgrenze trennt Abstürze und erlaubt Abbruch. Sie ist keine
/// vollständige Sandbox (siehe Schutzbedarf „Ausführung und Ressourcen“).
#[derive(Clone, Debug)]
pub struct TectonicEngine {
    pub executable: PathBuf,
    pub timeout: Duration,
    /// Nur lokal zwischengespeicherte Ressourcen verwenden (Offline-Betrieb).
    pub only_cached: bool,
    /// Lokales Bundle statt Standardquelle.
    pub bundle: Option<PathBuf>,
    /// Elternverzeichnis für auftragsbezogene Arbeitsverzeichnisse.
    pub work_root: Option<PathBuf>,
    /// Festes Build-Datum für reproduzierbare PDFs.
    pub source_date_epoch: u64,
    /// Von außen setzbares Abbruchsignal.
    pub cancel: Arc<AtomicBool>,
    /// Eigenes Tectonic-Cacheverzeichnis (TeX-Ressourcen und Formate).
    pub cache_dir: Option<PathBuf>,
}

impl Default for TectonicEngine {
    fn default() -> Self {
        TectonicEngine {
            executable: PathBuf::from("tectonic"),
            timeout: Duration::from_secs(60),
            only_cached: false,
            bundle: None,
            work_root: None,
            source_date_epoch: 1_700_000_000,
            cancel: Arc::new(AtomicBool::new(false)),
            cache_dir: None,
        }
    }
}

impl TectonicEngine {
    fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = [
            "-X",
            "compile",
            "main.tex",
            "--untrusted",
            "--outdir",
            "out",
            "--keep-logs",
        ]
        .map(String::from)
        .into();
        if self.only_cached {
            a.push("--only-cached".into());
        }
        if let Some(b) = &self.bundle {
            a.push("--bundle".into());
            a.push(b.to_string_lossy().into_owned());
        }
        a
    }

    fn prepare(&self, request: &RenderRequest) -> std::io::Result<tempfile::TempDir> {
        let dir = match &self.work_root {
            Some(root) => {
                fs::create_dir_all(root)?;
                tempfile::Builder::new().prefix("job-").tempdir_in(root)?
            }
            None => tempfile::Builder::new()
                .prefix("formavium-job-")
                .tempdir()?,
        };
        fs::write(dir.path().join("main.tex"), &request.main_tex)?;
        fs::create_dir(dir.path().join("out"))?;
        for (rel, src) in &request.resources {
            if !formavium_core::latex::is_safe_resource_path(rel) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unzulässiger Pfad {rel}"),
                ));
            }
            let dst = dir.path().join(rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(src, &dst)
                .map_err(|e| std::io::Error::new(e.kind(), format!("Ressource {rel}: {e}")))?;
        }
        Ok(dir)
    }
}

fn env_failure(revision: u64, start: Instant, message: String) -> RenderOutcome {
    RenderOutcome {
        revision,
        result: Err(CompileFailure {
            kind: FailureKind::Environment,
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                file: None,
                line: None,
                message: message.clone(),
            }],
        }),
        warnings: Vec::new(),
        duration: start.elapsed(),
        log: message,
    }
}

fn drain(mut r: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    })
}

enum Wait {
    Exited(bool),
    TimedOut,
    Cancelled,
}

fn wait(child: &mut Child, timeout: Duration, cancel: &AtomicBool) -> std::io::Result<Wait> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Wait::Exited(status.success()));
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Wait::Cancelled);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Wait::TimedOut);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

impl Engine for TectonicEngine {
    fn compile(&self, request: &RenderRequest) -> RenderOutcome {
        let start = Instant::now();
        let rev = request.revision;
        let dir = match self.prepare(request) {
            Ok(d) => d,
            Err(e) => return env_failure(rev, start, format!("Arbeitsverzeichnis: {e}")),
        };
        let mut cmd = Command::new(&self.executable);
        cmd.args(self.args())
            .current_dir(dir.path())
            .env("SOURCE_DATE_EPOCH", self.source_date_epoch.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cache) = &self.cache_dir {
            cmd.env("TECTONIC_CACHE_DIR", cache);
        }
        #[cfg(windows)]
        {
            // Aus der GUI gestartet kein Konsolenfenster öffnen.
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return env_failure(
                    rev,
                    start,
                    format!(
                        "Tectonic ({}) nicht startbar: {e}",
                        self.executable.display()
                    ),
                )
            }
        };
        let out = drain(child.stdout.take().expect("stdout"));
        let err = drain(child.stderr.take().expect("stderr"));
        let waited = wait(&mut child, self.timeout, &self.cancel);
        let log = format!(
            "{}{}",
            out.join().unwrap_or_default(),
            err.join().unwrap_or_default()
        );
        let diagnostics = parse_tectonic_output(&log);
        let failure = |kind| RenderOutcome {
            revision: rev,
            result: Err(CompileFailure {
                kind,
                diagnostics: diagnostics.clone(),
            }),
            warnings: Vec::new(),
            duration: start.elapsed(),
            log: log.clone(),
        };
        // `dir` wird beim Verlassen entfernt, auch nach Abbruch.
        match waited {
            Err(e) => env_failure(rev, start, format!("Warten auf Tectonic: {e}")),
            Ok(Wait::TimedOut) => failure(FailureKind::Timeout),
            Ok(Wait::Cancelled) => failure(FailureKind::Cancelled),
            Ok(Wait::Exited(false)) => failure(FailureKind::Compile),
            Ok(Wait::Exited(true)) => match fs::read(dir.path().join("out").join("main.pdf")) {
                Ok(bytes) => RenderOutcome {
                    revision: rev,
                    result: Ok(RenderedPdf::new(bytes)),
                    warnings: diagnostics
                        .into_iter()
                        .filter(|d| d.severity == Severity::Warning)
                        .collect(),
                    duration: start.elapsed(),
                    log,
                },
                Err(e) => env_failure(
                    rev,
                    start,
                    format!("PDF fehlt nach erfolgreichem Lauf: {e}"),
                ),
            },
        }
    }
}

/// Sucht die Tectonic-Programmdatei: Umgebungsvariable, neben der eigenen
/// Programmdatei (Auslieferung als Sidecar), sonst Suchpfad.
pub fn locate_tectonic() -> PathBuf {
    if let Some(p) = std::env::var_os("FORMAVIUM_TECTONIC") {
        return PathBuf::from(p);
    }
    let name = if cfg!(windows) {
        "tectonic.exe"
    } else {
        "tectonic"
    };
    if let Some(dir) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::parent)
    {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(name)
}
