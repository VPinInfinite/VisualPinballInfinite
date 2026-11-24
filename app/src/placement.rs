//! The app's window where the user left it: its place and size followed as it moves and changes
//! size — the last before it was maximized, and whether it was — written when it closes, and put
//! back as it opens, where a screen still shows it (a screen taken away, it opens in the middle of
//! the main one). Full screen is kept apart, as the preferences' `fullscreen`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{PhysicalPosition, PhysicalSize, WindowEvent};

/// Its outer corner and inner size in the screens' pixels, and whether it was maximized.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Placement {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

static NOW: Mutex<Option<Placement>> = Mutex::new(None);

/// Not followed: the test mode's window, off every screen.
static OFF: AtomicBool = AtomicBool::new(false);

pub fn off() {
    OFF.store(true, Ordering::Relaxed);
}

fn file() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Library").join("window.json"))
}

/// The window's place followed as it moves and changes size; minimized or full screen, the last
/// kept.
pub fn follow(window: &tauri::Window, event: &WindowEvent) {
    if OFF.load(Ordering::Relaxed) || !matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
        return;
    }
    if window.is_minimized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
        return;
    }
    let maximized = window.is_maximized().unwrap_or(false);
    let mut now = NOW.lock().unwrap();
    let mut p = now.unwrap_or_default();
    p.maximized = maximized;
    if !maximized
        && let (Ok(at), Ok(size)) = (window.outer_position(), window.inner_size())
    {
        (p.x, p.y, p.width, p.height) = (at.x, at.y, size.width, size.height);
    }
    *now = Some(p);
}

/// Where the window was left, written as it closes.
pub fn keep() {
    if OFF.load(Ordering::Relaxed) {
        return;
    }
    let (Some(p), Some(file)) = (*NOW.lock().unwrap(), file()) else { return };
    if let Ok(text) = serde_json::to_vec_pretty(&p) {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&file, text) {
            eprintln!("{}: the window's place wasn't kept: {e}", file.display());
        }
    }
}

/// The window put where it was left, where a screen still shows its middle: true where it was.
pub fn put_back(window: &tauri::WebviewWindow) -> bool {
    let Some(p) = file().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice::<Placement>(&b).ok()) else { return false };
    if p.width < 300 || p.height < 200 {
        return false;
    }
    let (mx, my) = (p.x + (p.width / 2) as i32, p.y + (p.height / 2) as i32);
    let shown = window.available_monitors().unwrap_or_default().iter().any(|m| {
        let (at, size) = (m.position(), m.size());
        mx >= at.x && mx < at.x + size.width as i32 && my >= at.y && my < at.y + size.height as i32
    });
    if !shown {
        return false;
    }
    let _ = window.set_position(PhysicalPosition::new(p.x, p.y));
    let _ = window.set_size(PhysicalSize::new(p.width, p.height));
    if p.maximized {
        let _ = window.maximize();
    }
    *NOW.lock().unwrap() = Some(p);
    true
}
