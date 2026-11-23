// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its PinUp Player
// plugin's screens, their media queue and custom places (plugins/pup/PUPScreen.cpp, PUPMediaManager.cpp,
// PUPCustomPos.cpp, PUPMediaPlayer.cpp's game clock and held picture), with the newer fixes matching PinUP
// Player (to commit a4a1e75ca: the volume model and ducking, 108a65d69; screen modes, 781f88258;
// queued plays, pages and custom places' flags, 59536ebbb; the background kept playing and the last picture
// held, a7be2c94a; the stalled game clock, 17d326833 and 5bcd5d490; 16c805a69's held picture rules), and to
// 401567c15 (pages as PinUP's SetPageNum, the background label holding the overlays, labels held
// back while media plays, a label sent to the background or disposed; 2db9d4c6b, 9984b4dc8, 303cf471d, d858d33ef).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A pack's screen: what it plays (a queue of media over a looping
//! background), its playlists, triggers and labels, and where it sits —
//! on its own, or at a place inside another screen (`screens.pup`:
//! `ScreenNum,ScreenDes,PlayList,PlayFile,Loopit,Active,Priority,CustomPos`).
//!
//! Its volume follows PinUP Player's model: each play's volume (1 to 200,
//! a script's plays at the screen's default volume) scaled by the screen's
//! multiplier (`screens.pup`'s volume column) and capped at 100%, times the
//! player's own volume. A file whose name starts with `-` and two digits
//! (`-30…`) ducks every other screen to that percent of its volume while it
//! plays, the volume sliding down and back up in steps every 50 ms.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::fonts::FontLibrary;
use super::label::{Label, XAlign, YAlign};
use super::playlist::{Function, Playlist};
use super::trigger::{PlayAction, Trigger};
use super::{parse_csv_line, string_to_float, string_to_int};

/// The loudest a play's volume goes, in percent (VPX's `PUP_VOLUME_MAX`).
const VOLUME_MAX: i32 = 200;
/// A volume slide's steps come every 50 ms (VPX's `PUP_VOLUME_FADE_INTERVAL_MS`).
const VOLUME_FADE_MS: u64 = 50;

/// How a screen shows (`screens.pup`'s Active column).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Show,
    /// Shown, kept on top.
    ForceOn,
    /// Shown on top while it plays.
    ForcePop,
    /// Shown, kept behind.
    ForceBack,
    /// Shown behind while it plays.
    ForcePopBack,
    /// Its sound only.
    MusicOnly,
}

impl Mode {
    /// A mode as `screens.pup` writes it: PinUP Player's `ForceHide` plays sound only, its
    /// `Disable` and `JavaScript` are off, as anything unknown.
    pub fn parse(text: &str) -> Mode {
        match text.to_ascii_lowercase().as_str() {
            "show" => Mode::Show,
            "forceon" => Mode::ForceOn,
            "forcepop" => Mode::ForcePop,
            "forceback" => Mode::ForceBack,
            "forcepopback" => Mode::ForcePopBack,
            "musiconly" | "forcehide" => Mode::MusicOnly,
            _ => Mode::Off,
        }
    }

    /// It draws something (not off, not sound only).
    pub fn visible(self) -> bool {
        !matches!(self, Mode::Off | Mode::MusicOnly)
    }
}

/// A screen's place inside another, in percent of it (VPX's `PUPCustomPos`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomPos {
    pub source_screen: i32,
    /// Left, top, width, height, in percent of the source screen.
    pub rect: [f32; 4],
    /// A turn PinUP Player gives the screen (a sixth value: 90, 180 or 270 degrees), read but
    /// not drawn, as in VPX.
    pub rotation: i32,
}

impl CustomPos {
    /// `screen,x,y,w,h`, perhaps with a flag (a sixth value: a turn, or a window flag VPX leaves
    /// out) or an aspect ratio (a sixth and seventh), as in VPX: PinUP Player's own places — a
    /// source screen of 99 (an overlay only), 100 (the desktop's pixels) or 101 (percent of the
    /// screen's own window) — aren't done there either, and give no place.
    pub fn from_csv(text: &str) -> Option<CustomPos> {
        if text.is_empty() {
            return None;
        }
        let parts = parse_csv_line(text);
        if parts.len() < 5 {
            return None;
        }
        let source_screen = string_to_int(&parts[0], 0);
        if source_screen >= 99 {
            crate::script::log::once(&format!("PinUP: a screen placed by PinUP Player's own rule {source_screen} isn't done (as in VPX): {text}"));
            return None;
        }
        let mut rotation = 0;
        if parts.len() == 6 {
            rotation = match string_to_int(&parts[5], 0) {
                1 => 90,
                2 => 180,
                3 => 270,
                _ => 0,
            };
            if rotation != 0 {
                crate::script::log::once(&format!("PinUP: a screen turned {rotation} degrees isn't done (as in VPX): {text}"));
            }
        } else if parts.len() >= 7 {
            crate::script::log::once(&format!("PinUP: a screen's aspect ratio {}:{} isn't done (as in VPX): {text}", parts[5], parts[6]));
        }
        Some(CustomPos {
            source_screen,
            rect: [string_to_float(&parts[1], 0.0), string_to_float(&parts[2], 0.0), string_to_float(&parts[3], 0.0), string_to_float(&parts[4], 0.0)],
            rotation,
        })
    }

    /// The place in a screen of `w` × `h` pixels.
    pub fn scaled(&self, w: i32, h: i32) -> [i32; 4] {
        let [x, y, rw, rh] = self.rect;
        [(x / 100.0 * w as f32) as i32, (y / 100.0 * h as f32) as i32, (rw / 100.0 * w as f32) as i32, (rh / 100.0 * h as f32) as i32]
    }
}

/// What a splash page does when the screen's media ends (`LabelShowPage`'s specials).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HudReturn {
    #[default]
    None,
    /// `hidehudplay`: the page to return to, its background label shown again.
    RestoreHud,
    /// `returnplay`: the page to return to.
    RestorePage,
}

/// The window ordering a play asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reorder {
    Back,
    Front,
}

pub struct Screen {
    /// The pack's folder, which a label's picture may also be named from.
    pack: Option<PathBuf>,
    pub num: i32,
    pub description: String,
    pub mode: Mode,
    pub topmost: bool,
    pub transparent: bool,
    /// What its plays' volumes are scaled by, in percent (`screens.pup`'s volume column, 100
    /// where it's empty or 0; `SendMSG` 26).
    pub volume_multiplier: i32,
    /// The volume the script's plays take (`setVolume`), 1 to 200.
    pub default_volume: i32,
    /// The volume of what it plays now, 1 to 200.
    pub current_volume: i32,
    /// The player's own volume, 0 to 1.
    main_volume: f32,
    /// Its file ducks the others: the screens' volumes come back when its main media ends.
    duck_master: bool,
    /// Its volume before it was ducked, or -1.
    duck_orig_volume: i32,
    /// A volume slide: where it is (-1: none), where it goes, by how much each step, and when
    /// the next step comes (the game's milliseconds).
    fade_volume: i32,
    fade_target: i32,
    volume_fade_step: i32,
    fade_next_tick: u64,
    pub custom_pos: Option<CustomPos>,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub playlists: Vec<Playlist>,
    /// Its triggers, grouped by their events as written, in the order the pack lists them.
    pub triggers: Vec<(String, Vec<Trigger>)>,
    pub labels: Vec<Label>,
    label_index: HashMap<String, usize>,
    label_init: bool,
    pub page: i32,
    default_page: i32,
    page_expiry: Option<u64>,
    /// Its labels shown at all (`SendMSG` 3).
    pub hud_visible: bool,
    /// Its labels hidden, and the script's label commands ignored, until its media ends
    /// (`SendMSG` 34).
    pub hide_during_play: bool,
    pub hud_return: HudReturn,
    pub screen_alpha: f32,
    /// The opacity of its overlay and labels, with the screen's (`SendMSG` 22).
    pub overlay_alpha: f32,
    pub pad_text_always: bool,
    /// What it plays.
    pub media: MediaManager,
    /// The pictures behind, and in place of, its media (an overlay is its background label's).
    pub background: Option<PathBuf>,
    pub static_image: Option<PathBuf>,
    image_expiry: Option<u64>,
    /// Its place in its window, in pixels: left, top, width, height.
    pub rect: [i32; 4],
    /// It's drawn in one of the player's windows (`Pup::update_seen`): its pictures are decoded.
    pub in_window: bool,
}

impl Screen {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pack: Option<&Path>,
        mode: Mode,
        num: i32,
        description: &str,
        transparent: bool,
        volume_multiplier: i32,
        custom_pos: Option<CustomPos>,
        playlists: &[Playlist],
    ) -> Screen {
        let mut s = Screen {
            pack: pack.map(Path::to_path_buf),
            num,
            description: description.to_string(),
            mode,
            topmost: matches!(mode, Mode::ForceOn | Mode::ForcePop),
            transparent,
            volume_multiplier,
            default_volume: 100,
            current_volume: 100,
            main_volume: 1.0,
            duck_master: false,
            duck_orig_volume: -1,
            fade_volume: -1,
            fade_target: 0,
            volume_fade_step: 0,
            fade_next_tick: 0,
            custom_pos,
            parent: None,
            children: Vec::new(),
            // each screen keeps a copy of the pack's playlists
            playlists: playlists.to_vec(),
            triggers: Vec::new(),
            labels: Vec::new(),
            label_index: HashMap::new(),
            label_init: false,
            page: 0,
            default_page: 0,
            page_expiry: None,
            hud_visible: true,
            hide_during_play: false,
            hud_return: HudReturn::None,
            screen_alpha: 1.0,
            overlay_alpha: 1.0,
            pad_text_always: false,
            media: MediaManager::default(),
            background: None,
            static_image: None,
            image_expiry: None,
            rect: [0; 4],
            in_window: false,
        };
        if let Some(pack) = pack {
            s.load_triggers(pack);
        }
        s
    }

    /// A line of `screens.pup` (VPX's `CreateFromCSV`), its background playlist started — at a
    /// volume of 0, which comes to the least, 1%, as in PinUP Player.
    pub fn from_csv(pack: &Path, line: &str, playlists: &[Playlist], fonts: &FontLibrary, now: u64) -> Option<Screen> {
        let parts = parse_csv_line(line);
        if parts.len() != 8 {
            return None;
        }
        let multiplier = string_to_int(&parts[6], 100);
        let mut s = Screen::new(
            Some(pack),
            Mode::parse(&parts[5]),
            string_to_int(&parts[0], 0),
            &parts[1],
            parts[4] == "1",
            if multiplier > 0 { multiplier } else { 100 },
            CustomPos::from_csv(&parts[7]),
            playlists,
        );
        if !parts[2].is_empty()
            && let Some(p) = s.playlist(&parts[2])
        {
            s.play(p, &parts[3], 0.0, -1, PlayAction::SetBG, 0, fonts, now);
        }
        Some(s)
    }

    /// The screen `Init` makes for a number (VPX's `CreateDefault`).
    pub fn default_for(num: i32, pack: Option<&Path>, playlists: &[Playlist], _now: u64) -> Screen {
        let description = match num {
            super::SCREEN_TOPPER => "Topper",
            super::SCREEN_DMD => "DMD",
            super::SCREEN_BACKGLASS => "Backglass",
            super::SCREEN_PLAYFIELD => "Playfield",
            _ => "Unknown",
        };
        Screen::new(pack, Mode::Show, num, description, false, 100, None, playlists)
    }

    /// Its triggers in the pack's `triggers.pup`.
    fn load_triggers(&mut self, pack: &Path) {
        let Some(file) = super::top_file(pack, "triggers.pup") else { return };
        let Ok(bytes) = std::fs::read(file) else { return };
        let text: String = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
        };
        for line in text.lines().skip(1) {
            if let Some(t) = Trigger::from_csv(self.num, &self.playlists, line) {
                match self.triggers.iter_mut().find(|(k, _)| *k == t.trigger) {
                    Some((_, group)) => group.push(t),
                    None => self.triggers.push((t.trigger.clone(), vec![t])),
                }
            }
        }
    }

    pub fn is_pop(&self) -> bool {
        matches!(self.mode, Mode::ForcePop | Mode::ForcePopBack)
    }

    /// Changes how it shows; a screen going out of sight stops its media.
    pub fn set_mode(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        let was = self.mode.visible();
        self.mode = mode;
        if was && !mode.visible() {
            self.media.stop();
        }
    }

    /// A play's volume (VPX's `ResolveVolume`): -1 for its playlist's, else as given, 1 to 200.
    fn resolve_volume(&self, volume: f32, playlist: usize) -> i32 {
        let mut v = volume.round() as i32;
        if v == -1
            && let Some(p) = self.playlists.get(playlist)
        {
            v = p.volume.round() as i32;
        }
        v.clamp(1, VOLUME_MAX)
    }

    /// What a volume plays at, in percent (VPX's `EffectiveVolume`): scaled by the screen's
    /// multiplier, 1 to 100, times the player's volume.
    fn effective_volume(&self, volume: i32) -> f32 {
        let scaled = ((f64::from(volume) * f64::from(self.volume_multiplier) / 100.0).round() as i32).clamp(1, 100);
        self.main_volume * scaled as f32
    }

    fn apply_volume(&mut self, volume: i32) {
        self.current_volume = volume;
        let effective = self.effective_volume(volume);
        self.media.set_volume(effective);
    }

    /// The player's own volume, 0 to 1.
    pub fn set_main_volume(&mut self, volume: f32) {
        self.main_volume = volume;
        let effective = self.effective_volume(self.current_volume);
        self.media.set_volume(effective);
    }

    /// `SendMSG` 26.
    pub fn set_volume_multiplier(&mut self, multiplier: i32) {
        self.volume_multiplier = multiplier.max(0);
        let effective = self.effective_volume(self.current_volume);
        self.media.set_volume(effective);
    }

    /// `setVolume`, `SendMSG` 10: the volume of the script's plays from now on.
    pub fn set_default_volume(&mut self, volume: i32) {
        self.default_volume = volume.clamp(1, VOLUME_MAX);
    }

    /// `setVolumeCurrent`, `SendMSG` 11 and 40: what plays now, at once.
    pub fn set_volume_current(&mut self, volume: i32) {
        self.fade_volume = -1;
        self.apply_volume(volume.clamp(1, VOLUME_MAX));
    }

    /// The `-` volume files duck the others to, where it's set (`SendMSG` 54; -1: none).
    pub fn set_default_duck(&mut self, percent: i32) {
        self.media.default_duck = percent;
    }

    fn start_volume_fade(&mut self, from: i32, to: i32, step: i32, now: u64) {
        self.fade_volume = from;
        self.fade_target = to;
        self.volume_fade_step = step;
        self.fade_next_tick = now;
    }

    /// A volume slide's next step, when it's due (VPX's `UpdateVolumeFade`, each frame).
    pub fn update_volume_fade(&mut self, now: u64) {
        if self.fade_volume < 0 || now < self.fade_next_tick {
            return;
        }
        self.fade_next_tick = now + VOLUME_FADE_MS;
        self.fade_volume = if self.fade_volume > self.fade_target {
            self.fade_target.max(self.fade_volume - self.volume_fade_step)
        } else {
            self.fade_target.min(self.fade_volume + self.volume_fade_step)
        };
        self.apply_volume(self.fade_volume);
        if self.fade_volume == self.fade_target {
            self.fade_volume = -1;
        }
    }

    /// Ducked to `percent` of its volume, sliding there, if it plays anything (VPX's `Duck`).
    pub fn duck(&mut self, percent: i32, now: u64) {
        if !self.media.is_main_playing() && !self.media.is_background_playing() {
            return;
        }
        if self.duck_orig_volume < 0 {
            self.duck_orig_volume = self.current_volume;
        }
        let target = ((f64::from(percent) / 100.0 * f64::from(self.duck_orig_volume)).round() as i32).clamp(1, VOLUME_MAX);
        self.start_volume_fade(self.current_volume, target, 5, now);
    }

    /// Its volume back from a duck (VPX's `Unduck`).
    pub fn unduck(&mut self, now: u64) {
        if self.duck_orig_volume < 0 {
            return;
        }
        let orig = self.duck_orig_volume;
        self.duck_orig_volume = -1;
        if self.media.is_main_playing() || self.media.is_background_playing() {
            self.start_volume_fade(self.current_volume, orig, 6, now);
        } else {
            self.apply_volume(orig);
        }
    }

    pub fn is_duck_master(&self) -> bool {
        self.duck_master
    }

    pub fn set_duck_master(&mut self, master: bool) {
        self.duck_master = master;
    }

    /// A playlist by its folder's name, ignoring case.
    pub fn playlist(&self, folder: &str) -> Option<usize> {
        self.playlists.iter().position(|p| p.folder.eq_ignore_ascii_case(folder))
    }

    /// Adds a playlist (`playlistadd`); false where one of that folder is there already.
    pub fn add_playlist(&mut self, p: Playlist) -> bool {
        if self.playlist(&p.folder).is_some() {
            return false;
        }
        self.playlists.push(p);
        true
    }

    pub fn is_label_init(&self) -> bool {
        self.label_init
    }

    /// Readies its labels (VPX's `LabelInit`): PinUP's label canvas always starts with a
    /// `pBackground` label, behind the others.
    pub fn label_init(&mut self, fonts: &FontLibrary) {
        self.label_init = true;
        if !self.has_label("pBackground") {
            self.add_label(Label::new("pBackground", "Liberation Sans", fonts, 50.0, 0xFF, 0.0, XAlign::Center, YAlign::Center, 0.0, 0.0, -1, true));
            self.reorder_label("pBackground", super::label::Reorder::Back);
        }
    }

    /// Adds a label; false where one of that name is there already.
    pub fn add_label(&mut self, label: Label) -> bool {
        let key = label.name.to_lowercase();
        if self.label_index.contains_key(&key) {
            return false;
        }
        self.label_index.insert(key, self.labels.len());
        self.labels.push(label);
        true
    }

    pub fn label_mut(&mut self, name: &str) -> Option<&mut Label> {
        let i = *self.label_index.get(&name.to_lowercase())?;
        self.labels.get_mut(i)
    }

    pub fn has_label(&self, name: &str) -> bool {
        self.label_index.contains_key(&name.to_lowercase())
    }

    /// Draws a label behind the others, or in front (VPX's `SendLabelToBack`, `…ToFront`); or
    /// just in front of the first, the background label (VPX's `SendLabelToBackground`).
    pub fn reorder_label(&mut self, name: &str, to: super::label::Reorder) {
        let Some(&i) = self.label_index.get(&name.to_lowercase()) else { return };
        let label = self.labels.remove(i);
        match to {
            super::label::Reorder::Back => self.labels.insert(0, label),
            super::label::Reorder::Front => self.labels.push(label),
            super::label::Reorder::Background => self.labels.insert(1.min(self.labels.len()), label),
        }
        self.reindex_labels();
    }

    /// Removes a label (VPX's `RemoveLabel`, a disposed one's).
    pub fn remove_label(&mut self, name: &str) {
        let Some(&i) = self.label_index.get(&name.to_lowercase()) else { return };
        self.labels.remove(i);
        self.reindex_labels();
    }

    fn reindex_labels(&mut self) {
        self.label_index = self.labels.iter().enumerate().map(|(k, l)| (l.name.to_lowercase(), k)).collect();
    }

    /// Shows a page of labels (VPX's `SetPage`, as PinUP's `TtextForm.SetPageNum`): for
    /// `duration_ms` (a splash, then back), or as the page to return to (0 or less). `hidehudplay`
    /// and `returnplay` say what comes when the screen's media ends, `forceshow` and `forcehide`
    /// only show or hide the page's labels; the page showing asked again with nothing to come
    /// only ends its labels' animations and makes its splash longer; otherwise the page changes,
    /// every label's animation ends, and the new page's labels are emptied (`clear`) and shown as
    /// made to (from page 2 on, unless `dodefault` is off). Nothing while media plays over held-
    /// back labels.
    pub fn set_page(&mut self, page: i32, duration_ms: i32, special: &str, now: u64) {
        if self.hide_during_play {
            return;
        }
        let lower = special.to_lowercase();
        if lower == "hidehudplay" {
            self.hud_return = HudReturn::RestoreHud;
            if let Some(background) = self.label_mut("pBackground")
                && !background.is_animating()
            {
                background.visible = false;
            }
        } else if lower == "returnplay" {
            self.hud_return = HudReturn::RestorePage;
        } else {
            self.hud_return = HudReturn::None;
        }
        // as VPX compares them, in their case
        if special == "forceshow" || special == "forcehide" {
            if page >= 0 {
                for l in self.labels.iter_mut().filter(|l| l.page == page) {
                    l.visible = special == "forceshow" && l.on_show_visible;
                }
            }
            return;
        }
        if page == self.page && self.hud_return == HudReturn::None {
            for l in &mut self.labels {
                l.clear_animation();
            }
            if let Some(t) = self.page_expiry.as_mut()
                && duration_ms > 0
            {
                *t += duration_ms as u64;
            }
            return;
        }
        self.page = page;
        for l in &mut self.labels {
            l.clear_animation();
            if l.page != page {
                continue;
            }
            if lower == "clear" {
                l.clear_caption();
            }
            if page > 1 && l.do_default {
                l.visible = l.on_show_visible;
            }
        }
        if duration_ms > 0 {
            self.page_expiry = Some(now + duration_ms as u64);
        } else {
            self.page_expiry = None;
            if page >= 0 {
                self.default_page = page;
            }
        }
    }

    /// A splash page's end, and a picture's.
    pub fn update_timers(&mut self, now: u64) {
        if self.page_expiry.is_some_and(|t| now >= t) {
            self.page_expiry = None;
            self.set_page(self.default_page, 0, "", now);
        }
        if self.image_expiry.is_some_and(|t| now >= t) {
            self.image_expiry = None;
            self.static_image = None;
        }
    }

    /// Its main media ended: held-back labels shown again, what a splash page asked for then,
    /// and its own volume slide and duck forgotten. Returns whether its file was ducking the
    /// others (whose volumes then come back).
    pub fn on_main_media_end(&mut self, now: u64) -> bool {
        if self.hide_during_play {
            self.hide_during_play = false;
            self.hud_visible = true;
        }
        match std::mem::take(&mut self.hud_return) {
            HudReturn::RestoreHud => {
                self.set_page(self.default_page, 0, "", now);
                if let Some(background) = self.label_mut("pBackground")
                    && !background.is_animating()
                {
                    background.visible = true;
                }
            }
            HudReturn::RestorePage => self.set_page(self.default_page, 0, "", now),
            HudReturn::None => {}
        }
        self.fade_volume = -1;
        self.duck_orig_volume = -1;
        std::mem::take(&mut self.duck_master)
    }

    /// Its place in its window of `w` × `h` pixels from `x`, `y`, from its custom place (VPX's `SetBounds`).
    pub fn set_bounds(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let mut r = self.custom_pos.map_or([0, 0, w, h], |c| c.scaled(w, h));
        r[0] += x;
        r[1] += y;
        self.rect = r;
    }

    pub fn set_custom_pos(&mut self, text: &str) {
        self.custom_pos = CustomPos::from_csv(text);
    }

    /// Plays a playlist by name (VPX's `Play(string, …)`).
    #[allow(clippy::too_many_arguments)]
    pub fn play_named(&mut self, playlist: &str, file: &str, volume: f32, priority: i32, action: PlayAction, length: i32, fonts: &FontLibrary, now: u64) -> Option<Reorder> {
        let p = self.playlist(playlist)?;
        self.play(p, file, volume, priority, action, length, fonts, now)
    }

    /// Plays from a playlist (VPX's `PUPScreen::Play`): a picture shows in place of video until
    /// replaced; a playlist of frames or shapes sets that picture, one of overlays the background
    /// label's. A file played takes its volume (-1: its playlist's) as the screen's current one.
    /// Returns the window ordering the screen's mode asks for.
    #[allow(clippy::too_many_arguments)]
    pub fn play(&mut self, p: usize, file: &str, volume: f32, priority: i32, action: PlayAction, length: i32, fonts: &FontLibrary, now: u64) -> Option<Reorder> {
        let background = action == PlayAction::SetBG;
        match self.playlists[p].function {
            Function::Default => {
                let extension = Path::new(file).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
                if matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "bmp") {
                    let path = self.playlists[p].play_file_path(file);
                    if background {
                        self.background = path;
                        self.media.stop_background();
                    } else {
                        self.stop();
                        self.static_image = path;
                        self.image_expiry = (length > 0).then(|| now + length as u64 * 1000);
                    }
                    return None;
                }
                let reorder = match self.mode {
                    Mode::Off => return None,
                    Mode::ForcePopBack => Some(Reorder::Back),
                    Mode::ForcePop => Some(Reorder::Front),
                    _ => None,
                };
                self.static_image = None;
                let volume = self.resolve_volume(volume, p);
                let effective = self.effective_volume(volume);
                let pop = self.is_pop();
                self.media.seen = self.in_window && self.mode.visible();
                if self.media.play(&mut self.playlists[p], file, effective, priority, action, length, pop, now) {
                    self.current_volume = volume;
                }
                reorder
            }
            Function::Frames => {
                self.background = self.playlists[p].play_file_path(file);
                None
            }
            Function::Overlays | Function::Alphas => {
                // PinUP makes the file the background label's picture, stretched over the labels'
                // canvas: it follows that label's visibility and sits below the others
                self.label_init(fonts);
                let caption = format!("{}\\{file}", self.playlists[p].folder);
                let mut playlists = std::mem::take(&mut self.playlists);
                let pack = self.pack.clone();
                let pad = self.pad_text_always;
                if let Some(background) = self.label_mut("pBackground") {
                    background.set_caption(&caption, &mut playlists, pack.as_deref(), pad, now);
                    background.set_special(r#"{"mt":2, "width":100, "height":100, "xpos":50, "ypos":50}"#, fonts, now);
                }
                self.playlists = playlists;
                None
            }
            Function::Shapes => {
                self.media.mask = self.playlists[p].play_file_path(file);
                None
            }
        }
    }

    pub fn stop(&mut self) {
        self.media.stop();
        self.image_expiry = None;
    }

    pub fn stop_priority(&mut self, priority: i32) {
        self.media.stop_priority(priority);
    }

    pub fn stop_file(&mut self, p: usize, file: &str) {
        let path = self.playlists.get_mut(p).and_then(|pl| pl.play_file_path(file));
        self.media.stop_file(path.as_deref());
    }

    pub fn set_length(&mut self, length: i32, now: u64) {
        self.media.max_length = length;
        if length > 0 && self.static_image.is_some() {
            self.image_expiry = Some(now + length as u64 * 1000);
        }
    }

    pub fn is_main_playing(&self) -> bool {
        self.media.is_main_playing()
    }
}

/// One media on a screen's queue.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayItem {
    pub path: PathBuf,
    pub volume: f32,
    pub priority: i32,
    pub length: i32,
    pub looping: bool,
}

/// The file a screen loops when nothing else plays.
#[derive(Clone, Debug, Default)]
pub struct Background {
    pub path: Option<PathBuf>,
    pub volume: f32,
    pub active: bool,
    /// Set by `SetBackGround` (which `SetBackGround 0` clears), not by a trigger's SetBG.
    set_by_script: bool,
}

/// What a screen plays (VPX's `PUPMediaManager`): a queue — the head plays,
/// a splash pushed over it returns to it, a queued play waits after it —
/// over a looping background. The media itself is played by [`Player`].
pub struct MediaManager {
    pub queue: VecDeque<PlayItem>,
    pub background: Background,
    pub player: Player,
    /// A picture whose top-left colour is cut out of what the screen shows (`PuPShapes`).
    pub mask: Option<PathBuf>,
    pub max_length: i32,
    pub fade_step: i32,
    pub current_alpha: i32,
    /// Its screen is drawn in a window: the files it plays have their pictures decoded.
    pub seen: bool,
    /// The percent a `-` file without its own two digits ducks the others to (`SendMSG` 54; -1: none).
    pub default_duck: i32,
    /// A file starting that ducks every other screen, to this percent (VPX's `ApplyDuckPrefix`),
    /// for the pack to do (`Pup::apply_ducks`).
    pub duck_request: Option<i32>,
}

impl Default for MediaManager {
    fn default() -> MediaManager {
        MediaManager {
            queue: VecDeque::new(),
            background: Background::default(),
            player: Player::default(),
            mask: None,
            max_length: 0,
            // no fade: a video shows at once (VPX's defaults)
            fade_step: 255,
            current_alpha: 255,
            seen: false,
            default_duck: -1,
            duck_request: None,
        }
    }
}

impl MediaManager {
    /// The game's time has moved on: a finished head gives way to the next, or the background.
    /// True when the main media ended with nothing queued after it.
    pub fn update(&mut self, now: u64) -> bool {
        self.player.set_game_time(now);
        if !self.queue.is_empty() && !self.player.is_playing() {
            return self.on_player_end(now);
        }
        false
    }

    /// Plays a file (VPX's `Play`): priorities decide, a resting playlist waits, SetBG only sets
    /// the background (left playing where it plays already), a queued play waits after what
    /// plays. True when it played, or was set to (a play skipped because it plays already
    /// counts, unless it's the background's).
    #[allow(clippy::too_many_arguments)]
    pub fn play(&mut self, playlist: &mut Playlist, file: &str, volume: f32, priority: i32, action: PlayAction, length: i32, pop: bool, now: u64) -> bool {
        let _ = pop;
        let is_bg = action == PlayAction::SetBG;
        let main_playing = !self.queue.is_empty();
        let current = self.queue.front().map_or(0, |i| i.priority);
        if !is_bg && priority == 999 {
            if main_playing {
                self.stop();
            }
            return false;
        }
        if !is_bg && main_playing && priority < current && action != PlayAction::QueueAlways {
            return false;
        }
        if action == PlayAction::SkipSamePriority && main_playing && priority > 0 && priority == current {
            return false;
        }
        let preempting = priority > 0 && priority > current;
        if action == PlayAction::Normal && !preempting && playlist.is_resting(now) {
            return false;
        }
        let Some(path) = playlist.play_file_path(file) else { return false };
        if is_bg {
            let bg_playing = !main_playing && self.background.active && self.background.path.as_ref() == Some(&path) && self.player.is_playing();
            self.background.path = Some(path);
            self.background.volume = volume;
            self.background.active = true;
            playlist.mark_played(now);
            if bg_playing {
                self.player.set_volume(volume);
            } else if !main_playing {
                self.play_background(now);
            }
            return true;
        }
        if main_playing && self.queue.front().is_some_and(|i| i.path == path) {
            self.player.set_volume(volume);
            playlist.mark_played(now);
            return true;
        }
        if !main_playing && self.background.active && self.background.path.as_ref() == Some(&path) && self.player.is_playing() {
            playlist.mark_played(now);
            return false;
        }
        let item = PlayItem { path, volume, priority, length, looping: action == PlayAction::Loop };
        if matches!(action, PlayAction::QueueSameP | PlayAction::QueueAlways) && main_playing {
            // next after what plays (VPX's insert at the queue's second place)
            self.queue.insert(1, item);
            playlist.mark_played(now);
            return true;
        }
        let splash = matches!(action, PlayAction::SplashReset | PlayAction::SplashReturn);
        if !splash {
            self.queue.pop_front();
        }
        self.queue.push_front(item);
        self.start_current(now);
        playlist.mark_played(now);
        true
    }

    fn start_current(&mut self, now: u64) {
        let Some(head) = self.queue.front().cloned() else { return };
        self.player.play(&head.path, head.volume, head.looping, self.seen, now);
        self.player.length = head.length;
        self.current_alpha = if self.fade_step < 255 { self.fade_step } else { 255 };
        self.duck_prefix(&head.path);
    }

    fn play_background(&mut self, now: u64) {
        if self.background.active
            && let Some(path) = self.background.path.clone()
        {
            let volume = self.background.volume;
            self.player.play(&path, volume, true, self.seen, now);
            self.duck_prefix(&path);
        }
    }

    /// A file whose name starts with `-` ducks the other screens while it plays: to the percent
    /// its next two digits give (00 is 1%), else the screen's default duck where one is set
    /// (VPX's `ApplyDuckPrefix`).
    fn duck_prefix(&mut self, path: &Path) {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let b = name.as_bytes();
        if b.len() < 3 || b[0] != b'-' {
            return;
        }
        let mut percent = self.default_duck;
        if b[1].is_ascii_digit() && b[2].is_ascii_digit() {
            percent = i32::from(b[1] - b'0') * 10 + i32::from(b[2] - b'0');
            if percent == 0 {
                percent = 1;
            }
        }
        if percent > 0 {
            self.duck_request = Some(percent);
        }
    }

    pub fn pause(&mut self) {
        self.player.set_paused(true);
    }

    pub fn resume(&mut self) {
        self.player.set_paused(false);
    }

    /// `SetBackGround 1`: the playing file becomes the background (not on a pop-up screen);
    /// `SetBackGround 0` clears one set so.
    pub fn set_as_background(&mut self, on: bool, pop: bool, now: u64) {
        if on {
            if pop {
                return;
            }
            if let Some(head) = self.queue.front().cloned() {
                self.background = Background { path: Some(head.path), volume: head.volume, active: true, set_by_script: true };
                self.player.set_looping(true);
                self.queue.clear();
            }
        } else if self.background.set_by_script {
            self.background = Background::default();
            if self.queue.is_empty() {
                self.player.stop();
            }
        }
        let _ = now;
    }

    pub fn set_loop(&mut self, on: bool) {
        self.player.set_looping(on);
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.player.set_volume(volume);
    }

    /// Stops the main media; the background plays again if there is one, unless it plays
    /// already (VPX no longer starts it over).
    pub fn stop(&mut self) {
        let main_playing = !self.queue.is_empty();
        self.queue.clear();
        if self.background.active && self.background.path.is_some() {
            if main_playing || !self.player.is_playing() {
                let now = self.player.now;
                self.play_background(now);
            }
        } else {
            self.player.stop();
        }
    }

    pub fn stop_background(&mut self) {
        self.background.path = None;
        self.background.active = false;
        if self.queue.is_empty() {
            self.player.stop();
        }
    }

    /// Stops the main media if `priority` is 0 or higher than its own.
    pub fn stop_priority(&mut self, priority: i32) {
        let current = self.queue.front().map_or(0, |i| i.priority);
        if priority == 0 || priority > current {
            self.stop();
        }
    }

    /// Stops the main media if it's the file named.
    pub fn stop_file(&mut self, path: Option<&Path>) {
        if path.is_some() && self.queue.front().map(|i| i.path.as_path()) == path {
            self.stop();
        }
    }

    pub fn set_fade_step(&mut self, step: i32) {
        self.fade_step = if step < 0 { 255 } else { step.min(255) };
        if self.fade_step >= 255 {
            self.current_alpha = 255;
        }
    }

    /// The head ended: the next queued plays, else the background. True when nothing was queued after it.
    fn on_player_end(&mut self, now: u64) -> bool {
        self.queue.pop_front();
        if !self.queue.is_empty() {
            self.start_current(now);
            return false;
        }
        self.play_background(now);
        true
    }

    pub fn is_main_playing(&self) -> bool {
        !self.queue.is_empty() && self.player.is_playing()
    }

    pub fn is_background_playing(&self) -> bool {
        self.background.active && self.background.path.is_some() && self.queue.is_empty() && self.player.is_playing()
    }

    /// Its screen now shows in one of the player's windows: what it plays has its pictures
    /// decoded from here on (a screen's background starts as the screen is made, before the
    /// pack places it in a window).
    pub fn see(&mut self) {
        if !self.seen {
            self.seen = true;
            self.player.see();
        }
    }
}

/// Plays one media file at a time on a screen: the pack's state follows the length the file's
/// headers give (or ends at once where none can be read), so what the scripts see about
/// playing and ending holds whether or not it's decoded; where the player has sound, the file
/// is heard too ([`super::playback`]). Its pictures on the screens come with the screens'
/// drawing: from one file to the next, the last picture shown stays until the next file's
/// first, as in VPX (none for a file of sound only, or one that can't be read; none after a
/// stop).
///
/// Its clock is the game's, as VPX gives its media players the game's time; where the game's
/// clock stands still for more than 100 ms, what plays goes on with the wall clock, as in VPX
/// (not in test mode, whose pictures repeat).
#[derive(Default)]
pub struct Player {
    pub file: Option<PathBuf>,
    volume: f32,
    looping: bool,
    /// Seconds after which it stops (0: none).
    pub length: i32,
    paused: bool,
    /// When it started, on its clock.
    started: u64,
    /// The file's length in milliseconds, where known.
    duration: Option<u64>,
    /// The game's time last given.
    pub now: u64,
    /// Its clock last: the game's time, with the time the game's clock stood still added.
    clock: u64,
    /// What's added to the game's time, and what was before the game's clock last stood still.
    offset: u64,
    offset_base: u64,
    /// The game's time when it last moved on, and the wall clock then.
    last_advance: Option<(u64, std::time::Instant)>,
    /// The file decoding and heard, where the player has sound.
    playback: Option<super::playback::Playback>,
    /// The last picture shown, and the one held from the file before until this one's first.
    last_shown: Option<Arc<super::playback::Picture>>,
    held: Option<Arc<super::playback::Picture>>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player").field("file", &self.file).field("volume", &self.volume).field("looping", &self.looping).field("paused", &self.paused).finish()
    }
}

impl Player {
    pub fn play(&mut self, path: &Path, volume: f32, looping: bool, seen: bool, now: u64) {
        // the last picture shown stays until this file's first (VPX's held texture)
        if let Some(shown) = self.last_shown.take() {
            self.held = Some(shown);
        }
        self.file = Some(path.to_path_buf());
        self.volume = volume;
        self.looping = looping;
        self.now = now;
        self.clock = now + self.offset;
        self.started = self.clock;
        self.paused = false;
        self.length = 0;
        self.duration = crate::pup::media_length(path);
        self.playback = super::playback::Playback::start(path, volume, looping, seen);
    }

    /// Stopped: nothing shows.
    pub fn stop(&mut self) {
        self.end();
        self.last_shown = None;
        self.held = None;
    }

    /// Played to its end: nothing shows until the next file, which its last picture waits for.
    fn end(&mut self) {
        self.file = None;
        self.playback = None;
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        if let Some(p) = &self.playback {
            p.set_volume(volume);
        }
    }

    pub fn looping(&self) -> bool {
        self.looping
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
        if let Some(p) = &self.playback {
            p.set_looping(looping);
        }
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if let Some(p) = &self.playback {
            p.set_paused(paused);
        }
    }

    pub fn set_game_time(&mut self, now: u64) {
        // the game's clock standing still (VPX's SetGameTime): after 100 ms, the wall clock's time
        if !super::playback::exact() {
            let wall = std::time::Instant::now();
            match self.last_advance {
                Some((t, at)) if now <= t => {
                    let stood = wall.duration_since(at).as_millis() as u64;
                    if stood > 100 {
                        self.offset = self.offset_base + stood;
                    }
                }
                _ => {
                    self.last_advance = Some((now, wall));
                    self.offset_base = self.offset;
                }
            }
        }
        let clock = now + self.offset;
        if self.paused {
            // a paused file holds its place
            self.started += clock.saturating_sub(self.clock);
        }
        self.now = now;
        self.clock = clock;
        if let Some(p) = &self.playback {
            p.set_position(clock.saturating_sub(self.started) as i64);
        }
        if self.file.is_some() && !self.looping {
            let played = clock.saturating_sub(self.started);
            let limit = if self.length > 0 { Some(self.length as u64 * 1000) } else { None };
            let ends = match (self.duration, limit) {
                (Some(d), Some(l)) => Some(d.min(l)),
                (d, l) => d.or(l),
            };
            if ends.is_none_or(|e| played >= e) {
                self.end();
            }
        }
    }

    /// Its screen now shows in a window: the file playing is decoded again with its pictures,
    /// from its place in the game's time (its sound starts again with it; this comes as the
    /// screen is made, when the file has only just started).
    pub fn see(&mut self) {
        if self.playback.as_ref().is_some_and(super::playback::Playback::shows_pictures) {
            return;
        }
        let Some(path) = self.file.clone() else { return };
        self.playback = super::playback::Playback::start(&path, self.volume, self.looping, true);
        if let Some(p) = &self.playback {
            p.set_paused(self.paused);
            p.set_position(self.clock.saturating_sub(self.started) as i64);
        }
    }

    /// The picture showing now, where the file's pictures are decoded: the file's, else the one
    /// held from the file before until the file's first picture (none where the file turns out
    /// to have no pictures, as VPX drops its held picture for a file of sound only or one it
    /// can't open).
    pub fn picture(&mut self) -> Option<Arc<super::playback::Picture>> {
        let playback = self.playback.as_ref()?;
        if let Some(p) = playback.picture() {
            self.held = None;
            self.last_shown = Some(p.clone());
            return Some(p);
        }
        if playback.has_no_pictures() {
            self.held = None;
        }
        self.held.clone()
    }

    pub fn is_playing(&self) -> bool {
        self.file.is_some()
    }
}
