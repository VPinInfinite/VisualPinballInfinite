// Follows Visual Pinball X: src/parts/pintable.cpp (license:GPLv3+): how PinTable::Load reads the
// MATE, PHMA and MATR records, RemoveInvalidReferences' lookup table, and GetMaterial.
// Commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The table's materials, as far as physics uses them, looked up the way VPX
//! looks them up while playing.

use std::collections::HashMap;

use vpin::vpx::VPX;

/// What physics takes from a material. A missing material reads as VPX's
/// placeholder: all zeros, not see-through.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MaterialPhysics {
    pub elasticity: f32,
    pub elasticity_falloff: f32,
    pub friction: f32,
    /// In degrees.
    pub scatter_angle: f32,
    /// See-through (`m_bOpacityActive`): VPX then traces ramps on it at the
    /// player's detail level instead of the finest.
    pub opacity_active: bool,
}

pub struct Materials {
    /// By lowercase name.
    by_name: HashMap<String, MaterialPhysics>,
}

impl Materials {
    /// The materials as VPX loads them: the old records (`MATE`, with physics
    /// in `PHMA`), replaced by the 10.8 records (`MATR`) when the table has
    /// those.
    pub fn from_vpx(vpx: &VPX) -> Self {
        let gd = &vpx.gamedata;
        let version = vpx.version.u32();
        let count = gd.materials_size as usize;
        let mut list: Vec<(String, MaterialPhysics)> = Vec::new();

        // MATE: the materials, without physics
        for m in &gd.materials_old {
            let opacity_active = m.opacity_active_edge_alpha & 1 != 0;
            list.push((m.name.clone(), MaterialPhysics { opacity_active, ..MaterialPhysics::default() }));
        }

        // PHMA: their physics, by name (VPX's lookup while loading: the first exact match)
        if let Some(phys) = &gd.materials_physics_old
            && (version < 1080 || list.len() == count)
        {
            for p in phys {
                let values = |m: &mut MaterialPhysics| {
                    m.elasticity = p.elasticity;
                    m.elasticity_falloff = p.elasticity_falloff;
                    m.friction = p.friction;
                    m.scatter_angle = p.scatter_angle;
                };
                match list.iter_mut().find(|(n, _)| *n == p.name) {
                    Some((_, m)) => values(m),
                    None => {
                        let mut m = MaterialPhysics::default();
                        values(&mut m);
                        list.push((p.name.clone(), m));
                    }
                }
            }
        }

        // MATR: 10.8's records replace the old ones
        if let Some(mats) = &gd.materials {
            for m in mats {
                if version >= 1080 && list.len() == count {
                    list.clear();
                }
                list.push((
                    m.name.clone(),
                    MaterialPhysics {
                        elasticity: m.elasticity,
                        elasticity_falloff: m.elasticity_falloff,
                        friction: m.friction,
                        scatter_angle: m.scatter_angle,
                        opacity_active: m.opacity_active,
                    },
                ));
            }
        }

        // the lookup table VPX plays with: names ignore case, and a later material wins
        let mut by_name = HashMap::with_capacity(list.len());
        for (name, m) in list {
            by_name.insert(name.to_ascii_lowercase(), m);
        }
        Self { by_name }
    }

    /// VPX's `GetMaterial`.
    pub fn get(&self, name: &str) -> MaterialPhysics {
        if name.is_empty() {
            return MaterialPhysics::default();
        }
        self.by_name.get(&name.to_ascii_lowercase()).copied().unwrap_or_default()
    }
}
