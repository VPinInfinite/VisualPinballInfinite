//! Which VPX release each part of the compatibility physics follows:
//! the settings screen shows it, so players can see how current
//! compatibility mode is. When a part is re-ported from a newer release, its
//! entry changes with it.

/// A VPX release the port follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VpxRelease {
    /// As VPX names it, e.g. "10.8.1".
    pub version: &'static str,
    /// Whether VPX had published it as a final release when it was ported.
    pub pre_release: bool,
    /// VPX's build number.
    pub build: u32,
    /// The source commit.
    pub commit: &'static str,
    /// When VPX published it.
    pub published: &'static str,
}

/// VPX 10.8.1 pre-release build 5436: the physics core.
pub const VPX_10_8_1_BUILD_5436: VpxRelease =
    VpxRelease { version: "10.8.1", pre_release: true, build: 5436, commit: "af26b2d", published: "2026-08-21" };

/// A part of the physics, and the release it follows (`None`: not ported yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsPart {
    pub name: &'static str,
    pub release: Option<VpxRelease>,
}

/// Every part of the compatibility physics, as the settings screen lists it.
pub const PARTS: &[PhysicsPart] = &[
    PhysicsPart { name: "Ball motion and collisions", release: Some(VPX_10_8_1_BUILD_5436) },
    PhysicsPart { name: "Collision search", release: Some(VPX_10_8_1_BUILD_5436) },
    PhysicsPart { name: "Walls, ramps, rubbers, posts and 3D models", release: Some(VPX_10_8_1_BUILD_5436) },
    PhysicsPart { name: "Flippers", release: Some(VPX_10_8_1_BUILD_5436) },
    PhysicsPart { name: "Plunger", release: Some(VPX_10_8_1_BUILD_5436) },
    PhysicsPart {
        name: "Bumpers, slingshots, gates, spinners, targets, triggers and kickers",
        release: Some(VPX_10_8_1_BUILD_5436),
    },
    PhysicsPart { name: "Building a table's physics from its file", release: Some(VPX_10_8_1_BUILD_5436) },
    // these follow whichever release is current when they're ported
    PhysicsPart { name: "Nudging and tilt", release: None },
    PhysicsPart { name: "Analog plunger", release: None },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_part_is_listed_once() {
        for (i, p) in PARTS.iter().enumerate() {
            assert!(PARTS[..i].iter().all(|q| q.name != p.name), "{} listed twice", p.name);
        }
    }
}
