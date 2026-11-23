// Ported from the copy in Visual Pinball X: third-party/include/forsyth.h, commit af26b2d (10.8.1
// pre-release build 5436), which VPX altered (templated vertex type, const correctness, remember if
// already init'ed, vertex cache size 64, structs). Original notice:
//
//   Copyright (C) 2008 Martin Storsjo
//
//   This software is provided 'as-is', without any express or implied
//   warranty.  In no event will the authors be held liable for any damages
//   arising from the use of this software.
//
//   Permission is granted to anyone to use this software for any purpose,
//   including commercial applications, and to alter it and redistribute it
//   freely, subject to the following restrictions:
//
//   1. The origin of this software must not be misrepresented; you must not
//      claim that you wrote the original software. If you use this software
//      in a product, an acknowledgment in the product documentation would be
//      appreciated but is not required.
//   2. Altered source versions must be plainly marked as such, and must not be
//      misrepresented as being the original software.
//   3. This notice may not be removed or altered from any source distribution.
//
// Altered: this is a Rust port, by Claude for Visual Pinball Infinite, of VPX's altered version,
// keeping its arithmetic and order of operations. It is part of a GPL-3.0-or-later project.

//! Tom Forsyth's vertex cache optimisation, as Martin Storsjö implemented
//! it: reorders a mesh's triangles so a graphics card reuses more of its
//! work. VPX applies it to every primitive's mesh in tables saved before
//! 10.1.1 as it loads them, which changes the order of that primitive's
//! colliders.

use std::sync::OnceLock;

// Set these to adjust the performance and result quality
const VERTEX_CACHE_SIZE: usize = 64;
const CACHE_FUNCTION_LENGTH: u32 = 64;

// The size of these data types affect the memory usage
type ScoreType = u16;
const SCORE_SCALING: f32 = 7281.0;

const MAX_ADJACENCY: u8 = u8::MAX;

// The size of the precalculated tables
const CACHE_SCORE_TABLE_SIZE: usize = 64;
const VALENCE_SCORE_TABLE_SIZE: usize = 64;

// Score function constants
const CACHE_DECAY_POWER: f32 = 1.5;
const LAST_TRI_SCORE: f32 = 0.75;
const VALENCE_BOOST_SCALE: f32 = 2.0;
const VALENCE_BOOST_POWER: f32 = 0.5;

#[derive(Clone, Copy, Default)]
struct VertexData {
    offset: i32,
    last_score: ScoreType,
    num_active_tri: u8,
    cache_tag: i8,
}

struct Tables {
    cache_position_score: [ScoreType; CACHE_SCORE_TABLE_SIZE],
    valence_score: [ScoreType; VALENCE_SCORE_TABLE_SIZE],
}

/// `initForsyth`: the precalculated tables.
fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut t = Tables { cache_position_score: [0; CACHE_SCORE_TABLE_SIZE], valence_score: [0; VALENCE_SCORE_TABLE_SIZE] };
        for i in 0..CACHE_SCORE_TABLE_SIZE as u32 {
            let score = if i < 3 {
                // This vertex was used in the last triangle, so it has a fixed score, which ever of
                // the three it's in. Otherwise, you can get very different answers depending on
                // whether you add the triangle 1,2,3 or 3,1,2 - which is silly
                LAST_TRI_SCORE
            } else {
                // Points for being high in the cache.
                let scaler = 1.0 / (CACHE_FUNCTION_LENGTH - 3) as f32;
                (1.0 - (i - 3) as f32 * scaler).powf(CACHE_DECAY_POWER)
            };
            t.cache_position_score[i as usize] = (SCORE_SCALING * score) as ScoreType;
        }
        for i in 1..VALENCE_SCORE_TABLE_SIZE {
            // Bonus points for having a low number of tris still to use the vert, so we get rid of
            // lone verts quickly
            let valence_boost = (i as f32).powf(-VALENCE_BOOST_POWER);
            let score = VALENCE_BOOST_SCALE * valence_boost;
            t.valence_score[i] = (SCORE_SCALING * score) as ScoreType;
        }
        t
    })
}

/// `findVertexScore`. `cache_position` is the cache tag as VPX passes it: a
/// signed tag read as unsigned, so -1 (not cached) is out of range.
fn find_vertex_score(t: &Tables, num_active_tris: i32, cache_position: u32) -> ScoreType {
    if num_active_tris == 0 {
        // No triangles need this vertex!
        return 0;
    }

    let mut score: ScoreType = if cache_position >= CACHE_SCORE_TABLE_SIZE as u32 {
        0 // Vertex is not in LRU cache - no score
    } else {
        t.cache_position_score[cache_position as usize]
    };

    if (num_active_tris as usize) < VALENCE_SCORE_TABLE_SIZE {
        score = score.wrapping_add(t.valence_score[num_active_tris as usize]);
    } else {
        let valence_boost = (num_active_tris as f32).powf(-VALENCE_BOOST_POWER);
        let s = VALENCE_BOOST_SCALE * valence_boost;
        score = score.wrapping_add((SCORE_SCALING * s) as ScoreType);
    }
    score
}

/// `reorderForsyth`: the triangles in a cache-friendly order, or `None` where
/// VPX keeps the mesh as it is (empty, a vertex in more than 255 triangles, a
/// degenerate mesh, or a cache overrun).
pub fn reorder_forsyth(indices: &[u32], n_vertices: usize) -> Option<Vec<u32>> {
    if indices.is_empty() || n_vertices == 0 {
        return None;
    }
    let t = tables();

    let mut c_vertex = vec![VertexData::default(); n_vertices];

    // First scan over the vertex data, count the total number of occurrances of each vertex
    for &i in indices {
        let c = &mut c_vertex[i as usize];
        if c.num_active_tri == MAX_ADJACENCY {
            // Unsupported mesh, vertex shared by too many triangles
            return None;
        }
        c.num_active_tri += 1;
        if c.num_active_tri as usize == indices.len() {
            // Degenerated mesh
            return None;
        }
    }

    let n_triangles = indices.len() / 3;

    let mut triangle_added = vec![false; n_triangles];
    let mut triangle_score: Vec<ScoreType> = vec![0; n_triangles];
    let mut out_triangles = vec![0u32; n_triangles];
    let mut triangle_indices = vec![0u32; indices.len()];

    // Count the triangle array offset for each vertex, initialize the rest of the data.
    let mut sum = 0i32;
    for c in c_vertex.iter_mut() {
        c.offset = sum;
        sum += c.num_active_tri as i32;
        c.num_active_tri = 0;
        c.cache_tag = -1;
    }

    // Fill the vertex data structures with indices to the triangles using each vertex
    for i in 0..n_triangles {
        for j in 0..3 {
            let c = &mut c_vertex[indices[3 * i + j] as usize];
            triangle_indices[(c.offset + c.num_active_tri as i32) as usize] = i as u32;
            c.num_active_tri += 1;
        }
    }

    // Initialize the score for all vertices
    for c in c_vertex.iter_mut() {
        c.last_score = find_vertex_score(t, c.num_active_tri as i32, c.cache_tag as i32 as u32);
        for j in 0..c.num_active_tri as i32 {
            let tri = triangle_indices[(c.offset + j) as usize] as usize;
            triangle_score[tri] = triangle_score[tri].wrapping_add(c.last_score);
        }
    }

    // Find the best triangle
    let mut best_triangle: i32 = -1;
    let mut best_score: i32 = -1;

    for (i, &s) in triangle_score.iter().enumerate() {
        if s as i32 > best_score {
            best_score = s as i32;
            best_triangle = i as i32;
        }
    }

    // Initialize the cache
    let mut cache = [-1i32; VERTEX_CACHE_SIZE + 3];

    let mut scan_pos = 0usize;
    let mut out_pos = 0usize;

    // Output the currently best triangle, as long as there are triangles left to output
    while best_triangle >= 0 {
        let best = best_triangle as usize;
        // Mark the triangle as added
        triangle_added[best] = true;
        // Output this triangle
        out_triangles[out_pos] = best as u32;
        out_pos += 1;
        for i in 0..3 {
            // Update this vertex
            let v = indices[3 * best + i] as usize;

            // Check the current cache position, if it is in the cache
            let mut endpos = c_vertex[v].cache_tag as i32;
            if endpos < 0 {
                endpos = (VERTEX_CACHE_SIZE + i) as i32;
            }
            // Move all cache entries from the previous position in the cache to the new target
            // position (i) one step backwards
            if endpos >= (VERTEX_CACHE_SIZE + 3) as i32 {
                return None;
            }
            let mut j = endpos as usize;
            while j > i {
                cache[j] = cache[j - 1];
                // If this cache slot contains a real vertex, update its cache tag
                if cache[j] >= 0 {
                    let c = &mut c_vertex[cache[j] as usize];
                    c.cache_tag += 1;
                    if c.cache_tag as i32 >= (VERTEX_CACHE_SIZE + 3) as i32 {
                        return None;
                    }
                }
                j -= 1;
            }
            // Insert the current vertex into its new target slot
            cache[i] = v as i32;
            c_vertex[v].cache_tag = i as i8;

            // Find the current triangle in the list of active triangles and remove it (moving the
            // last triangle in the list to the slot of this triangle).
            let c = c_vertex[v];
            for j in 0..c.num_active_tri as i32 {
                if triangle_indices[(c.offset + j) as usize] == best as u32 {
                    triangle_indices[(c.offset + j) as usize] =
                        triangle_indices[(c.offset + c.num_active_tri as i32 - 1) as usize];
                    break;
                }
            }
            // Shorten the list
            c_vertex[v].num_active_tri = c_vertex[v].num_active_tri.wrapping_sub(1);
        }

        // Update the scores of all triangles in the cache
        for (i, slot) in cache.iter_mut().enumerate() {
            let v = *slot;
            if v < 0 {
                break;
            }
            let v = v as usize;
            // This vertex has been pushed outside of the actual cache
            if i >= VERTEX_CACHE_SIZE {
                c_vertex[v].cache_tag = -1;
                *slot = -1;
            }
            let c = c_vertex[v];
            let new_score = find_vertex_score(t, c.num_active_tri as i32, c.cache_tag as i32 as u32);
            let diff = new_score.wrapping_sub(c.last_score);
            for j in 0..c.num_active_tri as i32 {
                let tri = triangle_indices[(c.offset + j) as usize] as usize;
                triangle_score[tri] = triangle_score[tri].wrapping_add(diff);
            }
            c_vertex[v].last_score = new_score;
        }

        // Find the best triangle referenced by vertices in the cache
        best_triangle = -1;
        best_score = -1;
        for &v in &cache[..VERTEX_CACHE_SIZE] {
            if v < 0 {
                break;
            }
            let c = c_vertex[v as usize];
            for j in 0..c.num_active_tri as i32 {
                let tri = triangle_indices[(c.offset + j) as usize];
                if triangle_score[tri as usize] as i32 > best_score {
                    best_triangle = tri as i32;
                    best_score = triangle_score[tri as usize] as i32;
                }
            }
        }

        // If no active triangle was found at all, continue scanning the whole list of triangles
        if best_triangle < 0 {
            while scan_pos < n_triangles {
                if !triangle_added[scan_pos] {
                    best_triangle = scan_pos as i32;
                    break;
                }
                scan_pos += 1;
            }
        }
    }

    // Convert the triangle index array into a full triangle list
    let mut out_pos = 0;
    for &tri in &out_triangles {
        let t = tri as usize * 3;
        for j in 0..3 {
            triangle_indices[out_pos] = indices[t + j];
            out_pos += 1;
        }
    }
    Some(triangle_indices)
}
