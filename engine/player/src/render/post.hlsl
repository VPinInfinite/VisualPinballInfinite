// VPX's post-processed antialiasing and sharpening passes but SMAA: see post_common.hlsl. Registers follow SDL3
// GPU's rules (see player.hlsl); slot 0 holds the picture, slot 1 its depth.

Texture2D<float4> post_source : register(t0, space2);
Texture2D<float> post_depth : register(t1, space2);

#define texNoLod(pos) post_source.SampleLevel(post_linear_sampler, pos, 0.0)

// nothing of the table drawn here: its depth still cleared (VPX: the background, or the DMD and the like)
bool post_untouched(float2 u)
{
    [branch] if (w_h_height.w == 1.0)
    {
        float depth0 = post_depth.SampleLevel(post_point_sampler, u, 0.0);
        return depth0 == 1.0 || depth0 == 0.0;
    }
    return false;
}

// ---- FXAA (fs_pp_fxaa.sc) and FAAA (fs_pp_faaa.sc) ----
//
// Their code isn't VPX's own: VPX marks the files GPL, and ships them so, but FXAA 2 and 3 are NVIDIA's FXAA 3.11 by
// Timothy Lottes, whose notice reserves NVIDIA's rights and grants no licence in writing, and FAAA is G. Rebord's
// shader, based on FXAA 3.11, whose licence isn't stated. They're ported as VPX has them (CREDITS.md); if their
// authors object, they come out or are replaced.

float fxaa_luma(float3 l)
{
    return dot(l, float3(0.25, 0.5, 0.25)); // experimental, red and blue should not suffer too much
}

// Approximation of FXAA (VPX's "Fast FXAA")
float3 fxaa1(float2 u)
{
    float3 rMc = texNoLod(u).xyz;
    [branch] if (post_untouched(u))
        return rMc;

    float2 offs = w_h_height.xy;
    float rNW = fxaa_luma(texNoLod(u - offs).xyz);
    float rN = fxaa_luma(texNoLod(u - float2(0.0, offs.y)).xyz);
    float rNE = fxaa_luma(texNoLod(u - float2(-offs.x, offs.y)).xyz);
    float rW = fxaa_luma(texNoLod(u - float2(offs.x, 0.0)).xyz);
    float rM = fxaa_luma(rMc);
    float rE = fxaa_luma(texNoLod(u + float2(offs.x, 0.0)).xyz);
    float rSW = fxaa_luma(texNoLod(u + float2(-offs.x, offs.y)).xyz);
    float rS = fxaa_luma(texNoLod(u + float2(0.0, offs.y)).xyz);
    float rSE = fxaa_luma(texNoLod(u + offs).xyz);
    float rMrN = rM + rN;
    float lumaNW = rMrN + rNW + rW;
    float lumaNE = rMrN + rNE + rE;
    float rMrS = rM + rS;
    float lumaSW = rMrS + rSW + rW;
    float lumaSE = rMrS + rSE + rE;
    bool g0 = (lumaSW > lumaSE);
    float tempMax = g0 ? lumaSW : lumaSE;
    float tempMin = g0 ? lumaSE : lumaSW;
    bool g1 = (lumaNW > lumaNE);
    float tempMax2 = g1 ? lumaNW : lumaNE;
    float tempMin2 = g1 ? lumaNE : lumaNW;
    float lumaMin = min(rM, min(tempMin, tempMin2));
    float lumaMax = max(rM, max(tempMax, tempMax2));
    float SWSE = lumaSW + lumaSE;
    float NWNE = lumaNW + lumaNE;
    float2 dir = float2(SWSE - NWNE, (lumaNW + lumaSW) - (lumaNE + lumaSE));
    float temp = 1.0 / (min(abs(dir.x), abs(dir.y)) + max((NWNE + SWSE) * 0.03125, 0.0078125)); //!! tweak?
    dir = clamp(dir * temp, -8.0, 8.0) * offs; //!! tweak?
    float3 rgbA = 0.5 * (texNoLod(u - dir * (0.5 / 3.0)).xyz + texNoLod(u + dir * (0.5 / 3.0)).xyz);
    float3 rgbB = 0.5 * rgbA + 0.25 * (texNoLod(u - dir * 0.5).xyz + texNoLod(u + dir * 0.5).xyz);
    float lumaB = fxaa_luma(rgbB);
    return ((lumaB < lumaMin) || (lumaB > lumaMax)) ? rgbA : rgbB;
}

// the steps along an edge: FXAA_QUALITY_P0 to P2 for mid quality, P0 to P11 for extreme quality
static const float FXAA2_STEPS[12] = { 1.5, 2.0, 8.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0 };
static const float FXAA3_STEPS[12] = { 1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 2.0, 2.0, 2.0, 4.0, 8.0 };

// Full PC FXAA 3.11, mid quality (VPX's "Standard FXAA", 3 steps) or extreme quality ("Quality FXAA", 12 steps)
float3 fxaa_pc(float2 u, float p[12], int count)
{
    float3 rgbyM = texNoLod(u).xyz;
    [branch] if (post_untouched(u))
        return rgbyM;

    float2 offs = w_h_height.xy;
    float lumaNW = fxaa_luma(texNoLod(u - offs).xyz);
    float lumaN = fxaa_luma(texNoLod(u - float2(0.0, offs.y)).xyz);
    float lumaNE = fxaa_luma(texNoLod(u - float2(-offs.x, offs.y)).xyz);
    float lumaW = fxaa_luma(texNoLod(u - float2(offs.x, 0.0)).xyz);
    float lumaM = fxaa_luma(rgbyM);
    float lumaE = fxaa_luma(texNoLod(u + float2(offs.x, 0.0)).xyz);
    float lumaSW = fxaa_luma(texNoLod(u + float2(-offs.x, offs.y)).xyz);
    float lumaS = fxaa_luma(texNoLod(u + float2(0.0, offs.y)).xyz);
    float lumaSE = fxaa_luma(texNoLod(u + offs).xyz);
    float maxSM = max(lumaS, lumaM);
    float minSM = min(lumaS, lumaM);
    float maxESM = max(lumaE, maxSM);
    float minESM = min(lumaE, minSM);
    float maxWN = max(lumaN, lumaW);
    float minWN = min(lumaN, lumaW);
    float rangeMax = max(maxWN, maxESM);
    float rangeMin = min(minWN, minESM);
    float rangeMaxScaled = rangeMax * 0.125; //0.333 (faster) .. 0.063 (slower) // reshade: 0.125, fxaa : 0.166
    float range = rangeMax - rangeMin;
    float rangeMaxClamped = max(0.0833, rangeMaxScaled); //0.0625 (high quality/faster) .. 0.0312 (visible limit/slower) // reshade: 0.0, fxaa : 0.0833
    bool earlyExit = range < rangeMaxClamped;
    [branch] if (earlyExit)
        return rgbyM;
    float lumaNS = lumaN + lumaS;
    float lumaWE = lumaW + lumaE;
    float subpixRcpRange = 1.0 / range;
    float subpixNSWE = lumaNS + lumaWE;
    float edgeHorz1 = -2.0 * lumaM + lumaNS;
    float edgeVert1 = -2.0 * lumaM + lumaWE;
    float lumaNESE = lumaNE + lumaSE;
    float lumaNWNE = lumaNW + lumaNE;
    float edgeHorz2 = -2.0 * lumaE + lumaNESE;
    float edgeVert2 = -2.0 * lumaN + lumaNWNE;
    float lumaNWSW = lumaNW + lumaSW;
    float lumaSWSE = lumaSW + lumaSE;
    float edgeHorz4 = abs(edgeHorz1) * 2.0 + abs(edgeHorz2);
    float edgeVert4 = abs(edgeVert1) * 2.0 + abs(edgeVert2);
    float edgeHorz3 = -2.0 * lumaW + lumaNWSW;
    float edgeVert3 = -2.0 * lumaS + lumaSWSE;
    float edgeHorz = abs(edgeHorz3) + edgeHorz4;
    float edgeVert = abs(edgeVert3) + edgeVert4;
    float subpixNWSWNESE = lumaNWSW + lumaNESE;
    float lengthSign = offs.x;
    bool horzSpan = edgeHorz >= edgeVert;
    float subpixA = subpixNSWE * 2.0 + subpixNWSWNESE;
    if (!horzSpan) lumaN = lumaW;
    if (!horzSpan) lumaS = lumaE;
    if (horzSpan) lengthSign = offs.y;
    float subpixB = subpixA * (1.0 / 12.0) - lumaM;
    float gradientN = lumaN - lumaM;
    float gradientS = lumaS - lumaM;
    float lumaNN = lumaN + lumaM;
    float lumaSS = lumaS + lumaM;
    bool pairN = (abs(gradientN) >= abs(gradientS));
    float gradient = max(abs(gradientN), abs(gradientS));
    if (pairN) lengthSign = -lengthSign;
    float subpixC = clamp(abs(subpixB) * subpixRcpRange, 0.0, 1.0);
    float2 offNP = float2(!horzSpan ? 0.0 : offs.x, horzSpan ? 0.0 : offs.y);
    float2 posB = u;
    float l05 = lengthSign * 0.5;
    if (horzSpan) posB.y += l05;
    else posB.x += l05;
    float2 posN = float2(posB.x - offNP.x * p[0], posB.y - offNP.y * p[0]);
    float2 posP = float2(posB.x + offNP.x * p[0], posB.y + offNP.y * p[0]);
    float subpixD = -2.0 * subpixC + 3.0;
    float lumaEndN = fxaa_luma(texNoLod(posN).xyz);
    float subpixE = subpixC * subpixC;
    float lumaEndP = fxaa_luma(texNoLod(posP).xyz);
    if (!pairN) lumaNN = lumaSS;
    float gradientScaled = gradient * (1.0 / 4.0);
    float lumaMM = lumaM - lumaNN * 0.5;
    float subpixF = subpixD * subpixE;
    bool lumaMLTZero = (lumaMM < 0.0);
    lumaEndN -= lumaNN * 0.5;
    lumaEndP -= lumaNN * 0.5;
    bool doneN = (abs(lumaEndN) >= gradientScaled);
    bool doneP = (abs(lumaEndP) >= gradientScaled);
    if (!doneN) posN.x -= offNP.x * p[1];
    if (!doneN) posN.y -= offNP.y * p[1];
    bool doneNP = ((!doneN) || (!doneP));
    if (!doneP) posP.x += offNP.x * p[1];
    if (!doneP) posP.y += offNP.y * p[1];
    // VPX's nested steps, each only while an end is still to be found
    [unroll] for (int k = 2; k < count; k++)
    {
        [branch] if (doneNP)
        {
            if (!doneN) lumaEndN = fxaa_luma(texNoLod(posN).xyz);
            if (!doneP) lumaEndP = fxaa_luma(texNoLod(posP).xyz);
            if (!doneN) lumaEndN = lumaEndN - lumaNN * 0.5;
            if (!doneP) lumaEndP = lumaEndP - lumaNN * 0.5;
            doneN = abs(lumaEndN) >= gradientScaled;
            doneP = abs(lumaEndP) >= gradientScaled;
            if (!doneN) posN.x -= offNP.x * p[k];
            if (!doneN) posN.y -= offNP.y * p[k];
            doneNP = ((!doneN) || (!doneP));
            if (!doneP) posP.x += offNP.x * p[k];
            if (!doneP) posP.y += offNP.y * p[k];
        }
    }
    float dstN = horzSpan ? (u.x - posN.x) : (u.y - posN.y);
    float dstP = horzSpan ? (posP.x - u.x) : (posP.y - u.y);
    bool goodSpanN = ((lumaEndN < 0.0) != lumaMLTZero);
    float spanLength = dstP + dstN;
    bool goodSpanP = ((lumaEndP < 0.0) != lumaMLTZero);
    float spanLengthRcp = 1.0 / spanLength;
    bool directionN = (dstN < dstP);
    float dst = min(dstN, dstP);
    bool goodSpan = directionN ? goodSpanN : goodSpanP;
    float subpixG = subpixF * subpixF;
    float pixelOffset = 0.5 - dst * spanLengthRcp;
    float subpixH = subpixG * 0.5; //1.00 (upper limit/softer) .. 0.50 (lower limit/sharper) .. 0.00 (completely off) // reshade : 0.25, fxaa : 0.75
    float pixelOffsetGood = goodSpan ? pixelOffset : 0.0;
    float pixelOffsetSubpix = max(pixelOffsetGood, subpixH);
    float2 un = u;
    float pl = pixelOffsetSubpix * lengthSign;
    if (horzSpan) un.y += pl;
    else un.x += pl;
    return texNoLod(un).xyz;
}

float4 fxaa1_fs(PostVaryings v) : SV_Target
{
    return post_out(fxaa1(v.clip.xy * w_h_height.xy));
}

float4 fxaa2_fs(PostVaryings v) : SV_Target
{
    return post_out(fxaa_pc(v.clip.xy * w_h_height.xy, FXAA2_STEPS, 3));
}

float4 fxaa3_fs(PostVaryings v) : SV_Target
{
    return post_out(fxaa_pc(v.clip.xy * w_h_height.xy, FXAA3_STEPS, 12));
}

// FAAA - Antialiasing shader by G.Rebord, based on FXAA 3.11 by TIMOTHY LOTTES (VPX's "Quality FAAA": its edge
// threshold 9, was 5 in original FAAA, and quality 9, was 5)
#define FAAA_EDGE_THRESHOLD 9.0
#define FAAA_QUALITY__PI 12
static const float FAAA_OFF_MULT[12] = { 1.0, 1.0, 1.0, 1.0, 1.0, 1.5, 2.0, 2.0, 2.0, 2.0, 4.0, 8.0 };

float3 faaa(float2 u)
{
    float3 rgbyM = texNoLod(u).xyz;
    [branch] if (post_untouched(u))
        return rgbyM;

    float2 offs = w_h_height.xy;
    float lumaSE = fxaa_luma(texNoLod(u + offs).xyz);
    float lumaNW = fxaa_luma(texNoLod(u - offs).xyz);
    float lumaSW = fxaa_luma(texNoLod(u + float2(-offs.x, offs.y)).xyz);
    float lumaNE = fxaa_luma(texNoLod(u - float2(-offs.x, offs.y)).xyz);
    // Edge detection. This does much better than a head-on luma delta.
    float gradientSWNE = lumaSW - lumaNE;
    float gradientSENW = lumaSE - lumaNW;
    float2 dirM = float2(abs(gradientSWNE + gradientSENW), abs(gradientSWNE - gradientSENW));
    float lumaMax = max(max(lumaSW, lumaSE), max(lumaNE, lumaNW));
    float localLumaFactor = lumaMax * 0.5 + 0.5;
    float localThres = ((10.0 - FAAA_EDGE_THRESHOLD) * 0.0625) * localLumaFactor;
    bool lowDelta = abs(dirM.x - dirM.y) < localThres;
    [branch] if (lowDelta)
        return rgbyM;

    bool horzSpan = dirM.x > dirM.y;
    float lumaM = fxaa_luma(rgbyM);
    float lumaN = 0.0;
    float lumaS = 0.0;
    if (horzSpan) lumaN = fxaa_luma(texNoLod(u - float2(0.0, offs.y)).xyz);
    if (horzSpan) lumaS = fxaa_luma(texNoLod(u + float2(0.0, offs.y)).xyz);
    if (!horzSpan) lumaN = fxaa_luma(texNoLod(u - float2(offs.x, 0.0)).xyz);
    if (!horzSpan) lumaS = fxaa_luma(texNoLod(u + float2(offs.x, 0.0)).xyz);
    float gradientN = lumaN - lumaM;
    float gradientS = lumaS - lumaM;
    bool pairN = abs(gradientN) > abs(gradientS);
    float gradient = abs(gradientN);
    if (!pairN) gradient = abs(gradientS);
    float gradientScaled = gradient * 0.25;
    float lumaNN = lumaN + lumaM;
    if (!pairN) lumaNN = lumaS + lumaM;
    float lumaMN = lumaNN * 0.5;
    float lengthSign = offs.y;
    if (!horzSpan) lengthSign = offs.x;
    if (pairN) lengthSign = -lengthSign;
    float2 posN = u;
    if (!horzSpan) posN.x += lengthSign * 0.5;
    if (horzSpan) posN.y += lengthSign * 0.5;
    float2 posP = posN;
    float2 offNP = offs;
    if (!horzSpan) offNP.x = 0.0;
    if (horzSpan) offNP.y = 0.0;
    posP += offNP * FAAA_OFF_MULT[0];
    posN -= offNP * FAAA_OFF_MULT[0];
    float lumaEndP = fxaa_luma(texNoLod(posP).xyz);
    float lumaEndN = fxaa_luma(texNoLod(posN).xyz);
    lumaEndP -= lumaMN;
    lumaEndN -= lumaMN;
    bool doneP = abs(lumaEndP) > gradientScaled;
    bool doneN = abs(lumaEndN) > gradientScaled;
    [unroll] for (int i = 1; i < FAAA_QUALITY__PI; i++)
    {
        if (!doneP)
        {
            posP += offNP * FAAA_OFF_MULT[i];
            lumaEndP = fxaa_luma(texNoLod(posP).xyz);
            lumaEndP -= lumaMN;
            doneP = abs(lumaEndP) > gradientScaled;
        }
        if (!doneN)
        {
            posN -= offNP * FAAA_OFF_MULT[i];
            lumaEndN = fxaa_luma(texNoLod(posN).xyz);
            lumaEndN -= lumaMN;
            doneN = abs(lumaEndN) > gradientScaled;
        }
        if (doneN && doneP)
            break;
    }
    float2 posM = u;
    float dstN = posM.x - posN.x;
    float dstP = posP.x - posM.x;
    if (!horzSpan) dstN = posM.y - posN.y;
    if (!horzSpan) dstP = posP.y - posM.y;
    bool dstNLTdstP = dstN < dstP;
    bool lumaMLTZero = lumaM - lumaMN < 0.0;
    bool mSpanLTZero = dstNLTdstP ? lumaEndN < 0.0 : lumaEndP < 0.0;
    bool goodSpan = mSpanLTZero != lumaMLTZero;
    float dst = dstNLTdstP ? dstN : dstP;
    float spanLength = dstP + dstN;
    float pixelOffset = (-dst / spanLength) + 0.5;
    bool pixelOffsetLTZero = pixelOffset < 0.0;
    if (pixelOffsetLTZero || !goodSpan)
        return rgbyM;
    if (!horzSpan) posM.x += pixelOffset * lengthSign;
    if (horzSpan) posM.y += pixelOffset * lengthSign;
    return texNoLod(posM).xyz;
}

float4 faaa_fs(PostVaryings v) : SV_Target
{
    return post_out(faaa(v.clip.xy * w_h_height.xy));
}

// ---- NFAA (fs_pp_nfaa.sc: VPX's variant 1, edges from luminance) ----

float nfaa_luminance(float3 l)
{
    return dot(l, float3(0.25, 0.5, 0.25)); // experimental, red and blue should not suffer too much
}

float2 nfaa_contrast(float2 XYCoord, float filterSpread)
{
    float2 upOffset = float2(0.0, w_h_height.y * filterSpread);
    float2 rightOffset = float2(w_h_height.x * filterSpread, 0.0);

    float topHeight = nfaa_luminance(texNoLod(XYCoord + upOffset).rgb);
    float bottomHeight = nfaa_luminance(texNoLod(XYCoord - upOffset).rgb);
    float rightHeight = nfaa_luminance(texNoLod(XYCoord + rightOffset).rgb);
    float leftHeight = nfaa_luminance(texNoLod(XYCoord - rightOffset).rgb);
    float leftTopHeight = nfaa_luminance(texNoLod(XYCoord - rightOffset + upOffset).rgb);
    float leftBottomHeight = nfaa_luminance(texNoLod(XYCoord - rightOffset - upOffset).rgb);
    float rightBottomHeight = nfaa_luminance(texNoLod(XYCoord + rightOffset + upOffset).rgb);
    float rightTopHeight = nfaa_luminance(texNoLod(XYCoord + rightOffset - upOffset).rgb);

    float sum0 = rightTopHeight + topHeight + rightBottomHeight;
    float sum1 = leftTopHeight + bottomHeight + leftBottomHeight;
    float sum2 = leftTopHeight + leftHeight + rightTopHeight;
    float sum3 = leftBottomHeight + rightHeight + rightBottomHeight;

    // finite differences for final vectors
    return float2(sum1 - sum0, sum2 - sum3);
}

float4 nfaa_fs(PostVaryings v) : SV_Target
{
    float filterStrength = 1.0;
    float filterSpread = 4.0;

    float2 u = v.clip.xy * w_h_height.xy;

    float3 Scene0 = texNoLod(u).rgb;
    [branch] if (post_untouched(u))
        return post_out(Scene0);

    float2 Vectors = nfaa_contrast(u, filterSpread);

    float filterStrength2 = filterStrength + filterSpread * 0.5;
    float filterClamp = filterStrength2 / filterSpread;

    Vectors = clamp(Vectors * filterStrength2, -float2(filterClamp, filterClamp), float2(filterClamp, filterClamp));

    float2 Normal = Vectors * w_h_height.xy;

    float3 Scene1 = texNoLod(u + Normal).rgb;
    float3 Scene2 = texNoLod(u - Normal).rgb;
    float3 Scene3 = texNoLod(u + float2(Normal.x, -Normal.y) * 0.5).rgb;
    float3 Scene4 = texNoLod(u - float2(Normal.x, -Normal.y) * 0.5).rgb;

    return post_out((Scene0 + Scene1 + Scene2 + Scene3 + Scene4) * 0.2);
}

// ---- DLAA (fs_pp_dlaa_edge.sc, then fs_pp_dlaa.sc) ----

#define sampleOffset(u, pixelOffset) texNoLod(u + (pixelOffset) * w_h_height.xy)

float dlaa_avg(float3 l)
{
    return (l.x + l.y + l.z) * (1.0 / 3.0);
}

// the edges found, into the picture's alpha, its colours kept
float4 dlaa_edge_fs(PostVaryings v) : SV_Target
{
    float2 u = v.clip.xy * w_h_height.xy;

    float3 sCenter = sampleOffset(u, float2(0.0, 0.0)).rgb;
    float3 sUpLeft = sampleOffset(u, float2(-0.5, -0.5)).rgb;
    float3 sUpRight = sampleOffset(u, float2(0.5, -0.5)).rgb;
    float3 sDownLeft = sampleOffset(u, float2(-0.5, 0.5)).rgb;
    float3 sDownRight = sampleOffset(u, float2(0.5, 0.5)).rgb;

    float3 diff = abs((sUpLeft + sUpRight) + (sDownLeft + sDownRight) - sCenter * 4.0);
    float edgeMask = dlaa_avg(diff) * 4.0; //!! magic

    return float4(sCenter, edgeMask);
}

float4 dlaa_fs(PostVaryings v) : SV_Target
{
    float2 u = v.clip.xy * w_h_height.xy;

    float4 sampleCenter = sampleOffset(u, float2(0.0, 0.0));
    [branch] if (post_untouched(u))
        return post_out(sampleCenter.xyz);

    // short edges
    float4 sampleHorizNeg0 = sampleOffset(u, float2(-1.5, 0.0));
    float4 sampleHorizPos0 = sampleOffset(u, float2(1.5, 0.0));
    float4 sampleVertNeg0 = sampleOffset(u, float2(0.0, -1.5));
    float4 sampleVertPos0 = sampleOffset(u, float2(0.0, 1.5));

    float3 sumHoriz = sampleHorizNeg0.xyz + sampleHorizPos0.xyz;
    float3 sumVert = sampleVertNeg0.xyz + sampleVertPos0.xyz;

    float3 sampleLeft = sampleOffset(u, float2(-1.0, 0.0)).rgb;
    float3 sampleRight = sampleOffset(u, float2(1.0, 0.0)).rgb;
    float3 sampleTop = sampleOffset(u, float2(0.0, -1.0)).rgb;
    float3 sampleDown = sampleOffset(u, float2(0.0, 1.0)).rgb;

    float3 diffToCenterHoriz = abs((sampleLeft + sampleRight) * 0.5 - sampleCenter.xyz);
    float3 diffToCenterVert = abs((sampleTop + sampleDown) * 0.5 - sampleCenter.xyz);

    float valueEdgeHoriz = dlaa_avg(diffToCenterHoriz);
    float valueEdgeVert = dlaa_avg(diffToCenterVert);

    float edgeDetectHoriz = 3.0 * valueEdgeHoriz - 0.1; //!! magic params
    float edgeDetectVert = 3.0 * valueEdgeVert - 0.1;

    float3 avgHoriz = (sumHoriz + sampleCenter.xyz) * (1.0 / 3.0);
    float3 avgVert = (sumVert + sampleCenter.xyz) * (1.0 / 3.0);

    float valueHoriz = dlaa_avg(avgHoriz);
    float valueVert = dlaa_avg(avgVert);

    float blurAmountHoriz = saturate(edgeDetectHoriz / valueHoriz);
    float blurAmountVert = saturate(edgeDetectVert / valueVert);

    float3 aaResult = lerp(sampleCenter.xyz, avgHoriz, blurAmountVert * 0.5); //!! magic sharpen
    aaResult = lerp(aaResult, avgVert, blurAmountHoriz * 0.5); //!! magic sharpen

    // long edges
    float4 sampleVertNeg1 = sampleOffset(u, float2(0.0, -3.5));
    float4 sampleVertNeg15 = sampleOffset(u, float2(0.0, -5.5));
    float4 sampleVertNeg2 = sampleOffset(u, float2(0.0, -7.5));
    float4 sampleVertPos1 = sampleOffset(u, float2(0.0, 3.5));
    float4 sampleVertPos15 = sampleOffset(u, float2(0.0, 5.5));
    float4 sampleVertPos2 = sampleOffset(u, float2(0.0, 7.5));

    float4 sampleHorizNeg1 = sampleOffset(u, float2(-3.5, 0.0));
    float4 sampleHorizNeg15 = sampleOffset(u, float2(-5.5, 0.0));
    float4 sampleHorizNeg2 = sampleOffset(u, float2(-7.5, 0.0));
    float4 sampleHorizPos1 = sampleOffset(u, float2(3.5, 0.0));
    float4 sampleHorizPos15 = sampleOffset(u, float2(5.5, 0.0));
    float4 sampleHorizPos2 = sampleOffset(u, float2(7.5, 0.0));

    float pass1EdgeAvgHoriz = saturate((sampleHorizNeg2.a + sampleHorizNeg1.a + sampleHorizNeg15.a + sampleHorizNeg0.a + sampleHorizPos0.a + sampleHorizPos1.a + sampleHorizPos15.a + sampleHorizPos2.a) * (2.0 / 8.0) - 1.0);
    float pass1EdgeAvgVert = saturate((sampleVertNeg2.a + sampleVertNeg1.a + sampleVertNeg15.a + sampleVertNeg0.a + sampleVertPos0.a + sampleVertPos1.a + sampleVertPos15.a + sampleVertPos2.a) * (2.0 / 8.0) - 1.0);

    [branch] if (abs(pass1EdgeAvgHoriz - pass1EdgeAvgVert) > 0.2) //!! magic
    {
        float valueHorizLong = dlaa_avg(sampleHorizNeg2.xyz + sampleHorizNeg1.xyz + sampleHorizNeg15.xyz + sampleHorizNeg0.xyz + sampleHorizPos0.xyz + sampleHorizPos1.xyz + sampleHorizPos15.xyz + sampleHorizPos2.xyz) * (1.0 / 8.0);
        float valueVertLong = dlaa_avg(sampleVertNeg2.xyz + sampleVertNeg1.xyz + sampleVertNeg15.xyz + sampleVertNeg0.xyz + sampleVertPos0.xyz + sampleVertPos1.xyz + sampleVertPos15.xyz + sampleVertPos2.xyz) * (1.0 / 8.0);

        float valueCenter = dlaa_avg(sampleCenter.xyz);
        float valueLeft = dlaa_avg(sampleLeft);
        float valueRight = dlaa_avg(sampleRight);
        float valueTop = dlaa_avg(sampleTop);
        float valueBottom = dlaa_avg(sampleDown);

        float vx = (valueCenter == valueLeft) ? 0. : saturate((valueVertLong - valueLeft) / (valueCenter - valueLeft));
        float hx = (valueCenter == valueTop) ? 0. : saturate((valueHorizLong - valueTop) / (valueCenter - valueTop));
        float vy = (valueCenter == valueRight) ? 0. : saturate(1.0 + (valueVertLong - valueCenter) / (valueCenter - valueRight));
        float hy = (valueCenter == valueBottom) ? 0. : saturate(1.0 + (valueHorizLong - valueCenter) / (valueCenter - valueBottom));

        float3 longBlurVert = lerp(sampleRight, lerp(sampleLeft, sampleCenter.xyz, vx), vy);
        float3 longBlurHoriz = lerp(sampleDown, lerp(sampleTop, sampleCenter.xyz, hx), hy);

        aaResult = lerp(aaResult, longBlurVert, pass1EdgeAvgVert * 0.5); //!! magic
        aaResult = lerp(aaResult, longBlurHoriz, pass1EdgeAvgHoriz * 0.5); //!! magic
    }

    return post_out(aaResult);
}

// ---- CAS (fs_pp_cas.sc: the variant with better diagonals) ----

float4 cas_fs(PostVaryings v) : SV_Target
{
    float Contrast = 0.0; // 0..1, Adjusts the range the shader adapts to high contrast (0 is not all the way off).  Higher values = more high contrast sharpening.
    float Sharpening = 1.0; // 0..1, Adjusts sharpening intensity by averaging the original pixels to the sharpened result.  1.0 is the unmodified default.

    float2 u = v.clip.xy * w_h_height.xy;

    float3 e = texNoLod(u).xyz;
    [branch] if (post_untouched(u))
        return post_out(e);

    // fetch a 3x3 neighborhood around the pixel 'e',
    //  a b c
    //  d(e)f
    //  g h i
    float2 um1 = u - w_h_height.xy;
    float2 up1 = u + w_h_height.xy;

    float3 a = texNoLod(um1).rgb;
    float3 b = texNoLod(float2(u.x, um1.y)).rgb;
    float3 c = texNoLod(float2(up1.x, um1.y)).rgb;
    float3 d = texNoLod(float2(um1.x, u.y)).rgb;
    float3 g = texNoLod(float2(um1.x, up1.y)).rgb;
    float3 f = texNoLod(float2(up1.x, u.y)).rgb;
    float3 h = texNoLod(float2(u.x, up1.y)).rgb;
    float3 i = texNoLod(up1).rgb;

    // Soft min and max.
    //  a b c             b
    //  d e f * 0.5  +  d e f * 0.5
    //  g h i             h
    // These are 2.0x bigger (factored out the extra multiply).
    float3 mnRGB = min(min(min(d, e), min(f, b)), h);
    float3 mnRGB2 = min(mnRGB, min(min(a, c), min(g, i)));
    mnRGB += mnRGB2;

    float3 mxRGB = max(max(max(d, e), max(f, b)), h);
    float3 mxRGB2 = max(mxRGB, max(max(a, c), max(g, i)));
    mxRGB += mxRGB2;

    // Smooth minimum distance to signal limit divided by smooth max.
    float3 rcpMRGB = rcp(mxRGB);
    float3 ampRGB = saturate(min(mnRGB, 2.0 - mxRGB) * rcpMRGB);

    // Shaping amount of sharpening.
    ampRGB = rsqrt(ampRGB);

    float peak = -3.0 * Contrast + 8.0;
    float3 wRGB = rcp(ampRGB * -peak);

    float3 rcpWeightRGB = rcp(4.0 * wRGB + 1.0);

    //                          0 w 0
    //  Filter shape:           w 1 w
    //                          0 w 0
    float3 window = (b + d) + (f + h);
    float3 outColor = saturate((window * wRGB + e) * rcpWeightRGB);

    return post_out(lerp(e, outColor, Sharpening));
}

// ---- Bilateral CAS (fs_pp_bilateral_cas.sc) ----

float bcas_normpdf(float3 v, float sigma)
{
    return exp(dot(v, v) * (-0.5 / (sigma * sigma))) * (0.39894228040143 / sigma);
}

float bcas_LI(float3 l)
{
    return (l.x * 0.5 + l.y) + l.z * 0.5; // experimental, red and blue should not suffer too much //!! scaled by 2, so that we save a mul, factors out below, except for one constant! (see below)
}

float4 bilateral_cas_fs(PostVaryings v) : SV_Target
{
    float sharpness = 0.625 * 3.1; // ~0.1..~2 *3.1
    float balance = 0.0;           // [0..1]

    float2 u = v.clip.xy * w_h_height.xy;

    float3 mid = texNoLod(u).rgb;
    [branch] if (post_untouched(u))
        return post_out(mid);

    float3 e[9] = {
        texNoLod(float2(u.x - w_h_height.x, u.y - w_h_height.y)).rgb,
        texNoLod(float2(u.x, u.y - w_h_height.y)).rgb,
        texNoLod(float2(u.x + w_h_height.x, u.y - w_h_height.y)).rgb,

        texNoLod(float2(u.x - w_h_height.x, u.y)).rgb,
        mid,
        texNoLod(float2(u.x + w_h_height.x, u.y)).rgb,

        texNoLod(float2(u.x - w_h_height.x, u.y + w_h_height.y)).rgb,
        texNoLod(float2(u.x, u.y + w_h_height.y)).rgb,
        texNoLod(float2(u.x + w_h_height.x, u.y + w_h_height.y)).rgb
    };

    // Bilateral Blur (crippled)
    float3 final_colour = float3(0., 0., 0.);
    float Z = 0.0;
    [unroll] for (int j = -2; j <= 2; ++j) // 2 = kernelradius
    {
        int y = j < 0 ? -3 : 0;
        y = j > 0 ? 3 : y;

        [unroll] for (int i = -2; i <= 2; ++i)
        {
            int x = i < 0 ? -1 : 0;
            x = i > 0 ? 1 : x;

            float3 cc;
            if ((abs(j) == 2 || j == 0) && (abs(i) == 2 || i == 0)) // integer offset pixels, no filtering
                cc = e[4 + x + y];
            else
            {
                if (abs(j) == 1 && abs(i) == 1)
                    cc = ((e[4] + e[4 + x]) + (e[4 + y] + e[4 + x + y])) * 0.25;
                else
                {
                    if (abs(j) == 2)
                        cc = e[4 + y] + e[4 + x + y];
                    else if (abs(i) == 2)
                        cc = e[4 + x] + e[4 + x + y];
                    else if (abs(j) == 1)
                        cc = e[4] + e[4 + y];
                    else
                        cc = e[4] + e[4 + x];
                    cc *= 0.5;
                }
            }

            float factor = bcas_normpdf(cc - e[4], 0.25); // 0.25 = BSIGMA
            Z += factor;
            final_colour += factor * cc;
        }
    }

    // CAS (without Better Diagonals)

    float b = bcas_LI(e[1]);
    float d = bcas_LI(e[3]);
    float f = bcas_LI(e[5]);
    float h = bcas_LI(e[7]);
    float e4 = bcas_LI(e[4]);

    float mnRGB = min(min(min(d, e4), min(f, b)), h);
    float mxRGB = max(max(max(d, e4), max(f, b)), h);

    // Smooth minimum distance to signal limit divided by smooth max.
    float rcpMRGB = rcp(mxRGB);
    float ampRGB = saturate(min(mnRGB, 2.0 - mxRGB) * rcpMRGB); //!! 2.0 instead of 1.0, as LI() is scaled by x2!
    ampRGB *= saturate(sharpness);

    ampRGB = lerp(ampRGB, 1.0 - ampRGB, balance);

    float3 sharpen = (e[4] - final_colour / Z) * sharpness;

    float gs_sharpen = (sharpen.x + sharpen.y + sharpen.z) * 0.333333333333;
    sharpen = lerp(float3(gs_sharpen, gs_sharpen, gs_sharpen), sharpen, 0.5);

    return post_out(e[4] + sharpen * ampRGB);
}

