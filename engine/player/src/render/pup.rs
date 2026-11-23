// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): how its PinUp Player
// plugin draws a picture over a screen's place (plugins/pup/PUPImage.cpp: Load, Render) and how VPX draws a plugin's
// picture in a window of its own (Renderer::DrawImage, src/renderer/Renderer.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A PinUP pack's screens in the player's windows: the layers the pack shows
//! this frame (`pup::draw`), each picture stretched over its screen's place and blended over
//! what's below at the screen's opacity, as VPX's plugin draws them. A picture file loads on a
//! thread of its own, as VPX loads it, and shows once loaded; a video's picture is uploaded
//! when it changes. The window is drawn again only when what it shows changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::pup::draw::{Content, Layer, PupNow, WindowLayers};

/// A picture file's texture, loading or loaded.
enum FileTexture {
    Loading(std::thread::JoinHandle<Option<image::RgbaImage>>),
    Ready(*mut SDL_GPUTexture),
    Failed,
}

/// A video's picture on the graphics card: the picture it holds, its size.
struct FrameTexture {
    id: u64,
    width: u32,
    height: u32,
    texture: *mut SDL_GPUTexture,
}

/// What a window showed last: each layer's picture, place and opacity, and the window's size.
type Shown = (Vec<(u64, [i32; 4], u32)>, u32, u32);

/// One window's drawing of a pack.
pub(super) struct PupDrawing {
    format: SDL_GPUTextureFormat,
    /// Pictures decoded as sRGB as they're drawn: in a window whose output is linear.
    srgb: bool,
    pipeline: *mut SDL_GPUGraphicsPipeline,
    files: HashMap<PathBuf, FileTexture>,
    /// An id for each picture file, for knowing what was shown.
    file_ids: HashMap<PathBuf, u64>,
    frames: Vec<FrameTexture>,
    vertices: *mut SDL_GPUBuffer,
    vertex_transfer: *mut SDL_GPUTransferBuffer,
    /// Room in the vertex buffer, in layers.
    capacity: usize,
    /// For a video's pictures.
    upload: *mut SDL_GPUTransferBuffer,
    upload_size: usize,
    shown: Option<Shown>,
}

impl PupDrawing {
    pub(super) fn new(format: SDL_GPUTextureFormat, pipeline: *mut SDL_GPUGraphicsPipeline) -> PupDrawing {
        let srgb = matches!(format, SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM_SRGB | SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB);
        PupDrawing {
            format,
            srgb,
            pipeline,
            files: HashMap::new(),
            file_ids: HashMap::new(),
            frames: Vec::new(),
            vertices: null_mut(),
            vertex_transfer: null_mut(),
            capacity: 0,
            upload: null_mut(),
            upload_size: 0,
            shown: None,
        }
    }

    /// Forgets what was shown, so the next frame draws it again.
    pub(super) fn forget(&mut self) {
        self.shown = None;
    }

    fn texture_format(&self) -> SDL_GPUTextureFormat {
        if self.srgb { SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB } else { SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM }
    }
}

/// A layer's quad in a window of `w` × `h` pixels, its place given for a window of `size`:
/// left, top, right, bottom, and the picture's corners in it (VPX's `ClipDrawImage`: what lies
/// outside the clip is cut, the picture's coordinates with it). None when nothing is left.
fn quad_of(layer: &Layer, size: (u32, u32), w: u32, h: u32) -> Option<([f32; 4], [f32; 4])> {
    let (sx, sy) = (w as f32 / size.0.max(1) as f32, h as f32 / size.1.max(1) as f32);
    let [x, y, rw, rh] = layer.rect;
    let (x0, y0, x1, y1) = (x * sx, y * sy, (x + rw) * sx, (y + rh) * sy);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (mut q, mut uv) = ([x0, y0, x1, y1], [0.0, 0.0, 1.0, 1.0]);
    if let Some([cx, cy, cw, ch]) = layer.clip {
        let c = [cx as f32 * sx, cy as f32 * sy, (cx + cw) as f32 * sx, (cy + ch) as f32 * sy];
        let cut = [q[0].max(c[0]), q[1].max(c[1]), q[2].min(c[2]), q[3].min(c[3])];
        if cut[2] <= cut[0] || cut[3] <= cut[1] {
            return None;
        }
        uv = [(cut[0] - x0) / (x1 - x0), (cut[1] - y0) / (y1 - y0), (cut[2] - x0) / (x1 - x0), (cut[3] - y0) / (y1 - y0)];
        q = cut;
    }
    Some((q, uv))
}

impl Renderer {
    /// Draws a pack's layers onto `surface` on `command` when they or its size changed.
    ///
    /// # Safety
    /// A window must be live.
    pub(super) unsafe fn present_pup(&mut self, command: *mut SDL_GPUCommandBuffer, surface: super::Surface, d: &mut PupDrawing, wl: &WindowLayers) {
        let (w, h) = self.surface_size(surface);
        if w == 0 || h == 0 {
            return;
        }
        let layers = &wl.layers;
        self.load_files(d, layers);
        let key: Vec<(u64, [i32; 4], u32)> = layers
            .iter()
            .filter_map(|l| {
                let id = self.layer_key(d, l)?;
                let (q, _) = quad_of(l, wl.size, w as u32, h as u32)?;
                Some((id, q.map(|v| (v * 16.0) as i32), l.alpha.to_bits()))
            })
            .collect();
        let shown = (key, w, h);
        if d.shown.as_ref() == Some(&shown) {
            return;
        }
        // SAFETY: as above; the image is drawn on this command buffer
        unsafe {
            let Some((target, tw, th)) = self.acquire_surface(command, surface) else { return };
            if let Err(e) = self.record_pup(d, command, target, tw, th, wl) {
                crate::script::log::once(&format!("PinUP: {e}"));
                return;
            }
            self.drawn_into = Some((target, tw, th, d.format));
        }
        d.shown = Some(shown);
    }

    /// The id of what a layer shows, None while its picture isn't ready.
    fn layer_key(&self, d: &PupDrawing, layer: &Layer) -> Option<u64> {
        match &layer.content {
            Content::Frame(p) => Some(p.id),
            Content::File(path) => match d.files.get(path) {
                Some(FileTexture::Ready(_)) => d.file_ids.get(path).map(|id| id | 1 << 63),
                _ => None,
            },
        }
    }

    /// Starts loading the layers' picture files not seen yet, and uploads those loaded since.
    fn load_files(&self, d: &mut PupDrawing, layers: &[Layer]) {
        for l in layers {
            if let Content::File(path) = &l.content
                && !d.files.contains_key(path)
            {
                let p = path.clone();
                let next = d.file_ids.len() as u64 + 1;
                d.file_ids.insert(path.clone(), next);
                let job = std::thread::Builder::new().name("pup picture".into()).spawn(move || image::open(&p).ok().map(|i| i.to_rgba8()));
                d.files.insert(path.clone(), job.map_or(FileTexture::Failed, FileTexture::Loading));
            }
        }
        let done: Vec<PathBuf> = d.files.iter().filter(|(_, f)| matches!(f, FileTexture::Loading(j) if j.is_finished())).map(|(p, _)| p.clone()).collect();
        for path in done {
            let Some(FileTexture::Loading(job)) = d.files.remove(&path) else { continue };
            let state = match job.join().ok().flatten() {
                Some(img) if img.width() > 0 && img.height() > 0 => match self.upload_picture(d.texture_format(), img.width(), img.height(), img.as_raw()) {
                    Ok(texture) => FileTexture::Ready(texture),
                    Err(e) => {
                        crate::script::log::once(&format!("PinUP: {}: {e}", path.display()));
                        FileTexture::Failed
                    }
                },
                _ => {
                    crate::script::log::once(&format!("PinUP: {} can't be read", path.display()));
                    FileTexture::Failed
                }
            };
            d.files.insert(path, state);
        }
    }

    /// A picture uploaded to a new texture of its own.
    pub(super) fn upload_picture(&self, format: SDL_GPUTextureFormat, width: u32, height: u32, rgba: &[u8]) -> Result<*mut SDL_GPUTexture, String> {
        let texture = self.texture(format, SDL_GPU_TEXTUREUSAGE_SAMPLER, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
            info.size = rgba.len() as u32;
            let transfer = SDL_CreateGPUTransferBuffer(self.device, &info);
            if transfer.is_null() {
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(format!("create transfer buffer: {}", sdl_error()));
            }
            let mapped = SDL_MapGPUTransferBuffer(self.device, transfer, false) as *mut u8;
            if mapped.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, transfer);
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(format!("map transfer buffer: {}", sdl_error()));
            }
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), mapped, rgba.len());
            SDL_UnmapGPUTransferBuffer(self.device, transfer);
            let command = SDL_AcquireGPUCommandBuffer(self.device);
            if command.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, transfer);
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(format!("acquire command buffer: {}", sdl_error()));
            }
            let copy = SDL_BeginGPUCopyPass(command);
            upload_texture(copy, transfer, texture, 0, width, height);
            SDL_EndGPUCopyPass(copy);
            let submitted = SDL_SubmitGPUCommandBuffer(command);
            SDL_ReleaseGPUTransferBuffer(self.device, transfer);
            if !submitted {
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(format!("upload a picture: {}", sdl_error()));
            }
        }
        Ok(texture)
    }

    /// The videos' pictures on the graphics card: each layer's picture kept where it was
    /// uploaded already, else uploaded into a texture of its size no other layer needs. Returns
    /// each layer's texture (null where it has none yet).
    fn upload_frames(&self, d: &mut PupDrawing, command: *mut SDL_GPUCommandBuffer, layers: &[Layer]) -> Result<Vec<*mut SDL_GPUTexture>, String> {
        let wanted: Vec<u64> = layers.iter().filter_map(|l| if let Content::Frame(p) = &l.content { Some(p.id) } else { None }).collect();
        let mut out = vec![null_mut(); layers.len()];
        let mut used = vec![false; d.frames.len()];
        // the pictures already there
        for (k, l) in layers.iter().enumerate() {
            if let Content::Frame(p) = &l.content
                && let Some(i) = d.frames.iter().position(|f| f.id == p.id)
            {
                out[k] = d.frames[i].texture;
                used[i] = true;
            }
        }
        // the new ones: into a free texture of their size, else a new one (each kept by its
        // texture, not its place in the list, which the releasing below shifts)
        let mut uploads: Vec<(*mut SDL_GPUTexture, Arc<crate::pup::playback::Picture>)> = Vec::new();
        for (k, l) in layers.iter().enumerate() {
            let Content::Frame(p) = &l.content else { continue };
            if !out[k].is_null() {
                continue;
            }
            let free = (0..d.frames.len()).find(|&i| !used[i] && !wanted.contains(&d.frames[i].id) && d.frames[i].width == p.width && d.frames[i].height == p.height);
            let i = match free {
                Some(i) => i,
                None => {
                    let texture = self.texture(d.texture_format(), SDL_GPU_TEXTUREUSAGE_SAMPLER, p.width, p.height, 1, SDL_GPU_SAMPLECOUNT_1)?;
                    d.frames.push(FrameTexture { id: 0, width: p.width, height: p.height, texture });
                    used.push(false);
                    d.frames.len() - 1
                }
            };
            d.frames[i].id = p.id;
            used[i] = true;
            out[k] = d.frames[i].texture;
            uploads.push((d.frames[i].texture, p.clone()));
        }
        // textures no layer used this time and of no size wanted: released
        let mut k = 0;
        while k < d.frames.len() {
            if !used[k] && d.frames.len() > wanted.len() + 2 {
                unsafe { SDL_ReleaseGPUTexture(self.device, d.frames[k].texture) };
                d.frames.remove(k);
                used.remove(k);
            } else {
                k += 1;
            }
        }
        if uploads.is_empty() {
            return Ok(out);
        }
        let total: usize = uploads.iter().map(|(_, p)| p.rgba.len()).sum();
        unsafe {
            if total > d.upload_size {
                if !d.upload.is_null() {
                    SDL_ReleaseGPUTransferBuffer(self.device, d.upload);
                }
                let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
                info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
                info.size = total as u32;
                d.upload = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
                d.upload_size = total;
            }
            let mapped = SDL_MapGPUTransferBuffer(self.device, d.upload, true) as *mut u8;
            if mapped.is_null() {
                return Err(format!("map transfer buffer: {}", sdl_error()));
            }
            let mut at = 0;
            let mut offsets = Vec::with_capacity(uploads.len());
            for (_, p) in &uploads {
                std::ptr::copy_nonoverlapping(p.rgba.as_ptr(), mapped.add(at), p.rgba.len());
                offsets.push(at);
                at += p.rgba.len();
            }
            SDL_UnmapGPUTransferBuffer(self.device, d.upload);
            let copy = SDL_BeginGPUCopyPass(command);
            for ((texture, p), offset) in uploads.iter().zip(offsets) {
                upload_texture(copy, d.upload, *texture, offset as u32, p.width, p.height);
            }
            SDL_EndGPUCopyPass(copy);
        }
        Ok(out)
    }

    /// Draws the layers into `target`, `width` × `height` pixels: black, then each picture over
    /// its place.
    unsafe fn record_pup(&self, d: &mut PupDrawing, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32, wl: &WindowLayers) -> Result<(), String> {
        let layers = &wl.layers;
        let frames = self.upload_frames(d, command, layers)?;
        let mut vertices: Vec<OverlayVertex> = Vec::with_capacity(layers.len() * 6);
        let mut draws: Vec<(*mut SDL_GPUTexture, u32)> = Vec::new();
        for (k, l) in layers.iter().enumerate() {
            let texture = match &l.content {
                Content::Frame(_) => frames[k],
                Content::File(path) => match d.files.get(path) {
                    Some(FileTexture::Ready(texture)) => *texture,
                    _ => null_mut(),
                },
            };
            if texture.is_null() || l.alpha <= 0.0 {
                continue;
            }
            let Some(([x0, y0, x1, y1], [u0, v0, u1, v1])) = quad_of(l, wl.size, width, height) else { continue };
            let color = [1.0, 1.0, 1.0, l.alpha.min(1.0)];
            let v = |px: f32, py: f32, u: f32, t: f32| OverlayVertex { position: [px, py], uv: [u, t], color };
            draws.push((texture, vertices.len() as u32));
            vertices.extend([v(x0, y0, u0, v0), v(x1, y0, u1, v0), v(x0, y1, u0, v1), v(x1, y0, u1, v0), v(x1, y1, u1, v1), v(x0, y1, u0, v1)]);
        }
        unsafe {
            if !vertices.is_empty() {
                let layers_needed = vertices.len() / 6;
                if layers_needed > d.capacity {
                    if !d.vertices.is_null() {
                        SDL_ReleaseGPUBuffer(self.device, d.vertices);
                    }
                    if !d.vertex_transfer.is_null() {
                        SDL_ReleaseGPUTransferBuffer(self.device, d.vertex_transfer);
                    }
                    let capacity = layers_needed.next_power_of_two().max(8);
                    let bytes = capacity * 6 * size_of::<OverlayVertex>();
                    d.vertices = self.buffer(SDL_GPU_BUFFERUSAGE_VERTEX, bytes)?;
                    let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
                    info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
                    info.size = bytes as u32;
                    d.vertex_transfer = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
                    d.capacity = capacity;
                }
                let mapped = SDL_MapGPUTransferBuffer(self.device, d.vertex_transfer, true) as *mut OverlayVertex;
                if mapped.is_null() {
                    return Err(format!("map transfer buffer: {}", sdl_error()));
                }
                std::ptr::copy_nonoverlapping(vertices.as_ptr(), mapped, vertices.len());
                SDL_UnmapGPUTransferBuffer(self.device, d.vertex_transfer);
                let copy = SDL_BeginGPUCopyPass(command);
                let source = SDL_GPUTransferBufferLocation { transfer_buffer: d.vertex_transfer, offset: 0 };
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
                let screen = [2.0 / width as f32, -2.0 / height as f32, 0.0, 0.0];
                SDL_PushGPUVertexUniformData(command, 0, screen.as_ptr().cast(), 16);
                for &(texture, first) in &draws {
                    let binding = SDL_GPUTextureSamplerBinding { texture, sampler: self.clamp_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                    SDL_DrawGPUPrimitives(pass, 6, 1, first, 0);
                }
            }
            SDL_EndGPURenderPass(pass);
        }
        Ok(())
    }

    /// Draws a pack's layers off screen, as `window`'s drawing would (an achievement's
    /// announcement over them with `toast`), and returns them as RGBA pixels, for pictures taken
    /// by the player's test mode; picture files are waited for.
    pub(super) fn capture_pup(&mut self, d: &mut PupDrawing, wl: &WindowLayers, width: u32, height: u32, toast: bool) -> Result<Vec<u8>, String> {
        // the files: loaded before the picture (test mode's pictures repeat)
        loop {
            self.load_files(d, &wl.layers);
            if !d.files.values().any(|f| matches!(f, FileTexture::Loading(_))) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let target = self.texture(d.format, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
        unsafe {
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            self.record_pup(d, command, target, width, height, wl)?;
            d.shown = None;
            if toast {
                self.record_toast(command, target, width, height, d.format);
            }
            self.read_back(command, target, width, height, d.format, None)
        }
    }

    /// Releases a window's drawing of a pack.
    pub(super) fn release_pup(&self, d: PupDrawing) {
        unsafe {
            for f in d.files.into_values() {
                if let FileTexture::Ready(texture) = f {
                    SDL_ReleaseGPUTexture(self.device, texture);
                }
            }
            for f in d.frames {
                SDL_ReleaseGPUTexture(self.device, f.texture);
            }
            if !d.vertices.is_null() {
                SDL_ReleaseGPUBuffer(self.device, d.vertices);
            }
            for t in [d.vertex_transfer, d.upload] {
                if !t.is_null() {
                    SDL_ReleaseGPUTransferBuffer(self.device, t);
                }
            }
            if !d.pipeline.is_null() {
                SDL_ReleaseGPUGraphicsPipeline(self.device, d.pipeline);
            }
        }
    }

    /// The pack's layers for a window (0 the topper, 1 the backglass, 2 the score display).
    pub(super) fn pup_layers(pup: Option<&Arc<PupNow>>, window: usize) -> Option<&WindowLayers> {
        pup?.windows.get(window)?.as_ref()
    }

    /// Gives the topper a window of its own (VPX's topper window), where a pack's topper screen
    /// shows; drawn along with the playfield's frames whenever that changes, never making a
    /// frame wait.
    ///
    /// # Safety
    /// `window` must be a live SDL window, created on this thread, that outlives the renderer.
    pub unsafe fn add_topper_window(&mut self, window: *mut SDL_Window) -> Result<(), String> {
        unsafe {
            check(SDL_ClaimWindowForGPUDevice(self.device, window), "claim the topper window")?;
            let mode = [SDL_GPU_PRESENTMODE_MAILBOX, SDL_GPU_PRESENTMODE_IMMEDIATE]
                .into_iter()
                .find(|&m| SDL_WindowSupportsGPUPresentMode(self.device, window, m))
                .unwrap_or(SDL_GPU_PRESENTMODE_VSYNC);
            // sRGB values in and out, as the backglass window's
            if let Err(e) = check(SDL_SetGPUSwapchainParameters(self.device, window, SDL_GPU_SWAPCHAINCOMPOSITION_SDR, mode), "set the topper window's present mode") {
                SDL_ReleaseWindowFromGPUDevice(self.device, window);
                return Err(e);
            }
            let format = SDL_GetGPUSwapchainTextureFormat(self.device, window);
            self.add_topper_surface(super::Surface::Window(window), format);
        }
        Ok(())
    }

    /// The topper drawn on `surface`, pictures of `format`.
    pub(super) fn add_topper_surface(&mut self, surface: super::Surface, format: SDL_GPUTextureFormat) {
        self.topper = Some(TopperWindow { surface, format, pup: None });
    }

    /// Draws the topper window on `command` when the pack's topper screen changed since it was
    /// last shown, and the window has an image free.
    pub(super) unsafe fn present_topper(&mut self, command: *mut SDL_GPUCommandBuffer, pup: Option<&Arc<PupNow>>) {
        let Some(layers) = Self::pup_layers(pup, 0) else { return };
        let Some(t) = &mut self.topper else { return };
        let (surface, format, kept) = (t.surface, t.format, t.pup.take());
        self.reveal(surface);
        let mut p = match kept {
            Some(p) => p,
            None => match self.page_pipeline(format) {
                Ok(pipeline) => PupDrawing::new(format, pipeline),
                Err(e) => {
                    crate::script::log::once(&format!("PinUP in the topper window: {e}"));
                    return;
                }
            },
        };
        // SAFETY: the window outlives the renderer
        unsafe { self.present_pup(command, surface, &mut p, layers) };
        if let Some(t) = &mut self.topper {
            t.pup = Some(p);
        }
    }

    /// Draws the topper window off screen and returns it as RGBA pixels, for pictures taken by
    /// the player's test mode; None while nothing shows there.
    pub fn capture_topper_window(&mut self, pup: Option<&Arc<PupNow>>, width: u32, height: u32) -> Result<Option<Vec<u8>>, String> {
        let Some(layers) = Self::pup_layers(pup, 0) else { return Ok(None) };
        let t = self.topper.as_mut().ok_or("the topper has no window")?;
        let format = t.format;
        let mut p = match t.pup.take() {
            Some(p) => p,
            None => PupDrawing::new(format, self.page_pipeline(format)?),
        };
        let rgba = self.capture_pup(&mut p, layers, width, height, false);
        if let Some(t) = &mut self.topper {
            t.pup = Some(p);
        }
        rgba.map(Some)
    }

    /// Releases the topper window's resources (the renderer's `Drop`).
    pub(super) fn release_topper(&mut self) {
        if let Some(mut t) = self.topper.take() {
            if let Some(p) = t.pup.take() {
                self.release_pup(p);
            }
            if let Some(window) = t.surface.window() {
                unsafe { SDL_ReleaseWindowFromGPUDevice(self.device, window) };
            }
        }
    }
}

/// The topper's own window (VPX's topper window), or its place inside the table's: a pack's topper
/// screen is all that shows there.
pub(super) struct TopperWindow {
    pub(super) surface: super::Surface,
    format: SDL_GPUTextureFormat,
    /// The pack's topper screen, once shown here.
    pup: Option<PupDrawing>,
}
