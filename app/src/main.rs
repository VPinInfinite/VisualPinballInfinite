// Visual Pinball Infinite's app: a window drawn by Windows' web viewer showing the
// screens in ui/, our engine underneath — the library read on a thread of its own, the pictures by a
// protocol of the app's own, PLAY through our player.
#![windows_subsystem = "windows"]

mod added;
mod altsound_make;
mod cabinet;
mod achievements;
mod achievements_editor;
mod colouring;
mod vpidb;
mod controls;
mod dof_config;
mod drop;
mod ini_page;
mod inspector;
mod keep_shape;
mod layout_switch;
mod library;
mod location;
mod media_facts;
mod own_pictures;
mod physics;
mod pick;
mod pictures;
mod play;
mod popper_media;
mod pup_packs;
mod report;
mod rom_checks;
mod roms_page;
mod screens;
mod script_editor;
mod screenshots;
mod setup;
mod shell;
mod shot;
mod sound;
mod table_settings;
mod tasks;
mod tunes;
mod updates;

use serde::Serialize;
use tauri::webview::Color;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use library::{Shared, Status, TableView};
use shot::Shot;

#[derive(Serialize)]
struct LibraryReply<'a> {
    status: &'a Status,
    tables: Vec<TableView<'a>>,
    playlists: &'a [library::Playlist],
    /// PinUP Popper's media folder is there, for its media to be put in.
    popper: bool,
    /// The front ends whose lists the ratings set here can go in, by name.
    #[serde(rename = "ratingFronts")]
    rating_fronts: Vec<&'static str>,
}

/// The library as it stands, with how far its reading has come.
#[tauri::command]
fn library(shared: State<'_, Shared>) -> serde_json::Value {
    let st = shared.0.lock().unwrap();
    let popper = st.layout.as_ref().and_then(::library::installed::pop_media).is_some();
    let reply = LibraryReply { status: &st.status, tables: st.tables.iter().map(|t| library::view(&st, t)).collect(), playlists: &st.user.playlists, popper, rating_fronts: library::rating_fronts(&st) };
    serde_json::to_value(reply).unwrap_or_default()
}

/// A table rated (none: cleared); what went wrong putting it in a front end's list, in words.
#[tauri::command]
async fn set_rating(app: AppHandle, id: String, stars: Option<u8>) -> Result<Vec<String>, String> {
    let handle = app.clone();
    let problems = tauri::async_runtime::spawn_blocking(move || library::set_rating(&mut handle.state::<Shared>().0.lock().unwrap(), &id, stars)).await.map_err(|e| e.to_string())??;
    vpidb::library_changed(&app);
    Ok(problems)
}

/// Every rating set here put in the front ends' lists, once the user wants them there too.
#[tauri::command]
async fn ratings_to_fronts(app: AppHandle) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || library::send_all_ratings(&mut app.state::<Shared>().0.lock().unwrap())).await.map_err(|e| e.to_string())?
}

/// Every file VPS lists for a table's game, by kind.
#[tauri::command]
fn vps_files(shared: State<'_, Shared>, id: String) -> serde_json::Value {
    let st = shared.0.lock().unwrap();
    let files = st.tables.iter().find(|t| t.id == id).map(|t| library::vps_files(&st, t)).unwrap_or_default();
    serde_json::to_value(files).unwrap_or_default()
}

/// Every machine VPS lists, for the VPS browser.
#[tauri::command]
fn vps_browser(shared: State<'_, Shared>) -> library::Browser {
    library::browser(&shared.0.lock().unwrap())
}

/// A VPS game and every file VPS lists for it, for the drop window's "See them".
#[tauri::command]
fn vps_game(shared: State<'_, Shared>, game: String, file: Option<String>) -> serde_json::Value {
    let st = shared.0.lock().unwrap();
    serde_json::to_value(library::vps_game(&st, &game, file.as_deref())).unwrap_or_default()
}

#[tauri::command]
fn set_favourite(shared: State<'_, Shared>, id: String, on: bool) -> Result<(), String> {
    let mut st = shared.0.lock().unwrap();
    let key = library::key(&st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?.path);
    if on {
        st.user.favourites.insert(key);
    } else {
        st.user.favourites.remove(&key);
    }
    st.user.save()
}

/// Cabinet mode opened on the cabinet's screens. Its windows are made off the window's own thread,
/// as every command making windows is: made from a command on it, Windows' web view waits on that
/// thread for good and the app stops answering.
#[tauri::command]
async fn cabinet_open(app: AppHandle, preset: Option<String>) -> Result<Vec<&'static str>, String> {
    cabinet::open(&app, preset.as_deref())
}

/// Cabinet mode closed, the app's window as it was (off the window's thread too).
#[tauri::command]
async fn cabinet_close(app: AppHandle) -> bool {
    cabinet::close(&app)
}

/// What the cabinet's other screens show: the table chosen's backglass, score display and topper.
#[tauri::command]
fn cabinet_show(app: AppHandle, what: serde_json::Value) {
    cabinet::show(&app, what);
}

/// What the cabinet's other screens show now, for one just opened.
#[tauri::command]
fn cabinet_showing(state: State<'_, cabinet::Showing>) -> serde_json::Value {
    state.0.lock().unwrap().clone()
}

/// The cabinet's other screens shown, or put away while a table plays.
#[tauri::command]
async fn cabinet_screens(app: AppHandle, shown: bool) {
    cabinet::screens_shown(&app, shown);
}

/// Whether the app starts with Windows.
#[tauri::command]
fn starts_with_windows() -> bool {
    cabinet::starts_with_windows()
}

/// The app started with Windows, or no longer.
#[tauri::command]
fn set_start_with_windows(on: bool) -> Result<(), String> {
    cabinet::start_with_windows(on)
}

/// The PC shut down, from cabinet mode's menu, once asked.
#[tauri::command]
fn shut_down() -> Result<(), String> {
    let mut cmd = std::process::Command::new("shutdown.exe");
    cmd.args(["/s", "/t", "0"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Every PinUP pack, for Settings › PinUP packs: its tables, layouts and check.
#[tauri::command]
async fn pup_packs(app: AppHandle) -> Result<Vec<pup_packs::Pack>, String> {
    // each pack's option files read and its layouts' files compared: off the window's thread
    tauri::async_runtime::spawn_blocking(move || pup_packs::all(&app)).await.map_err(|e| e.to_string())
}

/// One PinUP pack, for its Layout window.
#[tauri::command]
async fn pup_pack(app: AppHandle, path: String) -> Result<pup_packs::Pack, String> {
    tauri::async_runtime::spawn_blocking(move || pup_packs::one(&app, &path)).await.map_err(|e| e.to_string())?
}

/// One of a PinUP pack's layouts applied, the app making its option file's copies itself.
#[tauri::command]
async fn pup_use_layout(app: AppHandle, path: String, file: String) -> Result<pup_packs::Used, String> {
    tauri::async_runtime::spawn_blocking(move || pup_packs::use_layout(&app, &path, &file)).await.map_err(|e| e.to_string())?
}

/// A table started with one of its PinUP pack's layouts tried, the pack left as it is.
#[tauri::command]
fn try_layout(app: AppHandle, id: String, path: String, file: String) -> Result<(), String> {
    let trying = pup_packs::trying(&app, &path, &file)?;
    play::try_layout(&app, &id, trying)
}

/// A PinUP pack's folder shown in Explorer, or one of its option files there.
#[tauri::command]
fn pup_pack_show(app: AppHandle, path: String, file: Option<String>) -> Result<(), String> {
    pup_packs::show(&app, &path, file.as_deref())
}

/// A PinUP pack no table plays renamed to the name a table's script asks for.
#[tauri::command]
async fn pup_pack_rename(app: AppHandle, path: String, to: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pup_packs::rename(&app, &path, &to)).await.map_err(|e| e.to_string())?
}

/// A PinUP pack renamed here put back to its name before.
#[tauri::command]
async fn pup_pack_undo_rename(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pup_packs::undo_rename(&app, &path)).await.map_err(|e| e.to_string())?
}

/// A PinUP pack put in the Recycle Bin.
#[tauri::command]
async fn pup_pack_remove(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || pup_packs::remove(&app, &path)).await.map_err(|e| e.to_string())?
}

/// Whether a table's PinUP pack plays for it, and whether packs play for every table.
#[tauri::command]
fn table_pup(shared: State<'_, Shared>, id: String) -> Result<(bool, bool), String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    Ok(table_settings::pup_of(&t.path))
}

/// A table's PinUP pack on or off for it alone, in its own settings.
#[tauri::command]
fn set_table_pup(shared: State<'_, Shared>, id: String, on: bool) -> Result<(), String> {
    let path = shared.0.lock().unwrap().tables.iter().find(|t| t.id == id).map(|t| t.path.clone()).ok_or("That table isn't in the library any more.")?;
    table_settings::set_pup(&path, on)
}

/// A table hidden from the library, or shown again.
#[tauri::command]
fn set_hidden(shared: State<'_, Shared>, id: String, on: bool) -> Result<(), String> {
    let mut st = shared.0.lock().unwrap();
    let key = library::key(&st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?.path);
    if on {
        st.user.hidden_tables.insert(key);
    } else {
        st.user.hidden_tables.remove(&key);
    }
    st.user.save()
}

/// A table's details as the user set them ("Edit its details"), or VPS's again for none.
#[tauri::command]
fn set_details(app: AppHandle, shared: State<'_, Shared>, id: String, details: library::Details) -> Result<(), String> {
    library::set_details(&mut shared.0.lock().unwrap(), &id, details)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// The user's word on a guessed VPS match: confirmed, or not on VPS.
#[tauri::command]
fn choose_vps(app: AppHandle, shared: State<'_, Shared>, id: String, confirm: bool) -> Result<(), String> {
    library::choose_vps(&mut shared.0.lock().unwrap(), &id, confirm)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// A table's ROM choices, for its Change window.
#[tauri::command]
async fn rom_choices(app: AppHandle, id: String) -> Result<library::RomChoices, String> {
    // PinMAME is asked each set's name: off the window's thread
    tauri::async_runtime::spawn_blocking(move || library::rom_choices(&app.state::<Shared>(), &id)).await.map_err(|e| e.to_string())?
}

/// What changing a table's ROM would change, in words.
#[tauri::command]
fn rom_effects(shared: State<'_, Shared>, id: String, to: String) -> Result<Vec<String>, String> {
    library::rom_effects(&shared.0.lock().unwrap(), &id, &to)
}

/// A table's ROM changed, or put back (`to` none), then the library read again.
#[tauri::command]
async fn change_rom(app: AppHandle, id: String, to: Option<String>) -> Result<(), String> {
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            match to {
                Some(to) => library::change_rom(&mut st, &id, &to),
                None => library::undo_rom(&mut st, &id),
            }
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    library::start(app);
    Ok(())
}

/// A table's screenshots, the newest first.
#[tauri::command]
fn screenshots(shared: State<'_, Shared>, id: String) -> Vec<screenshots::Shot> {
    let st = shared.0.lock().unwrap();
    st.tables.iter().find(|t| t.id == id).map(|t| screenshots::list(&t.stem, &t.id)).unwrap_or_default()
}

/// A table's VPS match chosen by hand, kept as VPI's own.
#[tauri::command]
fn set_vps(app: AppHandle, shared: State<'_, Shared>, id: String, game: String, file: Option<String>) -> Result<(), String> {
    library::set_vps(&mut shared.0.lock().unwrap(), &id, game, file)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// What "Overwrite for all players" would rename, for the screens to show first: each file's
/// place now and after.
#[tauri::command]
fn vps_names(shared: State<'_, Shared>, id: String, game: String) -> Result<library::Renames, String> {
    let st = shared.0.lock().unwrap();
    library::vps_names(&st, &id, &game)
}

/// A picture or video on a table's page deleted (to the Recycle Bin), or one of VPS's hidden from
/// it: `deleted` or `hidden`.
#[tauri::command]
async fn delete_media(app: AppHandle, id: String, url: String) -> Result<String, String> {
    let done = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || library::delete_media(&mut app.state::<Shared>().0.lock().unwrap(), &id, &url)).await.map_err(|e| e.to_string())??
    };
    let _ = app.emit("library-changed", ());
    Ok(done.to_string())
}

/// VPS's pictures hidden from a table's page shown on it again.
#[tauri::command]
fn unhide_media(app: AppHandle, shared: State<'_, Shared>, id: String) -> Result<(), String> {
    library::unhide_media(&mut shared.0.lock().unwrap(), &id)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// "Overwrite for all players": the table matched to VPS and renamed to VPS's name for it, for
/// every program, then the library read again. Its new ID, for its page.
#[tauri::command]
async fn overwrite_vps(app: AppHandle, id: String, game: String, file: Option<String>) -> Result<String, String> {
    let handle = app.clone();
    let new_id = tauri::async_runtime::spawn_blocking(move || library::overwrite_vps(&mut handle.state::<Shared>().0.lock().unwrap(), &id, game, file))
        .await
        .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(new_id)
}

/// What removing a table could take, each ticked or not to start with.
#[tauri::command]
async fn remove_plan(app: AppHandle, id: String) -> Result<Vec<::library::remove::Item>, String> {
    tauri::async_runtime::spawn_blocking(move || library::remove_plan(&app.state::<Shared>().0.lock().unwrap(), &id)).await.map_err(|e| e.to_string())?
}

/// The ticked of what removing a table could take (a whole table, or one file from its page) moved
/// to the Recycle Bin or taken out, then the library read again; what was done.
#[tauri::command]
async fn remove(app: AppHandle, id: String, keys: Vec<String>) -> Result<::library::remove::Done, String> {
    let handle = app.clone();
    let keys: std::collections::BTreeSet<String> = keys.into_iter().collect();
    let done = tauri::async_runtime::spawn_blocking(move || library::remove(&mut handle.state::<Shared>().0.lock().unwrap(), &id, &keys))
        .await
        .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(done)
}

/// A table renamed so given its names back, then the library read again. Its ID then.
#[tauri::command]
async fn put_names_back(app: AppHandle, id: String) -> Result<String, String> {
    let handle = app.clone();
    let back = tauri::async_runtime::spawn_blocking(move || library::put_names_back(&mut handle.state::<Shared>().0.lock().unwrap(), &id))
        .await
        .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(back)
}

/// The picture or video of a kind used for a table, where it has several (the blue check).
#[tauri::command]
fn choose_media(app: AppHandle, shared: State<'_, Shared>, id: String, kind: String, pick: String) -> Result<(), String> {
    library::choose_media(&mut shared.0.lock().unwrap(), &id, &kind, &pick)?;
    let _ = app.emit("library-changed", ());
    // and put where PinUP Popper shows it, when that's on
    popper_media::after(&app, &id, Some(&kind), false);
    Ok(())
}

/// A front end's playfield picture or video kept on its side turned the other way, on the user's
/// word (its window's Turn it the other way).
#[tauri::command]
fn turn_media(app: AppHandle, shared: State<'_, Shared>, id: String, pick: String, turn: u16) -> Result<(), String> {
    library::turn_media(&mut shared.0.lock().unwrap(), &id, &pick, turn)?;
    let _ = app.emit("library-changed", ());
    Ok(())
}

/// One kind of a table's media put where PinUP Popper shows it, on the user's word (its window's
/// Put it in Popper's folder), the setting on or not. How many moved.
#[tauri::command]
async fn popper_media_put(app: AppHandle, id: String, kind: String) -> Result<usize, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || popper_media::after(&handle, &id, Some(&kind), true).join().map_err(|_| "It stopped.".to_string())?.said())
        .await
        .map_err(|e| e.to_string())?
}

/// What putting every table's media in PinUP Popper's folders would do: how many of each kind.
#[tauri::command]
fn popper_media_plan(shared: State<'_, Shared>) -> Vec<(String, &'static str, usize)> {
    popper_media::plan(&shared.0.lock().unwrap())
}

/// Every table's chosen media put where PinUP Popper shows them, and PinballX and PinballY where
/// that's on (Tools › Media, on a word), then the library read again. How many moved.
#[tauri::command]
async fn popper_media_all(app: AppHandle) -> Result<usize, String> {
    let of = app.state::<Shared>().0.lock().unwrap().tables.len() as u32;
    let task = tasks::begin(&app, "Putting the chosen media into the front ends' folders", of);
    let handle = app.clone();
    let moved = tauri::async_runtime::spawn_blocking(move || {
        let per_table: Vec<Vec<popper_media::Job>> = {
            let shared = handle.state::<Shared>();
            let st = shared.0.lock().unwrap();
            st.tables.iter().map(|t| popper_media::jobs(&st, t, None, false)).collect()
        };
        let mut done = popper_media::Done::default();
        for (n, jobs) in per_table.into_iter().enumerate() {
            task.step(n as u32, None);
            popper_media::run(&handle, jobs, &mut done);
        }
        done.said()
    })
    .await
    .map_err(|e| e.to_string())?;
    library::start(app);
    moved
}

/// The user's own playlists, as the screens have them now (made, renamed, reordered, tables added or
/// taken off), kept.
#[tauri::command]
fn save_playlists(shared: State<'_, Shared>, playlists: Vec<library::Playlist>) -> Result<(), String> {
    let mut st = shared.0.lock().unwrap();
    st.user.playlists = playlists;
    st.user.save()
}

#[tauri::command]
fn prefs(shared: State<'_, Shared>) -> serde_json::Value {
    shared.0.lock().unwrap().user.prefs.clone()
}

#[tauri::command]
fn set_prefs(shared: State<'_, Shared>, prefs: serde_json::Value) -> Result<(), String> {
    let mut st = shared.0.lock().unwrap();
    // the front end a drop's media go in, as the user chose
    let front = library::media_front(&prefs);
    if let Some(layout) = st.layout.as_mut() {
        layout.media_front = front;
    }
    st.user.prefs = prefs;
    st.user.save()
}

/// A front end there for the library, for Settings › Media: its key, its name, its media folder.
#[derive(Serialize)]
struct FrontEnd {
    key: &'static str,
    name: &'static str,
    folder: String,
}

/// The front ends there for the library (PinUP Popper's media folder, PinballX and PinballY where
/// either plays its tables), and the one a drop's media go in.
#[tauri::command]
async fn front_ends(shared: State<'_, Shared>) -> Result<(Vec<FrontEnd>, Option<&'static str>), String> {
    let layout = shared.0.lock().unwrap().layout.clone().ok_or("The library isn't read yet.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let there = ::library::frontends::there(&layout).into_iter().map(|(f, folder)| FrontEnd { key: f.key(), name: f.name(), folder: folder.display().to_string() }).collect();
        (there, ::library::frontends::media_target(&layout).map(|t| t.front().key()))
    })
    .await
    .map_err(|e| e.to_string())
}

/// PLAY; `mode`, `hardcore` (Play Hardcore) or `softcore` (Play Softcore), else as it stands.
#[tauri::command]
fn play(app: AppHandle, id: String, preset: Option<String>, stay: Option<bool>, mode: Option<String>) -> Result<(), String> {
    let asked = match mode.as_deref() {
        Some("hardcore") => achievements::Asked::Hardcore,
        Some("softcore") => achievements::Asked::Softcore,
        _ => achievements::Asked::AsItStands,
    };
    play::play(&app, &id, preset.as_deref(), stay.unwrap_or(false), asked)
}

#[tauri::command]
fn open_link(url: String) -> Result<(), String> {
    shell::open_link(&url)
}

/// One of a table's files shown in Explorer, by what it is — never a path the screens give.
#[tauri::command]
fn show_file(shared: State<'_, Shared>, id: String, what: String) -> Result<(), String> {
    let path = {
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let h = &t.holds;
        let found = match what.as_str() {
            "table" => Some(t.path.clone()),
            "backglass" => h.backglass.clone(),
            "rom" => h.rom.clone(),
            "pupPack" => h.pup_pack.clone(),
            "altSound" => h.altsound.clone(),
            "colour" => h.colour.as_ref().map(|c| c.folder.clone()),
            "pov" => h.pov.clone(),
            "physics" => h.physics.clone(),
            "settings" => h.settings.clone(),
            "script" => h.script.clone(),
            // a picture our player made of it: `own:playfield`, `own:video` …
            m if m.starts_with("own:") => own_pictures::made(&t.id, &m[4..]).map(|(p, _)| p),
            m => {
                // one of a list: `media:2`, `music:0`, `helper:1`, `folder:0`, `beside:0`, `other:3`
                let (list, n) = m.split_once(':').and_then(|(l, n)| Some((l, n.parse::<usize>().ok()?))).ok_or("It isn't there.")?;
                match list {
                    "media" => h.media.get(n).map(|m| m.path.clone()),
                    "music" => h.music.get(n).cloned(),
                    "helper" => h.helpers.get(n).and_then(|x| x.path.clone()),
                    "folder" => h.folders.get(n).cloned(),
                    "beside" => h.beside.get(n).cloned(),
                    "backglasses" => h.backglasses.get(n).cloned(),
                    "pictures" => h.pictures.get(n).cloned(),
                    "other" => t.other.get(n).map(|f| f.path.clone()),
                    _ => None,
                }
            }
        };
        found.ok_or("It isn't there.")?
    };
    shell::show(&path)
}

/// Where each kind of file lives, for Settings › Library's folders (Folders
/// a part of the Library page): the layout the app reads and adds to today, VPX's own, and the
/// player's data folder.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Folder {
    key: &'static str,
    label: &'static str,
    path: String,
    there: bool,
}

fn folder_list(layout: &::library::plan::Layout) -> Vec<(&'static str, &'static str, std::path::PathBuf)> {
    let visual = layout.tables.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let ours = library::data_folder().unwrap_or_default();
    let mut list = vec![
        ("tables", "Tables, with their backglasses, POV, physics and settings files", layout.tables.clone()),
        ("scripts", "VPX’s scripts", layout.scripts.clone()),
        ("music", "Music the tables play", visual.join("Music")),
        ("roms", "ROMs", layout.vpinmame.join("roms")),
        ("altsound", "AltSound packs", layout.vpinmame.join("altsound")),
        ("colour", "Colorings", layout.vpinmame.join("altcolor")),
        ("pup", "PinUP packs", layout.pup_videos.clone()),
        ("popper", "PinUP Popper’s media", layout.pup_videos.parent().map(|p| p.join("POPMedia").join("Visual Pinball X")).unwrap_or_default()),
        ("memory", "The machines’ memory, our player’s", ours.join("PinMAME").join("nvram")),
        ("pictures", "Pictures and videos our player made", ours.join("Pictures")),
        ("screenshots", "Screenshots", ours.join("Screenshots")),
    ];
    // PinballX's and PinballY's media, where either plays the library's tables
    for f in ::library::frontends::find(layout) {
        let (key, label) = match f.front {
            ::library::frontends::Front::PinballX => ("pinballx", "PinballX’s media"),
            _ => ("pinbally", "PinballY’s media"),
        };
        list.push((key, label, f.media));
    }
    list
}

#[tauri::command]
fn folders(shared: State<'_, Shared>) -> Vec<Folder> {
    let st = shared.0.lock().unwrap();
    let Some(layout) = st.layout.as_ref() else { return Vec::new() };
    folder_list(layout).into_iter().map(|(key, label, path)| Folder { key, label, there: path.is_dir(), path: path.display().to_string() }).collect()
}

/// One of those folders opened in Explorer, by its key — never a path the screens give.
#[tauri::command]
fn show_folder(shared: State<'_, Shared>, key: String) -> Result<(), String> {
    let path = {
        let st = shared.0.lock().unwrap();
        let layout = st.layout.as_ref().ok_or("The library isn't read yet.")?;
        folder_list(layout).into_iter().find(|(k, _, _)| *k == key).map(|(_, _, p)| p).ok_or("It isn't there.")?
    };
    shell::show(&path)
}

/// A settings page made of plain settings (Picture, General): its settings as the file has them.
#[tauri::command]
async fn ini_values(keys: Vec<ini_page::Key>) -> Result<ini_page::View, String> {
    ini_page::load(&keys)
}

/// A settings page's changes saved, only those.
#[tauri::command]
async fn save_ini_values(sets: Vec<ini_page::Set>) -> Result<(), String> {
    ini_page::save(&sets)
}

/// The Advanced page: every setting the settings file holds.
#[tauri::command]
async fn all_settings() -> Result<Vec<(String, String, String)>, String> {
    ini_page::all()
}

/// The Advanced page's changes saved: any setting but the PIN2DMD key.
#[tauri::command]
async fn save_any_settings(sets: Vec<ini_page::Set>) -> Result<(), String> {
    ini_page::save_any(&sets)
}

/// The player's log of its last run, shown in Explorer.
#[tauri::command]
fn show_player_log() -> Result<(), String> {
    let log = library::data_folder().ok_or("no APPDATA folder")?.join("Logs").join("player.log");
    shell::show(&log)
}

/// The Screens page: the screens, and where the settings file puts the player's windows. Async,
/// as everything the player is asked runs off the window's own thread.
#[tauri::command]
async fn screens_view(app: AppHandle, table: Option<String>) -> Result<screens::ScreensView, String> {
    match table {
        // one table's own screens (its settings' View, Custom)
        Some(id) => screens::load_for_table(&table_path(&app, &id)?),
        None => screens::load(),
    }
}

/// The screen presets kept, by name.
#[tauri::command]
fn screen_presets() -> Vec<String> {
    screens::presets()
}

/// The Screens page's settings kept as a preset.
#[tauri::command]
fn save_screen_preset(name: String, settings: screens::ScreenSettings) -> Result<(), String> {
    screens::save_preset(&name, &settings)
}

/// A preset as the Screens page shows it.
#[tauri::command]
fn screen_preset(name: String) -> Result<screens::ScreenSettings, String> {
    screens::load_preset(&name)
}

/// A preset renamed, or deleted (`to` none).
#[tauri::command]
fn change_screen_preset(name: String, to: Option<String>) -> Result<(), String> {
    screens::change_preset(&name, to.as_deref())
}

/// A table's own settings: the options its script offers, its own choices over the usual, and the
/// machine's for its game.
#[tauri::command]
async fn table_settings(app: AppHandle, id: String) -> Result<table_settings::View, String> {
    let (path, game) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        (t.path.clone(), t.read.game_name.clone().filter(|_| t.read.pinmame))
    };
    tauri::async_runtime::spawn_blocking(move || table_settings::load(&app.state::<table_settings::Asked>(), &path, game)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_table_settings(
    app: AppHandle,
    id: String,
    options: std::collections::BTreeMap<String, Option<f32>>,
    for_table: table_settings::ForTable,
    machine: Option<(String, table_settings::MachineChoices)>,
) -> Result<(), String> {
    table_settings::save(&table_path(&app, &id)?, &options, &for_table, machine.as_ref().map(|(game, m)| (game.as_str(), m)))?;
    // its settings file shown on its page, made if it wasn't there
    library::start(app);
    Ok(())
}

/// What a table's high scores are read from: its ROM set, the names its script saves values under,
/// and the folders.
fn scores_of(app: &AppHandle, id: &str) -> Result<(Option<String>, Vec<String>, ::library::plan::Layout), String> {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let layout = st.layout.clone().ok_or("The library isn't read yet.")?;
    Ok((t.holds.rom_set.clone(), t.read.saved_under.clone(), layout))
}

/// A table's high scores, as our player keeps them and as VPX does.
#[tauri::command]
async fn high_scores(app: AppHandle, id: String) -> Result<::library::scores::HighScores, String> {
    let (rom, saved_under, layout) = scores_of(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || ::library::scores::for_table(&layout, rom.as_deref(), &saved_under)).await.map_err(|e| e.to_string())
}

/// Every table's high scores at once, for the player hub, read off the window's thread.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TableScores {
    id: String,
    scores: ::library::scores::HighScores,
}

#[tauri::command]
async fn high_scores_all(app: AppHandle) -> Result<Vec<TableScores>, String> {
    let (layout, wanted) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let layout = st.layout.clone().ok_or("The library isn't read yet.")?;
        let wanted: Vec<(String, Option<String>, Vec<String>)> = st.tables.iter().map(|t| (t.id.clone(), t.holds.rom_set.clone(), t.read.saved_under.clone())).collect();
        (layout, wanted)
    };
    tauri::async_runtime::spawn_blocking(move || wanted.into_iter().map(|(id, rom, saved)| TableScores { id, scores: ::library::scores::for_table(&layout, rom.as_deref(), &saved) }).collect())
        .await
        .map_err(|e| e.to_string())
}

/// A table's ROM checked file by file against what PinMAME expects, for its health check: how it
/// looks (good, warn, bad) and what it says.
#[tauri::command]
async fn rom_check(app: AppHandle, id: String) -> Result<(String, String), String> {
    let (rom, _, layout) = scores_of(&app, &id)?;
    let rom = rom.ok_or("It drives no machine.")?;
    // where the table keeps it: its own `pinmame\roms`, else VPinMAME's
    let own = table_path(&app, &id).ok().and_then(|p| p.parent().map(|d| d.join("pinmame").join("roms"))).filter(|d| d.join(format!("{rom}.zip")).is_file());
    let roms = own.unwrap_or_else(|| layout.vpinmame.join("roms"));
    tauri::async_runtime::spawn_blocking(move || ::library::roms::check_installed(&roms, &rom).map(|(look, text)| (look.to_string(), text)))
        .await
        .map_err(|e| e.to_string())?
}

/// A table's pictures made by our player, with no window, those wanted (the make-media window's
/// ticks): shown where VPS and PinUP Popper have none. The wheel the screens draw themselves.
#[tauri::command]
async fn make_pictures(app: AppHandle, id: String, wanted: own_pictures::Wanted) -> Result<(), String> {
    let path = table_path(&app, &id)?;
    if !wanted.kinds.iter().any(|k| k != "wheel") {
        return Ok(());
    }
    let name = table_name(&app, &id);
    // one table's made at a time (`own_pictures::turn`), the others said as waiting
    let task = tasks::waiting(&app, format!("Waiting to make {name}'s media"));
    let handle = app.clone();
    let made = id.clone();
    let remade = wanted.kinds.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _turn = own_pictures::turn();
        // VPX's memory of its machine taken first, as PLAY takes it; a machine never run here
        // started once before it's drawn (decided in its turn: one before it may have run it)
        let warm_up = library::before_a_run(&handle, &id);
        let steps = own_pictures::steps(&wanted, warm_up);
        task.start(format!("Making {name}'s media"), steps.iter().map(|(_, label)| (*label).to_string()).collect());
        own_pictures::make(&path, &id, &wanted, warm_up, &|kind| {
            if let Some(n) = steps.iter().position(|(k, _)| *k == kind) {
                task.done(n);
            }
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    // what was put in PinUP Popper's folders before is the old one now
    library::forget_popper_copies(&app, &made, &remade);
    let _ = app.emit("library-changed", ());
    // what it made that's used, put where PinUP Popper shows it, when that's on (the wheel once drawn)
    popper_media::after(&app, &made, None, false);
    Ok(())
}

/// A picture our player made of a table, by its kind, addressed with when it was made; None where
/// it has none (the table editor's look of the table from above).
#[tauri::command]
fn own_picture(id: String, kind: String) -> Option<String> {
    own_pictures::made(&id, &kind).map(|(_, when)| format!("{}o/{id}/{kind}?v={when}", pictures::base()))
}

/// Whether a table may reach outside its own files, its own word (None: as every table), kept in
/// the main settings file.
#[tauri::command]
fn save_reach_outside(app: AppHandle, id: String, value: Option<bool>) -> Result<(), String> {
    let path = table_path(&app, &id)?;
    table_settings::save_reach_outside(&path, value)
}

/// The screens, as the player lists them, for first setup.
#[tauri::command]
async fn screens_now() -> Result<Vec<screens::Screen>, String> {
    tauri::async_runtime::spawn_blocking(screens::screens).await.map_err(|e| e.to_string())?
}

/// Whether this build sends reports (Report a problem's Send), or only makes them to attach to a post.
#[tauri::command]
fn report_can_send() -> bool {
    report::can_send()
}

/// A report made here sent to the team, on the user's word (Report a problem's Send).
#[tauri::command]
async fn send_report(path: String, note: String, table: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || report::send(std::path::Path::new(&path), &note, table.as_deref())).await.map_err(|e| e.to_string())?
}

/// A newer version on the releases, if there is one (the one-click updater).
#[tauri::command]
async fn update_check(app: AppHandle) -> Result<Option<updates::Found>, String> {
    updates::check(&app).await
}

/// The newer version installed, on the user's word.
#[tauri::command]
async fn update_install(app: AppHandle) -> Result<(), String> {
    updates::install(&app).await
}

/// This PC's place from Windows' location service, on the user's click (General › Where you are).
#[tauri::command]
async fn this_pc_location() -> Result<(f64, f64), String> {
    tauri::async_runtime::spawn_blocking(location::this_pc).await.map_err(|e| e.to_string())?
}

/// What first setup offers: the VPX setup found here, a new library's folder, what's set now.
#[tauri::command]
async fn setup_offer() -> Result<setup::Offer, String> {
    tauri::async_runtime::spawn_blocking(setup::offer).await.map_err(|e| e.to_string())
}

/// A VPX setup in the folder the user chose (first setup): its tables, ROMs, scripts, PinUP packs.
#[tauri::command]
async fn setup_look(folder: String) -> Result<setup::Folders, String> {
    tauri::async_runtime::spawn_blocking(move || setup::look_in(std::path::Path::new(&folder))).await.map_err(|e| e.to_string())
}

/// A folder chosen in Windows' own folder picker over the app's window; none when cancelled.
#[tauri::command]
async fn pick_folder(window: tauri::WebviewWindow, title: String, start: Option<String>) -> Result<Option<String>, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    let picked = tauri::async_runtime::spawn_blocking(move || pick::folder(owner, &title, start.as_deref().map(std::path::Path::new))).await.map_err(|e| e.to_string())??;
    Ok(picked.map(|p| p.display().to_string()))
}

/// Where the player finds the cabinet's toys' configuration (DOF's) — ours, else VPX's — and DOF's
/// own folder, to bring in.
#[tauri::command]
fn dof_found() -> dof_config::Where {
    dof_config::look()
}

/// A DOF configuration brought in and copied into ours: from `from` (DOF's own folder), else
/// picked in Windows' file picker over the app — the DOF Config Tool's download, or a file of a
/// set. None when the picker's cancelled, else how many files came in.
#[tauri::command]
async fn import_dof(window: tauri::WebviewWindow, from: Option<String>) -> Result<Option<usize>, String> {
    let owner = window.hwnd().map(|h| h.0 as isize).unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<usize>, String> {
        let picked = match from {
            Some(folder) => std::path::PathBuf::from(folder),
            None => {
                let kinds = [("The DOF Config Tool's download, or a file of a set", "*.zip;*.ini;*.xml")];
                let Some(picked) = pick::file(owner, "Bring in a DOF configuration", &kinds)? else {
                    return Ok(None);
                };
                picked
            }
        };
        dof_config::import(&picked).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The app's window full screen, or not (F11, the top bar's button).
#[tauri::command]
fn set_fullscreen(window: tauri::WebviewWindow, on: bool) -> Result<(), String> {
    window.set_fullscreen(on).map_err(|e| e.to_string())
}

/// Whether the app's window is full screen, as Windows has it.
#[tauri::command]
fn is_fullscreen(window: tauri::WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

/// The app closed: its window's close button hides with the rest of its frame in full screen.
#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

/// Whether the achievements editor holds changes not saved: the window's close asks first.
static UNSAVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
fn set_unsaved(on: bool) {
    UNSAVED.store(on, std::sync::atomic::Ordering::Release);
}

/// The library's folders kept in the settings — VPX's found, or a new library's made under `root`
/// — and the library read from them.
#[tauri::command]
async fn setup_keep(app: AppHandle, folders: Option<setup::Folders>, root: Option<String>, per_game: Option<bool>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // a VPX setup as it's laid out; a new library as the user chose
        let (folders, per_game) = match (folders, root) {
            (Some(f), _) => {
                let per = setup::looks_per_game(std::path::Path::new(f.tables.trim()));
                (f, per)
            }
            (None, Some(root)) => (setup::fresh(&root)?, per_game.unwrap_or(false)),
            (None, None) => return Err("No library was chosen.".to_string()),
        };
        setup::keep(&app, &folders, per_game)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A problem report made of what the user asked for (logs, settings, this PC, the table): one zip
/// in `Logs\Reports`, its path; nothing sent.
#[tauri::command]
async fn make_report(app: AppHandle, asked: report::Asked) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || report::make(&app, &asked).map(|p| p.display().to_string())).await.map_err(|e| e.to_string())?
}

/// A report shown in Explorer.
#[tauri::command]
fn show_report(path: String) -> Result<(), String> {
    report::show(std::path::Path::new(&path))
}

/// The wheel the screens drew for a table, from its pictures (a PNG).
#[tauri::command]
fn save_wheel(app: AppHandle, id: String, png: Vec<u8>) -> Result<(), String> {
    table_path(&app, &id)?;
    own_pictures::save_wheel(&id, &png)?;
    library::forget_popper_copies(&app, &id, &["wheel".to_string()]);
    let _ = app.emit("library-changed", ());
    // and put where PinUP Popper shows it, when that's on and it's the wheel used
    popper_media::after(&app, &id, Some("wheel"), false);
    Ok(())
}

/// A table's script for the script editor: the one that plays, its option values as its own script
/// has them, its ROM.
#[tauri::command]
async fn script_open(app: AppHandle, id: String) -> Result<script_editor::Doc, String> {
    let (path, rom) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        (t.path.clone(), t.read.game_name.clone())
    };
    tauri::async_runtime::spawn_blocking(move || script_editor::open(&path, rom)).await.map_err(|e| e.to_string())?
}

/// The option lines of a script as the editor has it.
#[tauri::command]
fn script_options(text: String) -> Vec<::library::script_edit::ScriptOption> {
    script_editor::options(&text)
}

/// The script with option lines changed.
#[tauri::command]
fn script_with_options(text: String, changes: Vec<(String, String)>) -> Result<String, String> {
    script_editor::with_options(&text, &changes)
}

/// The script saved beside the table, which then plays in its own's place; the library read again.
/// A table's backglasses to choose from: the one that plays, then those kept beside it, each with
/// its file's name and size and the address of a small picture of it (`pictures`' `b/<id>/<n>`).
#[tauri::command]
fn backglass_choices(shared: State<'_, Shared>, id: String) -> Result<serde_json::Value, String> {
    let st = shared.0.lock().unwrap();
    let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
    let one = |n: usize, p: &std::path::Path| {
        let bytes = std::fs::metadata(p).map_or(0, |m| m.len());
        serde_json::json!({
            "n": n,
            "name": p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            "bytes": bytes,
            // its size in the address too: the picture at a place changes when another is chosen
            "picture": format!("{}b/{}/{n}?{bytes}", pictures::base(), t.id),
        })
    };
    let playing = t.holds.backglass.iter().map(|p| one(0, p));
    let kept = t.holds.backglasses.iter().enumerate().map(|(i, p)| one(i + 1, p));
    Ok(serde_json::Value::Array(playing.chain(kept).collect()))
}

/// The backglass that plays for a table: one of those kept beside it (`version`, its place among
/// them), or none (`::library::backglass`); the library read again after.
#[tauri::command]
async fn backglass_play(app: AppHandle, id: String, version: Option<usize>) -> Result<(), String> {
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let shared = app.state::<Shared>();
            let st = shared.0.lock().unwrap();
            let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
            match version {
                Some(n) => ::library::backglass::play(&t.path, t.holds.backglasses.get(n).ok_or("That backglass isn't there any more.")?),
                None => ::library::backglass::play_none(&t.path).map(|_| ()),
            }
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    library::start(app);
    Ok(())
}

#[tauri::command]
async fn script_save(app: AppHandle, id: String, text: String, crlf: bool) -> Result<(), String> {
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            library::save_script(&mut st, &id, &text, crlf)
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    library::start(app);
    Ok(())
}

/// The script as the editor has it played with no window for a few seconds: its errors.
#[tauri::command]
async fn script_check(app: AppHandle, id: String, text: String, crlf: bool) -> Result<script_editor::Checked, String> {
    let path = table_path(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || script_editor::check(&path, &text, crlf)).await.map_err(|e| e.to_string())?
}

/// The table inspector: a table's parts, each with its outline seen from above, what achievements
/// can count of it and its settings.
#[tauri::command]
async fn inspect_table(app: AppHandle, id: String) -> Result<inspector::Inspected, String> {
    let path = table_path(&app, &id)?;
    tauri::async_runtime::spawn_blocking(move || inspector::open(&path)).await.map_err(|e| e.to_string())?
}

/// The script beside the table opened in Notepad, made from the one that plays where there's none.
#[tauri::command]
fn script_notepad(app: AppHandle, id: String) -> Result<(), String> {
    script_editor::notepad(&table_path(&app, &id)?)
}

/// The table's own script put back, the one beside it to the Recycle Bin; the library read again.
#[tauri::command]
async fn script_put_back(app: AppHandle, id: String) -> Result<(), String> {
    {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            library::put_back_script(&mut st, &id)
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    library::start(app);
    Ok(())
}

/// A table updated since its script was edited here: its changes carried over, or its new script
/// played as it is. What was done, in words; the library read again.
#[tauri::command]
async fn script_update(app: AppHandle, id: String, carry: bool) -> Result<String, String> {
    let said = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            library::resolve_script_update(&mut st, &id, carry)
        })
        .await
        .map_err(|e| e.to_string())??
    };
    library::start(app);
    Ok(said)
}

/// Whether an AltSound pack can be made from a table's ROM here, and if not, why.
#[tauri::command]
async fn altsound_offer(app: AppHandle, id: String) -> Result<altsound_make::Offer, String> {
    let (rom, zip) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        (t.holds.rom_set.clone(), t.holds.rom.clone())
    };
    tauri::async_runtime::spawn_blocking(move || altsound_make::offer(rom.as_deref(), zip.as_deref())).await.map_err(|e| e.to_string())
}

/// An AltSound pack made from a table's ROM in the background, told in the tasks bar, put in
/// VPinMAME's `altsound` folder; the library read again after, for its row.
#[tauri::command]
async fn make_altsound(app: AppHandle, id: String) -> Result<(), String> {
    let (rom, zip, altsound) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == id).ok_or("That table isn't in the library any more.")?;
        let layout = st.layout.as_ref().ok_or("The library's folders aren't known.")?;
        (t.holds.rom_set.clone().ok_or("Its script names no ROM.")?, t.holds.rom.clone().ok_or("Its ROM isn't there yet.")?, layout.vpinmame.join("altsound"))
    };
    let roms = zip.parent().map(std::path::Path::to_path_buf).ok_or("Its ROM's folder isn't known.")?;
    let name = table_name(&app, &id);
    let task = tasks::waiting(&app, format!("Waiting to make {name}'s AltSound pack"));
    tauri::async_runtime::spawn_blocking(move || {
        let _turn = altsound_make::turn();
        task.start(format!("Making {name}'s AltSound pack"), Vec::new());
        altsound_make::make(&rom, &roms, &altsound, &|p| match p {
            altsound_make::Progress::Starting => task.step(0, Some(format!("{name}'s AltSound pack: starting its machine"))),
            altsound_make::Progress::Recording(done, of) => task.count(done, of, format!("{name}'s AltSound pack: sound {done} of {of}")),
            altsound_make::Progress::Checking => task.step(0, Some(format!("{name}'s AltSound pack: checking its sounds"))),
            altsound_make::Progress::Writing => task.step(0, Some(format!("{name}'s AltSound pack: writing it"))),
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    library::start(app);
    Ok(())
}

/// The page of a pack's sounds that rom2altsound writes beside them, in the user's browser.
#[tauri::command]
fn altsound_listen(app: AppHandle, id: String) -> Result<(), String> {
    let pack = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        st.tables.iter().find(|t| t.id == id).and_then(|t| t.holds.altsound.clone()).ok_or("This table has no AltSound pack.")?
    };
    shell::open_page(&pack.join("index.html"))
}

/// A table's name for the background tasks: its file's.
fn table_name(app: &AppHandle, id: &str) -> String {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    st.tables.iter().find(|t| t.id == id).map(|t| t.stem.clone()).unwrap_or_default()
}

fn table_path(app: &AppHandle, id: &str) -> Result<std::path::PathBuf, String> {
    let shared = app.state::<Shared>();
    let st = shared.0.lock().unwrap();
    st.tables.iter().find(|t| t.id == id).map(|t| t.path.clone()).ok_or_else(|| "That table isn't in the library any more.".to_string())
}

/// The Controls page: the player's input test while it shows.
#[tauri::command]
async fn controls_start(app: AppHandle) -> Result<(), String> {
    controls::start(&app)
}

#[tauri::command]
fn controls_stop(app: AppHandle) {
    controls::stop(&app);
}

#[tauri::command]
async fn save_controls(mappings: std::collections::BTreeMap<String, String>, defaults: std::collections::BTreeMap<String, String>) -> Result<(), String> {
    controls::save(&mappings, &defaults)
}

/// The Sound page: the sound outputs, and what plays where.
#[tauri::command]
async fn sound_view() -> Result<sound::SoundView, String> {
    sound::load()
}

#[tauri::command]
async fn save_sound(settings: sound::SoundSettings) -> Result<(), String> {
    sound::save(&settings)
}

#[tauri::command]
async fn save_screens(app: AppHandle, settings: screens::ScreenSettings, table: Option<String>) -> Result<(), String> {
    match table {
        Some(id) => screens::save_for_table(&table_path(&app, &id)?, &settings),
        None => screens::save(&settings),
    }
}

/// The settings file shown in Explorer.
#[tauri::command]
fn show_settings_file() -> Result<(), String> {
    shell::show(&vpi_settings::settings_path().ok_or("no APPDATA folder")?)
}

/// Each screen's number and name shown on it. Windows are made from an async command, as Tauri
/// asks on Windows.
#[tauri::command]
async fn identify_screens(app: AppHandle, jobs: std::collections::BTreeMap<String, String>) -> Result<(), String> {
    screens::identify(&app, &jobs)
}

#[tauri::command]
async fn arrange_windows(app: AppHandle, windows: Vec<screens::WindowPlace>, keep: std::collections::BTreeMap<String, f64>) -> Result<(), String> {
    screens::arrange(&app, &windows, &keep)
}

#[tauri::command]
async fn arranged_windows(app: AppHandle, windows: Vec<screens::WindowPlace>) -> Result<Vec<screens::WindowPlace>, String> {
    screens::arranged(&app, &windows)
}

#[tauri::command]
async fn cancel_arranging(app: AppHandle) {
    screens::finish_arranging(&app);
}

/// A drop sorted out for the drop window: its downloads, each file's place. Off the window's
/// thread: reading a table out of an archive takes a moment.
#[tauri::command]
async fn inspect_drop(app: AppHandle, paths: Vec<String>, table_id: Option<String>, join: bool) -> Result<serde_json::Value, String> {
    let paths = paths.into_iter().map(std::path::PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || drop::inspect(&app, paths, table_id, join)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn choose_drop_table(app: AppHandle, index: usize, table_id: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || drop::choose_table(&app, index, table_id)).await.map_err(|e| e.to_string())?
}

/// One download taken out of the drop window before Add; the file itself left where it is.
#[tauri::command]
async fn remove_from_drop(app: AppHandle, index: usize) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || drop::take_out(&app, index)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn add_drop(app: AppHandle, choices: Vec<drop::Choice>, already: ::library::install::Already, test: bool) -> Result<Vec<drop::Result_>, String> {
    let _task = tasks::begin(&app, "Adding files", 0);
    tauri::async_runtime::spawn_blocking(move || drop::add(&app, choices, already, test)).await.map_err(|e| e.to_string())?
}

/// "Test it" on a table's page: the table run with no window, as the compatibility dashboard runs
/// it, its findings kept.
#[tauri::command]
async fn test_table(app: AppHandle, id: String) -> Result<library::Tested, String> {
    let _task = tasks::begin(&app, format!("Testing {}", table_name(&app, &id)), 0);
    tauri::async_runtime::spawn_blocking(move || library::test_table(&app, &id)).await.map_err(|e| e.to_string())?
}

/// The background tasks running now.
#[tauri::command]
fn tasks_now(app: AppHandle) -> Vec<tasks::Task> {
    tasks::now(&app)
}

/// PinUP packs named so the player won't find them renamed to what their tables' scripts hand
/// PinUP (a table page's Fix, Needs attention's Rename all), then the library read again. How many.
#[tauri::command]
async fn rename_packs(app: AppHandle, ids: Vec<String>) -> Result<usize, String> {
    let wanted: Vec<(std::path::PathBuf, String)> = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        st.tables.iter().filter(|t| ids.contains(&t.id)).filter_map(|t| Some((t.holds.pup_misnamed.clone()?, t.read.pup_names.first()?.clone()))).collect()
    };
    let app_for_renames = app.clone();
    let renamed = tauri::async_runtime::spawn_blocking(move || -> Result<usize, String> {
        let mut done = 0;
        let mut problems = Vec::new();
        for (from, name) in &wanted {
            // two tables may want the same pack: the first one's name wins
            if !from.exists() {
                continue;
            }
            match ::library::install::rename_pack(from, name) {
                Ok(now) => {
                    done += 1;
                    let before = from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    pup_packs::kept_rename(&app_for_renames, from, &now, &before);
                }
                Err(e) => problems.push(e),
            }
        }
        if done == 0 && !problems.is_empty() { Err(problems.join("; ")) } else { Ok(done) }
    })
    .await
    .map_err(|e| e.to_string())??;
    library::start(app.clone());
    Ok(renamed)
}

#[tauri::command]
fn forget_drop(app: AppHandle) {
    drop::forget(&app);
}

/// In the test mode, the page the screens are to show; None otherwise.
#[tauri::command]
fn test_mode(shot: State<'_, Option<Shot>>) -> Option<Shot> {
    shot.inner().clone()
}

/// The screens' word that everything on the page has loaded: in the test mode, its picture taken.
#[tauri::command]
fn settled(app: AppHandle, shot: State<'_, Option<Shot>>) -> Result<(), String> {
    match shot.inner() {
        Some(s) => shot::take(&app, s),
        None => Ok(()),
    }
}

fn main() {
    let shot = shot::from_args();
    // the app's own log, beside the player's; not for the test mode's runs, which would push the
    // user's last log aside
    if shot.is_none() {
        report::log_to_file();
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Shared::default())
        .manage(shot.clone())
        .manage(screens::Arranging::default())
        .manage(cabinet::Before::default())
        .manage(cabinet::Showing::default())
        .manage(controls::Testing::default())
        .manage(table_settings::Asked::default())
        .manage(drop::Dropped::default())
        .manage(tasks::Tasks::default())
        .register_asynchronous_uri_scheme_protocol("pic", pictures::handle)
        .invoke_handler(tauri::generate_handler![
            library,
            vps_files,
            vps_game,
            vps_browser,
            set_favourite,
            set_rating,
            ratings_to_fronts,
            set_hidden,
            pup_packs,
            pup_pack,
            pup_use_layout,
            pup_pack_remove,
            pup_pack_show,
            pup_pack_rename,
            pup_pack_undo_rename,
            try_layout,
            table_pup,
            set_table_pup,
            cabinet_open,
            cabinet_close,
            is_fullscreen,
            quit,
            cabinet_show,
            cabinet_showing,
            cabinet_screens,
            starts_with_windows,
            set_start_with_windows,
            shut_down,
            set_details,
            choose_vps,
            delete_media,
            unhide_media,
            choose_media,
            overwrite_vps,
            put_names_back,
            remove_plan,
            remove,
            popper_media_all,
            popper_media_plan,
            save_reach_outside,
            make_report,
            show_report,
            setup_offer,
            this_pc_location,
            update_check,
            report_can_send,
            send_report,
            update_install,
            setup_keep,
            setup_look,
            pick_folder,
            turn_media,
            altsound_offer,
            make_altsound,
            altsound_listen,
            script_open,
            script_options,
            script_with_options,
            script_save,
            backglass_play,
            backglass_choices,
            physics::physics_view,
            roms_page::rom_list,
            high_scores_all,
            achievements::achievements_for,
            achievements::play_mode,
            vpidb::vpidb_status,
            vpidb::vpidb_sync,
            vpidb::vpidb_packs,
            vpidb::vpidb_pack_add,
            achievements::achievements_all,
            achievements::achievements_add,
            achievements::achievements_remove,
            achievements::achievements_drop,
            achievements_editor::ach_edit_open,
            achievements_editor::ach_edit_picture,
            achievements_editor::ach_edit_save,
            achievements_editor::ach_edit_try,
            achievements_editor::ach_edit_tried,
            achievements_editor::ach_edit_show,
            achievements_editor::ach_edit_draft_save,
            achievements_editor::ach_edit_draft_drop,
            achievements_editor::ach_edit_sound,
            vpidb::vpidb_link,
            vpidb::vpidb_connect,
            vpidb::vpidb_poll,
            vpidb::vpidb_cancel,
            vpidb::vpidb_disconnect,
            vpidb::vpidb_sharing,
            vpidb::vpidb_share_library,
            roms_page::rom_files,
            roms_page::rom_show,
            roms_page::rom_fix,
            layout_switch::layout_state,
            layout_switch::layout_plan,
            layout_switch::layout_switch,
            layout_switch::layout_put_back,
            colouring::colour_record,
            colouring::colour_by_itself,
            colouring::colour_by_itself_now,
            colouring::colour_recordings,
            colouring::colour_show,
            colouring::colour_use,
            colouring::colour_recordings_all,
            colouring::colour_for_colorizingdmd,
            colouring::colour_programs,
            colouring::colour_program_install,
            colouring::colour_program_open,
            physics::physics_save,
            physics::physics_back,
            physics::physics_preset_save,
            physics::physics_preset_import,
            script_check,
            script_notepad,
            inspect_table,
            script_put_back,
            script_update,
            dof_found,
            import_dof,
            set_fullscreen,
            screens_now,
            popper_media_put,
            folders,
            show_folder,
            ini_values,
            save_ini_values,
            all_settings,
            save_any_settings,
            set_vps,
            vps_names,
            save_playlists,
            screenshots,
            screen_presets,
            save_screen_preset,
            screen_preset,
            change_screen_preset,
            rom_choices,
            rom_effects,
            change_rom,
            tasks_now,
            rename_packs,
            save_wheel,
            prefs,
            set_prefs,
            front_ends,
            play,
            open_link,
            show_file,
            show_player_log,
            screens_view,
            save_screens,
            sound_view,
            controls_start,
            table_settings,
            save_table_settings,
            controls_stop,
            save_controls,
            save_sound,
            show_settings_file,
            identify_screens,
            arrange_windows,
            arranged_windows,
            cancel_arranging,
            inspect_drop,
            choose_drop_table,
            remove_from_drop,
            add_drop,
            forget_drop,
            test_table,
            high_scores,
            rom_check,
            make_pictures,
            own_picture,
            set_unsaved,
            test_mode,
            settled
        ])
        // the helper windows go with the app's own
        .on_window_event(|window, event| {
            // closed with an achievement pack's changes not saved: asked first, on the page
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
                && UNSAVED.load(std::sync::atomic::Ordering::Acquire)
            {
                api.prevent_close();
                let _ = window.emit("close-asked", ());
            }
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                // the Controls page's input test with it
                controls::stop(window.app_handle());
                window.app_handle().exit(0);
            }
        })
        .setup(move |app| {
            let mut window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("Visual Pinball Infinite")
                .min_inner_size(1024.0, 640.0)
                .background_color(Color(13, 16, 22, 255));
            window = match &shot {
                // off every screen, out of the taskbar and the user's way
                Some(s) => window.inner_size(s.size.0, s.size.1).position(-30000.0, -30000.0).focused(false).skip_taskbar(true),
                // full screen when it was left so
                None => window.inner_size(1600.0, 1000.0).center().fullscreen(library::starts_fullscreen()),
            };
            window.build()?;
            library::start(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the app's window");
}
