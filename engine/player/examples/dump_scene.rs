//! Prints the node tree of a table's mesh export, for checking what the
//! player draws: `cargo run -p player --example dump_scene -- table.vpx`.

use std::time::Instant;

fn print(doc: &player::gltf::Document, node: usize, depth: usize) {
    let n = &doc.nodes[node];
    let mesh = n.mesh.map(|m| {
        let mesh = &doc.meshes[m];
        let v: usize = mesh.primitives.iter().map(|p| p.positions.len()).sum();
        let t: usize = mesh.primitives.iter().map(|p| p.indices.len() / 3).sum();
        let mats: Vec<String> = mesh
            .primitives
            .iter()
            .map(|p| p.material.map_or("-".into(), |m| doc.materials[m].name.clone()))
            .collect();
        format!(" mesh {v} verts {t} tris [{}]", mats.join(", "))
    });
    println!(
        "{:indent$}{} t={:?} r={:?} s={:?}{}{}",
        "",
        n.name,
        n.translation,
        n.rotation,
        n.scale,
        if n.visible { "" } else { " HIDDEN" },
        mesh.unwrap_or_default(),
        indent = depth * 2
    );
    for &c in &n.children {
        print(doc, c, depth + 1);
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_scene <table.vpx>");
    let t0 = Instant::now();
    let mut vpx = vpin::vpx::read(std::path::Path::new(&path)).expect("read the table");
    let t1 = Instant::now();
    let (doc, _) = player::table_meshes::load(&mut vpx).expect("export");
    let t2 = Instant::now();
    println!(
        "read {:.0} ms, export+parse {:.0} ms: {} nodes, {} meshes, {} materials, {} images, {} MB binary",
        (t1 - t0).as_secs_f64() * 1e3,
        (t2 - t1).as_secs_f64() * 1e3,
        doc.nodes.len(),
        doc.meshes.len(),
        doc.materials.len(),
        doc.images.len(),
        doc.bin.len() / 1_000_000
    );
    for &r in &doc.roots {
        print(&doc, r, 0);
    }
    for (i, m) in doc.materials.iter().enumerate() {
        println!("material {i}: {:?}", m);
    }
    for (i, im) in doc.images.iter().enumerate() {
        println!("image {i}: {} {} {} bytes", im.name, im.mime_type, im.bytes.len());
    }
}
