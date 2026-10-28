// The score display's dots, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and
// contributors): src/shaders/bgfx/fs_dmd.sc, its DMD path — VPX's legacy DMD renderer,
// which VPX uses by default for text boxes showing the display and for the DMD
// flashers of tables made before 10.8.1. Each pixel averages 13 jittered samples of
// the frame, each lighting a round dot inside its texel, and fades the dots to plain
// squares where they are too small to draw.
//
// Compiled after overlay.hlsl (DMD_OVERLAY) or player.hlsl (DMD_SCENE), whose
// varyings it uses. The frame holds linear colour a dot; a brightness-only frame holds
// its brightness in all three, so the tint colours it (VPX multiplies the red channel
// alone by the tint, the same value).

Texture2D<float4> dmd_frame : register(t0, space2);
SamplerState dmd_frame_sampler : register(s0, space2);

cbuffer Dmd : register(b0, space3)
{
    float4 dmd_tint;           // rgb: the dots' tint times brightness (VPX's vColor_Intensity)
    float4 dmd_res_alpha_time; // xy: the frame's size in dots; z: alpha; w: frame number % 2048
};

float2 dmd_hash22(float2 uv)
{
    float3 p3 = frac(uv.xyx * float3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return frac((p3.xx + p3.yz) * p3.zy);
}

// from -1 to 1, most likely 0, for a random number from 0 to 1
float dmd_triangular(float r)
{
    float p = 2.0 * r;
    bool b = p > 1.0;
    if (b)
        p = 2.0 - p;
    p = 1.0 - sqrt(p);
    return b ? p : -p;
}

float4 dmd_dots(float2 uv)
{
    const float blur = 1.5;
    float2 ddxs = ddx(uv) * blur;
    float2 ddys = ddy(uv) * blur;
    float dist_factor = clamp(1.0 - length(ddxs + ddys) * 6.66, 0.4, 1.0);
    float2 offs = dmd_hash22(uv + dmd_res_alpha_time.w);
    float3 color = float3(0.0, 0.0, 0.0);
    const int samples = 13;
    [unroll] for (int i = 0; i < samples; ++i)
    {
        float fi = float(i);
        float2 xi = float2(frac(fi * (1.0 / 13.0) + offs.x), frac(fi * (8.0 / 13.0) + offs.y));
        float2 s = uv + dmd_triangular(xi.x) * ddxs + dmd_triangular(xi.y) * ddys;
        float4 rgba = dmd_frame.SampleLevel(dmd_frame_sampler, s, 0.0);
        // the dot within the sampled texel
        float2 dist = (frac(s * dmd_res_alpha_time.xy) * 2.2 - 1.1) * dist_factor;
        float r2 = dist.x * dist.x + dist.y * dist.y;
        float d = smoothstep(0.0, 1.0, 1.0 - r2 * r2);
        color += rgba.rgb * d;
    }
    color *= dmd_tint.xyz * ((1.0 / 13.0) * dist_factor * dist_factor);
    return float4(color, dmd_res_alpha_time.z);
}

#ifdef DMD_OVERLAY
// A text box showing the display, on the backdrop
float4 dmd_backdrop_fs(OverlayVaryings v) : SV_Target0
{
    return dmd_dots(v.uv);
}
#endif

#ifdef DMD_SCENE
// A flasher showing the display, in the table
float4 fs_dmd_flasher(Varyings v) : SV_Target0
{
    return dmd_dots(v.uv);
}
#endif
