//! Reads the binary glTF (GLB) that `vpin` exports from a table — its meshes,
//! materials and images — which the player can draw (`--shapes export`). Only
//! what `vpin`'s exporter writes is read.

use std::ops::Range;

use serde_json::Value;

/// A part of the table as the exporter wrote it: a named node with its own
/// placement, a mesh, and child nodes.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub mesh: Option<usize>,
    pub children: Vec<usize>,
    pub translation: [f32; 3],
    /// A quaternion, x, y, z, w.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// False for a part hidden at the start (`KHR_node_visibility`); scripts
    /// can show it later.
    pub visible: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Primitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    /// Index into `Document::images`.
    pub image: Option<usize>,
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}

#[derive(Clone, Debug)]
pub struct Image {
    pub name: String,
    pub mime_type: String,
    /// The encoded image (PNG, JPEG, WebP, …) within `Document::bin`.
    pub bytes: Range<usize>,
}

/// A whole export.
pub struct Document {
    pub nodes: Vec<Node>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub images: Vec<Image>,
    /// The scene's top-level nodes.
    pub roots: Vec<usize>,
    pub bin: Vec<u8>,
}

impl Document {
    pub fn image_bytes(&self, image: usize) -> &[u8] {
        &self.bin[self.images[image].bytes.clone()]
    }
}

const GLB_MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E_4942; // "BIN\0"

fn u32_at(bytes: &[u8], at: usize) -> Result<u32, String> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| "GLB ends early".to_string())
}

/// Parses a GLB file.
pub fn parse(glb: Vec<u8>) -> Result<Document, String> {
    if u32_at(&glb, 0)? != GLB_MAGIC {
        return Err("not a GLB file".into());
    }
    let mut at = 12;
    let mut json = None;
    let mut bin = None;
    while at + 8 <= glb.len() {
        let length = u32_at(&glb, at)? as usize;
        let kind = u32_at(&glb, at + 4)?;
        let body = at + 8..at + 8 + length;
        if body.end > glb.len() {
            return Err("GLB chunk runs past the end".into());
        }
        match kind {
            CHUNK_JSON => json = Some(serde_json::from_slice::<Value>(&glb[body.clone()]).map_err(|e| e.to_string())?),
            CHUNK_BIN => bin = Some(body.clone()),
            _ => {}
        }
        at = body.end;
    }
    let json = json.ok_or("GLB has no JSON chunk")?;
    let bin = match bin {
        Some(range) => {
            let mut glb = glb;
            glb.truncate(range.end);
            glb.drain(..range.start);
            glb
        }
        None => Vec::new(),
    };
    read(&json, bin)
}

fn array<'a>(json: &'a Value, key: &str) -> &'a [Value] {
    json.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn index(v: &Value, key: &str) -> Option<usize> {
    v.get(key).and_then(Value::as_u64).map(|i| i as usize)
}

fn floats<const N: usize>(v: Option<&Value>, default: [f32; N]) -> [f32; N] {
    let mut out = default;
    if let Some(a) = v.and_then(Value::as_array) {
        for (o, x) in out.iter_mut().zip(a) {
            *o = x.as_f64().unwrap_or(0.0) as f32;
        }
    }
    out
}

/// One accessor's data: where it starts, its stride and element count.
struct View<'a> {
    bytes: &'a [u8],
    start: usize,
    stride: usize,
    count: usize,
    component_type: u64,
    components: usize,
}

impl View<'_> {
    fn component(&self, element: usize, c: usize) -> f64 {
        let size = match self.component_type {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            _ => 4,
        };
        let at = self.start + element * self.stride + c * size;
        let b = &self.bytes[at..at + size];
        match self.component_type {
            5121 => f64::from(b[0]),
            5123 => f64::from(u16::from_le_bytes([b[0], b[1]])),
            5125 => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            _ => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        }
    }
}

fn accessor<'a>(json: &Value, bin: &'a [u8], accessor: usize) -> Result<View<'a>, String> {
    let a = array(json, "accessors").get(accessor).ok_or("missing accessor")?;
    let view = array(json, "bufferViews").get(index(a, "bufferView").ok_or("accessor without a buffer view")?).ok_or("missing buffer view")?;
    let component_type = a.get("componentType").and_then(Value::as_u64).unwrap_or(5126);
    let components = match a.get("type").and_then(Value::as_str).unwrap_or("SCALAR") {
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => 1,
    };
    let size = match component_type {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        _ => 4,
    };
    let start = index(view, "byteOffset").unwrap_or(0) + index(a, "byteOffset").unwrap_or(0);
    let stride = index(view, "byteStride").unwrap_or(size * components);
    let count = index(a, "count").unwrap_or(0);
    if count > 0 && start + (count - 1) * stride + size * components > bin.len() {
        return Err("accessor runs past the buffer".into());
    }
    Ok(View { bytes: bin, start, stride, count, component_type, components })
}

fn vectors<const N: usize>(json: &Value, bin: &[u8], a: usize) -> Result<Vec<[f32; N]>, String> {
    let v = accessor(json, bin, a)?;
    Ok((0..v.count)
        .map(|e| {
            let mut out = [0.0; N];
            for (c, o) in out.iter_mut().enumerate().take(v.components) {
                *o = v.component(e, c) as f32;
            }
            out
        })
        .collect())
}

fn read(json: &Value, bin: Vec<u8>) -> Result<Document, String> {
    let nodes = array(json, "nodes")
        .iter()
        .map(|n| Node {
            name: n.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
            mesh: index(n, "mesh"),
            children: array(n, "children").iter().filter_map(Value::as_u64).map(|i| i as usize).collect(),
            translation: floats(n.get("translation"), [0.0; 3]),
            rotation: floats(n.get("rotation"), [0.0, 0.0, 0.0, 1.0]),
            scale: floats(n.get("scale"), [1.0; 3]),
            visible: n
                .pointer("/extensions/KHR_node_visibility/visible")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        })
        .collect();

    let mut meshes = Vec::new();
    for m in array(json, "meshes") {
        let mut primitives = Vec::new();
        for p in array(m, "primitives") {
            let attributes = p.get("attributes").ok_or("primitive without attributes")?;
            let mut prim = Primitive { material: index(p, "material"), ..Default::default() };
            if let Some(a) = index(attributes, "POSITION") {
                prim.positions = vectors::<3>(json, &bin, a)?;
            }
            if let Some(a) = index(attributes, "NORMAL") {
                prim.normals = vectors::<3>(json, &bin, a)?;
            }
            if let Some(a) = index(attributes, "TEXCOORD_0") {
                prim.uvs = vectors::<2>(json, &bin, a)?;
            }
            prim.indices = match index(p, "indices") {
                Some(a) => vectors::<1>(json, &bin, a)?.into_iter().map(|[i]| i as u32).collect(),
                None => (0..prim.positions.len() as u32).collect(),
            };
            primitives.push(prim);
        }
        meshes.push(Mesh { name: m.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), primitives });
    }

    // textures point at images, either directly or through the WebP extension
    let texture_image: Vec<Option<usize>> = array(json, "textures")
        .iter()
        .map(|t| index(t, "source").or_else(|| t.pointer("/extensions/EXT_texture_webp/source").and_then(Value::as_u64).map(|i| i as usize)))
        .collect();

    let materials = array(json, "materials")
        .iter()
        .map(|m| {
            let pbr = m.get("pbrMetallicRoughness");
            Material {
                name: m.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                base_color: floats(pbr.and_then(|p| p.get("baseColorFactor")), [1.0; 4]),
                image: pbr
                    .and_then(|p| p.pointer("/baseColorTexture/index"))
                    .and_then(Value::as_u64)
                    .and_then(|t| texture_image.get(t as usize).copied().flatten()),
                metallic: pbr.and_then(|p| p.get("metallicFactor")).and_then(Value::as_f64).unwrap_or(1.0) as f32,
                roughness: pbr.and_then(|p| p.get("roughnessFactor")).and_then(Value::as_f64).unwrap_or(1.0) as f32,
                emissive: floats(m.get("emissiveFactor"), [0.0; 3]),
                alpha_mode: match m.get("alphaMode").and_then(Value::as_str) {
                    Some("MASK") => AlphaMode::Mask,
                    Some("BLEND") => AlphaMode::Blend,
                    _ => AlphaMode::Opaque,
                },
                alpha_cutoff: m.get("alphaCutoff").and_then(Value::as_f64).unwrap_or(0.5) as f32,
                double_sided: m.get("doubleSided").and_then(Value::as_bool).unwrap_or(false),
            }
        })
        .collect();

    let views = array(json, "bufferViews");
    let images = array(json, "images")
        .iter()
        .map(|i| {
            let bytes = index(i, "bufferView")
                .and_then(|v| views.get(v))
                .map(|v| {
                    let start = index(v, "byteOffset").unwrap_or(0);
                    start..start + index(v, "byteLength").unwrap_or(0)
                })
                .filter(|r| r.end <= bin.len())
                .unwrap_or(0..0);
            Image {
                name: i.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                mime_type: i.get("mimeType").and_then(Value::as_str).unwrap_or_default().to_string(),
                bytes,
            }
        })
        .collect();

    let scene = index(json, "scene").unwrap_or(0);
    let roots = array(json, "scenes")
        .get(scene)
        .map(|s| array(s, "nodes").iter().filter_map(Value::as_u64).map(|i| i as usize).collect())
        .unwrap_or_default();

    Ok(Document { nodes, meshes, materials, images, roots, bin })
}
