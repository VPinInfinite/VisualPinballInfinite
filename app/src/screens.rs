//! The Screens page's side: the screens as SDL lists them, as VPX does (`vpi-player --displays
//! --json`, so their names are the ones VPX's settings take), where VPX's settings file puts each
//! of its windows — by VPX 5436's rules, `vpi_settings::place` — and the page's changes written
//! back into that file in place, the file before them kept beside it as `VPinballX.previous.ini`.
//! Two helpers on the real screens: each screen's name shown on it (Identify), and VPX's windows
//! stood in for by windows of the app's own, to be dragged and sized where they should go
//! (Arrange).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Whether the newest VPX found for the library names a screen with its place, as its settings
/// read it from build 5515 on (`vpi_settings::DISPLAY_IDS_FROM`); set as the library is read.
static NAMES_PLACES: AtomicBool = AtomicBool::new(false);
/// Whether the VPX found places a backglass's score display only in its own menu (`OVERLAY_BY_HAND_FROM`).
static OVERLAY_BY_HAND: AtomicBool = AtomicBool::new(false);

/// The first VPX build whose B2S add-on has no setting placing a backglass's score display by
/// itself (`[Plugin.B2S]` `BackglassDMDOverlay`, `ScoreViewDMDOverlay`): each backglass's is placed
/// in VPX's own menu and kept in its file (VPX's 268276e6b).
pub const OVERLAY_BY_HAND_FROM: u16 = 6267;

/// Whether the VPX found keeps a backglass's score display's place in the backglass file, placed in
/// its own menu (and on a table page's Score display…).
pub fn overlay_by_hand() -> bool {
    OVERLAY_BY_HAND.load(Ordering::Relaxed)
}

/// The VPX found for the library noted, for how its settings name a screen.
pub fn note_vpx(layout: &::library::plan::Layout) {
    let vpx = ::library::vpx::find(layout);
    let names = vpx.as_ref().is_some_and(|v| v.version >= [10, 8, 1, vpi_settings::DISPLAY_IDS_FROM]);
    NAMES_PLACES.store(names, Ordering::Relaxed);
    OVERLAY_BY_HAND.store(vpx.as_ref().is_some_and(|v| v.version >= [10, 8, 1, OVERLAY_BY_HAND_FROM]), Ordering::Relaxed);
}

/// A screen's own name: a second of one name is told apart in our list (`<name> (2)`) and nowhere
/// else.
fn own_name(screen: &Screen, screens: &[Screen]) -> String {
    match screen.name.rsplit_once(" (") {
        Some((base, n)) if n.strip_suffix(')').is_some_and(|n| n.parse::<u32>().is_ok()) && screens.iter().any(|s| s.name == base) => base.to_string(),
        _ => screen.name.clone(),
    }
}

/// A screen as VPX's settings name it: by its name, and from build 5515 on by its own name and place.
fn vpx_name(screen: &Screen, screens: &[Screen]) -> String {
    if NAMES_PLACES.load(Ordering::Relaxed) { vpi_settings::display_id(&own_name(screen, screens), screen.x, screen.y) } else { screen.name.clone() }
}

/// Whether a screen setting names a connected screen, either way VPX's builds name one.
fn names_a_screen(value: &str, screens: &[Screen]) -> bool {
    match vpi_settings::split_display_id(value) {
        Some((name, x, y)) => screens.iter().any(|s| own_name(s, screens) == name && s.x == x && s.y == y),
        None => screens.iter().any(|s| s.name == value),
    }
}

/// The screen settings naming a screen alone, as VPX wrote them before build 5515, named again with
/// its place where the newest VPX found names screens so: a build reading them as before would
/// put each window on the main screen. The main settings, every table's own settings file in
/// `tables` (`<table>.ini`, where VPX's menu keeps what's changed for one table) and the screen
/// presets. How many were named again.
pub fn fit_screen_names(tables: Option<&std::path::Path>) -> Result<usize, String> {
    fit_screen_names_with(None, tables)
}

/// As `fit_screen_names`, the screens listed by the player at `player` where one is given.
pub(crate) fn fit_screen_names_with(player: Option<&std::path::Path>, tables: Option<&std::path::Path>) -> Result<usize, String> {
    if !NAMES_PLACES.load(Ordering::Relaxed) {
        return Ok(0);
    }
    let entries = vpi_settings::read_all();
    // the other files, those naming a screen alone
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    for file in tables.map(table_settings_files).unwrap_or_default().into_iter().chain(preset_files()) {
        if let Some(text) = std::fs::read(&file).ok().and_then(|b| String::from_utf8(b).ok())
            && names_a_screen_alone(&vpi_settings::parse_ini(&text))
        {
            files.push((file, text));
        }
    }
    if !names_a_screen_alone(&entries) && files.is_empty() {
        return Ok(0);
    }
    let screens = match player {
        Some(p) => screens_from(p)?,
        None => screens()?,
    };
    let displays: Vec<Display> = screens.iter().map(|s| Display { name: own_name(s, &screens), ..s.display() }).collect();
    let changes = vpi_settings::display_ids_for(&entries, &displays);
    vpi_settings::save(&changes)?;
    let mut count = changes.len();
    for (file, text) in files {
        count += fit_names_in(&file, &text, &displays)?;
    }
    Ok(count)
}

/// Whether settings name a screen alone, as VPX wrote them before build 5515.
fn names_a_screen_alone(entries: &[(String, String, String)]) -> bool {
    let alone = |v: &str| !v.trim().is_empty() && vpi_settings::split_display_id(v.trim()).is_none();
    vpi_settings::DISPLAY_SETTINGS.iter().any(|(s, k)| vpi_settings::get(entries, s, k).is_some_and(alone))
}

/// A settings file's screens named with their place, everything else in it as it was. How many.
fn fit_names_in(file: &std::path::Path, text: &str, displays: &[Display]) -> Result<usize, String> {
    let changes = vpi_settings::display_ids_for(&vpi_settings::parse_ini(text), displays);
    if !changes.is_empty() {
        std::fs::write(file, vpi_settings::edit(text, &changes)).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Ok(changes.len())
}

/// Every table's own settings file in `tables`: `<table>.ini` beside each table file there.
fn table_settings_files(tables: &std::path::Path) -> Vec<PathBuf> {
    ::library::installed::table_files(tables).unwrap_or_default().into_iter().map(|t| t.with_extension("ini")).filter(|f| f.is_file()).collect()
}

/// The screen presets' files.
fn preset_files() -> Vec<PathBuf> {
    let Some(folder) = vpi_settings::preset_path("a").and_then(|p| p.parent().map(std::path::Path::to_path_buf)) else { return Vec::new() };
    std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ini"))).collect()
}

pub(crate) fn player() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let player = exe.with_file_name("vpi-player.exe");
    if player.exists() { Ok(player) } else { Err(format!("The player isn't built yet: {}", player.display())) }
}

/// The screens, as the player sees them.
pub fn screens() -> Result<Vec<Screen>, String> {
    screens_from(&player()?)
}

/// The screens, as the player at `player` sees them.
fn screens_from(player: &std::path::Path) -> Result<Vec<Screen>, String> {
    let mut command = Command::new(player);
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
    /// The DMD over the backglass's picture (`[Plugin.B2S]` `BackglassDMDOverlay` and
    /// `…AutoPos`): `off`, `auto` (found in its picture) or `placed` (where the settings say).
    pub backglass_dmd: String,
    /// The DMD over the backglass file's score panel, in the score window (`ScoreViewDMD…`).
    pub score_panel_dmd: String,
    /// The backglass picture's speaker grill shown (`[Plugin.B2S]` `ShowGrill`).
    pub show_grill: bool,
    /// Where each achievement earned shows its card (`[VPI]` `AchievementPopup`): `backglass`,
    /// `scoreview`, or `playfield`, which is VPX's own notice, as VPX lets add-ons draw only in its
    /// other windows. The main settings' alone: a table's own and a preset's leave it.
    #[serde(default = "popup_on_default")]
    pub popup_on: String,
    /// The card's size (`[VPI]` `AchievementPopupSize`): `small`, `medium` or `large`.
    #[serde(default = "popup_size_default")]
    pub popup_size: String,
}

fn popup_on_default() -> String {
    "backglass".into()
}

fn popup_size_default() -> String {
    "medium".into()
}

/// Where the achievement popup shows and how big, as the settings say, their defaults otherwise.
pub fn popup_of(entries: &[(String, String, String)]) -> (String, String) {
    let get = |k: &str| vpi_settings::get(entries, "VPI", k).map(|v| v.trim().to_ascii_lowercase());
    let on = get("AchievementPopup").filter(|v| ["backglass", "scoreview", "playfield"].contains(&v.as_str())).unwrap_or_else(popup_on_default);
    let size = get("AchievementPopupSize").filter(|v| ["small", "medium", "large"].contains(&v.as_str())).unwrap_or_else(popup_size_default);
    (on, size)
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
    /// name: VPX puts them on the main screen meanwhile, and so does the page.
    pub missing: BTreeMap<String, String>,
    pub file: String,
    pub exists: bool,
    /// VPX's supersampling (`[Player]` `AAFactor`, 1 unless set): VPX 10.8.1 draws a window kept
    /// inside the table's into its picture as it's drawn, before it's scaled to the screen, so with
    /// any other value, there at that much of its place and size.
    pub supersampling: f32,
    /// The VPX found places a backglass's score display only in its own menu, backglass by
    /// backglass: the page's choices for it are ignored there.
    pub overlay_by_hand: bool,
}

/// VPX's supersampling as the settings have it, VPX's default where they don't or can't be read.
fn supersampling(entries: &[(String, String, String)]) -> f32 {
    vpi_settings::get(entries, "Player", "AAFactor").and_then(vpi_settings::vpx_float).filter(|f| (0.5..=2.0).contains(f)).unwrap_or(1.0)
}

/// The DMD over a backglass file's picture (`window`: `Backglass`, or `ScoreView` for its score
/// panel), as VPX 5436's B2S add-on reads its settings — none there, off, its default — and the
/// app for a newer VPX, which keeps the place in the backglass file (`backglass_dmd`).
pub(crate) fn overlay(entries: &[(String, String, String)], window: &str) -> String {
    let key = |k: &str| format!("{window}DMD{k}");
    if ["Overlay", "AutoPos", "X", "Y", "W", "H"].iter().all(|k| vpi_settings::get(entries, "Plugin.B2S", &key(k)).is_none()) {
        return "off".into();
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
    let path = vpi_settings::vpx_settings_path().ok_or("no APPDATA folder")?;
    let exists = path.is_file();
    let entries = vpi_settings::read_all();
    let settings = settings_of(&entries, &screens);
    let set = WindowId::ALL.into_iter().filter(|&id| vpi_settings::has_window(&entries, id)).map(|id| id_name(id).to_string()).collect();
    let missing = WindowId::ALL
        .into_iter()
        .filter_map(|id| {
            let ws = vpi_settings::window(&entries, id);
            (ws.output != Output::Disabled && !ws.display.is_empty() && !names_a_screen(&ws.display, &screens)).then(|| (id_name(id).to_string(), ws.display))
        })
        .collect();
    let supersampling = supersampling(&entries);
    Ok(ScreensView { screens, settings, set, missing, file: path.display().to_string(), exists, supersampling, overlay_by_hand: OVERLAY_BY_HAND.load(Ordering::Relaxed) })
}

/// What the settings say, by the player's own rules, its defaults for what they leave out.
fn settings_of(entries: &[(String, String, String)], screens: &[Screen]) -> ScreenSettings {
    let displays: Vec<Display> = screens.iter().map(Screen::display).collect();
    let mut windows: Vec<WindowPlace> = Vec::new();
    for id in WindowId::ALL {
        let ws = vpi_settings::window(entries, id);
        // drawn inside the playfield's window: its place in that window's pixels, as VPX
        // takes it (its embedded output), held inside the window
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
    let (popup_on, popup_size) = popup_of(entries);
    ScreenSettings {
        popup_on,
        popup_size,
        windows,
        view: match vpi_settings::int(entries, "Player", "BGSet", 0, 2) {
            Some(1) => "cabinet",
            Some(2) => "fss",
            _ => "desktop",
        }
        .into(),
        backglass_dmd: overlay(entries, "Backglass"),
        score_panel_dmd: overlay(entries, "ScoreView"),
        show_grill: get("Plugin.B2S", "ShowGrill").and_then(vpi_settings::vpx_bool).or_else(|| get("Plugin.B2SLegacy", "B2SHideGrill").and_then(vpi_settings::vpx_bool).map(|hide| !hide)).unwrap_or(false),
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
        // held inside its screen, as VPX would hold it
        let width = w.width.clamp(1, screen.width);
        let height = w.height.clamp(1, screen.height);
        let x = w.x.clamp(0, (screen.width - width).max(0));
        let y = w.y.clamp(0, (screen.height - height).max(0));
        put("Display", vpx_name(screen, screens), None);
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
    Ok(out)
}

/// The page's settings written into VPX's settings file, the file before them kept as
/// `VPinballX.previous.ini`.
pub fn save(settings: &ScreenSettings) -> Result<(), String> {
    let screens = screens()?;
    let entries = vpi_settings::read_all();
    let was = settings_of(&entries, &screens);
    let mut out = changes(&entries, &[], &was, settings, &screens, false)?;
    // the achievement popup, the main settings' alone
    if settings.popup_on != was.popup_on {
        out.push(Change::set("VPI", "AchievementPopup", &settings.popup_on));
    }
    if settings.popup_size != was.popup_size {
        out.push(Change::set("VPI", "AchievementPopupSize", &settings.popup_size));
    }
    vpi_settings::save(&out)
}

fn read_text(path: &std::path::Path) -> String {
    std::fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

/// What a table's own screens stand on: the main settings, then the preset it (or they) name.
fn table_base(table: &std::path::Path) -> Vec<(String, String, String)> {
    let mut base = vpi_settings::read_all();
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
    let supersampling = supersampling(&entries);
    Ok(ScreensView { screens, settings, set: Vec::new(), missing: BTreeMap::new(), file: file.display().to_string(), exists: file.exists(), supersampling, overlay_by_hand: OVERLAY_BY_HAND.load(Ordering::Relaxed) })
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
    let mut entries = vpi_settings::read_all();
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

/// Windows of the app's own where VPX's windows go, each named, to be dragged and sized
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

    #[test]
    fn a_tables_own_settings_name_their_screens_with_their_place() {
        let dir = std::env::temp_dir().join(format!("vpi-screen-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("A Table.ini");
        // as VPX's menu kept one table's backglass, before screens were named with their place
        let text = "[Backglass]\r\nBackglassOutput = 1\r\nBackglassDisplay = Wide Screen\r\nPriority.PUP = 1\r\n\r\n[Player]\r\nSoundVolume = 65\r\n";
        std::fs::write(&file, text).unwrap();
        let screen = |name: &str, x| Display { name: name.into(), x, y: 0, width: 100, height: 100, density: 1.0, primary: x == 0, size_cm: None };
        let displays = [screen("Main Screen", 0), screen("Wide Screen", -3440)];
        assert!(names_a_screen_alone(&vpi_settings::parse_ini(text)));
        assert_eq!(fit_names_in(&file, text, &displays).unwrap(), 1);
        let now = std::fs::read_to_string(&file).unwrap();
        assert_eq!(now, text.replace("BackglassDisplay = Wide Screen\r\n", "BackglassDisplay = Wide Screen [-3440, 0]\r\n"));
        // named so already, or a screen not connected: left as it is
        assert!(!names_a_screen_alone(&vpi_settings::parse_ini(&now)));
        assert_eq!(fit_names_in(&file, "[Topper]\r\nTopperDisplay = Gone\r\n", &displays).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
    fn the_page_writes_vpxs_own_settings() {
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
            backglass_dmd: "auto".into(),
            score_panel_dmd: "off".into(),
            show_grill: true,
            popup_on: "scoreview".into(),
            popup_size: "large".into(),
        };
        let empty = settings_of(&[], &desk());
        // the popup's defaults where nothing says otherwise; it's never a window's to write
        assert_eq!((empty.popup_on.as_str(), empty.popup_size.as_str()), ("backglass", "medium"));
        assert_eq!(popup_of(&vpi_settings::parse_ini("[VPI]\nAchievementPopup = ScoreView\nAchievementPopupSize = huge\n")), ("scoreview".to_string(), "medium".to_string()));
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
        // a screen that isn't connected says so
        let gone = ScreenSettings { windows: vec![w("topper", "window", "Gone", true, 0, 0, 1, 1)], ..settings };
        assert!(changes(&[], &[], &empty, &gone, &desk(), false).is_err());
    }

    #[test]
    fn only_what_changed_is_written() {
        // an ultrawide layout, its score display moved: that window alone is written, and what the
        // file leaves to VPX's defaults stays left (no score display over a picture)
        let file = "[Player]\nPlayfieldDisplay = Big Screen\nPlayfieldFullScreen = 1\n\n[ScoreView]\nScoreViewOutput = 1\nScoreViewDisplay = Wide Screen\nScoreViewWndX = 2680\nScoreViewWndY = 506\nScoreViewWidth = 760\nScoreViewHeight = 428\n";
        let entries = vpi_settings::parse_ini(file);
        let before = settings_of(&entries, &desk());
        assert_eq!((before.score_panel_dmd.as_str(), before.backglass_dmd.as_str(), before.view.as_str()), ("off", "off", "desktop"));
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
