//! The Controls page's side — every game action on any button, pressed to assign it, and a live
//! test of the buttons, plunger and nudge: the player's own input test
//! (`vpi-player --input-test`) running while the page shows — the actions and their mappings, the
//! controllers, each press and the actions it makes, the nudge and plunger — its lines handed to
//! the page as they come (`input-test`); the mappings saved under VPX 5436's names (`[Input]`
//! `Mapping.<action>`), only those that differ from what the file says.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use vpi_settings::Change;

/// The input test running, while the Controls page shows.
#[derive(Default)]
pub struct Testing(pub Mutex<Option<Child>>);

/// The player's input test started (one already running stopped first), each line it says handed
/// to the page.
pub fn start(app: &AppHandle) -> Result<(), String> {
    stop(app);
    let mut command = Command::new(crate::screens::player()?);
    command.arg("--input-test").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the player is a console program; no console shows
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| format!("The player's input test didn't start: {e}"))?;
    let out = child.stdout.take().ok_or("no output from the input test")?;
    *app.state::<Testing>().0.lock().unwrap() = Some(child);
    let app = app.clone();
    std::thread::Builder::new()
        .name("input test".into())
        .spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                // its own lines are JSON; the player's notes (`controller: …`) aren't
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    let _ = app.emit_to("main", "input-test", v);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The input test stopped: its input closed, which ends it, then made sure of.
pub fn stop(app: &AppHandle) {
    if let Some(mut child) = app.state::<Testing>().0.lock().unwrap().take() {
        drop(child.stdin.take());
        std::thread::spawn(move || {
            for _ in 0..20 {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let _ = child.kill();
            let _ = child.wait();
        });
    }
}

/// The mappings saved, by action: each written where it differs from what the file says (its own
/// value, else VPX's default the page was given), the rest of the file as it was.
pub fn save(mappings: &BTreeMap<String, String>, defaults: &BTreeMap<String, String>) -> Result<(), String> {
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let before = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let entries = vpi_settings::parse_ini(&before);
    let changes = changes(&entries, mappings, defaults);
    let after = vpi_settings::edit(&before, &changes);
    crate::screens::write_settings(&path, &before, &after)
}

fn changes(entries: &[(String, String, String)], mappings: &BTreeMap<String, String>, defaults: &BTreeMap<String, String>) -> Vec<Change> {
    let mut out = Vec::new();
    for (action, text) in mappings {
        // an action's name, never anything else of the file's
        if action.is_empty() || !action.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let key = format!("Mapping.{action}");
        let now = vpi_settings::get(entries, "Input", &key).map(|v| v.trim().to_string()).or_else(|| defaults.get(action).cloned()).unwrap_or_default();
        if normalised(&now) != normalised(text) {
            out.push(Change::set("Input", &key, text.trim()));
        }
    }
    out
}

/// A mapping's text with its spacing made even, for comparing.
fn normalised(text: &str) -> String {
    text.split('|').map(|g| g.split('&').map(str::trim).collect::<Vec<_>>().join(" & ")).collect::<Vec<_>>().join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mappings_that_changed_are_written() {
        let file = "[Input]\nMapping.Start = Key;30\n; mine\n";
        let entries = vpi_settings::parse_ini(file);
        let defaults: BTreeMap<String, String> = [("Start", "Key;30"), ("LeftFlipper", "Key;225"), ("Tilt", "Key;23")].map(|(a, b)| (a.to_string(), b.to_string())).into();
        let mut mappings = defaults.clone();
        assert!(changes(&entries, &mappings, &defaults).is_empty());
        mappings.insert("LeftFlipper".into(), "Key;225 | SDLJoy_03000000_1;4".into());
        mappings.insert("Start".into(), "Key;30|Key;40".into());
        let out = vpi_settings::edit(file, &changes(&entries, &mappings, &defaults));
        assert!(out.contains("Mapping.Start = Key;30|Key;40") && out.contains("Mapping.LeftFlipper = Key;225 | SDLJoy_03000000_1;4") && out.contains("; mine"));
        assert!(!out.contains("Mapping.Tilt"));
        // no name but an action's goes in
        mappings.insert("Bad]\n[X".into(), "Key;1".into());
        assert!(!vpi_settings::edit(file, &changes(&entries, &mappings, &defaults)).contains("[X"));
    }
}
