// VPX's look: the basic material, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and contributors):
// src/shaders/bgfx/fs_basic.sc and material.sh, with the environment lookups of common.sh
// (ray_to_equirectangular_uv and its approximations), and the environment's irradiance from
// fs_pp_irradiance.sc (Renderer.cpp computes it once, on the GPU), and light from below: bulb lights
// drawn into a small blurred picture first (Renderer::DrawBulbLightBuffer), which see-through parts
// add, and playfield reflections: the mirror's picture (the table drawn mirrored first, as VPX's
// reflection probe draws it) added where a surface faces the mirror, at its strength (fs_basic.sc's
// compute_reflection), or as a pass of its own over a see-through part (its REFL variant). Compiled
// with REFRACTION defined, a part drawn bent: the table as drawn before it (its refraction probe's
// picture, RenderProbe::RenderScreenSpaceTransparency) seen through it where the ray bent by its
// thickness lands, mixed in by its opacity (fs_basic.sc's compute_refraction). Compiled with
// CLASSIC_LIGHT defined, it also gives a classic light with a picture of its own
// (fs_classic_light.sc).
//
// VPX lights in view space; its view is a rigid turn of the table (the layback and the scene's
// scale go into its projection), so the same lighting is done here in the table's own space.
//
// VPX 5436 sets the ambient colour and light range (cAmbient_LightRange) for its ball shader only;
// the basic shader keeps them at zero, so its two point lights add nothing there but the
// "disable lighting" blend, and surfaces are lit by the table's environment picture. The renderer
// hands the same values, and the code is VPX's.
//
// The ball (fs_ball.sc, compiled with BALL defined): its own picture reflected (the table's, or VPX's
// default environment), its scratches, the playfield mirrored in it from the previous frame, and the
// table's two lights and up to eight nearby ones (VPX gives the ball shader their range and the
// ambient colour); its scratches turn with it (its turn is in its model matrix, as in Ball::Render). Not yet:
// VPX's stretch correction (a setting VPX leaves off).
//
// Registers follow SDL3 GPU's rules (see player.hlsl): the vertex stage is player.hlsl's vs_main.
// Compiled with IRRADIANCE defined, the file gives the irradiance pass instead, whose picture sits
// in the same register as the material's own.

#define PI 3.1415926535897932384626433832795
#define FLT_MIN_VALUE 0.00006103515625

float sqr(float v)
{
    return v * v;
}

float acos_approx_divPI(float v)
{
    float x = abs(v);
    if (1.0 - x <= FLT_MIN_VALUE)
        return (v >= 0.0) ? 0.0 : 1.0;
    float res = ((-0.155972 / PI) * x + (1.56467 / PI)) * sqrt(1.0 - x);
    return (v >= 0.0) ? res : 1.0 - res;
}

float atan2_approx_div2PI(float y, float x)
{
    float abs_y = abs(y);
    float abs_x = abs(x);
    if (abs_x <= FLT_MIN_VALUE && abs_y <= FLT_MIN_VALUE)
        return 0.0;
    float r = (abs_x - abs_y) / (abs_x + abs_y);
    float angle = ((x < 0.0) ? (3.0 / 8.0) : (1.0 / 8.0)) + (0.211868 / (2.0 * PI) * r * r - 0.987305 / (2.0 * PI)) * ((x < 0.0) ? -r : r);
    return (y < 0.0) ? -angle : angle;
}

float2 ray_to_equirectangular_uv(float3 ray)
{
    return float2(0.5 + atan2_approx_div2PI(ray.y, ray.x), acos_approx_divPI(ray.z));
}

#ifndef IRRADIANCE

struct Varyings
{
    float4 clip : SV_Position;
    float3 world : TEXCOORD0;
    float3 normal : TEXCOORD1;
    float2 uv : TEXCOORD2;
    float3 to_eye : TEXCOORD3;
    float clip_distance : SV_ClipDistance0;
};

#ifdef BALL

cbuffer VpxBall : register(b0, space3)
{
    float4 cBase_Alpha;                    // the ball's colour
    float4 Roughness_WrapL_Edge_Thickness; // 2^(10 R + 1), 0, 1, 0.05: R rougher the farther the nearest light
    float4 cAmbient_LightRange;            // the ambient colour and the lights' range
    float4 invTableRes_reflection;         // z: how much it mirrors the playfield
    float4 w_h_disableLighting;            // xy: 1.5 texels of the previous frame; z: lighting off; w: the previous frame is there
    float4 ball_flags;                     // x: a sphere map (else equirectangular); y: decal mode (else scratches)
    row_major float4x4 ball_view;          // the camera's turn, for a sphere map
    row_major float4x4 ball_view_proj;     // to find the mirrored playfield in the previous frame
    float4 lightPos[10];                   // the table's two lights, then the nearest eight that shine on balls
    float4 lightEmission[10];
};

static const float4 fDisableLighting_top_below = float4(0.0, 0.0, 0.0, 0.0);

cbuffer VpxScene : register(b1, space3)
{
    float4 fenvEmissionScale_TexWidth;
    float4 scene_ambient_light_range;
    float4 scene_light_pos[2];
    float4 scene_light_emission[2];
    float4 scene_w_h_transmission;
};

Texture2D<float4> tex_ball_color : register(t0, space2);
SamplerState ball_sampler : register(s0, space2);
Texture2D<float4> tex_diffuse_env : register(t2, space2);
SamplerState diffuse_env_sampler : register(s2, space2);
Texture2D<float4> tex_ball_decal : register(t3, space2);
SamplerState decal_sampler : register(s3, space2);
Texture2D<float4> tex_ball_playfield : register(t4, space2);
SamplerState playfield_sampler : register(s4, space2);

#else

cbuffer VpxMaterial : register(b0, space3)
{
    float4 cBase_Alpha;
    float4 cGlossy_ImageLerp;
    float4 cClearcoat_EdgeAlpha;
    float4 Roughness_WrapL_Edge_Thickness; // x: glossy power, 2^(10 roughness + 1)
    float4 fDisableLighting_top_below;
    float4 staticColor_Alpha;
    float4 vpx_flags; // x: metal; y: textured; z: texels below this alpha vanish (0 none); w: 1 marks a solid part for tonemapping
    float4 mirrorNormal_factor; // the mirror it shows: xyz its plane's normal, w how strongly (0: none)
    float4 normal_map_flags;    // x: a normal map; y: in the primitive's own space (else tangent space)
    float4 object_normal[3];    // the rows taking the primitive's own normals to the table's
};

cbuffer VpxScene : register(b1, space3)
{
    float4 fenvEmissionScale_TexWidth; // x: the environment's emission scale; y: its picture's height
    float4 cAmbient_LightRange;
    float4 lightPos[2];
    float4 lightEmission[2];
    float4 w_h_transmission;           // xy: 1 / the picture's size; z: the bulb light buffer has light
    float4 view_rows[3];               // the view's turn: VPX's view space, for the normal maps' frame
};

Texture2D<float4> tex_base_color : register(t0, space2);
SamplerState base_sampler : register(s0, space2);
Texture2D<float4> tex_env : register(t1, space2);
SamplerState env_sampler : register(s1, space2);
Texture2D<float4> tex_diffuse_env : register(t2, space2);
SamplerState diffuse_env_sampler : register(s2, space2);
Texture2D<float4> tex_base_transmission : register(t3, space2);
SamplerState transmission_sampler : register(s3, space2);
Texture2D<float4> tex_reflection : register(t4, space2);
SamplerState reflection_sampler : register(s4, space2);
Texture2D<float4> tex_base_normalmap : register(t5, space2);
SamplerState normalmap_sampler : register(s5, space2);

#ifdef REFRACTION
// a part drawn bent: its refraction probe's picture, the table as drawn before it, and its depth
Texture2D<float4> tex_refraction : register(t6, space2);
SamplerState refraction_sampler : register(s6, space2);
Texture2D<float> tex_probe_depth : register(t7, space2);
SamplerState probe_depth_sampler : register(s7, space2);

cbuffer VpxRefraction : register(b3, space3)
{
    row_major float4x4 refraction_view_proj; // VPX's matProj, here from the table's own space
    float4 refractionTint_thickness;
    float4 refraction_flags;                 // x: the probe has no depth (multisampled): nothing held back
};

// Where the ray, bent into the part (air to plastic, 1 / 1.5) and gone its thickness, lands in the
// probe's picture, unless what's there stands in front of the part (then straight behind); tinted
float3 compute_refraction(float3 pos, float3 frag, float3 N, float3 V)
{
    float3 R = refract(V, N, 1.0 / 1.5);
    float3 refracted_pos = pos + refractionTint_thickness.w * R;
    float4 proj = mul(float4(refracted_pos, 1.0), refraction_view_proj);
    float2 uv = float2(0.5, 0.5) + float2(proj.x, -proj.y) * (0.5 / proj.w);
    if (refraction_flags.x == 0.0 && tex_probe_depth.SampleLevel(probe_depth_sampler, uv, 0.0) < frag.z)
        uv = frag.xy * w_h_transmission.xy;
    return refractionTint_thickness.rgb * tex_refraction.SampleLevel(refraction_sampler, uv, 0.0).rgb;
}
#endif

#endif

// Lower alpha towards the silhouette (blending: how much; thickness: how fast)
float GeometricOpacity(float NdotV, float alpha, float blending, float thickness)
{
    float x = abs(NdotV);
    float g = blending * (1.0 - (x / (x * (1.0 - thickness) + thickness)));
    return lerp(alpha, 1.0, g);
}

float3 FresnelSchlick(float3 spec, float LdotH, float edge)
{
    return spec + (float3(edge, edge, edge) - spec) * pow(1.0 - LdotH, 5.0);
}

float3 DoPointLight(float3 pos, float3 N, float3 V, float3 diffuse, float3 glossy, float edge, float glossyPower, int i, bool is_metal)
{
    float3 lightDir = lightPos[i].xyz - pos;
    float3 L = normalize(lightDir);
    float NdotL = dot(N, L);
    float3 Out = float3(0.0, 0.0, 0.0);

    // diffuse: Lambert with the optional wrap
    if (!is_metal && (NdotL + Roughness_WrapL_Edge_Thickness.y > 0.0))
        Out = diffuse * ((NdotL + Roughness_WrapL_Edge_Thickness.y) / sqr(1.0 + Roughness_WrapL_Edge_Thickness.y));

    // glossy: VPX's modified Ashikhmin/Blinn
    [branch] if (NdotL > 0.0)
    {
        float3 H = normalize(L + V);
        float NdotH = dot(N, H);
        float LdotH = dot(L, H);
        float VdotH = dot(V, H);
        if ((NdotH > 0.0) && (LdotH > 0.0) && (VdotH > 0.0))
            Out += FresnelSchlick(glossy, LdotH, edge) * (((glossyPower + 1.0) / (8.0 * VdotH)) * pow(NdotH, glossyPower));
    }

    // VPX's ranged falloff (zero for this shader in VPX 5436, its range being zero)
    float sqrl_lightDir = dot(lightDir, lightDir);
    float fAtten = saturate(1.0 - sqrl_lightDir * sqrl_lightDir / (cAmbient_LightRange.w * cAmbient_LightRange.w * cAmbient_LightRange.w * cAmbient_LightRange.w));
    fAtten = fAtten * fAtten / (sqrl_lightDir + 1.0);

    float3 ambient = glossy;
    if (!is_metal)
        ambient += diffuse;

    float3 result = Out * lightEmission[i].rgb * fAtten + ambient * cAmbient_LightRange.xyz;
    if (fDisableLighting_top_below.x != 0.0)
        return lerp(result, diffuse, fDisableLighting_top_below.x);
    return result;
}

float3 DoEnvmapDiffuse(float3 N, float3 diffuse)
{
    float3 env = tex_diffuse_env.SampleLevel(diffuse_env_sampler, ray_to_equirectangular_uv(N), 0.0).xyz;
    return diffuse * env * fenvEmissionScale_TexWidth.x;
}

#ifndef BALL

// A normal map's normal (fs_basic.sc's normal_map): in the primitive's own space, taken to the
// table's; else in tangent space, by VPX's frame from the screen-space derivatives of the view
// direction and the texture coordinates (TBN_trafo, its texture's v pointing down as on Direct3D).
// As VPX's BGFX build compiles it for Direct3D (shaderc's output): in VPX's view space (the table
// turned by view_rows, or a mirror's view), dFdy as ddy of the value negated (GL's y up), and
// mat3(T, B, N) as HLSL's float3x3, rows, so the frame applies transposed; the normal then
// turned back to the table's space
float3 normal_map(float3 N, float3 V, float2 uv)
{
    float3 tn = tex_base_normalmap.Sample(normalmap_sampler, uv).xyz * (255.0 / 127.0) - (128.0 / 127.0);
    [branch] if (normal_map_flags.y != 0.0)
    {
        float3 o = float3(tn.x, tn.y, -tn.z);
        return normalize(o.x * object_normal[0].xyz + o.y * object_normal[1].xyz + o.z * object_normal[2].xyz);
    }
    float3x3 view = float3x3(view_rows[0].xyz, view_rows[1].xyz, view_rows[2].xyz);
    float3 Vv = mul(V, view);
    float3 Nv = mul(N, view);
    float3 dpx = ddx(Vv);
    float3 dpy = ddy(-Vv);
    float2 duvx = ddx(float2(uv.x, -uv.y));
    float2 duvy = ddy(-float2(uv.x, -uv.y));
    float3 dp2perp = cross(Nv, dpy);
    float3 dp1perp = cross(dpx, Nv);
    float3 T = dp2perp * duvx.x + dp1perp * duvy.x;
    float3 B = dp2perp * duvx.y + dp1perp * duvy.y;
    // VPX scales N instead of normalizing the frame, as the result is normalized anyway
    float3x3 frame = float3x3(T, B, Nv * sqrt(max(dot(T, T), dot(B, B))));
    return normalize(mul(view, mul(frame, tn)));
}

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

// The mirror's picture where a surface faces the mirror (fs_basic.sc's compute_reflection): only
// added, not mixed by the fresnel term; the smoothstep's bounds are VPX's, from visual tests. Half
// a texel off the pixel's centre, so the picture's filtering blurs it a little
float3 compute_reflection(float2 frag, float3 N)
{
    return smoothstep(0.5, 0.9, dot(mirrorNormal_factor.xyz, N)) * mirrorNormal_factor.w
         * tex_reflection.Sample(reflection_sampler, (frag + 0.5) * w_h_transmission.xy).rgb;
}

// VPX's very crude approximation of the glossy lobe, by mip level
float3 DoEnvmapGlossy(float3 N, float3 V, float2 Ruv, float3 glossy, float glossyPower)
{
    float mip = min(log2(fenvEmissionScale_TexWidth.y * sqrt(3.0)) - 0.5 * log2(glossyPower + 1.0), log2(fenvEmissionScale_TexWidth.y) - 1.0);
    float3 env = tex_env.SampleLevel(env_sampler, Ruv, mip).xyz;
    return glossy * env * fenvEmissionScale_TexWidth.x;
}

float3 DoEnvmap2ndLayer(float3 color1stLayer, float3 pos, float3 N, float3 V, float NdotV, float2 Ruv, float3 specular)
{
    float3 w = FresnelSchlick(specular, NdotV, Roughness_WrapL_Edge_Thickness.z);
    float3 env = tex_env.SampleLevel(env_sampler, Ruv, 0.0).xyz;
    return lerp(color1stLayer, env * fenvEmissionScale_TexWidth.x, w);
}

// The environment and the table's two point lights on the diffuse, glossy and clearcoat layers
// (material.sh's lightLoop); both faces lit, as VPX lights them
float3 lightLoop(float3 pos, float3 N, float3 V, float3 diffuse, float3 glossy, float3 specular, float edge, bool is_metal)
{
    float3 color = float3(0.0, 0.0, 0.0);
    float NdotV = dot(N, V);
    if (NdotV < 0.0)
    {
        N = -N;
        NdotV = -NdotV;
    }
    NdotV = min(NdotV, 1.0);
    float diffuseMax = max(diffuse.x, max(diffuse.y, diffuse.z));
    float glossyMax = max(glossy.x, max(glossy.y, glossy.z));
    float specularMax = max(specular.x, max(specular.y, specular.z));
    float sum = diffuseMax + glossyMax;
    // energy conservation, unless lighting is switched off
    if (sum > 1.0 && fDisableLighting_top_below.x < 1.0)
    {
        float invsum = 1.0 / sum;
        diffuse *= invsum;
        glossy *= invsum;
    }

    [branch] if ((!is_metal && (diffuseMax > 0.0)) || (glossyMax > 0.0))
    {
        // VPX's old behaviour kept: lighting switched off counts the diffuse colour twice
        [branch] if (fDisableLighting_top_below.x == 1.0)
            color += 2.0 * diffuse;
        else
        {
            color += DoPointLight(pos, N, V, diffuse, glossy, edge, Roughness_WrapL_Edge_Thickness.x, 0, is_metal);
            color += DoPointLight(pos, N, V, diffuse, glossy, edge, Roughness_WrapL_Edge_Thickness.x, 1, is_metal);
        }
    }

    [branch] if (!is_metal && (diffuseMax > 0.0))
        color += DoEnvmapDiffuse(N, diffuse);
    [branch] if ((glossyMax > 0.0) || (specularMax > 0.0))
    {
        float3 R = (2.0 * NdotV) * N - V;
        float2 Ruv = ray_to_equirectangular_uv(R);
        if (glossyMax > 0.0)
            color += DoEnvmapGlossy(N, V, Ruv, glossy, Roughness_WrapL_Edge_Thickness.x);
        if (specularMax > 0.0)
            color = DoEnvmap2ndLayer(color, pos, N, V, NdotV, Ruv, specular);
    }
    return color;
}

float4 fs_vpx(Varyings v) : SV_Target0
{
    bool is_metal = vpx_flags.x != 0.0;
    float4 pixel;
    float3 diffuse;
    float3 glossy;
    [branch] if (vpx_flags.y != 0.0)
    {
        pixel = tex_base_color.Sample(base_sampler, v.uv);
        // VPX's alpha test: on the picture's own alpha, before the material's opacity
        if (vpx_flags.z > 0.0 && pixel.a < vpx_flags.z)
            discard;
        pixel.a *= cBase_Alpha.a;
        // with lighting on, a high-range picture is held to 0..1
        if (fDisableLighting_top_below.x < 1.0)
            pixel.rgb = saturate(pixel.rgb);
        diffuse = pixel.rgb * cBase_Alpha.rgb;
        glossy = is_metal ? diffuse : (pixel.rgb * cGlossy_ImageLerp.w + (1.0 - cGlossy_ImageLerp.w)) * cGlossy_ImageLerp.rgb * 0.08;
    }
    else
    {
        pixel = cBase_Alpha;
        diffuse = cBase_Alpha.rgb;
        glossy = is_metal ? cBase_Alpha.rgb : cGlossy_ImageLerp.rgb * 0.08;
    }
    float3 specular = cClearcoat_EdgeAlpha.rgb * 0.08;
    float edge = is_metal ? 1.0 : Roughness_WrapL_Edge_Thickness.z;

    // a part scaled to zero along an axis (z: set): VPX can't invert its matrix, so its normals
    // aren't numbers, and with any lighting on it comes out nearly black in VPX's picture (dim, by
    // the graphics card's rules for such numbers; black here, closer than lit, and safer)
    [branch] if (fDisableLighting_top_below.z != 0.0 && fDisableLighting_top_below.x < 1.0)
        return float4(0.0, 0.0, 0.0, 1.0);

    float3 V = normalize(v.to_eye);
    float3 N = normalize(v.normal);
    // a textured part's normal map (VPX: normal_map(N, -V, uv), the view direction towards it)
    [branch] if (normal_map_flags.x != 0.0 && vpx_flags.y != 0.0)
        N = normal_map(N, -V, v.uv);
    float4 color = float4(lightLoop(v.world, N, V, diffuse, glossy, specular, edge, is_metal), pixel.a);

    [branch] if (color.a < 1.0)
    {
        color.a = GeometricOpacity(dot(N, V), color.a, cClearcoat_EdgeAlpha.w, Roughness_WrapL_Edge_Thickness.w);
        // light from below, from the bulb lights drawn first and blurred ("sqrt = magic", VPX's note)
        if (fDisableLighting_top_below.y < 1.0 && w_h_transmission.z != 0.0)
            color.rgb += lerp(sqrt(diffuse) * tex_base_transmission.SampleLevel(transmission_sampler, v.clip.xy * w_h_transmission.xy, 0.0).rgb * color.a,
                              float3(0.0, 0.0, 0.0), fDisableLighting_top_below.y);
    }

    // a light map's ball shadows
    color.rgb *= ball_shadow(v.world);

    [branch] if (mirrorNormal_factor.w > 0.0)
        color.rgb += compute_reflection(v.clip.xy, N);

#ifdef REFRACTION
    // what's behind, bent through the part, showing through by its opacity (drawn unblended:
    // tinted even where the part is clear)
    color.rgb = lerp(compute_refraction(v.world, v.clip.xyz, N, V), color.rgb, color.a);
    color.a = 1.0;
#endif

    color *= staticColor_Alpha;
    if (vpx_flags.w > 0.5)
        color.a = 1.0;
    return color;
}

// Only the mirror, added over a see-through part already drawn (fs_basic.sc's REFL variant)
float4 fs_vpx_reflection(Varyings v) : SV_Target0
{
    float3 color = compute_reflection(v.clip.xy, normalize(v.normal));
    return float4(color * staticColor_Alpha.rgb, staticColor_Alpha.a);
}

#ifdef CLASSIC_LIGHT

// A classic light with a picture of its own (fs_classic_light.sc, its TEX variant, and the
// OverlayHDR and ScreenHDR blends of common.sh): its picture lit as the material of what it sits
// on (the material slots), then, while it gives light, its glow added and the sum laid over the
// picture, overlay then screen, so the glow shows where the picture is bright and not where it's
// dark. The renderer adds the result to the table (one, one). VPX compiles it without ball shadows.
cbuffer VpxClassicLight : register(b3, space3)
{
    float4 lightCenter_maxRange;      // where it shines from; w: 1 / its falloff radius
    float4 lightColor2_falloff_power; // the edge's colour; w: the falloff's power
    float4 lightColor_intensity;      // the centre's colour; w: its intensity now (0: off)
    float4 lightingOff;               // x: its picture as it is, unlit (image mode)
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

float4 fs_vpx_classic_light(Varyings v) : SV_Target0
{
    bool is_metal = vpx_flags.x != 0.0;
    float4 pixel = tex_base_color.SampleLevel(base_sampler, v.uv, 0.0);
    float4 color;
    [branch] if (lightingOff.x != 0.0)
        color = pixel;
    else
    {
        pixel.rgb = saturate(pixel.rgb);
        float3 diffuse = pixel.rgb * cBase_Alpha.rgb;
        float3 glossy = is_metal ? diffuse : pixel.rgb * cGlossy_ImageLerp.rgb * 0.08;
        float3 specular = cClearcoat_EdgeAlpha.rgb * 0.08;
        float edge = is_metal ? 1.0 : Roughness_WrapL_Edge_Thickness.z;
        color.rgb = lightLoop(v.world, normalize(v.normal), normalize(v.to_eye), diffuse, glossy, specular, edge, is_metal);
        color.a = pixel.a;
    }
    color.a *= cBase_Alpha.a;

    [branch] if (lightColor_intensity.w != 0.0)
    {
        float len = length(lightCenter_maxRange.xyz - v.world) * lightCenter_maxRange.w;
        float atten = pow(1.0 - saturate(len), lightColor2_falloff_power.w);
        float3 lcolor = lerp(lightColor2_falloff_power.rgb, lightColor_intensity.rgb, sqrt(len));
        color += float4(lcolor * (atten * lightColor_intensity.w), saturate(atten * lightColor_intensity.w));
        color = OverlayHDR(pixel, color);
        color = ScreenHDR(pixel, color);
    }
    return color;
}

#endif

#else

// The ball's lighting (fs_ball.sc's ballLightLoop): its ten lights on the diffuse and glossy layers,
// the environment's light on the diffuse one, and its reflection added as the specular layer
float3 ballLightLoop(float3 pos, float3 N, float3 V, float3 diffuse, float3 glossy, float3 specular, float edge, bool is_metal)
{
    float diffuseMax = max(diffuse.x, max(diffuse.y, diffuse.z));
    float glossyMax = max(glossy.x, max(glossy.y, glossy.z));
    float specularMax = max(specular.x, max(specular.y, specular.z));
    float sum = diffuseMax + glossyMax;
    if (sum > 1.0)
    {
        float invsum = 1.0 / sum;
        diffuse *= invsum;
        glossy *= invsum;
    }
    float3 color = float3(0.0, 0.0, 0.0);
    [branch] if ((!is_metal && (diffuseMax > 0.0)) || (glossyMax > 0.0))
    {
        [loop] for (int i = 0; i < 10; i++)
            color += DoPointLight(pos, N, V, diffuse, glossy, edge, Roughness_WrapL_Edge_Thickness.x, i, is_metal);
    }
    [branch] if (!is_metal && (diffuseMax > 0.0))
        color += DoEnvmapDiffuse(N, diffuse);
    if (specularMax > 0.0)
        color += specular;
    return color;
}

float3 ScreenHDR(float3 cBase, float3 cBlend)
{
    return max(1.0 - (1.0 - cBase) * (1.0 - cBlend), float3(0.0, 0.0, 0.0));
}

float4 fs_vpx_ball(Varyings v) : SV_Target0
{
    float3 V = normalize(v.to_eye);
    float3 N = normalize(v.normal);
    float3 R = reflect(V, N);
    // towards the edge, a smaller mip level against aliasing
    float edge = dot(V, R);
    float lod = (edge > 0.6) ? edge * (6.0 / 0.4) - (6.0 * 0.6 / 0.4) : 0.0;
    float3 ballImageColor;
    [branch] if (ball_flags.x == 0.0)
    {
        // an equirectangular map, by the reflection in the table's own space
        ballImageColor = tex_ball_color.SampleLevel(ball_sampler, ray_to_equirectangular_uv(-R), lod).rgb;
    }
    else
    {
        // a sphere map, by the reflection as the camera sees it
        float3 r = mul(float4(R, 0.0), ball_view).xyz;
        float m = (1.0 - r.z > 0.0) ? 0.3535533905932737622 * rsqrt(1.0 - r.z) : 0.0;
        ballImageColor = tex_ball_color.SampleLevel(ball_sampler, float2(0.5 - m * r.x, 0.5 - m * r.y), lod).rgb;
    }

    float4 decalColorT = tex_ball_decal.Sample(decal_sampler, v.uv);
    float3 decalColor = decalColorT.rgb;
    bool decal_mode = ball_flags.y != 0.0;
    if (!decal_mode)
    {
        // scratches: added by their alpha
        decalColor *= decalColorT.a;
        ballImageColor += decalColor;
    }
    else
        ballImageColor = ScreenHDR(ballImageColor, decalColor);

    [branch] if (w_h_disableLighting.z != 0.0)
        return float4(ballImageColor, 1.0);

    ballImageColor *= (decal_mode ? 0.5 : 1.0) * fenvEmissionScale_TexWidth.x;

    // the playfield (z = 0) mirrored in the ball, from the previous frame, fading in as the
    // reflection turns down
    float NdotR = R.z;
    float t = v.world.z / NdotR;
    float3 hit = v.world - t * R;
    float4 proj = mul(float4(hit, 1.0), ball_view_proj);
    float2 uvp = float2(0.5, 0.5) + float2(proj.x, -proj.y) * (0.5 / proj.w);
    [branch] if (w_h_disableLighting.w != 0.0 && !(uvp.x < 0.0 || uvp.x > 1.0 || uvp.y < 0.0 || uvp.y > 1.0) && !(t <= 0.0))
    {
        float3 playfieldColor = 0.25 * (tex_ball_playfield.SampleLevel(playfield_sampler, uvp + float2(w_h_disableLighting.x, 0.0), 0.0).rgb
                                      + tex_ball_playfield.SampleLevel(playfield_sampler, uvp - float2(w_h_disableLighting.x, 0.0), 0.0).rgb
                                      + tex_ball_playfield.SampleLevel(playfield_sampler, uvp + float2(0.0, w_h_disableLighting.y), 0.0).rgb
                                      + tex_ball_playfield.SampleLevel(playfield_sampler, uvp - float2(0.0, w_h_disableLighting.y), 0.0).rgb);
        ballImageColor = lerp(ballImageColor, playfieldColor, smoothstep(0.0, 0.15, NdotR) * invTableRes_reflection.z);
    }
    // against endless ball-to-playfield reflections and lighting spikes
    ballImageColor = min(ballImageColor, float3(15.0, 15.0, 15.0));

    float3 diffuse = cBase_Alpha.rgb * 0.075;
    if (!decal_mode)
        diffuse *= decalColor; // scratches make the surface rougher
    float3 glossy = max(diffuse * 2.0, float3(0.1, 0.1, 0.1));
    float3 specular = ballImageColor * cBase_Alpha.rgb;
    if (!decal_mode)
        specular *= float3(1.0, 1.0, 1.0) - decalColor;
    // the ball is solid: its pixels are tonemapped
    return float4(ballLightLoop(v.world, N, V, diffuse, glossy, specular, 1.0, false), 1.0);
}

#endif

#else

// The environment's irradiance, once per table (fs_pp_irradiance.sc): for each direction of a small
// picture laid out as the environment is, the environment's light over the half sphere around it,
// weighted by the cosine, from 32768 samples on a rank-1 lattice

struct IrradianceVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
};

IrradianceVaryings irradiance_vs(uint id : SV_VertexID)
{
    float2 c = float2((id << 1) & 2, id & 2);
    IrradianceVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    o.uv = c;
    return o;
}

Texture2D<float4> irradiance_env : register(t0, space2);
SamplerState irradiance_env_sampler : register(s0, space2);

float3 cos_hemisphere_sample(float2 t)
{
    float phi = t.y * (2.0 * PI);
    float cosTheta = sqrt(1.0 - t.x);
    float sinTheta = sqrt(t.x);
    float sp, cp;
    sincos(phi, sp, cp);
    return float3(cp * sinTheta, cosTheta, sp * sinTheta);
}

float3 rotate_to_vector_upper(float3 vec, float3 normal)
{
    if (normal.y > -0.99999)
    {
        float h = 1.0 / (1.0 + normal.y);
        float hz = h * normal.z;
        float hzx = hz * normal.x;
        return float3(
            vec.x * (normal.y + hz * normal.z) + vec.y * normal.x - vec.z * hzx,
            vec.y * normal.y - vec.x * normal.x - vec.z * normal.z,
            vec.y * normal.z - vec.x * hzx + vec.z * (normal.y + h * normal.x * normal.x));
    }
    return -vec;
}

float4 irradiance_fs(IrradianceVaryings v) : SV_Target0
{
    float phi = PI + v.uv.x * (2.0 * PI);
    float theta = v.uv.y * PI;
    float3 N = float3(sin(theta) * cos(phi), sin(theta) * sin(phi), cos(theta));
    const int NumSamples = 32768;
    const float InvNumSamples = 1.0 / float(NumSamples);
    const float g = 15936.0 / float(NumSamples);
    float3 irradiance = float3(0.0, 0.0, 0.0);
    [loop] for (int i = 0; i < NumSamples; ++i)
    {
        float2 u = float2(float(i) * InvNumSamples, frac(float(i) * g));
        float3 Li = rotate_to_vector_upper(cos_hemisphere_sample(u), N);
        float2 uv = ray_to_equirectangular_uv(Li);
        irradiance += irradiance_env.SampleLevel(irradiance_env_sampler, uv, 0.0).rgb * InvNumSamples;
    }
    return float4(irradiance, 1.0);
}

#endif
