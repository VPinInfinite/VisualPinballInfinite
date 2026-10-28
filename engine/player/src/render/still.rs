// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its static prepass
// (src/renderer/Renderer.cpp: RenderStaticPrepass, RenderDynamics; src/renderer/RenderProbe.cpp:
// PreRenderStaticReflectionProbe, RenderReflectionProbe), its sample offsets for it (src/math/bluenoise.h) and their
// spread (gaussianDistribution, src/math/math.h), and the in-pixel offset it gives the camera (src/renderer/ViewSetup.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! VPX's still parts drawn once (its static prepass): the
//! parts that never move — VPX's choice per part (`Draw::still`) — drawn
//! 128 times, each time shifted by part of a texel and with their pictures'
//! mipmaps off, and the passes averaged into a picture of their own, kept
//! with the centred pass's depth; a mirror of everything likewise, for its
//! still parts. Each frame then starts from those and draws only what moves,
//! a still part showing a mirror getting the mirror's moving reflections
//! added over it. The whole picture is redrawn only when its size changes.
//!
//! With multisampling it's kept, as VPX's DirectX and OpenGL builds keep it
//!: the passes are drawn multisampled,
//! each resolved before it's added, the centred pass's depth kept
//! multisampled; each frame copies that depth over and draws the average
//! into every sample, so what moves gets its edges smoothed against the
//! still parts too. VPX 5436's BGFX build, which our VPX pictures come from,
//! draws every part every frame instead when multisampling: asked for with
//! `--still-parts every-frame`.

use std::cell::Cell;

use super::*;

/// VPX's passes for the still parts (`STATIC_PRERENDER_ITERATIONS`).
pub(super) const PASSES: usize = 128;

/// Which parts a frame draws: all of them, as before; the still ones, in one
/// pass shifted by this much (screen units); or only the moving ones, over
/// the still ones' picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Stage {
    Full,
    Still([f32; 2]),
    Moving,
}

/// VPX's sample points for the passes (`xyLDBNbnot`, "blue-noise'ish"),
/// from 0 to 1 across and down; the first, unshifted.
const POINTS: [[f32; 2]; PASSES] = [
    [0.00000, 0.00000], [0.01267, 0.07171], [0.10232, 0.04930], [0.18638, 0.01007], [0.28725, 0.02688], [0.43295, 0.08292], [0.50579, 0.00447], [0.53942, 0.07732],
    [0.66830, 0.03249], [0.75796, 0.06611], [0.88685, 0.01568], [0.98211, 0.05490], [0.07431, 0.14456], [0.15836, 0.13335], [0.22000, 0.09413], [0.30406, 0.13896],
    [0.36010, 0.08852], [0.52260, 0.16137], [0.61226, 0.12775], [0.69632, 0.11654], [0.78598, 0.17258], [0.84202, 0.09973], [0.91487, 0.11094], [0.99332, 0.15577],
    [0.01827, 0.18379], [0.13034, 0.23422], [0.23681, 0.18939], [0.33208, 0.22301], [0.41613, 0.17818], [0.48898, 0.22862], [0.59545, 0.24543], [0.67951, 0.20060],
    [0.77477, 0.25103], [0.88124, 0.19499], [0.97090, 0.23982], [0.04629, 0.27345], [0.08551, 0.34630], [0.19759, 0.26784], [0.25923, 0.31828], [0.35449, 0.30707],
    [0.43855, 0.29586], [0.52821, 0.31267], [0.62347, 0.33509], [0.70753, 0.28465], [0.80840, 0.32388], [0.89245, 0.27905], [0.99892, 0.32948], [0.06310, 0.42475],
    [0.16397, 0.35750], [0.24242, 0.43035], [0.32087, 0.39113], [0.41053, 0.38552], [0.47778, 0.41354], [0.56743, 0.39673], [0.64589, 0.43596], [0.71313, 0.37431],
    [0.81400, 0.40794], [0.90366, 0.36871], [0.97651, 0.41914], [0.00006, 0.45837], [0.10653, 0.52001], [0.16817, 0.45277], [0.29145, 0.51441], [0.35870, 0.48079],
    [0.44836, 0.50320], [0.54362, 0.49199], [0.62768, 0.53122], [0.73975, 0.46397], [0.87424, 0.47518], [0.95269, 0.49760], [0.01687, 0.54803], [0.16257, 0.61527],
    [0.20179, 0.54243], [0.26904, 0.60967], [0.36430, 0.58165], [0.46517, 0.59286], [0.54922, 0.58726], [0.70052, 0.55363], [0.79018, 0.53682], [0.80139, 0.62088],
    [0.88545, 0.57044], [0.07291, 0.70493], [0.07851, 0.62648], [0.20739, 0.68252], [0.30826, 0.68812], [0.39792, 0.66571], [0.49319, 0.67131], [0.60526, 0.64890],
    [0.69492, 0.63209], [0.77337, 0.71054], [0.86303, 0.69933], [0.93028, 0.63769], [0.01126, 0.73856], [0.14575, 0.76097], [0.24102, 0.77218], [0.31387, 0.79459],
    [0.40353, 0.74976], [0.48758, 0.76658], [0.57724, 0.72735], [0.68371, 0.72175], [0.72294, 0.80020], [0.82941, 0.77778], [0.93588, 0.75537], [0.06730, 0.81701],
    [0.11774, 0.88986], [0.19058, 0.85624], [0.28024, 0.88425], [0.38671, 0.83942], [0.47637, 0.86184], [0.56603, 0.83382], [0.63888, 0.80580], [0.76777, 0.87305],
    [0.84622, 0.86744], [0.91347, 0.84503], [0.02808, 0.90667], [0.10092, 0.97952], [0.18498, 0.96831], [0.26343, 0.97391], [0.36990, 0.92348], [0.45956, 0.94589],
    [0.55483, 0.91788], [0.65009, 0.89546], [0.70613, 0.95710], [0.80699, 0.96271], [0.90786, 0.95150], [0.99192, 0.91227], [0.00566, 0.99072], [0.61086, 0.98512],
];

/// Pass `n`'s shift in texels (VPX's `gaussianDistribution(u1, u2, 0.5, 0)`,
/// its "lump3" kind, limited to about a texel and a half): the first is none.
pub(super) fn shift(n: usize) -> [f32; 2] {
    let [u1, u2] = POINTS[n];
    let sigma = 0.5;
    let root4 = (1.0 - u1).sqrt().sqrt();
    let r = sigma * (9.0 - 9.0 * root4).sqrt();
    let angle = std::f32::consts::TAU * u2;
    [r * angle.cos(), r * angle.sin()]
}

/// The still parts' pictures and the pipelines that make and use them.
pub(super) struct Still {
    /// The average of the passes, and the centred pass's depth.
    pub(super) color: *mut SDL_GPUTexture,
    pub(super) depth: *mut SDL_GPUTexture,
    /// Each pass's own picture.
    pub(super) scratch: *mut SDL_GPUTexture,
    /// For each mirror of everything (VPX's dynamic reflection mode), its
    /// still parts' picture and depth likewise; null for the others.
    pub(super) mirror_color: Vec<*mut SDL_GPUTexture>,
    pub(super) mirror_depth: Vec<*mut SDL_GPUTexture>,
    pub(super) size: (u32, u32),
    /// The pictures hold the still parts as they are.
    pub(super) valid: Cell<bool>,
    /// Adding a pass to the average: the first replaces it, the last keeps
    /// its mark of the table's pixels (VPX: the centred pass's depth).
    accumulate: [*mut SDL_GPUGraphicsPipeline; 3],
    /// A frame's start: the picture and depth drawn back.
    pub(super) restore: *mut SDL_GPUGraphicsPipeline,
    /// With multisampling, the picture alone drawn back into every sample,
    /// its depth copied over beforehand.
    pub(super) restore_color: *mut SDL_GPUGraphicsPipeline,
    /// The parts' pictures sampled without mipmaps (VPX's `m_disableMipmaps`):
    /// repeating, and for the backdrop, clamped.
    pub(super) sampler: *mut SDL_GPUSampler,
    pub(super) clamp_sampler: *mut SDL_GPUSampler,
}

const STILL_HLSL: &str = include_str!("still.hlsl");

impl Renderer {
    /// Draws the still parts once a frame from now on (VPX's static prepass),
    /// where it applies: VPX's look.
    pub fn draw_still_parts_once(&mut self, once: bool) {
        self.still_wanted = once;
    }

    /// The view being set while the table plays: every part drawn every frame
    /// meanwhile, as VPX draws them while its view page shows
    /// (`Renderer::DisableStaticPrePass`); done, the still parts drawn once
    /// more, from the new view.
    pub fn set_view_changing(&mut self, changing: bool) {
        if self.view_changing && !changing
            && let Some(s) = &self.still
        {
            s.valid.set(false);
        }
        self.view_changing = changing;
    }

    /// Whether the still parts are drawn once: with VPX's look, unless asked
    /// otherwise.
    pub fn still_parts_once(&self) -> bool {
        self.still_wanted && self.hdr
    }

    /// Whether edges are smoothed by multisampling.
    pub fn multisampled(&self) -> bool {
        self.samples != SDL_GPU_SAMPLECOUNT_1
    }

    /// Makes the still parts' pictures for a picture of `width` × `height`,
    /// and its pipelines the first time.
    pub(super) fn ensure_still(&mut self, width: u32, height: u32) -> Result<(), String> {
        if !self.still_parts_once() {
            return Ok(());
        }
        // a mirror of everything with a picture has its still parts' own
        let mirrors: Vec<bool> = (0..self.scene.mirrors.len()).map(|k| self.reflection_mode(k) >= 5 && self.mirror_textures.get(k).is_some_and(|t| !t.is_null())).collect();
        if self.still.as_ref().is_some_and(|s| s.size == (width, height) && s.mirror_color.iter().map(|t| !t.is_null()).eq(mirrors.iter().copied())) {
            return Ok(());
        }
        let pipelines = match self.still.take() {
            Some(mut s) => {
                self.release_still_textures(&mut s);
                (s.accumulate, s.restore, s.restore_color, s.sampler, s.clamp_sampler)
            }
            None => {
                let accumulate = [self.still_pipeline(c"still_accumulate_fs", 0)?, self.still_pipeline(c"still_accumulate_fs", 1)?, self.still_pipeline(c"still_accumulate_fs", 2)?];
                let restore = self.still_pipeline(c"still_restore_fs", 3)?;
                let restore_color = if self.samples == SDL_GPU_SAMPLECOUNT_1 { null_mut() } else { self.still_pipeline(c"still_restore_color_fs", 4)? };
                (accumulate, restore, restore_color, self.lod0_sampler(SDL_GPU_SAMPLERADDRESSMODE_REPEAT)?, self.lod0_sampler(SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE)?)
            }
        };
        let color_usage = SDL_GPU_TEXTUREUSAGE_COLOR_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER;
        // the depth drawn back, or with multisampling copied over (SDL can't read a multisampled one)
        let single = self.samples == SDL_GPU_SAMPLECOUNT_1;
        let depth_usage = SDL_GPU_TEXTUREUSAGE_DEPTH_STENCIL_TARGET | if single { SDL_GPU_TEXTUREUSAGE_SAMPLER } else { SDL_GPUTextureUsageFlags(0) };
        let mut s = Still {
            color: null_mut(),
            depth: null_mut(),
            scratch: null_mut(),
            mirror_color: Vec::new(),
            mirror_depth: Vec::new(),
            size: (width, height),
            valid: Cell::new(false),
            accumulate: pipelines.0,
            restore: pipelines.1,
            restore_color: pipelines.2,
            sampler: pipelines.3,
            clamp_sampler: pipelines.4,
        };
        let made = (|| -> Result<(), String> {
            s.color = self.texture(HDR_FORMAT, color_usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            s.scratch = self.texture(HDR_FORMAT, color_usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            s.depth = self.texture(DEPTH_FORMAT, depth_usage, width, height, 1, self.samples)?;
            for &everything in &mirrors {
                let (c, d) = if everything {
                    (self.texture(HDR_FORMAT, color_usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?, self.texture(DEPTH_FORMAT, depth_usage, width, height, 1, self.samples)?)
                } else {
                    (null_mut(), null_mut())
                };
                s.mirror_color.push(c);
                s.mirror_depth.push(d);
            }
            Ok(())
        })();
        if let Err(e) = made {
            self.release_still_textures(&mut s);
            self.release_still_pipelines(&s);
            return Err(e);
        }
        self.still = Some(s);
        Ok(())
    }

    /// One of the still parts' pipelines over the HDR picture: adding a pass
    /// to the average (`kind` 0 replacing it, 1 adding, 2 adding and keeping
    /// the mark), (3) drawing the picture back with its depth, or (4) the
    /// picture alone into every sample of a multisampled pass.
    pub(super) fn still_pipeline(&self, entry: &CStr, kind: u32) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let restore = kind == 3;
        let restore_color = kind == 4;
        let vs = self.shader(STILL_HLSL, c"still_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 0)?;
        let fs = match self.shader(STILL_HLSL, entry, c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, if restore { 2 } else { 1 }, if restore || restore_color { 0 } else { 1 }) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = HDR_FORMAT;
            if kind == 1 || kind == 2 {
                let b = &mut color.blend_state;
                b.enable_blend = true;
                b.src_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                b.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                b.color_blend_op = SDL_GPU_BLENDOP_ADD;
                // the mark (alpha) kept, until the last pass, the centred one, sets it
                b.src_alpha_blendfactor = if kind == 2 { SDL_GPU_BLENDFACTOR_ONE } else { SDL_GPU_BLENDFACTOR_ZERO };
                b.dst_alpha_blendfactor = if kind == 2 { SDL_GPU_BLENDFACTOR_ZERO } else { SDL_GPU_BLENDFACTOR_ONE };
                b.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            }
            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            if restore {
                // every texel's depth written as it was, whatever is there
                info.depth_stencil_state.enable_depth_test = true;
                info.depth_stencil_state.enable_depth_write = true;
                info.depth_stencil_state.compare_op = SDL_GPU_COMPAREOP_ALWAYS;
                info.target_info.depth_stencil_format = DEPTH_FORMAT;
                info.target_info.has_depth_stencil_target = true;
            }
            if restore_color {
                // into the frame's multisampled pass, its depth left as copied
                info.multisample_state.sample_count = self.samples;
                info.target_info.depth_stencil_format = DEPTH_FORMAT;
                info.target_info.has_depth_stencil_target = true;
            }
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create a still parts pipeline")
        }
    }

    /// A sampler reading a picture's own texels, without its mipmaps
    /// (VPX: trilinear and anisotropic filtering become bilinear).
    fn lod0_sampler(&self, address: SDL_GPUSamplerAddressMode) -> Result<*mut SDL_GPUSampler, String> {
        unsafe {
            let mut info: SDL_GPUSamplerCreateInfo = zeroed();
            info.min_filter = SDL_GPU_FILTER_LINEAR;
            info.mag_filter = SDL_GPU_FILTER_LINEAR;
            info.mipmap_mode = SDL_GPU_SAMPLERMIPMAPMODE_NEAREST;
            info.address_mode_u = address;
            info.address_mode_v = address;
            info.address_mode_w = address;
            info.max_lod = 0.0;
            non_null(SDL_CreateGPUSampler(self.device, &info), "create the still parts' sampler")
        }
    }

    /// Draws the still parts' pictures when they don't hold the still parts
    /// as `input` has them (VPX's `RenderStaticPrepass`): 128 passes, each its
    /// own command buffer, waited for once all are sent.
    pub(super) fn draw_still(&self, input: &FrameInput, width: u32, height: u32) -> Result<(), String> {
        let Some(s) = self.still.as_ref() else { return Ok(()) };
        // the table's size (supersampled), the passes shifted by its texels
        let (rw, rh) = self.render_size(width, height);
        if self.view_changing || s.valid.get() || s.size != (rw, rh) {
            return Ok(());
        }
        let started = std::time::Instant::now();
        let (w, h) = (rw as f32, rh as f32);
        for n in (0..PASSES).rev() {
            let [u1, u2] = shift(n);
            unsafe {
                let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
                // VPX's in-pixel offset, after the projection (ViewSetup::ComputeMVP's projTrans)
                self.record(command, null_mut(), width, height, input, Stage::Still([u1 / w, u2 / h]));
                // the pass added to the average, and a mirror of everything's to its own
                let kind = if n == PASSES - 1 { 0 } else if n == 0 { 2 } else { 1 };
                self.accumulate_still(command, s.scratch, s.color, kind);
                for (k, &target) in s.mirror_color.iter().enumerate() {
                    if let Some(&source) = self.mirror_textures.get(k).filter(|_| !target.is_null()) {
                        self.accumulate_still(command, source, target, kind);
                    }
                }
                check(SDL_SubmitGPUCommandBuffer(command), "draw the still parts")?;
            }
        }
        unsafe { SDL_WaitForGPUIdle(self.device) };
        // VPX's static occlusion, baked into the picture once its passes are done
        if self.ao_mode() == 1 {
            self.bake_still_ao((rw, rh))?;
        }
        s.valid.set(true);
        eprintln!("still parts drawn once: {PASSES} passes in {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
        Ok(())
    }

    /// Adds `source`, one pass's picture, to the average in `target`.
    unsafe fn accumulate_still(&self, command: *mut SDL_GPUCommandBuffer, source: *mut SDL_GPUTexture, target: *mut SDL_GPUTexture, kind: usize) {
        let Some(s) = self.still.as_ref() else { return };
        unsafe {
            let mut info: SDL_GPUColorTargetInfo = zeroed();
            info.texture = target;
            info.load_op = if kind == 0 { SDL_GPU_LOADOP_DONT_CARE } else { SDL_GPU_LOADOP_LOAD };
            info.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &info, 1, null());
            SDL_BindGPUGraphicsPipeline(pass, s.accumulate[kind]);
            let binding = SDL_GPUTextureSamplerBinding { texture: source, sampler: self.font_sampler };
            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
            // VPX's weight, 1 / the passes; the mark taken whole from the last
            let weights = [1.0 / PASSES as f32, if kind == 2 { 1.0 } else { 0.0 }, 0.0, 0.0];
            SDL_PushGPUFragmentUniformData(command, 0, weights.as_ptr().cast(), 16);
            SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
        }
    }

    /// Draws a still picture and its depth back into the pass, as the
    /// frame's start (VPX copies them over); with multisampling, the picture
    /// alone into every sample, its depth copied over before the pass
    /// ([`Renderer::copy_still_depth`]).
    pub(super) unsafe fn restore_still(&self, pass: *mut SDL_GPURenderPass, color: *mut SDL_GPUTexture, depth: *mut SDL_GPUTexture) {
        let Some(s) = self.still.as_ref() else { return };
        unsafe {
            if self.samples != SDL_GPU_SAMPLECOUNT_1 {
                SDL_BindGPUGraphicsPipeline(pass, s.restore_color);
                let binding = SDL_GPUTextureSamplerBinding { texture: color, sampler: self.font_sampler };
                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
                return;
            }
            SDL_BindGPUGraphicsPipeline(pass, s.restore);
            let bindings = [
                SDL_GPUTextureSamplerBinding { texture: color, sampler: self.font_sampler },
                SDL_GPUTextureSamplerBinding { texture: depth, sampler: self.font_sampler },
            ];
            SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 2);
            SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
        }
    }

    /// With multisampling, copies the still parts' multisampled depth into
    /// the frame's, on `command` before its pass (VPX's DirectX and OpenGL
    /// builds copy their multisampled prepass over likewise).
    pub(super) unsafe fn copy_still_depth(&self, command: *mut SDL_GPUCommandBuffer, depth: *mut SDL_GPUTexture, width: u32, height: u32) {
        unsafe {
            let copy = SDL_BeginGPUCopyPass(command);
            let mut from: SDL_GPUTextureLocation = zeroed();
            from.texture = depth;
            let mut to: SDL_GPUTextureLocation = zeroed();
            to.texture = self.depth;
            SDL_CopyGPUTextureToTexture(copy, &from, &to, width, height, 1, false);
            SDL_EndGPUCopyPass(copy);
        }
    }

    fn release_still_textures(&self, s: &mut Still) {
        unsafe {
            for t in [s.color, s.depth, s.scratch].into_iter().chain(s.mirror_color.drain(..)).chain(s.mirror_depth.drain(..)) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t);
                }
            }
        }
        s.color = null_mut();
        s.depth = null_mut();
        s.scratch = null_mut();
    }

    fn release_still_pipelines(&self, s: &Still) {
        unsafe {
            for p in s.accumulate.into_iter().chain([s.restore, s.restore_color]) {
                if !p.is_null() {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, p);
                }
            }
            for sampler in [s.sampler, s.clamp_sampler] {
                if !sampler.is_null() {
                    SDL_ReleaseGPUSampler(self.device, sampler);
                }
            }
        }
    }

    /// Releases the still parts' pictures and pipelines (the renderer's `Drop`).
    pub(super) fn release_still(&mut self) {
        if let Some(mut s) = self.still.take() {
            self.release_still_textures(&mut s);
            self.release_still_pipelines(&s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_pass_is_centred_and_none_strays_far() {
        assert_eq!(shift(0), [0.0, 0.0]);
        for n in 0..PASSES {
            let [x, y] = shift(n);
            assert!(x.abs() < 1.5 && y.abs() < 1.5, "pass {n}: {x}, {y}");
        }
    }
}
