// The player's on-screen readouts for SDL3 GPU on Direct3D 12: text drawn from
// the font texture, in pixels. Registers follow SDL3 GPU's rules (see player.hlsl).

Texture2D<float4> font_texture : register(t0, space2);
SamplerState font_sampler : register(s0, space2);

cbuffer Overlay : register(b0, space1)
{
    float4 screen; // xy: 2 / width, -2 / height
};

struct OverlayInput
{
    float2 position : TEXCOORD0;
    float2 uv : TEXCOORD1;
    float4 color : TEXCOORD2;
};

struct OverlayVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
    float4 color : TEXCOORD1;
};

OverlayVaryings overlay_vs(OverlayInput input)
{
    OverlayVaryings o;
    o.clip = float4(input.position * screen.xy + float2(-1.0, 1.0), 0.0, 1.0);
    o.uv = input.uv;
    o.color = input.color;
    return o;
}

float4 overlay_fs(OverlayVaryings v) : SV_Target0
{
    return float4(v.color.rgb, v.color.a * font_texture.Sample(font_sampler, v.uv).a);
}

#ifdef OVERLAY_PQ_WHITE
// The readout over VPX 5436's HDR10 output (render/hdr10.rs): its colours at the screen's SDR white
// (OVERLAY_PQ_WHITE, in 10000s of nits), as VPX scales its own interface there, in BT.2020's
// colours, encoded by ST.2084 (PQ) as tonemap.hlsl encodes the picture
float4 overlay_pq_fs(OverlayVaryings v) : SV_Target0
{
    const float3x3 srgb_to_rec2020 = float3x3(
        0.6274, 0.0691, 0.0164,
        0.3293, 0.9195, 0.0880,
        0.0433, 0.0113, 0.8956);
    float3 col = mul(v.color.rgb * OVERLAY_PQ_WHITE, srgb_to_rec2020);
    const float m1 = (2610.0 / 4096.0) / 4.0;
    const float m2 = (2523.0 / 4096.0) * 128.0;
    const float c1 = 3424.0 / 4096.0;
    const float c2 = (2413.0 / 4096.0) * 32.0;
    const float c3 = (2392.0 / 4096.0) * 32.0;
    float3 cp = pow(max(col, 0.0), m1);
    col = pow((c1 + c2 * cp) / (1.0 + c3 * cp), m2);
    return float4(col, v.color.a * font_texture.Sample(font_sampler, v.uv).a);
}
#endif

// A backglass picture (Visual Pinball X build 5436's Renderer::DrawImage for its
// plugins, license:GPLv3+): the picture tinted and as opaque as asked, blended over
// what's below; its sRGB values used as they are.
float4 backglass_fs(OverlayVaryings v) : SV_Target0
{
    return font_texture.Sample(font_sampler, v.uv) * v.color;
}

// A picture drawn flat over the window (the backdrop image, score reels):
// VPX's DMD shader without a DMD (ps_main_noDMD). The vertex colour's alpha
// carries VPX's alpha test: texels less opaque than it are not drawn.
float4 sprite_fs(OverlayVaryings v) : SV_Target0
{
    float4 t = font_texture.Sample(font_sampler, v.uv);
    if (t.a < v.color.a)
        discard;
    return float4(t.rgb * v.color.rgb, 1.0);
}

// Lights on the backdrop, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and
// contributors): src/shaders/bgfx/fs_classic_light.sc (light_with_texture and
// light_without_texture with lighting off, which is how VPX draws a classic light
// on the backdrop), src/shaders/bgfx/fs_light.sc (bulb_light) and the OverlayHDR
// and ScreenHDR blends of src/shaders/bgfx/common.sh. Each vertex's colour carries
// its position in pixels (VPX's tablePos in backdrop mode), from which the glow
// falls off.
cbuffer BackdropLight : register(b0, space3)
{
    float4 light_color_intensity; // rgb colour, w intensity now
    float4 light_color2_power;    // rgb edge colour, w falloff power
    float4 light_center_range;    // xy centre in pixels, z 0, w 1 / falloff
    float4 light_mode;            // x: has the light's image; y: modulate vs add; z: bulb term (0 modulate, 1 add, 2 VPX's single pass)
};

float4 OverlayHDR(float4 cBase, float4 cBlend)
{
    float4 cNew = step(0.5, cBase);
    return max(lerp(cBase * cBlend * 2.0, 1.0 - 2.0 * (1.0 - cBase) * (1.0 - cBlend), cNew), float4(0.0, 0.0, 0.0, 0.0));
}

float4 ScreenHDR(float4 cBase, float4 cBlend)
{
    return max(1.0 - (1.0 - cBase) * (1.0 - cBlend), float4(0.0, 0.0, 0.0, 0.0));
}

// The glow at a point: VPX's falloff from the centre, from the light's colour
// to its edge colour.
float4 backdrop_glow(float2 table_pos)
{
    float len = length(light_center_range.xyz - float3(table_pos, 0.0)) * light_center_range.w;
    float atten = pow(1.0 - saturate(len), light_color2_power.w);
    float3 lcolor = lerp(light_color2_power.rgb, light_color_intensity.rgb, sqrt(len));
    return float4(lcolor * (atten * light_color_intensity.w), saturate(atten * light_color_intensity.w));
}

// A classic light: its image (unlit look) with the glow laid over it, or
// without an image its colour plus the glow. Drawn opaque, as in VPX.
float4 backdrop_light_fs(OverlayVaryings v) : SV_Target0
{
    float4 glow = light_color_intensity.w != 0.0 ? backdrop_glow(v.color.xy) : float4(0.0, 0.0, 0.0, 0.0);
    if (light_mode.x != 0.0)
    {
        float4 pixel = font_texture.Sample(font_sampler, v.uv);
        float4 color = pixel;
        if (light_color_intensity.w != 0.0)
        {
            color += glow;
            color = OverlayHDR(pixel, color);
            color = ScreenHDR(pixel, color);
        }
        return float4(color.rgb, 1.0);
    }
    return float4(light_color_intensity.rgb + glow.rgb, 1.0);
}

// A bulb light. VPX blends its glow L as dst * (1 + m L) + (1 - m) L, with m
// its modulate vs add, using negative colours in a floating-point picture: in
// a single pass in VPX's look (term 2). The first look's picture clamps at 1,
// so the two terms are drawn in turn: m L multiplied in (blend: dst colour,
// one), then (1 - m) L added (one, one). Exact until either term passes 1.
float4 backdrop_bulb_fs(OverlayVaryings v) : SV_Target0
{
    float3 l = backdrop_glow(v.color.xy).rgb;
    float m = light_mode.y;
    if (light_mode.z == 2.0)
        return float4(-m * l, 1.0 / m - 1.0);
    return float4(l * (light_mode.z == 0.0 ? m : 1.0 - m), 1.0);
}
