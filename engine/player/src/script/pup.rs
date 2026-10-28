// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its PinUp Player
// plugin's script object (plugins/pup/PUPPlugin.cpp's PUP_PinDisplay class, PUPPinDisplay.cpp), with the newer
// fixes matching PinUP Player (to commit a4a1e75ca: volumes, 108a65d69; Show and more SendMSG
// functions, 781f88258; LabelShowPage's specials and queued plays, 59536ebbb; the version, 66e97a62b).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! `CreateObject("PinUpPlayer.PinDisplay")`: PinUP Player's object, which
//! original tables drive their packs with: play a
//! playlist on a screen, add a screen, set a label, send a message. Every
//! member VPX 5436's plugin offers, doing what it does there, on our pack
//! player (`crate::pup`); those VPX leaves undone are accepted and do
//! nothing, as there. One object per table, which every `CreateObject`
//! gets (VPX makes a new proxy each time, over the same pack).

use std::collections::HashSet;

use super::com::*;
use super::idl_types::{Access, Interface, Member, Param, Ty};
use super::object::{Obj, Target};
use super::value::{Args, Ret};
use super::with;
use crate::pup::label::{Label, XAlign, YAlign};
use crate::pup::playlist::Playlist;
use crate::pup::screen::Mode;
use crate::pup::trigger::PlayAction;
use crate::pup::Pup;

type Res = Result<Ret, HRESULT>;

const fn param(name: &'static str, ty: Ty) -> Param {
    Param { name, ty, optional: false, default: None, by_ref: false }
}

const fn method(name: &'static str, dispid: i32, params: &'static [Param], ret: Option<Ty>) -> Member {
    Member { name, dispid, access: Access::Method, params, ret }
}

const fn get(name: &'static str, dispid: i32, params: &'static [Param], ret: Ty) -> Member {
    Member { name, dispid, access: Access::Get, params, ret: Some(ret) }
}

const fn put(name: &'static str, dispid: i32, params: &'static [Param]) -> Member {
    Member { name, dispid, access: Access::Put, params, ret: None }
}

const NONE: &[Param] = &[];
const SCREEN: &[Param] = &[param("ScreenNum", Ty::Int)];
const SCREEN_INT: &[Param] = &[param("ScreenNum", Ty::Int), param("Value", Ty::Int)];

/// PinUP Player's `PinDisplay`, as VPX 5436's plugin offers it.
pub static I_PIN_DISPLAY: Interface = Interface {
    name: "IPinDisplay",
    members: &[
        method("Init", 1, &[param("ScreenNum", Ty::Int), param("RomName", Ty::Str)], None),
        method("playlistadd", 2, &[param("ScreenNum", Ty::Int), param("Folder", Ty::Str), param("Sort", Ty::Int), param("RestSeconds", Ty::Int)], None),
        method("playlistplay", 3, &[param("ScreenNum", Ty::Int), param("Playlist", Ty::Str)], None),
        method(
            "playlistplayex",
            4,
            &[param("ScreenNum", Ty::Int), param("Playlist", Ty::Str), param("PlayFile", Ty::Str), param("Volume", Ty::Int), param("Priority", Ty::Int)],
            None,
        ),
        method("play", 5, &[param("ScreenNum", Ty::Int), param("Playlist", Ty::Str), param("PlayFile", Ty::Str)], None),
        method("setWidth", 6, SCREEN_INT, None),
        method("setHeight", 7, SCREEN_INT, None),
        method("setPosX", 8, SCREEN_INT, None),
        method("setPosY", 9, SCREEN_INT, None),
        method("setAspect", 10, &[param("ScreenNum", Ty::Int), param("AspectWide", Ty::Int), param("AspectHigh", Ty::Int)], None),
        method("setVolume", 11, SCREEN_INT, None),
        method("playpause", 12, SCREEN, None),
        method("playresume", 13, SCREEN, None),
        method("playstop", 14, SCREEN, None),
        method("CloseApp", 15, NONE, None),
        get("isPlaying", 16, SCREEN, Ty::Bool),
        put("isPlaying", 16, SCREEN),
        method("SetLength", 17, SCREEN_INT, None),
        method("SetLoop", 18, SCREEN_INT, None),
        method("SetBackGround", 19, SCREEN_INT, None),
        method("BlockPlay", 20, SCREEN_INT, None),
        method("SetScreen", 21, SCREEN, None),
        method(
            "SetScreenEx",
            22,
            &[param("ScreenNum", Ty::Int), param("XPos", Ty::Int), param("YPos", Ty::Int), param("Width", Ty::Int), param("Height", Ty::Int), param("PopUp", Ty::Int)],
            None,
        ),
        get("SN", 23, NONE, Ty::Int),
        put("SN", 23, NONE),
        method("B2SData", 24, &[param("Index", Ty::Str), param("Value", Ty::Int)], None),
        get("B2SFilter", 25, NONE, Ty::Str),
        put("B2SFilter", 25, NONE),
        method("Show", 26, SCREEN, None),
        method("Hide", 27, SCREEN, None),
        method("B2SInit", 28, &[param("TName", Ty::Str), param("RomName", Ty::Str)], None),
        method("SendMSG", 29, &[param("Msg", Ty::Str)], None),
        method(
            "LabelNew",
            30,
            &[
                param("ScreenNum", Ty::Int),
                param("LabelName", Ty::Str),
                param("FontName", Ty::Str),
                param("Size", Ty::Int),
                param("Color", Ty::Int),
                param("Angle", Ty::Int),
                param("XAlign", Ty::Int),
                param("YAlign", Ty::Int),
                param("XMargin", Ty::Int),
                param("YMargin", Ty::Int),
                param("PageNum", Ty::Int),
                param("Visible", Ty::Bool),
            ],
            None,
        ),
        method(
            "LabelSet",
            31,
            &[param("ScreenNum", Ty::Int), param("LabelName", Ty::Str), param("Caption", Ty::Str), param("Visible", Ty::Bool), param("Special", Ty::Str)],
            None,
        ),
        method("LabelSetEx", 32, NONE, None),
        method("LabelShowPage", 33, &[param("ScreenNum", Ty::Int), param("PageNum", Ty::Int), param("Seconds", Ty::Int), param("Special", Ty::Str)], None),
        method("LabelInit", 34, SCREEN, None),
        get("GetGame", 35, NONE, Ty::Str),
        put("GetGame", 35, NONE),
        get("GetRoot", 36, NONE, Ty::Str),
        put("GetRoot", 36, NONE),
        method(
            "SoundAdd",
            37,
            &[param("SName", Ty::Str), param("FName", Ty::Str), param("SVol", Ty::Int), param("SX", Ty::Double), param("SY", Ty::Double), param("SP", Ty::Str)],
            None,
        ),
        method("SoundPlay", 38, &[param("SName", Ty::Str)], None),
        method(
            "PuPSound",
            39,
            &[param("SName", Ty::Str), param("SX", Ty::Int), param("SY", Ty::Int), param("SZ", Ty::Int), param("Vol", Ty::Int), param("SP", Ty::Str)],
            None,
        ),
        method("InitPuPMenu", 40, &[param("Param1", Ty::Int)], None),
        get("B2SDisplays", 41, NONE, Ty::Str),
        method("setVolumeCurrent", 42, SCREEN_INT, None),
        method("GameUpdate", 43, &[param("GameTitle", Ty::Str), param("Func", Ty::Int), param("FuncData", Ty::Int), param("Extra", Ty::Str)], Some(Ty::Int)),
        method("GetVersion", 44, NONE, Some(Ty::Str)),
        method(
            "playevent",
            45,
            &[
                param("ScreenNum", Ty::Int),
                param("Playlist", Ty::Str),
                param("PlayFile", Ty::Str),
                param("Volume", Ty::Int),
                param("Priority", Ty::Int),
                param("PlayType", Ty::Int),
                param("Seconds", Ty::Int),
                param("Special", Ty::Str),
            ],
            None,
        ),
        method(
            "SetPosVideo",
            46,
            &[param("ScreenNum", Ty::Int), param("StartPos", Ty::Int), param("EndPos", Ty::Int), param("Mode", Ty::Int), param("Special", Ty::Str)],
            None,
        ),
        method("PuPClose", 47, NONE, None),
    ],
};

/// The table's PinUP object and its pack.
pub struct ScriptPup {
    pub obj: *mut Obj,
    pub pup: Pup,
    /// Labels a script set that weren't there, each named once.
    warned: HashSet<(i32, String)>,
    /// The DMD's identify frame, read 60 times a second for the pack's frame triggers.
    pub(crate) identify_buffer: Vec<u8>,
}

impl ScriptPup {
    /// The pack player for the table at `table`, drawing in the player's `windows` (topper,
    /// backglass, score display), as the settings' `[Plugin.PUP]` choose.
    fn new(table: Option<&std::path::Path>, windows: [Option<(u32, u32)>; 3], settings: &crate::pup::PupSettings) -> ScriptPup {
        let mut pup = Pup::new(table, settings.root(table));
        pup.set_windows(windows);
        pup.apply_settings(settings);
        ScriptPup { obj: Obj::new(Target::PupDisplay, &I_PIN_DISPLAY), pup, warned: HashSet::new(), identify_buffer: Vec::new() }
    }
}

/// `CreateObject("PinUpPlayer.PinDisplay")`.
pub fn create() -> Res {
    with(|_, w| {
        let (table, windows, settings) = (w.table_path.clone(), w.pup_windows, w.pup_settings.clone());
        let p = w.pup.get_or_insert_with(|| Box::new(ScriptPup::new(table.as_deref(), windows, &settings)));
        let obj = p.obj;
        w.sounds.play_pup(crate::pup::playback::sink());
        Ret::Object(Obj::dispatch(obj))
    })
    .ok_or(E_FAIL)
}

/// A ROM's machine started (VPX's plugin, `OnControllersChanged`): the game's pack loads, unless
/// the script started one already. A table whose script never asks for PinUP gets its pack this
/// way, its triggers fired by the machine.
pub fn on_game_start(w: &mut crate::world::World, game: &str, now: u64) {
    if game.is_empty() {
        return;
    }
    if let Some(p) = &mut w.pup {
        if !p.pup.is_running() {
            p.pup.load_config(game, now);
        }
        return;
    }
    if !w.pup_settings.enabled {
        return;
    }
    let mut p = ScriptPup::new(w.table_path.as_deref(), w.pup_windows, &w.pup_settings);
    p.pup.load_config(game, now);
    if p.pup.path().is_some() {
        w.pup = Some(Box::new(p));
        w.sounds.play_pup(crate::pup::playback::sink());
    } else {
        // SAFETY: made just now, never handed to the script
        unsafe { Obj::free(p.obj) };
    }
}

/// One member (VPX 5436's `PUPPinDisplay`).
pub fn member(m: &'static Member, a: &Args) -> Option<Res> {
    let r = with(|e, w| {
        let now = u64::from(e.time_msec());
        let sp = w.pup.as_mut()?;
        let r = call(sp, m, a, now);
        // a file that started ducks the other screens at once
        sp.pup.apply_ducks(now);
        Some(r)
    })??;
    Some(r)
}

fn call(sp: &mut ScriptPup, m: &'static Member, a: &Args, now: u64) -> Res {
    let int = |i: usize| a.i32(i).unwrap_or(0);
    let text = |i: usize| a.string(i).unwrap_or_default();
    let pup = &mut sp.pup;
    // the screen a call names, if the pack has it
    let screen = |pup: &Pup| pup.screen(int(0));
    match (m.name, m.access) {
        ("Init", _) => {
            pup.set_game_dir(&text(1));
            pup.add_default_screen(int(0), now);
        }
        ("playlistadd", _) => {
            if let Some(s) = screen(pup) {
                let folder = text(1);
                let path = pup.path().map(std::path::Path::to_path_buf).unwrap_or_default();
                let screen = &mut pup.screens[s];
                if screen.playlist(&folder).is_none() {
                    screen.add_playlist(Playlist::new(&path, &folder, "", int(2) != 0, int(3), 100.0, 1));
                }
            }
        }
        ("playlistplay", _) => {
            // the playlist's next file, at the screen's default volume
            if let Some(s) = screen(pup)
                && pup.screens[s].playlist(&text(1)).is_some()
            {
                let volume = pup.screens[s].default_volume as f32;
                pup.play(s, &text(1), "", volume, 0, PlayAction::Normal, now);
            }
        }
        ("playlistplayex", _) => {
            if let Some(s) = screen(pup) {
                pup.play(s, &text(1), &text(2), int(3) as f32, int(4), PlayAction::Normal, now);
            }
        }
        ("play", _) => {
            if let Some(s) = screen(pup)
                && pup.screens[s].playlist(&text(1)).is_some()
            {
                let volume = pup.screens[s].default_volume as f32;
                pup.play(s, &text(1), &text(2), volume, 0, PlayAction::Normal, now);
            }
        }
        ("setVolume", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].set_default_volume(int(1));
            }
        }
        ("setVolumeCurrent", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].set_volume_current(int(1));
            }
        }
        ("playpause", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].media.pause();
            }
        }
        ("playresume", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].media.resume();
            }
        }
        ("playstop", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].stop();
            }
        }
        ("CloseApp" | "PuPClose", _) => pup.unload(),
        ("isPlaying", Access::Get) => return Ok(Ret::Bool(screen(pup).is_some_and(|s| pup.screens[s].is_main_playing()))),
        ("SetLength", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].set_length(int(1), now);
            }
        }
        ("SetLoop", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].media.set_loop(int(1) != 0);
            }
        }
        ("SetBackGround", _) => {
            if let Some(s) = screen(pup) {
                let pop = pup.screens[s].is_pop();
                pup.screens[s].media.set_as_background(int(1) != 0, pop, now);
            }
        }
        ("SetScreenEx", _) => {
            if let Some(s) = screen(pup) {
                pup.screens[s].set_mode(match int(5) {
                    0 => Mode::Show,
                    1 => Mode::ForcePop,
                    _ => Mode::MusicOnly,
                });
            }
        }
        ("B2SData", _) => {
            let index = text(0);
            let mut chars = index.chars();
            if let Some(kind) = chars.next() {
                pup.queue_dof_event(kind, crate::pup::string_to_int(chars.as_str(), 0), int(1));
            }
        }
        ("Show", _) => {
            if let Some(s) = screen(pup) {
                pup.show_screen(s);
            }
        }
        ("Hide", _) => {
            // seems to be called for music screens (VPX's note)
            if let Some(s) = screen(pup) {
                pup.screens[s].set_mode(Mode::MusicOnly);
            }
        }
        ("B2SInit", _) => pup.load_config(&text(1), now),
        ("SendMSG", _) => pup.send_msg(&text(0), now),
        ("LabelNew", _) => {
            if let Some(s) = screen(pup) {
                let fonts = pup.fonts.clone();
                let screen = &mut pup.screens[s];
                screen.label_init(&fonts);
                screen.add_label(Label::new(
                    &text(1),
                    &text(2),
                    &fonts,
                    int(3) as f32,
                    int(4),
                    int(5) as f32 / 10.0,
                    XAlign::from_number(int(6)),
                    YAlign::from_number(int(7)),
                    int(8) as f32,
                    int(9) as f32,
                    int(10),
                    a.bool(11).unwrap_or(false),
                ));
            }
        }
        ("LabelSet", _) => {
            if let Some(s) = screen(pup) {
                let name = text(1);
                let visible = a.bool(3).unwrap_or(false);
                let pack = pup.path().map(std::path::Path::to_path_buf);
                let screen = &mut pup.screens[s];
                // labels held back while the screen's media plays (SendMSG 34) take no commands
                if screen.hide_during_play {
                    return Ok(Ret::Empty);
                }
                if screen.has_label(&name) {
                    let fonts = pup.fonts.clone();
                    let screen = &mut pup.screens[s];
                    let mut playlists = std::mem::take(&mut screen.playlists);
                    let (page, pad) = (screen.page, screen.pad_text_always);
                    let mut reorder = None;
                    let mut dispose = false;
                    let caption = text(2);
                    if let Some(label) = screen.label_mut(&name) {
                        label.set_caption(&caption, &mut playlists, pack.as_deref(), pad, now);
                        // the visibility changes only while the label's page shows, unless the
                        // caption is `u` (as PinUP); an animation shows the label whatever it is
                        if (caption == "`u`" || label.page == page) && !label.is_animating() {
                            label.visible = visible;
                        }
                        reorder = label.set_special(&text(4), &fonts, now);
                        dispose = label.do_dispose;
                    }
                    screen.playlists = playlists;
                    if let Some(to) = reorder {
                        screen.reorder_label(&name, to);
                    }
                    // pDMDLabelDispose: removed once this update is done
                    if dispose {
                        screen.remove_label(&name);
                    }
                } else if sp.warned.insert((int(0), name.to_lowercase())) {
                    super::log::once(&format!("PinUP: screen {} has no label {name}", int(0)));
                }
            }
        }
        // a page of labels for `Seconds` (from 501 on, milliseconds), or the page to return to (0)
        ("LabelShowPage", _) => {
            if let Some(s) = screen(pup) {
                let seconds = int(2);
                pup.screens[s].set_page(int(1), if seconds >= 501 { seconds } else { seconds.saturating_mul(1000) }, &text(3), now);
            }
        }
        ("LabelInit", _) => {
            if let Some(s) = screen(pup) {
                let fonts = pup.fonts.clone();
                pup.screens[s].label_init(&fonts);
            }
        }
        // PinUP Player's sounds: a sound of the pack's PUPSOUNDS folder; SoundAdd and SoundPlay
        // ignored, as PinUP Player ignores them
        ("PuPSound", _) => pup.play_sound(&text(0), int(1), int(2), int(3), int(4), &text(5)),
        ("GetRoot", Access::Get) => {
            // the folder holding the pack in use, else the shared folder of packs, with a closing
            // separator either way, as PinUP Player gives it and scripts add to it (VPX 5436 leaves
            // it off the shared folder, which Darkest Dungeon reads before its pack loads)
            let root = pup.path().and_then(std::path::Path::parent).or(pup.root());
            return Ok(Ret::Str(root.map(|d| format!("{}\\", d.display().to_string().trim_end_matches('\\'))).unwrap_or_default()));
        }
        ("GetGame" | "B2SFilter" | "B2SDisplays", Access::Get) => return Ok(Ret::Str(String::new())),
        ("SN", Access::Get) | ("GameUpdate", _) => return Ok(Ret::Int(0)),
        // PinUP Player 2's, as VPX reports it now
        ("GetVersion", _) => return Ok(Ret::Str("2.0.99".to_string())),
        ("playevent", _) => {
            if let Some(s) = screen(pup)
                && let Some(action) = PlayAction::from_number(int(5))
            {
                match action {
                    PlayAction::StopPlayer => pup.screens[s].stop_priority(int(4)),
                    PlayAction::StopFile => {
                        if let Some(p) = pup.screens[s].playlist(&text(1)) {
                            pup.screens[s].stop_file(p, &text(2));
                        }
                    }
                    PlayAction::PlaySSF => {
                        let volume = int(3);
                        pup.play_sound(&text(2), 0, 0, 0, if volume < 0 { 100 } else { volume }, "");
                    }
                    PlayAction::CustomFunction => {}
                    _ => {
                        pup.play(s, &text(1), &text(2), int(3) as f32, int(4), action, now);
                        if action == PlayAction::Loop {
                            pup.screens[s].media.set_loop(true);
                        } else if int(6) > 0 {
                            pup.screens[s].set_length(int(6), now);
                        }
                    }
                }
            }
        }
        // left undone in VPX too: sizes and places by number, BlockPlay, SetScreen, the menu,
        // LabelSetEx, SetPosVideo, and setting what's only read
        _ => {}
    }
    Ok(Ret::Empty)
}
