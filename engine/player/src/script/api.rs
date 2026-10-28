// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the script methods of
// ScriptGlobalTable (src/core/ScriptGlobalTable.cpp), PinTable, Ball and the parts (src/parts/*.cpp). Collections
// are our own: VPX's collection.cpp carries no GPL marker.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! What our script objects do, member by member, following VPX's script
//! interface. Members not supported yet still work as storage: a value the
//! script assigns reads back unchanged, and the member is named once in the
//! log, so each table's gaps show.

use std::path::PathBuf;

use physics_compat::engine::Shape;
use physics_compat::env::BallId;
use physics_compat::math::Vertex3Ds;
use physics_compat::hitball::HitBall;
use physics_compat::physconst::{DEFAULT_BALL_SIZE, GRAVITYCONST};
use vpin::vpx::gameitem::GameItemEnum;

use super::com::*;
use super::idl_types::{Access, Interface, Member};
use super::object::{Enumerator, Obj, Target};
use super::value::{Args, Kept, Ret};
use super::{log, with};
use crate::camera::View;
use crate::world::World;

type Res = Result<Ret, HRESULT>;

/// The key a stored value is kept under.
pub(super) fn owner_key(t: Target) -> u64 {
    match t {
        Target::Part(p) => u64::from(p),
        Target::Global => 1 << 40,
        Target::Table => 2 << 40,
        Target::Debug => 3 << 40,
        Target::Collection(c) => (4 << 40) + u64::from(c),
        Target::Ball(id) => (5 << 40) + u64::from(id.0),
        Target::Fence => 6 << 40,
        Target::Controller => 7 << 40,
        Target::ControllerSettings => 8 << 40,
        Target::Game(g) => (9 << 40) + u64::from(g),
        Target::GameSettings(g) => (10 << 40) + u64::from(g),
        Target::B2sServer => 11 << 40,
        Target::PupDisplay => 12 << 40,
    }
}

/// Calls one member of one object.
pub fn invoke(target: Target, iface: &'static Interface, m: &'static Member, a: &Args) -> Res {
    let handled = match target {
        Target::Global => global(m, a),
        Target::Table => table(m, a),
        Target::Debug => debug(m, a),
        Target::Part(p) => part(p, m, a),
        Target::Collection(c) => collection(c, m, a),
        Target::Ball(id) => ball(id, m, a),
        Target::Fence => Some(super::fence::create(a)),
        Target::Controller => super::controller::controller(m, a),
        Target::ControllerSettings => super::controller::settings(None, m, a),
        Target::Game(g) => super::controller::game(g, m, a),
        Target::GameSettings(g) => super::controller::settings(Some(g), m, a),
        Target::B2sServer => super::controller::b2s_server(m, a),
        Target::PupDisplay => super::pup::member(m, a),
    };
    match handled {
        Some(r) => r,
        None => fallback(target, iface, m, a),
    }
}

/// Members we don't support yet: assignments are stored, reads give back
/// what was stored (or nothing), methods do nothing. Each is named once.
fn fallback(target: Target, iface: &'static Interface, m: &'static Member, a: &Args) -> Res {
    let example = match target {
        Target::Part(p) => with(|_, w| w.parts[p as usize].name.clone()),
        _ => None,
    };
    log::unsupported(iface.name.trim_start_matches('I'), m.name, example.as_deref());
    let key = (owner_key(target), m.dispid);
    match m.access {
        Access::Put | Access::PutRef => {
            if let Some(v) = a.value_kept() {
                with(|_, w| {
                    w.kept.insert(key, v);
                });
            }
            Ok(Ret::Empty)
        }
        // never set: the value the table file gives, as VPX would read it
        Access::Get => Ok(with(|_, w| match w.kept.get(&key) {
            Some(v) => Ret::Kept(v.clone()),
            None => initial_value(w, target, m.name).unwrap_or(Ret::Empty),
        })
        .unwrap_or(Ret::Empty)),
        Access::Method => Ok(Ret::Empty),
    }
}

/// What VPX's getter returns for a property no script has set yet: the
/// table file's value. The properties scripts commonly read to change and
/// set back (looks, mostly), so the script works with the right numbers.
fn initial_value(w: &World, target: Target, member: &str) -> Option<Ret> {
    let f = Ret::Float;
    let s = |v: &str| Ret::Str(v.to_string());
    let c = |v: &vpin::vpx::color::Color| Ret::Int(crate::light::colorref(v) as i32);
    if target == Target::Table {
        let g = &w.vpx.gamedata;
        return Some(match member {
            "DefaultBulbIntensityScale" => f(g.default_bulb_intensity_scale_on_ball.unwrap_or(1.0)),
            _ => return None,
        });
    }
    let Target::Part(p) = target else { return None };
    Some(match (&w.vpx.gameitems[p as usize], member) {
        (GameItemEnum::Light(l), "Image") => s(&l.image),
        (GameItemEnum::Wall(wl), "HasHitEvent") => Ret::Bool(wl.hit_event),
        (GameItemEnum::HitTarget(t), "Orientation") => f(t.rot_z),
        (GameItemEnum::Flipper(fl), m) => match m {
            // its pivot (VPX's m_d.m_Center), which flipper scripts measure from
            "X" => f(fl.center.x),
            "Y" => f(fl.center.y),
            "Length" => f(fl.flipper_radius_max),
            "BaseRadius" => f(fl.base_radius),
            "EndRadius" => f(fl.end_radius),
            "FlipperRadiusMin" => f(fl.flipper_radius_min),
            "Height" => f(fl.height),
            "Material" => s(&fl.material),
            "RubberMaterial" => s(&fl.rubber_material),
            "RubberThickness" => f(fl.rubber_thickness.unwrap_or(fl.rubber_thickness_int as f32)),
            "RubberHeight" => f(fl.rubber_height.unwrap_or(fl.rubber_height_int as f32)),
            "RubberWidth" => f(fl.rubber_width.unwrap_or(fl.rubber_width_int as f32)),
            "Surface" => s(&fl.surface),
            "Image" => s(fl.image.as_deref().unwrap_or("")),
            "ReflectionEnabled" => Ret::Bool(fl.is_reflection_enabled.unwrap_or(true)),
            _ => return None,
        },
        (GameItemEnum::Flasher(fl), m) => match m {
            "Height" => f(fl.height),
            "X" => f(fl.pos_x.unwrap_or(0.0)),
            "Y" => f(fl.pos_y.unwrap_or(0.0)),
            "RotX" => f(fl.rot_x),
            "RotY" => f(fl.rot_y),
            "RotZ" => f(fl.rot_z),
            "Color" => c(&fl.color),
            "ImageA" => s(&fl.image_a),
            "ImageB" => s(&fl.image_b),
            "Opacity" => Ret::Int(fl.alpha),
            "IntensityScale" => f(1.0),
            "ModulateVsAdd" => f(fl.modulate_vs_add),
            "Amount" => Ret::Int(fl.filter_amount as i32),
            "DisplayTexture" => Ret::Bool(fl.display_texture),
            "DepthBias" => f(fl.depth_bias),
            _ => return None,
        },
        (GameItemEnum::Primitive(pr), m) => match m {
            "Material" => s(&pr.material),
            "Image" => s(&pr.image),
            "NormalMap" => s(pr.normal_map.as_deref().unwrap_or("")),
            "ObjectSpaceNormalMap" => Ret::Bool(pr.object_space_normal_map.unwrap_or(false)),
            "BlendDisableLighting" => f(crate::look::disable_lighting_top(pr.disable_lighting_top, pr.disable_lighting_top_old)),
            "SideColor" => c(&pr.side_color),
            "DepthBias" => f(pr.depth_bias),
            "ReflectionProbe" => s(pr.reflection_probe.as_deref().unwrap_or("")),
            "RefractionProbe" => s(pr.refraction_probe.as_deref().unwrap_or("")),
            "ReflectionEnabled" => Ret::Bool(pr.is_reflection_enabled.unwrap_or(true)),
            "BlendDisableLightingFromBelow" => f(crate::scene::primitive_below(&w.vpx, &w.materials, pr)),
            _ => return None,
        },
        (GameItemEnum::Ramp(r), m) => match m {
            "Image" => s(&r.image),
            "Material" => s(&r.material),
            "ReflectionEnabled" => Ret::Bool(r.is_reflection_enabled.unwrap_or(true)),
            "HeightTop" => f(r.height_top),
            "HeightBottom" => f(r.height_bottom),
            "WidthTop" => f(r.width_top),
            "WidthBottom" => f(r.width_bottom),
            _ => return None,
        },
        (GameItemEnum::Wall(wl), m) => match m {
            "TopMaterial" => s(&wl.top_material),
            "SideMaterial" => s(&wl.side_material),
            "Image" => s(&wl.image),
            "SideImage" => s(&wl.side_image),
            "SideVisible" => Ret::Bool(wl.is_side_visible),
            "BlendDisableLighting" => f(crate::look::disable_lighting_top(wl.disable_lighting_top, wl.disable_lighting_top_old)),
            "BlendDisableLightingFromBelow" => f(wl.disable_lighting_below.unwrap_or(1.0)),
            "ReflectionEnabled" => Ret::Bool(wl.is_reflection_enabled.unwrap_or(true)),
            "HeightTop" => f(wl.height_top),
            "HeightBottom" => f(wl.height_bottom),
            _ => return None,
        },
        (GameItemEnum::Spinner(sp), m) => match m {
            "Image" => s(&sp.image),
            "Material" => s(&sp.material),
            "ReflectionEnabled" => Ret::Bool(sp.is_reflection_enabled.unwrap_or(true)),
            _ => return None,
        },
        (GameItemEnum::HitTarget(t), m) => match m {
            "BlendDisableLighting" => f(crate::look::disable_lighting_top(t.disable_lighting_top, t.disable_lighting_top_old)),
            "BlendDisableLightingFromBelow" => f(t.disable_lighting_below.unwrap_or(1.0)),
            "ReflectionEnabled" => Ret::Bool(t.is_reflection_enabled),
            "Image" => s(&t.image),
            "Material" => s(&t.material),
            "DropSpeed" => f(t.drop_speed),
            _ => return None,
        },
        (GameItemEnum::Rubber(r), m) => match m {
            "Image" => s(&r.image),
            "Material" => s(&r.material),
            "ReflectionEnabled" => Ret::Bool(r.is_reflection_enabled.unwrap_or(true)),
            _ => return None,
        },
        (GameItemEnum::Plunger(pl), m) => match m {
            "Image" => s(&pl.image),
            "Material" => s(&pl.material),
            "ReflectionEnabled" => Ret::Bool(pl.is_reflection_enabled.unwrap_or(true)),
            _ => return None,
        },
        (GameItemEnum::Trigger(t), "Material") => s(&t.material),
        (GameItemEnum::Trigger(t), "ReflectionEnabled") => Ret::Bool(t.is_reflection_enabled.unwrap_or(true)),
        (GameItemEnum::Kicker(k), "Material") => s(&k.material),
        (GameItemEnum::Gate(g), "Material") => s(&g.material),
        (GameItemEnum::Gate(g), "ReflectionEnabled") => Ret::Bool(g.is_reflection_enabled.unwrap_or(true)),
        (GameItemEnum::Bumper(b), "ReflectionEnabled") => Ret::Bool(b.is_reflection_enabled.unwrap_or(true)),
        _ => return None,
    })
}

fn is(m: &Member, name: &str) -> bool {
    m.name.eq_ignore_ascii_case(name)
}

// ----------------------------------------------------------------- global

/// The library's folders the settings name (first setup), when they do.
static LIBRARY: std::sync::OnceLock<vpi_settings::LibraryFolders> = std::sync::OnceLock::new();

/// Sets the library's folders, once, before the table's script starts.
pub fn set_library(folders: vpi_settings::LibraryFolders) {
    let _ = LIBRARY.set(folders);
}

/// The library's folders, when the settings name them.
pub fn library() -> Option<&'static vpi_settings::LibraryFolders> {
    LIBRARY.get()
}

/// The library's VPX folder: the one holding its tables' folder, where VPX keeps its `Scripts` and
/// `Music` beside `Tables`.
fn library_root() -> Option<PathBuf> {
    library().and_then(|f| f.tables.parent().map(std::path::Path::to_path_buf))
}

/// Where `GetTextFile` looks: the table's folder, the scripts shipped with
/// the player, then (until the player ships them) those of VPX build
/// 5436, the build the player follows, in the repository's
/// `vpx-build`, then those of a VPX installed on this machine, for any
/// those lack. The build's own matter: its `core.vbs` defines what the
/// installed 10.8.0 copy leaves undefined (`HasTimeFence`), which that
/// copy's `vpmInit` reads as true, holding PinMAME at 0.01 s.
pub(crate) fn script_folders(w: &World) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = w.table_dir.clone() {
        out.push(dir);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        out.push(dir.join("scripts"));
        if crate::DEVELOPMENT
            && let Some(build) = dir.ancestors().map(|d| d.join(r"vpx-build\vpinball-5436\scripts")).find(|d| d.is_dir())
        {
            out.push(build);
        }
    }
    // the library's own scripts folder, as the settings name it (first setup)
    if let Some(scripts) = library().and_then(|f| f.scripts.clone()).or_else(|| library_root().map(|r| r.join("Scripts"))) {
        out.push(scripts);
    }
    out.push(PathBuf::from(r"C:\vPinball\VisualPinball\Scripts"));
    // the shared folder of PinUP packs, where scripts read their pack's own files
    if let Some(root) = w.table_path.as_deref().and_then(crate::pup::Pup::find_root) {
        out.push(root);
    }
    out
}

/// Where music is looked for, as VPX looks: the `music` folder beside the
/// table, the player's own `music` folder, then (until the player has
/// settings of its own) that of a VPX installed on this machine.
pub(crate) fn music_folders(w: &World) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Some(dir) = &w.table_dir {
        folders.push(dir.join("music"));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        folders.push(dir.join("music"));
    }
    // the library's own, beside its tables' folder as VPX keeps it (first setup)
    if let Some(root) = library_root() {
        folders.push(root.join("Music"));
    }
    folders.push(PathBuf::from(r"C:\vPinball\VisualPinball\Music"));
    folders
}

/// The music file `PlayMusic` names, in the first music folder that has
/// it. Only files inside those folders: a name that climbs out or
/// names a drive finds nothing.
fn music_file(w: &World, name: &str) -> Option<PathBuf> {
    let relative: PathBuf = name.split(['/', '\\']).filter(|c| !c.is_empty()).collect();
    if relative.as_os_str().is_empty() || relative.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        return None;
    }
    music_folders(w).into_iter().map(|d| d.join(&relative)).find(|p| p.is_file())
}

/// VPX's user folder, where tables keep their high scores and settings:
/// the player's own data folder, made the first time a script asks for it
/// (VPX's `GetTablePath(User, true)`), so a table never writes beside
/// itself.
pub(crate) fn user_directory() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("User");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// A folder as VPX hands it to scripts: ending in a separator, or a
/// failure when there is none.
fn folder_ret(dir: Option<PathBuf>) -> Res {
    let Some(dir) = dir.filter(|d| d.is_dir()) else { return Err(E_FAIL) };
    let mut s = dir.to_string_lossy().into_owned();
    if !s.ends_with('\\') {
        s.push('\\');
    }
    Ok(Ret::Str(s))
}

/// A text file as VPX reads one: UTF-8 or UTF-16 by their marks, else the
/// Windows code page (read here as Latin-1, which matches it for letters).
fn read_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// VPX build 5436's `ScriptGlobalTable::get_VPXActionKey`: each input action
/// by number, which `VPMKeys.vbs` reads its keys from (coins, the coin
/// door's buttons, reset). Build 5436 hands scripts action numbers
/// (`0x10000 | action`); the player hands keyboard codes throughout, so
/// this answers the action's key, VPX's default (`InputManager.cpp`).
fn action_key(k: &crate::dik::Keys, index: i32) -> Option<u8> {
    Some(match index {
        0 => k.left_flipper,
        1 => k.right_flipper,
        2 => k.staged_left_flipper,
        3 => k.staged_right_flipper,
        4 => k.left_tilt,
        5 => k.right_tilt,
        6 => k.center_tilt,
        7 => k.plunger,
        8 => k.start_game,
        9 => k.add_credit,
        10 => k.add_credit_2,
        11 => 0x04, // 3
        12 => 0x07, // 6
        13 => k.mech_tilt,
        14 => k.left_magna_save,
        15 => k.right_magna_save,
        16 => k.exit_game,
        17 => 0x58, // F12: the in-game menu
        18 => k.lockbar,
        19 => 0x3D, // F3: reset
        20 => 0x0C, // minus: volume down
        21 => 0x0D, // equals: volume up
        22 => 0x30, // B: extra ball
        23 => 0xC7, // Home: slam tilt
        24 => 0xCF, // End: coin door
        // the service buttons: 7, 8, 9, 0, 6, Page Up, minus, and one unassigned
        25 => 0x08,
        26 => 0x09,
        27 => 0x0A,
        28 => 0x0B,
        29 => 0x07,
        30 => 0xC9,
        31 => 0x0C,
        // and the four custom joystick buttons, unassigned
        32 | 64..=67 => 0,
        _ => return None,
    })
}

fn global(m: &'static Member, a: &Args) -> Option<Res> {
    let name = m.name;
    // the table's score display as the script sends it, a frame at a time (VPX's put_DMDWidth,
    // put_DMDHeight, put_DMDPixels and put_DMDColoredPixels); a frame the same as the last
    // keeps its number, so the renderer uploads only what changed
    if matches!(name, "DMDWidth" | "DMDHeight" | "DMDPixels" | "DMDColoredPixels") {
        return Some(
            with(|_, w| {
                match name {
                    "DMDWidth" => w.dmd_size[0] = a.value_i32().unwrap_or(0),
                    "DMDHeight" => w.dmd_size[1] = a.value_i32().unwrap_or(0),
                    _ => {
                        let ([width, height], Some(values)) = (w.dmd_size, a.value_array_u32()) else { return Err(E_FAIL) };
                        if width <= 0 || height <= 0 {
                            return Err(E_FAIL);
                        }
                        let id = w.dmd_frames + 1;
                        let frame = if name == "DMDPixels" {
                            let percent: Vec<i32> = values.iter().map(|&v| v as i32).collect();
                            crate::dmd::DmdFrame::from_percent(width as u32, height as u32, &percent, id)
                        } else {
                            crate::dmd::DmdFrame::from_srgb(width as u32, height as u32, &values, id)
                        };
                        let same = w.dmd_frame.as_ref().is_some_and(|f| f.dots == frame.dots && f.width == frame.width && f.luminance == frame.luminance);
                        if !same {
                            w.dmd_frames = id;
                            w.dmd_frame = Some(std::sync::Arc::new(frame));
                        }
                    }
                }
                Ok(Ret::Empty)
            })
            .unwrap_or(Err(E_FAIL)),
        );
    }
    // the keys a script compares against
    if let Some(k) = with(|_, w| {
        let k = &w.keys;
        Some(match name {
            "LeftFlipperKey" => Some(k.left_flipper),
            "RightFlipperKey" => Some(k.right_flipper),
            "StagedLeftFlipperKey" => Some(k.staged_left_flipper),
            "StagedRightFlipperKey" => Some(k.staged_right_flipper),
            "LeftTiltKey" => Some(k.left_tilt),
            "RightTiltKey" => Some(k.right_tilt),
            "CenterTiltKey" => Some(k.center_tilt),
            "PlungerKey" => Some(k.plunger),
            "StartGameKey" => Some(k.start_game),
            "AddCreditKey" => Some(k.add_credit),
            "AddCreditKey2" => Some(k.add_credit_2),
            "MechanicalTilt" => Some(k.mech_tilt),
            "LeftMagnaSave" => Some(k.left_magna_save),
            "RightMagnaSave" => Some(k.right_magna_save),
            "ExitGame" => Some(k.exit_game),
            "LockbarKey" => Some(k.lockbar),
            "VPXActionKey" => action_key(k, a.i32(0).unwrap_or(-1)),
            _ => return None,
        })
    })
    .flatten()
    {
        return Some(k.map_or(Err(E_FAIL), |k| Ok(Ret::Int(i32::from(k)))));
    }
    Some(match name {
        "PlaySound" => {
            let sound = a.string(0).unwrap_or_default();
            let opt_f = |i: usize, d: f32| a.f32(i).unwrap_or(d);
            let opt_i = |i: usize, d: i32| a.i32(i).unwrap_or(d);
            let opt_b = |i: usize, d: bool| a.bool(i).unwrap_or(d);
            let (loops, volume, pan, random_pitch, pitch, use_same, restart, fade) =
                (opt_i(1, 1), opt_f(2, 1.0), opt_f(3, 0.0), opt_f(4, 0.0), opt_i(5, 0), opt_b(6, false), opt_b(7, true), opt_f(8, 0.0));
            with(|e, w| {
                // a random pitch draws from VPX's shared generator (rand_mt_01)
                let rng = &mut e.env.rng;
                w.sounds.play(&sound, loops, volume, pan, fade, random_pitch, pitch, use_same, restart, || rng.rand_01());
            });
            Ok(Ret::Empty)
        }
        "StopSound" => {
            let sound = a.string(0).unwrap_or_default();
            with(|_, w| w.sounds.stop(&sound));
            Ok(Ret::Empty)
        }
        // VPX's PlayMusic: any music pauses, then the file plays once, at the table's music
        // volume times the script's
        "PlayMusic" => {
            let file = a.string(0).unwrap_or_default();
            let volume = a.f32(1).unwrap_or(1.0);
            with(|_, w| {
                w.sounds.pause_music();
                if file.is_empty() {
                    return;
                }
                match music_file(w, &file) {
                    Some(path) => w.sounds.play_music(path, w.vpx.gamedata.table_music_volume * volume),
                    None => log::once(&format!("PlayMusic: {file} is not in the table's music folder")),
                }
            });
            Ok(Ret::Empty)
        }
        "EndMusic" => {
            with(|_, w| w.sounds.pause_music());
            Ok(Ret::Empty)
        }
        "MusicVolume" => {
            let volume = a.value_f32().unwrap_or(1.0);
            with(|_, w| w.sounds.set_music_volume(volume));
            Ok(Ret::Empty)
        }
        "FireKnocker" => {
            log::once("FireKnocker: the knocker comes with cabinet feedback");
            Ok(Ret::Empty)
        }
        // a push of the cabinet (physics_compat::nudge)
        "Nudge" => {
            let (angle, force) = (a.f32(0).unwrap_or(0.0), a.f32(1).unwrap_or(0.0));
            with(|e, _| {
                if let Some(n) = &mut e.nudge {
                    n.apply_keyboard_impulse(angle, force);
                }
            });
            Ok(Ret::Empty)
        }
        // an accelerometer's old calibration: VPX 5436 does nothing with it now
        "NudgeGetCalibration" | "NudgeSetCalibration" => Ok(Ret::Empty),
        // the largest cabinet acceleration since the last call (VPX 5436's tilt bob keeps it)
        "NudgeSensorStatus" => {
            let v = with(|e, _| e.plumb.as_mut().map(|p| p.take_sensor_status())).flatten().unwrap_or_default();
            a.set(0, Ret::Float(v.x));
            a.set(1, Ret::Float(v.y));
            Ok(Ret::Empty)
        }
        // the tilt bob's largest swing and tilt percentage since the last call
        "NudgeTiltStatus" => {
            let (swing, tilt) = with(|e, _| e.plumb.as_mut().map(|p| p.take_tilt_status())).flatten().unwrap_or_default();
            a.set(0, Ret::Float(swing.x));
            a.set(1, Ret::Float(swing.y));
            a.set(2, Ret::Float(tilt));
            Ok(Ret::Empty)
        }
        "GetBalls" => Ok(with(|e, w| {
            let ids: Vec<BallId> = e.balls().iter().map(|b| b.id).collect();
            Ret::Array(ids.into_iter().map(|id| Ret::Object(Obj::dispatch(w.ball_object(id)))).collect())
        })
        .unwrap_or_default()),
        "ActiveBall" => with(|e, w| match w.active_ball {
            Some(id) if e.balls().iter().any(|b| b.id == id) => Ok(Ret::Object(Obj::dispatch(w.ball_object(id)))),
            _ => Err(E_POINTER),
        })
        .unwrap_or(Err(E_POINTER)),
        "ActiveTable" => Ok(with(|_, w| Ret::Object(Obj::dispatch(w.table))).unwrap_or_default()),
        "GetTextFile" => {
            let file = a.string(0).unwrap_or_default();
            let folders = with(|_, w| script_folders(w)).unwrap_or_default();
            match folders.iter().map(|d| d.join(&file)).find_map(|p| std::fs::read(p).ok()) {
                Some(bytes) => Ok(Ret::Str(read_text(&bytes))),
                None => {
                    log::once(&format!("GetTextFile: {file} not found"));
                    Err(E_FAIL)
                }
            }
        }
        "GameTime" => Ok(Ret::Int(with(|e, _| e.time_msec() as i32).unwrap_or(0))),
        "SystemTime" => Ok(Ret::Int(with(|_, w| w.started.elapsed().as_millis() as i32).unwrap_or(0))),
        // VPX build 5436 (10.8.1 pre-release), the code the player follows
        "Version" => Ok(Ret::Int(10801)),
        "VersionMajor" => Ok(Ret::Int(10)),
        "VersionMinor" => Ok(Ret::Int(8)),
        "VersionRevision" => Ok(Ret::Int(1)),
        "VPBuildVersion" => Ok(Ret::Double(10801.0 + 5436.0 / 10000.0)),
        // VPX's platform names (src/core/def.h's GET_PLATFORM_*), for Windows on a 64-bit PC
        "PlatformOS" => Ok(Ret::Str("windows".into())),
        "PlatformCPU" => Ok(Ret::Str("x86".into())),
        "PlatformBits" => Ok(Ret::Str("64".into())),
        // the command line's custom parameters (VPX's `-c1` to `-c9`): none given here
        "GetCustomParam" => match a.i32(0) {
            Some(1..=9) => Ok(Ret::Str(String::new())),
            _ => Err(E_FAIL),
        },
        "GetElements" => Ok(with(|_, w| Ret::Array(w.parts.iter().map(|p| Ret::Object(Obj::dispatch(p.obj))).collect())).unwrap_or_default()),
        "GetElementByName" => {
            let n = a.string(0).unwrap_or_default();
            // VPX compares the name exactly, case included
            Ok(with(|_, w| {
                w.parts.iter().find(|p| p.name == n).map_or(Ret::Object(std::ptr::null_mut()), |p| Ret::Object(Obj::dispatch(p.obj)))
            })
            .unwrap_or_default())
        }
        // the values a script keeps between plays, as VPX 5436 keeps them (`saved_values.rs`):
        // handed back as text, an empty one for a value never saved
        "LoadValue" => {
            let (table, name) = (a.string(0).unwrap_or_default(), a.string(1).unwrap_or_default());
            Ok(Ret::Str(with(|_, w| w.saved_values.get(&table, &name).map(str::to_string)).flatten().unwrap_or_default()))
        }
        "SaveValue" => {
            let (table, name) = (a.string(0).unwrap_or_default(), a.string(1).unwrap_or_default());
            // VPX's `BstrFromVariant`: a yes or no as `True` or `False`, else the value as text,
            // nothing when it can't be
            let text = match a.get(2) {
                Some(v) if v.vt == VT_BOOL => (if v.i16() != 0 { "True" } else { "False" }).to_string(),
                Some(v) => super::winevbs::get().map(|lib| super::value::text(lib, v)).unwrap_or_default(),
                None => String::new(),
            };
            // a new high score among them takes a screenshot, when that's on (`screenshot.rs`)
            let result = with(|_, w| {
                crate::screenshot::saved_value(&name, &text, w.saved_values.get(&table, &name));
                w.saved_values.set(&table, &name, text)
            });
            if let Some(Err(e)) = result {
                log::once(&format!("SaveValue: {e}"));
            }
            Ok(Ret::Empty)
        }
        "BeginModal" | "EndModal" | "ShowCursor" | "DisableStaticPrerendering" => Ok(Ret::Empty),
        "QuitPlayer" => {
            with(|_, w| w.quit_requested = true);
            Ok(Ret::Empty)
        }
        "GetPlayerHWnd" => Ok(Ret::Int(0)),
        "FrameIndex" => Ok(Ret::Int(with(|_, w| w.frame_index as i32).unwrap_or(0))),
        // the precise game time, in seconds
        "PreciseGameTime" => Ok(Ret::Double(with(|e, _| e.time_sec()).unwrap_or(0.0))),
        "RenderingMode" => Ok(Ret::Int(0)), // 2D: no stereo, no VR
        // VPX: the desktop and single-screen views count as desktop
        "ShowDT" => Ok(Ret::Bool(with(|_, w| w.view != View::Cabinet).unwrap_or(true))),
        "ShowFSS" => Ok(Ret::Bool(with(|_, w| w.view == View::SingleScreen).unwrap_or(false))),
        "NightDay" if m.access == Access::Get => {
            // VPX's quantizeUnsignedPercent of the scene's light level (`SceneLighting`): the player's
            // when chosen, else the table's
            Ok(Ret::Int(with(|_, w| ((w.light_level.unwrap_or(w.vpx.gamedata.global_emission_scale).max(0.0) * 101.0) as u32).min(100) as i32).unwrap_or(100)))
        }
        "Setting" => {
            // VPX's settings, for the few scripts read: `controller.vbs`'s, from the settings file
            // or VPX's defaults
            let (section, key) = (a.string(0).unwrap_or_default(), a.string(1).unwrap_or_default());
            match controller_setting(&key).filter(|_| section.eq_ignore_ascii_case("Controller")) {
                Some(v) => Ok(Ret::Str(v.to_string())),
                None => Err(E_FAIL), // VPX: the setting isn't there
            }
        }
        // the table's materials: a name the table doesn't have fails, as in VPX
        "UpdateMaterial" => update_material(a),
        "GetMaterial" => get_material(a),
        "MaterialColor" => with_material(a, |m| {
            let base = a.i32(1).unwrap_or(0) as u32;
            m.changed |= m.base != base;
            m.base = base;
        }),
        "UpdateMaterialPhysics" => with_material(a, |m| {
            let f = |i| a.f32(i).unwrap_or(0.0);
            (m.elasticity, m.elasticity_falloff, m.friction, m.scatter_angle) = (f(1), f(2), f(3), f(4));
        }),
        "GetMaterialPhysics" => {
            let m = with(|_, w| w.materials.find(&a.string(0).unwrap_or_default()).map(|k| w.materials.all[k as usize].clone())).flatten();
            match m {
                Some(m) => {
                    for (i, v) in [m.elasticity, m.elasticity_falloff, m.friction, m.scatter_angle].into_iter().enumerate() {
                        a.set(i + 1, Ret::Float(v));
                    }
                    Ok(Ret::Empty)
                }
                None => Err(E_FAIL),
            }
        }
        // VPX's folders, each ending in a separator, or a failure when there is none: the user
        // folder (the player's own, never the table's, which may be read-only), the table's
        // folder, the scripts folder, and the music folder with an optional subfolder
        "UserDirectory" => folder_ret(user_directory()),
        "TablesDirectory" => folder_ret(with(|_, w| w.table_dir.clone()).flatten()),
        "ScriptsDirectory" => {
            // the folders GetTextFile looks in after the table's own
            let shared = with(|_, w| script_folders(w).into_iter().skip(usize::from(w.table_dir.is_some())).collect::<Vec<_>>()).unwrap_or_default();
            folder_ret(shared.into_iter().find(|d| d.is_dir()))
        }
        "MusicDirectory" => {
            let sub = a.string(0).filter(|s| !s.is_empty());
            let music = with(|_, w| music_folders(w)).unwrap_or_default().into_iter().find(|d| d.is_dir());
            folder_ret(music.map(|d| sub.map_or(d.clone(), |s| d.join(s))).filter(|d| d.is_dir()))
        }
        _ => return None,
    })
}

/// Changes the material named by the first argument, or fails as VPX does
/// for a name the table doesn't have.
fn with_material(a: &Args, change: impl FnOnce(&mut crate::look::MaterialRt)) -> Res {
    let name = a.string(0).unwrap_or_default();
    with(|_, w| {
        let k = w.materials.find(&name)?;
        change(&mut w.materials.all[k as usize]);
        Some(())
    })
    .flatten()
    .map_or(Err(E_FAIL), |()| Ok(Ret::Empty))
}

/// `UpdateMaterial name, wrapLighting, roughness, glossyImageLerp,
/// thickness, edge, edgeAlpha, opacity, base, glossy, clearcoat, isMetal,
/// opacityActive, elasticity, elasticityFalloff, friction, scatterAngle`.
/// The physics values change the material, not the colliders already made
/// from it (VPX copies them when play starts).
fn update_material(a: &Args) -> Res {
    let f = |i| a.f32(i).unwrap_or(0.0);
    let c = |i| a.i32(i).unwrap_or(0) as u32;
    let b = |i| a.bool(i).unwrap_or(false);
    with_material(a, |m| {
        let before = m.now();
        m.wrap_lighting = f(1);
        m.roughness = f(2);
        m.glossy_image_lerp = f(3);
        m.thickness = f(4);
        m.edge = f(5);
        m.edge_alpha = f(6);
        m.opacity = f(7);
        m.base = c(8);
        m.glossy = c(9);
        m.clearcoat = c(10);
        m.metal = b(11);
        m.opacity_active = b(12);
        m.elasticity = f(13);
        m.elasticity_falloff = f(14);
        m.friction = f(15);
        m.scatter_angle = f(16);
        // only a change to how it looks takes it off the table file's look
        m.changed |= m.now() != before;
    })
}

/// `GetMaterial name, wrapLighting, ...`: the material's values into the
/// script's variables, in `UpdateMaterial`'s order.
fn get_material(a: &Args) -> Res {
    let name = a.string(0).unwrap_or_default();
    let Some(m) = with(|_, w| w.materials.find(&name).map(|k| w.materials.all[k as usize].clone())).flatten() else { return Err(E_FAIL) };
    let f = Ret::Float;
    // colours as whole numbers (VPX hands OLE_COLORs), which libwinevbs's arithmetic takes
    let c = |v: u32| Ret::Int(v as i32);
    let values = [
        f(m.wrap_lighting),
        f(m.roughness),
        f(m.glossy_image_lerp),
        f(m.thickness),
        f(m.edge),
        f(m.edge_alpha),
        f(m.opacity),
        c(m.base),
        c(m.glossy),
        c(m.clearcoat),
        Ret::Bool(m.metal),
        Ret::Bool(m.opacity_active),
        f(m.elasticity),
        f(m.elasticity_falloff),
        f(m.friction),
        f(m.scatter_angle),
    ];
    let mut set = 0;
    for (i, v) in values.into_iter().enumerate() {
        set += usize::from(a.set(i + 1, v));
    }
    if set == 0 && a.len() > 1 {
        log::once("GetMaterial: the script's variables came by value, so they can't be set");
    }
    Ok(Ret::Empty)
}

// ------------------------------------------------------------------ table

fn table(m: &'static Member, a: &Args) -> Option<Res> {
    if m.name == "ColorGradeImage" {
        return table_color_grade(m, a);
    }
    // the bloom's strength, read by VPX's renderer every frame (pintable.cpp, Renderer.cpp)
    if m.name == "BloomStrength" {
        return with(|_, w| match m.access {
            Access::Put => {
                w.bloom_strength = Some(a.value_f32().unwrap_or(0.0));
                Ok(Ret::Empty)
            }
            _ => Ok(Ret::Float(w.bloom_strength.unwrap_or(w.vpx.gamedata.bloom_strength))),
        });
    }
    // VPX's version, as the global table gives it, and the members VPX 5436 answers with set
    // values only, its setting gone (src/parts/pintable.cpp: a put does nothing)
    let constant = match m.name {
        "Version" => Some(Ret::Int(10801)),
        "VersionMajor" => Some(Ret::Int(10)),
        "VersionMinor" => Some(Ret::Int(8)),
        "VersionRevision" => Some(Ret::Int(1)),
        "VPBuildVersion" => Some(Ret::Double(10801.0 + 5436.0 / 10000.0)),
        "AccelNormalMount" | "ReflectElementsOnPlayfield" | "GlobalAlphaAcc" => Some(Ret::Bool(true)),
        "Accelerometer" | "GlobalDayNight" | "GlobalStereo3D" | "PlungerFilter" => Some(Ret::Bool(false)),
        "AccelerometerAngle" | "MaxSeparation" | "Offset" | "TableHeight" | "ZPD" => Some(Ret::Float(0.0)),
        "DeadZone" | "YieldTime" => Some(Ret::Int(0)),
        "PlungerNormalize" => Some(Ret::Int(100)),
        "TableAdaptiveVSync" | "EnableAntialiasing" | "EnableFXAA" => Some(Ret::Int(-1)),
        // VPX's UserDefaultOnOff::On
        "BallReflection" => Some(Ret::Int(1)),
        _ => None,
    };
    if let Some(value) = constant {
        return Some(Ok(if matches!(m.access, Access::Put | Access::PutRef) { Ret::Empty } else { value }));
    }
    // the look new balls take (VPX: the table's, read by each ball's RenderSetup)
    if matches!(m.name, "BallImage" | "BallFrontDecal" | "BallDecalMode" | "DefaultBulbIntensityScale") {
        let put = m.access == Access::Put;
        return with(|_, w| {
            let look = &mut w.ball_table_look;
            Some(Ok(if put {
                match m.name {
                    "BallImage" => look.image = a.value_string()?,
                    "BallFrontDecal" => look.decal = a.value_string()?,
                    "BallDecalMode" => look.decal_mode = a.value_bool()?,
                    _ => look.bulb_scale = a.value_f32()?,
                }
                Ret::Empty
            } else {
                match m.name {
                    "BallImage" => Ret::Str(look.image.clone()),
                    "BallFrontDecal" => Ret::Str(look.decal.clone()),
                    "BallDecalMode" => Ret::Bool(look.decal_mode),
                    _ => Ret::Float(look.bulb_scale),
                }
            }))
        })
        .flatten();
    }
    // the table's own day and night (PinTable's, not the player's): set, it's kept, as VPX keeps it in
    // the table, the lighting staying as play began
    if m.name == "NightDay" {
        return with(|_, w| {
            if m.access == Access::Put {
                // VPX's dequantizeUnsignedPercent
                w.table_night_day = Some((a.value_i32().unwrap_or(0).max(0) as f32 / 100.0).min(1.0));
                Ok(Ret::Empty)
            } else {
                // VPX's quantizeUnsignedPercent
                let scale = w.table_night_day.unwrap_or(w.vpx.gamedata.global_emission_scale);
                Ok(Ret::Int(((scale.max(0.0) * 101.0) as u32).min(100) as i32))
            }
        });
    }
    // kept in the table (PinTable::put_PlayfieldReflectionStrength), which VPX 5436 reads only at
    // load: set during play, it changes nothing drawn
    if m.name == "PlayfieldReflectionStrength" {
        return with(|_, w| {
            if m.access == Access::Put {
                // VPX's dequantizeUnsignedPercent
                w.table_reflection_strength = Some((a.value_i32().unwrap_or(0).max(0) as f32 / 100.0).min(1.0));
                Ok(Ret::Empty)
            } else {
                // VPX's quantizeUnsignedPercent
                let strength = w.table_reflection_strength.unwrap_or(w.vpx.gamedata.playfield_reflection_strength);
                Ok(Ret::Int(((strength.max(0.0) * 101.0) as u32).min(100) as i32))
            }
        });
    }
    // the backdrop pictures' names (PinTable::put_BackdropImage_DT and the rest): kept in the table,
    // the backdrop drawn into the still parts at start, so a change shows nothing during play
    let backdrop = match m.name {
        "BackdropImage_DT" => Some(0),
        "BackdropImage_FS" => Some(1),
        "BackdropImage_FSS" => Some(2),
        _ => None,
    };
    if let Some(k) = backdrop {
        return with(|_, w| {
            if m.access == Access::Put {
                w.backdrop_images[k] = Some(a.value_string().unwrap_or_default());
                return Ok(Ret::Empty);
            }
            let g = &w.vpx.gamedata;
            let file = match k {
                0 => g.backglass_image_full_desktop.clone(),
                1 => g.backglass_image_full_fullscreen.clone(),
                _ => g.backglass_image_full_single_screen.clone().unwrap_or_default(),
            };
            Ok(Ret::Str(w.backdrop_images[k].clone().unwrap_or(file)))
        });
    }
    if m.access == Access::Put {
        return table_gravity(m, a);
    }
    if m.access != Access::Get {
        return None;
    }
    with(|e, w| {
        let g = &w.vpx.gamedata;
        Some(match m.name {
            "Width" => Ok(Ret::Float(g.right - g.left)),
            "Height" => Ok(Ret::Float(g.bottom - g.top)),
            "ShowDT" => Ok(Ret::Bool(w.view != View::Cabinet)),
            // the table's own single-screen setting (VPX's IsFSSEnabled)
            "ShowFSS" => Ok(Ret::Bool(g.bg_enable_fss.unwrap_or(false))),
            "Gravity" => Ok(Ret::Float(w.gravity.gravity * (1.0f64 / f64::from(GRAVITYCONST)) as f32)),
            "SlopeMin" => Ok(Ret::Float(w.gravity.slope_min)),
            "SlopeMax" => Ok(Ret::Float(w.gravity.slope_max)),
            "GlobalDifficulty" => Ok(Ret::Float(e.env.global_difficulty * 100.0)),
            "Name" => Ok(Ret::Str(w.table_name.clone())),
            "FileName" => Ok(Ret::Str(w.table_file.clone())),
            "Option" => table_option(a),
            _ => return None,
        })
    })
    .flatten()
}

/// VPX's `ColorGradeImage` (build 5436): its renderer looks the picture up
/// by name each frame, so a change shows at once, and a name no picture has,
/// or none, means no colour grade. A picture that isn't 256 by 16 is
/// refused, as VPX refuses it.
fn table_color_grade(m: &'static Member, a: &Args) -> Option<Res> {
    with(|_, w| match m.access {
        Access::Put => {
            let name = a.value_string().unwrap_or_default();
            if let Some(i) = w.image_named(&name)
                && let Some(image) = w.vpx.images.get(i as usize)
                && (image.width != 256 || image.height != 16)
            {
                return Err(E_FAIL);
            }
            w.color_grade = Some(name);
            Ok(Ret::Empty)
        }
        _ => Ok(Ret::Str(w.color_grade.clone().unwrap_or_else(|| w.vpx.gamedata.image_color_grade.clone()))),
    })
}

/// VPX's setters for the table's gravity and slopes (build 5436): the
/// physics takes them at once. The difficulty is fixed while playing.
fn table_gravity(m: &'static Member, a: &Args) -> Option<Res> {
    if !matches!(m.name, "Gravity" | "SlopeMin" | "SlopeMax" | "GlobalDifficulty") {
        return None;
    }
    let v = a.value_f32().unwrap_or(0.0);
    with(|e, w| {
        let g = &mut w.gravity;
        let difficulty = e.env.global_difficulty;
        // VPX's lerp, between the slopes by difficulty
        let lerp = |min: f32, max: f32| (1.0 - difficulty) * min + difficulty * max;
        let strength = |g: &crate::world::TableGravity| g.set.map_or(g.gravity, |s| s.2);
        match m.name {
            "Gravity" => {
                g.gravity = v.max(0.0) * GRAVITYCONST;
                let (min, max) = g.set.map_or((g.slope_min, g.slope_max), |s| (s.0, s.1));
                e.set_gravity(lerp(min, max), strength(g));
            }
            // as VPX: the table's slopes, even with a physics set
            "SlopeMin" | "SlopeMax" => {
                if m.name == "SlopeMin" { g.slope_min = v } else { g.slope_max = v }
                e.set_gravity(lerp(g.slope_min, g.slope_max), strength(g));
            }
            _ => {}
        }
    });
    Some(Ok(Ret::Empty))
}

/// The settings `controller.vbs` reads (VPX 5436's `[Controller]`): `ForceDisableB2S`, 1 to leave
/// the backglass server unused, and the nine toys' `DOFContactors` … `DOFDropTargets` — 0 a hit
/// plays its sound only, 1 fires the cabinet's toy only, 2 both — from the settings file (a
/// number, or the names VPX shows: "Sound FX", "DOF", "Both"), else VPX's defaults: the backglass
/// server used, every toy both. None for any other name. The same answer whether a script asks
/// VPX's way (`Setting`) or, carrying an older `controller.vbs`, the registry's (toys
/// work on every table).
pub(crate) fn controller_setting(key: &str) -> Option<i32> {
    const TOYS: [&str; 9] = ["DOFContactors", "DOFKnocker", "DOFChimes", "DOFBell", "DOFGear", "DOFShaker", "DOFFlippers", "DOFTargets", "DOFDropTargets"];
    let stored = with(|_, w| w.controller_settings.iter().rev().find(|(k, v)| k.eq_ignore_ascii_case(key) && !v.is_empty()).map(|(_, v)| v.clone())).flatten();
    let stored = stored.as_deref().map(crate::settings::vpx_trim);
    if key.eq_ignore_ascii_case("ForceDisableB2S") {
        return Some(i32::from(stored.and_then(crate::settings::vpx_bool).unwrap_or(false)));
    }
    if !TOYS.iter().any(|t| t.eq_ignore_ascii_case(key)) {
        return None;
    }
    let by_name = |s: &str| ["Sound FX", "DOF", "Both"].iter().position(|n| *n == s).map(|k| k as i32);
    Some(stored.and_then(|s| by_name(s).or_else(|| crate::settings::vpx_int(s).filter(|v| (0..=2).contains(v)))).unwrap_or(2))
}

/// VPX's `Table.Option`: an option's value, from the settings' table options
/// (`[TableOption]`, a table's own settings file over the main one) under
/// build 5436's name for it, read as 5436 reads it (`RegisterOption`,
/// `LayeredINIPropertyStore::LoadFromINI`): a toggle (two names, off and on
/// or the like) as yes or no, one of several names by name or number, a
/// whole number, or a number held to its step; its default when it isn't set
/// or can't be read. Checked as build 5436 checks them. What 5436 refuses
/// with an error — a default outside the range, a list of names that isn't
/// one name a whole value — VPX 10.8.0 answers quietly, leaving the value
/// unset, so the script reads 0; ours answers so too (Darkest Dungeon's co-op
/// option has its default below its minimum, which leaves co-op off in 10.8.0). Options written by VPX 10.8.0
/// under their names as shown ("Rails Visible") aren't found under 5436's
/// ("Rails_Visible"), as in 5436.
fn table_option(a: &Args) -> Res {
    let quiet = Ok(Ret::Float(0.0));
    let (min, max, step, default) = (a.f32(1).unwrap_or(0.0), a.f32(2).unwrap_or(0.0), a.f32(3).unwrap_or(0.0), a.f32(4).unwrap_or(0.0));
    // a list of named values must be an array (or left out: VT_EMPTY, VT_ERROR)
    const VT_ARRAY_OF_VARIANTS: u16 = 0x2000 | 12;
    let named = match a.get(6).map(|v| v.vt) {
        None | Some(0 | 10) => false,
        Some(VT_ARRAY_OF_VARIANTS) => true,
        Some(_) => return quiet,
    };
    if min >= max || step <= 0.0 || default < min || default > max {
        return quiet;
    }
    // named values step by 1 between whole numbers, a name for each
    if named && (step != 1.0 || min.fract() != 0.0 || max.fract() != 0.0 || a.array_len(6) != usize::try_from(1 + max as i32 - min as i32).ok()) {
        return quiet;
    }
    let id = crate::settings::table_option_id(&a.string(0).unwrap_or_default());
    let stored = with(|_, w| w.table_options.iter().rev().find(|(k, v)| k.eq_ignore_ascii_case(&id) && !v.is_empty()).map(|(_, v)| v.clone())).flatten();
    let names = if named { Some(a.array_texts(6).unwrap_or_default()) } else { None };
    record_option(TableOptionDef {
        id: id.clone(),
        name: crate::settings::vpx_trim(&a.string(0).unwrap_or_default()).to_string(),
        min,
        max,
        step,
        default,
        unit: a.i32(5).unwrap_or(0),
        names: names.clone(),
    });
    Ok(Ret::Float(crate::settings::table_option_value(min, max, step, default, names.as_deref(), stored.as_deref())))
}

/// A table option as its script offers it (VPX's `Table.Option`'s arguments): what the app's
/// table settings show and set (`vpi-headless --options`).
#[derive(Clone, Debug, PartialEq)]
pub struct TableOptionDef {
    /// Its name in the settings (`[TableOption]`), build 5436's.
    pub id: String,
    /// As the script names it.
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub default: f32,
    /// VPX's unit: 0 none, 1 percent.
    pub unit: i32,
    /// Its values' names, when it has them.
    pub names: Option<Vec<String>>,
}

static OPTIONS: std::sync::Mutex<Vec<TableOptionDef>> = std::sync::Mutex::new(Vec::new());

fn record_option(def: TableOptionDef) {
    let mut options = OPTIONS.lock().unwrap_or_else(|e| e.into_inner());
    match options.iter_mut().find(|o| o.id.eq_ignore_ascii_case(&def.id)) {
        Some(o) => *o = def,
        None => options.push(def),
    }
}

/// The options the table's script has offered so far, in its order.
pub fn table_options() -> Vec<TableOptionDef> {
    OPTIONS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Those options as JSON, for the app.
pub fn table_options_json() -> String {
    let list: Vec<serde_json::Value> = table_options()
        .into_iter()
        .map(|o| serde_json::json!({ "id": o.id, "name": o.name, "min": o.min, "max": o.max, "step": o.step, "default": o.default, "unit": o.unit, "names": o.names }))
        .collect();
    serde_json::Value::Array(list).to_string()
}

// ------------------------------------------------------------------ debug

fn debug(m: &'static Member, a: &Args) -> Option<Res> {
    if !is(m, "Print") {
        return None;
    }
    println!("[debug] {}", a.string(0).unwrap_or_default());
    Some(Ok(Ret::Empty))
}

// ------------------------------------------------------------- collection

fn collection(c: u32, m: &'static Member, a: &Args) -> Option<Res> {
    with(|_, w| {
        let col = &w.collections[c as usize];
        Some(match m.name {
            "Count" => Ok(Ret::Int(col.parts.len() as i32)),
            "Item" => match a.i32(0).and_then(|i| col.parts.get(usize::try_from(i).ok()?)) {
                Some(&p) => Ok(Ret::Object(Obj::dispatch(w.parts[p as usize].obj))),
                None => Err(E_FAIL),
            },
            "_NewEnum" => Ok(Ret::Unknown(Enumerator::create(col.parts.iter().map(|&p| Obj::dispatch(w.parts[p as usize].obj)).collect()))),
            _ => return None,
        })
    })
    .flatten()
}

// ------------------------------------------------------------------- ball

fn ball(id: BallId, m: &'static Member, a: &Args) -> Option<Res> {
    let put = matches!(m.access, Access::Put);
    let value = a.value_f32();
    with(|e, w| {
        if is(m, "UserValue") {
            return Some(if put {
                w.ball_user_values.insert(id, a.value_kept()?);
                Ok(Ret::Empty)
            } else {
                Ok(w.ball_user_values.get(&id).cloned().map_or(Ret::Empty, Ret::Kept))
            });
        }
        // the ball's look (VPX's Ball properties), as it's drawn
        if matches!(m.name, "Color" | "Image" | "FrontDecal" | "DecalMode" | "BulbIntensityScale" | "PlayfieldReflectionScale" | "Visible" | "ReflectionEnabled" | "ForceReflection") {
            let table = w.ball_table_look.clone();
            let look = w.ball_looks.entry(id).or_insert(table);
            return Some(Ok(if put {
                match m.name {
                    "Color" => look.color = a.value_f32()? as i64 as u32,
                    "Image" => look.image = a.value_string()?,
                    "FrontDecal" => look.decal = a.value_string()?,
                    "DecalMode" => look.decal_mode = a.value_bool()?,
                    "BulbIntensityScale" => look.bulb_scale = value?,
                    "Visible" => look.visible = a.value_bool()?,
                    "ReflectionEnabled" => look.reflected = a.value_bool()?,
                    "ForceReflection" => look.force_reflection = a.value_bool()?,
                    _ => look.reflection = value?,
                }
                Ret::Empty
            } else {
                match m.name {
                    "Color" => Ret::Int(look.color as i32),
                    "Image" => Ret::Str(look.image.clone()),
                    "FrontDecal" => Ret::Str(look.decal.clone()),
                    "DecalMode" => Ret::Bool(look.decal_mode),
                    "BulbIntensityScale" => Ret::Float(look.bulb_scale),
                    "Visible" => Ret::Bool(look.visible),
                    "ReflectionEnabled" => Ret::Bool(look.reflected),
                    "ForceReflection" => Ret::Bool(look.force_reflection),
                    _ => Ret::Float(look.reflection),
                }
            }));
        }
        // a destroyed ball keeps answering with its last state, as VPX's ball objects do
        let b = match e.ball_mut(id) {
            Some(b) => b,
            None => w.gone_balls.get_mut(&id)?,
        };
        let field: Option<&mut f32> = match m.name {
            "X" => Some(&mut b.d.pos.x),
            "Y" => Some(&mut b.d.pos.y),
            "Z" => Some(&mut b.d.pos.z),
            "VelX" => Some(&mut b.d.vel.x),
            "VelY" => Some(&mut b.d.vel.y),
            "VelZ" => Some(&mut b.d.vel.z),
            "AngMomX" => Some(&mut b.angularmomentum.x),
            "AngMomY" => Some(&mut b.angularmomentum.y),
            "AngMomZ" => Some(&mut b.angularmomentum.z),
            "Radius" => Some(&mut b.d.radius),
            "Mass" => Some(&mut b.d.mass),
            _ => None,
        };
        if let Some(f) = field {
            return Some(if put {
                *f = value?;
                Ok(Ret::Empty)
            } else {
                Ok(Ret::Float(*f))
            });
        }
        Some(match m.name {
            "ID" => Ok(Ret::Int(id.0 as i32)),
            "AngVelX" | "AngVelY" | "AngVelZ" if !put => {
                // VPX: the angular momentum divided by the ball's inertia
                let inertia = b.inertia();
                let am = b.angularmomentum;
                let v = match m.name {
                    "AngVelX" => am.x,
                    "AngVelY" => am.y,
                    _ => am.z,
                };
                Ok(Ret::Float(v / inertia))
            }
            "Name" => Ok(Ret::Str(format!("Ball{}", id.0))),
            // VPX: any ball's DestroyBall destroys the active ball
            "DestroyBall" => {
                if let Some(active) = w.active_ball.take()
                    && let Some(gone) = e.remove_ball(active)
                {
                    w.gone_balls.insert(active, gone);
                }
                Ok(Ret::Int(1))
            }
            _ => return None,
        })
    })
    .flatten()
}

// ------------------------------------------------------------------ parts

fn part(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = matches!(m.access, Access::Put | Access::PutRef);
    // members every part has
    let common = with(|e, w| {
        let now = e.time_msec();
        let part = &mut w.parts[p as usize];
        let is_timer_part = matches!(w.vpx.gameitems[p as usize], GameItemEnum::Timer(_));
        let (enabled_name, interval_name) = if is_timer_part { ("Enabled", "Interval") } else { ("TimerEnabled", "TimerInterval") };
        Some(match m.name {
            "Name" if !put => Ok(Ret::Str(part.name.clone())),
            "UserValue" => {
                if put {
                    part.user_value = a.value_kept();
                    Ok(Ret::Empty)
                } else {
                    Ok(part.user_value.clone().map_or(Ret::Empty, Ret::Kept))
                }
            }
            n if n == enabled_name => {
                let t = part.timer?;
                if put {
                    w.timers.set_enabled(t, a.value_bool().unwrap_or(false), now);
                    Ok(Ret::Empty)
                } else {
                    Ok(Ret::Bool(w.timers.all[t].enabled))
                }
            }
            n if n == interval_name => {
                let t = part.timer?;
                if put {
                    w.timers.put_interval(t, a.value_i32().unwrap_or(0), now);
                    Ok(Ret::Empty)
                } else {
                    // as stored or set, unclamped, as VPX's `get_TimerInterval` reads it
                    Ok(Ret::Int(w.timers.all[t].stored))
                }
            }
            "Visible" => {
                if put {
                    part.visible = a.value_bool().unwrap_or(true);
                    Ok(Ret::Empty)
                } else {
                    Ok(Ret::Bool(part.visible))
                }
            }
            _ => return None,
        })
    })
    .flatten();
    if common.is_some() {
        return common;
    }
    if let Some(r) = look_member(p, m, a) {
        return Some(r);
    }
    if let Some(r) = kept_member(p, m, a) {
        return Some(r);
    }
    let item_is = |f: fn(&GameItemEnum) -> bool| with(|_, w| f(&w.vpx.gameitems[p as usize])).unwrap_or(false);
    if item_is(|g| matches!(g, GameItemEnum::Flasher(_)))
        && let Some(r) = flasher(p, m, a)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Flipper(_))) {
        return flipper(p, m, a);
    }
    // a kicker's, trigger's or bumper's own members, else its Enabled or Collidable below
    if item_is(|g| matches!(g, GameItemEnum::Kicker(_)))
        && let Some(r) = kicker(p, m, a)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Trigger(_)))
        && let Some(r) = trigger(p, m, a)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Bumper(_)))
        && let Some(r) = bumper(p, m, a)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Plunger(_))) {
        return plunger(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::Light(_))) {
        return light(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::Reel(_))) {
        return reel(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::TextBox(_))) {
        return text_box(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::LightSequencer(_))) {
        return light_seq(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::HitTarget(_))) {
        return hit_target(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::Primitive(_))) {
        return primitive(p, m, a);
    }
    if is(m, "Collidable") && item_is(|g| matches!(g, GameItemEnum::Gate(_))) {
        return gate_collidable(p, m, a);
    }
    if is(m, "Open") && item_is(|g| matches!(g, GameItemEnum::Gate(_))) {
        return gate_open(p, m, a);
    }
    if item_is(|g| matches!(g, GameItemEnum::Gate(_) | GameItemEnum::Spinner(_)))
        && let Some(r) = axle_angle(p, m)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Gate(_)))
        && let Some(r) = gate_motion(p, m, a)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Spinner(_)))
        && let Some(r) = spinner_motion(p, m, a)
    {
        return Some(r);
    }
    if item_is(|g| matches!(g, GameItemEnum::Ramp(_)))
        && let Some(r) = ramp_shape(p, m, a)
    {
        return Some(r);
    }
    let enables = (is(m, "Enabled") && item_is(|g| matches!(g, GameItemEnum::Kicker(_) | GameItemEnum::Trigger(_))))
        || (is(m, "Collidable") && item_is(|g| matches!(g, GameItemEnum::Wall(_) | GameItemEnum::Ramp(_) | GameItemEnum::Rubber(_) | GameItemEnum::Bumper(_))));
    if enables {
        return Some(if m.access == Access::Put {
            let on = a.value_bool().unwrap_or(true);
            // a dropped droppable wall stays off however it's set (VPX)
            let dropped_wall = with(|_, w| matches!(&w.vpx.gameitems[p as usize], GameItemEnum::Wall(wl) if wl.is_droppable) && w.parts[p as usize].is_dropped)
                .unwrap_or(false);
            set_colliders(p, on && !dropped_wall);
            with(|_, w| w.parts[p as usize].collidable = on);
            Ok(Ret::Empty)
        } else {
            Ok(Ret::Bool(with(|_, w| w.parts[p as usize].collidable).unwrap_or_else(|| colliders_on(p).unwrap_or(true))))
        });
    }
    if item_is(|g| matches!(g, GameItemEnum::Wall(_))) {
        return wall(p, m, a);
    }
    position(p, m)
}

/// Which look a member reads or sets.
#[derive(Clone, Copy)]
enum LookField {
    Image,
    /// A primitive's `NormalMap`.
    NormalMap,
    SideImage,
    Material,
    SideMaterial,
    DisableLighting,
    /// `DisableLighting`, the on-or-off form of `BlendDisableLighting`.
    DisableLightingOn,
    DisableLightingBelow,
    SideVisible,
    /// `ReflectionEnabled`: drawn into mirrors.
    Reflected,
    /// A primitive's `ReflectionProbe`: the mirror it shows.
    Mirror,
    /// A primitive's `RefractionProbe`: the refraction it shows.
    Refraction,
    /// A primitive's `DepthBias`: where it sorts among see-through parts, which VPX reads as it draws.
    DepthBias,
}

/// A part's images and materials (VPX's `put_Image`, `put_Material` and
/// their kin): kept to be read back, and drawn by the renderer. Getters
/// give back what was set, else the table file's value.
fn look_member(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    use GameItemEnum as G;
    let field = with(|_, w| {
        Some(match (&w.vpx.gameitems[p as usize], m.name) {
            // (a light's picture, VPX's put_Image, drawn as set: Apollo 13 swaps its playfield's)
            (G::Primitive(_) | G::Ramp(_) | G::Rubber(_) | G::HitTarget(_) | G::Flipper(_) | G::Spinner(_) | G::Plunger(_) | G::Wall(_) | G::Light(_), "Image") => LookField::Image,
            (G::Primitive(_), "DepthBias") => LookField::DepthBias,
            (G::Wall(_), "SideImage") => LookField::SideImage,
            (G::Primitive(_), "NormalMap") => LookField::NormalMap,
            (G::Wall(_), "TopMaterial") => LookField::Material,
            (G::Wall(_), "SideMaterial") | (G::Flipper(_), "RubberMaterial") => LookField::SideMaterial,
            (
                G::Primitive(_) | G::Ramp(_) | G::Rubber(_) | G::HitTarget(_) | G::Flipper(_) | G::Spinner(_) | G::Trigger(_) | G::Kicker(_) | G::Gate(_) | G::Plunger(_),
                "Material",
            ) => LookField::Material,
            (G::Primitive(_) | G::HitTarget(_) | G::Wall(_), "BlendDisableLighting") => LookField::DisableLighting,
            (G::Primitive(_) | G::HitTarget(_) | G::Wall(_), "DisableLighting") => LookField::DisableLightingOn,
            (G::Primitive(_) | G::HitTarget(_) | G::Wall(_), "BlendDisableLightingFromBelow") => LookField::DisableLightingBelow,
            (G::Wall(_), "SideVisible") => LookField::SideVisible,
            (
                G::Primitive(_) | G::Wall(_) | G::Ramp(_) | G::Rubber(_) | G::Flipper(_) | G::Bumper(_) | G::Gate(_) | G::Spinner(_) | G::Trigger(_) | G::Plunger(_) | G::HitTarget(_),
                "ReflectionEnabled",
            ) => LookField::Reflected,
            (G::Primitive(_), "ReflectionProbe") => LookField::Mirror,
            (G::Primitive(_), "RefractionProbe") => LookField::Refraction,
            _ => return None,
        })
    })
    .flatten()?;
    let put = matches!(m.access, Access::Put | Access::PutRef);
    with(|_, w| {
        if put {
            let name = || a.value_string().unwrap_or_default();
            match field {
                LookField::Image | LookField::SideImage => {
                    let n = name();
                    let index = Some(w.image_named(&n));
                    let look = &mut w.parts[p as usize].look;
                    if matches!(field, LookField::Image) {
                        (look.image, look.image_index) = (Some(n), index);
                    } else {
                        (look.side_image, look.side_image_index) = (Some(n), index);
                    }
                }
                LookField::NormalMap => {
                    let n = name();
                    let index = Some(w.image_named(&n));
                    let look = &mut w.parts[p as usize].look;
                    (look.normal_map, look.normal_map_index) = (Some(n), index);
                }
                LookField::Material | LookField::SideMaterial => {
                    let n = name();
                    let index = Some(w.materials.find(&n));
                    let look = &mut w.parts[p as usize].look;
                    if matches!(field, LookField::Material) {
                        (look.material, look.material_index) = (Some(n), index);
                    } else {
                        (look.side_material, look.side_material_index) = (Some(n), index);
                    }
                }
                LookField::DisableLighting => w.parts[p as usize].look.disable_lighting = Some(a.value_f32().unwrap_or(0.0)),
                LookField::DisableLightingOn => w.parts[p as usize].look.disable_lighting = Some(if a.value_bool().unwrap_or(false) { 1.0 } else { 0.0 }),
                LookField::DisableLightingBelow => w.parts[p as usize].look.disable_lighting_below = Some(a.value_f32().unwrap_or(0.0)),
                LookField::SideVisible => w.parts[p as usize].look.side_visible = Some(a.value_bool().unwrap_or(true)),
                LookField::Reflected => w.parts[p as usize].look.reflected = Some(a.value_bool().unwrap_or(true)),
                LookField::Mirror => {
                    let n = name();
                    let index = Some(w.mirror_named(&n));
                    let look = &mut w.parts[p as usize].look;
                    (look.mirror, look.mirror_index) = (Some(n), index);
                }
                LookField::Refraction => {
                    let n = name();
                    let index = Some(w.refraction_named(&n));
                    let look = &mut w.parts[p as usize].look;
                    (look.refraction, look.refraction_index) = (Some(n), index);
                }
                LookField::DepthBias => w.parts[p as usize].look.depth_bias = Some(a.value_f32().unwrap_or(0.0)),
            }
            return Ok(Ret::Empty);
        }
        let look = &w.parts[p as usize].look;
        let set = match field {
            LookField::Image => look.image.clone().map(Ret::Str),
            LookField::NormalMap => look.normal_map.clone().map(Ret::Str),
            LookField::SideImage => look.side_image.clone().map(Ret::Str),
            LookField::Material => look.material.clone().map(Ret::Str),
            LookField::SideMaterial => look.side_material.clone().map(Ret::Str),
            LookField::DisableLighting => look.disable_lighting.map(Ret::Float),
            LookField::DisableLightingOn => look.disable_lighting.map(|d| Ret::Bool(d != 0.0)),
            LookField::DisableLightingBelow => look.disable_lighting_below.map(Ret::Float),
            LookField::SideVisible => look.side_visible.map(Ret::Bool),
            LookField::Reflected => look.reflected.map(Ret::Bool),
            LookField::Mirror => look.mirror.clone().map(Ret::Str),
            LookField::Refraction => look.refraction.clone().map(Ret::Str),
            LookField::DepthBias => look.depth_bias.map(Ret::Float),
        };
        Ok(set.or_else(|| match field {
            LookField::DisableLightingOn => match initial_value(w, Target::Part(p), "BlendDisableLighting") {
                Some(Ret::Float(d)) => Some(Ret::Bool(d != 0.0)),
                _ => None,
            },
            _ => initial_value(w, Target::Part(p), m.name),
        })
        .unwrap_or(Ret::Empty))
    })
}

/// Members VPX only keeps while it plays, changing nothing it draws or plays from then on: what a
/// script sets is given back, else the table file's value. A wall's `HasHitEvent` (its colliders
/// keep the one they were made with, as VPX's); a hit target's `Orientation` (VPX turns its mesh to
/// it only while the target is hit, which ours doesn't yet); a flasher's `VideoCapWidth` and
/// `VideoCapHeight`, and its `VideoCapUpdate`, which copies another program's window by its title
/// into the flasher — the window Blood Machines names is Scorbit's QR code program's, not ours, so
/// none is found, as in VPX without that program.
fn kept_member(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let kept = with(|_, w| {
        matches!(
            (&w.vpx.gameitems[p as usize], m.name),
            (GameItemEnum::Wall(_), "HasHitEvent")
                | (GameItemEnum::HitTarget(_), "Orientation")
                | (GameItemEnum::Flasher(_), "VideoCapWidth" | "VideoCapHeight" | "VideoCapUpdate")
                // read by Tales of the Arabian Nights to tell VPX's versions apart; one a script
                // sets is kept but drawn as the table loaded it, where VPX's next frame would
                // read the normal map the other way
                | (GameItemEnum::Primitive(_), "ObjectSpaceNormalMap")
        )
    })?;
    if !kept {
        return None;
    }
    let key = (owner_key(Target::Part(p)), m.dispid);
    with(|_, w| match m.access {
        Access::Put | Access::PutRef => {
            if let Some(v) = a.value_kept() {
                w.kept.insert(key, v);
            }
            Ok(Ret::Empty)
        }
        Access::Get => Ok(match w.kept.get(&key) {
            Some(v) => Ret::Kept(v.clone()),
            None => initial_value(w, Target::Part(p), m.name).unwrap_or(Ret::Empty),
        }),
        Access::Method => Ok(Ret::Empty),
    })
}

/// A flasher's place, colour and pictures (VPX's flasher setters).
fn flasher(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|_, w| {
        // a picture by name, found before the flasher is borrowed
        let image = if put && matches!(m.name, "ImageA" | "ImageB") {
            let n = a.value_string().unwrap_or_default();
            Some((w.image_named(&n), n))
        } else {
            None
        };
        let f = w.parts[p as usize].flasher.as_mut()?;
        let float = |v: &mut f32| {
            if put {
                *v = a.value_f32().unwrap_or(*v);
                Ret::Empty
            } else {
                Ret::Float(*v)
            }
        };
        Some(Ok(match m.name {
            // VPX's MoveOffset: the outline and its centre move by the difference
            "X" => float(&mut f.center[0]),
            "Y" => float(&mut f.center[1]),
            "Height" => float(&mut f.height),
            "RotX" => float(&mut f.rot[0]),
            "RotY" => float(&mut f.rot[1]),
            "RotZ" => float(&mut f.rot[2]),
            "IntensityScale" => float(&mut f.intensity_scale),
            "ModulateVsAdd" => float(&mut f.modulate_vs_add),
            "DepthBias" => float(&mut f.depth_bias),
            "Color" if put => {
                f.color = a.value_i32().unwrap_or(f.color as i32) as u32;
                Ret::Empty
            }
            "Color" => Ret::Int(f.color as i32),
            // SetAlpha and SetFilterAmount: never below 0
            "Opacity" if put => {
                f.alpha = a.value_i32().unwrap_or(f.alpha).max(0);
                Ret::Empty
            }
            "Opacity" => Ret::Int(f.alpha),
            "Amount" if put => {
                f.amount = a.value_i32().unwrap_or(f.amount).max(0);
                Ret::Empty
            }
            "Amount" => Ret::Int(f.amount),
            "AddBlend" if put => {
                f.add_blend = a.value_bool().unwrap_or(f.add_blend);
                Ret::Empty
            }
            "AddBlend" => Ret::Bool(f.add_blend),
            "DisplayTexture" if put => {
                f.display_texture = a.value_bool().unwrap_or(f.display_texture);
                Ret::Empty
            }
            "DisplayTexture" => Ret::Bool(f.display_texture),
            "Filter" if put => {
                f.set_filter(&a.value_string().unwrap_or_default());
                Ret::Empty
            }
            "Filter" => Ret::Str(f.filter_name().to_string()),
            "ImageA" | "ImageB" if put => {
                let (index, name) = image?;
                if m.name == "ImageA" {
                    (f.image_a_index, f.image_a) = (index, name);
                } else {
                    (f.image_b_index, f.image_b) = (index, name);
                }
                Ret::Empty
            }
            "ImageA" => Ret::Str(f.image_a.clone()),
            "ImageB" => Ret::Str(f.image_b.clone()),
            _ => return None,
        }))
    })
    .flatten()
}

/// `X` and `Y` of parts placed by a center point.
fn position(p: u32, m: &'static Member) -> Option<Res> {
    if m.access != Access::Get || !(is(m, "X") || is(m, "Y")) {
        return None;
    }
    with(|_, w| {
        let at = match &w.vpx.gameitems[p as usize] {
            GameItemEnum::Flipper(f) => (f.center.x, f.center.y),
            GameItemEnum::Bumper(b) => (b.center.x, b.center.y),
            GameItemEnum::Light(l) => (l.center.x, l.center.y),
            GameItemEnum::Kicker(k) => (k.center.x, k.center.y),
            GameItemEnum::Gate(g) => (g.center.x, g.center.y),
            GameItemEnum::Spinner(s) => (s.center.x, s.center.y),
            GameItemEnum::Trigger(t) => (t.center.x, t.center.y),
            GameItemEnum::Plunger(pl) => (pl.center.x, pl.center.y),
            GameItemEnum::Timer(t) => (t.center.x, t.center.y),
            GameItemEnum::Decal(d) => (d.center.x, d.center.y),
            GameItemEnum::HitTarget(t) => (t.position.x, t.position.y),
            GameItemEnum::Flasher(f) => (f.pos_x.unwrap_or(0.0), f.pos_y.unwrap_or(0.0)),
            _ => return None,
        };
        Some(Ok(Ret::Float(if is(m, "X") { at.0 } else { at.1 })))
    })
    .flatten()
}

fn flipper(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    // its pivot (VPX's m_d.m_Center), which flipper scripts measure from: VPX keeps the place a
    // script sets and draws the flipper there, its physics staying where play began; ours keeps
    // the place only, for now
    if m.name == "X" || m.name == "Y" {
        let key = (owner_key(Target::Part(p)), m.dispid);
        // kept as VPX keeps it, a float
        let now = |w: &World| match w.kept.get(&key).and_then(Kept::number) {
            Some(v) => v as f32,
            None => match initial_value(w, Target::Part(p), m.name) {
                Some(Ret::Float(v)) => v,
                _ => 0.0,
            },
        };
        return with(|_, w| match m.access {
            Access::Get => Ok(Ret::Float(now(w))),
            _ => {
                let before = now(w);
                if let Some(v) = a.value_kept() {
                    let moved = v.number().is_some_and(|after| after as f32 != before);
                    w.kept.insert(key, v);
                    // (Indiana Jones writes its flippers' place every frame, unchanged)
                    if moved {
                        log::once("a flipper its script moves (X, Y) is drawn where play began, for now");
                    }
                }
                Ok(Ret::Empty)
            }
        });
    }
    // VPX keeps the length a script sets, and does nothing more with it: the
    // flipper's shape and its physics were made before the script started
    if m.name == "Length" {
        return with(|_, w| match m.access {
            Access::Get => Ok(w.lengths.get(&p).map_or_else(|| initial_value(w, Target::Part(p), m.name).unwrap_or(Ret::Empty), |&v| Ret::Float(v))),
            _ => {
                w.lengths.insert(p, a.value_f32().ok_or(E_FAIL)?);
                Ok(Ret::Empty)
            }
        });
    }
    with(|e, w| {
        let k = w.parts[p as usize].collider?;
        let collider = e.collider_mut(k);
        let base = &mut collider.base;
        let Shape::Flipper(f) = &mut collider.shape else { return None };
        let put = m.access == Access::Put;
        let value = a.value_f32();
        let d = &mut f.mover.data;
        // VPX ignores a script's change while a physics set governs the flipper
        let free = !d.use_override;
        let done = Ok(Ret::Empty);
        Some(match (m.name, m.access) {
            ("Strength", Access::Get) => Ok(Ret::Float(f.mover.get_strength())),
            ("Strength", _) if put => {
                if free {
                    d.strength = value?;
                }
                done
            }
            ("Return", Access::Get) => Ok(Ret::Float(f.mover.get_return_ratio())),
            ("Return", _) if put => {
                if free {
                    d.return_strength = value?.clamp(0.0, 1.0);
                }
                done
            }
            ("RampUp", Access::Get) => Ok(Ret::Float(if d.use_override { d.override_coil_ramp_up } else { d.ramp_up })),
            ("RampUp", _) if put => {
                if free {
                    d.ramp_up = value?;
                }
                done
            }
            ("EOSTorque", Access::Get) => Ok(Ret::Float(if d.use_override { d.override_torque_damping } else { d.torque_damping })),
            ("EOSTorque", _) if put => {
                if free {
                    d.torque_damping = value?;
                }
                done
            }
            ("EOSTorqueAngle", Access::Get) => Ok(Ret::Float(if d.use_override { d.override_torque_damping_angle } else { d.torque_damping_angle })),
            ("EOSTorqueAngle", _) if put => {
                if free {
                    d.torque_damping_angle = value?;
                }
                done
            }
            ("Mass", Access::Get) => Ok(Ret::Float(f.mover.get_mass())),
            ("Mass", _) if put => {
                if free {
                    f.mover.set_mass(value?);
                }
                done
            }
            ("Elasticity", Access::Get) => Ok(Ret::Float(base.elasticity)),
            ("Elasticity", _) if put => {
                if free {
                    base.elasticity = value?;
                }
                done
            }
            ("ElasticityFalloff", Access::Get) => Ok(Ret::Float(base.elasticity_falloff)),
            ("ElasticityFalloff", _) if put => {
                if free {
                    base.elasticity_falloff = value?;
                }
                done
            }
            ("Scatter", Access::Get) => Ok(Ret::Float(base.scatter.to_degrees())),
            ("Scatter", _) if put => {
                if free {
                    base.scatter = value?.to_radians();
                }
                done
            }
            ("Friction", Access::Get) => Ok(Ret::Float(base.friction)),
            ("Friction", _) if put => {
                base.friction = value?;
                done
            }
            ("RotateToEnd", _) => {
                f.rotate_to_end();
                Ok(Ret::Empty)
            }
            ("RotateToStart", _) => {
                f.rotate_to_start();
                Ok(Ret::Empty)
            }
            // VPX's switch for whether balls hit it (the flipper stays drawn): Bride of Pin-bot's
            // diverter is a flipper switched so
            ("Enabled", Access::Get) => Ok(Ret::Bool(f.mover.enabled)),
            ("Enabled", _) if put => {
                f.mover.enabled = a.value_bool().unwrap_or(f.mover.enabled);
                done
            }
            ("CurrentAngle", Access::Get) => Ok(Ret::Float(f.mover.angle_cur.to_degrees())),
            ("StartAngle", Access::Get) => Ok(Ret::Float(f.mover.angle_start.to_degrees())),
            ("EndAngle", Access::Get) => Ok(Ret::Float(f.mover.angle_end.to_degrees())),
            ("StartAngle", Access::Put) => {
                f.mover.set_start_angle(a.value_f32()?.to_radians());
                Ok(Ret::Empty)
            }
            ("EndAngle", Access::Put) => {
                f.mover.set_end_angle(a.value_f32()?.to_radians());
                Ok(Ret::Empty)
            }
            _ => return None,
        })
    })
    .flatten()
    .or_else(|| position(p, m))
}

fn kicker(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let scatter = with(|_, w| match &w.vpx.gameitems[p as usize] {
        GameItemEnum::Kicker(k) => k.scatter,
        _ => 0.0,
    })
    .unwrap_or(0.0);
    let r = with(|e, w| {
        let k = w.parts[p as usize].collider?;
        Some(match m.name {
            "CreateBall" | "CreateSizedBall" | "CreateSizedBallWithMass" => {
                let radius = if m.name == "CreateBall" { DEFAULT_BALL_SIZE } else { a.f32(0).unwrap_or(DEFAULT_BALL_SIZE) };
                let mass = if m.name == "CreateSizedBallWithMass" { a.f32(1).unwrap_or(1.0) } else { 1.0 };
                let id = BallId(w.next_ball);
                w.next_ball += 1;
                // the new ball fires no event (VPX's newBall)
                e.kicker_create_ball(k, id, radius, mass, &mut physics_compat::engine::NoScript);
                Ok(Ret::Object(Obj::dispatch(w.ball_object(id))))
            }
            "DestroyBall" => Ok(Ret::Int(match e.kicker_destroy_ball(k) {
                Some(gone) => {
                    // the active ball, if destroyed, passes to the first ball left
                    if w.active_ball == Some(gone.id) {
                        w.active_ball = e.balls().first().map(|b| b.id);
                    }
                    w.gone_balls.insert(gone.id, gone);
                    1
                }
                None => 0,
            })),
            // the balls inside the kicker; the last one counted becomes the active ball
            "BallCntOver" => {
                let inside: Vec<BallId> = e.balls().iter().filter(|b| b.d.is_inside(Some(physics_compat::env::PartId(p)))).map(|b| b.id).collect();
                if let Some(&last) = inside.last() {
                    w.active_ball = Some(last);
                }
                Ok(Ret::Int(inside.len() as i32))
            }
            // VPX's `get_LastCapturedBall`: the last ball the kicker caught, while it's on the table;
            // none caught yet, or that ball destroyed, an error (VPX also shows one)
            "LastCapturedBall" => {
                let Shape::Kicker(kc) = &e.colliders()[k as usize].shape else { return None };
                match kc.last_captured_ball {
                    Some(id) if e.balls().iter().any(|b| b.id == id) => Ok(Ret::Object(Obj::dispatch(w.ball_object(id)))),
                    caught => {
                        log::once(if caught.is_some() {
                            "LastCapturedBall was called but ball is already destroyed!"
                        } else {
                            "LastCapturedBall was called but no ball was captured!"
                        });
                        Err(E_POINTER)
                    }
                }
            }
            "Kick" | "KickXYZ" | "KickZ" => {
                let (angle, speed, inclination) = (a.f32(0).unwrap_or(0.0), a.f32(1).unwrap_or(0.0), a.f32(2).unwrap_or(0.0));
                let offset = match m.name {
                    "KickXYZ" => Vertex3Ds::new(a.f32(3).unwrap_or(0.0), a.f32(4).unwrap_or(0.0), a.f32(5).unwrap_or(0.0)),
                    "KickZ" => Vertex3Ds::new(0.0, 0.0, a.f32(3).unwrap_or(0.0)),
                    _ => Vertex3Ds::default(),
                };
                e.kicker_kick(k, angle, speed, inclination, offset, scatter);
                Ok(Ret::Empty)
            }
            // VPX's `FallThrough`: whether a ball falls through instead of being held, read at the
            // next hit (the hit circle keeps the size the table's setting gave it at start)
            "FallThrough" => {
                if m.access == Access::Put {
                    let on = a.value_bool().unwrap_or(false);
                    if let Shape::Kicker(kc) = &mut e.collider_mut(k).shape {
                        kc.data.fall_through = on;
                    }
                    Ok(Ret::Empty)
                } else {
                    let Shape::Kicker(kc) = &e.colliders()[k as usize].shape else { return None };
                    Ok(Ret::Bool(kc.data.fall_through))
                }
            }
            _ => return None,
        })
    })
    .flatten();
    r.or_else(|| position(p, m))
}

/// A ramp's `HeightBottom`, `HeightTop`, `WidthBottom` and `WidthTop` (VPX's `Ramp::put_HeightBottom`
/// and the rest): its shape rebuilt at the new heights and widths, as VPX regenerates its mesh; a
/// solid ramp, which VPX draws once at start with the still parts, keeps showing its old shape
/// until they're drawn again, and its colliders stay.
fn ramp_shape(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let k = match m.name {
        "HeightBottom" => 0,
        "HeightTop" => 1,
        "WidthBottom" => 2,
        "WidthTop" => 3,
        _ => return None,
    };
    with(|_, w| {
        let GameItemEnum::Ramp(r) = &w.vpx.gameitems[p as usize] else { return None };
        let now = w.parts[p as usize].ramp_shape.unwrap_or([r.height_bottom, r.height_top, r.width_bottom, r.width_top]);
        Some(if m.access == Access::Put {
            let mut shape = now;
            shape[k] = a.value_f32().unwrap_or(shape[k]);
            if shape != now {
                w.parts[p as usize].ramp_shape = Some(shape);
            }
            Ok(Ret::Empty)
        } else {
            Ok(Ret::Float(now[k]))
        })
    })
    .flatten()
}

/// `CurrentAngle` of gates and spinners, in degrees.
fn axle_angle(p: u32, m: &'static Member) -> Option<Res> {
    if !is(m, "CurrentAngle") || m.access != Access::Get {
        return None;
    }
    with(|e, w| {
        let k = w.parts[p as usize].collider?;
        Some(Ok(Ret::Float(match &e.colliders()[k as usize].shape {
            Shape::Gate(g) => g.mover.angle.to_degrees(),
            Shape::Spinner(s) => s.mover.angle.to_degrees(),
            _ => return None,
        })))
    })
    .flatten()
}

/// A gate's `Collidable` (VPX's `Gate::put_Collidable`): the flap, and a one-way gate's blocking
/// segment, switched on or off, not its bracket's posts. Switched on, it closes all the way again
/// (its close angle 0); off, it swings back to the table's close angle.
fn gate_collidable(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    with(|e, w| {
        let GameItemEnum::Gate(g) = &w.vpx.gameitems[p as usize] else { return Ok(Ret::Empty) };
        // the table's angles, an inversion corrected (as VPX's PhysicSetup leaves them)
        let (two_way, closed, open) = (g.two_way, g.angle_min.min(g.angle_max), g.angle_min.max(g.angle_max));
        let colliders = w.part_colliders.get(&p).cloned().unwrap_or_default();
        let flap = colliders.iter().copied().find(|&k| matches!(e.colliders()[k as usize].shape, Shape::Gate(_)));
        if m.access != Access::Put {
            return Ok(Ret::Bool(flap.map_or(g.is_collidable, |k| e.colliders()[k as usize].base.enabled)));
        }
        let on = a.value_bool().unwrap_or(true);
        let Some(k) = flap else { return Ok(Ret::Empty) };
        let c = e.collider_mut(k);
        c.base.enabled = on;
        if let Shape::Gate(hg) = &mut c.shape {
            hg.mover.angle_max = open;
            hg.mover.angle_min = if on { 0.0 } else { closed };
        }
        if !two_way && let Some(&seg) = colliders.iter().find(|&&k| matches!(e.colliders()[k as usize].shape, Shape::LineSeg(_))) {
            e.collider_mut(seg).base.enabled = on;
        }
        Ok(Ret::Empty)
    })
}

/// A gate's `Open` (VPX build 5436's `Gate::put_Open` and `get_Open`): the script swings the flap
/// open or shut at a set speed (0.2), between the table's angles, without its bounce at the ends
/// (a forced move). Held open, the flap and a one-way gate's blocking segment stop colliding; shut,
/// they collide again as the table set them (its `Collidable`, which the script's own setting of it
/// doesn't change, as in VPX). The angles are the table's: VPX's would follow the script's
/// `AngleMin` and `AngleMax`, which the player doesn't model yet.
fn gate_open(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    with(|e, w| {
        let GameItemEnum::Gate(g) = &w.vpx.gameitems[p as usize] else { return Ok(Ret::Empty) };
        // the table's angles, an inversion corrected (as VPX's PhysicSetup leaves them)
        let (two_way, collidable, closed, open) = (g.two_way, g.is_collidable, g.angle_min.min(g.angle_max), g.angle_min.max(g.angle_max));
        let colliders = w.part_colliders.get(&p).cloned().unwrap_or_default();
        let flap = colliders.iter().copied().find(|&k| matches!(e.colliders()[k as usize].shape, Shape::Gate(_)));
        if m.access != Access::Put {
            let held = flap.is_some_and(|k| matches!(&e.colliders()[k as usize].shape, Shape::Gate(hg) if hg.mover.open));
            return Ok(Ret::Bool(held));
        }
        let Some(k) = flap else { return Ok(Ret::Empty) };
        let hold = a.value_bool().unwrap_or(false);
        let c = e.collider_mut(k);
        c.base.enabled = !hold && collidable;
        if let Shape::Gate(hg) = &mut c.shape {
            let mv = &mut hg.mover;
            mv.hit_direction = false;
            mv.angle_max = open;
            mv.angle_min = closed;
            mv.forced_move = true;
            mv.open = hold;
            if hold && mv.angle < mv.angle_max {
                mv.anglespeed = 0.2;
            } else if !hold && mv.angle > mv.angle_min {
                mv.anglespeed = -0.2;
            }
        }
        if !two_way && let Some(&seg) = colliders.iter().find(|&&k| matches!(e.colliders()[k as usize].shape, Shape::LineSeg(_))) {
            e.collider_mut(seg).base.enabled = !hold && collidable;
        }
        Ok(Ret::Empty)
    })
}

/// A gate's members its physics reads while playing (VPX build 5436's `Gate`): `Damping` and
/// `GravityFactor` (the flap's swing), `TwoWay` (as its collision reads it; the getter still gives
/// the table's, as VPX's does), `OpenAngle` and `CloseAngle` (the swing's limits, held to the
/// table's), and `Move`: the script swinging the flap to a direction or angle at a speed, its
/// collision off, as `Open` does.
fn gate_motion(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|e, w| {
        let GameItemEnum::Gate(g) = &w.vpx.gameitems[p as usize] else { return None };
        // the table's angles in radians, an inversion corrected (VPX's `m_d` while playing)
        let (two_way, collidable, d_min, d_max) = (g.two_way, g.is_collidable, g.angle_min.min(g.angle_max), g.angle_min.max(g.angle_max));
        let colliders = w.part_colliders.get(&p).cloned().unwrap_or_default();
        let flap = colliders.iter().copied().find(|&k| matches!(e.colliders()[k as usize].shape, Shape::Gate(_)))?;
        let seg = colliders.iter().copied().find(|&k| matches!(e.colliders()[k as usize].shape, Shape::LineSeg(_)));
        let c = e.collider_mut(flap);
        let Shape::Gate(hg) = &mut c.shape else { return None };
        let mv = &mut hg.mover;
        let phys = physics_compat::physconst::PHYS_FACTOR as f32;
        Some(match m.name {
            "Damping" if put => {
                mv.damping = clamp01(a.value_f32().unwrap_or(0.0)).powf(phys);
                Ok(Ret::Empty)
            }
            "Damping" => Ok(Ret::Float(mv.damping.powf((1.0 / physics_compat::physconst::PHYS_FACTOR) as f32))),
            "GravityFactor" if put => {
                mv.gravityfactor = a.value_f32().unwrap_or(0.0).clamp(0.0, 100.0);
                Ok(Ret::Empty)
            }
            "GravityFactor" => Ok(Ret::Float(mv.gravityfactor)),
            "TwoWay" if put => {
                hg.two_way = a.value_bool().unwrap_or(false);
                Ok(Ret::Empty)
            }
            "TwoWay" => Ok(Ret::Bool(two_way)),
            "OpenAngle" | "CloseAngle" if put => {
                let open = m.name == "OpenAngle";
                // VPX: a collidable gate's angles can't change (it shows an error)
                let angle = if collidable {
                    log::once(&format!("Gate is collidable! {} angles other than {} aren't possible!", if open { "open" } else { "closing" }, if open { 90 } else { 0 }));
                    if open { std::f32::consts::FRAC_PI_2 } else { 0.0 }
                } else {
                    a.value_f32().unwrap_or(0.0).to_radians()
                };
                let v = if angle > d_max { d_max } else if angle < d_min { d_min } else { angle };
                if open {
                    if mv.angle_min < v { mv.angle_max = v } else { mv.angle_min = v }
                } else if mv.angle_max > v {
                    mv.angle_min = v
                } else {
                    mv.angle_max = v
                }
                Ok(Ret::Empty)
            }
            "OpenAngle" => Ok(Ret::Float(mv.angle_max.to_degrees())),
            "CloseAngle" => Ok(Ret::Float(mv.angle_min.to_degrees())),
            "Move" => {
                let (mut dir, speed, mut angle) = (a.i32(0).unwrap_or(0), a.f32(1).unwrap_or(0.0), a.f32(2).unwrap_or(0.0));
                mv.hit_direction = false;
                mv.forced_move = true;
                // a move turns its natural swing off, and its collision
                mv.open = true;
                c.base.enabled = false;
                let speed = if speed <= 0.0 { 0.2 } else { speed * (std::f64::consts::PI / 180.0) as f32 };
                let Shape::Gate(hg) = &mut c.shape else { return None };
                let mv = &mut hg.mover;
                if dir == 0 || angle != 0.0 {
                    angle = (angle * (std::f64::consts::PI / 180.0) as f32).clamp(d_min, d_max);
                    let da = angle - mv.angle;
                    if da > 1.0e-5 {
                        dir = 1;
                    } else if da < -1.0e-5 {
                        dir = -1;
                    } else {
                        dir = 0;
                        mv.anglespeed = 0.0;
                    }
                } else {
                    angle = if dir < 0 { d_min } else { d_max };
                }
                if dir > 0 {
                    mv.angle_max = angle;
                    if mv.angle < mv.angle_max {
                        mv.anglespeed = speed;
                    }
                } else if dir < 0 {
                    mv.angle_min = angle;
                    if mv.angle > mv.angle_min {
                        mv.anglespeed = -speed;
                    }
                }
                if !two_way && let Some(seg) = seg {
                    e.collider_mut(seg).base.enabled = false;
                }
                Ok(Ret::Empty)
            }
            _ => return None,
        })
    })
    .flatten()
}

/// A spinner's members its physics reads while playing (VPX build 5436's `Spinner`): `Damping`,
/// `Elasticity`, and in its limited-angle mode `AngleMax` (a free spinner refuses it), `AngleMin`
/// as VPX 5436 has it (refused in that mode, nothing in the other: its check is the other way
/// round).
fn spinner_motion(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|e, w| {
        let GameItemEnum::Spinner(s) = &w.vpx.gameitems[p as usize] else { return None };
        // the table's limits, in degrees (VPX's `m_d` while playing)
        let (d_min, d_max) = (s.angle_min, s.angle_max);
        let k = w.parts[p as usize].collider?;
        let Shape::Spinner(hs) = &mut e.collider_mut(k).shape else { return None };
        let mv = &mut hs.mover;
        let limited = d_min != d_max;
        let held = |v: f32| (if v > d_max { d_max } else if v < d_min { d_min } else { v }).to_radians();
        Some(match m.name {
            "Damping" if put => {
                mv.damping = clamp01(a.value_f32().unwrap_or(0.0)).powf(physics_compat::physconst::PHYS_FACTOR as f32);
                Ok(Ret::Empty)
            }
            "Damping" => Ok(Ret::Float(mv.damping.powf((1.0 / physics_compat::physconst::PHYS_FACTOR) as f32))),
            "Elasticity" if put => {
                mv.elasticity = a.value_f32().unwrap_or(0.0);
                Ok(Ret::Empty)
            }
            "Elasticity" => Ok(Ret::Float(mv.elasticity)),
            "AngleMax" if put => {
                if !limited {
                    return Some(Err(E_FAIL));
                }
                let v = held(a.value_f32().unwrap_or(0.0));
                if mv.angle_min < v { mv.angle_max = v } else { mv.angle_min = v }
                Ok(Ret::Empty)
            }
            "AngleMax" => Ok(Ret::Float(mv.angle_max.to_degrees())),
            "AngleMin" if put => {
                if limited {
                    return Some(Err(E_FAIL));
                }
                Ok(Ret::Empty)
            }
            "AngleMin" => Ok(Ret::Float(mv.angle_min.to_degrees())),
            _ => return None,
        })
    })
    .flatten()
}

/// VPX's `saturate`.
fn clamp01(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// Switches all of a part's colliders on or off (VPX's `m_enabled` on each
/// hit object: a kicker's or trigger's `Enabled`, a wall's, ramp's or
/// rubber's `Collidable`).
fn set_colliders(p: u32, on: bool) {
    with(|e, w| {
        for &k in w.part_colliders.get(&p).map_or(&[][..], Vec::as_slice) {
            e.collider_mut(k).base.enabled = on;
        }
    });
}

/// Whether the part's colliders are on (they switch together).
fn colliders_on(p: u32) -> Option<bool> {
    with(|e, w| w.part_colliders.get(&p).and_then(|c| c.first()).map(|&k| e.colliders()[k as usize].base.enabled)).flatten()
}

fn plunger(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|e, w| {
        let k = w.parts[p as usize].collider?;
        // the pull speed PullBack reads when called (VPX's `m_d.m_speedPull`), the script's if it set one
        let file_pull_speed = match &w.vpx.gameitems[p as usize] {
            GameItemEnum::Plunger(pl) => pl.speed_pull,
            _ => 0.0,
        };
        let pull_speed = *w.plunger_pull_speeds.get(&p).unwrap_or(&file_pull_speed);
        let retract = w.plunger_retract;
        if m.name == "PullSpeed" {
            return Some(if put {
                w.plunger_pull_speeds.insert(p, a.value_f32().unwrap_or(0.0));
                Ok(Ret::Empty)
            } else {
                Ok(Ret::Float(pull_speed))
            });
        }
        let Shape::Plunger(pl) = &mut e.collider_mut(k).shape else { return None };
        Some(match m.name {
            // VPX's `MechPlunger`, `MechStrength` and `MomentumXfer` (build 5436): settings its
            // physics reads each step, or as the plunger hits the ball, changed as play goes
            "MechPlunger" if put => {
                pl.mover.data.mech_plunger = a.value_bool().unwrap_or(false);
                Ok(Ret::Empty)
            }
            "MechPlunger" => Ok(Ret::Bool(pl.mover.data.mech_plunger)),
            "MechStrength" if put => {
                pl.mover.data.mech_strength = a.value_f32().unwrap_or(0.0);
                Ok(Ret::Empty)
            }
            "MechStrength" => Ok(Ret::Float(pl.mover.data.mech_strength)),
            "MomentumXfer" if put => {
                pl.mover.data.momentum_xfer = a.value_f32().unwrap_or(0.0);
                Ok(Ret::Empty)
            }
            "MomentumXfer" => Ok(Ret::Float(pl.mover.data.momentum_xfer)),
            // deprecated in VPX: always its generic layout
            "MotionDevice" => Ok(Ret::Int(6)),
            "PullBack" => {
                // VPX's `PullBack`: with the player's one-second retract on, `PullBackandRetract`
                if retract {
                    pl.mover.pull_back_and_retract(pull_speed);
                } else {
                    pl.mover.pull_back(pull_speed);
                }
                Ok(Ret::Empty)
            }
            "PullBackandRetract" => {
                pl.mover.pull_back_and_retract(pull_speed);
                Ok(Ret::Empty)
            }
            "Fire" => {
                // an auto plunger fires from fully back, as a launch button does
                if pl.mover.data.auto_plunger { pl.mover.fire_from(1.0) } else { pl.mover.fire() }
                Ok(Ret::Empty)
            }
            // VPX's `AutoPlunger` (build 5436, `Plunger::put_AutoPlunger`): the setting its physics
            // reads each step and `Fire` follows, changed as play goes
            "AutoPlunger" if m.access == Access::Put => {
                pl.mover.data.auto_plunger = a.value_bool().unwrap_or(false);
                Ok(Ret::Empty)
            }
            "AutoPlunger" => Ok(Ret::Bool(pl.mover.data.auto_plunger)),
            // VPX's `FireSpeed` (build 5436, `Plunger::put_FireSpeed`): the release speed its
            // physics reads at each release (Star Gazer varies it before every launch)
            "FireSpeed" if m.access == Access::Put => {
                pl.mover.data.speed_fire = a.value_f32().unwrap_or(0.0);
                Ok(Ret::Empty)
            }
            "FireSpeed" => Ok(Ret::Float(pl.mover.data.speed_fire)),
            // how far back it is, 0 to 25 (without a mechanical plunger)
            "Position" => {
                let m = &pl.mover;
                let frame = (m.pos - m.frame_start) / (m.frame_end - m.frame_start);
                Ok(Ret::Float(25.0 - frame.clamp(0.0, 1.0) * 25.0))
            }
            "CreateBall" => {
                // VPX places the new ball on the tip, assuming a ball radius of 25
                let x = (pl.mover.x + pl.mover.x2) * 0.5;
                let y = pl.mover.pos - (25.0 + 0.01);
                let z = pl.mover.part_hit.hit_bbox.zlow; // the plunger's surface height
                let id = BallId(w.next_ball);
                w.next_ball += 1;
                let mut ball = HitBall::new(id);
                ball.d.pos = Vertex3Ds::new(x, y, z + DEFAULT_BALL_SIZE);
                ball.d.vel = Vertex3Ds::new(0.1, 0.0, 0.0);
                e.add_ball(ball);
                Ok(Ret::Object(Obj::dispatch(w.ball_object(id))))
            }
            _ => return None,
        })
    })
    .flatten()
    .or_else(|| position(p, m))
}

/// VPX's trigger members its animation answers (src/parts/trigger.cpp).
fn trigger(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    // the balls inside the trigger (VPX's Trigger::BallCntOver); each counted becomes the active
    // ball in turn, so the last one counted is
    if m.name == "BallCntOver" {
        return with(|e, w| {
            let inside: Vec<BallId> = e.balls().iter().filter(|b| b.d.is_inside(Some(physics_compat::env::PartId(p)))).map(|b| b.id).collect();
            if let Some(&last) = inside.last() {
                w.active_ball = Some(last);
            }
            Ok(Ret::Int(inside.len() as i32))
        });
    }
    if m.name == "DestroyBall" {
        // VPX's Trigger::DestroyBall: every ball inside the trigger leaves
        // its list of volumes and is destroyed; the active ball, if one of
        // them, passes to the first ball left (Player::DestroyBall)
        return with(|e, w| {
            let part = Some(physics_compat::env::PartId(p));
            let inside: Vec<BallId> = e.balls().iter().filter(|b| b.d.is_inside(part)).map(|b| b.id).collect();
            for &id in &inside {
                if let Some(b) = e.balls().iter().find(|b| b.id == id) {
                    b.d.remove_volume(part);
                }
                if let Some(gone) = e.remove_ball(id) {
                    if w.active_ball == Some(id) {
                        w.active_ball = e.balls().first().map(|b| b.id);
                    }
                    w.gone_balls.insert(id, gone);
                }
            }
            Ok(Ret::Int(inside.len() as i32))
        });
    }
    with(|_, w| {
        let t = w.parts[p as usize].trigger.as_mut()?;
        Some(match m.name {
            "CurrentAnimOffset" if !put => Ok(Ret::Float(t.offset)),
            // the physics keeps the radius it started with, as in VPX
            "Radius" if put => {
                t.radius = a.value_f32().unwrap_or(t.radius);
                Ok(Ret::Empty)
            }
            "Radius" => Ok(Ret::Float(t.radius)),
            _ => return None,
        })
    })
    .flatten()
    .or_else(|| position(p, m))
}

/// VPX's bumper members (src/parts/bumper.cpp): what the physics reads
/// while playing lives in the bumper's collider, as in VPX.
fn bumper(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|e, w| {
        let part = &mut w.parts[p as usize];
        if m.name == "EnableSkirtAnimation" {
            let anim = part.bumper.as_mut()?;
            return Some(if put {
                anim.put_enable_skirt_animation(a.value_bool().unwrap_or(true));
                Ok(Ret::Empty)
            } else {
                Ok(Ret::Bool(anim.enable_skirt_animation))
            });
        }
        // which of its pieces draw, as VPX's Render reads them each frame
        let piece = ["BaseVisible", "SkirtVisible", "RingVisible", "CapVisible"].iter().position(|n| *n == m.name);
        if let Some(i) = piece {
            let anim = part.bumper.as_mut()?;
            return Some(if put {
                anim.shown[i] = a.value_bool().unwrap_or(true);
                Ok(Ret::Empty)
            } else {
                Ok(Ret::Bool(anim.shown[i]))
            });
        }
        // the skirt's tilt now (read only, as in VPX)
        if !put && (m.name == "RotX" || m.name == "RotY") {
            let anim = part.bumper.as_ref()?;
            return Some(Ok(Ret::Float(if m.name == "RotX" { anim.rot_x } else { anim.rot_y })));
        }
        let &k = w.part_colliders.get(&p)?.iter().find(|&&k| matches!(e.colliders()[k as usize].shape, Shape::Bumper(_)))?;
        let Shape::Bumper(b) = &mut e.collider_mut(k).shape else { return None };
        Some(match m.name {
            "HasHitEvent" if put => {
                b.data.hit_event = a.value_bool().unwrap_or(true);
                Ok(Ret::Empty)
            }
            "HasHitEvent" => Ok(Ret::Bool(b.data.hit_event)),
            "Threshold" if put => {
                b.data.threshold = a.value_f32().unwrap_or(b.data.threshold);
                Ok(Ret::Empty)
            }
            "Threshold" => Ok(Ret::Float(b.data.threshold)),
            "Force" if put => {
                b.data.force = a.value_f32().unwrap_or(b.data.force);
                Ok(Ret::Empty)
            }
            "Force" => Ok(Ret::Float(b.data.force)),
            "CurrentRingOffset" if !put => Ok(Ret::Float(b.anim_ring_anim_offset)),
            // the ring and skirt move as if hit
            "PlayHit" => {
                b.anim_hit_event = true;
                Ok(Ret::Empty)
            }
            _ => return None,
        })
    })
    .flatten()
    .or_else(|| position(p, m))
}

/// VPX's light members (src/parts/light.cpp); the light's play-time
/// behaviour is in [`crate::light`].
fn light(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    // VPX refuses a glow that reaches less than nothing
    if put && m.name == "Falloff" && a.value_f32().is_some_and(|v| v < 0.0) {
        return Some(Err(E_FAIL));
    }
    let r = with(|e, w| {
        let now = e.time_msec();
        let l = w.parts[p as usize].light.as_mut()?;
        let f = || a.value_f32().unwrap_or(0.0);
        Some(match m.name {
            "State" if put => {
                l.put_state(f(), now);
                Ret::Empty
            }
            "State" => Ret::Float(l.get_state()),
            "Intensity" if put => {
                l.put_intensity(f());
                Ret::Empty
            }
            "Intensity" => Ret::Float(l.intensity),
            "IntensityScale" if put => {
                l.put_intensity_scale(f());
                Ret::Empty
            }
            "IntensityScale" => Ret::Float(l.intensity_scale),
            "FadeSpeedUp" if put => {
                l.fade_speed_up = f();
                Ret::Empty
            }
            "FadeSpeedUp" => Ret::Float(l.fade_speed_up),
            "FadeSpeedDown" if put => {
                l.fade_speed_down = f();
                Ret::Empty
            }
            "FadeSpeedDown" => Ret::Float(l.fade_speed_down),
            "Fader" if put => {
                l.fader = a.value_i32().unwrap_or(0);
                Ret::Empty
            }
            "Fader" => Ret::Int(l.fader),
            "Color" if put => {
                l.color = a.value_i32().unwrap_or(0) as u32;
                Ret::Empty
            }
            "Color" => Ret::Int(l.color as i32),
            "ColorFull" if put => {
                l.color2 = a.value_i32().unwrap_or(0) as u32;
                Ret::Empty
            }
            "ColorFull" => Ret::Int(l.color2 as i32),
            "BlinkPattern" if put => {
                l.put_blink_pattern(&a.value_string().unwrap_or_default(), now);
                Ret::Empty
            }
            "BlinkPattern" => Ret::Str(l.blink_pattern()),
            "BlinkInterval" if put => {
                l.put_blink_interval(a.value_i32().unwrap_or(0), now);
                Ret::Empty
            }
            "BlinkInterval" => Ret::Int(l.blink_interval),
            "Duration" => {
                l.duration(a.f32(0).unwrap_or(0.0), a.i32(1).unwrap_or(0), a.f32(2).unwrap_or(0.0), now);
                Ret::Empty
            }
            "GetInPlayState" => Ret::Float(l.in_play_state),
            "GetInPlayStateBool" => Ret::Bool(l.in_play_on()),
            "GetInPlayIntensity" => Ret::Float(l.current_intensity),
            "FilamentTemperature" if !put => Ret::Float(l.filament_temperature()),
            // where it's brightest, its halo's height, its blend, what it passes up: drawn as set
            "X" if put => {
                l.center[0] = f();
                Ret::Empty
            }
            "X" => Ret::Float(l.center[0]),
            "Y" if put => {
                l.center[1] = f();
                Ret::Empty
            }
            "Y" => Ret::Float(l.center[1]),
            "BulbHaloHeight" if put => {
                l.halo_height = f();
                Ret::Empty
            }
            "BulbHaloHeight" => Ret::Float(l.halo_height),
            // VPX draws a light switched between bulb and classic during play the other way at
            // once; ours keeps the way the table loaded it, for now (The Addams Family's swamp light)
            "Bulb" if put => {
                let bulb = a.value_bool().unwrap_or(l.bulb);
                if bulb != l.bulb {
                    crate::script::log::once("a light switched between bulb and classic during play is drawn as the table loaded it, for now");
                }
                l.bulb = bulb;
                Ret::Empty
            }
            "Bulb" => Ret::Bool(l.bulb),
            // how far its glow reaches (VPX refuses less than nothing) and how it fades, where it
            // sorts: drawn as set, as VPX reads them as it draws
            "Falloff" if put => {
                l.falloff = f();
                Ret::Empty
            }
            "Falloff" => Ret::Float(l.falloff),
            "FalloffPower" if put => {
                l.falloff_power = f();
                Ret::Empty
            }
            "FalloffPower" => Ret::Float(l.falloff_power),
            "DepthBias" if put => {
                l.depth_bias = f();
                Ret::Empty
            }
            "DepthBias" => Ret::Float(l.depth_bias),
            // its bulb's size: VPX shapes the bulb before play, and keeps a size set later
            "ScaleBulbMesh" if put => {
                l.mesh_radius = f();
                Ret::Empty
            }
            "ScaleBulbMesh" => Ret::Float(l.mesh_radius),
            "BulbModulateVsAdd" if put => {
                l.modulate_vs_add = f();
                Ret::Empty
            }
            "BulbModulateVsAdd" => Ret::Float(l.modulate_vs_add),
            "TransmissionScale" if put => {
                l.transmission = f().max(0.0);
                Ret::Empty
            }
            "TransmissionScale" => Ret::Float(l.transmission),
            // VPX reads both as it draws each frame (Renderer::RenderFrame gathers the lights that
            // shine on balls; Light::Render picks the shader casting their shadows)
            "ShowReflectionOnBall" if put => {
                l.on_ball = a.value_bool().unwrap_or(false);
                Ret::Empty
            }
            "ShowReflectionOnBall" => Ret::Bool(l.on_ball),
            // 0 none, 1 the balls' shadows (VPX's ShadowMode)
            "Shadows" if put => {
                l.ball_shadows = a.value_f32().unwrap_or(0.0) as i32 == 1;
                Ret::Empty
            }
            "Shadows" => Ret::Int(i32::from(l.ball_shadows)),
            // its bulb and socket: VPX reads it as it draws each frame (`Light::Render`)
            "ShowBulbMesh" if put => {
                l.show_bulb_mesh = a.value_bool().unwrap_or(false);
                Ret::Empty
            }
            "ShowBulbMesh" => Ret::Bool(l.show_bulb_mesh),
            _ => return None,
        })
    })
    .flatten();
    r.map(Ok).or_else(|| position(p, m))
}

/// VPX's score reel methods (src/parts/dispreel.cpp); the reels'
/// motor is in [`crate::reel`].
fn reel(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    // its picture (DispReel::put_Image), read as it's drawn: Flash Gordon lights its backdrop's
    // words by swapping them in and out
    if m.name == "Image" {
        return with(|_, w| {
            if put {
                let name = a.value_string().unwrap_or_default();
                let index = w.image_named(&name);
                w.parts[p as usize].reel.as_mut()?.image = Some((name, index));
                return Some(Ok(Ret::Empty));
            }
            let set = w.parts[p as usize].reel.as_ref()?.image.as_ref().map(|(n, _)| n.clone());
            let file = match &w.vpx.gameitems[p as usize] {
                GameItemEnum::Reel(r) => r.image.clone(),
                _ => String::new(),
            };
            Some(Ok(Ret::Str(set.unwrap_or(file))))
        })
        .flatten();
    }
    let r = with(|e, w| {
        let now = e.time_msec();
        let r = w.parts[p as usize].reel.as_mut()?;
        Some(match m.name {
            "AddValue" => {
                r.add_value(a.i32(0).unwrap_or(0));
                Ok(Ret::Empty)
            }
            "SetValue" => {
                r.set_value(a.i32(0).unwrap_or(0), now);
                Ok(Ret::Empty)
            }
            "ResetToZero" => {
                r.reset_to_zero();
                Ok(Ret::Empty)
            }
            "SpinReel" => {
                if r.spin_reel(a.i32(0).unwrap_or(0), a.i32(1).unwrap_or(0)) { Ok(Ret::Empty) } else { Err(E_FAIL) }
            }
            "UpdateInterval" if put => {
                r.put_update_interval(a.value_i32().unwrap_or(0), now);
                Ok(Ret::Empty)
            }
            "UpdateInterval" => Ok(Ret::Int(r.update_interval)),
            _ => return None,
        })
    })
    .flatten();
    r.or_else(|| position(p, m))
}

/// VPX's light sequencer members (src/parts/lightseq.cpp); the shows are
/// in [`crate::lightseq`].
fn light_seq(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|e, w| {
        let now = e.time_msec();
        // Play and StopPlay change the collection's parts: take the sequencer out meanwhile
        let mut seq = w.parts[p as usize].light_seq.take()?;
        let r = match m.name {
            "Collection" if put => {
                // as in VPX, the collection played stays the one found when play started
                seq.collection = a.value_string().unwrap_or_default();
                Ok(Ret::Empty)
            }
            "Collection" => Ok(Ret::Str(seq.collection.clone())),
            "CenterX" if put => {
                if seq.put_center_x(a.value_f32().unwrap_or(0.0)) { Ok(Ret::Empty) } else { Err(E_FAIL) }
            }
            "CenterX" => Ok(Ret::Float(seq.center_x())),
            "CenterY" if put => {
                if seq.put_center_y(a.value_f32().unwrap_or(0.0)) { Ok(Ret::Empty) } else { Err(E_FAIL) }
            }
            "CenterY" => Ok(Ret::Float(seq.center_y())),
            "UpdateInterval" if put => {
                seq.put_update_interval(a.value_i32().unwrap_or(0));
                Ok(Ret::Empty)
            }
            "UpdateInterval" => Ok(Ret::Int(seq.update_interval())),
            "Play" => {
                let arg = |i: usize, default: i32| a.i32(i).unwrap_or(default);
                if seq.play(arg(0, 0), arg(1, 0), arg(2, 1), arg(3, 0), &mut w.parts, now) { Ok(Ret::Empty) } else { Err(E_FAIL) }
            }
            "StopPlay" => {
                seq.stop_play(&mut w.parts, now);
                Ok(Ret::Empty)
            }
            _ => {
                w.parts[p as usize].light_seq = Some(seq);
                return None;
            }
        };
        w.parts[p as usize].light_seq = Some(seq);
        Some(r)
    })
    .flatten()
}

/// VPX's text box members (src/parts/textbox.cpp). Only a new `Text`
/// redraws the box's picture, as in VPX; the drawing is in
/// [`crate::textbox`]. `Font` stays the table's.
fn text_box(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    with(|_, w| {
        let t = w.parts[p as usize].text_box.as_mut()?;
        let number = || a.value_f32().unwrap_or(0.0);
        Some(Ok(match m.name {
            "Text" if put => {
                t.text = a.value_string().unwrap_or_default();
                t.version = t.version.wrapping_add(1);
                Ret::Empty
            }
            "Text" => Ret::Str(t.text.clone()),
            "BackColor" if put => {
                t.back_color = a.value_i32().unwrap_or(0) as u32;
                Ret::Empty
            }
            "BackColor" => Ret::Int(t.back_color as i32),
            "FontColor" if put => {
                t.font_color = a.value_i32().unwrap_or(0) as u32;
                Ret::Empty
            }
            "FontColor" => Ret::Int(t.font_color as i32),
            "IntensityScale" if put => {
                t.intensity_scale = number();
                Ret::Empty
            }
            "IntensityScale" => Ret::Float(t.intensity_scale),
            "Alignment" if put => {
                t.align = a.value_i32().unwrap_or(0);
                Ret::Empty
            }
            "Alignment" => Ret::Int(t.align),
            "IsTransparent" if put => {
                t.transparent = a.value_bool().unwrap_or(false);
                Ret::Empty
            }
            "IsTransparent" => Ret::Bool(t.transparent),
            "DMD" if put => {
                t.is_dmd = a.value_bool().unwrap_or(false);
                Ret::Empty
            }
            "DMD" => Ret::Bool(t.is_dmd),
            "Width" if put => {
                t.v2[0] = t.v1[0] + number();
                Ret::Empty
            }
            "Width" => Ret::Float(t.v2[0] - t.v1[0]),
            "Height" if put => {
                t.v2[1] = t.v1[1] + number();
                Ret::Empty
            }
            "Height" => Ret::Float(t.v2[1] - t.v1[1]),
            // moving the box moves both corners
            "X" | "Y" if put => {
                let k = usize::from(m.name == "Y");
                let delta = number() - t.v1[k];
                t.v1[k] += delta;
                t.v2[k] += delta;
                Ret::Empty
            }
            "X" => Ret::Float(t.v1[0]),
            "Y" => Ret::Float(t.v1[1]),
            _ => return None,
        }))
    })
    .flatten()
}

/// VPX's hit target members (src/parts/hittarget.cpp); the
/// animation is in [`crate::anim`].
fn hit_target(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    let put = m.access == Access::Put;
    // VPX's put_Collidable: every one of its colliders, when it changes
    if is(m, "Collidable") {
        return with(|e, w| {
            if !put {
                return Ok(Ret::Bool(w.parts[p as usize].collidable));
            }
            let on = a.value_bool().unwrap_or(true);
            if w.parts[p as usize].collidable != on {
                w.parts[p as usize].collidable = on;
                for &k in w.part_colliders.get(&p).map_or(&[][..], Vec::as_slice) {
                    e.collider_mut(k).base.enabled = on;
                }
            }
            Ok(Ret::Empty)
        });
    }
    with(|e, w| {
        let now = e.time_msec();
        let part = &mut w.parts[p as usize];
        Some(match m.name {
            "IsDropped" if put => {
                let dropped = a.value_bool().unwrap_or(false);
                if part.is_dropped != dropped {
                    part.is_dropped = dropped;
                    // the physics reads it at collision time (VPX's m_isDropped)
                    let id = physics_compat::env::PartId(p);
                    if dropped { e.env.dropped_targets.insert(id) } else { e.env.dropped_targets.remove(id) }
                    if let Some(anim) = &mut part.target {
                        anim.put_is_dropped(dropped, now);
                    }
                }
                Ok(Ret::Empty)
            }
            "IsDropped" => Ok(Ret::Bool(part.is_dropped)),
            "CurrentAnimOffset" if !put => Ok(Ret::Float(part.target.as_ref().map_or(0.0, |t| t.offset))),
            "HitThreshold" if !put => Ok(Ret::Float(part.hit_threshold)),
            _ => return None,
        })
    })
    .flatten()
    .or_else(|| position(p, m))
}

fn wall(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    if is(m, "IsDropped") {
        let r = with(|_, w| {
            let droppable = matches!(&w.vpx.gameitems[p as usize], GameItemEnum::Wall(wl) if wl.is_droppable);
            let part = &mut w.parts[p as usize];
            Some(if m.access == Access::Put {
                let dropped = a.value_bool().unwrap_or(false);
                // VPX: only a droppable wall drops; its colliders go with it
                if droppable && part.is_dropped != dropped {
                    part.is_dropped = dropped;
                    Ok(Some(!dropped && part.collidable))
                } else {
                    Ok(None)
                }
            } else {
                Err(part.is_dropped)
            })
        })
        .flatten()?;
        return Some(match r {
            Ok(change) => {
                if let Some(on) = change {
                    set_colliders(p, on);
                }
                Ok(Ret::Empty)
            }
            Err(dropped) => Ok(Ret::Bool(dropped)),
        });
    }
    if is(m, "Disabled") {
        // VPX's m_disabled, which its slingshot segments read as they're hit
        return with(|e, w| {
            if m.access != Access::Put {
                return Ok(Ret::Bool(w.parts[p as usize].disabled));
            }
            let disabled = a.value_bool().unwrap_or(false);
            w.parts[p as usize].disabled = disabled;
            for &k in w.part_colliders.get(&p).map_or(&[][..], Vec::as_slice) {
                if let Shape::Slingshot(s) = &mut e.collider_mut(k).shape {
                    s.data.disabled = disabled;
                }
            }
            Ok(Ret::Empty)
        });
    }
    if is(m, "SlingshotThreshold") {
        // VPX's slingshot segments read the wall's threshold live
        // (`LineSegSlingshot::Collide`): each of this wall's takes the new one
        return with(|e, w| {
            let colliders = w.part_colliders.get(&p).cloned().unwrap_or_default();
            if m.access == Access::Put {
                let threshold = a.value_f32().unwrap_or(0.0);
                for k in colliders {
                    if let Shape::Slingshot(s) = &mut e.collider_mut(k).shape {
                        s.data.threshold = threshold;
                    }
                }
                return Ok(Ret::Empty);
            }
            let from_physics = colliders.iter().find_map(|&k| match &e.colliders()[k as usize].shape {
                Shape::Slingshot(s) => Some(s.data.threshold),
                _ => None,
            });
            let from_file = match &w.vpx.gameitems[p as usize] {
                GameItemEnum::Wall(wl) => wl.slingshot_threshold,
                _ => 0.0,
            };
            Ok(Ret::Float(from_physics.unwrap_or(from_file)))
        });
    }
    None
}

/// Which of a primitive's numbers a member reads or sets.
enum PrimField {
    Pos(usize),
    Size(usize),
    Rot(usize),
}

fn primitive(p: u32, m: &'static Member, a: &Args) -> Option<Res> {
    // its animation (VPX's PlayAnim, PlayAnimEndless, StopAnim, ContinueAnim, ShowFrame): nothing
    // happens to a primitive without frames, or one drawn ahead
    if matches!(m.name, "PlayAnim" | "PlayAnimEndless" | "StopAnim" | "ContinueAnim" | "ShowFrame") {
        with(|_, w| {
            if let Some(anim) = w.parts[p as usize].prim_anim.as_mut() {
                let f = |i: usize| a.f32(i).unwrap_or(0.0);
                match m.name {
                    "PlayAnim" => anim.play(f(0), f(1)),
                    "PlayAnimEndless" => anim.play_endless(f(0)),
                    "StopAnim" => anim.stop(),
                    "ContinueAnim" => anim.resume(f(0)),
                    _ => anim.show(f(0)),
                }
            }
        });
        return Some(Ok(Ret::Empty));
    }
    // its colour, opacity (SetAlpha: never below 0) and blending, as drawn
    if matches!(m.name, "Color" | "Opacity" | "AddBlend") {
        let put = m.access == Access::Put;
        return with(|_, w| {
            let pr = w.parts[p as usize].primitive.as_mut()?;
            Some(Ok(match (m.name, put) {
                ("Color", true) => {
                    pr.color = a.value_i32().unwrap_or(pr.color as i32) as u32;
                    Ret::Empty
                }
                ("Color", false) => Ret::Int(pr.color as i32),
                ("Opacity", true) => {
                    pr.alpha = a.value_f32().unwrap_or(pr.alpha).max(0.0);
                    Ret::Empty
                }
                ("Opacity", false) => Ret::Float(pr.alpha),
                ("AddBlend", true) => {
                    pr.add_blend = a.value_bool().unwrap_or(pr.add_blend);
                    Ret::Empty
                }
                _ => Ret::Bool(pr.add_blend),
            }))
        })
        .flatten();
    }
    let field = match m.name {
        "X" => PrimField::Pos(0),
        "Y" => PrimField::Pos(1),
        "Z" => PrimField::Pos(2),
        "Size_X" => PrimField::Size(0),
        "Size_Y" => PrimField::Size(1),
        "Size_Z" => PrimField::Size(2),
        "RotX" | "RotAndTra0" => PrimField::Rot(0),
        "RotY" | "RotAndTra1" => PrimField::Rot(1),
        "RotZ" | "RotAndTra2" => PrimField::Rot(2),
        "TransX" | "RotAndTra3" => PrimField::Rot(3),
        "TransY" | "RotAndTra4" => PrimField::Rot(4),
        "TransZ" | "RotAndTra5" => PrimField::Rot(5),
        "ObjRotX" | "RotAndTra6" => PrimField::Rot(6),
        "ObjRotY" | "RotAndTra7" => PrimField::Rot(7),
        "ObjRotZ" | "RotAndTra8" => PrimField::Rot(8),
        "Collidable" => {
            return with(|e, w| {
                let id = physics_compat::env::PartId(p);
                Some(if m.access == Access::Put {
                    // VPX's put_Collidable: the part's flag, which the collision tree reads to skip
                    // places holding nothing else, and each of its colliders, switched when the
                    // first differs — without them a primitive sharing a place with other parts
                    // was still hit (Harry Potter's lowered masks)
                    let on = a.value_bool().unwrap_or(true);
                    if on { e.env.uncollidable_primitives.remove(id) } else { e.env.uncollidable_primitives.insert(id) }
                    let colliders = w.part_colliders.get(&p).map_or(&[][..], Vec::as_slice);
                    if colliders.first().is_some_and(|&k| e.colliders()[k as usize].base.enabled != on) {
                        for &k in colliders {
                            e.collider_mut(k).base.enabled = on;
                        }
                    }
                    Ok(Ret::Empty)
                } else {
                    Ok(Ret::Bool(!e.env.uncollidable_primitives.contains(&id)))
                })
            })
            .flatten();
        }
        "HitThreshold" if m.access == Access::Get => {
            return with(|_, w| Ok(Ret::Float(w.parts[p as usize].hit_threshold)));
        }
        _ => return None,
    };
    with(|_, w| {
        let place = w.placement(p)?;
        let slot = match field {
            PrimField::Pos(i) => &mut place.position[i],
            PrimField::Size(i) => &mut place.size[i],
            PrimField::Rot(i) => &mut place.rot_and_tra[i],
        };
        Some(if m.access == Access::Put {
            *slot = a.value_f32()?;
            place.changed = true;
            Ok(Ret::Empty)
        } else {
            Ok(Ret::Float(*slot))
        })
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_settings_default_to_vpxs() {
        // no settings: the backglass server used, every toy both its sound and the toy
        assert_eq!(controller_setting("ForceDisableB2S"), Some(0));
        assert_eq!(controller_setting("dofflippers"), Some(2));
        assert_eq!(controller_setting("DOFDropTargets"), Some(2));
        assert_eq!(controller_setting("DOFSomethingElse"), None);
    }
}
