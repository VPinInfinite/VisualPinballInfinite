// Ported from the copy in Visual Pinball X: third-party/include/progmesh.cpp, progmesh.h and
// progmesh_vecmatquat_minimal.h, commit af26b2d (10.8.1 pre-release build 5436). Original notice:
//
//   Progressive Mesh type Polygon Reduction Algorithm
//   by Stan Melax (c) 1998
//   Permission to use any of this code wherever you want is granted..
//   Although, please do acknowledge authorship if appropriate.
//
// This Rust port, by Claude for Visual Pinball Infinite, keeps the algorithm and its data structures'
// order of operations exactly, with indices in place of pointers. It is part of a GPL-3.0-or-later
// project.

//! Stan Melax's progressive mesh: collapses a mesh's edges one by one,
//! cheapest first, down to nothing, and records the order. VPX uses it to
//! give a primitive fewer collision triangles than its visible mesh when the
//! table asks for that (its collision reduction factor).
//!
//! Which edge goes next depends on ties and on the order of each vertex's
//! lists, which removals reshuffle (the last element fills the gap). The port
//! keeps those lists and removals as they are, so it collapses the same edges
//! in the same order as VPX.

/// The original's `float3`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Float3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Float3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn sub(self, b: Float3) -> Float3 {
        Float3::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
    fn scale(self, s: f32) -> Float3 {
        Float3::new(self.x * s, self.y * s, self.z * s)
    }
}

fn dot(a: Float3, b: Float3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: Float3, b: Float3) -> Float3 {
    Float3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

fn magnitude(v: Float3) -> f32 {
    dot(v, v).sqrt()
}

/// `std::min`: the second only when strictly smaller.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// `std::max`: the second only when strictly larger.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// The original's `tridata`.
pub type TriData = [u32; 3];

/// `RemoveFillWithBack`: removes `t` by moving the last element into its place.
fn remove_fill_with_back(c: &mut Vec<usize>, t: usize) {
    let idxof = c.iter().position(|&x| x == t).expect("element present");
    let val = c.pop().expect("not empty");
    if idxof == c.len() {
        return;
    }
    c[idxof] = val;
}

/// `AddUnique`.
fn add_unique(c: &mut Vec<usize>, t: usize) {
    if !c.contains(&t) {
        c.push(t);
    }
}

struct Vertex {
    position: Float3,
    /// place of vertex in original array
    id: u32,
    neighbor: Vec<usize>,
    face: Vec<usize>,
    /// cached cost of collapsing edge
    objdist: f32,
    /// candidate vertex for collapse
    collapse: Option<usize>,
}

struct Triangle {
    vertex: [usize; 3],
    normal: Float3,
}

/// The original's global lists, with every vertex and triangle ever made
/// (by index) and the live ones in the original's order.
struct Mesh {
    verts: Vec<Vertex>,
    tris: Vec<Triangle>,
    vertices: Vec<usize>,
    triangles: Vec<usize>,
}

impl Mesh {
    fn has_vertex(&self, t: usize, v: usize) -> bool {
        let tv = &self.tris[t].vertex;
        v == tv[0] || v == tv[1] || v == tv[2]
    }

    fn compute_normal(&mut self, t: usize) {
        let [a, b, c] = self.tris[t].vertex;
        let v0 = self.verts[a].position;
        let v1 = self.verts[b].position;
        let v2 = self.verts[c].position;
        let mut normal = cross(v1.sub(v0), v2.sub(v1));
        let l = magnitude(normal);
        if l > f32::MIN_POSITIVE {
            normal = normal.scale(1.0 / l);
        }
        self.tris[t].normal = normal;
    }

    fn new_triangle(&mut self, v0: usize, v1: usize, v2: usize) {
        let t = self.tris.len();
        self.tris.push(Triangle { vertex: [v0, v1, v2], normal: Float3::default() });
        self.compute_normal(t);
        self.triangles.push(t);

        let vertex = self.tris[t].vertex;
        for i in 0..3 {
            self.verts[vertex[i]].face.push(t);
            for j in 0..3 {
                if i != j {
                    add_unique(&mut self.verts[vertex[i]].neighbor, vertex[j]);
                }
            }
        }
    }

    fn delete_triangle(&mut self, t: usize) {
        remove_fill_with_back(&mut self.triangles, t);
        let vertex = self.tris[t].vertex;
        for &v in &vertex {
            remove_fill_with_back(&mut self.verts[v].face, t);
        }
        for i in 0..3 {
            let i2 = (i + 1) % 3;
            self.remove_if_non_neighbor(vertex[i], vertex[i2]);
            self.remove_if_non_neighbor(vertex[i2], vertex[i]);
        }
    }

    fn replace_vertex(&mut self, t: usize, vold: usize, vnew: usize) {
        let tv = &mut self.tris[t].vertex;
        if vold == tv[0] {
            tv[0] = vnew;
        } else if vold == tv[1] {
            tv[1] = vnew;
        } else {
            tv[2] = vnew;
        }
        remove_fill_with_back(&mut self.verts[vold].face, t);
        self.verts[vnew].face.push(t);

        let vertex = self.tris[t].vertex;
        for &v in &vertex {
            self.remove_if_non_neighbor(vold, v);
            self.remove_if_non_neighbor(v, vold);
        }
        for i in 0..3 {
            for j in 0..3 {
                if i != j {
                    add_unique(&mut self.verts[vertex[i]].neighbor, vertex[j]);
                }
            }
        }
        self.compute_normal(t);
    }

    fn delete_vertex(&mut self, v: usize) {
        while !self.verts[v].neighbor.is_empty() {
            let n0 = self.verts[v].neighbor[0];
            remove_fill_with_back(&mut self.verts[n0].neighbor, v);
            remove_fill_with_back(&mut self.verts[v].neighbor, n0);
        }
        remove_fill_with_back(&mut self.vertices, v);
    }

    /// Removes `n` from `v`'s neighbors if no face of `v` still has it.
    fn remove_if_non_neighbor(&mut self, v: usize, n: usize) {
        if !self.verts[v].neighbor.contains(&n) {
            return;
        }
        for &f in &self.verts[v].face {
            if self.has_vertex(f, n) {
                return;
            }
        }
        remove_fill_with_back(&mut self.verts[v].neighbor, n);
    }

    /// How much the model changes if `u` moves onto `v`.
    fn compute_edge_collapse_cost(&self, u: usize, v: usize) -> f32 {
        let uf = &self.verts[u].face;
        // find the "sides" triangles that are on the edge uv
        let sides: Vec<usize> = uf.iter().copied().filter(|&f| self.has_vertex(f, v)).collect();

        // use the triangle facing most away from the sides to determine our curvature term
        let mut curvature = 0.0f32;
        for &f in uf {
            let mut mincurv = 1.0f32; // curve for face i and closer side to it
            for &s in &sides {
                let dotprod = dot(self.tris[f].normal, self.tris[s].normal); // use dot product of face normals.
                mincurv = std_min(mincurv, (1.0 - dotprod) * 0.5);
            }
            curvature = std_max(curvature, mincurv);
        }

        // the more coplanar the lower the curvature term
        let edgelength = magnitude(self.verts[v].position.sub(self.verts[u].position));
        edgelength * curvature
    }

    fn compute_edge_cost_at_vertex(&mut self, v: usize) {
        if self.verts[v].neighbor.is_empty() {
            // v doesn't have neighbors so it costs nothing to collapse
            self.verts[v].collapse = None;
            self.verts[v].objdist = -0.01;
            return;
        }
        let mut objdist = f32::MAX;
        let mut collapse = None;
        // search all neighboring edges for "least cost" edge
        for i in 0..self.verts[v].neighbor.len() {
            let n = self.verts[v].neighbor[i];
            let dist = self.compute_edge_collapse_cost(v, n);
            if dist < objdist {
                collapse = Some(n); // candidate for edge collapse
                objdist = dist; // cost of the collapse
            }
        }
        self.verts[v].objdist = objdist;
        self.verts[v].collapse = collapse;
    }

    /// Collapses the edge uv by moving `u` onto `v`.
    fn collapse(&mut self, u: usize, v: Option<usize>) {
        let Some(v) = v else {
            // u is a vertex all by itself so just delete it
            self.delete_vertex(u);
            return;
        };
        let tmp = self.verts[u].neighbor.clone();

        // delete triangles on edge uv
        let mut i = self.verts[u].face.len();
        while i > 0 {
            i -= 1;
            let f = self.verts[u].face[i];
            if self.has_vertex(f, v) {
                self.delete_triangle(f);
            }
        }
        // update remaining triangles to have v instead of u
        let mut i = self.verts[u].face.len();
        while i > 0 {
            i -= 1;
            let f = self.verts[u].face[i];
            self.replace_vertex(f, u, v);
        }
        self.delete_vertex(u);

        // recompute the edge collapse costs for neighboring vertices
        for t in tmp {
            self.compute_edge_cost_at_vertex(t);
        }
    }

    /// The vertex whose cheapest edge is the cheapest of all (the first one on ties).
    fn minimum_cost_edge(&self) -> usize {
        let mut mn = self.vertices[0];
        for &v in &self.vertices {
            if self.verts[v].objdist < self.verts[mn].objdist {
                mn = v;
            }
        }
        mn
    }
}

/// `ProgressiveMesh`: the collapse order (`permutation`, new position of
/// each vertex) and, after permuting, the vertex each one collapses to
/// (`map`). Both are empty without vertices or triangles.
pub fn progressive_mesh(vert: &[Float3], tri: &[TriData]) -> (Vec<u32>, Vec<u32>) {
    if vert.is_empty() || tri.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut m = Mesh {
        verts: Vec::with_capacity(vert.len()),
        tris: Vec::with_capacity(tri.len()),
        vertices: Vec::with_capacity(vert.len()),
        triangles: Vec::with_capacity(tri.len()),
    };
    // put input data into our data structures
    for (i, p) in vert.iter().enumerate() {
        m.verts.push(Vertex { position: *p, id: i as u32, neighbor: Vec::new(), face: Vec::new(), objdist: 0.0, collapse: None });
        m.vertices.push(i);
    }
    for t in tri {
        let (a, b, c) = (m.vertices[t[0] as usize], m.vertices[t[1] as usize], m.vertices[t[2] as usize]);
        m.new_triangle(a, b, c);
    }
    // cache all edge collapse costs
    for i in 0..m.vertices.len() {
        let v = m.vertices[i];
        m.compute_edge_cost_at_vertex(v);
    }

    let n = m.vertices.len();
    let mut permutation = vec![0u32; n];
    let mut map = vec![0u32; n];

    // reduce the object down to nothing
    while !m.vertices.is_empty() {
        // get the next vertex to collapse
        let mn = m.minimum_cost_edge();
        let last = (m.vertices.len() - 1) as u32;
        // keep track of this vertex, i.e. the collapse ordering
        permutation[m.verts[mn].id as usize] = last;
        // keep track of vertex to which we collapse to
        let collapse = m.verts[mn].collapse;
        map[last as usize] = collapse.map_or(!0u32, |c| m.verts[c].id);
        // Collapse this edge
        m.collapse(mn, collapse);
    }

    // reorder the map array based on the collapse ordering
    for x in map.iter_mut() {
        *x = if *x == !0u32 { 0 } else { permutation[*x as usize] };
    }
    (map, permutation)
}

/// `PermuteVertices`: puts the vertices in collapse order and renumbers the
/// triangles to match.
pub fn permute_vertices<T: Copy>(permutation: &[u32], vert: &mut [T], tri: &mut [TriData]) {
    // rearrange the vertex array
    let temp: Vec<T> = vert.to_vec();
    for (i, v) in temp.into_iter().enumerate() {
        vert[permutation[i] as usize] = v;
    }
    // update the changes in the entries in the triangle array
    for t in tri.iter_mut() {
        for v in t.iter_mut() {
            *v = permutation[*v as usize];
        }
    }
}

/// `MapVertex`: follows the collapses until the vertex is one of the first `mx`.
fn map_vertex(mut a: u32, mx: u32, map: &[u32]) -> u32 {
    while a >= mx {
        a = map[a as usize];
    }
    a
}

/// `ReMapIndices`: the triangles of the mesh reduced to `num_vertices`
/// vertices, without the ones that collapsed to a line or point.
pub fn remap_indices(num_vertices: u32, tri: &[TriData], map: &[u32]) -> Vec<TriData> {
    let mut new_tri = Vec::new();
    for t in tri {
        let n = [map_vertex(t[0], num_vertices, map), map_vertex(t[1], num_vertices, map), map_vertex(t[2], num_vertices, map)];
        if n[0] == n[1] || n[1] == n[2] || n[2] == n[0] {
            continue;
        }
        new_tri.push(n);
    }
    new_tri
}
