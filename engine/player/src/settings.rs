//! One settings model for every window: where the
//! playfield, the backglass, the score view and the topper go, kept as VPX
//! build 5436 keeps them — its window settings (`Settings_properties.inl`:
//! `PlayfieldDisplay`, `BackglassOutput`, `ScoreViewWndX`, …), read as its
//! `Window::Window` and `RenderOutput::SetMode` read them
//! (`renderer/Window.cpp`) — in a file of ours,
//! `%APPDATA%\Visual Pinball Infinite\Settings.ini`, under VPX's own section
//! and key names. The player's options (`--size`, `--fullscreen`, `--view`,
//! `--backglass-window`, `--dmd-window`, `--topper-window`) come first;
//! what neither sets keeps the player's defaults. Test mode reads only the
//! file `VPI_SETTINGS` names, when it names one (the VPX picture tool's
//! cabinet view), and never a table's own, so its pictures never depend on
//! anyone's settings unless asked. The app's settings pages write it.

use std::path::{Path, PathBuf};

use crate::camera::View;

// the file's own rules, shared with the app's settings pages (vpi-settings)
pub use vpi_settings::{Display, Output, Placement, WindowId, WindowSettings, parse_ini, pick_display, place, pov_entries, vpx_bool, vpx_float, vpx_int, vpx_trim};

/// The settings file's contents.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    entries: Vec<(String, String, String)>,
    /// Where they were read from, if anywhere.
    pub path: Option<PathBuf>,
    /// The table's own settings file, when one was read over them.
    pub table_path: Option<PathBuf>,
    /// A camera file read over them (`<table>.pov` or `autopov.pov`), if one was.
    pub pov_path: Option<PathBuf>,
}

impl Settings {
    /// The settings file (`VPI_SETTINGS` names another), or none when it
    /// isn't there.
    pub fn load() -> Settings {
        let path = vpi_settings::settings_path();
        match path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => Settings { entries: parse_ini(&text), path, ..Settings::default() },
            None => Settings::default(),
        }
    }

    /// Settings from an ini file's text.
    pub fn from_text(text: &str) -> Settings {
        Settings { entries: parse_ini(text), ..Settings::default() }
    }

    /// These with the table's own settings file over them (VPX's `<table>.ini` beside the
    /// table, `PinTable::GetSettingsFileName`): its values count before these, as VPX's table
    /// settings come before their parent, the application's (`LayeredINIPropertyStore`). As
    /// they were when the table has none.
    pub fn with_table(self, table: &Path) -> Settings {
        let file = table.with_extension("ini");
        let own = std::fs::read(&file).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        // a screen preset the table's settings (or the main ones) name, between the two (a No Topper
        // preset and the like, chosen for a table)
        // or the one whoever starts the player asks for (`VPI_SCREEN_PRESET`: the app's cabinet mode)
        let asked = std::env::var("VPI_SCREEN_PRESET").ok().filter(|n| !n.trim().is_empty()).and_then(|name| {
            let text = vpi_settings::preset_path(&name).and_then(|p| std::fs::read_to_string(p).ok())?;
            Some((name, text))
        });
        let mut s = match asked.or_else(|| vpi_settings::preset_named(&parse_ini(own.as_deref().unwrap_or("")), &self.entries)) {
            Some((name, text)) => {
                eprintln!("screen preset: {name}");
                self.over(&text)
            }
            None => self,
        };
        if let Some(text) = own {
            if crate::achievements::hardcore_play() {
                // hardcore: what changes how the table plays left out of its own settings — its
                // script's options and its difficulty — its screens and sound kept
                let mut entries = parse_ini(&text);
                entries.retain(|(section, name, _)| !section.eq_ignore_ascii_case("TableOption") && !(section.eq_ignore_ascii_case("TableOverride") && name.eq_ignore_ascii_case("Difficulty")));
                s.entries.extend(entries);
            } else {
                s = s.over(&text);
            }
            s.table_path = Some(file);
        }
        s
    }

    /// These with a camera file over them, as VPX's loading imports one into the table's settings
    /// (`PinTable::LoadGameFromFilename`, `ImportBackdropPOV`): `<table>.pov` beside the table
    /// when the table has no settings file of its own, else `autopov.pov` beside it; its views'
    /// values (`desktop`, `fullscreen`, `fullsinglescreen`) as `[TableOverride]`'s `ViewDT…`,
    /// `ViewCab…` and `ViewFSS…`. As they were when there's none.
    pub fn with_pov(mut self, table: &Path) -> Settings {
        let own = table.with_extension("pov");
        let file = if self.table_path.is_none() && own.is_file() { Some(own) } else { table.parent().map(|d| d.join("autopov.pov")).filter(|p| p.is_file()) };
        if let Some(file) = file
            && let Ok(bytes) = std::fs::read(&file)
        {
            self.entries.extend(pov_entries(&String::from_utf8_lossy(&bytes)));
            self.pov_path = Some(file);
        }
        self
    }

    /// These with another file's text over them, its values counting first.
    pub fn over(mut self, text: &str) -> Settings {
        self.entries.extend(parse_ini(text));
        self
    }

    /// A section's settings, by name and value, in the order read: where a name comes twice the
    /// later counts (the table's own over the main file's).
    pub fn section(&self, section: &str) -> Vec<(String, String)> {
        self.entries.iter().filter(|(s, _, _)| s.eq_ignore_ascii_case(section)).map(|(_, n, v)| (n.clone(), v.clone())).collect()
    }

    /// A setting, its section and name in any case, when set (an empty
    /// value counts as not set, as VPX's ini reads it).
    pub fn get(&self, section: &str, name: &str) -> Option<&str> {
        vpi_settings::get(&self.entries, section, name)
    }

    /// A whole-number setting as VPX reads it (`IntPropertyDef::GetValid`, and an enumeration's):
    /// one outside its range counts as not set, so its default holds.
    fn int(&self, section: &str, name: &str, min: i32, max: i32) -> Option<i32> {
        vpi_settings::int(&self.entries, section, name, min, max)
    }

    /// The same, its default given.
    pub fn int_or(&self, section: &str, name: &str, default: i32, min: i32, max: i32) -> i32 {
        self.int(section, name, min, max).unwrap_or(default)
    }

    /// The player's playfield and backglass volumes, 0 to 1 (VPX's `[Player]` `SoundVolume` and
    /// `MusicVolume`, 0 to 100, all of it when not set or out of range:
    /// `dequantizeUnsignedPercent`).
    pub fn volumes(&self) -> (f32, f32) {
        let percent = |name| self.int("Player", name, 0, 100).unwrap_or(100) as f32 / 100.0;
        (percent("SoundVolume"), percent("MusicVolume"))
    }

    /// The sound outputs the settings name, by name (VPX's `[Player]` `SoundDevice` for the
    /// playfield's sound and `SoundDeviceBG` for the backglass's; unset, the default plays).
    pub fn sound_outputs(&self) -> (Option<String>, Option<String>) {
        let name = |key| self.get("Player", key).map(|v| vpx_trim(v).to_string()).filter(|v| !v.is_empty());
        (name("SoundDevice"), name("SoundDeviceBG"))
    }

    /// Whether the backglass, score and topper windows show from the start rather than with their
    /// first picture, VPX's way: a setting of ours, not VPX's (`[VPI]` `ExtraWindowsAtStart`,
    /// 0 or 1: some users want the windows open from the start).
    pub fn windows_at_start(&self) -> bool {
        self.int("VPI", "ExtraWindowsAtStart", 0, 1) == Some(1)
    }

    /// Whether a PinUP pack's screens come before the backglass file's pictures in a window's
    /// section (`Backglass`, `ScoreView`): VPX's renderer priorities, `Priority.PUP` against
    /// `Priority.B2S`, 0 to 100, the higher first and the backglass file first when they're
    /// equal, as VPX lists it first (`Player::OnAuxRendererChanged`). None when neither is set:
    /// then the pack comes first wherever it draws.
    pub fn pack_first(&self, section: &str) -> Option<bool> {
        let (pack, file) = (self.int(section, "Priority.PUP", 0, 100), self.int(section, "Priority.B2S", 0, 100));
        (pack.is_some() || file.is_some()).then(|| pack.unwrap_or(0) > file.unwrap_or(0))
    }

    /// A number setting as VPX reads it (`FloatPropertyDef::GetValid`): one outside its range
    /// gives its default, one within is held to its step; the default itself is held to the
    /// range and the step (`GetSteppedClamped`, as when VPX makes a table's own value a
    /// setting's default).
    pub fn float_or(&self, section: &str, name: &str, default: f32, min: f32, max: f32, step: f32) -> f32 {
        let stepped = |v: f32| (if step != 0.0 { min + ((v - min) / step).round() * step } else { v }).clamp(min, max);
        match self.get(section, name).and_then(|v| v.parse::<f32>().ok()) {
            Some(v) if v >= min && v <= max => stepped(v),
            _ => stepped(default),
        }
    }

    /// An enumeration setting with `count` values from 0, as VPX reads it: one that isn't among
    /// them gives `default`.
    pub fn enum_or(&self, section: &str, name: &str, default: i32, count: i32) -> i32 {
        self.int(section, name, 0, count - 1).unwrap_or(default)
    }

    /// Whether any of a window's settings is in the file.
    pub fn has_window(&self, id: WindowId) -> bool {
        vpi_settings::has_window(&self.entries, id)
    }

    /// A window's settings, VPX's defaults for what isn't set.
    pub fn window(&self, id: WindowId) -> WindowSettings {
        vpi_settings::window(&self.entries, id)
    }

    /// The view the table is drawn for (VPX's `BGSet`: desktop, cabinet or
    /// full single screen), when set.
    pub fn view(&self) -> Option<View> {
        match self.int("Player", "BGSet", 0, 2)? {
            1 => Some(View::Cabinet),
            2 => Some(View::SingleScreen),
            _ => Some(View::Desktop),
        }
    }
}


/// Build 5436's name for a table option in the settings (`PinTable::RegisterOption`): the name
/// trimmed, each byte but a letter or digit made `_` ("Rails Visible" is "Rails_Visible").
pub fn table_option_id(name: &str) -> String {
    vpx_trim(name).bytes().map(|b| if b.is_ascii_alphanumeric() { char::from(b) } else { '_' }).collect()
}

/// A table option's value as build 5436 reads it from the settings (`PinTable::RegisterOption`,
/// `LayeredINIPropertyStore::LoadFromINI`), its range, step and default checked already: with
/// two names, off and on or the like, a toggle — the minimum or one above it; with other names,
/// one of them, set by its name or its number; else a whole number when it steps by 1 from a
/// whole minimum, or a number held to its step. Its default when nothing's stored or what's
/// stored can't be read or lies outside its range.
pub fn table_option_value(min: f32, max: f32, step: f32, default: f32, names: Option<&[String]>, stored: Option<&str>) -> f32 {
    if let Some(names) = names {
        let lower: Vec<String> = names.iter().map(|n| vpx_trim(n).to_lowercase()).collect();
        let toggle = matches!(
            lower.iter().map(String::as_str).collect::<Vec<_>>().as_slice(),
            ["off", "on"] | ["hide", "show"] | ["false", "true"] | ["on", "off"] | ["show", "hide"] | ["true", "false"]
        );
        if toggle {
            return min + f32::from(u8::from(stored.and_then(vpx_bool).unwrap_or(default != min)));
        }
        let (low, count) = (min as i32, names.len() as i32);
        let by_name = |s: &str| names.iter().position(|n| n == s).map(|k| low + k as i32);
        return stored.and_then(|s| by_name(s).or_else(|| vpx_int(s).filter(|v| (low..low + count).contains(v)))).unwrap_or(default as i32) as f32;
    }
    if step.round() == 1.0 && min.round() == min {
        return stored.and_then(vpx_int).filter(|v| (min as i32..=max as i32).contains(v)).unwrap_or(default as i32) as f32;
    }
    let held = |v: f32| (min + step * ((v - min) / step).round()).clamp(min, max);
    held(stored.and_then(vpx_float).filter(|v| (min..=max).contains(v)).unwrap_or(default))
}


/// The screens, as SDL lists them, with their desktop modes' pixel
/// densities.
///
/// # Safety
/// SDL's video must be started, on this thread.
pub unsafe fn displays() -> Vec<Display> {
    use sdl3_sys::everything::*;
    let mut out = Vec::new();
    unsafe {
        let primary = SDL_GetPrimaryDisplay();
        let mut count = 0;
        let ids = SDL_GetDisplays(&mut count);
        if ids.is_null() {
            return out;
        }
        for i in 0..count.max(0) as usize {
            let id = *ids.add(i);
            let mut r: SDL_Rect = std::mem::zeroed();
            let mode = SDL_GetDesktopDisplayMode(id);
            if !SDL_GetDisplayBounds(id, &mut r) || mode.is_null() {
                continue;
            }
            let name = SDL_GetDisplayName(id);
            let name = if name.is_null() { String::new() } else { std::ffi::CStr::from_ptr(name).to_string_lossy().into_owned() };
            let primary = if primary != 0 { id == primary } else { r.x == 0 && r.y == 0 };
            out.push(Display { name, x: r.x, y: r.y, width: r.w, height: r.h, density: (*mode).pixel_density, primary, size_cm: physical_size_cm(id) });
        }
        SDL_free(ids.cast());
    }
    out
}

/// A screen's picture's physical size in centimetres, its long side first: what the screen
/// reports of itself (its EDID) as Windows hands it on (`GetDeviceCaps`' `HORZSIZE` and
/// `VERTSIZE` for the screen's own device). None where it reports nothing.
///
/// # Safety
/// SDL's video must be started, on this thread; `id` is one of its screens.
#[cfg(windows)]
pub unsafe fn physical_size_cm(id: sdl3_sys::everything::SDL_DisplayID) -> Option<(f32, f32)> {
    use core::ffi::c_void;
    use sdl3_sys::everything::*;
    #[repr(C)]
    struct MonitorInfoExW {
        size: u32,
        monitor: [i32; 4],
        work: [i32; 4],
        flags: u32,
        device: [u16; 32],
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfoExW) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateDCW(driver: *const u16, device: *const u16, output: *const u16, init: *const c_void) -> *mut c_void;
        fn GetDeviceCaps(dc: *mut c_void, index: i32) -> i32;
        fn DeleteDC(dc: *mut c_void) -> i32;
    }
    const HORZSIZE: i32 = 4;
    const VERTSIZE: i32 = 6;
    // SAFETY: SDL's own property of the screen, then plain Windows calls on its monitor
    unsafe {
        let monitor = SDL_GetPointerProperty(SDL_GetDisplayProperties(id), SDL_PROP_DISPLAY_WINDOWS_HMONITOR_POINTER, core::ptr::null_mut());
        if monitor.is_null() {
            return None;
        }
        let mut info = MonitorInfoExW { size: size_of::<MonitorInfoExW>() as u32, monitor: [0; 4], work: [0; 4], flags: 0, device: [0; 32] };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let dc = CreateDCW(info.device.as_ptr(), info.device.as_ptr(), core::ptr::null(), core::ptr::null());
        if dc.is_null() {
            return None;
        }
        let (w, h) = (GetDeviceCaps(dc, HORZSIZE), GetDeviceCaps(dc, VERTSIZE));
        DeleteDC(dc);
        (w > 0 && h > 0).then(|| {
            let (a, b) = (w as f32 / 10.0, h as f32 / 10.0);
            (a.max(b), a.min(b))
        })
    }
}

/// Elsewhere, unknown.
///
/// # Safety
/// None needed.
#[cfg(not(windows))]
pub unsafe fn physical_size_cm(_id: u32) -> Option<(f32, f32)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screens() -> Vec<Display> {
        // two screens: a 4K screen at 125% (3072x1728 units) as the main one, an ultrawide to its left
        vec![
            Display { name: "ULTRAWIDE".into(), x: -3440, y: 0, width: 3440, height: 1440, density: 1.0, primary: false, size_cm: None },
            Display { name: "4K MAIN".into(), x: 0, y: 0, width: 3072, height: 1728, density: 1.25, primary: true, size_cm: None },
        ]
    }

    #[test]
    fn vpx_names_and_defaults() {
        let s = Settings::from_text(
            "[Player]\nPlayfieldDisplay = 4K MAIN\nPlayfieldFullScreen = 1\nBGSet = 1\n\n[Backglass]\nBackglassOutput = 1\nBackglassDisplay = ULTRAWIDE\nBackglassWndX = 100\nBackglassWidth = 1280\nBackglassHeight = 1024\n\n[topper]\ntopperoutput = 2\nTopperWidth =\n",
        );
        assert_eq!(s.view(), Some(View::Cabinet));
        let p = s.window(WindowId::Playfield);
        assert_eq!((p.output, p.display.as_str(), p.fullscreen), (Output::Floating, "4K MAIN", true));
        let b = s.window(WindowId::Backglass);
        assert_eq!((b.output, b.x, b.y, b.width, b.height), (Output::Floating, Some(100), None, 1280, 1024));
        // names in any case; an empty value is VPX's default
        let t = s.window(WindowId::Topper);
        assert_eq!((t.output, t.width), (Output::Embedded, 16384));
        assert_eq!(s.window(WindowId::ScoreView).output, Output::Disabled);
        assert!(s.has_window(WindowId::Backglass) && s.has_window(WindowId::Topper) && !s.has_window(WindowId::ScoreView));
        assert_eq!(Settings::default().view(), None);
    }

    #[test]
    fn out_of_range_values_count_as_not_set() {
        // VPX: a value outside its range is its default, not the nearest end of the range
        let s = Settings::from_text("[Backglass]
BackglassOutput = 5
BackglassWidth = 20000
BackglassWndX = -4
BackglassFullScreen = 2
[Player]
BGSet = 7
ScreenWidth = 0
ScreenInclination = 12.5
");
        let b = s.window(WindowId::Backglass);
        assert_eq!((b.output, b.width, b.x, b.fullscreen), (Output::Disabled, 16384, None, false));
        assert_eq!(s.view(), None);
        assert_eq!(s.float_or("Player", "ScreenWidth", 95.89, 5.0, 200.0, 0.0), 95.89);
        assert_eq!(s.float_or("Player", "ScreenInclination", 0.0, -30.0, 30.0, 0.0), 12.5);
        // a stepped value is held to its step, the default too
        assert_eq!(Settings::from_text("[T]
R = 100
").float_or("T", "R", 0.0, 0.0, 360.0, 90.0), 90.0);
        assert_eq!(Settings::default().float_or("T", "R", 300.0, 0.0, 360.0, 90.0), 270.0);
    }

    #[test]
    fn table_options_read_as_build_5436_reads_them() {
        let names = |n: &[&str]| n.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(table_option_id(" Rails Visible "), "Rails_Visible");
        assert_eq!(table_option_id("Mod-Level 2.0"), "Mod_Level_2_0");
        // a toggle: off at the minimum, on one above it, its default when unset
        let hide_show = names(&["Hide", "Show"]);
        assert_eq!(table_option_value(0.0, 1.0, 1.0, 1.0, Some(&hide_show), None), 1.0);
        assert_eq!(table_option_value(0.0, 1.0, 1.0, 1.0, Some(&hide_show), Some("0")), 0.0);
        assert_eq!(table_option_value(0.0, 1.0, 1.0, 0.0, Some(&hide_show), Some("on")), 1.0);
        assert_eq!(table_option_value(0.0, 1.0, 1.0, 0.0, Some(&hide_show), Some("maybe")), 0.0);
        // names: by name or number, else the default
        let art = names(&["None", "Blades", "Full"]);
        assert_eq!(table_option_value(0.0, 2.0, 1.0, 1.0, Some(&art), Some("Full")), 2.0);
        assert_eq!(table_option_value(0.0, 2.0, 1.0, 1.0, Some(&art), Some("2.000000")), 2.0);
        assert_eq!(table_option_value(0.0, 2.0, 1.0, 1.0, Some(&art), Some("7")), 1.0);
        // whole numbers read by their leading digits; out of range, the default
        assert_eq!(table_option_value(0.0, 5.0, 1.0, 3.0, None, Some("4.000000")), 4.0);
        assert_eq!(table_option_value(0.0, 5.0, 1.0, 3.0, None, Some("9")), 3.0);
        // numbers held to their step
        assert_eq!(table_option_value(0.0, 1.0, 0.25, 0.5, None, Some("0.6")), 0.5);
        assert_eq!(table_option_value(0.0, 1.0, 0.25, 0.5, None, Some("0.7")), 0.75);
        assert_eq!(table_option_value(0.0, 1.0, 0.25, 0.5, None, None), 0.5);
    }

    #[test]
    fn a_camera_file_reads_as_vpx_imports_it() {
        let pov = "<?xml version=\"1.0\"?>\n<POV>\n<desktop><inclination>40</inclination><fov>45.5</fov><lookat>52</lookat><LayoutMode>1</LayoutMode><xscale> 1.2</xscale></desktop>\n<fullscreen><rotation>270</rotation></fullscreen>\n</POV>";
        let e = pov_entries(pov);
        let s = Settings::default().over("");
        let s = Settings { entries: e, ..s };
        assert_eq!(s.get("TableOverride", "ViewDTLookAt"), Some("52"));
        assert_eq!(s.get("TableOverride", "ViewDTFOV"), Some("45.5"));
        assert_eq!(s.get("TableOverride", "ViewDTMode"), Some("1"));
        assert_eq!(s.get("TableOverride", "ViewDTScaleX"), None);
        assert_eq!(s.get("TableOverride", "ViewCabRotation"), Some("270"));
        assert!(pov_entries("<settings/>").is_empty());
    }

    #[test]
    fn a_tables_own_settings_count_before_the_main_files() {
        let s = Settings::from_text("[Backglass]
Priority.B2S = 1
[TableOption]
Rails_Visible = 1
").over("[backglass]
priority.b2s = 0
[TableOption]
Side = 2
");
        assert_eq!(s.get("Backglass", "Priority.B2S"), Some("0"));
        assert_eq!(s.pack_first("Backglass"), Some(false));
        let options = s.section("tableoption");
        assert_eq!(options, vec![("Rails_Visible".to_string(), "1".to_string()), ("Side".to_string(), "2".to_string())]);
    }

    #[test]
    fn the_extra_windows_wait_for_their_first_picture_unless_set() {
        assert!(!Settings::default().windows_at_start());
        assert!(Settings::from_text("[VPI]\nExtraWindowsAtStart = 1\n").windows_at_start());
        assert!(!Settings::from_text("[VPI]\nExtraWindowsAtStart = 2\n").windows_at_start());
    }

    #[test]
    fn renderer_priorities_order_the_pack_and_the_backglass_file() {
        // nothing set: the pack wherever it draws
        assert_eq!(Settings::default().pack_first("Backglass"), None);
        // VPX's priorities: the higher first, the backglass file first when they're equal
        let s = Settings::from_text("[Backglass]\nPriority.B2S = 1\n\n[ScoreView]\nPriority.PUP = 5\nPriority.B2S = 5\n\n[Topper]\nPriority.PUP = 2\n");
        assert_eq!(s.pack_first("Backglass"), Some(false));
        assert_eq!(s.pack_first("ScoreView"), Some(false));
        assert_eq!(s.pack_first("Topper"), Some(true));
        // out of VPX's range, not set
        assert_eq!(Settings::from_text("[Backglass]\nPriority.B2S = 400\n").pack_first("Backglass"), None);
    }
}
