// The final picture, ported from Visual Pinball X build 5436 (commit af26b2d, license:GPLv3+,
// Copyright (C) 2000-2026 Visual Pinball development team and contributors):
// src/shaders/bgfx/fs_pp_tonemap.sc (its path for ordinary, SDR screens, and its
// HDR10 path, with its WCG spline) and the sRGB curves of src/shaders/bgfx/common.sh.
//
// Its tone mappers carry their own published sources, as VPX notes them: Khronos's PBR Neutral
// (KhronosGroup/ToneMapping, Apache-2.0) and the AgX of three.js (MIT) and Filament (Apache-2.0),
// after Troy Sobotka's AgX and Benjamin Wrensch's minimal version of it. VPX's dither comes from a
// Shadertoy page whose license we can't confirm, so the noise here is our own, from Chris Wellons's
// public-domain integer hash (lowbias32).
//
// The table is drawn into a half-float picture where light adds up past white. This pass turns it
// into the screen's colours: VPX's bloom added (bloom.hlsl), the table's exposure, its tone mapper
// (Reinhard unless the table names another), a little noise against banding (dither), and the
// table's colour grade. Where no
// part of the table was drawn — the backdrop, its lights, reels and text boxes, the backdrop's score
// display — the picture keeps its colours untouched: VPX finds those by its depth buffer still at
// its cleared value; here the table's solid parts mark their pixels in the picture's alpha.
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The triangle comes from the vertex number.

struct TonemapVaryings
{
    float4 clip : SV_Position;
};

TonemapVaryings tonemap_vs(uint id : SV_VertexID)
{
    // one triangle over the whole window
    float2 c = float2((id << 1) & 2, id & 2);
    TonemapVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}

Texture2D<float4> tonemap_picture : register(t0, space2);
SamplerState tonemap_picture_sampler : register(s0, space2);
Texture2D<float4> tonemap_lut : register(t1, space2);
SamplerState tonemap_lut_sampler : register(s1, space2);
Texture2D<float4> tonemap_bloom : register(t2, space2);
SamplerState tonemap_bloom_sampler : register(s2, space2);
Texture2D<float4> tonemap_ao : register(t3, space2);
SamplerState tonemap_ao_sampler : register(s3, space2);

cbuffer Tonemap : register(b0, space3)
{
    float4 tonemap_exposure_mapper; // x: exposure; y: tone mapper (0 Reinhard, 1 AgX, 2 Filmic, 3 Neutral, 4 AgX punchy); zw: the nudge shake, in pixels
    float4 tonemap_flags;           // x: dither; y: colour grade; z: the target encodes sRGB itself; w: bloom
    float4 tonemap_frame;           // x: the frame's number, 0 to 2047, so the dither changes from frame to frame; yz: 1 / the window's size; w: the picture is bigger (supersampled), read filtered
    float4 tonemap_ao_info;         // x: VPX's dynamic ambient occlusion over the picture (effects.hlsl); yz: 1 / the picture's size
    float4 tonemap_hdr;             // x: 1 for VPX's HDR10 output (hdr10.rs); y: SDR white / the screen's peak (VPX's sceneLum_x_invDisplayMaxLum); z: the peak / 10000 nits (displayMaxLum); w: VPX's spline in the tone mapper's place
    float4 tonemap_spline1;         // VPX's spline: the source and destination pivots, the slope, Pa
    float4 tonemap_spline2;         // xy: Qa, Qb
};

#define BURN_HIGHLIGHTS 0.25
#define MAX_BURST 1000.0
#define FLT_MIN_VALUE 0.00006103515625

// VPX's sRGB curves (common.sh: FBGamma, InvGamma), and the plain 2.2 one (FBGamma22)
float3 tonemap_srgb(float3 c)
{
    float3 lo = c * 12.92;
    float3 hi = 1.055 * pow(max(c, 0.0), 1.0 / 2.4) - 0.055;
    return float3(c.r <= 0.0031308 ? lo.r : hi.r, c.g <= 0.0031308 ? lo.g : hi.g, c.b <= 0.0031308 ? lo.b : hi.b);
}

float3 tonemap_inv_srgb(float3 c)
{
    float3 lo = c * (1.0 / 12.92);
    float3 hi = pow(max(c * (1.0 / 1.055) + (0.055 / 1.055), 0.0), 2.4);
    return float3(c.r <= 0.04045 ? lo.r : hi.r, c.g <= 0.04045 ? lo.g : hi.g, c.b <= 0.04045 ? lo.b : hi.b);
}

float3 tonemap_gamma22(float3 c)
{
    return pow(max(c, 0.0), 1.0 / 2.2);
}

float3 reinhard(float3 color)
{
    // the clamp keeps an overflow from turning into black blobs
    float l = min(dot(color, float3(0.176204, 0.812985, 0.0108109)), MAX_BURST); // CIE RGB to XYZ, Y row (relative luminance)
    return color * ((l * BURN_HIGHLIGHTS + 1.0) / (l + 1.0));
}

// John Hable's filmic curve, sRGB out
float3 filmic(float3 color)
{
    color = min(color, float3(MAX_BURST, MAX_BURST, MAX_BURST));
    float3 x = max(float3(0.0, 0.0, 0.0), color - 0.004);
    return (x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06);
}

// Khronos PBR neutral, linear out
float3 neutral(float3 color)
{
    const float start_compression = 0.8 - 0.04;
    const float desaturation = 0.15;
    float x = min(color.x, min(color.y, color.z));
    float offset = x < 0.08 ? x - 6.25 * (x * x) : 0.04;
    color -= offset;
    float peak = max(color.x, max(color.y, color.z));
    if (peak < start_compression)
        return color;
    const float d = 1.0 - start_compression;
    float new_peak = 1.0 - (d * d) / (peak + (d - start_compression));
    float inv_g = desaturation * (peak - new_peak) + 1.0;
    float w = new_peak / (inv_g * peak);
    float n = new_peak - new_peak / inv_g;
    return n + color * w;
}

// AgX (as three.js, Godot and Filament have it), sRGB out; `punchy` adds VPX's punchy look
float3 agx(float3 color, bool punchy)
{
    // VPX's matrices are given in rows for its row-vector multiply: color * M
    const float3x3 inset = float3x3(
        0.8424790622530940, 0.0423282422610123, 0.0423756549057051,
        0.0784335999999992, 0.8784686364697720, 0.0784336000000000,
        0.0792237451477643, 0.0791661274605434, 0.8791429737931040);
    const float3x3 outset = float3x3(
        1.1968790051201700, -0.0528968517574562, -0.0529716355144438,
        -0.0980208811401368, 1.1519031299041700, -0.0980434501171241,
        -0.0990297440797205, -0.0989611768448433, 1.1510736726411600);
    const float min_ev = -12.47393;
    const float max_ev = 4.026069;
    color = mul(color, inset);
    color = max(color, FLT_MIN_VALUE);
    color = log2(color);
    color = (color - min_ev) / (max_ev - min_ev);
    color = saturate(color);
    float3 x2 = color * color;
    float3 x4 = x2 * x2;
    color = 15.5 * x4 * x2 - 40.14 * x4 * color + 31.96 * x4 - 6.868 * x2 * color + 0.4298 * x2 + 0.1191 * color - 0.00232;
    if (punchy)
    {
        const float3 lw = float3(0.2126, 0.7152, 0.0722);
        float luma = dot(color, lw);
        color = pow(max(color, 0.0), float3(1.35, 1.35, 1.35));
        color = luma + 1.4 * (color - luma);
    }
    return mul(color, outset);
}

// Chris Wellons's lowbias32 (public domain): 32 well-mixed bits from 32
uint tonemap_hash(uint x)
{
    x ^= x >> 16;
    x *= 0x7feb352du;
    x ^= x >> 15;
    x *= 0x846ca68bu;
    x ^= x >> 16;
    return x;
}

// two 16-bit halves summed: noise from -1 to 1, most often near 0
float tonemap_triangular(uint h)
{
    return float(h & 0xffffu) * (1.0 / 65536.0) + float(h >> 16) * (1.0 / 65536.0) - 1.0;
}

// Noise of under one step of the screen's 255 on each colour, different for every pixel and frame,
// so smooth gradients don't show bands where the picture is cut to 8 bits (VPX's FBDither, by a
// noise of our own, as strong as VPX's: a spread of about 0.3 of a step)
float3 dither(float3 color, float2 pixel)
{
    if (tonemap_flags.x == 0.0)
        return color;
    uint2 p = uint2(pixel);
    uint h0 = tonemap_hash(p.x + tonemap_hash(p.y + tonemap_hash(uint(tonemap_frame.x))));
    uint h1 = tonemap_hash(h0);
    uint h2 = tonemap_hash(h1);
    return color + float3(tonemap_triangular(h0), tonemap_triangular(h1), tonemap_triangular(h2)) * (0.75 / 255.0);
}

// VPX's colour grade: a 16 x 16 x 16 table laid out in a 256 x 16 picture
float3 color_grade(float3 color)
{
    if (tonemap_flags.y == 0.0)
        return color;
    color.xy = color.xy * (15.0 / 16.0) + 1.0 / 32.0;
    color.z *= 15.0;
    float x = (color.x + floor(color.z)) / 16.0;
    float3 lut1 = tonemap_lut.SampleLevel(tonemap_lut_sampler, float2(x, color.y), 0.0).xyz;
    float3 lut2 = tonemap_lut.SampleLevel(tonemap_lut_sampler, float2(x + 1.0 / 16.0, color.y), 0.0).xyz;
    return lerp(lut1, lut2, frac(color.z));
}

// VPX's spline for an HDR screen (its WCG tone mapper, after VideoLAN's): the scene's luminance
// bent to the screen's range, nearly a straight line on a screen of 1000 nits or more
float3 wcg_spline(float3 result)
{
    result /= tonemap_hdr.y;
    float y_old = dot(result, float3(0.2627, 0.6780, 0.0593));
    if (y_old >= FLT_MIN_VALUE)
    {
        float y = y_old - tonemap_spline1.x;
        y = y > 0.0 ? ((tonemap_spline2.x * y + tonemap_spline2.y) * y + tonemap_spline1.z) * y : (tonemap_spline1.w * y + tonemap_spline1.z) * y;
        y += tonemap_spline1.y;
        result *= y / y_old;
    }
    return result * tonemap_hdr.y;
}

// VPX's HDR10 output (Microsoft's guide for DXGI, as VPX follows it): the tonemapped picture, 1 at
// the screen's peak, coloured, in nits, into BT.2020's colours, and encoded by ST.2084 (PQ)
float3 hdr10_out(float3 col, bool gamma_out, bool wcg)
{
    if (tonemap_flags.y != 0.0)
    {
        // the colour grade on the tonemapped value brought back to the scene's range
        if (!gamma_out)
            col = tonemap_srgb(col);
        float3 sat = clamp(col / tonemap_hdr.y, 0.0001, 1.0);
        col *= color_grade(sat) / sat;
        if (!gamma_out)
            col = tonemap_inv_srgb(col);
    }
    if (gamma_out)
        col = tonemap_inv_srgb(col);
    // only VPX's spline may go past the screen's peak
    if (!wcg)
        col = saturate(col);
    col *= tonemap_hdr.z;
    // VPX's matrix in rows for its row-vector multiply: color * M
    const float3x3 srgb_to_rec2020 = float3x3(
        0.6274, 0.0691, 0.0164,
        0.3293, 0.9195, 0.0880,
        0.0433, 0.0113, 0.8956);
    col = mul(col, srgb_to_rec2020);
    const float m1 = (2610.0 / 4096.0) / 4.0;
    const float m2 = (2523.0 / 4096.0) * 128.0;
    const float c1 = 3424.0 / 4096.0;
    const float c2 = (2413.0 / 4096.0) * 32.0;
    const float c3 = (2392.0 / 4096.0) * 32.0;
    // below zero has no PQ value (VPX's NaN, which the screen's 10 bits take as 0)
    float3 cp = pow(max(col, 0.0), m1);
    return pow((c1 + c2 * cp) / (1.0 + c3 * cp), m2);
}

float4 tonemap_fs(TonemapVaryings v) : SV_Target0
{
    int2 texel = int2(v.clip.xy);
    // VPX's nudge shake, with the still parts drawn once: the whole picture moved, filtered, black
    // where it moved away (Renderer's post-processing quad, offset by the screen offset)
    float2 shake = tonemap_exposure_mapper.zw;
    float2 uv = (v.clip.xy - shake) * tonemap_frame.yz;
    float4 picture;
    // shaken, or supersampled (VPX's AAFactor: read down filtered, as VPX 10.8.0 reads it)
    if (any(shake != 0.0) || tonemap_frame.w != 0.0)
        picture = all(uv >= 0.0) && all(uv <= 1.0) ? tonemap_picture.SampleLevel(tonemap_picture_sampler, uv, 0.0) : float4(0.0, 0.0, 0.0, 0.0);
    else
        picture = tonemap_picture.Load(int3(texel, 0));
    float3 result = picture.rgb;
    // VPX's ambient occlusion before the bloom (fs_pp_tonemap.sc's AO), shifted half a texel to blur over 2x2
    if (tonemap_ao_info.x != 0.0)
        result *= tonemap_ao.SampleLevel(tonemap_ao_sampler, uv - 0.5 * tonemap_ao_info.yz, 0.0).x;
    // VPX adds its bloom before the exposure, everywhere
    if (tonemap_flags.w != 0.0)
        result += tonemap_bloom.SampleLevel(tonemap_bloom_sampler, uv, 0.0).rgb;
    result *= tonemap_exposure_mapper.x;
    // an HDR screen: the scene's white at the screen's SDR white, so its peak is 1
    bool hdr10 = tonemap_hdr.x != 0.0;
    if (hdr10)
        result *= tonemap_hdr.y;
    bool wcg = hdr10 && tonemap_hdr.w != 0.0;
    float mapper = tonemap_exposure_mapper.y;
    // Filmic and AgX hand back sRGB, the others (and VPX's spline) linear
    bool gamma_out = !wcg && (mapper == 1.0 || mapper == 2.0 || mapper == 4.0);
    // the table's pixels (VPX: its depth buffer was written there)
    if (picture.a > 0.0)
    {
        if (wcg)
            result = wcg_spline(result);
        else if (mapper == 0.0)
            result = reinhard(result);
        else if (mapper == 2.0)
            result = filmic(result);
        else if (mapper == 3.0)
            result = neutral(result);
        else
            result = agx(result, mapper == 4.0);
    }
    else if (gamma_out)
        result = tonemap_srgb(result);
    if (hdr10)
        return float4(hdr10_out(result, gamma_out, wcg), 1.0);
    // to the screen's encoding, as VPX writes its 8-bit picture
    float3 g;
    if (gamma_out)
        g = saturate(dither(result, v.clip.xy));
    else if (mapper == 3.0)
        g = tonemap_gamma22(saturate(dither(result, v.clip.xy)));
    else
        g = tonemap_srgb(saturate(dither(result, v.clip.xy)));
    g = color_grade(g);
    // a target that encodes sRGB itself gets the linear value that it encodes back to this
    return float4(tonemap_flags.z != 0.0 ? tonemap_inv_srgb(g) : g, 1.0);
}
