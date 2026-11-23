// The score display's own window, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and
// contributors): src/shaders/bgfx/vs_dmd.sc (its 2D path) and src/shaders/bgfx/fs_display.sc
// (its DMD and segment paths). This is VPX's newer display renderer, which its score view
// window draws with: every dot an emitter whose light is gathered from the 5 x 5 dots around
// each pixel, or every segment of a digit an emitter whose shape a distance field gives,
// seen through a glass that a rough surface blurs into a glow.
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The quad comes from the vertex
// number, with no vertex buffer. A DMD frame holds linear colour a dot, a brightness-only
// frame its brightness in all three, so both read the same (VPX reads .rrr or .rgb). For a
// digit, the texture holds its segments' distance fields: VPX's pictures (engine/player/assets/vpx/segments).

cbuffer DisplayQuad : register(b0, space1)
{
    float4 display_rect;       // the quad's left, top, right and bottom, in clip space
    float4 display_glass_area; // xy: where the glass picture starts; zw: how much of it (VPX's glassArea)
    float4 display_glass_pad;  // the emitter's padding inside the glass: left, right, top, bottom (VPX's glassPad)
};

struct DisplayVaryings
{
    float4 clip : SV_Position;
    float2 glass_uv : TEXCOORD0;
    float2 display_uv : TEXCOORD1;
};

DisplayVaryings display_vs(uint id : SV_VertexID)
{
    // two triangles over the corners, the first dot's at the top left
    static const float2 corners[6] = { float2(0.0, 0.0), float2(1.0, 0.0), float2(0.0, 1.0), float2(0.0, 1.0), float2(1.0, 0.0), float2(1.0, 1.0) };
    float2 c = corners[id];
    DisplayVaryings o;
    o.clip = float4(lerp(display_rect.x, display_rect.z, c.x), lerp(display_rect.y, display_rect.w, c.y), 0.0, 1.0);
    o.glass_uv = display_glass_area.xy + c * display_glass_area.zw;
    o.display_uv = c * float2(1.0 + display_glass_pad.x + display_glass_pad.y, 1.0 + display_glass_pad.z + display_glass_pad.w) - float2(display_glass_pad.x, display_glass_pad.z);
    return o;
}

Texture2D<float4> display_frame : register(t0, space2);
SamplerState display_frame_sampler : register(s0, space2);
Texture2D<float4> display_glass : register(t1, space2);
SamplerState display_glass_sampler : register(s1, space2);

cbuffer Display : register(b0, space3)
{
    float4 display_lit_diffuse;          // rgb: a lit dot's colour times its brightness; w: the diffuse glow (VPX's vColor_Intensity)
    float4 display_unlit_mode;           // rgb: an unlit dot's colour; w: 0 linear, 1 Reinhard tonemapped (VPX's staticColor_Alpha)
    float4 display_glass_tint_roughness; // VPX's glassTint_Roughness
    float4 display_glass_ambient;        // rgb: the room's light on the glass; w: 1 with a glass picture (VPX's w_h_height)
    float4 display_size;                 // xy: the frame's size in dots (VPX's vRes_Alpha_time)
    float4 display_properties;           // y: the dots' SDF offset; z: their threshold (VPX's displayProperties)
    float4 display_seg_state[4];         // a digit's 16 segments, each lit from 0 to 1 (VPX's alphaSegState)
};

#define DISPLAY_SAMPLES 2 // the dots around each pixel whose light is gathered, each way

// VPX's basic Reinhard, "while waiting for full tonemapping support"
float3 display_reinhard(float3 color)
{
    float l = min(dot(color, float3(0.176204, 0.812985, 0.0108109)), 1000.0);
    return color * ((l * 0.25 + 1.0) / (l + 1.0));
}

// sRGB's curve, for a window that takes sRGB values (VPX's FBGamma)
float3 display_srgb(float3 c)
{
    float3 lo = c * 12.92;
    float3 hi = 1.055 * pow(max(c, 0.0), 1.0 / 2.4) - 0.055;
    return float3(c.r <= 0.0031308 ? lo.r : hi.r, c.g <= 0.0031308 ? lo.g : hi.g, c.b <= 0.0031308 ? lo.b : hi.b);
}

// the glass over the emitter: its colour, and how rough it is
float4 display_glass_at(float2 glass_uv, out float roughness)
{
    float4 glass;
    if (display_glass_ambient.w != 0.0)
    {
        glass = display_glass.Sample(display_glass_sampler, glass_uv);
        glass.rgb *= display_glass_tint_roughness.rgb;
        roughness = lerp(glass.a, 1.0, display_glass_tint_roughness.w);
    }
    else
    {
        glass = float4(display_glass_tint_roughness.rgb, 0.0);
        roughness = display_glass_tint_roughness.w;
    }
    return glass;
}

// the unlit emitters' own colour, the lit ones' light, through the glass, in the window's colours:
// mode 0 linear, 1 Reinhard tonemapped and linear (a swap chain that encodes sRGB itself),
// 2 Reinhard tonemapped and sRGB (a swap chain that takes sRGB values)
float4 display_shade(float unlit_lum, float3 lit_lum, float4 glass)
{
    float3 lum = unlit_lum * display_unlit_mode.rgb + lit_lum * display_lit_diffuse.rgb;
    if (display_glass_ambient.w != 0.0)
        lum = lerp(lum, display_glass_ambient.rgb, 0.5 * glass.a);
    lum *= glass.rgb;
    if (display_unlit_mode.w == 0.0)
        return float4(lum, 1.0);
    if (display_unlit_mode.w == 1.0)
        return float4(display_reinhard(lum), 1.0);
    return float4(display_srgb(display_reinhard(lum)), 1.0);
}

float4 display_dmd_fs(DisplayVaryings v) : SV_Target0
{
    float roughness;
    float4 glass = display_glass_at(v.glass_uv, roughness);

    // light from the surrounding dots
    float unlit_lum = 0.0;
    float3 lit_lum = float3(0.0, 0.0, 0.0);
    float2 size = display_size.xy;
    float2 dmd_uv = v.display_uv * size;
    int2 dot_pos = int2(floor(dmd_uv));
    float2 dot_relative = frac(dmd_uv) - 0.5;
    float thr = 0.15 * (abs(ddx(dmd_uv.x)) + abs(ddx(dmd_uv.y))); // VPX: antialiasing by the gradient
    float min_thr = 0.5 - thr, max_thr = display_properties.z + thr;
    [unroll] for (int y = -DISPLAY_SAMPLES; y <= DISPLAY_SAMPLES; y++)
    {
        [unroll] for (int x = -DISPLAY_SAMPLES; x <= DISPLAY_SAMPLES; x++)
        {
            int2 dot_uv = dot_pos + int2(x, y);
            if (all(dot_uv >= int2(0, 0)) && all(dot_uv < int2(size)))
            {
                // 0.5 at the dot's border, rising inside, falling to 0 outside
                float sdf = saturate(display_properties.y - length(dot_relative - float2(x, y)) * (0.5 / (float(DISPLAY_SAMPLES) + 0.5)));
                float sharp = smoothstep(min_thr, max_thr, sdf);
                float diffuse = display_lit_diffuse.w * sdf * sdf; // the light spread at the glass's roughest
                float light = lerp(sharp, diffuse, roughness);
                unlit_lum += light;
                lit_lum += light * display_frame.Load(int3(dot_uv, 0)).rgb;
            }
        }
    }

    return display_shade(unlit_lum, lit_lum, glass);
}

// One digit of a segment display: the light of its 16 segments, each read from its distance
// field (four tiles side by side, a segment a colour channel), drawn sharp where the glass is
// smooth and as a glow where it is rough. Digits are drawn with max blending, so their glows
// overlap without adding up (VPX's DrawSegmentDisplay).
float4 display_seg_fs(DisplayVaryings v) : SV_Target0
{
    float roughness;
    float4 glass = display_glass_at(v.glass_uv, roughness);
    float4 lit4 = float4(0.0, 0.0, 0.0, 0.0);
    float4 unlit4 = float4(0.0, 0.0, 0.0, 0.0);
    [unroll] for (int i = 0; i < 4; i++)
    {
        float4 sdf = display_frame.Sample(display_frame_sampler, float2(0.25 * (v.display_uv.x + float(i)), v.display_uv.y));
        float4 sharp = smoothstep(0.475, 0.525, sdf); // the edge, after the texture's filtering
        float4 diffuse = display_lit_diffuse.w * sdf * sdf; // the light spread at the glass's roughest
        float4 light = lerp(sharp, diffuse, roughness);
        // VPX: max rather than a sum, so corners between segments aren't lit twice
        unlit4 = max(light, unlit4);
        lit4 = max(light * display_seg_state[i], lit4);
    }
    float3 lit_lum = max(max(lit4.x, lit4.y), max(lit4.z, lit4.w));
    float unlit_lum = max(max(unlit4.x, unlit4.y), max(unlit4.z, unlit4.w));
    return display_shade(unlit_lum, lit_lum, glass);
}
