// Ported from Visual Pinball X: src/physics/quadtree.cpp and quadtree.h (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's quadtree over the table's colliders: the collision search for
//! everything but balls.
//!
//! The order in which the search offers colliders to the hit test is part of
//! the physics: when two hits happen at the same time, the one tested last
//! wins (`DoHitTest`), and contacts are handled in the order they were found.
//! So this keeps the tree VPX's x64 build makes and searches, quirks included:
//!
//! - Leaves are tested four colliders at a time, in aligned groups of four
//!   from the tree's item list (VPX's SSE test). A group can straddle two
//!   nodes, so a collider near a node boundary can be offered more than once.
//! - Within a group, the box test only decides whether the group is looked at;
//!   the sphere test alone decides which of its four colliders are offered.
//! - A random draw per search flips the order in which groups are walked.
//! - Children are searched last-in-first-out: bottom right before top left.
//! - The node pool is sized from the item count, and subdivision stops where
//!   it runs out.
//!
//! VPX builds subtrees of more than 1000 colliders on worker threads. That
//! changes nothing unless the node pool runs out, and then VPX's own tree
//! varies from run to run; this port builds in one thread, in the order VPX
//! uses when it has no workers.

use crate::math::bbox::{FRect, FRect3D};
use crate::math::def::Mwc64x;
use crate::math::vector::{Vertex2D, Vertex3Ds};

/// The part that owns a collider, as far as the search cares (VPX's
/// `IHitable`). The same id must always come with the same kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    pub id: u32,
    pub kind: OwnerKind,
}

/// VPX only skips whole subtrees for primitives and hit targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerKind {
    /// Skipped while not collidable.
    Primitive,
    /// Skipped while dropped.
    HitTarget,
    Other,
}

/// One collider in the tree: an id chosen by the caller, its box and owner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeItem {
    pub id: u32,
    pub bbox: FRect3D,
    pub owner: Owner,
}

/// The searching ball, as the trees see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallQuery {
    /// The ball's hit box (`HitBall::m_hitBBox`, from `calc_hit_bbox`).
    pub bbox: FRect3D,
    pub pos: Vertex3Ds,
    /// `HitBall::HitRadiusSqr()`.
    pub rsqr: f32,
}

const MAX_LEVEL: u32 = 128;
const NONE: u32 = u32::MAX;
const ROOT: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug)]
struct Node {
    /// First item in `vho`.
    start: u32,
    items: u32,
    /// Everything in this subtree belongs to this primitive or hit target.
    unique: Option<Owner>,
    /// First of four children in the node pool, or `NONE` for a leaf.
    children: u32,
    /// Centre of the node's bounds; only set when it has children.
    vcenter: Vertex2D,
}

impl Node {
    /// VPX's `HitQuadtreeNode::Reset`. Like VPX's, it leaves `vcenter` alone.
    fn reset(&mut self) {
        self.children = NONE;
        self.unique = None;
        self.start = 0;
        self.items = 0;
    }
}

impl Default for Node {
    fn default() -> Self {
        Self { start: 0, items: 0, unique: None, children: NONE, vcenter: Vertex2D::default() }
    }
}

/// The static items' boxes, laid out as VPX's SSE test reads them: one array
/// per edge, padded to a multiple of four with boxes nothing can hit.
#[derive(Clone, Debug, Default)]
pub(crate) struct SseBoxes {
    pub(crate) l: Vec<f32>,
    pub(crate) r: Vec<f32>,
    pub(crate) t: Vec<f32>,
    pub(crate) b: Vec<f32>,
    pub(crate) zl: Vec<f32>,
    pub(crate) zh: Vec<f32>,
}

impl SseBoxes {
    /// VPX's `InitSseArrays`.
    pub(crate) fn fill(&mut self, boxes: impl ExactSizeIterator<Item = FRect3D>) {
        let n = boxes.len();
        let padded = (n + 3) & !3;
        for v in [&mut self.l, &mut self.r, &mut self.t, &mut self.b, &mut self.zl, &mut self.zh] {
            v.clear();
            v.reserve(padded);
        }
        for r in boxes {
            self.l.push(r.left);
            self.r.push(r.right);
            self.t.push(r.top);
            self.b.push(r.bottom);
            self.zl.push(r.zlow);
            self.zh.push(r.zhigh);
        }
        for _ in n..padded {
            self.l.push(f32::MAX);
            self.r.push(-f32::MAX);
            self.t.push(f32::MAX);
            self.b.push(-f32::MAX);
            self.zl.push(f32::MAX);
            self.zh.push(-f32::MAX);
        }
    }
}

/// The four-at-a-time tests, with VPX's SSE instructions on x86-64 (each lane
/// computes exactly what the plain version below does; the instructions just
/// do four lanes at once, and the box test stops as soon as no lane is left).
#[cfg(target_arch = "x86_64")]
mod lanes {
    use super::{BallQuery, SseBoxes};
    use std::arch::x86_64::*;

    /// A ball's search values, each in all four lanes (VPX's `_mm_set1_ps`).
    #[derive(Clone, Copy)]
    pub(crate) struct Lanes {
        bleft: __m128,
        bright: __m128,
        btop: __m128,
        bbottom: __m128,
        bzlow: __m128,
        bzhigh: __m128,
        posx: __m128,
        posy: __m128,
        posz: __m128,
        rsqr: __m128,
    }

    impl Lanes {
        #[inline(always)]
        pub(crate) fn new(ball: &BallQuery) -> Self {
            let b = &ball.bbox;
            // SAFETY (all intrinsics in this module): SSE is part of every x86-64 processor
            unsafe {
                Self {
                    bleft: _mm_set1_ps(b.left),
                    bright: _mm_set1_ps(b.right),
                    btop: _mm_set1_ps(b.top),
                    bbottom: _mm_set1_ps(b.bottom),
                    bzlow: _mm_set1_ps(b.zlow),
                    bzhigh: _mm_set1_ps(b.zhigh),
                    posx: _mm_set1_ps(ball.pos.x),
                    posy: _mm_set1_ps(ball.pos.y),
                    posz: _mm_set1_ps(ball.pos.z),
                    rsqr: _mm_set1_ps(ball.rsqr),
                }
            }
        }
    }

    #[inline(always)]
    fn load(v: &[f32], i: usize) -> __m128 {
        let lanes = &v[i * 4..i * 4 + 4]; // the arrays are padded to a multiple of four
        // SAFETY: `lanes` holds four floats; the load needs no alignment
        unsafe { _mm_loadu_ps(lanes.as_ptr()) }
    }

    /// The sphere-against-box test: a bit per lane whose box is within the
    /// ball's search radius. `_mm_max_ps(a, zero)` is zero unless a > 0.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn sphere(l: __m128, r: __m128, t: __m128, b: __m128, zl: __m128, zh: __m128, q: &Lanes) -> u32 {
        unsafe {
            let zero = _mm_setzero_ps();
            let ex = _mm_add_ps(_mm_max_ps(_mm_sub_ps(l, q.posx), zero), _mm_max_ps(_mm_sub_ps(q.posx, r), zero));
            let ey = _mm_add_ps(_mm_max_ps(_mm_sub_ps(t, q.posy), zero), _mm_max_ps(_mm_sub_ps(q.posy, b), zero));
            let ez = _mm_add_ps(_mm_max_ps(_mm_sub_ps(zl, q.posz), zero), _mm_max_ps(_mm_sub_ps(q.posz, zh), zero));
            let ex = _mm_mul_ps(ex, ex);
            let ey = _mm_mul_ps(ey, ey);
            let ez = _mm_mul_ps(ez, ez);
            let d = _mm_add_ps(_mm_add_ps(ex, ey), ez);
            _mm_movemask_ps(_mm_cmple_ps(d, q.rsqr)) as u32
        }
    }

    impl SseBoxes {
        /// The quadtree's test of group `i` (items `4i` to `4i+3`): the box test
        /// only decides whether the group is looked at; the sphere test alone
        /// picks the items, a bit each.
        #[inline(always)]
        pub(crate) fn group_mask(&self, i: usize, q: &Lanes) -> u32 {
            unsafe {
                let (l, r) = (load(&self.l, i), load(&self.r, i));
                let mut mask = _mm_movemask_ps(_mm_cmpge_ps(q.bright, l));
                if mask == 0 {
                    return 0;
                }
                mask &= _mm_movemask_ps(_mm_cmple_ps(q.bleft, r));
                if mask == 0 {
                    return 0;
                }
                let (t, b) = (load(&self.t, i), load(&self.b, i));
                mask &= _mm_movemask_ps(_mm_cmpge_ps(q.bbottom, t));
                if mask == 0 {
                    return 0;
                }
                mask &= _mm_movemask_ps(_mm_cmple_ps(q.btop, b));
                if mask == 0 {
                    return 0;
                }
                let (zl, zh) = (load(&self.zl, i), load(&self.zh, i));
                mask &= _mm_movemask_ps(_mm_cmpge_ps(q.bzhigh, zl));
                if mask == 0 {
                    return 0;
                }
                mask &= _mm_movemask_ps(_mm_cmple_ps(q.bzlow, zh));
                if mask == 0 {
                    return 0;
                }
                sphere(l, r, t, b, zl, zh, q)
            }
        }

        /// The k-d tree's test of group `i`: the sphere test alone.
        #[inline(always)]
        pub(crate) fn sphere_mask(&self, i: usize, q: &Lanes) -> u32 {
            let (l, r, t, b) = (load(&self.l, i), load(&self.r, i), load(&self.t, i), load(&self.b, i));
            sphere(l, r, t, b, load(&self.zl, i), load(&self.zh, i), q)
        }
    }
}

/// The same tests one lane at a time, for other processors.
#[cfg(not(target_arch = "x86_64"))]
mod lanes {
    use super::{BallQuery, SseBoxes};

    pub(crate) type Lanes = BallQuery;

    // _mm_max_ps(a, zero) returns zero unless a > 0, NaN included
    #[inline(always)]
    fn max0(a: f32) -> f32 {
        if a > 0.0 { a } else { 0.0 }
    }

    impl SseBoxes {
        #[inline(always)]
        pub(crate) fn group_mask(&self, i: usize, ball: &Lanes) -> u32 {
            let bb = &ball.bbox;
            let any = (0..4).any(|k| {
                let j = i * 4 + k;
                bb.right >= self.l[j]
                    && bb.left <= self.r[j]
                    && bb.bottom >= self.t[j]
                    && bb.top <= self.b[j]
                    && bb.zhigh >= self.zl[j]
                    && bb.zlow <= self.zh[j]
            });
            if any { self.sphere_mask(i, ball) } else { 0 }
        }

        #[inline(always)]
        pub(crate) fn sphere_mask(&self, i: usize, ball: &Lanes) -> u32 {
            let mut mask = 0;
            for k in 0..4 {
                let j = i * 4 + k;
                let ex = max0(self.l[j] - ball.pos.x) + max0(ball.pos.x - self.r[j]);
                let ey = max0(self.t[j] - ball.pos.y) + max0(ball.pos.y - self.b[j]);
                let ez = max0(self.zl[j] - ball.pos.z) + max0(ball.pos.z - self.zh[j]);
                let d = ex * ex + ey * ey + ez * ez;
                if d <= ball.rsqr {
                    mask |= 1 << k;
                }
            }
            mask
        }
    }

    impl Lanes {
        #[inline(always)]
        pub(crate) fn new(ball: &BallQuery) -> Self {
            *ball
        }
    }
}

pub(crate) use lanes::Lanes;

/// The groups of four a node's items fall in, walked forwards or backwards
/// (VPX's `start`/`end`/`dt` loop, unsigned wrap-around included).
#[inline(always)]
pub(crate) fn for_each_group(start: u32, items: u32, forwards: bool, mut f: impl FnMut(usize)) {
    let size = (start + items).div_ceil(4);
    let (first, end) = if forwards { (start / 4, size) } else { (size.wrapping_sub(1), (start / 4).wrapping_sub(1)) };
    let mut i = first;
    while i != end {
        f(i as usize);
        i = if forwards { i.wrapping_add(1) } else { i.wrapping_sub(1) };
    }
}

/// VPX's `HitQuadtree` (the non-Embree build, with the SSE leaf test).
#[derive(Clone, Debug)]
pub struct HitQuadtree {
    vho: Vec<TreeItem>,
    bounds: FRect,
    root: Node,
    nodes: Vec<Node>,
    num_nodes: usize,
    /// The last build ran out of nodes.
    pool_ran_out: bool,
    tmp1: Vec<TreeItem>,
    tmp2: Vec<TreeItem>,
    max_items: usize,
    n_levels: u32,
    sse: SseBoxes,
}

impl Default for HitQuadtree {
    fn default() -> Self {
        Self::new()
    }
}

impl HitQuadtree {
    pub fn new() -> Self {
        let mut bounds = FRect::default();
        bounds.clear();
        Self {
            vho: Vec::new(),
            bounds,
            root: Node::default(),
            nodes: Vec::new(),
            num_nodes: 0,
            pool_ran_out: false,
            tmp1: Vec::new(),
            tmp2: Vec::new(),
            max_items: 0,
            n_levels: 0,
            sse: SseBoxes::default(),
        }
    }

    /// The area the tree divides; without it, the items' combined box.
    pub fn set_bounds(&mut self, bounds: FRect) {
        self.bounds = bounds;
    }

    /// VPX's `BeginReset`/`EndReset`: replaces all items and rebuilds. The
    /// items' boxes must be current.
    pub fn reset(&mut self, items: Vec<TreeItem>) {
        self.vho = items;
        self.initialize();
    }

    pub fn insert(&mut self, item: TreeItem) {
        self.vho.push(item);
        self.initialize();
    }

    /// Removes the first item with this id, if any, and rebuilds.
    pub fn remove(&mut self, id: u32) {
        if let Some(i) = self.vho.iter().position(|it| it.id == id) {
            self.vho.remove(i);
        }
        self.initialize();
    }

    /// VPX's `Finalize`: frees the build's scratch space.
    pub fn finalize(&mut self) {
        self.tmp1 = Vec::new();
        self.tmp2 = Vec::new();
    }

    /// The items, in the tree's order.
    pub fn items(&self) -> &[TreeItem] {
        &self.vho
    }

    pub fn object_count(&self) -> usize {
        self.vho.len()
    }

    pub fn n_levels(&self) -> u32 {
        self.n_levels
    }

    /// Whether the last build ran out of nodes, where VPX's threaded build
    /// can differ from run to run.
    pub fn pool_ran_out(&self) -> bool {
        self.pool_ran_out
    }

    fn initialize(&mut self) {
        let n = self.vho.len();
        if n > self.max_items {
            self.max_items = n;
            // VPX: "no a priori bound on the number of nodes in the tree. We just make an educated
            // guess on the maximum and truncate the subdivision if we run out of nodes."
            self.nodes = vec![Node::default(); (n * 2 + 1) & !3];
        }
        let filler = TreeItem { id: NONE, bbox: FRect3D::default(), owner: Owner { id: NONE, kind: OwnerKind::Other } };
        if self.tmp1.len() < self.max_items {
            self.tmp1.resize(self.max_items, filler);
        }
        if self.tmp2.len() < self.max_items {
            self.tmp2.resize(self.max_items, filler);
        }

        let mut bounds = self.bounds;
        if bounds.left == f32::MAX {
            bounds.clear();
            for it in &self.vho {
                bounds.extend_3d(&it.bbox);
            }
        }

        self.n_levels = 0;
        self.num_nodes = 0;
        self.pool_ran_out = false;
        self.root.reset();
        self.root.start = 0;
        self.root.items = n as u32;
        self.create_next_level(ROOT, &bounds, 0, 0);

        self.sse.fill(self.vho.iter().map(|it| it.bbox));
    }

    fn node(&self, r: u32) -> &Node {
        if r == ROOT { &self.root } else { &self.nodes[r as usize] }
    }

    fn node_mut(&mut self, r: u32) -> &mut Node {
        if r == ROOT { &mut self.root } else { &mut self.nodes[r as usize] }
    }

    /// VPX's `AllocFourNodes`: `None` once the pool is used up.
    fn alloc_four_nodes(&mut self) -> Option<u32> {
        if self.num_nodes + 3 >= self.nodes.len() {
            self.pool_ran_out = true;
            return None;
        }
        for n in &mut self.nodes[self.num_nodes..self.num_nodes + 4] {
            n.reset();
        }
        self.num_nodes += 4;
        Some((self.num_nodes - 4) as u32)
    }

    /// VPX's `HitQuadtreeNode::CreateNextLevel`.
    fn create_next_level(&mut self, node: u32, bounds: &FRect, level: u32, mut level_empty: u32) {
        if level > self.n_levels {
            self.n_levels = level;
        }

        let (start, items) = {
            let n = self.node(node);
            (n.start, n.items)
        };
        if items <= 4 // !! magic
            || level == MAX_LEVEL - 1
        {
            return;
        }

        let Some(children) = self.alloc_four_nodes() else {
            return; // ran out of nodes
        };

        let vcenter = Vertex2D::new((bounds.left + bounds.right) * 0.5, (bounds.top + bounds.bottom) * 0.5);

        // sort items into appropriate child nodes: VPX's SSE quadrant test, where an item goes into a
        // child if its box is on one side of both centre lines ("> centre" against "not > centre")
        const QUADRANT: [u8; 16] = [0, 1, 128, 1, 2, 3, 128, 3, 128, 128, 128, 128, 2, 3, 128, 3];
        let (s, e) = (start as usize, (start + items) as usize);
        let mut unique = Some(self.vho[s].owner);
        let (mut stay, mut c0, mut c1, mut c2, mut c3) = (0, 0, 0, 0, 0);
        for idx in s..e {
            let it = self.vho[idx];
            if unique != Some(it.owner) {
                unique = None; // are all objects in current node unique/belong to the same hitable?
            }
            let b = &it.bbox;
            let mask = usize::from(b.left > vcenter.x)
                | usize::from(b.right > vcenter.x) << 1
                | usize::from(b.top > vcenter.y) << 2
                | usize::from(b.bottom > vcenter.y) << 3;
            match QUADRANT[mask] {
                0 => {
                    self.tmp1[s + c0] = it;
                    c0 += 1;
                }
                1 => {
                    self.tmp2[s + c1] = it;
                    c1 += 1;
                }
                // children 2 and 3 fill their scratch space from the end, so they end up reversed
                2 => {
                    self.tmp1[e - 1 - c2] = it;
                    c2 += 1;
                }
                3 => {
                    self.tmp2[e - 1 - c3] = it;
                    c3 += 1;
                }
                _ => {
                    self.vho[s + stay] = it;
                    stay += 1;
                }
            }
        }

        let starts = [s + stay, s + stay + c0, s + stay + c0 + c1, s + stay + c0 + c1 + c2];
        self.vho[starts[0]..starts[0] + c0].copy_from_slice(&self.tmp1[s..s + c0]);
        self.vho[starts[1]..starts[1] + c1].copy_from_slice(&self.tmp2[s..s + c1]);
        self.vho[starts[2]..starts[2] + c2].copy_from_slice(&self.tmp1[e - c2..e]);
        self.vho[starts[3]..starts[3] + c3].copy_from_slice(&self.tmp2[e - c3..e]);
        let counts = [c0, c1, c2, c3];
        for i in 0..4 {
            let child = &mut self.nodes[children as usize + i];
            child.start = starts[i] as u32;
            child.items = counts[i] as u32;
        }

        // We only early out for Primitive and HitTarget objects
        if unique.is_some_and(|u| u.kind == OwnerKind::Other) {
            unique = None;
        }
        {
            let n = self.node_mut(node);
            n.children = children;
            n.vcenter = vcenter;
            n.items = stay as u32;
            n.unique = unique;
        }

        // check if at least two nodes feature objects, otherwise don't bother subdividing further
        let count_empty = usize::from(stay == 0) + counts.iter().filter(|&&c| c == 0).count();
        if count_empty >= 4 {
            level_empty += 1;
        } else {
            level_empty = 0;
        }

        if vcenter.x - bounds.left > 0.0001 // !! magic
            && level_empty <= 8 // If 8 levels were all just subdividing the same objects without luck, exit & Free the nodes again (but at least empty space was cut off)
            && level + 1 < 128 / 3
        {
            for i in 0..4u32 {
                let child_bounds = FRect {
                    left: if i & 1 != 0 { vcenter.x } else { bounds.left },
                    top: if i & 2 != 0 { vcenter.y } else { bounds.top },
                    right: if i & 1 != 0 { bounds.right } else { vcenter.x },
                    bottom: if i & 2 != 0 { bounds.bottom } else { vcenter.y },
                };
                self.create_next_level(children + i, &child_bounds, level + 1, level_empty);
            }
        }
    }

    /// VPX's `HitQuadtree::HitTestBall` (SSE build): offers `test` every
    /// collider the ball may hit, in VPX's order, possibly more than once.
    ///
    /// `active` says whether a primitive is collidable or a hit target is
    /// standing; subtrees made only of an inactive one are skipped. Draws one
    /// random number from `rng`, as VPX does.
    pub fn hit_test_ball(
        &self,
        ball: &BallQuery,
        rng: &mut Mwc64x,
        mut active: impl FnMut(Owner) -> bool,
        mut test: impl FnMut(u32),
    ) {
        let mut stack = [NONE; MAX_LEVEL as usize];
        let mut stackpos = 0usize; // stack[0] is the sentinel
        let mut current = ROOT;

        let traversal_order = rng.rand_01() < 0.5; // swaps test order in leafs randomly
        let q = Lanes::new(ball);

        loop {
            let node = self.node(current);
            // early out if only one unique primitive/hittarget stored inside all of the subtree/current
            // node that is also not collidable (at the moment)
            if node.unique.is_none_or(&mut active) {
                if node.items != 0 {
                    for_each_group(node.start, node.items, traversal_order, |i| {
                        // the box test only decides whether the group is looked at at all
                        let mask = self.sse.group_mask(i, &q);
                        if mask == 0 {
                            return;
                        }
                        for k in 0..4 {
                            // padding never passes (VPX doesn't check; only an infinite search
                            // radius would get there, and VPX then reads past its list)
                            if mask & (1 << k) != 0
                                && let Some(it) = self.vho.get(i * 4 + k)
                            {
                                test(it.id);
                            }
                        }
                    });
                }

                if node.children != NONE {
                    let left = ball.bbox.left <= node.vcenter.x;
                    let right = ball.bbox.right >= node.vcenter.x;
                    if ball.bbox.top <= node.vcenter.y {
                        if left {
                            stackpos += 1;
                            stack[stackpos] = node.children;
                        }
                        if right {
                            stackpos += 1;
                            stack[stackpos] = node.children + 1;
                        }
                    }
                    if ball.bbox.bottom >= node.vcenter.y {
                        if left {
                            stackpos += 1;
                            stack[stackpos] = node.children + 2;
                        }
                        if right {
                            stackpos += 1;
                            stack[stackpos] = node.children + 3;
                        }
                    }
                }
            }

            current = stack[stackpos];
            if current == NONE {
                return; // the sentinel: stack is empty
            }
            stackpos -= 1;
        }
    }
}
