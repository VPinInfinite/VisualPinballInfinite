// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its ambient occlusion
// (src/renderer/Renderer.cpp: GetAOMode, UpdateAmbientOcclusion, the static AO of RenderStaticPrepass,
// GetAORenderTarget; src/core/def.h: radical_inverse) and ball motion blur (Renderer::ApplyBallMotionBlur, with
// AntiStretchHelper::computeProjBounds from src/parts/ball.h), and their settings (src/core/Settings_properties.inl:
// DisableAO, DynamicAO, ForceMotionBlurOff).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Two of VPX's effects that are on by default, so part of its look, each
//! with a setting to turn it off:
//!
//! - **Ambient occlusion**: corners and creases darkened, where light from the
//!   room reaches less — for tables that ask for it (`EnableAO`, with their
//!   `AOScale`). Worked out every frame from the depth, blended with the last
//!   frames' (VPX's default, dynamic), or once, 50 times over, into the still
//!   parts' picture (static).
//! - **Ball motion blur** (VPX's BGFX builds): a ball that moved since the last
//!   frame is drawn smeared along its path, raytraced against this frame's
//!   picture and the last one's, over the ball's own part of the picture.
//!
//! Both read the frame's depth, which SDL can't read under multisampling: with
//! multisampling on, neither shows (VPX resolves its depth).
//!
//! VPX's ambient occlusion jitters its samples by a noise picture of its own,
//! from before VPX's GPL rule: ours is computed, from Chris Wellons's
//! public-domain `lowbias32` hash, as the tonemapping's dither is.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use super::*;

/// VPX's ambient occlusion setting (its `DisableAO` and `DynamicAO`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmbientOcclusion {
    /// None (VPX's `DisableAO`).
    Off,
    /// Once, into the still parts' picture (`DynamicAO` off).
    Static,
    /// Every frame (VPX's default).
    #[default]
    Dynamic,
}

const EFFECTS_HLSL: &str = include_str!("effects.hlsl");

/// The occlusion's pictures: one byte a texel (VPX's `GREY8`).
pub(super) const AO_FORMAT: SDL_GPUTextureFormat = SDL_GPU_TEXTUREFORMAT_R8_UNORM;

/// The passes' uniforms (`effects.hlsl`'s `Effects`).
#[repr(C)]
#[derive(Clone, Copy)]
struct EffectUniforms {
    m0: [[f32; 4]; 4],
    m1: [[f32; 4]; 4],
    v0: [f32; 4],
    v1: [f32; 4],
    v2: [f32; 4],
    v3: [f32; 4],
}

impl EffectUniforms {
    fn new(v0: [f32; 4], v1: [f32; 4]) -> EffectUniforms {
        EffectUniforms { m0: Mat4::IDENTITY.m, m1: Mat4::IDENTITY.m, v0, v1, v2: [0.0; 4], v3: [0.0; 4] }
    }
}

/// The effects' pictures and pipelines.
pub(super) struct Effects {
    /// The table's picture's size they were made for.
    size: (u32, u32),
    /// The occlusion, two pictures in turn: the newest and the one before.
    ao: [*mut SDL_GPUTexture; 2],
    pub(super) ao_latest: Cell<usize>,
    /// Our noise for the occlusion's samples, 64 x 64.
    noise: *mut SDL_GPUTexture,
    /// The blurred balls' parts of the picture, before tonemapping.
    pub(super) blur: *mut SDL_GPUTexture,
    ao_pipeline: *mut SDL_GPUGraphicsPipeline,
    apply_pipeline: *mut SDL_GPUGraphicsPipeline,
    blur_pipeline: *mut SDL_GPUGraphicsPipeline,
    /// The noise's sampler: its own texels, repeated.
    repeat_point: *mut SDL_GPUSampler,
    /// Each ball's place when last drawn, in the camera's space (VPX's
    /// `m_lastRenderedPos`), by its number.
    last_drawn: RefCell<HashMap<u32, Vec3>>,
}

/// A blurred ball's part of the picture: left, top, right and bottom, from -1
/// to 1 across and up (the camera's projection).
pub(super) type BlurRegion = [f32; 4];

/// VPX's `radical_inverse`: the number's bits reversed, as a fraction.
fn radical_inverse(i: u32) -> f32 {
    let u = i.reverse_bits();
    (u & !(u >> 24)) as f32 * 2.328_306_4e-10
}

/// VPX's `radical_inverse<3>`: the number's base-3 digits reversed, as a fraction.
fn radical_inverse3(mut a: u32) -> f32 {
    let inv_base = (1.0f64 / 3.0) as f32;
    let (mut reversed, mut inv_base_n) = (0u32, 1.0f32);
    while a != 0 {
        let next = a / 3;
        reversed = reversed.wrapping_mul(3).wrapping_add(a - next * 3);
        inv_base_n *= inv_base;
        a = next;
    }
    (f64::from(reversed) * f64::from(inv_base_n)) as f32
}

/// Chris Wellons's `lowbias32` (public domain), our noise's source.
fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// The occlusion's noise: 64 x 64 texels, three independent values each.
fn noise_texels() -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 64 * 4);
    for n in 0..64 * 64u32 {
        for channel in 0..3u32 {
            out.push((lowbias32(n * 3 + channel + 0x9e37_79b9) >> 24) as u8);
        }
        out.push(255);
    }
    out
}

/// VPX's points on a ball's surface for its bounds on screen
/// (`AntiStretchHelper`: 35 points spread evenly; any it doesn't place stay
/// at the centre).
fn sphere_points() -> [[f32; 3]; 35] {
    let mut out = [[0.0f32; 3]; 35];
    let a = 4.0 * std::f64::consts::PI / 35.0;
    let d = a.sqrt();
    let n_theta = (std::f64::consts::PI / d).round() as i32;
    let d_theta = std::f64::consts::PI / f64::from(n_theta);
    let d_phi = a / d_theta;
    let mut pos = 0;
    for j in 0..n_theta {
        let theta = (f64::from(j) + 0.5) * std::f64::consts::PI / f64::from(n_theta);
        let n_phi = (2.0 * std::f64::consts::PI * theta.sin() / d_phi).round() as i32;
        for i in 0..n_phi {
            let phi = f64::from(i) * (2.0 * std::f64::consts::PI) / f64::from(n_phi);
            if pos < 35 {
                out[pos] = [(theta.sin() * phi.cos()) as f32, (theta.sin() * phi.sin()) as f32, theta.cos() as f32];
            }
            pos += 1;
        }
    }
    out
}

/// VPX's `computeProjBounds`: a ball's bounds on screen, through the
/// projection, widening `bounds` (left, right, bottom, top, from -1 to 1).
fn widen_bounds(proj: &Mat4, points: &[[f32; 3]; 35], centre: Vec3, radius: f32, bounds: &mut [f32; 4]) {
    let m = &proj.m;
    for p in points {
        let (px, py, pz) = (centre.x + radius * p[0], centre.y + radius * p[1], centre.z + radius * p[2]);
        let wp = m[0][3] * px + m[1][3] * py + m[2][3] * pz + m[3][3];
        if wp <= 1e-3 {
            return;
        }
        let xp = (m[0][0] * px + m[1][0] * py + m[2][0] * pz + m[3][0]) / wp;
        let yp = (m[0][1] * px + m[1][1] * py + m[2][1] * pz + m[3][1]) / wp;
        bounds[0] = bounds[0].min(xp);
        bounds[1] = bounds[1].max(xp);
        bounds[2] = bounds[2].min(yp);
        bounds[3] = bounds[3].max(yp);
    }
}

impl Effects {
    /// The newest occlusion.
    pub(super) fn ao_latest_picture(&self) -> *mut SDL_GPUTexture {
        self.ao[self.ao_latest.get()]
    }
}

impl Renderer {
    /// VPX's ambient occlusion setting from now on.
    pub fn set_ambient_occlusion(&mut self, mode: AmbientOcclusion) {
        self.ao_setting = mode;
    }

    /// VPX's ball motion blur on or off from now on (its `ForceMotionBlurOff`).
    pub fn set_motion_blur(&mut self, on: bool) {
        self.motion_blur = on;
    }

    /// VPX's `GetAOMode`: 0 none, 1 static, 2 dynamic. None unless the table
    /// asks for it, nor in the first look, nor where the depth can't be read
    /// (multisampling); static only with the still parts drawn once.
    pub(super) fn ao_mode(&self) -> u8 {
        if self.tone.ao_scale == 0.0 || !self.hdr || self.samples != SDL_GPU_SAMPLECOUNT_1 {
            return 0;
        }
        match self.ao_setting {
            AmbientOcclusion::Off => 0,
            AmbientOcclusion::Static => u8::from(self.still_parts_once()),
            AmbientOcclusion::Dynamic => 2,
        }
    }

    /// Whether balls are blurred: VPX's look, the depth readable.
    pub(super) fn motion_blur_on(&self) -> bool {
        self.motion_blur && self.hdr && self.samples == SDL_GPU_SAMPLECOUNT_1
    }

    /// Makes the effects' pictures for a table's picture of `width` ×
    /// `height`, and their pipelines the first time.
    pub(super) fn ensure_effects(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self.ao_mode() == 0 && !self.motion_blur_on() {
            return Ok(());
        }
        if self.effects.is_none() {
            let mut e = Effects {
                size: (0, 0),
                ao: [null_mut(); 2],
                ao_latest: Cell::new(0),
                noise: null_mut(),
                blur: null_mut(),
                ao_pipeline: null_mut(),
                apply_pipeline: null_mut(),
                blur_pipeline: null_mut(),
                repeat_point: null_mut(),
                last_drawn: RefCell::new(HashMap::new()),
            };
            let made = (|| -> Result<(), String> {
                e.ao_pipeline = self.effect_pipeline(c"ao_fs", 3, AO_FORMAT)?;
                e.apply_pipeline = self.effect_pipeline(c"ao_apply_fs", 2, HDR_FORMAT)?;
                e.blur_pipeline = self.effect_pipeline(c"motion_blur_fs", 3, HDR_FORMAT)?;
                e.repeat_point = self.sampler(SDL_GPU_FILTER_NEAREST, SDL_GPU_SAMPLERADDRESSMODE_REPEAT, false)?;
                e.noise = self.unorm_picture(64, 64, &noise_texels(), false, "upload the occlusion's noise")?;
                Ok(())
            })();
            if let Err(err) = made {
                self.release_effect_parts(&mut e);
                return Err(err);
            }
            self.effects = Some(e);
        }
        if self.effects.as_ref().is_some_and(|e| e.size == (width, height)) {
            return Ok(());
        }
        let mut e = self.effects.take().expect("made above");
        self.release_effect_pictures(&mut e);
        let usage = SDL_GPU_TEXTUREUSAGE_COLOR_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER;
        let made = (|| -> Result<(), String> {
            for k in 0..2 {
                e.ao[k] = self.texture(AO_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            }
            e.blur = self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            // the occlusion starts from none
            unsafe {
                let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
                for &t in &e.ao {
                    let mut info: SDL_GPUColorTargetInfo = zeroed();
                    info.texture = t;
                    info.load_op = SDL_GPU_LOADOP_CLEAR;
                    info.clear_color = SDL_FColor { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
                    info.store_op = SDL_GPU_STOREOP_STORE;
                    SDL_EndGPURenderPass(SDL_BeginGPURenderPass(command, &info, 1, null()));
                }
                check(SDL_SubmitGPUCommandBuffer(command), "clear the occlusion")?;
            }
            Ok(())
        })();
        e.size = (width, height);
        e.last_drawn.borrow_mut().clear();
        self.effects = Some(e);
        made
    }

    /// One of the effects' pipelines: a triangle over a picture of `format`,
    /// reading `samplers` pictures.
    fn effect_pipeline(&self, entry: &CStr, samplers: u32, format: SDL_GPUTextureFormat) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(EFFECTS_HLSL, c"effect_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 0)?;
        let fs = match self.shader(EFFECTS_HLSL, entry, c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, samplers, 1) {
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
            non_null(pipeline, "create an effect's pipeline")
        }
    }

    /// One occlusion pass (VPX's `UpdateAmbientOcclusion`, and each of its
    /// static passes): from `depth`, a picture of `size`, blended with the
    /// newest result by `timeblur` (the new one's share), jittered by
    /// `jitter`; it becomes the newest.
    pub(super) unsafe fn record_ao(&self, command: *mut SDL_GPUCommandBuffer, depth: *mut SDL_GPUTexture, size: (u32, u32), jitter: [f32; 2], timeblur: f32) {
        let Some(e) = self.effects.as_ref() else { return };
        let (last, next) = (e.ao_latest.get(), 1 - e.ao_latest.get());
        unsafe {
            let mut info: SDL_GPUColorTargetInfo = zeroed();
            info.texture = e.ao[next];
            info.load_op = SDL_GPU_LOADOP_DONT_CARE;
            info.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &info, 1, null());
            SDL_BindGPUGraphicsPipeline(pass, e.ao_pipeline);
            let bindings = [
                SDL_GPUTextureSamplerBinding { texture: e.ao[last], sampler: self.flat_sampler },
                SDL_GPUTextureSamplerBinding { texture: depth, sampler: self.font_sampler },
                SDL_GPUTextureSamplerBinding { texture: e.noise, sampler: e.repeat_point },
            ];
            SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 3);
            let (w, h) = (size.0 as f32, size.1 as f32);
            let uniforms = EffectUniforms::new([1.0 / w, 1.0 / h, jitter[0], jitter[1]], [self.tone.ao_scale, timeblur, 0.0, 0.0]);
            SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<EffectUniforms>() as u32);
            SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
        }
        e.ao_latest.set(next);
    }

    /// VPX's dynamic occlusion for this frame (its `UpdateAmbientOcclusion`):
    /// the new result's share 0.4, jittered by the frame's number.
    pub(super) unsafe fn record_frame_ao(&self, command: *mut SDL_GPUCommandBuffer, size: (u32, u32)) {
        let frame = self.frames_drawn.get() % 2048;
        let jitter = [radical_inverse(frame) * (1.0 / 8.0), radical_inverse3(frame) * (1.0 / 8.0)];
        // SAFETY: as the caller's
        unsafe { self.record_ao(command, self.depth, size, jitter, 0.4) };
    }

    /// VPX's static occlusion (`RenderStaticPrepass`): 50 passes over the
    /// still parts' depth, each new one's share 0.1, then the still parts'
    /// picture darkened by it.
    pub(super) fn bake_still_ao(&self, size: (u32, u32)) -> Result<(), String> {
        let (Some(e), Some(s)) = (self.effects.as_ref(), self.still.as_ref()) else { return Ok(()) };
        unsafe {
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            // VPX's occlusion pictures start black
            for &t in &e.ao {
                let mut info: SDL_GPUColorTargetInfo = zeroed();
                info.texture = t;
                info.load_op = SDL_GPU_LOADOP_CLEAR;
                info.clear_color = SDL_FColor { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
                info.store_op = SDL_GPU_STOREOP_STORE;
                SDL_EndGPURenderPass(SDL_BeginGPURenderPass(command, &info, 1, null()));
            }
            for i in 0..50 {
                self.record_ao(command, s.depth, size, [radical_inverse(i) * (1.0 / 8.0), radical_inverse3(i) * (1.0 / 8.0)], 0.1);
            }
            // the picture darkened: copied aside, then drawn back through the occlusion
            let copy = SDL_BeginGPUCopyPass(command);
            let mut from: SDL_GPUTextureLocation = zeroed();
            from.texture = s.color;
            let mut to: SDL_GPUTextureLocation = zeroed();
            to.texture = s.scratch;
            SDL_CopyGPUTextureToTexture(copy, &from, &to, size.0, size.1, 1, false);
            SDL_EndGPUCopyPass(copy);
            let mut info: SDL_GPUColorTargetInfo = zeroed();
            info.texture = s.color;
            info.load_op = SDL_GPU_LOADOP_DONT_CARE;
            info.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &info, 1, null());
            SDL_BindGPUGraphicsPipeline(pass, e.apply_pipeline);
            let bindings = [
                SDL_GPUTextureSamplerBinding { texture: s.scratch, sampler: self.font_sampler },
                SDL_GPUTextureSamplerBinding { texture: e.ao[e.ao_latest.get()], sampler: self.flat_sampler },
            ];
            SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 2);
            let uniforms = EffectUniforms::new([1.0 / size.0 as f32, 1.0 / size.1 as f32, 0.0, 0.0], [0.0; 4]);
            SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<EffectUniforms>() as u32);
            SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
            check(SDL_SubmitGPUCommandBuffer(command), "darken the still parts")
        }
    }

    /// VPX's ball motion blur (`ApplyBallMotionBlur`), its first half: each
    /// ball that moved since it was last drawn, by more than a tenth of a
    /// unit and less than about 32, blurred along its path into the effects'
    /// picture, over its part of the picture (`size`). Returns those parts,
    /// for the tonemapping to draw again from that picture.
    pub(super) unsafe fn record_motion_blur(&self, command: *mut SDL_GPUCommandBuffer, input: &FrameInput, size: (u32, u32)) -> Vec<BlurRegion> {
        let Some(e) = self.effects.as_ref() else { return Vec::new() };
        let mut regions = Vec::new();
        // the camera's projection on its own (the frame's camera less its turn), and back
        let proj = input.view.inverse() * input.unshaken;
        let proj_inv = proj.inverse();
        let points = sphere_points();
        let (w, h) = (size.0 as f32, size.1 as f32);
        let (pad_x, pad_y) = (1.0 / w, 1.0 / h);
        let mut last_drawn = e.last_drawn.borrow_mut();
        let mut pass: *mut SDL_GPURenderPass = null_mut();
        for b in input.balls.iter().filter(|b| !b.locked) {
            if regions.len() >= 16 {
                break;
            }
            // discard still balls and those moved too far (likely made or moved by the script), VPX
            // judging from half a step ahead
            let ahead = Vec3::new(b.position.x + 0.5 * b.velocity.x, b.position.y + 0.5 * b.velocity.y, b.position.z + 0.5 * b.velocity.z);
            let ahead = input.view.transform_no_perspective(ahead);
            let Some(&last) = last_drawn.get(&b.id) else {
                last_drawn.insert(b.id, ahead);
                continue;
            };
            let (dx, dy, dz) = (ahead.x - last.x, ahead.y - last.y, ahead.z - last.z);
            let delta = dx * dx + dy * dy + dz * dz;
            if !(0.01..=1000.0).contains(&delta) {
                last_drawn.insert(b.id, ahead);
                continue;
            }
            // from where it was last drawn to where it's drawn now
            let now = input.view.transform_no_perspective(b.position);
            let mut bounds = [f32::MAX, -f32::MAX, f32::MAX, -f32::MAX];
            widen_bounds(&proj, &points, last, b.radius, &mut bounds);
            let prev_len = ((bounds[1] - bounds[0]) * w).hypot((bounds[3] - bounds[2]) * h);
            widen_bounds(&proj, &points, now, b.radius, &mut bounds);
            let full_len = ((bounds[1] - bounds[0]) * w).hypot((bounds[3] - bounds[2]) * h) - prev_len;
            let samples = ((0.5 * full_len) as i32).clamp(2, 32);
            let region = [bounds[0] - pad_x, bounds[3] + pad_y, bounds[1] + pad_x, bounds[2] - pad_y];
            last_drawn.insert(b.id, now);
            let Some(rect) = scissor_of(region, size) else { continue };
            unsafe {
                if pass.is_null() {
                    let mut info: SDL_GPUColorTargetInfo = zeroed();
                    info.texture = e.blur;
                    info.load_op = SDL_GPU_LOADOP_DONT_CARE;
                    info.store_op = SDL_GPU_STOREOP_STORE;
                    pass = SDL_BeginGPURenderPass(command, &info, 1, null());
                    SDL_BindGPUGraphicsPipeline(pass, e.blur_pipeline);
                    let bindings = [
                        SDL_GPUTextureSamplerBinding { texture: self.hdr_target, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.prev_hdr, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.depth, sampler: self.font_sampler },
                    ];
                    SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 3);
                }
                SDL_SetGPUScissor(pass, &rect);
                let uniforms = EffectUniforms {
                    m0: proj.m,
                    m1: proj_inv.m,
                    v0: [1.0 / w, 1.0 / h, 0.0, samples as f32],
                    v1: [now.x, now.y, now.z, b.radius],
                    v2: [last.x, last.y, last.z, b.radius],
                    v3: [0.0; 4],
                };
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<EffectUniforms>() as u32);
                SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            }
            regions.push(region);
        }
        if !pass.is_null() {
            unsafe { SDL_EndGPURenderPass(pass) };
        }
        regions
    }

    fn release_effect_pictures(&self, e: &mut Effects) {
        unsafe {
            for t in e.ao.iter_mut().chain([&mut e.blur]) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, *t);
                }
                *t = null_mut();
            }
        }
        e.size = (0, 0);
    }

    fn release_effect_parts(&self, e: &mut Effects) {
        self.release_effect_pictures(e);
        unsafe {
            if !e.noise.is_null() {
                SDL_ReleaseGPUTexture(self.device, e.noise);
            }
            for p in [e.ao_pipeline, e.apply_pipeline, e.blur_pipeline] {
                if !p.is_null() {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, p);
                }
            }
            if !e.repeat_point.is_null() {
                SDL_ReleaseGPUSampler(self.device, e.repeat_point);
            }
        }
        e.noise = null_mut();
        e.ao_pipeline = null_mut();
        e.apply_pipeline = null_mut();
        e.blur_pipeline = null_mut();
        e.repeat_point = null_mut();
    }

    /// Releases the effects' pictures and pipelines (the renderer's `Drop`).
    pub(super) fn release_effects(&mut self) {
        if let Some(mut e) = self.effects.take() {
            self.release_effect_parts(&mut e);
        }
    }
}

/// A region (left, top, right, bottom, from -1 to 1 across and up) as a
/// scissor rectangle on a picture of `size` pixels, pixels from the top
/// left; None when nothing of it is on the picture.
pub(super) fn scissor_of(region: BlurRegion, size: (u32, u32)) -> Option<SDL_Rect> {
    let (w, h) = (size.0 as f32, size.1 as f32);
    let x0 = ((region[0] * 0.5 + 0.5) * w).floor().max(0.0);
    let x1 = ((region[2] * 0.5 + 0.5) * w).ceil().min(w);
    let y0 = ((0.5 - region[1] * 0.5) * h).floor().max(0.0);
    let y1 = ((0.5 - region[3] * 0.5) * h).ceil().min(h);
    (x1 > x0 && y1 > y0).then_some(SDL_Rect { x: x0 as i32, y: y0 as i32, w: (x1 - x0) as i32, h: (y1 - y0) as i32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jitter_follows_vpxs_sequences() {
        assert_eq!(radical_inverse(0), 0.0);
        assert_eq!(radical_inverse(1), 0.5);
        assert_eq!(radical_inverse(2), 0.25);
        assert_eq!(radical_inverse(3), 0.75);
        assert!((radical_inverse3(1) - 1.0 / 3.0).abs() < 1e-7);
        assert!((radical_inverse3(2) - 2.0 / 3.0).abs() < 1e-7);
        assert!((radical_inverse3(3) - 1.0 / 9.0).abs() < 1e-7);
    }

    #[test]
    fn a_ball_is_bounded_by_35_points_on_its_surface() {
        let points = sphere_points();
        // VPX's spread places them all, every one on the unit sphere
        for p in points {
            assert!(((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 1.0).abs() < 1e-5, "{p:?}");
        }
    }

    #[test]
    fn regions_become_scissors_on_the_picture() {
        // the whole picture, and a quarter of it at the top left
        let full = scissor_of([-1.0, 1.0, 1.0, -1.0], (800, 600)).unwrap();
        assert_eq!((full.x, full.y, full.w, full.h), (0, 0, 800, 600));
        let quarter = scissor_of([-1.0, 1.0, 0.0, 0.0], (800, 600)).unwrap();
        assert_eq!((quarter.x, quarter.y, quarter.w, quarter.h), (0, 0, 400, 300));
        assert!(scissor_of([2.0, 1.0, 3.0, 0.0], (800, 600)).is_none());
    }
}
