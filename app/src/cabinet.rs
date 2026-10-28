//! Cabinet mode: the app on the cabinet's screens — its own window full screen where the settings
//! put the player's playfield, and windows of its own where they put the backglass, the score
//! display and a topper (`screens::load`), each showing the table chosen; closed again, the app as
//! it was. While a table plays, the other screens step aside for the player's own. The app can
//! start in it, and start with Windows (the user's Run key, `HKCU\…\Run`).

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

/// Where the app's window was before cabinet mode: its place, its size, full screen, maximised.
#[derive(Default)]
pub struct Before(pub Mutex<Option<(PhysicalPosition<i32>, PhysicalSize<u32>, bool, bool)>>);

/// What the cabinet's other screens show now, for a window opened after it was told.
#[derive(Default)]
pub struct Showing(pub Mutex<serde_json::Value>);

/// The player's windows the cabinet's other screens stand in for, and their windows' labels.
const SCREENS: [(&str, &str); 3] = [("backglass", "cabinet-backglass"), ("scoreView", "cabinet-dmd"), ("topper", "cabinet-topper")];

/// Cabinet mode opened: the app's window full screen on the playfield's screen, the other screens'
/// windows where the settings put the player's. The other screens opened, by the player's names.
pub fn open(app: &AppHandle, preset: Option<&str>) -> Result<Vec<&'static str>, String> {
    let mut view = crate::screens::load()?;
    // the screens a preset gives, where cabinet mode has one of its own
    if let Some(p) = preset.filter(|p| !p.trim().is_empty()) {
        view.settings.windows = crate::screens::load_preset(p)?.windows;
    }
    let main = app.get_webview_window("main").ok_or("The app's window isn't there.")?;
    {
        let state = app.state::<Before>();
        let mut before = state.0.lock().unwrap();
        if before.is_none() {
            let (pos, size) = (main.outer_position().map_err(|e| e.to_string())?, main.outer_size().map_err(|e| e.to_string())?);
            *before = Some((pos, size, main.is_fullscreen().unwrap_or(false), main.is_maximized().unwrap_or(false)));
        }
    }
    let screen_of = |w: &crate::screens::WindowPlace| view.screens.iter().find(|s| s.name == w.screen).or_else(|| view.screens.first());
    if let Some(s) = view.settings.windows.iter().find(|w| w.id == "playfield").and_then(|w| screen_of(w)) {
        let _ = main.set_fullscreen(false);
        let _ = main.unmaximize();
        let _ = main.set_position(PhysicalPosition::new(s.x, s.y));
    }
    let _ = main.set_fullscreen(true);
    focus(&main);
    let mut opened = Vec::new();
    for (id, label) in SCREENS {
        if let Some(old) = app.get_webview_window(label) {
            let _ = old.close();
        }
        let Some(w) = view.settings.windows.iter().find(|w| w.id == id && w.output == "window") else { continue };
        let Some(s) = screen_of(w) else { continue };
        let (x, y, width, height) = if w.fullscreen { (s.x, s.y, s.width, s.height) } else { (s.x + w.x, s.y + w.y, w.width, w.height) };
        let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html#cabinet&screen={id}").into()))
            .title("Cabinet mode")
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .focused(false)
            // shown, they'd take the keyboard from the main window: they never may
            .focusable(false)
            .visible(false)
            .background_color(tauri::webview::Color(0, 0, 0, 255));
        let window = crate::screens::made_at(builder, app, x, y, width, height).build().map_err(|e| e.to_string())?;
        crate::screens::place_and_show(&window, x, y, width, height);
        opened.push(id);
    }
    // the keyboard the main window's, the others up
    focus(&main);
    Ok(opened)
}

/// The window brought to the front with the keyboard's focus in its page: Windows gives a window
/// focus, but the web view inside it takes the keys only once focused itself.
pub fn focus(window: &tauri::WebviewWindow) {
    let _ = window.set_focus();
    let page: &tauri::Webview = window.as_ref();
    let _ = page.set_focus();
}

/// Cabinet mode closed: its windows gone, the app's window as it was. Whether it's full screen now,
/// for the screens to know.
pub fn close(app: &AppHandle) -> bool {
    for (_, label) in SCREENS {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.close();
        }
    }
    let Some(main) = app.get_webview_window("main") else { return false };
    let _ = main.set_fullscreen(false);
    if let Some((pos, size, full, max)) = app.state::<Before>().0.lock().unwrap().take() {
        let _ = main.set_position(pos);
        let _ = main.set_size(size);
        if max {
            let _ = main.maximize();
        }
        if full {
            let _ = main.set_fullscreen(true);
        }
    }
    main.is_fullscreen().unwrap_or(false)
}

/// What the other screens show, told to them and kept for any opened after.
pub fn show(app: &AppHandle, what: serde_json::Value) {
    *app.state::<Showing>().0.lock().unwrap() = what.clone();
    let _ = app.emit("cabinet-show", what);
}

/// The other screens shown again, or put away while a table plays.
pub fn screens_shown(app: &AppHandle, shown: bool) {
    for (_, label) in SCREENS {
        if let Some(w) = app.get_webview_window(label) {
            let _ = if shown { w.show() } else { w.hide() };
            // shown again, its page made to take its size a moment later
            if shown {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    crate::screens::refit(&w);
                });
            }
        }
    }
    // back from a game: the keyboard the main window's again
    if shown && let Some(main) = app.get_webview_window("main") {
        focus(&main);
    }
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "Visual Pinball Infinite";

fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
    let mut cmd = std::process::Command::new("reg.exe");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // no console window
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output()
}

/// Whether the app starts with Windows: its line in the user's Run key.
pub fn starts_with_windows() -> bool {
    reg(&["query", RUN_KEY, "/v", RUN_NAME]).is_ok_and(|o| o.status.success())
}

/// The app started with Windows, or no longer: its line in the user's Run key added or taken out.
pub fn start_with_windows(on: bool) -> Result<(), String> {
    let out = if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        reg(&["add", RUN_KEY, "/v", RUN_NAME, "/t", "REG_SZ", "/d", &format!("\"{}\"", exe.display()), "/f"])
    } else {
        reg(&["delete", RUN_KEY, "/v", RUN_NAME, "/f"])
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() || (!on && !starts_with_windows()) {
        Ok(())
    } else {
        Err(format!("Windows wouldn't change it: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}
