//! The app's own pause menu over VPX, where the app's settings ask for it (`prefs.pauseMenu`; VPX's
//! own menu where it's `vpx`): for each game a port of the app's on this PC, handed to our add-on in
//! VPX (`VPI_MENU_PORT`), which keeps VPX's own menu key for ours — F12, or the button a controller
//! has for VPX's menu — and tells the app when it's pressed, with where the achievements stand. The
//! menu opens as a window of the app's own over VPX's playfield, in front and with the focus, the
//! table showing through it darkened: VPX pauses by itself while its window hasn't the focus, and
//! plays on once it has it back. It shows what the app can tell of the game as it plays — the
//! volume and the level of each part of its sound, the table's rules (its own, kept in its file, and
//! the instruction cards and flyers of the front ends' media), the guide VPS links for its game
//! (Pinball Primer's own page, as the site draws it, in a view of its own over the menu), its
//! achievements, the keys and buttons it's played with — and what the menu asks of VPX — its own menu,
//! the game ended, the volume, a part's level — goes to the add-on. The levels are kept for the table,
//! and the volume in VPX's settings, once VPX has quit, as VPX writes its settings as it does.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::library::Shared;

/// The menu window's label.
const WINDOW: &str = "pause-menu";

/// The guide view's window label: a guide page as Pinball Primer draws it, over the menu's Guide page.
pub const GUIDE: &str = "pause-guide";

/// The guide view: whether the menu's Guide page wants it shown, the page asked for, whether that
/// page has loaded, and where it goes (desktop pixels: `x`, `y`, width, height).
struct GuideView {
    wanted: bool,
    page: String,
    loaded: bool,
    at: (i32, i32, i32, i32),
}

static GUIDE_VIEW: Mutex<GuideView> = Mutex::new(GuideView { wanted: false, page: String::new(), loaded: false, at: (0, 0, 1, 1) });

/// The game's link with our add-on, while a game plays.
#[derive(Default)]
pub struct PauseLink(Mutex<Option<Link>>);

/// What a game plays with, for what the menu says of it: the settings file VPX reads (a copy made
/// for the game, else VPX's own), the table's own settings file over it, and the table file.
pub struct Played {
    pub settings: Option<PathBuf>,
    pub table_settings: PathBuf,
    pub table: PathBuf,
}

struct Link {
    table_id: String,
    /// The add-on's end, once it's connected.
    stream: Option<TcpStream>,
    /// Where the achievements stood when the menu was last asked for.
    summary: Option<String>,
    lines: serde_json::Value,
    ended: Arc<AtomicBool>,
    played: Played,
    /// VPX's volume keys pressed for the menu this game, a hundredth of the volume each.
    volume_steps: i64,
    /// What the table's file says for the menu.
    facts: Facts,
    /// The add-ons' sources of sound when the menu was last asked for.
    sources: serde_json::Value,
    /// Each part of its sound's level, 1 as it came, and as they were when the game began.
    levels: BTreeMap<String, f64>,
    levels_then: BTreeMap<String, f64>,
}

/// What a table's file says for the menu: its own rules, and what its script reads beyond the
/// usual controls.
#[derive(Clone, Default)]
struct Facts {
    rules: Option<String>,
    /// Its own pictures that show its rules, by name, where its rules name no picture.
    cards: Vec<String>,
    controls: Vec<(&'static str, &'static [&'static str])>,
    /// Its own levels for its sounds and its music, 0 to 1.
    sound_volume: f32,
    music_volume: f32,
    /// Whether its script plays music: then whether the song playing can be set too
    /// (`library::table::plays_music`).
    music: Option<bool>,
}

impl Facts {
    fn of(table: &Path) -> Facts {
        let own = ::library::table::own_facts(table).unwrap_or_else(|e| {
            eprintln!("the table's file: {e}");
            ::library::table::OwnFacts { rules: None, script: String::new(), sound_volume: 1.0, music_volume: 1.0 }
        });
        // a script beside the table plays in the table's own's place, as VPX plays it
        let beside = std::fs::read(table.with_extension("vbs")).map(|b| String::from_utf8_lossy(&b).into_owned()).ok();
        let script = beside.as_deref().unwrap_or(&own.script);
        let cards = if own.rules.as_deref().is_some_and(|r| r.contains("![")) { Vec::new() } else { ::library::table::rule_pictures(table).unwrap_or_default() };
        Facts { controls: ::library::table::own_controls(script), music: ::library::table::plays_music(script), rules: own.rules, cards, sound_volume: own.sound_volume, music_volume: own.music_volume }
    }
}

/// Whether the app's own menu is asked for (`prefs.pauseMenu`, VPX's own where it's `vpx`).
fn ours(app: &AppHandle) -> bool {
    app.state::<Shared>().0.lock().unwrap().user.prefs.get("pauseMenu").and_then(serde_json::Value::as_str) != Some("vpx")
}

/// The levels kept for a table, each a share of that part of the sound as it came: never above it.
fn as_kept(levels: Option<&BTreeMap<String, f64>>) -> BTreeMap<String, f64> {
    levels.into_iter().flatten().map(|(id, level)| (id.clone(), level.min(1.0))).collect()
}

/// One of a table's own levels, its share of the level the table was made with, as a script gives
/// it (a percentage).
fn percent_of(own: f32, level: f64) -> i64 {
    let own = if own > 0.0 { f64::from(own) } else { 1.0 };
    (own * level * 100.0).round() as i64
}

/// The levels kept for a table as our add-on starts from them.
fn levels_for_add_on(levels: &BTreeMap<String, f64>, facts: &Facts) -> serde_json::Value {
    let add_ons: BTreeMap<&String, &f64> = levels.iter().filter(|(id, _)| *id != "table" && *id != "music").collect();
    let mut out = serde_json::json!({ "addOns": add_ons });
    if let Some(level) = levels.get("table") {
        out["table"] = percent_of(facts.sound_volume, *level).into();
    }
    if let (Some(level), Some(now)) = (levels.get("music"), facts.music) {
        out["music"] = serde_json::json!({ "percent": percent_of(facts.music_volume, *level), "now": now });
    }
    out
}

/// The menu for the game of table `table_id` about to start: a port of the app's for the add-on in
/// VPX to connect to, and the levels kept for the table as the add-on takes them, where the app's own
/// menu is asked for; none otherwise. A thread waits for the add-on (none comes from a VPX without
/// the newer add-on interface, which keeps its own menu) and hears when the menu is asked for.
pub fn start(app: &AppHandle, table_id: &str, played: Played) -> Option<(u16, String)> {
    if !ours(app) {
        return None;
    }
    let facts = Facts::of(&played.table);
    let levels = as_kept(app.state::<Shared>().0.lock().unwrap().user.levels.get(&crate::library::key(&played.table)));
    let kept = levels_for_add_on(&levels, &facts).to_string();
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    listener.set_nonblocking(true).ok()?;
    let ended = Arc::new(AtomicBool::new(false));
    *app.state::<PauseLink>().0.lock().unwrap() =
        Some(Link { table_id: table_id.to_string(), stream: None, summary: None, lines: serde_json::Value::Array(Vec::new()), ended: ended.clone(), played, volume_steps: 0, facts, sources: serde_json::Value::Array(Vec::new()), levels_then: levels.clone(), levels });
    let app = app.clone();
    std::thread::Builder::new()
        .name("pause-menu".into())
        .spawn(move || {
            // the add-on connects as VPX starts the game; given up after two minutes, or the game over
            let until = Instant::now() + Duration::from_secs(120);
            let stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(_) if ended.load(Ordering::Relaxed) || Instant::now() > until => return,
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            };
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_nodelay(true);
            if let Some(link) = app.state::<PauseLink>().0.lock().unwrap().as_mut() {
                link.stream = stream.try_clone().ok();
            }
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                if v.get("menu").and_then(serde_json::Value::as_str) != Some("open") {
                    continue;
                }
                if let Some(link) = app.state::<PauseLink>().0.lock().unwrap().as_mut() {
                    link.summary = v.get("summary").and_then(serde_json::Value::as_str).map(str::to_string);
                    link.lines = v.get("lines").cloned().unwrap_or_default();
                    link.sources = v.get("sources").cloned().unwrap_or_default();
                }
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || open_window(&handle));
            }
        })
        .ok()?;
    Some((port, kept))
}

/// The volume as the game left it kept for the games after: in the table's own settings where they
/// set it, else in VPX's.
fn keep_volume(table: &Path, steps: i64) {
    let table_file = table.with_extension("ini");
    let own_text = std::fs::read(&table_file).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let own = vpi_settings::parse_ini(&own_text);
    let main_text = vpi_settings::vpx_settings_text();
    let main = vpi_settings::parse_ini(&main_text);
    let (mut own_changes, mut main_changes) = (Vec::new(), Vec::new());
    for name in ["MusicVolume", "SoundVolume"] {
        let set_own = vpi_settings::int(&own, "Player", name, 0, 100);
        let was = set_own.or_else(|| vpi_settings::int(&main, "Player", name, 0, 100)).unwrap_or(100);
        let change = vpi_settings::Change::set("Player", name, (i64::from(was) + steps).clamp(0, 100));
        if set_own.is_some() { own_changes.push(change) } else { main_changes.push(change) }
    }
    if !own_changes.is_empty() && let Err(e) = std::fs::write(&table_file, vpi_settings::edit(&own_text, &own_changes)) {
        eprintln!("the volume wasn't kept: {e}");
    }
    if !main_changes.is_empty()
        && let Some(file) = vpi_settings::vpx_prefs_ready().ok().and_then(|_| vpi_settings::vpx_settings_path())
        && let Err(e) = std::fs::write(&file, vpi_settings::edit(&main_text, &main_changes))
    {
        eprintln!("the volume wasn't kept: {e}");
    }
}

/// The game over: what the menu changed kept (VPX has quit, and writes its settings as it does), the
/// link let go, the menu closed if it's open.
pub fn end(app: &AppHandle) {
    if let Some(link) = app.state::<PauseLink>().0.lock().unwrap().take() {
        link.ended.store(true, Ordering::Relaxed);
        if link.volume_steps != 0 {
            keep_volume(&link.played.table, link.volume_steps);
        }
        if link.levels != link.levels_then {
            let shared = app.state::<Shared>();
            let mut st = shared.0.lock().unwrap();
            let key = crate::library::key(&link.played.table);
            let levels: BTreeMap<String, f64> = link.levels.into_iter().filter(|(_, v)| (v - 1.0).abs() > 0.001).collect();
            if levels.is_empty() {
                st.user.levels.remove(&key);
            } else {
                st.user.levels.insert(key, levels);
            }
            if let Err(e) = st.user.save() {
                eprintln!("the sound's levels weren't kept: {e}");
            }
        }
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        close_guide(&handle);
        if let Some(w) = handle.get_webview_window(WINDOW) {
            let _ = w.close();
        }
    });
}

/// The menu's window over VPX's playfield, in front and with the focus, the table seen through it;
/// shown again, and told so, where it's open already.
fn open_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.show();
        let _ = w.set_focus();
        let _ = app.emit_to(WINDOW, "pause-menu-open", ());
        return;
    }
    let builder = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::App("index.html#pause".into()))
        .title("Paused")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .shadow(false)
        // the table seen through what the menu doesn't cover, darkened by the page
        .transparent(true)
        .background_color(tauri::webview::Color(0, 0, 0, 0));
    let at = vpx_window();
    let builder = match at {
        Some((_, x, y, w, h)) => crate::screens::made_at(builder, app, x, y, w, h),
        None => builder.maximized(true),
    };
    if let Ok(w) = builder.build() {
        // over VPX's window exactly, and the page inside made to take the window's size: on a screen
        // at another scale it starts smaller, in the window's corner
        match at {
            Some((_, x, y, width, height)) => crate::screens::place_and_show(&w, x, y, width, height),
            None => {
                let later = w.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(300));
                    crate::screens::refit(&later);
                });
            }
        }
        crate::screens::refit(&w);
        let _ = w.set_focus();
        // back in front once it has the focus again (a page opened in the browser set it back), and
        // given the focus back should the guide's view ever take it
        let handle = app.clone();
        w.on_window_event(move |event| match event {
            tauri::WindowEvent::Focused(true) => on_top_again(&handle),
            tauri::WindowEvent::Focused(false) => focus_back_from_guide(&handle),
            _ => {}
        });
    }
}

/// The menu closed and VPX given the focus back, to play on.
fn close_and_resume(app: &AppHandle) {
    close_guide(app);
    if let Some(w) = app.get_webview_window(WINDOW) {
        let _ = w.close();
    }
    focus_vpx();
}

/// What the menu shows: the table playing (its view, as its page has it), where its achievements
/// stood when the menu was asked for, the volume as the game plays it, the table's own rules and
/// the pictures of its own that show them (`cards`), the guide pages VPS links for its game
/// (`guide`), and the keys and buttons it's played with (`controls`), with the keys VPX's menu moves
/// by (`nav`).
/// `test`: in the app's test mode, a table named, shown as if paused (no game plays), its
/// achievements a made-up example.
#[tauri::command]
pub fn pause_menu_state(app: AppHandle, test: Option<String>) -> serde_json::Value {
    let testing = test.is_some() && crate::shot::from_args().is_some();
    let link = app.state::<PauseLink>();
    let (table_id, summary, lines, settings, table_settings, table, steps, facts, sources, levels) = {
        let guard = link.0.lock().unwrap();
        match guard.as_ref() {
            Some(l) => (l.table_id.clone(), l.summary.clone(), l.lines.clone(), l.played.settings.clone(), Some(l.played.table_settings.clone()), Some(l.played.table.clone()), l.volume_steps, Some(l.facts.clone()), l.sources.clone(), l.levels.clone()),
            None if testing => (test.clone().unwrap_or_default(), None, serde_json::Value::Array(Vec::new()), None, None, None, 0, None, example_sources(), BTreeMap::new()),
            None => return serde_json::Value::Null,
        }
    };
    let (view, path, guide) = {
        let shared = app.state::<Shared>();
        let st = shared.0.lock().unwrap();
        let t = st.tables.iter().find(|t| t.id == table_id).or_else(|| {
            // the test mode names a table by part of its file's name
            let want = table_id.to_lowercase();
            testing.then(|| st.tables.iter().find(|t| t.stem.to_lowercase().contains(&want))).flatten()
        });
        match t {
            Some(t) => (serde_json::to_value(crate::library::view(&st, t)).unwrap_or_default(), Some(t.path.clone()), crate::library::guide_of(&st, t)),
            None => (serde_json::Value::Null, None, None),
        }
    };
    let table = table.or(path);
    // the test mode's table, read for its picture of the menu (a game's is read as it starts)
    let facts = facts.unwrap_or_else(|| table.as_deref().map(Facts::of).unwrap_or_default());
    let main = settings.and_then(|p| std::fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_else(vpi_settings::vpx_settings_text);
    let main = vpi_settings::parse_ini(&main);
    let own = table_settings.or_else(|| table.as_ref().map(|t| t.with_extension("ini"))).and_then(|p| std::fs::read(p).ok()).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
    let (summary, lines) = if testing && summary.is_none() { example_achievements() } else { (summary, lines) };
    // where the table's own pictures its rules name are served, by name
    let pictures = view.get("id").and_then(serde_json::Value::as_str).map(|id| format!("{}u/{id}/", crate::pictures::base()));
    serde_json::json!({
        "table": view,
        "pictures": pictures,
        "summary": summary,
        "lines": lines,
        "volume": volume(&own, &main, steps),
        "levels": level_rows(&sources, &levels, facts.music.is_some()),
        "rules": facts.rules,
        "cards": facts.cards,
        "guide": guide,
        "controls": controls(&main, &facts.controls),
        "nav": nav(&main),
        "testing": testing,
    })
}

/// The guide pages VPS links for the game, each with the title it gives itself, read from Pinball
/// Primer's site into memory alone: those naming the game's year where VPS lists several.
#[tauri::command]
pub async fn pause_menu_guide(pages: Vec<String>, year: Option<i32>) -> Result<Vec<::library::guide::Page>, String> {
    tauri::async_runtime::spawn_blocking(move || ::library::guide::pages(&pages, year)).await.map_err(|e| e.to_string())?
}

/// A guide page shown as Pinball Primer draws it, in a window of the app's own over the menu's Guide
/// page, `x`, `y`, `width` by `height` of the page asking (the menu's own pixels). The window is owned
/// by the menu's, so always just above it and gone with it, and never takes the focus, so VPX's keys
/// and a controller stay with the menu, which scrolls it. Private, so nothing of the site is kept on
/// the PC; it goes nowhere but the site (a link off it opens in the user's browser) and downloads
/// nothing. Shown once its page has loaded, and the menu told (`pause-guide-loaded`). Not run on the
/// window's own thread, as a window made from there waits on that thread on Windows.
#[tauri::command]
pub async fn pause_menu_guide_show(app: AppHandle, window: tauri::WebviewWindow, page: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let address = ::library::guide::address_of(&page).ok_or("That isn't a page of the guides' site.")?;
    let url: tauri::Url = address.parse().map_err(|e| format!("{address}: {e}"))?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let at = (origin.x + (x * scale).round() as i32, origin.y + (y * scale).round() as i32, (width * scale).round().max(1.0) as i32, (height * scale).round().max(1.0) as i32);
    let existing = app.get_webview_window(GUIDE);
    // decided under the lock, done after it: the window's calls run its events, which take it too
    let (load, place) = {
        let mut view = GUIDE_VIEW.lock().unwrap();
        view.wanted = true;
        view.at = at;
        let load = existing.is_none() || view.page != page;
        if load {
            view.page = page;
            view.loaded = false;
        }
        (load, !load && view.loaded)
    };
    if let Some(view) = existing {
        if load {
            let _ = view.hide();
            view.navigate(url).map_err(|e| e.to_string())?;
        } else if place {
            crate::screens::place_and_show(&view, at.0, at.1, at.2, at.3);
        }
        return Ok(());
    }
    let (on_load, on_new) = (app.clone(), app.clone());
    let builder = WebviewWindowBuilder::new(&app, GUIDE, WebviewUrl::External(url))
        .title("Guide")
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .shadow(false)
        .visible(false)
        .incognito(true)
        .owner(&window)
        .map_err(|e| e.to_string())?
        .on_navigation(|to| ::library::guide::on_site(to.as_str()))
        .on_new_window(move |to, _| {
            if matches!(to.scheme(), "https" | "http") {
                let _ = browse(&on_new, to.as_str());
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .on_download(|_, _| false)
        .on_page_load(move |view, payload| {
            if payload.event() != tauri::webview::PageLoadEvent::Finished {
                return;
            }
            let (wanted, at, page) = {
                let mut state = GUIDE_VIEW.lock().unwrap();
                state.loaded = true;
                (state.wanted, state.at, state.page.clone())
            };
            if wanted {
                crate::screens::place_and_show(&view, at.0, at.1, at.2, at.3);
                focus_back_from_guide(&on_load);
            }
            let _ = on_load.emit("pause-guide-loaded", page);
        });
    crate::screens::made_at(builder, &app, at.0, at.1, at.2, at.3).build().map_err(|e| e.to_string())?;
    Ok(())
}

/// The guide's page scrolled by `by` of the menu's pixels, smoothly where `smooth` (a key's step;
/// a controller's stick moves it a little each moment).
#[tauri::command]
pub fn pause_menu_guide_scroll(app: AppHandle, by: f64, smooth: bool) -> Result<(), String> {
    let view = app.get_webview_window(GUIDE).ok_or("The guide isn't showing.")?;
    if !by.is_finite() {
        return Ok(());
    }
    view.eval(format!("window.scrollBy({{ top: {by:.1}, behavior: '{}' }})", if smooth { "smooth" } else { "instant" })).map_err(|e| e.to_string())
}

/// The guide's view hidden, the menu's Guide page left; the page stays loaded for going back to it.
#[tauri::command]
pub fn pause_menu_guide_hide(app: AppHandle) {
    GUIDE_VIEW.lock().unwrap().wanted = false;
    if let Some(view) = app.get_webview_window(GUIDE) {
        let _ = view.hide();
    }
}

/// A guide page opened in the user's own browser.
#[tauri::command]
pub fn pause_menu_guide_browse(app: AppHandle, page: String) -> Result<(), String> {
    let address = ::library::guide::address_of(&page).ok_or("That isn't a page of the guides' site.")?;
    browse(&app, &address)
}

/// A web page opened in the user's browser, in front of the game: the menu, and the guide's view it
/// owns, kept on top no more until the menu has the focus again (`on_top_again`).
fn browse(app: &AppHandle, address: &str) -> Result<(), String> {
    for label in [WINDOW, GUIDE] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_always_on_top(false);
        }
    }
    crate::shell::open_link(address)
}

/// The menu, and the guide's view it owns, kept on top of the game again.
fn on_top_again(app: &AppHandle) {
    for label in [WINDOW, GUIDE] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_always_on_top(true);
        }
    }
}

/// The guide's view closed with the menu.
fn close_guide(app: &AppHandle) {
    GUIDE_VIEW.lock().unwrap().wanted = false;
    if let Some(view) = app.get_webview_window(GUIDE) {
        let _ = view.close();
    }
}

/// The menu given the focus back where the guide's view took it (a click in its page): VPX's keys and
/// a controller are the menu's. Whatever else takes the focus — the user's browser, another program —
/// keeps it.
fn focus_back_from_guide(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        // once Windows has moved the focus
        std::thread::sleep(Duration::from_millis(80));
        let (Some(view), Some(menu)) = (app.get_webview_window(GUIDE), app.get_webview_window(WINDOW)) else { return };
        if in_front(&view) {
            crate::cabinet::focus(&menu);
        }
    });
}

/// Whether a window of the app's is the one in front, with the focus.
#[cfg(windows)]
fn in_front(window: &tauri::WebviewWindow) -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> *mut std::ffi::c_void;
    }
    let Ok(hwnd) = window.hwnd() else { return false };
    // SAFETY: asks Windows which window is in front; nothing is passed
    let front = unsafe { GetForegroundWindow() };
    front == hwnd.0
}

#[cfg(not(windows))]
fn in_front(_: &tauri::WebviewWindow) -> bool {
    false
}

/// What the menu asks: `resume` (VPX plays on), `vpx-menu` (VPX's own menu opened), `quit` (the game
/// ended), `volume` (`steps` of VPX's volume, a hundredth each, up or down, the menu staying: the
/// volume it's at then returned), `level` (the part `id` of the sound set to `value`, 1 as it came).
#[tauri::command]
pub fn pause_menu_do(app: AppHandle, what: String, steps: Option<i64>, id: Option<String>, value: Option<f64>) -> Result<Option<i64>, String> {
    let link = app.state::<PauseLink>();
    let mut now = None;
    let line = match what.as_str() {
        "resume" => None,
        "vpx-menu" | "quit" => Some(serde_json::json!({ "do": what })),
        "volume" => {
            let mut guard = link.0.lock().unwrap();
            let l = guard.as_mut().ok_or("VPX isn't playing any more.")?;
            // never past what VPX's own keys reach, so the count stays the volume's
            let main = l.played.settings.as_ref().and_then(|p| std::fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_else(vpi_settings::vpx_settings_text);
            let main = vpi_settings::parse_ini(&main);
            let own = std::fs::read(&l.played.table_settings).map(|b| vpi_settings::parse_ini(&String::from_utf8_lossy(&b))).unwrap_or_default();
            let before = volume(&own, &main, l.volume_steps);
            let after = (before + steps.unwrap_or(0).clamp(-100, 100)).clamp(0, 100);
            l.volume_steps += after - before;
            now = Some(after);
            (after != before).then(|| serde_json::json!({ "do": "volume", "steps": after - before }))
        }
        "level" => {
            let mut guard = link.0.lock().unwrap();
            let l = guard.as_mut().ok_or("VPX isn't playing any more.")?;
            let (id, value) = (id.ok_or("Which part of the sound?")?, value.unwrap_or(1.0).clamp(0.0, 1.0));
            l.levels.insert(id.clone(), value);
            Some(level_line(&id, value, &l.facts))
        }
        _ => return Err(format!("The pause menu can't {what}.")),
    };
    if let Some(line) = line {
        let mut guard = link.0.lock().unwrap();
        let stream = guard.as_mut().and_then(|l| l.stream.as_mut()).ok_or("VPX isn't playing any more.")?;
        stream.write_all(format!("{line}\n").as_bytes()).map_err(|e| format!("VPX couldn't be told: {e}"))?;
    }
    if what != "volume" && what != "level" {
        close_and_resume(&app);
    }
    Ok(now)
}

/// What our add-on is told of a part's level: the table's own sounds and music by the table's own
/// levels for them, an add-on's sound by its ID.
fn level_line(id: &str, value: f64, facts: &Facts) -> serde_json::Value {
    match id {
        "table" => serde_json::json!({ "do": "table-sounds", "percent": percent_of(facts.sound_volume, value) }),
        "music" => serde_json::json!({ "do": "table-music", "percent": percent_of(facts.music_volume, value), "now": facts.music.unwrap_or(false) }),
        _ => serde_json::json!({ "do": "level", "id": id, "value": value }),
    }
}

/// The parts of the sound the menu shows levels for: the table's own sounds, its machine's, its
/// music where its script plays some, then each other add-on with sound now, in that order, named as
/// a player knows them; each from 0 to 1, 1 as it came.
fn level_rows(sources: &serde_json::Value, levels: &BTreeMap<String, f64>, music: bool) -> Vec<serde_json::Value> {
    let sources: Vec<(String, String)> = sources
        .as_array()
        .map(|a| a.iter().filter_map(|s| Some((s.get("id")?.as_str()?.to_string(), s.get("name")?.as_str()?.to_string()))).collect())
        .unwrap_or_default();
    let row = |id: &str, label: &str| serde_json::json!({ "id": id, "label": label, "value": levels.get(id).copied().unwrap_or(1.0) });
    let known = [("PinMAME", "Machine"), ("AltSound", "Machine (AltSound)"), ("PUP", "PinUP pack"), ("WMP", "Media Player"), ("B2S", "Backglass sounds"), ("VPIAchievements", "Achievements")];
    let mut rows = vec![row("table", "Table sounds")];
    for (id, label) in &known[..2] {
        if sources.iter().any(|(s, _)| s == id) {
            rows.push(row(id, label));
        }
    }
    if music {
        rows.push(row("music", "Music"));
    }
    for (id, label) in &known[2..] {
        if sources.iter().any(|(s, _)| s == id) {
            rows.push(row(id, label));
        }
    }
    for (id, name) in sources.iter().filter(|(s, _)| !known.iter().any(|(k, _)| k == s)) {
        rows.push(row(id, name));
    }
    rows
}

/// The add-ons' sources of sound, made up, for the test mode's picture of the menu.
fn example_sources() -> serde_json::Value {
    serde_json::json!([{ "id": "PinMAME", "name": "PinMAME" }, { "id": "PUP", "name": "PUP" }, { "id": "VPIAchievements", "name": "Achievements" }])
}

/// The volume as VPX plays the game: its backglass's (`[Player]` `MusicVolume`, the one VPX's volume
/// keys say), the table's own settings over the main ones, with the steps the menu pressed since.
fn volume(own: &[(String, String, String)], main: &[(String, String, String)], steps: i64) -> i64 {
    let set = vpi_settings::int(own, "Player", "MusicVolume", 0, 100).or_else(|| vpi_settings::int(main, "Player", "MusicVolume", 0, 100)).unwrap_or(100);
    (i64::from(set) + steps).clamp(0, 100)
}

/// The keys VPX's menu moves by, as VPX's settings map them (its defaults where they don't): up,
/// down, left and right, and choosing (the launch button and Start) — the keyboard's alone, by the
/// numbers VPX's settings give keys (SDL's scancodes).
fn nav(main: &[(String, String, String)]) -> serde_json::Value {
    let keys = |action: &str| -> Vec<u32> {
        mapping(main, action)
            .split('|')
            .filter_map(|alt| {
                let parts: Vec<&str> = alt.split('&').map(str::trim).collect();
                // a key held alone
                match parts.as_slice() {
                    [one] => one.strip_prefix("Key;").and_then(|n| n.trim().parse().ok()),
                    _ => None,
                }
            })
            .collect()
    };
    let choose: Vec<u32> = [keys("LaunchBall"), keys("Start")].concat();
    serde_json::json!({
        "up": keys("UIUp"),
        "down": keys("UIDown"),
        "left": keys("UILeft"),
        "right": keys("UIRight"),
        "choose": choose,
    })
}

/// VPX's default keys for the actions the menu names, as VPX 10.8.1 sets them (`InputManager`):
/// what plays where its settings map nothing.
const DEFAULT_KEYS: [(&str, u32); 28] = [
    ("LeftFlipper", 225),
    ("RightFlipper", 229),
    ("LeftStagedFlipper", 225),
    ("RightStagedFlipper", 229),
    ("LeftMagna", 224),
    ("RightMagna", 228),
    ("UIUp", 224),
    ("UIDown", 228),
    ("UILeft", 225),
    ("UIRight", 229),
    ("LaunchBall", 40),
    ("LeftNudge", 29),
    ("RightNudge", 56),
    ("CenterNudge", 44),
    ("Tilt", 23),
    ("Credit1", 34),
    ("Credit2", 33),
    ("Start", 30),
    ("Lockbar", 226),
    ("Pause", 19),
    ("ExitGame", 41),
    ("InGameUI", 69),
    ("VolumeDown", 45),
    ("VolumeUp", 46),
    ("CoinDoor", 77),
    ("Service1", 36),
    ("Service2", 37),
    ("Service3", 38),
];

/// An action's mapping as VPX reads it: its settings' (`[Input]` `Mapping.<action>`), else VPX's
/// default key.
fn mapping(main: &[(String, String, String)], action: &str) -> String {
    let set = main.iter().rev().find(|(s, n, _)| s.eq_ignore_ascii_case("Input") && n.eq_ignore_ascii_case(&format!("Mapping.{action}"))).map(|e| e.2.trim().to_string());
    match set {
        // set, even to nothing: VPX's own
        Some(m) => m,
        None => DEFAULT_KEYS.iter().find(|(a, _)| *a == action).map(|(_, k)| format!("Key;{k}")).unwrap_or_default(),
    }
}

/// The Controls page's lines: the table's own controls, the game's, then the menu's and VPX's own,
/// each with the keys and buttons VPX's settings put it on.
fn controls(main: &[(String, String, String)], own: &[(&'static str, &'static [&'static str])]) -> Vec<serde_json::Value> {
    let named = |actions: &[&str]| -> String {
        let each: Vec<String> = actions
            .iter()
            .map(|a| {
                let names = names_of(main, &mapping(main, a));
                if names.is_empty() { "none".to_string() } else { names.join(" or ") }
            })
            .collect();
        // the actions side by side: left, right; one only once where they're alike
        let mut unique = each.clone();
        unique.dedup();
        unique.join("  ·  ")
    };
    let mut lines = Vec::new();
    let mut group = |section: &str, items: &[(&str, &[&str])]| {
        for (k, (label, actions)) in items.iter().enumerate() {
            lines.push(serde_json::json!({ "section": (k == 0).then_some(section), "label": label, "keys": named(actions) }));
        }
    };
    if !own.is_empty() {
        let items: Vec<(&str, &[&str])> = own.iter().map(|(l, a)| (*l, *a)).collect();
        group("This table", &items);
    }
    group(
        "Playing",
        &[
            ("Flippers", &["LeftFlipper", "RightFlipper"]),
            ("Launch the ball", &["LaunchBall"]),
            ("Start", &["Start"]),
            ("Coin", &["Credit1"]),
            ("Nudge", &["LeftNudge", "CenterNudge", "RightNudge"]),
        ],
    );
    group("While playing", &[("This menu", &["InGameUI"]), ("Pause", &["Pause"])]);
    let one = |action: &str| {
        let names = names_of(main, &mapping(main, action));
        if names.is_empty() { "none".to_string() } else { names.join(" or ") }
    };
    lines.push(serde_json::json!({ "section": null, "label": "Volume", "keys": format!("{} down  ·  {} up", one("VolumeDown"), one("VolumeUp")) }));
    lines.push(serde_json::json!({ "section": null, "label": "Quit", "keys": one("ExitGame") }));
    if let Some(k) = crate::play::screenshot_key().and_then(|code| crate::play::SCREENSHOT_KEYS.iter().find(|(_, c)| *c == code).map(|(n, _)| *n)) {
        lines.push(serde_json::json!({ "section": null, "label": "Screenshot", "keys": k }));
    }
    lines
}

/// A mapping's alternatives in words, each the buttons held together joined by "+": keys as they're
/// called, the mouse's buttons, a controller's by its own labels where it's an Xbox controller, else
/// by its name and number.
fn names_of(main: &[(String, String, String)], mapping: &str) -> Vec<String> {
    let device = |id: &str| -> String {
        main.iter().rev().find(|(s, n, _)| s.eq_ignore_ascii_case("Input") && n.eq_ignore_ascii_case(&format!("Device.{id}.Name"))).map(|e| e.2.trim().to_string()).unwrap_or_default()
    };
    let mut out: Vec<String> = Vec::new();
    for alt in mapping.split('|') {
        let names: Vec<String> = alt.split('&').map(str::trim).filter(|b| !b.is_empty()).filter_map(|b| bind_name(b, &device)).collect();
        if names.is_empty() {
            continue;
        }
        let text = names.join(" + ");
        if !out.contains(&text) {
            out.push(text);
        }
    }
    out
}

/// One button of a mapping, `<device>;<element>[;<way>;<threshold>]`, in words.
fn bind_name(bind: &str, device: &dyn Fn(&str) -> String) -> Option<String> {
    let mut parts = bind.split(';');
    let (id, element) = (parts.next()?.trim(), parts.next()?.trim().parse::<u32>().ok()?);
    let (way, threshold) = (parts.next().map(str::trim), parts.next().and_then(|t| t.trim().parse::<f32>().ok()));
    match id {
        "Key" => Some(key_name(element)),
        "Mouse" => Some(match element {
            1 => "Left click".into(),
            2 => "Middle click".into(),
            3 => "Right click".into(),
            n => format!("Mouse button {n}"),
        }),
        _ => {
            let name = device(id);
            Some(pad_name(&name, element, way, threshold))
        }
    }
}

/// A key as VPX's settings number it (SDL's scancodes, USB's usage codes), in words.
fn key_name(code: u32) -> String {
    let plain = match code {
        4..=29 => return char::from(b'A' + (code - 4) as u8).to_string(),
        30..=38 => return char::from(b'1' + (code - 30) as u8).to_string(),
        58..=69 => return format!("F{}", code - 57),
        89..=97 => return format!("Keypad {}", code - 88),
        104..=115 => return format!("F{}", code - 91),
        39 => "0",
        40 => "Enter",
        41 => "Esc",
        42 => "Backspace",
        43 => "Tab",
        44 => "Space",
        45 => "-",
        46 => "=",
        47 => "[",
        48 => "]",
        49 => "\\",
        51 => ";",
        52 => "'",
        53 => "`",
        54 => ",",
        55 => ".",
        56 => "/",
        57 => "Caps Lock",
        70 => "Print Screen",
        71 => "Scroll Lock",
        72 => "Pause",
        73 => "Insert",
        74 => "Home",
        75 => "Page Up",
        76 => "Delete",
        77 => "End",
        78 => "Page Down",
        79 => "Right arrow",
        80 => "Left arrow",
        81 => "Down arrow",
        82 => "Up arrow",
        83 => "Num Lock",
        84 => "Keypad /",
        85 => "Keypad *",
        86 => "Keypad -",
        87 => "Keypad +",
        88 => "Keypad Enter",
        98 => "Keypad 0",
        99 => "Keypad .",
        224 => "Left Ctrl",
        225 => "Left Shift",
        226 => "Left Alt",
        227 => "Left Windows",
        228 => "Right Ctrl",
        229 => "Right Shift",
        230 => "Right Alt",
        231 => "Right Windows",
        _ => return format!("Key {code}"),
    };
    plain.to_string()
}

/// A controller's element as VPX's settings number them — its buttons from 0, its hats' four ways
/// from 0x100, its axes from 0x200, an axis past or below a point — by an Xbox controller's own
/// labels (SDL's order for it: A, B, X, Y, LB, RB, View, Menu, the sticks pressed; its axes the left
/// stick's two, LT, the right stick's two, RT), else by the controller's name and number.
fn pad_name(device: &str, element: u32, way: Option<&str>, threshold: Option<f32>) -> String {
    let xbox = device.to_lowercase().contains("xbox") || device.to_lowercase().contains("xinput");
    let short: String = device.split('#').next().unwrap_or(device).trim().chars().take(24).collect();
    let who = if short.is_empty() { "Controller".to_string() } else { short };
    let below = way == Some("x");
    if element < 0x100 {
        const XBOX: [&str; 11] = ["A", "B", "X", "Y", "LB", "RB", "View", "Menu", "Left stick press", "Right stick press", "Xbox button"];
        return match XBOX.get(element as usize).filter(|_| xbox) {
            Some(n) => (*n).to_string(),
            None => format!("{who} button {}", element + 1),
        };
    }
    if element < 0x200 {
        let hat = (element - 0x100) / 4;
        let dir = ["left", "right", "up", "down"][((element - 0x100) % 4) as usize];
        return if hat == 0 { format!("D-pad {dir}") } else { format!("{who} hat {} {dir}", hat + 1) };
    }
    let axis = element - 0x200;
    if xbox {
        let further = threshold.is_some_and(|t| t > 0.0);
        return match axis {
            0 => format!("Left stick {}", if below { "left" } else { "right" }),
            1 => format!("Left stick {}", if below { "up" } else { "down" }),
            3 => format!("Right stick {}", if below { "left" } else { "right" }),
            4 => format!("Right stick {}", if below { "up" } else { "down" }),
            2 => (if further { "LT, pressed further" } else { "LT" }).to_string(),
            5 => (if further { "RT, pressed further" } else { "RT" }).to_string(),
            n => format!("Axis {}", n + 1),
        };
    }
    format!("{who} axis {}", axis + 1)
}

/// Where the achievements stand, made up, for the test mode's picture of the menu.
fn example_achievements() -> (Option<String>, serde_json::Value) {
    let lines = serde_json::json!([
        { "heading": "**Example pack**" },
        { "line": "**First Blood** · Earned · 10 points\nScore a jackpot" },
        { "line": "**Halfway There** · 2 of 4 · 20 points\nLight every mode" },
        { "line": "**The Long Game** · 25 points\nReach 500 million in one game" },
    ]);
    (Some("Hardcore · 1 of 3 earned · 10 of 55 points".into()), lines)
}

/// VPX's playfield window, by the title VPX gives it: its handle and where it is, in desktop pixels.
#[cfg(windows)]
fn vpx_window() -> Option<(isize, i32, i32, i32, i32)> {
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> *mut std::ffi::c_void;
        fn GetWindowRect(hwnd: *mut std::ffi::c_void, rect: *mut Rect) -> i32;
    }
    let title: Vec<u16> = "Visual Pinball Player".encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: a NUL-ended title; no class asked for
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd.is_null() {
        return None;
    }
    let mut r = Rect::default();
    // SAFETY: a window Windows found, a rectangle to fill
    (unsafe { GetWindowRect(hwnd, &mut r) } != 0).then(|| (hwnd as isize, r.left, r.top, r.right - r.left, r.bottom - r.top))
}

#[cfg(not(windows))]
fn vpx_window() -> Option<(isize, i32, i32, i32, i32)> {
    None
}

/// VPX's playfield window given the focus (the menu's window had it, so Windows lets it).
fn focus_vpx() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn SetForegroundWindow(hwnd: *mut std::ffi::c_void) -> i32;
        }
        if let Some((hwnd, ..)) = vpx_window() {
            // SAFETY: a window Windows found
            unsafe { SetForegroundWindow(hwnd as *mut std::ffi::c_void) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> Vec<(String, String, String)> {
        vpi_settings::parse_ini(text)
    }

    /// The keys and buttons named as a player knows them: a controller's by an Xbox controller's
    /// labels, its triggers past their rest and pressed further, VPX's defaults where nothing's set.
    #[test]
    fn controls_named_from_vpxs_mappings() {
        let main = ini(
            "[Input]\nDevice.SDLJoy_03_1.Name = Xbox Series X Controller #1\nMapping.LeftFlipper = Key;225 | SDLJoy_03_1;514;o;-0.300000\nMapping.RightFlipper = Key;229 | SDLJoy_03_1;517;o;-0.300000\n\
             Mapping.LeftStagedFlipper = Key;225 | SDLJoy_03_1;514;o;0.300000\nMapping.LaunchBall = Key;40 | SDLJoy_03_1;0\nMapping.Start = Key;30 | SDLJoy_03_1;1\nMapping.Credit1 = Key;34 | SDLJoy_03_1;3\n\
             Mapping.Service1 = Key;36 | SDLJoy_03_1;256\nMapping.Tilt = \nMapping.GenTournament = Key;226 & Key;30\n",
        );
        assert_eq!(names_of(&main, &mapping(&main, "LeftFlipper")), ["Left Shift", "LT"]);
        assert_eq!(names_of(&main, &mapping(&main, "LeftStagedFlipper")), ["Left Shift", "LT, pressed further"]);
        assert_eq!(names_of(&main, &mapping(&main, "LaunchBall")), ["Enter", "A"]);
        assert_eq!(names_of(&main, &mapping(&main, "Service1")), ["7", "D-pad left"]);
        assert_eq!(names_of(&main, &mapping(&main, "GenTournament")), ["Left Alt + 1"]);
        // unset: VPX's default; set to nothing: nothing
        assert_eq!(names_of(&main, &mapping(&main, "CenterNudge")), ["Space"]);
        assert!(names_of(&main, &mapping(&main, "Tilt")).is_empty());
        // another controller by its name and number
        assert_eq!(pad_name("Pinscape Controller", 4, None, None), "Pinscape Controller button 5");
        let lines = controls(&main, &[("Magna-saves", &["LeftMagna", "RightMagna"])]);
        assert_eq!(lines[0]["section"], "This table");
        assert_eq!(lines[0]["keys"], "Left Ctrl  ·  Right Ctrl");
        assert_eq!(lines[1]["label"], "Flippers");
        assert_eq!(lines[1]["keys"], "Left Shift or LT  ·  Right Shift or RT");
    }

    /// VPX's menu keys, the keyboard's alone, its defaults where its settings map nothing.
    #[test]
    fn the_menu_moves_by_vpxs_menu_keys() {
        let main = ini("[Input]\nMapping.UIUp = Key;224 | SDLJoy_03_1;258\nMapping.UIDown = Key;82\nMapping.Start = Key;30 & Key;225\n");
        let n = nav(&main);
        assert_eq!(n["up"], serde_json::json!([224]));
        assert_eq!(n["down"], serde_json::json!([82]));
        assert_eq!(n["left"], serde_json::json!([225]));
        // Start held with another key isn't a key of its own
        assert_eq!(n["choose"], serde_json::json!([40]));
    }

    /// The parts of the sound in the order they show, an unknown add-on's by its own name.
    #[test]
    fn the_parts_of_the_sound() {
        let sources = serde_json::json!([{ "id": "VPIAchievements", "name": "Achievements" }, { "id": "Other", "name": "Its sound" }, { "id": "PUP", "name": "PUP" }, { "id": "AltSound", "name": "AltSound" }]);
        let levels = BTreeMap::from([("PUP".to_string(), 0.5)]);
        let rows = level_rows(&sources, &levels, true);
        let labels: Vec<&str> = rows.iter().map(|r| r["label"].as_str().unwrap()).collect();
        assert_eq!(labels, ["Table sounds", "Machine (AltSound)", "Music", "PinUP pack", "Achievements", "Its sound"]);
        assert_eq!(rows[3]["value"], 0.5);
        assert_eq!(level_rows(&serde_json::json!([]), &BTreeMap::new(), false).len(), 1);
    }

    /// A part's level told to the add-on as it takes it, a table's own by its share of what the
    /// table was made with.
    #[test]
    fn a_parts_level_told_to_the_add_on() {
        let facts = Facts { sound_volume: 0.5, music_volume: 1.0, music: Some(true), ..Facts::default() };
        assert_eq!(level_line("table", 0.8, &facts), serde_json::json!({ "do": "table-sounds", "percent": 40 }));
        assert_eq!(level_line("music", 0.5, &facts), serde_json::json!({ "do": "table-music", "percent": 50, "now": true }));
        assert_eq!(level_line("PUP", 0.75, &facts), serde_json::json!({ "do": "level", "id": "PUP", "value": 0.75 }));
        let kept = BTreeMap::from([("PUP".to_string(), 0.75), ("table".to_string(), 0.8), ("music".to_string(), 0.5)]);
        let v = levels_for_add_on(&kept, &facts);
        assert_eq!(v["addOns"]["PUP"], 0.75);
        assert!(v["addOns"].get("table").is_none());
        assert_eq!(v["table"], 40);
        assert_eq!(v["music"]["percent"], 50);
    }

    /// A level kept above what the part came with is read as what it came with.
    #[test]
    fn a_level_never_above_as_it_came() {
        let kept = BTreeMap::from([("PUP".to_string(), 1.5), ("B2S".to_string(), 0.95)]);
        let levels = as_kept(Some(&kept));
        assert_eq!(levels["PUP"], 1.0);
        assert_eq!(levels["B2S"], 0.95);
        assert!(as_kept(None).is_empty());
    }

    /// The volume VPX's keys say: the table's own over the main settings, then what the menu pressed.
    #[test]
    fn the_volume_as_the_game_plays_it() {
        let main = ini("[Player]\nMusicVolume = 80\nSoundVolume = 60\n");
        assert_eq!(volume(&[], &main, 0), 80);
        assert_eq!(volume(&ini("[Player]\nMusicVolume = 50\n"), &main, -5), 45);
        assert_eq!(volume(&[], &main, 40), 100);
        assert_eq!(volume(&[], &[], 0), 100);
    }
}
