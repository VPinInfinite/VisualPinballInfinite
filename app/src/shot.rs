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
        size,
        full: args.iter().any(|a| a == "--full"),
        drop: args.windows(2).filter(|w| w[0] == "--drop").map(|w| w[1].clone()).collect(),
        // never into the user's own folders: only with a scratch layout
        add: args.iter().any(|a| a == "--add") && std::env::var_os("VPI_LAYOUT").is_some(),
        test: args.iter().any(|a| a == "--test"),
        see: args.iter().any(|a| a == "--see"),
        cabinet: args.iter().any(|a| a == "--cabinet"),
        table_settings: args.iter().any(|a| a == "--table-settings"),
        make_pictures: args.iter().any(|a| a == "--make-pictures"),
        checks: args.iter().any(|a| a == "--checks"),
        find_vps: args.iter().any(|a| a == "--find-vps"),
        change_rom: args.iter().any(|a| a == "--change-rom"),
        custom_screens: args.iter().any(|a| a == "--custom-screens"),
        open_stack: args.iter().any(|a| a == "--open-stack"),
        open_media: value("--open-media"),
        then_drop: args.windows(2).filter(|w| w[0] == "--then-drop").map(|w| w[1].clone()).collect(),
        click: args.windows(2).filter(|w| w[0] == "--click").map(|w| w[1].clone()).collect(),
    })
}

/// A picture of the page, saved where the test mode was told, then the app closed.
pub fn take(app: &AppHandle, shot: &Shot) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("no window")?;
    let (path, full, done) = (shot.path.clone(), shot.full, app.clone());
    window
        .with_webview(move |webview| {
            if let Err(e) = capture(webview, full, path, done.clone()) {
                eprintln!("the picture wasn't taken: {e}");
                done.exit(1);
            }
        })
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn capture(webview: tauri::webview::PlatformWebview, full: bool, path: PathBuf, app: AppHandle) -> Result<(), String> {
    use webview2_com::CallDevToolsProtocolMethodCompletedHandler;
    use windows::core::HSTRING;

    let params = if full { r#"{"format":"png","captureBeyondViewport":true}"# } else { r#"{"format":"png"}"# };
    let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(move |result, json: String| {
        let saved = result.map_err(|e| e.to_string()).and_then(|()| {
            let reply: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            let data = reply["data"].as_str().ok_or("no picture in the reply")?;
            std::fs::write(&path, base64(data)?).map_err(|e| format!("{}: {e}", path.display()))
        });
        match saved {
            Ok(()) => {
                println!("saved {}", path.display());
                app.exit(0);
            }
            Err(e) => {
                eprintln!("the picture wasn't saved: {e}");
                app.exit(1);
            }
        }
        Ok(())
    }));
    unsafe {
        let core = webview.controller().CoreWebView2().map_err(|e| e.to_string())?;
        core.CallDevToolsProtocolMethod(&HSTRING::from("Page.captureScreenshot"), &HSTRING::from(params), &handler).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
fn capture(_: tauri::webview::PlatformWebview, _: bool, _: PathBuf, _: AppHandle) -> Result<(), String> {
    Err("the test mode's pictures are taken on Windows only".into())
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
