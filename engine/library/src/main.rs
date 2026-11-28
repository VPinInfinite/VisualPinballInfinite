//! `vpi-library`: the table library's engine side, from the command line, for
//! checks while the app's screens are designed. Nothing is written to
//! any table folder: `identify` shows where a drop's files would go.
//!
//! ```text
//! vpi-library identify <file or folder> [--vps <vpsdb.json>]
//! vpi-library match <table.vpx>... [--explain] [--vps <vpsdb.json>]
//! vpi-library match-all <folder> [--vps <vpsdb.json>] [--compare <tested-tables.md>] [--key <links.md>] [--report <file.md>]
//! vpi-library fetch [--vps <vpsdb.json>]
//! vpi-library copy-out <file, folder or archive> <its file> <to>
//! vpi-library scores <table.vpx>...
//! vpi-library script-options <table.vpx>...
//! vpi-library physics <table.vpx>...
//! vpi-library remove-plan <part of a table's file name>
//! vpi-library pup [<pack folder>...]
//! vpi-library dmd-places <tables folder or table>... [--pictures <folder>]
//! vpi-library guide <a saved guide page>...
//! ```
//!
//! VPS's database: `--vps`, else the app's copy (`%APPDATA%\Visual Pinball Infinite\VPS`), else
//! the repository's (`results\vps`, the tested-tables page's); `fetch` gets a fresh one.

use std::path::{Path, PathBuf};

use library::analysis::{Kind, analyse};
use library::archive::{self, Source};
use library::matcher::{Clues, Found, Match, match_table};
use library::plan::{Layout, There, plan, there};
use library::vps::{self, Database, GAME_LINK};

const USAGE: &str = "usage: vpi-library identify <file or folder> | match <table.vpx>... [--explain] | match-all <folder> [--compare <tested-tables.md>] [--key <links.md>] [--report <file.md>] | fetch | copy-out <drop> <its file> <to> | drop <files or folders>... | scores <table.vpx>... | script-options <table.vpx>... | physics <table.vpx>... | remove-plan <part of a name> | pup [<pack folder>...] | dmd-places <folder or table>... [--pictures <folder>] | guide <a saved guide page>...   [--vps <vpsdb.json>]";

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vpi-library: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str| -> Option<String> {
        let at = args.iter().position(|a| a == flag)?;
        args.remove(at);
        (at < args.len()).then(|| args.remove(at))
    };
    let vps_given = take("--vps").map(PathBuf::from);
    let compare = take("--compare").map(PathBuf::from);
    let key = take("--key").map(PathBuf::from);
    let report = take("--report").map(PathBuf::from);
    let pictures = take("--pictures").map(PathBuf::from);
    let explain = args.iter().position(|a| a == "--explain").map(|at| args.remove(at)).is_some();
    let command = args.first().cloned().ok_or(USAGE)?;
    let rest: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
    let vps_path = vps_given.clone().or_else(|| vps::default_path().filter(|p| p.exists())).or_else(|| {
        let repo = std::env::current_exe().ok()?.ancestors().nth(3)?.join("results").join("vps").join("vpsdb.json");
        repo.exists().then_some(repo)
    });
    match command.as_str() {
        "guide" => {
            // saved guide pages' titles as the pause menu reads them, as JSON, a line for each
            for path in &rest {
                let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let title = library::guide::title_of(&library::reading::text(&bytes, false));
                println!("{}", serde_json::json!({ "file": name, "title": title }));
            }
            Ok(())
        }
        "fetch" => {
            let to = vps_given.or_else(vps::default_path).ok_or("no data folder")?;
            vps::fetch(&to)?;
            let db = Database::load(&to)?;
            println!("VPS's database: {} games, saved in {}", db.games.len(), to.display());
            Ok(())
        }
        "identify" => {
            let path = rest.first().ok_or(USAGE)?;
            let db = load(vps_path.as_deref())?;
            identify(path, &db)
        }
        "match" => {
            let db = load(vps_path.as_deref())?;
            for path in &rest {
                let facts = library::table::read(path)?;
                let m = match_table(&db, &facts.clues);
                println!("{}", describe(&m, facts.game_name.as_deref(), &db));
                if explain {
                    explain_match(&facts, &m, &db);
                }
            }
            Ok(())
        }
        "copy-out" => {
            // one file of a drop written out, for checks
            let [drop, entry, to] = rest.as_slice() else { return Err(USAGE.into()) };
            let listing = archive::list(drop)?;
            let entry = entry.to_string_lossy().replace('\\', "/");
            let mut out = std::io::BufWriter::new(std::fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?);
            listing.copy_out(&entry, &mut out, u64::MAX)?;
            std::io::Write::flush(&mut out).map_err(|e| format!("{}: {e}", to.display()))?;
            Ok(())
        }
        "match-all" => {
            let folder = rest.first().ok_or(USAGE)?;
            let db = load(vps_path.as_deref())?;
            match_all(folder, &db, compare.as_deref(), key.as_deref(), report.as_deref())
        }
        "drop" => {
            if rest.is_empty() {
                return Err(USAGE.into());
            }
            let db = load(vps_path.as_deref())?;
            drop_report(&rest, &db)
        }
        "health" => {
            // every installed table's health check, as its page shows it: what isn't right
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            let media = library::installed::media_folders(&layout);
            let around = library::installed::Around::read(&layout);
            let mut remembered = library::installed::Remembered::load(&library::installed::Remembered::default_path().ok_or("no APPDATA folder")?);
            let found = library::installed::scan(&layout.tables, &mut remembered, |_, _, _| {})?;
            let roms = layout.vpinmame.join("roms");
            let (mut fine, mut checked) = (0, 0);
            for (path, read) in &found {
                let h = library::installed::holds(&layout, &aliases, &media, &around, path, read);
                let mut said: Vec<String> = Vec::new();
                match (&h.rom_set, &h.rom) {
                    (Some(set), Some(_)) => match library::roms::check_installed(&roms, set) {
                        Ok(("good", _)) => {}
                        Ok((_, text)) => said.push(format!("ROM: {text}")),
                        Err(e) => said.push(format!("ROM: {e}")),
                    },
                    (Some(set), None) => said.push(format!("ROM: {set}.zip isn't installed")),
                    _ => {}
                }
                if let Some(pack) = &h.altsound {
                    let (look, text) = library::altsound::check(pack);
                    if look != "good" {
                        said.push(format!("AltSound: {text}"));
                    }
                }
                said.extend(h.helpers.iter().filter(|x| x.path.is_none()).map(|x| format!("script it loads: {}", x.name)));
                said.extend(h.folders_missing.iter().map(|f| format!("DMD folder: {f}")));
                checked += 1;
                if said.is_empty() {
                    fine += 1;
                } else {
                    println!("{}", path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
                    for s in said {
                        println!("  {s}");
                    }
                }
            }
            println!("{fine} of {checked} tables hold everything their scripts ask for");
            Ok(())
        }
        "rom-check" => {
            // every installed table's ROM set checked file by file, as its page checks it
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            let roms = layout.vpinmame.join("roms");
            let mut sets: Vec<String> = Vec::new();
            for path in library::installed::table_files(&layout.tables)? {
                let Ok(facts) = library::table::read(&path) else { continue };
                if let Some(set) = facts.game_name.as_deref().filter(|_| facts.pinmame).map(|g| aliases.resolve(g))
                    && !sets.contains(&set)
                {
                    sets.push(set);
                }
            }
            sets.sort();
            for set in &sets {
                let (look, text) = if library::roms::installed(&roms, set) {
                    library::roms::check_installed(&roms, set).unwrap_or_else(|e| ("warn", e))
                } else {
                    ("warn", format!("{set}.zip isn't installed"))
                };
                println!("{look:<5} {set:<12} {text}");
            }
            Ok(())
        }
        // a table opened as the inspector opens it: its parts by kind, or all of it as JSON
        "inspect" => {
            let table = rest.iter().find(|p| p.to_string_lossy() != "--json").ok_or("inspect takes a table file")?;
            let got = library::inspect::inspect(table)?;
            if rest.iter().any(|a| a.to_string_lossy() == "--json") {
                println!("{}", serde_json::to_string_pretty(&got).map_err(|e| e.to_string())?);
            } else {
                let mut kinds: std::collections::BTreeMap<&str, (usize, usize)> = std::collections::BTreeMap::new();
                for p in &got.parts {
                    let e = kinds.entry(p.kind).or_default();
                    e.0 += 1;
                    e.1 += usize::from(!matches!(p.outline, library::inspect::Outline::None));
                }
                println!("playfield {:?}: {} parts", got.bounds, got.parts.len());
                for (k, (n, drawn)) in kinds {
                    println!("  {k}: {n} ({drawn} drawn)");
                }
            }
            Ok(())
        }
        "layout-plan" => {
            // the switch to one folder per game planned for the reference install: what would move
            // or be copied where, how much, what stays shared and what couldn't be done; nothing moves
            let db = load(vps_path.as_deref())?;
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            let media = library::installed::media_folders(&layout);
            let around = library::installed::Around::read(&layout);
            let mut remembered = library::installed::Remembered::load(&library::installed::Remembered::default_path().ok_or("no APPDATA folder")?);
            let found = library::installed::scan(&layout.tables, &mut remembered, |_, _, _| {})?;
            let mut held: Vec<(PathBuf, library::installed::Holds, String)> = Vec::new();
            for (path, read) in &found {
                let holds = library::installed::holds(&layout, &aliases, &media, &around, path, read);
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                // its game's name on VPS, else its file's
                let folder = library::table::read(path)
                    .ok()
                    .and_then(|facts| library::matcher::match_table(&db, &facts.clues).game.map(|g| library::rename::vps_name(&g.name, Some(&g.manufacturer), g.year)))
                    .unwrap_or(stem);
                held.push((path.clone(), holds, folder));
            }
            let tables: Vec<library::layout_switch::Table> = held.iter().map(|(path, holds, folder)| library::layout_switch::Table { path, holds, folder: folder.clone() }).collect();
            let p = library::layout_switch::plan(&layout, &tables);
            let gb = |b: u64| format!("{:.1} GB", b as f64 / 1e9);
            for (folder, ts) in &p.folders {
                println!("{}  ({} table{})", folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), ts.len(), if ts.len() == 1 { "" } else { "s" });
                for m in p.moves.iter().filter(|m| m.to.starts_with(folder)) {
                    let rel = m.to.strip_prefix(folder).map(|r| r.display().to_string()).unwrap_or_default();
                    println!("    {} {rel}  ({} files, {} MB)", if m.copy { "copy" } else { "move" }, m.files, m.bytes / 1_000_000);
                }
            }
            for (path, folders) in &p.shared {
                println!("stays shared: {} (used by {})", path.display(), folders.join(", "));
            }
            for problem in &p.problems {
                println!("problem: {problem}");
            }
            println!(
                "{} tables into {} folders: {} files, {} moved (renamed in place, the same drive), {} files, {} copied; {} things stay shared; {} problems",
                p.folders.values().map(Vec::len).sum::<usize>(),
                p.folders.len(),
                p.moved.1,
                gb(p.moved.0),
                p.copied.1,
                gb(p.copied.0),
                p.shared.len(),
                p.problems.len()
            );
            Ok(())
        }
        "roms" => {
            // every ROM set there and every one a table asks for, checked, its tables beside it:
            // the ROM manager's list, nothing changed
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            let mut tables: Vec<(PathBuf, String)> = Vec::new();
            for path in library::installed::table_files(&layout.tables)? {
                let Ok(facts) = library::table::read(&path) else { continue };
                if let Some(set) = facts.game_name.as_deref().filter(|_| facts.pinmame).map(|g| aliases.resolve(g)) {
                    tables.push((path, set));
                }
            }
            let all = library::roms::all_rom_files()?;
            let list = library::rom_inventory::inventory(&layout, &tables, &all);
            let name = |p: &PathBuf| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            for e in &list {
                let tables: Vec<String> = e.used_by.iter().map(name).collect();
                let clones: Vec<String> = e.parent_of.iter().map(name).collect();
                let others: Vec<String> = e.other_version_of.iter().map(|(t, set)| format!("{} plays {set}", name(t))).collect();
                let whose = match (tables.is_empty(), clones.is_empty()) {
                    (true, true) if !others.is_empty() => format!("no table: another version of a game {}", others.join(", ")),
                    (true, true) => "no table".to_string(),
                    (false, true) => tables.join(", "),
                    (true, false) => format!("the set of {}", clones.join(", ")),
                    (false, false) => format!("{}; the set of {}", tables.join(", "), clones.join(", ")),
                };
                println!("{:<4} {:<16} {:<16} {:<10} {}", e.look, e.set, e.status, format!("{} KB", e.size / 1024), whose);
                if e.status != "good" {
                    println!("{:21}{}", "", e.text);
                }
                for f in library::rom_inventory::fixes(e) {
                    let said = match f {
                        library::rom_inventory::Fix::Rename { to } => format!("rename it {}", to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                        library::rom_inventory::Fix::RemoveCopy { of } => format!("remove it: the same as {}", of.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
                        library::rom_inventory::Fix::Remove => "remove it".to_string(),
                    };
                    println!("{:21}fix offered: {said}", "");
                }
            }
            let count = |s: &str| list.iter().filter(|e| e.status == s).count();
            println!(
                "{} sets there: {} good, {} unused; {} files missing, {} wrong files, {} needing their parent, {} misnamed, {} unknown; {} asked for and not installed",
                list.iter().filter(|e| e.zip.is_some()).count(),
                count("good"),
                list.iter().filter(|e| e.unused()).count(),
                count("files missing"),
                count("wrong files"),
                count("needs its parent"),
                count("misnamed"),
                count("unknown"),
                count("not installed")
            );
            Ok(())
        }
        "pup" => {
            // each PinUP pack's layouts, the one in place and its check: read, nothing run or changed
            let packs: Vec<PathBuf> = if rest.is_empty() {
                let mut all: Vec<PathBuf> = std::fs::read_dir(&Layout::reference().pup_videos).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
                all.sort();
                all
            } else {
                rest.clone()
            };
            // and the layout that fits VPX's windows as its settings have them
            let windows = library::pup_pack::windows_in(&vpi_settings::vpx_settings_text());
            println!("VPX's windows: {windows:?}");
            for pack in &packs {
                let all = library::pup_pack::layouts(pack);
                let c = library::pup_pack::check(pack, &all);
                println!("{:<4} {}: {}", c.look, pack.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), c.text);
                for l in &all.layouts {
                    let mark = if all.current.as_deref() == Some(l.file.as_str()) { '*' } else { ' ' };
                    let shows = library::pup_pack::layout_shows(pack, l).map(|s| format!("  [{s:?}]")).unwrap_or_default();
                    println!("    {mark} {}{shows}{}{}", l.name, l.refused.as_deref().map(|r| format!("  — refused: {r}")).unwrap_or_default(), l.note.as_deref().map(|n| format!("  — {n}")).unwrap_or_default());
                }
                if let Some(f) = library::pup_pack::fitting(pack, &all, windows) {
                    println!("    fits: {}{} — {}", f.name, if f.in_place { " (in place)" } else { "" }, f.why);
                }
            }
            Ok(())
        }
        "remove-plan" => {
            // what removing a table would take, each ticked or not and which tables share it: a dry
            // run, nothing moved
            let want = rest.first().ok_or("remove-plan <part of a table's file name>")?.to_string_lossy().to_lowercase();
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            let media = library::installed::media_folders(&layout);
            let around = library::installed::Around::read(&layout);
            let mut remembered = library::installed::Remembered::load(&library::installed::Remembered::default_path().ok_or("no APPDATA folder")?);
            let found = library::installed::scan(&layout.tables, &mut remembered, |_, _, _| {})?;
            let holds: Vec<_> = found.iter().map(|(path, read)| library::installed::holds(&layout, &aliases, &media, &around, path, read)).collect();
            let stem = |p: &std::path::Path| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let names: Vec<String> = found.iter().map(|(p, r)| r.info_name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| stem(p))).collect();
            let at = found.iter().position(|(p, _)| stem(p).to_lowercase().contains(&want)).ok_or("No table's file name has that in it.")?;
            let seen = |k: usize| library::remove::Seen { name: &names[k], path: &found[k].0, read: &found[k].1, holds: &holds[k] };
            let others: Vec<_> = (0..found.len()).filter(|&k| k != at).map(seen).collect();
            println!("{}", found[at].0.display());
            for i in library::remove::plan(&layout, &seen(at), &others) {
                let size = if i.bytes >= 1 << 20 { format!("{:.1} MB", i.bytes as f64 / f64::from(1 << 20)) } else { format!("{} bytes", i.bytes) };
                let shared = if i.shared.is_empty() { String::new() } else { format!("  — also used by {}", i.shared.join(", ")) };
                println!("  [{}] {}: {} ({} files, {size}){shared}", if i.ticked { 'x' } else { ' ' }, i.what, i.name, i.files);
            }
            Ok(())
        }
        "remove" => {
            // a table taken out of the library as its Remove window takes it with what it ticks to
            // start with — its own files to the Recycle Bin, its game out of PinUP Popper's list
            // (the list kept first) — anything another table uses too left where it is; only with
            // --yes, after remove-plan has shown what goes
            let want = rest.first().ok_or("remove <part of a table's file name> --yes")?.to_string_lossy().to_lowercase();
            if !rest.iter().any(|a| a == "--yes") {
                return Err("remove carries out what remove-plan shows: add --yes".into());
            }
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            let media = library::installed::media_folders(&layout);
            let around = library::installed::Around::read(&layout);
            let mut remembered = library::installed::Remembered::load(&library::installed::Remembered::default_path().ok_or("no APPDATA folder")?);
            let found = library::installed::scan(&layout.tables, &mut remembered, |_, _, _| {})?;
            let holds: Vec<_> = found.iter().map(|(path, read)| library::installed::holds(&layout, &aliases, &media, &around, path, read)).collect();
            let stem = |p: &std::path::Path| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            // the tables by their files' names, as the app sees them: two releases of a machine
            // can carry the same name inside
            let names: Vec<String> = found.iter().map(|(p, _)| stem(p)).collect();
            let matching: Vec<usize> = (0..found.len()).filter(|&k| stem(&found[k].0).to_lowercase().contains(&want)).collect();
            let at = match matching.as_slice() {
                [one] => *one,
                [] => return Err("No table's file name has that in it.".into()),
                many => return Err(format!("{} tables' file names have that in them: name one alone.", many.len())),
            };
            let seen = |k: usize| library::remove::Seen { name: &names[k], path: &found[k].0, read: &found[k].1, holds: &holds[k] };
            let others: Vec<_> = (0..found.len()).filter(|&k| k != at).map(seen).collect();
            let items = library::remove::plan(&layout, &seen(at), &others);
            let ticked: std::collections::BTreeSet<String> = items.iter().filter(|i| i.ticked).map(|i| i.key.clone()).collect();
            let kept = PathBuf::from(std::env::var_os("APPDATA").ok_or("no APPDATA folder")?).join("Visual Pinball Infinite").join("Library").join("Kept");
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let done = library::remove::carry_out(&items, &ticked, &kept, &format!("{} UTC", utc(secs)))?;
            println!("{}: {} moved to the Recycle Bin{}", found[at].0.display(), done.recycled, if done.table_gone { ", the table out of the library" } else { "" });
            for p in done.problems {
                println!("  {p}");
            }
            Ok(())
        }
        "dmd-places" => {
            // where the score display would be placed over each table's backglass picture, as VPX's
            // newest builds keep it in the file: nothing written to any table's folder;
            // `--pictures` saves each picture searched, its place drawn on it, to look at
            use library::backglass_dmd::{self as dmd, Target};
            let mut tables = Vec::new();
            for path in &rest {
                if path.is_dir() {
                    let mut found: Vec<PathBuf> = Vec::new();
                    for entry in std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))?.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            found.extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()).filter(|q| q.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx"))));
                        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx")) {
                            found.push(p);
                        }
                    }
                    found.sort();
                    tables.extend(found);
                } else {
                    tables.push(path.clone());
                }
            }
            if let Some(folder) = &pictures {
                std::fs::create_dir_all(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            }
            let (mut placed, mut own, mut none) = (0, 0, 0);
            for table in &tables {
                let name = table.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let Some(file) = dmd::playing_file(table) else {
                    println!("{name}: no backglass file");
                    continue;
                };
                let text = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
                if dmd::holds_displays(&text) {
                    own += 1;
                    println!("{name}: holds its own: {:?}", dmd::places_in(&text));
                    continue;
                }
                if dmd::shows_scores(&text) {
                    println!("{name}: shows its own scores");
                    continue;
                }
                let rom = library::table::read(table).ok().and_then(|f| f.game_name.filter(|_| f.pinmame));
                let shape = dmd::dmd_shape(rom.as_deref());
                let started = std::time::Instant::now();
                let vpxs = dmd::find_place(&text, Target::Backglass, shape);
                let place = dmd::fitting_place(&text, Target::Backglass, shape);
                let took = started.elapsed().as_millis();
                let looked = dmd::searched_picture(&text, Target::Backglass).and_then(|(rgb, w, h)| dmd::search(&rgb, w, h, shape).map(|(_, limit)| (limit, h)));
                match place {
                    Some(p) => {
                        placed += 1;
                        let (limit, h) = looked.unwrap_or_default();
                        let moved = if vpxs == Some(p) { String::new() } else { " (in its speaker panel; VPX's search: elsewhere)".to_string() };
                        println!("{name}: {}:1 at {}, {} {} × {}{moved} ({took} ms){} | limit {limit} height {h} grill {}", shape, p.x, p.y, p.w, p.h, rom.map(|r| format!(", {r}")).unwrap_or_default(), dmd::grill_height(&text));
                    }
                    None => {
                        none += 1;
                        let vpx = vpxs.map(|p| format!("; VPX's search: {}, {} {} × {}", p.x, p.y, p.w, p.h)).unwrap_or_default();
                        println!("{name}: no place fits ({took} ms){vpx}");
                    }
                }
                if let (Some(folder), Some((rgb, w, h))) = (&pictures, dmd::searched_picture(&text, Target::Backglass)) {
                    let mut picture = image::RgbImage::from_raw(w, h, rgb).ok_or("a picture of the wrong size")?;
                    if let Some(p) = place {
                        let thick = (w / 400).max(2) as i32;
                        for y in p.y - thick..p.y + p.h + thick {
                            for x in p.x - thick..p.x + p.w + thick {
                                let edge = x < p.x || x >= p.x + p.w || y < p.y || y >= p.y + p.h;
                                if edge && x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                                    picture.put_pixel(x as u32, y as u32, image::Rgb([255, 0, 255]));
                                }
                            }
                        }
                    }
                    let small = image::imageops::resize(&picture, 640, (640 * h / w).max(1), image::imageops::FilterType::Triangle);
                    small.save(folder.join(format!("{name}.png"))).map_err(|e| e.to_string())?;
                }
            }
            println!("{} tables: {placed} placed, {own} holding their own, {none} with no place found", tables.len());
            Ok(())
        }
        "script-options" => {
            // the option lines the app's script editor shows for a table, from the script that plays
            for path in &rest {
                let script = library::table::script_that_plays(path)?;
                let text = String::from_utf8_lossy(&script);
                let found = library::script_edit::options(&text);
                println!("{}: {} option line(s)", path.display(), found.len());
                for o in &found {
                    let choices = o.choices.iter().map(|(v, w)| format!("{v}: {w}")).collect::<Vec<_>>().join(" | ");
                    println!("  line {:>4}  [{}] {} = {} ({}){}{}", o.line + 1, o.group, o.name, o.value, o.kind, if o.note.is_empty() { String::new() } else { format!("  ' {}", o.note) }, if choices.is_empty() { String::new() } else { format!("\n             choices: {choices}") });
                }
            }
            Ok(())
        }
        "physics" => {
            // a table's physics as the app's physics window shows them: its own, and the files
            // that play in their place
            for path in &rest {
                let v = library::physics::view(path)?;
                println!("{}", path.display());
                if let Some(set) = v.set {
                    println!("  plays VPX's physics set {set} from the settings, over any file");
                }
                for (label, f) in [("its own file", &v.file), ("the folder's", &v.shared)] {
                    if let Some(f) = f {
                        println!("  {label}: {} ({})", f.path.display(), f.name.as_deref().unwrap_or("no name"));
                    }
                }
                for (i, element) in v.elements.iter().enumerate() {
                    let own = v.own.map_or("-".to_string(), |o| o[i].to_string());
                    let file = v.file.as_ref().and_then(|f| f.values[i]).map_or(String::new(), |x| format!("  file: {x}"));
                    println!("  {:<22} {own}{file}", if i < 8 { format!("table {element}") } else { format!("flipper {element}") });
                }
            }
            Ok(())
        }
        "scores" => {
            // a table's high scores, as our player and VPX keep them
            let layout = Layout::reference();
            let aliases = library::installed::Aliases::load(&layout.vpinmame);
            for path in &rest {
                let facts = library::table::read(path)?;
                let rom = facts.game_name.as_deref().filter(|_| facts.pinmame).map(|g| aliases.resolve(g));
                let s = library::scores::for_table(&layout, Some(path.as_path()), rom.as_deref(), &facts.saved_under);
                println!("{}: {} ({})", path.display(), s.kind, s.under.as_deref().unwrap_or("-"));
                if let Some(p) = &s.problem {
                    println!("  {p}");
                }
                if s.kind == "script" && s.kept.is_empty() {
                    // what VPX keeps instead, to see a name that doesn't match
                    for file in library::vpx::saved_values(&layout, Some(path.as_path())).into_iter().filter(|f| f.is_file()) {
                        let names = if file.extension().is_some_and(|x| x.eq_ignore_ascii_case("ini")) {
                            let text = std::fs::read(&file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
                            let entries = vpi_settings::parse_ini(&text);
                            let mut names: Vec<(String, usize)> = Vec::new();
                            for (section, _, _) in &entries {
                                match names.iter_mut().find(|(n, _)| n == section) {
                                    Some((_, count)) => *count += 1,
                                    None => names.push((section.clone(), 1)),
                                }
                            }
                            names
                        } else {
                            library::scores::names_in_stg(&file).unwrap_or_default()
                        };
                        println!("  none kept; VPX's {} keeps: {}", file.display(), names.iter().map(|(n, c)| format!("{n} ({c})")).collect::<Vec<_>>().join(", "));
                    }
                }
                for k in &s.kept {
                    println!("  {} — {}{}", k.by, k.file.display(), k.games_played.map_or(String::new(), |g| format!(", {g} games played")));
                    for e in k.scores.iter().chain(&k.champions) {
                        println!("    {:<24} {:<4} {:>16} {}", e.label.as_deref().unwrap_or(""), e.initials, e.score.map_or(String::new(), |s| s.to_string()), e.extra.as_deref().unwrap_or(""));
                    }
                }
            }
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

fn load(path: Option<&Path>) -> Result<Database, String> {
    let path = path.ok_or("no copy of VPS's database: `vpi-library fetch` gets one")?;
    if vps::is_stale(path) {
        eprintln!("vpi-library: {} is more than a day old (`vpi-library fetch` renews it)", path.display());
    }
    Database::load(path)
}

/// A match in a line or two: the game, its table file, and the ROM.
fn describe(m: &Match, rom: Option<&str>, db: &Database) -> String {
    let mut out = String::new();
    match m.game {
        Some(g) => {
            out += &format!("VPS: {} ({} {}) — game {} {GAME_LINK}{}", g.name, g.manufacturer, g.year.map_or(String::new(), |y| y.to_string()), g.id, g.id);
            out += &format!("\n     found {} (distance {:.2})", m.found.describe(), m.distance);
            match m.table_file {
                Some(t) => {
                    out += &format!("\n     table file {} — {} by {}", t.id, t.version.as_deref().unwrap_or("no version"), t.authors.join(", "));
                }
                None => out += "\n     no table file of it matched",
            }
            if let Some(r) = rom {
                let known = g.rom_files.iter().find(|f| f.version.as_deref().is_some_and(|v| v.eq_ignore_ascii_case(r)));
                out += &match known {
                    Some(f) => format!("\n     ROM {r}: VPS lists it ({})", f.id),
                    None => format!("\n     ROM {r}: not among the {} VPS lists for the game", g.rom_files.len()),
                };
            }
        }
        None => out += "VPS: no game matched",
    }
    let _ = db;
    out
}

/// Why a table matched as it did: its clues, the nearest games, and every table file's distances.
fn explain_match(facts: &library::table::TableFacts, m: &Match, db: &Database) {
    let c = &facts.clues;
    println!("     clues: file `{}`, info name {:?}, authors {:?}, version {:?}, made {:?}", c.file_name, c.info_name, c.info_authors, c.info_version, c.made);
    let p = &m.parts;
    println!("     parts: name {:?}, maker {:?}, year {}, extra {:?}, version {:?} (cleaned {:?})", p.table_name, p.manufacturer, p.year, p.extra, p.version, m.version);
    let fresh = library::matcher::parse_filename(&c.file_name);
    for (g, d) in library::matcher::closest_games(&["VPX"], &fresh, &db.games).iter().take(5) {
        println!("     game {:.3}  {} ({} {}) `{}`", d, g.name, g.manufacturer, g.year.unwrap_or(0), g.id);
    }
    // the nearest by the table info's name, and whether the file name says something of each
    if let Some(info) = c.info_name.as_deref() {
        for (g, d) in library::matcher::closest_games(&["VPX"], &library::matcher::parse_filename(info), &db.games).iter().take(5) {
            println!("     by its info: game {:.3}  {} ({} {}) `{}`, the file name says something of it: {}", d, g.name, g.manufacturer, g.year.unwrap_or(0), g.id, library::matcher::relates(&c.file_name, g));
        }
    }
    if let (Some(g), Some(q)) = (m.game, &m.asked) {
        println!(
            "     table files weighed against name {:?}, the table info's authors {:?}, the file name's {:?}, version {:?} (the file name's: {})",
            q.name, q.info_authors, q.name_authors, q.version, q.version_from_name
        );
        let named = library::matcher::of_named_version(g, &["VPX"], q);
        if !named.is_empty() {
            println!("     of the file name's version: {}", named.iter().map(|t| format!("`{}`", t.id)).collect::<Vec<_>>().join(", "));
        }
        for s in library::matcher::table_file_scores(g, &["VPX"], q) {
            let t = s.file;
            let days = c.made.zip(t.created_at).map_or(String::new(), |(a, b)| format!(", {:.0} days apart", (b - a) as f64 / 86_400_000.0));
            println!(
                "     file {:.3} (name {:.2}, authors {:.2}, version {:.2})  `{}` {} by {} — {:?}{days}",
                s.total,
                s.name,
                s.authors,
                s.version,
                t.id,
                t.version.as_deref().unwrap_or("?"),
                t.authors.join(", "),
                t.comment.as_deref().unwrap_or("").chars().take(60).collect::<String>()
            );
        }
    }
}

fn identify(path: &Path, db: &Database) -> Result<(), String> {
    let listing = archive::list(path)?;
    let contents = analyse(&listing);
    match &listing.source {
        Source::Folder(_) => println!("{}: a folder, {} files", path.display(), contents.files.len()),
        Source::Archive(a) if a.parts.len() > 1 => println!("{}: a {} in {} parts, {} files", path.display(), a.format.describe(), a.parts.len(), contents.files.len()),
        Source::Archive(a) => println!("{}: a {}, {} files", path.display(), a.format.describe(), contents.files.len()),
        Source::File(_) => println!("{}: a file", path.display()),
    }
    for (kind, n) in contents.summary() {
        println!("  {:<40} {n}", kind.describe());
    }
    if let Some(r) = &contents.readme {
        let first: String = r.lines().filter(|l| !l.trim().is_empty()).take(3).collect::<Vec<_>>().join(" / ");
        println!("  read-me: {}", first.chars().take(200).collect::<String>());
    }

    // the table, its clues: its table info and script read, wherever it is
    let tables = contents.tables();
    let table_entry = tables.first().map(|e| (*e).clone());
    let table_stem = table_entry.as_ref().map(|e| {
        let name = e.path.rsplit('/').next().unwrap_or(&e.path);
        name.rsplit_once('.').map_or(name, |(s, _)| s).to_string()
    });
    let read = table_entry.as_ref().map(|e| library::table::read_in(&listing, e));
    let (clues, game_name, pup_name, helpers) = match read {
        Some(Ok(t)) => (t.clues, t.game_name, t.pup_names.into_iter().next(), t.helpers),
        Some(Err(e)) => {
            println!("  the table file couldn't be read ({e}): matched by its name alone");
            (Clues { file_name: table_stem.clone().unwrap_or_default(), ..Default::default() }, None, None, Vec::new())
        }
        None => (Clues::default(), None, None, Vec::new()),
    };
    if tables.len() > 1 {
        println!("  {} tables in it: the first matched, the rest each to drop by itself", tables.len());
    }
    let hints = contents.rom_hints();
    let rom = game_name.clone().or_else(|| hints.first().cloned());
    let layout = Layout::reference();
    // with no table, the table its companions are named for, found among those installed
    let mut for_table = table_stem.clone();
    let companion = contents.files.iter().find(|(_, k)| matches!(k, Kind::Backglass | Kind::Pov | Kind::Physics | Kind::TableSettings | Kind::Script | Kind::BackglassPlaces));
    if table_entry.is_none()
        && let Some((e, _)) = companion
    {
        let name = e.path.rsplit('/').next().unwrap_or(&e.path);
        let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
        match library::matcher::game_by_file_name(db, stem) {
            Some((g, d)) if d <= library::matcher::CONFIRMING => {
                println!("VPS: for {} ({} {}) — game {} {GAME_LINK}{}, by its file name", g.name, g.manufacturer, g.year.map_or(String::new(), |y| y.to_string()), g.id, g.id);
                let mut installed: Vec<(String, f64)> = library::installed::table_files(&layout.tables)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                    .map(|s| {
                        let d = library::matcher::file_name_distance(g, &s);
                        (s, d)
                    })
                    .filter(|(_, d)| *d <= library::matcher::CONFIRMING)
                    .collect();
                installed.sort_by(|a, b| a.1.total_cmp(&b.1));
                match installed.as_slice() {
                    [] => println!("     no table of it installed: which table it's for, to choose"),
                    [(t, _)] => println!("     for the table installed of it: {t}"),
                    [(t, _), rest @ ..] => println!("     for the nearest of {} tables installed of it: {t} (or {})", rest.len() + 1, rest.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join(", ")),
                }
                for_table = installed.first().map(|(t, _)| t.clone());
            }
            _ => println!("VPS: no game near its name: which table it's for, to choose"),
        }
    }
    if table_entry.is_some() {
        let m = match_table(db, &clues);
        println!("{}", describe(&m, rom.as_deref(), db));
        if let Some(g) = m.game {
            let lists = [
                ("backglass", "backglasses", g.b2s_files.len()),
                ("ROM", "ROMs", g.rom_files.len()),
                ("PinUP pack", "PinUP packs", g.pup_pack_files.len()),
                ("AltSound pack", "AltSound packs", g.alt_sound_files.len()),
                ("colouring", "colourings", g.alt_color_files.len()),
                ("POV file", "POV files", g.pov_files.len()),
            ];
            let listed: Vec<String> = lists.iter().filter(|(_, _, n)| *n > 0).map(|(one, many, n)| format!("{n} {}", if *n == 1 { one } else { many })).collect();
            if !listed.is_empty() {
                println!("     VPS also lists for the game: {}", listed.join(", "));
            }
        }
    } else if let Some(r) = &rom {
        // no table: the game whose ROMs, packs or colourings name this ROM
        let owners: Vec<&library::vps::Game> = db
            .games
            .iter()
            .filter(|g| g.rom_files.iter().chain(&g.alt_color_files).chain(&g.alt_sound_files).chain(&g.pup_pack_files).any(|f| f.version.as_deref().is_some_and(|v| v.eq_ignore_ascii_case(r)) || f.folder.as_deref().is_some_and(|v| v.eq_ignore_ascii_case(r))))
            .collect();
        match owners.as_slice() {
            [] => println!("VPS: no game lists `{r}`"),
            [g] => println!("VPS: for {} ({} {}) — game {} {GAME_LINK}{}", g.name, g.manufacturer, g.year.map_or(String::new(), |y| y.to_string()), g.id, g.id),
            many => println!("VPS: `{r}` is listed for {} games: {}", many.len(), many.iter().map(|g| g.name.as_str()).collect::<Vec<_>>().join(", ")),
        }
    }

    println!("Where each would go (nothing written):");
    let steps = plan(&contents, &listing.source, for_table.as_deref(), rom.as_deref(), pup_name.as_deref(), &helpers, &layout);
    // a pack's many files in one line each
    let mut shown_packs: Vec<Kind> = Vec::new();
    for s in &steps {
        let many = matches!(s.kind, Kind::PupPack | Kind::AltSound | Kind::DmdFolder | Kind::Music);
        if many {
            if shown_packs.contains(&s.kind) {
                continue;
            }
            shown_packs.push(s.kind);
            let of_kind: Vec<&library::plan::Step> = steps.iter().filter(|o| o.kind == s.kind).collect();
            let to = library::plan::common_folder(of_kind.iter().filter_map(|o| o.to.as_deref())).map_or(String::from("—"), |t| t.display().to_string());
            let (same, different) = of_kind.iter().fold((0, 0), |(a, b), o| match there(o) {
                There::SameSize => (a + 1, b),
                There::Different | There::Named => (a, b + 1),
                There::Nothing => (a, b),
            });
            let mut known = String::new();
            if same + different > 0 {
                known = format!("  ({same} there already, {different} different there)");
            }
            println!("  {} ({} files) -> {to}{known}{}", s.kind.describe(), of_kind.len(), s.note.as_ref().map_or(String::new(), |n| format!("  [{n}]")));
            continue;
        }
        let to = s.to.as_ref().map_or(String::from("—"), |t| t.display().to_string());
        let there = match there(s) {
            There::Nothing => "",
            There::SameSize => "  (there already, the same size)",
            There::Different => "  (a different file there: it would be replaced)",
            There::Named => "  (one of that name there already, to compare when it's made)",
        };
        println!("  {} -> {to}{there}{}", s.from, s.note.as_ref().map_or(String::new(), |n| format!("  [{n}]")));
    }
    Ok(())
}

/// A table's VPS game and table file, by ID.
type Ids = (Option<String>, Option<String>);

/// Matches every table in a folder, beside the tested-tables page's IDs and the test library's
/// links (the table files the test library names: the answer where it's given) when asked.
fn match_all(folder: &Path, db: &Database, compare: Option<&Path>, key: Option<&Path>, report: Option<&Path>) -> Result<(), String> {
    let expected = compare.map(read_tested_tables).transpose()?.unwrap_or_default();
    let answers = key.map(read_test_library).transpose()?.unwrap_or_default();
    let mut tables: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|e| format!("{}: {e}", folder.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("vpx")))
        .collect();
    tables.sort();
    let mut lines = vec![
        "# VPS matching".to_string(),
        String::new(),
        format!(
            "Every table in `{}` matched to VPS's database by `vpi-library match-all` (VPin Studio's matcher, ported, with ours on top), beside the tested-tables page's IDs and the test library's links, the table files sent for download, where given. A game found further than VPin Studio's confirming distance is a guess, for the user to confirm.",
            folder.display()
        ),
        String::new(),
        "| Table | VPS game | Table file | ROM | Beside the tested-tables page | Beside the test library |".to_string(),
        "|---|---|---|---|---|---|".to_string(),
    ];
    let (mut games_same, mut files_same, mut differ, mut compared) = (0, 0, 0, 0);
    let (mut keyed, mut ours_right, mut page_right) = (0, 0, 0);
    for path in &tables {
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let (facts, error) = match library::table::read(path) {
            Ok(f) => (f, None),
            Err(e) => (library::table::TableFacts { clues: Clues { file_name: stem.clone(), ..Default::default() }, ..Default::default() }, Some(e)),
        };
        let m = match_table(db, &facts.clues);
        let how = match m.found {
            Found::ByFileName => String::new(),
            Found::ByTableInfo => " (by its table info)".into(),
            Found::ByRom => " (by its ROM)".into(),
            Found::Guess => format!(" (a guess, {:.1})", m.distance),
            Found::Loosely => " (looser)".into(),
        };
        let game = m.game.map_or("—".to_string(), |g| format!("{} `{}`{how}", g.name, g.id));
        let file = m.table_file.map_or("—".to_string(), |t| format!("`{}` ({})", t.id, t.version.as_deref().unwrap_or("")));
        let rom = facts.game_name.clone().unwrap_or_default();
        let (ours_game, ours_file) = (m.game.map(|x| x.id.clone()), m.table_file.map(|x| x.id.clone()));
        let page = expected.iter().find(|(name, _)| *name == stem).map(|(_, ids)| ids.clone());
        let beside = match &page {
            Some((g, f)) => {
                compared += 1;
                let (gs, fs) = (ours_game == *g, ours_file == *f);
                games_same += usize::from(gs);
                files_same += usize::from(fs);
                if gs && fs {
                    "same".to_string()
                } else {
                    differ += 1;
                    format!("differs: the page says game {} and table file {}", g.as_deref().unwrap_or("none"), f.as_deref().unwrap_or("none"))
                }
            }
            None => String::new(),
        };
        // the library's answer for the game either found
        let answer = [page.as_ref().and_then(|(g, _)| g.clone()), ours_game.clone()].into_iter().flatten().find_map(|g| answers.iter().find(|(ag, _)| *ag == g).cloned());
        let checked = match &answer {
            Some((g, f)) => {
                keyed += 1;
                let right = ours_game.as_deref() == Some(g) && ours_file.as_deref() == Some(f);
                ours_right += usize::from(right);
                page_right += usize::from(page.as_ref().is_some_and(|(pg, pf)| pg.as_deref() == Some(g) && pf.as_deref() == Some(f)));
                if right { "right".to_string() } else { format!("**not**: the library's is game `{g}`, table file `{f}`") }
            }
            None => String::new(),
        };
        let note = error.map_or(String::new(), |e| format!(" (not read: {e})"));
        lines.push(format!("| {stem}{note} | {game} | {file} | {rom} | {beside} | {checked} |"));
    }
    lines.push(String::new());
    if compare.is_some() {
        lines.push(format!("Beside the tested-tables page: {compared} tables compared; the same game for {games_same}, the same table file for {files_same}; {differ} differ."));
    }
    if key.is_some() {
        lines.push(String::new());
        lines.push(format!("Beside the test library's links: {keyed} tables; the matcher found the library's game and table file for {ours_right}, the tested-tables page for {page_right}."));
    }
    let text = lines.join("\n") + "\n";
    match report {
        Some(file) => {
            std::fs::write(file, &text).map_err(|e| format!("{}: {e}", file.display()))?;
            println!("{} tables matched; report in {}", tables.len(), file.display());
            if compare.is_some() {
                println!("beside the tested-tables page: {compared} compared, same game {games_same}, same table file {files_same}, {differ} differ");
            }
            if key.is_some() {
                println!("beside the test library's links: {keyed} tables, the matcher right on {ours_right}, the page on {page_right}");
            }
        }
        None => print!("{text}"),
    }
    Ok(())
}

/// The test library's links: each table's VPS game and the table file sent for download.
fn read_test_library(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let link = regex::Regex::new(r"virtualpinballspreadsheet\.github\.io/tables\?game=([A-Za-z0-9_-]+)&fileType=tables&fileId=([A-Za-z0-9_-]+)").map_err(|e| e.to_string())?;
    Ok(link.captures_iter(&text).map(|c| (c[1].to_string(), c[2].to_string())).collect())
}

/// The tested-tables page's rows: our file's name, its VPS game's ID and table file's ID.
fn read_tested_tables(path: &Path) -> Result<Vec<(String, Ids)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let id = |cell: &str| -> Option<String> {
        let start = cell.rfind('`')?;
        let before = &cell[..start];
        let open = before.rfind('`')?;
        Some(cell[open + 1..start].to_string())
    };
    let first_id = |cell: &str| -> Option<String> {
        let open = cell.find('`')?;
        let close = cell[open + 1..].find('`')?;
        Some(cell[open + 1..open + 1 + close].to_string())
    };
    Ok(text
        .lines()
        .filter(|l| l.starts_with("| ") && !l.starts_with("| Table") && !l.starts_with("|---"))
        .filter_map(|l| {
            let cells: Vec<&str> = l.trim().trim_matches('|').split('|').map(str::trim).collect();
            (cells.len() >= 3).then(|| (cells[0].to_string(), (id(cells[1]), first_id(cells[2]))))
        })
        .collect())
}

/// A drop sorted out as the app's drop window shows it, the library's tables known (from what the
/// app remembers of them): its downloads, each one's table or the table it's for, its rows. Nothing
/// is copied.
fn drop_report(paths: &[PathBuf], db: &Database) -> Result<(), String> {
    // the library's tables as the app remembers them, brought up to date as the app does
    let path = library::installed::Remembered::default_path().ok_or("no APPDATA folder")?;
    let mut remembered = library::installed::Remembered::load(&path);
    library::installed::scan(&Layout::reference().tables, &mut remembered, |_, _, _| {})?;
    remembered.save(&path)?;
    let known: Vec<library::drop::Known> = remembered
        .tables
        .iter()
        .map(|(path, read)| {
            let stem = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let m = match_table(db, &read.clues(&stem));
            library::drop::Known {
                vps_game: m.game.filter(|_| m.found.sure()).map(|g| g.id.clone()),
                stem,
                game_name: read.game_name.clone(),
                pup_names: read.pup_names.clone(),
                helpers: read.helpers.clone(),
                music: read.music.clone(),
                file: Some(PathBuf::from(path)),
            }
        })
        .collect();
    let downloads = library::drop::inspect(paths, db, &known, &Layout::reference(), library::drop::Tie::Found, &[]);
    println!("{} download(s), {} tables in the library known", downloads.len(), known.len());
    for d in &downloads {
        println!();
        println!("{} ({})", d.name, if d.what.is_empty() { "unreadable" } else { &d.what });
        if let Some(e) = &d.error {
            println!("  couldn't be read: {e}");
            continue;
        }
        match (&d.table, &d.for_table) {
            (Some(t), _) => println!(
                "  table: {} -> {} ({} {}) {}{}",
                t.file,
                t.game.as_deref().unwrap_or("not on VPS"),
                t.maker.as_deref().unwrap_or(""),
                t.year.map_or(String::new(), |y| y.to_string()),
                t.version.as_deref().map_or(String::new(), |v| format!("release {v}, ")),
                t.how
            ),
            (None, Some(f)) => println!("  for: {f} ({})", d.for_note.as_deref().unwrap_or("")),
            (None, None) => println!("  for: — ({})", d.for_note.as_deref().unwrap_or("")),
        }
        if let Some(r) = &d.rom {
            println!("  machine: {r}");
        }
        for n in &d.notes {
            println!("  note: {n}");
        }
        for r in &d.rows {
            let to = r.to.as_ref().map_or("—".to_string(), |t| t.display().to_string());
            let files = if r.files > 1 { format!(" ({} files)", r.files) } else { String::new() };
            // a media card: its shape, and the app's guess and why
            let guess = r.media.as_ref().map_or(String::new(), |m| {
                let size = m.width.zip(m.height).map_or(String::new(), |(w, h)| format!("{w}x{h}, "));
                format!("  [{size}{}]", match (m.guess, m.what, m.why) {
                    (Some(place), Some(what), Some(why)) => format!("our guess: {what} -> {place}, by its {why}"),
                    (None, Some(what), _) => format!("looks like a {what}, no place for it here: the table's media"),
                    _ => "no guess: the table's media".to_string(),
                })
            });
            println!("  [{}] {:<8} {}: {}{} -> {}{}{guess}", if r.include { "x" } else { " " }, r.state, r.label, r.name, files, to, r.note.as_ref().map_or(String::new(), |n| format!("  [{n}]")));
            if let Some(c) = &r.check {
                println!("             checked ({}): {}", c.look, c.text);
            }
        }
    }
    Ok(())
}

/// Seconds since 1970 as a UTC date and time, `2025-01-02 21-14-05`, as the app names what it keeps
/// (Howard Hinnant's days-to-date algorithm).
fn utc(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}-{:02}-{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}
