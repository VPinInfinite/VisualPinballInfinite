// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its PinUp Player
// plugin (plugins/pup: PUPManager.cpp, PUPScreen.cpp, PUPPlaylist.cpp, PUPTrigger.cpp, PUPCustomPos.cpp,
// PUPMediaManager.cpp, common.cpp), "a rewrite of NailBuster's excellent PinUp Player as a plugin, based on Jason
// Millard port for VPX standalone", from the community's research into the pack format; with the newer fixes
// matching PinUP Player (to commit a4a1e75ca: the events' matching, delayed and counted triggers,
// f03f32689 and a4a1e75ca; ducking, 108a65d69; and to 401567c15: the sound bank, 7b7f7ec0e; custom
// functions on every screen and from triggers, 7028ee18a; a duck while another screen ducks ignored, the screen
// functions 5, 6 and 34, 2db9d4c6b; PUPPinDisplay's SendMSG and CustomFunction).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! PinUP packs: a table's folder of
//! videos, pictures, sounds and fonts that plays on the cabinet's other
//! screens, driven by the table's script (`PinUpPlayer.PinDisplay`,
//! `script::pup`) and by the machine's events. This is the pack's state, as
//! VPX's plugin keeps it: its screens, each with its playlists, triggers and
//! labels, and what each screen plays (`screen::MediaManager`). Playing the
//! media itself is ours, and comes next.

pub mod av;
pub mod capture;
pub mod draw;
pub mod fonts;
pub mod label;
pub mod media;
pub mod playback;
pub mod playlist;
pub mod screen;
pub mod sound;
pub mod trigger;

pub use media::length as media_length;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use playlist::Playlist;
use screen::{Mode, Screen};

/// The screens a pack's numbers stand for (VPX's `PUP_SCREEN_*`).
pub const SCREEN_TOPPER: i32 = 0;
pub const SCREEN_DMD: i32 = 1;
pub const SCREEN_BACKGLASS: i32 = 2;
pub const SCREEN_PLAYFIELD: i32 = 3;
pub const SCREEN_MUSIC: i32 = 4;
pub const SCREEN_FULL_DMD: i32 = 5;

/// VPX 5436's PinUP settings (`[Plugin.PUP]`): packs on unless `Enable = 0`; a folder of
/// the user's own whose `PUPVideos` holds the packs (`PUPFolder`, else ours: `PinUPSystem\PUPVideos`
/// above the table); the main volume (`MainVol`, 0 to 1); each window's padding, left, right, top and
/// bottom (`TopperPad…`, `BGPad…`, `SVPad…`, 0 to 4096 pixels), the pack's screens drawn inside it.
/// VPX's frame overlays (`BGFrameOverlay`, `SVFrameOverlay`, `TopperFrameOverlay`) are left out as
/// VPX 5436 leaves them: their settings are never registered and nothing reads them.
#[derive(Clone, Debug, PartialEq)]
pub struct PupSettings {
    pub enabled: bool,
    pub folder: Option<PathBuf>,
    pub main_volume: f32,
    /// The topper's, the backglass's and the score display's, as `[left, right, top, bottom]`.
    pub pads: [[i32; 4]; 3],
}

impl Default for PupSettings {
    fn default() -> Self {
        PupSettings { enabled: true, folder: None, main_volume: 1.0, pads: [[0; 4]; 3] }
    }
}

impl PupSettings {
    pub fn from_settings(s: &crate::settings::Settings) -> PupSettings {
        let section = "Plugin.PUP";
        // clamped to VPX's range, as its settings clamp a whole number
        let pad = |prefix: &str| ["Left", "Right", "Top", "Bottom"].map(|side| s.get(section, &format!("{prefix}Pad{side}")).and_then(crate::settings::vpx_int).map_or(0, |v| v.clamp(0, 4096)));
        PupSettings {
            enabled: s.get(section, "Enable").and_then(crate::settings::vpx_bool).unwrap_or(true),
            folder: s.get(section, "PUPFolder").map(str::trim).filter(|f| !f.is_empty()).map(PathBuf::from),
            main_volume: s.float_or(section, "MainVol", 1.0, 0.0, 1.0, 0.01),
            pads: [pad("Topper"), pad("BG"), pad("SV")],
        }
    }

    /// The shared folder of packs: the settings' folder's `PUPVideos` (VPX's), else ours found
    /// above the table.
    pub fn root(&self, table: Option<&Path>) -> Option<PathBuf> {
        if let Some(folder) = &self.folder {
            match find_case_insensitive(&folder.join("pupvideos"), true) {
                Some(found) => return Some(found),
                None => eprintln!("PinUP: no PUPVideos in {} (`[Plugin.PUP]` `PUPFolder`); looking above the table instead", folder.display()),
            }
        }
        table.and_then(Pup::find_root)
    }
}

/// A pack and what it plays (VPX's `PUPManager`).
pub struct Pup {
    /// The folder of packs every table shares (`PinUPSystem\PUPVideos`), if found.
    root: Option<PathBuf>,
    /// The table's own folder: a pack beside the table (`pupvideos\<name>`) comes first.
    table_dir: Option<PathBuf>,
    /// The pack in use.
    path: Option<PathBuf>,
    rom_name: String,
    /// The pack's playlists (`playlists.pup`), which every screen copies.
    playlists: Vec<Playlist>,
    /// Every screen made, by its index; a screen of a number replaced by another stays, out of use.
    pub screens: Vec<Screen>,
    /// Each number's screen.
    by_num: HashMap<i32, usize>,
    /// The numbers in the order their first screen came (VPX's screen map keeps that order, a
    /// replaced screen taking its number's place), which events reach them in.
    num_order: Vec<i32>,
    /// The screens in drawing order, back to front.
    pub order: Vec<usize>,
    running: bool,
    main_volume: f32,
    /// Packs load at all (VPX's `[Plugin.PUP]` `Enable`).
    enabled: bool,
    /// Each window's padding, left, right, top and bottom, in pixels (`PupSettings::pads`).
    pads: [[i32; 4]; 3],
    /// Triggers fired, to run at the next update: (screen, trigger group, trigger).
    pending: Vec<(usize, usize, usize)>,
    /// Triggers waiting out their delay (a negative rest time), with when they play.
    delayed: Vec<(u64, usize, usize, usize)>,
    /// The player's windows a pack can draw in, by their size in pixels: the topper's, the
    /// backglass's, the score display's.
    windows: [Option<(u32, u32)>; 3],
    /// The pack's fonts.
    pub fonts: Arc<fonts::FontLibrary>,
    /// The pack's sound bank (`PUPSOUNDS`).
    sounds: sound::SoundBank,
    /// Makes the labels' pictures, started when a label first shows.
    labels: Option<label::LabelWorker>,
    /// The machine's outputs as last polled, by their trigger letter and number (VPX's
    /// B2SPluginEventStream starts each at -1, so its first value is an event too),.
    machine_last: HashMap<(char, i32), i32>,
    /// The pack's pictures of the score display (`PupCapture`), read at the game's first DMD
    /// frame, and the number of the last frame compared.
    captures: Option<capture::Captures>,
    last_dmd_frame: Option<u32>,
    /// How many times a picture of the score display matched, and its trigger fired.
    captures_matched: u32,
}

impl Pup {
    /// A pack player for the table at `table` (its file), packs found beside it or in `root`.
    pub fn new(table: Option<&Path>, root: Option<PathBuf>) -> Pup {
        Pup {
            root,
            table_dir: table.and_then(Path::parent).map(Path::to_path_buf),
            path: None,
            rom_name: String::new(),
            playlists: Vec::new(),
            screens: Vec::new(),
            by_num: HashMap::new(),
            num_order: Vec::new(),
            order: Vec::new(),
            running: false,
            main_volume: 1.0,
            enabled: true,
            pads: [[0; 4]; 3],
            pending: Vec::new(),
            delayed: Vec::new(),
            windows: [None; 3],
            fonts: Arc::default(),
            sounds: sound::SoundBank::default(),
            labels: None,
            machine_last: HashMap::new(),
            captures: None,
            last_dmd_frame: None,
            captures_matched: 0,
        }
    }

    /// The game's DMD's latest identify frame (VPX's B2SPluginEventStream, polled 60
    /// times a second): when new, matched against the pack's pictures of the score display,
    /// read at the first frame for its `bits` a dot (`ProcessDmdFrame`); a picture matched fires
    /// its trigger, `D` and its number, on and off.
    pub fn dmd_frame(&mut self, width: usize, height: usize, bits: u32, number: u32, frame: &[u8]) {
        if !self.running || self.last_dmd_frame == Some(number) {
            return;
        }
        self.last_dmd_frame = Some(number);
        // for checking a pack's pictures: each frame compared, its shades as digits
        if let Some(dir) = std::env::var_os("VPI_PUP_FRAME_DUMP") {
            let text: String = frame.chunks(width.max(1)).map(|row| row.iter().map(|&v| char::from_digit(u32::from(v).min(35), 36).unwrap_or('?')).collect::<String>() + "\n").collect();
            let _ = std::fs::write(std::path::Path::new(&dir).join(format!("frame-{number:06}-{width}x{height}-{bits}bit.txt")), text);
        }
        let Some(path) = self.path.as_ref() else { return };
        let captures = self.captures.get_or_insert_with(|| capture::Captures::load(path, if bits == 2 { 2 } else { 4 }));
        let trigger = captures.process(width, height, frame);
        if trigger > 0 {
            self.captures_matched += 1;
            self.queue_dof_event('D', trigger, 1);
            self.queue_dof_event('D', trigger, 0);
        }
    }

    /// The machine's outputs now — lamps (`L`), solenoids (`S`), lighting strings (`G`),
    /// switches (`W`, 0 or 1), mechanisms (`N`), segment display digits (`D`) — each that
    /// changed since the last poll an event for the triggers (VPX's B2SPluginEventStream).
    pub fn machine_states(&mut self, states: &[(char, i32, i32)]) {
        for &(kind, number, value) in states {
            let last = self.machine_last.entry((kind, number)).or_insert(-1);
            if *last != value {
                *last = value;
                self.queue_dof_event(kind, number, value);
            }
        }
    }

    /// The player's windows the pack can draw in, by their size in pixels: the topper's, the
    /// backglass's, the score display's.
    pub fn set_windows(&mut self, windows: [Option<(u32, u32)>; 3]) {
        self.windows = windows;
        self.update_seen();
    }

    /// The screens each window may show (VPX's `PUPManager::Render`): the topper screen 0, the
    /// backglass 2 (else 6), the score display 5 (else 1).
    const WINDOW_ROOTS: [&'static [i32]; 3] = [&[SCREEN_TOPPER], &[SCREEN_BACKGLASS, 6], &[SCREEN_FULL_DMD, SCREEN_DMD]];

    /// Marks the screens drawn in one of the player's windows: those under a screen a window
    /// may show.
    fn update_seen(&mut self) {
        let roots: Vec<usize> =
            (0..3).filter(|&w| self.windows[w].is_some()).flat_map(|w| Self::WINDOW_ROOTS[w].iter().filter_map(|n| self.by_num.get(n).copied())).collect();
        for k in 0..self.screens.len() {
            let mut at = Some(k);
            let mut seen = false;
            let mut steps = 0;
            while let Some(i) = at {
                if roots.contains(&i) {
                    seen = true;
                    break;
                }
                at = self.screens[i].parent;
                steps += 1;
                if steps > self.screens.len() {
                    break;
                }
            }
            self.screens[k].in_window = seen;
            // what it already plays, its background above all, shows there too
            if seen && self.screens[k].mode.visible() {
                self.screens[k].media.see();
            }
        }
    }

    /// The settings' PinUP choices (`PupSettings`): packs on or off, the volume and the windows'
    /// padding.
    pub fn apply_settings(&mut self, settings: &PupSettings) {
        self.enabled = settings.enabled;
        self.pads = settings.pads;
        self.set_main_volume(settings.main_volume);
    }

    /// The shared folder of packs for a table in a VPX install's usual layout:
    /// `PinUPSystem\PUPVideos` beside the folder holding `VisualPinball`
    /// (`C:\vPinball\VisualPinball\Tables\x.vpx` → `C:\vPinball\PinUPSystem\PUPVideos`).
    pub fn find_root(table: &Path) -> Option<PathBuf> {
        // the library's own packs' folder, as the settings name it (first setup)
        if let Some(own) = crate::script::api::library().and_then(|f| f.pinup.as_deref())
            && own.is_dir()
        {
            return Some(own.to_path_buf());
        }
        let mut dir = table.parent();
        while let Some(d) = dir {
            if let Some(found) = find_case_insensitive(&d.join("PinUPSystem").join("PUPVideos"), true) {
                return Some(found);
            }
            dir = d.parent();
        }
        None
    }

    /// The pack in use, if one was found.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// The shared folder of packs.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn rom_name(&self) -> &str {
        &self.rom_name
    }

    /// The pack named `rom`: beside the table first, then in the shared folder (VPX's `SetGameDir`).
    pub fn set_game_dir(&mut self, rom: &str) {
        // an empty name would find the folder of packs itself; packs switched off load nothing
        if rom.is_empty() || !self.enabled {
            return;
        }
        let beside = self.table_dir.as_ref().and_then(|d| find_case_insensitive(&d.join("pupvideos").join(rom), true));
        let Some(path) = beside.or_else(|| self.root.as_ref().and_then(|r| find_case_insensitive(&r.join(rom), true))) else {
            return;
        };
        if self.path.as_ref() == Some(&path) {
            return;
        }
        // the pack's fonts and sounds (VPX's LoadFonts, and its sound bank's Load)
        self.fonts = Arc::new(fonts::FontLibrary::load(&path));
        self.sounds = sound::SoundBank::load(&path);
        self.path = Some(path);
        self.rom_name = rom.to_string();
    }

    /// Loads the pack named `rom` and starts it (VPX's `LoadConfig`, from `B2SInit` or a ROM's start).
    pub fn load_config(&mut self, rom: &str, now: u64) {
        // tables call B2SInit several times, or set everything up from their script first: same pack, kept
        if self.path.is_some() && rom.eq_ignore_ascii_case(&self.rom_name) {
            if !self.running {
                self.running = true;
                self.queue_dof_event('D', 0, 1);
            }
            return;
        }
        self.unload();
        self.set_game_dir(rom);
        let Some(path) = self.path.clone() else { return };
        self.load_playlists(&path);
        if let Some(file) = top_file(&path, "screens.pup")
            && let Ok(text) = read_text(&file)
        {
            for line in text.lines().skip(1) {
                if let Some(s) = Screen::from_csv(&path, line, &self.playlists, &self.fonts.clone(), now) {
                    self.add_screen(s, now);
                }
            }
        }
        self.running = true;
        // the game's first event
        self.queue_dof_event('D', 0, 1);
    }

    /// Forgets the pack (VPX's `Unload`).
    pub fn unload(&mut self) {
        self.run_pending(0);
        self.running = false;
        self.sounds.unload();
        self.captures = None;
        self.last_dmd_frame = None;
        self.order.clear();
        self.by_num.clear();
        self.num_order.clear();
        self.screens.clear();
        self.playlists.clear();
        self.pending.clear();
        self.delayed.clear();
        self.path = None;
        self.rom_name.clear();
    }

    /// The pack's playlists, one per folder (`playlists.pup`).
    fn load_playlists(&mut self, path: &Path) {
        let Some(file) = top_file(path, "playlists.pup") else { return };
        let Ok(text) = read_text(&file) else { return };
        for line in text.lines().skip(1) {
            if let Some(p) = Playlist::from_csv(path, line)
                && !self.playlists.iter().any(|o| o.folder.eq_ignore_ascii_case(&p.folder))
            {
                self.playlists.push(p);
            }
        }
    }

    /// The pack's main volume, 0 to 1, for every screen (VPX 5436's `[Plugin.PUP]` `MainVol`,
    /// the user's own: `PUPScreen::SetMainVolume`).
    pub fn set_main_volume(&mut self, volume: f32) {
        self.main_volume = volume;
        for s in &mut self.screens {
            s.set_main_volume(volume);
        }
    }

    /// Adds a screen, replacing one of the same number (VPX's `AddScreen`); a screen placed on
    /// another that doesn't exist yet makes it, as a default screen for its number.
    pub fn add_screen(&mut self, mut s: Screen, now: u64) {
        s.set_main_volume(self.main_volume);
        let num = s.num;
        let index = self.screens.len();
        if let Some(&old) = self.by_num.get(&num) {
            if let Some(parent) = self.screens[old].parent {
                for c in &mut self.screens[parent].children {
                    if *c == old {
                        *c = index;
                    }
                }
                s.parent = Some(parent);
            }
            let adopted: Vec<usize> = self.by_num.values().copied().filter(|&k| self.screens[k].parent == Some(old)).collect();
            for k in adopted {
                self.screens[k].parent = Some(index);
                s.children.push(k);
            }
            self.screens[old].parent = None;
            self.order.retain(|&k| k != old);
        }
        let source = s.custom_pos.map(|c| c.source_screen);
        self.screens.push(s);
        if self.by_num.insert(num, index).is_none() {
            self.num_order.push(num);
        }
        self.update_seen();
        self.order.push(index);
        if let Some(source) = source {
            let parent = match self.by_num.get(&source) {
                Some(&p) => Some(p),
                None => {
                    let line = match source {
                        0 => r#"0,"Topper","",,0,ForceBack,0,"#.to_string(),
                        1 => r#"1,"DMD 4x1","",,0,ForceBack,0,"#.to_string(),
                        2 => r#"2,"Backglass 16x9","",,0,ForceBack,0,"#.to_string(),
                        3 => r#"3,"Playfield","",,0,Off,0,"#.to_string(),
                        4 => r#"4,"Music","",,0,MusicOnly,0,"#.to_string(),
                        5 => r#"5,"FullDMD 16x9","",,0,ForceBack,0,"#.to_string(),
                        6 => r#"6,"Backglass 4x3-5x4","",,0,ForceBack,0,"#.to_string(),
                        n => format!(r#"({n},"","",,0,ForceBack,0,"#),
                    };
                    // (VPX's line for other numbers starts with a stray bracket, so it reads as screen 0)
                    let path = self.path.clone().unwrap_or_default();
                    let fonts = self.fonts.clone();
                    Screen::from_csv(&path, &line, &self.playlists, &fonts, now).map(|p| {
                        let made = self.screens.len();
                        self.add_screen(p, now);
                        made
                    })
                }
            };
            if let Some(p) = parent
                && p != index
            {
                self.screens[p].children.push(index);
                self.screens[index].parent = Some(p);
            }
            self.update_seen();
        }
    }

    /// Adds a default screen for a number (VPX's `AddScreen(int)`, from `Init`).
    pub fn add_default_screen(&mut self, num: i32, now: u64) {
        let s = Screen::default_for(num, self.path.as_deref(), &self.playlists, now);
        self.add_screen(s, now);
    }

    /// The screen of a number.
    pub fn screen(&self, num: i32) -> Option<usize> {
        self.by_num.get(&num).copied()
    }

    pub fn send_to_back(&mut self, index: usize) {
        if let Some(at) = self.order.iter().position(|&k| k == index) {
            let k = self.order.remove(at);
            self.order.insert(0, k);
        }
    }

    pub fn send_to_front(&mut self, index: usize) {
        if let Some(at) = self.order.iter().position(|&k| k == index) {
            let k = self.order.remove(at);
            self.order.push(k);
        }
    }

    /// Plays on a screen (VPX's `PUPScreen::Play`), with the window ordering its mode asks.
    #[allow(clippy::too_many_arguments)]
    pub fn play(&mut self, index: usize, playlist: &str, file: &str, volume: f32, priority: i32, action: trigger::PlayAction, now: u64) {
        let fonts = self.fonts.clone();
        if let Some(order) = self.screens[index].play_named(playlist, file, volume, priority, action, 0, &fonts, now) {
            match order {
                screen::Reorder::Back => self.send_to_back(index),
                screen::Reorder::Front => self.send_to_front(index),
            }
        }
    }

    /// An event from the machine or the script (VPX's `QueueDOFEvent`): `kind` is a trigger's
    /// letter (S solenoid, W switch, L lamp, G GI, E event, M mech, D display), `id` its number.
    /// Every group of triggers waiting for it takes its value, and fires — at the next update —
    /// when all its events have the values they wait for, as often as the event comes so. Its
    /// `D` values are cleared once it fires, so a `D9=1,W5=1` doesn't fire again on a later W5
    /// (as VPX does, where PinUP Player keeps them).
    pub fn queue_dof_event(&mut self, kind: char, id: i32, value: i32) {
        for num in &self.num_order {
            let s = self.by_num[num];
            let screen = &mut self.screens[s];
            for (g, (_, group)) in screen.triggers.iter_mut().enumerate() {
                let Some(first) = group.first_mut() else { continue };
                let mut matched = false;
                for c in first.conditions.iter_mut().filter(|c| c.kind == kind && c.number == id) {
                    c.value = value;
                    matched = true;
                }
                if matched && first.is_triggered() {
                    for t in 0..group.len() {
                        self.pending.push((s, g, t));
                    }
                    for c in group[0].conditions.iter_mut().filter(|c| c.kind == 'D') {
                        c.value = 0;
                    }
                }
            }
        }
    }

    /// Runs the triggers fired since the last update and those whose delay is out, each screen's
    /// timers, play queue and volume slide, and the ducks files ask for (VPX: its main-thread
    /// callbacks, `UpdateTimers`, `SetGameTime`, `UpdateVolumeFade`).
    pub fn update(&mut self, now: u64) {
        self.run_pending(now);
        let due: Vec<(u64, usize, usize, usize)> = self.delayed.iter().copied().filter(|d| d.0 <= now).collect();
        self.delayed.retain(|d| d.0 > now);
        for (_, s, g, t) in due {
            if let Some(trigger) = self.screens.get_mut(s).and_then(|sc| sc.triggers.get_mut(g)).and_then(|(_, group)| group.get_mut(t)) {
                trigger.delay_pending = false;
            }
            self.fire_trigger(s, g, t, false, now);
        }
        let mut ended = Vec::new();
        for num in &self.num_order {
            let k = self.by_num[num];
            let s = &mut self.screens[k];
            s.update_timers(now);
            if s.media.update(now) {
                ended.push(k);
            }
        }
        for k in ended {
            self.on_main_end(k, now);
        }
        for num in &self.num_order {
            let k = self.by_num[num];
            self.screens[k].update_volume_fade(now);
        }
        self.apply_ducks(now);
    }

    fn run_pending(&mut self, now: u64) {
        let pending = std::mem::take(&mut self.pending);
        for (s, g, t) in pending {
            self.invoke_trigger(s, g, t, now);
        }
    }

    /// A trigger's turn (VPX's `PUPTrigger::Invoke`): nothing on a screen that's off; a negative
    /// rest time waits that long first (repeats meanwhile ignored), and plays then whatever its
    /// rest; else it plays now, unless it rests.
    pub fn invoke_trigger(&mut self, s: usize, g: usize, t: usize, now: u64) {
        let Some(screen) = self.screens.get_mut(s) else { return };
        let off = screen.mode == Mode::Off;
        let Some(trigger) = screen.triggers.get_mut(g).and_then(|(_, group)| group.get_mut(t)) else { return };
        if off {
            return;
        }
        if let Some(delay) = trigger.delay_ms() {
            if !trigger.delay_pending {
                trigger.delay_pending = true;
                self.delayed.push((now + delay, s, g, t));
            }
            return;
        }
        self.fire_trigger(s, g, t, true, now);
    }

    /// A trigger played (VPX's `Fire`): its counter counts the hit, its rest decides where asked,
    /// then its action.
    fn fire_trigger(&mut self, s: usize, g: usize, t: usize, check_rest: bool, now: u64) {
        let fonts = self.fonts.clone();
        let Some(screen) = self.screens.get_mut(s) else { return };
        let Some(trigger) = screen.triggers.get_mut(g).and_then(|(_, group)| group.get_mut(t)) else { return };
        if !trigger.counts() || (check_rest && trigger.is_resting(now)) {
            return;
        }
        trigger.last_triggered = Some(now);
        let (playlist, file, volume, priority, length, action, defaults, custom) = (
            trigger.playlist,
            trigger.play_file.clone(),
            trigger.volume,
            trigger.priority,
            trigger.length,
            trigger.action,
            trigger.use_defaults,
            trigger.custom_function.clone(),
        );
        use trigger::PlayAction;
        match action {
            // a sound of the pack's sound bank, at the trigger's x,y,z
            PlayAction::PlaySSF => {
                let mut place = [0; 3];
                for (k, field) in custom.split(',').take(3).enumerate() {
                    place[k] = string_to_int(field.trim(), 0);
                }
                self.play_sound(&file, place[0], place[1], place[2], volume as i32, "");
                return;
            }
            // its custom function on its screen, then its file sent as a message (PinUP's SendSelfPuPMSG)
            PlayAction::CustomFunction => {
                let num = screen.num;
                self.custom_function(num, &custom, now);
                if !file.is_empty() {
                    self.send_msg(&file, now);
                }
                return;
            }
            _ => {}
        }
        let Some(playlist) = playlist else { return };
        let reorder = if defaults {
            // its playlist's next file at the screen's default volume, no priority
            let volume = screen.default_volume as f32;
            screen.play(playlist, "", volume, 0, PlayAction::Normal, 0, &fonts, now)
        } else {
            match action {
                PlayAction::StopPlayer => {
                    screen.stop_priority(priority);
                    None
                }
                PlayAction::StopFile => {
                    screen.stop_file(playlist, &file);
                    None
                }
                _ => {
                    let r = screen.play(playlist, &file, volume, priority, action, length, &fonts, now);
                    if action == PlayAction::Loop {
                        screen.media.set_loop(true);
                    }
                    r
                }
            }
        };
        match reorder {
            Some(screen::Reorder::Back) => self.send_to_back(s),
            Some(screen::Reorder::Front) => self.send_to_front(s),
            None => {}
        }
    }

    /// The screen's main media ended (VPX's `OnPlayerEnd` with nothing queued): its held-back
    /// labels back, a splash page's return, and the ducked screens' volumes back if its file was
    /// ducking them.
    fn on_main_end(&mut self, index: usize, now: u64) {
        if self.screens[index].on_main_media_end(now) {
            self.unduck(now);
        }
    }

    /// Ducks every screen but `master` to `percent` of its volume (`SendMSG` 42, and files named
    /// `-NN…`; VPX's `DuckAllExcept`): `master` then brings them back when its main media ends;
    /// `all` ducks it too. A percent of -1 brings them back now.
    pub fn duck_all_except(&mut self, percent: i32, master: i32, all: bool, now: u64) {
        if percent == -1 {
            self.unduck(now);
            return;
        }
        // PinUP ignores a new duck while another screen is still the duck source
        if self.num_order.iter().any(|n| self.screens[self.by_num[n]].is_duck_master()) {
            return;
        }
        for num in &self.num_order {
            let k = self.by_num[num];
            if *num == master && !all {
                self.screens[k].set_duck_master(true);
            } else {
                self.screens[k].duck(percent, now);
            }
        }
    }

    /// Every ducked screen's volume back (VPX's `Unduck`).
    pub fn unduck(&mut self, now: u64) {
        for num in &self.num_order {
            let k = self.by_num[num];
            let s = &mut self.screens[k];
            if s.is_duck_master() {
                s.set_duck_master(false);
            } else {
                s.unduck(now);
            }
        }
    }

    /// Plays a sound of the pack's sound bank (VPX's `PlaySound`: `PuPSound`, `SendMSG` 402,
    /// `PlaySSF` triggers and `playevent`s).
    pub fn play_sound(&mut self, name: &str, x: i32, y: i32, z: i32, volume: i32, special: &str) {
        self.sounds.play(name, x, y, z, volume, special);
    }

    /// `Show` (`SendMSG`'s function 1): a screen that's off or plays sound only shows again.
    pub fn show_screen(&mut self, s: usize) {
        if matches!(self.screens[s].mode, Mode::Off | Mode::MusicOnly) {
            self.screens[s].set_mode(Mode::Show);
        }
    }

    /// `SendMSG`: a JSON message (VPX's `PUPPinDisplay::SendMSG`): `mt` 301 a screen's custom
    /// function, 402 a sound of the pack's sound bank (`PuPSound`'s); 400 and 401, the older sound
    /// calls, ignored as PinUP Player ignores them.
    pub fn send_msg(&mut self, msg: &str, now: u64) {
        let Some(json) = lenient_json(msg) else { return };
        let number = |key: &str| json_number(&json, key);
        match number("mt") {
            Some(301) => {
                if let Some(sn) = number("SN") {
                    self.custom_function(sn as i32, msg, now);
                }
            }
            Some(402) => {
                let text = |key: &str| json.get(key).map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string)).unwrap_or_default();
                let int = |key: &str, default: i64| number(key).unwrap_or(default) as i32;
                self.play_sound(&text("ns"), int("sX", 0), int("sY", 0), int("sZ", 0), int("vol", 100), &text("sp"));
            }
            _ => {}
        }
    }

    /// A screen's custom function, `{ "mt":301, "SN": screen, "FN": function, … }` (VPX's
    /// `PUPPinDisplay::CustomFunction`; `SendMSG` 301 and `CustomFunc` triggers): screen 99 is
    /// every screen.
    pub fn custom_function(&mut self, sn: i32, msg: &str, now: u64) {
        let Some(json) = lenient_json(msg) else { return };
        let number = |key: &str| json_number(&json, key);
        let Some(fn_) = number("FN") else { return };
        if sn == 99 {
            for num in 0..20 {
                if self.screen(num).is_some() {
                    self.custom_function(num, msg, now);
                }
            }
            return;
        }
        let Some(s) = self.screen(sn) else {
            crate::script::log::once(&format!("PinUP: no screen {sn} for {msg}"));
            return;
        };
        let int = |key: &str, default: i64| number(key).unwrap_or(default) as i32;
        match fn_ {
            1 => self.show_screen(s),
            // every label of the screen shown or not
            3 => self.screens[s].hud_visible = number("OT").unwrap_or(0) != 0,
            // on top (ForceOn) or behind (ForceBack)
            4 => {
                let top = number("FS") == Some(1);
                self.screens[s].topmost = top;
                self.screens[s].set_mode(if top { Mode::ForceOn } else { Mode::ForceBack });
            }
            // behind its siblings (PinUP's TControl.SendToBack)
            5 => self.send_to_back(s),
            // in front of its siblings, on top or not as it was (TControl.BringToFront)
            6 => self.send_to_front(s),
            // every screen's default volume
            10 => self.set_default_volume_all(int("VL", 0)),
            // what plays now, at once
            11 | 40 => self.screens[s].set_volume_current(int("VL", 0)),
            // stopped and off
            12 => {
                self.screens[s].stop();
                self.screens[s].set_mode(Mode::Off);
            }
            15 => {
                let cp = json.get("CP").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                self.screens[s].set_custom_pos(&cp);
            }
            // behind again
            18 => {
                self.screens[s].topmost = false;
                self.screens[s].set_mode(Mode::ForceBack);
            }
            // the screen's opacity, and its labels'
            20 => self.screens[s].screen_alpha = if int("AM", 0) != 0 { int("AV", 255) as f32 / 255.0 } else { 1.0 },
            22 => self.screens[s].overlay_alpha = if int("AM", 0) != 0 { int("AV", 255) as f32 / 255.0 } else { 1.0 },
            26 => self.screens[s].set_volume_multiplier(int("VL", 0)),
            // LabelShowPage's
            27 => {
                let special = json.get("EX").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
                let seconds = int("HS", 0);
                self.screens[s].set_page(int("PN", 0), if seconds >= 501 { seconds } else { seconds.saturating_mul(1000) }, &special, now);
            }
            28 => self.screens[s].set_mode(Mode::MusicOnly),
            // the labels hidden, and the script's label commands ignored, until the media ends
            34 => {
                self.screens[s].hide_during_play = true;
                self.screens[s].hud_visible = false;
            }
            // the others ducked while this screen's main media plays (ALL: this one too)
            42 => {
                let percent = int("DV", 0);
                self.duck_all_except(if percent == 0 { 1 } else { percent }, sn, int("ALL", 0) == 1, now);
            }
            46 => self.screens[s].pad_text_always = number("PA").unwrap_or(0) != 0,
            // the percent `-` files without their own digits duck the others to
            54 => self.screens[s].set_default_duck(int("DV", -1)),
            55 => {
                if let Some(step) = number("FS")
                    && step >= 0
                {
                    self.screens[s].media.set_fade_step(step as i32);
                }
            }
            // left undone in VPX too: loop refresh flags, programs to start, window order, jukebox,
            // font quality, DMD mirroring, safe loop, slow PC, aspect, media position, text
            // quality, frame rescale
            _ => {}
        }
    }

    /// `SendMSG` 10: every screen's default volume.
    pub fn set_default_volume_all(&mut self, volume: i32) {
        for s in &mut self.screens {
            s.set_default_volume(volume);
        }
    }

    /// The ducks asked for by files that started (`-NN…`), done for each screen that asked.
    pub fn apply_ducks(&mut self, now: u64) {
        for k in 0..self.screens.len() {
            if let Some(percent) = self.screens[k].media.duck_request.take() {
                let num = self.screens[k].num;
                if self.by_num.get(&num) == Some(&k) {
                    self.duck_all_except(percent, num, false, now);
                }
            }
        }
    }

    /// What the pack holds and plays now, a screen a line, back to front: for checking a
    /// table's pack without its screens (`vpi-headless --pup`).
    pub fn summary(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let name = |p: &Option<PathBuf>| p.as_ref().and_then(|p| p.file_name()).map_or_else(|| "-".to_string(), |n| n.to_string_lossy().into_owned());
        match &self.path {
            Some(p) => {
                let sounds = if self.sounds.is_empty() { String::new() } else { format!(", {} sounds", self.sounds.len()) };
                let _ = writeln!(out, "PinUP pack {} ({} playlists{sounds}){}", p.display(), self.playlists.len(), if self.running { "" } else { ", not started" });
            }
            None => {
                let _ = writeln!(out, "PinUP: no pack (shared folder {})", self.root.as_ref().map_or_else(|| "not found".to_string(), |r| r.display().to_string()));
            }
        }
        if let Some(c) = &self.captures {
            let _ = writeln!(out, "  score-display pictures: {} read, matched {} times", c.len(), self.captures_matched);
        }
        for &k in &self.order {
            let s = &self.screens[k];
            let triggers: usize = s.triggers.iter().map(|(_, g)| g.len()).sum();
            let shown = s.labels.iter().filter(|l| l.visible && (l.page == s.page || l.page == -1)).count();
            let _ = writeln!(
                out,
                "  screen {:>2} {:<22} {:?}{}: {} playlists, {} triggers, {} labels ({} showing on page {}), playing {}{}, background {}",
                s.num,
                format!("\"{}\"", s.description),
                s.mode,
                s.parent.map_or_else(String::new, |p| format!(" on screen {}", self.screens[p].num)),
                s.playlists.len(),
                triggers,
                s.labels.len(),
                shown,
                s.page,
                if s.media.is_main_playing() { name(&s.media.player.file) } else { "-".to_string() },
                if s.media.queue.len() > 1 { format!(" (+{} queued)", s.media.queue.len() - 1) } else { String::new() },
                if s.media.is_background_playing() { name(&s.media.player.file) } else { name(&s.background.clone().or(s.media.background.path.clone())) },
            );
        }
        out
    }
}

/// A PinUP message's JSON as VPX's lenient reader takes it: as written, with single quotes, or as
/// a flat object without its quotes — the way a `CustomFunc` trigger's comes out of a CSV field,
/// whose doubled quotes the CSV reader drops (`{mt:301,SN:2,FN:3}`).
pub fn lenient_json(text: &str) -> Option<serde_json::Value> {
    let text = text.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return Some(v);
    }
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text.replace('\'', "\"")) {
        return Some(v);
    }
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    let mut map = serde_json::Map::new();
    for pair in inner.split(',').filter(|p| !p.trim().is_empty()) {
        let (key, value) = pair.split_once(':')?;
        let key = key.trim().trim_matches(['"', '\'']);
        let value = value.trim().trim_matches(['"', '\'']);
        let value = if let Ok(i) = value.parse::<i64>() {
            i.into()
        } else if let Ok(f) = value.parse::<f64>() {
            f.into()
        } else {
            serde_json::Value::String(value.to_string())
        };
        map.insert(key.to_string(), value);
    }
    Some(serde_json::Value::Object(map))
}

/// A whole number in a JSON message, however the script wrote it.
fn json_number(json: &serde_json::Value, key: &str) -> Option<i64> {
    let v = json.get(key)?;
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// A file at the pack's top, as the pack has it — or, for a layout of the pack being tried without
/// it being applied (`VPI_PUP_LAYOUT`, the app's Layout window), the file that layout would put there.
pub(crate) fn top_file(pack: &Path, name: &str) -> Option<PathBuf> {
    if let Some((tried, files)) = TRYING.as_ref()
        && same_folder(tried, pack)
        && let Some(file) = files.get(&name.to_lowercase())
        && file.is_file()
    {
        return Some(file.clone());
    }
    find_case_insensitive(&pack.join(name), false)
}

/// The layout being tried: a file whose first line names the pack's folder, each line after it a
/// file at the pack's top and the file the layout copies there, apart by a tab.
static TRYING: std::sync::LazyLock<Option<(PathBuf, HashMap<String, PathBuf>)>> = std::sync::LazyLock::new(|| {
    let text = std::fs::read_to_string(std::env::var_os("VPI_PUP_LAYOUT")?).ok()?;
    let mut lines = text.lines();
    let pack = PathBuf::from(lines.next()?.trim());
    let files = lines.filter_map(|l| l.split_once('\t')).map(|(name, from)| (name.trim().to_lowercase(), PathBuf::from(from.trim()))).collect();
    eprintln!("PinUP: trying a layout of {}", pack.display());
    Some((pack, files))
});

fn same_folder(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(b)
}

/// A line of a pack's `.pup` file, split at commas outside double quotes, the quotes dropped
/// (VPX's `parse_csv_line`).
pub fn parse_csv_line(line: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    for c in line.trim_matches([' ', '\t', '\r', '\n']).chars() {
        match (quoted, c) {
            (false, '"') => quoted = true,
            (false, ',') => parts.push(std::mem::take(&mut field)),
            (true, '"') => quoted = false,
            _ => field.push(c),
        }
    }
    parts.push(field);
    parts
}

/// A number at the start of a field, after any blanks (C++'s `from_chars`, as VPX reads them):
/// the longest run that reads as one, else `default`.
pub fn string_to_int(s: &str, default: i32) -> i32 {
    let t = s.trim_matches([' ', '\t', '\r', '\n']);
    let end = t.char_indices().take_while(|&(i, c)| c.is_ascii_digit() || (i == 0 && c == '-')).map(|(i, c)| i + c.len_utf8()).last().unwrap_or(0);
    t[..end].parse().unwrap_or(default)
}

/// A decimal number at the start of a field, likewise.
pub fn string_to_float(s: &str, default: f32) -> f32 {
    let t = s.trim_matches([' ', '\t', '\r', '\n']);
    let mut end = 0;
    let mut seen_dot = false;
    let mut seen_digit = false;
    for (i, c) in t.char_indices() {
        if c.is_ascii_digit() {
            seen_digit = true;
        } else if c == '.' && !seen_dot {
            seen_dot = true;
        } else if !(i == 0 && c == '-') {
            break;
        }
        end = i + c.len_utf8();
    }
    if !seen_digit {
        return default;
    }
    t[..end].parse().unwrap_or(default)
}

/// A path whose letters' case may differ from what's on disk (packs are written on Windows
/// and their names don't always match): each part found as it is, else by ignoring case
/// (VPX's `find_case_insensitive_file_path`, `…_directory_path`).
pub fn find_case_insensitive(path: &Path, directory: bool) -> Option<PathBuf> {
    let kind_ok = |p: &Path| if directory { p.is_dir() } else { p.is_file() };
    if kind_ok(path) {
        return Some(path.to_path_buf());
    }
    let mut found = PathBuf::new();
    let parts: Vec<std::path::Component> = path.components().collect();
    for (i, part) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        let next = found.join(part.as_os_str());
        let fits = |p: &Path| if last { kind_ok(p) } else { p.is_dir() };
        if fits(&next) || matches!(part, std::path::Component::Prefix(_) | std::path::Component::RootDir) {
            found = next;
            continue;
        }
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        let base = if found.as_os_str().is_empty() { PathBuf::from(".") } else { found.clone() };
        let entry = std::fs::read_dir(&base).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == name && fits(&e.path()))?;
        found = entry.path();
    }
    kind_ok(&found).then_some(found)
}

/// A pack's text file: UTF-8, else read as Windows' Latin-1, as packs are often written.
fn read_text(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(match String::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VPX 5436's `[Plugin.PUP]`: the switch, the folder, the volume and each window's padding.
    #[test]
    fn settings_under_vpx_names() {
        let s = crate::settings::Settings::from_text("[Plugin.PUP]\nEnable = 0\nPUPFolder = D:\\PinUP\nMainVol = 0.5\nBGPadLeft = 10\nBGPadBottom = 5000\nSVPadTop = 7\n");
        let p = PupSettings::from_settings(&s);
        assert!(!p.enabled);
        assert_eq!(p.folder, Some(PathBuf::from("D:\\PinUP")));
        assert_eq!(p.main_volume, 0.5);
        assert_eq!(p.pads, [[0, 0, 0, 0], [10, 0, 0, 4096], [0, 0, 7, 0]]);
        assert_eq!(PupSettings::from_settings(&crate::settings::Settings::default()), PupSettings::default());
    }

    /// A machine output's change fires its trigger once; its first value counts as a change (VPX's
    /// event stream starts each output at -1), an unchanged one doesn't.
    #[test]
    fn machine_outputs_fire_triggers_once_per_change() {
        let dir = std::env::temp_dir().join(format!("vpi-pup-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("clips")).expect("a folder");
        std::fs::write(dir.join("clips").join("a.mp4"), b"x").expect("a file");
        let playlists = vec![Playlist::new(&dir, "clips", "", false, 0, 100.0, 1)];
        let mut screen = Screen::new(Some(&dir), Mode::Show, 2, "Backglass", false, 100, None, &playlists);
        let t = trigger::Trigger::from_csv(2, &screen.playlists, r#"1,1,"lamp",L11,2,clips,a.mp4,,,,,,,0"#).expect("a trigger");
        screen.triggers.push((t.trigger.clone(), vec![t]));
        let mut pup = Pup::new(None, None);
        pup.add_screen(screen, 0);
        pup.machine_states(&[('L', 11, 0)]);
        assert!(pup.pending.is_empty());
        pup.machine_states(&[('L', 11, 1)]);
        assert_eq!(pup.pending.len(), 1);
        pup.pending.clear();
        pup.machine_states(&[('L', 11, 1)]);
        assert!(pup.pending.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A screen made before the pack places it in a window — its background starts as it's made
    /// — has what it plays shown once it's placed there (Harry Potter's topper video), and a
    /// screen that's off or has no window doesn't.
    #[test]
    fn a_screen_placed_in_a_window_shows_what_it_already_plays() {
        let mut pup = Pup::new(None, None);
        pup.set_windows([Some((640, 200)), None, None]);
        pup.add_screen(Screen::new(None, Mode::Show, SCREEN_TOPPER, "Topper", false, 100, None, &[]), 0);
        pup.add_screen(Screen::new(None, Mode::Off, 3, "PlayField", false, 100, None, &[]), 0);
        pup.add_screen(Screen::new(None, Mode::Show, SCREEN_BACKGLASS, "Backglass", false, 100, None, &[]), 0);
        let seen = |n: i32| pup.screens[pup.by_num[&n]].media.seen;
        assert!(seen(SCREEN_TOPPER) && !seen(3) && !seen(SCREEN_BACKGLASS));
    }

    /// A trigger on two events fires each time both have the values it waits for (a bare event
    /// waits for 1); its `D` value is cleared as it fires, so the other event alone doesn't fire
    /// it again, the same D event sent again does. A negative rest time delays it: -150 is
    /// 150 ms, repeats meanwhile ignored. A file named `-30…` ducks the other screens to 30% of
    /// their volume, sliding there in steps of 5 every 50 ms, and back when it ends.
    #[test]
    fn triggers_match_delay_and_duck_as_pinup_player() {
        let dir = std::env::temp_dir().join(format!("vpi-pup-test-trig-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("clips")).expect("a folder");
        // just an MP4's movie header, its length in milliseconds
        let mp4 = |ms: u32| [&36u32.to_be_bytes()[..], b"moov", &28u32.to_be_bytes(), b"mvhd", &[0; 12], &1000u32.to_be_bytes(), &ms.to_be_bytes()].concat();
        std::fs::write(dir.join("clips").join("a.mp4"), mp4(10_000)).expect("a file");
        std::fs::write(dir.join("clips").join("-30duck.mp4"), mp4(5_000)).expect("a file");
        let playlists = vec![Playlist::new(&dir, "clips", "", false, 0, 100.0, 1)];
        let mut screen = Screen::new(Some(&dir), Mode::Show, 2, "Backglass", false, 100, None, &playlists);
        for line in [r#"1,1,"both","D9=1,W5",2,clips,a.mp4,,,,,,,0"#, r#"2,1,"later",E7,2,clips,a.mp4,,,,,-150,,0"#] {
            let t = trigger::Trigger::from_csv(2, &screen.playlists, line).expect("a trigger");
            screen.triggers.push((t.trigger.clone(), vec![t]));
        }
        let mut pup = Pup::new(None, None);
        pup.add_screen(screen, 0);
        pup.queue_dof_event('W', 5, 1);
        assert!(pup.pending.is_empty());
        pup.queue_dof_event('D', 9, 1);
        assert_eq!(pup.pending, [(0, 0, 0)]);
        pup.pending.clear();
        pup.queue_dof_event('W', 5, 1);
        assert!(pup.pending.is_empty(), "D9 was cleared as it fired");
        pup.queue_dof_event('D', 9, 1);
        assert_eq!(pup.pending.len(), 1);
        pup.pending.clear();
        // the delayed one: waits 150 ms, a repeat meanwhile ignored
        pup.queue_dof_event('E', 7, 1);
        pup.update(1000);
        pup.queue_dof_event('E', 7, 1);
        pup.update(1100);
        assert_eq!(pup.delayed.len(), 1);
        assert!(!pup.screens[0].is_main_playing());
        pup.update(1150);
        assert!(pup.delayed.is_empty());
        assert!(pup.screens[0].is_main_playing());
        // a second screen playing at 100, ducked by the first's "-30" file
        let mut music = Screen::new(Some(&dir), Mode::MusicOnly, 4, "Music", false, 100, None, &playlists);
        music.play(0, "a.mp4", 100.0, 0, trigger::PlayAction::Loop, 0, &fonts::FontLibrary::default(), 1200);
        pup.add_screen(music, 1200);
        let m = pup.by_num[&4];
        assert_eq!(pup.screens[m].current_volume, 100);
        pup.play(0, "clips", "-30duck.mp4", 100.0, 5, trigger::PlayAction::Normal, 1200);
        pup.apply_ducks(1200);
        for step in 0..30 {
            pup.update(1200 + step * 50);
        }
        assert_eq!(pup.screens[m].current_volume, 30);
        // the ducking file ends at 6.2 s: the volume slides back, in steps of 6
        for step in 0..30 {
            pup.update(6200 + step * 50);
        }
        assert!(!pup.screens[0].is_main_playing());
        assert_eq!(pup.screens[m].current_volume, 100);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn csv_and_numbers_read_as_vpx_reads_them() {
        assert_eq!(parse_csv_line(r#"2,"Backglass, main","",,0,ForceBack,0,"#), ["2", "Backglass, main", "", "", "0", "ForceBack", "0", ""]);
        assert_eq!(parse_csv_line("1,22.5,45.3,55.1,54.7\r\n"), ["1", "22.5", "45.3", "55.1", "54.7"]);
        assert_eq!(string_to_int(" 12 ", 0), 12);
        assert_eq!(string_to_int("12abc", 0), 12);
        assert_eq!(string_to_int("-3", 0), -3);
        assert_eq!(string_to_int("", 7), 7);
        assert_eq!(string_to_int("abc", 7), 7);
        assert_eq!(string_to_float("22.5", 0.0), 22.5);
        assert_eq!(string_to_float(" 100 ", 0.0), 100.0);
        assert_eq!(string_to_float("", 100.0), 100.0);
    }
}
