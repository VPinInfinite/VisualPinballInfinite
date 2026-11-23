//! The app's pause menu over VPX, where the app asks for it (`VPI_MENU_PORT`: a port of the app's on
//! this PC, the newer interface alone). VPX tells its add-ons of every action pressed or let go as
//! it happens (`OnActionChanged`), and lets them stop VPX handling it; its own menu's action — F12,
//! or the button a controller has for it — is told apart from the others by asking VPX, as the
//! event comes, whether that action's state has just changed (VPX hands the event its own number
//! for an action, which moves between builds). Ours stops VPX opening its menu and asks the app to
//! open the app's, with where the achievements stand. What the app's menu asks of VPX is done here
//! a frame at a time — VPX's own menu opened, the game ended, the volume changed — by pressing VPX's
//! own actions as an add-on can (all but its Pause; VPX pauses by itself while the app's window has
//! the focus). The levels of the game's sounds are set here too: each add-on's by its stream's own
//! volume, as VPX hands every add-on's sound round its add-ons before it plays it (`level_of`, this
//! add-on loading before VPX listens); the table's own sounds and its music by the table's own levels
//! for them, a line run in its script. Ours.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

/// VPX's actions as its add-on interface numbers them (`VPXAction`).
pub const OPEN_MENU: u32 = 17;
pub const EXIT_GAME: u32 = 18;
pub const VOLUME_DOWN: u32 = 20;
pub const VOLUME_UP: u32 = 21;

/// `VPXActionEvent`: an action pressed or let go, and whether VPX handles it.
#[repr(C)]
pub struct ActionEvent {
    pub action: i32,
    pub is_pressed: i32,
    pub enable_vpx_processing: i32,
}

/// `VPXInputState`: actions' states asked for or set, by `VPXAction` bits.
#[repr(C)]
#[derive(Default)]
pub struct InputState {
    pub action_mask: u64,
    pub action_state: u64,
    pub state_mask: u16,
    pub plunger_position: f32,
    pub plunger_velocity: f32,
    pub nudge_acceleration_x: f32,
    pub nudge_acceleration_y: f32,
    pub nudge_displacement_x: f32,
    pub nudge_displacement_y: f32,
}

pub type InputFn = unsafe extern "C" fn(*mut InputState);

/// VPX's `RunScript`: a line run in the table's own script.
pub type RunScriptFn = unsafe extern "C" fn(*const std::ffi::c_char);

/// The levels the app's pause menu set for each add-on's sound, by the add-on's ID, and each add-on's
/// ID as VPX numbers it for this session, looked up once (`get_id`).
#[derive(Default)]
pub struct Levels {
    by_add_on: std::collections::HashMap<String, f32>,
    ids: std::collections::HashMap<u32, Option<String>>,
    get_id: Option<fn(u32) -> Option<String>>,
    /// The add-ons a level has been applied to, and what's still to say of them in the log.
    told: std::collections::HashSet<String>,
    notes: Vec<String>,
}

pub static LEVELS: std::sync::LazyLock<Mutex<Levels>> = std::sync::LazyLock::new(|| Mutex::new(Levels::default()));

/// The level to apply to a stream of the add-on VPX numbers `endpoint`, where one's set other than
/// as it came. Never waits: VPX hands sound round on its main thread.
pub fn level_of(endpoint: u32) -> Option<f32> {
    let mut levels = LEVELS.try_lock().ok()?;
    if levels.by_add_on.is_empty() {
        return None;
    }
    let get_id = levels.get_id;
    let id = levels.ids.entry(endpoint).or_insert_with(|| get_id.and_then(|g| g(endpoint))).clone()?;
    let level = levels.by_add_on.get(&id).copied().filter(|l| (l - 1.0).abs() > f32::EPSILON)?;
    if levels.told.insert(id.clone()) {
        levels.notes.push(format!("the sound of {id} at {:.0}%", level * 100.0));
    }
    Some(level)
}

/// What's to say in the log of the levels applied since it was last asked.
pub fn level_notes() -> Vec<String> {
    LEVELS.try_lock().map(|mut l| std::mem::take(&mut l.notes)).unwrap_or_default()
}

/// The line that sets a table's own level for its sounds, its errors passed over.
fn table_sounds_line(percent: i64) -> Option<std::ffi::CString> {
    std::ffi::CString::new(format!("On Error Resume Next : ActiveTable.TableSoundVolume = {}", percent.clamp(0, 100))).ok()
}

/// The line that sets a table's own level for its music, for the song playing too where nothing in
/// its script gives a song a volume of its own (`now`).
fn table_music_line(percent: i64, now: bool) -> Option<std::ffi::CString> {
    let percent = percent.clamp(0, 100);
    let line = if now { format!("On Error Resume Next : ActiveTable.TableMusicVolume = {percent} : MusicVolume = 1") } else { format!("On Error Resume Next : ActiveTable.TableMusicVolume = {percent}") };
    std::ffi::CString::new(line).ok()
}

/// The menu's link with the app, and what it's doing.
pub struct Menu {
    stream: TcpStream,
    asked: Receiver<String>,
    get_input: InputFn,
    set_input: InputFn,
    /// VPX's menu action's state last seen.
    was_pressed: bool,
    /// VPX's own menu opened at the app's asking: its keys its own until the game goes on (the
    /// clock it was opened at).
    vpx_menu_from: Option<u64>,
    /// Presses still to make, a frame each: (action, pressed).
    presses: std::collections::VecDeque<(u32, bool)>,
    run_script: Option<RunScriptFn>,
    /// Lines to run in the table's script as play begins: the levels kept for this table.
    at_start: Vec<std::ffi::CString>,
    /// For checks (`VPI_MENU_CHECK_PRESS_AT_MS`): VPX's menu action pressed as a player would, once
    /// the game's clock reaches it, and let go the frame after.
    check_press_at: Option<u64>,
    check_release: bool,
}

/// The menu while a game plays; its own lock, apart from the add-on's, as VPX tells of an action
/// pressed by the add-on itself while it presses it.
pub static MENU: Mutex<Option<Menu>> = Mutex::new(None);

impl Menu {
    /// The link with the app, where it asks for its menu: connected to its port, a thread hearing
    /// what it asks.
    pub fn connect(port: u16, get_input: InputFn, set_input: InputFn, run_script: Option<RunScriptFn>, get_id: fn(u32) -> Option<String>) -> Option<Menu> {
        let stream = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(500)).ok()?;
        let _ = stream.set_nodelay(true);
        let reader = stream.try_clone().ok()?;
        let (tx, asked) = channel();
        std::thread::Builder::new()
            .name("vpi-menu".into())
            .spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        let check_press_at = std::env::var("VPI_MENU_CHECK_PRESS_AT_MS").ok().and_then(|v| v.trim().parse().ok());
        // the levels kept for this table, as the app hands them
        let kept: serde_json::Value = std::env::var("VPI_MENU_LEVELS").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut by_add_on = std::collections::HashMap::new();
        if let Some(add_ons) = kept.get("addOns").and_then(serde_json::Value::as_object) {
            by_add_on.extend(add_ons.iter().filter_map(|(id, v)| Some((id.clone(), v.as_f64()? as f32))));
        }
        *LEVELS.lock().unwrap_or_else(|e| e.into_inner()) = Levels { by_add_on, get_id: Some(get_id), ..Levels::default() };
        let mut at_start = Vec::new();
        at_start.extend(kept.get("table").and_then(serde_json::Value::as_i64).and_then(table_sounds_line));
        at_start.extend(kept.get("music").and_then(|m| table_music_line(m.get("percent")?.as_i64()?, m.get("now")?.as_bool()?)));
        Some(Menu { stream, asked, get_input, set_input, was_pressed: false, vpx_menu_from: None, presses: Default::default(), run_script, at_start, check_press_at, check_release: false })
    }

    /// A line run in the table's own script.
    fn run(&self, line: &std::ffi::CStr) {
        if let Some(run) = self.run_script {
            unsafe { run(line.as_ptr()) };
        }
    }

    /// Whether VPX's action is pressed now.
    fn pressed(&self, action: u32) -> bool {
        let mut s = InputState { action_mask: 1 << action, ..InputState::default() };
        unsafe { (self.get_input)(&mut s) };
        s.action_state & (1 << action) != 0
    }

    /// An action VPX tells of: VPX's own menu's, while its own menu isn't open at the app's asking,
    /// kept from VPX; pressed, the app asked to open its menu (`open`, the line telling it).
    /// Whether it was VPX's menu's.
    pub fn on_action(&mut self, ev: &mut ActionEvent, open: impl FnOnce() -> String) -> bool {
        let now = self.pressed(OPEN_MENU);
        if now == self.was_pressed {
            return false;
        }
        self.was_pressed = now;
        if self.vpx_menu_from.is_some() {
            return true;
        }
        ev.enable_vpx_processing = 0;
        if now {
            let line = open();
            // VPX has the focus, so Windows lets it hand the app the right to take it: the menu's
            // window comes to the front over the playfield, not behind it
            allow_foreground();
            let _ = self.stream.write_all(format!("{line}\n").as_bytes());
        }
        true
    }

    /// Each frame, at `clock` (the game's, in ms): what the app asked taken in, and the next press
    /// made — VPX's menu opened, the game ended, the volume changed a step at a time.
    pub fn frame(&mut self, clock: u64) {
        // VPX's own menu closed: the game goes on
        if self.vpx_menu_from.is_some_and(|from| clock > from + 50) && self.presses.is_empty() {
            self.vpx_menu_from = None;
        }
        while let Ok(line) = self.asked.try_recv() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
            match v.get("do").and_then(|d| d.as_str()) {
                Some("vpx-menu") => {
                    self.vpx_menu_from = Some(clock);
                    self.presses.extend([(OPEN_MENU, true), (OPEN_MENU, false)]);
                }
                Some("quit") => self.presses.extend([(EXIT_GAME, true), (EXIT_GAME, false)]),
                Some("volume") => {
                    let steps = v.get("steps").and_then(serde_json::Value::as_i64).unwrap_or(0).clamp(-100, 100);
                    let action = if steps < 0 { VOLUME_DOWN } else { VOLUME_UP };
                    for _ in 0..steps.unsigned_abs() {
                        self.presses.extend([(action, true), (action, false)]);
                    }
                }
                // an add-on's sound, at once: each of its streams carries its volume as it's fed
                Some("level") => {
                    if let (Some(id), Some(value)) = (v.get("id").and_then(|i| i.as_str()), v.get("value").and_then(serde_json::Value::as_f64)) {
                        LEVELS.lock().unwrap_or_else(|e| e.into_inner()).by_add_on.insert(id.to_string(), (value as f32).clamp(0.0, 1.0));
                    }
                }
                Some("table-sounds") => {
                    if let Some(line) = v.get("percent").and_then(serde_json::Value::as_i64).and_then(table_sounds_line) {
                        self.run(&line);
                    }
                }
                Some("table-music") => {
                    if let Some(line) = v.get("percent").and_then(serde_json::Value::as_i64).and_then(|p| table_music_line(p, v.get("now").and_then(serde_json::Value::as_bool).unwrap_or(false))) {
                        self.run(&line);
                    }
                }
                _ => {}
            }
        }
        // the levels kept for this table, its script running by now
        for line in std::mem::take(&mut self.at_start) {
            self.run(&line);
        }
        if let Some((action, down)) = self.presses.pop_front() {
            // VPX's menu opened at our asking keeps its keys from the moment it's pressed
            if action == OPEN_MENU {
                self.vpx_menu_from = Some(clock);
            }
            let mut s = InputState { action_mask: 1 << action, action_state: if down { 1 << action } else { 0 }, ..InputState::default() };
            unsafe { (self.set_input)(&mut s) };
        }
    }
}

impl Menu {
    /// For checks: VPX's menu action to press or let go now, as a player would — made by the caller
    /// outside the menu's lock, so VPX tells of it as of a player's press.
    pub fn check_press(&mut self, clock: u64) -> Option<(InputFn, bool)> {
        if self.check_release {
            self.check_release = false;
            return Some((self.set_input, false));
        }
        if self.check_press_at.is_some_and(|at| clock >= at) {
            self.check_press_at = None;
            self.check_release = true;
            return Some((self.set_input, true));
        }
        None
    }
}

/// Any program let take the focus from VPX's for its next window (Windows' `ASFW_ANY`): the app's
/// menu, asked for at once after.
fn allow_foreground() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn AllowSetForegroundWindow(pid: u32) -> i32;
        }
        // SAFETY: no pointers; ASFW_ANY
        unsafe { AllowSetForegroundWindow(u32::MAX) };
    }
}

/// VPX's menu action pressed or let go through `set`, as a player's would be.
pub fn press_menu(set: InputFn, down: bool) {
    let mut s = InputState { action_mask: 1 << OPEN_MENU, action_state: if down { 1 << OPEN_MENU } else { 0 }, ..InputState::default() };
    unsafe { set(&mut s) };
}

/// The line asking the app to open its menu, with where the achievements stand — a summing up and
/// the page's lines, as VPX's own menu shows them — and the add-ons' sources of sound now, each by
/// its add-on's ID and name.
pub fn open_line(page: Option<&crate::follow::Page>, sources: &[(String, String)]) -> String {
    let lines: Vec<serde_json::Value> = page
        .map(|p| {
            p.lines
                .iter()
                .map(|l| match l {
                    crate::follow::PageLine::Heading(h) => serde_json::json!({ "heading": h }),
                    crate::follow::PageLine::Achievement(a) => serde_json::json!({ "line": a }),
                })
                .collect()
        })
        .unwrap_or_default();
    let sources: Vec<serde_json::Value> = sources.iter().map(|(id, name)| serde_json::json!({ "id": id, "name": name })).collect();
    serde_json::json!({ "menu": "open", "summary": page.map(|p| p.summary.clone()), "lines": lines, "sources": sources }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_asking_for_the_menu() {
        let page = crate::follow::Page { summary: "Hardcore · 1 of 2 earned".into(), lines: vec![crate::follow::PageLine::Achievement("Again · Earned · 5 points".into())] };
        let sources = [("PinMAME".to_string(), "PinMAME".to_string())];
        let v: serde_json::Value = serde_json::from_str(&open_line(Some(&page), &sources)).unwrap();
        assert_eq!(v["menu"], "open");
        assert_eq!(v["summary"], "Hardcore · 1 of 2 earned");
        assert_eq!(v["lines"][0]["line"], "Again · Earned · 5 points");
        assert_eq!(v["sources"][0]["id"], "PinMAME");
        assert!(serde_json::from_str::<serde_json::Value>(&open_line(None, &[])).unwrap()["lines"].as_array().unwrap().is_empty());
    }

    /// A level applies to the add-on it's set for, found once by the number VPX gives it, and only
    /// where it isn't as the sound came.
    #[test]
    fn a_level_set_for_an_add_on() {
        fn id_of(endpoint: u32) -> Option<String> {
            (endpoint == 7).then(|| "PinMAME".to_string())
        }
        *LEVELS.lock().unwrap() = Levels { by_add_on: [("PinMAME".to_string(), 0.5)].into(), get_id: Some(id_of), ..Levels::default() };
        assert_eq!(level_of(7), Some(0.5));
        assert_eq!(level_of(7), Some(0.5));
        assert_eq!(level_notes(), ["the sound of PinMAME at 50%"]);
        assert_eq!(level_of(8), None);
        LEVELS.lock().unwrap().by_add_on.insert("PinMAME".to_string(), 1.0);
        assert_eq!(level_of(7), None);
        *LEVELS.lock().unwrap() = Levels::default();
    }

    #[test]
    fn the_lines_that_set_a_tables_levels() {
        assert_eq!(table_sounds_line(150).unwrap().to_str().unwrap(), "On Error Resume Next : ActiveTable.TableSoundVolume = 100");
        assert_eq!(table_music_line(40, false).unwrap().to_str().unwrap(), "On Error Resume Next : ActiveTable.TableMusicVolume = 40");
        assert_eq!(table_music_line(40, true).unwrap().to_str().unwrap(), "On Error Resume Next : ActiveTable.TableMusicVolume = 40 : MusicVolume = 1");
    }

    #[test]
    fn the_input_state_laid_out_as_vpxs() {
        assert_eq!(std::mem::size_of::<InputState>(), 48);
        assert_eq!(std::mem::offset_of!(InputState, plunger_position), 20);
        assert_eq!(std::mem::size_of::<ActionEvent>(), 12);
    }
}
