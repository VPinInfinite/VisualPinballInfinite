//! A table's screenshots, for its page (a Screenshots tile beside its media,
//! opening them): those our player took — with the Screenshot key, or when a game made the high
//! scores — kept in `Screenshots\<table>` in the player's data folder, each window's picture named
//! for when (`2025-01-02 21-14-05 playfield.png`), served by the app's own protocol.

use std::path::PathBuf;

use serde::Serialize;

/// One picture: its file's name, the moment it was taken (the name's start), the window, and where
/// the screens fetch it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub name: String,
    pub when: String,
    pub window: String,
    pub url: String,
}

/// The table's folder of screenshots, by its table file's name.
pub fn folder(stem: &str) -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Screenshots").join(stem))
}

/// The windows in the order a moment's pictures show: the playfield's first, the stack's top.
const WINDOWS: [&str; 4] = ["playfield", "backglass", "score display", "topper"];

/// The pictures in the table's folder, the newest moment first, its playfield's first.
pub fn list(stem: &str, id: &str) -> Vec<Shot> {
    let Some(dir) = folder(stem) else { return Vec::new() };
    let names: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".png"))
        .collect();
    let mut shots: Vec<Shot> = names
        .into_iter()
        .map(|name| {
            let base = name.trim_end_matches(".png").trim_end_matches(".PNG");
            // `2025-01-02 21-14-05 playfield`: the date and time, then the window
            let (when, window) = match base.match_indices(' ').nth(1) {
                Some((at, _)) => (base[..at].to_string(), base[at + 1..].to_string()),
                None => (base.to_string(), String::new()),
            };
            let url = format!("{}s/{id}/{}", crate::pictures::base(), encoded(&name));
            Shot { name, when, window, url }
        })
        .collect();
    let place = |w: &str| WINDOWS.iter().position(|x| x.eq_ignore_ascii_case(w)).unwrap_or(WINDOWS.len());
    shots.sort_by(|a, b| b.when.cmp(&a.when).then_with(|| place(&a.window).cmp(&place(&b.window))).then_with(|| a.name.cmp(&b.name)));
    shots
}

/// The picture on top of the table's stack of screenshots: the newest moment's playfield, else
/// its first.
pub fn first(stem: &str, id: &str) -> Option<String> {
    list(stem, id).into_iter().next().map(|s| s.url)
}

/// How many pictures the table has.
pub fn count(stem: &str) -> usize {
    folder(stem).and_then(|d| std::fs::read_dir(d).ok()).map_or(0, |d| d.flatten().filter(|e| e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(".png")).count())
}

/// A picture's place by its name, from an address: none that reaches outside the folder.
pub fn file(stem: &str, name: &str) -> Option<PathBuf> {
    let name = decoded(name);
    if name.is_empty() || name.contains(['/', '\\', ':']) || name.starts_with('.') {
        return None;
    }
    let path = folder(stem)?.join(&name);
    path.is_file().then_some(path)
}

/// A name as an address's part: what isn't a plain letter, digit or `-._~` written as `%XX`.
fn encoded(name: &str) -> String {
    name.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// An address's part as the name it spells.
fn decoded(part: &str) -> String {
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = part.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_through_an_address_and_back() {
        let name = "2025-01-02 21-14-05 score display.png";
        assert_eq!(decoded(&encoded(name)), name);
        assert!(file("t", "..\\x.png").is_none() && file("t", "a/b.png").is_none());
    }
}
