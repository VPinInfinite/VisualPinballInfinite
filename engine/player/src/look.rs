// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): the
// table's materials as loaded (src/parts/pintable.cpp, `MATE` and `PHMA`) and as scripts read and
// change them (src/core/ScriptGlobalTable.cpp: UpdateMaterial, GetMaterial, MaterialColor,
// UpdateMaterialPhysics, GetMaterialPhysics), and the flasher's script setters and drawing state
// (src/parts/flasher.cpp, flasher.h).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! What scripts change about how parts look: their images and materials,
//! a primitive's lighting, a flasher's place, colour and pictures. Kept as
//! VPX's setters keep them, and handed to the renderer each frame.
//! A part no script has touched keeps the look its table file gives it.

use std::collections::{HashMap, HashSet};

use vpin::vpx::VPX;
use vpin::vpx::gameitem::flasher::{AddBlendMode, Filter, Flasher};
use vpin::vpx::gameitem::primitive::Primitive;
use vpin::vpx::material::MaterialType;

use crate::light::colorref;

/// A table material, as scripts read and change it (VPX's `Material`).
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialRt {
    pub name: String,
    pub wrap_lighting: f32,
    pub roughness: f32,
    pub glossy_image_lerp: f32,
    pub thickness: f32,
    pub edge: f32,
    pub edge_alpha: f32,
    pub opacity: f32,
    /// Colours as `COLORREF`s (red in the low byte).
    pub base: u32,
    pub glossy: u32,
    pub clearcoat: u32,
    pub metal: bool,
    pub opacity_active: bool,
    /// What it tints a refraction seen through it, as a `COLORREF` (10.8's
    /// materials; white before).
    pub refraction_tint: u32,
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    pub scatter_angle: f32,
    /// A script changed how it looks since play started: parts using it
    /// are drawn from these values rather than the table file's.
    pub changed: bool,
}

/// A 10.8 material's refraction tint, as a `COLORREF`: vpin 0.38 keeps it
/// to itself, so it's read from its debug text (white if that doesn't read).
fn refraction_tint(m: &vpin::vpx::material::Material) -> u32 {
    let text = format!("{m:?}");
    let channel = |key: &str| -> Option<u32> {
        let tint = &text[text.find("refraction_tint: Color {")?..];
        let at = tint.find(key)? + key.len();
        tint[at..].split([',', ' ', '}']).next()?.parse().ok()
    };
    match (channel(" r: "), channel(" g: "), channel(" b: ")) {
        (Some(r), Some(g), Some(b)) => (r & 0xFF) | (g & 0xFF) << 8 | (b & 0xFF) << 16,
        _ => 0xFF_FF_FF,
    }
}

/// VPX's `dequantizeUnsigned<bits>`, for the old material record.
fn dequantize(i: u8, bits: u32) -> f32 {
    (f32::from(i) / ((1u32 << bits) - 1) as f32).min(1.0)
}

/// The table's materials, found by name ignoring case, as VPX finds them.
#[derive(Clone, Debug, Default)]
pub struct Materials {
    pub all: Vec<MaterialRt>,
    by_name: HashMap<String, u32>,
}

impl Materials {
    /// As VPX loads them: the 10.8 records when the table has them, else
    /// the old ones with their physics matched by name.
    pub fn new(vpx: &VPX) -> Materials {
        let mut all = Vec::new();
        match &vpx.gamedata.materials {
            Some(mats) if !mats.is_empty() => {
                for m in mats {
                    all.push(MaterialRt {
                        name: m.name.clone(),
                        wrap_lighting: m.wrap_lighting,
                        roughness: m.roughness,
                        glossy_image_lerp: m.glossy_image_lerp,
                        thickness: m.thickness,
                        edge: m.edge,
                        edge_alpha: m.edge_alpha,
                        opacity: m.opacity,
                        base: colorref(&m.base_color),
                        glossy: colorref(&m.glossy_color),
                        clearcoat: colorref(&m.clearcoat_color),
                        metal: m.type_ == MaterialType::Metal,
                        opacity_active: m.opacity_active,
                        refraction_tint: refraction_tint(m),
                        elasticity: m.elasticity,
                        elasticity_falloff: m.elasticity_falloff,
                        friction: m.friction,
                        scatter_angle: m.scatter_angle,
                        changed: false,
                    });
                }
            }
            _ => {
                for m in &vpx.gamedata.materials_old {
                    all.push(MaterialRt {
                        name: m.name.clone(),
                        wrap_lighting: m.wrap_lighting,
                        roughness: m.roughness,
                        // '1 -' and 0 → 0.05: VPX's compatibility with older tables
                        glossy_image_lerp: 1.0 - dequantize(m.glossy_image_lerp, 8),
                        thickness: if m.thickness == 0 { 0.05 } else { dequantize(m.thickness, 8) },
                        edge: m.edge,
                        edge_alpha: dequantize(m.opacity_active_edge_alpha >> 1, 7),
                        opacity: m.opacity,
                        base: colorref(&m.base_color),
                        glossy: colorref(&m.glossy_color),
                        clearcoat: colorref(&m.clearcoat_color),
                        metal: m.is_metal,
                        opacity_active: m.opacity_active_edge_alpha & 1 != 0,
                        refraction_tint: 0xFF_FF_FF,
                        elasticity: 0.0,
                        elasticity_falloff: 0.0,
                        friction: 0.0,
                        scatter_angle: 0.0,
                        changed: false,
                    });
                }
                for p in vpx.gamedata.materials_physics_old.iter().flatten() {
                    if let Some(m) = all.iter_mut().find(|m| m.name.eq_ignore_ascii_case(&p.name)) {
                        m.elasticity = p.elasticity;
                        m.elasticity_falloff = p.elasticity_falloff;
                        m.friction = p.friction;
                        m.scatter_angle = p.scatter_angle;
                    }
                }
            }
        }
        let mut by_name = HashMap::new();
        for (i, m) in all.iter().enumerate() {
            // the last of a name wins: VPX's lookup while playing ignores case, a later material
            // of the same name replacing an earlier one (PinTable::SetupLookUpTables, GetMaterial)
            by_name.insert(m.name.to_ascii_lowercase(), i as u32);
        }
        Materials { all, by_name }
    }

    pub fn find(&self, name: &str) -> Option<u32> {
        if name.is_empty() {
            return None;
        }
        self.by_name.get(&name.to_ascii_lowercase()).copied()
    }
}

/// The table's images by name, as VPX finds them while playing: its load
/// drops an image named exactly as an earlier one ("Duplicate image name
/// found"), and its lookup ignores case, a later image of the same name
/// replacing an earlier one (`PinTable::LoadData`, `SetupLookUpTables`,
/// `GetImage`; build 5436). Keyed by the name in lower case.
pub fn images_by_name(vpx: &VPX) -> HashMap<String, u32> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut by_name = HashMap::new();
    for (i, image) in vpx.images.iter().enumerate() {
        if seen.insert(image.name.as_str()) {
            by_name.insert(image.name.to_ascii_lowercase(), i as u32);
        }
    }
    by_name
}

/// The table image VPX finds by `name` while playing (see [`images_by_name`]),
/// as an index into `vpx.images`.
pub fn image_named(vpx: &VPX, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    let images = &vpx.images;
    (0..images.len()).rev().find(|&i| images[i].name.eq_ignore_ascii_case(name) && !images[..i].iter().any(|e| e.name == images[i].name))
}

/// What VPX's basic shader takes from a material (`Shader::SetMaterial`,
/// `material.sh`), for VPX's look: its colours as floats, the rest as stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shading {
    pub base: [f32; 3],
    pub glossy: [f32; 3],
    pub clearcoat: [f32; 3],
    /// VPX's roughness, 0..1 (the shader's glossy power is 2^(10 r + 1)).
    pub roughness: f32,
    pub wrap_lighting: f32,
    pub glossy_image_lerp: f32,
    pub thickness: f32,
    pub edge: f32,
    pub edge_alpha: f32,
    pub opacity: f32,
    pub metal: bool,
    pub opacity_active: bool,
    /// What it tints a refraction seen through it (0..1, as stored).
    pub refraction_tint: [f32; 3],
}

/// A primitive's or target's "disable lighting" for its top, as VPX reads
/// it (`Primitive::LoadToken`, `HitTarget::LoadToken`): 10.8's value
/// (`DILT`), else the older one (`DILI`, a byte), in which VPX reads a stored
/// 1, the old yes, as fully off.
pub fn disable_lighting_top(new: Option<f32>, old: Option<f32>) -> f32 {
    new.unwrap_or_else(|| old.map_or(0.0, |v| if (v - 1.0 / 255.0).abs() < 1e-6 { 1.0 } else { v }))
}

/// A `COLORREF` as VPX's `convertColor` reads it: red in the low byte.
fn color_of(c: u32) -> [f32; 3] {
    [(c & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, ((c >> 16) & 0xFF) as f32 / 255.0]
}

impl Shading {
    /// VPX's `Material()`: what a part with a material name the table
    /// doesn't have is drawn with (`PinTable::m_dummyMaterial`, the editor's
    /// default colour, hot pink).
    pub fn dummy() -> Shading {
        Shading {
            base: color_of(0xB469FF),
            glossy: [0.0; 3],
            clearcoat: [0.0; 3],
            roughness: 0.0,
            wrap_lighting: 0.0,
            glossy_image_lerp: 1.0,
            thickness: 0.05,
            edge: 1.0,
            edge_alpha: 1.0,
            opacity: 1.0,
            metal: false,
            opacity_active: false,
            refraction_tint: [1.0; 3],
        }
    }

    /// The material VPX gives a light's socket (`Light::Render`).
    pub fn light_socket() -> Shading {
        Shading { base: color_of(0x181818), glossy: color_of(0xB4B4B4), roughness: 0.9, wrap_lighting: 0.5, ..Shading::dummy() }
    }

    /// The material VPX gives a light's bulb (`Light::Render`): see-through glass.
    pub fn light_bulb() -> Shading {
        Shading {
            base: [0.0; 3],
            glossy: [1.0; 3],
            clearcoat: [1.0; 3],
            roughness: 0.9,
            wrap_lighting: 0.5,
            opacity: 0.2,
            opacity_active: true,
            ..Shading::dummy()
        }
    }

    /// The material VPX gives a spinner's bracket (`Spinner::Render`).
    pub fn spinner_bracket() -> Shading {
        Shading {
            base: color_of(0x20202020),
            glossy: color_of(0x60606060),
            clearcoat: color_of(0x20202020),
            roughness: 0.4,
            wrap_lighting: 0.9,
            metal: true,
            ..Shading::dummy()
        }
    }

    /// The material VPX gives a bumper's ring when it names none
    /// (`Bumper::Render`): white metal.
    pub fn bumper_ring() -> Shading {
        Shading { base: color_of(0xFFFFFFFF), glossy: [0.0; 3], metal: true, ..Shading::dummy() }
    }
}

/// How a material looks now, for the renderer: the same reading of VPX's
/// material that `vpin`'s export makes for the table's own meshes, so a
/// changed material draws like an unchanged one with its new values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialNow {
    /// The base colour (0..1, as stored), and its opacity when active.
    pub base_color: [f32; 4],
    /// Drawn blended: VPX blends only an active opacity below 0.999.
    pub blend: bool,
    /// Its opacity is active: only then does VPX blend at all (a picture's
    /// alpha included).
    pub opacity_active: bool,
    pub metallic: f32,
    /// Rough (1) to shiny (0): VPX's roughness turned around.
    pub roughness: f32,
    /// All of it, as VPX's look draws it.
    pub shading: Shading,
}

impl MaterialRt {
    pub fn now(&self) -> MaterialNow {
        let c = |k: u32| ((self.base >> (8 * k)) & 0xFF) as f32 / 255.0;
        MaterialNow {
            base_color: [c(0), c(1), c(2), if self.opacity_active { self.opacity } else { 1.0 }],
            blend: self.opacity_active && self.opacity < 0.999,
            opacity_active: self.opacity_active,
            metallic: if self.metal { 1.0 } else { 0.0 },
            roughness: 1.0 - self.roughness,
            shading: self.shading(),
        }
    }

    pub fn shading(&self) -> Shading {
        Shading {
            base: color_of(self.base),
            glossy: color_of(self.glossy),
            clearcoat: color_of(self.clearcoat),
            roughness: self.roughness,
            wrap_lighting: self.wrap_lighting,
            glossy_image_lerp: self.glossy_image_lerp,
            thickness: self.thickness,
            edge: self.edge,
            edge_alpha: self.edge_alpha,
            opacity: self.opacity,
            metal: self.metal,
            opacity_active: self.opacity_active,
            refraction_tint: color_of(self.refraction_tint),
        }
    }
}

/// An image or material slot as a script left it: `None` keeps the table
/// file's; `Some(None)` names something the table doesn't have (VPX then
/// draws without it).
pub type Slot = Option<Option<u32>>;

/// What a script changed about a part's look (VPX's `put_Image`,
/// `put_Material`, `put_SideImage`, `put_TopMaterial`, `put_SideMaterial`,
/// `put_BlendDisableLighting`, `put_BlendDisableLightingFromBelow`,
/// `put_SideVisible`, `put_ReflectionEnabled`, `put_ReflectionProbe`). The
/// names are kept for reading back; the renderer gets the indices.
#[derive(Clone, Debug, Default)]
pub struct PartLook {
    pub image: Option<String>,
    pub side_image: Option<String>,
    pub material: Option<String>,
    pub side_material: Option<String>,
    /// A primitive's normal map as the script set it (`NormalMap`), by name and picture.
    pub normal_map: Option<String>,
    pub normal_map_index: Slot,
    pub image_index: Slot,
    pub side_image_index: Slot,
    pub material_index: Slot,
    pub side_material_index: Slot,
    /// The table file's materials for the part's two slots.
    pub file_material: Option<u32>,
    pub file_side_material: Option<u32>,
    pub disable_lighting: Option<f32>,
    pub disable_lighting_below: Option<f32>,
    pub side_visible: Option<bool>,
    /// Drawn into mirrors.
    pub reflected: Option<bool>,
    /// The mirror a primitive shows, by name, and as an index into the
    /// table's mirrors.
    pub mirror: Option<String>,
    pub mirror_index: Slot,
    /// The refraction a primitive shows, by name, and as an index into the
    /// table's refraction probes.
    pub refraction: Option<String>,
    pub refraction_index: Slot,
    /// A primitive's depth bias, where it sorts among see-through parts (`DepthBias`).
    pub depth_bias: Option<f32>,
}

impl PartLook {
    /// Whether anything about the part's look is the script's.
    pub fn touched(&self, materials: &Materials) -> bool {
        let changed = |m: Option<u32>| m.is_some_and(|m| materials.all[m as usize].changed);
        self.image_index.is_some()
            || self.side_image_index.is_some()
            || self.normal_map_index.is_some()
            || self.material_index.is_some()
            || self.side_material_index.is_some()
            || self.disable_lighting.is_some()
            || self.disable_lighting_below.is_some()
            || self.side_visible.is_some()
            || self.reflected.is_some()
            || self.mirror_index.is_some()
            || self.refraction_index.is_some()
            || self.depth_bias.is_some()
            || changed(self.file_material)
            || changed(self.file_side_material)
    }

    /// The look for the renderer: only what differs from the table file.
    pub fn now(&self, materials: &Materials) -> LookNow {
        let material = |slot: Slot, file: Option<u32>| match slot {
            Some(Some(m)) => Some(materials.all[m as usize].now()),
            // VPX's dummy material: the editor's default colour (hot pink), roughness 0
            Some(None) => Some(MaterialNow {
                base_color: [1.0, 105.0 / 255.0, 180.0 / 255.0, 1.0],
                blend: false,
                opacity_active: false,
                metallic: 0.0,
                roughness: 1.0,
                shading: Shading::dummy(),
            }),
            None => file.filter(|&m| materials.all[m as usize].changed).map(|m| materials.all[m as usize].now()),
        };
        LookNow {
            image: self.image_index,
            normal_map: self.normal_map_index,
            side_image: self.side_image_index,
            material: material(self.material_index, self.file_material),
            side_material: material(self.side_material_index, self.file_side_material),
            disable_lighting: self.disable_lighting,
            disable_lighting_below: self.disable_lighting_below,
            side_visible: self.side_visible,
            reflected: self.reflected,
            mirror: self.mirror_index,
            refraction: self.refraction_index,
            depth_bias: self.depth_bias,
        }
    }
}

/// A part's look as a script changed it, for the renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookNow {
    pub image: Slot,
    /// The normal map the script set (`Some(None)`: none).
    pub normal_map: Slot,
    pub side_image: Slot,
    pub material: Option<MaterialNow>,
    pub side_material: Option<MaterialNow>,
    pub disable_lighting: Option<f32>,
    pub disable_lighting_below: Option<f32>,
    pub side_visible: Option<bool>,
    pub reflected: Option<bool>,
    /// The mirror it shows instead of the table file's (`Some(None)`: none).
    pub mirror: Slot,
    /// The refraction it shows instead of the table file's.
    pub refraction: Slot,
    /// Where it sorts among see-through parts instead of the table file's.
    pub depth_bias: Option<f32>,
}

/// A flasher's state while playing: what its script setters change
/// (VPX's `FlasherData` during play).
#[derive(Clone, Debug)]
pub struct FlasherRt {
    /// Where its outline's bounds are centred now (`m_vCenter`, kept by
    /// `MoveOffset`), and where they were when play started.
    pub center: [f32; 2],
    pub start_center: [f32; 2],
    pub height: f32,
    pub rot: [f32; 3],
    pub color: u32,
    pub alpha: i32,
    pub intensity_scale: f32,
    pub modulate_vs_add: f32,
    pub image_a: String,
    pub image_b: String,
    pub image_a_index: Option<u32>,
    pub image_b_index: Option<u32>,
    /// 0 none, 1 additive, 2 overlay, 3 multiply, 4 screen (VPX's `Filters`).
    pub filter: u32,
    pub amount: i32,
    pub add_blend: bool,
    pub depth_bias: f32,
    pub display_texture: bool,
    /// The light whose brightness scales the flasher's (`m_szLightmap`).
    pub lightmap: Option<u32>,
}

impl FlasherRt {
    /// `center` is the outline's bounds' centre (VPX's `UpdateCenter`);
    /// `images` finds a table image by name.
    pub fn new(f: &Flasher, center: [f32; 2], images: &dyn Fn(&str) -> Option<u32>, lightmap: Option<u32>) -> FlasherRt {
        FlasherRt {
            center,
            start_center: center,
            height: f.height,
            rot: [f.rot_x, f.rot_y, f.rot_z],
            color: colorref(&f.color),
            alpha: f.alpha,
            intensity_scale: 1.0,
            modulate_vs_add: f.modulate_vs_add,
            image_a: f.image_a.clone(),
            image_b: f.image_b.clone(),
            image_a_index: images(&f.image_a),
            image_b_index: images(&f.image_b),
            filter: filter_number(&f.filter),
            amount: f.filter_amount as i32,
            // build 5436 reads the setting as on or off
            add_blend: !matches!(f.add_blend, AddBlendMode::None),
            depth_bias: f.depth_bias,
            display_texture: f.display_texture,
            lightmap,
        }
    }

    /// The flasher for the renderer; `light_factor` is what its light map
    /// leaves of its opacity (1 without one).
    pub fn now(&self, light_factor: f32) -> FlasherNow {
        FlasherNow {
            offset: [self.center[0] - self.start_center[0], self.center[1] - self.start_center[1]],
            height: self.height,
            rot: self.rot,
            color: self.color,
            alpha: self.alpha as f32 * light_factor,
            intensity_scale: self.intensity_scale,
            modulate_vs_add: self.modulate_vs_add,
            image_a: self.image_a_index,
            image_b: self.image_b_index,
            filter: self.filter,
            amount: self.amount,
            add_blend: self.add_blend,
            depth_bias: self.depth_bias,
        }
    }

    /// VPX's `put_Filter`: a known name, ignoring case; anything else
    /// leaves it as it was.
    pub fn set_filter(&mut self, name: &str) {
        let n = match name.to_lowercase().as_str() {
            "none" => 0,
            "additive" => 1,
            "overlay" => 2,
            "multiply" => 3,
            "screen" => 4,
            _ => return,
        };
        self.filter = n;
    }

    /// VPX's `get_Filter`.
    pub fn filter_name(&self) -> &'static str {
        match self.filter {
            1 => "Additive",
            2 => "Overlay",
            3 => "Multiply",
            4 => "Screen",
            _ => "None",
        }
    }
}

/// A flasher's outline on the table, as VPX traces it from its drag
/// points (`GetRgVertex`: a closed loop, accuracy 4).
pub fn flasher_outline(f: &Flasher) -> Vec<[f32; 2]> {
    use physics_compat::math::{Vertex2D, Vertex3Ds};
    use physics_compat::setup::curves::{DragPoint, get_rg_vertex};
    let points: Vec<DragPoint> =
        f.drag_points.iter().map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) }).collect();
    get_rg_vertex::<Vertex2D>(&points, true, 4.0).iter().map(|v| [v.v.x, v.v.y]).collect()
}

/// The centre of an outline's bounds (VPX's `UpdateCenter`).
pub fn bounds_center(outline: &[[f32; 2]]) -> [f32; 2] {
    let (mut min, mut max) = ([f32::MAX; 2], [-f32::MAX; 2]);
    for p in outline {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    [0.5 * (min[0] + max[0]), 0.5 * (min[1] + max[1])]
}

fn filter_number(f: &Filter) -> u32 {
    match f {
        Filter::None => 0,
        Filter::Additive => 1,
        Filter::Overlay => 2,
        Filter::Multiply => 3,
        Filter::Screen => 4,
        _ => 0,
    }
}

/// A primitive's colour, opacity and blending as play goes on (VPX's
/// `Color`, `Opacity`, `AddBlend`), and the light whose brightness scales
/// its opacity (its light map, `m_szLightmap`): how tables' baked light
/// maps fade with their lights.
#[derive(Clone, Debug)]
pub struct PrimitiveRt {
    pub color: u32,
    /// 0 to 100 (VPX's `m_alpha`).
    pub alpha: f32,
    pub add_blend: bool,
    pub lightmap: Option<u32>,
}

impl PrimitiveRt {
    /// Tables saved before VPX had these settings get its defaults: white,
    /// fully opaque, not added.
    pub fn new(p: &Primitive, lightmap: Option<u32>) -> PrimitiveRt {
        PrimitiveRt {
            color: p.color.as_ref().map_or(0xFF_FFFF, colorref),
            alpha: p.alpha.unwrap_or(100.0),
            add_blend: p.add_blend.unwrap_or(false),
            lightmap,
        }
    }

    /// The primitive for the renderer; `light_factor` is what its light map
    /// leaves of its opacity (1 without one).
    pub fn now(&self, light_factor: f32) -> PrimitiveNow {
        PrimitiveNow { color: self.color, alpha: self.alpha * light_factor, add_blend: self.add_blend }
    }
}

/// A primitive's colour and opacity as it shows now, for the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrimitiveNow {
    pub color: u32,
    /// 0 to 100, after its light map.
    pub alpha: f32,
    pub add_blend: bool,
}

impl PrimitiveNow {
    /// What VPX's `Primitive::Render` hands its shaders as
    /// `staticColor_Alpha`: the colour and the opacity as a fraction,
    /// with the colour multiplied by the opacity when it is added.
    pub fn static_color_alpha(&self) -> [f32; 4] {
        let c = |shift: u32| ((self.color >> shift) & 0xFF) as f32 / 255.0;
        let a = self.alpha / 100.0;
        if self.add_blend { [c(0) * a, c(8) * a, c(16) * a, a] } else { [c(0), c(8), c(16), a] }
    }

    /// VPX doesn't draw an added primitive that is black or faded out.
    pub fn adds_nothing(&self) -> bool {
        self.add_blend && (self.color == 0 || self.alpha == 0.0)
    }
}

/// A flasher as it shows now, for the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlasherNow {
    /// How far a script moved it from where play started.
    pub offset: [f32; 2],
    pub height: f32,
    /// Its turns about x, y and z, in degrees.
    pub rot: [f32; 3],
    pub color: u32,
    /// Its opacity (0..100 and beyond), after its light map.
    pub alpha: f32,
    pub intensity_scale: f32,
    pub modulate_vs_add: f32,
    /// Table images, as indices into `vpx.images`.
    pub image_a: Option<u32>,
    pub image_b: Option<u32>,
    pub filter: u32,
    pub amount: i32,
    pub add_blend: bool,
    pub depth_bias: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_materials_refraction_tint_reads_from_vpins_description() {
        let json = r##"{"name":"m","type_":"basic","wrap_lighting":0.0,"roughness":0.0,"glossy_image_lerp":1.0,"thickness":0.05,"edge":1.0,"edge_alpha":1.0,"opacity":1.0,"base_color":"#000000","glossy_color":"#000000","clearcoat_color":"#000000","opacity_active":false,"elasticity":0.0,"elasticity_falloff":0.0,"friction":0.0,"scatter_angle":0.0,"refraction_tint":"#ff8001"}"##;
        let m: vpin::vpx::material::Material = serde_json::from_str(json).expect("a material");
        assert_eq!(refraction_tint(&m), 0x01_80_FF);
    }

    fn material(opacity_active: bool, opacity: f32) -> MaterialRt {
        MaterialRt {
            name: "M".into(),
            wrap_lighting: 0.0,
            roughness: 0.25,
            glossy_image_lerp: 1.0,
            thickness: 0.05,
            edge: 1.0,
            edge_alpha: 1.0,
            opacity,
            base: 0x00_80_40_20,
            glossy: 0,
            clearcoat: 0,
            metal: true,
            refraction_tint: 0xFF_FF_FF,
            opacity_active,
            elasticity: 0.0,
            elasticity_falloff: 0.0,
            friction: 0.0,
            scatter_angle: 0.0,
            changed: false,
        }
    }

    #[test]
    fn material_reads_as_the_export_does() {
        let now = material(true, 0.5).now();
        assert_eq!(now.base_color, [0x20 as f32 / 255.0, 0x40 as f32 / 255.0, 0x80 as f32 / 255.0, 0.5]);
        assert!(now.blend);
        assert_eq!((now.metallic, now.roughness), (1.0, 0.75));
        // an active opacity of 0.999 and above isn't blended; an inactive one is opaque
        assert!(!material(true, 0.999).now().blend);
        assert_eq!(material(false, 0.5).now().base_color[3], 1.0);
    }

    #[test]
    fn untouched_parts_keep_the_file_look() {
        let mut materials = Materials { all: vec![material(false, 1.0)], by_name: HashMap::new() };
        let mut look = PartLook { file_material: Some(0), ..PartLook::default() };
        assert!(!look.touched(&materials));
        assert_eq!(look.now(&materials).material, None);
        // UpdateMaterial on the part's own material: drawn from its new values
        materials.all[0].changed = true;
        materials.all[0].opacity_active = true;
        materials.all[0].opacity = 0.25;
        assert!(look.touched(&materials));
        assert_eq!(look.now(&materials).material.map(|m| m.base_color[3]), Some(0.25));
        // an image the table doesn't have
        look.image_index = Some(None);
        assert_eq!(look.now(&materials).image, Some(None));
    }

    #[test]
    fn filter_names_follow_vpx() {
        let f = Flasher::default();
        let mut rt = FlasherRt::new(&f, [0.0; 2], &|_| None, None);
        rt.set_filter("MULTIPLY");
        assert_eq!((rt.filter, rt.filter_name()), (3, "Multiply"));
        rt.set_filter("sideways");
        assert_eq!(rt.filter, 3);
    }

    #[test]
    fn primitives_hand_vpx_their_colour_and_opacity() {
        // an old table's primitive: white, opaque, not added
        let rt = PrimitiveRt::new(&Primitive::default(), None);
        assert_eq!((rt.color, rt.alpha, rt.add_blend), (0xFF_FFFF, 100.0, false));
        assert_eq!(rt.now(1.0).static_color_alpha(), [1.0; 4]);
        // drawn lit: the colour as it is, the opacity as a fraction
        let lit = PrimitiveRt { color: 0x00_00_FF, alpha: 50.0, add_blend: false, lightmap: None };
        assert_eq!(lit.now(1.0).static_color_alpha(), [1.0, 0.0, 0.0, 0.5]);
        // added: the colour times the opacity, after the light map's share
        let added = PrimitiveRt { color: 0x00_FF_00, alpha: 100.0, add_blend: true, lightmap: Some(3) };
        assert_eq!(added.now(0.25).static_color_alpha(), [0.0, 0.25, 0.0, 0.25]);
        // VPX skips an added primitive that is faded out or black
        assert!(added.now(0.0).adds_nothing());
        assert!(PrimitiveRt { color: 0, ..added.clone() }.now(1.0).adds_nothing());
        assert!(!added.now(0.5).adds_nothing());
        assert!(!lit.now(0.0).adds_nothing());
    }
}
