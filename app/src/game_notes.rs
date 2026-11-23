//! What a game left in VPX's log that the player can do something about, said on the table's page
//! once it ends: VPX out of graphics memory (its budget reached, pictures skipped), a screen the
//! settings name that VPX couldn't find. The rest of the log stays for Report a problem.

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::library::Shared;

/// One thing the last game's log says, for the table's page.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum GameNote {
    /// VPX reached its graphics-memory budget and skipped pictures; the supersampling it played at
    /// (`[Player]` `AAFactor`), the biggest share of that memory.
    #[serde(rename = "memory")]
    Memory { supersampling: f32 },
    /// A screen the settings name that VPX couldn't find, and the one it used instead; `fixed` where
    /// the app has since named it as VPX names screens.
    #[serde(rename = "screen")]
    Screen { asked: String, used: String, fixed: bool },
    /// The table's script stopped with an error: its line and VPX's words for it; `ball_id` where
    /// that line sets a ball's ID, which VPX 10.8.1 refuses (a ball's ID kept unique, VPX's
    /// `Ball::put_ID`) and 10.8.0 allowed; `older` where VPX 10.8.0 is in the library to play it with.
    #[serde(rename = "script", rename_all = "camelCase")]
    Script { line: u32, text: String, ball_id: bool, older: bool },
}

/// The first script error of VPX's last run, as `Player::OnScriptError` logs one: `Runtime error
/// on line <n>, col <c>: <words>` (or `Compile error …`).
fn script_error(run: &str) -> Option<(u32, String)> {
    run.lines().find_map(|l| {
        let at = l.find(" error on line ")?;
        if !(l[..at].ends_with("Runtime") || l[..at].ends_with("Compile")) {
            return None;
        }
        let rest = &l[at + " error on line ".len()..];
        let (line, rest) = rest.split_once(',')?;
        let text = rest.split_once(": ").map_or("", |(_, t)| t).trim().to_string();
        Some((line.trim().parse().ok()?, text))
    })
}

/// Whether a script's line sets a ball's ID (`ball.ID = 3`, `BOT(i).ID = …`): a statement of its
/// own starting with a name ending `.ID`, then `=`; a comparison after `If`, `ElseIf`, `While`,
/// `Until` or `Case` doesn't count.
pub fn sets_a_balls_id(line: &str) -> bool {
    line.split(':').any(|statement| {
        let s = statement.trim();
        let lower = s.to_ascii_lowercase();
        if ["if ", "elseif ", "while ", "do ", "loop ", "case ", "select "].iter().any(|k| lower.starts_with(k)) {
            return false;
        }
        let Some(eq) = lower.find('=') else { return false };
        let target = lower[..eq].trim_end();
        target.ends_with(".id") && target.chars().all(|c| c.is_ascii_alphanumeric() || "_.()".contains(c))
    })
}

/// The script that played the table: the one beside it (`<table>.vbs`, which VPX plays in its
/// own's place), else its own.
fn script_played(table: &Path) -> Option<String> {
    let beside = table.with_extension("vbs");
    let bytes = std::fs::read(&beside).ok().or_else(|| ::library::table::own_script(table).ok())?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The script error's note filled in: whether its line sets a ball's ID, read from the script that
/// played; whether VPX 10.8.0 is there to play the table with (`older`, decided by the caller).
pub fn fill_script(notes: &mut [GameNote], table: &Path, older: bool) {
    for n in notes {
        if let GameNote::Script { line, ball_id, older: o, .. } = n {
            *ball_id = script_played(table).and_then(|s| s.lines().nth((*line as usize).saturating_sub(1)).map(sets_a_balls_id)).unwrap_or(false);
            *o = older;
        }
    }
}

/// The notes VPX's log holds for its last run (from its last start): each kind once, each screen
/// once. `supersampling`: what VPX played at, for the memory note.
pub fn from_log(text: &str, supersampling: f32) -> Vec<GameNote> {
    let run = text.rfind("Starting VPX").map_or(text, |at| &text[at..]);
    let mut notes = Vec::new();
    if let Some((line, text)) = script_error(run) {
        notes.push(GameNote::Script { line, text, ball_id: false, older: false });
    }
    if run.contains("GPU memory budget reached") {
        notes.push(GameNote::Memory { supersampling });
    }
    // VPX's `Window::GetDisplayConfig`: `The selected display "<asked>" is not available. Using
    // display "<used>" instead.`
    for line in run.lines() {
        let Some(rest) = line.split_once("The selected display \"").map(|(_, r)| r) else { continue };
        let Some((asked, rest)) = rest.split_once('"') else { continue };
        let used = rest.split_once("Using display \"").and_then(|(_, r)| r.split_once('"')).map_or("", |(u, _)| u);
        let shown = |d: &str| vpi_settings::split_display_id(d).map_or(d, |(name, _, _)| name).to_string();
        let note = GameNote::Screen { asked: asked.to_string(), used: shown(used), fixed: false };
        if !notes.contains(&note) {
            notes.push(note);
        }
    }
    notes
}

/// The supersampling a table plays at: its own settings', else the main ones', else VPX's 1.
pub fn supersampling_of(table: &Path, main: &[(String, String, String)]) -> f32 {
    let own = std::fs::read(table.with_extension("ini")).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
    [&own[..], main].iter().find_map(|e| vpi_settings::get(e, "Player", "AAFactor").and_then(vpi_settings::vpx_float)).filter(|f| (0.5..=2.0).contains(f)).unwrap_or(1.0)
}

/// Supersampling off, VPX's own 1×: in the main settings, and taken out of the table's own where
/// it sets one.
#[tauri::command]
pub fn supersampling_off(shared: State<'_, Shared>, id: Option<String>) -> Result<(), String> {
    vpi_settings::save(&[vpi_settings::Change::set("Player", "AAFactor", "1.0")])?;
    let table = id.and_then(|id| shared.0.lock().unwrap().tables.iter().find(|t| t.id == id).map(|t| t.path.clone()));
    if let Some(file) = table.map(|t| t.with_extension("ini"))
        && let Ok(bytes) = std::fs::read(&file)
    {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if vpi_settings::get(&vpi_settings::parse_ini(&text), "Player", "AAFactor").is_some() {
            let left = vpi_settings::edit(&text, &[vpi_settings::Change::remove("Player", "AAFactor")]);
            std::fs::write(&file, left).map_err(|e| format!("{}: {e}", file.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_runs_notes_are_read_from_vpxs_log() {
        let log = "\u{feff}2025-01-02 21:28:43.026 INFO  [Main] [VPApp::InitInstance@332] Starting VPX - v10.8.1 Beta (Rev. 5436)\n\
            2025-01-02 21:29:00.000 WARN  [Main] [TextureManager::NotifyPressure@304] GPU memory budget reached: 15000 / 15977 MB used\n\
            2025-01-03 17:41:54.689 INFO  [Main] [VPApp::InitInstance@354] Starting VPX - v10.8.1 Beta (Rev. 6625 (2ad1916b2), windows BGFX 64bits)\n\
            2025-01-03 17:41:56.052 WARN  [Main] [VPX::Window::GetDisplayConfig@87] The selected display \"Wide Screen\" is not available. Using display \"Main Screen [0, 0]\" instead.\n\
            2025-01-03 17:42:31.730 WARN  [Main] [TextureManager::NotifyPressure@304] GPU memory budget reached: 13570 / 15977 MB used, 3432 MB of managed textures\n\
            2025-01-03 17:42:31.731 WARN  [Main] [TextureManager::NotifyPressure@304] GPU memory budget reached: 13570 / 15977 MB used, 3431 MB of managed textures\n\
            2025-01-03 17:42:40.000 WARN  [Main] [VPX::Window::GetDisplayConfig@87] The selected display \"Wide Screen\" is not available. Using display \"Main Screen [0, 0]\" instead.\n";
        assert_eq!(
            from_log(log, 2.0),
            vec![GameNote::Memory { supersampling: 2.0 }, GameNote::Screen { asked: "Wide Screen".into(), used: "Main Screen".into(), fixed: false }]
        );
        // a run without either: nothing, whatever runs before it said
        assert!(from_log(&format!("{log}2025-01-03 18:00:00.000 INFO  [Main] [VPApp::InitInstance@354] Starting VPX\nall well\n"), 1.0).is_empty());
        // a script stopped by an error: its first one, its line and words
        let stopped = format!("{log}2025-01-03 18:10:00.000 INFO  [Main] [VPApp::InitInstance@354] Starting VPX\n2025-01-03 18:10:05.000 ERROR [Main] [Player::OnScriptError@1663] Runtime error on line 1234, col 5: Unspecified error\n2025-01-03 18:10:06.000 ERROR [Main] [Player::OnScriptError@1663] Runtime error on line 99, col 1: Object required\n");
        assert_eq!(from_log(&stopped, 1.0), vec![GameNote::Script { line: 1234, text: "Unspecified error".into(), ball_id: false, older: false }]);
    }

    #[test]
    fn a_line_setting_a_balls_id_is_told_from_a_comparison() {
        assert!(sets_a_balls_id("    BOT(b).ID = b"));
        assert!(sets_a_balls_id("Set aBall = kicker1.CreateBall : aBall.ID = 3"));
        assert!(sets_a_balls_id("ball.id=7 ' its number"));
        assert!(!sets_a_balls_id("If ball.ID = 3 Then PlaySound \"x\""));
        assert!(!sets_a_balls_id("id = ball.ID"));
        assert!(!sets_a_balls_id("Select Case ball.ID"));
    }
}
