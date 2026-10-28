//! The backglass, score view and topper drawn inside the table's own window: VPX
//! 5436's embedded output (`[Backglass]` `BackglassOutput = 2`, and the same for the score view
//! and the topper; `Renderer::ClearEmbeddedAncillaryWindow`, `RenderAncillaryWindow`,
//! `SetupAncillaryRenderTarget`), for a cabinet or desk with one screen. Each is drawn as its window
//! would be, into a picture of ours the size of its place, and that picture taken into the table's
//! at its place: in VPX's look into the table's half-float picture before its tonemapping, as VPX
//! does — its place cleared first, so the occlusion and the bloom don't read the table under it,
//! and the occlusion kept off it — in the first look over the finished picture. Its place: the
//! window's settings (`…WndX`, `…WndY`, `…Width`, `…Height`), counted from the table window's
//! corner and kept inside it, as VPX keeps them; a place nothing draws in shows the table, as in VPX.
//! Ours, following VPX's.

use std::cell::Cell;
use std::mem::{size_of, zeroed};
use std::ptr::null_mut;

use sdl3_sys::everything::*;

use super::{EMBED_HLSL, HDR_FORMAT, Renderer, non_null};

/// Where an extra window's pictures go: its own window, or a picture of ours that the table's
/// frame takes in at its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Surface {
    Window(*mut SDL_Window),
    Embedded(Kind),
}

/// The extra windows that can be drawn inside the table's, in VPX's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Backglass,
    ScoreView,
    Topper,
}

impl Kind {
    fn index(self) -> usize {
        self as usize
    }
}

/// An extra window drawn inside the table's.
pub(super) struct Embedded {
    /// Its place in the table's window, in the window's pixels: left, top, width, height.
    place: [i32; 4],
    texture: *mut SDL_GPUTexture,
    format: SDL_GPUTextureFormat,
    /// Something drew in it (VPX's `m_ancillaryWndRendered`): its place is cleared and its
    /// picture taken in.
    drawn: Cell<bool>,
}

/// The pipelines the embedded windows' steps draw with.
pub(super) struct Pipelines {
    fill_hdr: *mut SDL_GPUGraphicsPipeline,
    fill_ao: *mut SDL_GPUGraphicsPipeline,
    copy_hdr: *mut SDL_GPUGraphicsPipeline,
    copy_out: *mut SDL_GPUGraphicsPipeline,
}

/// The embed shader's uniforms.
#[repr(C)]
struct EmbedUniforms {
    fill: [f32; 4],
    mode: [f32; 4],
}

impl Surface {
    pub(super) fn window(self) -> Option<*mut SDL_Window> {
        match self {
            Surface::Window(w) => Some(w),
            Surface::Embedded(_) => None,
        }
    }

    pub(super) fn is_embedded(self) -> bool {
        matches!(self, Surface::Embedded(_))
    }
}

/// Whether pictures of `format` hold sRGB values as they are (the hardware not encoding them).
pub(super) fn holds_srgb_values(format: SDL_GPUTextureFormat) -> bool {
    format != SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM_SRGB && format != SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB
}

impl Renderer {
    /// Draws the backglass, the score view or the topper inside the table's window at `place`
    /// (left, top, width, height in the window's units, from its corner), kept inside the window
    /// as VPX keeps it (`RenderOutput::SetMode`).
    ///
    /// # Safety
    /// On the renderer's thread.
    pub unsafe fn add_embedded(&mut self, kind: Kind, place: [i32; 4]) -> Result<(), String> {
        let (mut ww, mut wh, mut pw, mut ph) = (0, 0, 0, 0);
        // SAFETY: the table's window, live with the renderer
        unsafe {
            SDL_GetWindowSize(self.window, &mut ww, &mut wh);
            SDL_GetWindowSizeInPixels(self.window, &mut pw, &mut ph);
        }
        let [x, y, w, h] = place;
        let (w, h) = (w.clamp(1, ww.max(1)), h.clamp(1, wh.max(1)));
        let (x, y) = (x.clamp(0, (ww - w).max(0)), y.clamp(0, (wh - h).max(0)));
        // in the window's pixels (VPX's display scale)
        let (sx, sy) = (pw as f32 / ww.max(1) as f32, ph as f32 / wh.max(1) as f32);
        let place = [(x as f32 * sx) as i32, (y as f32 * sy) as i32, ((w as f32 * sx) as i32).max(1), ((h as f32 * sy) as i32).max(1)];
        if self.embed_pipelines.is_none() {
            self.embed_pipelines = Some(self.embed_pipelines()?);
        }
        // the score view draws linear values, as its window's (an sRGB picture encodes them); the
        // backglass and topper sRGB values as they are, as theirs
        let format = match kind {
            Kind::ScoreView => SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM_SRGB,
            Kind::Backglass | Kind::Topper => SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM,
        };
        let usage = SDL_GPU_TEXTUREUSAGE_COLOR_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER;
        let texture = self.texture(format, usage, place[2] as u32, place[3] as u32, 1, SDL_GPU_SAMPLECOUNT_1)?;
        if let Some(old) = self.embedded[kind.index()].replace(Embedded { place, texture, format, drawn: Cell::new(false) }) {
            // SAFETY: ours, no longer drawn with
            unsafe { SDL_ReleaseGPUTexture(self.device, old.texture) };
        }
        match kind {
            Kind::ScoreView => self.add_display_surface(Surface::Embedded(kind), format),
            Kind::Backglass => self.add_backglass_surface(Surface::Embedded(kind), format),
            Kind::Topper => {
                self.add_topper_surface(Surface::Embedded(kind), format);
                Ok(())
            }
        }
    }

    fn embed_pipelines(&self) -> Result<Pipelines, String> {
        let fill_hdr = self.embed_pipeline(c"embed_fill_fs", 0, HDR_FORMAT)?;
        let fill_ao = self.embed_pipeline(c"embed_fill_fs", 0, super::effects::AO_FORMAT)?;
        let copy_hdr = self.embed_pipeline(c"embed_copy_fs", 1, HDR_FORMAT)?;
        let copy_out = self.embed_pipeline(c"embed_copy_fs", 1, self.format)?;
        Ok(Pipelines { fill_hdr, fill_ao, copy_hdr, copy_out })
    }

    /// One of the embed shader's passes into a picture of `format`: one triangle over the
    /// viewport, no blending.
    fn embed_pipeline(&self, entry: &std::ffi::CStr, samplers: u32, format: SDL_GPUTextureFormat) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(EMBED_HLSL, c"embed_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 0)?;
        let fs = match self.shader(EMBED_HLSL, entry, c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, samplers, 1) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = format;
            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create an embedded window's pipeline")
        }
    }

    /// A surface's size in pixels.
    pub(super) fn surface_size(&self, s: Surface) -> (u32, u32) {
        match s {
            Surface::Window(window) => {
                let (mut w, mut h) = (0, 0);
                // SAFETY: the window outlives the renderer
                unsafe { SDL_GetWindowSizeInPixels(window, &mut w, &mut h) };
                (w.max(0) as u32, h.max(0) as u32)
            }
            Surface::Embedded(kind) => self.embedded[kind.index()].as_ref().map_or((0, 0), |e| (e.place[2] as u32, e.place[3] as u32)),
        }
    }

    /// The picture a surface's frame is drawn into on `command`, and its size: the window's next
    /// image (None while it has none free), or the embedded window's picture.
    ///
    /// # Safety
    /// The window outlives the renderer; `command` is recording.
    pub(super) unsafe fn acquire_surface(&self, command: *mut SDL_GPUCommandBuffer, s: Surface) -> Option<(*mut SDL_GPUTexture, u32, u32)> {
        match s {
            Surface::Window(window) => {
                let mut target = null_mut();
                let (mut tw, mut th) = (0, 0);
                // SAFETY: as the caller promises
                let acquired = unsafe { SDL_AcquireGPUSwapchainTexture(command, window, &mut target, &mut tw, &mut th) };
                (acquired && !target.is_null()).then_some((target, tw, th))
            }
            Surface::Embedded(kind) => self.embedded[kind.index()].as_ref().map(|e| (e.texture, e.place[2] as u32, e.place[3] as u32)),
        }
    }

    /// Something draws in an embedded window: its place is cleared and its picture taken in.
    pub(super) fn mark_embedded(&self, kind: Kind) {
        if let Some(e) = &self.embedded[kind.index()] {
            e.drawn.set(true);
        }
    }

    /// The embedded windows something drew in, their places in a picture of `size` drawn for a
    /// window of `window` pixels.
    fn embedded_places(&self, size: (u32, u32), window: (u32, u32)) -> Vec<(&Embedded, SDL_GPUViewport)> {
        let (sx, sy) = (size.0 as f32 / window.0.max(1) as f32, size.1 as f32 / window.1.max(1) as f32);
        self.embedded
            .iter()
            .flatten()
            .filter(|e| e.drawn.get())
            .map(|e| {
                let [x, y, w, h] = e.place.map(|v| v as f32);
                (e, SDL_GPUViewport { x: x * sx, y: y * sy, w: w * sx, h: h * sy, min_depth: 0.0, max_depth: 1.0 })
            })
            .collect()
    }

    /// One pass over the embedded windows' places in `target`, `size` pixels for a window of
    /// `window`: a fill, or their pictures.
    unsafe fn embed_pass(
        &self,
        command: *mut SDL_GPUCommandBuffer,
        target: *mut SDL_GPUTexture,
        size: (u32, u32),
        window: (u32, u32),
        pipeline: *mut SDL_GPUGraphicsPipeline,
        uniforms: impl Fn(&Embedded) -> EmbedUniforms,
        with_picture: bool,
    ) {
        let places = self.embedded_places(size, window);
        if places.is_empty() || target.is_null() {
            return;
        }
        // SAFETY: our pictures and pipelines, on a recording command buffer
        unsafe {
            let mut color: SDL_GPUColorTargetInfo = zeroed();
            color.texture = target;
            color.load_op = SDL_GPU_LOADOP_LOAD;
            color.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &color, 1, std::ptr::null());
            SDL_BindGPUGraphicsPipeline(pass, pipeline);
            for (e, viewport) in places {
                SDL_SetGPUViewport(pass, &viewport);
                if with_picture {
                    let binding = SDL_GPUTextureSamplerBinding { texture: e.texture, sampler: self.flat_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                }
                let u = uniforms(e);
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const u).cast(), size_of::<EmbedUniforms>() as u32);
                SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            }
            SDL_EndGPURenderPass(pass);
        }
    }

    /// VPX's look: the embedded windows' places cleared in the table's picture, before the
    /// occlusion and the bloom read it (VPX's `ClearEmbeddedAncillaryWindow`; marked for
    /// tonemapping, as VPX's clear writes depth there).
    pub(super) unsafe fn record_embedded_clear(&self, command: *mut SDL_GPUCommandBuffer, size: (u32, u32), window: (u32, u32)) {
        let Some(p) = &self.embed_pipelines else { return };
        let black = |_: &Embedded| EmbedUniforms { fill: [0.0, 0.0, 0.0, 1.0], mode: [0.0; 4] };
        // SAFETY: as for the frame
        unsafe { self.embed_pass(command, self.hdr_target, size, window, p.fill_hdr, black, false) };
    }

    /// VPX's look: no occlusion at the embedded windows' places (VPX clears the depth there).
    pub(super) unsafe fn record_embedded_ao(&self, command: *mut SDL_GPUCommandBuffer, ao: *mut SDL_GPUTexture, size: (u32, u32), window: (u32, u32)) {
        let Some(p) = &self.embed_pipelines else { return };
        let none = |_: &Embedded| EmbedUniforms { fill: [1.0; 4], mode: [0.0; 4] };
        // SAFETY: as for the frame
        unsafe { self.embed_pass(command, ao, size, window, p.fill_ao, none, false) };
    }

    /// The embedded windows' pictures taken into the frame: in VPX's look into the table's
    /// picture before its tonemapping, made linear (VPX: `RenderAncillaryWindow` after the bloom,
    /// before the post-processing); in the first look over the finished picture in `target`.
    pub(super) unsafe fn record_embedded_copy(&self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, size: (u32, u32), window: (u32, u32)) {
        let Some(p) = &self.embed_pipelines else { return };
        let (texture, pipeline, encode) = if self.hdr { (self.hdr_target, p.copy_hdr, false) } else { (target, p.copy_out, holds_srgb_values(self.format)) };
        let mode = |e: &Embedded| EmbedUniforms {
            fill: [0.0; 4],
            mode: [if holds_srgb_values(e.format) { 1.0 } else { 0.0 }, if encode { 1.0 } else { 0.0 }, 1.0, 0.0],
        };
        // SAFETY: as for the frame
        unsafe { self.embed_pass(command, texture, size, window, pipeline, mode, true) };
    }

    /// Releases the embedded windows' pictures and pipelines (the renderer's `Drop`).
    pub(super) fn release_embedded(&mut self) {
        for e in self.embedded.iter_mut().filter_map(Option::take) {
            // SAFETY: ours, the device idle
            unsafe { SDL_ReleaseGPUTexture(self.device, e.texture) };
        }
        if let Some(p) = self.embed_pipelines.take() {
            for pipeline in [p.fill_hdr, p.fill_ao, p.copy_hdr, p.copy_out] {
                // SAFETY: as above
                unsafe { SDL_ReleaseGPUGraphicsPipeline(self.device, pipeline) };
            }
        }
    }
}
