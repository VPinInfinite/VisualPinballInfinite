// Follows Visual Pinball X (license:GPLv3+), build 6625 (commit 2ad1916b2): its B2S add-on's
// displays over a backglass's pictures — plugins/b2s/DMDOverlay/DMDOverlay.cpp (SearchDmdSubFrame),
// plugins/b2s/B2SServer.cpp (FindB2SFile, WriteDisplays) and plugins/b2s/B2SDataModel.cpp (the
// `<Displays>` element, and the pictures it's searched in).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The score display over a backglass file's picture, as VPX's newest builds keep it — in the file
//! itself, from build 6267 on (its 268276e6b): a `<DMD>` in the file's own `<Displays>`, which VPX's
//! B2S add-on draws the table's DMD into. VPX places one in its own menu, its "Detect position"
//! finding the place in the picture when asked. Its builds before kept no place: their add-on found
//! one in the picture by itself as the DMD first showed, where `[Plugin.B2S]` `BackglassDMDOverlay`
//! and `BackglassDMDAutoPos` said so (`ScoreViewDMD…` for the score panel's).
//!
//! A place is found here as "Detect position" finds it, in the picture it searches, for the DMD's
//! shape ([`find`], [`dmd_shape`]), and written into the file as VPX's menu writes one
//! ([`with_places`], [`place_in`]), the file as it was kept first. A file holding a `<Displays>`
//! already — its author's, VPX's menu's, or one placed before — is left as it is
//! ([`holds_displays`]): a place moved or taken out in VPX's menu stays so.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Which of a backglass file's pictures a place is over (VPX's `Target`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    /// The backglass's own picture, in the backglass window.
    Backglass,
    /// The file's score panel (its `DMDImage`), in the score window.
    ScoreView,
}

impl Target {
    /// As VPX writes it.
    pub fn name(self) -> &'static str {
        match self {
            Target::Backglass => "Backglass",
            Target::ScoreView => "ScoreView",
        }
    }

    /// The pictures VPX searches for it, the first one the file holds: the backglass's own, else
    /// its unlit one, else its lit one; the score panel's.
    fn pictures(self) -> &'static [&'static str] {
        match self {
            Target::Backglass => &["BackglassImage", "BackglassOffImage", "BackglassOnImage"],
            Target::ScoreView => &["DMDImage"],
        }
    }
}

/// A place for the score display over one of a file's pictures: left, top, width and height in
/// the picture's own pixels from its top left (its speaker panel counted in), as VPX keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub target: Target,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// The backglass file VPX plays a table with (`FindB2SFile`): the one named as the table, beside
/// it, else the one named as its folder.
pub fn playing_file(table: &Path) -> Option<PathBuf> {
    let dir = table.parent()?;
    let named = |name: &str| Some(dir.join(format!("{name}.directb2s"))).filter(|p| !name.is_empty() && p.is_file());
    named(&table.file_stem()?.to_string_lossy()).or_else(|| named(&dir.file_name()?.to_string_lossy()))
}

/// The DMD's shape VPX searches for, its width over its height: the display's own (the source's
/// width over height, as "Detect position" takes it). 128 × 32 dots for nearly every machine
/// PinMAME plays and the DMDs scripts draw; Data East's first DMD machines' 128 × 16 and its and
/// Sega's last 192 × 64, by PinMAME's drivers (`degames.c`, `bowlgames.c`: `GEN_DEDMD16`,
/// `GEN_DEDMD64`), by the sets' names.
pub fn dmd_shape(rom: Option<&str>) -> f32 {
    // Checkpoint, Teenage Mutant Ninja Turtles, Batman, Star Trek 25th Anniversary
    const LOW: [&str; 4] = ["ckpt_", "tmnt_", "btmn_", "trek_"];
    // Maverick, Frankenstein, Baywatch, Batman Forever, Cut the Cheese
    const TALL: [&str; 5] = ["mav_", "frankst", "baywatch", "batmanf", "ctch"];
    let Some(rom) = rom.map(str::to_ascii_lowercase) else { return 4.0 };
    if LOW.iter().any(|p| rom.starts_with(p)) {
        8.0
    } else if TALL.iter().any(|p| rom.starts_with(p)) {
        3.0
    } else {
        4.0
    }
}

/// The height of the backglass picture's speaker panel, at its foot (`GrillHeight`, as VPX reads
/// it): 0 where it has none.
pub fn grill_height(text: &[u8]) -> i32 {
    tag(text, b"GrillHeight", 0)
        .and_then(|at| attribute(&text[at..], b"Value"))
        .and_then(|v| std::str::from_utf8(v).ok()?.trim().parse::<i32>().ok())
        .unwrap_or(0)
        .max(0)
}

/// Whether a backglass file shows scores of its own: score displays of lights or reels
/// (`<Scores>`' `<Score>`s), over the backglass's picture or its score panel's — made for a machine
/// that shows its scores so, not on a DMD.
pub fn shows_scores(text: &[u8]) -> bool {
    let Some(at) = tag(text, b"Scores", 0) else { return false };
    let end = find_bytes(&text[at..], b"</Scores").map_or(text.len(), |e| at + e);
    tag(&text[..end], b"Score", at + 1).is_some()
}

/// Whether a backglass file's text holds a `<Displays>`: places put there by its author, VPX's
/// menu or before — or none, left by VPX's menu once its places were taken out.
pub fn holds_displays(text: &[u8]) -> bool {
    tag(text, b"Displays", 0).is_some()
}

/// The places VPX's file holds (`<Displays>`' `<DMD>` children), as VPX reads them.
pub fn places_in(text: &[u8]) -> Vec<Place> {
    let Some(at) = tag(text, b"Displays", 0) else { return Vec::new() };
    let end = find_bytes(&text[at..], b"</Displays").map_or(text.len(), |e| at + e);
    let mut places = Vec::new();
    let mut from = at;
    while let Some(dmd) = tag(&text[..end], b"DMD", from) {
        let number = |name: &[u8]| attribute(&text[dmd..end], name).and_then(|v| std::str::from_utf8(v).ok()?.trim().parse::<i32>().ok()).unwrap_or(0);
        let target = match attribute(&text[dmd..end], b"Target") {
            Some(t) if t.eq_ignore_ascii_case(b"scoreview") => Target::ScoreView,
            _ => Target::Backglass,
        };
        places.push(Place { target, x: number(b"X"), y: number(b"Y"), w: number(b"W"), h: number(b"H") });
        from = dmd + 1;
    }
    places
}

/// Where VPX's "Detect position" places the score display over one of a file's pictures, for a DMD
/// `shape` times wider than high ([`dmd_shape`]): None where the file holds no such picture, VPX
/// couldn't search it, or finds no place in it.
pub fn find_place(text: &[u8], target: Target, shape: f32) -> Option<Place> {
    let bytes = target.pictures().iter().find_map(|name| picture(text, name))?;
    let (rgb, width, height) = pixels(&bytes)?;
    let [x, y, w, h] = find(&rgb, width, height, shape)?;
    Some(Place { target, x, y, w, h })
}

/// The brightness limit under which a place is taken for a window drawn black in a picture with
/// no speaker panel: the search's two darkest.
const BLACK: f32 = 4.0;

/// Where the score display fits a backglass picture's own window for it, by VPX's search
/// ([`find_place`]): the place it finds in the speaker panel at the picture's foot, or in a window
/// drawn black anywhere; where it finds one in darker art above a speaker panel, the panel
/// searched alone (a window between its speakers). None where none fits: a backglass picture made
/// for a cabinet whose score display shows on a screen of its own, whose darkest art is no place
/// for one. The score panel's own picture as VPX's search finds it.
pub fn fitting_place(text: &[u8], target: Target, shape: f32) -> Option<Place> {
    if target == Target::ScoreView {
        return find_place(text, target, shape);
    }
    let (rgb, width, height) = searched_picture(text, target)?;
    let grill = grill_height(text).min(i32::try_from(height).ok()?);
    let panel_top = i32::try_from(height).ok()? - grill;
    let at = |[x, y, w, h]: [i32; 4]| Place { target, x, y, w, h };
    let found = search(&rgb, width, height, shape);
    if let Some((p, _)) = found.filter(|(p, limit)| *limit <= BLACK || (grill > 0 && p[1] >= panel_top)) {
        return Some(at(p));
    }
    if grill == 0 {
        return None;
    }
    let row = width as usize * 3;
    let panel = &rgb[panel_top as usize * row..];
    let (p, _) = search(panel, width, grill as u32, shape)?;
    Some(at([p[0], p[1] + panel_top, p[2], p[3]]))
}

/// The picture a place over `target` is searched in, as [`find`] reads it, with its width and
/// height: for looking at what was found.
pub fn searched_picture(text: &[u8], target: Target) -> Option<(Vec<u8>, u32, u32)> {
    pixels(&target.pictures().iter().find_map(|name| picture(text, name))?)
}

/// Where VPX's "Detect position" places a DMD `shape` times wider than high in a picture of
/// `width` × `height` pixels (`rgb`: red, green and blue, rows from the top), as `SearchDmdSubFrame`
/// finds it: left, top, width and height; None where it finds none.
///
/// The largest dark rectangle in the picture, cut to the DMD's shape about its middle, at least a
/// third of the picture across, favouring one near the middle across; how dark is dark searched
/// for in seven halvings of a brightness limit, each search kept to where the one before found any.
pub fn find(rgb: &[u8], width: u32, height: u32, shape: f32) -> Option<[i32; 4]> {
    search(rgb, width, height, shape).map(|(place, _)| place)
}

/// [`find`]'s place, with the brightness limit it was last found under: every pixel of the
/// rectangle it was cut from darker than that.
pub fn search(rgb: &[u8], width: u32, height: u32, shape: f32) -> Option<([i32; 4], f32)> {
    let (tex_w, tex_h) = (i32::try_from(width).ok()?, i32::try_from(height).ok()?);
    if tex_w <= 0 || tex_h <= 0 || rgb.len() < width as usize * height as usize * 3 || shape.is_nan() || shape <= 0.0 {
        return None;
    }
    // large rectangles, favouring ones about the middle across
    let texels = tex_w as f32;
    let heuristic = |f: &[i32; 4]| f[2] as f32 / texels - (0.5 - (f[0] + f[2] / 2) as f32 / texels).abs();
    let padding = (2 * tex_w) / 1920;
    let (mut lum_min, mut lum_max) = (0.0f32, 256.0f32);
    let mut area = [0, 0, tex_w, tex_h];
    let mut best = [0; 4];
    for _ in 0..7 {
        let limit = (lum_min + lum_max) * 0.5;
        // the largest dark rectangle: each column's run of dark pixels down to the row, then every
        // rectangle those runs make, through a stack of columns
        let mut found = [0; 4];
        let mut next_area = [0; 4];
        let mut stack: Vec<i32> = Vec::new();
        let mut heights = vec![0i32; tex_w as usize];
        let end = area[0] + area[2];
        for y in area[1]..area[1] + area[3] {
            let row = (y * tex_w) as usize;
            for x in area[0]..end {
                let p = (row + x as usize) * 3;
                let lum = 0.299 * f32::from(rgb[p]) + 0.587 * f32::from(rgb[p + 1]) + 0.114 * f32::from(rgb[p + 2]);
                let column = &mut heights[x as usize];
                *column = if lum < limit { *column + 1 } else { 0 };
            }
            for x in area[0]..=end {
                while let Some(&top) = stack.last() {
                    if x != end && heights[top as usize] <= heights[x as usize] {
                        break;
                    }
                    let h = heights[top as usize];
                    stack.pop();
                    let w = stack.last().map_or(x - area[0], |&left| x - left - 1);
                    if w > 6 * padding && h > 3 * padding {
                        let unpadded = [stack.last().map_or(area[0], |&left| left + 1), y - h + 1, w, h];
                        // the next search covers it, chosen or not
                        next_area = if next_area[2] == 0 { unpadded } else { union(next_area, unpadded) };
                        // padded, and cut to the DMD's shape about its middle
                        let mut f = [unpadded[0] + padding, unpadded[1] + padding, unpadded[2] - 2 * padding, unpadded[3] - 2 * padding];
                        if f[2] as f32 / f[3] as f32 > shape {
                            let cut = (f[3] as f32 * shape) as i32;
                            f[0] += (f[2] - cut) / 2;
                            f[2] = cut;
                        } else {
                            let cut = (f[2] as f32 / shape) as i32;
                            f[1] += (f[3] - cut) / 2;
                            f[3] = cut;
                        }
                        // at least a third of the picture across
                        if 3 * f[2] >= tex_w && heuristic(&f) > heuristic(&found) {
                            found = f;
                        }
                    }
                }
                if x < end {
                    stack.push(x);
                }
            }
        }
        if found[3] > 0 {
            lum_max = limit;
            best = found;
            area = next_area;
        } else {
            lum_min = limit;
        }
    }
    (best[2] != 0 && best[3] != 0).then_some((best, lum_max))
}

fn union(a: [i32; 4], b: [i32; 4]) -> [i32; 4] {
    let x = a[0].min(b[0]);
    let y = a[1].min(b[1]);
    [x, y, (a[0] + a[2]).max(b[0] + b[2]) - x, (a[1] + a[3]).max(b[1] + b[3]) - y]
}

/// A picture's pixels as VPX's search reads them — red, green and blue, rows from the top, any
/// transparency left out, as VPX decodes a backglass file's pictures (stb_image's channels as the
/// file has them) — with its width and height. None for one VPX hands add-ons in another form
/// (grey alone), or one that can't be read.
fn pixels(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory(bytes).ok()?;
    if matches!(image.color(), image::ColorType::L8 | image::ColorType::L16) {
        return None;
    }
    let rgb = image.to_rgb8();
    let (w, h) = rgb.dimensions();
    Some((rgb.into_raw(), w, h))
}

/// The file's text with `places` in a `<Displays>` of their own, put last in the file's root, as
/// VPX's menu puts it, in the file's own way — its line ends and indent — and nothing else of it
/// changed. None where the text has no `<Displays>`-free root closing it.
pub fn with_places(text: &[u8], places: &[Place]) -> Option<Vec<u8>> {
    if holds_displays(text) || places.is_empty() {
        return None;
    }
    let close = rfind_bytes(text, b"</DirectB2SData")?;
    let nl: &[u8] = if find_bytes(text, b"\r\n").is_some() { b"\r\n" } else { b"\n" };
    // the indent the root's first child has
    let indent: Vec<u8> = text.iter().position(|&b| b == b'\n').map_or_else(Vec::new, |n| text[n + 1..].iter().take_while(|&&b| b == b' ' || b == b'\t').copied().collect());
    let indent = if indent.is_empty() { b"  ".to_vec() } else { indent };
    let mut block = Vec::new();
    block.extend_from_slice(&indent);
    block.extend_from_slice(b"<Displays>");
    block.extend_from_slice(nl);
    for p in places {
        block.extend_from_slice(&indent);
        block.extend_from_slice(&indent);
        block.extend_from_slice(format!("<DMD Target=\"{}\" X=\"{}\" Y=\"{}\" W=\"{}\" H=\"{}\" />", p.target.name(), p.x, p.y, p.w, p.h).as_bytes());
        block.extend_from_slice(nl);
    }
    block.extend_from_slice(&indent);
    block.extend_from_slice(b"</Displays>");
    block.extend_from_slice(nl);
    // the root's closing tag on a line of its own keeps it, the block on the lines before
    let line = text[..close].iter().rposition(|&b| b == b'\n').map_or(0, |n| n + 1);
    let at = if text[line..close].iter().all(u8::is_ascii_whitespace) {
        line
    } else {
        block.splice(0..0, nl.iter().copied());
        close
    };
    let mut out = Vec::with_capacity(text.len() + block.len());
    out.extend_from_slice(&text[..at]);
    out.extend_from_slice(&block);
    out.extend_from_slice(&text[at..]);
    Some(out)
}

/// `places` written into a backglass file holding none ([`with_places`]), the file as it was kept
/// at `keep` first (copied whole, its date with it), the file then replaced in one step — written
/// whole under a passing name beside it first, as VPX saves one — so no half-written file ever
/// stands in its place. Nothing is written over a file at `keep`.
pub fn place_in(file: &Path, places: &[Place], keep: &Path) -> std::io::Result<()> {
    let text = std::fs::read(file)?;
    let new = with_places(&text, places).ok_or_else(|| std::io::Error::other("it holds displays of its own, or isn't a backglass file"))?;
    if keep.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("{} is there already", keep.display())));
    }
    if let Some(dir) = keep.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::copy(file, keep)?;
    replace(file, |passing| std::fs::write(passing, &new))
}

/// The picture a place over `target` is drawn over, as the file keeps it (a PNG or a JPEG, as its
/// author saved it), for showing it: the first of those VPX's search reads ([`find_place`]).
pub fn picture_for(text: &[u8], target: Target) -> Option<Vec<u8>> {
    target.pictures().iter().find_map(|name| picture(text, name))
}

/// A picture's width and height, read from its header alone.
pub fn picture_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

/// The file's text with the score display over `place.target` at `place`: the first `<DMD>` over
/// that picture given its new `X`, `Y`, `W` and `H`, the rest of it as it was (the style, tint and
/// glass VPX's menu gives one, as VPX's `WriteDisplays` writes them); one added to the file's
/// `<Displays>` where none is over that picture, in the file's own way — its line ends and indent;
/// a `<Displays>` of its own where the file has none ([`with_places`]). None where the text isn't a
/// backglass file's.
pub fn with_place(text: &[u8], place: Place) -> Option<Vec<u8>> {
    let Some(at) = tag(text, b"Displays", 0) else { return with_places(text, &[place]) };
    // the element's own end: `<Displays/>` (VPX's menu leaves one with every place taken out), or its
    // closing tag
    let open_end = at + text[at..].iter().position(|&b| b == b'>')?;
    let empty = text[open_end - 1] == b'/';
    let end = if empty { open_end + 1 } else { at + find_bytes(&text[at..], b"</Displays")? };
    let mut from = at + 1;
    while let Some(dmd) = tag(&text[..end], b"DMD", from) {
        let close = dmd + text[dmd..end].iter().position(|&b| b == b'>')?;
        let opening = &text[dmd..=close];
        let target = match attribute(opening, b"Target") {
            Some(t) if t.eq_ignore_ascii_case(b"scoreview") => Target::ScoreView,
            _ => Target::Backglass,
        };
        if target == place.target {
            let mut changed = opening.to_vec();
            for (name, value) in [(&b"X"[..], place.x), (&b"Y"[..], place.y), (&b"W"[..], place.w), (&b"H"[..], place.h)] {
                changed = with_attribute(&changed, name, value.to_string().as_bytes())?;
            }
            return Some([&text[..dmd], &changed[..], &text[close + 1..]].concat());
        }
        from = dmd + 1;
    }
    // none over that picture: one added
    let nl: &[u8] = if find_bytes(text, b"\r\n").is_some() { b"\r\n" } else { b"\n" };
    let line = text[..at].iter().rposition(|&b| b == b'\n').map_or(0, |n| n + 1);
    let indent: Vec<u8> = if text[line..at].iter().all(|&b| b == b' ' || b == b'\t') { text[line..at].to_vec() } else { Vec::new() };
    let child = if indent.is_empty() { b"  ".to_vec() } else { [indent.as_slice(), indent.as_slice()].concat() };
    let element = format!("<DMD Target=\"{}\" X=\"{}\" Y=\"{}\" W=\"{}\" H=\"{}\" />", place.target.name(), place.x, place.y, place.w, place.h);
    let added = if empty {
        let opened = [b"<Displays>".as_slice(), nl, &child, element.as_bytes(), nl, &indent, b"</Displays>"].concat();
        [&text[..at], &opened[..], &text[open_end + 1..]].concat()
    } else {
        // on a line of its own before the closing tag's, where that's on a line of its own
        let closing_line = text[..end].iter().rposition(|&b| b == b'\n').map_or(0, |n| n + 1);
        if closing_line > at && text[closing_line..end].iter().all(|&b| b == b' ' || b == b'\t') {
            [&text[..closing_line], &child, element.as_bytes(), nl, &text[closing_line..]].concat()
        } else {
            [&text[..end], nl, &child, element.as_bytes(), nl, &indent, &text[end..]].concat()
        }
    };
    Some(added)
}

/// An element's opening tag (`<Name a="…" …>`) with attribute `name` given `value`: its value
/// changed where it has one, in its own quotes, else the attribute added last.
fn with_attribute(opening: &[u8], name: &[u8], value: &[u8]) -> Option<Vec<u8>> {
    if let Some((start, end)) = attribute_span(opening, name) {
        return Some([&opening[..start], value, &opening[end..]].concat());
    }
    // before the tag's end, `/>` or `>`
    let mut at = opening.len().checked_sub(1)?;
    if at > 0 && opening[at - 1] == b'/' {
        at -= 1;
    }
    while at > 0 && opening[at - 1].is_ascii_whitespace() {
        at -= 1;
    }
    let tail = opening[at..].trim_ascii_start();
    let space: &[u8] = if tail.first() == Some(&b'/') { b" " } else { b"" };
    Some([&opening[..at], b" ", name, b"=\"", value, b"\"", space, tail].concat())
}

/// `place` written into a backglass file ([`with_place`]), the file kept at `keep` first where one
/// is given and none is kept there yet (copied whole, its date with it), then replaced in one step.
pub fn place_by_hand(file: &Path, place: Place, keep: Option<&Path>) -> std::io::Result<()> {
    let text = std::fs::read(file)?;
    let new = with_place(&text, place).ok_or_else(|| std::io::Error::other("it isn't a backglass file"))?;
    if let Some(keep) = keep.filter(|k| !k.exists()) {
        if let Some(dir) = keep.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::copy(file, keep)?;
    }
    replace(file, |passing| std::fs::write(passing, &new))
}

/// A backglass file put back as it was kept by [`place_in`] (its date with it), the copy taken away.
pub fn put_back(file: &Path, keep: &Path) -> std::io::Result<()> {
    replace(file, |passing| std::fs::copy(keep, passing).map(|_| ()))?;
    std::fs::remove_file(keep)
}

/// `file` replaced by what `write` puts under a passing name beside it, in one step.
fn replace(file: &Path, write: impl FnOnce(&Path) -> std::io::Result<()>) -> std::io::Result<()> {
    let mut passing = file.as_os_str().to_owned();
    passing.push(".vpi-new");
    let passing = PathBuf::from(passing);
    let done = write(&passing).and_then(|_| std::fs::rename(&passing, file));
    if done.is_err() {
        let _ = std::fs::remove_file(&passing);
    }
    done
}

/// A picture of the file's (`Images/<name>`'s `Value`), decoded from the base64 text it's kept in.
fn picture(text: &[u8], name: &str) -> Option<Vec<u8>> {
    let images = tag(text, b"Images", 0)?;
    let end = find_bytes(&text[images..], b"</Images").map_or(text.len(), |e| images + e);
    let at = tag(&text[..end], name.as_bytes(), images + 1)?;
    base64(attribute(&text[at..end], b"Value")?)
}

/// Where the first `<name` tag starts in `text` from `from` on (a tag of that name, not a longer one).
fn tag(text: &[u8], name: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    while at < text.len() {
        let open = at + text[at..].iter().position(|&b| b == b'<')?;
        let after = open + 1 + name.len();
        if text.get(open + 1..after) == Some(name) && text.get(after).is_none_or(|&b| b.is_ascii_whitespace() || b == b'>' || b == b'/') {
            return Some(open);
        }
        at = open + 1;
    }
    None
}

/// The value of attribute `name` on the element whose tag starts `element` (`<Name a="…" …>`), as
/// written (its entities left in).
fn attribute<'a>(element: &'a [u8], name: &[u8]) -> Option<&'a [u8]> {
    attribute_span(element, name).map(|(start, end)| &element[start..end])
}

/// Where the value of attribute `name` lies on the element whose tag starts `element`, inside its
/// quotes.
fn attribute_span(element: &[u8], name: &[u8]) -> Option<(usize, usize)> {
    let mut i = 1 + element[1..].iter().position(|b| b.is_ascii_whitespace() || *b == b'>' || *b == b'/')?;
    loop {
        while element.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        match element.get(i)? {
            b'>' | b'/' => return None,
            _ => {}
        }
        let start = i;
        while element.get(i).is_some_and(|b| !b.is_ascii_whitespace() && *b != b'=' && *b != b'>') {
            i += 1;
        }
        let key = &element[start..i];
        while element.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        if element.get(i) != Some(&b'=') {
            return None;
        }
        i += 1;
        while element.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let quote = *element.get(i).filter(|q| **q == b'"' || **q == b'\'')?;
        let value_start = i + 1;
        let value_end = value_start + element[value_start..].iter().position(|&b| b == quote)?;
        if key == name {
            return Some((value_start, value_end));
        }
        i = value_end + 1;
    }
}

/// Base64 text decoded as VPX reads it: entities first (a line break written as `&#xD;&#xA;`),
/// then anything not of base64's alphabet passed over, up to its padding.
fn base64(text: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut i = 0;
    while i < text.len() {
        let mut c = text[i];
        i += 1;
        if c == b'&' {
            // a character written as an entity: `&#13;`, `&#xD;`; a named one stands for none of base64's
            let end = i + text[i..].iter().position(|&b| b == b';')?;
            let entity = &text[i..end];
            i = end + 1;
            let code = match entity {
                [b'#', b'x' | b'X', hex @ ..] => std::str::from_utf8(hex).ok().and_then(|h| u32::from_str_radix(h, 16).ok()),
                [b'#', dec @ ..] => std::str::from_utf8(dec).ok().and_then(|d| d.parse::<u32>().ok()),
                _ => None,
            };
            match code.and_then(|c| u8::try_from(c).ok()) {
                Some(b) => c = b,
                None => continue,
            }
        }
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = ((acc << 6) | u32::from(v)) & 0xFF_FFFF;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    (!out.is_empty()).then_some(out)
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let first = needle[0];
    let mut at = 0;
    while at + needle.len() <= hay.len() {
        let next = at + hay[at..=hay.len() - needle.len()].iter().position(|&b| b == first)?;
        if &hay[next..next + needle.len()] == needle {
            return Some(next);
        }
        at = next + 1;
    }
    None
}

fn rfind_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    (0..=hay.len().checked_sub(needle.len())?).rev().find(|&at| &hay[at..at + needle.len()] == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture `w` × `h` of grey `level`, with a black window at `[x, y, w, h]`.
    fn picture_with_window(w: u32, h: u32, level: u8, window: [u32; 4]) -> Vec<u8> {
        let mut rgb = vec![level; (w * h * 3) as usize];
        for y in window[1]..window[1] + window[3] {
            for x in window[0]..window[0] + window[2] {
                let p = ((y * w + x) * 3) as usize;
                rgb[p..p + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        rgb
    }

    fn encode(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
            for i in 0..4 {
                s.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
            }
        }
        s
    }

    fn png(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_raw(w, h, rgb.to_vec()).unwrap().write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// A backglass file as B2S's makers write one: its pictures' base64 broken into lines written
    /// as entities, CR LF line ends, two spaces of indent.
    fn backglass_file(picture: &[u8]) -> Vec<u8> {
        let text = encode(picture);
        let broken: Vec<String> = text.as_bytes().chunks(76).map(|c| String::from_utf8(c.to_vec()).unwrap()).collect();
        format!(
            "<DirectB2SData Version=\"1.2\">\r\n  <Name Value=\"Test\" />\r\n  <GrillHeight Value=\"0\" />\r\n  <Images>\r\n    <ThumbnailImage Value=\"{}\" />\r\n    <BackglassImage Value=\"{}\" FileName=\"bg.png\" />\r\n  </Images>\r\n</DirectB2SData>",
            encode(b"thumbnail"),
            broken.join("&#xD;&#xA;")
        )
        .into_bytes()
    }

    #[test]
    fn the_dark_window_is_found_cut_to_the_dmds_shape() {
        // a window 400 × 150 at 120, 300 in a white picture 640 × 480 (under 960 pixels across, so
        // unpadded): the first rectangle of the DMD's shape as wide as it, from its top, as VPX
        // keeps the first of those it rates the same
        let rgb = picture_with_window(640, 480, 255, [120, 300, 400, 150]);
        assert_eq!(find(&rgb, 640, 480, 4.0), Some([120, 300, 400, 100]));
        assert_eq!(find(&rgb, 640, 480, 3.0), Some([120, 300, 400, 133]));
        assert_eq!(find(&rgb, 640, 480, 8.0), Some([120, 300, 400, 50]));
        // a window narrower than a third of the picture is no place
        let rgb = picture_with_window(640, 480, 255, [10, 10, 200, 60]);
        assert_eq!(find(&rgb, 640, 480, 4.0), None);
        // nor is a picture with none
        assert_eq!(find(&vec![255; 640 * 480 * 3], 640, 480, 4.0), None);
        // a light grey one is dark enough in time: the search's limit raised to find any, the
        // whole picture the darkest rectangle, cut to the DMD's shape
        assert_eq!(find(&vec![200; 640 * 480 * 3], 640, 480, 4.0), Some([0, 0, 640, 160]));
    }

    #[test]
    fn a_wide_picture_pads_the_window() {
        // 1920 across: two pixels of padding each side, the rest cut to the DMD's shape about its middle
        let rgb = picture_with_window(1920, 1080, 255, [460, 700, 1000, 250]);
        assert_eq!(find(&rgb, 1920, 1080, 4.0), Some([468, 702, 984, 246]));
    }

    #[test]
    fn the_dmds_shape_is_by_the_machine() {
        assert_eq!(dmd_shape(Some("afm_113b")), 4.0);
        assert_eq!(dmd_shape(None), 4.0);
        assert_eq!(dmd_shape(Some("btmn_106")), 8.0);
        assert_eq!(dmd_shape(Some("TMNT_104")), 8.0);
        assert_eq!(dmd_shape(Some("mav_402")), 3.0);
        assert_eq!(dmd_shape(Some("frankst")), 3.0);
        assert_eq!(dmd_shape(Some("bdk_294")), 4.0);
    }

    #[test]
    fn a_place_is_found_in_the_files_picture_and_written_as_vpx_writes_it() {
        let rgb = picture_with_window(640, 480, 255, [120, 300, 400, 150]);
        let file = backglass_file(&png(&rgb, 640, 480));
        assert!(!holds_displays(&file));
        let place = find_place(&file, Target::Backglass, 4.0).unwrap();
        assert_eq!(place, Place { target: Target::Backglass, x: 120, y: 300, w: 400, h: 100 });
        // the file holds no score panel
        assert_eq!(find_place(&file, Target::ScoreView, 4.0), None);
        let placed = with_places(&file, &[place]).unwrap();
        let text = String::from_utf8(placed.clone()).unwrap();
        assert!(text.ends_with("  </Images>\r\n  <Displays>\r\n    <DMD Target=\"Backglass\" X=\"120\" Y=\"300\" W=\"400\" H=\"100\" />\r\n  </Displays>\r\n</DirectB2SData>"), "{}", &text[text.len() - 200..]);
        // everything before it as it was
        assert_eq!(&placed[..file.len() - "</DirectB2SData>".len()], &file[..file.len() - "</DirectB2SData>".len()]);
        assert!(holds_displays(&placed));
        assert_eq!(places_in(&placed), vec![place]);
        // a file holding displays is left as it is
        assert_eq!(with_places(&placed, &[place]), None);
        assert!(roxmltree::Document::parse(&text).is_ok());
    }

    /// A backglass file of one picture `w` × `h` and a speaker panel `grill` high at its foot,
    /// painted white with rectangles of grey.
    fn painted_file(w: u32, h: u32, grill: i32, rects: &[([u32; 4], u8)]) -> Vec<u8> {
        let mut rgb = vec![255u8; (w * h * 3) as usize];
        for &(r, level) in rects {
            for y in r[1]..r[1] + r[3] {
                for x in r[0]..r[0] + r[2] {
                    let p = ((y * w + x) * 3) as usize;
                    rgb[p..p + 3].copy_from_slice(&[level; 3]);
                }
            }
        }
        let file = String::from_utf8(backglass_file(&png(&rgb, w, h))).unwrap();
        file.replace("<GrillHeight Value=\"0\" />", &format!("<GrillHeight Value=\"{grill}\" />")).into_bytes()
    }

    #[test]
    fn a_place_fits_a_window_not_the_darkest_art() {
        let place = |file: &[u8]| fitting_place(file, Target::Backglass, 4.0).map(|p| [p.x, p.y, p.w, p.h]);
        // no speaker panel: a window drawn black is a place, grey art isn't
        assert_eq!(place(&painted_file(640, 480, 0, &[([120, 300, 400, 150], 0)])), Some([120, 300, 400, 100]));
        let art = painted_file(640, 480, 0, &[([120, 300, 400, 150], 50)]);
        assert!(find_place(&art, Target::Backglass, 4.0).is_some());
        assert_eq!(place(&art), None);
        // a speaker panel 120 high, darker art above it than its window: the panel searched alone
        let panel = painted_file(640, 480, 120, &[([100, 50, 440, 200], 40), ([170, 390, 300, 60], 60)]);
        assert_eq!(find_place(&panel, Target::Backglass, 4.0).map(|p| p.y), Some(50));
        assert_eq!(place(&panel), Some([200, 390, 240, 60]));
        // a window in the panel VPX's search finds itself, as it finds it
        let found = painted_file(640, 480, 120, &[([100, 50, 440, 200], 40), ([170, 390, 300, 60], 0)]);
        assert_eq!(place(&found), find_place(&found, Target::Backglass, 4.0).map(|p| [p.x, p.y, p.w, p.h]));
        assert_eq!(place(&found), Some([200, 390, 240, 60]));
        // a file showing scores of its own is for a machine without a DMD
        assert!(!shows_scores(&found));
        let scores = String::from_utf8(found).unwrap().replace("<Images>", "<Scores>\r\n    <Score Parent=\"DMD\" ID=\"1\" Digits=\"7\" />\r\n  </Scores>\r\n  <Images>");
        assert!(shows_scores(scores.as_bytes()));
        assert_eq!(grill_height(scores.as_bytes()), 120);
    }

    /// A folder copied whole, but for VPX's texture cache and capture mode's pictures.
    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let (p, name) = (entry.path(), entry.file_name());
            if p.is_dir() {
                if name != "cache" && name != "Capture" {
                    copy_tree(&p, &to.join(&name));
                }
            } else {
                std::fs::copy(&p, to.join(&name)).unwrap();
            }
        }
    }

    /// VPX drawing the score display where a place written here puts it: a scratch copy of the test
    /// library's Attack from Mars, its backglass file given the place found in it, played in VPX's
    /// capture mode on a desktop never shown, its backglass window's pictures kept for a look in
    /// `%TEMP%\vpi-backglass-dmd`. `cargo test --release -p library --lib
    /// vpx_draws_the_score_display_where_its_placed -- --ignored --nocapture`, `VPI_TEST_LIBRARY`
    /// naming a test library whose VPX keeps places in the file (build 6267 on).
    #[test]
    #[ignore]
    fn vpx_draws_the_score_display_where_its_placed() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-6758"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        println!("VPX {:?}", vpx.version);
        let work = std::env::temp_dir().join(format!("vpi-backglass-dmd-vpx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let folder = work.join("Attack from Mars (Bally 1995)");
        copy_tree(&lib.join("Tables").join("Attack from Mars (Bally 1995)"), &folder);
        let table = folder.join("Attack from Mars (Bally 1995).vpx");
        let file = playing_file(&table).unwrap();
        let text = std::fs::read(&file).unwrap();
        let place = fitting_place(&text, Target::Backglass, dmd_shape(Some("afm_113b"))).unwrap();
        let (_, width, height) = searched_picture(&text, Target::Backglass).unwrap();
        println!("placed at {place:?} in its picture of {width} × {height}");
        place_in(&file, &[place], &work.join("kept.directb2s")).unwrap();
        // the test library's settings, its playfield a small window, its backglass one of its own
        // the backglass picture's shape, its speaker panel shown
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
        let bg_height = (960 * height / width).to_string();
        let changes = [
            set("Player", "PlayfieldDisplay", ""),
            set("Player", "PlayfieldFullScreen", "0"),
            set("Player", "PlayfieldWndX", "0"),
            set("Player", "PlayfieldWndY", "0"),
            set("Player", "PlayfieldWidth", "640"),
            set("Player", "PlayfieldHeight", "480"),
            set("Player", "BGSet", "0"),
            set("Player", "SoundVolume", "0"),
            set("Player", "MusicVolume", "0"),
            set("Backglass", "BackglassOutput", "1"),
            set("Backglass", "BackglassDisplay", ""),
            set("Backglass", "BackglassFullScreen", "0"),
            set("Backglass", "BackglassWndX", "660"),
            set("Backglass", "BackglassWndY", "0"),
            set("Backglass", "BackglassWidth", "960"),
            set("Backglass", "BackglassHeight", &bg_height),
            set("ScoreView", "ScoreViewOutput", "0"),
            set("Topper", "TopperOutput", "0"),
            set("Plugin.B2S", "Enable", "1"),
            set("Plugin.B2S", "ShowGrill", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // every picture kept as taken (`noloop`)
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "3".into(), "1".into(), table.clone().into(), "noloop".into()];
        let code = crate::vpx_test::unseen(&vpx.program, &args, 150).unwrap();
        println!("VPX ended: {code:?}");
        let look = std::env::temp_dir().join("vpi-backglass-dmd");
        let _ = std::fs::remove_dir_all(&look);
        std::fs::create_dir_all(&look).unwrap();
        for p in std::fs::read_dir(folder.join("Capture")).into_iter().flatten().flatten().map(|e| e.path()) {
            let _ = std::fs::copy(&p, look.join(p.file_name().unwrap()));
        }
        println!("capture mode's pictures: {:?}", std::fs::read_dir(&look).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>());
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("B2S") || l.contains("rror") || l.contains("isplay")) {
            println!("vpx: {l}");
        }
        // the place as written, the file left so by the run
        assert_eq!(places_in(&std::fs::read(&file).unwrap()), vec![place]);
        assert!(!vpx_log.contains("Failed to load B2S"), "VPX didn't read the backglass file");
        assert_eq!(code, Some(0), "VPX didn't end cleanly");
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn vpxs_own_places_are_read() {
        let text = b"<DirectB2SData>\n<Displays>\n    <DMD Target=\"ScoreView\" Source=\"ctrl://default/display\" X=\"5\" Y=\"6\" W=\"700\" H=\"175\" Style=\"Plasma\"/>\n    <DMD X=\"1\" Y=\"2\" W=\"3\" H=\"4\"/>\n</Displays>\n</DirectB2SData>\n";
        assert_eq!(
            places_in(text),
            vec![Place { target: Target::ScoreView, x: 5, y: 6, w: 700, h: 175 }, Place { target: Target::Backglass, x: 1, y: 2, w: 3, h: 4 }]
        );
        // none left by VPX's menu: displays held all the same
        assert!(holds_displays(b"<DirectB2SData><Displays/></DirectB2SData>"));
        assert!(!holds_displays(b"<DirectB2SData><DisplaysOther/></DirectB2SData>"));
    }

    #[test]
    fn base64_is_read_through_its_entities() {
        assert_eq!(base64(b"aGVs&#xD;&#xA;bG8=").as_deref(), Some(&b"hello"[..]));
        // a character written as an entity stands for itself
        assert_eq!(base64(b"aGVsbG8&#43;").as_deref(), base64(b"aGVsbG8+").as_deref());
        assert_eq!(attribute(b"<BackglassImage FileName='a b.png' Value=\"QUJD\" />", b"Value"), Some(&b"QUJD"[..]));
        assert_eq!(attribute(b"<BackglassImage FileName='a b.png' />", b"Value"), None);
    }

    #[test]
    fn the_file_is_kept_and_put_back() {
        let dir = std::env::temp_dir().join(format!("vpi-backglass-dmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = dir.join("Test (Maker 1990).vpx");
        std::fs::write(&table, b"").unwrap();
        assert_eq!(playing_file(&table), None);
        let rgb = picture_with_window(640, 480, 255, [120, 300, 400, 150]);
        let file = dir.join("Test (Maker 1990).directb2s");
        let original = backglass_file(&png(&rgb, 640, 480));
        std::fs::write(&file, &original).unwrap();
        assert_eq!(playing_file(&table).as_deref(), Some(file.as_path()));
        let keep = dir.join("kept").join("Test (Maker 1990).directb2s");
        let place = find_place(&original, Target::Backglass, 4.0).unwrap();
        place_in(&file, &[place], &keep).unwrap();
        assert_eq!(std::fs::read(&keep).unwrap(), original);
        assert_eq!(places_in(&std::fs::read(&file).unwrap()), vec![place]);
        // never twice, never over a copy kept
        assert!(place_in(&file, &[place], &dir.join("other.directb2s")).is_err());
        put_back(&file, &keep).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), original);
        assert!(!keep.exists());
        std::fs::write(&keep, b"someone's").unwrap();
        assert!(place_in(&file, &[place], &keep).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), original);
        // a table in a folder of its own, its backglass named as the folder
        let own = dir.join("Game");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("Game.directb2s"), b"x").unwrap();
        assert_eq!(playing_file(&own.join("Game v1.2.vpx")), Some(own.join("Game.directb2s")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_place_set_by_hand_changes_only_its_numbers() {
        let place = |target, x, y, w, h| Place { target, x, y, w, h };
        let at = place(Target::Backglass, 500, 1910, 1380, 345);
        // as VPX's menu writes one (tinyxml2's indent), its style, tint and glass picture kept
        let vpx = "<DirectB2SData Version=\"1.2\">\n    <Images>\n        <BackglassImage Value=\"QUJD\" />\n    </Images>\n    <Displays>\n        <DMD Target=\"ScoreView\" X=\"1\" Y=\"2\" W=\"3\" H=\"4\"/>\n        <DMD Target=\"Backglass\" X=\"490\" Y=\"1900\" W=\"1388\" H=\"347\" Style=\"Neon Plasma\" Tint=\"1,0.5,0.2\">\n            <GlassImage Value=\"R0lG\"/>\n        </DMD>\n    </Displays>\n</DirectB2SData>\n";
        let changed = String::from_utf8(with_place(vpx.as_bytes(), at).unwrap()).unwrap();
        assert_eq!(changed, vpx.replace("X=\"490\" Y=\"1900\" W=\"1388\" H=\"347\"", "X=\"500\" Y=\"1910\" W=\"1380\" H=\"345\""));
        assert_eq!(places_in(changed.as_bytes()), vec![place(Target::ScoreView, 1, 2, 3, 4), at]);
        // every place taken out in VPX's menu: the one put back in
        let emptied = "<DirectB2SData Version=\"1.2\">\r\n  <GrillHeight Value=\"120\" />\r\n  <Displays/>\r\n</DirectB2SData>";
        let opened = String::from_utf8(with_place(emptied.as_bytes(), at).unwrap()).unwrap();
        assert_eq!(opened, "<DirectB2SData Version=\"1.2\">\r\n  <GrillHeight Value=\"120\" />\r\n  <Displays>\r\n    <DMD Target=\"Backglass\" X=\"500\" Y=\"1910\" W=\"1380\" H=\"345\" />\r\n  </Displays>\r\n</DirectB2SData>");
        // the score panel's alone: the backglass's added beside it
        let panel = "<DirectB2SData>\n  <Displays>\n    <DMD Target=\"ScoreView\" X=\"1\" Y=\"2\" W=\"3\" H=\"4\" />\n  </Displays>\n</DirectB2SData>\n";
        let beside = with_place(panel.as_bytes(), at).unwrap();
        assert_eq!(places_in(&beside), vec![place(Target::ScoreView, 1, 2, 3, 4), at]);
        assert!(String::from_utf8(beside).unwrap().contains("\n    <DMD Target=\"Backglass\" X=\"500\" Y=\"1910\" W=\"1380\" H=\"345\" />\n  </Displays>"));
        // none at all: a `<Displays>` of its own, as the app places one
        let bare = "<DirectB2SData>\n  <GrillHeight Value=\"0\" />\n</DirectB2SData>\n";
        assert_eq!(with_place(bare.as_bytes(), at), with_places(bare.as_bytes(), &[at]));
        // an attribute missing from a place: added last
        assert_eq!(with_attribute(b"<DMD Target=\"Backglass\" X=\"1\" />", b"Y", b"7").unwrap(), b"<DMD Target=\"Backglass\" X=\"1\" Y=\"7\" />".to_vec());
        assert_eq!(with_attribute(b"<DMD X=\"1\">", b"W", b"9").unwrap(), b"<DMD X=\"1\" W=\"9\">".to_vec());
        // not a backglass file
        assert!(with_place(b"<Other />", at).is_none());
    }

    #[test]
    fn a_place_set_by_hand_keeps_the_file_as_it_came_once() {
        let dir = std::env::temp_dir().join(format!("vpi-dmd-by-hand-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Game.directb2s");
        let original = b"<DirectB2SData>\n  <Displays>\n    <DMD Target=\"Backglass\" X=\"1\" Y=\"2\" W=\"3\" H=\"4\" />\n  </Displays>\n</DirectB2SData>\n".to_vec();
        std::fs::write(&file, &original).unwrap();
        let keep = dir.join("kept").join("Game.directb2s");
        let first = Place { target: Target::Backglass, x: 10, y: 20, w: 400, h: 100 };
        place_by_hand(&file, first, Some(&keep)).unwrap();
        assert_eq!(places_in(&std::fs::read(&file).unwrap()), vec![first]);
        assert_eq!(std::fs::read(&keep).unwrap(), original);
        // a second time: the copy kept as it came, never written over
        let second = Place { x: 12, ..first };
        place_by_hand(&file, second, Some(&keep)).unwrap();
        assert_eq!(places_in(&std::fs::read(&file).unwrap()), vec![second]);
        assert_eq!(std::fs::read(&keep).unwrap(), original);
        assert!(!dir.join("Game.directb2s.vpi-new").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
