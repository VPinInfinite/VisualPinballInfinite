// VPX's look: bloom, ported from Visual Pinball X build 5436 (commit af26b2d, license:GPLv3+,
// Copyright (C) 2000-2026 Visual Pinball development team and contributors):
// src/shaders/bgfx/fs_pp_bloom.sc (the cut-off, whose soft threshold follows Catlike Coding's bloom
// tutorial, MIT-0), fs_pp_blur.sc (its 39-texel Gaussian, two taps a fetch by linear filtering) and
// the passes of Renderer::UpdateBloom and RenderDevice::DrawGaussianBlur.
//
// What is brighter than VPX's threshold is gathered into a picture a quarter of the size, blurred
// across and then down, and added to the table's picture before tonemapping. The same file gives
// VPX's other Gaussian blurs, their offsets and weights handed in (a rough mirror's picture).
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The triangle comes from the vertex number.

struct BloomVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
};

BloomVaryings bloom_vs(uint id : SV_VertexID)
{
    float2 c = float2((id << 1) & 2, id & 2);
    BloomVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    o.uv = c;
    return o;
}

Texture2D<float4> bloom_source : register(t0, space2);
SamplerState bloom_sampler : register(s0, space2);

cbuffer Bloom : register(b0, space3)
{
    // xy: one texel of the source (VPX's w_h_height.xy); z: the table's bloom strength; w: the
    // cut-off's shift of its sampling point, in source texels (VPX's quad is drawn 2.25 texels over)
    float4 bloom_w_h_height;
};

#define BURN_HIGHLIGHTS 0.25

// VPX's FBToneMap (common.sh): Reinhard with burnt highlights, before the threshold
float3 bloom_tone_map(float3 color)
{
    float l = dot(color, float3(0.176204, 0.812985, 0.0108109));
    return color * ((l * BURN_HIGHLIGHTS + 1.0) / (l + 1.0));
}

float3 bloom_cutoff(float3 c)
{
    const float Threshold = 2.5;
    const float SoftThreshold = 1.0;
    const float Knee = Threshold * SoftThreshold;
    float brightness = max(c.r, max(c.g, c.b));
    float soft = brightness - (Threshold - Knee);
    soft = clamp(soft, 0.0, 2.0 * Knee);
    soft *= soft * (1.0 / (4.0 * Knee + 0.00001));
    float contribution = max(soft, brightness - Threshold) / max(brightness, 0.00001);
    return c * contribution;
}

// the cut-off: four filtered samples of the full picture around each quarter-size texel
float4 bloom_cut_fs(BloomVaryings v) : SV_Target0
{
    float2 uv = v.uv + bloom_w_h_height.w * bloom_w_h_height.xy;
    float2 d = bloom_w_h_height.xy;
    float3 result = (bloom_source.SampleLevel(bloom_sampler, uv - d, 0.0).rgb
                   + bloom_source.SampleLevel(bloom_sampler, uv + d, 0.0).rgb
                   + bloom_source.SampleLevel(bloom_sampler, uv + float2(d.x, -d.y), 0.0).rgb
                   + bloom_source.SampleLevel(bloom_sampler, uv + float2(-d.x, d.y), 0.0).rgb) * 0.25;
    return float4(max(bloom_cutoff(bloom_tone_map(result)), 0.0) * bloom_w_h_height.z, 1.0);
}

// VPX's 39-texel blur, its edges fading smoothly (BLUR_39)
static const float blur_offset[10] = { 0.66063, 2.46625, 4.43946, 6.41301, 8.38706, 10.36173, 12.33715, 14.31341, 16.29062, 18.26884 };
static const float blur_weight[10] = { 0.13669, 0.15600, 0.10738, 0.05971, 0.02682, 0.00973, 0.00285, 0.00067, 0.00013, 0.00002 };

float3 blur(float2 uv, float2 step)
{
    float3 result = float3(0.0, 0.0, 0.0);
    [unroll] for (int i = 0; i < 10; ++i)
    {
        float2 o = step * blur_offset[i];
        result += (bloom_source.SampleLevel(bloom_sampler, uv + o, 0.0).rgb + bloom_source.SampleLevel(bloom_sampler, uv - o, 0.0).rgb) * blur_weight[i];
    }
    return result;
}

float4 blur_h_fs(BloomVaryings v) : SV_Target0
{
    return float4(blur(v.uv, float2(bloom_w_h_height.x, 0.0)), 1.0);
}

// VPX's 19-texel blur (BLUR_19), for the bulb light buffer (Renderer::DrawBulbLightBuffer)
static const float blur19_offset[5] = { 0.65323, 2.42572, 4.36847, 6.31470, 8.26547 };
static const float blur19_weight[5] = { 0.19923, 0.18937, 0.08396, 0.02337, 0.00408 };

float3 blur19(float2 uv, float2 step)
{
    float3 result = float3(0.0, 0.0, 0.0);
    [unroll] for (int i = 0; i < 5; ++i)
    {
        float2 o = step * blur19_offset[i];
        result += (bloom_source.SampleLevel(bloom_sampler, uv + o, 0.0).rgb + bloom_source.SampleLevel(bloom_sampler, uv - o, 0.0).rgb) * blur19_weight[i];
    }
    return result;
}

float4 blur19_h_fs(BloomVaryings v) : SV_Target0
{
    return float4(blur19(v.uv, float2(bloom_w_h_height.x, 0.0)), 1.0);
}

float4 blur19_v_fs(BloomVaryings v) : SV_Target0
{
    return float4(blur19(v.uv, float2(0.0, bloom_w_h_height.y)), 1.0);
}

float4 blur_v_fs(BloomVaryings v) : SV_Target0
{
    return float4(blur(v.uv, float2(0.0, bloom_w_h_height.y)), 1.0);
}

// Any of VPX's Gaussian blurs (fs_pp_blur.sc's BLUR_7 to BLUR_39), its taps handed in: up to ten
// offsets and weights, each tap two fetches either side, but a centre tap taken once
cbuffer Kernel : register(b1, space3)
{
    float4 kernel_offset[3];
    float4 kernel_weight[3];
    float4 kernel_taps; // x: how many; y: 1 when the first is the centre
};

float3 blur_kernel(float2 uv, float2 step)
{
    float3 result = float3(0.0, 0.0, 0.0);
    int taps = (int)kernel_taps.x;
    [loop] for (int i = 0; i < taps; ++i)
    {
        float o = kernel_offset[i >> 2][i & 3];
        float w = kernel_weight[i >> 2][i & 3];
        if (i == 0 && kernel_taps.y != 0.0)
            result += bloom_source.SampleLevel(bloom_sampler, uv, 0.0).rgb * w;
        else
            result += (bloom_source.SampleLevel(bloom_sampler, uv + step * o, 0.0).rgb + bloom_source.SampleLevel(bloom_sampler, uv - step * o, 0.0).rgb) * w;
    }
    return result;
}

float4 blur_kernel_h_fs(BloomVaryings v) : SV_Target0
{
    return float4(blur_kernel(v.uv, float2(bloom_w_h_height.x, 0.0)), 1.0);
}

float4 blur_kernel_v_fs(BloomVaryings v) : SV_Target0
{
    return float4(blur_kernel(v.uv, float2(0.0, bloom_w_h_height.y)), 1.0);
}
