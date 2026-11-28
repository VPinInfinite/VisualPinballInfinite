//! Our add-on for VPX 10.8.1 (`engine/vpx-addon`), which follows a table's achievements while VPX
//! plays it: put where VPX loads its add-ons (its `plugins` folder, one folder each, a
//! `plugin.cfg` beside the library), switched on in VPX's settings (`[Plugin.VPIAchievements]`
//! `Enable`), and what it reads of a ROM machine given to VPX's PinMAME add-on, which shares a
//! machine's game state only where it finds the machine's memory map (its `memmaps` folder: an
//! `index.json` naming each ROM's map, the maps' platforms in `platforms`) — the map written from
//! the Pinball Memory Maps this program carries (`pinmame-nvram`), its game state alone.

use std::path::{Path, PathBuf};

use crate::vpx::Vpx;

/// The add-on's ID, as VPX names its settings and its functions (`VPIAchievementsPluginLoad`).
pub const ID: &str = "VPIAchievements";
/// Its folder in VPX's `plugins`, and its library there.
pub const FOLDER: &str = "vpi-achievements";
pub const LIBRARY: &str = "plugin-vpi-achievements64.dll";
/// Where our maps go in a `memmaps` folder, beside any others there.
const OURS: &str = "vpi";

/// The add-on's library: beside this program under the name VPX's `plugin.cfg` gives it, as the
/// release carries it, else as Cargo builds it.
pub fn built() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    [LIBRARY, "vpi_vpx_addon.dll"].iter().map(|n| exe.with_file_name(n)).find(|p| p.is_file())
}

/// Its `plugin.cfg`, as VPX reads one.
pub fn cfg() -> String {
    format!(
        "[configuration]\r\nid = \"{ID}\"\r\nname = \"Achievements\"\r\ndescription = \"A table's achievements followed as it plays, for Visual Pinball Infinite\"\r\nauthor = \"Visual Pinball Infinite\"\r\nversion = \"{}\"\r\n\r\n[libraries]\r\nwindows.x64 = \"{LIBRARY}\"\r\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// Its folder in VPX's `plugins`.
pub fn folder(vpx: &Vpx) -> Option<PathBuf> {
    Some(vpx.program.parent()?.join("plugins").join(FOLDER))
}

/// Whether two files hold the same bytes.
fn same(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// The add-on put in VPX's `plugins` folder, or brought up to date there: where it is. A library
/// VPX holds open (a game playing) is left as it is, if one's there.
pub fn put_in(vpx: &Vpx) -> Result<PathBuf, String> {
    let from = built().ok_or("the achievements add-on isn't built beside the app")?;
    let dir = folder(vpx).ok_or("VPX's folder isn't known")?;
    copy_into(&from, &dir)?;
    Ok(dir)
}

/// The add-on's library `from`, and its `plugin.cfg`, put in `dir`, or brought up to date there.
fn copy_into(from: &Path, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let to = dir.join(LIBRARY);
    if !same(from, &to) {
        let new = dir.join(format!("{LIBRARY}.new"));
        let copied = std::fs::copy(from, &new).and_then(|_| std::fs::rename(&new, &to));
        if let Err(e) = copied {
            let _ = std::fs::remove_file(&new);
            if !to.is_file() {
                return Err(format!("the achievements add-on couldn't be put in VPX's add-ons: {e}"));
            }
        }
    }
    let cfg_file = dir.join("plugin.cfg");
    let text = cfg();
    if std::fs::read_to_string(&cfg_file).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&cfg_file, text).map_err(|e| format!("{}: {e}", cfg_file.display()))?;
    }
    Ok(())
}

/// The first VPX build that loads add-ons from a folder of the player's own as well as its own:
/// `plugins` in its preferences folder, beside its settings (VPX's 81f6153), its own folder's first.
pub const PLAYERS_FOLDER_FROM: u16 = 6503;

/// Whether `vpx` loads add-ons from the player's own folder.
pub fn takes_players_folder(vpx: &Vpx) -> bool {
    vpx.has_addons() && vpx.version >= [10, 8, 1, PLAYERS_FOLDER_FROM]
}

/// The add-on put where `vpx` loads it for the player: for a VPX that loads add-ons from the
/// player's own folder too, there (`plugins` in its preferences folder, `prefs` where the app
/// names another), its copy in VPX's own folder taken out, as VPX would load that one first — so
/// VPX's folder is left as it came; for one that doesn't, VPX's own `plugins` (`put_in`). Where
/// it is.
pub fn put_in_for(vpx: &Vpx, prefs: Option<&Path>) -> Result<PathBuf, String> {
    let from = built().ok_or("the achievements add-on isn't built beside the app")?;
    place_for(&from, vpx, prefs)
}

fn place_for(from: &Path, vpx: &Vpx, prefs: Option<&Path>) -> Result<PathBuf, String> {
    let own = folder(vpx).ok_or("VPX's folder isn't known")?;
    if !takes_players_folder(vpx) {
        copy_into(from, &own)?;
        return Ok(own);
    }
    // its preferences folder made as VPX's first start makes it, where it isn't there yet: made
    // empty, VPX would bring in none of an older version's settings
    let dir = crate::vpx::prefs_ready(vpx, prefs)?.join("plugins").join(FOLDER);
    copy_into(from, &dir)?;
    // our files alone taken out of VPX's folder, and the folder if that leaves it empty; where
    // they can't be (VPX holding them), brought up to date there instead, so whichever loads is
    // this one
    if own.is_dir() {
        let gone = [LIBRARY.to_string(), format!("{LIBRARY}.new"), "plugin.cfg".to_string()].iter().all(|f| {
            let p = own.join(f);
            !p.exists() || std::fs::remove_file(&p).is_ok()
        });
        if gone {
            let _ = std::fs::remove_dir(&own);
        } else {
            copy_into(from, &own)?;
        }
    }
    Ok(dir)
}

/// The `memmaps` folder VPX's PinMAME add-on reads for `table` (its `PinMAMEPlugin.cpp`): the table
/// folder's `pinmame\memmaps` where there's one, else the `memmaps` of PinMAME's folder as its
/// settings say (`pinmame_path`, `[Plugin.PinMAME]` `PinMAMEPath`). With that unset, the add-on
/// plays only a machine whose ROMs are in the table folder's `pinmame\roms`: that `pinmame`
/// folder's, made; None for a table kept beside others, whose machine VPX can't start.
pub fn memory_maps_folder(table: &Path, pinmame_path: Option<&str>) -> Option<PathBuf> {
    let own = table.parent()?.join("pinmame");
    let beside = own.join("memmaps");
    if beside.is_dir() {
        return Some(beside);
    }
    match pinmame_path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => Some(PathBuf::from(p).join("memmaps")),
        None => own.join("roms").is_dir().then_some(beside),
    }
}

/// The community's memory maps a VPX's own PinMAME add-on carries (from build 6625, in its
/// `plugins\pinmame\memmaps`), which that add-on reads where no `memmaps` folder is beside the table
/// or in PinMAME's folder; None for a VPX carrying none (5436).
pub fn own_maps(vpx: &Vpx) -> Option<PathBuf> {
    let dir = vpx.program.parent()?.join("plugins").join("pinmame").join("memmaps");
    dir.join("index.json").is_file().then_some(dir)
}

/// Whether a `memmaps` folder's index names a ROM's map.
pub fn maps_know(dir: &Path, rom: &str) -> bool {
    let index: serde_json::Value = std::fs::read(dir.join("index.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    index.get(rom.to_ascii_lowercase().as_str()).is_some()
}

/// Whether a `memmaps` folder holds only what `put_memory_map` makes — our maps under `vpi\`, an
/// index naming only them, the platforms added for them — which hides a VPX's own maps from its
/// PinMAME add-on; false where anything else is there.
pub fn only_ours(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else { return false };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_ascii_lowercase();
        if !matches!(name.as_str(), "index.json" | "platforms" | OURS) {
            return false;
        }
    }
    let index: serde_json::Value = std::fs::read(dir.join("index.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    index.as_object().is_some_and(|m| m.values().all(|v| v.as_str().is_some_and(|s| s.replace('\\', "/").starts_with(&format!("{OURS}/")))))
}

/// A ROM's memory map and its platform, from the maps carried here, the map cut to what our add-on
/// reads through the PinMAME add-on — its game state and its high scores: None where no map knows
/// the ROM, or knows no game state of it.
pub fn map_of(rom: &str) -> Option<(serde_json::Value, String, serde_json::Value)> {
    // the maps' reader finds a ROM's map by a memory file's name; one that's empty serves
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("vpi-map-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let nv = dir.join(format!("{}.nv", rom.to_ascii_lowercase()));
    std::fs::write(&nv, []).ok()?;
    let opened = pinmame_nvram::Nvram::open(&nv);
    let _ = std::fs::remove_dir_all(&dir);
    let nv = opened.ok()??;
    let whole = serde_json::to_value(&nv.map).ok()?;
    let mut map = serde_json::Map::new();
    for key in ["_fileformat", "_metadata", "game_state", "high_scores"] {
        if let Some(v) = whole.get(key) {
            map.insert(key.into(), v.clone());
        }
    }
    map.get("game_state")?;
    let platform = nv.map._metadata.platform.clone();
    Some((serde_json::Value::Object(map), platform, serde_json::to_value(&nv.platform).ok()?))
}

/// A ROM's memory map put where VPX's PinMAME add-on reads it for `table`, beside any maps there:
/// ours under `vpi\`, named in the folder's `index.json` unless it names another map for the ROM
/// already, its platform added where it's missing. Where it went; None where no map knows the ROM.
pub fn put_memory_map(table: &Path, rom: &str, pinmame_path: Option<&str>) -> Result<Option<PathBuf>, String> {
    let Some((map, platform_name, platform)) = map_of(rom) else { return Ok(None) };
    // nowhere VPX's PinMAME would read it: no machine it can start
    let Some(folder) = memory_maps_folder(table, pinmame_path) else { return Ok(None) };
    let rom = rom.to_ascii_lowercase();
    let index_file = folder.join("index.json");
    let mut index: serde_json::Map<String, serde_json::Value> = std::fs::read(&index_file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let ours = format!("{OURS}/{rom}.json");
    // another map named for the ROM, there: that one's read
    if let Some(theirs) = index.get(&rom).and_then(serde_json::Value::as_str)
        && theirs != ours
        && folder.join(theirs).is_file()
    {
        return Ok(Some(folder));
    }
    let write = |path: &Path, v: &serde_json::Value| -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
        if std::fs::read(path).ok().as_deref() == Some(text.as_slice()) {
            return Ok(());
        }
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    };
    write(&folder.join(OURS).join(format!("{rom}.json")), &map)?;
    let platform_file = folder.join("platforms").join(format!("{platform_name}.json"));
    if !platform_file.is_file() {
        write(&platform_file, &platform)?;
    }
    if index.get(&rom).and_then(serde_json::Value::as_str) != Some(ours.as_str()) {
        index.insert(rom, serde_json::Value::String(ours));
        write(&index_file, &serde_json::Value::Object(index))?;
    }
    Ok(Some(folder))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vpi-addon-maps-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_vpxs_own_maps_and_a_folder_of_ours_alone_told() {
        let dir = std::env::temp_dir().join(format!("vpi-own-maps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let program = dir.join("VPinballX_BGFX64.exe");
        let carried = dir.join("plugins").join("pinmame").join("memmaps");
        std::fs::create_dir_all(&carried).unwrap();
        let vpx = Vpx { program: program.clone(), version: [10, 8, 1, 6681] };
        assert_eq!(own_maps(&vpx), None, "no index: none carried");
        std::fs::write(carried.join("index.json"), r#"{ "afm_113b": "maps/williams/wpc/afm_113.map.json" }"#).unwrap();
        assert_eq!(own_maps(&vpx), Some(carried.clone()));
        assert!(maps_know(&carried, "AFM_113b") && !maps_know(&carried, "mm_109c"));
        // a folder of ours alone, then with a map someone else put there
        let ours = dir.join("VPinMAME").join("memmaps");
        std::fs::create_dir_all(ours.join("vpi")).unwrap();
        std::fs::create_dir_all(ours.join("platforms")).unwrap();
        std::fs::write(ours.join("index.json"), r#"{ "afm_113b": "vpi/afm_113b.json", "mm_109c": "vpi/mm_109c.json" }"#).unwrap();
        assert!(only_ours(&ours));
        std::fs::write(ours.join("index.json"), r#"{ "afm_113b": "vpi/afm_113b.json", "tz_94h": "maps/williams/wpc/tz_94h.map.json" }"#).unwrap();
        assert!(!only_ours(&ours), "another map named");
        std::fs::write(ours.join("index.json"), r#"{ "afm_113b": "vpi/afm_113b.json" }"#).unwrap();
        std::fs::create_dir_all(ours.join("maps")).unwrap();
        assert!(!only_ours(&ours), "another folder");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_roms_map_goes_where_vpxs_pinmame_reads_it_beside_others() {
        let dir = scratch("put");
        let table = dir.join("Tables").join("Attack from Mars").join("afm.vpx");
        std::fs::create_dir_all(table.parent().unwrap()).unwrap();
        // PinMAME's folder as the settings name it
        let pinmame = dir.join("VPinMAME");
        let folder = put_memory_map(&table, "afm_113b", pinmame.to_str()).unwrap().unwrap();
        assert_eq!(folder, pinmame.join("memmaps"));
        let index: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("index.json")).unwrap()).unwrap();
        assert_eq!(index["afm_113b"], "vpi/afm_113b.json");
        let map: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("vpi").join("afm_113b.json")).unwrap()).unwrap();
        assert_eq!(map["_metadata"]["platform"], "williams-wpc-12K");
        assert_eq!(map["game_state"]["scores"][0]["label"], "Player 1");
        assert!(map.get("audits").is_none());
        assert!(folder.join("platforms").join("williams-wpc-12K.json").is_file());
        // another ROM added beside it; a map someone else put in for a ROM kept
        std::fs::write(folder.join("theirs.json"), "{}").unwrap();
        let mut index: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(&std::fs::read(folder.join("index.json")).unwrap()).unwrap();
        index.insert("mm_109c".into(), "theirs.json".into());
        std::fs::write(folder.join("index.json"), serde_json::to_vec(&index).unwrap()).unwrap();
        put_memory_map(&table, "mm_109c", pinmame.to_str()).unwrap();
        put_memory_map(&table, "tz_92", pinmame.to_str()).unwrap();
        let index: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("index.json")).unwrap()).unwrap();
        assert_eq!((index["afm_113b"].as_str(), index["mm_109c"].as_str(), index["tz_92"].as_str()), (Some("vpi/afm_113b.json"), Some("theirs.json"), Some("vpi/tz_92.json")));
        // PinMAME's folder unset: a table kept beside others gets none, as VPX can't start its
        // machine; a table with its own ROMs gets its own folder's
        let shared = dir.join("Tables").join("Black Knight (Williams 1980).vpx");
        assert_eq!(memory_maps_folder(&shared, None), None);
        assert_eq!(put_memory_map(&shared, "afm_113b", Some(" ")).unwrap(), None);
        assert!(!dir.join("Tables").join("pinmame").exists());
        std::fs::create_dir_all(table.parent().unwrap().join("pinmame").join("roms")).unwrap();
        assert_eq!(memory_maps_folder(&table, None), Some(table.parent().unwrap().join("pinmame").join("memmaps")));
        // a table with its own memmaps folder: that one; no map for a ROM: nothing
        std::fs::create_dir_all(table.parent().unwrap().join("pinmame").join("memmaps")).unwrap();
        assert_eq!(memory_maps_folder(&table, pinmame.to_str()), Some(table.parent().unwrap().join("pinmame").join("memmaps")));
        assert_eq!(put_memory_map(&table, "nosuchrom_x", None).unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The add-on following a game of Attack from Mars in VPX, for real, unseen: VPX build 5436 as
    /// `tools/vpx-tester` builds it (it presses keys by itself, `VPI_KEYS`), the latency bench's
    /// copy of the table and its PinMAME folder (`cargo run --release -p rom-latency` makes them)
    /// copied into a scratch folder, a pack of our own. `cargo build --release -p vpx-addon`, then
    /// `cargo test --release -p library --lib the_addon_follows_a_game_in_vpx -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_addon_follows_a_game_in_vpx() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let tester = repo.join("vpx-build").join("vpinball-5436").join("build").join("Release").join("VPinballX_BGFX64.exe");
        let bench = repo.join("results").join("rom-latency").join("work").join("vpx");
        let name = "Attack from Mars (Bally 1995) JPSalas 6.0.0.vpx";
        let work = scratch("game");
        let table = work.join(name);
        std::fs::copy(bench.join(name), &table).unwrap();
        for (dir, file) in [("roms", "afm_113b.zip"), ("nvram", "afm_113b.nv"), ("cfg", "afm_113b.cfg")] {
            std::fs::create_dir_all(work.join("pinmame").join(dir)).unwrap();
            std::fs::copy(bench.join("pinmame").join(dir).join(file), work.join("pinmame").join(dir).join(file)).unwrap();
        }
        let vpx = Vpx { program: std::fs::canonicalize(&tester).unwrap(), version: [10, 8, 1, 5436] };
        println!("add-on in {}", put_in(&vpx).unwrap().display());
        println!("map in {}", put_memory_map(&table, "afm_113b", None).unwrap().unwrap().display());
        // a pack: the game begun, any points, the coils firing five times, a second ball
        let coils: Vec<serde_json::Value> = (1..=28).map(|n| serde_json::json!({ "solenoid": n })).collect();
        let pack = serde_json::json!({ "format": "vpi-achievements", "formatVersion": 1,
            "pack": { "id": "0c9a7b1e-2d3f-4a5b-8c6d-7e8f9a0b1c2d", "name": "Check", "version": "1.0" },
            "for": { "game": { "name": "Attack from Mars" } },
            "achievements": [
                { "id": "start", "name": "Begun", "about": "a", "points": 1, "since": "1.0", "when": { "game": "started" } },
                { "id": "points", "name": "Scored", "about": "b", "points": 1, "since": "1.0", "when": { "score": { "atLeast": 10 } } },
                { "id": "coils", "name": "Coils", "about": "c", "points": 1, "since": "1.0", "when": { "count": { "any": coils }, "times": 5, "within": "game" } },
                { "id": "ball2", "name": "Second Ball", "about": "d", "points": 1, "since": "1.0", "when": { "ball": 2 } }
            ] });
        std::fs::create_dir_all(work.join("pack")).unwrap();
        std::fs::write(work.join("pack").join("achievements.json"), serde_json::to_vec_pretty(&pack).unwrap()).unwrap();
        // the sound's path taken, all but silent
        let asked = serde_json::json!({ "mode": "hardcore", "table": name, "packs": [work.join("pack")], "earned": work.join("earned.json"), "result": work.join("result.json"), "sound": { "on": true, "gain": 0.001 } });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        let ini = "[Player]\r\nPlayfieldFullScreen = 0\r\nPlayfieldWndX = 0\r\nPlayfieldWndY = 0\r\nPlayfieldWidth = 640\r\nPlayfieldHeight = 480\r\nBGSet = 0\r\nMaxFramerate = 60.000000\r\nSoundVolume = 0\r\nMusicVolume = 0\r\n[Controller]\r\nForceDisableB2S = 1\r\n[Plugin.PinMAME]\r\nEnable = 1\r\n[Plugin.VPIAchievements]\r\nEnable = 1\r\n";
        std::fs::write(prefs.join("VPinballX.ini"), ini).unwrap();
        // two coins once the machine has tested itself, start, the ball launched, then the
        // flippers in turn
        let mut keys = vec![(16_000, 0x06, true), (16_120, 0x06, false), (17_000, 0x06, true), (17_120, 0x06, false), (19_000, 0x02, true), (19_150, 0x02, false)];
        for t in [23_000u64, 30_000, 40_000, 50_000] {
            keys.extend([(t, 0x1C, true), (t + 200, 0x1C, false)]);
        }
        // paused a moment near the end, and on again: where the achievements stand is shown
        keys.extend([(70_000, 0x19, true), (70_100, 0x19, false), (71_500, 0x19, true), (71_600, 0x19, false)]);
        let mut t = 24_000u64;
        while t < 75_000 {
            let key = if (t / 1000) % 2 == 0 { 0x2A } else { 0x36 };
            keys.extend([(t, key, true), (t + 250, key, false)]);
            t += 900;
        }
        let keys = keys.iter().map(|(ms, code, down)| format!("{ms}:{code:02X}:{}", if *down { 'd' } else { 'u' })).collect::<Vec<_>>().join(",");
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
            std::env::set_var("VPI_PREF_PATH", &prefs);
            std::env::set_var("VPI_KEYS", keys);
            std::env::set_var("VPI_QUIT_AFTER_MS", "80000");
            std::env::set_var("VPI_SCREENSHOTS", work.join("shots"));
            std::env::set_var("VPI_SCREENSHOT_AT_MS", "60000");
            std::env::set_var("VPI_BLOCK_COM", "B2S.Server,VPinMAME.Controller,VPinMAME.WSHDlg,VPROC.Controller,FlexDMD.FlexDMD,UltraDMD.DMDObject,PinUpPlayer.PinDisplay");
        }
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-Ini".into(), prefs.join("VPinballX.ini").into(), "-Play".into(), table.clone().into()];
        let code = crate::vpx_test::unseen(&vpx.program, &args, 240).unwrap();
        println!("VPX ended: {code:?}");
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        let shown: Vec<&str> = log.lines().filter(|l| !l.contains("Lamp") && !l.contains("Solenoid") && !l.contains("Switch")).collect();
        println!("{}", shown.join("\n"));
        println!("{} lamp, solenoid and switch changes", log.lines().count() - shown.len());
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("Plugin") || l.contains("plugin") || l.contains("memmap") || l.contains("rror")) {
            println!("vpx: {l}");
        }
        let result: serde_json::Value = serde_json::from_slice(&std::fs::read(work.join("result.json")).unwrap()).unwrap();
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        assert_eq!(result["followed"], true);
        assert!(result["earned"].as_array().unwrap().iter().any(|e| e == "Begun"));
        assert!(log.contains("sound: Ours"), "no sound played");
        let shots: Vec<String> = std::fs::read_dir(work.join("shots")).into_iter().flatten().flatten().map(|e| format!("{} ({} bytes)", e.file_name().to_string_lossy(), e.metadata().map(|m| m.len()).unwrap_or(0))).collect();
        println!("screenshots: {shots:?}");
        assert!(shots.iter().any(|s| s.contains(" playfield.png")), "no screenshot of the playfield");
        // the picture kept for a look, outside the scratch folder
        if let Some(first) = std::fs::read_dir(work.join("shots")).into_iter().flatten().flatten().next() {
            let _ = std::fs::copy(first.path(), std::env::temp_dir().join("vpi-addon-last-screenshot.png"));
        }
        assert!(log.lines().any(|l| l.starts_with("notice: Achievements:") && l.contains("earned")), "no standing shown when paused");
        // the add-on taken out of the test build again
        let _ = std::fs::remove_dir_all(folder(&vpx).unwrap());
        let _ = std::fs::remove_dir_all(&work);
    }

    /// VPX 10.8.1 as its own GitHub release ships it loading the add-on, and its PinMAME add-on
    /// sharing a machine's game through the map put in: Attack from Mars in the test library
    /// (`tools/vpx-test`), its attract mode run unseen, the add-on hearing the machine.
    /// `cargo build --release -p vpx-addon`, then `cargo test --release -p library --lib
    /// the_addon_hears_a_machine_in_vpx_as_released -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_addon_hears_a_machine_in_vpx_as_released() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        // another test library where `VPI_TEST_LIBRARY` names one, a newer VPX in it
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        put_in(&vpx).unwrap();
        let table = lib.join("Tables").join("Attack from Mars (Bally 1995)").join("Attack from Mars (Bally 1995).vpx");
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let entries = vpi_settings::parse_ini(&prefs_text);
        let folder = put_memory_map(&table, "afm_113b", vpi_settings::get(&entries, "Plugin.PinMAME", "PinMAMEPath")).unwrap().unwrap();
        println!("map in {}", folder.display());
        let work = scratch("released");
        let pack = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let asked = serde_json::json!({ "mode": "hardcore", "packs": [pack], "earned": work.join("earned.json"), "result": work.join("result.json") });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
        }
        let settings = format!("{prefs_text}\r\n[Plugin.{ID}]\r\nEnable = 1\r\n");
        let (log, code) = crate::vpx_test::run(&vpx, &table, &settings, "15").unwrap();
        println!("VPX ended: {code:?}");
        for l in log.lines().filter(|l| l.contains("VPIAchievements") || l.contains("Plugin PinMAME") || l.contains("rror")) {
            println!("vpx: {l}");
        }
        println!("{}", std::fs::read_to_string(work.join("addon.log")).unwrap_or_default());
        let result: serde_json::Value = serde_json::from_slice(&std::fs::read(work.join("result.json")).unwrap()).unwrap();
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
        assert_eq!(result["machine"], true);
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn the_addon_goes_in_the_players_own_folder_where_vpx_takes_one() {
        let work = scratch("players-folder");
        let from = work.join("built.dll");
        std::fs::write(&from, b"the add-on").unwrap();
        let program = work.join("VPX").join("VPinballX_BGFX64.exe");
        let prefs = work.join("prefs");
        // build 5436: VPX's own folder
        let old = Vpx { program: program.clone(), version: [10, 8, 1, 5436] };
        assert!(!takes_players_folder(&old));
        let own = place_for(&from, &old, Some(&prefs)).unwrap();
        assert_eq!(own, work.join("VPX").join("plugins").join(FOLDER));
        assert!(own.join(LIBRARY).is_file() && own.join("plugin.cfg").is_file());
        // a later build: the player's own, the copy in VPX's folder taken out (VPX loads that first)
        let new = Vpx { program, version: [10, 8, 1, 6625] };
        assert!(takes_players_folder(&new));
        let theirs = place_for(&from, &new, Some(&prefs)).unwrap();
        assert_eq!(theirs, prefs.join("plugins").join(FOLDER));
        assert_eq!(std::fs::read(theirs.join(LIBRARY)).unwrap(), b"the add-on");
        assert!(std::fs::read_to_string(theirs.join("plugin.cfg")).unwrap().contains(&format!("id = \"{ID}\"")));
        assert!(!own.exists());
        // something else in that folder: kept, and the folder with it
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("notes.txt"), b"theirs").unwrap();
        std::fs::write(own.join(LIBRARY), b"an old one").unwrap();
        place_for(&from, &new, Some(&prefs)).unwrap();
        assert!(own.join("notes.txt").is_file() && !own.join(LIBRARY).exists());
        let _ = std::fs::remove_dir_all(&work);
    }

    /// The balls in play heard in VPX through a line of the table's own script, unseen and silent:
    /// Attack from Mars in a test library of VPX's newer build (`VPI_TEST_LIBRARY`, else
    /// `vpx-build\vpx-test-7001`) in VPX's capture mode on a desktop never shown, its sound off, a
    /// copy of its sample pack given an achievement for a ball in play. VPX plays only with its
    /// window focused, which a desktop never shown never gives, and its capture mode takes no input
    /// and races the machine past a coin's pulse; so a line of a check's own moves one of the
    /// trough's balls onto the playfield (`VPI_ACHIEVEMENTS_CHECK_SCRIPT`), as a ball served would
    /// leave the trough. Our routine says it's there, the trough's balls aren't counted, the ball
    /// moved is, and the count is told to the achievements (which count it only while a game is
    /// on: capture mode starts none). `cargo build --release -p vpx-addon`, then `cargo test --release -p
    /// library --lib balls_in_play_are_heard_in_vpx -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn balls_in_play_are_heard_in_vpx() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-7001"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        let table = lib.join("Tables").join("Attack from Mars (Bally 1995)").join("Attack from Mars (Bally 1995).vpx");
        let work = scratch("balls");
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        let at = put_in_for(&vpx, Some(&prefs)).unwrap();
        println!("VPX {:?}; add-on in {}", vpx.version, at.display());
        // the sample pack, copied, with an achievement for a ball in play first
        let pack = work.join("pack");
        let sample = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let copy = |from: &Path, to: &Path| -> std::io::Result<()> {
            let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
            while let Some((f, t)) = stack.pop() {
                std::fs::create_dir_all(&t)?;
                for e in std::fs::read_dir(&f)?.flatten() {
                    if e.path().is_dir() {
                        stack.push((e.path(), t.join(e.file_name())));
                    } else {
                        std::fs::copy(e.path(), t.join(e.file_name()))?;
                    }
                }
            }
            Ok(())
        };
        copy(&sample, &pack).unwrap();
        let file = pack.join("achievements.json");
        let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        let one = serde_json::json!({ "id": "one-in-play", "name": "One in Play", "about": "Have a ball in play", "points": 1, "since": "1.0", "when": { "ballsInPlay": { "atLeast": 1 } } });
        json["achievements"].as_array_mut().unwrap().insert(0, one);
        std::fs::write(&file, serde_json::to_vec_pretty(&json).unwrap()).unwrap();
        let asked = serde_json::json!({ "mode": "hardcore", "packs": [pack], "earned": work.join("earned.json"), "result": work.join("result.json"), "sound": { "on": false } });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        // the test library's settings: a small playfield window, no other window, no sound
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
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
            set("Backglass", "BackglassOutput", "0"),
            set("ScoreView", "ScoreViewOutput", "0"),
            set("Topper", "TopperOutput", "0"),
            set(&format!("Plugin.{ID}"), "Enable", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
            // once capture mode's fast first 30 seconds are over, one of the trough's balls moved up the
            // playfield, rolling
            std::env::set_var(
                "VPI_ACHIEVEMENTS_CHECK_SCRIPT",
                "33000=Dim vpiCheckBall: For Each vpiCheckBall In GetBalls: vpiCheckBall.X = 450: vpiCheckBall.Y = 1200: vpiCheckBall.VelY = -8: Exit For: Next",
            );
        }
        let capture = table.parent().unwrap().join("Capture");
        let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let had = capture.is_dir();
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "12".into(), "1".into(), table.clone().into(), "noloop".into()];
        let (code, boxes) = crate::vpx_test::unseen_reading(&vpx.program, &args, 180).unwrap();
        println!("VPX ended: {code:?}; its message boxes: {boxes:?}");
        // capture mode's pictures taken out of the table's folder
        for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
            if p.is_file() {
                let _ = std::fs::remove_file(&p);
            }
        }
        if !had {
            let _ = std::fs::remove_dir_all(&capture);
        }
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        println!("{}", log.lines().filter(|l| !l.contains("Lamp") && !l.contains("Solenoid") && !l.contains("Switch")).collect::<Vec<_>>().join("\n"));
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("VPIAchievements") || l.contains("rror") || l.contains("overriden") || l.contains("cript")) {
            println!("vpx: {l}");
        }
        let result = std::fs::read_to_string(work.join("result.json")).unwrap_or_default();
        println!("result: {result}");
        assert!(log.contains("our routine put in the table's script: it's there"), "our routine didn't say it's there");
        assert!(log.contains(" 1 in play"), "no ball was counted in play");
        assert!(log.contains("BallsInPlay(1)"), "the achievements weren't told a ball's in play");
        // and the table never showed an error of ours
        assert!(boxes.is_empty(), "VPX showed {boxes:?}");
        assert!(!vpx_log.contains(" error on line "), "a script error in VPX's log");
        assert!(result.contains("ballsInPlay"), "the balls weren't said to be heard");
        assert_eq!(code, Some(0), "VPX didn't end cleanly");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// A table's lights heard in VPX, unseen and silent: Leprechaun King, a table with no ROM, in a
    /// test library of VPX's newer build (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test-7001`) in
    /// VPX's capture mode, a pack reading three of its lights by name followed. VPX shares a table's
    /// lights with add-ons from build 6148; our add-on reads those a pack names and tells the
    /// achievements each change. Lines of a check's own set the shoot-again light blinking, then
    /// off, in the table's script, once capture mode's fast first 30 seconds are over: the changes
    /// are told (capture mode starts no game, so nothing is earned). `cargo build --release -p
    /// vpx-addon`, then `cargo test --release -p library --lib a_tables_lights_are_heard_in_vpx --
    /// --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_tables_lights_are_heard_in_vpx() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-7001"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.version >= [10, 8, 1, 6148], "{:?} shares no lights", vpx.version);
        let table = lib.join("Tables").join("Leprechaun King (Orbital 2020)").join("Leprechaun King (Orbital 2020).vpx");
        let work = scratch("lights");
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        let at = put_in_for(&vpx, Some(&prefs)).unwrap();
        println!("VPX {:?}; add-on in {}", vpx.version, at.display());
        let pack = work.join("pack");
        std::fs::create_dir_all(&pack).unwrap();
        let json = serde_json::json!({ "format": "vpi-achievements", "formatVersion": 1,
            "pack": { "id": "7c2b4e6a-3d5f-4b7a-8c9d-2e3f4a5b6c7d", "name": "Lights check", "version": "1.0" },
            "for": { "game": { "name": "Leprechaun King" } },
            "achievements": [
                { "id": "again", "name": "Again", "about": "The shoot-again light blinking", "points": 1, "since": "1.0", "when": { "light": "LightShootAgain", "turns": "blinking" } },
                { "id": "rainbow", "name": "Rainbow", "about": "Two rainbow lights lit at once", "points": 5, "since": "1.0", "when": { "all": [{ "light": "rain1", "is": "on" }, { "light": "rain2", "is": "on" }] } }
            ] });
        std::fs::write(pack.join("achievements.json"), serde_json::to_vec_pretty(&json).unwrap()).unwrap();
        let asked = serde_json::json!({ "mode": "hardcore", "packs": [pack], "earned": work.join("earned.json"), "result": work.join("result.json"), "sound": { "on": false } });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        // the test library's settings: a small playfield window, no other window, no sound
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
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
            set("Backglass", "BackglassOutput", "0"),
            set("ScoreView", "ScoreViewOutput", "0"),
            set("Topper", "TopperOutput", "0"),
            set(&format!("Plugin.{ID}"), "Enable", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
            std::env::set_var("VPI_ACHIEVEMENTS_CHECK_SCRIPT", "33000=LightShootAgain.State = 2|36000=LightShootAgain.State = 0");
        }
        let capture = table.parent().unwrap().join("Capture");
        let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let had = capture.is_dir();
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "12".into(), "1".into(), table.clone().into(), "noloop".into()];
        let (code, boxes) = crate::vpx_test::unseen_reading(&vpx.program, &args, 180).unwrap();
        println!("VPX ended: {code:?}; its message boxes: {boxes:?}");
        for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
            if p.is_file() {
                let _ = std::fs::remove_file(&p);
            }
        }
        if !had {
            let _ = std::fs::remove_dir_all(&capture);
        }
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        println!("{log}");
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("VPIAchievements") || l.contains("rror")) {
            println!("vpx: {l}");
        }
        let result = std::fs::read_to_string(work.join("result.json")).unwrap_or_default();
        println!("result: {result}");
        assert!(log.contains("lights: 3 of the table's read"), "the table's lights weren't found");
        assert!(log.contains("name: \"LightShootAgain\", state: Blinking"), "the shoot-again light blinking wasn't told");
        assert!(log.contains("name: \"LightShootAgain\", state: Off"), "the shoot-again light going out wasn't told");
        assert!(result.contains("\"light\""), "the lights weren't said to be heard");
        assert!(boxes.is_empty(), "VPX showed {boxes:?}");
        assert!(!vpx_log.contains(" error on line "), "a script error in VPX's log");
        assert_eq!(code, Some(0), "VPX didn't end cleanly");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// The add-on on its own, as in a VPX with no app beside it, unseen and silent: Attack from Mars
    /// in a test library of VPX's newer build (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test-7001`)
    /// in VPX's capture mode, no game's packs handed over (`VPI_ACHIEVEMENTS` unset), its sample pack
    /// copied into the `achievements` folder of VPX's preferences folder for this run. The add-on
    /// finds it by the game's name and year in the table file's name (the pack names no ROM set),
    /// hears the machine's set from VPX's controllers list, follows the game hardcore and keeps
    /// what's earned in that folder. `cargo build --release -p vpx-addon`, then `cargo test
    /// --release -p library --lib the_addon_plays_on_its_own_in_vpx -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_addon_plays_on_its_own_in_vpx() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-7001"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        let table = lib.join("Tables").join("Attack from Mars (Bally 1995)").join("Attack from Mars (Bally 1995).vpx");
        let work = scratch("own");
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        let at = put_in_for(&vpx, Some(&prefs)).unwrap();
        println!("VPX {:?}; add-on in {}", vpx.version, at.display());
        // the sample pack where the add-on looks on its own: VPX's preferences folder's achievements
        let sample = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let mut stack = vec![(sample, prefs.join("achievements").join("attack-from-mars"))];
        while let Some((from, to)) = stack.pop() {
            std::fs::create_dir_all(&to).unwrap();
            for e in std::fs::read_dir(&from).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push((e.path(), to.join(e.file_name())));
                } else {
                    std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
                }
            }
        }
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
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
            set("Backglass", "BackglassOutput", "0"),
            set("ScoreView", "ScoreViewOutput", "0"),
            set("Topper", "TopperOutput", "0"),
            set(&format!("Plugin.{ID}"), "Enable", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::remove_var("VPI_ACHIEVEMENTS");
            std::env::remove_var("VPI_ACHIEVEMENTS_CHECK_SCRIPT");
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
        }
        let capture = table.parent().unwrap().join("Capture");
        let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let had = capture.is_dir();
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "12".into(), "1".into(), table.clone().into(), "noloop".into()];
        let (code, boxes) = crate::vpx_test::unseen_reading(&vpx.program, &args, 180).unwrap();
        println!("VPX ended: {code:?}; its message boxes: {boxes:?}");
        for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
            if p.is_file() {
                let _ = std::fs::remove_file(&p);
            }
        }
        if !had {
            let _ = std::fs::remove_dir_all(&capture);
        }
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        println!("{}", log.lines().filter(|l| !l.contains("Lamp") && !l.contains("Solenoid") && !l.contains("Switch")).collect::<Vec<_>>().join("\n"));
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("VPIAchievements") || l.contains("rror")) {
            println!("vpx: {l}");
        }
        let kept = std::fs::read_to_string(prefs.join("VPinballX.ini")).unwrap_or_default();
        println!("its settings as VPX kept them: {:?}", kept.lines().skip_while(|l| !l.contains(&format!("[Plugin.{ID}]"))).take(5).collect::<Vec<_>>());
        assert!(log.contains("on its own: 1 pack(s) for"), "the pack wasn't found on its own");
        assert!(log.contains("its machine afm_113b"), "the machine's set wasn't heard");
        assert!(log.contains("started:"), "the game wasn't followed");
        assert!(boxes.is_empty(), "VPX showed {boxes:?}");
        assert!(!vpx_log.contains(" error on line "), "a script error in VPX's log");
        assert_eq!(code, Some(0), "VPX didn't end cleanly");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// The app's pause menu on VPX's menu key, unseen and silent: Attack from Mars in a test library
    /// (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test-7001`) in VPX's capture mode for 12 s, the
    /// add-on on its own with its sample pack, a port of this test's standing for the app's
    /// (`VPI_MENU_PORT`). Four seconds in, VPX's menu action is pressed as a player's F12 would be
    /// (`VPI_MENU_CHECK_PRESS_AT_MS`): the add-on keeps it from VPX and asks for the menu with where
    /// the achievements stand; this test answers as the menu's Quit does, and VPX ends well before
    /// its 12 s. `cargo build --release -p vpx-addon`, then `cargo test --release -p library --lib
    /// the_pause_menu_takes_vpxs_menu_key_in_vpx -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_pause_menu_takes_vpxs_menu_key_in_vpx() {
        use std::io::{BufRead, Write};
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-7001"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        let table = lib.join("Tables").join("Attack from Mars (Bally 1995)").join("Attack from Mars (Bally 1995).vpx");
        let work = scratch("menu");
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        put_in_for(&vpx, Some(&prefs)).unwrap();
        let sample = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let mut stack = vec![(sample, prefs.join("achievements").join("attack-from-mars"))];
        while let Some((from, to)) = stack.pop() {
            std::fs::create_dir_all(&to).unwrap();
            for e in std::fs::read_dir(&from).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push((e.path(), to.join(e.file_name())));
                } else {
                    std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
                }
            }
        }
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
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
            set("Backglass", "BackglassOutput", "0"),
            set("ScoreView", "ScoreViewOutput", "0"),
            set("Topper", "TopperOutput", "0"),
            set(&format!("Plugin.{ID}"), "Enable", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // the app's end of the link: the menu asked for, answered as its Quit does
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let heard = std::thread::spawn(move || -> Option<String> {
            // the add-on connects as the game starts: given up after a minute
            let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
            let stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(_) if std::time::Instant::now() > until => return None,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
                }
            };
            stream.set_nonblocking(false).ok()?;
            let mut writer = stream.try_clone().ok()?;
            for line in std::io::BufReader::new(stream).lines().map_while(Result::ok) {
                if line.contains("\"menu\":\"open\"") {
                    // levels set as the menu sets them, read back by the check's line, then its Quit
                    writer.write_all(b"{\"do\":\"level\",\"id\":\"PinMAME\",\"value\":0.5}\n{\"do\":\"table-sounds\",\"percent\":80}\n").ok()?;
                    std::thread::sleep(std::time::Duration::from_millis(700));
                    let _ = writer.write_all(b"{\"do\":\"quit\"}\n");
                    return Some(line);
                }
            }
            None
        });
        let read_back = work.join("table-sounds.txt");
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::remove_var("VPI_ACHIEVEMENTS");
            std::env::set_var("VPI_ACHIEVEMENTS_CHECK_SCRIPT", format!("30500=Set vpiFile = CreateObject(\"Scripting.FileSystemObject\").CreateTextFile(\"{}\") : vpiFile.WriteLine ActiveTable.TableSoundVolume : vpiFile.Close", read_back.display()));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
            std::env::set_var("VPI_MENU_PORT", port.to_string());
            std::env::set_var("VPI_MENU_CHECK_PRESS_AT_MS", "4000");
        }
        let capture = table.parent().unwrap().join("Capture");
        let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let had = capture.is_dir();
        let started = std::time::Instant::now();
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "12".into(), "1".into(), table.clone().into(), "noloop".into()];
        let (code, boxes) = crate::vpx_test::unseen_reading(&vpx.program, &args, 180).unwrap();
        let took = started.elapsed();
        // SAFETY: as above
        unsafe {
            std::env::remove_var("VPI_MENU_PORT");
            std::env::remove_var("VPI_MENU_CHECK_PRESS_AT_MS");
            std::env::remove_var("VPI_ACHIEVEMENTS_CHECK_SCRIPT");
        }
        for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
            if p.is_file() {
                let _ = std::fs::remove_file(&p);
            }
        }
        if !had {
            let _ = std::fs::remove_dir_all(&capture);
        }
        let asked = heard.join().unwrap();
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        println!("VPX ended: {code:?} after {took:?}; its message boxes: {boxes:?}\nthe menu asked for: {asked:?}");
        println!("{log}");
        let table_sounds = std::fs::read_to_string(&read_back).unwrap_or_default();
        println!("the table's own level for its sounds, read back: {table_sounds:?}");
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("VPIAchievements") || l.contains("rror") || l.contains("InGameUI")) {
            println!("vpx: {l}");
        }
        let asked = asked.expect("the add-on never asked for the menu");
        assert!(asked.contains("\"summary\":\"Hardcore"), "{asked}");
        assert!(asked.contains("\"sources\":[") && asked.contains("PinMAME"), "{asked}");
        assert_eq!(table_sounds.trim(), "80", "the table's own level wasn't set by its script line");
        assert!(log.contains("the sound of PinMAME at 50%"), "no sound of PinMAME's was levelled: {log}");
        assert!(took < std::time::Duration::from_secs(11), "VPX played on: the menu's Quit didn't end the game");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// A hardcore game turned softcore when a setting that makes a game softcore changes in a
    /// settings file it plays with, unseen and silent: Attack from Mars in a test library
    /// (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test-7001`) in VPX's capture mode, its sample pack
    /// followed hardcore, a table settings file of the game's own watched (`hardcoreFiles`), a table
    /// option written into it a few seconds in, as VPX's menu would keep one: the add-on says so and
    /// the game ends softcore. `cargo build --release -p vpx-addon`, then `cargo test --release -p
    /// library --lib a_hardcore_game_turns_softcore_in_vpx -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_hardcore_game_turns_softcore_in_vpx() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-7001"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        let table = lib.join("Tables").join("Attack from Mars (Bally 1995)").join("Attack from Mars (Bally 1995).vpx");
        let work = scratch("hardcore");
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        put_in_for(&vpx, Some(&prefs)).unwrap();
        let watched = work.join("game's own table settings.ini");
        std::fs::write(&watched, "[TableOption]\r\n").unwrap();
        let sample = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let asked = serde_json::json!({ "mode": "hardcore", "packs": [sample], "earned": work.join("earned.json"), "result": work.join("result.json"), "sound": { "on": false }, "hardcoreFiles": [watched] });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
        let changes = [
            set("Player", "PlayfieldDisplay", ""),
            set("Player", "PlayfieldFullScreen", "0"),
            set("Player", "PlayfieldWidth", "640"),
            set("Player", "PlayfieldHeight", "480"),
            set("Player", "BGSet", "0"),
            set("Player", "SoundVolume", "0"),
            set("Player", "MusicVolume", "0"),
            set("Backglass", "BackglassOutput", "0"),
            set("ScoreView", "ScoreViewOutput", "0"),
            set("Topper", "TopperOutput", "0"),
            set(&format!("Plugin.{ID}"), "Enable", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::remove_var("VPI_ACHIEVEMENTS_CHECK_SCRIPT");
            std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
        }
        // a table option kept a few seconds in, as VPX's menu keeps one
        let writer = {
            let watched = watched.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                std::fs::write(&watched, "[TableOption]\r\nOutlane_Difficulty = 2\r\n").unwrap();
            })
        };
        let capture = table.parent().unwrap().join("Capture");
        let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let had = capture.is_dir();
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "400".into(), "1".into(), table.clone().into(), "noloop".into()];
        let (code, boxes) = crate::vpx_test::unseen_reading(&vpx.program, &args, 180).unwrap();
        writer.join().unwrap();
        for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
            if p.is_file() {
                let _ = std::fs::remove_file(&p);
            }
        }
        if !had {
            let _ = std::fs::remove_dir_all(&capture);
        }
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        println!("VPX ended {code:?}, boxes {boxes:?}\n{}", log.lines().filter(|l| !l.contains("Lamp") && !l.contains("Solenoid") && !l.contains("Switch")).collect::<Vec<_>>().join("\n"));
        let result = std::fs::read_to_string(work.join("result.json")).unwrap_or_default();
        println!("result: {result}");
        assert!(log.contains("softcore from now: a table option was changed"), "the change wasn't heard");
        assert!(result.contains("\"mode\": \"practice\""), "the game didn't end softcore");
        assert_eq!(code, Some(0), "VPX didn't end cleanly");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// What VPX says it was never given back as it closes (its plugin manager's "was not released"
    /// lines), with our add-on off, on with nothing to follow, and on following a pack that counts
    /// the balls in play: Attack from Mars (or the table whose folder `VPI_TEST_TABLE` names) in a
    /// test library (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test-7001`) in VPX's capture mode,
    /// unseen and silent, each run in a scratch preferences folder of its own. Our add-on adds
    /// nothing to what's left: every message id it asks for goes back, VPX's own GetAPI's, 0,
    /// among them. `cargo build --release -p vpx-addon`, then `cargo test --release -p library
    /// --lib vpx_is_given_back_all_of_ours -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn vpx_is_given_back_all_of_ours() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test-7001"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        // another of the library's tables where `VPI_TEST_TABLE` names its folder
        let name = std::env::var("VPI_TEST_TABLE").unwrap_or_else(|_| "Attack from Mars (Bally 1995)".to_string());
        let table = lib.join("Tables").join(&name).join(format!("{name}.vpx"));
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let sample = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let mut left: Vec<(&str, Vec<String>)> = Vec::new();
        for (setup, on, follows) in [("off", false, false), ("on, nothing to follow", true, false), ("on, a pack counting the balls", true, true)] {
            let work = scratch("given-back");
            let prefs = work.join("prefs");
            std::fs::create_dir_all(&prefs).unwrap();
            let at = put_in_for(&vpx, Some(&prefs)).unwrap();
            let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
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
                set("Backglass", "BackglassOutput", "0"),
                set("ScoreView", "ScoreViewOutput", "0"),
                set("Topper", "TopperOutput", "0"),
                set(&format!("Plugin.{ID}"), "Enable", if on { "1" } else { "0" }),
            ];
            std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
            // SAFETY: a test of its own, run alone
            unsafe {
                std::env::remove_var("VPI_ACHIEVEMENTS_CHECK_SCRIPT");
                std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
                if follows {
                    let file = sample.join("achievements.json");
                    let mut json: serde_json::Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
                    let one = serde_json::json!({ "id": "one-in-play", "name": "One in Play", "about": "Have a ball in play", "points": 1, "since": "1.0", "when": { "ballsInPlay": { "atLeast": 1 } } });
                    json["achievements"].as_array_mut().unwrap().insert(0, one);
                    let pack = work.join("pack");
                    std::fs::create_dir_all(&pack).unwrap();
                    std::fs::write(pack.join("achievements.json"), serde_json::to_vec_pretty(&json).unwrap()).unwrap();
                    let asked = serde_json::json!({ "mode": "hardcore", "packs": [pack], "earned": work.join("earned.json"), "result": work.join("result.json"), "sound": { "on": false } });
                    std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
                    std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
                } else {
                    std::env::remove_var("VPI_ACHIEVEMENTS");
                }
            }
            let capture = table.parent().unwrap().join("Capture");
            let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
            let had = capture.is_dir();
            let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), "12".into(), "1".into(), table.clone().into(), "noloop".into()];
            let (code, boxes) = crate::vpx_test::unseen_reading(&vpx.program, &args, 180).unwrap();
            for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
                if p.is_file() {
                    let _ = std::fs::remove_file(&p);
                }
            }
            if !had {
                let _ = std::fs::remove_dir_all(&capture);
            }
            let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
            let loaded = vpx_log.contains(&format!("Plugin {ID} loaded"));
            let lines: Vec<String> = vpx_log.lines().filter(|l| l.contains("was not released")).map(|l| l.split("] ").last().unwrap_or(l).to_string()).collect();
            println!("{setup} (VPX {:?}, add-on in {}, loaded: {loaded}): VPX ended {code:?}, boxes {boxes:?}, left: {lines:?}", vpx.version, at.display());
            assert_eq!(code, Some(0), "VPX didn't end cleanly ({setup})");
            assert_eq!(loaded, on, "the add-on loaded where it wasn't to, or not where it was ({setup})");
            left.push((setup, lines));
            let _ = std::fs::remove_dir_all(&work);
        }
        let off = left[0].1.clone();
        for (setup, lines) in &left[1..] {
            let ours: Vec<&String> = lines.iter().filter(|l| !off.contains(l)).collect();
            assert!(ours.is_empty(), "with our add-on {setup}, VPX was left {ours:?} more than with it off");
        }
    }

    /// A card shown on the backglass in VPX, unseen: Attack from Mars in a test library
    /// (`VPI_TEST_LIBRARY`, else `vpx-build\vpx-test`) with a backglass window of its own, VPX's
    /// B2S add-on drawing it, the sample pack's first achievement shown as a card 31 seconds in
    /// (`VPI_ACHIEVEMENTS_CARD_AT_MS`, once capture mode's fast first 30 seconds are over) at the
    /// size `VPI_TEST_CARD_SIZE` names (medium unless); the pictures capture mode takes of the
    /// playfield and the backglass kept in the temp folder's `vpi-addon-card` for a look, and the
    /// windows' screenshots taken a second and a half in, unless `VPI_TEST_CARD_NO_SHOTS` is set.
    /// `cargo build --release -p vpx-addon`, then `cargo test --release -p library --lib
    /// a_card_shows_on_the_backglass_in_vpx -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn a_card_shows_on_the_backglass_in_vpx() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        assert!(vpx.has_addons(), "{:?}", vpx.version);
        let table = lib.join("Tables").join("Attack from Mars (Bally 1995)").join("Attack from Mars (Bally 1995).vpx");
        let work = scratch("card");
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        // the add-on where the app puts it for the player (`VPI_TEST_ADDON_AS_PLAYED`: for a later
        // VPX, the preferences folder's `plugins`), else in VPX's own folder
        let at = if std::env::var_os("VPI_TEST_ADDON_AS_PLAYED").is_some() { put_in_for(&vpx, Some(&prefs)) } else { put_in(&vpx) };
        println!("VPX {:?}; add-on in {}", vpx.version, at.unwrap().display());
        let pack = repo.join("tools").join("achievements").join("samples").join("attack-from-mars");
        let size = std::env::var("VPI_TEST_CARD_SIZE").unwrap_or_else(|_| "medium".into());
        // the window it shows in: the backglass, or the score display where `VPI_TEST_CARD_ON` says
        let on = std::env::var("VPI_TEST_CARD_ON").unwrap_or_else(|_| "backglass".into());
        let asked = serde_json::json!({ "mode": "hardcore", "packs": [pack], "earned": work.join("earned.json"), "result": work.join("result.json"), "sound": { "on": true, "gain": 0.05 }, "popup": { "on": on, "size": size } });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        // the test library's settings, its playfield a small window, its backglass one of its own
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let set = |s: &str, k: &str, v: &str| vpi_settings::Change::set(s, k, v);
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
            set("Backglass", "BackglassHeight", "720"),
            set("ScoreView", "ScoreViewOutput", if on == "scoreview" { "1" } else { "0" }),
            set("ScoreView", "ScoreViewDisplay", ""),
            set("ScoreView", "ScoreViewFullScreen", "0"),
            set("ScoreView", "ScoreViewWndX", "0"),
            set("ScoreView", "ScoreViewWndY", "500"),
            set("ScoreView", "ScoreViewWidth", "640"),
            set("ScoreView", "ScoreViewHeight", "160"),
            set("Topper", "TopperOutput", "0"),
            set("Plugin.B2S", "Enable", "1"),
            set(&format!("Plugin.{ID}"), "Enable", "1"),
        ];
        std::fs::write(prefs.join("VPinballX.ini"), vpi_settings::edit(&prefs_text, &changes)).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            std::env::set_var("VPI_ACHIEVEMENTS_LOG", work.join("addon.log"));
            std::env::set_var("VPI_ACHIEVEMENTS_CARD_AT_MS", "31000");
            // the screenshots left out where `VPI_TEST_CARD_NO_SHOTS` says
            if std::env::var_os("VPI_TEST_CARD_NO_SHOTS").is_none() {
                std::env::set_var("VPI_SCREENSHOTS", work.join("shots"));
                std::env::set_var("VPI_SCREENSHOT_AT_MS", "32500");
            }
        }
        let capture = table.parent().unwrap().join("Capture");
        let before: Vec<PathBuf> = std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).collect();
        let had = capture.is_dir();
        // every picture kept as taken (`noloop`), none cut to a loop
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-CaptureAttract".into(), std::env::var("VPI_TEST_CARD_PICTURES").unwrap_or_else(|_| "4".into()).into(), "1".into(), table.clone().into(), "noloop".into()];
        let code = crate::vpx_test::unseen(&vpx.program, &args, 150).unwrap();
        println!("VPX ended: {code:?}");
        // capture mode's pictures kept for a look, then taken out of the table's folder
        let look = std::env::temp_dir().join("vpi-addon-card");
        let _ = std::fs::remove_dir_all(&look);
        std::fs::create_dir_all(&look).unwrap();
        for p in std::fs::read_dir(&capture).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| !before.contains(p)) {
            if p.is_file() {
                let _ = std::fs::copy(&p, look.join(p.file_name().unwrap()));
                let _ = std::fs::remove_file(&p);
            }
        }
        if !had {
            let _ = std::fs::remove_dir_all(&capture);
        }
        println!("capture mode's pictures: {:?}", std::fs::read_dir(&look).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>());
        let log = std::fs::read_to_string(work.join("addon.log")).unwrap_or_default();
        println!("{}", log.lines().filter(|l| !l.contains("Lamp") && !l.contains("Solenoid") && !l.contains("Switch")).collect::<Vec<_>>().join("\n"));
        let vpx_log = std::fs::read_to_string(prefs.join("vpinball.log")).unwrap_or_default();
        for l in vpx_log.lines().filter(|l| l.contains("VPIAchievements") || l.contains("Achievements") || l.contains("B2S") || l.contains("rror") || l.contains("Backglass") || l.contains("udio")) {
            println!("vpx: {l}");
        }
        let lines: Vec<&str> = vpx_log.lines().collect();
        println!("--- the end of VPX's log\n{}", lines[lines.len().saturating_sub(12)..].join("\n"));
        let shots: Vec<PathBuf> = std::fs::read_dir(work.join("shots")).into_iter().flatten().flatten().map(|e| e.path()).collect();
        println!("screenshots: {:?}", shots.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect::<Vec<_>>());
        if let Some(bg) = shots.iter().find(|p| p.to_string_lossy().ends_with(" backglass.png")) {
            let _ = std::fs::copy(bg, std::env::temp_dir().join("vpi-addon-card.png"));
        }
        assert!(log.contains("card shown:"), "no card was drawn");
        // its sound through VPX's own, as our source of sound
        assert!(log.contains("played by VPX"), "the sound wasn't VPX's");
        // and VPX made our source of sound one of its own, its level among its settings
        let kept = std::fs::read_to_string(prefs.join("VPinballX.ini")).unwrap_or_default();
        println!("VPX's settings: {:?}", kept.lines().filter(|l| l.contains("AudioSource")).collect::<Vec<_>>());
        assert!(kept.contains("AudioSource.VPIAchievements"), "VPX made no source of ours");
        assert_eq!(code, Some(0), "VPX didn't end cleanly");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// VPX taking a table settings file of the app's own after the table on its command line, as
    /// Play Hardcore hands one (`-TableIni`): the test library's VPX extracting a scratch copy's
    /// script with one. `cargo test --release -p library --lib vpx_takes_a_table_ini_after_the_table
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn vpx_takes_a_table_ini_after_the_table() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let lib = std::fs::canonicalize(repo.join("vpx-build").join("vpx-test")).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        let work = scratch("tableini");
        let table = work.join("Black Knight.vpx");
        std::fs::copy(lib.join("Tables").join("Black Knight (Williams 1980)").join("Black Knight (Williams 1980).vpx"), &table).unwrap();
        let ini = work.join("own.ini");
        std::fs::write(&ini, "[TableOverride]\r\nExposure = 1.0\r\n").unwrap();
        let prefs = work.join("prefs");
        std::fs::create_dir_all(&prefs).unwrap();
        std::fs::write(prefs.join("VPinballX.ini"), "").unwrap();
        let args: Vec<std::ffi::OsString> = vec!["-PrefPath".into(), prefs.clone().into(), "-ExtractVBS".into(), table.clone().into(), "-TableIni".into(), ini.into()];
        let code = crate::vpx_test::unseen(&vpx.program, &args, 30).unwrap();
        println!("VPX ended: {code:?}");
        assert_eq!(code, Some(0));
        assert!(table.with_extension("vbs").is_file());
        let _ = std::fs::remove_dir_all(&work);
    }

    /// Every table of the test library tested in the released VPX 10.8.1 as Test it tests it, our
    /// add-on loaded beside VPX's own (screenshots on, nothing to follow): none trips on it.
    /// `cargo build --release -p vpx-addon`, then `cargo test --release -p library --lib
    /// every_test_table_plays_with_the_addon_loaded -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn every_test_table_plays_with_the_addon_loaded() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        // another test library where `VPI_TEST_LIBRARY` names one (`tools/vpx-test --out`)
        let lib = std::env::var_os("VPI_TEST_LIBRARY").map(PathBuf::from).unwrap_or_else(|| repo.join("vpx-build").join("vpx-test"));
        let lib = std::fs::canonicalize(lib).unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        put_in(&vpx).unwrap();
        let work = scratch("every");
        // following the game, no packs: the machine's states read through its map all the same
        let asked = serde_json::json!({ "mode": "hardcore", "packs": [], "earned": work.join("earned.json"), "result": work.join("result.json") });
        std::fs::write(work.join("playing.json"), serde_json::to_vec_pretty(&asked).unwrap()).unwrap();
        // SAFETY: a test of its own, run alone
        unsafe {
            // `VPI_TEST_NO_FOLLOW`: loaded, following nothing
            if std::env::var_os("VPI_TEST_NO_FOLLOW").is_some() {
                std::env::remove_var("VPI_ACHIEVEMENTS");
            } else {
                std::env::set_var("VPI_ACHIEVEMENTS", work.join("playing.json"));
            }
            std::env::set_var("VPI_SCREENSHOTS", work.join("shots"));
        }
        let prefs_text = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let pinmame_path = vpi_settings::get(&vpi_settings::parse_ini(&prefs_text), "Plugin.PinMAME", "PinMAMEPath").map(str::to_string);
        // `VPI_TEST_ADDON_OFF=1`: the add-on switched off, to tell what's VPX's own; `VPI_TEST_ONLY`: the
        // tables whose folders' names hold it, alone
        let on = if std::env::var_os("VPI_TEST_ADDON_OFF").is_some() { 0 } else { 1 };
        let only = std::env::var("VPI_TEST_ONLY").ok();
        let settings = format!("{prefs_text}\r\n[Plugin.{ID}]\r\nEnable = {on}\r\n");
        let mut failed = Vec::new();
        for dir in std::fs::read_dir(lib.join("Tables")).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            if only.as_deref().is_some_and(|o| !o.split('|').any(|o| dir.to_string_lossy().contains(o))) {
                continue;
            }
            let Some(table) = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx"))) else { continue };
            // its ROM's map put in, as PLAY puts it
            let rom = std::fs::read_dir(dir.join("pinmame").join("roms")).into_iter().flatten().flatten().find_map(|e| e.path().file_stem().map(|s| s.to_string_lossy().into_owned()));
            if let Some(rom) = &rom {
                put_memory_map(&table, rom, pinmame_path.as_deref()).unwrap();
            }
            let log = work.join(format!("{}.log", dir.file_name().unwrap().to_string_lossy()));
            // SAFETY: as above
            unsafe { std::env::set_var("VPI_ACHIEVEMENTS_LOG", &log) };
            let report = crate::vpx_test::test(&vpx, &table, &settings);
            let heard = std::fs::read_to_string(&log).unwrap_or_default().lines().filter(|l| l.starts_with("machine:")).collect::<Vec<_>>().join("; ");
            println!("{}: {} ({:.1} s): {} {}", table.file_name().unwrap().to_string_lossy(), report.look, report.seconds, report.text, if heard.is_empty() { String::new() } else { format!("[add-on {heard}]") });
            if report.look == "bad" {
                failed.push(format!("{}: {}", table.display(), report.text));
            }
        }
        let _ = std::fs::remove_dir_all(&work);
        assert!(failed.is_empty(), "{failed:#?}");
    }

    /// The end of VPX's own log of a quiet run, for a table whose run doesn't end: `VPI_TEST_LIBRARY`
    /// and `VPI_TEST_ONLY` name it. `cargo test --release -p library --lib vpx_log_of_a_long_quiet_run
    /// -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn vpx_log_of_a_long_quiet_run() {
        let lib = std::fs::canonicalize(std::env::var_os("VPI_TEST_LIBRARY").unwrap()).unwrap();
        let only = std::env::var("VPI_TEST_ONLY").unwrap();
        let vpx = crate::vpx::newest_in(std::slice::from_ref(&lib)).unwrap();
        let dir = std::fs::read_dir(lib.join("Tables")).unwrap().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().contains(&only)).unwrap();
        let table = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx"))).unwrap();
        let settings = std::fs::read_to_string(lib.join("prefs").join("VPinballX.ini")).unwrap();
        let started = std::time::Instant::now();
        let (log, code) = crate::vpx_test::run(&vpx, &table, &settings, "15").unwrap();
        println!("ended {code:?} after {:.1} s", started.elapsed().as_secs_f64());
        let lines: Vec<&str> = log.lines().collect();
        for l in lines.iter().filter(|l| l.contains("WARN") || l.contains("ERROR") || l.contains("Captur") || l.contains("pause") || l.contains("Pause")).take(60) {
            println!("{l}");
        }
        println!("--- last lines ---");
        for l in &lines[lines.len().saturating_sub(25)..] {
            println!("{l}");
        }
    }

    #[test]
    fn its_cfg_names_the_library_vpx_loads() {
        let text = cfg();
        assert!(text.contains("id = \"VPIAchievements\""));
        assert!(text.contains("windows.x64 = \"plugin-vpi-achievements64.dll\""));
    }
}
