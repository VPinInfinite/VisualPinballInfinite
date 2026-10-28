//! The Screens page's side: the screens as our player sees them (`vpi-player
//! --displays --json`, so their names are the ones its settings take), where the settings file
//! puts each of the player's windows — by the player's own rules, `vpi_settings::place` — and the
//! page's changes written back into that file in place, the file before them kept beside it as
//! `Settings.previous.ini`. Two helpers on the real screens: each screen's name shown on it
//! (Identify), and the player's windows stood in for by windows of the app's own, to be dragged
//! and sized where they should go (Arrange).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use vpi_settings::{Change, Display, Output, WindowId};

/// A screen, as the player lists it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    /// As `vpi-player --displays` numbers them, from 1.
    pub number: u32,
    pub name: String,
    /// Its place on the desktop, in SDL's units (pixels on Windows).
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Pixels per unit.
    pub density: f32,
    /// Windows' scale for it, in percent.
    pub scale: f32,
    pub refresh: f32,
    /// Its picture's physical size, the long side first, when it reports one.
    pub size_cm: Option<[f32; 2]>,
    pub hdr: bool,
    pub primary: bool,
}

impl Screen {
    fn display(&self) -> Display {
        Display {
            name: self.name.clone(),
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            density: self.density,
            primary: self.primary,
            size_cm: self.size_cm.map(|[a, b]| (a, b)),
        }
    }
}

pub(crate) fn player() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let player = exe.with_file_name("vpi-player.exe");
    if player.exists() { Ok(player) } else { Err(format!("The player isn't built yet: {}", player.display())) }
}

/// The screens, as the player sees them.
pub fn screens() -> Result<Vec<Screen>, String> {
    let mut command = Command::new(player()?);
    command.args(["--displays", "--json"]).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the player is a console program; no console shows
        command.creation_flags(0x0800_0000);
    }
    let out = command.output().map_err(|e| format!("The player couldn't list the screens: {e}"))?;
    if !out.status.success() {
        return Err("The player couldn't list the screens.".into());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("The player's list of screens couldn't be read: {e}"))
}

/// The player's ways of handing frames to the screen, as `[VPI]` `Present` names them.
const FRAMES: [&str; 4] = ["immediate", "mailbox", "vsync", "paced"];

fn id_name(id: WindowId) -> &'static str {
    match id {
        WindowId::Playfield => "playfield",
        WindowId::Backglass => "backglass",
        WindowId::ScoreView => "scoreView",
        WindowId::Topper => "topper",
    }
}

fn id_of(name: &str) -> Option<WindowId> {
    WindowId::ALL.into_iter().find(|&id| id_name(id) == name)
}

/// One of the player's windows: how it shows and where, in its screen's pixels.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowPlace {
    /// `playfield`, `backglass`, `scoreView` or `topper`.
    pub id: String,
    /// `window` (one of its own), `inside` (drawn inside the playfield's window) or `off`.
    pub output: String,
    /// The screen it's on, by name.
    pub screen: String,
    /// Borderless and covering its screen.
    pub fullscreen: bool,
    /// Where it sits in its screen, and its size, in the screen's pixels.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// The page's settings: the windows, and the rest of what decides what shows where.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenSettings {
    pub windows: Vec<WindowPlace>,
    /// The table's view (VPX's `[Player]` `BGSet`): `desktop`, `cabinet` or `fss` (a full
    /// single screen).
    pub view: String,
    /// The score display floating over the table on one screen, for ROM tables whose script
    /// shows PinMAME's own window (`[VPI]` `FloatingScore`, on unless set off).
    pub floating_score: bool,
    /// The DMD over the backglass's picture (`[Plugin.B2S]` `BackglassDMDOverlay` and
    /// `…AutoPos`): `off`, `auto` (found in its picture) or `placed` (where the settings say).
    pub backglass_dmd: String,
    /// The DMD over the backglass file's score panel, in the score window (`ScoreViewDMD…`).
    pub score_panel_dmd: String,
    /// The backglass picture's speaker grill shown (`[Plugin.B2S]` `ShowGrill`).
    pub show_grill: bool,
    /// How frames reach the screen, the player's own choices: `immediate` (as fast as they
    /// come, may tear), `mailbox` (tear-free and quick, ours: `[VPI]` `Present`), `paced` (each
    /// drawn just before the screen shows it), `vsync` (the screen's own pace; VPX's adaptive
    /// sync too, which SDL's graphics play as it) — VPX's `[Player]` `SyncMode` for all but
    /// ours — or `default`, the player's own, as fast as they come.
    pub frames: String,
    /// The most frames a second (`[Player]` `MaxFramerate`): -1 the screen's refresh rate, 0 no
    /// limit; None the player's own default, 240.
    pub max_framerate: Option<f32>,
}

/// What the page shows: the screens, the settings, and where they're kept.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreensView {
    pub screens: Vec<Screen>,
    pub settings: ScreenSettings,
    /// Which windows the settings file says anything of; the rest are the player's defaults.
    pub set: Vec<String>,
    /// Windows the settings put on a screen that isn't connected, by window, with that screen's
    /// name: the player puts them on the main screen meanwhile, and so does the page.
    pub missing: BTreeMap<String, String>,
    pub file: String,
    pub exists: bool,
}

/// The DMD over a backglass file's picture, as the player reads it (`vpx_settings::b2s_dmd_overlay`):
/// none of its settings there, the player's own default — over the score panel found by itself,
/// over the backglass off.
fn overlay(entries: &[(String, String, String)], window: &str) -> String {
    let key = |k: &str| format!("{window}DMD{k}");
    if ["Overlay", "AutoPos", "X", "Y", "W", "H"].iter().all(|k| vpi_settings::get(entries, "Plugin.B2S", &key(k)).is_none()) {
        return if window == "ScoreView" { "auto" } else { "off" }.into();
    }
    let flag = |k: &str| vpi_settings::get(entries, "Plugin.B2S", &key(k)).and_then(vpi_settings::vpx_bool).unwrap_or(false);
    match (flag("Overlay"), flag("AutoPos")) {
        (true, true) => "auto",
        (true, false) => "placed",
        _ => "off",
    }
    .into()
}

/// The page's view of the settings file as it stands.
pub fn load() -> Result<ScreensView, String> {
    let screens = screens()?;
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let text = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned());
    let exists = text.is_ok();
    let entries = vpi_settings::parse_ini(&text.unwrap_or_default());
    let settings = settings_of(&entries, &screens);
    let set = WindowId::ALL.into_iter().filter(|&id| vpi_settings::has_window(&entries, id)).map(|id| id_name(id).to_string()).collect();
    let missing = WindowId::ALL
        .into_iter()
        .filter_map(|id| {
            let ws = vpi_settings::window(&entries, id);
            (ws.output != Output::Disabled && !ws.display.is_empty() && !screens.iter().any(|s| s.name == ws.display)).then(|| (id_name(id).to_string(), ws.display))
        })
        .collect();
    Ok(ScreensView { screens, settings, set, missing, file: path.display().to_string(), exists })
}

/// What the settings say, by the player's own rules, its defaults for what they leave out.
fn settings_of(entries: &[(String, String, String)], screens: &[Screen]) -> ScreenSettings {
    let displays: Vec<Display> = screens.iter().map(Screen::display).collect();
    let mut windows: Vec<WindowPlace> = Vec::new();
    for id in WindowId::ALL {
        let ws = vpi_settings::window(entries, id);
        // drawn inside the playfield's window: its place in that window's pixels, as the player
        // takes it (VPX 5436's embedded output), held inside the window
        if ws.output == Output::Embedded
            && let Some(pf) = windows.first().cloned()
        {
            let width = ws.width.clamp(1, pf.width);
            let height = ws.height.clamp(1, pf.height);
            windows.push(WindowPlace {
                id: id_name(id).into(),
                output: "inside".into(),
                screen: pf.screen.clone(),
                fullscreen: false,
                x: ws.x.unwrap_or(0).clamp(0, pf.width - width),
                y: ws.y.unwrap_or(0).clamp(0, pf.height - height),
                width,
                height,
            });
            continue;
        }
        let Some(p) = vpi_settings::place(&ws, &displays) else { continue };
        let d = vpi_settings::pick_display(&p.display, &displays).expect("the screen it was placed on");
        let density = if d.density > 0.0 { d.density } else { 1.0 };
        let px = |units: i32| (units as f32 * density).round() as i32;
        windows.push(WindowPlace {
            id: id_name(id).into(),
            output: match ws.output {
                Output::Floating => "window",
                Output::Embedded => "inside",
                Output::Disabled => "off",
            }
            .into(),
            screen: p.display.clone(),
            fullscreen: p.fullscreen,
            x: px(p.x - d.x),
            y: px(p.y - d.y),
            width: px(p.width),
            height: px(p.height),
        });
    }
    let get = |s: &str, n: &str| vpi_settings::get(entries, s, n);
    ScreenSettings {
        windows,
        view: match vpi_settings::int(entries, "Player", "BGSet", 0, 2) {
            Some(1) => "cabinet",
            Some(2) => "fss",
            _ => "desktop",
        }
        .into(),
        floating_score: get("VPI", "FloatingScore").and_then(vpi_settings::vpx_bool).unwrap_or(true),
        backglass_dmd: overlay(entries, "Backglass"),
        score_panel_dmd: overlay(entries, "ScoreView"),
        show_grill: get("Plugin.B2S", "ShowGrill").and_then(vpi_settings::vpx_bool).or_else(|| get("Plugin.B2SLegacy", "B2SHideGrill").and_then(vpi_settings::vpx_bool).map(|hide| !hide)).unwrap_or(false),
        frames: match get("VPI", "Present").map(|v| vpi_settings::vpx_trim(v).to_ascii_lowercase()) {
            Some(p) if FRAMES.contains(&p.as_str()) => p,
            _ => match vpi_settings::int(entries, "Player", "SyncMode", 0, 3) {
                Some(0) => "immediate",
                Some(1 | 2) => "vsync",
                Some(3) => "paced",
                _ => "default",
            }
            .into(),
        },
        max_framerate: get("Player", "MaxFramerate").and_then(vpi_settings::vpx_float).map(|v| v.clamp(-1.0, 1000.0)),
    }
}

/// The settings file's changes from `before` (what `entries`, the file, says) to `after`: only
/// what the page changed is written, a window's settings only where the file doesn't already
/// say them, so what the file leaves to the player's defaults stays left to them. `base`: the
/// files under it (a table's own file: the main one and its preset), whose values a key the file
/// doesn't say holds. `full`: everything written, as a preset keeps it, whatever it was.
fn changes(entries: &[(String, String, String)], base: &[(String, String, String)], before: &ScreenSettings, after: &ScreenSettings, screens: &[Screen], full: bool) -> Result<Vec<Change>, String> {
    let mut out = Vec::new();
    for w in &after.windows {
        if !full && before.windows.contains(w) {
            continue;
        }
        let id = id_of(&w.id).ok_or_else(|| format!("There's no window called {}.", w.id))?;
        let (section, prefix) = id.names();
        // a setting written only when the file doesn't already say it (unset counts as its default)
        let mut put = |name: &str, value: String, default: Option<&str>| {
            let name = format!("{prefix}{name}");
            let equal = |now: &str| now.trim() == value || vpi_settings::vpx_float(now).is_some_and(|n| Some(n) == vpi_settings::vpx_float(&value));
            let same = !full
                && match vpi_settings::get(entries, section, &name) {
                    Some(now) => equal(now),
                    None => match vpi_settings::get(base, section, &name) {
                        Some(under) => equal(under),
                        None => default == Some(value.as_str()),
                    },
                };
            if !same {
                out.push(Change::set(section, &name, value));
            }
        };
        if id != WindowId::Playfield {
            let output = match w.output.as_str() {
                "window" => 1,
                "inside" => 2,
                _ => 0,
            };
            put("Output", output.to_string(), Some("0"));
            if output == 0 {
                continue;
            }
            // inside the playfield's window: its place in that window's pixels
            if output == 2 {
                put("WndX", w.x.max(0).to_string(), None);
                put("WndY", w.y.max(0).to_string(), None);
                put("Width", w.width.max(1).to_string(), Some("16384"));
                put("Height", w.height.max(1).to_string(), Some("16384"));
                continue;
            }
        }
        let screen = screens.iter().find(|s| s.name == w.screen).ok_or_else(|| format!("The screen {} isn't connected.", w.screen))?;
        // held inside its screen, as the player would hold it
        let width = w.width.clamp(1, screen.width);
        let height = w.height.clamp(1, screen.height);
        let x = w.x.clamp(0, (screen.width - width).max(0));
        let y = w.y.clamp(0, (screen.height - height).max(0));
        put("Display", screen.name.clone(), None);
        put("FullScreen", i32::from(w.fullscreen).to_string(), Some("0"));
        put("WndX", x.to_string(), None);
        put("WndY", y.to_string(), None);
        put("Width", width.to_string(), Some("16384"));
        put("Height", height.to_string(), Some("16384"));
    }
    if full || after.view != before.view {
        let view = match after.view.as_str() {
            "cabinet" => 1,
            "fss" => 2,
            _ => 0,
        };
        out.push(Change::set("Player", "BGSet", view));
    }
    if full || after.floating_score != before.floating_score {
        out.push(Change::set("VPI", "FloatingScore", i32::from(after.floating_score)));
    }
    for (window, choice, was) in [("Backglass", &after.backglass_dmd, &before.backglass_dmd), ("ScoreView", &after.score_panel_dmd, &before.score_panel_dmd)] {
        if !full && choice == was {
            continue;
        }
        match choice.as_str() {
            "auto" => {
                out.push(Change::set("Plugin.B2S", &format!("{window}DMDOverlay"), 1));
                out.push(Change::set("Plugin.B2S", &format!("{window}DMDAutoPos"), 1));
            }
            // placed by hand: its place stays as the settings file has it
            "placed" => {
                out.push(Change::set("Plugin.B2S", &format!("{window}DMDOverlay"), 1));
                out.push(Change::set("Plugin.B2S", &format!("{window}DMDAutoPos"), 0));
            }
            _ => out.push(Change::set("Plugin.B2S", &format!("{window}DMDOverlay"), 0)),
        }
    }
    if full || after.show_grill != before.show_grill {
        out.push(Change::set("Plugin.B2S", "ShowGrill", i32::from(after.show_grill)));
    }
    // back to unset: the player's own default; a preset keeps no display timing
    if !full && after.frames != before.frames {
        // VPX's sync mode for its own choices, ours for the one it hasn't; the player's default: neither
        let vpx = match after.frames.as_str() {
            "immediate" => Some(0),
            "vsync" => Some(1),
            "paced" => Some(3),
            _ => None,
        };
        if after.frames == "mailbox" {
            out.push(Change::set("VPI", "Present", "mailbox"));
        } else {
            out.push(Change::remove("VPI", "Present"));
            out.push(match vpx {
                Some(mode) => Change::set("Player", "SyncMode", mode),
                None => Change::remove("Player", "SyncMode"),
            });
        }
    }
    if !full && after.max_framerate != before.max_framerate {
        out.push(match after.max_framerate {
            Some(fps) => Change::set("Player", "MaxFramerate", fps.clamp(-1.0, 1000.0).round() as i32),
            None => Change::remove("Player", "MaxFramerate"),
        });
    }
    Ok(out)
}

/// The page's settings written into the settings file, the file before them kept as
/// `Settings.previous.ini`.
pub fn save(settings: &ScreenSettings) -> Result<(), String> {
    let screens = screens()?;
    let path = vpi_settings::settings_path().ok_or("no APPDATA folder")?;
    let before = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let entries = vpi_settings::parse_ini(&before);
    let was = settings_of(&entries, &screens);
    let after = vpi_settings::edit(&before, &changes(&entries, &[], &was, settings, &screens, false)?);
    write_settings(&path, &before, &after)
}

fn read_text(path: &std::path::Path) -> String {
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// What a table's own screens stand on: the main settings, then the preset it (or they) name.
fn table_base(table: &std::path::Path) -> Vec<(String, String, String)> {
    let mut base = vpi_settings::settings_path().map(|p| vpi_settings::parse_ini(&read_text(&p))).unwrap_or_default();
    let own = vpi_settings::parse_ini(&read_text(&table.with_extension("ini")));
    if let Some((_, text)) = vpi_settings::preset_named(&own, &base) {
        base.extend(vpi_settings::parse_ini(&text));
    }
    base
}

/// The page for one table alone (its settings' View, Custom): the screens as
/// the player places them for it — the main settings, its preset, its own file over them.
pub fn load_for_table(table: &std::path::Path) -> Result<ScreensView, String> {
    let screens = screens()?;
    let file = table.with_extension("ini");
    let mut entries = table_base(table);
    entries.extend(vpi_settings::parse_ini(&read_text(&file)));
    let settings = settings_of(&entries, &screens);
    Ok(ScreensView { screens, settings, set: Vec::new(), missing: BTreeMap::new(), file: file.display().to_string(), exists: file.exists() })
}

/// The page's settings for one table written into its own file, only what differs from what it
/// stands on (the main settings and its preset).
pub fn save_for_table(table: &std::path::Path, settings: &ScreenSettings) -> Result<(), String> {
    let screens = screens()?;
    let file = table.with_extension("ini");
    let before = read_text(&file);
    let entries = vpi_settings::parse_ini(&before);
    let base = table_base(table);
    let was = settings_of(&[base.clone(), entries.clone()].concat(), &screens);
    let mut changes = changes(&entries, &base, &was, settings, &screens, false)?;
    if !changes.is_empty() {
        // a file made for the first time takes in the POV file's views, which it ends the reading of
        changes.splice(0..0, vpi_settings::first_table_settings(table));
    }
    let after = vpi_settings::edit(&before, &changes);
    if after == before {
        return Ok(());
    }
    let new = file.with_extension("ini.new");
    std::fs::write(&new, after).and_then(|_| std::fs::rename(&new, &file)).map_err(|e| format!("The table's settings couldn't be saved: {e}"))
}

/// The screen presets kept, by name.
pub fn presets() -> Vec<String> {
    let Some(dir) = vpi_settings::preset_path("x").and_then(|p| p.parent().map(std::path::Path::to_path_buf)) else { return Vec::new() };
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().strip_suffix(".ini").map(str::to_string))
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// The page's settings kept as a preset (Save as preset): every window's place and the score
/// display's choices, whatever the main settings say, so it holds over them.
pub fn save_preset(name: &str, settings: &ScreenSettings) -> Result<(), String> {
    let path = vpi_settings::preset_path(name).ok_or("That name can't be a preset's: no \\ / : * ? \" < > |.")?;
    let screens = screens()?;
    let changes = changes(&[], &[], settings, settings, &screens, true)?;
    let text = format!("; Visual Pinball Infinite's screen preset \"{}\"\r\n{}", name.trim(), vpi_settings::edit("", &changes));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("The presets' folder couldn't be made: {e}"))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("The preset couldn't be saved: {e}"))
}

/// A preset as the page shows it: over the main settings.
pub fn load_preset(name: &str) -> Result<ScreenSettings, String> {
    let path = vpi_settings::preset_path(name).ok_or("No preset by that name.")?;
    let text = std::fs::read(&path).map_err(|_| "No preset by that name.".to_string())?;
    let mut entries = vpi_settings::settings_path().map(|p| vpi_settings::parse_ini(&read_text(&p))).unwrap_or_default();
    entries.extend(vpi_settings::parse_ini(&String::from_utf8_lossy(&text)));
    Ok(settings_of(&entries, &screens()?))
}

/// A preset renamed, or deleted (`to` none: to the Recycle Bin).
pub fn change_preset(name: &str, to: Option<&str>) -> Result<(), String> {
    let path = vpi_settings::preset_path(name).ok_or("No preset by that name.")?;
    match to {
        Some(to) => {
            let new = vpi_settings::preset_path(to).ok_or("That name can't be a preset's.")?;
            if new.exists() && !new.to_string_lossy().eq_ignore_ascii_case(&path.to_string_lossy()) {
                return Err(format!("A preset called {to} is there already."));
            }
            std::fs::rename(&path, &new).map_err(|e| format!("The preset couldn't be renamed: {e}"))
        }
        None => ::library::install::to_recycle_bin(&path),
    }
}

/// The settings file written as `after`, if that differs from what it was (`before`): the file
/// before kept as `Settings.previous.ini`, the new one written beside it and moved into place, so a
/// failed write leaves the old one. For every settings page.
pub(crate) fn write_settings(path: &std::path::Path, before: &str, after: &str) -> Result<(), String> {
    if after == before {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("The settings folder couldn't be made: {e}"))?;
    }
    if !before.is_empty() {
        std::fs::write(path.with_file_name("Settings.previous.ini"), before).map_err(|e| format!("The settings couldn't be kept before changing them: {e}"))?;
    }
    let new = path.with_extension("ini.new");
    std::fs::write(&new, after).map_err(|e| format!("The settings couldn't be written: {e}"))?;
    std::fs::rename(&new, path).map_err(|e| format!("The settings couldn't be written: {e}"))
}

/// A helper window's inside put at `x`, `y` on the desktop, `width` by `height` pixels: moved onto
/// its screen first, as a window moved between screens of different scales is resized by Windows
/// on the way (a stand-in sized on a 125% screen arrived on a 100% ultrawide at four fifths of its
/// size), then sized, then moved by its frame's
/// invisible edges, which Windows counts in a window's place. Twice: once hidden, once shown, as a
/// hidden window may be rescaled only when it shows.
fn place(window: &tauri::WebviewWindow, x: i32, y: i32, width: i32, height: i32) {
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(width.max(1) as u32, height.max(1) as u32));
    if let (Ok(outer), Ok(inner)) = (window.outer_position(), window.inner_position()) {
        let _ = window.set_position(PhysicalPosition::new(x - (inner.x - outer.x), y - (inner.y - outer.y)));
    }
}

pub(crate) fn place_and_show(window: &tauri::WebviewWindow, x: i32, y: i32, width: i32, height: i32) {
    place(window, x, y, width, height);
    let _ = window.show();
    place(window, x, y, width, height);
    // a moment after it shows, placed once more and the page inside made to take its size
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        place(&window, x, y, width, height);
        refit(&window);
    });
}

/// The page inside a window made to take the window's size again: one pixel narrower and back, as
/// a size the window has already makes no resize for the page to follow.
pub(crate) fn refit(window: &tauri::WebviewWindow) {
    if let Ok(size) = window.inner_size() {
        let _ = window.set_size(PhysicalSize::new(size.width.saturating_sub(1).max(1), size.height.max(1)));
        let _ = window.set_size(size);
    }
}

/// A window made where it goes, at its size (`x`, `y`, `width`, `height`: desktop pixels): given
/// in the units of the screen holding that place, Windows makes it there, so it's never moved
/// across screens of another scale — a window made on a screen at 125% and moved to one at 100%
/// showed its page smaller than itself, cropped in a corner (cabinet mode's score display and
/// topper beside a playfield on the main screen).
pub(crate) fn made_at<'a, R: tauri::Runtime, M: Manager<R>>(builder: WebviewWindowBuilder<'a, R, M>, app: &AppHandle<R>, x: i32, y: i32, width: i32, height: i32) -> WebviewWindowBuilder<'a, R, M> {
    let scale = app
        .available_monitors()
        .ok()
        .and_then(|monitors| {
            monitors
                .into_iter()
                .find(|m| {
                    let (p, s) = (m.position(), m.size());
                    x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
                })
                .map(|m| m.scale_factor())
        })
        .unwrap_or(1.0);
    builder.position(f64::from(x) / scale, f64::from(y) / scale).inner_size(f64::from(width.max(1)) / scale, f64::from(height.max(1)) / scale)
}

/// Each screen's number and name shown on it for a few seconds.
pub fn identify(app: &AppHandle, jobs: &BTreeMap<String, String>) -> Result<(), String> {
    for s in screens()? {
        let label = format!("identify-{}", s.number);
        if let Some(old) = app.get_webview_window(&label) {
            let _ = old.close();
        }
        let job = jobs.get(&s.name).map(String::as_str).unwrap_or("");
        let hash = format!("identify&n={}&name={}&job={}&size={}x{}", s.number, encode(&s.name), encode(job), s.width, s.height);
        // a card in the middle of the screen, a third of its width
        let (w, h) = ((s.width / 3).max(420), (s.height / 4).max(240));
        let (x, y) = (s.x + (s.width - w) / 2, s.y + (s.height - h) / 2);
        let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(format!("index.html#{hash}").into()))
            .title("Identify")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .visible(false)
            .background_color(tauri::webview::Color(13, 16, 22, 255));
        let window = made_at(builder, app, x, y, w, h).build().map_err(|e| e.to_string())?;
        place_and_show(&window, x, y, w, h);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(5));
            let _ = window.close();
        });
    }
    Ok(())
}

/// The windows standing in for the player's while they're arranged, by label, each with the shape
/// it keeps, if it keeps one.
#[derive(Default)]
pub struct Arranging(pub Mutex<Vec<(String, Option<f64>)>>);

/// Windows of the app's own where the player's windows go, each named, to be dragged and sized
/// on the real screens; those in `keep` keep that shape, width over height, as they're sized.
pub fn arrange(app: &AppHandle, windows: &[WindowPlace], keep: &BTreeMap<String, f64>) -> Result<(), String> {
    finish_arranging(app);
    let screens = screens()?;
    let mut labels = Vec::new();
    for w in windows.iter().filter(|w| w.output == "window" || w.id == "playfield") {
        let Some(s) = screens.iter().find(|s| s.name == w.screen) else { continue };
        let label = format!("arrange-{}", w.id);
        let shape = keep.get(&w.id).copied().filter(|v| v.is_finite() && *v > 0.0);
        let hash = format!("index.html#arrange&id={}{}", w.id, if shape.is_some() { "&keep=1" } else { "" });
        // as small as a stand-in gets, in its shape
        let (width, height) = match shape {
            Some(shape) if w.width < 160 || w.height < 120 => {
                let f = (160.0 / f64::from(w.width.max(1))).max(120.0 / f64::from(w.height.max(1)));
                ((f64::from(w.width) * f).round() as i32, (f64::from(w.width) * f / shape).round() as i32)
            }
            _ => (w.width.max(160), w.height.max(120)),
        };
        let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(hash.into()))
            .title("Arrange")
            .decorations(false)
            .resizable(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .visible(false)
            .background_color(tauri::webview::Color(13, 16, 22, 255));
        let window = made_at(builder, app, s.x + w.x, s.y + w.y, width, height).build().map_err(|e| e.to_string())?;
        place_and_show(&window, s.x + w.x, s.y + w.y, width, height);
        if let Some(shape) = shape {
            crate::keep_shape::keep(&window, shape);
        }
        labels.push((label, shape));
    }
    *app.state::<Arranging>().0.lock().unwrap() = labels;
    Ok(())
}

/// Where the stand-in windows were left: each on the screen holding most of it, held inside it,
/// its edges snapped to the screen's within a few pixels — one keeping its shape moved to them,
/// not sized — covering it whole made fullscreen. The stand-ins closed.
pub fn arranged(app: &AppHandle, windows: &[WindowPlace]) -> Result<Vec<WindowPlace>, String> {
    let screens = screens()?;
    let shapes: BTreeMap<String, Option<f64>> = app.state::<Arranging>().0.lock().unwrap().iter().cloned().collect();
    let mut out = windows.to_vec();
    for w in &mut out {
        let label = format!("arrange-{}", w.id);
        let Some(window) = app.get_webview_window(&label) else { continue };
        // its inside, not its frame's invisible edges
        let (Ok(pos), Ok(size)) = (window.inner_position(), window.inner_size()) else { continue };
        let (x, y, width, height) = (pos.x, pos.y, size.width as i32, size.height as i32);
        // the screen holding most of it
        let overlap = |s: &Screen| {
            let w = (x + width).min(s.x + s.width) - x.max(s.x);
            let h = (y + height).min(s.y + s.height) - y.max(s.y);
            i64::from(w.max(0)) * i64::from(h.max(0))
        };
        let Some(s) = screens.iter().max_by_key(|s| overlap(s)) else { continue };
        w.screen = s.name.clone();
        if shapes.get(&label).copied().flatten().is_some() {
            *w = kept_in(w, s, x - s.x, y - s.y, width, height);
            continue;
        }
        let snap = |v: i32, edge: i32| if (v - edge).abs() <= 16 { edge } else { v };
        let mut left = snap(x - s.x, 0).max(0);
        let mut top = snap(y - s.y, 0).max(0);
        let right = snap(x - s.x + width, s.width).min(s.width);
        let bottom = snap(y - s.y + height, s.height).min(s.height);
        left = left.min(right - 1);
        top = top.min(bottom - 1);
        w.x = left;
        w.y = top;
        w.width = right - left;
        w.height = bottom - top;
        w.fullscreen = w.x == 0 && w.y == 0 && w.width == s.width && w.height == s.height;
    }
    finish_arranging(app);
    Ok(out)
}

/// A stand-in that kept its shape, left at `x`, `y` on screen `s`, `width` by `height`: no
/// bigger than the screen, the shape kept; moved to an edge within a few pixels of it, not sized
/// to it; within a few pixels of the screen's size, fullscreen.
fn kept_in(w: &WindowPlace, s: &Screen, x: i32, y: i32, width: i32, height: i32) -> WindowPlace {
    let mut out = w.clone();
    let f = (f64::from(s.width) / f64::from(width.max(1))).min(f64::from(s.height) / f64::from(height.max(1))).min(1.0);
    let (width, height) = ((f64::from(width) * f).round() as i32, (f64::from(height) * f).round() as i32);
    let along = |v: i32, size: i32, room: i32| {
        let v = if v.abs() <= 16 {
            0
        } else if (v + size - room).abs() <= 16 {
            room - size
        } else {
            v
        };
        v.clamp(0, (room - size).max(0))
    };
    out.screen = s.name.clone();
    out.fullscreen = s.width - width <= 16 && s.height - height <= 16;
    if out.fullscreen {
        (out.x, out.y, out.width, out.height) = (0, 0, s.width, s.height);
    } else {
        (out.x, out.y, out.width, out.height) = (along(x, width, s.width), along(y, height, s.height), width, height);
    }
    out
}

/// The stand-in windows closed.
pub fn finish_arranging(app: &AppHandle) {
    let labels = std::mem::take(&mut *app.state::<Arranging>().0.lock().unwrap());
    for (label, _) in labels {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.close();
        }
    }
}

/// Text made safe for a link's address.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desk() -> Vec<Screen> {
        let s = |number, name: &str, x, width, height, primary| Screen {
            number,
            name: name.into(),
            x,
            y: 0,
            width,
            height,
            density: 1.0,
            scale: 100.0,
            refresh: 60.0,
            size_cm: None,
            hdr: false,
            primary,
        };
        vec![s(1, "Big Screen", 0, 3840, 2160, true), s(2, "Wide Screen", -3440, 3440, 1440, false)]
    }

    #[test]
    fn the_page_writes_the_players_own_settings() {
        let w = |id: &str, output: &str, screen: &str, fullscreen, x, y, width, height| WindowPlace {
            id: id.into(),
            output: output.into(),
            screen: screen.into(),
            fullscreen,
            x,
            y,
            width,
            height,
        };
        let settings = ScreenSettings {
            windows: vec![
                w("playfield", "window", "Big Screen", true, 0, 0, 3840, 2160),
                // held inside its screen
                w("backglass", "window", "Wide Screen", false, 3000, 0, 1280, 1024),
                w("scoreView", "off", "Wide Screen", false, 0, 0, 10, 10),
            ],
            view: "cabinet".into(),
            floating_score: false,
            backglass_dmd: "auto".into(),
            score_panel_dmd: "off".into(),
            show_grill: true,
            frames: "mailbox".into(),
            max_framerate: Some(-1.0),
        };
        let empty = settings_of(&[], &desk());
        let text = vpi_settings::edit("", &changes(&[], &[], &empty, &settings, &desk(), false).unwrap());
        let e = vpi_settings::parse_ini(&text);
        let screens: Vec<Display> = desk().iter().map(Screen::display).collect();
        let p = vpi_settings::place(&vpi_settings::window(&e, WindowId::Playfield), &screens).unwrap();
        assert_eq!((p.display.as_str(), p.fullscreen, p.width), ("Big Screen", true, 3840));
        let b = vpi_settings::window(&e, WindowId::Backglass);
        assert_eq!((b.output, b.x, b.width), (Output::Floating, Some(3440 - 1280), 1280));
        let p = vpi_settings::place(&b, &screens).unwrap();
        assert_eq!((p.x, p.y), (-1280, 0));
        assert_eq!(vpi_settings::window(&e, WindowId::ScoreView).output, Output::Disabled);
        assert_eq!(vpi_settings::get(&e, "Player", "BGSet"), Some("1"));
        assert_eq!(vpi_settings::get(&e, "Plugin.B2S", "BackglassDMDAutoPos"), Some("1"));
        assert_eq!(vpi_settings::get(&e, "Player", "MaxFramerate"), Some("-1"));
        assert_eq!(vpi_settings::get(&e, "VPI", "Present"), Some("mailbox"));
        assert_eq!(settings_of(&e, &desk()).frames, "mailbox");
        // a screen that isn't connected says so
        let gone = ScreenSettings { windows: vec![w("topper", "window", "Gone", true, 0, 0, 1, 1)], ..settings };
        assert!(changes(&[], &[], &empty, &gone, &desk(), false).is_err());
    }

    #[test]
    fn only_what_changed_is_written() {
        // an ultrawide layout, its score display moved: that window alone is written, and what the
        // file leaves to the player's defaults stays left
        let file = "[Player]\nPlayfieldDisplay = Big Screen\nPlayfieldFullScreen = 1\n\n[ScoreView]\nScoreViewOutput = 1\nScoreViewDisplay = Wide Screen\nScoreViewWndX = 2680\nScoreViewWndY = 506\nScoreViewWidth = 760\nScoreViewHeight = 428\n";
        let entries = vpi_settings::parse_ini(file);
        let before = settings_of(&entries, &desk());
        assert_eq!((before.score_panel_dmd.as_str(), before.backglass_dmd.as_str(), before.view.as_str()), ("auto", "off", "desktop"));
        assert!(changes(&entries, &[], &before, &before, &desk(), false).unwrap().is_empty());
        let mut after = before.clone();
        after.windows.iter_mut().find(|w| w.id == "scoreView").unwrap().y = 0;
        let out = vpi_settings::edit(file, &changes(&entries, &[], &before, &after, &desk(), false).unwrap());
        assert_eq!(out, file.replace("ScoreViewWndY = 506", "ScoreViewWndY = 0"));
    }

    #[test]
    fn a_preset_says_everything_a_tables_own_only_what_differs() {
        let main = "[Player]\nPlayfieldDisplay = Big Screen\nPlayfieldFullScreen = 1\n\n[Topper]\nTopperOutput = 1\nTopperDisplay = Wide Screen\nTopperWidth = 760\nTopperHeight = 428\n";
        let base = vpi_settings::parse_ini(main);
        let usual = settings_of(&base, &desk());
        // a No Topper preset: the topper off, said even where it's the default, so it holds over
        // a main file that shows it
        let mut no_topper = usual.clone();
        no_topper.windows.iter_mut().find(|w| w.id == "topper").unwrap().output = "off".into();
        let preset = vpi_settings::parse_ini(&vpi_settings::edit("", &changes(&[], &[], &no_topper, &no_topper, &desk(), true).unwrap()));
        assert_eq!(vpi_settings::get(&preset, "Topper", "TopperOutput"), Some("0"));
        assert_eq!(vpi_settings::get(&preset, "Player", "PlayfieldFullScreen"), Some("1"));
        let layered = [base.clone(), preset].concat();
        assert_eq!(vpi_settings::window(&layered, WindowId::Topper).output, Output::Disabled);
        // a table's own screens as the main ones: nothing written to its file; the topper off: that alone
        assert!(changes(&[], &base, &usual, &usual, &desk(), false).unwrap().is_empty());
        let own = vpi_settings::parse_ini(&vpi_settings::edit("", &changes(&[], &base, &usual, &no_topper, &desk(), false).unwrap()));
        assert_eq!(own.iter().map(|(s, n, v)| format!("{s} {n} {v}")).collect::<Vec<_>>(), ["Topper TopperOutput 0"]);
    }

    #[test]
    fn a_stand_in_keeping_its_shape_is_moved_to_edges_not_sized() {
        let screens = desk();
        let wide = &screens[1];
        let w = WindowPlace { id: "backglass".into(), output: "window".into(), screen: "Wide Screen".into(), fullscreen: false, x: 0, y: 0, width: 1, height: 1 };
        // 4:3, left 10 pixels from the top and 12 from the right: taken to both, still 4:3
        let k = kept_in(&w, wide, 3440 - 1200 - 12, 10, 1200, 900);
        assert_eq!((k.x, k.y, k.width, k.height, k.fullscreen), (3440 - 1200, 0, 1200, 900, false));
        // taller than the screen: made smaller, its shape kept
        let k = kept_in(&w, wide, 100, 0, 2000, 1500);
        assert_eq!((k.width, k.height), (1920, 1440));
        // all but the screen's size: fullscreen
        let k = kept_in(&w, wide, 4, 2, 3430, 1436);
        assert_eq!((k.x, k.y, k.width, k.height, k.fullscreen), (0, 0, 3440, 1440, true));
    }
}
