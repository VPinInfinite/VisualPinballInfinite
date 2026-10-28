//! Building a table's colliders from its parts: the physics side of VPX's
//! `PhysicSetup`, part by part, in VPX's order.
//!
//! The input is each part's physics settings, as plain data; reading them
//! from a table file is the table model's job. The output is the colliders in
//! the order VPX adds them, which the engine needs (its collision search
//! depends on it).

pub mod curves;
pub mod forsyth;
pub mod parts;
pub mod primitive;
pub mod progmesh;
pub mod ramp;
pub mod rubber;
pub mod surface;
pub mod target;

use crate::engine::Collider;
use crate::env::PartId;
use crate::quadtree::{Owner, OwnerKind};

/// A part's physics material, with the part's own values or its physics
/// material's, as the part's "overwrite physics" switch picks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsMaterial {
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    /// In degrees.
    pub scatter_deg: f32,
}

/// Where a part's colliders go: VPX's pending list, in order.
pub struct Colliders<'a> {
    pub out: &'a mut Vec<Collider>,
    /// The part being set up.
    pub part: PartId,
    pub kind: OwnerKind,
}

impl Colliders<'_> {
    pub fn owner(&self) -> Owner {
        Owner { id: self.part.0, kind: self.kind }
    }
}
