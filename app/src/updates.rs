//! The one-click updater: Tauri's own, its releases signed with our key (`tools/release/
//! make-updater-key.ps1`; the public half in `tauri.conf.json`) and looked for on the public
//! repository's GitHub releases. The app looks once as it starts, saying nothing unless it finds
//! one; Settings › General looks on a click. A new version is installed on the user's word: its
//! signature checked, Windows' installer run in its quiet way, the app closing for it.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// A new version found.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub version: String,
    /// What's new in it, as its release says.
    pub notes: Option<String>,
}

/// A newer version than this one, if the releases have one.
pub async fn check(app: &AppHandle) -> Result<Option<Found>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    Ok(update.map(|u| Found { version: u.version.clone(), notes: u.body.clone() }))
}

/// The new version downloaded, its signature checked and installed; how much has come told to the
/// screens as it comes (`update-progress`: bytes, and the whole when known).
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Err("There's no new version to install.".into());
    };
    let progress = app.clone();
    let mut got = 0usize;
    update
        .download_and_install(
            move |chunk, total| {
                got += chunk;
                let _ = progress.emit("update-progress", (got, total));
            },
            || {},
        )
        .await
        .map_err(|e| format!("The new version couldn't be installed: {e}"))?;
    // on Windows the installer has taken over by now; anywhere else, start again
    app.restart();
}
