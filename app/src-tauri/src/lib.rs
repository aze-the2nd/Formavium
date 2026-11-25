//! Desktop-Hülle: reicht Befehle der Oberfläche an die Sitzung weiter.
//!
//! Die Oberfläche erhält nur drei Befehle (`call`, `pdf`, `resource`) und
//! das Dialog-Plugin. Dateizugriffe laufen ausschließlich über den Rust-Kern.

use std::sync::{Arc, Mutex};

use formavium_render::{locate_tectonic, TectonicEngine};
use formavium_session::{install_seed_cache, Session};
use serde_json::Value;
use tauri::ipc::Response;
use tauri::{Manager, State};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let engine = engine(app)?;
            app.manage(AppState(Mutex::new(Session::new(Arc::new(engine)))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![call, pdf, resource])
        .run(tauri::generate_context!())
        .expect("Formavium konnte nicht gestartet werden");
}
