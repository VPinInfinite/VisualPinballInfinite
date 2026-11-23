// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): how its B2S plugin draws a
// backglass and its score panel (plugins/b2s/B2SRenderer.cpp: RenderBackglass, RenderScoreView, RenderBulbs and
// RenderScores) with the DMD over them (plugins/b2s/B2SDMDOverlay.cpp: Render), and how VPX draws a plugin's pictures
// and displays in a window of its own (Renderer::DrawImage and DrawMatrixDisplay, src/renderer/Renderer.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! The backglass window and the score panel: a table's
//! backglass file drawn as VPX 5436 draws it. The backglass picture fills
//! the window (stretched, as B2S does), its speaker grill cut off; over it
//! the lit picture, as bright as what lights it; then each light — its
//! unlit picture under it while it isn't fully lit, its lit picture tinted
//! and as opaque as its brightness; then the score reels, and the segment
//! scores, lit as the machine's segment displays are. Pictures blend as VPX
//! blends them here, on their sRGB values, as B2S always has.
//!
//! The score panel — the file's picture for a third screen, with lights and
//! scores of its own — draws the same way into the score window, stretched
//! to it (VPX's score view); there the pictures blend on their linear
//! values, as VPX blends them in a window whose output is linear.
//!
//! Over either page, last, the DMD can show (VPX's DMD overlay,
//! `dmd_overlay.rs`): in the score panel's dark window, found as VPX finds
//! it on a thread of its own, unless set otherwise; over the backglass only
//! when asked, as in VPX.

use std::sync::Arc;

use super::*;
use crate::b2s::{B2sFile, BackglassNow, Page, Picture, ScoreKind, Snippit};
use crate::dmd::DmdFrame;
use crate::dmd_overlay::Overlay;
use segments::DigitDraw;

/// What a page showed: the backglass, the window's size, and the DMD's frame
/// and place over it.
type Shown = (Arc<BackglassNow>, u32, u32, Option<(u64, [f32; 4])>);

/// One window's drawing of a page of the backglass file: the page's
/// pictures on the graphics card, its pipeline, room for its pictures'
/// corners, and what it showed last.
pub(super) struct PageDrawing {
    page: Page,
    format: SDL_GPUTextureFormat,
    pipeline: *mut SDL_GPUGraphicsPipeline,
    /// The file whose pictures are uploaded, and their textures, in
    /// [`pictures`]'s order (null where the file has none).
    file: Option<Arc<B2sFile>>,
    textures: Vec<*mut SDL_GPUTexture>,
    vertices: *mut SDL_GPUBuffer,
    transfer: *mut SDL_GPUTransferBuffer,
    /// Room in the vertex buffer, in vertices.
    capacity: usize,
    /// What was shown last: it's drawn again only when something changes.
    shown: Option<Shown>,
    /// The DMD's pipeline for this window, made when the DMD first shows over the page.
    dmd_pipeline: *mut SDL_GPUGraphicsPipeline,
    /// The search for the DMD's window in the page's picture.
    search: Option<Search>,
}

impl PageDrawing {
    fn new(page: Page, format: SDL_GPUTextureFormat, pipeline: *mut SDL_GPUGraphicsPipeline) -> PageDrawing {
        PageDrawing {
            page,
            format,
            pipeline,
            file: None,
            textures: Vec::new(),
            vertices: null_mut(),
            transfer: null_mut(),
            capacity: 0,
            shown: None,
            dmd_pipeline: null_mut(),
            search: None,
        }
    }

    /// Whether `now`, at `size`, with the DMD's frame and place `dmd`, shows what was shown last.
    fn showing(&self, now: &BackglassNow, size: (u32, u32), dmd: Option<(u64, [f32; 4])>) -> bool {
        self.shown.as_ref().is_some_and(|(s, w, h, m)| (*w, *h) == size && *m == dmd && s.same_page(now, self.page))
    }

    /// Forgets what was shown last, so the next frame draws it again.
    pub(super) fn forget(&mut self) {
        self.shown = None;
    }
}

/// The search for the DMD's window in a page's picture, for a DMD of one
/// shape, run on a thread of its own as VPX runs it.
struct Search {
    file: Arc<B2sFile>,
    dots: (u32, u32),
    running: Option<std::thread::JoinHandle<Option<[i32; 4]>>>,
    found: Option<[i32; 4]>,
}

/// The DMD over a page: its frame, where (left, top, width and height, in
/// the page's pixels) and how it looks.
struct PageDmd {
    id: u64,
    place: [f32; 4],
    look: crate::scoreview::Look,
    luminance: bool,
}

/// The picture VPX searches for a page's DMD window: the score panel's; the
/// backglass, else its unlit picture, else its lit one.
fn search_picture(f: &B2sFile, page: Page) -> Option<&Picture> {
    match page {
        Page::Panel => f.dmd_image.as_ref(),
        Page::Backglass => f.backglass.as_ref().or(f.backglass_off.as_ref()).or(f.backglass_on.as_ref()),
    }
}

/// The DMD's window in a page's picture, for a frame of `f`'s shape. It's
/// searched for on a thread of its own the first time it's asked for, and
/// again when the DMD's shape changes (as VPX does when its display source
/// changes); None until it's found, or where there's none. `wait`: wait for
/// the search instead.
fn found(d: &mut PageDrawing, now: &BackglassNow, f: &DmdFrame, wait: bool) -> Option<[i32; 4]> {
    let dots = (f.width, f.height);
    if d.search.as_ref().is_none_or(|s| !Arc::ptr_eq(&s.file, &now.file) || s.dots != dots) {
        let (file, page, aspect) = (now.file.clone(), d.page, f.width as f32 / f.height as f32);
        let running = std::thread::Builder::new()
            .name("dmd window".into())
            .spawn(move || {
                search_picture(&file, page).and_then(|p| {
                    // over the backglass, first the window in the middle of its speaker panel (ours), then
                    // the darkest window anywhere, as VPX searches
                    let grill = if page == Page::Backglass { file.grill_height.max(0) as u32 } else { 0 };
                    crate::dmd_overlay::find_in_grill(&p.rgba, p.width, p.height, grill, aspect)
                        .or_else(|| crate::dmd_overlay::find(&p.rgba, p.width, p.height, aspect))
                })
            })
            .ok();
        d.search = Some(Search { file: now.file.clone(), dots, running, found: None });
    }
    let s = d.search.as_mut()?;
    if let Some(r) = s.running.take_if(|r| wait || r.is_finished()) {
        s.found = r.join().ok().flatten();
    }
    s.found
}

/// The backglass's own window, or its place inside the table's.
pub(super) struct BackglassWindow {
    pub(super) surface: super::Surface,
    drawing: Option<PageDrawing>,
    /// A PinUP pack's screens, once shown here.
    pup: Option<super::pup::PupDrawing>,
    /// The announcements come and gone when it last drew (`Renderer::toast_count`).
    toast_seen: u64,
}

/// Every picture of a page, in a fixed order: its background (the
/// backglass, lit and unlit; the score panel's picture), then each light's
/// lit and unlit pictures, then the reels (for the score panel, when it has
/// scores drawn from them).
fn pictures(f: &B2sFile, page: Page) -> Vec<Option<&Picture>> {
    let mut out = match page {
        Page::Backglass => vec![f.backglass.as_ref(), f.backglass_on.as_ref(), f.backglass_off.as_ref()],
        Page::Panel => vec![f.dmd_image.as_ref(), None, None],
    };
    for b in f.bulbs_of(page) {
        out.push(b.image.as_ref());
        out.push(b.off_image.as_ref());
    }
    if page == Page::Backglass || f.dmd_scores.iter().any(|s| s.kind != ScoreKind::Dream7) {
        out.extend(f.reels.iter().map(|r| r.image.as_ref()));
    }
    out
}

const BACKGROUND: usize = 0;
const BACKGLASS_ON: usize = 1;
const BACKGLASS_OFF: usize = 2;
const fn bulb_image(i: usize) -> usize {
    3 + 2 * i
}
const fn bulb_off(i: usize) -> usize {
    4 + 2 * i
}

/// A page's size in its own pixels: the backglass picture's, its grill cut
/// off; the score panel's picture's.
fn source_size(now: &BackglassNow, page: Page) -> (f32, f32) {
    match page {
        Page::Backglass => (now.base().map_or(1024.0, |p| p.width as f32), now.shown_height.max(1.0)),
        Page::Panel => {
            let (w, h) = now.file.panel_size();
            (w, h.max(1.0))
        }
    }
}

/// One picture to draw: which, where in the window (each corner, from 0 to 1
/// across and down), which part of it, and its tint and opacity.
struct Quad {
    picture: usize,
    corners: [[f32; 2]; 4],
    uv: [f32; 4],
    color: [f32; 4],
}

/// The pictures to draw for a page as it is now, in order.
fn quads(now: &BackglassNow, page: Page) -> Vec<Quad> {
    let f = &*now.file;
    let state = &now.pages[page.index()];
    let (src_w, src_h) = source_size(now, page);
    let mut out = Vec::new();
    // VPX's DrawImage: a rectangle on the page, in its pixels, turned by degrees about its centre
    let mut draw = |picture: usize, rect: [f32; 4], uv: [f32; 4], color: [f32; 4], turn: f32| {
        if color[3] <= 0.0 {
            return;
        }
        let [x, y, w, h] = rect;
        let (x0, y0, x1, y1) = (x / src_w, y / src_h, (x + w) / src_w, (y + h) / src_h);
        let mut corners = [[x0, y0], [x1, y0], [x0, y1], [x1, y1]];
        if turn != 0.0 {
            let (cx, cy) = (0.5 * (x0 + x1), 0.5 * (y0 + y1));
            let (s, c) = turn.to_radians().sin_cos();
            for p in &mut corners {
                let (dx, dy) = (p[0] - cx, p[1] - cy);
                *p = [cx + dx * c - dy * s, cy + dx * s + dy * c];
            }
        }
        out.push(Quad { picture, corners, uv, color });
    };
    match page {
        Page::Backglass => {
            // the backglass, its grill cut off, under its lit picture unless that is fully lit
            let on = f.backglass_on.as_ref();
            if on.is_none() || now.on < 1.0 {
                let which = if f.backglass.is_some() { BACKGROUND } else { BACKGLASS_OFF };
                if let Some(p) = now.base() {
                    draw(which, [0.0, 0.0, p.width as f32, src_h], [0.0, 0.0, 1.0, src_h / p.height as f32], [1.0; 4], 0.0);
                }
            }
            if let Some(p) = on {
                draw(BACKGLASS_ON, [0.0, 0.0, p.width as f32, src_h], [0.0, 0.0, 1.0, src_h / p.height as f32], [1.0, 1.0, 1.0, now.on], 0.0);
            }
        }
        Page::Panel => {
            if f.dmd_image.is_some() {
                draw(BACKGROUND, [0.0, 0.0, src_w, src_h], [0.0, 0.0, 1.0, 1.0], [1.0; 4], 0.0);
            }
        }
    }
    // the lights
    let bulbs = f.bulbs_of(page);
    for (i, b) in bulbs.iter().enumerate() {
        let brightness = state.bulbs[i];
        let turn = if b.snippit == Snippit::MechRotating { state.turns[i] } else { 0.0 };
        let rect = [b.x as f32, b.y as f32, b.width as f32, b.height as f32];
        let [r, g, bl] = b.light_color;
        if b.off_image.is_some() && brightness < 1.0 {
            draw(bulb_off(i), rect, [0.0, 0.0, 1.0, 1.0], [r, g, bl, 1.0], turn);
        }
        if b.image.is_some() {
            draw(bulb_image(i), rect, [0.0, 0.0, 1.0, 1.0], [r, g, bl, brightness], turn);
        }
    }
    // the reels: each digit's picture, across the score's width
    let reel_base = 3 + 2 * bulbs.len();
    let scores = f.scores_of(page);
    for (n, digits) in &state.reels {
        let s = &scores[*n];
        // segment scores (Dream7) are drawn by the segment renderer, after the pictures
        if !matches!(s.kind, ScoreKind::Reel | ScoreKind::Led | ScoreKind::ImportedLed) || s.digits <= 0 {
            continue;
        }
        let width = (s.width as f32 - 0.5 * ((s.digits - 1) * s.spacing) as f32) / s.digits as f32;
        for (i, &digit) in digits.iter().enumerate() {
            let x = s.x as f32 + i as f32 * (width + 0.5 * s.spacing as f32);
            let Some(k) = f.reel_image(&s.reel_type, digit).and_then(|p| f.reels.iter().position(|r| r.image.as_ref().is_some_and(|q| std::ptr::eq(q, p)))) else { continue };
            draw(reel_base + k, [x, s.y as f32, width, s.height as f32], [0.0, 0.0, 1.0, 1.0], [1.0; 4], 0.0);
        }
    }
    out
}

/// A page's segment scores' digits, on a window of `size` pixels: each
/// digit's place across its score's width, as the reels' (VPX's
/// `RenderScores`).
fn digits(now: &BackglassNow, page: Page, size: (f32, f32)) -> Vec<DigitDraw> {
    let scores = now.file.scores_of(page);
    let (src_w, src_h) = source_size(now, page);
    let (sx, sy) = (size.0 / src_w, size.1 / src_h);
    let mut out = Vec::new();
    for (n, lit) in &now.pages[page.index()].dream7 {
        let s = &scores[*n];
        if s.digits <= 0 {
            continue;
        }
        let width = (s.width as f32 - 0.5 * ((s.digits - 1) * s.spacing) as f32) / s.digits as f32;
        for (i, &(kind, style, hint, levels)) in lit.iter().enumerate() {
            let x = s.x as f32 + i as f32 * (width + 0.5 * s.spacing as f32);
            out.push(DigitDraw {
                rect: [x * sx, s.y as f32 * sy, width * sx, s.height as f32 * sy],
                kind,
                hint,
                levels,
                look: crate::segments::look(style, [1.0; 3], 1.0),
                // VPX's backglass: no tint, a glass of roughness 0.15
                glass_tint: [1.0; 3],
                roughness: crate::segments::GLASS_ROUGHNESS,
                glass: None,
            });
        }
    }
    out
}

/// How a page's segment digits and DMD leave the shader, by the window's
/// format: VPX's Reinhard, then sRGB for a window that takes sRGB values (the
/// backglass window, the score window in HDR10); linear for one whose swap
/// chain encodes them (the score window's usual).
fn digit_mode(format: SDL_GPUTextureFormat) -> f32 {
    if super::embed::holds_srgb_values(format) { 2.0 } else { 1.0 }
}

impl Renderer {
    /// Gives the backglass a window of its own, drawn along with the
    /// playfield's frames whenever it changes; like the score display's, it
    /// never makes a frame wait.
    ///
    /// # Safety
    /// `window` must be a live SDL window, created on this thread, that
    /// outlives the renderer.
    pub unsafe fn add_backglass_window(&mut self, window: *mut SDL_Window) -> Result<(), String> {
        unsafe {
            check(SDL_ClaimWindowForGPUDevice(self.device, window), "claim the backglass window")?;
            let mode = [SDL_GPU_PRESENTMODE_MAILBOX, SDL_GPU_PRESENTMODE_IMMEDIATE]
                .into_iter()
                .find(|&m| SDL_WindowSupportsGPUPresentMode(self.device, window, m))
                .unwrap_or(SDL_GPU_PRESENTMODE_VSYNC);
            // sRGB values in and out, blended as they are (VPX: no sRGB decoding when the output is sRGB)
            if let Err(e) = check(SDL_SetGPUSwapchainParameters(self.device, window, SDL_GPU_SWAPCHAINCOMPOSITION_SDR, mode), "set the backglass window's present mode") {
                SDL_ReleaseWindowFromGPUDevice(self.device, window);
                return Err(e);
            }
            let format = SDL_GetGPUSwapchainTextureFormat(self.device, window);
            if let Err(e) = self.add_backglass_surface(super::Surface::Window(window), format) {
                SDL_ReleaseWindowFromGPUDevice(self.device, window);
                return Err(e);
            }
        }
        Ok(())
    }

    /// The backglass drawn on `surface`, pictures of `format`.
    pub(super) fn add_backglass_surface(&mut self, surface: super::Surface, format: SDL_GPUTextureFormat) -> Result<(), String> {
        let pipeline = self.page_pipeline(format)?;
        self.backglass = Some(BackglassWindow { surface, drawing: Some(PageDrawing::new(Page::Backglass, format, pipeline)), pup: None, toast_seen: 0 });
        Ok(())
    }

    /// A page's pipeline, for a window of `format`: pictures tinted and
    /// blended over what's below (VPX's `DrawImage`).
    pub(super) fn page_pipeline(&self, format: SDL_GPUTextureFormat) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(OVERLAY_HLSL, c"overlay_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 1)?;
        let fs = match self.shader(OVERLAY_HLSL, c"backglass_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 0) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut buffer: SDL_GPUVertexBufferDescription = zeroed();
            buffer.pitch = size_of::<OverlayVertex>() as u32;
            buffer.input_rate = SDL_GPU_VERTEXINPUTRATE_VERTEX;
            let mut attributes: [SDL_GPUVertexAttribute; 3] = zeroed();
            for (n, (format, offset)) in
                [(SDL_GPU_VERTEXELEMENTFORMAT_FLOAT2, 0), (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT2, 8), (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT4, 16)].into_iter().enumerate()
            {
                attributes[n].location = n as u32;
                attributes[n].format = format;
                attributes[n].offset = offset;
            }
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = format;
            color.blend_state.enable_blend = true;
            color.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_SRC_ALPHA;
            color.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE_MINUS_SRC_ALPHA;
            color.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
            color.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            color.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE_MINUS_SRC_ALPHA;
            color.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.vertex_input_state.vertex_buffer_descriptions = &buffer;
            info.vertex_input_state.num_vertex_buffers = 1;
            info.vertex_input_state.vertex_attributes = attributes.as_ptr();
            info.vertex_input_state.num_vertex_attributes = 3;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create the backglass pipeline")
        }
    }

    /// The backglass, when its score panel shows in the score window: when
    /// the file has one, unless set not to (VPX's B2S score view draws it
    /// whenever the file has one).
    pub(super) fn shows_panel<'a>(&self, backglass: Option<&'a Arc<BackglassNow>>) -> Option<&'a Arc<BackglassNow>> {
        backglass.filter(|b| self.score_panel.is_some() && b.file.has_panel())
    }

    /// The DMD over a page as VPX's backglass draws it
    /// (`B2SDMDOverlay::Render`), when it shows there, with its pipeline
    /// made ready: the default display's frame, uploaded, in the profile
    /// VPX gives it (neon plasma: PinMAME's DMDs and a script's alike, but
    /// for video screens, which our link doesn't carry), no glass picture,
    /// where the page's setting puts it. `wait`: wait for the window's
    /// search, instead of showing the page without the DMD until it's found
    /// (test mode's pictures).
    fn page_dmd(&self, d: &mut PageDrawing, now: &BackglassNow, frame: Option<&DmdFrame>, wait: bool) -> Option<PageDmd> {
        let overlay = match d.page {
            Page::Panel => self.score_panel.unwrap_or(Overlay::Off),
            Page::Backglass => self.backglass_dmd,
        };
        if overlay == Overlay::Off {
            return None;
        }
        let f = frame.filter(|f| !self.dmd_texture.is_null() && self.dmd_id == Some(f.id) && f.width > 0 && f.height > 0)?;
        let window = if overlay == Overlay::Auto { found(d, now, f, wait) } else { None };
        let place = overlay.place(window, source_size(now, d.page).1)?;
        if d.dmd_pipeline.is_null() {
            match self.display_pipeline(d.format) {
                Ok(p) => d.dmd_pipeline = p,
                Err(e) => {
                    crate::script::log::once(&format!("the DMD over the backglass: {e}"));
                    return None;
                }
            }
        }
        let profile = crate::scoreview::with_source_tint(&crate::scoreview::profiles()[crate::scoreview::NEON_PLASMA], f.tint);
        let look = crate::scoreview::look(&profile, f.luminance, (f.width, f.height), [1.0; 3], 1.0, [1.0; 3], 0.0, [1.0; 3]);
        Some(PageDmd { id: f.id, place, look, luminance: f.luminance })
    }

    /// The score panel's drawing for the score window, made the first time
    /// it's shown.
    fn panel_drawing(&mut self) -> Result<PageDrawing, String> {
        let d = self.display.as_mut().ok_or("the score display has no window")?;
        if let Some(p) = d.panel.take() {
            return Ok(p);
        }
        let format = d.format;
        Ok(PageDrawing::new(Page::Panel, format, self.page_pipeline(format)?))
    }

    /// Uploads a page's pictures, each with its mipmaps, the first time its
    /// file is shown, and makes room for its pictures' corners. The score
    /// panel's pictures are sRGB, decoded as they're drawn where the window encodes sRGB itself.
    fn upload_page(&self, d: &mut PageDrawing, file: &Arc<B2sFile>) -> Result<(), String> {
        if d.file.as_ref().is_some_and(|f| Arc::ptr_eq(f, file)) {
            return Ok(());
        }
        self.release_page_pictures(d);
        let list = pictures(file, d.page);
        // sRGB values as they are into a window that takes them (the backglass window's, the score
        // window in HDR10), else decoded as they're drawn, for a window that encodes sRGB itself
        let format = if super::embed::holds_srgb_values(d.format) { SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM } else { SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB };
        let usage = SDL_GPU_TEXTUREUSAGE_SAMPLER | SDL_GPU_TEXTUREUSAGE_COLOR_TARGET;
        let mut textures = Vec::with_capacity(list.len());
        for p in &list {
            textures.push(match p {
                Some(p) if p.width > 0 && p.height > 0 => {
                    let levels = 32 - p.width.max(p.height).leading_zeros();
                    match self.texture(format, usage, p.width, p.height, levels, SDL_GPU_SAMPLECOUNT_1) {
                        Ok(t) => t,
                        Err(e) => {
                            d.textures = textures;
                            self.release_page_pictures(d);
                            return Err(e);
                        }
                    }
                }
                _ => null_mut(),
            });
        }
        d.textures = textures;
        // room for every picture drawn at once: the background twice, two a light, every reel digit
        let quads = 2 + 2 * file.bulbs_of(d.page).len() + file.scores_of(d.page).iter().map(|s| s.digits.max(0) as usize).sum::<usize>();
        let capacity = 6 * quads.max(1);
        d.vertices = self.buffer(SDL_GPU_BUFFERUSAGE_VERTEX, capacity * size_of::<OverlayVertex>())?;
        let total: usize = list.iter().flatten().map(|p| p.rgba.len()).sum();
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
            info.size = (capacity * size_of::<OverlayVertex>()) as u32;
            d.transfer = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
            d.capacity = capacity;
            d.file = Some(file.clone());
            d.shown = None;
            if total == 0 {
                return Ok(());
            }
            info.size = total as u32;
            let upload = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
            let mapped = SDL_MapGPUTransferBuffer(self.device, upload, false) as *mut u8;
            if mapped.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, upload);
                return Err(format!("map transfer buffer: {}", sdl_error()));
            }
            let mut offsets = Vec::with_capacity(list.len());
            let mut offset = 0;
            for p in &list {
                offsets.push(offset);
                if let Some(p) = p {
                    std::ptr::copy_nonoverlapping(p.rgba.as_ptr(), mapped.add(offset), p.rgba.len());
                    offset += p.rgba.len();
                }
            }
            SDL_UnmapGPUTransferBuffer(self.device, upload);
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            let copy = SDL_BeginGPUCopyPass(command);
            for (k, p) in list.iter().enumerate() {
                let (Some(p), t) = (p, d.textures[k]) else { continue };
                if t.is_null() {
                    continue;
                }
                let mut from: SDL_GPUTextureTransferInfo = zeroed();
                from.transfer_buffer = upload;
                from.offset = offsets[k] as u32;
                let mut to: SDL_GPUTextureRegion = zeroed();
                to.texture = t;
                to.w = p.width;
                to.h = p.height;
                to.d = 1;
                SDL_UploadToGPUTexture(copy, &from, &to, false);
            }
            SDL_EndGPUCopyPass(copy);
            for (k, p) in list.iter().enumerate() {
                if p.is_some_and(|p| p.width.max(p.height) > 1) && !d.textures[k].is_null() {
                    SDL_GenerateMipmapsForGPUTexture(command, d.textures[k]);
                }
            }
            let submitted = SDL_SubmitGPUCommandBuffer(command);
            SDL_ReleaseGPUTransferBuffer(self.device, upload);
            check(submitted, "upload the backglass")
        }
    }

    /// Releases a page's pictures and buffers uploaded so far.
    fn release_page_pictures(&self, d: &mut PageDrawing) {
        unsafe {
            for t in d.textures.drain(..) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t);
                }
            }
            if !d.vertices.is_null() {
                SDL_ReleaseGPUBuffer(self.device, d.vertices);
            }
            if !d.transfer.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, d.transfer);
            }
        }
        d.vertices = null_mut();
        d.transfer = null_mut();
        d.file = None;
        d.shown = None;
    }

    /// Releases a page's drawing: its pictures, buffers and pipelines.
    pub(super) fn release_page(&self, mut d: PageDrawing) {
        self.release_page_pictures(&mut d);
        for p in [d.pipeline, d.dmd_pipeline] {
            if !p.is_null() {
                unsafe { SDL_ReleaseGPUGraphicsPipeline(self.device, p) };
            }
        }
    }

    /// Draws the backglass window on `command` when the backglass or the
    /// window's size changed since it was last shown, and the window has an
    /// image free.
    pub(super) unsafe fn present_backglass(
        &mut self,
        command: *mut SDL_GPUCommandBuffer,
        now: Option<&Arc<BackglassNow>>,
        frame: Option<&DmdFrame>,
        pup: Option<&Arc<crate::pup::draw::PupNow>>,
    ) {
        // an achievement's announcement come or gone since it last drew: drawn again, with it
        let count = self.toast_count();
        if let Some(bg) = &mut self.backglass
            && bg.toast_seen != count
        {
            bg.toast_seen = count;
            if let Some(d) = &mut bg.drawing {
                d.forget();
            }
            if let Some(p) = &mut bg.pup {
                p.forget();
            }
        }
        self.drawn_into = None;
        // VPX's renderers for the window, in order: the backglass file's, then a PinUP pack's
        // backglass screens (first when preferred)
        if let Some(layers) = Self::pup_layers(pup, 1).filter(|_| now.is_none() || self.pack_first[0]) {
            let Some(bg) = &mut self.backglass else { return };
            let surface = bg.surface;
            if let Some(d) = &mut bg.drawing {
                d.forget();
            }
            let format = bg.drawing.as_ref().map(|d| d.format);
            let kept = bg.pup.take();
            self.reveal(surface);
            let mut p = match (kept, format) {
                (Some(p), _) => p,
                (None, Some(format)) => match self.page_pipeline(format) {
                    Ok(pipeline) => super::pup::PupDrawing::new(format, pipeline),
                    Err(e) => {
                        crate::script::log::once(&format!("PinUP in the backglass window: {e}"));
                        return;
                    }
                },
                (None, None) => return,
            };
            // SAFETY: the window outlives the renderer
            unsafe { self.present_pup(command, surface, &mut p, layers) };
            if let Some(bg) = &mut self.backglass {
                bg.pup = Some(p);
            }
            // SAFETY: as above
            unsafe { self.toast_over(command) };
            return;
        }
        let Some(now) = now else { return };
        let Some(bg) = &mut self.backglass else { return };
        if let Some(p) = &mut bg.pup {
            p.forget();
        }
        let surface = bg.surface;
        let Some(mut d) = bg.drawing.take() else { return };
        self.reveal(surface);
        // SAFETY: the window outlives the renderer
        unsafe { self.present_page(command, surface, &mut d, now, frame) };
        if let Some(bg) = &mut self.backglass {
            bg.drawing = Some(d);
        }
        // SAFETY: as above
        unsafe { self.toast_over(command) };
    }

    /// An achievement's announcement over what was drawn in the backglass window this frame, if
    /// anything was.
    ///
    /// # Safety
    /// On the renderer's thread, `command` recording the frame.
    unsafe fn toast_over(&mut self, command: *mut SDL_GPUCommandBuffer) {
        if let Some((target, w, h, format)) = self.drawn_into.take() {
            // SAFETY: as the caller promises; the target was drawn on this command buffer
            unsafe { self.record_toast(command, target, w, h, format) };
        }
    }

    /// Draws the score panel in the score window, in place of the machine's
    /// displays, on `command`, the DMD `frame` in it as set.
    ///
    /// # Safety
    /// The score window must be live.
    pub(super) unsafe fn present_panel(&mut self, command: *mut SDL_GPUCommandBuffer, now: &Arc<BackglassNow>, frame: Option<&DmdFrame>) {
        let Some(surface) = self.display.as_ref().map(|d| d.surface) else { return };
        let mut d = match self.panel_drawing() {
            Ok(d) => d,
            Err(e) => {
                crate::script::log::once(&format!("score panel: {e}"));
                return;
            }
        };
        // SAFETY: as the caller promises
        unsafe { self.present_page(command, surface, &mut d, now, frame) };
        if let Some(display) = &mut self.display {
            display.panel = Some(d);
        }
    }

    /// Draws a page onto `surface` on `command` when it changed since it was
    /// last shown there, or the surface's size did, or the DMD over it, and
    /// the window has an image free.
    unsafe fn present_page(&mut self, command: *mut SDL_GPUCommandBuffer, surface: super::Surface, d: &mut PageDrawing, now: &Arc<BackglassNow>, frame: Option<&DmdFrame>) {
        let size = self.surface_size(surface);
        let dmd = self.page_dmd(d, now, frame, false);
        let dmd_key = dmd.as_ref().map(|m| (m.id, m.place));
        if d.showing(now, size, dmd_key) {
            return;
        }
        if let Err(e) = self.upload_page(d, &now.file) {
            crate::script::log::once(&format!("backglass: {e}"));
            return;
        }
        let digits = self.page_digits(d, now);
        // SAFETY: as above; the image is drawn on this command buffer
        unsafe {
            let Some((target, tw, th)) = self.acquire_surface(command, surface) else { return };
            self.record_page(d, command, target, tw, th, now, digits, dmd.as_ref());
            self.drawn_into = Some((target, tw, th, d.format));
        }
        d.shown = Some((now.clone(), size.0, size.1, dmd_key));
    }

    /// A page's segment scores' pipeline, made ready before its pass; None
    /// when the page has none.
    fn page_digits(&mut self, d: &PageDrawing, now: &BackglassNow) -> Option<*mut SDL_GPUGraphicsPipeline> {
        // the pictures they need don't depend on where they go
        let list = digits(now, d.page, (1.0, 1.0));
        if list.is_empty() {
            return None;
        }
        match self.prepare_digits(d.format, &list) {
            Ok(pipeline) => Some(pipeline),
            Err(e) => {
                crate::script::log::once(&format!("backglass segment scores: {e}"));
                None
            }
        }
    }

    /// Draws a page into `target`, `width` × `height` pixels.
    #[allow(clippy::too_many_arguments)]
    unsafe fn record_page(
        &self,
        d: &PageDrawing,
        command: *mut SDL_GPUCommandBuffer,
        target: *mut SDL_GPUTexture,
        width: u32,
        height: u32,
        now: &BackglassNow,
        digits: Option<*mut SDL_GPUGraphicsPipeline>,
        dmd: Option<&PageDmd>,
    ) {
        let (w, h) = (width as f32, height as f32);
        let quads = quads(now, d.page);
        let mut vertices: Vec<OverlayVertex> = Vec::with_capacity(quads.len() * 6);
        let mut draws: Vec<(usize, u32)> = Vec::with_capacity(quads.len());
        for q in quads.iter().take(d.capacity / 6) {
            if d.textures.get(q.picture).is_none_or(|t| t.is_null()) {
                continue;
            }
            let [u0, v0, u1, v1] = q.uv;
            let v = |corner: [f32; 2], u: f32, t: f32| OverlayVertex { position: [corner[0] * w, corner[1] * h], uv: [u, t], color: q.color };
            let [tl, tr, bl, br] = q.corners;
            draws.push((q.picture, vertices.len() as u32));
            vertices.extend([v(tl, u0, v0), v(tr, u1, v0), v(bl, u0, v1), v(tr, u1, v0), v(br, u1, v1), v(bl, u0, v1)]);
        }
        unsafe {
            if !vertices.is_empty() {
                let mapped = SDL_MapGPUTransferBuffer(self.device, d.transfer, true) as *mut OverlayVertex;
                if mapped.is_null() {
                    return;
                }
                std::ptr::copy_nonoverlapping(vertices.as_ptr(), mapped, vertices.len());
                SDL_UnmapGPUTransferBuffer(self.device, d.transfer);
                let copy = SDL_BeginGPUCopyPass(command);
                let source = SDL_GPUTransferBufferLocation { transfer_buffer: d.transfer, offset: 0 };
                let region = SDL_GPUBufferRegion { buffer: d.vertices, offset: 0, size: (vertices.len() * size_of::<OverlayVertex>()) as u32 };
                SDL_UploadToGPUBuffer(copy, &source, &region, true);
                SDL_EndGPUCopyPass(copy);
            }
            let mut color: SDL_GPUColorTargetInfo = zeroed();
            color.texture = target;
            color.load_op = SDL_GPU_LOADOP_CLEAR;
            color.store_op = SDL_GPU_STOREOP_STORE;
            color.clear_color = SDL_FColor { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
            let pass = SDL_BeginGPURenderPass(command, &color, 1, std::ptr::null());
            if !draws.is_empty() {
                SDL_BindGPUGraphicsPipeline(pass, d.pipeline);
                let binding = SDL_GPUBufferBinding { buffer: d.vertices, offset: 0 };
                SDL_BindGPUVertexBuffers(pass, 0, &binding, 1);
                let screen = [2.0 / w, -2.0 / h, 0.0, 0.0];
                SDL_PushGPUVertexUniformData(command, 0, screen.as_ptr().cast(), 16);
                for &(picture, first) in &draws {
                    let binding = SDL_GPUTextureSamplerBinding { texture: d.textures[picture], sampler: self.clamp_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                    SDL_DrawGPUPrimitives(pass, 6, 1, first, 0);
                }
            }
            // the segment scores over the pictures, tonemapped, in the values the window takes
            if let Some(pipeline) = digits {
                self.draw_digits(command, pass, pipeline, &self::digits(now, d.page, (w, h)), (w, h), digit_mode(d.format));
            }
            // and last the DMD, as VPX's overlay draws it, likewise
            if let Some(m) = dmd.filter(|_| !d.dmd_pipeline.is_null()) {
                let (src_w, src_h) = source_size(now, d.page);
                let [x, y, pw, ph] = m.place;
                let rect = [x / src_w * w, y / src_h * h, (x + pw) / src_w * w, (y + ph) / src_h * h];
                self.draw_dmd(command, pass, d.dmd_pipeline, rect, &m.look, m.luminance, (w, h), digit_mode(d.format));
            }
            SDL_EndGPURenderPass(pass);
        }
    }

    /// Draws a page off screen, as its window would show it, the DMD
    /// `frame` over it as set (and an achievement's announcement, with `toast`), and returns it as
    /// RGBA pixels.
    fn capture_page(&mut self, d: &mut PageDrawing, now: &Arc<BackglassNow>, frame: Option<&DmdFrame>, width: u32, height: u32, toast: bool) -> Result<Vec<u8>, String> {
        self.upload_page(d, &now.file)?;
        let digits = self.page_digits(d, now);
        let target = self.texture(d.format, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
        unsafe {
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            self.update_dmd(command, frame);
            let dmd = self.page_dmd(d, now, frame, true);
            self.record_page(d, command, target, width, height, now, digits, dmd.as_ref());
            if toast {
                self.record_toast(command, target, width, height, d.format);
            }
            self.read_back(command, target, width, height, d.format, None)
        }
    }

    /// Draws the backglass window off screen, the DMD `frame` over it as
    /// set, and returns it as RGBA pixels, for pictures taken by the
    /// player's test mode; an achievement's announcement on it with `toast`.
    pub fn capture_backglass_window(
        &mut self,
        now: Option<&Arc<BackglassNow>>,
        pup: Option<&Arc<crate::pup::draw::PupNow>>,
        frame: Option<&DmdFrame>,
        width: u32,
        height: u32,
        toast: bool,
    ) -> Result<Vec<u8>, String> {
        if let Some(layers) = Self::pup_layers(pup, 1).filter(|_| now.is_none() || self.pack_first[0]) {
            let bg = self.backglass.as_mut().ok_or("the backglass has no window")?;
            let format = bg.drawing.as_ref().map(|d| d.format).ok_or("the backglass has no window")?;
            let mut p = match bg.pup.take() {
                Some(p) => p,
                None => super::pup::PupDrawing::new(format, self.page_pipeline(format)?),
            };
            let rgba = self.capture_pup(&mut p, layers, width, height, toast);
            if let Some(bg) = &mut self.backglass {
                bg.pup = Some(p);
            }
            return rgba;
        }
        let now = now.ok_or("nothing shows in the backglass window yet")?;
        self.capture_backglass(now, frame, width, height, toast)
    }

    pub fn capture_backglass(&mut self, now: &Arc<BackglassNow>, frame: Option<&DmdFrame>, width: u32, height: u32, toast: bool) -> Result<Vec<u8>, String> {
        let mut d = self.backglass.as_mut().and_then(|b| b.drawing.take()).ok_or("the backglass has no window")?;
        let rgba = self.capture_page(&mut d, now, frame, width, height, toast);
        if let Some(bg) = &mut self.backglass {
            bg.drawing = Some(d);
        }
        rgba
    }

    /// Draws the score panel off screen, as the score window would show it.
    pub(super) fn capture_panel(&mut self, now: &Arc<BackglassNow>, frame: Option<&DmdFrame>, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let mut d = self.panel_drawing()?;
        let rgba = self.capture_page(&mut d, now, frame, width, height, false);
        if let Some(display) = &mut self.display {
            display.panel = Some(d);
        }
        rgba
    }

    /// Releases the backglass window's resources (the renderer's `Drop`).
    pub(super) fn release_backglass(&mut self) {
        if let Some(mut bg) = self.backglass.take() {
            if let Some(d) = bg.drawing.take() {
                self.release_page(d);
            }
            if let Some(p) = bg.pup.take() {
                self.release_pup(p);
            }
            if let Some(window) = bg.surface.window() {
                unsafe { SDL_ReleaseWindowFromGPUDevice(self.device, window) };
            }
        }
    }
}
