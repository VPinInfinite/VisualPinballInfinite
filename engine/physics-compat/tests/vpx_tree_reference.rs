//! Comparison of the ported collision search with VPX's: which colliders the static quadtree and
//! the balls' k-d tree offer to the hit test, in which order, duplicates included.
//!
//! The data files were written by `tests/vpi/test-vpi-trees.cpp`, a test we add to VPX's physics
//! tester (build 5436). Its colliders are probes that only record that they were offered; their
//! owners are real walls, primitives and hit targets, switched between collidable and not, standing
//! and dropped, between searches. Each `qtree` / `kdtree` line holds a tree's contents and VPX's
//! build result; each query line a ball, the random generator's state, and VPX's search result:
//! the ball's hit box, the probes offered in order, and the generator's state after.
//!
//! The search makes no float decisions beyond comparisons and the sphere test, so it must match
//! both of VPX's builds exactly. Only the ball's hit box, computed before the search, carries
//! float noise in the shipped build: against it, a box that differs is counted, and the search is
//! then run with VPX's box.

use physics_compat::env::BallId;
use physics_compat::fpmode::VpxFloatMode;
use physics_compat::hitball::HitBall;
use physics_compat::kdtree::{HitKd, KdItem};
use physics_compat::math::{FRect, FRect3D, Mwc64x, Vertex3Ds};
use physics_compat::quadtree::{BallQuery, HitQuadtree, Owner, OwnerKind, TreeItem};

/// Reads one line's tokens by type.
struct Cursor<'a> {
    t: Vec<&'a str>,
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(line: &'a str) -> Self {
        Self { t: line.split_whitespace().collect(), i: 1 }
    }
    fn tok(&mut self) -> &'a str {
        let s = self.t[self.i];
        self.i += 1;
        s
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.tok(), 16).expect("float bits"))
    }
    fn i(&mut self) -> i64 {
        self.tok().parse().expect("int")
    }
    fn u(&mut self) -> usize {
        self.i() as usize
    }
    fn u64(&mut self) -> u64 {
        u64::from_str_radix(self.tok(), 16).expect("u64")
    }
    fn v3(&mut self) -> Vertex3Ds {
        Vertex3Ds::new(self.f(), self.f(), self.f())
    }
    fn rect3d(&mut self) -> FRect3D {
        FRect3D::new(self.f(), self.f(), self.f(), self.f(), self.f(), self.f())
    }
    fn ids(&mut self) -> Vec<u32> {
        let n = self.u();
        (0..n).map(|_| self.i() as u32).collect()
    }
    fn arrow(&mut self) {
        assert_eq!(self.tok(), "->");
    }
}

/// The ball's hit box and search radius, from the port's `HitBall`.
fn ball_query(pos: Vertex3Ds, vel: Vertex3Ds, radius: f32) -> BallQuery {
    let mut hb = HitBall::new(BallId(0));
    hb.d.pos = pos;
    hb.d.vel = vel;
    hb.d.radius = radius;
    hb.calc_hit_bbox();
    BallQuery { bbox: hb.base.hit_bbox, pos, rsqr: hb.hit_radius_sqr() }
}

fn same_box(a: &FRect3D, b: &FRect3D) -> bool {
    let bits = |r: &FRect3D| [r.left, r.right, r.top, r.bottom, r.zlow, r.zhigh].map(f32::to_bits);
    bits(a) == bits(b)
}

#[derive(Default)]
struct Tally {
    builds: usize,
    queries: usize,
    offered: usize,
    /// Shipped build: searches whose ball box differed by float noise.
    box_drift: usize,
    errors: Vec<String>,
}

impl Tally {
    fn error(&mut self, line: &str, what: String) {
        if self.errors.len() < 8 {
            let short: String = line.chars().take(200).collect();
            self.errors.push(format!("{what}\n      {short}"));
        } else if self.errors.len() == 8 {
            self.errors.push("...".into());
        }
    }
}

/// The ball box the search uses: the port's, which must match VPX's exactly against the exact
/// build; against the shipped build, VPX's when they differ.
fn search_box(t: &mut Tally, line: &str, exact: bool, ball: BallQuery, vbox: FRect3D, vrsqr: f32) -> Option<BallQuery> {
    if same_box(&ball.bbox, &vbox) && ball.rsqr.to_bits() == vrsqr.to_bits() {
        return Some(ball);
    }
    if exact {
        t.error(line, format!("ball box: port {:?} {} vpx {vbox:?} {vrsqr}", ball.bbox, ball.rsqr));
        return None;
    }
    t.box_drift += 1;
    Some(BallQuery { bbox: vbox, rsqr: vrsqr, ..ball })
}

fn compare(data: &str, exact: bool) -> (Tally, Tally) {
    let _mode = VpxFloatMode::enter();
    let (mut qt, mut kd) = (Tally::default(), Tally::default());

    let mut quadtree = HitQuadtree::new();
    let mut owners: Vec<Owner> = Vec::new();
    let mut kdtree = HitKd::new();
    let mut kd_boxes: Vec<FRect3D> = Vec::new();
    let mut kd_self = 0u32;

    for line in data.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let kind = line.split_whitespace().next().unwrap();
        let mut c = Cursor::new(line);
        match kind {
            "qtree" => {
                let _scenario = c.i();
                quadtree = HitQuadtree::new();
                if c.i() != 0 {
                    let (l, r, t, b) = (c.f(), c.f(), c.f(), c.f());
                    quadtree.set_bounds(FRect::new(l, r, t, b));
                }
                let n_owners = c.u();
                owners = (0..n_owners)
                    .map(|id| Owner {
                        id: id as u32,
                        kind: match c.i() {
                            0 => OwnerKind::Other,
                            1 => OwnerKind::Primitive,
                            _ => OwnerKind::HitTarget,
                        },
                    })
                    .collect();
                let n = c.u();
                let items: Vec<TreeItem> =
                    (0..n).map(|id| TreeItem { id: id as u32, owner: owners[c.u()], bbox: c.rect3d() }).collect();
                c.arrow();
                quadtree.reset(items);
                quadtree.finalize();

                qt.builds += 1;
                let levels = c.i() as u32;
                if quadtree.n_levels() != levels {
                    qt.error(line, format!("levels: port {} vpx {levels}", quadtree.n_levels()));
                }
                let order: Vec<u32> = (0..n).map(|_| c.i() as u32).collect();
                let port: Vec<u32> = quadtree.items().iter().map(|it| it.id).collect();
                if port != order {
                    qt.error(line, format!("item order differs: port {port:?}\n      vpx {order:?}"));
                }
            }
            "qquery" => {
                let active: Vec<bool> = owners.iter().map(|_| c.i() != 0).collect();
                let (pos, vel, radius) = (c.v3(), c.v3(), c.f());
                let mut rng = Mwc64x { state: c.u64() };
                c.arrow();
                let ball = ball_query(pos, vel, radius);
                let (vbox, vrsqr) = (c.rect3d(), c.f());
                let vpx = c.ids();
                let rng_after = c.u64();

                qt.queries += 1;
                let Some(ball) = search_box(&mut qt, line, exact, ball, vbox, vrsqr) else { continue };
                let mut port = Vec::new();
                quadtree.hit_test_ball(&ball, &mut rng, |o| active[o.id as usize], |id| port.push(id));
                qt.offered += port.len();
                if port != vpx {
                    qt.error(line, format!("offered: port {port:?}\n      vpx {vpx:?}"));
                }
                if rng.state != rng_after {
                    qt.error(line, "random generator state differs".into());
                }
            }
            "kdtree" => {
                let _scenario = c.i();
                let n = c.u();
                kd_self = c.i() as u32;
                kd_boxes = (0..n).map(|id| if id as u32 == kd_self { FRect3D::default() } else { c.rect3d() }).collect();
                // VPX resets the tree with the searching ball inside; its box is recomputed by each update
                kdtree = HitKd::new();
                kdtree.reset((0..n).map(|id| KdItem { id: id as u32, bbox: kd_boxes[id] }).collect());
                kdtree.finalize();
                kd.builds += 1;
            }
            "kdquery" => {
                let (pos, vel, radius) = (c.v3(), c.v3(), c.f());
                let mut rng = Mwc64x { state: c.u64() };
                c.arrow();
                let ball = ball_query(pos, vel, radius);
                let (vbox, vrsqr) = (c.rect3d(), c.f());
                let levels = c.i() as u32;
                let vpx = c.ids();
                let rng_after = c.u64();

                kd.queries += 1;
                let Some(ball) = search_box(&mut kd, line, exact, ball, vbox, vrsqr) else { continue };
                kd_boxes[kd_self as usize] = ball.bbox;
                kdtree.update(|id| kd_boxes[id as usize]);
                if kdtree.n_levels() != levels {
                    kd.error(line, format!("levels: port {} vpx {levels}", kdtree.n_levels()));
                }
                let mut port = Vec::new();
                kdtree.hit_test_ball(&ball, kd_self, &mut rng, |id| port.push(id));
                kd.offered += port.len();
                if port != vpx {
                    kd.error(line, format!("offered: port {port:?}\n      vpx {vpx:?}"));
                }
                if rng.state != rng_after {
                    kd.error(line, "random generator state differs".into());
                }
            }
            other => panic!("unknown reference line {other}"),
        }
    }
    (qt, kd)
}

fn check(file: &str, exact: bool) {
    let path = format!("{}/tests/data/{file}", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read_to_string(&path).expect("reference data");
    let (qt, kd) = compare(&data, exact);
    let mut report = format!("{file}\n");
    for (name, t) in [("quadtree", &qt), ("k-d tree", &kd)] {
        report.push_str(&format!(
            "{name:9} {:4} trees {:5} searches {:7} offers  {} mismatches{}\n",
            t.builds,
            t.queries,
            t.offered,
            t.errors.len(),
            if exact { String::new() } else { format!("  ({} ball boxes differ by float noise)", t.box_drift) }
        ));
        for e in &t.errors {
            report.push_str(&format!("    {e}\n"));
        }
    }
    println!("{report}");
    assert!(qt.builds > 0 && kd.builds > 0, "no reference data in {file}");
    assert!(qt.errors.is_empty() && kd.errors.is_empty(), "collision search differs from VPX:\n{report}");
}

#[test]
#[cfg(target_arch = "x86_64")]
fn collision_search_matches_exact_vpx() {
    check("vpx-trees-5436-precise.txt", true);
}

#[test]
#[cfg(target_arch = "x86_64")]
fn collision_search_matches_shipped_vpx() {
    check("vpx-trees-5436.txt", false);
}
