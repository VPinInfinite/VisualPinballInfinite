// Follows Visual Pinball X (license:GPLv3+), commit af26b2d (10.8.1 pre-release build 5436): the
// PhysicsEngine constructor (src/physics/PhysicsEngine.cpp), PinTable::GetSurfaceHeight and the part
// list's order after loading (src/parts/pintable.cpp), each part's defaults and PhysicSetup
// (src/parts/*.cpp, src/core/Settings_properties.inl), and the player's physics setup
// (src/core/player.cpp: the physics sets, the implicit playfield mesh, gravity).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A converted table's compatibility physics: the colliders VPX builds for a
//! table, made from the table's parts as `vpin` reads them, in VPX's order.
//!
//! VPX sets parts up in the order of its part list. After loading, that list
//! holds the table's part groups first, then every other part in file order;
//! groups have no colliders, so file order is VPX's order. Part ids here are
//! positions in `vpx.gameitems`.
//!
//! Missing values (older tables) take VPX's defaults for a loaded part, and
//! VPX's load-time repairs apply (old flipper heights, rubbers without a hit
//! height, playfield meshes before 10.7.1, the triangle order of meshes
//! before 10.1.1).
//!
//! Not handled: walls with VP8's "outer" setting (`INNR`), which `vpin`
//! doesn't read, and nested part groups' spaces (`vpin` doesn't read a
//! group's parent).

pub mod materials;
pub mod settings;
pub mod view_bounds;
pub mod vpp;


use physics_compat::engine::{Collider, PhysicsEngine, PlayfieldMaterial, TableBox};
use physics_compat::env::{PartId, PartSet};
use physics_compat::flipper::FlipperData;
use physics_compat::kicker::KickerData;
use physics_compat::math::def::ang_to_rad;
use physics_compat::math::{Vertex2D, Vertex3Ds};
use physics_compat::physconst::GRAVITYCONST;
use physics_compat::plunger::PlungerData;
use physics_compat::quadtree::{Owner, OwnerKind};
use physics_compat::setup::curves::DragPoint;
use physics_compat::setup::parts::{
    BumperSetup, FlipperSetup, GateSetup, KickerSetup, PlungerSetup, SpinnerSetup, TriggerSetup, bumper_colliders,
    flipper_colliders, gate_colliders, kicker_colliders, plunger_colliders, spinner_colliders, trigger_colliders,
};
use physics_compat::setup::forsyth::reorder_forsyth;
use physics_compat::setup::primitive::{Mesh, PrimitiveData, builtin_mesh, primitive_colliders};
use physics_compat::setup::ramp::{RampData, RampType, ramp_colliders, ramp_surface_height};
use physics_compat::setup::rubber::{RubberData, rubber_colliders};
use physics_compat::setup::surface::{WallData, wall_colliders};
use physics_compat::setup::target::{HitTargetData, TargetType, hit_target_colliders};
use physics_compat::setup::{Colliders, PhysicsMaterial};
use vpin::vpx::VPX;
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::gameitem::dragpoint::DragPoint as VpinDragPoint;
use vpin::vpx::gameitem::hittarget::{HitTarget, TargetType as VpinTargetType};
use vpin::vpx::gameitem::partgroup::SpaceReference;
use vpin::vpx::gameitem::primitive::Primitive;
use vpin::vpx::gameitem::ramp::{Ramp, RampType as VpinRampType};
use vpin::vpx::gameitem::rubber::Rubber;
use vpin::vpx::gameitem::trigger::TriggerShape;
use vpin::vpx::gameitem::vertex2d::Vertex2D as VpinVertex2D;
use vpin::vpx::gameitem::wall::Wall;

use materials::Materials;
use settings::PhysicsSettings;

/// The table itself, as the owner of its outer walls and glass.
pub const TABLE: PartId = PartId(u32::MAX);

/// A table's physics, ready for [`physics_compat::engine::PhysicsEngine::new`]
/// and the settings VPX's player applies before play.
pub struct TablePhysics {
    pub table: TableBox,
    /// The playfield's material: the table's, or its physics set's.
    pub playfield: PlayfieldMaterial,
    /// For `set_gravity`: the playfield's slope in degrees, between the
    /// table's two slopes by difficulty, and the gravity's strength.
    pub slope_deg: f32,
    pub gravity: f32,
    /// The difficulty in play (`m_globalDifficulty`).
    pub global_difficulty: f32,
    /// The table's physics set's slopes and gravity (min, max, strength), if
    /// it uses one: VPX's script setters for gravity and slope take them.
    pub physics_set_gravity: Option<(f32, f32, f32)>,
    /// Extra random scatter on every contact, in radians (`c_hardScatter`):
    /// only a table using a physics set has any.
    pub hard_scatter: f32,
    /// Without a `playfield_mesh` primitive, the flat playfield is hit tested.
    pub implicit_playfield_mesh: bool,
    /// Targets saved as dropped (`IsDropped`): the physics skips them until
    /// they're raised.
    pub dropped_targets: PartSet,
    /// Primitives saved with collisions off (`Collidable`): the collision
    /// search skips the parts of its tree that hold only them.
    pub uncollidable_primitives: PartSet,
    /// VPX's pending list: each part's colliders, parts in VPX's order. The
    /// engine adds the table's own.
    pub colliders: Vec<Collider>,
    /// Parts with colliders this crate doesn't build yet: position and type.
    pub not_built: Vec<(usize, &'static str)>,
}

impl TablePhysics {
    /// The physics engine, set up for play as VPX's player sets it up: the
    /// table's own colliders added, the flat playfield tested when the table
    /// has no playfield mesh, gravity, the contact scatter, the difficulty,
    /// and the parts that start dropped or without collisions. The random
    /// generator starts as in a new VPX process. Not started yet.
    pub fn into_engine(self) -> PhysicsEngine {
        let owner = Owner { id: TABLE.0, kind: OwnerKind::Other };
        let mut e = PhysicsEngine::new(&self.table, &self.playfield, owner, self.colliders, vec![]);
        e.set_implicit_playfield_mesh(self.implicit_playfield_mesh);
        e.set_gravity(self.slope_deg, self.gravity);
        e.env.playfield_slope_deg = self.slope_deg;
        e.env.global_difficulty = self.global_difficulty;
        e.env.hard_scatter = self.hard_scatter;
        e.env.dropped_targets = self.dropped_targets;
        e.env.uncollidable_primitives = self.uncollidable_primitives;
        e
    }
}

/// The table's colliders, as VPX builds them when play starts.
pub fn build(vpx: &VPX, settings: &PhysicsSettings) -> TablePhysics {
    let gd = &vpx.gamedata;
    let materials = Materials::from_vpx(vpx);
    let surfaces = Surfaces::new(vpx, &materials, settings);
    let difficulty = settings.difficulty.unwrap_or(gd.global_difficulty);

    let mut out = Vec::new();
    let mut not_built = Vec::new();
    let mut dropped_targets = PartSet::default();
    let mut uncollidable_primitives = PartSet::default();
    for (i, item) in vpx.gameitems.iter().enumerate() {
        if !in_playfield_space(vpx, item) {
            continue; // VPX gives parts placed in another space (cabinet, room) no physics
        }
        let mut c = Colliders { out: &mut out, part: PartId(i as u32), kind: OwnerKind::Other };
        match item {
            GameItemEnum::Wall(w) => wall_colliders(&wall(w, &materials), &mut c),
            GameItemEnum::Ramp(r) => ramp_colliders(&ramp(r, &materials), &mut c),
            GameItemEnum::Bumper(b) => {
                let s = BumperSetup {
                    center: v2(&b.center),
                    radius: b.radius,
                    height_scale: b.height_scale,
                    hit_event: b.hit_event.unwrap_or(true),
                    threshold: b.threshold,
                    force: b.force,
                    collidable: b.is_collidable.unwrap_or(true),
                    scatter: b.scatter.unwrap_or(0.0),
                };
                bumper_colliders(&s, surfaces.height(&b.surface, s.center), &mut c);
            }
            GameItemEnum::Gate(g) => {
                let s = GateSetup {
                    center: v2(&g.center),
                    length: g.length,
                    height: g.height,
                    rotation: g.rotation,
                    angle_min: g.angle_min,
                    angle_max: g.angle_max,
                    damping: g.damping.unwrap_or(0.985),
                    gravityfactor: g.gravity_factor.unwrap_or(0.25),
                    two_way: g.two_way,
                    collidable: g.is_collidable,
                    show_bracket: g.show_bracket,
                    elasticity: g.elasticity,
                    friction: g.friction,
                    scatter: 0.0, // VPX doesn't save a gate's scatter: it is always its default
                };
                gate_colliders(&s, surfaces.height(&g.surface, s.center), &mut c);
            }
            GameItemEnum::Spinner(s) => {
                let d = SpinnerSetup {
                    center: v2(&s.center),
                    length: s.length,
                    rotation: s.rotation,
                    height: s.height,
                    damping: s.damping,
                    angle_max: s.angle_max,
                    angle_min: s.angle_min,
                    elasticity: s.elasticity,
                    visible: s.is_visible,
                    show_bracket: s.show_bracket,
                };
                spinner_colliders(&d, surfaces.height(&s.surface, d.center), &mut c);
            }
            GameItemEnum::Trigger(t) => {
                let s = TriggerSetup {
                    center: v2(&t.center),
                    radius: t.radius,
                    hit_height: t.hit_height,
                    enabled: t.is_enabled,
                    round: matches!(t.shape, TriggerShape::Star | TriggerShape::Button),
                    points: drag_points(&t.drag_points),
                };
                trigger_colliders(&s, surfaces.height(&t.surface, s.center), &mut c);
            }
            GameItemEnum::Kicker(k) => {
                let s = KickerSetup {
                    center: v2(&k.center),
                    radius: k.radius,
                    hit_height: k.hit_height.unwrap_or(35.0),
                    enabled: k.is_enabled,
                    data: KickerData { legacy_mode: k.legacy_mode, hit_accuracy: k.hit_accuracy, fall_through: k.fall_through },
                };
                kicker_colliders(&s, surfaces.height(&k.surface, s.center), &mut c);
            }
            GameItemEnum::Flipper(f) => {
                let s = flipper(vpx, f, settings);
                flipper_colliders(&s, surfaces.height(&f.surface, s.center), difficulty, &mut c);
            }
            GameItemEnum::Plunger(p) => {
                let s = PlungerSetup {
                    v: v2(&p.center),
                    width: p.width,
                    height: p.height,
                    stroke: p.stroke,
                    park_position: p.park_position,
                    scatter_velocity: p.scatter_velocity,
                    data: PlungerData {
                        speed_fire: p.speed_fire,
                        mech_plunger: p.is_mech_plunger,
                        auto_plunger: p.auto_plunger,
                        mech_strength: p.mech_strength,
                        momentum_xfer: p.momentum_xfer,
                    },
                };
                plunger_colliders(&s, surfaces.height(&p.surface, s.v), &mut c);
            }
            GameItemEnum::Primitive(p) => match primitive(vpx, p, &materials, settings) {
                Ok(Some(d)) => {
                    if !d.collidable {
                        uncollidable_primitives.insert(PartId(i as u32));
                    }
                    c.kind = OwnerKind::Primitive;
                    primitive_colliders(&d, &mut c);
                }
                Ok(None) => {} // a toy: no physics
                Err(why) => not_built.push((i, why)),
            },
            GameItemEnum::Rubber(r) => rubber_colliders(&rubber(r, &materials), &mut c),
            GameItemEnum::HitTarget(t) => {
                if t.is_dropped {
                    dropped_targets.insert(PartId(i as u32));
                }
                c.kind = OwnerKind::HitTarget;
                hit_target_colliders(&hit_target(t, &materials), &mut c);
            }
            _ => {} // no colliders (balls join the physics separately)
        }
    }

    // glass was horizontal before 10.8; since, a table without a bottom height gets VPX's default
    let glass_bottom =
        if vpx.version.u32() < 1080 { gd.glass_top_height } else { gd.glass_bottom_height.unwrap_or(210.0) };
    let table = TableBox {
        left: gd.left,
        top: gd.top,
        right: gd.right,
        bottom: gd.bottom,
        glass_top_height: gd.glass_top_height,
        glass_bottom_height: glass_bottom,
    };

    // what VPX's player sets before play: the table's physics set, if it picks one
    let set = (gd.override_physics != 0)
        .then(|| settings.table_sets.get(gd.override_physics as usize - 1).copied().unwrap_or_default());
    let playfield = match set {
        Some(s) => PlayfieldMaterial {
            friction: s.contact_friction,
            elasticity: s.elasticity,
            elasticity_falloff: s.elasticity_falloff,
            scatter_deg: s.scatter_angle,
        },
        None => PlayfieldMaterial {
            friction: gd.friction,
            elasticity: gd.elasticity,
            elasticity_falloff: gd.elastic_falloff,
            scatter_deg: gd.scatter,
        },
    };
    let (min_slope, max_slope) = match set {
        Some(s) => (s.min_slope, s.max_slope),
        None => (gd.angle_tilt_min, gd.angle_tilt_max),
    };
    // VPX's lerp
    let slope_deg = (1.0 - difficulty) * min_slope + difficulty * max_slope;
    let gravity = match set {
        Some(s) => GRAVITYCONST * s.gravity_constant,
        None => gd.gravity,
    };
    let hard_scatter = set.map_or(0.0, |s| ang_to_rad(s.contact_scatter_angle));
    let implicit_playfield_mesh =
        !vpx.gameitems.iter().any(|g| matches!(g, GameItemEnum::Primitive(p) if p.is_playfield()));

    TablePhysics {
        table,
        playfield,
        slope_deg,
        gravity,
        global_difficulty: difficulty,
        physics_set_gravity: set.map(|s| (s.min_slope, s.max_slope, GRAVITYCONST * s.gravity_constant)),
        hard_scatter,
        implicit_playfield_mesh,
        dropped_targets,
        uncollidable_primitives,
        colliders: out,
        not_built,
    }
}

fn v2(v: &VpinVertex2D) -> Vertex2D {
    Vertex2D::new(v.x, v.y)
}

fn drag_points(points: &[VpinDragPoint]) -> Vec<DragPoint> {
    points
        .iter()
        .map(|p| DragPoint { v: Vertex3Ds::new(p.x, p.y, p.z), smooth: p.smooth, slingshot: p.is_slingshot.unwrap_or(false) })
        .collect()
}

/// VPX's `GetPartGroup()->GetReferenceSpace()` test: parts in a group placed
/// relative to the cabinet or the room get no physics. A group inheriting its
/// space follows its parent, which `vpin` doesn't read yet; a group at the
/// root inherits the playfield's.
fn in_playfield_space(vpx: &VPX, item: &GameItemEnum) -> bool {
    let Some(group) = item.part_group_name() else { return true };
    let space = vpx.gameitems.iter().find_map(|g| match g {
        GameItemEnum::PartGroup(pg) if pg.name == group => Some(&pg.space_reference),
        _ => None,
    });
    matches!(space, None | Some(SpaceReference::Playfield) | Some(SpaceReference::Inherit))
}

/// A part's physics: its own values, unless it uses a physics material
/// instead (the parts' `SetupHitObject`). Parts saved before physics
/// materials existed use their own.
fn physics(overwrite: Option<bool>, own: PhysicsMaterial, material: &Option<String>, materials: &Materials) -> PhysicsMaterial {
    if overwrite.unwrap_or(true) {
        own
    } else {
        let m = materials.get(material.as_deref().unwrap_or(""));
        PhysicsMaterial {
            elasticity: m.elasticity,
            elasticity_falloff: m.elasticity_falloff,
            friction: m.friction,
            scatter_deg: m.scatter_angle,
        }
    }
}

/// A wall's physics settings.
fn wall(w: &Wall, materials: &Materials) -> WallData {
    let own = PhysicsMaterial {
        elasticity: w.elasticity,
        elasticity_falloff: w.elasticity_falloff.unwrap_or(0.0),
        friction: w.friction,
        scatter_deg: w.scatter,
    };
    let material = physics(w.overwrite_physics, own, &w.physics_material, materials);
    WallData {
        points: drag_points(&w.drag_points),
        height_bottom: w.height_bottom,
        height_top: w.height_top,
        is_bottom_solid: w.is_bottom_solid,
        collidable: w.is_collidable,
        hit_event: w.hit_event,
        threshold: w.threshold,
        droppable: w.is_droppable,
        slingshot_force: w.slingshot_force,
        slingshot_threshold: w.slingshot_threshold,
        material,
    }
}

/// A ramp's physics settings. A ramp has no elasticity falloff.
fn ramp(r: &Ramp, materials: &Materials) -> RampData {
    let own = PhysicsMaterial { elasticity: r.elasticity, elasticity_falloff: 0.0, friction: r.friction, scatter_deg: r.scatter };
    let m = physics(r.overwrite_physics, own, &r.physics_material, materials);
    let (elasticity, friction, scatter) = (m.elasticity, m.friction, m.scatter_deg);
    RampData {
        points: drag_points(&r.drag_points),
        height_bottom: r.height_bottom,
        height_top: r.height_top,
        width_bottom: r.width_bottom,
        width_top: r.width_top,
        ramp_type: match r.ramp_type {
            VpinRampType::FourWire => RampType::FourWire,
            VpinRampType::TwoWire => RampType::TwoWire,
            VpinRampType::ThreeWireLeft => RampType::ThreeWireLeft,
            VpinRampType::ThreeWireRight => RampType::ThreeWireRight,
            VpinRampType::OneWire => RampType::OneWire,
            _ => RampType::Flat,
        },
        left_wall_height: r.left_wall_height,
        right_wall_height: r.right_wall_height,
        wire_diameter: r.wire_diameter,
        wire_distance_x: r.wire_distance_x,
        collidable: r.is_collidable,
        hit_event: r.hit_event.unwrap_or(false),
        threshold: r.threshold.unwrap_or(2.0),
        elasticity,
        friction,
        scatter,
    }
}

/// A primitive's physics settings, or `None` for a toy (no physics).
fn primitive(
    vpx: &VPX,
    p: &Primitive,
    materials: &Materials,
    settings: &PhysicsSettings,
) -> Result<Option<PrimitiveData>, &'static str> {
    let gd = &vpx.gamedata;
    let version = vpx.version.u32();
    let is_playfield = p.is_playfield();
    // playfield meshes were always forced as collidable until 10.7.1 (PinTable's load)
    let (toy, collidable) = if version < 1072 && is_playfield { (false, true) } else { (p.is_toy, p.is_collidable) };
    if toy {
        return Ok(None);
    }

    let mesh = if p.use_3d_mesh {
        let mut mesh = match p.read_mesh() {
            Ok(Some(m)) => Mesh {
                vertices: m
                    .vertices
                    .iter()
                    .map(|w| {
                        let b = &w.vpx_encoded_vertex;
                        let f = |i: usize| f32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
                        Vertex3Ds::new(f(0), f(4), f(8))
                    })
                    .collect(),
                indices: m.indices.iter().flat_map(|t| [t.i0 as u32, t.i1 as u32, t.i2 as u32]).collect(),
            },
            Ok(None) => Mesh::default(),
            Err(_) => return Err("primitive (unreadable mesh)"),
        };
        // VPX reorders the triangles of tables saved before 10.1.1 as it loads them
        if version < 1011
            && mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len())
            && let Some(indices) = reorder_forsyth(&mesh.indices, mesh.vertices.len())
        {
            mesh.indices = indices;
        }
        mesh
    } else {
        builtin_mesh(p.sides as i32, p.draw_textures_inside)
    };

    let material = if is_playfield {
        // the playfield mesh takes the playfield's physics, or the table's physics set's
        if gd.override_physics != 0 {
            let set = settings.table_sets.get(gd.override_physics as usize - 1).copied().unwrap_or_default();
            PhysicsMaterial {
                elasticity: set.elasticity,
                elasticity_falloff: set.elasticity_falloff,
                friction: set.contact_friction,
                scatter_deg: set.scatter_angle,
            }
        } else {
            PhysicsMaterial {
                elasticity: gd.elasticity,
                elasticity_falloff: gd.elastic_falloff,
                friction: gd.friction,
                scatter_deg: gd.scatter,
            }
        }
    } else {
        let own = PhysicsMaterial {
            elasticity: p.elasticity,
            elasticity_falloff: p.elasticity_falloff,
            friction: p.friction,
            scatter_deg: p.scatter,
        };
        physics(p.overwrite_physics, own, &p.physics_material, materials)
    };

    Ok(Some(PrimitiveData {
        mesh,
        position: Vertex3Ds::new(p.position.x, p.position.y, p.position.z),
        size: Vertex3Ds::new(p.size.x, p.size.y, p.size.z),
        rot_and_tra: p.rot_and_tra,
        collidable,
        hit_event: p.hit_event,
        threshold: p.threshold,
        collision_reduction_factor: p.collision_reduction_factor.unwrap_or(0.0),
        material,
    }))
}

/// A target's physics settings.
fn hit_target(t: &HitTarget, materials: &Materials) -> HitTargetData {
    let own = PhysicsMaterial {
        elasticity: t.elasticity,
        elasticity_falloff: t.elasticity_falloff,
        friction: t.friction,
        scatter_deg: t.scatter,
    };
    HitTargetData {
        target_type: match t.target_type {
            VpinTargetType::DropTargetBeveled => TargetType::DropTargetBeveled,
            VpinTargetType::HitTargetRound => TargetType::HitTargetRound,
            VpinTargetType::HitTargetRectangle => TargetType::HitTargetRectangle,
            VpinTargetType::HitFatTargetRectangle => TargetType::HitFatTargetRectangle,
            VpinTargetType::HitFatTargetSquare => TargetType::HitFatTargetSquare,
            VpinTargetType::DropTargetFlatSimple => TargetType::DropTargetFlatSimple,
            VpinTargetType::HitFatTargetSlim => TargetType::HitFatTargetSlim,
            VpinTargetType::HitTargetSlim => TargetType::HitTargetSlim,
            // and any other value: VPX's SetMeshType falls back to the simple drop target
            _ => TargetType::DropTargetSimple,
        },
        position: Vertex3Ds::new(t.position.x, t.position.y, t.position.z),
        size: Vertex3Ds::new(t.size.x, t.size.y, t.size.z),
        rot_z: t.rot_z,
        legacy: t.is_legacy,
        collidable: t.is_collidable,
        hit_event: t.use_hit_event,
        threshold: t.threshold,
        material: physics(t.overwrite_physics, own, &t.physics_material, materials),
    }
}

/// A rubber's physics settings. Rubbers without a hit height use their
/// visible height (`Rubber::Load`).
fn rubber(r: &Rubber, materials: &Materials) -> RubberData {
    let own = PhysicsMaterial {
        elasticity: r.elasticity,
        elasticity_falloff: r.elasticity_falloff,
        friction: r.friction,
        scatter_deg: r.scatter,
    };
    let hit_height = match r.hit_height {
        Some(h) if h != -1.0 => h,
        _ => r.height,
    };
    RubberData {
        points: drag_points(&r.drag_points),
        hit_height,
        thickness: r.thickness,
        rot_x: r.rot_x,
        rot_y: r.rot_y,
        rot_z: r.rot_z,
        collidable: r.is_collidable,
        hit_event: r.hit_event,
        material: physics(r.overwrite_physics, own, &r.physics_material, materials),
    }
}

/// A flipper's settings: its own physics, or a physics set the flipper or
/// the table picks (`UpdatePhysicsSettings`).
fn flipper(vpx: &VPX, f: &vpin::vpx::gameitem::flipper::Flipper, settings: &PhysicsSettings) -> FlipperSetup {
    let gd = &vpx.gamedata;
    let scatter = f.scatter.unwrap_or(0.0);
    let torque_damping = f.torque_damping.unwrap_or(0.75);
    let torque_damping_angle = f.torque_damping_angle.unwrap_or(6.0);
    let table_set = gd.override_physics_flipper.unwrap_or(false) && gd.override_physics != 0;
    let use_override = f.override_physics != 0 || table_set;

    let mut data = FlipperData {
        use_override,
        mass: f.mass,
        override_mass: f.mass,
        strength: f.strength,
        override_strength: f.strength,
        return_strength: f.return_,
        override_return_strength: f.return_,
        torque_damping,
        override_torque_damping: torque_damping,
        torque_damping_angle,
        override_torque_damping_angle: torque_damping_angle,
        ramp_up: f.ramp_up,
        override_coil_ramp_up: f.ramp_up,
        elasticity: f.elasticity,
        override_elasticity: f.elasticity,
        elasticity_falloff: f.elasticity_falloff,
        override_elasticity_falloff: f.elasticity_falloff,
        friction: f.friction,
        override_friction: f.friction,
        scatter,
        override_scatter_angle: scatter,
    };
    if use_override {
        let index = if f.override_physics != 0 { f.override_physics } else { gd.override_physics } as usize - 1;
        // a set VPX doesn't have reads as its default values
        let set = settings.flipper_sets.get(index).copied().unwrap_or_default();
        let pick = |v: f32, own: f32| if v < 0.0 { own } else { v };
        data.override_mass = pick(set.mass, f.mass);
        data.override_strength = pick(set.strength, f.strength);
        data.override_elasticity = pick(set.elasticity, f.elasticity);
        data.override_scatter_angle = pick(set.scatter, scatter);
        data.override_return_strength = pick(set.return_strength, f.return_);
        data.override_elasticity_falloff = pick(set.elasticity_falloff, f.elasticity_falloff);
        data.override_friction = pick(set.friction, f.friction);
        data.override_coil_ramp_up = pick(set.coil_ramp_up, f.ramp_up);
        data.override_torque_damping = pick(set.eos_torque, torque_damping);
        data.override_torque_damping_angle = pick(set.eos_torque_angle, torque_damping_angle);
    }

    FlipperSetup {
        center: v2(&f.center),
        base_radius: f.base_radius,
        end_radius: f.end_radius,
        flipper_radius_min: f.flipper_radius_min,
        flipper_radius_max: f.flipper_radius_max,
        start_angle: f.start_angle,
        end_angle: f.end_angle,
        height: if f.height > 1000.0 { 50.0 } else { f.height }, // Flipper::Load repairs broken heights
        enabled: f.is_enabled,
        data,
    }
}

/// What parts can stand on: walls and ramps, by name, in part order.
pub struct Surfaces {
    list: Vec<(String, Surface)>,
}

enum Surface {
    Wall { height_top: f32 },
    /// A ramp, and the detail level VPX traces its centre line at for a part
    /// standing on it.
    Ramp { data: Box<RampData>, detail_level: f32 },
}

impl Surfaces {
    /// The table's surfaces as the player's drawing sees them too (its
    /// shapes): the player's settings are the defaults.
    pub fn of_table(vpx: &VPX) -> Self {
        Self::new(vpx, &Materials::from_vpx(vpx), &PhysicsSettings::default())
    }

    fn new(vpx: &VPX, materials: &Materials, settings: &PhysicsSettings) -> Self {
        let mut list = Vec::new();
        for item in &vpx.gameitems {
            match item {
                GameItemEnum::Wall(w) => list.push((w.name.clone(), Surface::Wall { height_top: w.height_top })),
                GameItemEnum::Ramp(r) => {
                    // VPX traces see-through ramps at the player's detail level, others at the finest
                    let detail_level =
                        if materials.get(&r.material).opacity_active { settings.detail_level as f32 } else { 10.0 };
                    list.push((r.name.clone(), Surface::Ramp { data: Box::new(ramp(r, materials)), detail_level }));
                }
                _ => {}
            }
        }
        Self { list }
    }

    /// VPX's `PinTable::GetSurfaceHeight`: the first wall or ramp with exactly
    /// this name, else the playfield (0).
    pub fn height(&self, name: &str, at: Vertex2D) -> f32 {
        if name.is_empty() {
            return 0.0;
        }
        match self.list.iter().find(|(n, _)| n == name) {
            Some((_, Surface::Wall { height_top })) => *height_top,
            Some((_, Surface::Ramp { data, detail_level })) => ramp_surface_height(data, at.x, at.y, *detail_level),
            None => 0.0,
        }
    }
}
