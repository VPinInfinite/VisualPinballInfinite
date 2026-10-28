// VPX's ambient occlusion and ball motion blur, ported from Visual Pinball X build 5436 (commit af26b2d,
// license:GPLv3+, Copyright (C) 2000-2026 Visual Pinball development team and contributors):
// src/shaders/bgfx/fs_pp_ssao.sc (its screen-space ambient occlusion), fs_pp_ao.sc (the static picture darkened
// by it), fs_pp_motionblur.sc (a moving ball raytraced along its path), with common.sh's cos_hemisphere_sample and
// rotate_to_vector_upper, run as Renderer::UpdateAmbientOcclusion, RenderStaticPrepass and ApplyBallMotionBlur run
// them (src/renderer/Renderer.cpp).
//
// VPX's ambient occlusion jitters its samples by a 64 x 64 noise picture of its own (src/assets/AODither.webp), a
// re-encoding of a 2016 picture, before VPX's GPL rule: not taken. Ours is computed (effects.rs).
//
// Registers follow SDL3 GPU's rules (see player.hlsl); every pass shares the same three picture slots and one
// block of uniforms, each using what it needs. The triangle comes from the vertex number.

struct EffectVaryings
{
    float4 clip : SV_Position;
};

EffectVaryings effect_vs(uint id : SV_VertexID)
{
    // one triangle over the whole picture
    float2 c = float2((id << 1) & 2, id & 2);
    EffectVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}

Texture2D<float4> effect_t0 : register(t0, space2);
SamplerState effect_s0 : register(s0, space2);
Texture2D<float4> effect_t1 : register(t1, space2);
SamplerState effect_s1 : register(s1, space2);
Texture2D<float4> effect_t2 : register(t2, space2);
SamplerState effect_s2 : register(s2, space2);

cbuffer Effects : register(b0, space3)
{
    row_major float4x4 effect_m0; // motion blur: VPX's matProj, from the camera's space
    row_major float4x4 effect_m1; // motion blur: its inverse
    float4 effect_v0;             // VPX's w_h_height: 1 / the picture's size, then a pass's own (AO: zw its jitter; blur: w the samples)
    float4 effect_v1;             // AO: VPX's AO_scale_timeblur; blur: the ball now, in the camera's space, and its radius
    float4 effect_v2;             // blur: the ball where it was last drawn
    float4 effect_v3;
};

#define PI 3.1415926535897932384626433832795

float3 cos_hemisphere_sample(float2 t) // u,v in [0..1), returns y-up
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
    else
        return -vec;
}

// ---- ambient occlusion (fs_pp_ssao.sc): slot 0 the last result, slot 1 the depth, slot 2 the noise ----

#define ao_w_h_height effect_v0
#define AO_scale_timeblur effect_v1

float ao_depth(float2 u)
{
    return effect_t1.SampleLevel(effect_s1, u, 0.0).x;
}

float3 get_nonunit_normal(float depth0, float2 u) // use neighboring pixels // quite some tex access by this
{
    float depth1 = ao_depth(float2(u.x, u.y + ao_w_h_height.y));
    float depth2 = ao_depth(float2(u.x + ao_w_h_height.x, u.y));
    return float3(ao_w_h_height.y * (depth2 - depth0), (depth1 - depth0) * ao_w_h_height.x, ao_w_h_height.y * ao_w_h_height.x); //!!
}

float4 ao_fs(EffectVaryings v) : SV_Target
{
    float2 u = v.clip.xy * ao_w_h_height.xy;
    float2 uv0 = u - ao_w_h_height.xy * 0.5 + ao_w_h_height.xy; // half pixel shift in x & y for filter
    float2 uv1 = u - ao_w_h_height.xy * 0.5; // dto.

    float depth0 = ao_depth(u);
    [branch] if ((depth0 == 1.0) || (depth0 == 0.0)) //!! early out if depth too large (=BG) or too small (=DMD,etc -> retweak render options (depth write on), otherwise also screwup with stereo)
        return float4(1.0, 0., 0., 0.);

    float3 ushift = effect_t2.SampleLevel(effect_s2, uv1 / (64.0 * ao_w_h_height.xy) + ao_w_h_height.zw, 0.0).xyz; // 64 is the dither picture's size
    float area = 0.06; //!!
    float falloff = 0.0002; //!!
    const int samples = 8; //4,8,9,13,16,21,25,32 korobov,fibonacci
    float samples_float = float(samples);
    float radius = 0.001 + (ushift.z) * 0.009; // sample radius
    float depth_threshold_normal = 0.005;
    float total_strength = AO_scale_timeblur.x * (0.5 / samples_float);
    float3 normal = normalize(get_nonunit_normal(depth0, u));
    float radius_depth = radius / depth0;

    float occlusion = 0.0;
    [unroll] for (int i = 0; i < samples; ++i)
    {
        float i_float = float(i);
        float2 r = float2(i_float * (1.0 / samples_float), i_float * (5.0 / samples_float)); //1,5,2,8,4,13,7,7 korobov,fibonacci
        float2 ray = rotate_to_vector_upper(cos_hemisphere_sample(frac(r + ushift.xy)), normal).xy; // shift lattice
        float2 hemi_ray = u + radius_depth * ray.xy;
        float occ_depth = ao_depth(hemi_ray);
        float3 occ_normal = get_nonunit_normal(occ_depth, hemi_ray);
        float diff_depth = depth0 - occ_depth;
        float diff_norm = dot(occ_normal, normal);
        occlusion += step(falloff, diff_depth) * (diff_depth < depth_threshold_normal ? (1.0 - diff_norm * diff_norm / dot(occ_normal, occ_normal)) : 1.0) * (1.0 - smoothstep(falloff, area, diff_depth));
    }
    // weight with result(s) from previous frames
    float ao = 1.0 - total_strength * occlusion;
    return float4((effect_t0.SampleLevel(effect_s0, uv0, 0.0).x //abuse bilerp for filtering (by using half texel/pixel shift)
                   + effect_t0.SampleLevel(effect_s0, uv1, 0.0).x
                   + effect_t0.SampleLevel(effect_s0, float2(uv0.x, uv1.y), 0.0).x
                   + effect_t0.SampleLevel(effect_s0, float2(uv1.x, uv0.y), 0.0).x)
        * (0.25 * (1.0 - AO_scale_timeblur.y)) + saturate(ao) * AO_scale_timeblur.y, 0., 0., 0.);
}

// ---- the still parts' picture darkened by it (fs_pp_ao.sc, VPX's fb_AO_static): slot 0 the picture, slot 1
// the occlusion; the picture's mark of the table's pixels (its alpha, which our tonemapping reads where VPX reads
// its depth) kept ----

float4 ao_apply_fs(EffectVaryings v) : SV_Target
{
    float2 u = v.clip.xy * effect_v0.xy;
    float4 picture = effect_t0.SampleLevel(effect_s0, u, 0.0);
    return float4(picture.rgb * effect_t1.SampleLevel(effect_s1, u - 0.5 * effect_v0.xy, 0.0).x, picture.a); // shift half a texel to blurs over 2x2 window
}

// ---- ball motion blur (fs_pp_motionblur.sc): slot 0 this frame's picture, slot 1 the last one's, slot 2 the
// depth ----

float4 motion_blur_fs(EffectVaryings v) : SV_Target
{
    int nSamples = int(effect_v0.w);
    float2 screenUV = v.clip.xy * effect_v0.xy;
    float screenDepth = effect_t2.SampleLevel(effect_s2, screenUV, 0.0).x;
    float4 projPos = float4(screenUV.x * 2. - 1., 1. - screenUV.y * 2., screenDepth, 1.);

    // Compute position in world/view space of rendered fragment
    float4 pixelPos = mul(projPos, effect_m1);
    pixelPos.xyz /= pixelPos.w;

    // Compute camera ray, knowing that camera is always at origin in world/view space
    float3 rayDirection = normalize(pixelPos.xyz);

    // Integrate over nSamples samples (nSamples+1 positions excluding the previous one which was already part of the previous displayed frame)
    float3 ball_pos = effect_v1.xyz;
    float3 prev_ball_pos = effect_v2.xyz;
    float ball_r2 = effect_v1.w * effect_v1.w;

    // Start with current ball position, as it allows to detect occluded pixel and select the best background (no ball) color source
    float3 color = effect_t0.SampleLevel(effect_s0, screenUV, 0.0).rgb;
    float3 noIntersectColor;
    {
        // Optimized ray / sphere intersection for a ray origin at origin (camera is always at origin)
        float b = dot(rayDirection, ball_pos);
        float c = dot(ball_pos, ball_pos) - ball_r2 * 1.15; // 1.15 is a magic number to avoid acne at the border of the ball (wrongly considering there is no intersection when the pixel is partially occluded by the ball)
        float det = b * b - c;
        if (det < 0.0)
        {
            // No intersection with current ball pos: background of the ball is available in current render
            noIntersectColor = color;
        }
        else
        {
            // Nearest intersection (other intersection is b + sqrt(det) which is always farther from camera with our setup)
            float intersect_pos_z = (b - sqrt(det)) * rayDirection.z;
            if (pixelPos.z > intersect_pos_z + 1.0) // 1.0 is a magic value to avoid surface acnee
            {
                // Ball is behind something, disable motion blur (not fully correct as it may be a transparent object with depth write enabled)
                return float4(color, 1.);
            }
            // Intersection with current ball pos: background of the ball will be taken from previous render
            noIntersectColor = effect_t1.SampleLevel(effect_s1, screenUV, 0.0).rgb;
        }
    }

    // Process interpolated positions from previous (excluded as part of previous frame) to current (excluded as already processed)
    [loop] for (int i = 1; i < nSamples; i++)
    {
        float mix_pos = float(i) / effect_v0.w;
        float3 sample_ball_pos = lerp(prev_ball_pos, ball_pos, mix_pos);
        float b = dot(rayDirection, sample_ball_pos);
        float c = dot(sample_ball_pos, sample_ball_pos) - ball_r2;
        float det = b * b - c;
        if (det < 0.0)
        {
            // No intersection: use the selected background render
            color += noIntersectColor;
        }
        else
        {
            // Intersection: build up render at current sample from corresponding points from previous and current renders
            float3 intersect_ofs = (b - sqrt(det)) * rayDirection - sample_ball_pos; // intersection position relative to sphere center
            float4 prev_proj = mul(float4(prev_ball_pos + intersect_ofs, 1.0), effect_m0);
            float4 cur_proj = mul(float4(ball_pos + intersect_ofs, 1.0), effect_m0);
            prev_proj.xy /= prev_proj.w;
            cur_proj.xy /= cur_proj.w;
            float3 prev_sample = effect_t1.SampleLevel(effect_s1, 0.5 + 0.5 * float2(prev_proj.x, -prev_proj.y), 0.0).rgb;
            float3 cur_sample = effect_t0.SampleLevel(effect_s0, 0.5 + 0.5 * float2(cur_proj.x, -cur_proj.y), 0.0).rgb;
            color += lerp(prev_sample, cur_sample, mix_pos);
        }
    }

    return float4(color / effect_v0.w, 1.0);
}
