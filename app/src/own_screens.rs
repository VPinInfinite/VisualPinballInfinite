//! A table's own screen settings: the windows its own settings file (`<table>.ini`, where VPX's
//! menu keeps what's changed for one table) sets over the user's, said on its page in words, and
//! taken out on the user's word so the user's apply; or kept, the page then quiet about them until
//! they change.

use std::path::Path;

use serde::Serialize;
use tauri::State;

use crate::library::{self, Shared};

/// One window a table sets for itself, in words: what it is for this table, and the user's.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OwnWindow {
    pub window: &'static str,
    pub theirs: String,
    pub yours: String,
}

/// Each window's name on the page, by its section.
fn label(section: &str) -> &'static str {
    match section {
        "Player" => "Playfield",
        "Backglass" => "Backglass",
        "ScoreView" => "Score display",
        _ => "Topper",
    }
}

/// The user's screens as a game of the table starts: the main settings, with the screen preset the
/// table's own settings or the main ones choose over them.
fn yours(own: &[(String, String, String)], main: &[(String, String, String)]) -> Vec<(String, String, String)> {
    let mut out = main.to_vec();
    if let Some((_, text)) = vpi_settings::preset_named(own, main) {
        for (s, k, v) in vpi_settings::parse_ini(&text) {
            out.retain(|(os, ok, _)| !(os.eq_ignore_ascii_case(&s) && ok.eq_ignore_ascii_case(&k)));
            out.push((s, k, v));
        }
    }
    out
}

/// Two settings' values the same: as written, or as numbers.
fn same(a: &str, b: &str) -> bool {
    a.trim() == b.trim() || vpi_settings::vpx_float(a).is_some_and(|x| vpi_settings::vpx_float(b) == Some(x))
}

/// What a window's place setting holds where nothing sets it (`Settings_properties.inl`).
fn default_of(key: &str) -> &'static str {
    let k = key.to_ascii_lowercase();
    if k.ends_with("wndx") || k.ends_with("wndy") {
        "0"
    } else if k.ends_with("width") || k.ends_with("height") {
        "16384"
    } else {
        ""
    }
}

/// What a table's own settings text sets of its windows over the user's (`main`), window by
/// window, those that come out the same as the user's left out; and a fingerprint of all its window
/// settings, to keep the page quiet by until they change.
pub fn own_windows_in(text: &str, main: &[(String, String, String)]) -> (Vec<OwnWindow>, String) {
    let own = vpi_settings::parse_ini(text);
    let mine = yours(&own, main);
    let size = |k: &str, default: i32| vpi_settings::get(&mine, "Player", k).and_then(vpi_settings::vpx_float).map_or(default, |v| v as i32);
    let playfield = (size("PlayfieldWidth", 1920), size("PlayfieldHeight", 1080));
    let mut rows = Vec::new();
    let mut print = String::new();
    for (section, prefix, keys) in vpi_settings::window_settings(&own) {
        // the user's, with the table's own over them, as VPX takes them
        let mut theirs = mine.clone();
        for (k, v) in &keys {
            print.push_str(&format!("{section}.{k}={}\n", v.trim()));
            theirs.retain(|(s, ok, _)| !(s.eq_ignore_ascii_case(section) && ok.eq_ignore_ascii_case(k)));
            theirs.push((section.to_string(), k.clone(), v.clone()));
        }
        let t = vpi_settings::window_in_words(&theirs, section, prefix, playfield);
        let y = vpi_settings::window_in_words(&mine, section, prefix, playfield);
        if t != y {
            rows.push(OwnWindow { window: label(section), theirs: t, yours: y });
            continue;
        }
        // said the same, but a window that shows put somewhere else
        let moved = !t.starts_with("Off")
            && keys.iter().any(|(k, v)| {
                let lk = k.to_ascii_lowercase();
                let place = ["wndx", "wndy", "width", "height"].iter().any(|p| lk.ends_with(p)) && !lk.contains("fs");
                place && !same(vpi_settings::get(&mine, section, k).map(str::trim).unwrap_or(default_of(k)), v)
            });
        if moved {
            rows.push(OwnWindow { window: label(section), theirs: format!("{t}, placed differently"), yours: y });
        }
    }
    (rows, print)
}

/// As `own_windows_in`, from the table's own settings file beside it; none where it has none.
fn own_windows(table: &Path, main: &[(String, String, String)]) -> (Vec<OwnWindow>, String) {
    match std::fs::read(table.with_extension("ini")) {
        Ok(bytes) => own_windows_in(&String::from_utf8_lossy(&bytes), main),
        Err(_) => (Vec::new(), String::new()),
    }
}

/// The windows a table sets for itself over the user's, for its page: none where it sets none, or
/// where the user chose to keep them as they are now.
#[tauri::command]
pub fn own_screens(shared: State<'_, Shared>, id: String) -> Result<Option<Vec<OwnWindow>>, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let (rows, print) = own_windows(&t.path, &vpi_settings::read_all());
    if rows.is_empty() || st.user.kept_screens.get(&library::key(&t.path)) == Some(&print) {
        return Ok(None);
    }
    Ok(Some(rows))
}

/// The table's own window settings taken out of its settings file, so the user's screens apply;
/// its other settings (its options, sound, renderers' priorities) left as they are.
#[tauri::command]
pub fn own_screens_use_mine(shared: State<'_, Shared>, id: String) -> Result<(), String> {
    let mut st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let (file, key) = (t.path.with_extension("ini"), library::key(&t.path));
    let text = std::fs::read(&file).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| format!("{}: {e}", file.display()))?;
    let left = vpi_settings::without_window_settings(&text);
    if left != text {
        std::fs::write(&file, left).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    if st.user.kept_screens.remove(&key).is_some() {
        st.user.save()?;
    }
    Ok(())
}

/// The table's own window settings kept as they are: its page quiet about them until they change.
#[tauri::command]
pub fn own_screens_keep(shared: State<'_, Shared>, id: String) -> Result<(), String> {
    let mut st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let key = library::key(&t.path);
    let (_, print) = own_windows(&t.path, &vpi_settings::read_all());
    st.user.kept_screens.insert(key, print);
    st.user.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tables_own_windows_are_set_beside_the_users() {
        let main = vpi_settings::parse_ini(
            "[Player]\nPlayfieldDisplay = Main Screen [0, 0]\nPlayfieldFullScreen = 1\nPlayfieldWidth = 3840\nPlayfieldHeight = 2160\n[Backglass]\nBackglassOutput = 1\nBackglassDisplay = Wide Screen [-3440, 0]\nBackglassWndX = 1520\n[ScoreView]\nScoreViewOutput = 2\nScoreViewWndX = 2748\nScoreViewWndY = 0\nScoreViewWidth = 1077\nScoreViewHeight = 607\n[Topper]\nTopperOutput = 2\nTopperWndX = 0\nTopperWndY = 0\nTopperWidth = 1018\nTopperHeight = 573\n",
        );
        // as VPX's menu kept them for one table: its backglass off, its score display off, its
        // topper moved down; the backglass's place as the user's
        let own = "[ScoreView]\r\nScoreViewOutput = 0\r\n\r\n[Topper]\r\nTopperOutput = 2\r\nTopperWndX = 0\r\nTopperWndY = 1500\r\nTopperWidth = 1018\r\nTopperHeight = 573\r\n\r\n[Backglass]\r\nBackglassOutput = 0\r\nBackglassWndX = 1520\r\nPriority.PUP = 1\r\n";
        let (rows, print) = own_windows_in(own, &main);
        assert_eq!(
            rows,
            vec![
                OwnWindow { window: "Backglass", theirs: "Off".into(), yours: "Its own window on Wide Screen".into() },
                OwnWindow { window: "Score display", theirs: "Off".into(), yours: "Inside the playfield, at its top right".into() },
                OwnWindow { window: "Topper", theirs: "Inside the playfield, at its bottom left".into(), yours: "Inside the playfield, at its top left".into() },
            ]
        );
        assert!(print.contains("Backglass.BackglassOutput=0") && !print.contains("Priority"), "{print}");
        // the same window moved a little: said as placed differently; the same as the user's: none
        let (rows, _) = own_windows_in("[Topper]\nTopperOutput = 2\nTopperWndX = 40\n", &main);
        assert_eq!(rows, vec![OwnWindow { window: "Topper", theirs: "Inside the playfield, at its top left, placed differently".into(), yours: "Inside the playfield, at its top left".into() }]);
        assert!(own_windows_in("[Backglass]\nBackglassOutput = 1\nBackglassDisplay = Wide Screen [-3440, 0]\n[Player]\nSoundVolume = 65\n", &main).0.is_empty());
    }
}
