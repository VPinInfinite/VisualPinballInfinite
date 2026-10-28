// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the script interface of
// its PinMAME plugin (plugins/pinmame/PinMAMEPlugin.cpp, Controller.cpp, Game.h, GameSettings.cpp, SettingsDefaults.h),
// VPX's rebuild of VPinMAME's controller; with its later fixes (2026-10: the DIP switches read from the machine,
// 2eddf7617; Lamp and Solenoid on or off as VPinMAME, 60130e8d7).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPinMAME's controller for table scripts: what
//! `CreateObject("VPinMAME.Controller")` gives a table, driving PinMAME in
//! `vpi-pinmame` (`crate::pinmame`). The members are those VPX's own
//! rebuild offers, and a few more VPinMAME has that tables set; the ones
//! about PinMAME's own window are accepted and kept, as there.

use super::com::*;
use super::idl_types::{Access, Interface, Member, Param, Ty};
use super::object::{Obj, Target};
use super::value::{Args, Ret};
use super::{log, with};
use crate::pinmame::{self, Controller};
use crate::world::World;

type Res = Result<Ret, HRESULT>;

const fn param(name: &'static str, ty: Ty, optional: bool) -> Param {
    Param { name, ty, optional, default: None, by_ref: false }
}

const NUMBER: &[Param] = &[param("Number", Ty::Int, false)];
const NAME: &[Param] = &[param("Name", Ty::Str, false)];
const NONE: &[Param] = &[];

const fn get(name: &'static str, dispid: i32, params: &'static [Param], ret: Ty) -> Member {
    Member { name, dispid, access: Access::Get, params, ret: Some(ret) }
}

const fn put(name: &'static str, dispid: i32, params: &'static [Param], ty: Ty) -> Member {
    let _ = ty;
    Member { name, dispid, access: Access::Put, params, ret: None }
}

const fn method(name: &'static str, dispid: i32, params: &'static [Param], ret: Option<Ty>) -> Member {
    Member { name, dispid, access: Access::Method, params, ret }
}

/// VPinMAME's `IController`.
pub static I_CONTROLLER: Interface = Interface {
    name: "IController",
    members: &[
        get("Version", 1, NONE, Ty::Str),
        get("GameName", 2, NONE, Ty::Str),
        put("GameName", 2, NONE, Ty::Str),
        get("ROMName", 3, NONE, Ty::Str),
        method("Run", 4, &[param("hParentWnd", Ty::Int, true), param("nMinVersion", Ty::Int, true)], None),
        method("Stop", 5, NONE, None),
        get("Running", 6, NONE, Ty::Bool),
        get("Pause", 7, NONE, Ty::Bool),
        put("Pause", 7, NONE, Ty::Bool),
        get("Switch", 8, NUMBER, Ty::Bool),
        put("Switch", 8, NUMBER, Ty::Bool),
        get("Dip", 9, NUMBER, Ty::Int),
        put("Dip", 9, NUMBER, Ty::Int),
        get("Lamp", 10, NUMBER, Ty::Bool),
        get("Solenoid", 11, NUMBER, Ty::Bool),
        get("GIString", 12, NUMBER, Ty::Int),
        get("ChangedLamps", 13, NONE, Ty::Variant),
        get("ChangedSolenoids", 14, NONE, Ty::Variant),
        get("ChangedGIStrings", 15, NONE, Ty::Variant),
        get("ChangedLEDs", 16, &[param("nHigh", Ty::Int, false), param("nLow", Ty::Int, false), param("nnHigh", Ty::Int, true), param("nnLow", Ty::Int, true)], Ty::Variant),
        get("SolMask", 17, NUMBER, Ty::Int),
        put("SolMask", 17, NUMBER, Ty::Int),
        get("ModOutputType", 18, &[param("Output", Ty::Int, false), param("Number", Ty::Int, false)], Ty::Int),
        put("ModOutputType", 18, &[param("Output", Ty::Int, false), param("Number", Ty::Int, false)], Ty::Int),
        get("HandleMechanics", 19, NONE, Ty::Int),
        put("HandleMechanics", 19, NONE, Ty::Int),
        get("GetMech", 20, NUMBER, Ty::Int),
        put("Mech", 21, NUMBER, Ty::Int),
        get("TimeFence", 22, NONE, Ty::Double),
        put("TimeFence", 22, NONE, Ty::Double),
        get("Games", 23, NAME, Ty::Object),
        get("Settings", 24, NONE, Ty::Object),
        get("RawDmdWidth", 25, NONE, Ty::Int),
        get("RawDmdHeight", 26, NONE, Ty::Int),
        get("RawDmdPixels", 27, NONE, Ty::Variant),
        get("RawDmdColoredPixels", 28, NONE, Ty::Variant),
        get("NVRAM", 29, NONE, Ty::Variant),
        get("ChangedNVRAM", 30, NONE, Ty::Variant),
        get("NewSoundCommands", 31, NONE, Ty::Variant),
        get("IsPlugin", 32, NONE, Ty::Bool),
        get("Hidden", 33, NONE, Ty::Bool),
        put("Hidden", 33, NONE, Ty::Bool),
        get("SplashInfoLine", 34, NONE, Ty::Str),
        put("SplashInfoLine", 34, NONE, Ty::Str),
        get("HandleKeyboard", 35, NONE, Ty::Bool),
        put("HandleKeyboard", 35, NONE, Ty::Bool),
        // about PinMAME's own window and options: kept, and nothing more, as in VPX
        get("DoubleSize", 40, NONE, Ty::Bool),
        put("DoubleSize", 40, NONE, Ty::Bool),
        get("LockDisplay", 41, NONE, Ty::Bool),
        put("LockDisplay", 41, NONE, Ty::Bool),
        get("ShowFrame", 42, NONE, Ty::Bool),
        put("ShowFrame", 42, NONE, Ty::Bool),
        get("ShowDMDOnly", 43, NONE, Ty::Bool),
        put("ShowDMDOnly", 43, NONE, Ty::Bool),
        get("ShowTitle", 44, NONE, Ty::Bool),
        put("ShowTitle", 44, NONE, Ty::Bool),
        get("FastFrames", 45, NONE, Ty::Int),
        put("FastFrames", 45, NONE, Ty::Int),
        get("IgnoreRomCrc", 46, NONE, Ty::Bool),
        put("IgnoreRomCrc", 46, NONE, Ty::Bool),
        get("CabinetMode", 47, NONE, Ty::Bool),
        put("CabinetMode", 47, NONE, Ty::Bool),
        get("SoundMode", 48, NONE, Ty::Int),
        put("SoundMode", 48, NONE, Ty::Int),
        method("ShowOptsDialog", 49, &[param("hParentWnd", Ty::Int, true)], None),
        // VPinMAME's, which some tables set though VPX's rebuild leaves them out
        get("ShowWinDMD", 50, NONE, Ty::Bool),
        put("ShowWinDMD", 50, NONE, Ty::Bool),
        get("ShowPinDMD", 51, NONE, Ty::Bool),
        put("ShowPinDMD", 51, NONE, Ty::Bool),
        get("MechSamples", 52, NONE, Ty::Bool),
        put("MechSamples", 52, NONE, Ty::Bool),
        get("WPCNumbering", 53, NONE, Ty::Bool),
        get("Solenoids", 54, NONE, Ty::Variant),
        get("Lamps", 55, NONE, Ty::Variant),
        get("GIStrings", 56, NONE, Ty::Variant),
        get("Switches", 57, NONE, Ty::Variant),
        get("ChangedPDLeds", 58, NONE, Ty::Variant),
        get("Game", 59, NONE, Ty::Object),
        method("ShowAboutDialog", 60, &[param("hParentWnd", Ty::Int, true)], None),
        method("ShowPathesDialog", 61, &[param("hParentWnd", Ty::Int, true)], None),
        method("CheckROMS", 62, &[param("nShowOptions", Ty::Int, true), param("hParentWnd", Ty::Int, true)], Some(Ty::Bool)),
        method("SetDisplayPosition", 63, &[param("x", Ty::Int, false), param("y", Ty::Int, false), param("hParentWnd", Ty::Int, true)], None),
    ],
};

/// VPinMAME's `IGame`: one game PinMAME knows.
pub static I_GAME: Interface = Interface {
    name: "IGame",
    members: &[
        get("Name", 1, NONE, Ty::Str),
        get("Description", 2, NONE, Ty::Str),
        get("Year", 3, NONE, Ty::Str),
        get("Manufacturer", 4, NONE, Ty::Str),
        get("CloneOf", 5, NONE, Ty::Str),
        get("Settings", 6, NONE, Ty::Object),
        get("IsSupported", 7, NONE, Ty::Bool),
        method("ShowInfoDlg", 8, &[param("nShowOptions", Ty::Int, true), param("hParentWnd", Ty::Int, true)], Some(Ty::Int)),
    ],
};

/// VPinMAME's `IGameSettings` and `ISettings`: whole-number settings by
/// name, read back as set or as VPinMAME's defaults.
pub static I_GAME_SETTINGS: Interface = Interface {
    name: "IGameSettings",
    members: &[
        get("Value", 1, NAME, Ty::Int),
        put("Value", 1, NAME, Ty::Int),
        method("Clear", 2, NONE, None),
        method("ShowSettingsDlg", 3, &[param("hParentWnd", Ty::Int, true)], None),
        method("SetDisplayPosition", 4, &[param("x", Ty::Int, false), param("y", Ty::Int, false), param("hParentWnd", Ty::Int, true)], None),
    ],
};

pub static I_SETTINGS: Interface = Interface {
    name: "ISettings",
    members: &[get("Value", 1, NAME, Ty::Int), put("Value", 1, NAME, Ty::Int), method("ShowSettingsDlg", 3, &[param("hParentWnd", Ty::Int, true)], None)],
};

/// Where B2S.Server's own members start; below are the controller's, which
/// it hands on.
const B2S_FIRST: i32 = 200;

const ID_VALUE: &[Param] = &[param("ID", Ty::Variant, false), param("Value", Ty::Int, false)];
const DIGIT_VALUE: &[Param] = &[param("Digit", Ty::Int, false), param("Value", Ty::Int, false)];
const VALUE: &[Param] = &[param("Value", Ty::Int, false)];
/// `(value)` for the usual light or digit, or `(id, value)` for another.
const VALUE_OR_ID_VALUE: &[Param] = &[param("ID", Ty::Int, false), param("Value", Ty::Int, true)];
const ANIMATION: &[Param] = &[param("Name", Ty::Str, false), param("Reverse", Ty::Bool, true)];

/// B2S.Server's own members, as VPX 5436's B2S plugin offers them
/// (`plugins/b2s/B2SPlugin.cpp`, `B2S_Server`).
static B2S_MEMBERS: &[Member] = &[
    method("Dispose", 200, NONE, None),
    get("TableName", 201, NONE, Ty::Str),
    put("TableName", 201, NONE, Ty::Str),
    put("WorkingDir", 202, NONE, Ty::Str),
    method("SetPath", 203, &[param("Path", Ty::Str, false)], None),
    get("VPMBuildVersion", 204, NONE, Ty::Str),
    get("LaunchBackglass", 205, NONE, Ty::Bool),
    put("LaunchBackglass", 205, NONE, Ty::Bool),
    get("PuPHide", 206, NONE, Ty::Bool),
    put("PuPHide", 206, NONE, Ty::Bool),
    get("B2SName", 207, NONE, Ty::Str),
    put("B2SName", 207, NONE, Ty::Str),
    get("B2SServerVersion", 208, NONE, Ty::Str),
    get("B2SBuildVersion", 209, NONE, Ty::Double),
    get("B2SServerDirectory", 210, NONE, Ty::Str),
    method("B2SSetData", 211, ID_VALUE, None),
    method("B2SPulseData", 212, &[param("ID", Ty::Variant, false)], None),
    method("B2SSetPos", 213, &[param("ID", Ty::Variant, false), param("X", Ty::Int, false), param("Y", Ty::Int, false)], None),
    method("B2SSetIllumination", 214, &[param("Name", Ty::Str, false), param("Value", Ty::Int, false)], None),
    method("B2SSetLED", 215, &[param("Digit", Ty::Int, false), param("Value", Ty::Variant, false)], None),
    method("B2SSetLEDDisplay", 216, &[param("Display", Ty::Int, false), param("Text", Ty::Str, false)], None),
    method("B2SSetReel", 217, DIGIT_VALUE, None),
    method("B2SSetScore", 218, DIGIT_VALUE, None),
    method("B2SSetScorePlayer", 219, &[param("Player", Ty::Int, false), param("Score", Ty::Int, false)], None),
    method("B2SSetScorePlayer1", 220, VALUE, None),
    method("B2SSetScorePlayer2", 221, VALUE, None),
    method("B2SSetScorePlayer3", 222, VALUE, None),
    method("B2SSetScorePlayer4", 223, VALUE, None),
    method("B2SSetScorePlayer5", 224, VALUE, None),
    method("B2SSetScorePlayer6", 225, VALUE, None),
    method("B2SSetScoreDigit", 226, DIGIT_VALUE, None),
    method("B2SSetScoreRollover", 227, DIGIT_VALUE, None),
    method("B2SSetScoreRolloverPlayer1", 228, VALUE, None),
    method("B2SSetScoreRolloverPlayer2", 229, VALUE, None),
    method("B2SSetScoreRolloverPlayer3", 230, VALUE, None),
    method("B2SSetScoreRolloverPlayer4", 231, VALUE, None),
    method("B2SSetCredits", 232, VALUE_OR_ID_VALUE, None),
    method("B2SSetPlayerUp", 233, VALUE_OR_ID_VALUE, None),
    method("B2SSetCanPlay", 234, VALUE_OR_ID_VALUE, None),
    method("B2SSetBallInPlay", 235, VALUE_OR_ID_VALUE, None),
    method("B2SSetTilt", 236, VALUE_OR_ID_VALUE, None),
    method("B2SSetMatch", 237, VALUE_OR_ID_VALUE, None),
    method("B2SSetGameOver", 238, VALUE_OR_ID_VALUE, None),
    method("B2SSetShootAgain", 239, VALUE_OR_ID_VALUE, None),
    method("B2SStartAnimation", 240, ANIMATION, None),
    method("B2SStartAnimationReverse", 241, NAME, None),
    method("B2SStopAnimation", 242, NAME, None),
    method("B2SStopAllAnimations", 243, NONE, None),
    get("B2SIsAnimationRunning", 244, NAME, Ty::Bool),
    method("StartAnimation", 245, ANIMATION, None),
    method("StopAnimation", 246, NAME, None),
    method("B2SStartRotation", 247, NONE, None),
    method("B2SStopRotation", 248, NONE, None),
    method("B2SShowScoreDisplays", 249, NONE, None),
    method("B2SHideScoreDisplays", 250, NONE, None),
    method("B2SStartSound", 251, NAME, None),
    method("B2SPlaySound", 252, NAME, None),
    method("B2SStopSound", 253, NAME, None),
    method("B2SMapSound", 254, &[param("Digit", Ty::Int, false), param("Name", Ty::Str, false)], None),
];

/// B2S.Server: the backglass's own members, and the controller's, which it
/// hands on to PinMAME as VPX 5436's B2S plugin does.
pub static I_B2S_SERVER: std::sync::LazyLock<Interface> =
    std::sync::LazyLock::new(|| Interface { name: "Server", members: Box::leak([I_CONTROLLER.members, B2S_MEMBERS].concat().into_boxed_slice()) });

/// The controller's script objects, and the machine behind them. One per
/// table: a second `CreateObject("VPinMAME.Controller")` gets the same,
/// as there is one PinMAME; and `B2S.Server`, when the table asks for it,
/// drives the same machine.
pub struct ScriptController {
    pub obj: *mut Obj,
    pub settings_obj: *mut Obj,
    /// The games scripts asked for (`Games(name)`), with their objects and
    /// their settings' objects.
    pub games: Vec<(String, *mut Obj, *mut Obj)>,
    pub state: Controller,
    /// `B2S.Server`, once asked for, and what the script told it.
    pub b2s_obj: *mut Obj,
    pub b2s: Option<crate::b2s::ServerState>,
}

impl ScriptController {
    fn new() -> ScriptController {
        ScriptController {
            obj: Obj::new(Target::Controller, &I_CONTROLLER),
            settings_obj: Obj::new(Target::ControllerSettings, &I_SETTINGS),
            games: Vec::new(),
            state: Controller::default(),
            b2s_obj: std::ptr::null_mut(),
            b2s: None,
        }
    }

    /// The objects, for freeing when play ends.
    pub fn objects(&self) -> Vec<*mut Obj> {
        let mut out = vec![self.obj, self.settings_obj];
        for &(_, game, settings) in &self.games {
            out.extend([game, settings]);
        }
        if !self.b2s_obj.is_null() {
            out.push(self.b2s_obj);
        }
        out
    }

    /// The game object for `name`, made the first time it is asked for.
    fn game(&mut self, name: &str) -> *mut Obj {
        if let Some((_, g, _)) = self.games.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(name)) {
            return *g;
        }
        let i = self.games.len() as u32;
        let (game, settings) = (Obj::new(Target::Game(i), &I_GAME), Obj::new(Target::GameSettings(i), &I_GAME_SETTINGS));
        self.games.push((name.to_string(), game, settings));
        game
    }
}

/// The table's controller, made the first time a script asks for it, with the player's PinMAME
/// settings: VPinMAME's for each game from our file, and VPX 5436's `[Plugin.PinMAME]`.
fn ensure_controller(w: &mut World) {
    if w.controller.is_none() {
        let mut c = ScriptController::new();
        c.state.machine_file = w.machine_settings.clone();
        c.state.plugin = w.pinmame_settings.clone();
        c.state.table_dir = w.table_dir.clone();
        w.controller = Some(Box::new(c));
    }
}

/// `CreateObject("VPinMAME.Controller")`: the table's controller.
pub fn create() -> Res {
    with(|_, w| {
        ensure_controller(w);
        let c = w.controller.as_mut().expect("made just now");
        Ret::Object(Obj::dispatch(c.obj))
    })
    .ok_or(E_FAIL)
}

/// `CreateObject("B2S.Server")`: the backglass server, which is the table's
/// controller too. Its backglass file starts loading when the player has a
/// window to show it in.
pub fn create_b2s() -> Res {
    with(|_, w| {
        if w.backglass_window && w.backglass.is_none() {
            w.backglass = w.table_path.as_deref().and_then(crate::b2s::Loading::start);
        }
        ensure_controller(w);
        let c = w.controller.as_mut().expect("made just now");
        if c.b2s_obj.is_null() {
            c.b2s_obj = Obj::new(Target::B2sServer, &I_B2S_SERVER);
            c.b2s = Some(crate::b2s::ServerState::default());
        }
        Ret::Object(Obj::dispatch(c.b2s_obj))
    })
    .ok_or(E_FAIL)
}

/// One member of B2S.Server (VPX 5436's `B2SServer`): its own, or the
/// controller's, handed on. The animations play as B2S Server plays them
/// (VPX 5436 leaves them undone); members VPX leaves undone otherwise
/// (sounds, LEDs, positions, lights by group name, turning lights) are
/// accepted and do nothing, as there. Its data, scores and players' scores
/// reach a PinUP pack's triggers as `E`, `B` and `C` events, as VPX's server
/// broadcasts them.
pub fn b2s_server(m: &'static Member, a: &Args) -> Option<Res> {
    if m.dispid < B2S_FIRST {
        // an electromechanical table runs B2S.Server with no game named: nothing to start
        if m.name == "Run" && with(|_, w| w.controller.as_ref().is_none_or(|c| c.state.game_name.is_empty())).unwrap_or(true) {
            // but its toys run, named as it named itself (`B2SName`, its `cGameName`), as VPX
            // 10.8.0's B2S starts them
            with(|_, w| {
                let name = w.controller.as_ref().and_then(|c| c.b2s.as_ref()).map(|s| s.name.clone()).unwrap_or_default();
                crate::dof::on_game_start(w, &name);
            });
            return Some(Ok(Ret::Empty));
        }
        return controller(m, a);
    }
    let put = m.access == Access::Put;
    let r = with(|_, w| {
        let sc = w.controller.as_mut()?;
        let s = sc.b2s.get_or_insert_with(Default::default);
        let int = |i: usize| a.i32(i).unwrap_or(0);
        // (value) for the usual light, or (id, value)
        let usual = |id: i32| if a.get(1).is_some() { (int(0), int(1)) } else { (id, int(0)) };
        // what a PinUP pack hears of it (VPX's B2S OnStateChange events)
        let mut events: Vec<(char, i32, i32)> = Vec::new();
        let ret = match m.name {
            "B2SName" if put => {
                s.name = a.value_string().unwrap_or_default();
                Ret::Empty
            }
            "B2SName" => Ret::Str(s.name.clone()),
            "TableName" | "VPMBuildVersion" | "B2SServerVersion" | "B2SServerDirectory" if !put => Ret::Str(String::new()),
            "B2SBuildVersion" => Ret::Double(0.0),
            "LaunchBackglass" | "PuPHide" if !put => Ret::Bool(false),
            "B2SIsAnimationRunning" => Ret::Bool(s.animation_running(&a.string(0).unwrap_or_default())),
            "B2SStartAnimation" | "StartAnimation" => {
                s.start_animation(&a.string(0).unwrap_or_default(), a.bool(1).unwrap_or(false));
                Ret::Empty
            }
            "B2SStartAnimationReverse" => {
                s.start_animation(&a.string(0).unwrap_or_default(), true);
                Ret::Empty
            }
            "B2SStopAnimation" | "StopAnimation" => {
                s.stop_animation(&a.string(0).unwrap_or_default());
                Ret::Empty
            }
            "B2SStopAllAnimations" => {
                s.stop_all_animations();
                Ret::Empty
            }
            "B2SSetData" => {
                // by name, a group of lights: left undone in VPX 5436 too
                if a.get(0).is_some_and(|v| v.vt != VT_BSTR) {
                    s.set_data(int(0), int(1));
                    events.push(('E', int(0), int(1)));
                }
                Ret::Empty
            }
            "B2SPulseData" => {
                if a.get(0).is_some_and(|v| v.vt != VT_BSTR) {
                    s.set_data(int(0), 1);
                    s.set_data(int(0), 0);
                    events.extend([('E', int(0), 1), ('E', int(0), 0)]);
                }
                Ret::Empty
            }
            "B2SSetScore" | "B2SSetScoreDigit" | "B2SSetReel" => {
                s.set_score(int(0), int(1));
                events.push(('B', int(0), int(1)));
                Ret::Empty
            }
            "B2SSetScorePlayer" => {
                s.set_player_score(int(0), int(1));
                events.push(('C', int(0), int(1)));
                Ret::Empty
            }
            p if p.starts_with("B2SSetScorePlayer") => {
                let player = p["B2SSetScorePlayer".len()..].parse().unwrap_or(0);
                s.set_player_score(player, int(0));
                events.push(('C', player, int(0)));
                Ret::Empty
            }
            "B2SSetScoreRollover" => {
                s.set_data(int(0), int(1));
                events.push(('E', int(0), int(1)));
                Ret::Empty
            }
            p if p.starts_with("B2SSetScoreRolloverPlayer") => {
                let id = 24 + p["B2SSetScoreRolloverPlayer".len()..].parse().unwrap_or(0);
                s.set_data(id, int(0));
                events.push(('E', id, int(0)));
                Ret::Empty
            }
            "B2SSetCredits" => {
                let (id, value) = usual(29);
                s.set_score(id, value);
                events.push(('B', id, value));
                Ret::Empty
            }
            name => {
                let id = match name {
                    "B2SSetPlayerUp" => 30,
                    "B2SSetCanPlay" => 31,
                    "B2SSetBallInPlay" => 32,
                    "B2SSetTilt" => 33,
                    "B2SSetMatch" => 34,
                    "B2SSetGameOver" => 35,
                    "B2SSetShootAgain" => 36,
                    _ => return Some(Ret::Empty),
                };
                let (id, value) = usual(id);
                s.set_data(id, value);
                events.push(('E', id, value));
                Ret::Empty
            }
        };
        // to the PinUP pack and the toys, as VPX's B2S broadcasts them
        for (kind, id, value) in events {
            crate::events::queue(w, kind, id, value);
        }
        Some(ret)
    })
    .flatten();
    Some(r.ok_or(E_FAIL))
}

/// VPinMAME's default for a setting nobody set (`SettingsDefaults.h`), or 0.
pub(crate) fn setting_default(key: &str) -> i32 {
    const DEFAULTS: &[(&str, i32)] = &[
        ("dmd_red", 255),
        ("dmd_green", 88),
        ("dmd_blue", 32),
        ("dmd_perc0", 20),
        ("dmd_perc33", 33),
        ("dmd_perc66", 67),
        ("dmd_only", 0),
        ("dmd_compact", 0),
        ("dmd_antialias", 50),
        ("dmd_colorize", 0),
        ("dmd_red66", 225),
        ("dmd_green66", 15),
        ("dmd_blue66", 193),
        ("dmd_red33", 6),
        ("dmd_green33", 0),
        ("dmd_blue33", 214),
        ("dmd_red0", 0),
        ("dmd_green0", 0),
        ("dmd_blue0", 0),
        ("dmd_opacity", 100),
        ("dmd_border", 1),
        ("dmd_title", 1),
        ("dmd_pos_x", 0),
        ("dmd_pos_y", 0),
        ("dmd_width", 0),
        ("dmd_height", 0),
        ("dmd_doublesize", 0),
        ("showpindmd", 0),
        ("showwindmd", 1),
        ("sound", 1),
        ("samples", 1),
        ("resampling_quality", 0),
        ("sound_mode", 0),
        ("force_stereo", 0),
        ("rol", 0),
        ("ignore_rom_crc", 0),
        ("cabinet_mode", 0),
        ("threadpriority", 1),
        ("synclevel", 0),
        ("fastframes", -1),
    ];
    DEFAULTS.iter().find(|(k, _)| *k == key).map_or(0, |&(_, v)| v)
}

/// A list of changes as VPinMAME hands it: rows of (number, value), or
/// nothing when there are none.
fn changes(list: Vec<(i32, i32)>) -> Ret {
    if list.is_empty() {
        return Ret::Empty;
    }
    Ret::Rows(list.into_iter().map(|(n, v)| vec![Ret::Int(n), Ret::Int(v)]).collect())
}

/// Whether the script hid PinMAME's own window (`Controller.Hidden`, DISPID 33), as VPinMAME
/// hides it: the floating score window follows it.
pub fn hidden(w: &World) -> bool {
    w.kept.get(&(super::api::owner_key(Target::Controller), 33)).and_then(super::value::Kept::number).is_some_and(|n| n != 0.0)
}

/// Kept values of the members that only store what they're given.
fn kept(w: &mut World, m: &Member, a: &Args) -> Ret {
    let key = (super::api::owner_key(Target::Controller), m.dispid);
    if m.access == Access::Put {
        if let Some(v) = a.value_kept() {
            w.kept.insert(key, v);
        }
        return Ret::Empty;
    }
    w.kept.get(&key).map_or(Ret::Empty, |k| Ret::Kept(k.clone()))
}

/// One member of the controller.
pub fn controller(m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    let name = m.name;
    let r = with(|e, w| {
        let trace = pinmame::TRACE.load(std::sync::atomic::Ordering::Relaxed);
        let now_ms = e.time_msec();
        ensure_controller(w);
        let sc = w.controller.as_mut().expect("made just now");
        let c = &mut sc.state;
        if trace && matches!(name, "Run" | "Stop" | "Pause" | "SolMask" | "HandleMechanics" | "Mech") && (put || m.access == Access::Method) {
            let args: Vec<String> = (0..a.len()).filter_map(|i| a.string(i)).collect();
            eprintln!("[pinmame] {now_ms:>7} ms  {name}({}){}", args.join(", "), a.value_string().filter(|_| put).map_or(String::new(), |v| format!(" = {v}")));
        }
        Some(Ok(match name {
            "Version" => Ret::Str(pinmame::VERSION.into()),
            "GameName" if put => {
                c.set_game_name(&a.value_string().unwrap_or_default());
                Ret::Empty
            }
            "GameName" => Ret::Str(c.game_name.clone()),
            "ROMName" => Ret::Str(c.rom_name.clone()),
            "Run" => {
                // the game's AltSound pack, if it has one, in place of its own sound
                // VPinMAME's `sound_mode` for the game, when the file or the script gives it: 0 its own sound
                c.altsound = match c.setting("sound_mode") {
                    Some(0) => None,
                    _ => crate::pinmame::altsound_base(w.table_dir.as_deref(), &c.rom_name, &w.altsound_settings),
                };
                // and its colour file, if it has one, colouring its display
                c.colour_files = crate::colour::find(w.table_dir.as_deref(), &c.rom_name, &w.serum_settings, &w.vni_settings);
                return Some(match c.run(true, w.waker.clone()) {
                    Ok(()) => {
                        // its sound boards play on the backglass speakers, as a stream
                        if let Some(m) = &c.machine {
                            w.sounds.play_machine(Box::new(m.sound()), c.altsound.is_some());
                            // its high scores watched, for a screenshot when a game makes them
                            if crate::screenshot::at_high_score() {
                                crate::machine_scores::watch(m.link_while_running(), c.rom_name.clone());
                            }
                            // its game's state, for achievements or where a check asks for it
                            crate::game_state::watch_if_asked(m.link_while_running(), c.rom_name.clone());
                        }
                        // the game's PinUP pack, if it has one
                        let game = c.game_name.clone();
                        super::pup::on_game_start(w, &game, u64::from(now_ms));
                        // and the cabinet's toys
                        crate::dof::on_game_start(w, &game);
                        Ok(Ret::Empty)
                    }
                    Err(e) => {
                        log::once(&format!("Controller.Run: {e}"));
                        Err(E_FAIL)
                    }
                });
            }
            "Stop" => {
                c.stop();
                w.sounds.stop_stream();
                // the toys end with the game, as VPX 5436's DOF plugin ends them
                w.dof = None;
                Ret::Empty
            }
            "Running" => Ret::Bool(c.machine.as_mut().is_some_and(|m| m.running())),
            "Pause" if put => {
                c.set_pause(a.value_bool().unwrap_or(false));
                Ret::Empty
            }
            "Pause" => Ret::Bool(c.paused),
            "Switch" if put => {
                let (number, closed) = (a.i32(0).unwrap_or(0), a.value_bool().unwrap_or(false));
                if trace && c.switch(number) != closed {
                    eprintln!("[pinmame] {now_ms:>7} ms  switch {number} {}", if closed { "closed" } else { "open" });
                }
                pinmame::latency_log('S', number, i32::from(closed));
                c.set_switch(number, closed);
                Ret::Empty
            }
            "Switch" => Ret::Bool(c.switch(a.i32(0).unwrap_or(0))),
            "Dip" if put => {
                c.set_dip(a.i32(0).unwrap_or(0), a.value_i32().unwrap_or(0));
                Ret::Empty
            }
            // the machine's switches once it runs, else what the script set (VPX's GetDip falls
            // back to its cached states when PinMAME has none)
            "Dip" => {
                let bank = a.i32(0).unwrap_or(0);
                let live = c.machine().and_then(|m| if m.running() { Some(m.dip_bank(bank)) } else { None });
                Ret::Int(if bank < 0 { 0 } else { live.unwrap_or_else(|| c.dips.get(&bank).copied().unwrap_or(0)) })
            }
            // on or off, as VPinMAME answers (VPX's plugin since 60130e8d7): a lamp above 0, or above
            // half in the physical outputs mode; a solenoid at any level above 0 (Capcom's flippers
            // report their strength, under half while on)
            "Lamp" => {
                let half = if c.device_mode == crate::pinmame::DEVICE_PHYSICAL { 127 } else { 0 };
                Ret::Bool(c.machine().is_some_and(|m| m.lamps.current(a.i32(0).unwrap_or(0)) > half))
            }
            "Solenoid" => Ret::Bool(c.machine().is_some_and(|m| m.solenoids.current(a.i32(0).unwrap_or(0)) > 0)),
            "GIString" => Ret::Int(c.machine().map_or(0, |m| m.gis.current(a.i32(0).unwrap_or(0)))),
            "ChangedLamps" => changes(c.machine().map_or_else(Vec::new, |m| m.lamps.take(|_| true))),
            "ChangedGIStrings" => changes(c.machine().map_or_else(Vec::new, |m| m.gis.take(|_| true))),
            "ChangedSolenoids" => {
                let wanted = Controller::solenoid_wanted(c.sol_mask);
                let list = c.machine().map_or_else(Vec::new, |m| m.solenoids.take(&wanted));
                for &(n, v) in &list {
                    pinmame::latency_log('C', n, v);
                    if trace {
                        eprintln!("[pinmame] {now_ms:>7} ms  solenoid {n} {v}");
                    }
                }
                changes(list)
            }
            "SolMask" if put => {
                c.set_sol_mask(a.i32(0).unwrap_or(0), a.value_i32().unwrap_or(0));
                Ret::Empty
            }
            "SolMask" => Ret::Int(c.sol_mask(a.i32(0).unwrap_or(0))),
            "ModOutputType" if put => {
                c.set_mod_output_type(a.i32(0).unwrap_or(0), a.i32(1).unwrap_or(0), a.value_i32().unwrap_or(0));
                Ret::Empty
            }
            "HandleMechanics" if put => {
                c.handle_mechanics = a.value_i32().unwrap_or(0);
                Ret::Empty
            }
            "HandleMechanics" => Ret::Int(c.handle_mechanics),
            "RawDmdWidth" => Ret::Int(c.machine().and_then(|m| m.dmd()).map_or(0, |(width, _, _)| width)),
            "RawDmdHeight" => Ret::Int(c.machine().and_then(|m| m.dmd()).map_or(0, |(_, height, _)| height)),
            // VPinMAME's brightness: 0 to 100 a dot
            "RawDmdPixels" => match c.machine().and_then(|m| m.dmd()) {
                Some((_, _, dots)) => Ret::Array(dots.iter().map(|&d| Ret::Int(i32::from(d) * 100 / 255)).collect()),
                None => Ret::Empty,
            },
            // a grey per dot, as VPX's rebuild gives for an uncoloured DMD
            "RawDmdColoredPixels" => match c.machine().and_then(|m| m.dmd()) {
                Some((_, _, dots)) => Ret::Array(dots.iter().map(|&d| Ret::Int(i32::from(d) * 0x01_01_01)).collect()),
                None => Ret::Empty,
            },
            "Games" => Ret::Object(Obj::dispatch(sc.game(&a.string(0).unwrap_or_default()))),
            "Game" => {
                let game = sc.state.game_name.clone();
                Ret::Object(Obj::dispatch(sc.game(&game)))
            }
            "Settings" => Ret::Object(Obj::dispatch(sc.settings_obj)),
            // as VPX 5436's PinMAME plugin answers (and its backglass plugins pass on):
            // `core.vbs` then leaves the display to the player rather than copying it
            // into the script every frame, which also delayed every solenoid change
            "IsPlugin" => Ret::Bool(true),
            "WPCNumbering" => Ret::Bool(c.machine.is_some()),
            "CheckROMS" => Ret::Bool(true),
            // VPX's rebuild has no PinDMD LEDs either: nothing, unremarked
            "ChangedPDLeds" => Ret::Empty,
            "ChangedLEDs" => {
                let int = |i: usize| a.i32(i).unwrap_or(0);
                let (mask, mask2) = (pinmame::led_mask(int(0), int(1)), pinmame::led_mask(int(2), int(3)));
                let list = c.machine().map_or_else(Vec::new, |m| m.leds.take(mask, mask2));
                if list.is_empty() {
                    Ret::Empty
                } else {
                    Ret::Rows(list.into_iter().map(|(digit, changed, lit)| vec![Ret::Int(digit), Ret::Int(changed), Ret::Int(lit)]).collect())
                }
            }
            // the commands the game sent its sound board since the last call, a row each: VPX 5436's
            // plugin hands each command twice (its second field unused); none is Empty
            "NewSoundCommands" => {
                let list = c.machine().map_or_else(Vec::new, |m| std::mem::take(&mut m.sound_commands));
                if trace && !list.is_empty() {
                    eprintln!("[pinmame] {now_ms:>7} ms  sound commands {list:02x?}");
                }
                changes(list.into_iter().map(|n| (n, n)).collect())
            }
            // the game's NVRAM, a byte each (VPX 5436's plugin: `GetNVRAM`); Empty when it has none
            "NVRAM" => match c.machine().and_then(|m| m.nvram()) {
                Some(bytes) => Ret::Array(bytes.into_iter().map(|b| Ret::Int(i32::from(b))).collect()),
                None => Ret::Empty,
            },
            "ChangedNVRAM" | "Solenoids" | "Lamps" | "GIStrings" | "Switches" => {
                log::unsupported("Controller", name, None);
                if m.ret == Some(Ty::Int) { Ret::Int(0) } else { Ret::Empty }
            }
            "TimeFence" if put => {
                c.set_time_fence(a.value_f64().unwrap_or(0.0));
                Ret::Empty
            }
            "TimeFence" => Ret::Double(c.time_fence),
            "Mech" => {
                c.set_mech(a.i32(0).unwrap_or(0), a.value_i32().unwrap_or(0));
                Ret::Empty
            }
            "GetMech" => Ret::Int(c.mech(a.i32(0).unwrap_or(0))),
            "ShowOptsDialog" | "ShowAboutDialog" | "ShowPathesDialog" | "SetDisplayPosition" => Ret::Empty,
            _ => kept(w, m, a),
        }))
    })
    .flatten();
    Some(r.unwrap_or(Err(E_FAIL)))
}

/// One member of a game (`Games(name)`).
pub fn game(index: u32, m: &'static Member, _a: &Args) -> Option<Res> {
    let r = with(|_, w| {
        let sc = w.controller.as_mut()?;
        let (name, _, settings) = sc.games.get(index as usize)?;
        Some(match m.name {
            "Name" => Ret::Str(name.clone()),
            "Settings" => Ret::Object(Obj::dispatch(*settings)),
            "IsSupported" => Ret::Bool(true),
            "ShowInfoDlg" => Ret::Int(0),
            // PinMAME's game list isn't asked yet: the rest are blank
            _ => Ret::Str(String::new()),
        })
    })
    .flatten();
    Some(r.ok_or(E_FAIL))
}

/// One member of a game's settings, or the controller's (`index` None).
pub fn settings(index: Option<u32>, m: &'static Member, a: &Args) -> Option<Res> {
    let r = with(|_, w| {
        let sc = w.controller.as_mut()?;
        let file = sc.state.machine_file.clone();
        let (game, values) = match index {
            Some(i) => {
                let game = sc.games.get(i as usize)?.0.to_lowercase();
                (game.clone(), sc.state.game_settings.entry(game).or_default())
            }
            None => ("default".to_string(), &mut sc.state.settings),
        };
        Some(match m.name {
            "Value" if m.access == Access::Put => {
                values.insert(a.string(0).unwrap_or_default().to_lowercase(), a.value_i32().unwrap_or(0));
                Ret::Empty
            }
            "Value" => {
                let key = a.string(0).unwrap_or_default().to_lowercase();
                // the script's, else our file's for the game (VPinMAME's registry), else VPinMAME's default
                Ret::Int(values.get(&key).copied().or_else(|| file.get(&game, &key)).unwrap_or_else(|| setting_default(&key)))
            }
            "Clear" => {
                values.clear();
                Ret::Empty
            }
            _ => Ret::Empty,
        })
    })
    .flatten();
    Some(r.ok_or(E_FAIL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_member_has_its_put_beside_its_get() {
        for iface in [&I_CONTROLLER, &I_GAME, &I_GAME_SETTINGS, &I_SETTINGS, &*I_B2S_SERVER] {
            for m in iface.members {
                // a property's reader and writer share an id, and nothing else does
                let same: Vec<_> = iface.members.iter().filter(|o| o.dispid == m.dispid).collect();
                assert!(same.iter().all(|o| o.name == m.name), "{} {} shares id {}", iface.name, m.name, m.dispid);
            }
        }
        assert_eq!(setting_default("dmd_green"), 88);
        assert_eq!(setting_default("volume"), 0);
    }
}
