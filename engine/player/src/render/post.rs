// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its post-processed
// antialiasing and sharpening (src/renderer/Renderer.cpp: ApplyPostProcessedAntialiasing, ApplySharpening and the
// post-processing pictures of GetPostProcessRenderTarget; src/core/Settings_properties.inl: its FXAA and Sharpen
// settings), with SMAA's lookup pictures as src/renderer/RenderDevice.cpp makes them.
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's post-processed antialiasing and sharpening (VPX's
//! smoothing settings): settings, all off by default as in VPX.
//! With one on, the tonemapping pass writes an 8-bit picture of its own
//! (VPX's post-processing picture) instead of the window; the antialiasing
//! pass, then the sharpening pass, follow in VPX's order, the last writing
//! the window, and the readout goes over that.
//!
//! All of VPX's: FXAA in three qualities, NFAA, DLAA, SMAA and FAAA; CAS and
//! bilateral CAS. FXAA's and FAAA's code isn't VPX's own — NVIDIA's FXAA
//! 3.11 and G. Rebord's FAAA, their licences not stated — and is ported as
//! VPX ships it.

use super::*;

/// VPX's post-processed antialiasing (its `FXAA` setting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PostAa {
    #[default]
    Off,
    /// VPX's "Fast FXAA": its own approximation of FXAA.
    FxaaFast,
    /// VPX's "Standard FXAA": NVIDIA's FXAA 3.11, mid quality.
    Fxaa,
    /// VPX's "Quality FXAA": NVIDIA's FXAA 3.11, extreme quality.
    FxaaQuality,
    /// VPX's "Fast NFAA" (normal filter antialiasing).
    Nfaa,
    /// VPX's "Standard DLAA" (directionally localized antialiasing).
    Dlaa,
    /// VPX's "Quality SMAA" (enhanced subpixel morphological antialiasing).
    Smaa,
    /// VPX's "Quality FAAA": G. Rebord's FAAA, after FXAA 3.11.
    Faaa,
}

/// VPX's post-processed sharpening (its `Sharpen` setting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sharpen {
    #[default]
    Off,
    /// AMD's contrast adaptive sharpening, as VPX has it.
    Cas,
    /// VPX's bilateral CAS.
    BilateralCas,
}

const POST_HLSL: &str = concat!(include_str!("post_common.hlsl"), include_str!("post.hlsl"));
const SMAA_HLSL: &str = concat!(include_str!("post_common.hlsl"), include_str!("smaa.hlsl"), include_str!("smaa_passes.hlsl"));
/// SMAA's lookup pictures (MIT, engine/player/assets/smaa).
const SMAA_AREA: &[u8] = include_bytes!("../../assets/smaa/area.bin");
const SMAA_SEARCH: &[u8] = include_bytes!("../../assets/smaa/search.bin");

/// VPX's post-processing pictures: 8 bits a channel, sRGB values as they are.
const POST_FORMAT: SDL_GPUTextureFormat = SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM;

/// The passes after tonemapping: their pictures, and their pipelines for
/// writing a post-processing picture ([`POST_FORMAT`]) or the window.
pub(super) struct Post {
    size: (u32, u32),
    /// Two post-processing pictures, written in turn.
    pictures: [*mut SDL_GPUTexture; 2],
    /// SMAA's edges and blending weights.
    smaa: [*mut SDL_GPUTexture; 2],
    /// SMAA's area and search pictures.
    tables: [*mut SDL_GPUTexture; 2],
    /// The tonemapping pass into a post-processing picture.
    pub(super) tonemap: *mut SDL_GPUGraphicsPipeline,
    /// Each pass's pipelines: into a post-processing picture, into the window.
    passes: Vec<(&'static CStr, [*mut SDL_GPUGraphicsPipeline; 2])>,
}

/// One pass of the chain: its shader source and entry points, how many
/// pictures it reads, and whether its vertex shader takes uniforms (SMAA's).
struct PassShader {
    source: &'static str,
    vs: &'static CStr,
    fs: &'static CStr,
    samplers: u32,
    vs_uniforms: bool,
}

const fn filter(fs: &'static CStr) -> PassShader {
    PassShader { source: POST_HLSL, vs: c"post_vs", fs, samplers: 2, vs_uniforms: false }
}

const fn smaa(vs: &'static CStr, fs: &'static CStr, samplers: u32) -> PassShader {
    PassShader { source: SMAA_HLSL, vs, fs, samplers, vs_uniforms: true }
}

const FXAA_FAST: PassShader = filter(c"fxaa1_fs");
const FXAA: PassShader = filter(c"fxaa2_fs");
const FXAA_QUALITY: PassShader = filter(c"fxaa3_fs");
const FAAA: PassShader = filter(c"faaa_fs");
const NFAA: PassShader = filter(c"nfaa_fs");
const DLAA_EDGE: PassShader = filter(c"dlaa_edge_fs");
const DLAA: PassShader = filter(c"dlaa_fs");
const CAS: PassShader = filter(c"cas_fs");
const BILATERAL_CAS: PassShader = filter(c"bilateral_cas_fs");
const SMAA_EDGE: PassShader = smaa(c"smaa_edge_vs", c"smaa_edge_fs", 2);
const SMAA_WEIGHT: PassShader = smaa(c"smaa_weight_vs", c"smaa_weight_fs", 3);
const SMAA_BLEND: PassShader = smaa(c"smaa_blend_vs", c"smaa_blend_fs", 2);

/// A pass's fragment uniforms (`post_common.hlsl`'s `Post`).
#[repr(C)]
#[derive(Clone, Copy)]
struct PostUniforms {
    /// VPX's `w_h_height`: 1 / the picture's size, its width (or height, for SMAA), and 1 when its depth is read.
    w_h_height: [f32; 4],
    /// x: decoded to linear light (the window encodes sRGB itself).
    flags: [f32; 4],
}

impl Renderer {
    /// VPX's post-processed antialiasing and sharpening from now on (VPX's
    /// look only: the first look has no tonemapping pass to follow).
    pub fn set_post(&mut self, aa: PostAa, sharpen: Sharpen) {
        self.post_aa = aa;
        self.post_sharpen = sharpen;
    }

    /// Whether the frame has passes after tonemapping.
    pub(super) fn post_on(&self) -> bool {
        self.hdr && (self.post_aa != PostAa::Off || self.post_sharpen != Sharpen::Off)
    }

    /// Whether those passes read the frame's depth, to leave untouched what
    /// isn't the table (VPX's early out): not under multisampling, whose
    /// depth SDL can't read.
    pub(super) fn post_reads_depth(&self) -> bool {
        self.post_on() && self.samples == SDL_GPU_SAMPLECOUNT_1
    }

    /// The passes' shaders, in the order they run.
    fn post_shaders(&self) -> Vec<&'static PassShader> {
        let mut list: Vec<&'static PassShader> = match self.post_aa {
            PostAa::Off => Vec::new(),
            PostAa::FxaaFast => vec![&FXAA_FAST],
            PostAa::Fxaa => vec![&FXAA],
            PostAa::FxaaQuality => vec![&FXAA_QUALITY],
            PostAa::Faaa => vec![&FAAA],
            PostAa::Nfaa => vec![&NFAA],
            PostAa::Dlaa => vec![&DLAA_EDGE, &DLAA],
            PostAa::Smaa => vec![&SMAA_EDGE, &SMAA_WEIGHT, &SMAA_BLEND],
        };
        match self.post_sharpen {
            Sharpen::Off => {}
            Sharpen::Cas => list.push(&CAS),
            Sharpen::BilateralCas => list.push(&BILATERAL_CAS),
        }
        list
    }

    /// Makes the passes' pictures for a window of `width` × `height`, and
    /// their pipelines and SMAA's pictures the first time.
    pub(super) fn ensure_post(&mut self, width: u32, height: u32) -> Result<(), String> {
        if !self.post_on() {
            return Ok(());
        }
        if self.post.is_none() {
            let mut p = Post { size: (0, 0), pictures: [null_mut(); 2], smaa: [null_mut(); 2], tables: [null_mut(); 2], tonemap: null_mut(), passes: Vec::new() };
            let made = (|| -> Result<(), String> {
                p.tonemap = self.tonemap_pipeline_for(POST_FORMAT)?;
                for shader in self.post_shaders() {
                    p.passes.push((shader.fs, self.pass_pipelines(shader)?));
                }
                if self.post_aa == PostAa::Smaa {
                    p.tables[0] = self.picture_of(SDL_GPU_TEXTUREFORMAT_R8G8_UNORM, 160, 560, SMAA_AREA, false, "upload SMAA's area picture")?;
                    p.tables[1] = self.picture_of(SDL_GPU_TEXTUREFORMAT_R8_UNORM, 64, 16, SMAA_SEARCH, false, "upload SMAA's search picture")?;
                }
                Ok(())
            })();
            if let Err(e) = made {
                self.release_post_parts(&mut p);
                return Err(e);
            }
            self.post = Some(p);
        }
        if self.post.as_ref().is_some_and(|p| p.size == (width, height)) {
            return Ok(());
        }
        let mut p = self.post.take().expect("made above");
        self.release_post_pictures(&mut p);
        let usage = SDL_GPU_TEXTUREUSAGE_COLOR_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER;
        let made = (|| -> Result<(), String> {
            for k in 0..2 {
                p.pictures[k] = self.texture(POST_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            }
            if self.post_aa == PostAa::Smaa {
                for k in 0..2 {
                    p.smaa[k] = self.texture(POST_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
                }
            }
            Ok(())
        })();
        p.size = (width, height);
        self.post = Some(p);
        made
    }

    /// A pass's pipelines: writing a post-processing picture, and the window.
    fn pass_pipelines(&self, shader: &PassShader) -> Result<[*mut SDL_GPUGraphicsPipeline; 2], String> {
        let vs = self.shader(shader.source, shader.vs, c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, if shader.vs_uniforms { 1 } else { 0 })?;
        let fs = match self.shader(shader.source, shader.fs, c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, shader.samplers, 1) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        let mut out = [null_mut(); 2];
        for (k, format) in [POST_FORMAT, self.format].into_iter().enumerate() {
            out[k] = unsafe { self.fullscreen_pipeline(vs, fs, format) };
        }
        unsafe {
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
        }
        if out.iter().any(|p| p.is_null()) {
            for p in out.into_iter().filter(|p| !p.is_null()) {
                unsafe { SDL_ReleaseGPUGraphicsPipeline(self.device, p) };
            }
            return Err(format!("create a post-processing pipeline: {}", sdl_error()));
        }
        Ok(out)
    }

    /// A pipeline drawing one triangle over a picture of `format`: no
    /// vertex buffer, no depth, no blending.
    unsafe fn fullscreen_pipeline(&self, vs: *mut SDL_GPUShader, fs: *mut SDL_GPUShader, format: SDL_GPUTextureFormat) -> *mut SDL_GPUGraphicsPipeline {
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
            SDL_CreateGPUGraphicsPipeline(self.device, &info)
        }
    }

    /// The pipelines of the pass whose fragment shader is `fs`.
    fn pass(&self, fs: &CStr) -> [*mut SDL_GPUGraphicsPipeline; 2] {
        self.post.as_ref().and_then(|p| p.passes.iter().find(|(f, _)| *f == fs)).map_or([null_mut(); 2], |(_, p)| *p)
    }

    /// Where the tonemapping pass writes, and its pipeline: the first
    /// post-processing picture when passes follow, else `target`.
    pub(super) fn tonemap_target(&self, target: *mut SDL_GPUTexture) -> (*mut SDL_GPUTexture, *mut SDL_GPUGraphicsPipeline) {
        match self.post.as_ref().filter(|p| self.post_on() && !p.pictures[0].is_null()) {
            Some(p) => (p.pictures[0], p.tonemap),
            None => (target, self.tonemap),
        }
    }

    /// Runs the passes after tonemapping on `command`, the tonemapped
    /// picture in the first post-processing picture, the last into
    /// `target` (`width` × `height`); returns the last pass, still open, for
    /// the readout to go over.
    pub(super) unsafe fn record_post(&self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32) -> *mut SDL_GPURenderPass {
        let Some(p) = self.post.as_ref() else { return null_mut() };
        let (w, h) = (width as f32, height as f32);
        let srgb_target = self.format == SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM_SRGB || self.format == SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB;
        let reads_depth = self.post_reads_depth();
        let depth = if reads_depth { self.depth } else { self.white };
        // VPX's w_h_height for the filters: 1 / the size, the width, 1 when the depth is read; SMAA's metrics
        let filters = [1.0 / w, 1.0 / h, w, if reads_depth { 1.0 } else { 0.0 }];
        let metrics = [1.0 / w, 1.0 / h, w, h];
        let sharpen = self.post_sharpen != Sharpen::Off;
        // the passes in VPX's order: (shader, pictures read, picture written; None: the window), cleared first
        let [first, second] = p.pictures;
        let mut steps: Vec<(&CStr, [*mut SDL_GPUTexture; 3], Option<*mut SDL_GPUTexture>, bool)> = Vec::new();
        let mut last = first;
        let mut spare = second;
        // the single-pass filters: the picture (and its depth) into the other picture, or the window
        let single = match self.post_aa {
            PostAa::FxaaFast => Some(FXAA_FAST.fs),
            PostAa::Fxaa => Some(FXAA.fs),
            PostAa::FxaaQuality => Some(FXAA_QUALITY.fs),
            PostAa::Faaa => Some(FAAA.fs),
            PostAa::Nfaa => Some(NFAA.fs),
            _ => None,
        };
        if let Some(fs) = single {
            steps.push((fs, [last, depth, null_mut()], sharpen.then_some(spare), false));
            std::mem::swap(&mut last, &mut spare);
        }
        match self.post_aa {
            PostAa::Off | PostAa::FxaaFast | PostAa::Fxaa | PostAa::FxaaQuality | PostAa::Faaa | PostAa::Nfaa => {}
            PostAa::Dlaa => {
                // the edges into the alpha of the second picture, then the blend from it
                steps.push((DLAA_EDGE.fs, [last, depth, null_mut()], Some(spare), false));
                steps.push((DLAA.fs, [spare, depth, null_mut()], sharpen.then_some(last), false));
            }
            PostAa::Smaa => {
                steps.push((SMAA_EDGE.fs, [last, last, null_mut()], Some(p.smaa[0]), true));
                steps.push((SMAA_WEIGHT.fs, [p.smaa[0], p.tables[0], p.tables[1]], Some(p.smaa[1]), false));
                steps.push((SMAA_BLEND.fs, [last, p.smaa[1], null_mut()], sharpen.then_some(spare), false));
                std::mem::swap(&mut last, &mut spare);
            }
        }
        match self.post_sharpen {
            Sharpen::Off => {}
            Sharpen::Cas => steps.push((CAS.fs, [last, depth, null_mut()], None, false)),
            Sharpen::BilateralCas => steps.push((BILATERAL_CAS.fs, [last, depth, null_mut()], None, false)),
        }
        let mut open = null_mut();
        unsafe {
            for (n, &(fs, pictures, written, clear)) in steps.iter().enumerate() {
                let into_window = written.is_none();
                let mut info: SDL_GPUColorTargetInfo = zeroed();
                info.texture = written.unwrap_or(target);
                info.load_op = if clear { SDL_GPU_LOADOP_CLEAR } else { SDL_GPU_LOADOP_DONT_CARE };
                info.clear_color = SDL_FColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
                info.store_op = SDL_GPU_STOREOP_STORE;
                let pass = SDL_BeginGPURenderPass(command, &info, 1, null());
                SDL_BindGPUGraphicsPipeline(pass, self.pass(fs)[if into_window { 1 } else { 0 }]);
                // linear first, point second (post_common.hlsl's samplers), whatever the pictures
                let samplers = [self.flat_sampler, self.font_sampler, self.font_sampler];
                let bindings: Vec<SDL_GPUTextureSamplerBinding> = pictures
                    .iter()
                    .zip(samplers)
                    .take_while(|(t, _)| !t.is_null())
                    .map(|(&texture, sampler)| SDL_GPUTextureSamplerBinding { texture, sampler })
                    .collect();
                SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), bindings.len() as u32);
                let smaa = fs == SMAA_EDGE.fs || fs == SMAA_WEIGHT.fs || fs == SMAA_BLEND.fs;
                if smaa {
                    SDL_PushGPUVertexUniformData(command, 0, metrics.as_ptr().cast(), 16);
                }
                let uniforms = PostUniforms { w_h_height: if smaa { metrics } else { filters }, flags: [if into_window && srgb_target { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0] };
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<PostUniforms>() as u32);
                SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
                if n + 1 == steps.len() {
                    open = pass;
                } else {
                    SDL_EndGPURenderPass(pass);
                }
            }
        }
        open
    }

    fn release_post_pictures(&self, p: &mut Post) {
        unsafe {
            for t in p.pictures.iter_mut().chain(p.smaa.iter_mut()) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, *t);
                }
                *t = null_mut();
            }
        }
        p.size = (0, 0);
    }

    fn release_post_parts(&self, p: &mut Post) {
        self.release_post_pictures(p);
        unsafe {
            for t in p.tables.iter_mut() {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, *t);
                }
                *t = null_mut();
            }
            for pipeline in std::iter::once(p.tonemap).chain(p.passes.drain(..).flat_map(|(_, ps)| ps)) {
                if !pipeline.is_null() {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, pipeline);
                }
            }
        }
        p.tonemap = null_mut();
    }

    /// Releases the passes' pictures and pipelines (the renderer's `Drop`).
    pub(super) fn release_post(&mut self) {
        if let Some(mut p) = self.post.take() {
            self.release_post_parts(&mut p);
        }
    }
}
