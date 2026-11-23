//! A table's physics window (`Physics`, from its settings): the table author's own physics beside
//! the file that plays in their place, changed and saved beside the table as `<table>.vpp` — VPX's
//! own way to play other physics, so VPX plays them too — put back to the author's, and presets
//! kept in the player's data folder (`Physics presets`) to put on any table, saved from a table's
//! or brought in from a file (`library::physics`).

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::library::{self, Shared};

/// Where the presets are kept.
fn presets_dir() -> Result<PathBuf, String> {
    Ok(library::data_folder().ok_or("no APPDATA folder")?.join("Physics presets"))
}

fn table_of(shared: &Shared, id: &str) -> Result<(PathBuf, String), String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    Ok((t.path.clone(), t.stem.clone()))
}

/// What the window shows: the table's physics and the presets kept.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    #[serde(flatten)]
    pub view: ::library::physics::View,
    pub presets: Vec<::library::physics::File>,
}

#[tauri::command]
pub async fn physics_view(shared: State<'_, Shared>, id: String) -> Result<Window, String> {
    let (table, _) = table_of(&shared, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let view = ::library::physics::view(&table)?;
        Ok(Window { view, presets: presets_dir().map(|d| ::library::physics::presets(&d)).unwrap_or_default() })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The values given saved beside the table, the one there before to the Recycle Bin; the
/// library read again, for its page's physics row.
#[tauri::command]
pub async fn physics_save(app: AppHandle, shared: State<'_, Shared>, id: String, values: [f32; 18]) -> Result<(), String> {
    let (table, stem) = table_of(&shared, &id)?;
    tauri::async_runtime::spawn_blocking(move || ::library::physics::save(&table, &stem, &values, &::library::install::to_recycle_bin).map(|_| ()))
        .await
        .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(())
}

/// The table's own physics back: its file beside it to the Recycle Bin.
#[tauri::command]
pub async fn physics_back(app: AppHandle, shared: State<'_, Shared>, id: String) -> Result<(), String> {
    let (table, _) = table_of(&shared, &id)?;
    tauri::async_runtime::spawn_blocking(move || ::library::physics::back_to_its_own(&table, &::library::install::to_recycle_bin)).await.map_err(|e| e.to_string())??;
    library::start(app);
    Ok(())
}

/// The values given kept as a preset named `name`.
#[tauri::command]
pub async fn physics_preset_save(name: String, values: [f32; 18]) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || ::library::physics::save_preset(&presets_dir()?, &name, &values, &::library::install::to_recycle_bin).map(|_| ()))
        .await
        .map_err(|e| e.to_string())?
}

/// A physics settings file picked in Windows' file picker kept among the presets. False when the
/// picker's cancelled.
#[tauri::command]
pub async fn physics_preset_import(window: tauri::WebviewWindow) -> Result<bool, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || -> Result<bool, String> {
        let kinds = [("A physics settings file", "*.vpp")];
        let Some(picked) = crate::pick::file(owner, "Bring in a physics preset", &kinds)? else { return Ok(false) };
        ::library::physics::import_preset(&presets_dir()?, &picked, &::library::install::to_recycle_bin).map(|_| true)
    })
    .await
    .map_err(|e| e.to_string())?
}
