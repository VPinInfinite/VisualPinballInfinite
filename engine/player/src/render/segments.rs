// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): how it draws one digit
// of a segment display (Renderer::DrawSegmentDisplay, SetupSegmentRenderer and SetupDisplayRenderer,
// src/renderer/Renderer.cpp).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! Digits of segment displays, drawn with VPX's display
//! shader: a quad a digit, its segments' shapes from VPX's own pictures
//! (`crate::segments::picture_name`), lit as PinMAME lights them, and max
//! blending, so the glows of neighbouring digits overlap without adding up.

use super::*;
use crate::segments::{PICTURE_COUNT, SegHint, SegKind, SegLook};

/// A digit to draw: where (in pixels from the target's top left: left, top,
/// width, height), its kind and shape family, its segments' levels, and its
/// look.
pub(super) struct DigitDraw {
    pub rect: [f32; 4],
    pub kind: SegKind,
    pub hint: SegHint,
    pub levels: [f32; 16],
    pub look: SegLook,
    /// The glass's tint over it (white unless a layout tints it), and its
    /// roughness, which spreads the segments' light into a glow.
    pub glass_tint: [f32; 3],
    pub roughness: f32,
    /// A layout's glass picture over it and the room's light on that glass
    /// (VPX's score view; none on the backglass or with no layout).
    pub glass: Option<(crate::scoreview::Glass, [f32; 3])>,
}

impl DigitDraw {
    /// Its picture's place among VPX's.
    fn picture(&self) -> Option<usize> {
        crate::segments::picture_index(crate::segments::picture_name(self.kind, self.hint))
    }
}

/// The segment shapes' pictures and the pipelines that draw digits, each
/// made the first time it's needed.
pub(super) struct SegmentDraw {
    pictures: [*mut SDL_GPUTexture; PICTURE_COUNT],
    /// One pipeline for each kind of target it has drawn into.
    pipelines: Vec<(SDL_GPUTextureFormat, *mut SDL_GPUGraphicsPipeline)>,
}

impl SegmentDraw {
    pub(super) fn new() -> SegmentDraw {
        SegmentDraw { pictures: [null_mut(); PICTURE_COUNT], pipelines: Vec::new() }
    }
}

impl Renderer {
    /// Makes what drawing `digits` into a target of `format` needs — their
    /// pictures and the pipeline — before the drawing's pass begins.
    pub(super) fn prepare_digits(&mut self, format: SDL_GPUTextureFormat, digits: &[DigitDraw]) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        for index in digits.iter().filter_map(DigitDraw::picture) {
            if self.segment_draw.pictures[index].is_null() {
                self.segment_draw.pictures[index] = self.segment_picture(index)?;
            }
        }
        if let Some(&(_, p)) = self.segment_draw.pipelines.iter().find(|(f, _)| *f == format) {
            return Ok(p);
        }
        let pipeline = self.segment_pipeline(format)?;
        self.segment_draw.pipelines.push((format, pipeline));
        Ok(pipeline)
    }

    /// One of VPX's segment pictures, decoded and uploaded with its mipmaps
    /// (VPX samples it filtered between them), its values left as they are.
    fn segment_picture(&self, index: usize) -> Result<*mut SDL_GPUTexture, String> {
        let (w, h, rgba) = crate::segments::picture(index)?;
        self.unorm_picture(w, h, &rgba, true, "upload a segment picture")
    }

    /// The digits' pipeline for targets of `format`: no vertex buffer, no
    /// depth, and the brighter of the digit and what's below kept (VPX's
    /// `BLENDOP_MAX`).
    fn segment_pipeline(&self, format: SDL_GPUTextureFormat) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(DISPLAY_HLSL, c"display_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 1)?;
        let fs = match self.shader(DISPLAY_HLSL, c"display_seg_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 2, 1) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = format;
            color.blend_state.enable_blend = true;
            color.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_SRC_ALPHA;
            color.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            color.blend_state.color_blend_op = SDL_GPU_BLENDOP_MAX;
            color.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            color.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            color.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_MAX;
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
            non_null(pipeline, "create the segment display pipeline")
        }
    }

    /// Draws `digits` in `pass`, on a target of `size` pixels, with the
    /// pipeline [`Renderer::prepare_digits`] gave; `mode` is how colour
    /// leaves the shader (1: tonemapped, left linear for a swap chain that
    /// encodes sRGB; 2: tonemapped and sRGB), each through its layout's
    /// glass picture when it has one, as VPX 5436's score view draws them.
    pub(super) unsafe fn draw_digits(
        &self,
        command: *mut SDL_GPUCommandBuffer,
        pass: *mut SDL_GPURenderPass,
        pipeline: *mut SDL_GPUGraphicsPipeline,
        digits: &[DigitDraw],
        size: (f32, f32),
        mode: f32,
    ) {
        let (w, h) = size;
        unsafe {
            SDL_BindGPUGraphicsPipeline(pass, pipeline);
            for d in digits {
                let Some(picture) = d.picture().map(|i| self.segment_draw.pictures[i]).filter(|p| !p.is_null()) else { continue };
                let [x, y, dw, dh] = d.rect;
                let glass = d.glass.map(|(g, ambient)| (g, ambient, self.glass_texture(g.picture))).filter(|(.., t)| !t.is_null());
                let quad = DisplayQuadUniforms {
                    rect: [x / w * 2.0 - 1.0, 1.0 - y / h * 2.0, (x + dw) / w * 2.0 - 1.0, 1.0 - (y + dh) / h * 2.0],
                    glass_area: glass.map_or([0.0, 0.0, 1.0, 1.0], |(g, ..)| g.area),
                    glass_pad: [0.0; 4],
                };
                let l = &d.look;
                let mut state = [[0.0f32; 4]; 4];
                for (j, &level) in d.levels.iter().enumerate() {
                    state[j / 4][j % 4] = level;
                }
                let uniforms = DisplayUniforms {
                    lit_diffuse: [l.lit[0], l.lit[1], l.lit[2], l.glow],
                    unlit_mode: [l.unlit[0], l.unlit[1], l.unlit[2], mode],
                    glass_tint_roughness: [d.glass_tint[0], d.glass_tint[1], d.glass_tint[2], d.roughness],
                    glass_ambient: glass.map_or([0.0; 4], |(_, a, _)| [a[0], a[1], a[2], 1.0]),
                    size: [0.0; 4],
                    properties: [0.0; 4],
                    seg_state: state,
                };
                SDL_PushGPUVertexUniformData(command, 0, (&raw const quad).cast(), size_of::<DisplayQuadUniforms>() as u32);
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<DisplayUniforms>() as u32);
                let bindings = [
                    SDL_GPUTextureSamplerBinding { texture: picture, sampler: self.clamp_sampler },
                    match glass {
                        Some((.., texture)) => SDL_GPUTextureSamplerBinding { texture, sampler: self.clamp_sampler },
                        None => SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.flat_sampler },
                    },
                ];
                SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 2);
                SDL_DrawGPUPrimitives(pass, 6, 1, 0, 0);
            }
        }
    }

    /// Releases the pictures and pipelines (the renderer's `Drop`).
    pub(super) fn release_segments(&mut self) {
        unsafe {
            for t in &mut self.segment_draw.pictures {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, *t);
                    *t = null_mut();
                }
            }
            for (_, p) in self.segment_draw.pipelines.drain(..) {
                SDL_ReleaseGPUGraphicsPipeline(self.device, p);
            }
        }
    }
}
