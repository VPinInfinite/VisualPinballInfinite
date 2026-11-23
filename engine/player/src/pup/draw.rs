// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): what its PinUp Player
// plugin draws in each window, and in which order (plugins/pup/PUPManager.cpp: Render, its usable root;
// PUPScreen.cpp: Render, SetBounds; PUPMediaManager.cpp: Render, its fade; PUPLabel.cpp: Render), with the
// overlay's own opacity of the newer fixes (to commit a4a1e75ca: 781f88258).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! What a pack shows in the player's windows this frame, for the renderer: per window (topper,
//! backglass, score display), the screen a window shows — the backglass screen 2, else 6; the
//! score display's 5, else 1; the topper's 0 — and every screen placed in it, layer by layer
//! in VPX's order: the screens behind before those on top, a pop-up's layers together, and on
//! each screen its background picture, its video (or its still picture when nothing plays),
//! its overlay, then the labels of its page. Everything is placed in the window's pixels.

use std::path::PathBuf;
use std::sync::Arc;

use super::Pup;
use super::label::LabelWorker;
use super::playback::Picture;
use super::screen::Mode;

/// What a pack shows in the player's windows: for the topper, the backglass and the score
/// display, the layers to draw, back to front; None where the pack shows nothing there.
#[derive(Default)]
pub struct PupNow {
    pub windows: [Option<WindowLayers>; 3],
}

impl std::fmt::Debug for PupNow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let counts: Vec<Option<usize>> = self.windows.iter().map(|w| w.as_ref().map(|w| w.layers.len())).collect();
        f.debug_struct("PupNow").field("layers", &counts).finish()
    }
}

/// One window's layers, placed for a window of `size` pixels.
pub struct WindowLayers {
    pub size: (u32, u32),
    pub layers: Vec<Layer>,
}

/// One picture on a window.
pub struct Layer {
    /// Where it goes: left, top, width, height, in the window's pixels.
    pub rect: [f32; 4],
    /// What it's cut to: its screen's place (a label's), if anything.
    pub clip: Option<[i32; 4]>,
    pub alpha: f32,
    pub content: Content,
}

pub enum Content {
    /// A picture file — a background, an overlay, a still picture — which the renderer loads.
    File(PathBuf),
    /// A picture made already: a video's (or animated GIF's) picture now, a label's.
    Frame(Arc<Picture>),
}

impl Pup {
    /// The windows' layers this frame (VPX's `PUPManager::Render` for each window). A video's
    /// fade-in and the labels' animations step on as they're drawn, as in VPX. In `exact` mode
    /// (test mode), the labels' pictures are waited for.
    pub fn now(&mut self, now_ms: u64, exact: bool) -> PupNow {
        let mut out = PupNow::default();
        for w in 0..3 {
            let Some((width, height)) = self.windows[w] else { continue };
            let candidates = Self::WINDOW_ROOTS[w];
            let root = candidates.iter().filter_map(|n| self.by_num.get(n).copied()).find(|&r| self.usable_root(r));
            if let Some(root) = root {
                let layers = self.layers(root, w, width as i32, height as i32, now_ms, exact);
                out.windows[w] = Some(WindowLayers { size: (width, height), layers });
            }
        }
        out
    }

    /// Whether a screen can be a window's root (VPX's `isUsableRoot`): a canvas rather than a
    /// placed element (a screen placed on another and with none placed on it), with something
    /// under it that can show.
    fn usable_root(&self, root: usize) -> bool {
        let r = &self.screens[root];
        if r.parent.is_some() && r.children.is_empty() {
            return false;
        }
        self.order.iter().any(|&k| self.screens[k].mode.visible() && self.under(k, root))
    }

    /// Whether `k` is `root` or placed under it.
    fn under(&self, k: usize, root: usize) -> bool {
        let mut at = Some(k);
        let mut steps = 0;
        while let Some(i) = at {
            if i == root {
                return true;
            }
            at = self.screens[i].parent;
            steps += 1;
            if steps > self.screens.len() {
                return false;
            }
        }
        false
    }

    /// The layers under `root` in a window of `w` × `h` pixels, in VPX's order: two tiers, as
    /// Windows' topmost windows — in the back one a pop-up's layers before the others'
    /// overlays, so a frame over the whole window stays over the background videos; in the
    /// front one pop-ups last, so call-outs stay on top.
    fn layers(&mut self, root: usize, window: usize, w: i32, h: i32, now: u64, exact: bool) -> Vec<Layer> {
        let screens: Vec<usize> = self.order.iter().copied().filter(|&k| self.under(k, root)).collect();
        // each screen's place in the window inside the window's padding (VPX's SetBounds on the root
        // screen, which hands its children the same bounds)
        let [left, right, top, bottom] = self.pads[window];
        for &k in &screens {
            self.screens[k].set_bounds(left, top, w - left - right, h - top - bottom);
        }
        if self.labels.is_none() && screens.iter().any(|&k| !self.screens[k].labels.is_empty()) {
            self.labels = LabelWorker::start();
        }
        let mut out = Vec::new();
        let tiers = [(false, false, 0, 1), (true, false, 0, 3), (false, false, 2, 3), (false, true, 0, 1), (false, true, 2, 3), (true, true, 0, 3)];
        for (popup, topmost, first, last) in tiers {
            for pass in first..=last {
                for &k in &screens {
                    let s = &self.screens[k];
                    if s.is_pop() == popup && s.topmost == topmost && s.mode != Mode::Off {
                        self.screen_layer(k, pass, now, exact, &mut out);
                    }
                }
            }
        }
        out
    }

    /// One pass of a screen's drawing (VPX's `PUPScreen::Render`): 0 its background, 1 its video
    /// or else its still picture, 3 its labels while they're shown (an overlay being its background
    /// label's picture), at the overlay's opacity too, the animated ones last, over the rest.
    fn screen_layer(&mut self, k: usize, pass: u32, now: u64, exact: bool, out: &mut Vec<Layer>) {
        let worker = self.labels.as_ref();
        let s = &mut self.screens[k];
        if matches!(s.mode, Mode::Off | Mode::MusicOnly) {
            return;
        }
        let r = s.rect;
        let rect = [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32];
        let alpha = s.screen_alpha;
        let file = |path: &Option<PathBuf>, alpha: f32| path.clone().map(|p| Layer { rect, clip: None, alpha, content: Content::File(p) });
        match pass {
            0 => out.extend(file(&s.background, alpha)),
            1 => {
                let m = &mut s.media;
                if m.player.is_playing() {
                    // its fade-in, a step each time it's drawn (VPX's PUPMediaManager::Render)
                    if m.current_alpha < 255 {
                        m.current_alpha = (m.current_alpha + m.fade_step).min(255);
                    }
                    let a = alpha * m.current_alpha as f32 / 255.0;
                    if let Some(picture) = m.player.picture() {
                        out.push(Layer { rect, clip: None, alpha: a, content: Content::Frame(picture) });
                    }
                }
                if !s.media.is_main_playing() && !s.media.is_background_playing() {
                    out.extend(file(&s.static_image, alpha));
                }
            }
            3 if s.hud_visible => {
                let Some(worker) = worker else { return };
                let page = s.page;
                for animating in [false, true] {
                    for label in s.labels.iter_mut().filter(|l| l.is_animating() == animating) {
                        let (draw, fade) = label.render(r, page, s.screen_alpha * s.overlay_alpha, now, worker, exact);
                        if let Some(a) = fade {
                            s.overlay_alpha = a;
                        }
                        if let Some(d) = draw {
                            out.push(Layer { rect: d.dest, clip: Some(d.clip), alpha: d.alpha, content: Content::Frame(d.picture) });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
