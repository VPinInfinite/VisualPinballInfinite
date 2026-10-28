// Ported from Visual Pinball X: src/renderer/ViewSetup.cpp, ViewSetup::ComputeMVP,
// FitCameraToVertices, ApplyTableOverrideSettings, SetWindowModeFromSettings,
// SetViewPosFromPlayerPosition, GetRealToVirtualScale and GetRotation, and the view settings'
// ranges (src/core/Settings_properties.inl) (license:GPLv3+), commit af26b2d (10.8.1
// pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The camera: VPX's view setups (desktop, cabinet, single-screen cabinet) and
//! how VPX turns one into view and projection matrices, so a table is seen as
//! its author set it up — with what the settings file overrides, and VPX's
//! window mode for a cabinet's screen, which places the eye where the player
//! stands and turns the table to suit a portrait or landscape screen. The
//! camera is an input to the renderer.

use vpin::vpx::VPX;
use vpin::vpx::gamedata::{ViewLayoutMode, ViewSetupId};

use crate::math::{Mat4, Vec3};
use crate::settings::Settings;

/// VPX's `CMTOVPU`.
pub fn cm_to_vpu(cm: f32) -> f32 {
    cm * (50.0 / (2.54 * 1.0625)) as f32
}

/// VPX's `VPUTOCM`.
pub fn vpu_to_cm(vpu: f32) -> f32 {
    vpu * (2.54 * 1.0625 / 50.0) as f32
}

/// VPX's defaults for the cabinet's screen and where the player stands
/// before it (`[Player]` settings): a 43-inch 16:9 screen, flat, the player
/// 10 cm back from its bottom edge and 70 cm above it.
const SCREEN_WIDTH_CM: f32 = 95.89;
const PLAYER_CM: [f32; 3] = [0.0, -10.0, 70.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Legacy,
    Camera,
    Window,
}

/// VPX's `ViewSetup`, with VPX's defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewSetup {
    pub mode: Mode,
    pub scene_scale: Vec3,
    pub view_x: f32,
    pub view_y: f32,
    pub view_z: f32,
    pub look_at: f32,
    pub viewport_rotation: f32,
    pub fov: f32,
    pub layback: f32,
    pub view_h_ofs: f32,
    pub view_v_ofs: f32,
    pub window_top_z_ofs: f32,
    pub window_bottom_z_ofs: f32,
    /// The cabinet screen's physical width in centimetres, measured with the
    /// screen held landscape: in window mode, how tall the playfield shows
    /// (VPX's `ScreenWidth` setting).
    pub screen_width_cm: f32,
}

impl Default for ViewSetup {
    fn default() -> Self {
        Self {
            mode: Mode::Legacy,
            scene_scale: Vec3::new(1.0, 1.0, 1.0),
            view_x: 0.0,
            view_y: cm_to_vpu(20.0),
            view_z: cm_to_vpu(70.0),
            look_at: 0.25,
            viewport_rotation: 0.0,
            fov: 45.0,
            layback: 0.0,
            view_h_ofs: 0.0,
            view_v_ofs: 0.0,
            window_top_z_ofs: cm_to_vpu(20.0),
            window_bottom_z_ofs: cm_to_vpu(7.5),
            screen_width_cm: SCREEN_WIDTH_CM,
        }
    }
}

/// Which of the table's view setups to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Desktop,
    Cabinet,
    SingleScreen,
}

impl ViewSetup {
    /// The table's view setup, as VPX reads it: missing values keep VPX's
    /// defaults, and a cabinet or single-screen setup missing from an old
    /// table copies the desktop one.
    pub fn from_table(vpx: &VPX, view: View) -> Self {
        let read = |id: ViewSetupId| {
            let s = &vpx.gamedata.view_setups[id];
            let d = Self::default();
            Self {
                mode: match s.mode {
                    Some(ViewLayoutMode::Camera) => Mode::Camera,
                    Some(ViewLayoutMode::Window) => Mode::Window,
                    _ => Mode::Legacy,
                },
                scene_scale: Vec3::new(
                    s.scene_scale_x.unwrap_or(d.scene_scale.x),
                    s.scene_scale_y.unwrap_or(d.scene_scale.y),
                    s.scene_scale_z.unwrap_or(d.scene_scale.z),
                ),
                view_x: s.view_x.unwrap_or(d.view_x),
                view_y: s.view_y.unwrap_or(d.view_y),
                view_z: s.view_z.unwrap_or(d.view_z),
                look_at: s.look_at.unwrap_or(d.look_at),
                viewport_rotation: s.viewport_rotation.unwrap_or(d.viewport_rotation),
                fov: s.fov.unwrap_or(d.fov),
                layback: s.layback.unwrap_or(d.layback),
                view_h_ofs: s.view_horizontal_offset.unwrap_or(d.view_h_ofs),
                view_v_ofs: s.view_vertical_offset.unwrap_or(d.view_v_ofs),
                window_top_z_ofs: s.window_top_z_offset.unwrap_or(d.window_top_z_ofs),
                window_bottom_z_ofs: s.window_bottom_z_offset.unwrap_or(d.window_bottom_z_ofs),
                screen_width_cm: d.screen_width_cm,
            }
        };
        let id = match view {
            View::Desktop => ViewSetupId::Desktop,
            View::Cabinet => ViewSetupId::Fullscreen,
            View::SingleScreen => ViewSetupId::FullSingleScreen,
        };
        if id != ViewSetupId::Desktop && vpx.gamedata.view_setups[id].fov.is_none() {
            return read(ViewSetupId::Desktop);
        }
        read(id)
    }

    /// What the settings file says, as VPX applies it as play starts (build
    /// 5436): `ApplyTableOverrideSettings` for the view drawn — each value
    /// from `[TableOverride]` (`ViewDT…`, `ViewFSS…` or `ViewCab…`: `Mode`,
    /// `ScaleX`, `Rotation`, `PlayerX`, `LookAt`, `FOV`, `HOfs`, `VOfs`,
    /// `WindowTop`, `WindowBot`, `Layback`, …) when set, else the table's own,
    /// held to the setting's range and step, as VPX holds the table's value it
    /// makes the setting's default — then, in window mode,
    /// `SetWindowModeFromSettings`: the eye placed where the player stands
    /// before the cabinet's screen (`[Player]` `ScreenPlayerX`, `…Y`, `…Z`, in
    /// centimetres from the bottom middle of the playfield; the screen tilted
    /// `ScreenInclination` degrees and `ScreenWidth` centimetres across — by
    /// default the screen's own size, `screen_cm`, when it reports one,
    /// else VPX's default). With no settings
    /// file VPX's defaults hold.
    pub fn apply_settings(&mut self, settings: &Settings, view: View, table: &TableExtent, screen_cm: Option<(f32, f32)>) {
        let (prefix, layback_min) = match view {
            View::Desktop => ("ViewDT", -90.0),
            View::SingleScreen => ("ViewFSS", -90.0),
            View::Cabinet => ("ViewCab", 0.0),
        };
        let f = |name: &str, v: f32, min: f32, max: f32, step: f32| settings.float_or("TableOverride", &format!("{prefix}{name}"), v, min, max, step);
        let mode = match self.mode {
            Mode::Legacy => 0,
            Mode::Camera => 1,
            Mode::Window => 2,
        };
        self.mode = match settings.enum_or("TableOverride", &format!("{prefix}Mode"), mode, 3) {
            1 => Mode::Camera,
            2 => Mode::Window,
            _ => Mode::Legacy,
        };
        self.scene_scale = Vec3::new(f("ScaleX", self.scene_scale.x, 0.5, 1.5, 0.0), f("ScaleY", self.scene_scale.y, 0.5, 1.5, 0.0), f("ScaleZ", self.scene_scale.z, 0.5, 1.5, 0.0));
        self.viewport_rotation = f("Rotation", self.viewport_rotation, 0.0, 360.0, 90.0);
        self.view_x = f("PlayerX", self.view_x, cm_to_vpu(-30.0), cm_to_vpu(30.0), 0.0);
        self.view_y = f("PlayerY", self.view_y, cm_to_vpu(-100.0), cm_to_vpu(100.0), 0.0);
        self.view_z = f("PlayerZ", self.view_z, cm_to_vpu(-100.0), cm_to_vpu(100.0), 0.0);
        self.look_at = f("LookAt", self.look_at, 0.0, 100.0, 0.0);
        self.fov = f("FOV", self.fov, 25.0, 90.0, 0.0);
        self.view_h_ofs = f("HOfs", self.view_h_ofs, -30.0, 30.0, 0.0);
        self.view_v_ofs = f("VOfs", self.view_v_ofs, -20.0, 50.0, 0.0);
        self.window_top_z_ofs = f("WindowTop", self.window_top_z_ofs, cm_to_vpu(0.0), cm_to_vpu(50.0), 0.0);
        self.window_bottom_z_ofs = f("WindowBot", self.window_bottom_z_ofs, cm_to_vpu(0.0), cm_to_vpu(50.0), 0.0);
        self.layback = f("Layback", self.layback, layback_min, 90.0, 0.0);
        self.screen_width_cm = settings.float_or("Player", "ScreenWidth", screen_cm.map_or(SCREEN_WIDTH_CM, |(long, _)| long), 5.0, 200.0, 0.0);
        if self.mode == Mode::Window {
            let player = Vec3::new(
                settings.float_or("Player", "ScreenPlayerX", PLAYER_CM[0], -30.0, 30.0, 0.0),
                settings.float_or("Player", "ScreenPlayerY", PLAYER_CM[1], -70.0, 30.0, 0.0),
                settings.float_or("Player", "ScreenPlayerZ", PLAYER_CM[2], 30.0, 100.0, 0.0),
            );
            let inclination = settings.float_or("Player", "ScreenInclination", 0.0, -30.0, 30.0, 0.0);
            self.set_view_from_player(table, player, inclination);
        }
    }

    /// The `[TableOverride]` changes (VPX 5436's names: `ViewDT…`, `ViewFSS…`,
    /// `ViewCab…`) that make the table's own settings file hold this setup
    /// for the view drawn, as `apply_settings` reads it back: each value the
    /// table's own view (`own`: the table's, with the main settings file's
    /// over it — neither its own file's nor its camera file's) already holds
    /// taken out, every other written, so a camera file's values that the
    /// table's own file now stands in for aren't lost (VPX reads a table's
    /// camera file only while it has no settings file). In window mode the
    /// eye follows where the player stands (`[Player]` `ScreenPlayerX`, …),
    /// so its place is left as the file has it.
    pub fn overrides_from(&self, own: &ViewSetup, view: View) -> Vec<vpi_settings::Change> {
        let prefix = override_prefix(view);
        let window = self.mode == Mode::Window;
        self.override_values()
            .into_iter()
            .zip(own.override_values())
            .filter(|((name, _), _)| !(window && name.starts_with("Player")))
            .map(|((name, v), (_, o))| {
                let key = format!("{prefix}{name}");
                if v == o {
                    vpi_settings::Change::remove("TableOverride", &key)
                } else if name == "Mode" {
                    vpi_settings::Change::set("TableOverride", &key, v as i32)
                } else {
                    vpi_settings::Change::set("TableOverride", &key, v)
                }
            })
            .collect()
    }

    /// Each value VPX's view settings hold, by the name after their prefix.
    fn override_values(&self) -> [(&'static str, f32); 15] {
        let mode = match self.mode {
            Mode::Legacy => 0.0,
            Mode::Camera => 1.0,
            Mode::Window => 2.0,
        };
        [
            ("Mode", mode),
            ("ScaleX", self.scene_scale.x),
            ("ScaleY", self.scene_scale.y),
            ("ScaleZ", self.scene_scale.z),
            ("Rotation", self.viewport_rotation),
            ("PlayerX", self.view_x),
            ("PlayerY", self.view_y),
            ("PlayerZ", self.view_z),
            ("LookAt", self.look_at),
            ("FOV", self.fov),
            ("HOfs", self.view_h_ofs),
            ("VOfs", self.view_v_ofs),
            ("WindowTop", self.window_top_z_ofs),
            ("WindowBot", self.window_bottom_z_ofs),
            ("Layback", self.layback),
        ]
    }

    /// VPX's `SetViewPosFromPlayerPosition`: the eye from where the player
    /// stands, in centimetres, turned by the angle between the playfield and
    /// the room's level.
    fn set_view_from_player(&mut self, table: &TableExtent, player_cm: Vec3, inclination_deg: f32) {
        let real_to_virtual = self.real_to_virtual(table);
        let rotx = Mat4::rotate_x((self.window_top_z_ofs - self.window_bottom_z_ofs).atan2(table.bottom) - inclination_deg.to_radians());
        let pos = rotx.transform_no_perspective(Vec3::new(cm_to_vpu(player_cm.x), cm_to_vpu(player_cm.y), cm_to_vpu(player_cm.z)));
        self.view_x = pos.x;
        self.view_y = pos.y;
        self.view_z = pos.z + self.window_bottom_z_ofs * self.scene_scale.y / real_to_virtual;
    }

    /// VPX's `GetRealToVirtualScale`: in window mode, how much bigger the
    /// playfield is than the cabinet's screen shows it; 1 otherwise.
    fn real_to_virtual(&self, table: &TableExtent) -> f32 {
        if self.mode != Mode::Window {
            return 1.0;
        }
        let screen_height = self.screen_width_cm;
        let inc = (self.window_top_z_ofs - self.window_bottom_z_ofs).atan2(table.bottom - table.top);
        if screen_height <= 1.0 { 1.0 } else { (vpu_to_cm(table.bottom - table.top) / inc.cos()) / screen_height }
    }

    /// The window's top and bottom heights above the playfield (VPX's
    /// `GetWindowTopZOffset`, `GetWindowBottomZOffset`): none but in window mode.
    fn window_z(&self) -> (f32, f32) {
        if self.mode == Mode::Window { (self.window_top_z_ofs, self.window_bottom_z_ofs) } else { (0.0, 0.0) }
    }

    /// The table's turn on screen in degrees, 0 to 360 (VPX's `GetRotation`):
    /// in window mode a quarter turn more for a landscape window, so the
    /// playfield runs up a portrait screen and along a landscape one.
    pub fn rotation(&self, width: u32, height: u32) -> f32 {
        if self.mode == Mode::Window {
            let r = self.viewport_rotation as i32 % 360;
            (((if width < height { 0 } else { 3 }) * 90 + if r < 0 { r + 360 } else { r }) % 360) as f32
        } else {
            let r = self.viewport_rotation % 360.0;
            if r < 0.0 { r + 360.0 } else { r }
        }
    }
}

/// The start of VPX's settings names for a view's setup.
fn override_prefix(view: View) -> &'static str {
    match view {
        View::Desktop => "ViewDT",
        View::SingleScreen => "ViewFSS",
        View::Cabinet => "ViewCab",
    }
}

/// The table's extent, as the camera needs it.
#[derive(Clone, Copy, Debug)]
pub struct TableExtent {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// What the renderer needs: the view (orthonormal, for shading) and the
/// projection, which together place the table on screen.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view: Mat4,
    pub projection: Mat4,
    /// The eye, in table coordinates.
    pub eye: Vec3,
}

/// VPX's `FitCameraToVertices`.
fn fit_camera_to_vertices(vertices: &[Vec3], aspect: f32, rotation: f32, inclination: f32, fov: f32, xlatez: f32, layback: f32) -> Vec3 {
    let (rrotsin, rrotcos) = rotation.sin_cos();
    let (rincsin, rinccos) = inclination.sin_cos();
    let slopey = (0.5 * fov.to_radians()).tan();
    let slopex = slopey * aspect;
    let (mut maxy, mut miny, mut maxx, mut minx) = (f32::MIN, f32::MAX, f32::MIN, f32::MAX);
    let mut layback_trans = Mat4::IDENTITY;
    layback_trans.m[2][1] = -(0.5 * layback.to_radians()).tan();
    for &p in vertices {
        let mut v = layback_trans.transform(p);
        let temp = v.y;
        v.y = rinccos * temp - rincsin * v.z;
        v.z = rincsin * temp + rinccos * v.z;
        let temp = v.x;
        v.x = rrotcos * temp - rrotsin * v.y;
        v.y = rrotsin * temp + rrotcos * v.y;
        maxy = maxy.max(v.y + slopey * v.z);
        miny = miny.min(v.y - slopey * v.z);
        maxx = maxx.max(v.x + slopex * v.z);
        minx = minx.min(v.x - slopex * v.z);
    }
    let ydist = (maxy - miny) / (slopey * 2.0);
    let xdist = (maxx - minx) / (slopex * 2.0);
    Vec3::new((maxx + minx) * 0.5, (maxy + miny) * 0.5, ydist.max(xdist) + xlatez)
}

/// VPX's `ComputeNearFarPlane`.
fn near_far(bounds: &[Vec3], view: &Mat4) -> (f32, f32) {
    let (mut znear, mut zfar) = (f32::MAX, f32::MIN);
    for &v in bounds {
        let p = view.transform_no_perspective(v);
        if p.z > 0.0 {
            znear = znear.min(p.z);
            zfar = zfar.max(p.z);
        }
    }
    znear *= 0.9;
    zfar *= 1.1;
    znear = znear.max(cm_to_vpu(5.0));
    // VPX's clamp, the lower bound first: with nothing visible in front of the camera the
    // bounds cross, and VPX keeps the lower (Rust's clamp would stop the program)
    let (low, high) = (znear + 1.0, cm_to_vpu(100000.0));
    zfar = if zfar < low { low } else { zfar.min(high) };
    (znear, zfar)
}

/// VPX's `ComputeMVP` for one eye, without the pixel offset used for
/// oversampling. `legacy_bounds` are the vertices VPX fits its legacy camera
/// to; `bounds` every visible part's, for the depth range, which also covers
/// them as each of the table's `mirrors` (planes: normal, distance) shows
/// them. Window mode fits the playfield to the window, turned in quarter
/// turns to suit the window's shape, with the cabinet's screen measurements
/// from the settings (`ViewSetup::apply_settings`).
pub fn compute(setup: &ViewSetup, table: &TableExtent, aspect: f32, legacy_bounds: &[Vec3], bounds: &[Vec3], mirrors: &[[f32; 4]]) -> Camera {
    let mode = setup.mode;
    let fov = setup.fov.max(1.0);
    let is_legacy = mode == Mode::Legacy;
    let is_window = mode == Mode::Window;
    let (window_top_z, window_bot_z) = setup.window_z();
    let real_to_virtual = setup.real_to_virtual(table);

    // window mode doesn't turn freely: the table is fitted to the screen, in quarter turns
    let (rotation, quadrant) = if is_window {
        let r = setup.viewport_rotation as i32 % 360;
        let quadrant = if aspect < 1.0 { 0 } else { 3 } + (if r < 0 { r + 360 } else { r }) / 90;
        ((quadrant as f32 * 90.0).to_radians(), quadrant)
    } else {
        (setup.viewport_rotation.to_radians(), 0)
    };
    let inc = match mode {
        Mode::Legacy => setup.look_at.to_radians(),
        Mode::Camera => -std::f32::consts::PI + (-setup.view_y - (setup.look_at / 100.0) * table.bottom).atan2(-setup.view_z),
        Mode::Window => (window_top_z - window_bot_z).atan2(table.bottom),
    };
    let rotz = Mat4::rotate_z(rotation);

    let (scale, coords, lookat, layback, mat_view);
    if is_legacy {
        let mut lb = Mat4::IDENTITY;
        lb.m[2][1] = -(0.5 * setup.layback.to_radians()).tan();
        layback = lb;
        coords = Mat4::scale(setup.scene_scale.x, -setup.scene_scale.y, -setup.scene_scale.z);
        let fit = fit_camera_to_vertices(legacy_bounds, aspect, rotation, inc, fov, setup.view_z, setup.layback);
        let mut pos = Vec3::new(setup.view_x - fit.x, setup.view_y - fit.y, -fit.z);
        pos = Mat4::rotate_z(-rotation).transform_no_perspective(pos);
        pos = Mat4::rotate_x(-inc).transform_no_perspective(pos);
        let trans = Mat4::translate(pos.x, pos.y, pos.z);
        let rotx = Mat4::rotate_x(inc);
        lookat = trans * rotx;
        scale = Mat4::IDENTITY;
        mat_view = layback * trans * rotx * rotz * coords;
    } else {
        layback = Mat4::IDENTITY;
        let scale_z = if is_window { setup.scene_scale.y / real_to_virtual } else { setup.scene_scale.z };
        scale = Mat4::translate(-0.5 * table.right, -0.5 * table.bottom, -window_bot_z)
            * Mat4::scale(setup.scene_scale.x / real_to_virtual, setup.scene_scale.y / real_to_virtual, scale_z)
            * Mat4::translate(0.5 * table.right, 0.5 * table.bottom, window_bot_z);
        let trans = Mat4::translate(-setup.view_x - 0.5 * table.right, -setup.view_y - table.bottom, -setup.view_z);
        let rotx = Mat4::rotate_x(inc);
        coords = Mat4::scale(1.0, -1.0, -1.0);
        lookat = trans * rotx;
        mat_view = scale * trans * rotx * rotz * coords;
    }

    // the depth range covers the reflected views too, so mirrors aren't cut off
    let (mut znear, mut zfar) = near_far(bounds, &mat_view);
    for &[x, y, z, d] in mirrors {
        let len = (x * x + y * y + z * z).sqrt();
        if len == 0.0 {
            continue;
        }
        let reflect = Mat4::plane_reflection(Vec3::new(x / len, y / len, z / len), d);
        let (near, far) = near_far(bounds, &(reflect * mat_view));
        znear = znear.min(near);
        zfar = zfar.max(far);
    }

    let (xcenter, ycenter, xspan, yspan) = match mode {
        Mode::Legacy => {
            let yspan = znear * (0.5 * fov.to_radians()).tan();
            (0.0, 0.0, yspan * aspect, yspan)
        }
        Mode::Camera => {
            let ymax = znear * (0.5 * fov.to_radians()).tan();
            let (s, c) = rotation.sin_cos();
            (
                znear * 0.01 * (setup.view_v_ofs * s - setup.view_h_ofs * c),
                znear * 0.01 * (setup.view_v_ofs * c + setup.view_h_ofs * s),
                ymax * aspect,
                ymax,
            )
        }
        Mode::Window => {
            // fit the table's middle line, from the window's bottom to its top, to the screen's
            // height, seen at the table's own scale through a 90-degree view of aspect 1
            let unit = Mat4::perspective_off_center_lh(-znear, znear, -znear, znear, znear, zfar);
            let fit = Mat4::translate(-0.5 * table.right, -0.5 * table.bottom, -window_bot_z)
                * Mat4::scale(1.0 / real_to_virtual, 1.0 / real_to_virtual, 1.0 / real_to_virtual)
                * Mat4::translate(0.5 * table.right, 0.5 * table.bottom, window_bot_z)
                * lookat
                * rotz
                * Mat4::scale(1.0, -1.0, -1.0)
                * unit;
            let middle = 0.5 * (table.left + table.right);
            let top = fit.transform(Vec3::new(middle, table.top, window_top_z));
            let bottom = fit.transform(Vec3::new(middle, table.bottom, window_bot_z));
            let (xmin, xmax) = (znear * bottom.x.min(top.x), znear * bottom.x.max(top.x));
            let (ymin, ymax) = (znear * bottom.y.min(top.y), znear * bottom.y.max(top.y));
            // the screen's physical width, measured landscape, is its height in window mode
            let screen_height = setup.screen_width_cm;
            let per_offset = |span: f32| if screen_height <= 1.0 { 1.0 } else { span / (screen_height * real_to_virtual / setup.scene_scale.y) };
            let (offset_scale, xspan, yspan) = if quadrant & 1 == 0 {
                let yspan = 0.5 * (ymax - ymin);
                (per_offset(ymax - ymin), yspan * aspect, yspan)
            } else {
                let xspan = 0.5 * (xmax - xmin);
                (per_offset(xmax - xmin), xspan, xspan / aspect)
            };
            let (s, c) = rotation.sin_cos();
            (
                0.5 * (xmin + xmax) + offset_scale * (setup.view_v_ofs * s - setup.view_h_ofs * c),
                0.5 * (ymin + ymax) + offset_scale * (setup.view_v_ofs * c + setup.view_h_ofs * s),
                xspan,
                yspan,
            )
        }
    };

    let view = lookat;
    let inv_lookat = lookat.inverse();
    let scale = inv_lookat * scale * lookat;
    let proj = Mat4::perspective_off_center_lh(xcenter - xspan, xcenter + xspan, ycenter - yspan, ycenter + yspan, znear, zfar);
    let mut projection = scale * rotz * coords * proj;
    if is_legacy && setup.layback.abs() > 0.1 {
        projection = view.inverse() * layback * view * projection;
    }
    let eye = view.inverse().transform_no_perspective(Vec3::default());
    Camera { view, projection, eye }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: TableExtent = TableExtent { left: 0.0, top: 0.0, right: 1000.0, bottom: 2000.0 };

    fn window_setup() -> ViewSetup {
        ViewSetup { mode: Mode::Window, viewport_rotation: 0.0, window_top_z_ofs: 300.0, window_bottom_z_ofs: 100.0, ..ViewSetup::default() }
    }

    #[test]
    fn the_tables_view_is_held_to_the_settings_ranges() {
        // as VPX makes a table's value a setting's default: held to its range and step
        let mut v = ViewSetup { mode: Mode::Camera, fov: 20.0, viewport_rotation: 45.0, layback: -10.0, ..ViewSetup::default() };
        v.apply_settings(&Settings::default(), View::Cabinet, &TABLE, None);
        assert_eq!((v.fov, v.viewport_rotation, v.layback), (25.0, 90.0, 0.0));
        // what the file sets, when in range; out of range, the table's own holds
        let s = Settings::from_text("[TableOverride]\nViewDTRotation = 270\nViewDTFOV = 400\nViewDTMode = 2\n");
        let mut v = ViewSetup { mode: Mode::Camera, fov: 50.0, ..ViewSetup::default() };
        v.apply_settings(&s, View::Desktop, &TABLE, None);
        assert_eq!((v.viewport_rotation, v.fov, v.mode), (270.0, 50.0, Mode::Window));
    }

    #[test]
    fn a_view_set_while_playing_is_kept_as_what_differs_from_its_own_and_read_back_as_set() {
        let own = ViewSetup { mode: Mode::Camera, ..ViewSetup::default() };
        let mut set = own;
        set.fov = 61.5;
        set.look_at = 40.0;
        set.view_z = cm_to_vpu(80.0);
        let changes = set.overrides_from(&own, View::Desktop);
        let written: Vec<&str> = changes.iter().filter(|c| c.value.is_some()).map(|c| c.name.as_str()).collect();
        assert_eq!(written, ["ViewDTPlayerZ", "ViewDTLookAt", "ViewDTFOV"]);
        // written into a table's own settings file over a value it held, then read back as play
        // starts: what equals its own view taken out
        let text = vpi_settings::edit("[Player]\nSoundVolume = 80\n[TableOverride]\nViewDTLayback = 5\n", &changes);
        assert!(!text.contains("ViewDTLayback") && text.contains("SoundVolume = 80"), "{text}");
        let mut read = own;
        read.apply_settings(&Settings::from_text(&text), View::Desktop, &TABLE, None);
        assert_eq!((read.fov, read.look_at, read.mode), (61.5, 40.0, Mode::Camera));
        assert!((read.view_z - set.view_z).abs() < 1e-3, "{} {}", read.view_z, set.view_z);
        // its own view again: every one of the view's values taken out
        let again = vpi_settings::edit(&text, &own.overrides_from(&own, View::Desktop));
        assert!(!again.contains("ViewDT") && again.contains("SoundVolume = 80"), "{again}");
        // in window mode the eye's place follows the player's, so it's left as it is
        let mut window = window_setup();
        let was = window;
        window.view_y += 10.0;
        window.fov = 30.0;
        let changes = window.overrides_from(&was, View::Cabinet);
        assert!(changes.iter().all(|c| !c.name.starts_with("ViewCabPlayer")));
        assert_eq!(changes.iter().filter(|c| c.value.is_some()).map(|c| c.name.as_str()).collect::<Vec<_>>(), ["ViewCabFOV"]);
    }

    #[test]
    fn window_mode_turns_to_suit_the_screen() {
        let v = window_setup();
        assert_eq!((v.rotation(1080, 1920), v.rotation(1920, 1080)), (0.0, 270.0));
        let camera = ViewSetup { mode: Mode::Camera, viewport_rotation: -90.0, ..ViewSetup::default() };
        assert_eq!(camera.rotation(1920, 1080), 270.0);
    }

    #[test]
    fn window_mode_places_the_eye_where_the_player_stands() {
        let mut v = window_setup();
        v.apply_settings(&Settings::default(), View::Cabinet, &TABLE, None);
        // VPX's default player, 70 cm above the screen's bottom edge, 10 cm before it
        assert!(v.view_z > cm_to_vpu(60.0) && v.view_y < 0.0 && v.view_x.abs() < 1e-3, "{v:?}");
        // a 2000-unit playfield (about 108 cm) on a 95.89 cm screen
        assert!((v.real_to_virtual(&TABLE) - vpu_to_cm(2000.0) / (200.0f32.atan2(2000.0)).cos() / 95.89).abs() < 1e-4);
    }

    #[test]
    fn vpx_5436s_cabinet_settings_written_out_are_the_players_defaults() {
        // the VPX picture tool's cabinet view (tools/vpx-pictures --cabinet): VPX 5436's defaults
        // for the screen and the player written out, so the view is as with window mode alone
        let written = Settings::from_text("[Player]\r\nBGSet = 1\r\nScreenWidth = 95.89\r\nScreenPlayerX = 0\r\nScreenPlayerY = -10\r\nScreenPlayerZ = 70\r\nScreenInclination = 0\r\n\r\n[TableOverride]\r\nViewCabMode = 2\r\n");
        let table_view = ViewSetup { mode: Mode::Camera, window_top_z_ofs: 300.0, window_bottom_z_ofs: 100.0, ..ViewSetup::default() };
        let (mut a, mut b) = (table_view, table_view);
        a.apply_settings(&written, View::Cabinet, &TABLE, None);
        b.apply_settings(&Settings::from_text("[TableOverride]\nViewCabMode = 2\n"), View::Cabinet, &TABLE, None);
        assert_eq!(written.view(), Some(View::Cabinet));
        assert_eq!((a.mode, a.view_x, a.view_y, a.view_z, a.screen_width_cm), (Mode::Window, b.view_x, b.view_y, b.view_z, b.screen_width_cm));
    }

    #[test]
    fn window_mode_fits_the_playfield_from_bottom_to_top() {
        let mut v = window_setup();
        v.apply_settings(&Settings::default(), View::Cabinet, &TABLE, None);
        let bounds = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1000.0, 2000.0, 0.0), Vec3::new(0.0, 2000.0, 300.0), Vec3::new(1000.0, 0.0, 300.0)];
        // a portrait screen: the playfield runs up it, its top at the top edge and its bottom at the bottom
        let c = compute(&v, &TABLE, 9.0 / 16.0, &[], &bounds, &[]);
        let vp = c.view * c.projection;
        let (top, bottom) = (vp.transform(Vec3::new(500.0, 0.0, 300.0)), vp.transform(Vec3::new(500.0, 2000.0, 100.0)));
        assert!((top.y - 1.0).abs() < 1e-3 && (bottom.y + 1.0).abs() < 1e-3 && top.x.abs() < 1e-3, "{top:?} {bottom:?}");
        // a landscape one: turned a quarter, the playfield runs along it
        let c = compute(&v, &TABLE, 16.0 / 9.0, &[], &bounds, &[]);
        let vp = c.view * c.projection;
        let (top, bottom) = (vp.transform(Vec3::new(500.0, 0.0, 300.0)), vp.transform(Vec3::new(500.0, 2000.0, 100.0)));
        assert!((top.x.abs() - 1.0).abs() < 1e-3 && (bottom.x.abs() - 1.0).abs() < 1e-3 && top.x * bottom.x < 0.0, "{top:?} {bottom:?}");
    }
}
