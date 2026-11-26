//! Desktop-Hülle: reicht Befehle der Oberfläche an die Sitzung weiter.
//!
//! Dokumentzugriffe laufen ausschließlich über den Rust-Kern (`call`, `pdf`,
//! `resource`). Dazu kommen Feedback als GitHub-Issue, das Öffnen von
//! Adressen des eigenen Repositorys und die Plugins für Dateidialoge und
//! Updates.

mod feedback;

use std::sync::{Arc, Mutex};

use formavium_render::{locate_tectonic, TectonicEngine};
use formavium_session::{install_seed_cache, Session};
use serde_json::{json, Value};
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

struct AppState(Mutex<Session>);

#[tauri::command]
fn call(state: State<'_, AppState>, name: String, args: Value) -> Result<Value, String> {
    state
        .0
        .lock()
        .map_err(|_| "Sitzung nicht verfügbar".to_owned())?
        .call(&name, &args)
}

/// Angezeigtes PDF als Rohbytes (ohne JSON-Umweg).
#[tauri::command]
fn pdf(state: State<'_, AppState>) -> Result<Response, String> {
    let mut session = state
        .0
        .lock()
        .map_err(|_| "Sitzung nicht verfügbar".to_owned())?;
    match session.pdf() {
        Some((_, bytes)) => Ok(Response::new(bytes.as_ref().clone())),
        None => Err("Kein PDF vorhanden".into()),
    }
}

/// Bild neben der Arbeitsdatei für die Anzeige im visuellen Editor.
#[tauri::command]
fn resource(state: State<'_, AppState>, path: String) -> Result<Response, String> {
    let session = state
        .0
        .lock()
        .map_err(|_| "Sitzung nicht verfügbar".to_owned())?;
    match session.resource(&path) {
        Some((_, bytes)) => Ok(Response::new(bytes)),
        None => Err(format!("Bild „{path}“ nicht gefunden")),
    }
}

fn agent(app: &AppHandle) -> String {
    format!("Formavium/{}", app.package_info().version)
}

/// Ob Feedback direkt als Issue angelegt werden kann.
#[tauri::command]
fn feedback_status() -> Value {
    json!({ "repo": feedback::REPO, "has_token": feedback::token::get().is_some() })
}

/// Prüft das Zugriffstoken und legt es in der Anmeldeinformationsverwaltung ab.
#[tauri::command]
async fn feedback_set_token(app: AppHandle, token: String) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("Bitte ein Zugriffstoken eingeben".into());
    }
    feedback::check_token(feedback::API, &agent(&app), token).await?;
    feedback::token::set(token)
}

#[tauri::command]
fn feedback_clear_token() {
    feedback::token::clear();
}

/// Legt das Issue an und liefert Nummer und Adresse.
#[tauri::command]
async fn feedback_submit(
    app: AppHandle,
    feedback: feedback::Feedback,
) -> Result<feedback::Created, String> {
    let token = feedback::token::get().ok_or("Kein Zugriffstoken hinterlegt")?;
    feedback::submit(feedback::API, &agent(&app), &token, &feedback).await
}

/// Öffnet eine Adresse des eigenen Repositorys im Standardbrowser, dazu die
/// GitHub-Seite zum Erstellen des Zugriffstokens. Andere Adressen nicht.
#[tauri::command]
fn open_repo_url(app: AppHandle, url: String) -> Result<(), String> {
    if !(feedback::is_repo_url(&url) || url == feedback::TOKEN_HELP_URL) {
        return Err("Nur Adressen des Formavium-Repositorys werden geöffnet".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Engine mit mitgeliefertem Tectonic und Offline-Ressourcen.
///
/// Mit mitgeliefertem Ressourcenpaket rendert Formavium ausschließlich aus
/// dem lokalen Cache. `FORMAVIUM_ALLOW_NETWORK=1` erlaubt das Nachladen
/// fehlender Pakete.
fn engine(app: &tauri::App) -> Result<TectonicEngine, Box<dyn std::error::Error>> {
    let data = app.path().app_local_data_dir()?;
    let seed = app.path().resource_dir()?.join("tex-cache");
    let cache = install_seed_cache(&seed, &data.join("tectonic-cache"))?;
    let allow_network = std::env::var_os("FORMAVIUM_ALLOW_NETWORK").is_some();
    Ok(TectonicEngine {
        executable: locate_tectonic(),
        only_cached: cache.is_some() && !allow_network,
        cache_dir: cache,
        work_root: Some(data.join("jobs")),
        ..Default::default()
    })
}

/// Ordner der eigenen Bibliothek: `Dokumente/Formavium/Bibliothek`, damit
/// der Anwender ihn sieht, sichern und weitergeben kann. `FORMAVIUM_LIBRARY`
/// zeigt auf einen anderen Ordner, etwa ein gemeinsames Laufwerk.
fn library_dir(app: &tauri::App) -> Option<std::path::PathBuf> {
    if let Some(dir) = std::env::var_os("FORMAVIUM_LIBRARY") {
        return Some(dir.into());
    }
    let docs = app.path().document_dir().ok()?;
    Some(docs.join("Formavium").join("Bibliothek"))
}

/// Zeigt den Bibliotheksordner im Explorer und legt ihn dafür bei Bedarf an.
#[tauri::command]
fn reveal_library(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let dir = state
        .0
        .lock()
        .map_err(|_| "Sitzung nicht verfügbar".to_owned())?
        .library_dir()
        .map(std::path::Path::to_path_buf)
        .ok_or("Kein Bibliotheksordner eingerichtet")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Ein zweiter Start öffnet keine zweite App: Er holt das vorhandene
        // Fenster nach vorn und beendet sich. So kann auch kein Update aus
        // dem zweiten Start die laufende Sitzung beenden.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let engine = engine(app)?;
            let mut session = Session::new(Arc::new(engine));
            session.set_library_dir(library_dir(app));
            app.manage(AppState(Mutex::new(session)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            call,
            pdf,
            resource,
            feedback_status,
            feedback_set_token,
            feedback_clear_token,
            feedback_submit,
            open_repo_url,
            reveal_library
        ])
        .run(tauri::generate_context!())
        .expect("Formavium konnte nicht gestartet werden");
}
