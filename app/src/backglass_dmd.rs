//! The score display placed over each table's backglass picture for a VPX that keeps its place in
//! the backglass file (`library::backglass_dmd`; from build 6267, `screens::OVERLAY_BY_HAND_FROM`),
//! where Settings › Screens finds it in the picture — `[Plugin.B2S]` `BackglassDMDOverlay` with
//! `BackglassDMDAutoPos` (`ScoreViewDMD…` for a backglass file's score panel), VPX's own names for
//! it, which its builds before honoured themselves.
//!
//! Once the library is read, in the background, a file at a time, and for a table about to play.
//! Only a backglass made for a DMD gets one (one showing scores of its own is left alone), only in
//! its own window for it (`fitting_place`). Each file is kept as it came in the data folder's
//! `Backglasses as they came` before it's changed; what was done is remembered in
//! `Library\backglass-places.json`, with each file's size and date as left, so a file is looked at
//! again only once it changes — one VPX's menu changed holds places of its own then, left as they
//! are. Turned off, the places put in are taken out again, each file put back as it came, but those
//! changed since in VPX's menu.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use ::library::backglass_dmd::{self as dmd, Place, Target};
use ::library::plan::Layout;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::library::Shared;

/// A backglass file as it was looked at, and what was found in it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Seen {
    size: u64,
    modified: i64,
    /// The places asked for when it was looked at.
    targets: Vec<Target>,
    /// `placed`, `own` (places of its own), `scores` (scores of its own: made for a machine
    /// without a DMD), `none` (no window for one in its picture).
    look: String,
}

/// A file places were put in: as it was left, and where it's kept as it came.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Placed {
    file: String,
    size: u64,
    modified: i64,
    kept: String,
    places: Vec<Place>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Kept {
    version: u32,
    /// By the file's path.
    seen: BTreeMap<String, Seen>,
    placed: Vec<Placed>,
    /// Files a place was set in by hand (a table page's Score display…), by their paths: where each
    /// is kept as it came, where the app hadn't kept it before.
    by_hand: BTreeMap<String, String>,
}

/// Raised when files are looked at differently, so each is looked at again.
const VERSION: u32 = 1;

/// What's remembered, read once: the background pass and PLAY take turns with it, a file at a time.
static KEPT: Mutex<Option<Kept>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);
static AGAIN: AtomicBool = AtomicBool::new(false);

fn kept_path() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Library").join("backglass-places.json"))
}

/// Where each file changed is kept as it came.
fn copies_folder() -> Option<PathBuf> {
    Some(crate::library::data_folder()?.join("Backglasses as they came"))
}

fn load() -> Kept {
    kept_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<Kept>(&t).ok())
        .filter(|k| k.version == VERSION)
        .unwrap_or_else(|| Kept { version: VERSION, ..Kept::default() })
}

fn save(kept: &Kept) {
    let Some(path) = kept_path() else { return };
    let Ok(text) = serde_json::to_string_pretty(kept) else { return };
    let part = path.with_extension("json.part");
    if std::fs::write(&part, text).and_then(|_| std::fs::rename(&part, &path)).is_err() {
        eprintln!("the backglasses' places weren't remembered");
    }
}

/// `work` done with what's remembered, read the first time.
fn with_kept<T>(work: impl FnOnce(&mut Kept) -> T) -> T {
    let mut held = KEPT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    work(held.get_or_insert_with(load))
}

fn stamp(file: &Path) -> Option<(u64, i64)> {
    let meta = std::fs::metadata(file).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
    Some((meta.len(), modified))
}

/// The places Settings › Screens finds in the picture, for a VPX that keeps them in the backglass
/// file: the backglass's and the score panel's, each where it's asked for (none: those put in
/// before taken out again). None for a VPX that places them itself, or none at all: files left as
/// they are.
fn wanted(vpx: &::library::vpx::Vpx) -> Option<Vec<Target>> {
    if vpx.version < [10, 8, 1, crate::screens::OVERLAY_BY_HAND_FROM] {
        return None;
    }
    let entries = vpi_settings::read_all();
    let mut targets = Vec::new();
    if crate::screens::overlay(&entries, "Backglass") == "auto" {
        targets.push(Target::Backglass);
    }
    if crate::screens::overlay(&entries, "ScoreView") == "auto" {
        targets.push(Target::ScoreView);
    }
    Some(targets)
}

/// Where `file` is kept as it came: as it lies in the tables folder, numbered beside any kept there.
fn keep_path(copies: &Path, tables: &Path, file: &Path) -> PathBuf {
    let within = file.strip_prefix(tables).ok().map(Path::to_path_buf).or_else(|| file.file_name().map(PathBuf::from)).unwrap_or_default();
    let first = copies.join(within);
    if !first.exists() {
        return first;
    }
    let stem = first.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    (2..).map(|n| first.with_file_name(format!("{stem} ({n}).directb2s"))).find(|p| !p.exists()).unwrap_or(first)
}

/// Whether the table's backglass file may need looking at: no quick look settles it.
fn to_look_at(kept: &Kept, table: &Path, wanted: &[Target]) -> bool {
    let Some(file) = dmd::playing_file(table) else { return false };
    let key = file.to_string_lossy().into_owned();
    let Some(now) = stamp(&file) else { return false };
    let placed = kept.placed.iter().find(|p| p.file == key && (p.size, p.modified) == now);
    match placed {
        Some(p) => targets_of(&p.places) != wanted,
        None => !wanted.is_empty() && !kept.seen.get(&key).is_some_and(|s| (s.size, s.modified) == now && s.targets == wanted),
    }
}

fn targets_of(places: &[Place]) -> Vec<Target> {
    let mut t: Vec<Target> = places.iter().map(|p| p.target).collect();
    t.sort();
    t.dedup();
    t
}

/// The table's backglass file brought to what's `wanted`: places found and put in, or those put in
/// before taken out where others are asked for now. Whether anything was done.
fn settle(kept: &mut Kept, layout: &Layout, copies: &Path, table: &Path, rom: Option<&str>, wanted: &[Target]) -> Result<bool, String> {
    let Some(file) = dmd::playing_file(table) else { return Ok(false) };
    let key = file.to_string_lossy().into_owned();
    let Some(mut now) = stamp(&file) else { return Ok(false) };
    if let Some(i) = kept.placed.iter().position(|p| p.file == key && (p.size, p.modified) == now) {
        if targets_of(&kept.placed[i].places) == wanted {
            return Ok(false);
        }
        // others asked for now: put back as it came, then looked at afresh
        let p = kept.placed.remove(i);
        dmd::put_back(&file, Path::new(&p.kept)).map_err(|e| format!("{}: {e}", file.display()))?;
        kept.seen.remove(&key);
        now = stamp(&file).ok_or_else(|| format!("{}: gone", file.display()))?;
        if wanted.is_empty() {
            return Ok(true);
        }
    }
    if wanted.is_empty() || kept.seen.get(&key).is_some_and(|s| (s.size, s.modified) == now && s.targets == wanted) {
        return Ok(false);
    }
    let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let look = if dmd::holds_displays(&text) {
        "own"
    } else if dmd::shows_scores(&text) {
        "scores"
    } else {
        let shape = dmd::dmd_shape(rom);
        let places: Vec<Place> = wanted.iter().filter_map(|&t| dmd::fitting_place(&text, t, shape)).collect();
        if places.is_empty() {
            "none"
        } else {
            let keep = keep_path(copies, &layout.tables, &file);
            drop(text);
            dmd::place_in(&file, &places, &keep).map_err(|e| format!("{}: {e}", file.display()))?;
            now = stamp(&file).ok_or_else(|| format!("{}: gone", file.display()))?;
            kept.placed.push(Placed { file: key.clone(), size: now.0, modified: now.1, kept: keep.to_string_lossy().into_owned(), places });
            "placed"
        }
    };
    kept.seen.insert(key, Seen { size: now.0, modified: now.1, targets: wanted.to_vec(), look: look.into() });
    Ok(look == "placed")
}

/// Every place put in taken out again: each file put back as it came where it's still as left —
/// where it was, or renamed beside it (another backglass of the table's chosen to play) — those
/// changed since in VPX's menu left as they are, their copies kept.
fn put_all_back(kept: &mut Kept) {
    let mut left = Vec::new();
    for p in std::mem::take(&mut kept.placed) {
        let at = Path::new(&p.file);
        let same = |f: &Path| stamp(f) == Some((p.size, p.modified));
        let found = if same(at) {
            Some(at.to_path_buf())
        } else {
            at.parent().and_then(|dir| {
                std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|f| f.extension().is_some_and(|x| x.eq_ignore_ascii_case("directb2s")) && same(f))
            })
        };
        match found.map(|f| dmd::put_back(&f, Path::new(&p.kept)).map(|_| f)) {
            Some(Ok(f)) => {
                kept.seen.remove(&f.to_string_lossy().into_owned());
            }
            Some(Err(e)) => {
                eprintln!("a backglass file couldn't be put back as it came: {}: {e}", p.file);
                left.push(p);
            }
            None => left.push(p),
        }
    }
    kept.placed = left;
}

/// The library's backglass files brought to what Settings › Screens asks for, in the background
/// (told in the tasks bar), the table playing passed over; one run at a time, a run asked for
/// meanwhile following it.
pub fn start(app: &AppHandle) {
    // the test mode never changes files
    if app.state::<Option<crate::shot::Shot>>().is_some() {
        return;
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        AGAIN.store(true, Ordering::SeqCst);
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("backglass places".into()).spawn(move || {
        background();
        loop {
            AGAIN.store(false, Ordering::SeqCst);
            run(&app);
            if !AGAIN.load(Ordering::SeqCst) {
                break;
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

fn run(app: &AppHandle) {
    let (layout, tables, playing) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let Some(layout) = st.layout.clone() else { return };
        let playing = st.playing.as_ref().and_then(|id| st.tables.iter().find(|t| &t.id == id)).map(|t| t.path.clone());
        let tables: Vec<(PathBuf, Option<String>)> = st.tables.iter().map(|t| (t.path.clone(), t.holds.rom_set.clone())).collect();
        (layout, tables, playing)
    };
    let Some(vpx) = ::library::vpx::to_play(&layout) else { return };
    let (Some(wanted), Some(copies)) = (wanted(&vpx), copies_folder()) else { return };
    if wanted.is_empty() {
        with_kept(|kept| {
            if !kept.placed.is_empty() {
                put_all_back(kept);
                save(kept);
            }
        });
        return;
    }
    let todo: Vec<&(PathBuf, Option<String>)> = with_kept(|kept| tables.iter().filter(|(t, _)| Some(t) != playing.as_ref() && to_look_at(kept, t, &wanted)).collect());
    if todo.is_empty() {
        return;
    }
    let task = crate::tasks::begin(app, "Placing score displays on backglasses", todo.len() as u32);
    for (n, (table, rom)) in todo.iter().enumerate() {
        let name = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        task.step(n as u32, Some(format!("Placing score displays on backglasses: {name}")));
        with_kept(|kept| {
            match settle(kept, &layout, &copies, table, rom.as_deref(), &wanted) {
                Ok(true) => save(kept),
                Ok(false) => {}
                Err(e) => eprintln!("a backglass's score display wasn't placed: {e}"),
            }
        });
    }
    with_kept(|kept| save(kept));
}

/// The table about to play's backglass file brought to what Settings › Screens asks for, before
/// VPX reads it.
pub fn before_play(layout: &Layout, vpx: &::library::vpx::Vpx, table: &Path, rom: Option<&str>) {
    let (Some(wanted), Some(copies)) = (wanted(vpx), copies_folder()) else { return };
    with_kept(|kept| match settle(kept, layout, &copies, table, rom, &wanted) {
        Ok(true) => save(kept),
        Ok(false) => {}
        Err(e) => eprintln!("the backglass's score display wasn't placed: {e}"),
    });
}

/// What the placing window shows of a table's backglass file (a table page's Score display…): its
/// name, the address of its picture and the picture's size, the place over it now (none: none yet),
/// the DMD's shape (its width over its height) and the speaker panel's height at its foot.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByHand {
    file: String,
    picture: String,
    width: u32,
    height: u32,
    place: Option<Place>,
    shape: f32,
    grill: i32,
}

/// The table's backglass file, the one that plays, as the placing window shows it.
pub fn by_hand(table: &Path, id: &str, rom: Option<&str>) -> Result<ByHand, String> {
    let file = dmd::playing_file(table).ok_or("It has no backglass file that plays.")?;
    let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let bytes = dmd::picture_for(&text, Target::Backglass).ok_or("Its backglass file holds no picture.")?;
    let (width, height) = dmd::picture_size(&bytes).ok_or("Its backglass picture couldn't be read.")?;
    Ok(ByHand {
        file: file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        // addressed as the file is now, so a place saved and the window opened again shows it
        picture: format!("{}k/{id}/0?v={}", crate::pictures::base(), crate::media_facts::address_stamp(&file)),
        width,
        height,
        place: dmd::places_in(&text).into_iter().find(|p| p.target == Target::Backglass),
        shape: dmd::dmd_shape(rom),
        grill: dmd::grill_height(&text),
    })
}

/// Where the score display fits the table's backglass picture: as the app places it by itself, in
/// its own window for it, else wherever VPX's "Detect position" finds a place; None where neither
/// finds one.
pub fn find_for(table: &Path, rom: Option<&str>) -> Result<Option<Place>, String> {
    let file = dmd::playing_file(table).ok_or("It has no backglass file that plays.")?;
    let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let shape = dmd::dmd_shape(rom);
    Ok(dmd::fitting_place(&text, Target::Backglass, shape).or_else(|| dmd::find_place(&text, Target::Backglass, shape)))
}

/// A place set by hand written into the table's backglass file, as VPX's menu writes one: the file
/// kept as it came first where the app hadn't kept it before, then remembered as holding places of
/// its own, which the app's own placing leaves as they are — as it leaves those VPX's menu sets.
pub fn save_by_hand(layout: &Layout, table: &Path, place: Place) -> Result<(), String> {
    if place.w <= 0 || place.h <= 0 || place.x < 0 || place.y < 0 {
        return Err("The score display needs a place inside the picture.".into());
    }
    let copies = copies_folder().ok_or("The app's data folder isn't there.")?;
    let targets = ::library::vpx::to_play(layout).and_then(|v| wanted(&v)).unwrap_or_default();
    with_kept(|kept| {
        place_by_hand(kept, layout, &copies, table, place, targets)?;
        save(kept);
        Ok(())
    })
}

/// `save_by_hand`'s work on what's remembered (`targets`: the places Settings › Screens asks for).
fn place_by_hand(kept: &mut Kept, layout: &Layout, copies: &Path, table: &Path, place: Place, targets: Vec<Target>) -> Result<(), String> {
    let file = dmd::playing_file(table).ok_or("It has no backglass file that plays.")?;
    let key = file.to_string_lossy().into_owned();
    let keep = (!kept.placed.iter().any(|p| p.file == key) && !kept.by_hand.contains_key(&key)).then(|| keep_path(copies, &layout.tables, &file));
    dmd::place_by_hand(&file, place, keep.as_deref()).map_err(|e| format!("{}: {e}", file.display()))?;
    if let Some(k) = keep {
        kept.by_hand.insert(key.clone(), k.to_string_lossy().into_owned());
    }
    let now = stamp(&file).ok_or_else(|| format!("{}: gone", file.display()))?;
    kept.seen.insert(key, Seen { size: now.0, modified: now.1, targets, look: "own".into() });
    Ok(())
}

/// This thread's work behind everything else, its reading and writing too.
#[cfg(windows)]
fn background() {
    use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN};
    // SAFETY: this thread's own handle
    let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
}

#[cfg(not(windows))]
fn background() {}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test library's tables (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test-6758`) copied to a
    /// scratch folder and each brought to the backglass's place as the background pass brings
    /// them: what each was found to be, its file placed in and kept as it came; looked at again,
    /// none changed; then every place taken out, each file as it came byte for byte.
    /// `cargo test --release -p app the_test_librarys_backglasses_are_placed_and_put_back --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_test_librarys_backglasses_are_placed_and_put_back() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-6758"));
        let work = std::env::temp_dir().join(format!("vpi-backglass-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let layout = Layout::under(&work);
        // each table's folder: its table and backglass files (no pictures or machine files needed)
        let mut tables = Vec::new();
        for entry in std::fs::read_dir(lib.join("Tables")).unwrap().flatten().filter(|e| e.path().is_dir()) {
            let to = layout.tables.join(entry.file_name());
            std::fs::create_dir_all(&to).unwrap();
            for f in std::fs::read_dir(entry.path()).unwrap().flatten().map(|e| e.path()) {
                if f.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx") || x.eq_ignore_ascii_case("directb2s")) {
                    std::fs::copy(&f, to.join(f.file_name().unwrap())).unwrap();
                    if f.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx")) {
                        tables.push(to.join(f.file_name().unwrap()));
                    }
                }
            }
        }
        tables.sort();
        let originals: Vec<(PathBuf, Vec<u8>)> = tables.iter().filter_map(|t| dmd::playing_file(t)).map(|f| (f.clone(), std::fs::read(&f).unwrap())).collect();
        let copies = work.join("copies");
        let mut kept = Kept { version: VERSION, ..Kept::default() };
        let wanted = [Target::Backglass];
        for t in &tables {
            let rom = ::library::table::read(t).ok().and_then(|f| f.game_name.filter(|_| f.pinmame));
            let done = settle(&mut kept, &layout, &copies, t, rom.as_deref(), &wanted).unwrap();
            let look = dmd::playing_file(t).and_then(|f| kept.seen.get(&f.to_string_lossy().into_owned()).map(|s| s.look.clone())).unwrap_or_else(|| "no backglass".into());
            println!("{}: {look}{}", t.file_stem().unwrap().to_string_lossy(), if done { " (placed now)" } else { "" });
        }
        for p in &kept.placed {
            println!("  {} at {:?}; kept as {}", Path::new(&p.file).file_name().unwrap().to_string_lossy(), p.places, p.kept);
            assert!(Path::new(&p.kept).is_file());
            assert_eq!(dmd::places_in(&std::fs::read(&p.file).unwrap()), p.places);
        }
        assert!(kept.placed.iter().any(|p| p.file.contains("Attack from Mars")), "Attack from Mars's backglass got no place");
        // looked at again: nothing to do
        assert!(tables.iter().all(|t| !to_look_at(&kept, t, &wanted)));
        for t in &tables {
            let rom = ::library::table::read(t).ok().and_then(|f| f.game_name.filter(|_| f.pinmame));
            assert!(!settle(&mut kept, &layout, &copies, t, rom.as_deref(), &wanted).unwrap());
        }
        // off: every place taken out, each file as it came
        put_all_back(&mut kept);
        assert!(kept.placed.is_empty());
        for (f, bytes) in &originals {
            assert!(std::fs::read(f).unwrap() == *bytes, "{} isn't as it came", f.display());
        }
        let _ = std::fs::remove_dir_all(&work);
    }

    /// A table and its backglass file in `dir`'s layout: a picture 640 × 480, white, its speaker
    /// panel the bottom 120 holding a black window. The layout, the table, the backglass file and
    /// the file's text.
    fn game(dir: &Path) -> (Layout, PathBuf, PathBuf, String) {
        let layout = Layout::under(dir);
        std::fs::create_dir_all(&layout.tables).unwrap();
        let table = layout.tables.join("Game (Maker 1993).vpx");
        std::fs::write(&table, b"").unwrap();
        let mut rgb = vec![255u8; 640 * 480 * 3];
        for y in 390..450 {
            for x in 170..470 {
                let p = (y * 640 + x) * 3;
                rgb[p..p + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_raw(640, 480, rgb).unwrap().write_to(&mut png, image::ImageFormat::Png).unwrap();
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut encoded = String::new();
        for c in png.get_ref().chunks(3) {
            let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
            for i in 0..4 {
                encoded.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
            }
        }
        let text = format!("<DirectB2SData Version=\"1.2\">\r\n  <GrillHeight Value=\"120\" />\r\n  <Images>\r\n    <BackglassImage Value=\"{encoded}\" />\r\n  </Images>\r\n</DirectB2SData>");
        let file = layout.tables.join("Game (Maker 1993).directb2s");
        std::fs::write(&file, &text).unwrap();
        (layout, table, file, text)
    }

    #[test]
    fn a_place_set_by_hand_stays_as_set() {
        let dir = std::env::temp_dir().join(format!("vpi-backglass-by-hand-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (layout, table, file, _) = game(&dir.join("first"));
        let copies = dir.join("copies");
        let wanted = [Target::Backglass];
        let mut kept = Kept { version: VERSION, ..Kept::default() };
        // the app places it by itself first, keeping the file as it came
        assert!(settle(&mut kept, &layout, &copies, &table, None, &wanted).unwrap());
        assert_eq!(kept.placed[0].places[0], Place { target: Target::Backglass, x: 200, y: 390, w: 240, h: 60 });
        // then set by hand: written in its place, no second copy kept
        let by_hand = Place { target: Target::Backglass, x: 12, y: 390, w: 240, h: 60 };
        place_by_hand(&mut kept, &layout, &copies, &table, by_hand, wanted.to_vec()).unwrap();
        assert_eq!(dmd::places_in(&std::fs::read(&file).unwrap()), vec![by_hand]);
        assert!(kept.by_hand.is_empty());
        // looked at again, by the background pass or before a game: left as set
        assert!(!to_look_at(&kept, &table, &wanted));
        assert!(!settle(&mut kept, &layout, &copies, &table, None, &wanted).unwrap());
        // the app's own places taken out (Settings › Screens' Off): this one left as set
        put_all_back(&mut kept);
        assert_eq!(dmd::places_in(&std::fs::read(&file).unwrap()), vec![by_hand]);
        // a file the app never placed: kept as it came the first time, never written over
        let (layout, table, file, original) = game(&dir.join("second"));
        let mut fresh = Kept { version: VERSION, ..Kept::default() };
        place_by_hand(&mut fresh, &layout, &copies, &table, by_hand, wanted.to_vec()).unwrap();
        let keep = PathBuf::from(&fresh.by_hand[&file.to_string_lossy().into_owned()]);
        assert_eq!(std::fs::read_to_string(&keep).unwrap(), original);
        place_by_hand(&mut fresh, &layout, &copies, &table, Place { x: 30, ..by_hand }, wanted.to_vec()).unwrap();
        assert_eq!(std::fs::read_to_string(&keep).unwrap(), original);
        assert_eq!(fresh.by_hand.len(), 1);
        assert_eq!(dmd::places_in(&std::fs::read(&file).unwrap()), vec![Place { x: 30, ..by_hand }]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_is_kept_where_it_lies_numbered_beside_another() {
        let dir = std::env::temp_dir().join(format!("vpi-backglass-keep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (copies, tables) = (dir.join("copies"), dir.join("Tables"));
        let file = tables.join("Game").join("Game.directb2s");
        assert_eq!(keep_path(&copies, &tables, &file), copies.join("Game").join("Game.directb2s"));
        std::fs::create_dir_all(copies.join("Game")).unwrap();
        std::fs::write(copies.join("Game").join("Game.directb2s"), b"x").unwrap();
        assert_eq!(keep_path(&copies, &tables, &file), copies.join("Game").join("Game (2).directb2s"));
        // a file outside the tables folder, by its name alone
        assert_eq!(keep_path(&copies, &tables, &dir.join("Other.directb2s")), copies.join("Other.directb2s"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_files_places_are_put_in_kept_and_taken_out_again() {
        let dir = std::env::temp_dir().join(format!("vpi-backglass-settle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let tables = dir.join("Tables");
        std::fs::create_dir_all(&tables).unwrap();
        let layout = Layout::under(&dir);
        let table = layout.tables.join("Game (Maker 1993).vpx");
        std::fs::create_dir_all(&layout.tables).unwrap();
        std::fs::write(&table, b"").unwrap();
        // a picture 640 × 480, white, a speaker panel 120 high holding a black window
        let mut rgb = vec![255u8; 640 * 480 * 3];
        for y in 390..450 {
            for x in 170..470 {
                let p = (y * 640 + x) * 3;
                rgb[p..p + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_raw(640, 480, rgb).unwrap().write_to(&mut png, image::ImageFormat::Png).unwrap();
        let encoded = {
            const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            for c in png.get_ref().chunks(3) {
                let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | u32::from(b) << (16 - 8 * i));
                for i in 0..4 {
                    s.push(if i <= c.len() { A[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
                }
            }
            s
        };
        let original = format!("<DirectB2SData Version=\"1.2\">\r\n  <GrillHeight Value=\"120\" />\r\n  <Images>\r\n    <BackglassImage Value=\"{encoded}\" />\r\n  </Images>\r\n</DirectB2SData>");
        let file = layout.tables.join("Game (Maker 1993).directb2s");
        std::fs::write(&file, &original).unwrap();
        let copies = dir.join("copies");
        // what settle does, its copies kept in a folder of the test's own
        let mut kept = Kept { version: VERSION, ..Kept::default() };
        let keep = keep_path(&copies, &layout.tables, &file);
        let text = std::fs::read(&file).unwrap();
        let place = dmd::fitting_place(&text, Target::Backglass, 4.0).unwrap();
        assert_eq!([place.x, place.y, place.w, place.h], [200, 390, 240, 60]);
        dmd::place_in(&file, &[place], &keep).unwrap();
        let now = stamp(&file).unwrap();
        kept.placed.push(Placed { file: file.to_string_lossy().into_owned(), size: now.0, modified: now.1, kept: keep.to_string_lossy().into_owned(), places: vec![place] });
        assert!(!to_look_at(&kept, &table, &[Target::Backglass]));
        assert!(to_look_at(&kept, &table, &[Target::Backglass, Target::ScoreView]));
        // renamed beside it (another backglass chosen to play), then everything put back
        let aside = layout.tables.join("Game (Maker 1993) (1).directb2s");
        std::fs::rename(&file, &aside).unwrap();
        put_all_back(&mut kept);
        assert!(kept.placed.is_empty());
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), original);
        assert!(!keep.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
