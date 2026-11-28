//! PinUP Player's screens kept on Windows' screens before a game it shows a PinUP pack for — VPX
//! 10.8.0's, a newer VPX's with its own PinUP add-on off, a Future Pinball PinEvent table's: its
//! screens that show on none of Windows' screens moved with the screen they were on
//! (`library::pinup_screens`), the file before kept beside it. Ours.

use std::path::PathBuf;

/// PinUP's settings file: `PinUpPlayer.ini` in PinUP's folder, the one holding its packs' folder.
fn file() -> Option<PathBuf> {
    let layout = vpi_settings::library_folders_now().map(|f| ::library::plan::Layout::from_folders(&f))?;
    let file = layout.pup_videos.parent()?.join("PinUpPlayer.ini");
    file.is_file().then_some(file)
}

/// Before a game PinUP Player shows a pack for: its screens that show on none of Windows' screens
/// moved back onto the screen they were on, wherever Windows has moved it on the desktop, the file
/// before kept as `PinUpPlayer.previous.ini`. Never in the way of the game.
pub fn before_play() {
    let Some(file) = file() else { return };
    let Ok(bytes) = std::fs::read(&file) else { return };
    // a file in another encoding left as it is
    let Ok(text) = std::str::from_utf8(&bytes) else { return };
    let monitors: Vec<::library::pinup_screens::Monitor> = crate::b2s_screen::screens().into_iter().map(|((x, y), _, (w, h))| (x, y, w, h)).collect();
    let Some((after, said)) = ::library::pinup_screens::follow_monitors(text, &monitors) else { return };
    let _ = std::fs::write(file.with_file_name("PinUpPlayer.previous.ini"), &bytes);
    match std::fs::write(&file, after) {
        Ok(()) => eprintln!("PinUP's screens moved back onto Windows' screens: {}", said.join(", ")),
        Err(e) => eprintln!("PinUP's screens weren't moved: {e}"),
    }
}
