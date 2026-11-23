//! The table's meshes, materials and images, from `vpin`'s glTF export
//!, with the table's pictures decoded beside it.
//!
//! The export packs a copy of every picture it uses into its model file —
//! raw bitmaps turned into PNG on the way — and, to choose a material's
//! blending, decodes its picture whenever the table file doesn't say
//! whether it's opaque, once for every material that uses it: most of its
//! time on many tables (Black Knight's 6.5 s). So it's handed stand-ins: each
//! picture's name and settings, a one-texel picture, and the answer to its
//! question — has the picture see-through texels? — found as `vpin` finds it.
//! It then makes the same meshes and materials, while the pictures
//! themselves are decoded from the table file on the other threads, each
//! once, the way the player decoded them from the export's copies.

use std::collections::HashSet;
use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageDecoder, ImageReader};
use vpin::filesystem::MemoryFileSystem;
use vpin::vpx::VPX;
use vpin::vpx::export::gltf_export::{GltfExportOptions, export_gltf};
use vpin::vpx::gameitem::GameItemEnum;
use vpin::vpx::image::ImageData;
use vpin::vpx::pinbinary::PinBinary;
use vpin::vpx::units::ExportUnits;

use crate::gltf::{self, Document};
use crate::scene::{Image, in_parallel};

/// Pictures decoded ahead, by table image (`vpx.images`): not tried yet
/// (None), or tried, decoded or not.
pub struct Pictures {
    /// For each of the export's images, the table image it packed.
    pub sources: Vec<Option<usize>>,
    /// As a material's picture, from the bytes the export packs.
    pub material: Vec<Option<Option<Image>>>,
    /// As VPX decodes the pictures the script may swap in (`vpin`'s own decoding).
    pub script: Vec<Option<Option<Image>>>,
}

/// What `image_has_transparency` would answer for a picture, when known
/// without decoding it all or when decoded, and the picture, when decoded.
#[derive(Default)]
pub(crate) struct Answer {
    pub(crate) see_through: Option<bool>,
    pub(crate) picture: Option<Option<Image>>,
    /// Whether the export would pack it (`get_image_bytes`).
    pub(crate) packed: bool,
}

/// `image::load_from_memory`'s limit, which `vpin`'s transparency check
/// decodes under: a bigger picture fails there, and counts as opaque.
const DEFAULT_MAX_ALLOC: u64 = 512 * 1024 * 1024;

/// Exports the table in VPX units, hidden parts included (scripts can show
/// them), and reads the export back; meanwhile the pictures it uses, and
/// those the script may swap in, are decoded.
pub fn load(vpx: &mut VPX) -> Result<(Document, Pictures), String> {
    let n = vpx.images.len();
    let script_images = crate::scene::script_images(vpx);
    let names: Vec<String> = vpx.images.iter().map(|i| i.name.to_lowercase()).collect();
    let wanted = export_picture_names(vpx);
    // where several pictures share a name, the export takes the last
    let last = |name: &str| names.iter().rposition(|n| n == name);
    let candidate: Vec<bool> = (0..n).map(|i| wanted.contains(&names[i]) && last(&names[i]) == Some(i)).collect();
    let jobs: Vec<usize> = (0..n).collect();
    let mut answers: Vec<Answer> = in_parallel(&jobs, |&i| answer(&vpx.images[i], candidate[i]));

    let real = std::mem::take(&mut vpx.images);
    // a one-texel picture, for every stand-in
    let mut texel = Vec::new();
    image::RgbaImage::new(1, 1).write_to(&mut Cursor::new(&mut texel), image::ImageFormat::Png).map_err(|e| format!("make a stand-in picture: {e}"))?;
    let mut pictures = Pictures { sources: Vec::new(), material: (0..n).map(|_| None).collect(), script: (0..n).map(|_| None).collect() };
    let mut doc = None;
    for attempt in 0..2 {
        vpx.images = real.iter().zip(&answers).map(|(image, a)| stand_in(image, a, &texel)).collect();
        // the export on a thread of its own; the rest of its pictures, and the script's, on the others
        let ahead: Vec<(usize, bool)> = (0..n)
            .filter(|&i| candidate[i] && answers[i].packed && answers[i].picture.is_none() && pictures.material[i].is_none())
            .map(|i| (i, true))
            .chain(script_images.iter().filter(|&&i| i < n && !candidate[i] && pictures.script[i].is_none()).map(|&i| (i, false)))
            .collect();
        let vpx_ref: &VPX = vpx;
        let (exported, decoded) = std::thread::scope(|scope| {
            let export = scope.spawn(|| export(vpx_ref));
            let decoded = in_parallel(&ahead, |&(i, material)| if material { decode_material(&real[i]) } else { decode_script(&real[i]) });
            (export.join().unwrap_or_else(|_| Err("the export failed".into())), decoded)
        });
        for (&(i, material), picture) in ahead.iter().zip(decoded) {
            if material { pictures.material[i] = Some(picture) } else { pictures.script[i] = Some(picture) }
        }
        let d = match exported {
            Ok(d) => d,
            Err(e) => {
                vpx.images = real;
                return Err(e);
            }
        };
        // a picture the export used whose answer was a guess: answer it, and export again (once)
        let guessed: Vec<usize> =
            d.images.iter().filter_map(|image| last(&image.name.to_lowercase())).filter(|&i| answers[i].see_through.is_none() && answers[i].packed).collect();
        if guessed.is_empty() || attempt == 1 {
            doc = Some(d);
            break;
        }
        eprintln!("the export used {} pictures no part names: answered, and exported again", guessed.len());
        let exact = in_parallel(&guessed, |&i| answer(&real[i], true));
        for (&i, a) in guessed.iter().zip(exact) {
            answers[i] = a;
        }
    }
    vpx.images = real;
    let doc = doc.ok_or("the export failed")?;
    // the export packs the playfield's picture first, the first of its name (`find_image_by_name`),
    // for the playfield's own material; then each mesh's, the last of its name
    let playfield = Some(vpx.gamedata.image.to_lowercase()).filter(|n| !n.is_empty()).and_then(|n| names.iter().position(|m| *m == n)).filter(|&t| answers[t].packed);
    pictures.sources =
        doc.images.iter().enumerate().map(|(k, image)| if k == 0 && playfield.is_some() { playfield } else { last(&image.name.to_lowercase()) }).collect();
    for (i, a) in answers.into_iter().enumerate() {
        if let Some(picture) = a.picture {
            pictures.material[i] = Some(picture);
        }
    }
    Ok((doc, pictures))
}

/// The export itself.
fn export(vpx: &VPX) -> Result<Document, String> {
    let fs = MemoryFileSystem::new();
    let mut options = GltfExportOptions::glb().with_export_invisible_items(true);
    options.units = ExportUnits::Vpu;
    export_gltf(vpx, Path::new("table.glb"), &fs, &options).map_err(|e| format!("export the table's meshes: {e}"))?;
    let glb = fs.get_file("table.glb").ok_or("the export wrote nothing")?;
    gltf::parse(glb)
}

/// The pictures the export can give a mesh, by name in lower case (its
/// `texture_name`s, `gltf_export.rs` of `vpin` 0.38.4): the playfield's,
/// the ball's front, and the parts'.
pub(crate) fn export_picture_names(vpx: &VPX) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut add = |name: &str| {
        if !name.is_empty() {
            out.insert(name.to_lowercase());
        }
    };
    add(&vpx.gamedata.image);
    add(&vpx.gamedata.ball_image_front);
    for item in &vpx.gameitems {
        match item {
            GameItemEnum::Primitive(p) => add(&p.image),
            GameItemEnum::Wall(w) => {
                add(&w.image);
                add(&w.side_image);
            }
            GameItemEnum::Ramp(r) => add(&r.image),
            GameItemEnum::Flasher(f) => add(&f.image_a),
            GameItemEnum::Flipper(f) => add(f.image.as_deref().unwrap_or_default()),
            GameItemEnum::Spinner(s) => add(&s.image),
            GameItemEnum::HitTarget(t) => add(&t.image),
            GameItemEnum::Light(l) if !l.is_bulb_light => add(&l.image),
            GameItemEnum::Plunger(p) => add(&p.image),
            GameItemEnum::Decal(d) => add(&d.image),
            GameItemEnum::Ball(b) => add(&b.image_decal),
            _ => {}
        }
    }
    out
}

/// A picture's answer to the export (`vpin`'s `image_has_transparency`):
/// the table file's own flag when it has one; else none for a picture
/// without an alpha channel; else, for a `wanted` picture, its texels
/// decoded and looked at — and the picture kept.
pub(crate) fn answer(image: &ImageData, wanted: bool) -> Answer {
    if let Some(jpeg) = &image.jpeg {
        if let Some(opaque) = image.is_opaque {
            return Answer { see_through: Some(!opaque), picture: None, packed: true };
        }
        // the header: a format nobody recognises fails vpin's decoding, and counts as opaque, as
        // does a picture without an alpha channel (a TGA and a Photoshop file found as VPX finds them)
        let header = crate::pictures::reader(&jpeg.data, &image.path)
            .and_then(|r| r.into_decoder().ok().map(|d| (d.color_type().has_alpha(), d.total_bytes())))
            .or_else(|| crate::pictures::psd_info(&jpeg.data).map(|(w, h, alpha)| (alpha, u64::from(w) * u64::from(h) * 4)));
        let Some((true, total)) = header else {
            return Answer { see_through: Some(false), picture: None, packed: true };
        };
        if !wanted {
            return Answer { see_through: None, picture: None, packed: true };
        }
        // a picture of light past white is opaque to VPX, its alpha dropped: decoded when it's uploaded
        if jpeg.data.starts_with(&EXR_MAGIC) {
            return Answer { see_through: Some(false), picture: None, packed: true };
        }
        return match decode_material_texels(image) {
            Some(Ok(decoded)) => {
                // VPX keeps a picture of light past white opaque (its alpha dropped)
                let high_range = matches!(decoded, DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_));
                let see_through = !high_range && total <= DEFAULT_MAX_ALLOC && see_through_texels(&decoded);
                Answer { see_through: Some(see_through), picture: Some(Some(Image::from_decoded(decoded))), packed: true }
            }
            Some(Err(e)) => {
                eprintln!("image {}: {e}", image.name);
                Answer { see_through: Some(false), picture: Some(None), packed: true }
            }
            None => Answer { see_through: Some(false), picture: Some(None), packed: true },
        };
    }
    if image.bits.is_some() {
        // a raw bitmap: the export packs it as PNG when it decodes, which keeps an alpha channel
        // only when a texel isn't opaque (vpin's vpx_image_to_dynamic_image)
        return match image.decode() {
            Ok(decoded) => {
                let see_through = image.is_opaque.map_or(matches!(decoded, DynamicImage::ImageRgba8(_)), |opaque| !opaque);
                Answer { see_through: Some(see_through), picture: wanted.then(|| Some(Image::new(decoded.into_rgba8()))), packed: true }
            }
            Err(_) => Answer { see_through: Some(false), picture: None, packed: false },
        };
    }
    // a link to the table's screenshot, or nothing: the export packs nothing
    Answer { see_through: Some(false), picture: None, packed: false }
}

/// Whether a picture has texels that aren't fully opaque, as `vpin` looks
/// (`has_transparent_pixels`).
fn see_through_texels(image: &DynamicImage) -> bool {
    match image {
        DynamicImage::ImageRgba8(i) => i.pixels().any(|p| p[3] != 255),
        DynamicImage::ImageRgba16(i) => i.pixels().any(|p| p[3] != 65535),
        DynamicImage::ImageRgba32F(i) => i.pixels().any(|p| p[3] < 1.0),
        DynamicImage::ImageLumaA8(i) => i.pixels().any(|p| p[1] != 255),
        DynamicImage::ImageLumaA16(i) => i.pixels().any(|p| p[1] != 65535),
        _ => false,
    }
}

/// A picture as the export sees it: its name and settings, a one-texel
/// picture in place of its own when the export would pack it, and its
/// answer (a guess of opaque when not known: the export is run again if it
/// uses one).
fn stand_in(image: &ImageData, answer: &Answer, texel: &[u8]) -> ImageData {
    ImageData {
        name: image.name.clone(),
        internal_name: image.internal_name.clone(),
        path: image.path.clone(),
        width: image.width,
        height: image.height,
        link: image.link,
        alpha_test_value: image.alpha_test_value,
        is_opaque: Some(!answer.see_through.unwrap_or(false)),
        is_signed: image.is_signed,
        jpeg: answer.packed.then(|| PinBinary { name: image.name.clone(), internal_name: None, path: image.path.clone(), data: texel.to_vec() }),
        bits: None,
        md5_hash: image.md5_hash,
    }
}

/// A picture as a material's: the bytes the export would pack, decoded as
/// the player decoded the export's copy (the format from its bytes, no size
/// limit); a raw bitmap as its PNG would decode.
pub(crate) fn decode_material(image: &ImageData) -> Option<Image> {
    if let Some(decoded) = decode_exr(image) {
        return decoded;
    }
    decode_material_texels(image)?.map(Image::from_decoded).map_err(|e| eprintln!("image {}: {e}", image.name)).ok()
}

/// The magic number an OpenEXR file starts with.
const EXR_MAGIC: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];

/// An OpenEXR picture of light past white read straight into the half floats
/// it's kept as, the values `Image::from_decoded` makes of the image
/// library's 32-bit ones (red, green and blue held to ±65504, alpha 1;
/// texels outside the file's data window black), without the library's two
/// 32-bit copies: a quarter of the memory while decoding (an 8192 × 8192
/// light map 0.5 GB, not 2.7). None when the picture isn't OpenEXR.
fn decode_exr(image: &ImageData) -> Option<Option<Image>> {
    use exr::prelude::{ReadChannels, ReadLayers, Vec2, read};
    let bytes = image.jpeg.as_ref().map(|j| j.data.as_slice()).filter(|b| b.starts_with(&EXR_MAGIC))?;
    let half = |v: f32| half::f16::from_f32(v.clamp(-65504.0, 65504.0));
    let read = read()
        .no_deep_data()
        .largest_resolution_level()
        .rgba_channels(
            |size: Vec2<usize>, _| (size, vec![half::f16::ZERO; size.area() * 4]),
            move |(size, texels): &mut (Vec2<usize>, Vec<half::f16>), at: Vec2<usize>, (r, g, b, _): (f32, f32, f32, f32)| {
                let i = at.flat_index_for_size(*size) * 4;
                texels[i..i + 4].copy_from_slice(&[half(r), half(g), half(b), half::f16::ONE]);
            },
        )
        .first_valid_layer()
        .all_attributes()
        .from_buffered(Cursor::new(bytes));
    let exr = match read {
        Ok(exr) => exr,
        Err(e) => {
            eprintln!("image {}: {e}", image.name);
            return Some(None);
        }
    };
    let display = exr.attributes.display_window;
    let layer = exr.layer_data;
    let (data_size, texels) = layer.channel_data.pixels;
    let offset = layer.attributes.layer_position - display.position;
    let (width, height) = (display.size.width(), display.size.height());
    let texels = if offset == Vec2(0, 0) && data_size == display.size {
        texels
    } else {
        // the data window placed in the display window, the rest black and opaque
        let mut out = [half::f16::ZERO, half::f16::ZERO, half::f16::ZERO, half::f16::ONE].repeat(width * height);
        for y in 0..data_size.height() {
            for x in 0..data_size.width() {
                let (dx, dy) = (x as i64 + i64::from(offset.x()), y as i64 + i64::from(offset.y()));
                if dx >= 0 && dy >= 0 && (dx as usize) < width && (dy as usize) < height {
                    let (from, to) = ((y * data_size.width() + x) * 4, (dy as usize * width + dx as usize) * 4);
                    out[to..to + 4].copy_from_slice(&texels[from..from + 4]);
                }
            }
        }
        out
    };
    Some(Some(Image { width: width as u32, height: height as u32, rgba: Vec::new(), has_alpha: false, hdr: Some(texels), pending: None }))
}

/// [`decode_material`]'s texels, before they're made RGBA; None when the
/// export packs nothing.
fn decode_material_texels(image: &ImageData) -> Option<image::ImageResult<DynamicImage>> {
    if let Some(jpeg) = &image.jpeg {
        // its format found as VPX finds it: a TGA by its header, a Photoshop file's merged picture
        if let Some(r) = crate::pictures::reader(&jpeg.data, &image.path) {
            return Some(r.decode());
        }
        if let Some(psd) = crate::pictures::decode_psd(&jpeg.data) {
            return Some(Ok(psd));
        }
        return Some(ImageReader::new(Cursor::new(jpeg.data.as_slice())).with_guessed_format().map_err(image::ImageError::IoError).and_then(|mut r| {
            r.no_limits();
            r.decode()
        }));
    }
    image.bits.is_some().then(|| image.decode().map_err(image::ImageError::IoError))
}

/// About how much memory decoding a picture takes at once: its texels as the
/// decoder makes them and as they're kept (8 bytes a texel for 8-bit
/// pictures; about 10 for OpenEXR pictures of light past white, read
/// straight into halves; 24 for Radiance ones, decoded as 32-bit floats).
/// The size comes from the picture's own header, as many tables don't
/// record it; a raw bitmap's from the table.
pub(crate) fn decode_cost(image: &ImageData) -> usize {
    if let Some(jpeg) = &image.jpeg
        && let Some((w, h, _)) = crate::pictures::psd_info(&jpeg.data)
    {
        return w as usize * h as usize * 8;
    }
    if let Some(jpeg) = &image.jpeg
        && let Some(reader) = crate::pictures::reader(&jpeg.data, &image.path)
    {
        // an OpenEXR picture is read straight into the half floats it's kept as (`decode_exr`)
        let per_texel = match reader.format() {
            Some(image::ImageFormat::OpenExr) => 10,
            Some(image::ImageFormat::Hdr) => 24,
            _ => 8,
        };
        if let Ok((w, h)) = reader.into_dimensions() {
            return w as usize * h as usize * per_texel;
        }
    }
    image.width as usize * image.height as usize * 8
}

/// A picture as the script swaps it in (`vpin`'s decoding; a TGA and a Photoshop file found as
/// VPX finds them).
pub(crate) fn decode_script(image: &ImageData) -> Option<Image> {
    if let Some(decoded) = decode_exr(image) {
        return decoded;
    }
    if let Some(jpeg) = &image.jpeg
        && (crate::pictures::looks_like_tga(&jpeg.data) || crate::pictures::psd_info(&jpeg.data).is_some())
        && let Some(Ok(decoded)) = decode_material_texels(image)
    {
        return Some(Image::from_decoded(decoded));
    }
    match image.decode() {
        Ok(decoded) => Some(Image::from_decoded(decoded)),
        Err(e) => {
            eprintln!("image {}: {e}", image.name);
            None
        }
    }
}
