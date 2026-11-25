//! Dev-Bridge: stellt die Sitzung per HTTP bereit, damit das Frontend im
//! Browser gegen den echten Kern laufen kann (Entwicklung im Container).
//!
//! Nur für die Entwicklung. Die Bridge liest und schreibt beliebige Pfade
//! und darf deshalb nur an localhost gebunden werden.
//!
//! * `POST /api/call` mit `{"name": "...", "args": {...}}` → JSON
//! * `GET /api/pdf` → angezeigtes PDF, Revision im Kopf `X-Revision`
//! * `GET /api/resource?path=bilder/x.png` → Bild neben der Arbeitsdatei

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use formavium_session::Session;
use serde_json::{json, Value};

fn main() {
    let addr = std::env::var("FORMAVIUM_BRIDGE_ADDR").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| panic!("{addr}: {e}"));
    eprintln!("formavium-devbridge hört auf http://{addr}");
    let mut session = Session::default();
    // Eigene Bibliothek für die Entwicklung, abseits der Projektdateien.
    let library = std::env::var("FORMAVIUM_LIBRARY").unwrap_or_else(|_| "out/bibliothek".into());
    session.set_library_dir(Some(library.into()));
    let session = Arc::new(Mutex::new(session));
    for stream in listener.incoming().flatten() {
        let session = Arc::clone(&session);
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, &session) {
                eprintln!("Anfrage fehlgeschlagen: {e}");
            }
        });
    }
}

fn handle(stream: TcpStream, session: &Mutex<Session>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0; length.min(64 * 1024 * 1024)];
    reader.read_exact(&mut body)?;
    let mut out = stream;

    match (method, path.split('?').next().unwrap_or("")) {
        ("POST", "/api/call") => {
            let req: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
            let name = req["name"].as_str().unwrap_or("");
            let args = req.get("args").cloned().unwrap_or(json!({}));
            let result = session.lock().expect("Sitzung").call(name, &args);
            let (status, payload) = match result {
                Ok(v) => ("200 OK", json!({ "ok": v })),
                Err(e) => ("200 OK", json!({ "error": e })),
            };
            respond(
                &mut out,
                status,
                "application/json",
                &[],
                payload.to_string().as_bytes(),
            )
        }
        ("GET", "/api/pdf") => match session.lock().expect("Sitzung").pdf() {
            Some((rev, bytes)) => respond(
                &mut out,
                "200 OK",
                "application/pdf",
                &[("X-Revision", rev.to_string())],
                &bytes,
            ),
            None => respond(&mut out, "404 Not Found", "text/plain", &[], b"kein PDF"),
        },
        ("GET", "/api/resource") => {
            let rel = path
                .split_once("?path=")
                .map(|(_, q)| percent_decode(q))
                .unwrap_or_default();
            match session.lock().expect("Sitzung").resource(&rel) {
                Some((mime, bytes)) => respond(&mut out, "200 OK", mime, &[], &bytes),
                None => respond(&mut out, "404 Not Found", "text/plain", &[], b"fehlt"),
            }
        }
        _ => respond(&mut out, "404 Not Found", "text/plain", &[], b"unbekannt"),
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%')
            .then(|| s.get(i + 1..i + 3))
            .flatten()
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(v) => {
                out.push(v);
                i += 3;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn respond(
    out: &mut TcpStream,
    status: &str,
    content_type: &str,
    headers: &[(&str, String)],
    body: &[u8],
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    out.write_all(head.as_bytes())?;
    out.write_all(body)?;
    out.flush()
}
