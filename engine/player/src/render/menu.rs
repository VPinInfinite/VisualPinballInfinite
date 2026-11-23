//! The pause menu over the table (`pause_menu.rs`), ours: drawn into the
//! window's finished frame after everything else — the table dimmed, then the menu's picture at its
//! place, turned with the table on a turned screen as the readout is, and encoded for VPX 5436's
//! HDR10 output on a screen with HDR on as the readout is. The picture is made by the window
//! thread only when the menu changes (`pause_menu::picture`), and uploaded then; test mode's
//! pictures show it too.

use std::mem::{size_of, zeroed};
use std::ptr::null_mut;

use sdl3_sys::everything::*;

use super::{Renderer, non_null};

const MENU_HLSL: &str = include_str!("menu.hlsl");

/// The menu's picture: RGBA with straight alpha, sRGB values, laid out as the player sees the
/// screen, and where it goes.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuPicture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    /// Its left and top, in pixels, as the player sees the screen.
    pub at: (f32, f32),
    /// How dark the table goes behind it, 0 to 1.
    pub dim: f32,
}

/// An achievement's announcement on the graphics card: its picture, and a pipeline for each
/// window format it's laid over.
pub(super) struct Toast {
    texture: *mut SDL_GPUTexture,
    size: (u32, u32),
    at: (f32, f32),
    sampler: *mut SDL_GPUSampler,
    pipelines: Vec<(SDL_GPUTextureFormat, *mut SDL_GPUGraphicsPipeline)>,
}

/// The menu on the graphics card.
pub(super) struct Menu {
    pipeline: *mut SDL_GPUGraphicsPipeline,
    sampler: *mut SDL_GPUSampler,
    texture: *mut SDL_GPUTexture,
    size: (u32, u32),
    at: (f32, f32),
    dim: f32,
    shown: bool,
}

#[repr(C)]
struct MenuPlace {
    place: [f32; 4],
    window: [f32; 4],
}

impl Renderer {
    /// The pause menu's picture from now on, or none: uploaded once, drawn over every frame
    /// while it shows.
    pub fn set_menu(&mut self, picture: Option<&MenuPicture>) -> Result<(), String> {
        let Some(p) = picture else {
            if let Some(m) = &mut self.menu {
                m.shown = false;
            }
            return Ok(());
        };
        if p.width == 0 || p.height == 0 || p.pixels.len() != (p.width * p.height * 4) as usize {
            return Ok(());
        }
        if self.menu.is_none() {
            self.menu = Some(self.menu_layer()?);
        }
        let texture = self.upload_picture(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM, p.width, p.height, &p.pixels)?;
        let m = self.menu.as_mut().expect("made above");
        if !m.texture.is_null() {
            // SAFETY: ours; the frames drawn with it are on the device's queue, which SDL keeps
            // it for
            unsafe { SDL_ReleaseGPUTexture(self.device, m.texture) };
        }
        (m.texture, m.size, m.at, m.dim, m.shown) = (texture, (p.width, p.height), p.at, p.dim, true);
        Ok(())
    }

    /// An achievement's announcement over the backglass window from now on, or none: the window
    /// drawn again as it comes and goes.
    pub fn set_toast(&mut self, picture: Option<&MenuPicture>) -> Result<(), String> {
        self.toast_count += 1;
        let Some(p) = picture.filter(|p| p.width > 0 && p.height > 0 && p.pixels.len() == (p.width * p.height * 4) as usize) else {
            if let Some(t) = &mut self.toast
                && !t.texture.is_null()
            {
                // SAFETY: ours; frames drawn with it are on the queue, which SDL keeps it for
                unsafe { SDL_ReleaseGPUTexture(self.device, t.texture) };
                t.texture = null_mut();
            }
            return Ok(());
        };
        let texture = self.upload_picture(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM, p.width, p.height, &p.pixels)?;
        if self.toast.is_none() {
            let sampler = self.sampler(SDL_GPU_FILTER_NEAREST, SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE, false)?;
            self.toast = Some(Toast { texture: null_mut(), size: (0, 0), at: (0.0, 0.0), sampler, pipelines: Vec::new() });
        }
        let t = self.toast.as_mut().expect("made above");
        if !t.texture.is_null() {
            // SAFETY: as above
            unsafe { SDL_ReleaseGPUTexture(self.device, t.texture) };
        }
        (t.texture, t.size, t.at) = (texture, (p.width, p.height), p.at);
        Ok(())
    }

    /// How many announcements have come and gone: a window shows the latest once it's drawn again.
    pub(super) fn toast_count(&self) -> u64 {
        self.toast_count
    }

    /// The announcement laid over a window's drawing in `target` (`width` × `height`, `format`),
    /// when one shows.
    ///
    /// # Safety
    /// On the renderer's thread, with `command` recording into `target`.
    pub(super) unsafe fn record_toast(&mut self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32, format: SDL_GPUTextureFormat) {
        if self.toast.as_ref().is_none_or(|t| t.texture.is_null()) {
            return;
        }
        let pipeline = match self.toast.as_ref().and_then(|t| t.pipelines.iter().find(|(f, _)| *f == format).map(|(_, p)| *p)) {
            Some(p) => p,
            None => match self.overlay_pipeline(format, false) {
                Ok(p) => {
                    if let Some(t) = &mut self.toast {
                        t.pipelines.push((format, p));
                    }
                    p
                }
                Err(e) => {
                    crate::script::log::once(&format!("an achievement's announcement: {e}"));
                    return;
                }
            },
        };
        let Some(t) = self.toast.as_ref() else { return };
        // SAFETY: as the caller promises; the picture and pipeline are ours
        unsafe {
            let mut color: SDL_GPUColorTargetInfo = zeroed();
            color.texture = target;
            color.load_op = SDL_GPU_LOADOP_LOAD;
            color.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &color, 1, null_mut());
            if pass.is_null() {
                return;
            }
            SDL_BindGPUGraphicsPipeline(pass, pipeline);
            let place = MenuPlace { place: [t.at.0, t.at.1, t.size.0 as f32, t.size.1 as f32], window: [width as f32, height as f32, 0.0, 1.0] };
            SDL_PushGPUVertexUniformData(command, 0, (&raw const place).cast(), size_of::<MenuPlace>() as u32);
            let linear = !super::embed::holds_srgb_values(format);
            let look = [0.0, if linear { 1.0 } else { 0.0 }, 0.0, 0.0f32];
            SDL_PushGPUFragmentUniformData(command, 0, look.as_ptr().cast(), 16);
            let binding = SDL_GPUTextureSamplerBinding { texture: t.texture, sampler: t.sampler };
            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
            // the picture's quad alone
            SDL_DrawGPUPrimitives(pass, 6, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
        }
    }

    /// A pipeline laying a picture over a window's drawing in `format` (the menu's, an
    /// announcement's), encoding for VPX 5436's HDR10 output where `pq`.
    fn overlay_pipeline(&self, format: SDL_GPUTextureFormat, pq: bool) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(MENU_HLSL, c"menu_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 1)?;
        let fs = match self.hdr10.filter(|_| pq) {
            Some(h) => {
                let source = format!("#define MENU_PQ_WHITE {:.9}\n{MENU_HLSL}", h.sdr_white / 10000.0);
                self.shader(&source, c"menu_pq_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)
            }
            None => self.shader(MENU_HLSL, c"menu_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1),
        };
        let fs = match fs {
            Ok(fs) => fs,
            Err(e) => {
                // SAFETY: made above, not used
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        // SAFETY: the device is live; the shaders are released once the pipeline holds them
        unsafe {
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
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create an overlay's pipeline")
        }
    }

    /// The menu's pipeline, into the window's own colours (the readout's blending), and its
    /// sampler: the picture is made for the window's pixels, so each texel lands on one.
    fn menu_layer(&self) -> Result<Menu, String> {
        let pipeline = self.overlay_pipeline(self.format, self.hdr10.is_some())?;
        // SAFETY: the device is live
        unsafe {
            let sampler = match self.sampler(SDL_GPU_FILTER_NEAREST, SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE, false) {
                Ok(s) => s,
                Err(e) => {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, pipeline);
                    return Err(e);
                }
            };
            Ok(Menu { pipeline, sampler, texture: null_mut(), size: (0, 0), at: (0.0, 0.0), dim: 0.0, shown: false })
        }
    }

    /// The menu over the finished frame in `target` (`width` × `height` pixels), when it shows;
    /// `turn`: the text's quarter turns with the table on a turned screen.
    ///
    /// # Safety
    /// On the renderer's thread, with `command` recording the frame into `target`.
    pub(super) unsafe fn record_menu(&self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32, turn: u32) {
        let Some(m) = self.menu.as_ref().filter(|m| m.shown && !m.texture.is_null()) else { return };
        // SAFETY: as the caller promises; the picture and pipeline are ours
        unsafe {
            let mut color: SDL_GPUColorTargetInfo = zeroed();
            color.texture = target;
            color.load_op = SDL_GPU_LOADOP_LOAD;
            color.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &color, 1, null_mut());
            if pass.is_null() {
                return;
            }
            SDL_BindGPUGraphicsPipeline(pass, m.pipeline);
            let place = MenuPlace { place: [m.at.0, m.at.1, m.size.0 as f32, m.size.1 as f32], window: [width as f32, height as f32, (turn % 4) as f32, 0.0] };
            SDL_PushGPUVertexUniformData(command, 0, (&raw const place).cast(), size_of::<MenuPlace>() as u32);
            // a target taking linear values (an sRGB picture) encodes the picture's sRGB values itself,
            // and blends in linear light: the dimming made as dark to the eye as in sRGB values
            let linear = !super::embed::holds_srgb_values(self.format) && self.hdr10.is_none();
            let dim = m.dim.clamp(0.0, 1.0);
            let dim = if linear { 1.0 - (1.0 - dim).powf(2.2) } else { dim };
            let look = [dim, if linear { 1.0 } else { 0.0 }, 0.0, 0.0f32];
            SDL_PushGPUFragmentUniformData(command, 0, look.as_ptr().cast(), 16);
            let binding = SDL_GPUTextureSamplerBinding { texture: m.texture, sampler: m.sampler };
            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
            SDL_DrawGPUPrimitives(pass, 12, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
        }
    }

    /// Releases the menu's picture and pipeline, and the announcement's (the renderer's `Drop`).
    pub(super) fn release_menu(&mut self) {
        if let Some(t) = self.toast.take() {
            // SAFETY: ours, the device idle
            unsafe {
                if !t.texture.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t.texture);
                }
                SDL_ReleaseGPUSampler(self.device, t.sampler);
                for (_, p) in t.pipelines {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, p);
                }
            }
        }
        if let Some(m) = self.menu.take() {
            // SAFETY: ours, the device idle
            unsafe {
                if !m.texture.is_null() {
                    SDL_ReleaseGPUTexture(self.device, m.texture);
                }
                SDL_ReleaseGPUSampler(self.device, m.sampler);
                SDL_ReleaseGPUGraphicsPipeline(self.device, m.pipeline);
            }
        }
    }
}
