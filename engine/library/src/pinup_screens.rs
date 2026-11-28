//! Where PinUP Player shows a pack's screens: `PinUpPlayer.ini` in PinUP's folder, `[INFO]` for
//! its screen 0 (the topper), `[INFO1]` for 1 (the DMD), `[INFO2]` for 2 (the backglass) and on,
//! each with `ScreenXPos`, `ScreenYPos`, `ScreenWidth` and `ScreenHeight` in the desktop's pixels.
//! The classic PinUP Player reads them — the one a table's script starts where VPX's own PinUP
//! add-on doesn't play the pack (VPX 10.8.0, or a newer VPX with it off) and the one Future
//! Pinball's PinEvent tables start — and PinUP's setup writes them for the screens as they were
//! then: a screen Windows has since moved elsewhere on the desktop leaves PinUP's windows on it
//! where no screen is, out of sight. Ours.

/// One of PinUP's screens as its settings place it.
#[derive(Debug, Clone, PartialEq)]
struct Placed {
    /// PinUP's number for it.
    number: u32,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    /// The lines holding its `ScreenXPos` and `ScreenYPos`, to change in place.
    x_line: usize,
    y_line: usize,
}

impl Placed {
    fn centre(&self) -> (i64, i64) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// A screen of Windows' on the desktop: its top-left corner and size, in pixels.
pub type Monitor = (i32, i32, u32, u32);

fn inside((x, y): (i64, i64), (mx, my, mw, mh): (i64, i64, i64, i64)) -> bool {
    x >= mx && y >= my && x < mx + mw && y < my + mh
}

fn monitor_rect(m: &Monitor) -> (i64, i64, i64, i64) {
    (m.0 as i64, m.1 as i64, m.2 as i64, m.3 as i64)
}

/// PinUP's screens as `PinUpPlayer.ini`'s text places them: those with all four numbers and a
/// size.
fn placed(lines: &[&str]) -> Vec<Placed> {
    let mut out = Vec::new();
    let mut section: Option<u32> = None;
    let mut found: (Option<(i64, usize)>, Option<(i64, usize)>, Option<i64>, Option<i64>) = (None, None, None, None);
    let finish = |section: Option<u32>, found: &mut (Option<(i64, usize)>, Option<(i64, usize)>, Option<i64>, Option<i64>), out: &mut Vec<Placed>| {
        if let (Some(number), (Some((x, x_line)), Some((y, y_line)), Some(width), Some(height))) = (section, *found)
            && width > 0
            && height > 0
        {
            out.push(Placed { number, x, y, width, height, x_line, y_line });
        }
        *found = (None, None, None, None);
    };
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if let Some(name) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            finish(section, &mut found, &mut out);
            let name = name.trim().to_ascii_uppercase();
            section = name.strip_prefix("INFO").and_then(|n| if n.is_empty() { Some(0) } else { n.parse().ok() });
            continue;
        }
        let Some((key, value)) = t.split_once('=') else { continue };
        let Ok(value) = value.trim().parse::<i64>() else { continue };
        match key.trim().to_ascii_lowercase().as_str() {
            "screenxpos" => found.0 = Some((value, i)),
            "screenypos" => found.1 = Some((value, i)),
            "screenwidth" => found.2 = Some(value),
            "screenheight" => found.3 = Some(value),
            _ => {}
        }
    }
    finish(section, &mut found, &mut out);
    out
}

/// Where PinUP shows its screen `number` as `PinUpPlayer.ini`'s text places it: its top-left corner
/// and size, in desktop pixels; none where it's placed nowhere, or with no size.
pub fn screen_place(text: &str, number: u32) -> Option<(i32, i32, u32, u32)> {
    let lines: Vec<&str> = text.lines().collect();
    placed(&lines).into_iter().find(|p| p.number == number).map(|p| (p.x as i32, p.y as i32, p.width as u32, p.height as u32))
}

/// `PinUpPlayer.ini`'s text with PinUP's screens that show on none of Windows' screens now
/// (`monitors`) moved with the screen they were on: a screen of PinUP's sized as one of Windows'
/// that shows on none tells where that one was (PinUP's setup sizes the backglass's to its
/// screen), and one of Windows' that size showing none of PinUP's screens is where it is now;
/// every one of PinUP's screens that lay on it moves by as much, keeping its place on it. None
/// where nothing moves; else the text, only those lines changed, and what moved, for the log.
pub fn follow_monitors(text: &str, monitors: &[Monitor]) -> Option<(String, Vec<String>)> {
    let lines: Vec<&str> = text.split('\n').collect();
    let screens = placed(&lines);
    let shown = |s: &Placed| monitors.iter().any(|m| inside(s.centre(), monitor_rect(m)));
    let lost: Vec<&Placed> = screens.iter().filter(|s| !shown(s)).collect();
    if lost.is_empty() {
        return None;
    }
    // where each of Windows' screens was, told by a lost screen of PinUP's its size, the largest first
    let mut olds: Vec<(i64, i64, i64, i64)> = Vec::new();
    for s in &lost {
        let rect = (s.x, s.y, s.width, s.height);
        if monitors.iter().any(|m| (m.2 as i64, m.3 as i64) == (s.width, s.height)) && !olds.contains(&rect) {
            olds.push(rect);
        }
    }
    olds.sort_by_key(|r| std::cmp::Reverse(r.2 * r.3));
    let mut moved: Vec<(u32, i64, i64, usize, usize)> = Vec::new();
    let mut taken: Vec<usize> = Vec::new();
    for old in olds {
        // the screen of Windows' its size now holding none of PinUP's shown screens
        let holds_shown = |m: &Monitor| screens.iter().filter(|s| shown(s)).any(|s| inside(s.centre(), monitor_rect(m)));
        let Some(index) = monitors.iter().enumerate().position(|(i, m)| !taken.contains(&i) && (m.2 as i64, m.3 as i64) == (old.2, old.3) && !holds_shown(m)) else { continue };
        taken.push(index);
        let m = &monitors[index];
        let (dx, dy) = (m.0 as i64 - old.0, m.1 as i64 - old.1);
        for s in &lost {
            if inside(s.centre(), old) && !moved.iter().any(|(n, ..)| *n == s.number) {
                moved.push((s.number, s.x + dx, s.y + dy, s.x_line, s.y_line));
            }
        }
    }
    if moved.is_empty() {
        return None;
    }
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    let set = |line: &str, value: i64| -> String {
        let (key, rest) = line.split_once('=').unwrap_or((line, ""));
        // the line's own end kept: a carriage return before its newline
        let cr = if rest.ends_with('\r') { "\r" } else { "" };
        format!("{key}={value}{cr}")
    };
    let mut said = Vec::new();
    for (number, x, y, x_line, y_line) in &moved {
        out[*x_line] = set(lines[*x_line], *x);
        out[*y_line] = set(lines[*y_line], *y);
        said.push(format!("screen {number} to {x}, {y}"));
    }
    Some((out.join("\n"), said))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As PinUP's setup wrote them on a PC whose second screen, 3440 × 1440, was right of a 4K one,
    /// since moved left of it.
    const SET_UP: &str = "[INFO]\r\nScreenXPos=3943\r\nScreenYPos=80\r\nScreenWidth=500\r\nScreenHeight=125\r\nScreenRotation=0\r\n\r\n[INFO1]\r\nScreenXPos=4400\r\nScreenYPos=487\r\nScreenWidth=512\r\nScreenHeight=128\r\n\r\n[INFO2]\r\nScreenXPos=3840\r\nScreenYPos=0\r\nScreenWidth=3440\r\nScreenHeight=1440\r\n\r\n[INFO3]\r\nScreenXPos=0\r\nScreenYPos=0\r\nScreenWidth=3840\r\nScreenHeight=2160\r\n\r\n[INFO5]\r\nScreenXPos=3840\r\nScreenYPos=0\r\nScreenWidth=3440\r\nScreenHeight=1440\r\n\r\n[INFO6]\r\nScreenXPos=60\r\nScreenYPos=67\r\nScreenWidth=800\r\nScreenHeight=600\r\n";

    const NOW: [Monitor; 2] = [(0, 0, 3840, 2160), (-3440, 0, 3440, 1440)];

    #[test]
    fn screens_moved_with_the_screen_they_were_on() {
        let (text, said) = follow_monitors(SET_UP, &NOW).expect("moved");
        let lines: Vec<&str> = text.split('\n').collect();
        let place = |n: u32| placed(&lines).into_iter().find(|s| s.number == n).map(|s| (s.x, s.y, s.width, s.height));
        assert_eq!(place(2), Some((-3440, 0, 3440, 1440)));
        assert_eq!(place(5), Some((-3440, 0, 3440, 1440)));
        // the topper and the DMD keep their places on it
        assert_eq!(place(0), Some((-3337, 80, 500, 125)));
        assert_eq!(place(1), Some((-2880, 487, 512, 128)));
        // those on a screen there now left as they were
        assert_eq!(place(3), Some((0, 0, 3840, 2160)));
        assert_eq!(place(6), Some((60, 67, 800, 600)));
        assert_eq!(said.len(), 4);
        // only those lines changed, their own ends kept
        assert!(text.contains("ScreenXPos=-3440\r\n") && text.contains("ScreenRotation=0\r\n"));
        assert_eq!(text.lines().count(), SET_UP.lines().count());
    }

    #[test]
    fn nothing_where_every_screen_shows() {
        assert!(follow_monitors(SET_UP, &[(0, 0, 3840, 2160), (3840, 0, 3440, 1440)]).is_none());
    }

    #[test]
    fn nothing_where_the_lost_screen_is_told_by_none() {
        // no screen of Windows' sized as a lost one of PinUP's: where it was can't be told
        assert!(follow_monitors(SET_UP, &[(0, 0, 3840, 2160), (-1920, 0, 1920, 1080)]).is_none());
        // the screen its size holds PinUP's playfield: not the one that moved
        assert!(follow_monitors("[INFO2]\nScreenXPos=5000\nScreenYPos=0\nScreenWidth=1920\nScreenHeight=1080\n[INFO3]\nScreenXPos=0\nScreenYPos=0\nScreenWidth=1920\nScreenHeight=1080\n", &[(0, 0, 1920, 1080)]).is_none());
    }
}
