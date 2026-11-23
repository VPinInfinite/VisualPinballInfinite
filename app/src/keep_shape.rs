//! A window's shape kept while the user sizes it by an edge or a corner: Windows asks a window,
//! as it's dragged bigger or smaller, whether the size it's about to take suits it (`WM_SIZING`),
//! and here the side not dragged follows the one that is. For the Screens page's stand-ins, which
//! keep the shape of the player's window they stand for.

use std::ffi::c_void;

#[repr(C)]
#[derive(Default, Debug, PartialEq)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

type SubclassProc = unsafe extern "system" fn(*mut c_void, u32, usize, isize, usize, usize) -> isize;

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(hwnd: *mut c_void, subclass: SubclassProc, id: usize, data: usize) -> i32;
    fn RemoveWindowSubclass(hwnd: *mut c_void, subclass: SubclassProc, id: usize) -> i32;
    fn DefSubclassProc(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetWindowRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
    fn GetClientRect(hwnd: *mut c_void, rect: *mut Rect) -> i32;
}

const WM_NCDESTROY: u32 = 0x0082;
const WM_SIZING: u32 = 0x0214;
// the edge or corner being dragged
const WMSZ_LEFT: usize = 1;
const WMSZ_RIGHT: usize = 2;
const WMSZ_TOP: usize = 3;
const WMSZ_TOPLEFT: usize = 4;
const WMSZ_TOPRIGHT: usize = 5;
const WMSZ_BOTTOM: usize = 6;
const WMSZ_BOTTOMLEFT: usize = 7;

/// Ours among a window's subclasses: "VPIS".
const ID: usize = 0x5650_4953;

/// The window's inside kept `shape` wide for every unit high while the user sizes it.
pub fn keep(window: &tauri::WebviewWindow, shape: f64) {
    if !(shape.is_finite() && shape > 0.0) {
        return;
    }
    let Ok(hwnd) = window.hwnd() else { return };
    // the shape rides along as the subclass's own number, an f64's bits
    unsafe { SetWindowSubclass(hwnd.0, sizing, ID, shape.to_bits() as usize) };
}

unsafe extern "system" fn sizing(hwnd: *mut c_void, msg: u32, wparam: usize, lparam: isize, _: usize, data: usize) -> isize {
    match msg {
        WM_SIZING => {
            let shape = f64::from_bits(data as u64);
            // the window as it is, and its frame's edges, which aren't part of the shape
            let (mut outer, mut inner) = (Rect::default(), Rect::default());
            unsafe {
                GetWindowRect(hwnd, &mut outer);
                GetClientRect(hwnd, &mut inner);
            }
            let frame = ((outer.right - outer.left) - inner.right, (outer.bottom - outer.top) - inner.bottom);
            // the size it's about to take, made its shape
            kept(unsafe { &mut *(lparam as *mut Rect) }, wparam, shape, frame, (inner.right, inner.bottom));
            1
        }
        WM_NCDESTROY => unsafe {
            RemoveWindowSubclass(hwnd, sizing, ID);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        },
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

/// The place `r` a window is about to take as its `edge` is dragged, made `shape`: the side
/// dragged leads — at a corner, the side pulled further — and the other follows. `frame` is how
/// much the window's frame adds to its inside each way, `was` its inside's size before.
fn kept(r: &mut Rect, edge: usize, shape: f64, frame: (i32, i32), was: (i32, i32)) {
    let (w, h) = (f64::from(r.right - r.left - frame.0), f64::from(r.bottom - r.top - frame.1));
    let (was_w, was_h) = (f64::from(was.0.max(1)), f64::from(was.1.max(1)));
    let by_width = match edge {
        WMSZ_LEFT | WMSZ_RIGHT => true,
        WMSZ_TOP | WMSZ_BOTTOM => false,
        _ => (w / was_w - 1.0).abs() >= (h / was_h - 1.0).abs(),
    };
    if by_width {
        let h = (w / shape).round() as i32 + frame.1;
        match edge {
            WMSZ_TOPLEFT | WMSZ_TOPRIGHT => r.top = r.bottom - h,
            _ => r.bottom = r.top + h,
        }
    } else {
        let w = (h * shape).round() as i32 + frame.0;
        match edge {
            WMSZ_TOPLEFT | WMSZ_BOTTOMLEFT | WMSZ_LEFT => r.left = r.right - w,
            _ => r.right = r.left + w,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WMSZ_BOTTOMRIGHT: usize = 8;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect { left, top, right, bottom }
    }

    #[test]
    fn the_side_not_dragged_follows() {
        // a 4:3 window, 800 by 600 inside, its frame adding 2 each way
        let (shape, frame, was) = (4.0 / 3.0, (2, 2), (800, 600));
        // its right edge dragged 200 further: its bottom follows
        let mut r = rect(100, 100, 1102, 702);
        kept(&mut r, WMSZ_RIGHT, shape, frame, was);
        assert_eq!(r, rect(100, 100, 1102, 852));
        // its top edge dragged up 150: its right follows
        let mut r = rect(100, -50, 902, 702);
        kept(&mut r, WMSZ_TOP, shape, frame, was);
        assert_eq!(r, rect(100, -50, 1102, 702));
        // its left edge dragged in: its bottom follows, its top stays
        let mut r = rect(500, 100, 902, 702);
        kept(&mut r, WMSZ_LEFT, shape, frame, was);
        assert_eq!(r, rect(500, 100, 902, 402));
    }

    #[test]
    fn at_a_corner_the_side_pulled_further_leads() {
        let (shape, frame, was) = (16.0 / 9.0, (0, 0), (1600, 900));
        // pulled out 320 across, 18 down: the width leads, the bottom follows
        let mut r = rect(0, 0, 1920, 918);
        kept(&mut r, WMSZ_BOTTOMRIGHT, shape, frame, was);
        assert_eq!(r, rect(0, 0, 1920, 1080));
        // the top-left corner pulled up 180 and left 16: the height leads, the left follows
        let mut r = rect(-16, -180, 1600, 900);
        kept(&mut r, WMSZ_TOPLEFT, shape, frame, was);
        assert_eq!(r, rect(-320, -180, 1600, 900));
        // the bottom-left corner pushed in across: the width leads, the bottom follows
        let mut r = rect(800, 0, 1600, 880);
        kept(&mut r, WMSZ_BOTTOMLEFT, shape, frame, was);
        assert_eq!(r, rect(800, 0, 1600, 450));
        // the top-right corner pushed down: the height leads, the right follows
        let mut r = rect(0, 450, 1590, 900);
        kept(&mut r, WMSZ_TOPRIGHT, shape, frame, was);
        assert_eq!(r, rect(0, 450, 800, 900));
    }
}
