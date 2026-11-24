//! A web page in the VPS browser's panel: a file's page VPS links to (VPU's, VPForums', anyone's),
//! shown inside the app's window over the panel, as a browser shows it. The user signs in on the
//! site's own page and stays so, in a browser profile of the app's own, apart from its screens' — the
//! app never sees what's typed there. Its downloads go into a folder of their own beside the user's
//! downloads, waiting to be imported through the drop window, which sorts them by table.
//!
//! The page reaches none of the app's commands — a page from another site gets none unless a
//! capability names its site, and none does — and it can't load the app's own pages (`*.localhost`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Url, WebviewUrl};

/// The panel's web view, a webview of the app's window.
pub const LABEL: &str = "web";

/// The downloads under way: where each goes, the address it came from, its task in the tasks bar.
static DOWNLOADING: Mutex<Vec<(PathBuf, String, crate::tasks::Running)>> = Mutex::new(Vec::new());

/// The pages a Get opened, each with the VPS machine it was for: a download from one is that
/// machine's.
static GETS: Mutex<Vec<(Url, String)>> = Mutex::new(Vec::new());

/// The page the panel shows, as it last finished loading.
static PAGE: Mutex<Option<Url>> = Mutex::new(None);

/// The machine a download is for: the Get it came from — on the same site, the download, or the
/// page it was begun from, under the Get's page (VPU's `/files/file/<number>-<name>/`), or the same
/// file's number in the address (VPForums' `showfile` and `id`). The newest Get first.
fn machine_of(download: &Url) -> Option<String> {
    let page = PAGE.lock().unwrap().clone();
    let file_no = |u: &Url| u.query_pairs().find(|(k, _)| k == "showfile" || k == "id").map(|(_, v)| v.into_owned());
    let from = |u: &Url, get: &Url| {
        u.host_str() == get.host_str() && ((get.path().len() > 1 && !get.path().ends_with(".php") && u.path().starts_with(get.path())) || file_no(get).is_some_and(|n| file_no(u) == Some(n)))
    };
    let gets = GETS.lock().unwrap();
    gets.iter().rev().find(|(get, _)| from(download, get) || page.as_ref().is_some_and(|p| from(p, get))).map(|(_, game)| game.clone())
}

/// What each file waiting to be imported was got for, by its name: its VPS machine. Beside the
/// folder `VPI_INBOX` names, where it names one (`<folder>.json`).
fn sources_file() -> Option<PathBuf> {
    if let Some(folder) = std::env::var_os("VPI_INBOX") {
        return Some(PathBuf::from(folder).with_extension("json"));
    }
    Some(crate::library::data_folder()?.join("Library").join("waiting.json"))
}

fn sources() -> std::collections::BTreeMap<String, String> {
    sources_file().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// A waiting file's machine kept, or forgotten (`None`).
fn set_source(name: &str, game: Option<&str>) {
    let Some(file) = sources_file() else { return };
    let mut all = sources();
    match game {
        Some(g) => all.insert(name.to_string(), g.to_string()),
        None => all.remove(name),
    };
    if let Ok(text) = serde_json::to_vec_pretty(&all) {
        let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(&file, text);
    }
}

/// The folder downloads wait in to be imported: the app's own in the user's Downloads, or the one
/// `VPI_INBOX` names (the test mode's pictures, from a scratch folder).
pub fn inbox(app: &AppHandle) -> Result<PathBuf, String> {
    let folder = match std::env::var_os("VPI_INBOX") {
        Some(f) => PathBuf::from(f),
        None => app.path().download_dir().map_err(|e| format!("The Downloads folder: {e}"))?.join("Visual Pinball Infinite"),
    };
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    Ok(folder)
}

/// The web view's own browser profile: its sign-ins, its cache.
fn profile(app: &AppHandle) -> Result<PathBuf, String> {
    // another, where `VPI_WEB_PROFILE` names one: the test mode's pictures, signed in nowhere
    if let Some(folder) = std::env::var_os("VPI_WEB_PROFILE") {
        return Ok(PathBuf::from(folder));
    }
    Ok(app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("Web"))
}

/// Whether a view shows another site's pages: the panel's, and the pause menu's guide. The app's
/// own protocols answer them nothing (`pictures::handle`).
pub fn shows_other_sites(label: &str) -> bool {
    label == LABEL || label == crate::pause_menu::GUIDE
}

/// An address the panel shows: a web page, never one of the app's own.
fn allowed(url: &Url) -> bool {
    matches!(url.scheme(), "https" | "http")
        && url.host_str().is_some_and(|h| {
            let h = h.to_ascii_lowercase();
            h != "localhost" && !h.ends_with(".localhost")
        })
}

/// What the panel's toolbar shows of the page.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    url: String,
    loading: bool,
}

/// A page shown in the panel, at the panel's place in the window's page: made the first time, then
/// taken to the page asked for. Made on a thread of its own (an async command): WebView2 waits on
/// the window's thread as a webview is made.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn web_show(app: AppHandle, window: tauri::Window, url: String, game: Option<String>, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let url: Url = url.parse().map_err(|e| format!("{url}: {e}"))?;
    if !allowed(&url) {
        return Err("That isn't a web page.".into());
    }
    // the machine its Get was for, for what's downloaded from it
    if let Some(game) = game {
        GETS.lock().unwrap().push((url.clone(), game));
    }
    let (at, size) = (LogicalPosition::new(x, y), LogicalSize::new(width.max(1.0), height.max(1.0)));
    if let Some(view) = app.get_webview(LABEL) {
        view.navigate(url).map_err(|e| e.to_string())?;
        view.set_position(at).map_err(|e| e.to_string())?;
        view.set_size(size).map_err(|e| e.to_string())?;
        view.show().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let inbox = inbox(&app)?;
    let (on_load, on_new, on_down) = (app.clone(), app.clone(), app.clone());
    // the test mode's window takes no focus from the user's own: nor does its page
    let testing = app.try_state::<Option<crate::shot::Shot>>().is_some_and(|s| s.is_some());
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::External(url))
        .focused(!testing)
        .data_directory(profile(&app)?)
        .zoom_hotkeys_enabled(true)
        .on_navigation(allowed)
        // a link opening a window of its own: shown in the panel instead
        .on_new_window(move |to, _| {
            if allowed(&to)
                && let Some(view) = on_new.get_webview(LABEL)
            {
                // from the window's thread, which this runs on: done after it
                std::thread::spawn(move || {
                    let _ = view.navigate(to);
                });
            }
            NewWindowResponse::Deny
        })
        .on_download(move |_, event| download(&on_down, &inbox, event))
        .on_page_load(move |_, payload| {
            let loading = payload.event() == PageLoadEvent::Started;
            if !loading {
                *PAGE.lock().unwrap() = Some(payload.url().clone());
            }
            let _ = on_load.emit("web-page", Page { url: payload.url().to_string(), loading });
        });
    window.add_child(builder, at, size).map_err(|e| e.to_string())?;
    Ok(())
}

/// The web view at the panel's place and in sight, as the window or the panel changes size, or a
/// window of the app's own that was over it closes.
#[tauri::command]
pub fn web_place(app: AppHandle, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let Some(view) = app.get_webview(LABEL) else { return Ok(()) };
    view.set_position(LogicalPosition::new(x, y)).map_err(|e| e.to_string())?;
    view.set_size(LogicalSize::new(width.max(1.0), height.max(1.0))).map_err(|e| e.to_string())?;
    view.show().map_err(|e| e.to_string())
}

/// The web view out of sight — the panel showing a machine again, another screen, a window over
/// the app's — its page kept for coming back to.
#[tauri::command]
pub fn web_hide(app: AppHandle) {
    if let Some(view) = app.get_webview(LABEL) {
        let _ = view.hide();
    }
}

/// Back to the page before, on to the next again, or the page loaded again.
#[tauri::command]
pub fn web_go(app: AppHandle, how: String) -> Result<(), String> {
    let view = app.get_webview(LABEL).ok_or("No page is showing.")?;
    match how.as_str() {
        "back" => view.eval("history.back()").map_err(|e| e.to_string()),
        "forward" => view.eval("history.forward()").map_err(|e| e.to_string()),
        "reload" => view.reload().map_err(|e| e.to_string()),
        _ => Err(format!("{how}: not a way to go")),
    }
}

/// The page showing opened in the user's own browser.
#[tauri::command]
pub fn web_outside(app: AppHandle) -> Result<(), String> {
    let view = app.get_webview(LABEL).ok_or("No page is showing.")?;
    let url = view.url().map_err(|e| e.to_string())?;
    crate::shell::open_link(url.as_str())
}

/// Signed out of every site the panel was signed in to: its profile's cookies and cache cleared.
#[tauri::command]
pub fn web_sign_out(app: AppHandle) -> Result<(), String> {
    match app.get_webview(LABEL) {
        Some(view) => {
            view.clear_all_browsing_data().map_err(|e| e.to_string())?;
            view.reload().map_err(|e| e.to_string())
        }
        None => {
            let folder = profile(&app)?;
            if folder.exists() {
                std::fs::remove_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
            }
            Ok(())
        }
    }
}

/// A download begun in the panel put in the waiting folder, under the name the site gives it
/// (numbered past one of that name there already), its progress in the tasks bar; once done, the
/// screens told.
fn download(app: &AppHandle, inbox: &Path, event: DownloadEvent<'_>) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => {
            let name = suggested_name(destination, inbox)
                .and_then(|n| plain_name(&n))
                .or_else(|| url.path_segments().and_then(|mut s| s.next_back()).and_then(plain_name))
                .unwrap_or_else(|| "download".into());
            let to = unique(inbox, &name);
            *destination = to.clone();
            // the machine it was got for, kept with it until it's imported
            if let Some(game) = machine_of(&url)
                && let Some(file) = to.file_name()
            {
                set_source(&file.to_string_lossy(), Some(&game));
            }
            let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name);
            let task = crate::tasks::begin(app, format!("Downloading {name}"), 0);
            DOWNLOADING.lock().unwrap().push((to.clone(), url.to_string(), task));
            let _ = app.emit("inbox", list(app).unwrap_or_default());
            progress(to, name);
            true
        }
        DownloadEvent::Finished { url, path, success } => {
            let done = {
                let mut going = DOWNLOADING.lock().unwrap();
                let at = going.iter().position(|(to, from, _)| path.as_deref() == Some(to.as_path()) || *from == url.as_str());
                at.map(|i| going.remove(i))
            };
            let name = done.as_ref().and_then(|(to, _, _)| to.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            // its task gone with it
            drop(done);
            let _ = app.emit("web-download", Downloaded { name, success });
            let _ = app.emit("inbox", list(app).unwrap_or_default());
            true
        }
        _ => true,
    }
}

/// The name a download comes as. The browser suggests a place in its own downloads folder, the
/// name numbered as a copy (`<name> (1).zip`) where that folder holds a file of the name already;
/// the download goes to the inbox instead, numbered against what's there (`unique`), so it keeps
/// the name it came with.
fn suggested_name(suggested: &Path, inbox: &Path) -> Option<String> {
    let name = suggested.file_name()?.to_string_lossy().into_owned();
    let Some(folder) = suggested.parent().filter(|f| *f != inbox) else { return Some(name) };
    for (at, _) in name.match_indices(" (") {
        let rest = &name[at + 2..];
        let Some(close) = rest.find(')') else { continue };
        let (number, after) = (&rest[..close], &rest[close + 1..]);
        // the browser numbers copies from 1 to 100, just before the extension
        let numbered = number.len() <= 3 && number.bytes().all(|b| b.is_ascii_digit()) && number.parse::<u32>().is_ok_and(|n| (1..=100).contains(&n));
        if !numbered || !(after.is_empty() || after.starts_with('.')) {
            continue;
        }
        let plain = format!("{}{after}", &name[..at]);
        if folder.join(&plain).exists() {
            return Some(plain);
        }
    }
    Some(name)
}

/// A download's name for the waiting folder: one name of a file, nothing that leads out of the
/// folder (`..`, a drive, a folder), no character Windows refuses in a name, no device's name. The
/// browser's own suggestion is such a name already; an address's last part needn't be.
fn plain_name(name: &str) -> Option<String> {
    let name = name.trim().trim_end_matches(['.', ' ']);
    let stem = name.split('.').next().unwrap_or_default().trim().to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit());
    let fine = !name.is_empty() && !device && !name.chars().any(|c| c.is_control() || r#"<>:"/\|?*"#.contains(c));
    fine.then(|| name.to_string())
}

/// How much of a download is here, said in its task each second until it's done: the browser's
/// file for it while it comes (`.crdownload` beside where it goes), else the file itself. A site
/// that doesn't say how big a file is gives no telling how far, as in any browser.
fn progress(to: PathBuf, name: String) {
    std::thread::spawn(move || {
        let partial = PathBuf::from(format!("{}.crdownload", to.display()));
        let mut said = 0u64;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let size = [&partial, &to].iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).max().unwrap_or(0);
            let going = DOWNLOADING.lock().unwrap();
            let Some((_, _, task)) = going.iter().find(|(p, _, _)| *p == to) else { return };
            if size != said {
                said = size;
                task.step(0, Some(format!("Downloading {name} · {}", in_words(size))));
            }
        }
    });
}

/// A size as people read it.
fn in_words(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.2} GB", b as f64 / f64::from(1u32 << 30)),
        b if b >= 1 << 20 => format!("{:.0} MB", b as f64 / f64::from(1u32 << 20)),
        b => format!("{} KB", b.div_ceil(1024)),
    }
}

/// A download's end, for the screens' notice.
#[derive(Clone, Serialize)]
struct Downloaded {
    name: String,
    success: bool,
}

/// `name` in `folder`, numbered as Windows numbers a copy where one of that name is there already.
fn unique(folder: &Path, name: &str) -> PathBuf {
    let first = folder.join(name);
    let going = |p: &Path| DOWNLOADING.lock().unwrap().iter().any(|(to, _, _)| to == p);
    if !first.exists() && !going(&first) {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    (1..)
        .map(|n| folder.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists() && !going(p))
        .unwrap_or(first)
}

/// One file waiting to be imported.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Waiting {
    pub name: String,
    pub path: String,
    pub size: u64,
    /// When it came, in ms since 1970.
    pub at: i64,
    /// Still downloading.
    pub downloading: bool,
    /// The VPS machine it was got for, where its Get says.
    pub game: Option<String>,
}

/// The files waiting to be imported, the newest first, those still downloading among them; the
/// browser's own files for a download under way (`.crdownload`, `.tmp`) aside.
pub fn list(app: &AppHandle) -> Result<Vec<Waiting>, String> {
    let folder = inbox(app)?;
    let going: Vec<PathBuf> = DOWNLOADING.lock().unwrap().iter().map(|(to, _, _)| to.clone()).collect();
    let got_for = sources();
    let mut out: Vec<Waiting> = std::fs::read_dir(&folder)
        .map_err(|e| format!("{}: {e}", folder.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file() || t.is_dir()))
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            !n.ends_with(".crdownload") && !n.ends_with(".tmp") && !n.ends_with(".partial") && n != "desktop.ini"
        })
        .map(|e| {
            let meta = e.metadata().ok();
            let at = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
            let name = e.file_name().to_string_lossy().into_owned();
            Waiting { game: got_for.get(&name).cloned(), path: e.path().to_string_lossy().into_owned(), size: meta.map_or(0, |m| m.len()), at, downloading: going.contains(&e.path()), name }
        })
        .collect();
    // those under way that have no file of their own yet
    for to in going {
        if !out.iter().any(|w| Path::new(&w.path) == to) {
            let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(Waiting { game: got_for.get(&name).cloned(), path: to.to_string_lossy().into_owned(), size: 0, at: i64::MAX, downloading: true, name });
        }
    }
    out.sort_by_key(|w| std::cmp::Reverse(w.at));
    Ok(out)
}

#[tauri::command]
pub fn inbox_list(app: AppHandle) -> Result<Vec<Waiting>, String> {
    list(&app)
}

/// One waiting file taken out, to the Recycle Bin.
#[tauri::command]
pub fn inbox_remove(app: AppHandle, name: String) -> Result<Vec<Waiting>, String> {
    let folder = inbox(&app)?;
    let path = folder.join(&name);
    // a name of the folder's own: one plain name, never `..` or a drive
    let mut parts = Path::new(&name).components();
    if !matches!((parts.next(), parts.next()), (Some(std::path::Component::Normal(_)), None)) || !path.starts_with(&folder) {
        return Err("That file isn't waiting to be imported.".into());
    }
    library::install::to_recycle_bin(&path)?;
    set_source(&name, None);
    let now = list(&app)?;
    let _ = app.emit("inbox", now.clone());
    Ok(now)
}

/// The waiting folder in Explorer.
#[tauri::command]
pub fn inbox_show(app: AppHandle) -> Result<(), String> {
    crate::shell::show(&inbox(&app)?)
}

/// A download the drop window added that came from the waiting folder: out of it, to the Recycle
/// Bin, the screens told.
pub fn imported(app: &AppHandle, source: &Path) {
    let Ok(folder) = inbox(app) else { return };
    if source.parent() != Some(folder.as_path()) {
        return;
    }
    if let Err(e) = library::install::to_recycle_bin(source) {
        eprintln!("{}: not taken out of the waiting folder: {e}", source.display());
    }
    if let Some(name) = source.file_name() {
        set_source(&name.to_string_lossy(), None);
    }
    let _ = app.emit("inbox", list(app).unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_download_keeps_the_name_it_came_with() {
        let base = std::env::temp_dir().join(format!("vpi-web-names-{}", std::process::id()));
        let (theirs, inbox) = (base.join("Downloads"), base.join("Downloads").join("Visual Pinball Infinite"));
        std::fs::create_dir_all(&inbox).unwrap();
        for f in ["Table (Williams 1991) 1.0.zip", "Big Trouble (balutito 2022).zip", "pack.tar.gz", "Jaws.zip"] {
            std::fs::write(theirs.join(f), b"x").unwrap();
        }
        let named = |n: &str| suggested_name(&theirs.join(n), &inbox).unwrap();
        // numbered by the browser for the file of its name in the user's own Downloads
        assert_eq!(named("Table (Williams 1991) 1.0 (1).zip"), "Table (Williams 1991) 1.0.zip");
        assert_eq!(named("Big Trouble (balutito 2022) (2).zip"), "Big Trouble (balutito 2022).zip");
        assert_eq!(named("pack (1).tar.gz"), "pack.tar.gz");
        // a name of its own with a number in it, nothing there without it
        assert_eq!(named("Side art (2).zip"), "Side art (2).zip");
        assert_eq!(named("Jaws (1975).zip"), "Jaws (1975).zip");
        assert_eq!(named("plain.zip"), "plain.zip");
        // suggested in the inbox already: numbered against it, as it is
        std::fs::write(inbox.join("again.zip"), b"x").unwrap();
        assert_eq!(suggested_name(&inbox.join("again (1).zip"), &inbox).unwrap(), "again (1).zip");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_download_is_named_inside_the_waiting_folder() {
        assert_eq!(plain_name("Jaws (1975).zip").as_deref(), Some("Jaws (1975).zip"));
        assert_eq!(plain_name("console.zip").as_deref(), Some("console.zip"));
        for out in ["", "  ", "..", "C:", "a:b.zip", "..\\evil.exe", "a/b.zip", "CON", "nul.txt", "COM1", "lpt9.zip"] {
            assert_eq!(plain_name(out), None, "{out:?}");
        }
    }

    #[test]
    fn only_the_panel_and_the_guide_show_other_sites() {
        assert!(shows_other_sites(LABEL));
        assert!(shows_other_sites(crate::pause_menu::GUIDE));
        assert!(!shows_other_sites(crate::main_window::LABEL));
        assert!(!shows_other_sites("pause-menu"));
    }
}
