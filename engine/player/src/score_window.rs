//! The floating score window (one screen) moved with the mouse, as VPinMAME's own
//! window is: held anywhere with the left button and dragged, its place remembered for each
//! game. VPinMAME keeps its window's place per game in the
//! registry (`dmd_pos_x`, `dmd_pos_y`); ours goes in `Score windows.ini` beside the settings
//! file, in pixels from the table window's top-left corner, so it follows the table window.
//! The window follows the mouse itself rather than through Windows' own move loop, which would
//! hold the table's drawing while it's dragged. Ours.

use std::path::{Path, PathBuf};

use sdl3_sys::everything::*;

/// The file's section the places are kept in, by game.
const SECTION: &str = "FloatingScore";

/// What a new file starts with.
const HEADER: &str = "; Where the floating score window was left for each game (dragged with the mouse), in pixels\r\n\
; from the table window's top-left corner. Visual Pinball Infinite writes this file; delete a\r\n\
; line to put that game's window back where the settings place it.";

/// The floating score window, dragged by the mouse.
pub struct ScoreWindow {
    window: *mut SDL_Window,
    /// The table's window, which the places count from.
    table: *mut SDL_Window,
    id: SDL_WindowID,
    /// Where the mouse took hold of it, in the window, while it's dragged.
    grab: Option<(f32, f32)>,
    /// The game the machine plays, once the script names it.
    game: Option<String>,
    /// Where the places are kept, if anywhere.
    file: Option<PathBuf>,
}

impl ScoreWindow {
    /// The floating window `window` over the table's `table`, its places kept in `file`.
    ///
    /// # Safety
    /// Both windows outlive what's made, and it's used on their thread.
    pub unsafe fn new(window: *mut SDL_Window, table: *mut SDL_Window, file: Option<PathBuf>) -> ScoreWindow {
        // SAFETY: a live window, as the caller promises
        let id = unsafe { SDL_GetWindowID(window) };
        ScoreWindow { window, table, id, grab: None, game: None, file }
    }

    /// Where the places are kept: beside the settings file in use, else in the player's data
    /// folder (`%APPDATA%\Visual Pinball Infinite`).
    pub fn file_for(settings: Option<&Path>) -> Option<PathBuf> {
        let dir = match settings.and_then(Path::parent) {
            Some(d) => d.to_path_buf(),
            None => PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite"),
        };
        Some(dir.join("Score windows.ini"))
    }

    /// One of the window thread's events: the left button pressed on the window takes hold of
    /// it, the mouse moving then drags it, and letting go leaves it there, its place remembered
    /// for the game.
    pub fn event(&mut self, event: &SDL_Event) {
        // SAFETY: the event's kind says which of its parts holds; the windows are live
        unsafe {
            let kind = event.r#type;
            if kind == SDL_EVENT_MOUSE_BUTTON_DOWN.0 && event.button.windowID == self.id && i32::from(event.button.button) == SDL_BUTTON_LEFT {
                self.grab = Some((event.button.x, event.button.y));
            } else if kind == SDL_EVENT_MOUSE_MOTION.0
                && let Some((gx, gy)) = self.grab
            {
                let (mut x, mut y) = (0.0f32, 0.0f32);
                SDL_GetGlobalMouseState(&mut x, &mut y);
                SDL_SetWindowPosition(self.window, (x - gx).round() as i32, (y - gy).round() as i32);
            } else if kind == SDL_EVENT_MOUSE_BUTTON_UP.0 && i32::from(event.button.button) == SDL_BUTTON_LEFT && self.grab.take().is_some() {
                self.remember();
            }
        }
    }

    /// The game the machine plays (`Controller.GameName`): the window goes where it was left for
    /// that game, if it ever was and that's still on a screen.
    pub fn set_game(&mut self, game: &str) {
        if game.is_empty() || self.game.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(game)) {
            return;
        }
        self.game = Some(game.to_string());
        let Some((dx, dy)) = self.file.as_deref().and_then(|f| remembered(f, game)) else { return };
        // SAFETY: live windows
        unsafe {
            let (mut x, mut y, mut w, mut h) = (0, 0, 0, 0);
            SDL_GetWindowPosition(self.table, &mut x, &mut y);
            SDL_GetWindowSize(self.window, &mut w, &mut h);
            if on_a_screen(x + dx, y + dy, w, h) {
                SDL_SetWindowPosition(self.window, x + dx, y + dy);
            }
        }
    }

    /// The window's place now, from the table window's corner, kept for the game.
    fn remember(&self) {
        let (Some(game), Some(file)) = (self.game.as_deref(), self.file.as_deref()) else { return };
        let (mut x, mut y, mut tx, mut ty) = (0, 0, 0, 0);
        // SAFETY: live windows
        unsafe {
            SDL_GetWindowPosition(self.window, &mut x, &mut y);
            SDL_GetWindowPosition(self.table, &mut tx, &mut ty);
        }
        if let Err(e) = write_place(file, game, x - tx, y - ty) {
            eprintln!("the score window's place for {game} couldn't be kept in {}: {e}", file.display());
        }
    }
}

/// Whether at least a corner of a window at `x`, `y` of `w` × `h` is on one of the screens.
fn on_a_screen(x: i32, y: i32, w: i32, h: i32) -> bool {
    const SHOWN: i32 = 32;
    let mut count = 0;
    // SAFETY: SDL's list of screens, freed after reading
    unsafe {
        let displays = SDL_GetDisplays(&mut count);
        if displays.is_null() {
            return true;
        }
        let mut seen = false;
        for k in 0..count.max(0) as usize {
            let mut r = SDL_Rect { x: 0, y: 0, w: 0, h: 0 };
            if SDL_GetDisplayBounds(*displays.add(k), &mut r) {
                let across = (x + w).min(r.x + r.w) - x.max(r.x);
                let down = (y + h).min(r.y + r.h) - y.max(r.y);
                seen |= across >= SHOWN.min(w) && down >= SHOWN.min(h);
            }
        }
        SDL_free(displays.cast());
        seen
    }
}

/// The place kept for `game`, if one is.
fn remembered(file: &Path, game: &str) -> Option<(i32, i32)> {
    let text = std::fs::read_to_string(file).ok()?;
    let settings = crate::settings::Settings::from_text(&text);
    let (x, y) = settings.get(SECTION, game)?.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Keeps `game`'s place in the file, its other lines as they were.
fn write_place(file: &Path, game: &str, dx: i32, dy: i32) -> std::io::Result<()> {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let entry = format!("{game} = {dx}, {dy}");
    let header = format!("[{SECTION}]");
    match lines.iter().position(|l| l.trim().eq_ignore_ascii_case(&header)) {
        Some(start) => {
            let end = lines[start + 1..].iter().position(|l| l.trim_start().starts_with('[')).map_or(lines.len(), |k| start + 1 + k);
            let found = lines[start + 1..end].iter().position(|l| !l.trim_start().starts_with(';') && l.split_once('=').is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case(game)));
            match found {
                Some(k) => lines[start + 1 + k] = entry,
                None => {
                    // after the section's last setting, before any blank lines that end it
                    let mut at = end;
                    while at > start + 1 && lines[at - 1].trim().is_empty() {
                        at -= 1;
                    }
                    lines.insert(at, entry);
                }
            }
        }
        None => {
            if lines.is_empty() {
                lines.extend(HEADER.split("\r\n").map(str::to_string));
            }
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.push(entry);
        }
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, lines.join("\r\n") + "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game's place is written, read back, replaced, and kept beside another game's; a file
    /// that has other lines keeps them.
    #[test]
    fn places_are_kept_per_game() {
        let dir = std::env::temp_dir().join(format!("vpi-score-window-{}", std::process::id()));
        let file = dir.join("Score windows.ini");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(remembered(&file, "afm_113b"), None);
        write_place(&file, "afm_113b", 12, -40).expect("written");
        write_place(&file, "pb_l5", 0, 600).expect("written");
        assert_eq!(remembered(&file, "afm_113b"), Some((12, -40)));
        assert_eq!(remembered(&file, "AFM_113B"), Some((12, -40)));
        write_place(&file, "afm_113b", 300, 5).expect("written");
        assert_eq!((remembered(&file, "afm_113b"), remembered(&file, "pb_l5")), (Some((300, 5)), Some((0, 600))));
        let text = std::fs::read_to_string(&file).expect("read");
        assert!(text.starts_with("; Where the floating score window"));
        assert_eq!(text.matches("afm_113b").count(), 1);
        // another section and a comment stay as they were
        std::fs::write(&file, text.clone() + "\r\n[Other]\r\n; a note\r\nkeep = 1\r\n").expect("written");
        write_place(&file, "tz_92", 1, 2).expect("written");
        let text = std::fs::read_to_string(&file).expect("read");
        assert!(text.contains("[Other]\r\n; a note\r\nkeep = 1"));
        assert_eq!(remembered(&file, "tz_92"), Some((1, 2)));
        assert!(text.find("tz_92").expect("there") < text.find("[Other]").expect("there"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
