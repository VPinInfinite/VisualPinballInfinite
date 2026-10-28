//! A table's music, for cabinet mode to play while the table's chosen: PinUP Popper's audio for it,
//! else the first music its script plays, else its AltSound pack's music — its main theme where the
//! pack names one (the music channel of its list, `CHANNEL` 0 or a `TYPE` of music, a name with
//! "theme" or "main" first), else PinSound's `music` folder — else its PinUP pack's background music
//! (a folder of its whose name says music, or background, or attract).

use std::path::{Path, PathBuf};

use ::library::installed::Holds;

/// The sound files a browser plays.
const SOUNDS: [&str; 6] = ["mp3", "ogg", "wav", "m4a", "aac", "flac"];

fn is_sound(p: &Path) -> bool {
    p.is_file() && p.extension().is_some_and(|x| SOUNDS.iter().any(|s| x.eq_ignore_ascii_case(s)))
}

/// The table's music, by what it holds.
pub fn of(holds: &Holds) -> Option<PathBuf> {
    holds
        .media
        .iter()
        .find(|m| m.kind.eq_ignore_ascii_case("Audio") || m.kind.eq_ignore_ascii_case("AudioLaunch"))
        .map(|m| m.path.clone())
        .or_else(|| holds.music.iter().find(|p| is_sound(p)).cloned())
        .or_else(|| holds.altsound.as_deref().and_then(altsound))
        .or_else(|| holds.pup_pack.as_deref().and_then(pup))
}

/// A line of a pack's list split at its commas, quotes taken off.
fn fields(line: &str) -> Vec<String> {
    line.split(',').map(|f| f.trim().trim_matches('"').trim().to_string()).collect()
}

/// An AltSound pack's music: its list's, the main theme first; else PinSound's `music` folder's.
fn altsound(pack: &Path) -> Option<PathBuf> {
    for list in ["altsound.csv", "g-sound.csv"] {
        let Ok(bytes) = std::fs::read(pack.join(list)) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        let mut lines = text.lines();
        let header: Vec<String> = fields(lines.next().unwrap_or_default()).iter().map(|h| h.to_lowercase()).collect();
        let col = |name: &str| header.iter().position(|h| h == name);
        let (channel, kind, name, file) = (col("channel"), col("type"), col("name"), col("fname"));
        let Some(file) = file else { continue };
        let music: Vec<(String, PathBuf)> = lines
            .map(fields)
            .filter(|f| channel.and_then(|c| f.get(c)).is_some_and(|c| c == "0") || kind.and_then(|k| f.get(k)).is_some_and(|k| k.eq_ignore_ascii_case("music")))
            .filter_map(|f| {
                let path = pack.join(f.get(file)?.replace('\\', "/"));
                is_sound(&path).then(|| (name.and_then(|n| f.get(n)).cloned().unwrap_or_default().to_lowercase(), path))
            })
            .collect();
        let theme = music.iter().find(|(n, p)| {
            let file = p.file_name().map(|f| f.to_string_lossy().to_lowercase()).unwrap_or_default();
            ["theme", "main"].iter().any(|w| n.contains(w) || file.contains(w))
        });
        if let Some((_, p)) = theme.or(music.first()) {
            return Some(p.clone());
        }
    }
    first_sound(&pack.join("music"))
}

/// A PinUP pack's background music: the first sound in a folder of its named for music, else for the
/// background, else for its attract mode.
fn pup(pack: &Path) -> Option<PathBuf> {
    let folders: Vec<PathBuf> = std::fs::read_dir(pack).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    for word in ["music", "bg", "attract"] {
        for folder in folders.iter().filter(|f| f.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase().contains(word))) {
            if let Some(p) = first_sound(folder) {
                return Some(p);
            }
        }
    }
    None
}

fn first_sound(folder: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder).ok()?.flatten().map(|e| e.path()).filter(|p| is_sound(p)).collect();
    found.sort();
    found.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_altsound_packs_main_theme_first() {
        let pack = std::env::temp_dir().join(format!("vpi-tunes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pack);
        std::fs::create_dir_all(&pack).unwrap();
        for f in ["0x0001-ball_wait.ogg", "0x0002-main_theme.ogg", "0x00a3-jingle.ogg"] {
            std::fs::write(pack.join(f), b"x").unwrap();
        }
        std::fs::write(
            pack.join("altsound.csv"),
            "\"ID\",\"CHANNEL\",\"DUCK\",\"GAIN\",\"LOOP\",\"STOP\",\"NAME\",\"FNAME\"\n\"0x00a3\", 1, 10, 50, 0, 0, \"jingle\", \"0x00a3-jingle.ogg\"\n\"0x0001\", 0, 100, 99, 100, 0, \"ball wait\", \"0x0001-ball_wait.ogg\"\n\"0x0002\", 0, 100, 99, 100, 0, \"main theme\", \"0x0002-main_theme.ogg\"\n",
        )
        .unwrap();
        let found = altsound(&pack);
        let _ = std::fs::remove_dir_all(&pack);
        assert!(found.unwrap().ends_with("0x0002-main_theme.ogg"));
    }
}
