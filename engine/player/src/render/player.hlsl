// The player's shaders for SDL3 GPU on Direct3D 12, compiled at startup by FXC.
// Registers follow SDL3 GPU's rules: vertex-stage uniforms in space1,
// fragment-stage textures and samplers in space2, fragment uniforms in space3.
// Matrices are VPX's (row vectors: v * M).

cbuffer Frame : register(b0, space1)
{
    row_major float4x4 rot_view_proj; // VPX's matRotViewProj: the view without its move, then the projection
    float4 camera;                    // VPX's cameraPosWorld, taken off each point before it's projected
    float4 eye;
    float4 clip_plane; // drawing a mirror: only what's on the plane's side (0, 0, 0, 1 otherwise)
};

cbuffer Draw : register(b1, space1)
{
    row_major float4x4 model;
};

cbuffer Material : register(b0, space3)
{
    float4 base_color;
    float4 emissive;      // rgb; a: 1 when the material has a texture
    float4 params;        // x: alpha cutoff (0 = none), y: metallic, z: roughness, w: 1 for the ball
    float4 light0;        // xyz: the table's first light
    float4 light1;
    float4 extra;         // x: how far lighting is switched off (VPX's BlendDisableLighting); y: 1 for a solid part
                          // in VPX's look, which marks its pixels for tonemapping in the picture's alpha
    float4 part_color_alpha;   // the part's colour and opacity (VPX's staticColor_Alpha)
};

Texture2D<float4> base_texture : register(t0, space2);
SamplerState base_sampler : register(s0, space2);

struct VertexInput
{
    float3 position : TEXCOORD0;
    float3 normal : TEXCOORD1;
    float2 uv : TEXCOORD2;
};

struct Varyings
{
    float4 clip : SV_Position;
    float3 world : TEXCOORD0;
    float3 normal : TEXCOORD1;
    float2 uv : TEXCOORD2;
    float3 to_eye : TEXCOORD3;
    float clip_distance : SV_ClipDistance0;
};

Varyings vs_main(VertexInput input)
{
    float4 world = mul(float4(input.position, 1.0), model);
    Varyings result;
    // VPX's camera-relative projection (vs_basic.sc): the point less the camera's place, then
    // turned and projected, so the numbers stay small
    result.clip = mul(float4(world.xyz - camera.xyz, 1.0), rot_view_proj);
    result.world = world.xyz;
    result.normal = mul(float4(input.normal, 0.0), model).xyz;
    result.uv = input.uv;
    result.to_eye = eye.xyz - world.xyz;
    // VPX's reflection probes: the mirrored view keeps what's above the mirror (vs_basic.sc's CLIP)
    result.clip_distance = dot(clip_plane, float4(world.xyz, 1.0));
    return result;
}

// A kicker's plate, ported from Visual Pinball X build 5436 (commit af26b2d, license:GPLv3+,
// Copyright (C) 2000-2026 Visual Pinball development team and contributors): vs_basic.sc's KICKER.
// Drawn where it lies, but at the depth of a point 30 below, and over whatever is there (the
// renderer draws it before the other moving parts, depth test off): it punches a hole through the
// playfield, so the kicker's cup shows sunk in it.
Varyings vs_kicker(VertexInput input)
{
    Varyings result = vs_main(input);
    float4 below = mul(float4(input.position.xy, input.position.z - 30.0, 1.0), model);
    result.clip.z = mul(float4(below.xyz - camera.xyz, 1.0), rot_view_proj).z;
    return result;
}

float3 light_from(float3 light, float3 world, float3 n, float3 v, float shininess, float spec_strength)
{
    float3 l = normalize(light - world);
    float3 h = normalize(l + v);
    float diffuse = abs(dot(n, l)); // both faces lit: meshes are drawn without culling
    float specular = pow(saturate(abs(dot(n, h))), shininess) * spec_strength;
    return float3(diffuse, specular, 0.0);
}

float4 fs_main(Varyings v) : SV_Target0
{
    float4 color = base_color;
    if (emissive.a > 0.5)
    {
        float4 texel = base_texture.Sample(base_sampler, v.uv);
        // VPX's alpha test (fs_basic.sc): on the picture's own alpha, before the material's opacity
        if (params.x > 0.0 && texel.a < params.x)
            discard;
        color *= texel;
    }
    // a part scaled to zero along an axis (extra.z set): VPX can't invert its matrix, so its
    // normals aren't numbers, and with any lighting on it comes out black in VPX's picture
    if (extra.z != 0.0 && extra.x < 1.0)
        return float4(0.0, 0.0, 0.0, 1.0);

    float3 n = normalize(v.normal);
    float3 to_eye = normalize(v.to_eye);
    float roughness = clamp(params.z, 0.05, 1.0);
    float shininess = lerp(256.0, 8.0, roughness);
    float spec_strength = lerp(0.08, 0.6, params.y) * (1.0 - 0.6 * roughness);
    if (params.w > 0.5)
    {
        shininess = 512.0;
        spec_strength = 1.6;
    }
    float3 a = light_from(light0.xyz, v.world, n, to_eye, shininess, spec_strength);
    float3 b = light_from(light1.xyz, v.world, n, to_eye, shininess, spec_strength);
    float diffuse = 0.5 * (a.x + b.x);
    float specular = 0.5 * (a.y + b.y);
    float3 lit = color.rgb * (0.35 + 0.75 * diffuse) + specular + emissive.rgb;
    // VPX's material.sh: lighting switched off blends towards the unlit colour
    if (extra.x != 0.0)
        lit = lerp(lit, color.rgb, extra.x);
    // VPX's fs_basic.sc: a primitive's colour and opacity multiply the result
    float4 result = float4(lit, color.a) * part_color_alpha;
    if (extra.y > 0.5)
        result.a = 1.0;
    return result;
}

// A primitive set to add its light (a light map), ported from Visual Pinball X

// VPX's raytraced ball shadows, ported from Visual Pinball X build 5436 (commit af26b2d, license:GPLv3+,
// Copyright (C) 2000-2026 Visual Pinball development team and contributors): src/shaders/bgfx/ball_shadows.sh.
// A light set to cast them is darkened where a ball stands between it and the surface: on its halo, and
// on the parts and flashers that are its light map. The same slot in every shader that uses it.
cbuffer BallShadows : register(b2, space3)
{
    float4 lightCenter_doShadow; // the light's place; w: 1 to shadow
    float4 balls[8];             // each ball's place and radius; a radius of 0 ends them
};

float get_light_ball_shadow(float3 light_pos, float3 light_dir, float light_dist)
{
    float result = 1.0;
    [loop] for (int i = 0; i < 8; i++)
    {
        float ball_r = balls[i].w;
        [branch] if (ball_r == 0.0) // the first ball that isn't there ends them
            return result;
        float3 light_ball_ray = balls[i].xyz - light_pos;
        float dot_lbr_lr_divld = dot(light_ball_ray, light_dir) / (light_dist * light_dist);
        // no shadow behind the light, nor before the ball
        [branch] if (dot_lbr_lr_divld > 0.0 && dot_lbr_lr_divld < 1.0)
        {
            float3 dist = light_ball_ray - dot_lbr_lr_divld * light_dir;
            float d2 = length(dist);
            const float light_r = 5.0; // the light's radius, in VPX's units
            float smoothness = light_r - light_r * dot_lbr_lr_divld;
            // fading out as the light comes inside the ball
            float light_inside_ball_sqr = saturate((light_ball_ray.x * light_ball_ray.x + light_ball_ray.y * light_ball_ray.y) / (ball_r * ball_r));
            result *= 1.0 + light_inside_ball_sqr * (-1.0 + 0.1 + 0.9 * smoothstep(ball_r - smoothness, ball_r + smoothness, d2));
        }
    }
    return result;
}

// The shadow at a point of the table, for the light the draw names (none when it names none)
float ball_shadow(float3 world)
{
    [branch] if (lightCenter_doShadow.w == 0.0)
        return 1.0;
    float3 light_dir = world - lightCenter_doShadow.xyz;
    return get_light_ball_shadow(lightCenter_doShadow.xyz, light_dir, length(light_dir));
}

// build 5436 (commit af26b2d, license:GPLv3+, Copyright (C) 2000-2026 Visual
// Pinball development team and contributors): src/shaders/bgfx/fs_unshaded.sc,
// with the ball shadows of a light map's light. Unlit: its colour and
// opacity, the colour already multiplied by the opacity, times its picture;
// added on top by the blend (source alpha, one).
float4 fs_unshaded(Varyings v) : SV_Target0
{
    float4 result = part_color_alpha;
    if (emissive.a > 0.5)
        result *= base_texture.Sample(base_sampler, v.uv);
    result.rgb *= ball_shadow(v.world);
    return result;
}

// A light's glow on the table, added to what is below it: VPX's light shader
// (fs_light.sc), without its ball shadows. The material slots carry the
// light: base_color = colour and intensity, emissive = edge colour and
// falloff power, params = center and 1 / falloff radius, light0.x = how lit.
float4 fs_light(Varyings v) : SV_Target0
{
    float3 light_dir = v.world - params.xyz;
    float len = length(light_dir) * params.w;
    float atten = pow(1.0 - saturate(len), emissive.a);
    float3 lcolor = lerp(emissive.rgb, base_color.rgb, sqrt(len));
    return float4(lcolor * atten * base_color.a * light0.x, 1.0);
}

// A bulb light's halo, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and
// contributors): src/shaders/bgfx/fs_light.sc (bulb_light, and with its ball
// shadows when the light casts them). VPX blends the glow L as
// dst * (1 + m L) + (1 - m) L, with m its
// modulate vs add, using negative colours in a floating-point picture: the
// colour -m L and the alpha 1 / m - 1, blended as dst (1 - src) - src alpha
// (source alpha, one minus source colour, reverse subtract). VPX's look draws
// it so, in a single pass (light0.y = 2). The first look's picture clamps at
// 1, so there the terms are drawn in turn: m L multiplied in (blend: dst
// colour, one; light0.y = 0), then (1 - m) L added (one, one; light0.y = 1).
// The material slots carry the light: base_color = colour and intensity,
// emissive = edge colour and falloff power, params = where it shines from and
// 1 / falloff radius, light0.x = m.
float4 fs_bulb(Varyings v) : SV_Target0
{
    float len = length(v.world - params.xyz) * params.w;
    float atten = pow(1.0 - saturate(len), emissive.a);
    float3 lcolor = lerp(emissive.rgb, base_color.rgb, sqrt(len));
    float3 l = lcolor * (atten * base_color.a) * ball_shadow(v.world);
    float m = light0.x;
    if (light0.y == 2.0)
        return float4(-m * l, 1.0 / m - 1.0);
    return float4(l * (light0.y == 0.0 ? m : 1.0 - m), 1.0);
}

// A flasher, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and
// contributors): src/shaders/bgfx/fs_flasher.sc and the Overlay, Multiply,
// Additive and Screen blends of src/shaders/bgfx/common.sh, with the ball
// shadows of a light map's light. Unlit: its colour times its pictures.
// VPX blends an additive flasher's light L as dst * (1 + m L) + (1 - m) L
// using negative colours in a floating-point picture, as for bulb lights: in
// a single pass in VPX's look (term 3); in the first look's picture, which
// clamps at 1, the terms are drawn in turn: m L multiplied in (blend: dst
// colour, one; term 1), then (1 - m) L added (one, one; term 2).
cbuffer Flasher : register(b0, space3)
{
    float4 static_color_alpha;   // colour and opacity (VPX's staticColor_Alpha)
    float4 alpha_test_filter_add; // x, y: pictures' alpha tests; z: filter; w: additive
    float4 amount_mva_mode;       // x: filter amount; y: modulate vs add; z: mode (0 one picture, 1 two, 2 none)
    float4 flasher_term;          // x: 0 blended, 1 the multiplied term, 2 the added term, 3 VPX's single pass
};

Texture2D<float4> flasher_a : register(t0, space2);
Texture2D<float4> flasher_b : register(t1, space2);
SamplerState flasher_a_sampler : register(s0, space2);
SamplerState flasher_b_sampler : register(s1, space2);

float4 flasher_overlay(float4 cBase, float4 cBlend)
{
    float4 cNew = step(0.5, cBase);
    return max(lerp(cBase * cBlend * 2.0, 1.0 - 2.0 * (1.0 - cBase) * (1.0 - cBlend), cNew), float4(0.0, 0.0, 0.0, 0.0));
}

float4 flasher_screen(float4 cBase, float4 cBlend)
{
    return max(1.0 - (1.0 - cBase) * (1.0 - cBlend), float4(0.0, 0.0, 0.0, 0.0));
}

float4 fs_flasher(Varyings v) : SV_Target0
{
    float4 pixel1 = float4(1.0, 1.0, 1.0, 1.0);
    float4 pixel2 = float4(1.0, 1.0, 1.0, 1.0);
    float mode = amount_mva_mode.z;
    if (mode < 2.0)
    {
        pixel1 = flasher_a.Sample(flasher_a_sampler, v.uv);
        if (pixel1.a <= alpha_test_filter_add.x)
            discard;
    }
    if (mode == 1.0)
    {
        pixel2 = flasher_b.Sample(flasher_b_sampler, v.uv);
        if (pixel2.a <= alpha_test_filter_add.y)
            discard;
    }
    float4 result = static_color_alpha;
    if (mode == 0.0)
        result *= pixel1;
    if (mode == 1.0)
    {
        float filter = alpha_test_filter_add.z;
        float amount = amount_mva_mode.x;
        if (filter == 2.0)
            result *= flasher_overlay(pixel1, pixel2);
        if (filter == 3.0)
            result *= pixel1 * pixel2 * amount;
        if (filter == 1.0)
            result *= pixel1 + pixel2 * amount;
        if (filter == 4.0)
            result *= flasher_screen(pixel1, pixel2);
    }
    result.rgb *= ball_shadow(v.world);
    if (flasher_term.x == 0.0)
        return float4(result.rgb, saturate(result.a));
    float m = amount_mva_mode.y;
    if (flasher_term.x == 3.0)
        return float4(result.rgb * (-m * result.a), 1.0 / m - 1.0);
    float3 l = result.rgb * result.a;
    return float4(l * (flasher_term.x == 1.0 ? m : 1.0 - m), 1.0);
}
