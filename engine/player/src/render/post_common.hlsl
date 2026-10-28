// VPX's post-processed antialiasing and sharpening, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and contributors):
// src/shaders/bgfx/fs_pp_fxaa.sc, fs_pp_faaa.sc, fs_pp_nfaa.sc, fs_pp_dlaa_edge.sc, fs_pp_dlaa.sc, fs_pp_cas.sc,
// fs_pp_bilateral_cas.sc and the SMAA passes fs_pp_smaa_*.sc and vs_pp_smaa_*.sc with their settings (SMAA.sh), run
// as Renderer::ApplyPostProcessedAntialiasing and ApplySharpening run them (src/renderer/Renderer.cpp). This part is
// common to all of them: post.hlsl follows it for the others; for SMAA, its own code, Jorge Jimenez and others' (MIT), in
// smaa.hlsl, then its passes, smaa_passes.hlsl. The techniques are others' too: NFAA (normal filter
// antialiasing), Dmitry Andreev's DLAA (directionally localized antialiasing), AMD's CAS (contrast adaptive
// sharpening); that code is VPX's. FXAA's and FAAA's code is NVIDIA's and G. Rebord's, licences not stated: see
// their section in post.hlsl.
//
// Each pass reads the tonemapped picture, its sRGB values as they are (VPX's 8-bit post-processing pictures),
// and leaves pixels where nothing of the table was drawn untouched, as VPX finds them by its depth (when the
// depth can be read: not under multisampling). The last pass writes the window: decoded to linear light when
// the window encodes sRGB itself.
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The triangle comes from the vertex number.

struct PostVaryings
{
    float4 clip : SV_Position;
};

PostVaryings post_vs(uint id : SV_VertexID)
{
    // one triangle over the whole picture
    float2 c = float2((id << 1) & 2, id & 2);
    PostVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}

cbuffer Post : register(b0, space3)
{
    float4 w_h_height; // xy: 1 / the picture's size; z: its width; w: 1 when its depth can be read
    float4 post_flags; // x: the result is decoded to linear light (the target encodes sRGB itself)
};

// sRGB's curve undone (VPX's InvGamma)
float3 post_linear(float3 c)
{
    float3 lo = c * (1.0 / 12.92);
    float3 hi = pow(max(c * (1.0 / 1.055) + (0.055 / 1.055), 0.0), 2.4);
    return float3(c.r <= 0.04045 ? lo.r : hi.r, c.g <= 0.04045 ? lo.g : hi.g, c.b <= 0.04045 ? lo.b : hi.b);
}

float4 post_out(float3 c)
{
    return float4(post_flags.x != 0.0 ? post_linear(c) : c, 1.0);
}

// every pass's samplers, whatever its pictures: linear first, point second
SamplerState post_linear_sampler : register(s0, space2);
SamplerState post_point_sampler : register(s1, space2);

// SMAA's settings (SMAA.sh: the high preset), its porting through the samplers above
#define SMAA_CUSTOM_SL 1
#define SMAATexture2D(tex) Texture2D tex
#define SMAATexturePass2D(tex) tex
#define SMAASampleLevelZero(tex, coord) tex.SampleLevel(post_linear_sampler, coord, 0)
#define SMAASampleLevelZeroPoint(tex, coord) tex.SampleLevel(post_point_sampler, coord, 0)
#define SMAASampleLevelZeroOffset(tex, coord, offset) tex.SampleLevel(post_linear_sampler, coord, 0, offset)
#define SMAASample(tex, coord) tex.Sample(post_linear_sampler, coord)
#define SMAASamplePoint(tex, coord) tex.Sample(post_point_sampler, coord)
#define SMAASampleOffset(tex, coord, offset) tex.Sample(post_linear_sampler, coord, offset)
#define SMAA_FLATTEN [flatten]
#define SMAA_BRANCH [branch]
#define SMAATexture2DMS2(tex) Texture2DMS<float4, 2> tex
#define SMAALoad(tex, pos, sample) tex.Load(pos, sample)
#define SMAAGather(tex, coord) tex.Gather(post_linear_sampler, coord, 0)
#define SMAA_PRESET_HIGH
#define SMAA_RT_METRICS smaa_metrics
// 1 / the picture's size, and its size: set by each SMAA entry from its stage's uniforms
static float4 smaa_metrics;
