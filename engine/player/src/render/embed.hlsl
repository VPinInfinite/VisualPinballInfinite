// VPX 5436's embedded windows, ours: the backglass, score view or topper drawn into the table's
// own picture at its place, as VPX 5436 draws them there (Renderer::ClearEmbeddedAncillaryWindow and
// RenderAncillaryWindow with an embedded output): the place filled first (black in the table's picture before
// its occlusion and bloom are read; no occlusion there), then the window's picture copied in before the
// tonemapping. One triangle over the viewport, which the place sets.
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The triangle comes from the vertex number.

struct EmbedVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
};

EmbedVaryings embed_vs(uint id : SV_VertexID)
{
    float2 c = float2((id << 1) & 2, id & 2);
    EmbedVaryings o;
    o.clip = float4(c * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    o.uv = c;
    return o;
}

Texture2D<float4> embed_picture : register(t0, space2);
SamplerState embed_sampler : register(s0, space2);

cbuffer Embed : register(b0, space3)
{
    // the fill's colour
    float4 embed_fill;
    // x: the picture holds sRGB values (to be made linear); y: the target takes sRGB values (to be
    // encoded); z: the alpha written (1 marks the table's picture for tonemapping, as VPX's depth does)
    float4 embed_mode;
};

float4 embed_fill_fs(EmbedVaryings i) : SV_Target
{
    return embed_fill;
}

float3 embed_to_linear(float3 c)
{
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

float3 embed_to_srgb(float3 c)
{
    return c <= 0.0031308 ? c * 12.92 : 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

float4 embed_copy_fs(EmbedVaryings i) : SV_Target
{
    float3 c = embed_picture.Sample(embed_sampler, i.uv).rgb;
    if (embed_mode.x > 0.5)
        c = embed_to_linear(c);
    if (embed_mode.y > 0.5)
        c = embed_to_srgb(saturate(c));
    return float4(c, embed_mode.z);
}
