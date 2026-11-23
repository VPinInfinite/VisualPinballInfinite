// Follows Visual Pinball X (license:GPLv3+), commit af26b2d (10.8.1 pre-release build 5436), in
// `vpx_depth_bounds`: a primitive's GetBoundingVertices (src/parts/primitive.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later. Ours otherwise.

//! What the renderer draws: the table's meshes in one vertex buffer, a draw
//! list, materials and decoded images, built from `vpin`'s export.
//! Moving parts get a *group*: each frame their meshes are turned or shifted
//! from where the export placed them, by the state the game thread reports.

use std::collections::HashMap;

use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;

use crate::gltf::{AlphaMode, Document};
use crate::math::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// How a moving part's meshes follow the physics.
#[derive(Clone, Copy, Debug)]
pub enum Motion {
    /// Turns about the vertical axis through `center`; the export drew it at
    /// `start_angle` (radians).
    Flipper { center: Vec3, start_angle: f32 },
    /// Turns about its own horizontal axis through `pivot`; `rotation` is the
    /// part's direction on the table in degrees, `sign` VPX's direction of
    /// turning (a one-way gate's wire and a spinner's plate turn backwards).
    Axle { pivot: Vec3, rotation: f32, sign: f32 },
    /// Slides along the table's y axis.
    Plunger,
}

#[derive(Clone, Debug)]
pub struct Group {
    /// The part, as an index into `vpx.gameitems` (the physics' part id).
    pub part: u32,
    pub motion: Motion,
}

#[derive(Clone, Debug)]
pub struct Draw {
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub material: usize,
    /// Index into `Scene::groups`, or `None` for a part that never moves.
    pub group: Option<usize>,
    /// The part, as an index into `vpx.gameitems`, if the node names one.
    pub part: Option<u32>,
    pub visible: bool,
    /// For sorting transparent draws back to front.
    pub center: Vec3,
    /// A light's glow on the table (its `_insert` mesh), as an index into
    /// `Scene::lights`: drawn added to what's below, only while lit.
    pub glow: Option<usize>,
    /// A light's bulb (`_bulb`), which glows while lit.
    pub bulb: Option<usize>,
    /// A light's bulb or socket (`_bulb`, `_socket`): drawn while its `ShowBulbMesh` is on
    /// (VPX's `Light::Render` draws both in its bulb block).
    pub bulb_piece: Option<usize>,
    /// A wall's sides, which keep the table's setting when a script
    /// changes the wall's `Visible` (that is its top).
    pub wall_side: bool,
    /// A bumper's ring or skirt, which the bumper's animation moves.
    pub bumper: Option<BumperPiece>,
    /// Which of a bumper's pieces it is, of `anim::BUMPER_PIECES`: scripts show and hide each.
    /// (A piece the table file hides is never built, so a script can't show it yet.)
    pub bumper_piece: Option<u8>,
    /// Which of the part's images and materials it shows, as scripts change them.
    pub slot: LookSlot,
    /// Where VPX sorts it among see-through parts: far first by the depth
    /// bias less this point's height (`RenderDevice::DrawMesh`).
    pub sort_point: Vec3,
    pub depth_bias: f32,
    /// How far its lighting is switched off in the table file (VPX's
    /// `BlendDisableLighting` of primitives, targets and walls).
    pub disable_lighting: f32,
    /// How far light from below is kept off it (1: none comes through; VPX's
    /// `DisableLightingBelow`): a primitive's, target's or wall's own, 1 for
    /// those VPX draws ahead of time (static), 0 for every other part.
    pub disable_lighting_below: f32,
    /// VPX keeps that whatever the part says (a part drawn ahead of time,
    /// or one without the setting), so a script's
    /// `BlendDisableLightingFromBelow` changes nothing.
    pub below_fixed: bool,
    /// The table file's material for its look, as VPX reads it to decide
    /// blending (`Shader::SetMaterial`); None where the part has no
    /// material of its own for this mesh (the export's choice stands).
    pub material_alpha: Option<MaterialAlpha>,
    /// The material VPX draws this mesh with in VPX's look: the table file's
    /// for its slot or piece, or one of VPX's own (a light's bulb and
    /// socket, a spinner's bracket); None where we don't know VPX's (the
    /// export's material is read instead).
    pub shading: Option<crate::look::Shading>,
    /// Drawn see-through, it still writes depth, as VPX's parts do; a
    /// primitive may turn that off (its `DepthMask`).
    pub depth_mask: bool,
    /// A primitive's normal map (an index into `Scene::images`), read as
    /// plain values, and, for one drawn in the primitive's own space, the
    /// rows taking those normals to the table's (VPX's inverse transpose of
    /// its matrix); else the map is in tangent space.
    pub normal_map: Option<usize>,
    pub object_normal: Option<[[f32; 3]; 3]>,
    /// The mirror it shows (an index into `Scene::mirrors`) and how strongly
    /// (VPX's reflection strength): the playfield shows the playfield's at
    /// the table's strength, a primitive the one it names.
    pub mirror: Option<(usize, f32)>,
    /// A primitive's reflection strength, for a mirror its script names
    /// (VPX's `ReflectionStrength`, 1 unless the table says).
    pub mirror_strength: f32,
    /// A primitive's rows taking its own normals to the table's (its
    /// matrix's inverse transpose; None for other parts and a primitive in a
    /// render group). VPX turns a mirror's plane normal by them before
    /// weighing how much the part faces it (`Primitive::Render`, the model's
    /// `GetModelViewInverseTranspose`), so a turned primitive faces its
    /// mirror less: a playfield mesh stood up and flipped, as Tales from the
    /// Crypt's, not at all.
    pub normal_rows: Option<[[f32; 3]; 3]>,
    /// The refraction a primitive shows (an index into `Scene::refractions`;
    /// VPX's `RefractionProbe`), and how thick it is (`RefractionThickness`).
    pub refraction: Option<usize>,
    pub refraction_thickness: f32,
    /// Drawn into mirrors (VPX's `ReflectionEnabled`, on unless the table
    /// turns it off; never kickers).
    pub reflected: bool,
    /// A part VPX draws ahead, once (its static prepass): before every moving
    /// part, bulb halo and flasher, see-through or not.
    pub still: bool,
    /// The playfield (`playfield_mesh`, the table's or VPX's own), which VPX
    /// draws before everything else, blended or not.
    pub playfield: bool,
    /// A kicker's plate, which VPX draws before the other moving parts, over
    /// what's there, at the depth of a point 30 below (`Kicker::Render`'s
    /// `kickerBoolean`): the hole the kicker sits in.
    pub kicker_plate: bool,
    /// How VPX reads the part's picture.
    pub filter: PartFilter,
    /// Where VPX sorts it: with the solid parts or the see-through ones, by
    /// the flag each part's `Render` hands `DrawMesh` (VPX's legacy one,
    /// which `RenderPass::SortCommands` reads), not by whether it blends.
    pub sorting: Sorting,
    /// Which faces VPX draws (the cull mode each part's `Render` sets).
    pub culling: Culling,
    /// A primitive scaled to zero along an axis: VPX can't invert its
    /// matrix, so its normals aren't numbers there, and lit, it comes out
    /// black in VPX's picture (unlit, it shows as usual).
    pub degenerate: bool,
    /// One frame of a plunger drawn as VPX draws it (our own shapes): this
    /// frame and how many there are; shown only while the plunger's position
    /// picks it.
    pub frame: Option<(u32, u32)>,
    /// The light it's a light map of (a primitive's `LightMap`), an index
    /// into `Scene::light_parts`: its ball shadows fall on it.
    pub lightmap: Option<usize>,
}

/// VPX's legacy flag for a mesh, by part (each part's `Render`, its
/// `DrawMesh` calls): a part marked see-through is sorted far first with the
/// bulb halos and flashers, drawn blended or not; one marked solid is sorted
/// with the solid parts, by shader, then near first, blended or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sorting {
    /// Rubbers, bumpers, flippers, gates, spinners, triggers, kickers,
    /// plungers, lights' sockets, walls showing no see-through material, the
    /// playfield.
    Solid,
    /// Decals on the table, lights' bulbs, walls showing a see-through
    /// material (`Surface::m_isDynamic`).
    SeeThrough,
    /// By its material's opacity switch: ramps, targets.
    ByMaterial,
    /// By its material's opacity switch unless it's drawn ahead (static),
    /// and always when it adds its light: primitives.
    ByMaterialUnlessStill,
}

/// Which of a mesh's faces VPX draws, by part (the cull mode each part's
/// `Render` sets): VPX's default drops the faces turned away from the camera
/// (`CULL_CCW`), and in a mirror's pass the other way round, the mirror
/// reversing them (`RenderProbe::DoRenderReflectionProbe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Culling {
    /// Both faces always: ramps ("thinwalled"), wire triggers, plain kicker
    /// holes.
    None,
    /// The faces turned towards the camera (VPX's default): flippers, gates,
    /// spinners, rubbers, targets, plungers, other triggers and kickers,
    /// lights' meshes, bumpers' rings, decals.
    Front,
    /// Both faces while its material is see-through, else the front ones: a
    /// bumper's base, cap and skirt.
    BothWhenSeeThrough,
    /// A wall's top: the front faces only while the wall is drawn each frame
    /// (a see-through material on it, `Surface::m_isDynamic`) and its own
    /// material isn't see-through; else both.
    WallTop { dynamic: bool },
    /// A wall's side: as its top, and both faces while its top shows too.
    WallSide { dynamic: bool, top_visible: bool },
    /// A primitive: both faces without a depth mask; with its back faces on
    /// and a see-through material, the back faces drawn first, then the
    /// front ones (two passes); else the front ones.
    Primitive { depth_mask: bool, backfaces: bool },
}

/// A primitive's animation frames (VPX's `m_animationFrames`), with what it
/// takes to draw one: the vertices it fills in `Scene::vertices`, its
/// picture places, and how VPX places it on the table.
#[derive(Clone, Debug)]
pub struct PrimitiveAnimation {
    pub part: u32,
    pub base_vertex: u32,
    pub frames: Vec<Vec<crate::prim_anim::FrameVertex>>,
    pub uvs: Vec<[f32; 2]>,
    pub matrix: physics_compat::math::Matrix3D,
    /// Drawn in a render group: its normals turn by the matrix itself.
    pub grouped: bool,
}

impl PrimitiveAnimation {
    /// The vertices at `frame`, placed on the table as the scene keeps them
    /// (VPX blends the frames in the primitive's own space, then places them).
    pub fn vertices(&self, frame: f32) -> Vec<Vertex> {
        let local = crate::prim_anim::blend(&self.frames, frame);
        let mesh = crate::shapes::Mesh {
            positions: local.iter().map(|v| [v[0], v[1], v[2]]).collect(),
            normals: local.iter().map(|v| [v[3], v[4], v[5]]).collect(),
            uvs: Vec::new(),
            indices: Vec::new(),
        };
        let placed = crate::shapes::primitive::place_with(&self.matrix, mesh, self.grouped);
        placed.positions.iter().zip(&placed.normals).zip(&self.uvs).map(|((&position, &normal), &uv)| Vertex { position, normal, uv }).collect()
    }
}

/// How VPX reads a part's picture: most parts by the table's default sampler
/// (anisotropic since VPX forces it, repeating); a ramp and a decal trilinear
/// whatever that default, as their `Render` sets it (`SetTexture(...,
/// SF_TRILINEAR, ...)`) — a ramp clamped when its picture wraps along it, else
/// repeating; a decal mirrored.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PartFilter {
    #[default]
    Default,
    Trilinear,
    TrilinearClamp,
    TrilinearMirror,
}

/// A plane mirror: VPX's plane reflection probe (`RenderProbe`), whose
/// picture is the table above its plane seen mirrored, drawn every frame for
/// the parts that show it. Every table has the playfield's, first.
#[derive(Clone, Debug, PartialEq)]
pub struct Mirror {
    pub name: String,
    /// The plane, `n . p + d = 0`: what lies on its normal's side is mirrored.
    pub plane: [f32; 4],
    /// VPX's roughness (0 to 12), how much its picture is blurred: not yet.
    pub roughness: u32,
    /// What it mirrors, by VPX's reflection modes: 0 nothing, 1 balls, 2
    /// parts that never move, 3 those and balls, 4 and up everything.
    pub mode: u32,
    /// Light maps left out (lights' glows); always so for the playfield's.
    pub no_light_maps: bool,
}

impl Mirror {
    /// VPX's name for the playfield's mirror.
    pub const PLAYFIELD: &'static str = "Playfield Reflections";

    /// A plane reflection probe from vpin's description of it (vpin 0.38
    /// keeps a probe's fields to itself; its debug text has them all), None
    /// for other probes (refraction) or text that doesn't read.
    fn from_probe_text(text: &str) -> Option<Mirror> {
        let (kind, name, rest) = probe_text(text)?;
        if kind != "PlaneReflection" {
            return None;
        }
        let plane = after(rest, "reflection_plane: ")?;
        let coordinate = |key: &str| -> Option<f32> { word(after(plane, key)?).parse().ok() };
        let mode = match word(after(rest, "reflection_mode: ")?) {
            "None" => 0,
            "Balls" => 1,
            "Static" => 2,
            "StaticNBalls" => 3,
            "StaticNDynamic" => 4,
            "Dynamic" => 5,
            "Unknown" => 6,
            other => other.strip_prefix("Other(")?.strip_suffix(')')?.parse().ok()?,
        };
        Some(Mirror {
            name,
            plane: [coordinate(" x: ")?, coordinate(" y: ")?, coordinate(" z: ")?, coordinate(" w: ")?],
            roughness: word(after(rest, "roughness: ")?).parse().ok()?,
            mode,
            no_light_maps: word(after(rest, "disable_light_reflection: ")?) == "Some(true)",
        })
    }

    /// The table's mirrors as VPX sets them up when loading it (`PinTable::LoadData`,
    /// `Player`): the playfield's first, there even when the file has none (tables saved
    /// before 10.8, mirroring everything), its plane the playfield's, without light maps.
    pub(crate) fn of_table(vpx: &VPX) -> Vec<Mirror> {
        let mut mirrors = vec![Mirror { name: Mirror::PLAYFIELD.into(), plane: [0.0, 0.0, 1.0, 0.0], roughness: 0, mode: 5, no_light_maps: true }];
        for probe in vpx.gamedata.render_probes.iter().flatten() {
            let Some(m) = Mirror::from_probe_text(&format!("{:?}", probe.render_probe)) else { continue };
            if m.name.eq_ignore_ascii_case(Mirror::PLAYFIELD) {
                (mirrors[0].roughness, mirrors[0].mode) = (m.roughness, m.mode);
            } else if !mirrors.iter().any(|o| o.name.eq_ignore_ascii_case(&m.name)) {
                mirrors.push(m);
            }
        }
        mirrors
    }
}

/// The text after a key in vpin's description of something, and its first word.
fn after<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    s.find(key).map(|i| &s[i + key.len()..])
}

fn word(s: &str) -> &str {
    s.split([',', ' ', '}']).next().unwrap_or("")
}

/// A render probe's type, its name (as Rust quotes it) and the text after
/// that, from vpin's description of it.
fn probe_text(text: &str) -> Option<(&str, String, &str)> {
    let kind = word(after(text, "type_: ")?);
    let quoted = after(text, "name: \"")?;
    let mut name = String::new();
    let mut chars = quoted.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => name.push(chars.next()?.1),
            '"' => return Some((kind, name, &quoted[i + 1..])),
            c => name.push(c),
        }
    }
    None
}

/// A refraction probe: VPX's screen-space transparency probe (`RenderProbe`
/// of type `SCREEN_SPACE_TRANSPARENCY`), the table as drawn up to the first
/// part showing it in a frame, its colour and depth, blurred by its
/// roughness, seen bent through the parts that show it.
#[derive(Clone, Debug, PartialEq)]
pub struct Refraction {
    pub name: String,
    /// VPX's roughness (0 to 12), how much its picture is blurred.
    pub roughness: u32,
}

impl Refraction {
    fn from_probe_text(text: &str) -> Option<Refraction> {
        let (kind, name, rest) = probe_text(text)?;
        if kind != "ScreenSpaceTransparency" {
            return None;
        }
        Some(Refraction { name, roughness: word(after(rest, "roughness: ")?).parse().ok()? })
    }

    /// The table's refraction probes, the first of a name (ignoring case) kept.
    pub(crate) fn of_table(vpx: &VPX) -> Vec<Refraction> {
        let mut all: Vec<Refraction> = Vec::new();
        for probe in vpx.gamedata.render_probes.iter().flatten() {
            let Some(r) = Refraction::from_probe_text(&format!("{:?}", probe.render_probe)) else { continue };
            if !all.iter().any(|o| o.name.eq_ignore_ascii_case(&r.name)) {
                all.push(r);
            }
        }
        all
    }
}

/// How far a primitive keeps light from below off, as VPX has it once the
/// table is loaded (`DisableLightingBelow`, 1 unless the file says): for a
/// table saved before 10.8, VPX's load sets it to 1 when the primitive's
/// material is opaque — its opacity off or at 1, or a material the table
/// lacks (VPX's dummy) — since VPX then dropped the picture's alpha
/// (`PinTable::LoadGameFromStorage`, build 5436).
pub(crate) fn primitive_below(vpx: &VPX, materials: &crate::look::Materials, pr: &vpin::vpx::gameitem::primitive::Primitive) -> f32 {
    let below = pr.disable_lighting_below.unwrap_or(1.0);
    let opaque = materials.find(&pr.material).is_none_or(|k| {
        let m = &materials.all[k as usize];
        !m.opacity_active || m.opacity == 1.0
    });
    if vpx.version.u32() < 1080 && below != 1.0 && opaque { 1.0 } else { below }
}

/// The material VPX's part draws a mesh with: its slot's (`file_materials`),
/// a bumper piece's own, the table's playfield material for the playfield
/// (`Primitive::Render` for `playfield_mesh`, and VPX's implicit playfield),
/// or VPX's built-in ones. A name the table lacks is VPX's dummy material.
fn file_shading(item: Option<&GameItemEnum>, slot: LookSlot, node: &str, materials: &crate::look::Materials, playfield: &str) -> Option<crate::look::Shading> {
    use crate::look::Shading;
    let named = |name: &str| materials.find(name).map_or_else(Shading::dummy, |k| materials.all[k as usize].shading());
    let lower = node.to_lowercase();
    if lower == "playfield_mesh" {
        return Some(named(playfield));
    }
    match item? {
        GameItemEnum::Bumper(b) if lower.ends_with("base") => Some(named(&b.base_material)),
        GameItemEnum::Bumper(b) if lower.ends_with("socket") => Some(named(&b.socket_material)),
        GameItemEnum::Bumper(b) if lower.ends_with("cap") => Some(named(&b.cap_material)),
        GameItemEnum::Bumper(b) if lower.ends_with("ring") => Some(b.ring_material.as_deref().filter(|m| !m.is_empty()).map_or_else(Shading::bumper_ring, named)),
        GameItemEnum::Light(_) if lower.ends_with("_bulb") => Some(Shading::light_bulb()),
        GameItemEnum::Light(_) if lower.ends_with("_socket") => Some(Shading::light_socket()),
        GameItemEnum::Spinner(_) if lower.ends_with("bracket") => Some(Shading::spinner_bracket()),
        GameItemEnum::Gate(g) => Some(named(&g.material)),
        item => {
            let (main, side) = crate::world::file_materials(item);
            let name = match slot {
                LookSlot::Side => side,
                LookSlot::Main => main,
                LookSlot::Fixed => None,
            }?;
            Some(named(name))
        }
    }
}

/// What VPX's `Shader::SetMaterial` reads from a material to decide whether
/// a part is drawn blended: only when its opacity is active, and then when
/// its picture has alpha or its opacity is below 0.999. Otherwise it's
/// drawn solid, its picture's alpha only cutting out texels at or below
/// the picture's alpha test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialAlpha {
    pub opacity_active: bool,
    pub opacity: f32,
}

/// Which of a part's two looks a mesh shows, as scripts change them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookSlot {
    /// Its main look: a primitive, a wall's top, a flipper's body.
    Main,
    /// Its second: a wall's sides, a flipper's rubber.
    Side,
    /// Neither: a spinner's or gate's bracket, a light's or bumper's pieces.
    Fixed,
}

/// A flasher, as VPX's `Flasher::RenderSetup` builds it: its outline
/// flat at height 0, filled with triangles, which each frame turns about
/// its centre and lifts to its height (`Flasher::Render`).
#[derive(Clone, Debug)]
pub struct FlasherLook {
    pub part: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    /// Its outline's bounds' centre when play starts.
    pub center: [f32; 2],
    /// How it starts, for a frame drawn before the game's first.
    pub start: crate::look::FlasherNow,
    /// It shows the score display (VPX's DMD render mode).
    pub dmd: bool,
    /// The light it's a light map of, an index into `Scene::light_parts`.
    pub lightmap: Option<usize>,
}

impl FlasherLook {
    /// VPX's matrix for the flasher now: about its centre, turned about z,
    /// then y, then x, and lifted to its height where a script moved it.
    pub fn transform(&self, now: &crate::look::FlasherNow) -> Mat4 {
        let [cx, cy] = self.center;
        Mat4::translate(-cx, -cy, 0.0)
            * Mat4::rotate_z(now.rot[2].to_radians())
            * Mat4::rotate_y(now.rot[1].to_radians())
            * Mat4::rotate_x(now.rot[0].to_radians())
            * Mat4::translate(cx + now.offset[0], cy + now.offset[1], now.height)
    }
}

/// The moving pieces of a bumper (VPX's `Bumper::UpdateAnimation`).
#[derive(Clone, Copy, Debug)]
pub enum BumperPiece {
    /// Moves down and back up by the ring's offset.
    Ring,
    /// Tilts about `pivot` (the bumper's centre, 5 above its base) when hit;
    /// the tilt turns the unit skirt, before its `radius` and `height_scale`.
    Skirt { pivot: Vec3, radius: f32, height_scale: f32 },
}

impl BumperPiece {
    /// The matrix that moves the exported mesh to where the animation has it:
    /// `ring` the ring's offset, `rot` the skirt's tilt in degrees (x, y).
    pub fn transform(&self, ring: f32, rot: [f32; 2]) -> Mat4 {
        match *self {
            BumperPiece::Ring => Mat4::translate(0.0, 0.0, ring),
            BumperPiece::Skirt { pivot, radius, height_scale } => {
                if rot == [0.0, 0.0] || radius == 0.0 || height_scale == 0.0 {
                    return Mat4::IDENTITY;
                }
                // VPX turns the unit skirt by its orientation, then the tilt about y and x,
                // then scales it: undo the scale, tilt, scale again
                Mat4::translate(-pivot.x, -pivot.y, -pivot.z)
                    * Mat4::scale(1.0 / radius, 1.0 / radius, 1.0 / height_scale)
                    * Mat4::rotate_y(rot[1].to_radians())
                    * Mat4::rotate_x(rot[0].to_radians())
                    * Mat4::scale(radius, radius, height_scale)
                    * Mat4::translate(pivot.x, pivot.y, pivot.z)
            }
        }
    }
}

/// A light, as VPX's light shader needs it.
#[derive(Clone, Copy, Debug)]
pub struct LightInfo {
    pub part: u32,
    /// Where the glow is brightest: the light's center on its surface.
    pub center: Vec3,
    pub falloff_radius: f32,
    pub falloff_power: f32,
    /// Colour at the center and at the edge (`color2`), 0..1.
    pub color: [f32; 3],
    pub color2: [f32; 3],
    pub intensity: f32,
    /// On (1), off (0), anything between, or blinking (2), at the start.
    pub state: f32,
    /// A bulb light, whose halo floats `halo_height` above its surface and
    /// brightens what's below; else a classic light (an insert).
    pub bulb: bool,
    pub halo_height: f32,
    /// Where the light shines from, above its surface (VPX's `m_height`).
    pub height: f32,
    /// How much a bulb brightens what's below (1) rather than adds (0).
    pub modulate_vs_add: f32,
    /// A classic light with no picture of its own, which VPX draws as a
    /// shape in its colour whether lit or not (light.cpp's
    /// `light_without_texture`).
    pub plain: bool,
    /// How much of a bulb's light passes up through see-through parts above
    /// it (VPX's transmission scale, its bulb light buffer).
    pub transmission: f32,
    /// The light shines on balls near it (VPX's `ShowReflectionOnBall`), and
    /// the radius of its bulb, by which VPX finds the nearest.
    pub on_ball: bool,
    pub mesh_radius: f32,
    /// Where VPX sorts its halo and bulb among see-through parts (its depth bias).
    pub depth_bias: f32,
    /// A ball between it and what it lights casts a shadow (VPX's raytraced
    /// ball shadows): on its halo, and on its light maps.
    pub ball_shadows: bool,
    /// The material of what it sits on (VPX's `m_surfaceMaterial`, from
    /// `GetSurfaceMaterial`: the wall's top or the ramp's named as its
    /// surface, else the playfield's), which shades a classic light's shape.
    pub surface_shading: crate::look::Shading,
    /// A classic light whose picture is the one of what it sits on (VPX's
    /// `m_surfaceTexture == offTexel`, from `GetSurfaceImage`), which VPX
    /// leaves out while it gives no light unless it's in image mode; any
    /// other picture it draws lit or not.
    pub picture_is_surface: bool,
    /// Its picture drawn as it is, unlit (VPX's image mode, "passthrough").
    pub image_mode: bool,
}

/// A score reel unit, as VPX draws it (`DispReel::RenderSetup` and
/// `Render`): flat over the picture, placed on VPX's 1000 × 750 backdrop
/// grid, which covers the whole window.
#[derive(Clone, Debug)]
pub struct ReelLook {
    pub part: u32,
    /// Index into `Scene::images`.
    pub image: usize,
    /// The first reel's top left, as fractions of the window.
    pub x: f32,
    pub y: f32,
    /// Each reel's size, and the space after it, as fractions of the window.
    pub width: f32,
    pub height: f32,
    pub spacing_x: f32,
    /// Each digit's place in the image: u0, v0, u1, v1.
    pub digits: Vec<[f32; 4]>,
}

/// A light drawn on the backdrop (VPX's `Light` with `m_desktopBackdrop`):
/// its outline filled flat over the window, on VPX's 1000 × 750 grid.
#[derive(Clone, Debug)]
pub struct BackdropLightLook {
    pub part: u32,
    /// Index into `Scene::light_parts`, for its brightness and colours.
    pub light: usize,
    /// The outline's points, on the backdrop grid.
    pub points: Vec<[f32; 2]>,
    /// Three indices into `points` per triangle.
    pub triangles: Vec<u32>,
    /// The light's image (shown unlit, VPX's `offTexel`), as an index into
    /// `Scene::images`; for a classic light only.
    pub image: Option<usize>,
    /// A bulb light: its glow blends over what's below rather than replacing it.
    pub bulb: bool,
    /// For a bulb: how much it brightens what's below (1) rather than adds (0).
    pub modulate_vs_add: f32,
    pub center: [f32; 2],
    pub falloff: f32,
    pub falloff_power: f32,
}

/// What VPX shows behind the table: the backdrop image over the whole
/// window, or its colour (`Pin3D::DrawBackground`).
#[derive(Clone, Copy, Debug, Default)]
pub struct BackdropLook {
    /// Index into `Scene::images`.
    pub image: Option<usize>,
    /// The colour, 0..1 (sRGB), when there is no image.
    pub color: [f32; 3],
    /// How bright the image is (night and day).
    pub intensity: f32,
}

/// How the table's picture becomes the screen's colours in VPX's look (VPX's
/// tonemapping): the table's tone mapper and exposure, and its colour grade;
/// and the light VPX's materials take from the table: its environment
/// picture and the two point lights' emission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    /// VPX's `ToneMapper`: 0 Reinhard (the default), 1 AgX, 2 Filmic, 3
    /// Neutral, 4 AgX punchy.
    pub mapper: u32,
    pub exposure: f32,
    /// The colour grade picture, in `Scene::images` (VPX's `ColorGradeImage`).
    pub color_grade: Option<usize>,
    /// The environment picture, in `Scene::images`: the table's, else our
    /// default room (VPX's own default picture predates its GPL rule).
    pub env: Option<usize>,
    /// The table chose no environment picture (or one it doesn't have).
    pub env_default: bool,
    /// The environment's emission scale times the table's day and night
    /// scale (VPX's `fenvEmissionScale_TexWidth.x`).
    pub env_emission: f32,
    /// The first light's colour times the light emission and day and night
    /// scales: VPX gives both point lights this emission.
    pub light_emission: [f32; 3],
    /// The ambient colour times the day and night scale, and the lights'
    /// range: VPX hands them to its ball shader (`cAmbient_LightRange`).
    pub ambient: [f32; 3],
    pub light_range: f32,
    /// VPX's bloom strength for the table (0: none).
    pub bloom_strength: f32,
    /// VPX's ambient occlusion strength (`m_AOScale`), 0 when the table
    /// doesn't ask for it (`EnableAO`).
    pub ao_scale: f32,
    /// The ball, as VPX's look draws it: the table's ball picture (else
    /// VPX's default, a high-range environment) and whether it's a sphere
    /// map, its scratches picture and whether that's a decal, how much it
    /// mirrors the playfield, and how bright lights shine on it.
    pub ball_image: Option<usize>,
    pub ball_spherical: bool,
    pub ball_decal: Option<usize>,
    pub ball_decal_mode: bool,
    pub ball_reflection: f32,
    pub ball_bulb_scale: f32,
    /// 1 / the table's width and height (VPX's `invTableRes`).
    pub ball_inv_table: [f32; 2],
}

impl Default for Tone {
    fn default() -> Self {
        Tone {
            mapper: 0,
            exposure: 1.0,
            color_grade: None,
            env: None,
            env_default: false,
            env_emission: 1.0,
            light_emission: [0.0; 3],
            ambient: [0.0; 3],
            light_range: 0.0,
            bloom_strength: 0.0,
            ao_scale: 0.0,
            ball_image: None,
            ball_spherical: true,
            ball_decal: None,
            ball_decal_mode: false,
            ball_reflection: 1.0,
            ball_bulb_scale: 1.0,
            ball_inv_table: [1.0, 1.0],
        }
    }
}

/// The table's picture named `name` as half floats, if it holds light past
/// white (an EXR or HDR file): VPX keeps such pictures' range.
fn high_range(vpx: &VPX, name: &str) -> Option<HdrImage> {
    let i = crate::look::image_named(vpx, name)?;
    let decoded = vpx.images[i].decode().ok()?;
    if !matches!(decoded, image::DynamicImage::ImageRgb32F(_) | image::DynamicImage::ImageRgba32F(_)) {
        return None;
    }
    let f = decoded.to_rgba32f();
    let rgba = f.as_raw().iter().map(|&v| half::f16::from_f32(v.clamp(0.0, 65504.0))).collect();
    Some(HdrImage { width: f.width(), height: f.height(), rgba })
}

/// Our default room, for tables that name no environment picture (or one they don't hold).
/// VPX's own default (`EnvMap.webp`, an airport hall) can't be taken: it dates from 2014,
/// before VPX's GPL rule. Ours is Poly Haven's Rostock-Laage Airport, a hall like VPX's
/// (CC0, by Greg Zaal), made into a picture like VPX's by `tools/room-picture`: 512 x 256, an
/// ordinary picture, its light on parts facing up and facing down and its average colour those
/// of VPX's room. Laid out as VPX reads an environment: across, the direction
/// around the table; down, from straight up to straight down.
const DEFAULT_ROOM: &[u8] = include_bytes!("../assets/vpi/room/rostock-laage-airport.png");

fn default_environment() -> Image {
    // for checks only: another room picture in ours' place (`VPI_ROOM_PICTURE`), so a table
    // that falls back to VPX's built-in room can be compared with VPX everywhere else, VPX's
    // own picture read from its build (it can't be shipped)
    if let Some(picture) = std::env::var_os("VPI_ROOM_PICTURE").and_then(|p| image::open(p).ok()) {
        return Image::new(picture.into_rgba8());
    }
    let picture = image::load_from_memory(DEFAULT_ROOM).expect("the built-in room picture decodes");
    Image::new(picture.into_rgba8())
}

/// How a light shines in this frame.
#[derive(Clone, Copy, Debug)]
pub struct LightNow {
    /// VPX's intensity, 0 when off.
    pub intensity: f32,
    /// Colour at the center and at the edge, 0..1.
    pub color: [f32; 3],
    pub color2: [f32; 3],
    /// Its place, halo height and blend, which scripts change.
    pub live: LightLive,
}

/// What scripts change of how a light draws, which VPX reads live as it draws: where its glow is
/// brightest (`X`, `Y`; its shape stays where the table put it), a bulb's halo height, its blend,
/// and how much of its light passes up through see-through parts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightLive {
    pub center: [f32; 2],
    pub halo_height: f32,
    pub modulate_vs_add: f32,
    pub transmission: f32,
    /// How far its glow reaches and how it fades, and where it sorts among see-through parts.
    pub falloff: f32,
    pub falloff_power: f32,
    pub depth_bias: f32,
    /// Whether it shines on balls, and casts their shadows.
    pub on_ball: bool,
    pub ball_shadows: bool,
    /// Whether its bulb and socket show (`ShowBulbMesh`; they're made only when the file shows them).
    pub bulb_mesh: bool,
}

impl LightLive {
    /// As the table file sets them.
    pub fn of(l: &LightInfo) -> LightLive {
        LightLive {
            center: [l.center.x, l.center.y],
            halo_height: l.halo_height,
            modulate_vs_add: l.modulate_vs_add,
            transmission: l.transmission,
            falloff: l.falloff_radius,
            falloff_power: l.falloff_power,
            depth_bias: l.depth_bias,
            on_ball: l.on_ball,
            ball_shadows: l.ball_shadows,
            bulb_mesh: true,
        }
    }
}

impl LightNow {
    /// The light as the table starts it (blinking counts as on).
    pub fn at_start(l: &LightInfo) -> LightNow {
        let on = if l.state == crate::light::BLINKING { 1.0 } else { l.state };
        LightNow { intensity: l.intensity * on, color: l.color, color2: l.color2, live: LightLive::of(l) }
    }

    /// The light as the game left it: its brightness, its colours (`COLORREF`s, red in the low
    /// byte) and their tint, and what the script changed of how it draws.
    pub fn from_game(intensity: f32, colors: [u32; 2], tint: [f32; 3], live: LightLive) -> LightNow {
        let rgb = |c: u32| {
            let channel = |k: u32, t: f32| ((c >> (8 * k)) & 0xFF) as f32 / 255.0 * t;
            [channel(0, tint[0]), channel(1, tint[1]), channel(2, tint[2])]
        };
        LightNow { intensity, color: rgb(colors[0]), color2: rgb(colors[1]), live }
    }
}

#[derive(Clone, Debug)]
pub struct Material {
    pub base_color: [f32; 4],
    /// Index into `Scene::images`.
    pub image: Option<usize>,
    pub blend: bool,
    pub alpha_cutoff: Option<f32>,
    pub emissive: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
}

/// A picture of light past white, linear, as half floats (red, green, blue, alpha).
pub struct HdrImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<half::f16>,
}

#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// The texels, 8 bits a channel, sRGB; none for a high-range picture.
    pub rgba: Vec<u8>,
    /// Some texel isn't fully opaque (VPX's `!IsOpaque()`).
    pub has_alpha: bool,
    /// A picture of light past white (an EXR or HDR file), linear, as half
    /// floats (red, green, blue, alpha 1).
    pub hdr: Option<Vec<half::f16>>,
    /// Not decoded yet: the table's picture it comes from and how it's read.
    /// The renderer decodes it as it uploads it and lets the texels go, so a
    /// big table's pictures never all sit decoded in memory at once; its size
    /// is the table file's until then.
    pub pending: Option<(usize, Decode)>,
}

/// How a picture is decoded (`table_meshes`): as a part's picture, from the
/// bytes the export packed (`decode_material`), or as a script swaps it in,
/// by `vpin`'s decoding (`decode_script`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decode {
    Material,
    Script,
}

impl Image {
    pub fn new(rgba: image::RgbaImage) -> Image {
        let has_alpha = rgba.pixels().any(|p| p[3] < 255);
        Image { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw(), has_alpha, hdr: None, pending: None }
    }

    /// A picture of the table's (`vpx.images[t]`), decoded later, as `how`.
    pub fn pending(vpx: &VPX, t: usize, how: Decode) -> Image {
        let source = &vpx.images[t];
        Image { width: source.width, height: source.height, rgba: Vec::new(), has_alpha: false, hdr: None, pending: Some((t, how)) }
    }

    /// The table's picture `source` decoded `how`; None when it can't be.
    pub fn decode(source: &vpin::vpx::image::ImageData, how: Decode) -> Option<Image> {
        match how {
            Decode::Material => crate::table_meshes::decode_material(source),
            Decode::Script => crate::table_meshes::decode_script(source),
        }
    }

    /// A decoded picture as VPX keeps it (`BaseTexture::CreateFromFile`): one
    /// of light past white in half floats, linear, its alpha dropped and
    /// opaque (VPX's `RGB_FP16`), every other in 8 bits.
    pub fn from_decoded(decoded: image::DynamicImage) -> Image {
        let half = |v: f32| half::f16::from_f32(v.clamp(-65504.0, 65504.0));
        let floats = |w: u32, h: u32, texels: Vec<half::f16>| Image { width: w, height: h, rgba: Vec::new(), has_alpha: false, hdr: Some(texels), pending: None };
        match decoded {
            image::DynamicImage::ImageRgb32F(p) => {
                floats(p.width(), p.height(), p.pixels().flat_map(|t| [half(t[0]), half(t[1]), half(t[2]), half::f16::ONE]).collect())
            }
            image::DynamicImage::ImageRgba32F(p) => {
                floats(p.width(), p.height(), p.pixels().flat_map(|t| [half(t[0]), half(t[1]), half(t[2]), half::f16::ONE]).collect())
            }
            other => Image::new(other.into_rgba8()),
        }
    }

    /// The picture's size in memory.
    pub fn bytes(&self) -> usize {
        self.rgba.len() + self.hdr.as_ref().map_or(0, |h| h.len() * 2)
    }

    /// The texels in 8 bits, a high-range picture's held to 0..1, for what
    /// reads pictures as plain values (normal maps, colour grades).
    pub fn rgba8(&self) -> std::borrow::Cow<'_, [u8]> {
        match &self.hdr {
            Some(h) => h.iter().map(|v| (v.to_f32().clamp(0.0, 1.0) * 255.0).round() as u8).collect::<Vec<u8>>().into(),
            None => self.rgba.as_slice().into(),
        }
    }
}

/// Decodes pictures on up to eight threads at once — on big tables the
/// slowest part of loading — and hands them back in the jobs' order, so the
/// scene comes out as it would decoding them one by one.
fn decode_in_parallel<J: Sync>(jobs: &[J], decode: impl Fn(&J) -> Option<Image> + Sync) -> Vec<Option<Image>> {
    in_parallel(jobs, decode)
}

/// Does each job on up to eight threads at once, and hands the results back
/// in the jobs' order.
pub(crate) fn in_parallel<J: Sync, R: Send>(jobs: &[J], work: impl Fn(&J) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get).clamp(1, 8).min(jobs.len());
    let next = AtomicUsize::new(0);
    let mut out: Vec<Option<R>> = std::iter::repeat_with(|| None).take(jobs.len()).collect();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(job) = jobs.get(k) else { break };
                        done.push((k, work(job)));
                    }
                    done
                })
            })
            .collect();
        for worker in workers {
            for (k, result) in worker.join().unwrap_or_default() {
                out[k] = Some(result);
            }
        }
    });
    out.into_iter().map(|r| r.expect("every job done")).collect()
}

/// A ramp drawn with our own shapes, which a script may raise, lower, widen or narrow
/// (`HeightBottom`, `HeightTop`, `WidthBottom`, `WidthTop`): built again during play, as VPX
/// regenerates its mesh.
#[derive(Clone, Debug)]
pub struct RampShape {
    pub part: u32,
    /// Where its vertices start in the scene's.
    pub base_vertex: u32,
    pub source: crate::shapes::ramp::RampSource,
    pub ctx: crate::shapes::ramp::RampContext,
}

impl RampShape {
    /// Its vertices at `shape` (heights bottom and top, widths bottom and top), in the scene's order.
    pub fn vertices(&self, shape: [f32; 4]) -> Vec<Vertex> {
        let mesh = crate::shapes::ramp::shape_from(&self.ctx, &self.source.with_shape(shape));
        (mesh.positions.iter().zip(&mesh.normals).zip(&mesh.uvs)).map(|((&position, &normal), &uv)| Vertex { position, normal, uv }).collect()
    }

    /// Its heights and widths as the table file has them (bottom and top each).
    pub fn file_shape(&self) -> [f32; 4] {
        let d = &self.source.data;
        [d.height_bottom, d.height_top, d.width_bottom, d.width_top]
    }
}

pub struct Scene {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<Draw>,
    /// The primitives scripts animate, with our own shapes.
    pub animations: Vec<PrimitiveAnimation>,
    /// The ramps scripts may raise or lower, with our own shapes.
    pub ramps: Vec<RampShape>,
    pub materials: Vec<Material>,
    pub images: Vec<Image>,
    /// VPX's look: the table's environment picture when it holds light past
    /// white (an EXR or HDR file), as half floats; `Tone::env` then names
    /// its 8-bit copy, which VPX's look doesn't use.
    pub env_hdr: Option<HdrImage>,
    /// The table's ball picture, likewise, when it holds light past white.
    pub ball_hdr: Option<HdrImage>,
    pub groups: Vec<Group>,
    pub light_parts: Vec<LightInfo>,
    /// The ball mesh: (first index, index count, base vertex), a unit sphere.
    pub ball: (u32, u32, i32),
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    /// The corners of each visible part's own box, as VPX gathers them for
    /// the camera's depth range (`GetBoundingVertices` for
    /// `ComputeNearFarPlane`): a hidden part far away, such as a VR room,
    /// doesn't count.
    pub depth_bounds: Vec<Vec3>,
    /// The table's two light positions.
    pub lights: [Vec3; 2],
    /// The table's plane mirrors, the playfield's first (VPX's reflection probes).
    pub mirrors: Vec<Mirror>,
    /// The table's refraction probes.
    pub refractions: Vec<Refraction>,
    /// The score reels to draw over the picture.
    pub reels: Vec<ReelLook>,
    /// The lights and text boxes on the backdrop.
    pub backdrop_lights: Vec<BackdropLightLook>,
    pub text_boxes: Vec<crate::textbox::TextBoxLook>,
    pub backdrop: BackdropLook,
    /// The table's tonemapping, for VPX's look.
    pub tone: Tone,
    /// The scene's light level, 0 to 1: the table's day and night (`NightDay`) unless the player
    /// chose another (VPX's `SceneLighting`, `set_player_look`).
    pub light_level: f32,
    /// The flashers on the table.
    pub flashers: Vec<FlasherLook>,
    /// For each of the table's images (`vpx.images`), its decoded picture
    /// in `images`: those the meshes and flashers show, and those the
    /// script names, ready for its swaps.
    pub table_images: Vec<Option<usize>>,
    /// For each table image, VPX's alpha test: texels at or below it vanish
    /// (0 to 1; negative when off).
    pub image_alpha_test: Vec<f32>,
    /// The same for each decoded picture in `images`, from the table image
    /// it is (negative when off or unknown).
    pub picture_alpha_test: Vec<f32>,
    /// Our own kicker pictures in `images`, by kind ([`KICKER_PICTURES`]),
    /// once the first kicker of a kind asks.
    kicker_pictures: [Option<usize>; 5],
}

/// Our own pictures for the kinds of kicker VPX draws with a built-in one
/// (`Kicker::RenderSetup`): cup, Williams, Gottlieb, cup 2 and hole. VPX's
/// were drawn before its GPL rule and can't be taken; these are
/// painted from the kickers' meshes by `tools/kicker-pictures`.
const KICKER_PICTURES: [&[u8]; 5] = [
    include_bytes!("../assets/vpi/kickers/cup.png"),
    include_bytes!("../assets/vpi/kickers/williams.png"),
    include_bytes!("../assets/vpi/kickers/gottlieb.png"),
    include_bytes!("../assets/vpi/kickers/cup2.png"),
    include_bytes!("../assets/vpi/kickers/hole.png"),
];

/// VPX's backdrop grid, which spans the window (`EDITOR_BG_WIDTH`, `_HEIGHT`).
const EDITOR_BG_WIDTH: f32 = 1000.0;
const EDITOR_BG_HEIGHT: f32 = 750.0;

/// The export's axes (glTF: y up) to VPX's (z up): a swap of y and z.
fn to_vpx(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], v[1]]
}

const AXES: Mat4 = Mat4 { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };

fn node_matrix(n: &crate::gltf::Node) -> Mat4 {
    // glTF: T * R * S applied to column vectors, i.e. S first; in rows, S * R * T
    Mat4::scale(n.scale[0], n.scale[1], n.scale[2])
        * Mat4::from_quaternion(n.rotation)
        * Mat4::translate(n.translation[0], n.translation[1], n.translation[2])
}

/// The endings `vpin`'s export adds to a part's name for each of its meshes.
const MESH_SUFFIXES: [&str; 18] =
    ["_insert", "_bulb", "_socket", "base", "bracket", "cap", "flat", "kicker", "plate", "ring", "rod", "rubber", "side", "socket", "spring", "tip", "top", "wire"];

/// The part a node draws: by its parent group when it's a piece named after
/// that part (`LeftFlipperRubber` under `LeftFlipper`, even beside a primitive
/// of that name, as Centaur's), else by its name, by its name less one of the
/// export's mesh endings (`L1_insert` is part of `L1`, `Wall1Top` of
/// `Wall1`), or by its parent group. The name comes before the parent
/// otherwise: a part with one mesh sits straight under its editor layer.
fn part_of(name: &str, parent: Option<&str>, parts: &HashMap<String, u32>) -> Option<u32> {
    let lower = name.to_lowercase();
    let parent_part = parent.map(str::to_lowercase).and_then(|p| parts.get(&p).copied().filter(|_| lower.starts_with(&p)));
    parent_part
        .or_else(|| parts.get(&lower).copied())
        .or_else(|| MESH_SUFFIXES.iter().find_map(|s| lower.strip_suffix(s).and_then(|base| parts.get(base).copied())))
        .or_else(|| parent.and_then(|p| parts.get(&p.to_lowercase()).copied()))
}

/// The text of a script's string literals, line by line, leaving out
/// comments (from `'` outside a string).
/// The images the script may swap in: those its strings name, and those
/// whose names start with one of its strings of four letters or more
/// (`"PostIt" & letter`).
pub(crate) fn script_images(vpx: &VPX) -> Vec<usize> {
    let literals: Vec<String> = string_literals(&vpx.gamedata.code.string).into_iter().map(str::to_lowercase).collect();
    let exact: std::collections::HashSet<&str> = literals.iter().map(String::as_str).collect();
    let prefixes: Vec<&str> = literals.iter().map(String::as_str).filter(|l| l.chars().count() >= 4).collect();
    (0..vpx.images.len())
        .filter(|&i| {
            let name = vpx.images[i].name.to_lowercase();
            exact.contains(name.as_str()) || prefixes.iter().any(|p| name.starts_with(p))
        })
        .collect()
}

fn string_literals(code: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for line in code.lines() {
        let mut start = None;
        for (i, c) in line.char_indices() {
            match (c, start) {
                ('"', None) => start = Some(i + 1),
                ('"', Some(s)) => {
                    out.push(&line[s..i]);
                    start = None;
                }
                ('\'', None) => break,
                _ => {}
            }
        }
    }
    out
}

impl Scene {
    /// Decodes one of the table's images by name (VPX ignores case), for
    /// what is drawn beside the meshes.
    fn table_image(&mut self, vpx: &VPX, name: &str) -> Option<usize> {
        if name.is_empty() {
            return None;
        }
        let i = crate::look::image_named(vpx, name)?;
        self.decode_table_image(vpx, i)
    }

    /// Our own picture for a kind of kicker ([`KICKER_PICTURES`]), as one of
    /// the scene's pictures, decoded once a kind; none for a plain hole, which
    /// VPX draws without its picture.
    fn kicker_picture(&mut self, kind: &vpin::vpx::gameitem::kicker::KickerType) -> Option<usize> {
        use vpin::vpx::gameitem::kicker::KickerType;
        let k = match kind {
            KickerType::Cup => 0,
            KickerType::Williams => 1,
            KickerType::Gottlieb => 2,
            KickerType::Cup2 => 3,
            KickerType::Hole => 4,
            _ => return None,
        };
        if let Some(s) = self.kicker_pictures[k] {
            return Some(s);
        }
        let picture = image::load_from_memory(KICKER_PICTURES[k]).ok()?;
        self.images.push(Image::new(picture.into_rgba8()));
        let s = self.images.len() - 1;
        self.kicker_pictures[k] = Some(s);
        Some(s)
    }

    /// The light of a name (as a part's `LightMap` names it), an index into
    /// `light_parts`.
    fn light_named(&self, vpx: &VPX, name: Option<&str>) -> Option<usize> {
        let name = name.filter(|n| !n.is_empty())?;
        self.light_parts.iter().position(|l| vpx.gameitems.get(l.part as usize).is_some_and(|g| g.name().eq_ignore_ascii_case(name)))
    }

    /// `vpx.images[i]` as one of the scene's pictures, once, to decode when
    /// it's uploaded (`Image::pending`); none for a picture without data.
    fn decode_table_image(&mut self, vpx: &VPX, i: usize) -> Option<usize> {
        if let Some(&Some(s)) = self.table_images.get(i) {
            return Some(s);
        }
        let source = &vpx.images[i];
        if source.jpeg.is_none() && source.bits.is_none() {
            eprintln!("image {}: the image has no data in the table", source.name);
            return None;
        }
        self.images.push(Image::pending(vpx, i, Decode::Script));
        let s = self.images.len() - 1;
        if let Some(slot) = self.table_images.get_mut(i) {
            *slot = Some(s);
        }
        Some(s)
    }

    /// Decodes one of the scene's pictures now, if it waits to be: those the
    /// renderer reads as texels before it uploads the rest (normal maps,
    /// colour grades).
    fn decode_now(&mut self, vpx: &VPX, slot: usize) {
        let Some(image) = self.images.get_mut(slot) else { return };
        let Some((t, how)) = image.pending else { return };
        match vpx.images.get(t).and_then(|source| Image::decode(source, how)) {
            Some(decoded) => *image = decoded,
            None => eprintln!("image {}: could not be decoded", vpx.images.get(t).map_or("?", |i| i.name.as_str())),
        }
    }

    /// Decodes every picture still waiting to be, for the tools that compare
    /// scenes' pictures.
    pub fn decode_pending(&mut self, vpx: &VPX) {
        let waiting: Vec<usize> = (0..self.images.len()).filter(|&k| self.images[k].pending.is_some()).collect();
        let decoded = decode_in_parallel(&waiting, |&k| {
            let (t, how) = self.images[k].pending?;
            Image::decode(vpx.images.get(t)?, how)
        });
        for (k, image) in waiting.into_iter().zip(decoded) {
            if let Some(image) = image {
                self.images[k] = image;
            }
        }
    }

    /// The images the script may swap in ([`script_images`]), so a swap
    /// never waits for a decode: those decoded ahead taken as they are, the
    /// rest decoded as they're uploaded.
    fn add_script_images(&mut self, vpx: &VPX, ahead: &mut [Option<Option<Image>>]) {
        let (before, mut bytes) = (self.images.len(), 0usize);
        let wanted: Vec<usize> = script_images(vpx).into_iter().filter(|&i| self.table_images[i].is_none()).collect();
        let missing: Vec<usize> = wanted.iter().copied().filter(|&i| ahead.get(i).is_none_or(Option::is_none)).collect();
        // those not decoded ahead are decoded when they're uploaded
        let mut late = missing
            .iter()
            .map(|&i| {
                let source = &vpx.images[i];
                (source.jpeg.is_some() || source.bits.is_some()).then(|| Image::pending(vpx, i, Decode::Script))
            })
            .collect::<Vec<_>>()
            .into_iter();
        let decoded: Vec<Option<Image>> =
            wanted.iter().map(|&i| match ahead.get_mut(i).and_then(Option::take) { Some(picture) => picture, None => late.next().flatten() }).collect();
        for (i, image) in wanted.into_iter().zip(decoded) {
            let Some(image) = image else { continue };
            bytes += image.bytes();
            self.images.push(image);
            self.table_images[i] = Some(self.images.len() - 1);
        }
        if self.images.len() > before {
            eprintln!("{} images the script may swap in, uploaded ahead ({:.1} MB decoded so far)", self.images.len() - before, bytes as f64 / 1e6);
        }
    }

    /// VPX's `Flasher::RenderSetup` for the flashers on the table: the
    /// outline at height 0, its texture coordinates spanning its bounds
    /// (`ImageModeWrap`) or the table (`ImageModeWorld`, without the table's
    /// origin, as VPX has it), filled by `PolygonToTriangles`. Flashers on the
    /// backdrop and those showing a display are drawn elsewhere.
    fn add_flashers(&mut self, vpx: &VPX) {
        use vpin::vpx::gameitem::flasher::RenderMode;
        use vpin::vpx::gameitem::ramp_image_alignment::RampImageAlignment;
        let g = &vpx.gamedata;
        let (inv_table_w, inv_table_h) = (1.0 / (g.right - g.left), 1.0 / (g.bottom - g.top));
        let find = |n: &str| crate::look::image_named(vpx, n).map(|i| i as u32);
        for (i, item) in vpx.gameitems.iter().enumerate() {
            let GameItemEnum::Flasher(f) = item else { continue };
            // one showing the score display (VPX's DMD render mode, before 10.8.1 its DMD
            // flag); the other display modes come later
            let dmd = f.is_dmd == Some(true) || matches!(f.render_mode, Some(RenderMode::DMD));
            let display = dmd || f.render_mode.as_ref().is_some_and(|m| !matches!(m, RenderMode::Flasher));
            if (display && !dmd) || f.backglass == Some(true) {
                continue;
            }
            let outline = crate::look::flasher_outline(f);
            let triangles = crate::polygon::polygon_to_triangles(&outline, false);
            if triangles.is_empty() {
                continue;
            }
            let (mut min, mut max) = ([f32::MAX; 2], [-f32::MAX; 2]);
            for p in &outline {
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
            let base_vertex = self.vertices.len() as i32;
            let first_index = self.indices.len() as u32;
            let index_count = if dmd {
                // VPX's Flasher::RenderSetup: a display is the outline's bounds, the frame
                // spread over them from the top left
                let corners = [([min[0], min[1]], [0.0, 0.0]), ([min[0], max[1]], [0.0, 1.0]), ([max[0], max[1]], [1.0, 1.0]), ([max[0], min[1]], [1.0, 0.0])];
                for ([x, y], uv) in corners {
                    self.vertices.push(Vertex { position: [x, y, 0.0], normal: [0.0, 0.0, 1.0], uv });
                }
                self.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
                6
            } else {
                let (inv_w, inv_h) = (1.0 / (max[0] - min[0]), 1.0 / (max[1] - min[1]));
                let wrap = f.image_alignment == RampImageAlignment::Wrap;
                for &[x, y] in &outline {
                    let uv = if wrap { [(x - min[0]) * inv_w, (y - min[1]) * inv_h] } else { [x * inv_table_w, y * inv_table_h] };
                    self.vertices.push(Vertex { position: [x, y, 0.0], normal: [0.0, 0.0, 1.0], uv });
                }
                self.indices.extend_from_slice(&triangles);
                self.table_image(vpx, &f.image_a);
                self.table_image(vpx, &f.image_b);
                triangles.len() as u32
            };
            let center = [0.5 * (min[0] + max[0]), 0.5 * (min[1] + max[1])];
            let start = crate::look::FlasherRt::new(f, center, &find, None).now(1.0);
            let lightmap = self.light_named(vpx, f.light_map.as_deref());
            self.flashers.push(FlasherLook { part: i as u32, first_index, index_count, base_vertex, center, start, dmd, lightmap });
        }
    }

    /// What VPX draws flat over the window: the backdrop image named for
    /// the view (or the backdrop colour), then, unless `items` is false (the
    /// cabinet view, VPX's `m_noBackdrop`), the backdrop's lights, score
    /// reels and text boxes.
    /// The player's look for the table, from the settings, before the backdrop is added: its
    /// exposure and tone mapper over the table's (VPX's `[TableOverride]` `Exposure`, `ToneMapper`),
    /// and the light level VPX's `SceneLighting` chose — the environment's, the point lights' and the
    /// ambient emission scaled by it in the table's day and night's place (`Renderer::SetupShaders`).
    pub fn set_player_look(&mut self, vpx: &VPX, exposure: Option<f32>, mapper: Option<u32>, light_level: f32) {
        if let Some(e) = exposure {
            self.tone.exposure = e;
        }
        if let Some(m) = mapper {
            self.tone.mapper = m;
        }
        let g = &vpx.gamedata;
        if light_level != g.global_emission_scale {
            let c = |v: u8| f32::from(v) / 255.0 * g.light_emission_scale * light_level;
            self.tone.env_emission = g.env_emission_scale * light_level;
            self.tone.light_emission = [c(g.light0_emission.r), c(g.light0_emission.g), c(g.light0_emission.b)];
            self.tone.ambient = [g.light_ambient.r, g.light_ambient.g, g.light_ambient.b].map(|v| f32::from(v) / 255.0 * light_level);
        }
        self.light_level = light_level;
    }

    pub fn add_backdrop(&mut self, vpx: &VPX, backdrop_image: &str, items: bool) {
        let g = &vpx.gamedata;
        let c = &g.backdrop_color;
        self.backdrop = BackdropLook {
            image: if g.render_decals { self.table_image(vpx, backdrop_image) } else { None },
            color: [f32::from(c.r) / 255.0, f32::from(c.g) / 255.0, f32::from(c.b) / 255.0],
            intensity: if g.image_backdrop_night_day { self.light_level.sqrt() } else { 1.0 },
        };
        if !items {
            return;
        }
        // VPX draws backdrop lights only with decals on (Light::Render)
        if g.render_decals {
            self.add_backdrop_lights(vpx);
        }
        self.text_boxes = vpx
            .gameitems
            .iter()
            .enumerate()
            .filter_map(|(i, item)| match item {
                GameItemEnum::TextBox(t) => crate::textbox::TextBoxLook::new(i as u32, t),
                _ => None,
            })
            .collect();
        if !g.render_em_reels {
            return;
        }
        let mut images: HashMap<String, Option<usize>> = HashMap::new();
        for (i, item) in vpx.gameitems.iter().enumerate() {
            let GameItemEnum::Reel(r) = item else { continue };
            let key = r.image.to_lowercase();
            let image = match images.get(&key) {
                Some(&image) => image,
                None => {
                    let image = self.table_image(vpx, &r.image);
                    images.insert(key, image);
                    image
                }
            };
            // VPX draws nothing for a reel without an image
            let Some(image) = image else { continue };
            let digit_range = (r.digit_range as i32).max(0);
            let (cols, rows) = if r.use_image_grid {
                let cols = r.images_per_grid_row as i32;
                if cols != 0 { (cols, (digit_range + 1 + cols - 1) / cols) } else { (cols, 1) }
            } else {
                (digit_range + 1, 1)
            };
            if cols == 0 {
                continue;
            }
            let (ratio_x, ratio_y) = (1.0 / cols as f32, 1.0 / rows as f32);
            let digits = (0..=digit_range)
                .map(|d| {
                    let (u, v) = ((d % cols) as f32 * ratio_x, (d / cols) as f32 * ratio_y);
                    [u, v, u + ratio_x, v + ratio_y]
                })
                .collect();
            let spacing = r.reel_spacing.max(0.0);
            self.reels.push(ReelLook {
                part: i as u32,
                image,
                x: r.ver1.x / EDITOR_BG_WIDTH + spacing / EDITOR_BG_WIDTH,
                y: r.ver1.y / EDITOR_BG_HEIGHT + spacing / EDITOR_BG_HEIGHT,
                width: r.width.max(0.0) / EDITOR_BG_WIDTH,
                height: r.height.max(0.0) / EDITOR_BG_HEIGHT,
                spacing_x: spacing / EDITOR_BG_WIDTH,
                digits,
            });
        }
    }

    /// VPX's `Light::RenderSetup` for lights on the backdrop: the outline
    /// through the light's points (`GetRgVertex`, accuracy 4) filled with
    /// triangles (`PolygonToTriangles`).
    fn add_backdrop_lights(&mut self, vpx: &VPX) {
        use physics_compat::math::{Vertex2D, Vertex3Ds};
        use physics_compat::setup::curves::{DragPoint, get_rg_vertex};
        for (i, item) in vpx.gameitems.iter().enumerate() {
            let GameItemEnum::Light(l) = item else { continue };
            if !l.is_backglass {
                continue;
            }
            let Some(light) = self.light_parts.iter().position(|p| p.part == i as u32) else { continue };
            let points: Vec<DragPoint> =
                l.drag_points.iter().map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) }).collect();
            let outline: Vec<[f32; 2]> = get_rg_vertex::<Vertex2D>(&points, true, 4.0).iter().map(|v| [v.v.x, v.v.y]).collect();
            let triangles = crate::polygon::polygon_to_triangles(&outline, true);
            // VPX: a light whose shape can't be filled isn't drawn
            if triangles.is_empty() {
                continue;
            }
            let image = if l.is_bulb_light { None } else { self.table_image(vpx, &l.image) };
            self.backdrop_lights.push(BackdropLightLook {
                part: i as u32,
                light,
                points: outline,
                triangles,
                image,
                bulb: l.is_bulb_light,
                modulate_vs_add: l.bulb_modulate_vs_add,
                center: [l.center.x, l.center.y],
                falloff: l.falloff_radius,
                falloff_power: l.falloff_power,
            });
        }
    }

    pub fn build(vpx: &VPX, doc: &Document, mut ahead: crate::table_meshes::Pictures) -> Scene {
        let parts: HashMap<String, u32> =
            vpx.gameitems.iter().enumerate().map(|(i, item)| (item.name().to_lowercase(), i as u32)).collect();

        let mut scene = Scene {
            vertices: Vec::new(),
            indices: Vec::new(),
            draws: Vec::new(),
            animations: Vec::new(),
            ramps: Vec::new(),
            materials: Vec::new(),
            images: Vec::new(),
            groups: Vec::new(),
            light_parts: Vec::new(),
            ball: (0, 0, 0),
            bounds_min: Vec3::new(f32::MAX, f32::MAX, f32::MAX),
            bounds_max: Vec3::new(f32::MIN, f32::MIN, f32::MIN),
            depth_bounds: Vec::new(),
            lights: [Vec3::new(476.0, 720.0, 5000.0), Vec3::new(476.0, 1440.0, 5000.0)],
            mirrors: Mirror::of_table(vpx),
            refractions: Refraction::of_table(vpx),
            reels: Vec::new(),
            backdrop_lights: Vec::new(),
            text_boxes: Vec::new(),
            backdrop: BackdropLook::default(),
            tone: Tone::default(),
            light_level: vpx.gamedata.global_emission_scale,
            env_hdr: None,
            ball_hdr: None,
            flashers: Vec::new(),
            table_images: vec![None; vpx.images.len()],
            image_alpha_test: vpx.images.iter().map(|i| i.alpha_test_value / 255.0).collect(),
            picture_alpha_test: Vec::new(),
            kicker_pictures: [None; 5],
        };
        let table_materials = crate::look::Materials::new(vpx);

        // images: decoded once, only those a material uses, in the order they're first used
        let mut wanted: Vec<usize> = Vec::new();
        for i in doc.materials.iter().filter_map(|m| m.image) {
            if !wanted.contains(&i) {
                wanted.push(i);
            }
        }
        // each the table's picture the export packed, decoded while the export ran, else now; one
        // the export packed twice (the playfield's) decoded once, and copied
        let source = |i: usize| ahead.sources.get(i).copied().flatten();
        let mut firsts: Vec<usize> = Vec::new();
        for &i in &wanted {
            if source(i).is_some_and(|t| !firsts.iter().any(|&f| source(f) == Some(t))) {
                firsts.push(i);
            }
        }
        let missing: Vec<usize> = firsts.iter().copied().filter(|&i| source(i).is_some_and(|t| ahead.material.get(t).is_none_or(Option::is_none))).collect();
        // each decoded when it's uploaded
        let late: Vec<Option<Image>> = missing.iter().map(|&i| source(i).filter(|&t| t < vpx.images.len()).map(|t| Image::pending(vpx, t, Decode::Material))).collect();
        let mut by_source: HashMap<usize, Option<Image>> = HashMap::new();
        for (&i, picture) in missing.iter().zip(late) {
            by_source.extend(source(i).map(|t| (t, picture)));
        }
        for &i in &firsts {
            if let Some(t) = source(i)
                && let Some(Some(picture)) = ahead.material.get_mut(t).map(Option::take)
            {
                by_source.insert(t, picture);
            }
        }
        // the last to use a picture takes it, the others a copy
        let mut users: HashMap<usize, usize> = HashMap::new();
        for t in wanted.iter().filter_map(|&i| source(i)) {
            *users.entry(t).or_default() += 1;
        }
        let decoded: Vec<Option<Image>> = wanted
            .iter()
            .map(|&i| {
                let t = source(i)?;
                let left = users.get_mut(&t)?;
                *left -= 1;
                if *left == 0 { by_source.remove(&t).flatten() } else { by_source.get(&t).cloned().flatten() }
            })
            .collect();
        let mut image_slot: HashMap<usize, usize> = HashMap::new();
        // each picture's table image, a copy's too (the playfield's own), for its alpha test
        let mut image_source: HashMap<usize, usize> = HashMap::new();
        for (&i, image) in wanted.iter().zip(decoded) {
            let Some(image) = image else { continue };
            image_slot.insert(i, scene.images.len());
            if let Some(t) = source(i) {
                image_source.insert(scene.images.len(), t);
            }
            scene.images.push(image);
            // the export names each image after the table's
            if let Some(t) = crate::look::image_named(vpx, &doc.images[i].name)
                && scene.table_images[t].is_none()
            {
                scene.table_images[t] = Some(scene.images.len() - 1);
            }
        }
        scene.materials = doc
            .materials
            .iter()
            .map(|m| Material {
                base_color: m.base_color,
                image: m.image.and_then(|i| image_slot.get(&i).copied()),
                blend: m.alpha_mode == AlphaMode::Blend,
                alpha_cutoff: (m.alpha_mode == AlphaMode::Mask).then_some(m.alpha_cutoff),
                emissive: m.emissive,
                metallic: m.metallic,
                roughness: m.roughness,
            })
            .collect();
        let default_material = scene.materials.len();
        scene.materials.push(Material {
            base_color: [0.8, 0.8, 0.8, 1.0],
            image: None,
            blend: false,
            alpha_cutoff: None,
            emissive: [0.0; 3],
            metallic: 0.0,
            roughness: 0.5,
        });

        // a group for each part that moves
        let mut group_of_part: HashMap<u32, usize> = HashMap::new();
        for (i, item) in vpx.gameitems.iter().enumerate() {
            let motion = match item {
                GameItemEnum::Flipper(f) => Some(Motion::Flipper {
                    center: Vec3::new(f.center.x, f.center.y, 0.0),
                    start_angle: f.start_angle.to_radians(),
                }),
                GameItemEnum::Gate(g) => Some(Motion::Axle {
                    pivot: Vec3::new(g.center.x, g.center.y, 0.0),
                    rotation: g.rotation,
                    sign: if g.two_way { 1.0 } else { -1.0 },
                }),
                GameItemEnum::Spinner(s) => Some(Motion::Axle {
                    pivot: Vec3::new(s.center.x, s.center.y, 0.0),
                    rotation: s.rotation,
                    sign: -1.0,
                }),
                GameItemEnum::Plunger(_) => Some(Motion::Plunger),
                _ => None,
            };
            if let Some(motion) = motion {
                group_of_part.insert(i as u32, scene.groups.len());
                scene.groups.push(Group { part: i as u32, motion });
            }
        }

        // the lights
        let mut light_of_part: HashMap<u32, usize> = HashMap::new();
        for (i, item) in vpx.gameitems.iter().enumerate() {
            if let GameItemEnum::Light(l) = item {
                let c = |c: &vpin::vpx::color::Color| [f32::from(c.r) / 255.0, f32::from(c.g) / 255.0, f32::from(c.b) / 255.0];
                light_of_part.insert(i as u32, scene.light_parts.len());
                scene.light_parts.push(LightInfo {
                    part: i as u32,
                    center: Vec3::new(l.center.x, l.center.y, 0.0),
                    falloff_radius: l.falloff_radius.max(1.0),
                    falloff_power: l.falloff_power,
                    color: c(&l.color),
                    color2: c(&l.color2),
                    intensity: l.intensity,
                    state: crate::light::clamp_state(l.state.unwrap_or(l.state_u32 as i32 as f32)),
                    bulb: l.is_bulb_light,
                    halo_height: l.bulb_halo_height,
                    // VPX's load (pintable.cpp, build 5436): before 10.8 a bulb shone from its halo height
                    height: if vpx.version.u32() < 1080 {
                        if l.is_bulb_light { l.bulb_halo_height } else { 0.0 }
                    } else {
                        l.height.unwrap_or(0.0)
                    },
                    modulate_vs_add: l.bulb_modulate_vs_add,
                    plain: !l.is_bulb_light && crate::look::image_named(vpx, &l.image).is_none(),
                    transmission: l.transmission_scale,
                    // VPX's load: before 10.8 only a bulb light shone on the ball
                    on_ball: l.show_reflection_on_ball && (l.is_bulb_light || vpx.version.u32() >= 1080),
                    mesh_radius: l.mesh_radius,
                    depth_bias: l.depth_bias,
                    ball_shadows: matches!(l.shadows, Some(vpin::vpx::gameitem::light::ShadowMode::RaytracedBallShadows)),
                    surface_shading: {
                        let named = (!l.surface.is_empty())
                            .then(|| {
                                vpx.gameitems.iter().find_map(|g| match g {
                                    GameItemEnum::Wall(w) if w.name == l.surface => Some(w.top_material.as_str()),
                                    GameItemEnum::Ramp(r) if r.name == l.surface => Some(r.material.as_str()),
                                    _ => None,
                                })
                            })
                            .flatten();
                        let name = named.unwrap_or(vpx.gamedata.playfield_material.as_str());
                        table_materials.find(name).map_or_else(crate::look::Shading::dummy, |k| table_materials.all[k as usize].shading())
                    },
                    // VPX finds pictures by name, any case (GetImage)
                    picture_is_surface: {
                        let named = (!l.surface.is_empty())
                            .then(|| {
                                vpx.gameitems.iter().find_map(|g| match g {
                                    GameItemEnum::Wall(w) if w.name == l.surface => Some(w.image.as_str()),
                                    GameItemEnum::Ramp(r) if r.name == l.surface => Some(r.image.as_str()),
                                    _ => None,
                                })
                            })
                            .flatten();
                        let surface = named.unwrap_or(vpx.gamedata.image.as_str());
                        !l.is_bulb_light
                            && l.image.eq_ignore_ascii_case(surface)
                            && crate::look::image_named(vpx, &l.image).is_some()
                    },
                    image_mode: l.is_image_mode,
                });
            }
        }

        // walk the node tree
        // the table's surfaces, for the heights VPX sorts parts from
        let surfaces = table_physics::Surfaces::of_table(vpx);
        // the primitives VPX draws in render groups, their matrices left out (identity)
        let render_grouped = crate::shapes::primitive::in_render_groups(vpx);
        let mut stack: Vec<(usize, Mat4, Option<String>)> = doc.roots.iter().map(|&r| (r, Mat4::IDENTITY, None)).collect();
        while let Some((n, parent_matrix, parent_name)) = stack.pop() {
            let node = &doc.nodes[n];
            let world = node_matrix(node) * parent_matrix;
            if node.name.starts_with("TableLight") {
                let p = world.transform_no_perspective(Vec3::default());
                let k = if node.name.ends_with('1') { 1 } else { 0 };
                scene.lights[k] = Vec3::new(p.x, p.z, p.y);
            }
            let part = part_of(&node.name, parent_name.as_deref(), &parts);
            // pivots sit at the position of the part's mesh nodes: a flipper's
            // center, a gate's axle height
            if node.mesh.is_some()
                && let Some(p) = part
                && let Some(&g) = group_of_part.get(&p)
            {
                let origin = world.transform_no_perspective(Vec3::default());
                let at = Vec3::new(origin.x, origin.z, origin.y);
                match &mut scene.groups[g].motion {
                    Motion::Flipper { center, .. } => center.z = at.z,
                    Motion::Axle { pivot, .. } => *pivot = at,
                    Motion::Plunger => {}
                }
            }
            // flashers are built as VPX builds them (add_flashers)
            let is_flasher = part.is_some_and(|p| matches!(vpx.gameitems[p as usize], GameItemEnum::Flasher(_)));
            if let Some(m) = node.mesh
                && !is_flasher
            {
                let to_table = AXES * world * AXES; // export axes in and out
                for prim in &doc.meshes[m].primitives {
                    let base_vertex = scene.vertices.len() as i32;
                    let first_index = scene.indices.len() as u32;
                    let mut sum = Vec3::default();
                    for (k, p) in prim.positions.iter().enumerate() {
                        let pos = to_table.transform_no_perspective(Vec3::from(to_vpx(*p)));
                        let n = prim.normals.get(k).map_or([0.0, 0.0, 1.0], |n| to_vpx(*n));
                        let nn = normal_through(&to_table, n);
                        scene.vertices.push(Vertex {
                            position: [pos.x, pos.y, pos.z],
                            normal: nn,
                            uv: prim.uvs.get(k).copied().unwrap_or_default(),
                        });
                        scene.bounds_min = scene.bounds_min.min(pos);
                        scene.bounds_max = scene.bounds_max.max(pos);
                        sum = Vec3::new(sum.x + pos.x, sum.y + pos.y, sum.z + pos.z);
                    }
                    // a ramp, kept to be built again when a script changes its heights or widths: when
                    // our own shape lands vertex for vertex where this one did (not with the export's
                    // shapes)
                    if let Some(p) = part
                        && let GameItemEnum::Ramp(r) = &vpx.gameitems[p as usize]
                    {
                        let ctx = crate::shapes::ramp::context(vpx, &table_materials, r);
                        let shape = RampShape { part: p, base_vertex: base_vertex as u32, source: crate::shapes::ramp::source(r), ctx };
                        let rebuilt = shape.vertices(shape.file_shape());
                        let placed = &scene.vertices[base_vertex as usize..];
                        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
                        if rebuilt.len() == placed.len() && rebuilt.iter().zip(placed).all(|(a, b)| near(a.position, b.position)) {
                            scene.ramps.push(shape);
                        }
                    }
                    let count = prim.positions.len().max(1) as f32;
                    let light = part.and_then(|p| light_of_part.get(&p).copied());
                    let glow = light.filter(|_| node.name.ends_with("_insert"));
                    let bulb = light.filter(|_| node.name.ends_with("_bulb"));
                    let bulb_piece = light.filter(|_| node.name.ends_with("_bulb") || node.name.ends_with("_socket"));
                    if let Some(l) = glow {
                        // the glow's plane: the light's surface
                        let at = to_table.transform_no_perspective(Vec3::default());
                        scene.light_parts[l].center.z = at.z;
                    }
                    // a bumper's ring and skirt move with its animation
                    let bumper = part.and_then(|p| match &vpx.gameitems[p as usize] {
                        GameItemEnum::Bumper(_) if node.name.ends_with("Ring") => Some(BumperPiece::Ring),
                        GameItemEnum::Bumper(b) if node.name.ends_with("Socket") => {
                            // the node sits at the bumper's centre on its surface; the skirt 5 above
                            let at = to_table.transform_no_perspective(Vec3::default());
                            Some(BumperPiece::Skirt { pivot: Vec3::new(at.x, at.y, at.z + 5.0), radius: b.radius, height_scale: b.height_scale })
                        }
                        _ => None,
                    });
                    let item = part.map(|p| &vpx.gameitems[p as usize]);
                    let lower = node.name.to_lowercase();
                    let slot = match item {
                        Some(GameItemEnum::Wall(_)) if node.name.ends_with("Side") => LookSlot::Side,
                        Some(GameItemEnum::Flipper(_)) if lower.ends_with("rubber") => LookSlot::Side,
                        Some(GameItemEnum::Spinner(_)) if !lower.ends_with("plate") => LookSlot::Fixed,
                        Some(GameItemEnum::Gate(_)) if !lower.ends_with("wire") => LookSlot::Fixed,
                        Some(GameItemEnum::Light(_) | GameItemEnum::Bumper(_)) | None => LookSlot::Fixed,
                        Some(_) => LookSlot::Main,
                    };
                    let center = Vec3::new(sum.x / count, sum.y / count, sum.z / count);
                    // VPX's DrawMesh: where each part sorts from, the point it hands over (only its
                    // height counts): primitives and targets their position, the rest their bounds'
                    // centre at a height of their own (UpdateBounds, RenderSetup): a wall's top, a
                    // ramp's middle, a rubber's height, a flipper, trigger, decal, bumper, kicker or
                    // gate on its surface, a spinner at its height, a light's bulb glass at the
                    // bulb's height above its surface (bulbPos) and its socket on it; depth biases
                    // where VPX has them
                    let at = |z: f32| Vec3::new(center.x, center.y, z);
                    let on = |name: &str, x: f32, y: f32| surfaces.height(name, physics_compat::math::Vertex2D::new(x, y));
                    let (sort_point, depth_bias) = match item {
                        // the playfield: VPX's own with a depth bias of 100000 (Player's implicit
                        // playfield), as is a table's mesh saved before 10.8 (PinTable's load), so its
                        // pass adding the mirror sorts before every other see-through part
                        Some(GameItemEnum::Primitive(pr)) if lower == "playfield_mesh" && vpx.version.u32() < 1080 => {
                            (Vec3::new(pr.position.x, pr.position.y, pr.position.z), 100000.0)
                        }
                        None if lower == "playfield_mesh" => (center, 100000.0),
                        Some(GameItemEnum::Primitive(pr)) => (Vec3::new(pr.position.x, pr.position.y, pr.position.z), pr.depth_bias),
                        Some(GameItemEnum::HitTarget(t)) => (Vec3::new(t.position.x, t.position.y, t.position.z), t.depth_bias),
                        Some(GameItemEnum::Ramp(r)) => (at(0.5 * (r.height_bottom + r.height_top)), r.depth_bias),
                        Some(GameItemEnum::Wall(w)) => (at(w.height_top), 0.0),
                        Some(GameItemEnum::Rubber(r)) => (at(r.height), 0.0),
                        Some(GameItemEnum::Flipper(f)) => (at(on(&f.surface, f.center.x, f.center.y)), 0.0),
                        Some(GameItemEnum::Trigger(t)) => (at(on(&t.surface, t.center.x, t.center.y)), 0.0),
                        Some(GameItemEnum::Decal(d)) if !d.backglass => (at(on(&d.surface, d.center.x, d.center.y)), 0.0),
                        Some(GameItemEnum::Bumper(b)) => (at(on(&b.surface, b.center.x, b.center.y)), 0.0),
                        Some(GameItemEnum::Kicker(k)) => (at(on(&k.surface, k.center.x, k.center.y)), 0.0),
                        Some(GameItemEnum::Gate(g)) => (at(on(&g.surface, g.center.x, g.center.y)), 0.0),
                        Some(GameItemEnum::Spinner(s)) => (at(on(&s.surface, s.center.x, s.center.y) + s.height), 0.0),
                        Some(GameItemEnum::Light(l)) => {
                            let surface = on(&l.surface, l.center.x, l.center.y);
                            let raised = if bulb.is_some() { light.map_or(0.0, |k| scene.light_parts[k].height) } else { 0.0 };
                            (at(surface + raised), l.depth_bias)
                        }
                        _ => (center, 0.0),
                    };
                    // VPX draws a flipper's rubber with the flipper's picture too, its lower half
                    // (Flipper::Render's SetBasic with the rubber's material); the export leaves the
                    // rubber without it
                    let mut material = prim.material.unwrap_or(default_material);
                    if let Some(GameItemEnum::Flipper(f)) = item
                        && lower.ends_with("rubber")
                        && let Some(picture) = f.image.as_deref().and_then(|name| scene.table_image(vpx, name))
                    {
                        let mut m = scene.materials[material].clone();
                        m.image = Some(picture);
                        scene.materials.push(m);
                        material = scene.materials.len() - 1;
                    }
                    // VPX draws a kicker with a built-in picture of its kind (Kicker::RenderSetup), which
                    // can't be taken: ours in its place, the picture bringing the colour
                    // the export's stand-in tint gave
                    if let Some(GameItemEnum::Kicker(k)) = item
                        && lower.ends_with("kicker")
                        && let Some(picture) = scene.kicker_picture(&k.kicker_type)
                    {
                        let mut m = scene.materials[material].clone();
                        m.image = Some(picture);
                        m.base_color = [1.0, 1.0, 1.0, m.base_color[3]];
                        scene.materials.push(m);
                        material = scene.materials.len() - 1;
                    }
                    // VPX's static prepass (each part's Render with STATIC_ONLY): primitives and
                    // rubbers set to static rendering, solid ramps, decals and bumper bases and caps,
                    // walls that neither drop nor show a see-through material, lights' sockets and
                    // (unless set otherwise) bulbs, spinners' brackets, the playfield
                    let see_through = |name: &str| table_materials.find(name).is_some_and(|k| table_materials.all[k as usize].opacity_active);
                    let still = match item {
                        // (VPX's load: a table's playfield mesh saved before 10.8 always so)
                        Some(GameItemEnum::Primitive(p)) => p.static_rendering || (vpx.version.u32() < 1080 && p.is_playfield()),
                        Some(GameItemEnum::Rubber(r)) => r.static_rendering,
                        Some(GameItemEnum::Wall(w)) => {
                            !(w.is_droppable || (w.is_side_visible && see_through(&w.side_material)) || (w.is_top_bottom_visible && see_through(&w.top_material)))
                        }
                        Some(GameItemEnum::Ramp(r)) => !see_through(&r.material),
                        Some(GameItemEnum::Decal(d)) => d.backglass || !see_through(&d.material),
                        Some(GameItemEnum::Bumper(b)) if lower.ends_with("base") => !see_through(&b.base_material),
                        Some(GameItemEnum::Bumper(b)) if lower.ends_with("cap") => !see_through(&b.cap_material),
                        Some(GameItemEnum::Light(_)) if lower.ends_with("_socket") => true,
                        Some(GameItemEnum::Light(l)) if lower.ends_with("_bulb") => l.has_static_bulb_mesh.unwrap_or(true),
                        Some(GameItemEnum::Spinner(_)) => lower.ends_with("bracket"),
                        None => lower == "playfield_mesh",
                        _ => false,
                    };
                    let shading = file_shading(item, slot, &node.name, &table_materials, &vpx.gamedata.playfield_material);
                    // a primitive's own normals to the table's: its matrix's inverse transpose (none for
                    // one drawn in a render group, whose matrix VPX leaves out)
                    let normal_rows = match item {
                        Some(GameItemEnum::Primitive(p)) if !part.is_some_and(|k| render_grouped.contains(&(k as usize))) => {
                            let m = crate::game::primitive_matrix([p.position.x, p.position.y, p.position.z], [p.size.x, p.size.y, p.size.z], &p.rot_and_tra);
                            let inv = m.inverse();
                            // row i of the inverse's transpose is column i of the inverse
                            Some([0, 1, 2].map(|i| [inv.m[0][i], inv.m[1][i], inv.m[2][i]]))
                        }
                        _ => None,
                    };
                    // a primitive's normal map (Primitive::Render: drawn with its picture only), in
                    // tangent space or its own (then by its matrix's inverse transpose)
                    let (normal_map, object_normal) = match item {
                        Some(GameItemEnum::Primitive(p)) => {
                            let map = p.normal_map.as_deref().filter(|n| !n.is_empty()).and_then(|n| scene.table_image(vpx, n));
                            let object = map.filter(|_| p.object_space_normal_map.unwrap_or(false)).map(|_| {
                                let m = crate::game::primitive_matrix([p.position.x, p.position.y, p.position.z], [p.size.x, p.size.y, p.size.z], &p.rot_and_tra);
                                let inv = m.inverse();
                                // row i of the inverse's transpose is column i of the inverse
                                [0, 1, 2].map(|i| [inv.m[0][i], inv.m[1][i], inv.m[2][i]])
                            });
                            (map, object)
                        }
                        _ => (None, None),
                    };
                    scene.indices.extend_from_slice(&prim.indices);
                    scene.draws.push(Draw {
                        first_index,
                        index_count: prim.indices.len() as u32,
                        base_vertex,
                        material,
                        group: part.and_then(|p| group_of_part.get(&p).copied()),
                        part,
                        visible: node.visible,
                        center,
                        glow,
                        bulb,
                        bulb_piece,
                        wall_side: matches!(item, Some(GameItemEnum::Wall(_))) && node.name.ends_with("Side"),
                        bumper,
                        bumper_piece: match item {
                            Some(GameItemEnum::Bumper(_)) => crate::anim::BUMPER_PIECES.iter().position(|s| node.name.ends_with(s)).map(|i| i as u8),
                            _ => None,
                        },
                        slot,
                        sort_point,
                        depth_bias,
                        disable_lighting: match item {
                            Some(GameItemEnum::Primitive(pr)) => crate::look::disable_lighting_top(pr.disable_lighting_top, pr.disable_lighting_top_old),
                            Some(GameItemEnum::HitTarget(t)) => crate::look::disable_lighting_top(t.disable_lighting_top, t.disable_lighting_top_old),
                            Some(GameItemEnum::Wall(w)) => crate::look::disable_lighting_top(w.disable_lighting_top, w.disable_lighting_top_old),
                            _ => 0.0,
                        },
                        // VPX's defaults keep light from below off (1) unless the file says; static
                        // parts get none (they're drawn ahead of the bulb light buffer); the other
                        // parts draw after VPX resets it to 0 (Primitive, HitTarget, Surface::Render)
                        disable_lighting_below: match item {
                            Some(GameItemEnum::Primitive(pr)) => if still { 1.0 } else { primitive_below(vpx, &table_materials, pr) },
                            Some(GameItemEnum::HitTarget(t)) => t.disable_lighting_below.unwrap_or(1.0),
                            Some(GameItemEnum::Wall(w)) => {
                                let see_through = |name: &str| table_materials.find(name).is_some_and(|k| table_materials.all[k as usize].opacity_active);
                                let dynamic = w.is_droppable
                                    || (w.is_side_visible && see_through(&w.side_material))
                                    || (w.is_top_bottom_visible && see_through(&w.top_material));
                                if dynamic { w.disable_lighting_below.unwrap_or(1.0) } else { 1.0 }
                            }
                            _ => 0.0,
                        },
                        below_fixed: still || !matches!(item, Some(GameItemEnum::Primitive(_) | GameItemEnum::HitTarget(_) | GameItemEnum::Wall(_))),
                        // the slot's material from the table file; a name the table lacks (or none)
                        // is VPX's default material, whose opacity is off
                        // the playfield's is the table's playfield material (Primitive::Render for the
                        // playfield), with or without a part of its own
                        material_alpha: if lower == "playfield_mesh" { Some(vpx.gamedata.playfield_material.as_str()) } else { None }
                            .or_else(|| {
                                let (main, side) = crate::world::file_materials(item.filter(|_| slot != LookSlot::Fixed)?);
                                if slot == LookSlot::Side { side } else { main }
                            })
                            .map(|name| {
                                table_materials.find(name).map_or(MaterialAlpha { opacity_active: false, opacity: 1.0 }, |k| {
                                    let m = &table_materials.all[k as usize];
                                    MaterialAlpha { opacity_active: m.opacity_active, opacity: m.opacity }
                                })
                            })
                            // a piece of a part (a bumper's, a light's socket and bulb, a bracket) by the
                            // material VPX draws it with, its own or one of VPX's
                            .or_else(|| {
                                (slot == LookSlot::Fixed)
                                    .then_some(shading)
                                    .flatten()
                                    .map(|s| MaterialAlpha { opacity_active: s.opacity_active, opacity: s.opacity })
                            }),
                        shading,
                        depth_mask: match item {
                            Some(GameItemEnum::Primitive(pr)) => pr.use_depth_mask.unwrap_or(true),
                            _ => true,
                        },
                        // the playfield shows the playfield's mirror at the table's strength
                        // (Primitive::Render for the playfield); another primitive the one it names
                        mirror: if lower == "playfield_mesh" {
                            Some((0, vpx.gamedata.playfield_reflection_strength)).filter(|&(_, s)| s > 0.0)
                        } else if let Some(GameItemEnum::Primitive(pr)) = item {
                            pr.reflection_probe
                                .as_deref()
                                .and_then(|name| scene.mirrors.iter().position(|m| !name.is_empty() && m.name.eq_ignore_ascii_case(name)))
                                .map(|k| (k, pr.reflection_strength.unwrap_or(1.0)))
                                .filter(|&(_, s)| s > 0.0)
                        } else {
                            None
                        },
                        mirror_strength: match item {
                            Some(GameItemEnum::Primitive(pr)) => pr.reflection_strength.unwrap_or(1.0),
                            _ => 0.0,
                        },
                        normal_rows,
                        refraction: match item {
                            Some(GameItemEnum::Primitive(pr)) => pr
                                .refraction_probe
                                .as_deref()
                                .and_then(|name| scene.refractions.iter().position(|r| !name.is_empty() && r.name.eq_ignore_ascii_case(name))),
                            _ => None,
                        },
                        refraction_thickness: match item {
                            Some(GameItemEnum::Primitive(pr)) => pr.refraction_thickness.unwrap_or(10.0),
                            _ => 0.0,
                        },
                        // each part's Render: drawn into mirrors unless turned off; a light's
                        // bulb always (lights keep the setting out of the file), kickers never
                        reflected: match item {
                            Some(GameItemEnum::Primitive(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Wall(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Ramp(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Rubber(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Flipper(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Bumper(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Gate(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Spinner(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Trigger(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::Plunger(p)) => p.is_reflection_enabled.unwrap_or(true),
                            Some(GameItemEnum::HitTarget(p)) => p.is_reflection_enabled,
                            // (VPX's load: never before 10.8)
                            Some(GameItemEnum::Light(_)) => vpx.version.u32() >= 1080,
                            None => lower == "playfield_mesh",
                            _ => false,
                        },
                        still,
                        playfield: lower == "playfield_mesh",
                        kicker_plate: matches!(item, Some(GameItemEnum::Kicker(_))) && lower.ends_with("plate"),
                        filter: match item {
                            Some(GameItemEnum::Ramp(r)) if r.image_alignment == vpin::vpx::gameitem::ramp_image_alignment::RampImageAlignment::Wrap => {
                                PartFilter::TrilinearClamp
                            }
                            Some(GameItemEnum::Ramp(_)) => PartFilter::Trilinear,
                            Some(GameItemEnum::Decal(_)) => PartFilter::TrilinearMirror,
                            _ => PartFilter::Default,
                        },
                        sorting: match item {
                            _ if lower == "playfield_mesh" => Sorting::Solid,
                            Some(GameItemEnum::Primitive(_)) => Sorting::ByMaterialUnlessStill,
                            Some(GameItemEnum::Ramp(_) | GameItemEnum::HitTarget(_)) => Sorting::ByMaterial,
                            Some(GameItemEnum::Wall(w)) => {
                                if (w.is_side_visible && see_through(&w.side_material)) || (w.is_top_bottom_visible && see_through(&w.top_material)) {
                                    Sorting::SeeThrough
                                } else {
                                    Sorting::Solid
                                }
                            }
                            Some(GameItemEnum::Decal(d)) if !d.backglass => Sorting::SeeThrough,
                            Some(GameItemEnum::Light(_)) if lower.ends_with("_bulb") => Sorting::SeeThrough,
                            Some(_) => Sorting::Solid,
                            None => Sorting::ByMaterial,
                        },
                        culling: match item {
                            // (VPX's own playfield is a primitive too)
                            // (VPX's load: a playfield mesh saved before 10.8 never has back faces)
                            _ if lower == "playfield_mesh" => match item {
                                Some(GameItemEnum::Primitive(p)) => Culling::Primitive {
                                    depth_mask: p.use_depth_mask.unwrap_or(true),
                                    backfaces: p.backfaces_enabled.unwrap_or(false) && vpx.version.u32() >= 1080,
                                },
                                _ => Culling::Primitive { depth_mask: true, backfaces: false },
                            },
                            Some(GameItemEnum::Primitive(p)) => Culling::Primitive {
                                depth_mask: p.use_depth_mask.unwrap_or(true),
                                backfaces: p.backfaces_enabled.unwrap_or(false),
                            },
                            Some(GameItemEnum::Wall(w)) => {
                                let dynamic = (w.is_side_visible && see_through(&w.side_material)) || (w.is_top_bottom_visible && see_through(&w.top_material));
                                if node.name.ends_with("Side") {
                                    Culling::WallSide { dynamic, top_visible: w.is_top_bottom_visible }
                                } else {
                                    Culling::WallTop { dynamic }
                                }
                            }
                            Some(GameItemEnum::Ramp(_) | GameItemEnum::Flasher(_)) => Culling::None,
                            Some(GameItemEnum::Trigger(t))
                                if matches!(
                                    t.shape,
                                    vpin::vpx::gameitem::trigger::TriggerShape::WireA
                                        | vpin::vpx::gameitem::trigger::TriggerShape::WireB
                                        | vpin::vpx::gameitem::trigger::TriggerShape::WireC
                                        | vpin::vpx::gameitem::trigger::TriggerShape::WireD
                                        | vpin::vpx::gameitem::trigger::TriggerShape::Inder
                                ) =>
                            {
                                Culling::None
                            }
                            Some(GameItemEnum::Kicker(k)) if matches!(k.kicker_type, vpin::vpx::gameitem::kicker::KickerType::HoleSimple) => Culling::None,
                            Some(GameItemEnum::Bumper(_)) if !lower.ends_with("ring") => Culling::BothWhenSeeThrough,
                            _ => Culling::Front,
                        },
                        normal_map,
                        object_normal,
                        degenerate: matches!(item, Some(GameItemEnum::Primitive(p)) if p.size.x == 0.0 || p.size.y == 0.0 || p.size.z == 0.0),
                        frame: None,
                        lightmap: match item {
                            Some(GameItemEnum::Primitive(p)) => scene.light_named(vpx, p.light_map.as_deref()),
                            _ => None,
                        },
                    });
                }
            }
            for &c in &node.children {
                stack.push((c, world, Some(node.name.clone())));
            }
        }

        scene.add_ball_mesh();
        scene.add_flashers(vpx);
        scene.add_script_images(vpx, &mut ahead.script);
        // every 256 x 16 picture, which a script may make the colour grade (VPX's
        // `ColorGradeImage`) under a name it builds, like `"LUT" & n`: 16 kB each
        for i in 0..vpx.images.len() {
            if vpx.images[i].width == 256 && vpx.images[i].height == 16 {
                scene.decode_table_image(vpx, i);
            }
        }
        // the table's own tone mapper and exposure (VPX's defaults: Reinhard, 1), and colour grade
        let g = &vpx.gamedata;
        let mapper = g.tone_mapper.as_ref().map_or(0, u32::from);
        // VPX's materials: the table's environment picture, else a default (VPX's GetImage finds
        // no "<None>"), at its emission scale; the point lights' emission (Renderer::SetupShaders)
        let table_env = g.env_image.as_deref().and_then(|e| scene.table_image(vpx, e));
        // a high-range environment picture keeps its range (VPX's RGB_FP16/FP32 textures)
        if table_env.is_some() {
            scene.env_hdr = g.env_image.as_deref().and_then(|e| high_range(vpx, e));
        }
        let env = table_env.or_else(|| {
            scene.images.push(default_environment());
            Some(scene.images.len() - 1)
        });
        let day_night = g.global_emission_scale;
        let c = |v: u8| f32::from(v) / 255.0 * g.light_emission_scale * day_night;
        scene.tone = Tone {
            mapper: if mapper <= 4 { mapper } else { 0 },
            exposure: g.exposure.unwrap_or(1.0),
            color_grade: scene.table_image(vpx, &g.image_color_grade),
            env,
            env_default: table_env.is_none(),
            env_emission: g.env_emission_scale * day_night,
            light_emission: [c(g.light0_emission.r), c(g.light0_emission.g), c(g.light0_emission.b)],
            ambient: [g.light_ambient.r, g.light_ambient.g, g.light_ambient.b].map(|v| f32::from(v) / 255.0 * day_night),
            light_range: g.light_range,
            bloom_strength: g.bloom_strength,
            // VPX reads the switch as on unless 0 (before 10.8, -1 meant the player's choice)
            ao_scale: if g.use_ao.is_none_or(|v| v != 0) { g.ao_scale } else { 0.0 },
            // VPX's Ball::RenderSetup with the table's settings (balls take them unless a script
            // changes a ball's own): no picture means VPX's default, an equirectangular map
            ball_image: scene.table_image(vpx, &g.ball_image),
            ball_spherical: g.ball_spherical_mapping.unwrap_or(true),
            ball_decal: scene.table_image(vpx, &g.ball_image_front),
            ball_decal_mode: g.ball_decal_mode,
            ball_reflection: g.ball_playfield_reflection_strength.unwrap_or(1.0).clamp(0.0, 1.0),
            ball_bulb_scale: g.default_bulb_intensity_scale_on_ball.unwrap_or(1.0),
            ball_inv_table: [1.0 / (g.right - g.left), 1.0 / (g.bottom - g.top)],
        };
        if scene.tone.ball_image.is_some() {
            scene.ball_hdr = high_range(vpx, &g.ball_image);
        }
        scene.picture_alpha_test = (0..scene.images.len())
            .map(|s| image_source.get(&s).copied().or_else(|| scene.table_images.iter().position(|&t| t == Some(s))).map_or(-1.0, |t| scene.image_alpha_test[t]))
            .collect();
        // the pictures the renderer reads as texels: normal maps and colour grades (every 256 x 16
        // one, as a script may switch to it), and the environment's; the rest are decoded as
        // they're uploaded
        let mut now: Vec<usize> = scene.draws.iter().filter_map(|d| d.normal_map).collect();
        now.extend(scene.tone.color_grade);
        // and the environment picture, whose height the renderer's irradiance is sized by
        now.extend(scene.tone.env);
        now.extend((0..vpx.images.len()).filter(|&i| vpx.images[i].width == 256 && vpx.images[i].height == 16).filter_map(|i| scene.table_images[i]));
        now.sort_unstable();
        now.dedup();
        for slot in now {
            scene.decode_now(vpx, slot);
        }
        scene.depth_bounds = vpx_depth_bounds(vpx);
        scene
    }

    /// Lets go of the pictures' texels once the renderer has them on the
    /// graphics card, but for `keep` (the colour grades a script may switch
    /// to), and of the environment's and the ball's high-range copies: on a
    /// big table, gigabytes. Each picture keeps its size and kind.
    pub fn release_pictures(&mut self, keep: &[usize]) {
        for (i, image) in self.images.iter_mut().enumerate() {
            if keep.contains(&i) {
                continue;
            }
            image.rgba = Vec::new();
            if let Some(hdr) = &mut image.hdr {
                *hdr = Vec::new();
            }
        }
        self.env_hdr = None;
        self.ball_hdr = None;
    }

    /// The balls' mesh: VPX's own unit sphere (`ballMesh.h`), whose picture
    /// places lay out a ball's decal or scratches as VPX lays them; its
    /// low-detail sphere below detail level 10 (`Renderer::Renderer`).
    fn add_ball_mesh(&mut self) {
        let base_vertex = self.vertices.len() as i32;
        let first_index = self.indices.len() as u32;
        let low = crate::shapes::rubber::detail_level() < 10.0;
        let (vertices, indices): (&[[f32; 8]], &[u16]) =
            if low { (&crate::ball_mesh::VERTICES_LO, &crate::ball_mesh::INDICES_LO) } else { (&crate::ball_mesh::VERTICES, &crate::ball_mesh::INDICES) };
        for &[x, y, z, nx, ny, nz, u, v] in vertices {
            self.vertices.push(Vertex { position: [x, y, z], normal: [nx, ny, nz], uv: [u, v] });
        }
        self.indices.extend(indices.iter().map(|&i| u32::from(i)));
        self.ball = (first_index, self.indices.len() as u32 - first_index, base_vertex);
    }
}

/// The corners VPX gathers for the camera's depth range (each part's
/// `GetBoundingVertices`, for `ComputeNearFarPlane`): every wall as the
/// table's extent between its bottom and top, each visible ramp's and
/// rubber's box, and each visible primitive with a mesh of its own as the box
/// between its mesh's two extreme corners once placed. VPX places those two
/// corners alone, so a turned primitive's box can fall short of the
/// primitive itself, putting the far plane in front of its far end: Star
/// Gazer's room floor ends there in VPX.
fn vpx_depth_bounds(vpx: &VPX) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = table_physics::view_bounds::depth_view_bounds(vpx).into_iter().map(|v| Vec3::new(v.x, v.y, v.z)).collect();
    for item in &vpx.gameitems {
        let GameItemEnum::Primitive(p) = item else { continue };
        if !p.is_visible || !p.use_3d_mesh {
            continue;
        }
        let Some(mesh) = p.read_mesh().ok().flatten() else { continue };
        if mesh.vertices.is_empty() {
            continue;
        }
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for w in &mesh.vertices {
            for (k, c) in [w.vertex.x, w.vertex.y, w.vertex.z].into_iter().enumerate() {
                lo[k] = lo[k].min(c);
                hi[k] = hi[k].max(c);
            }
        }
        let m = crate::shapes::primitive::matrix(p);
        let a = m.multiply_vector_no_perspective(&physics_compat::math::Vertex3Ds::new(lo[0], lo[1], lo[2]));
        let b = m.multiply_vector_no_perspective(&physics_compat::math::Vertex3Ds::new(hi[0], hi[1], hi[2]));
        for i in 0..8 {
            out.push(Vec3::new(if i & 4 != 0 { b.x } else { a.x }, if i & 2 != 0 { b.y } else { a.y }, if i & 1 != 0 { b.z } else { a.z }));
        }
    }
    out
}

impl Scene {
    /// Our own shapes for the primitives a script may animate:
    /// each with frames, drawn from our own shape (its vertices in VPX's
    /// order, checked against it), keeps its frames for the renderer to show
    /// the one the script asks for. Returns how many.
    pub fn use_own_animations(&mut self, vpx: &VPX) -> usize {
        let grouped = crate::shapes::primitive::in_render_groups(vpx);
        for (p, item) in vpx.gameitems.iter().enumerate() {
            let GameItemEnum::Primitive(pr) = item else { continue };
            if !pr.use_3d_mesh || crate::prim_anim::PrimAnim::new(pr).is_none() {
                continue;
            }
            let Some(mesh) = pr.read_mesh().ok().flatten() else { continue };
            let count = mesh.vertices.len();
            let Some(frames) = crate::prim_anim::frames(pr, count) else { continue };
            let Some(draw) = self.draws.iter().find(|d| d.part == Some(p as u32) && !d.playfield) else { continue };
            let base = draw.base_vertex as usize;
            let Some(drawn) = self.vertices.get(base..base + count) else { continue };
            let matrix = crate::shapes::primitive::matrix(pr);
            let g = grouped.contains(&p);
            let local = crate::shapes::Mesh {
                positions: mesh.vertices.iter().map(|w| [w.vertex.x, w.vertex.y, w.vertex.z]).collect(),
                normals: mesh.vertices.iter().map(|w| [w.vertex.nx, w.vertex.ny, w.vertex.nz]).collect(),
                uvs: Vec::new(),
                indices: Vec::new(),
            };
            let placed = crate::shapes::primitive::place_with(&matrix, local, g);
            let same = placed.positions.iter().zip(drawn).all(|(a, v)| (0..3).all(|k| (a[k] - v.position[k]).abs() <= 0.05 + 1e-4 * a[k].abs()));
            if !same {
                continue;
            }
            let uvs = mesh.vertices.iter().map(|w| [w.vertex.tu, w.vertex.tv]).collect();
            self.animations.push(PrimitiveAnimation { part: p as u32, base_vertex: base as u32, frames, uvs, matrix, grouped: g });
        }
        self.animations.len()
    }

    /// Our own shapes for plungers: VPX draws a plunger as one
    /// mesh with a frame for each position (`shapes::plunger`), where the
    /// export has pieces the plunger's motion slides; each plunger's pieces
    /// give way to a draw per frame, looking as the first piece did, unmoved,
    /// shown while the plunger's position picks it, sorted from VPX's point
    /// for it. Returns how many plungers changed.
    pub fn use_own_plungers(&mut self, vpx: &VPX) -> usize {
        let surfaces = table_physics::Surfaces::of_table(vpx);
        let mut changed = 0;
        for (p, item) in vpx.gameitems.iter().enumerate() {
            let GameItemEnum::Plunger(plunger) = item else { continue };
            let p = p as u32;
            let Some(template) = self.draws.iter().find(|d| d.part == Some(p)).cloned() else { continue };
            let Some(shape) = crate::shapes::plunger::shape(&surfaces, plunger) else { continue };
            self.draws.retain(|d| d.part != Some(p));
            let count = shape.frames.len() as u32;
            let [x, y, z] = shape.sort_point;
            let sort_point = Vec3::new(x, y, z);
            for (k, frame) in shape.frames.iter().enumerate() {
                let base_vertex = self.vertices.len() as i32;
                let first_index = self.indices.len() as u32;
                for (i, pos) in frame.positions.iter().enumerate() {
                    self.vertices.push(Vertex { position: *pos, normal: frame.normals[i], uv: frame.uvs[i] });
                }
                self.indices.extend_from_slice(&frame.indices);
                self.draws.push(Draw {
                    first_index,
                    index_count: frame.indices.len() as u32,
                    base_vertex,
                    group: None,
                    frame: Some((k as u32, count)),
                    center: sort_point,
                    sort_point,
                    ..template.clone()
                });
            }
            changed += 1;
        }
        changed
    }
}

impl From<[f32; 3]> for Vec3 {
    fn from(v: [f32; 3]) -> Self {
        Vec3::new(v[0], v[1], v[2])
    }
}

/// A normal through a placement matrix (rotation and scale), renormalised.
fn normal_through(m: &Mat4, n: [f32; 3]) -> [f32; 3] {
    let x = m.m[0][0] * n[0] + m.m[1][0] * n[1] + m.m[2][0] * n[2];
    let y = m.m[0][1] * n[0] + m.m[1][1] * n[1] + m.m[2][1] * n[2];
    let z = m.m[0][2] * n[0] + m.m[1][2] * n[1] + m.m[2][2] * n[2];
    let len = (x * x + y * y + z * z).sqrt();
    if len > 0.0 { [x / len, y / len, z / len] } else { [0.0, 0.0, 1.0] }
}

/// Where each moving part is now, from the physics.
#[derive(Clone, Copy, Debug)]
pub enum PartState {
    /// A flipper's angle, in radians.
    Flipper(f32),
    /// A gate's or spinner's angle, in radians.
    Axle(f32),
    /// A plunger's distance from where it started, along y.
    Plunger(f32),
}

impl Motion {
    /// The matrix that moves the exported mesh to the part's current state.
    pub fn transform(&self, state: PartState) -> Mat4 {
        match (*self, state) {
            (Motion::Flipper { center, start_angle }, PartState::Flipper(angle)) => {
                Mat4::translate(-center.x, -center.y, 0.0) * Mat4::rotate_z(angle - start_angle) * Mat4::translate(center.x, center.y, 0.0)
            }
            (Motion::Axle { pivot, rotation, sign }, PartState::Axle(angle)) => {
                let r = rotation.to_radians();
                Mat4::translate(-pivot.x, -pivot.y, -pivot.z)
                    * Mat4::rotate_z(-r)
                    * Mat4::rotate_x(sign * angle)
                    * Mat4::rotate_z(r)
                    * Mat4::translate(pivot.x, pivot.y, pivot.z)
            }
            (Motion::Plunger, PartState::Plunger(dy)) => Mat4::translate(0.0, dy, 0.0),
            _ => Mat4::IDENTITY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Mirror, Refraction};

    #[test]
    fn mirrors_read_from_vpin_probes() {
        // vpin's own description of a probe with nothing set
        let text = format!("{:?}", vpin::vpx::renderprobe::RenderProbe::default());
        let blank = Mirror { name: String::new(), plane: [0.0; 4], roughness: 0, mode: 0, no_light_maps: false };
        assert_eq!(Mirror::from_probe_text(&text), Some(blank));
        // as VPX 10.8 saves one, a name with quotes in it
        let text = r#"RenderProbe { type_: PlaneReflection, name: "Right \"Side\" Reflections", roughness: 8, roughness_clear: None, reflection_plane: Vertex4D { x: -1.0, y: 0.0, z: 0.0, w: 955.0 }, reflection_mode: Dynamic, disable_light_reflection: Some(true) }"#;
        let side = Mirror { name: r#"Right "Side" Reflections"#.into(), plane: [-1.0, 0.0, 0.0, 955.0], roughness: 8, mode: 5, no_light_maps: true };
        assert_eq!(Mirror::from_probe_text(text), Some(side));
        let text = r#"RenderProbe { type_: PlaneReflection, name: "Odd", roughness: 0, roughness_clear: Some(2), reflection_plane: Vertex4D { x: 0.0, y: 0.0, z: 1.0, w: 0.0 }, reflection_mode: Other(9), disable_light_reflection: None }"#;
        assert_eq!(Mirror::from_probe_text(text).map(|m| m.mode), Some(9));
        // refraction probes aren't mirrors
        let text = r#"RenderProbe { type_: ScreenSpaceTransparency, name: "Refraction 1", roughness: 1, roughness_clear: None, reflection_plane: Vertex4D { x: 0.0, y: 0.0, z: 1.0, w: 0.0 }, reflection_mode: Dynamic, disable_light_reflection: Some(false) }"#;
        assert_eq!(Mirror::from_probe_text(text), None);
        assert_eq!(Refraction::from_probe_text(text), Some(Refraction { name: "Refraction 1".into(), roughness: 1 }));
        // and mirrors aren't refractions
        let text = format!("{:?}", vpin::vpx::renderprobe::RenderProbe::default());
        assert_eq!(Refraction::from_probe_text(&text), None);
    }
}
