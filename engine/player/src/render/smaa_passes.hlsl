// VPX's SMAA passes (fs_pp_smaa_*.sc, vs_pp_smaa_*.sc), after post_common.hlsl and smaa.hlsl.

// ---- SMAA (fs_pp_smaa_*.sc, vs_pp_smaa_*.sc; SMAA.sh's settings: high preset, colour edges) ----
//
// SMAA's own code comes before this, in smaa.hlsl; its pictures sit in the passes' slots in order, its samplers
// are the common ones.

cbuffer PostVertex : register(b0, space1)
{
    float4 smaa_vs_metrics; // 1 / the picture's size, and its size
};

Texture2D<float4> smaa_first : register(t0, space2);
Texture2D<float4> smaa_second : register(t1, space2);
Texture2D<float4> smaa_third : register(t2, space2);

struct SmaaEdgeVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
    float4 offset0 : TEXCOORD1;
    float4 offset1 : TEXCOORD2;
    float4 offset2 : TEXCOORD3;
};

struct SmaaWeightVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
    float2 pixcoord : TEXCOORD1;
    float4 offset0 : TEXCOORD2;
    float4 offset1 : TEXCOORD3;
    float4 offset2 : TEXCOORD4;
};

struct SmaaBlendVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
    float4 offset : TEXCOORD1;
};

// the corner of a triangle over the whole picture, and its place on the picture, 0 to 2
float2 smaa_corner(uint id, out float4 clip)
{
    float2 c = float2((id << 1) & 2, id & 2);
    clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return c;
}

SmaaEdgeVaryings smaa_edge_vs(uint id : SV_VertexID)
{
    smaa_metrics = smaa_vs_metrics;
    SmaaEdgeVaryings o;
    o.uv = smaa_corner(id, o.clip);
    float4 offset[3];
    SMAAEdgeDetectionVS(o.uv, offset);
    o.offset0 = offset[0];
    o.offset1 = offset[1];
    o.offset2 = offset[2];
    return o;
}

// the picture's edges (its first picture: the tonemapped one)
float4 smaa_edge_fs(SmaaEdgeVaryings v) : SV_Target
{
    smaa_metrics = w_h_height;
    float4 offset[3] = { v.offset0, v.offset1, v.offset2 };
    float2 rg = SMAAColorEdgeDetectionPS(v.uv, offset, smaa_first);
    return float4(rg.r, rg.g, 0.0, 0.0);
}

SmaaWeightVaryings smaa_weight_vs(uint id : SV_VertexID)
{
    smaa_metrics = smaa_vs_metrics;
    SmaaWeightVaryings o;
    o.uv = smaa_corner(id, o.clip);
    float4 offset[3];
    float2 pixcoord;
    SMAABlendingWeightCalculationVS(o.uv, pixcoord, offset);
    o.pixcoord = pixcoord;
    o.offset0 = offset[0];
    o.offset1 = offset[1];
    o.offset2 = offset[2];
    return o;
}

// the blending weights (its pictures: the edges, SMAA's area and search pictures)
float4 smaa_weight_fs(SmaaWeightVaryings v) : SV_Target
{
    smaa_metrics = w_h_height;
    float4 offset[3] = { v.offset0, v.offset1, v.offset2 };
    return SMAABlendingWeightCalculationPS(v.uv, v.pixcoord, offset, smaa_first, smaa_second, smaa_third, float4(0.0, 0.0, 0.0, 0.0));
}

SmaaBlendVaryings smaa_blend_vs(uint id : SV_VertexID)
{
    smaa_metrics = smaa_vs_metrics;
    SmaaBlendVaryings o;
    o.uv = smaa_corner(id, o.clip);
    SMAANeighborhoodBlendingVS(o.uv, o.offset);
    return o;
}

// the picture blended with its neighbours by the weights (its pictures: the tonemapped one, the weights)
float4 smaa_blend_fs(SmaaBlendVaryings v) : SV_Target
{
    smaa_metrics = w_h_height;
    return post_out(SMAANeighborhoodBlendingPS(v.uv, v.offset, smaa_first, smaa_second).rgb);
}
