// Ported from Visual Pinball X: src/physics/kdtree.cpp and kdtree.h (license:GPLv3+),
// commit af26b2d (10.8.1 pre-release build 5436).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's k-d tree over the balls: the collision search for ball against ball.
//! VPX rebuilds it at the start of every physics cycle.
//!
//! As in the quadtree, the order of the search is part of the physics, so this
//! keeps VPX's SSE build exactly: groups of four that can straddle nodes (and
//! offer a ball more than once), a sphere test only, a random draw per search
//! for the walking direction, and children searched last-in-first-out.

use crate::math::bbox::FRect3D;
use crate::math::def::Mwc64x;
use crate::quadtree::{BallQuery, Lanes, SseBoxes, for_each_group};

/// One ball in the tree: its id and hit box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KdItem {
    pub id: u32,
    pub bbox: FRect3D,
}

const NONE: u32 = u32::MAX;
const ROOT: u32 = u32::MAX - 1;

#[derive(Clone, Copy, Debug)]
struct KdNode {
    rectbounds: FRect3D,
    /// First item in `org_idx`.
    start: u32,
    /// Number of items (bits 0..29) and split axis (bits 30..31).
    items: u32,
    /// First of two children in the node pool, or `NONE` for a leaf.
    children: u32,
}

impl KdNode {
    fn reset(&mut self) {
        self.children = NONE;
        self.start = 0;
        self.items = 0;
    }
}

impl Default for KdNode {
    fn default() -> Self {
        Self { rectbounds: FRect3D::default(), start: 0, items: 0, children: NONE }
    }
}

/// VPX's `HitKD` (with the SSE leaf test).
#[derive(Clone, Debug)]
pub struct HitKd {
    /// The balls, in the order they were added.
    vho: Vec<KdItem>,
    max_items: usize,
    /// The tree's order: indices into `vho`.
    org_idx: Vec<u32>,
    root: KdNode,
    nodes: Vec<KdNode>,
    num_nodes: usize,
    tmp: Vec<u32>,
    n_levels: u32,
    sse: SseBoxes,
}

impl Default for HitKd {
    fn default() -> Self {
        Self::new()
    }
}

impl HitKd {
    pub fn new() -> Self {
        Self {
            vho: Vec::new(),
            max_items: 0,
            org_idx: Vec::new(),
            root: KdNode::default(),
            nodes: Vec::new(),
            num_nodes: 0,
            tmp: Vec::new(),
            n_levels: 0,
            sse: SseBoxes::default(),
        }
    }

    /// VPX's `Reset`: replaces all balls and rebuilds. The boxes must be current.
    pub fn reset(&mut self, items: Vec<KdItem>) {
        self.vho = items;
        self.initialize();
    }

    pub fn insert(&mut self, item: KdItem) {
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

    /// VPX's `HitKD::Update`: the balls have moved; takes their new boxes
    /// (from each ball's `calc_hit_bbox`) and rebuilds.
    pub fn update(&mut self, mut bbox_of: impl FnMut(u32) -> FRect3D) {
        for it in &mut self.vho {
            it.bbox = bbox_of(it.id);
        }
        self.initialize();
    }

    /// VPX's `Finalize`: frees the build's scratch space.
    pub fn finalize(&mut self) {
        self.tmp = Vec::new();
    }

    /// The balls, in the order they were added.
    pub fn items(&self) -> &[KdItem] {
        &self.vho
    }

    pub fn object_count(&self) -> usize {
        self.vho.len()
    }

    pub fn n_levels(&self) -> u32 {
        self.n_levels
    }

    fn initialize(&mut self) {
        let n = self.vho.len();
        if n > self.max_items {
            self.max_items = n;
            self.org_idx = vec![0; n];
            self.tmp = vec![0; n];
            // VPX: "no a priori bound on the number of nodes in the tree. We just make an educated
            // guess on the maximum and truncate the subdivision if we run out of nodes."
            self.nodes = vec![KdNode::default(); (n * 2 + 1) & !1];
        }
        if self.tmp.len() < self.max_items {
            self.tmp.resize(self.max_items, 0); // did somebody call finalize inbetween?
        }

        self.num_nodes = 0;
        self.root.reset();
        self.root.rectbounds.clear();
        self.root.start = 0;
        self.root.items = n as u32;
        for i in 0..n {
            self.root.rectbounds.extend(&self.vho[i].bbox);
            self.org_idx[i] = i as u32;
        }

        self.n_levels = 0;
        self.create_next_level(ROOT, 0, 0);

        let (vho, org_idx) = (&self.vho, &self.org_idx);
        self.sse.fill(org_idx[..n].iter().map(|&j| vho[j as usize].bbox));
    }

    fn node(&self, r: u32) -> &KdNode {
        if r == ROOT { &self.root } else { &self.nodes[r as usize] }
    }

    fn node_mut(&mut self, r: u32) -> &mut KdNode {
        if r == ROOT { &mut self.root } else { &mut self.nodes[r as usize] }
    }

    fn item_at(&self, i: usize) -> &KdItem {
        &self.vho[self.org_idx[i] as usize]
    }

    /// VPX's `AllocTwoNodes`: `None` once the pool is used up. Unlike the
    /// quadtree's, it doesn't reset the nodes.
    fn alloc_two_nodes(&mut self) -> Option<u32> {
        if self.num_nodes + 1 >= self.nodes.len() {
            return None;
        }
        self.num_nodes += 2;
        Some((self.num_nodes - 2) as u32)
    }

    /// VPX's `HitKDNode::CreateNextLevel`.
    fn create_next_level(&mut self, node: u32, level: u32, mut level_empty: u32) {
        let (rect, start, org_items) = {
            let n = self.node(node);
            (n.rectbounds, n.start as usize, (n.items & 0x3FFF_FFFF) as usize)
        };

        if org_items <= 4 // !! magic
            || level >= 128 / 2
        {
            return;
        }

        let vdiag = (rect.right - rect.left, rect.bottom - rect.top, rect.zhigh - rect.zlow);
        let axis: u32 = if vdiag.0 > vdiag.1 && vdiag.0 > vdiag.2 {
            if vdiag.0 < 0.0001 {
                return; // !! magic
            }
            0
        } else if vdiag.1 > vdiag.2 {
            if vdiag.1 < 0.0001 {
                return;
            }
            1
        } else {
            if vdiag.2 < 0.0001 {
                return;
            }
            2
        };

        // create children, calc bboxes
        let Some(children) = self.alloc_two_nodes() else {
            return; // ran out of nodes - abort
        };
        let (c0, c1) = (children as usize, children as usize + 1);
        self.node_mut(node).children = children;
        self.nodes[c0].rectbounds = rect;
        self.nodes[c1].rectbounds = rect;

        self.n_levels += 1;

        let vcenter = ((rect.left + rect.right) * 0.5, (rect.top + rect.bottom) * 0.5, (rect.zlow + rect.zhigh) * 0.5);
        match axis {
            0 => {
                self.nodes[c0].rectbounds.right = vcenter.0;
                self.nodes[c1].rectbounds.left = vcenter.0;
            }
            1 => {
                self.nodes[c0].rectbounds.bottom = vcenter.1;
                self.nodes[c1].rectbounds.top = vcenter.1;
            }
            _ => {
                self.nodes[c0].rectbounds.zhigh = vcenter.2;
                self.nodes[c1].rectbounds.zlow = vcenter.2;
            }
        }
        self.nodes[c0].children = NONE;
        self.nodes[c1].children = NONE;

        // which side of the split plane an item is on: 0, 1, or across it (2)
        let side = |b: &FRect3D| -> u8 {
            let (lo, hi, c) = match axis {
                0 => (b.left, b.right, vcenter.0),
                1 => (b.top, b.bottom, vcenter.1),
                _ => (b.zlow, b.zhigh, vcenter.2),
            };
            if hi < c {
                0
            } else if lo > c {
                1
            } else {
                2
            }
        };

        // determine amount of items that cross splitplane, or are passed on to the children
        let (mut n0, mut n1) = (0usize, 0usize);
        for i in start..start + org_items {
            match side(&self.item_at(i).bbox) {
                0 => n0 += 1,
                1 => n1 += 1,
                _ => {}
            }
        }

        // check if at least two nodes feature objects, otherwise don't bother subdividing further
        let count_empty = usize::from(n0 == 0) + usize::from(n1 == 0) + usize::from(org_items - n0 - n1 == 0);
        if count_empty >= 2 {
            level_empty += 1;
        } else {
            level_empty = 0;
        }

        if level_empty > 8 {
            // If 8 levels were all just subdividing the same objects without luck, exit & Free the nodes
            // again (but at least empty space was cut off)
            self.num_nodes -= 2;
            self.node_mut(node).children = NONE;
            return;
        }

        let s0 = start + org_items - n0 - n1;
        let s1 = s0 + n0;

        // sort items that cross splitplane in-place, the others are sorted into a temporary
        let (mut items, mut k0, mut k1) = (0usize, 0usize, 0usize);
        for i in start..start + org_items {
            let idx = self.org_idx[i];
            match side(&self.vho[idx as usize].bbox) {
                0 => {
                    self.tmp[s0 + k0] = idx;
                    k0 += 1;
                }
                1 => {
                    self.tmp[s1 + k1] = idx;
                    k1 += 1;
                }
                _ => {
                    self.org_idx[start + items] = idx;
                    items += 1;
                }
            }
        }

        self.node_mut(node).items = items as u32 | (axis << 30);
        self.nodes[c0].start = s0 as u32;
        self.nodes[c0].items = k0 as u32;
        self.nodes[c1].start = s1 as u32;
        self.nodes[c1].items = k1 as u32;

        // copy temporary back
        self.org_idx[s0..s0 + k0].copy_from_slice(&self.tmp[s0..s0 + k0]);
        self.org_idx[s1..s1 + k1].copy_from_slice(&self.tmp[s1..s1 + k1]);

        self.create_next_level(children, level + 1, level_empty);
        self.create_next_level(children + 1, level + 1, level_empty);
    }

    /// VPX's `HitKDNode::HitTestBall` (SSE build): offers `test` every ball
    /// that `ball` may hit, in VPX's order, possibly more than once, never
    /// the ball `exclude` itself. Draws one random number from `rng`.
    pub fn hit_test_ball(&self, ball: &BallQuery, exclude: u32, rng: &mut Mwc64x, mut test: impl FnMut(u32)) {
        let mut stack = [NONE; 128];
        let mut stackpos = 0usize; // stack[0] is the sentinel
        let mut current = ROOT;

        let traversal_order = rng.rand_01() < 0.5; // swaps test order in leafs randomly
        let q = Lanes::new(ball);

        loop {
            let node = self.node(current);
            let org_items = node.items & 0x3FFF_FFFF;
            let axis = node.items >> 30;

            // unlike the quadtree, VPX looks at a node's groups even when it has no items of its own
            for_each_group(node.start, org_items, traversal_order, |i| {
                let mask = self.sse.sphere_mask(i, &q);
                for k in 0..4 {
                    // padding never passes (VPX doesn't check; only an infinite search radius would
                    // get there, and VPX then reads past its list)
                    if mask & (1 << k) != 0 && i * 4 + k < self.vho.len() {
                        let id = self.item_at(i * 4 + k).id;
                        if id != exclude {
                            test(id); // ball can not hit itself
                        }
                    }
                }
            });

            if node.children != NONE {
                let r = &node.rectbounds;
                let (lo_ok, hi_ok) = match axis {
                    0 => {
                        let vcenter = (r.left + r.right) * 0.5;
                        (ball.bbox.left <= vcenter, ball.bbox.right >= vcenter)
                    }
                    1 => {
                        let vcenter = (r.top + r.bottom) * 0.5;
                        (ball.bbox.top <= vcenter, ball.bbox.bottom >= vcenter)
                    }
                    _ => {
                        let vcenter = (r.zlow + r.zhigh) * 0.5;
                        (ball.bbox.zlow <= vcenter, ball.bbox.zhigh >= vcenter)
                    }
                };
                if lo_ok {
                    stackpos += 1;
                    stack[stackpos] = node.children;
                }
                if hi_ok {
                    stackpos += 1;
                    stack[stackpos] = node.children + 1;
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
