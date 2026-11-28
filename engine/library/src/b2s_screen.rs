//! B2S Server's screen file (`ScreenRes.txt`): where the classic backglass program puts its
//! backglass window and its own DMD window, as `B2SScreen.ReadB2SSettingsFromFile` reads it. It
//! takes the file's lines that don't start with `#`, in order: the playfield's width and height;
//! the backglass's width and height; the screen it's on — the number of Windows' `\\.\DISPLAY<n>`,
//! or `@<x>`, the screen whose left edge is at x, or `=<n>`, the n-th from the left; the
//! backglass's x and y in that screen; its DMD window's width and height, then its x and y from the
//! backglass window's corner (0 and 0: no window of its own, the backglass's grill shown); the LEDs
//! flipped; and in a longer file a background's place, size and picture. It looks for a backglass
//! file's own `<name>.res` first, then the table's, a `ScreenRes.txt` in a folder named as the
//! table, one in the folder it runs in, and last its own folder's.

/// A window's place and size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Where B2S Server's windows go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The playfield window's size.
    pub playfield: (i32, i32),
    /// The backglass's screen, by its number in Windows' name for it (`\\.\DISPLAY<n>`), the way
    /// B2S Server's own screen tool writes it.
    pub screen: u32,
    /// The backglass window, its place in its screen's pixels.
    pub backglass: Rect,
    /// B2S Server's DMD window, its place from the backglass window's corner; `None` leaves the
    /// file's as it is.
    pub dmd: Option<Rect>,
}

/// The file's name, unless B2S Server's own setting renames it.
pub const FILE_NAME: &str = "ScreenRes.txt";

/// The values B2S Server reads from the file, without their line endings: every line not starting
/// with `#`, a blank one too.
pub fn values(text: &[u8]) -> Vec<String> {
    let (_, lines, _) = split(text);
    lines.iter().filter(|l| !l.starts_with(b"#")).map(|l| String::from_utf8_lossy(l).into_owned()).collect()
}

/// The file's lines without their endings, any UTF-8 byte order mark set apart first, and whether
/// the last line has its ending.
fn split(text: &[u8]) -> (&[u8], Vec<&[u8]>, bool) {
    const BOM: &[u8] = b"\xEF\xBB\xBF";
    let (bom, text) = if text.starts_with(BOM) { text.split_at(BOM.len()) } else { (&text[..0], text) };
    let ended = text.ends_with(b"\n");
    let body = if ended { &text[..text.len() - 1] } else { text };
    let lines = if body.is_empty() && ended {
        vec![&body[..0]]
    } else if body.is_empty() {
        Vec::new()
    } else {
        body.split(|&b| b == b'\n').map(|l| l.strip_suffix(b"\r").unwrap_or(l)).collect()
    };
    (bom, lines, ended)
}

/// The file with `place` in it: only its first values changed — the playfield's size, the
/// backglass's size, screen and place, and the DMD window's where `place` says it — the comments,
/// the line endings and every other value as they were; a new file where there's none.
pub fn with_place(before: Option<&[u8]>, place: &Place) -> Vec<u8> {
    let b = place.backglass;
    let mut wanted: Vec<String> = [place.playfield.0, place.playfield.1, b.width, b.height].iter().map(i32::to_string).collect();
    wanted.push(place.screen.to_string());
    wanted.extend([b.x, b.y].iter().map(i32::to_string));
    if let Some(d) = place.dmd {
        wanted.extend([d.width, d.height, d.x, d.y].iter().map(i32::to_string));
    }
    let Some(before) = before.filter(|t| !t.is_empty()) else {
        // B2S Server's twelve values: with no DMD window said, none of its own, the LEDs as drawn
        let mut lines = wanted;
        lines.resize(11, "0".into());
        lines.push("0".into());
        return format!("{}\r\n", lines.join("\r\n")).into_bytes();
    };
    let eol: &[u8] = if before.windows(2).any(|w| w == b"\r\n") { b"\r\n" } else { b"\n" };
    let (bom, lines, ended) = split(before);
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(lines.len() + 12);
    let mut n = 0;
    for line in lines {
        if line.starts_with(b"#") {
            out.push(line.to_vec());
            continue;
        }
        out.push(wanted.get(n).map_or_else(|| line.to_vec(), |v| v.as_bytes().to_vec()));
        n += 1;
    }
    // too few values for B2S Server: the rest added after the last, through the LEDs' line
    let added = n < 12;
    while n < 12 {
        out.push(wanted.get(n).map_or(b"0".to_vec(), |v| v.as_bytes().to_vec()));
        n += 1;
    }
    let mut text = bom.to_vec();
    text.extend(out.join(eol));
    if ended || added {
        text.extend(eol);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(dmd: Option<Rect>) -> Place {
        Place { playfield: (2560, 1440), screen: 2, backglass: Rect { x: 0, y: 0, width: 2560, height: 1600 }, dmd }
    }

    #[test]
    fn a_file_of_values_alone_gets_the_backglass_and_keeps_its_dmd() {
        // as B2S Server's screen tool writes one: twelve values, Windows' line endings
        let before = b"3840\r\n2160\r\n3440\r\n1440\r\n1\r\n0\r\n0\r\n512\r\n128\r\n100\r\n-200\r\n0\r\n";
        let after = with_place(Some(before), &place(None));
        assert_eq!(after, b"2560\r\n1440\r\n2560\r\n1600\r\n2\r\n0\r\n0\r\n512\r\n128\r\n100\r\n-200\r\n0\r\n");
    }

    #[test]
    fn the_dmd_window_is_written_where_its_place_is_known() {
        let before = b"1920\n1080\n1024\n768\n2\n0\n0\n0\n0\n0\n0\n1\n";
        let dmd = Rect { x: 300, y: 1650, width: 1280, height: 390 };
        let after = with_place(Some(before), &place(Some(dmd)));
        assert_eq!(values(&after), ["2560", "1440", "2560", "1600", "2", "0", "0", "1280", "390", "300", "1650", "1"]);
        // the file's own line endings kept
        assert!(!after.contains(&b'\r'));
    }

    #[test]
    fn comments_a_background_and_b2s_servers_own_lines_are_kept() {
        let before = "# Playfield Screen resolution width/height\r\n1920\r\n1080\r\n# Backglass width/height\r\n800\r\n600\r\n# Backglass Display Devicename\r\n1\r\n# Backglass x/y\r\n0\r\n0\r\n# DMD\r\n600\r\n130\r\n700\r\n300\r\n# Y-flip\r\n0\r\n# Background\r\n100\r\n0\r\n600\r\n600\r\nC:\\path\\Frame.png\r\n#B2SWindowPunch=off\r\n";
        let after = String::from_utf8(with_place(Some(before.as_bytes()), &place(None))).unwrap();
        assert_eq!(
            after,
            "# Playfield Screen resolution width/height\r\n2560\r\n1440\r\n# Backglass width/height\r\n2560\r\n1600\r\n# Backglass Display Devicename\r\n2\r\n# Backglass x/y\r\n0\r\n0\r\n# DMD\r\n600\r\n130\r\n700\r\n300\r\n# Y-flip\r\n0\r\n# Background\r\n100\r\n0\r\n600\r\n600\r\nC:\\path\\Frame.png\r\n#B2SWindowPunch=off\r\n"
        );
    }

    #[test]
    fn a_new_file_has_b2s_servers_twelve_values() {
        let after = with_place(None, &place(None));
        assert_eq!(after, b"2560\r\n1440\r\n2560\r\n1600\r\n2\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n");
        assert_eq!(with_place(Some(b""), &place(None)), after);
    }

    #[test]
    fn a_short_file_is_filled_out() {
        let after = with_place(Some(b"1920\r\n1080\r\n1024\r\n768\r\n1\r\n10"), &place(None));
        assert_eq!(after, b"2560\r\n1440\r\n2560\r\n1600\r\n2\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n0\r\n");
    }

    #[test]
    fn values_are_read_as_b2s_server_reads_them() {
        // a line starting with `#` passed over, a blank one and one starting with a space not
        let text = b"\xEF\xBB\xBF1920\r\n# note\r\n\r\n #not a note\r\n5";
        assert_eq!(values(text), ["1920", "", " #not a note", "5"]);
        // a byte order mark kept where it was
        assert!(with_place(Some(text), &place(None)).starts_with(b"\xEF\xBB\xBF2560\r\n# note\r\n"));
    }
}
