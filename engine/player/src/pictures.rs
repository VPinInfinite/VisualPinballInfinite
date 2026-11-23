//! What a table's picture file is, found as VPX finds it (its FreeImage, `BaseTexture::CreateFromData`):
//! by the file's own signature, else a TGA — which has none — when its header holds together as one
//! (the checks FreeImage makes of a TGA header, from the format's description) or its name says
//! so; and a Photoshop file's merged picture, which FreeImage reads and the image library doesn't
//! (tables carry ball scratches saved so, under a `.png` name). Ours, but for `resolve_links`, from
//! VPX build 5436 (license:GPLv3+: `Texture::CreateFromObjectReader` in src/renderer/Texture.cpp and
//! `PinTable::GetImageLinkBinary` in src/parts/pintable.cpp).

use std::io::Cursor;

/// A table's pictures stored as a link (VPX's `LINK` record: link 1, the only one VPX makes, is the
/// table's screenshot) take the screenshot's file as their own, as VPX gives it them when it reads
/// the table, so they read as any picture does. VPX gives the screenshot away to the first such
/// picture; any other gets nothing, as there. Call once a table is read.
pub fn resolve_links(vpx: &mut vpin::vpx::VPX) {
    let mut screenshot = vpx.info.screenshot.clone();
    for image in &mut vpx.images {
        if image.is_link() && image.jpeg.is_none() && image.bits.is_none() {
            let Some(data) = screenshot.take() else { break };
            image.jpeg = Some(vpin::vpx::pinbinary::PinBinary { name: image.name.clone(), internal_name: None, path: image.path.clone(), data });
        }
    }
}

/// The table's pictures that pictures kept beside it stand in for: each file in `<table> pictures`
/// named as one of the table's pictures (`backdrop2.png` for its picture `backdrop2`, as the app's
/// drop window puts an alternative a table's download brings), read in that picture's place, the
/// table file left as it is. Ours. The names of those replaced.
pub fn replace_from_folder(vpx: &mut vpin::vpx::VPX, table: &std::path::Path) -> Vec<String> {
    let Some(stem) = table.file_stem() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(table.with_file_name(format!("{} pictures", stem.to_string_lossy()))) else { return Vec::new() };
    let files: Vec<std::path::PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    let mut replaced = Vec::new();
    for image in &mut vpx.images {
        let Some(file) = files.iter().find(|f| f.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(&image.name))) else { continue };
        let Ok(data) = std::fs::read(file) else { continue };
        let path = file.to_string_lossy().into_owned();
        // its size, which VPX keeps beside the picture, from the file's own header
        if let Some((w, h)) = reader(&data, &path).and_then(|r| r.into_dimensions().ok()) {
            (image.width, image.height) = (w, h);
        }
        image.jpeg = Some(vpin::vpx::pinbinary::PinBinary { name: image.name.clone(), internal_name: None, path: path.clone(), data });
        image.bits = None;
        image.link = None;
        image.path = path;
        replaced.push(image.name.clone());
    }
    replaced
}

use image::{DynamicImage, ImageReader, RgbaImage};

/// A reader for a picture file, its format chosen as above; None for a format the image library
/// doesn't read (a Photoshop file: `decode_psd`).
pub fn reader<'a>(data: &'a [u8], name: &str) -> Option<ImageReader<Cursor<&'a [u8]>>> {
    let mut reader = ImageReader::new(Cursor::new(data)).with_guessed_format().ok()?;
    if reader.format().is_none() && (looks_like_tga(data) || name.to_ascii_lowercase().ends_with(".tga")) {
        reader.set_format(image::ImageFormat::Tga);
    }
    // the default limit of 512 MB rejects the 8k float bakes of recent tables
    reader.no_limits();
    reader.format().is_some().then_some(reader)
}

/// Whether a TGA file's header holds together: a colour map of none or one, its entries making
/// sense when there's one, a size, and one of the image kinds and depths TGA has (FreeImage's
/// check, `PluginTARGA.cpp`'s `Validate`, which a TGA 2.0 footer passes too).
pub fn looks_like_tga(d: &[u8]) -> bool {
    if d.len() >= 26 && d.ends_with(b"TRUEVISION-XFILE.\0") {
        return true;
    }
    if d.len() < 18 {
        return false;
    }
    let u16_at = |i: usize| u16::from_le_bytes([d[i], d[i + 1]]);
    let (map_type, kind, map_entry_bits, depth) = (d[1], d[2], d[7], d[16]);
    if map_type > 1 || (map_type == 1 && (u16_at(3) >= u16_at(5) || map_entry_bits == 0 || map_entry_bits > 32)) {
        return false;
    }
    u16_at(12) != 0 && u16_at(14) != 0 && matches!(kind, 1 | 2 | 3 | 9 | 10 | 11) && matches!(depth, 8 | 16 | 24 | 32)
}

/// A Photoshop file's header: its width, height, channels, bits a channel and colour mode.
fn psd_header(d: &[u8]) -> Option<(usize, usize, usize, usize, u16)> {
    if d.len() < 26 || &d[..4] != b"8BPS" || d[4..6] != [0, 1] {
        return None;
    }
    let u16_at = |i: usize| u16::from_be_bytes([d[i], d[i + 1]]);
    let u32_at = |i: usize| u32::from_be_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]);
    Some((u32_at(18) as usize, u32_at(14) as usize, usize::from(u16_at(12)), usize::from(u16_at(22)), u16_at(24)))
}

/// A Photoshop file's size and whether its picture has an alpha channel (a fourth channel to an
/// RGB one, a second to a grayscale one), from its header alone; None when it isn't one this reads.
pub fn psd_info(d: &[u8]) -> Option<(u32, u32, bool)> {
    let (width, height, channels, depth, mode) = psd_header(d)?;
    let alpha = match (mode, depth) {
        (3, 8 | 16) if channels >= 3 => channels >= 4,
        (1, 8 | 16) => channels >= 2,
        _ => return None,
    };
    Some((width as u32, height as u32, alpha))
}

/// A Photoshop file's merged picture (its image data section, as FreeImage reads it): RGB or
/// grayscale at 8 or 16 bits a channel, raw or PackBits-compressed, a fourth (or for grayscale a
/// second) channel its alpha. None for anything else, or data that doesn't hold together.
pub fn decode_psd(d: &[u8]) -> Option<DynamicImage> {
    let (width, height, channels, depth, mode) = psd_header(d)?;
    let (_, _, alpha) = psd_info(d)?;
    let colours = if mode == 3 { 3 } else { 1 };
    let wanted = colours + usize::from(alpha);
    let u32_at = |i: usize| -> Option<usize> { Some(u32::from_be_bytes(d.get(i..i + 4)?.try_into().ok()?) as usize) };
    // past the colour mode data, the image resources and the layers to the merged picture
    let mut at: usize = 26;
    for _ in 0..3 {
        at = at.checked_add(4)?.checked_add(u32_at(at)?)?;
    }
    let compression = u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?);
    at += 2;
    let row = width.checked_mul(depth / 8)?;
    let plane = row.checked_mul(height)?;
    let mut planes: Vec<Vec<u8>> = Vec::with_capacity(wanted);
    match compression {
        0 => {
            for c in 0..wanted {
                let start = at.checked_add(c.checked_mul(plane)?)?;
                planes.push(d.get(start..start.checked_add(plane)?)?.to_vec());
            }
        }
        1 => {
            // each row's length, for every channel, then the rows
            let counts = at;
            let mut data = at.checked_add(channels.checked_mul(height)?.checked_mul(2)?)?;
            for c in 0..wanted {
                let mut out = Vec::with_capacity(plane);
                for r in 0..height {
                    let i = counts + (c * height + r) * 2;
                    let len = usize::from(u16::from_be_bytes(d.get(i..i + 2)?.try_into().ok()?));
                    unpack_bits(d.get(data..data.checked_add(len)?)?, &mut out, (r + 1) * row)?;
                    data += len;
                }
                planes.push(out);
            }
        }
        _ => return None,
    }
    // big-endian 16-bit channels: their high bytes
    let step = depth / 8;
    let texel = |p: &[u8], i: usize| p[i * step];
    let mut rgba = RgbaImage::new(width as u32, height as u32);
    for (i, out) in rgba.pixels_mut().enumerate() {
        let c = |k: usize| texel(&planes[k], i);
        let a = if alpha { c(colours) } else { 255 };
        out.0 = if colours == 3 { [c(0), c(1), c(2), a] } else { [c(0), c(0), c(0), a] };
    }
    Some(DynamicImage::ImageRgba8(rgba))
}

/// One PackBits-compressed row added to `out`, which then holds `until` bytes.
fn unpack_bits(src: &[u8], out: &mut Vec<u8>, until: usize) -> Option<()> {
    let mut i = 0;
    while out.len() < until && i < src.len() {
        let header = src[i] as i8;
        i += 1;
        match header {
            0..=127 => {
                let n = header as usize + 1;
                out.extend_from_slice(src.get(i..i + n)?);
                i += n;
            }
            -127..=-1 => {
                let byte = *src.get(i)?;
                i += 1;
                out.extend(std::iter::repeat_n(byte, (1 - i32::from(header)) as usize));
            }
            -128 => {}
        }
    }
    (out.len() >= until).then(|| out.truncate(until))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture kept in `<table> pictures` read in the table's own picture's place, by name, case
    /// aside; the others as they were.
    #[test]
    fn a_picture_beside_the_table_stands_in_for_its_own() {
        let d = std::env::temp_dir().join(format!("vpi-table-pictures-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("Terrifier pictures")).unwrap();
        let mut png = Vec::new();
        RgbaImage::new(3, 2).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        std::fs::write(d.join("Terrifier pictures").join("Backdrop2.png"), &png).unwrap();
        let mut vpx = vpin::vpx::VPX::default();
        for name in ["backdrop2", "playfield"] {
            vpx.images.push(vpin::vpx::image::ImageData { name: name.into(), width: 1920, height: 1080, ..Default::default() });
        }
        assert_eq!(replace_from_folder(&mut vpx, &d.join("Terrifier.vpx")), ["backdrop2"]);
        let b = &vpx.images[0];
        assert_eq!((b.width, b.height, b.jpeg.as_ref().map(|j| j.data.len())), (3, 2, Some(png.len())));
        assert!(vpx.images[1].jpeg.is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A 2 × 2 RGBA Photoshop file, its merged picture PackBits-compressed: its texels read back.
    #[test]
    fn a_photoshop_files_merged_picture_reads() {
        let mut f = Vec::new();
        f.extend_from_slice(b"8BPS");
        f.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
        f.extend_from_slice(&4u16.to_be_bytes()); // channels
        f.extend_from_slice(&2u32.to_be_bytes()); // height
        f.extend_from_slice(&2u32.to_be_bytes()); // width
        f.extend_from_slice(&8u16.to_be_bytes()); // depth
        f.extend_from_slice(&3u16.to_be_bytes()); // RGB
        for section in [&[][..], &[1, 2, 3][..], &[][..]] {
            f.extend_from_slice(&(section.len() as u32).to_be_bytes());
            f.extend_from_slice(section);
        }
        f.extend_from_slice(&1u16.to_be_bytes()); // PackBits
        // red: a run of 2 per row; green: literal rows; blue: a run; alpha: literal rows
        let rows: [&[u8]; 8] = [&[0xFF, 200], &[0xFF, 100], &[1, 10, 20], &[1, 30, 40], &[0xFF, 7], &[0xFF, 7], &[1, 255, 0], &[1, 128, 64]];
        for r in rows {
            f.extend_from_slice(&(r.len() as u16).to_be_bytes());
        }
        for r in rows {
            f.extend_from_slice(r);
        }
        assert_eq!(psd_info(&f), Some((2, 2, true)));
        let picture = decode_psd(&f).expect("decoded").to_rgba8();
        assert_eq!(picture.get_pixel(0, 0).0, [200, 10, 7, 255]);
        assert_eq!(picture.get_pixel(1, 0).0, [200, 20, 7, 0]);
        assert_eq!(picture.get_pixel(0, 1).0, [100, 30, 7, 128]);
        assert_eq!(picture.get_pixel(1, 1).0, [100, 40, 7, 64]);
    }

    /// An uncompressed TGA's header passes FreeImage's check; text, a PNG's start and a header of
    /// no size don't.
    #[test]
    fn a_tga_header_is_told_apart() {
        let mut tga = vec![0u8, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        tga.extend_from_slice(&1080u16.to_le_bytes());
        tga.extend_from_slice(&1920u16.to_le_bytes());
        tga.extend_from_slice(&[32, 8]);
        assert!(looks_like_tga(&tga));
        assert!(!looks_like_tga(b"Hello, this is not a picture at all"));
        assert!(!looks_like_tga(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x10"));
        let mut empty = tga.clone();
        empty[12] = 0;
        empty[13] = 0;
        assert!(!looks_like_tga(&empty));
    }
}
