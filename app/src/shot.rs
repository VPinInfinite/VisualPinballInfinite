//! The app's test mode, as the player's (`--shot`): the window opened off the screens, the screens
//! told which page to show, and once they say everything on it has loaded, a picture of the page
//! saved and the app closed — so the screens can be checked without a window over the user's own.
//!
//! `vpi-app --shot picture.png [--page library|table|browser|settings] [--table <part of a name>]
//! [--pick <a library group>] [--picture playfield|backglass|wheel] [--view tiles|list]
//! [--size 1600x1000] [--full] [--drop <file or folder>]...` (`--full`: the whole page, not only
//! what fits the window; the picture, view and group shown for the picture only, the user's own
//! preferences left as they are; `--drop`: the drop window opened with those, as if dropped on the
//! page — its table's, on a table's page — the picture taken once they're sorted out, nothing added
//! unless `--add` asks, as the Add button does: only ever with `VPI_LAYOUT` naming a scratch folder;
//! `--test`: on a table's page, the table tested first, as its Test button does, what it found kept
//! for the page alone; `--see`: the first download's "See them on VPS" opened; `--table-settings`: on
//! a table's page, its settings window opened; `--make-pictures`: on a table's page, its pictures
//! made by our player first, as its Make its pictures button does — kept, as the button keeps them)

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    #[serde(skip)]
    pub path: PathBuf,
    pub page: String,
    pub table: Option<String>,
    /// The library's group, picture and view (tiles or list).
    pub pick: Option<String>,
    pub picture: Option<String>,
    pub view: Option<String>,
    /// The library's tables with more than one table file shown once (`combined`) or each on its
    /// own (`each`), over the preferences' (`--builds`).
    pub builds: Option<String>,
    #[serde(skip)]
    pub size: (f64, f64),
    /// The whole page, its pictures out of view loaded too.
    pub full: bool,
    /// Files dropped on the page, the drop window shown.
    pub drop: Vec<String>,
    /// The drop's ticked files added, as the Add button does.
    pub add: bool,
    /// The table shown tested, as its Test button does.
    pub test: bool,
    /// The drop window's first "See them on VPS" opened.
    /// Cabinet mode shown, in the test's window alone (`--cabinet`).
    pub cabinet: bool,
    pub see: bool,
    /// The table's settings window opened.
    pub table_settings: bool,
    /// The table's page showing notes from a last game as VPX's log would give them, one of each
    /// kind (`--last-game-notes`), there being no game in the test mode.
    pub last_game_notes: bool,
    /// The table's pictures made by our player first.
    pub make_pictures: bool,
    /// The library's ROMs checked in the background as when the app is used, the picture taken
    /// once they're done (else only what's been checked before shows).
    pub checks: bool,
    /// The table's VPS entry searched for by hand, its first machine's first release picked: the
    /// two ways of keeping the match shown.
    pub find_vps: bool,
    /// The table's Change the ROM window opened.
    pub change_rom: bool,
    /// The table's own screens opened (its settings' View, Custom).
    pub custom_screens: bool,
    /// The table's first stack of media opened (a kind with several), or its screenshots.
    pub open_stack: bool,
    /// The window of one kind of the table's media opened, by its name on the page (`Wheel`).
    pub open_media: Option<String>,
    /// Files dropped onto the drop window once the first drop shows, joining it.
    pub then_drop: Vec<String>,
    /// Buttons clicked once the page shows, in order, each by the start of its words — only ones
    /// that ask before doing anything, or change nothing.
    pub click: Vec<String>,
    /// Buttons of the drop window clicked once it shows, as `click` (`--drop-click`).
    pub drop_click: Vec<String>,
    /// Milliseconds more before the picture, for a web page in the panel to load (`--wait`).
    pub wait: Option<u64>,
}

/// The test mode's options, when it's asked for.
pub fn from_args() -> Option<Shot> {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let path = PathBuf::from(value("--shot")?);
    let size = value("--size")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1600.0, 1000.0));
    Some(Shot {
        path,
        page: value("--page").unwrap_or_else(|| "library".into()),
        table: value("--table"),
        pick: value("--pick"),
        picture: value("--picture"),
        view: value("--view"),
        builds: value("--builds"),
        size,
        full: args.iter().any(|a| a == "--full"),
        drop: args.windows(2).filter(|w| w[0] == "--drop").map(|w| w[1].clone()).collect(),
        // never into the user's own folders: only with a scratch layout
        add: args.iter().any(|a| a == "--add") && std::env::var_os("VPI_LAYOUT").is_some(),
        test: args.iter().any(|a| a == "--test"),
        see: args.iter().any(|a| a == "--see"),
        cabinet: args.iter().any(|a| a == "--cabinet"),
        table_settings: args.iter().any(|a| a == "--table-settings"),
        last_game_notes: args.iter().any(|a| a == "--last-game-notes"),
        make_pictures: args.iter().any(|a| a == "--make-pictures"),
        checks: args.iter().any(|a| a == "--checks"),
        find_vps: args.iter().any(|a| a == "--find-vps"),
        change_rom: args.iter().any(|a| a == "--change-rom"),
        custom_screens: args.iter().any(|a| a == "--custom-screens"),
        open_stack: args.iter().any(|a| a == "--open-stack"),
        open_media: value("--open-media"),
        then_drop: args.windows(2).filter(|w| w[0] == "--then-drop").map(|w| w[1].clone()).collect(),
        click: args.windows(2).filter(|w| w[0] == "--click").map(|w| w[1].clone()).collect(),
        drop_click: args.windows(2).filter(|w| w[0] == "--drop-click").map(|w| w[1].clone()).collect(),
        wait: value("--wait").and_then(|w| w.parse().ok()),
    })
}

/// A picture of the page, saved where the test mode was told, then the app closed. The pause menu's
/// guide, a window of its own over the page, saved first beside it (`<picture>-guide.png`); a web
/// page in the panel (the in-app browser, a view of its own laid over the page) saved beside it
/// (`<picture>-web.png`) and laid into the page's picture where it shows.
pub fn take(app: &AppHandle, shot: &Shot) -> Result<(), String> {
    let window = crate::main_window::page(app).ok_or("no window")?;
    let (path, full, done) = (shot.path.clone(), shot.full, app.clone());
    let stem = shot.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    // the page's picture, then the app closed; `web`, the panel's page laid into it where it shows
    let page = move |web: Option<Laid>| {
        let exit = done.clone();
        let whole = path.clone();
        let taken = window.with_webview(move |webview| {
            capture(
                webview,
                full,
                path,
                Box::new(move |saved| {
                    if saved.is_ok()
                        && let Some(part) = web
                        && part.path.is_file()
                        && let Err(e) = lay_in(&whole, &part)
                    {
                        eprintln!("The web page couldn't be laid into the picture: {e}");
                    }
                    exit.exit(if saved.is_ok() { 0 } else { 1 })
                }),
            )
        });
        if taken.is_err() {
            done.exit(1);
        }
    };
    // a web page in the panel: its own picture first, from a thread of its own — the view's place
    // and size are asked of the window's thread, which this command runs on — and the page's
    // picture alone where it never comes
    if let Some(view) = app.get_webview(crate::web_view::LABEL).filter(|_| !full) {
        let web_path = shot.path.with_file_name(format!("{stem}-web.png"));
        std::thread::spawn(move || {
            let laid = view.position().ok().zip(view.size().ok()).map(|(at, size)| Laid {
                path: web_path.clone(),
                x: at.x.max(0) as u32,
                y: at.y.max(0) as u32,
                width: size.width,
                height: size.height,
            });
            let once = std::sync::Arc::new(std::sync::Mutex::new(Some(page)));
            let go = move |web: Option<Laid>| {
                let page = once.lock().unwrap_or_else(|e| e.into_inner()).take();
                if let Some(page) = page {
                    page(web);
                }
            };
            let late = go.clone();
            let asked = view.with_webview(move |webview| capture(webview, false, web_path, Box::new(move |saved| go(laid.filter(|_| saved.is_ok())))));
            std::thread::sleep(std::time::Duration::from_secs(if asked.is_ok() { 20 } else { 0 }));
            late(None);
        });
        return Ok(());
    }
    match app.get_webview_window("pause-guide") {
        Some(view) => {
            let beside = shot.path.with_file_name(format!("{stem}-guide.png"));
            view.with_webview(move |webview| capture(webview, false, beside, Box::new(move |_| page(None)))).map_err(|e| e.to_string())
        }
        None => {
            page(None);
            Ok(())
        }
    }
}

/// A picture of the panel's web page, and where it goes in the page's: in the window's pixels, as
/// both pictures are taken.
struct Laid {
    path: PathBuf,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// The panel's picture laid into the page's at `whole`, at its place and fitted to its size.
fn lay_in(whole: &std::path::Path, part: &Laid) -> Result<(), String> {
    let mut picture = image::open(whole).map_err(|e| e.to_string())?.to_rgba8();
    let mut page = image::open(&part.path).map_err(|e| e.to_string())?.to_rgba8();
    if (page.width(), page.height()) != (part.width, part.height) && part.width > 0 && part.height > 0 {
        page = image::imageops::resize(&page, part.width, part.height, image::imageops::FilterType::Lanczos3);
    }
    image::imageops::overlay(&mut picture, &page, i64::from(part.x), i64::from(part.y));
    picture.save(whole).map_err(|e| e.to_string())
}

/// A picture of what a web view shows saved to `path` (`full`: all of its page), then `after` told
/// whether it was.
#[cfg(windows)]
fn capture(webview: tauri::webview::PlatformWebview, full: bool, path: PathBuf, after: Box<dyn FnOnce(Result<(), String>) + Send>) {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;

    let after = std::sync::Arc::new(std::sync::Mutex::new(Some(after)));
    let tell = {
        let after = after.clone();
        move |saved: Result<(), String>| {
            match &saved {
                Ok(()) => {}
                Err(e) => eprintln!("the picture wasn't saved: {e}"),
            }
            if let Some(f) = after.lock().unwrap().take() {
                f(saved);
            }
        }
    };
    let params = if full { r#"{"format":"png","captureBeyondViewport":true}"# } else { r#"{"format":"png"}"# };
    let told = tell.clone();
    let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, json: String| {
        let saved = result.map_err(|e| e.to_string()).and_then(|()| {
            let reply: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let data = reply["data"].as_str().ok_or("no picture in the reply")?;
            std::fs::write(&path, base64(data)?).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("saved {}", path.display());
            Ok(())
        });
        told(saved);
        Ok(())
    }));
    // SAFETY: the web view's own objects, used on its thread as Tauri hands it
    let asked = unsafe { webview.controller().CoreWebView2().and_then(|core| core.CallDevToolsProtocolMethod(&HSTRING::from("Page.captureScreenshot"), &HSTRING::from(params), &handler)) };
    if let Err(e) = asked {
        tell(Err(e.to_string()));
    }
}

#[cfg(not(windows))]
fn capture(_: tauri::webview::PlatformWebview, _: bool, _: PathBuf, after: Box<dyn FnOnce(Result<(), String>) + Send>) {
    after(Err("the test mode's pictures are taken on Windows only".into()));
}

/// Standard base64, as the web viewer hands its pictures.
fn base64(text: &str) -> Result<Vec<u8>, String> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = acc << 6 | u32::from(value(c).ok_or("not base64")?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_as_the_web_viewer_writes_it() {
        assert_eq!(super::base64("aGVsbG8gd29ybGQ=").unwrap(), b"hello world");
        assert_eq!(super::base64("YQ==").unwrap(), b"a");
    }
}
