//! Feedback aus der App als Issue im öffentlichen Repository.
//!
//! Mit hinterlegtem Zugriffstoken legt die App das Issue direkt an. Ohne
//! Token öffnet die Oberfläche das vorausgefüllte Formular im Browser. Das
//! Token liegt in der Windows-Anmeldeinformationsverwaltung, nie in einer
//! Datei und nie im Installationspaket.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Repository, in dem Feedback landet.
pub const REPO: &str = "aze-the2nd/Formavium";
pub const API: &str = "https://api.github.com";
/// Seite, auf der der Anwender sein Zugriffstoken erstellt.
pub const TOKEN_HELP_URL: &str = "https://github.com/settings/personal-access-tokens/new";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Bug,
    Idea,
}

impl Kind {
    /// Standard-Label von GitHub für diese Art.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Idea => "enhancement",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Feedback {
    pub kind: Kind,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Created {
    pub number: u64,
    pub url: String,
}

const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 60_000;

pub fn validate(f: &Feedback) -> Result<(), String> {
    let title = f.title.trim();
    if title.is_empty() {
        return Err("Bitte einen Titel angeben".into());
    }
    if title.chars().count() > MAX_TITLE {
        return Err(format!("Der Titel ist länger als {MAX_TITLE} Zeichen"));
    }
    if f.body.len() > MAX_BODY {
        return Err("Die Beschreibung ist zu lang".into());
    }
    Ok(())
}

pub fn payload(f: &Feedback) -> Value {
    json!({
        "title": f.title.trim(),
        "body": f.body.trim(),
        "labels": [f.kind.label()],
    })
}

fn client(agent: &str) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(agent)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

fn offline(e: reqwest::Error) -> String {
    if e.is_timeout() || e.is_connect() {
        "Keine Verbindung zu GitHub. Der Text bleibt erhalten, bitte später erneut senden.".into()
    } else {
        format!("Anfrage an GitHub fehlgeschlagen: {e}")
    }
}

/// Meldung zu einem abgelehnten Aufruf, verständlich für den Anwender.
fn rejected(status: u16, body: &Value) -> String {
    let detail = body["message"].as_str().unwrap_or("");
    match status {
        401 => "Das Zugriffstoken ist ungültig oder abgelaufen".into(),
        403 | 404 => format!(
            "Das Zugriffstoken darf in {REPO} keine Issues anlegen (Berechtigung „Issues: Read and write“ fehlt)"
        ),
        410 => format!("In {REPO} sind Issues abgeschaltet"),
        422 => format!("GitHub hat die Angaben abgelehnt: {detail}"),
        _ => format!("GitHub antwortet mit {status}: {detail}"),
    }
}

/// Legt das Issue an. `api` ist außerhalb von Tests [`API`].
pub async fn submit(api: &str, agent: &str, token: &str, f: &Feedback) -> Result<Created, String> {
    validate(f)?;
    let response = client(agent)?
        .post(format!("{api}/repos/{REPO}/issues"))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&payload(f))
        .send()
        .await
        .map_err(offline)?;
    let status = response.status().as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if status != 201 {
        return Err(rejected(status, &body));
    }
    match (body["number"].as_u64(), body["html_url"].as_str()) {
        (Some(number), Some(url)) => Ok(Created {
            number,
            url: url.to_owned(),
        }),
        _ => Err("GitHub hat das Issue angelegt, aber keine Adresse geliefert".into()),
    }
}

/// Prüft ein Token, bevor es gespeichert wird.
pub async fn check_token(api: &str, agent: &str, token: &str) -> Result<(), String> {
    let response = client(agent)?
        .get(format!("{api}/repos/{REPO}"))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(offline)?;
    match response.status().as_u16() {
        200 => Ok(()),
        status => Err(rejected(status, &Value::Null)),
    }
}

/// Nur Adressen des eigenen Repositorys werden im Browser geöffnet.
pub fn is_repo_url(url: &str) -> bool {
    url.strip_prefix("https://github.com/")
        .and_then(|rest| rest.strip_prefix(REPO))
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Ablage des Zugriffstokens.
pub mod token {
    const SERVICE: &str = "Formavium";
    const ACCOUNT: &str = "github-feedback";

    #[cfg(windows)]
    mod store {
        use super::{ACCOUNT, SERVICE};

        fn entry() -> Result<keyring::Entry, String> {
            keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
        }
        pub fn get() -> Option<String> {
            entry().ok()?.get_password().ok()
        }
        pub fn set(token: &str) -> Result<(), String> {
            entry()?.set_password(token).map_err(|e| e.to_string())
        }
        pub fn clear() {
            if let Ok(e) = entry() {
                let _ = e.delete_credential();
            }
        }
    }

    // Außerhalb von Windows (Entwicklung): nur für die laufende Sitzung.
    #[cfg(not(windows))]
    mod store {
        use std::sync::Mutex;

        static TOKEN: Mutex<Option<String>> = Mutex::new(None);
        const _: (&str, &str) = (super::SERVICE, super::ACCOUNT);

        pub fn get() -> Option<String> {
            TOKEN.lock().ok()?.clone()
        }
        pub fn set(token: &str) -> Result<(), String> {
            *TOKEN.lock().map_err(|e| e.to_string())? = Some(token.to_owned());
            Ok(())
        }
        pub fn clear() {
            if let Ok(mut t) = TOKEN.lock() {
                *t = None;
            }
        }
    }

    pub use store::{clear, get, set};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn feedback(title: &str) -> Feedback {
        Feedback {
            kind: Kind::Bug,
            title: title.into(),
            body: "Schritte:\n1. Tabelle öffnen".into(),
        }
    }

    /// Kleiner HTTP-Server für genau eine Anfrage. Liefert Anfragezeile,
    /// Kopfzeilen und Rumpf zurück.
    fn serve_once(
        status: &'static str,
        reply: &'static str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut seen = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = v.trim().parse().unwrap();
                }
                seen.push_str(&line);
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            seen.push_str(&String::from_utf8_lossy(&body));
            let mut out = stream;
            write!(
                out,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
            seen
        });
        (addr, handle)
    }

    #[test]
    fn validates_title_and_maps_kind_to_label() {
        assert!(validate(&feedback("  ")).is_err());
        assert!(validate(&feedback(&"x".repeat(201))).is_err());
        assert!(validate(&feedback("Tabelle flackert")).is_ok());
        let p = payload(&Feedback {
            kind: Kind::Idea,
            title: "  Zeilen sortieren ".into(),
            body: " Wäre praktisch.\n".into(),
        });
        assert_eq!(p["title"], "Zeilen sortieren");
        assert_eq!(p["body"], "Wäre praktisch.");
        assert_eq!(p["labels"], json!(["enhancement"]));
    }

    #[test]
    fn creates_issue_with_token_and_label() {
        let (api, server) = serve_once(
            "201 Created",
            r#"{"number":7,"html_url":"https://github.com/aze-the2nd/Formavium/issues/7"}"#,
        );
        let created = tauri::async_runtime::block_on(submit(
            &api,
            "Formavium/test",
            "geheim",
            &feedback("Tabelle flackert"),
        ))
        .unwrap();
        assert_eq!(created.number, 7);
        assert!(is_repo_url(&created.url));
        let seen = server.join().unwrap();
        assert!(seen.starts_with("POST /repos/aze-the2nd/Formavium/issues HTTP/1.1"));
        assert!(seen
            .to_ascii_lowercase()
            .contains("authorization: bearer geheim"));
        assert!(seen.contains(r#""labels":["bug"]"#), "{seen}");
        assert!(seen.contains("Tabelle flackert"));
    }

    #[test]
    fn explains_rejected_tokens() {
        let (api, server) = serve_once("401 Unauthorized", r#"{"message":"Bad credentials"}"#);
        let err = tauri::async_runtime::block_on(submit(&api, "t", "falsch", &feedback("x")))
            .unwrap_err();
        assert!(err.contains("ungültig"), "{err}");
        server.join().unwrap();

        let (api, server) = serve_once("403 Forbidden", r#"{"message":"Resource not accessible"}"#);
        let err = tauri::async_runtime::block_on(submit(&api, "t", "ohne-recht", &feedback("x")))
            .unwrap_err();
        assert!(err.contains("Issues"), "{err}");
        server.join().unwrap();
    }

    #[test]
    fn only_own_repository_urls_are_opened() {
        assert!(is_repo_url("https://github.com/aze-the2nd/Formavium"));
        assert!(is_repo_url(
            "https://github.com/aze-the2nd/Formavium/issues/new?title=x"
        ));
        assert!(!is_repo_url(
            "https://github.com/aze-the2nd/Formavium-evil/issues"
        ));
        assert!(!is_repo_url("https://example.com/aze-the2nd/Formavium"));
        assert!(!is_repo_url("file:///C:/Windows"));
    }

    #[test]
    fn token_store_roundtrip() {
        // Unter Windows würde dies die echte Anmeldeinformationsverwaltung berühren.
        if cfg!(windows) {
            return;
        }
        token::clear();
        assert_eq!(token::get(), None);
        token::set("abc").unwrap();
        assert_eq!(token::get().as_deref(), Some("abc"));
        token::clear();
        assert_eq!(token::get(), None);
    }
}
