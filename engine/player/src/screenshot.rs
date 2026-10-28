//! Screenshots: the Screenshot action — F12 unless the settings map it
//! elsewhere (`input_map`) — saves a picture of each of the windows the settings name
//! (`[VPI] ScreenshotScreens`: `playfield`, `backglass`, `dmd`, `topper`; every window showing
//! when it's unset), as it shows now, into `Screenshots\<table>` in the player's data folder, named
//! for when; and, with `[VPI] ScreenshotAtHighScore` on (off unless set; a table's own settings
//! file may turn it on for it), one when the table's script saves a new high score among its
//! values (`HighScore<n>` above the one it kept), or a ROM machine's game makes the high scores in
//! its memory (`machine_scores`). "Screenshot saved" shows on the playfield for a moment. Ours.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// Screenshots asked for and not yet taken (the input and game threads ask, the window thread takes).
static ASKED: AtomicU32 = AtomicU32::new(0);
/// A screenshot when the script saves a new high score (`[VPI] ScreenshotAtHighScore`).
static AT_HIGH_SCORE: AtomicBool = AtomicBool::new(false);
/// What the playfield window shows, until when.
static NOTICE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

const SHOWN_FOR: Duration = Duration::from_millis(1500);

/// The Screenshot action pressed (the input thread).
pub fn press() {
    ASKED.fetch_add(1, Ordering::AcqRel);
}

/// Whether a new high score takes one, from the settings, as the table starts.
pub fn set_at_high_score(on: bool) {
    AT_HIGH_SCORE.store(on, Ordering::Release);
}

/// Whether a new high score takes one.
pub fn at_high_score() -> bool {
    AT_HIGH_SCORE.load(Ordering::Acquire)
}

/// A ROM machine's game made the high scores (`machine_scores`): one is taken, when that's on.
pub fn at_new_high_score() {
    if at_high_score() {
        ASKED.fetch_add(1, Ordering::AcqRel);
    }
}

/// The script saved `name` as `text` over `before` (the game thread): a new high score — a
/// `HighScore<n>` value higher than the one kept — takes a screenshot, when that's on.
pub fn saved_value(name: &str, text: &str, before: Option<&str>) {
    if !AT_HIGH_SCORE.load(Ordering::Acquire) {
        return;
    }
    let low = name.to_ascii_lowercase();
    let Some(rest) = low.strip_prefix("highscore") else { return };
    let rest = rest.trim();
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return;
    }
    let number = |t: &str| t.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    if let Some(now) = number(text)
        && before.and_then(number).is_none_or(|was| now > was)
        && now > 0.0
    {
        ASKED.fetch_add(1, Ordering::AcqRel);
    }
}

/// Whether one is to be taken now (the window thread), taking the ask.
pub fn take() -> bool {
    ASKED.swap(0, Ordering::AcqRel) > 0
}

/// The notice to show now, if any (the window thread).
pub fn notice() -> Option<String> {
    let shown = NOTICE.lock().unwrap_or_else(|e| e.into_inner());
    shown.as_ref().filter(|(_, until)| Instant::now() < *until).map(|(text, _)| text.clone())
}

fn tell(text: String) {
    *NOTICE.lock().unwrap_or_else(|e| e.into_inner()) = Some((text, Instant::now() + SHOWN_FOR));
}

/// Which windows the settings name (`playfield`, `backglass`, `dmd`, `topper`), or None for all.
pub fn screens(settings: &crate::settings::Settings) -> Option<Vec<String>> {
    let text = settings.get("VPI", "ScreenshotScreens")?;
    Some(text.split([',', ' ', ';']).map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect())
}

/// The table's folder of screenshots: `Screenshots\<table>` in the player's data folder.
pub fn folder(table: &Path) -> Option<PathBuf> {
    let stem = table.file_stem()?.to_string_lossy().into_owned();
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("Screenshots").join(stem))
}

/// Now, local time, as a file's name begins: `2025-01-02 21-14-05`.
pub(crate) fn stamp() -> String {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocalTime(time: *mut SystemTime);
    }
    let mut t = SystemTime::default();
    // SAFETY: fills the structure it's given
    unsafe { GetLocalTime(&mut t) };
    format!("{:04}-{:02}-{:02} {:02}-{:02}-{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second)
}

/// The pictures taken, each window's RGBA pixels and size, saved as PNGs on a thread of their own
/// (a 4K picture takes a moment to write), the notice shown when they're kept.
pub fn save(table: &Path, pictures: Vec<(&'static str, Vec<u8>, u32, u32)>) {
    let Some(dir) = folder(table) else { return };
    let when = stamp();
    let spawned = std::thread::Builder::new().name("screenshot".into()).spawn(move || {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tell(format!("The screenshot couldn't be kept: {e}"));
            return;
        }
        let mut kept = 0;
        for (window, rgba, w, h) in pictures {
            let path = dir.join(format!("{when} {window}.png"));
            match image::save_buffer(&path, &rgba, w, h, image::ExtendedColorType::Rgba8) {
                Ok(()) => kept += 1,
                Err(e) => eprintln!("screenshot {}: {e}", path.display()),
            }
        }
        tell(if kept > 0 { "Screenshot saved".to_string() } else { "The screenshot couldn't be kept".to_string() });
        println!("screenshot: {kept} pictures in {}", dir.display());
    });
    if spawned.is_err() {
        tell("The screenshot couldn't be kept".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_high_score_asks_for_one_when_its_on() {
        let _ = take();
        set_at_high_score(false);
        saved_value("HighScore1", "900000", Some("500000"));
        assert!(!take());
        set_at_high_score(true);
        // higher than kept, or nothing kept: one; lower, a name, another value: none
        saved_value("HighScore1", "900000", Some("500000"));
        assert!(take());
        saved_value("highscore2", "1.2E+06", None);
        assert!(take());
        saved_value("HighScore1", "400000", Some("500000"));
        saved_value("HighScore1Name", "AAA", Some("BBB"));
        saved_value("Credits", "5", Some("4"));
        assert!(!take());
        set_at_high_score(false);
    }
}
