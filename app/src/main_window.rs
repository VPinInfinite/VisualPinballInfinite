//! The app's own window and its page. The page is the window's first web view; the in-app
//! browser's page is laid into the same window beside it (`web_view`), and from then on Tauri no
//! longer counts the window as a webview window — `get_webview_window` finds nothing, and a command
//! asking for a `WebviewWindow` is refused — so the window is found as a window, and its page as a
//! web view, and commands from the page ask for its `Window`.

use tauri::{AppHandle, Manager};

/// The app's window, and its page's web view.
pub const LABEL: &str = "main";

/// The app's window.
pub fn window(app: &AppHandle) -> Option<tauri::Window> {
    app.get_window(LABEL)
}

/// The app's page.
pub fn page(app: &AppHandle) -> Option<tauri::Webview> {
    app.get_webview(LABEL)
}

/// The app's window brought to the front with the keyboard's focus in its page: Windows gives a
/// window focus, but the web view inside it takes the keys only once focused itself.
pub fn focus(app: &AppHandle) {
    if let Some(w) = window(app) {
        let _ = w.set_focus();
    }
    if let Some(p) = page(app) {
        let _ = p.set_focus();
    }
}
