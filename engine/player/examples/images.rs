//! Lists a table's pictures: each one's name, the file it was imported from, how it's stored
//! (its first bytes; a link to the table's screenshot as VPX resolves it), its size, and whether
//! the player's decoder knows its format — for finding a picture the player can't read:
//! `cargo run --release -p player --example images -- table.vpx [--save <name> <file>]`.
//! `--save` writes that picture's file as the table holds it, for a closer look.

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: images <table.vpx> [--save <name> <file>]");
    let save = match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--save"), Some(name), Some(file)) => Some((name, file)),
        _ => None,
    };
    let mut vpx = vpin::vpx::read(std::path::Path::new(&path)).expect("read the table");
    let linked: Vec<bool> = vpx.images.iter().map(|i| i.is_link()).collect();
    player::pictures::resolve_links(&mut vpx);
    if let Some((name, file)) = save {
        let image = vpx.images.iter().find(|i| i.name.eq_ignore_ascii_case(&name)).expect("no picture of that name");
        let bytes = image.jpeg.as_ref().map(|j| j.data.as_slice()).expect("the picture isn't stored as a file");
        std::fs::write(&file, bytes).expect("write the file");
        println!("{} written to {file} ({} bytes)", image.name, bytes.len());
        return;
    }
    // the room picture the table's materials and mirrors reflect: none, or one it doesn't hold,
    // and VPX uses its own 2014 picture, which ours can't
    let env = vpx.gamedata.env_image.as_deref().unwrap_or("");
    let held = vpx.images.iter().any(|i| i.name.eq_ignore_ascii_case(env));
    println!("environment picture: {}", if env.is_empty() { "none named".to_string() } else if held { env.to_string() } else { format!("{env} (not in the table)") });
    for (index, image) in vpx.images.iter().enumerate() {
        let (stored, bytes) = match (&image.jpeg, &image.bits) {
            (Some(_), _) if linked[index] => ("link", image.jpeg.as_ref().map_or(&[][..], |j| j.data.as_slice())),
            (Some(j), _) => ("file", j.data.as_slice()),
            (None, Some(b)) => ("bitmap", b.lzw_compressed_data.as_slice()),
            (None, None) => ("none", &[][..]),
        };
        // as the player finds it: by its signature, a TGA by its header or name, a Photoshop file
        let format = if matches!(stored, "file" | "link") {
            match player::pictures::reader(bytes, &image.path).and_then(|r| r.format()) {
                Some(f) => format!("{f:?}"),
                None if player::pictures::psd_info(bytes).is_some() => "Psd".to_string(),
                None => "unknown".to_string(),
            }
        } else {
            "-".to_string()
        };
        let head: String = bytes.iter().take(12).map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ");
        println!(
            "#{index:<4} {:<32} {stored:<6} {:>9} bytes  {format:<8} {}x{}  from {:?}  starts {head}",
            image.name,
            bytes.len(),
            image.width,
            image.height,
            image.path
        );
    }
}
