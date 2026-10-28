// Default values from Visual Pinball X: src/core/Settings_properties.inl and src/physics/physconst.h
// (license:GPLv3+), commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The player's settings that change a table's colliders. VPX reads them from
//! its settings file; the defaults are VPX's.

/// One of VPX's eight flipper physics sets (its "physics options"), which a
/// table or a flipper can choose instead of the flipper's own values. A
/// negative value means "keep the flipper's own".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlipperPhysicsSet {
    pub mass: f32,
    pub strength: f32,
    pub elasticity: f32,
    /// In degrees.
    pub scatter: f32,
    pub return_strength: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    pub coil_ramp_up: f32,
    pub eos_torque: f32,
    /// In degrees.
    pub eos_torque_angle: f32,
}

impl Default for FlipperPhysicsSet {
    fn default() -> Self {
        Self {
            mass: 1.0,
            strength: 2200.0,
            elasticity: 0.8,
            scatter: 0.0,
            return_strength: 0.058,
            elasticity_falloff: 0.43,
            friction: 0.6,
            coil_ramp_up: 3.0,
            eos_torque: 0.75,
            eos_torque_angle: 6.0,
        }
    }
}

/// One of VPX's eight table physics sets, which a table can choose instead of
/// its own playfield physics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TablePhysicsSet {
    /// Gravity, in VPX's units before its scaling (`GRAVITYCONST`).
    pub gravity_constant: f32,
    pub contact_friction: f32,
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    /// The playfield's scatter, in degrees.
    pub scatter_angle: f32,
    /// The scatter of every contact, in degrees.
    pub contact_scatter_angle: f32,
    pub min_slope: f32,
    pub max_slope: f32,
}

impl Default for TablePhysicsSet {
    fn default() -> Self {
        Self {
            gravity_constant: 0.97,
            contact_friction: 0.075,
            elasticity: 0.25,
            elasticity_falloff: 0.0,
            scatter_angle: 0.0,
            contact_scatter_angle: 0.5,
            min_slope: 6.0,
            max_slope: 6.0,
        }
    }
}

/// The settings that change colliders.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsSettings {
    /// VPX's detail level, 1 to 10 (`Player/AlphaRampAccuracy`). For physics
    /// it only changes how finely VPX traces a see-through ramp's centre line
    /// to find the height of a part standing on it.
    pub detail_level: i32,
    /// The player's difficulty for this table (`TableOverride/Difficulty`),
    /// or `None` for the table's own. It shortens flippers that have a length
    /// range.
    pub difficulty: Option<f32>,
    /// VPX's physics sets (`Player/FlipperPhysics*0` to `7`).
    pub flipper_sets: [FlipperPhysicsSet; 8],
    /// The same sets' table values (`Player/TablePhysics*0` to `7`).
    pub table_sets: [TablePhysicsSet; 8],
}

impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            detail_level: 10,
            difficulty: None,
            flipper_sets: [FlipperPhysicsSet::default(); 8],
            table_sets: [TablePhysicsSet::default(); 8],
        }
    }
}
