//! The ROMs page's side (`library::rom_inventory`): every ROM set the library has or its tables ask
//! for, checked, with the tables using it and the fixes that fit, worked out off the window's
//! thread; and a fix carried out on the user's word — only on a set the list itself just found,
//! never a path the screens hand over, what's removed going to the Recycle Bin.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::library::{self, Shared};

/// One set as the page shows it: what the list found, and the fixes that fit it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    #[serde(flatten)]
    pub entry: ::library::rom_inventory::Entry,
    pub fixes: Vec<::library::rom_inventory::Fix>,
    /// No table plays it, nor a clone of it.
    pub unused: bool,
}

/// The library's layout and each table playing a ROM with the set it plays.
fn tables(shared: &Shared) -> Result<(::library::plan::Layout, Vec<(PathBuf, String)>), String> {
    let st = shared.0.lock().unwrap();
    let layout = st.layout.clone().ok_or("The library isn't read yet.")?;
    let tables = st.tables.iter().filter_map(|t| Some((t.path.clone(), t.holds.rom_set.clone()?))).collect();
    Ok((layout, tables))
}

fn list(layout: &::library::plan::Layout, tables: &[(PathBuf, String)]) -> Result<Vec<::library::rom_inventory::Entry>, String> {
    let all = ::library::roms::all_rom_files()?;
    Ok(::library::rom_inventory::inventory(layout, tables, &all))
}

/// Every set, checked, with its fixes.
#[tauri::command]
pub async fn rom_list(shared: State<'_, Shared>) -> Result<Vec<Row>, String> {
    let (layout, tables) = tables(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(list(&layout, &tables)?.into_iter().map(|entry| Row { fixes: ::library::rom_inventory::fixes(&entry), unused: entry.unused(), entry }).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The set the list finds now at `zip`: only a zip it finds is read, shown or changed.
fn found(layout: &::library::plan::Layout, tables: &[(PathBuf, String)], zip: &str) -> Result<::library::rom_inventory::Entry, String> {
    list(layout, tables)?
        .into_iter()
        .find(|e| e.zip.as_ref().is_some_and(|z| z.to_string_lossy().eq_ignore_ascii_case(zip)))
        .ok_or_else(|| "That ROM set isn't there any more.".to_string())
}

/// Each file of the set whose zip the list finds at `zip`, checked as PinMAME loads it.
#[tauri::command]
pub async fn rom_files(shared: State<'_, Shared>, zip: String) -> Result<Vec<::library::rom_inventory::FileRow>, String> {
    let (layout, tables) = tables(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        let zip = found(&layout, &tables, &zip)?.zip.ok_or("That set isn't installed.")?;
        ::library::rom_inventory::files_of(&zip, &::library::roms::all_rom_files()?)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The set's zip shown in Explorer.
#[tauri::command]
pub async fn rom_show(shared: State<'_, Shared>, zip: String) -> Result<(), String> {
    let (layout, tables) = tables(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        let zip = found(&layout, &tables, &zip)?.zip.ok_or("That set isn't installed.")?;
        crate::shell::show(&zip)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A fix carried out on the set whose zip the list found at `zip` — the list made again first, so
/// only a set it finds now is touched, and only with a fix that still fits it; the library read
/// again after.
#[tauri::command]
pub async fn rom_fix(app: AppHandle, shared: State<'_, Shared>, zip: String, fix: String) -> Result<(), String> {
    let (layout, tables) = tables(&shared)?;
    tauri::async_runtime::spawn_blocking(move || {
        let entry = found(&layout, &tables, &zip)?;
        let chosen = ::library::rom_inventory::fixes(&entry)
            .into_iter()
            .find(|f| {
                matches!((f, fix.as_str()), (::library::rom_inventory::Fix::Rename { .. }, "rename") | (::library::rom_inventory::Fix::RemoveCopy { .. }, "removeCopy") | (::library::rom_inventory::Fix::Remove, "remove"))
            })
            .ok_or("That fix doesn't fit this set any more.")?;
        ::library::rom_inventory::fix(&entry, &chosen, &::library::install::to_recycle_bin)
    })
    .await
    .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(())
}
