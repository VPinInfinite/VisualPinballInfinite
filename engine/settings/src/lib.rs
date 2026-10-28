//! The settings file's own rules, shared by the player and the app: `%APPDATA%\Visual Pinball Infinite\Settings.ini`, VPX build
//! 5436's settings under its own section and key names, read as VPX's
//! `LayeredINIPropertyStore` reads its ini; its window settings
//! (`Settings_properties.inl`: `PlayfieldDisplay`, `BackglassOutput`,
//! `ScoreViewWndX`, …) and where they put a window, as its `Window::Window`
//! places one (`renderer/Window.cpp`); and changes written back in place, as
//! a person would make them (`edit`), for the app's settings pages.

mod edit;

use std::path::PathBuf;

pub use edit::{Change, edit};

/// The settings file: the one `VPI_SETTINGS` names, else
/// `%APPDATA%\Visual Pinball Infinite\Settings.ini`.
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("VPI_SETTINGS")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("Settings.ini")))
}

/// The library's folders, as first setup keeps them in the settings file: the tables', VPX's scripts' and PinUP's packs' (`[VPI]` `TablesFolder`, `ScriptsFolder`,
/// `PinUPFolder`), and PinMAME's (VPX 5436's own `[Plugin.PinMAME]` `PinMAMEPath`). Only the
/// tables' must be there for a library; the others have their place in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFolders {
    pub tables: PathBuf,
    pub scripts: Option<PathBuf>,
    pub pinmame: Option<PathBuf>,
    pub pinup: Option<PathBuf>,
    /// PinballX's and PinballY's own folders, where they aren't in their usual places.
    pub pinballx: Option<PathBuf>,
    pub pinbally: Option<PathBuf>,
    /// Tables kept in a folder for each game (`[VPI]` `LibraryLayout = game`), VPX 10.8.1's layout,
    /// where a table being added goes; else together in the tables folder.
    pub per_game: bool,
}

/// The library's folders the settings name, if they name its tables' folder.
pub fn library_folders(entries: &[(String, String, String)]) -> Option<LibraryFolders> {
    let path = |section: &str, name: &str| get(entries, section, name).map(vpx_trim).filter(|v| !v.is_empty()).map(PathBuf::from);
    Some(LibraryFolders {
        tables: path("VPI", "TablesFolder")?,
        scripts: path("VPI", "ScriptsFolder"),
        pinmame: path("Plugin.PinMAME", "PinMAMEPath"),
        pinup: path("VPI", "PinUPFolder"),
        pinballx: path("VPI", "PinballXFolder"),
        pinbally: path("VPI", "PinballYFolder"),
        per_game: get(entries, "VPI", "LibraryLayout").map(vpx_trim).is_some_and(|v| v.eq_ignore_ascii_case("game")),
    })
}

/// The settings file's library folders, read as the player and the app read them.
pub fn library_folders_now() -> Option<LibraryFolders> {
    let text = std::fs::read(settings_path()?).ok()?;
    library_folders(&parse_ini(&String::from_utf8_lossy(&text)))
}

/// A screen preset's file (Save as preset on the Screens page, chosen for a
/// table in its settings): `Screen presets\<name>.ini` beside the settings file; None for a name
/// that can't be a file's.
pub fn preset_path(name: &str) -> Option<PathBuf> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 || name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\r', '\n']) || name.starts_with('.') {
        return None;
    }
    Some(settings_path()?.parent()?.join("Screen presets").join(format!("{name}.ini")))
}

/// The screen preset a table's settings or the main ones name (`[VPI]` `ScreenPreset`), the
/// table's own first, and its file's text: the player reads it between the main settings and the
/// table's own.
pub fn preset_named(table: &[(String, String, String)], main: &[(String, String, String)]) -> Option<(String, String)> {
    let name = get(table, "VPI", "ScreenPreset").or_else(|| get(main, "VPI", "ScreenPreset")).map(|n| vpx_trim(n).to_string()).filter(|n| !n.is_empty())?;
    let text = std::fs::read(preset_path(&name)?).ok()?;
    Some((name, String::from_utf8_lossy(&text).into_owned()))
}

/// An ini file's settings: (section, name, value), in file order. Lines
/// starting with `;` are comments.
pub fn parse_ini(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = s.trim().to_string();
        } else if let Some((k, v)) = line.split_once('=')
            && !line.starts_with(';')
        {
            out.push((section.clone(), k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

/// A setting, its section and name in any case, when set: the last one read
/// counts (a table's own file read over the main one), and an empty value
/// counts as not set, as VPX's ini reads it.
pub fn get<'a>(entries: &'a [(String, String, String)], section: &str, name: &str) -> Option<&'a str> {
    entries.iter().rev().find(|(s, n, v)| s.eq_ignore_ascii_case(section) && n.eq_ignore_ascii_case(name) && !v.is_empty()).map(|e| e.2.as_str())
}

/// A whole-number setting as VPX reads it (`IntPropertyDef::GetValid`, and an enumeration's):
/// one outside its range counts as not set, so its default holds.
pub fn int(entries: &[(String, String, String)], section: &str, name: &str, min: i32, max: i32) -> Option<i32> {
    get(entries, section, name).and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= f64::from(min) && *v <= f64::from(max)).map(|v| v as i32)
}

/// The player's windows, as VPX numbers them (`VPXWindowId`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowId {
    Playfield,
    Backglass,
    ScoreView,
    Topper,
}

impl WindowId {
    pub const ALL: [WindowId; 4] = [WindowId::Playfield, WindowId::Backglass, WindowId::ScoreView, WindowId::Topper];

    /// VPX's section and key prefix for the window's settings.
    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            WindowId::Playfield => ("Player", "Playfield"),
            WindowId::Backglass => ("Backglass", "Backglass"),
            WindowId::ScoreView => ("ScoreView", "ScoreView"),
            WindowId::Topper => ("Topper", "Topper"),
        }
    }
}

/// How a window other than the playfield shows (VPX's
/// `RenderOutput::OutputMode`, its settings' order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// No window (VPX's default for all three).
    Disabled,
    /// A window of its own.
    Floating,
    /// Drawn inside the playfield's window.
    Embedded,
}

/// A window's settings (VPX's for one `VPXWindowId`), with VPX's defaults
/// and ranges: a value outside its range counts as not set.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowSettings {
    pub output: Output,
    /// The screen, by the name SDL gives it; none or one that isn't there:
    /// the main screen.
    pub display: String,
    /// Borderless fullscreen (VPX's display mode 1; its BGFX build, like
    /// ours, has no other fullscreen); else a window.
    pub fullscreen: bool,
    /// Where a window sits inside its screen, in pixels; None: centred
    /// (VPX's default for the position).
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// A window's size in pixels (VPX: 16384 unless set, so as big as the
    /// screen takes, its shape kept).
    pub width: i32,
    pub height: i32,
}

/// A window's settings, VPX's defaults for what isn't set.
pub fn window(entries: &[(String, String, String)], id: WindowId) -> WindowSettings {
    let (section, prefix) = id.names();
    let key = |k: &str| format!("{prefix}{k}");
    let output = match id {
        WindowId::Playfield => Output::Floating,
        _ => match int(entries, section, &key("Output"), 0, 2) {
            Some(1) => Output::Floating,
            Some(2) => Output::Embedded,
            _ => Output::Disabled,
        },
    };
    WindowSettings {
        output,
        display: get(entries, section, &key("Display")).unwrap_or_default().to_string(),
        // VPX's BGFX build offers windowed and borderless fullscreen alone (2, its other
        // builds' exclusive fullscreen, isn't among them, so it reads as windowed)
        fullscreen: int(entries, section, &key("FullScreen"), 0, 1) == Some(1),
        x: int(entries, section, &key("WndX"), 0, 16384),
        y: int(entries, section, &key("WndY"), 0, 16384),
        width: int(entries, section, &key("Width"), 0, 16384).unwrap_or(16384),
        height: int(entries, section, &key("Height"), 0, 16384).unwrap_or(16384),
    }
}

/// Whether any of a window's settings is in the file.
pub fn has_window(entries: &[(String, String, String)], id: WindowId) -> bool {
    let (section, prefix) = id.names();
    entries.iter().any(|(s, n, v)| s.eq_ignore_ascii_case(section) && n.len() > prefix.len() && n.get(..prefix.len()).is_some_and(|p| p.eq_ignore_ascii_case(prefix)) && !v.is_empty())
}

/// A screen, as SDL lists it (VPX's `Window::DisplayConfig`).
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub name: String,
    /// Its place on the desktop, in SDL's units.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Pixels per unit (VPX's `pixelDensity`).
    pub density: f32,
    pub primary: bool,
    /// Its picture's physical size in centimetres, the long side first, as the screen reports
    /// it of itself; None where it doesn't.
    pub size_cm: Option<(f32, f32)>,
}

/// Where a window goes: its place on the desktop in SDL's units, and
/// whether it's made borderless fullscreen there.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub fullscreen: bool,
    /// The screen it's on, by name.
    pub display: String,
}

/// The screen a window goes on (VPX's `Window::GetDisplayConfig`): the one
/// named, else the main screen (else the first).
pub fn pick_display<'a>(name: &str, displays: &'a [Display]) -> Option<&'a Display> {
    let mut picked = None;
    for d in displays {
        if d.name == name {
            return Some(d);
        }
        if d.primary {
            picked = Some(d);
        }
    }
    picked.or(displays.first())
}

/// Where a window goes (VPX's `Window::Window`): borderless fullscreen
/// covers its screen; a window takes its size in pixels, shrunk to fit the
/// screen with its shape kept, centred, then moved to its position when
/// that keeps it on the screen. None when there's no screen.
pub fn place(ws: &WindowSettings, displays: &[Display]) -> Option<Placement> {
    let d = pick_display(&ws.display, displays)?;
    let density = if d.density > 0.0 { d.density } else { 1.0 };
    let (mut x, mut y) = (d.x, d.y);
    let (mut w, mut h) = if ws.fullscreen {
        (d.width, d.height)
    } else {
        ((ws.width as f32 / density).round() as i32, (ws.height as f32 / density).round() as i32)
    };
    // constrain the window to the screen
    if w > d.width {
        h = (h * d.width) / w.max(1);
        w = d.width;
    }
    if h > d.height {
        w = (w * d.height) / h.max(1);
        h = d.height;
    }
    x += (d.width - w) / 2;
    y += (d.height - h) / 2;
    // its saved position, inside the screen, when it isn't as big as the screen
    if !ws.fullscreen && (w != d.width || h != d.height) {
        if let Some(sx) = ws.x {
            let xn = (sx as f32 / density).round() as i32;
            if 0 <= xn && xn + w <= d.width {
                x = d.x + xn;
            }
        }
        if let Some(sy) = ws.y {
            let yn = (sy as f32 / density).round() as i32;
            if 0 <= yn && yn + h <= d.height {
                y = d.y + yn;
            }
        }
    }
    Some(Placement { x, y, width: w.max(1), height: h.max(1), fullscreen: ws.fullscreen, display: d.name.clone() })
}

/// VPX's `trim_string`: spaces, tabs and line ends off both ends.
pub fn vpx_trim(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\r', '\n'])
}

/// A whole number as VPX reads one (`try_parse_int`: `from_chars` on the trimmed text): its
/// leading digits, a minus sign before them allowed, whatever follows them ignored.
pub fn vpx_int(s: &str) -> Option<i32> {
    let t = vpx_trim(s);
    let b = t.as_bytes();
    let start = usize::from(b.first() == Some(&b'-'));
    let end = start + b[start..].iter().take_while(|c| c.is_ascii_digit()).count();
    if end == start { None } else { t[..end].parse().ok() }
}

/// A number as VPX reads one (`try_parse_float`: `from_chars` on the trimmed text): its leading
/// decimal number, an exponent included when it has digits, whatever follows ignored.
pub fn vpx_float(s: &str) -> Option<f32> {
    let t = vpx_trim(s);
    let b = t.as_bytes();
    let digits = |from: usize| b[from.min(b.len())..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = usize::from(b.first() == Some(&b'-'));
    let whole = digits(i);
    i += whole;
    let mut fraction = 0;
    if b.get(i) == Some(&b'.') {
        fraction = digits(i + 1);
        i += 1 + fraction;
    }
    if whole + fraction == 0 {
        return None;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(b.get(i + 1), Some(b'+' | b'-')));
        let exponent = digits(i + 1 + sign);
        if exponent > 0 {
            i += 1 + sign + exponent;
        }
    }
    t[..i].parse().ok()
}

/// A yes or no as VPX reads one (`LayeredINIPropertyStore::LoadFromINI`): a whole number, yes
/// unless 0; else true, on or enable, or false, off or disable, in any case.
pub fn vpx_bool(s: &str) -> Option<bool> {
    if let Some(v) = vpx_int(s) {
        return Some(v != 0);
    }
    let t = s.trim();
    if ["true", "on", "enable"].iter().any(|w| t.eq_ignore_ascii_case(w)) {
        Some(true)
    } else if ["false", "off", "disable"].iter().any(|w| t.eq_ignore_ascii_case(w)) {
        Some(false)
    } else {
        None
    }
}

/// A camera file's views as settings (VPX's `ImportBackdropPOV` into the table's settings): each
/// field read as `from_chars` reads it, from its first character, as `[TableOverride]`'s view
/// settings; the file's other settings (its smoothing, ambient occlusion, frame limit) aren't
/// the player's settings yet, so they're left. Nothing from text that isn't a POV file.
pub fn pov_entries(text: &str) -> Vec<(String, String, String)> {
    const VIEWS: [(&str, &str); 3] = [("desktop", "ViewDT"), ("fullscreen", "ViewCab"), ("fullsinglescreen", "ViewFSS")];
    // VPX's order: a later field of the same setting wins ("lookat" over "inclination")
    const FIELDS: [(&str, &str); 16] = [
        ("inclination", "LookAt"),
        ("fov", "FOV"),
        ("layback", "Layback"),
        ("lookat", "LookAt"),
        ("rotation", "Rotation"),
        ("xscale", "ScaleX"),
        ("yscale", "ScaleY"),
        ("zscale", "ScaleZ"),
        ("xoffset", "PlayerX"),
        ("yoffset", "PlayerY"),
        ("zoffset", "PlayerZ"),
        ("ViewHOfs", "HOfs"),
        ("ViewVOfs", "VOfs"),
        ("WindowTopZOfs", "WindowTop"),
        ("WindowBottomZOfs", "WindowBot"),
        ("LayoutMode", "Mode"),
    ];
    let Ok(doc) = roxmltree::Document::parse(text) else { return Vec::new() };
    let root = doc.root_element();
    if root.tag_name().name() != "POV" {
        return Vec::new();
    }
    fn child<'a, 'i>(node: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
        node.children().find(|c| c.is_element() && c.tag_name().name() == name)
    }
    let mut out = Vec::new();
    for (view, prefix) in VIEWS {
        let Some(section) = child(root, view) else { continue };
        for (field, setting) in FIELDS {
            let Some(t) = child(section, field).and_then(|n| n.text()) else { continue };
            // `from_chars` reads from the first character: none before the number
            if t.starts_with(char::is_whitespace) {
                continue;
            }
            let value = if setting == "Mode" { vpx_int(t).map(|v| v.to_string()) } else { vpx_float(t).map(|v| v.to_string()) };
            if let Some(value) = value {
                out.push(("TableOverride".to_string(), format!("{prefix}{setting}"), value));
            }
        }
    }
    out
}

/// What a table's settings file starts with when one is first made beside it: the views of its
/// POV file, `<table>.pov`, which VPX imports into a table's settings while it has no file of its
/// own (`PinTable::LoadGameFromFilename`, `ImportBackdropPOV`) and saves with them. Once the file
/// is there the POV is read no more, so its views go into it. Nothing where the file is there
/// already, or there's no POV.
pub fn first_table_settings(table: &std::path::Path) -> Vec<Change> {
    let pov = table.with_extension("pov");
    if table.with_extension("ini").exists() || !pov.is_file() {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(&pov) else { return Vec::new() };
    pov_entries(&String::from_utf8_lossy(&bytes)).into_iter().map(|(section, name, value)| Change::set(&section, &name, value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tables_first_settings_take_in_its_pov() {
        let dir = std::env::temp_dir().join(format!("vpi-first-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("A Table.vpx");
        assert!(first_table_settings(&table).is_empty());
        std::fs::write(table.with_extension("pov"), "<POV><desktop><fov>45.5</fov><lookat>52</lookat></desktop></POV>").unwrap();
        let made = parse_ini(&edit("", &first_table_settings(&table)));
        assert_eq!((get(&made, "TableOverride", "ViewDTFOV"), get(&made, "TableOverride", "ViewDTLookAt")), (Some("45.5"), Some("52")));
        // a file of its own there: the POV is read no more, and nothing's taken in
        std::fs::write(table.with_extension("ini"), "").unwrap();
        assert!(first_table_settings(&table).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn screens() -> Vec<Display> {
        // two screens: a 4K screen at 125% (3072x1728 units) as the main one, an ultrawide to its left
        vec![
            Display { name: "ULTRAWIDE".into(), x: -3440, y: 0, width: 3440, height: 1440, density: 1.0, primary: false, size_cm: None },
            Display { name: "4K MAIN".into(), x: 0, y: 0, width: 3072, height: 1728, density: 1.25, primary: true, size_cm: None },
        ]
    }

    #[test]
    fn vpx_names_and_defaults() {
        let e = parse_ini(
            "[Player]\nPlayfieldDisplay = 4K MAIN\nPlayfieldFullScreen = 1\nBGSet = 1\n\n[Backglass]\nBackglassOutput = 1\nBackglassDisplay = ULTRAWIDE\nBackglassWndX = 100\nBackglassWidth = 1280\nBackglassHeight = 1024\n\n[topper]\ntopperoutput = 2\nTopperWidth =\n",
        );
        let p = window(&e, WindowId::Playfield);
        assert_eq!((p.output, p.display.as_str(), p.fullscreen), (Output::Floating, "4K MAIN", true));
        let b = window(&e, WindowId::Backglass);
        assert_eq!((b.output, b.x, b.y, b.width, b.height), (Output::Floating, Some(100), None, 1280, 1024));
        // names in any case; an empty value is VPX's default
        let t = window(&e, WindowId::Topper);
        assert_eq!((t.output, t.width), (Output::Embedded, 16384));
        assert_eq!(window(&e, WindowId::ScoreView).output, Output::Disabled);
        assert!(has_window(&e, WindowId::Backglass) && has_window(&e, WindowId::Topper) && !has_window(&e, WindowId::ScoreView));
    }

    #[test]
    fn windows_go_where_vpx_puts_them() {
        let d = screens();
        let ws = |display: &str, fullscreen, x, y, width, height| WindowSettings { output: Output::Floating, display: display.into(), fullscreen, x, y, width, height };
        // borderless fullscreen covers its screen
        let p = place(&ws("ULTRAWIDE", true, Some(5), Some(5), 640, 480), &d).unwrap();
        assert_eq!((p.x, p.y, p.width, p.height, p.fullscreen), (-3440, 0, 3440, 1440, true));
        // a window's pixels become units on a 125% screen, centred unless placed
        let p = place(&ws("4K MAIN", false, None, None, 1600, 1000), &d).unwrap();
        assert_eq!((p.x, p.y, p.width, p.height), ((3072 - 1280) / 2, (1728 - 800) / 2, 1280, 800));
        let p = place(&ws("4K MAIN", false, Some(250), Some(0), 1600, 1000), &d).unwrap();
        assert_eq!((p.x, p.y), (200, 0));
        // a position that would put it off the screen is left centred
        let p = place(&ws("ULTRAWIDE", false, Some(3000), Some(10), 1280, 1024), &d).unwrap();
        assert_eq!((p.x, p.y, p.width, p.height), (-3440 + (3440 - 1280) / 2, 10, 1280, 1024));
        // VPX's default size, 16384 square, shrinks to the screen's height, its shape kept
        let p = place(&ws("", false, None, None, 16384, 16384), &d).unwrap();
        assert_eq!((p.display.as_str(), p.width, p.height, p.x, p.y), ("4K MAIN", 1728, 1728, (3072 - 1728) / 2, 0));
        // a screen that isn't there: the main one
        assert_eq!(place(&ws("GONE", true, None, None, 1, 1), &d).unwrap().display, "4K MAIN");
        assert!(place(&ws("", true, None, None, 1, 1), &[]).is_none());
    }

    #[test]
    fn out_of_range_values_count_as_not_set() {
        // VPX: a value outside its range is its default, not the nearest end of the range
        let e = parse_ini("[Backglass]\nBackglassOutput = 5\nBackglassWidth = 20000\nBackglassWndX = -4\nBackglassFullScreen = 2\n");
        let b = window(&e, WindowId::Backglass);
        assert_eq!((b.output, b.width, b.x, b.fullscreen), (Output::Disabled, 16384, None, false));
    }

    #[test]
    fn the_first_screen_without_a_main_one() {
        let mut d = screens();
        d.iter_mut().for_each(|s| s.primary = false);
        assert_eq!(pick_display("", &d).unwrap().name, "ULTRAWIDE");
    }

    #[test]
    fn numbers_read_as_vpx_reads_them() {
        assert_eq!(vpx_int(" 0.000000"), Some(0));
        assert_eq!(vpx_int("-12abc"), Some(-12));
        assert_eq!(vpx_int("+3"), None);
        assert_eq!(vpx_int("x"), None);
        assert_eq!(vpx_float("0.490196"), Some(0.490196));
        assert_eq!(vpx_float("2.5e2x"), Some(250.0));
        assert_eq!(vpx_float("1e"), Some(1.0));
        assert_eq!(vpx_float(".5"), Some(0.5));
        assert_eq!(vpx_float("-"), None);
        assert_eq!(vpx_bool("0.000000"), Some(false));
        assert_eq!(vpx_bool("Enable"), Some(true));
        assert_eq!(vpx_bool("maybe"), None);
    }
}
