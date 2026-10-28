// VPX's still parts drawn once, ported from Visual Pinball X build 5436 (commit af26b2d, license:GPLv3+,
// Copyright (C) 2000-2026 Visual Pinball development team and contributors): Renderer::RenderStaticPrepass
// adding each of its passes' pictures to their average with the framebuffer shader's mirror technique
// (fb_mirror: the picture times a weight), and each frame starting from that average and its depth,
// which VPX copies over (BlitRenderTarget), here drawn back by one triangle. With multisampling, as its
// DirectX and OpenGL builds do, the average is drawn into every sample and the depth, multisampled, is
// copied over beforehand.
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The triangle comes from the vertex number.

struct StillVaryings
{
    float4 clip : SV_Position;
};

StillVaryings still_vs(uint id : SV_VertexID)
{
    // one triangle over the whole picture
    float2 c = float2((id << 1) & 2, id & 2);
    StillVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}

Texture2D<float4> still_picture : register(t0, space2);
SamplerState still_picture_sampler : register(s0, space2);
Texture2D<float> still_depth : register(t1, space2);
SamplerState still_depth_sampler : register(s1, space2);

cbuffer StillWeights : register(b0, space3)
{
    float4 still_weights; // x: the colour's weight (1 / the passes); y: the mark's (the alpha)
};

// one pass's picture, texel for texel, weighted for the average
float4 still_accumulate_fs(StillVaryings v) : SV_Target
{
    float4 c = still_picture.Load(int3(v.clip.xy, 0));
    return float4(c.rgb * still_weights.x, c.a * still_weights.y);
}

struct StillRestored
{
    float4 color : SV_Target;
    float depth : SV_Depth;
};

// the still parts' picture and depth, as the frame's start
StillRestored still_restore_fs(StillVaryings v)
{
    int3 p = int3(v.clip.xy, 0);
    StillRestored o;
    o.color = still_picture.Load(p);
    o.depth = still_depth.Load(p);
    return o;
}

// the still parts' picture alone into every sample, as the frame's start when multisampled (its
// multisampled depth copied over beforehand)
float4 still_restore_color_fs(StillVaryings v) : SV_Target
{
    return still_picture.Load(int3(v.clip.xy, 0));
}
