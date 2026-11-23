// The pause menu (pause_menu.rs), ours: the table dimmed, then the menu's picture laid over the finished
// frame at its place, as the player sees the screen — turned with the table on a turned screen, as the
// readout is (overlay_vertices) — and encoded for VPX 5436's HDR10 output as the readout is.
//
// Registers follow SDL3 GPU's rules (see player.hlsl). The two quads come from the vertex number.

cbuffer MenuPlace : register(b0, space1)
{
    // the picture's place, laid out as the player sees the screen: left, top, width, height in pixels
    float4 menu_place;
    // the window's width and height in pixels, the text's quarter turns (0 to 3), and 1 for the
    // picture alone, with no dimming (an achievement's announcement)
    float4 menu_window;
};

struct MenuVaryings
{
    float4 clip : SV_Position;
    float2 uv : TEXCOORD0;
    // 1 for the dimming, 0 for the picture
    float dim : TEXCOORD1;
};

static const uint menu_corners[6] = { 0, 1, 2, 1, 3, 2 };

MenuVaryings menu_vs(uint id : SV_VertexID)
{
    // two quads of two triangles: the dimming over the whole window, then the picture (the
    // vertex number counts from 0 whatever vertex a draw starts at, so the picture alone is asked
    // for)
    uint c = menu_corners[id % 6];
    float2 t = float2(c & 1, c >> 1);
    bool dim = id < 6 && menu_window.w < 0.5;
    float w = menu_window.x;
    float h = menu_window.y;
    uint turn = (uint)menu_window.z;
    float2 seen = (turn & 1) != 0 ? float2(h, w) : float2(w, h);
    float4 place = dim ? float4(0.0, 0.0, seen.x, seen.y) : menu_place;
    float2 p = place.xy + t * place.zw;
    float2 q = p;
    if (turn == 1)
        q = float2(w - p.y, p.x);
    else if (turn == 2)
        q = float2(w - p.x, h - p.y);
    else if (turn == 3)
        q = float2(p.y, h - p.x);
    MenuVaryings o;
    o.clip = float4(q.x / w * 2.0 - 1.0, 1.0 - q.y / h * 2.0, 0.0, 1.0);
    o.uv = t;
    o.dim = dim ? 1.0 : 0.0;
    return o;
}

Texture2D<float4> menu_picture : register(t0, space2);
SamplerState menu_sampler : register(s0, space2);

cbuffer MenuLook : register(b0, space3)
{
    // x: how dark the table goes (the dimming's alpha); y: the target takes linear values (an sRGB
    // picture encodes them)
    float4 menu_look;
};

float3 menu_to_linear(float3 c)
{
    return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4);
}

// the picture holds sRGB values, its alpha straight
float4 menu_colour(MenuVaryings v)
{
    if (v.dim > 0.5)
        return float4(0.0, 0.0, 0.0, menu_look.x);
    return menu_picture.Sample(menu_sampler, v.uv);
}

float4 menu_fs(MenuVaryings v) : SV_Target0
{
    float4 c = menu_colour(v);
    if (menu_look.y > 0.5)
        c.rgb = menu_to_linear(c.rgb);
    return c;
}

#ifdef MENU_PQ_WHITE
// over VPX 5436's HDR10 output (render/hdr10.rs): the menu's colours at the screen's SDR white
// (MENU_PQ_WHITE, in 10000s of nits), as VPX scales its own interface there, in BT.2020's colours,
// encoded by ST.2084 (PQ) as tonemap.hlsl encodes the picture
float4 menu_pq_fs(MenuVaryings v) : SV_Target0
{
    float4 c = menu_colour(v);
    const float3x3 srgb_to_rec2020 = float3x3(
        0.6274, 0.0691, 0.0164,
        0.3293, 0.9195, 0.0880,
        0.0433, 0.0113, 0.8956);
    float3 col = mul(menu_to_linear(c.rgb) * MENU_PQ_WHITE, srgb_to_rec2020);
    const float m1 = (2610.0 / 4096.0) / 4.0;
    const float m2 = (2523.0 / 4096.0) * 128.0;
    const float c1 = 3424.0 / 4096.0;
    const float c2 = (2413.0 / 4096.0) * 32.0;
    const float c3 = (2392.0 / 4096.0) * 32.0;
    float3 cp = pow(max(col, 0.0), m1);
    col = pow((c1 + c2 * cp) / (1.0 + c3 * cp), m2);
    return float4(col, c.a);
}
#endif
