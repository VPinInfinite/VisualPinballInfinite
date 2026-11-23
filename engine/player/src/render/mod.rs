//! Draws the table with SDL3 GPU on Direct3D 12: the scene from
//! `vpin`'s meshes, the balls, and the on-screen readouts. Shaders are HLSL,
//! compiled at startup by FXC. Drawing never waits for the graphics card: when
//! no swapchain image is free, the frame is skipped.

mod backglass;
mod effects;
mod embed;
mod fxc;
mod hdr10;
mod menu;
mod mips;
mod post;
mod pup;
mod segments;
mod still;

pub use effects::AmbientOcclusion;
pub use hdr10::{Hdr10, HdrRequest};
pub use menu::MenuPicture;
pub use post::{PostAa, Sharpen};

use still::Stage;

pub use embed::Kind as EmbedKind;
use embed::Surface;

use std::ffi::CStr;
use std::mem::zeroed;
use std::ptr::{null, null_mut};

use sdl3_sys::everything::*;

use crate::font;
use crate::math::{Mat4, Vec3};
use crate::scene::{Scene, Vertex};

const SCENE_HLSL: &str = include_str!("player.hlsl");
const OVERLAY_HLSL: &str = include_str!("overlay.hlsl");
/// The score display's dots, compiled after the overlay's or the scene's shaders.
const DMD_HLSL: &str = include_str!("dmd.hlsl");
/// The score display's own window: VPX's newer display renderer.
const DISPLAY_HLSL: &str = include_str!("display.hlsl");
/// VPX's look: the final pass, tonemapping the table's picture into the window's colours.
const TONEMAP_HLSL: &str = include_str!("tonemap.hlsl");
/// VPX's look: VPX's basic material, and its environment's irradiance.
const MATERIAL_HLSL: &str = include_str!("material.hlsl");
/// The most bytes of pictures on their way to the graphics card at once.
const UPLOAD_BATCH: usize = 256 << 20;

/// About the most memory the pictures being decoded and waiting to be
/// uploaded may take at once (`decode_cost`): measured on the biggest test
/// tables, 1.5 GB loads them faster than decoding everything
/// at once did, at under half its memory; 1 GB saved 0.4 GB more but took
/// 1.5 s longer (two 8192 × 8192 light maps no longer decoded side by side).
const DECODE_BUDGET: usize = 1536 << 20;

/// Copies a picture from `transfer` (at `at`) into `texture`'s top level.
unsafe fn upload_texture(copy: *mut SDL_GPUCopyPass, transfer: *mut SDL_GPUTransferBuffer, texture: *mut SDL_GPUTexture, at: u32, w: u32, h: u32) {
    unsafe {
        let mut from: SDL_GPUTextureTransferInfo = zeroed();
        from.transfer_buffer = transfer;
        from.offset = at;
        let mut to: SDL_GPUTextureRegion = zeroed();
        to.texture = texture;
        to.w = w;
        to.h = h;
        to.d = 1;
        SDL_UploadToGPUTexture(copy, &from, &to, false);
    }
}

/// The bytes from one row of a picture `w` texels wide to the next in an
/// upload laid out as Direct3D 12 copies straight from it (rows 256 bytes
/// apart, `D3D12_TEXTURE_DATA_PITCH_ALIGNMENT`); SDL otherwise copies each
/// upload row by row into a buffer of its own first, which with every
/// picture's mipmaps took most of a second on a big table.
fn upload_pitch(w: u32, texel_bytes: usize) -> usize {
    (w as usize * texel_bytes).next_multiple_of(256)
}

/// The bytes a level `w` × `h` takes so laid out, rounded so the next starts
/// on 512 bytes (`D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT`).
fn upload_span(w: u32, h: u32, texel_bytes: usize) -> usize {
    (upload_pitch(w, texel_bytes) * h as usize).next_multiple_of(512)
}

/// The bytes a picture's levels take so laid out: the first (`w` × `h`),
/// then `chain`'s.
fn chain_span(w: u32, h: u32, texel_bytes: usize, chain: &[mips::Level]) -> usize {
    upload_span(w, h, texel_bytes) + chain.iter().map(|(lw, lh, _)| upload_span(*lw, *lh, texel_bytes)).sum::<usize>()
}

/// Writes a picture's levels, the first (`first`, `w` × `h`, rows packed)
/// and `chain`'s, at `at` in `mapped`, laid out for the upload; returns
/// the bytes taken (`chain_span`).
unsafe fn put_chain(mapped: *mut u8, at: usize, first: &[u8], w: u32, h: u32, texel_bytes: usize, chain: &[mips::Level]) -> usize {
    let mut offset = at;
    for (lw, lh, texels) in std::iter::once((w, h, first)).chain(chain.iter().map(|(lw, lh, l)| (*lw, *lh, l.as_slice()))) {
        let (row, pitch) = (lw as usize * texel_bytes, upload_pitch(lw, texel_bytes));
        for y in 0..lh as usize {
            unsafe { std::ptr::copy_nonoverlapping(texels.as_ptr().add(y * row), mapped.add(offset + y * pitch), row) };
        }
        offset += upload_span(lw, lh, texel_bytes);
    }
    offset - at
}

/// Uploads the levels `put_chain` wrote at `at` in `transfer` into
/// `texture`, each copied straight from its place.
#[allow(clippy::too_many_arguments)]
unsafe fn upload_chain(copy: *mut SDL_GPUCopyPass, transfer: *mut SDL_GPUTransferBuffer, texture: *mut SDL_GPUTexture, at: usize, w: u32, h: u32, texel_bytes: usize, chain: &[mips::Level]) {
    let mut offset = at;
    let sizes = std::iter::once((w, h)).chain(chain.iter().map(|(lw, lh, _)| (*lw, *lh)));
    for (level, (lw, lh)) in sizes.enumerate() {
        unsafe {
            let mut from: SDL_GPUTextureTransferInfo = zeroed();
            from.transfer_buffer = transfer;
            from.offset = offset as u32;
            from.pixels_per_row = (upload_pitch(lw, texel_bytes) / texel_bytes) as u32;
            from.rows_per_layer = lh;
            let mut to: SDL_GPUTextureRegion = zeroed();
            to.texture = texture;
            to.mip_level = level as u32;
            to.w = lw;
            to.h = lh;
            to.d = 1;
            SDL_UploadToGPUTexture(copy, &from, &to, false);
        }
        offset += upload_span(lw, lh, texel_bytes);
    }
}

/// `f` of each item, in order, worked out on up to 8 threads.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get).clamp(1, 8).min(items.len());
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let parts: Vec<Vec<(usize, R)>> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        done.push((i, f(item)));
                    }
                    done
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().expect("a worker thread")).collect()
    });
    let mut out: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    for (i, r) in parts.into_iter().flatten() {
        out[i] = Some(r);
    }
    out.into_iter().map(|r| r.expect("every item worked out")).collect()
}

/// VPX's mipmaps for a decoded picture (`mips`); none for one of a texel.
fn image_chain(image: &crate::scene::Image) -> Vec<mips::Level> {
    if image.width.max(image.height) <= 1 {
        return Vec::new();
    }
    match &image.hdr {
        Some(hdr) => mips::chain(mips::Texels::Half, bytes_of_slice(hdr), image.width, image.height),
        None => mips::chain(mips::Texels::Srgb8, &image.rgba, image.width, image.height),
    }
}

/// The texels of a picture of `format` whose mipmaps VPX makes itself
/// (`mips`); none for a format VPX's pictures don't come in.
fn mips_texels(format: SDL_GPUTextureFormat) -> Option<mips::Texels> {
    if format == SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB {
        Some(mips::Texels::Srgb8)
    } else if format == SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM {
        Some(mips::Texels::Unorm8)
    } else if format == SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT {
        Some(mips::Texels::Half)
    } else {
        None
    }
}

/// VPX's look: bloom, the glow around what's brighter than VPX's threshold.
const BLOOM_HLSL: &str = include_str!("bloom.hlsl");
/// VPX 5436's embedded windows: a window's picture taken into the table's (`embed.rs`).
const EMBED_HLSL: &str = include_str!("embed.hlsl");
/// The irradiance picture's format (VPX's for an environment of 8 bits a colour).
const IRRADIANCE_FORMAT: SDL_GPUTextureFormat = SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT;
const DEPTH_FORMAT: SDL_GPUTextureFormat = SDL_GPU_TEXTUREFORMAT_D32_FLOAT;
/// The table's picture in VPX's look: half floats, so light adds up past
/// white and blends can use negative colours, as VPX's do.
const HDR_FORMAT: SDL_GPUTextureFormat = SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT;

/// How the table is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookMode {
    /// The first look, kept for comparing: straight into the
    /// window's colours, which clamp at white; bulbs and additive flashers in
    /// two passes.
    Classic,
    /// VPX's look as it's ported, the default: a half-float picture, VPX's materials and single-pass
    /// blends, bloom, the ball, reflections, and VPX's tonemapping.
    Vpx,
}

/// VPX's tonemapping pass (`fs_pp_tonemap.sc`'s uniforms).
#[repr(C)]
#[derive(Clone, Copy)]
struct TonemapUniforms {
    /// x: the exposure; y: the tone mapper.
    exposure_mapper: [f32; 4],
    /// x: dither; y: colour grade; z: the target encodes sRGB itself.
    flags: [f32; 4],
    /// x: the frame's number, 0 to 2047, for the dither; yz: 1 / the
    /// window's size; w: the picture is bigger (supersampled), read filtered.
    frame: [f32; 4],
    /// x: VPX's dynamic occlusion over the picture; yz: 1 / the table's
    /// picture's size.
    ao: [f32; 4],
    /// VPX's HDR10 output, x 0 without it; its spline (`hdr10.rs`).
    hdr: [f32; 4],
    spline1: [f32; 4],
    spline2: [f32; 4],
}
const MAX_OVERLAY_CHARS: usize = 8192;

pub fn sdl_error() -> String {
    unsafe { CStr::from_ptr(SDL_GetError()).to_string_lossy().into_owned() }
}

fn check(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(format!("{what}: {}", sdl_error())) }
}

fn non_null<T>(pointer: *mut T, what: &str) -> Result<*mut T, String> {
    if pointer.is_null() { Err(format!("{what}: {}", sdl_error())) } else { Ok(pointer) }
}

/// The GPU device, on Direct3D 12: the graphics card Windows ranks for high performance, or with
/// `VPI_GPU_LOW_POWER` set the one it ranks for low power (a laptop's or a processor's own, where
/// there's a second). Which card it is goes in the log.
///
/// # Safety
/// On the thread that draws.
unsafe fn create_device() -> *mut SDL_GPUDevice {
    unsafe {
        let props = SDL_CreateProperties();
        SDL_SetStringProperty(props, SDL_PROP_GPU_DEVICE_CREATE_NAME_STRING, c"direct3d12".as_ptr());
        SDL_SetBooleanProperty(props, SDL_PROP_GPU_DEVICE_CREATE_SHADERS_DXBC_BOOLEAN, true);
        SDL_SetBooleanProperty(props, SDL_PROP_GPU_DEVICE_CREATE_DEBUGMODE_BOOLEAN, false);
        let low_power = std::env::var_os("VPI_GPU_LOW_POWER").is_some_and(|v| !v.is_empty() && v != "0");
        SDL_SetBooleanProperty(props, SDL_PROP_GPU_DEVICE_CREATE_PREFERLOWPOWER_BOOLEAN, low_power);
        let device = SDL_CreateGPUDeviceWithProperties(props);
        SDL_DestroyProperties(props);
        if !device.is_null() {
            let info = SDL_GetGPUDeviceProperties(device);
            let text = |name| {
                let s = SDL_GetStringProperty(info, name, null());
                (!s.is_null()).then(|| CStr::from_ptr(s).to_string_lossy().into_owned()).filter(|s| !s.is_empty())
            };
            let card = text(SDL_PROP_GPU_DEVICE_NAME_STRING).unwrap_or_else(|| "not named".into());
            match text(SDL_PROP_GPU_DEVICE_DRIVER_VERSION_STRING) {
                Some(version) => println!("the graphics card: {card} (its driver {version})"),
                None => println!("the graphics card: {card}"),
            }
        }
        device
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameUniforms {
    /// VPX's `matRotViewProj`: the view's turning, scaling and layback, then
    /// the projection (and the frame's shift), for points from which the
    /// camera's place was taken (VPX's camera-relative projection).
    rot_view_proj: [[f32; 4]; 4],
    /// VPX's `cameraPosWorld`: where the view moves the origin from, taken
    /// off each point before it's projected.
    camera: [f32; 4],
    eye: [f32; 4],
    /// Drawing a mirror, the plane whose side is kept; (0, 0, 0, 1) otherwise.
    clip: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MaterialUniforms {
    base_color: [f32; 4],
    emissive: [f32; 4],
    params: [f32; 4],
    light0: [f32; 4],
    light1: [f32; 4],
    /// x: how far lighting is switched off (VPX's `BlendDisableLighting`).
    extra: [f32; 4],
    /// The part's colour and opacity, multiplying what is drawn (VPX's
    /// `staticColor_Alpha`: a primitive's `Color` and `Opacity`).
    static_color_alpha: [f32; 4],
}

/// VPX's ball shadows (`ball_shadows.sh`'s uniforms): the light whose
/// shadows fall on the draw, and the balls.
#[repr(C)]
#[derive(Clone, Copy)]
struct ShadowUniforms {
    /// The light's place; w: 1 to shadow.
    light: [f32; 4],
    /// Each ball's place and radius; a radius of 0 ends them.
    balls: [[f32; 4]; 8],
}

/// A part in VPX's look, for `fs_vpx` (VPX's basic shader's material
/// uniforms, `Shader::SetMaterial`).
#[repr(C)]
#[derive(Clone, Copy)]
struct VpxMaterialUniforms {
    base_alpha: [f32; 4],
    glossy_image_lerp: [f32; 4],
    clearcoat_edge_alpha: [f32; 4],
    roughness_wrap_edge_thickness: [f32; 4],
    disable_lighting_top_below: [f32; 4],
    static_color_alpha: [f32; 4],
    /// x: metal; y: textured; z: the alpha test; w: marks a solid part.
    flags: [f32; 4],
    /// The mirror it shows: its plane's normal, and how strongly (0: none).
    mirror_normal_factor: [f32; 4],
    /// x: a normal map; y: in the primitive's own space.
    normal_flags: [f32; 4],
    /// The rows taking a normal from the primitive's own space to the table's.
    object_normal: [[f32; 4]; 3],
}

/// VPX's look: a classic light with a picture of its own (fs_classic_light.sc's
/// uniforms, as light.cpp sets them).
#[repr(C)]
#[derive(Clone, Copy)]
struct ClassicLightUniforms {
    /// Where it shines from, and 1 / its falloff radius.
    center_range: [f32; 4],
    /// The edge's colour, and the falloff's power.
    color2_falloff_power: [f32; 4],
    /// The centre's colour, and its intensity now.
    color_intensity: [f32; 4],
    /// x: drawn unlit (image mode).
    lighting_off: [f32; 4],
}

/// The table's light for `fs_vpx`, once a frame (VPX's `SetupShaders`).
#[repr(C)]
#[derive(Clone, Copy)]
struct VpxSceneUniforms {
    /// x: the environment's emission scale; y: its picture's height.
    env_emission_height: [f32; 4],
    /// VPX 5436 leaves the basic shader's ambient colour and light range at
    /// zero (it sets them for the ball only).
    ambient_range: [f32; 4],
    light_pos: [[f32; 4]; 2],
    light_emission: [[f32; 4]; 2],
    /// xy: 1 / the picture's size; z: the bulb light buffer has light.
    w_h_transmission: [f32; 4],
    /// The view's turn (rows; a mirror's view turned and mirrored): VPX's
    /// view space, where its normal maps' frame is made.
    view_rows: [[f32; 4]; 3],
}

/// A ball in VPX's look, for `fs_vpx_ball` (VPX's ball shader uniforms,
/// `Ball::Render`).
#[repr(C)]
#[derive(Clone, Copy)]
struct VpxBallUniforms {
    base_alpha: [f32; 4],
    roughness_wrap_edge_thickness: [f32; 4],
    ambient_range: [f32; 4],
    /// z: how much the ball mirrors the playfield.
    inv_table_res_reflection: [f32; 4],
    /// xy: 1.5 texels of the previous frame; z: lighting off; w: the previous frame is there.
    w_h_disable_lighting: [f32; 4],
    /// x: a sphere map; y: decal mode.
    flags: [f32; 4],
    view: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    light_pos: [[f32; 4]; 10],
    light_emission: [[f32; 4]; 10],
}

/// One of VPX's Gaussian blurs for `blur_kernel_*_fs` (`fs_pp_blur.sc`).
#[repr(C)]
#[derive(Clone, Copy)]
struct KernelUniforms {
    offset: [[f32; 4]; 3],
    weight: [[f32; 4]; 3],
    /// x: taps; y: 1 when the first is the centre, taken once.
    taps: [f32; 4],
}

impl KernelUniforms {
    /// VPX's blur for a kernel size (`RenderDevice::DrawGaussianBlur`'s choice), with
    /// `fs_pp_blur.sc`'s offsets and weights.
    fn vpx(size: f32) -> KernelUniforms {
        let (centre, offsets, weights): (bool, &[f32], &[f32]) = if size < 8.0 {
            (false, &[0.53473, 2.05896], &[0.45134, 0.04866])
        } else if size < 10.0 {
            (true, &[0.0, 1.384_615_4, 3.230_769_3], &[0.226_971_26, 0.316_138_54, 0.070_253_01])
        } else if size < 12.0 {
            (false, &[0.62195, 2.27357, 4.14706], &[0.32993, 0.15722, 0.01285])
        } else if size < 14.0 {
            (true, &[0.0, 1.411_764_7, 3.294_117_6, 5.176_470_5], &[0.196_479_55, 0.296_902_43, 0.094_468_96, 0.010_381_204])
        } else if size < 17.0 {
            (false, &[0.64417, 2.37795, 4.28970, 6.21493], &[0.25044, 0.19233, 0.05095, 0.00628])
        } else if size < 21.0 {
            (false, &[0.65323, 2.42572, 4.36847, 6.31470, 8.26547], &[0.19923, 0.18937, 0.08396, 0.02337, 0.00408])
        } else if size < 25.0 {
            (false, &[0.64851, 2.40054, 4.32612, 6.25954, 8.20247, 10.15531], &[0.22791, 0.19358, 0.06543, 0.01187, 0.00115, 0.00006])
        } else if size < 31.0 {
            (false, &[0.65275, 2.42313, 4.36403, 6.30877, 8.25848, 10.21386, 12.17512], &[0.20227, 0.19003, 0.08192, 0.02181, 0.00358, 0.00036, 0.00002])
        } else {
            (
                false,
                &[0.66063, 2.46625, 4.43946, 6.41301, 8.38706, 10.36173, 12.33715, 14.31341, 16.29062, 18.26884],
                &[0.13669, 0.15600, 0.10738, 0.05971, 0.02682, 0.00973, 0.00285, 0.00067, 0.00013, 0.00002],
            )
        };
        let mut k = KernelUniforms { offset: [[0.0; 4]; 3], weight: [[0.0; 4]; 3], taps: [offsets.len() as f32, if centre { 1.0 } else { 0.0 }, 0.0, 0.0] };
        for (i, (&o, &w)) in offsets.iter().zip(weights).enumerate() {
            k.offset[i / 4][i % 4] = o;
            k.weight[i / 4][i % 4] = w;
        }
        k
    }
}

/// VPX's blur for a mirror's roughness (0 to 12; `RenderProbe::ApplyRoughness`), and whether
/// VPX draws that mirror at half size (`GetRoughnessDownscale`), which doubles the blur's reach.
/// The frames the graphics card may have queued (VPX's `[Player]` `MaxPrerenderedFrames`, 1 to 3,
/// 1 by default), set before the renderer is made.
static FRAMES_IN_FLIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// Sets the frames the graphics card may have queued, 1 to 3, for renderers made from now on.
pub fn set_frames_in_flight(frames: u32) {
    FRAMES_IN_FLIGHT.store(frames.clamp(1, 3), std::sync::atomic::Ordering::Relaxed);
}

/// VPX 5436's ball anti-stretch (`Ball::Render` with its `AntiStretchHelper`, ball.h and ball.cpp,
/// GPL): 35 points of the ball's sphere projected to find how the view stretches it on the screen,
/// turned back by the view's rotation, then a scale for the ball's width and its height and depth
/// that balances the two (it only works for a view lined up with the screen, as VPX's notes say).
/// 1 and 1 when a point falls behind the camera.
fn anti_stretch(view_proj: &Mat4, center: Vec3, radius: f32, w: f32, h: f32, rotation_deg: f32) -> [f32; 2] {
    let m = &view_proj.m;
    let (mut x_min, mut y_min, mut x_max, mut y_max) = (f32::MAX, f32::MAX, -f32::MAX, -f32::MAX);
    for [u, v, n] in stretch_fit_points() {
        let (px, py, pz) = (center.x + radius * u, center.y + radius * v, center.z + radius * n);
        let xp = m[0][0] * px + m[1][0] * py + m[2][0] * pz + m[3][0];
        let yp = m[0][1] * px + m[1][1] * py + m[2][1] * pz + m[3][1];
        let wp = m[0][3] * px + m[1][3] * py + m[2][3] * pz + m[3][3];
        if wp <= 1e-3 {
            return [1.0, 1.0];
        }
        x_min = x_min.min(xp / wp);
        x_max = x_max.max(xp / wp);
        y_min = y_min.min(yp / wp);
        y_max = y_max.max(yp / wp);
    }
    let (s, c) = (-rotation_deg.to_radians()).sin_cos();
    let (rx, ry) = ((x_max - x_min) * w, (y_max - y_min) * h);
    let (sx, sy) = ((c * rx - s * ry).abs(), (s * rx + c * ry).abs());
    [0.5 * (1.0 + sy / sx), 0.5 * (1.0 + sx / sy)]
}

/// `AntiStretchHelper`'s points: rings of the sphere spread evenly, 35 in all.
fn stretch_fit_points() -> Vec<[f32; 3]> {
    use std::f64::consts::PI;
    let a = 4.0 * PI / 35.0;
    let d = a.sqrt();
    let n_theta = (PI / d).round() as i32;
    let d_theta = PI / f64::from(n_theta);
    let d_phi = a / d_theta;
    let mut points = Vec::with_capacity(35);
    for j in 0..n_theta {
        let theta = (f64::from(j) + 0.5) * PI / f64::from(n_theta);
        let n_phi = (2.0 * PI * theta.sin() / d_phi).round() as i32;
        for i in 0..n_phi {
            let phi = f64::from(i) * (2.0 * PI) / f64::from(n_phi);
            points.push([(theta.sin() * phi.cos()) as f32, (theta.sin() * phi.sin()) as f32, theta.cos() as f32]);
        }
    }
    points
}

fn mirror_blur(roughness: u32) -> Option<(f32, bool)> {
    const KERNEL: [f32; 13] = [0.0, 7.0, 9.0, 13.0, 15.0, 19.0, 23.0, 13.0, 15.0, 19.0, 23.0, 27.0, 39.0];
    let r = roughness.min(12) as usize;
    (r > 0).then(|| (KERNEL[r], r >= 7))
}

/// VPX's blur for a refraction probe's roughness (0 to 12; `RenderProbe::ApplyRoughness`'s
/// kernels for screen-space transparency; VPX draws those at full size).
fn refraction_blur(roughness: u32) -> Option<f32> {
    const KERNEL: [f32; 13] = [0.0, 7.0, 9.0, 11.0, 13.0, 15.0, 19.0, 23.0, 27.0, 39.0, 39.0, 39.0, 39.0];
    let r = roughness.min(12) as usize;
    (r > 0).then_some(KERNEL[r])
}

/// A part drawn bent, for `fs_vpx` with `REFRACTION` (VPX's `refractionTint_thickness`
/// and the projection it takes the bent ray through, `matProj`).
#[repr(C)]
#[derive(Clone, Copy)]
struct RefractionUniforms {
    view_proj: [[f32; 4]; 4],
    tint_thickness: [f32; 4],
    /// x: 1 when the probe has no depth (multisampled), so nothing is held back.
    flags: [f32; 4],
}

/// VPX's default ball picture (`src/assets/BallEnv.exr`): a
/// tileable environment the ball reflects when the table names no ball picture.
const BALL_ENV: &[u8] = include_bytes!("../../assets/vpx/ball/BallEnv.exr");

/// A flasher, for `fs_flasher` (VPX's flasher shader uniforms).
#[repr(C)]
#[derive(Clone, Copy)]
struct FlasherUniforms {
    static_color_alpha: [f32; 4],
    alpha_test_filter_add: [f32; 4],
    amount_mva_mode: [f32; 4],
    term: [f32; 4],
}

/// The score display's dots, for `dmd_backdrop_fs` and `fs_dmd_flasher`
/// (VPX's legacy DMD shader uniforms).
#[repr(C)]
#[derive(Clone, Copy)]
struct DmdUniforms {
    /// The dots' tint times brightness (VPX's `vColor_Intensity`).
    tint: [f32; 4],
    /// The frame's size in dots, the alpha, and the frame number % 2048
    /// (VPX's `vRes_Alpha_time`).
    res_alpha_time: [f32; 4],
}

/// Where the display window's quad goes (`display_vs`).
#[repr(C)]
#[derive(Clone, Copy)]
struct DisplayQuadUniforms {
    /// Left, top, right, bottom, in clip space.
    rect: [f32; 4],
    glass_area: [f32; 4],
    glass_pad: [f32; 4],
}

/// How the display window draws its dots (`display_dmd_fs`) or a digit's
/// segments (`display_seg_fs`): VPX's uniforms for its newer display
/// renderer.
#[repr(C)]
#[derive(Clone, Copy)]
struct DisplayUniforms {
    lit_diffuse: [f32; 4],
    unlit_mode: [f32; 4],
    glass_tint_roughness: [f32; 4],
    glass_ambient: [f32; 4],
    size: [f32; 4],
    properties: [f32; 4],
    /// A digit's 16 segments, each lit from 0 to 1.
    seg_state: [[f32; 4]; 4],
}

/// What the score display's window shows: the DMD's place and look, every
/// digit, and the digits' pipeline, made ready before the drawing's pass.
struct DisplayPlan {
    dmd: Option<crate::scoreview::Placed>,
    digits: Vec<segments::DigitDraw>,
    pipeline: Option<*mut SDL_GPUGraphicsPipeline>,
}

/// The score display's own window (VPX's score view window), or its place inside the table's.
struct DisplayWindow {
    surface: Surface,
    format: SDL_GPUTextureFormat,
    pipeline: *mut SDL_GPUGraphicsPipeline,
    /// The DMD frame, segment displays and window size last shown: it's
    /// drawn again only when one changes.
    shown: Option<(u64, u64, u32, u32)>,
    /// The backglass file's score panel, once shown here.
    panel: Option<backglass::PageDrawing>,
    /// A PinUP pack's screens, once shown here.
    pup: Option<pup::PupDrawing>,
}

/// One see-through thing to draw, in VPX's order.
enum Transparent<'a> {
    Draw(&'a crate::scene::Draw, MeshLook),
    /// VPX's look: a lit bulb's glass glowing with the light reaching the
    /// camera directly, drawn right after the glass (`Light::Render`).
    BulbHull(&'a crate::scene::Draw, usize),
    /// A bulb light's halo, floating its halo height above its surface
    /// (VPX's `UpdateMeshBuffer`), blended in two terms (VPX's look: one).
    Halo(&'a crate::scene::Draw, usize),
    /// VPX's look: a classic light on a surface whose material is marked
    /// see-through, which VPX sorts among the see-through parts
    /// (`Light::Render`: `DrawMesh`'s transparent flag is the surface
    /// material's opacity switch); the others come before them all.
    ClassicLight(&'a crate::scene::Draw, usize),
    Flasher(&'a crate::scene::FlasherLook, crate::look::FlasherNow),
    /// A flasher showing the score display.
    DmdFlasher(&'a crate::scene::FlasherLook, crate::look::FlasherNow),
    /// A still part drawn once already: its mirror's moving reflections
    /// added over it.
    MirrorOnly(&'a crate::scene::Draw, MeshLook, (usize, f32)),
}

impl Transparent<'_> {
    /// The part it comes from, by its place in the table (VPX hands its parts
    /// to the renderer in that order, `GetParts`, which keeps see-through
    /// draws of equal depth in it: a stable sort); after the parts when none.
    fn part(&self) -> u32 {
        match self {
            Transparent::Draw(d, _)
            | Transparent::BulbHull(d, _)
            | Transparent::Halo(d, _)
            | Transparent::ClassicLight(d, _)
            | Transparent::MirrorOnly(d, _, _) => d.part.unwrap_or(u32::MAX),
            Transparent::Flasher(f, _) | Transparent::DmdFlasher(f, _) => f.part,
        }
    }
}

/// How a mesh looks in this frame: its material from the table file, with
/// what a script changed laid over it.
#[derive(Clone, Copy)]
struct MeshLook {
    texture: *mut SDL_GPUTexture,
    textured: bool,
    base_color: [f32; 4],
    metallic: f32,
    roughness: f32,
    /// Texels below this alpha vanish (0: none).
    alpha_cutoff: f32,
    blend: bool,
    disable_lighting: f32,
    /// How far light from below is kept off it.
    disable_lighting_below: f32,
    /// A primitive's colour and opacity (white and opaque for other parts).
    static_color_alpha: [f32; 4],
    /// A primitive set to add its light: drawn unlit and added on top.
    added: bool,
    /// An added primitive that is black or faded out: VPX skips it.
    skip: bool,
    /// Its material's opacity is active, where the part's material is known:
    /// VPX blends nothing else (`Shader::SetMaterial`).
    opacity_active: Option<bool>,
    /// Its material as VPX's look draws it.
    shading: crate::look::Shading,
}

/// A backdrop light, for `backdrop_light_fs` and `backdrop_bulb_fs`.
#[repr(C)]
#[derive(Clone, Copy)]
struct BackdropLightUniforms {
    color_intensity: [f32; 4],
    color2_power: [f32; 4],
    center_range: [f32; 4],
    mode: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct OverlayVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

/// A ball to draw.
#[derive(Clone, Copy, Debug)]
pub struct BallDraw {
    /// Where it's drawn: a ball held in a kicker sunk by its radius, as VPX
    /// draws it (`Ball::Render`'s height).
    pub position: Vec3,
    /// Its velocity, in VPX's units per 10 ms (its physics' time unit), for
    /// the last-moment move.
    pub velocity: Vec3,
    /// Held in a kicker.
    pub locked: bool,
    pub radius: f32,
    /// How it has turned as it rolled (VPX's `HitBall::m_orientation`, its
    /// rows as VPX stores them), which turns its decal or scratches with it.
    pub orientation: [[f32; 3]; 3],
    /// Its number, the physics' (VPX's ball object), for its motion blur.
    pub id: u32,
    /// Its look (VPX's `Ball::m_d`): its colour, 0 to 1; the room it mirrors
    /// and its decal or scratches, as table images (none: VPX's own room, no
    /// decal); and how strongly bulbs and the playfield show in it.
    pub color: [f32; 3],
    pub image: Option<u32>,
    pub decal: Option<u32>,
    pub decal_mode: bool,
    pub bulb_scale: f32,
    pub reflection: f32,
    /// Its picture is a sphere map (else equirectangular).
    pub spherical: bool,
    /// Drawn in the playfield's reflection (VPX's `ReflectionEnabled`), and there even off the
    /// playfield (`ForceReflection`).
    pub reflected: bool,
    pub force_reflection: bool,
}

/// A line of text for the readout, in pixels from the top left.
pub struct Text {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub color: [f32; 4],
    pub text: String,
}

/// Everything one frame shows.
pub struct FrameInput<'a> {
    pub view_proj: Mat4,
    /// The view's rotation in the window, in degrees (VPX's viewport rotation), for the balls'
    /// anti-stretch.
    pub view_rotation: f32,
    /// The camera's view alone (VPX's `matView`): VPX's look turns a ball's
    /// reflection by it for sphere-mapped ball pictures.
    pub view: Mat4,
    /// The projection alone, and the nudge's shake after it (`view_proj` is
    /// the view, then these two).
    pub projection: Mat4,
    pub shake: Mat4,
    pub eye: Vec3,
    /// One transform per scene group (moving part), identity when at rest.
    pub group_transforms: &'a [Mat4],
    pub balls: &'a [BallDraw],
    /// How each of the scene's lights shines now.
    pub lights: &'a [crate::scene::LightNow],
    /// How each part looks now (visibility, a script's move), by part.
    pub part_render: &'a [crate::game::PartRender],
    /// The score reels' digits.
    pub reels: &'a [crate::game::ReelValues],
    /// The text boxes' text and looks.
    pub text_boxes: &'a [crate::textbox::TextBoxNow],
    /// How far a nudge has shaken the picture, in screen units (-1 to 1).
    pub screen_offset: [f32; 2],
    /// `view_proj` without the shake: with the still parts drawn once, the
    /// picture is shaken as a whole at the end, as VPX shakes it.
    pub unshaken: Mat4,
    pub text: &'a [Text],
    /// The text's quarter turns, 0 to 3, with the table on a turned screen (VPX's `LiveUI`
    /// turns its overlay by the view's rotation): laid out as the player sees the screen.
    pub text_turn: u32,
    /// The score display, for text boxes and flashers marked as DMDs.
    pub dmd: Option<&'a crate::dmd::DmdFrame>,
    /// The machine's segment displays, for the score display's window.
    pub segments: Option<&'a crate::segments::SegmentsNow>,
    /// The script hid PinMAME's own window (`Controller.Hidden`): a floating score window hides.
    pub display_hidden: bool,
    /// The backglass, for its window.
    pub backglass: Option<&'a std::sync::Arc<crate::b2s::BackglassNow>>,
    /// What a PinUP pack shows in the windows.
    pub pup: Option<&'a std::sync::Arc<crate::pup::draw::PupNow>>,
}

/// How frames reach the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    /// As soon as each is drawn, tearing allowed: the lowest latency
    /// (immediate, else mailbox).
    Immediate,
    /// Tear-free: the screen changes only between refreshes, and each
    /// refresh shows the newest frame (mailbox, else vsync).
    TearFree,
    /// Tear-free through the display's queue (vsync, first in first out):
    /// frames wait their turn, about three deep.
    Queued,
    /// Tear-free, each frame drawn just before the refresh that shows it
    /// (frame pacing, [`crate::vblank`]), through the display's queue.
    Paced,
}

pub enum FrameResult {
    /// Drawn and handed to the display.
    Submitted { width: u32, height: u32 },
    /// No swapchain image was free: the graphics card is still busy.
    Skipped,
}

pub struct Renderer {
    device: *mut SDL_GPUDevice,
    window: *mut SDL_Window,
    claimed: bool,
    format: SDL_GPUTextureFormat,
    pub present_mode: &'static str,
    opaque: *mut SDL_GPUGraphicsPipeline,
    blended: *mut SDL_GPUGraphicsPipeline,
    /// See-through parts that write depth, as VPX draws them unless they add
    /// their light or a primitive's depth mask is off; in VPX's look, both
    /// with VPX's material.
    blended_depth: *mut SDL_GPUGraphicsPipeline,
    vpx_blended_depth: *mut SDL_GPUGraphicsPipeline,
    glow: *mut SDL_GPUGraphicsPipeline,
    /// A bulb light's halo on the table: its two terms (VPX's bulb_light).
    bulb_modulate_3d: *mut SDL_GPUGraphicsPipeline,
    bulb_add_3d: *mut SDL_GPUGraphicsPipeline,
    overlay: *mut SDL_GPUGraphicsPipeline,
    sprite: *mut SDL_GPUGraphicsPipeline,
    backdrop_light: *mut SDL_GPUGraphicsPipeline,
    bulb_modulate: *mut SDL_GPUGraphicsPipeline,
    bulb_add: *mut SDL_GPUGraphicsPipeline,
    /// Flashers: blended, or additive in two terms (multiplied, then added).
    flasher_blend: *mut SDL_GPUGraphicsPipeline,
    flasher_modulate: *mut SDL_GPUGraphicsPipeline,
    flasher_add: *mut SDL_GPUGraphicsPipeline,
    /// Primitives set to add their light: unlit, added on top (light maps).
    unshaded_add: *mut SDL_GPUGraphicsPipeline,
    /// The score display: in a text box on the backdrop (drawn over what's
    /// below), and on flashers (opaque, blended or added).
    dmd_backdrop: *mut SDL_GPUGraphicsPipeline,
    dmd_opaque: *mut SDL_GPUGraphicsPipeline,
    dmd_blend: *mut SDL_GPUGraphicsPipeline,
    dmd_add: *mut SDL_GPUGraphicsPipeline,
    /// The display's newest frame, its size, and its number.
    dmd_texture: *mut SDL_GPUTexture,
    dmd_transfer: *mut SDL_GPUTransferBuffer,
    dmd_size: (u32, u32),
    dmd_id: Option<u64>,
    /// Frames drawn, for the dots' jitter (VPX's `m_overall_frames`).
    frames_drawn: std::cell::Cell<u32>,
    /// The score display's own window, when it has one.
    display: Option<DisplayWindow>,
    /// The backglass's own window, when it has one.
    backglass: Option<backglass::BackglassWindow>,
    /// The topper's own window, when it has one.
    topper: Option<pup::TopperWindow>,
    /// The backglass, score view and topper drawn inside the table's window (VPX 5436's embedded
    /// output), by `EmbedKind`, and the pipelines that take them in.
    embedded: [Option<embed::Embedded>; 3],
    embed_pipelines: Option<embed::Pipelines>,
    /// The pause menu over the frame, once it first shows (`menu.rs`).
    menu: Option<menu::Menu>,
    /// An achievement's announcement over the backglass window, while it shows (`menu.rs`), and
    /// how many have come and gone, for the window to be drawn again when one does.
    toast: Option<menu::Toast>,
    toast_count: u64,
    /// The window the backglass's drawing went into this frame: its picture, size and format.
    drawn_into: Option<(*mut SDL_GPUTexture, u32, u32, SDL_GPUTextureFormat)>,
    /// The backglass, score and topper windows show once something first draws in them (VPX);
    /// test mode keeps them hidden.
    reveal_windows: bool,
    /// The score window floats over the table (one screen, no score window placed).
    floating_display: bool,
    /// VPX's still parts drawn once (its static prepass), when that applies.
    still: Option<still::Still>,
    /// VPX's post-processed antialiasing and sharpening, as set, and their
    /// pictures and pipelines once made.
    post_aa: PostAa,
    post_sharpen: Sharpen,
    post: Option<post::Post>,
    /// VPX's ambient occlusion and ball motion blur, as set, and their
    /// pictures and pipelines once made.
    ao_setting: AmbientOcclusion,
    motion_blur: bool,
    /// VPX's `ForceBloomOff`: no bloom, whatever the table's strength.
    bloom_off: bool,
    /// The parts' pictures read anisotropically (VPX's `ForceAnisotropicFiltering`, its default).
    anisotropic: bool,
    /// The most a mirror reflects (VPX's `PFReflection`, `RenderProbe::ReflectionMode`: 0 nothing
    /// to 5 everything, its default); each mirror's own mode is held to it.
    max_reflection_mode: u32,
    effects: Option<effects::Effects>,
    still_wanted: bool,
    /// The view being set while the table plays: every part drawn every frame meanwhile
    /// (`set_view_changing`).
    view_changing: bool,
    /// The segment displays' shapes and pipelines, made when first drawn.
    segment_draw: segments::SegmentDraw,
    /// The score view's glass pictures, by their number in the layouts' store, uploaded when first drawn.
    glass_textures: std::cell::RefCell<std::collections::HashMap<u32, *mut SDL_GPUTexture>>,
    /// The score view's layouts for this table (VPX's defaults until told).
    score_layouts: Vec<crate::scoreview::Layout>,
    /// The backglass file's score panel in the score window, and where the DMD
    /// shows in it; None: the machine's displays alone, placed by the layouts.
    score_panel: Option<crate::dmd_overlay::Overlay>,
    /// Where the DMD shows over the backglass (VPX's default: not at all).
    backglass_dmd: crate::dmd_overlay::Overlay,
    /// A PinUP pack's screens before the backglass file's pictures, in the backglass and the
    /// score window (VPX's renderer priorities; unless they're set, the pack first).
    pack_first: [bool; 2],
    /// Table images a script swapped in that weren't decoded ahead, each named once.
    missing_images: std::cell::RefCell<std::collections::HashSet<u32>>,
    /// Each bulb whose halo VPX has placed at its halo height (`Light::UpdateMeshBuffer`, the
    /// first time it's drawn): until then VPX bounds it at its surface.
    halo_raised: std::cell::RefCell<Vec<bool>>,
    /// Each pipeline for parts' meshes, which draws both faces, with its two twins that drop
    /// faces as VPX's cull modes do: `CULL_CCW` (VPX's default) and `CULL_CW`.
    culled: std::collections::HashMap<*mut SDL_GPUGraphicsPipeline, [*mut SDL_GPUGraphicsPipeline; 2]>,
    vertex_buffer: *mut SDL_GPUBuffer,
    index_buffer: *mut SDL_GPUBuffer,
    overlay_buffer: *mut SDL_GPUBuffer,
    overlay_transfer: *mut SDL_GPUTransferBuffer,
    /// The animated primitives' vertices on their way to the vertex buffer,
    /// room for all of them at once; and the frame each shows now (-1: its
    /// mesh as loaded).
    anim_transfer: *mut SDL_GPUTransferBuffer,
    anim_shown: Vec<f32>,
    /// The ramps scripts raise or lower: their vertices on their way to the vertex buffer, made the
    /// first time one changes, and the heights each shows now (None: as loaded).
    ramp_transfer: *mut SDL_GPUTransferBuffer,
    ramp_shown: Vec<Option<[f32; 4]>>,
    textures: Vec<*mut SDL_GPUTexture>,
    white: *mut SDL_GPUTexture,
    font: *mut SDL_GPUTexture,
    sampler: *mut SDL_GPUSampler,
    /// Trilinear, without anisotropy, as VPX reads ramps' and decals' pictures:
    /// repeating, clamped and mirrored (`scene::PartFilter`).
    trilinear: [*mut SDL_GPUSampler; 3],
    font_sampler: *mut SDL_GPUSampler,
    /// For pictures drawn flat over the window (VPX's sprites clamp).
    flat_sampler: *mut SDL_GPUSampler,
    /// For flashers' pictures: clamped, filtered as the table's (VPX's
    /// `tex_flasher_A` and `_B`).
    clamp_sampler: *mut SDL_GPUSampler,
    /// Each text box's picture, and the text it shows (its version).
    text_box_textures: Vec<(*mut SDL_GPUTexture, Option<u32>)>,
    painter: crate::textbox::Painter,
    msaa: *mut SDL_GPUTexture,
    depth: *mut SDL_GPUTexture,
    target_size: (u32, u32),
    /// Samples per pixel: 1, VPX's default, or 4 or 8, edges smoothed (VPX's
    /// MSAA); test mode's pictures go without unless asked, so they repeat
    /// exactly (multisampled blending on this graphics card varies a little
    /// from frame to frame).
    samples: SDL_GPUSampleCount,
    /// VPX's supersampling (`AAFactor`): the table drawn this many times
    /// the window's size across and down, then read down to it (VPX's look).
    aa_factor: f32,
    /// VPX's look: the table drawn into a half-float picture, then
    /// tonemapped; else the first look, straight into the window's colours.
    hdr: bool,
    /// VPX's look on a screen in HDR: VPX 5436's HDR10 output (`hdr10.rs`).
    hdr10: Option<Hdr10>,
    /// The format the table is drawn in: half floats in VPX's look, else the window's.
    scene_format: SDL_GPUTextureFormat,
    /// VPX's look: the table's picture, resolved, for the tonemapping pass.
    hdr_target: *mut SDL_GPUTexture,
    /// VPX's look: the tonemapping pass, the readout drawn over it, and the
    /// single-pass blends of bulb lights (on the table and the backdrop) and
    /// additive flashers.
    tonemap: *mut SDL_GPUGraphicsPipeline,
    overlay_post: *mut SDL_GPUGraphicsPipeline,
    bulb_vpx_3d: *mut SDL_GPUGraphicsPipeline,
    flasher_vpx: *mut SDL_GPUGraphicsPipeline,
    bulb_vpx: *mut SDL_GPUGraphicsPipeline,
    /// The table's tonemapping, and its colour grade as a linear picture.
    tone: crate::scene::Tone,
    color_grade: *mut SDL_GPUTexture,
    /// VPX's look: parts drawn with VPX's basic material, solid and see-through;
    /// the environment picture's sampler (around: repeat; up and down: clamp,
    /// VPX's tex_env), and the environment's irradiance, made once.
    vpx_opaque: *mut SDL_GPUGraphicsPipeline,
    /// VPX's look: a solid part that writes no depth (its depth mask off).
    vpx_opaque_no_depth: *mut SDL_GPUGraphicsPipeline,
    /// VPX's look: a kicker's plate, the hole it sits in (`vs_kicker`).
    vpx_kicker_plate: *mut SDL_GPUGraphicsPipeline,
    vpx_blended: *mut SDL_GPUGraphicsPipeline,
    /// VPX's look: a classic light with a picture of its own, added
    /// (fs_classic_light.sc).
    vpx_classic_light: *mut SDL_GPUGraphicsPipeline,
    env_sampler: *mut SDL_GPUSampler,
    irradiance: *mut SDL_GPUTexture,
    /// The environment picture's height, for VPX's glossy lookups.
    env_height: f32,
    /// The environment picture: the table's picture, or a half-float copy of
    /// it when it holds light past white (then the renderer's own).
    env_texture: *mut SDL_GPUTexture,
    env_owned: bool,
    /// VPX's look: bloom (`Renderer::UpdateBloom`): two pictures a quarter
    /// of the window's size, the cut-off's and the blur's passes.
    bloom: [*mut SDL_GPUTexture; 2],
    bloom_size: (u32, u32),
    bloom_cut: *mut SDL_GPUGraphicsPipeline,
    blur_h: *mut SDL_GPUGraphicsPipeline,
    blur_v: *mut SDL_GPUGraphicsPipeline,
    /// VPX's look: the bulb light buffer, drawn into the bloom pictures before
    /// the table (`Renderer::DrawBulbLightBuffer`): the bulbs' halos, added,
    /// without depth, then VPX's 19-texel blur.
    light_buffer: *mut SDL_GPUGraphicsPipeline,
    blur19_h: *mut SDL_GPUGraphicsPipeline,
    blur19_v: *mut SDL_GPUGraphicsPipeline,
    /// VPX's look: the ball (`fs_ball.sc`): its pipeline, its picture (the
    /// table's, or VPX's default as half floats, then the renderer's own), its
    /// scratches (or VPX's null picture, transparent black), and the previous
    /// frame's picture, which the ball mirrors the playfield from.
    vpx_ball: *mut SDL_GPUGraphicsPipeline,
    ball_env: *mut SDL_GPUTexture,
    ball_env_owned: bool,
    /// VPX's own room picture and its null decal, for balls without theirs.
    vpx_ball_env: *mut SDL_GPUTexture,
    no_decal: *mut SDL_GPUTexture,
    ball_decal: *mut SDL_GPUTexture,
    ball_decal_owned: bool,
    /// The player's own ball pictures over the table's (VPX's `OverwriteBallImage`, `BallImage`,
    /// `DecalImage`), for every ball: null where none.
    ball_override_env: *mut SDL_GPUTexture,
    ball_override_decal: *mut SDL_GPUTexture,
    /// Balls without lighting or reflections (VPX's `DisableLightingForBalls`).
    ball_lighting_off: bool,
    /// Balls squeezed back against the view's stretching (VPX's `BallAntiStretch`).
    ball_anti_stretch: bool,
    prev_hdr: *mut SDL_GPUTexture,
    prev_valid: std::cell::Cell<bool>,
    /// VPX's look: each mirror's picture (VPX's reflection probes), for the
    /// mirrors some part shows, and the pass adding a mirror over a
    /// see-through part.
    mirror_textures: Vec<*mut SDL_GPUTexture>,
    vpx_mirror_add: *mut SDL_GPUGraphicsPipeline,
    /// VPX's look: the normal maps, for each picture in `textures` that is
    /// one, uploaded as plain values (VPX reads them linear).
    normal_textures: Vec<*mut SDL_GPUTexture>,
    /// VPX's look: a rough mirror's blur (any of VPX's kernels), and the
    /// picture it blurs across into.
    blur_kernel_h: *mut SDL_GPUGraphicsPipeline,
    blur_kernel_v: *mut SDL_GPUGraphicsPipeline,
    mirror_blur: *mut SDL_GPUTexture,
    /// VPX's look: each refraction probe's picture and depth (VPX's screen-space
    /// transparency probes), as the table was drawn up to the first part showing it
    /// this frame, for the probes some part shows (the depth only without
    /// multisampling, whose depth can't be read), and the picture they blur across into.
    refraction_textures: Vec<*mut SDL_GPUTexture>,
    refraction_depths: Vec<*mut SDL_GPUTexture>,
    refraction_blur: *mut SDL_GPUTexture,
    /// A part drawn bent: VPX's material over what's behind it, unblended, with
    /// and without writing depth; and the copy of the table so far into a probe.
    vpx_refract: *mut SDL_GPUGraphicsPipeline,
    vpx_refract_depth: *mut SDL_GPUGraphicsPipeline,
    refraction_copy: *mut SDL_GPUGraphicsPipeline,
    scene: SceneDraws,
}

/// The scene's draws, kept by the renderer.
struct SceneDraws {
    draws: Vec<crate::scene::Draw>,
    animations: Vec<crate::scene::PrimitiveAnimation>,
    ramps: Vec<crate::scene::RampShape>,
    materials: Vec<crate::scene::Material>,
    ball: (u32, u32, i32),
    lights: [Vec3; 2],
    light_parts: Vec<crate::scene::LightInfo>,
    reels: Vec<crate::scene::ReelLook>,
    backdrop_lights: Vec<crate::scene::BackdropLightLook>,
    text_boxes: Vec<crate::textbox::TextBoxLook>,
    backdrop: crate::scene::BackdropLook,
    flashers: Vec<crate::scene::FlasherLook>,
    /// For each table image (`vpx.images`), its picture in `textures`.
    table_images: Vec<Option<usize>>,
    /// For each table image, VPX's alpha test (texels at or below it vanish).
    image_alpha_test: Vec<f32>,
    /// For each picture in `textures`, whether some texel isn't opaque.
    image_has_alpha: Vec<bool>,
    /// For each picture in `textures`, its table image's alpha test.
    picture_alpha_test: Vec<f32>,
    mirrors: Vec<crate::scene::Mirror>,
    refractions: Vec<crate::scene::Refraction>,
}

impl Renderer {
    /// `samples` smooths edges with that many samples per pixel (1: none,
    /// VPX's default; 4 or 8, VPX's MSAA levels, or as many as the graphics
    /// card can below that); pictures meant to repeat exactly go without.
    /// `hdr` asks for VPX 5436's HDR10 output in VPX's look, used when the
    /// window's screen has HDR on (None: the ordinary colours always, as
    /// test mode's pictures).
    ///
    /// # Safety
    /// `window` must be a live SDL window, created on this thread, that
    /// outlives the renderer.
    pub unsafe fn new(
        window: *mut SDL_Window,
        scene: &Scene,
        sources: &[vpin::vpx::image::ImageData],
        presentation: Presentation,
        samples: u32,
        look: LookMode,
        hdr: Option<HdrRequest>,
    ) -> Result<Renderer, String> {
        let device = non_null(unsafe { create_device() }, "create GPU device")?;
        let mut r = Renderer {
            device,
            window,
            claimed: false,
            format: SDL_GPU_TEXTUREFORMAT_INVALID,
            present_mode: "vsync",
            opaque: null_mut(),
            blended: null_mut(),
            blended_depth: null_mut(),
            vpx_blended_depth: null_mut(),
            glow: null_mut(),
            bulb_modulate_3d: null_mut(),
            bulb_add_3d: null_mut(),
            overlay: null_mut(),
            sprite: null_mut(),
            backdrop_light: null_mut(),
            bulb_modulate: null_mut(),
            bulb_add: null_mut(),
            flasher_blend: null_mut(),
            flasher_modulate: null_mut(),
            flasher_add: null_mut(),
            unshaded_add: null_mut(),
            dmd_backdrop: null_mut(),
            dmd_opaque: null_mut(),
            dmd_blend: null_mut(),
            dmd_add: null_mut(),
            dmd_texture: null_mut(),
            dmd_transfer: null_mut(),
            dmd_size: (0, 0),
            dmd_id: None,
            frames_drawn: std::cell::Cell::new(0),
            display: None,
            backglass: None,
            topper: None,
            embedded: [None, None, None],
            embed_pipelines: None,
            menu: None,
            toast: None,
            toast_count: 0,
            drawn_into: None,
            floating_display: false,
            reveal_windows: true,
            still: None,
            post_aa: PostAa::Off,
            post_sharpen: Sharpen::Off,
            post: None,
            // VPX's defaults: dynamic occlusion for tables that ask for it, motion blur on
            ao_setting: AmbientOcclusion::Dynamic,
            motion_blur: true,
            bloom_off: false,
            anisotropic: true,
            max_reflection_mode: 5,
            effects: None,
            still_wanted: true,
            view_changing: false,
            segment_draw: segments::SegmentDraw::new(),
            glass_textures: Default::default(),
            score_layouts: crate::scoreview::defaults().to_vec(),
            score_panel: Some(crate::dmd_overlay::Overlay::Auto),
            backglass_dmd: crate::dmd_overlay::Overlay::Off,
            pack_first: [true, true],
            missing_images: Default::default(),
            halo_raised: std::cell::RefCell::new(vec![false; scene.light_parts.len()]),
            culled: std::collections::HashMap::new(),
            vertex_buffer: null_mut(),
            index_buffer: null_mut(),
            overlay_buffer: null_mut(),
            overlay_transfer: null_mut(),
            anim_transfer: null_mut(),
            anim_shown: vec![-1.0; scene.animations.len()],
            ramp_transfer: null_mut(),
            ramp_shown: vec![None; scene.ramps.len()],
            textures: Vec::new(),
            white: null_mut(),
            font: null_mut(),
            sampler: null_mut(),
            trilinear: [null_mut(); 3],
            font_sampler: null_mut(),
            flat_sampler: null_mut(),
            clamp_sampler: null_mut(),
            text_box_textures: vec![(null_mut(), None); scene.text_boxes.len()],
            painter: crate::textbox::Painter::default(),
            msaa: null_mut(),
            depth: null_mut(),
            target_size: (0, 0),
            aa_factor: 1.0,
            samples: match samples {
                8 => SDL_GPU_SAMPLECOUNT_8,
                4 => SDL_GPU_SAMPLECOUNT_4,
                _ => SDL_GPU_SAMPLECOUNT_1,
            },
            hdr: look == LookMode::Vpx,
            hdr10: None,
            scene_format: SDL_GPU_TEXTUREFORMAT_INVALID,
            hdr_target: null_mut(),
            tonemap: null_mut(),
            overlay_post: null_mut(),
            bulb_vpx_3d: null_mut(),
            flasher_vpx: null_mut(),
            bulb_vpx: null_mut(),
            tone: scene.tone,
            color_grade: null_mut(),
            vpx_opaque: null_mut(),
            vpx_opaque_no_depth: null_mut(),
            vpx_kicker_plate: null_mut(),
            vpx_blended: null_mut(),
            vpx_classic_light: null_mut(),
            env_sampler: null_mut(),
            irradiance: null_mut(),
            env_height: 1.0,
            env_texture: null_mut(),
            env_owned: false,
            bloom: [null_mut(); 2],
            bloom_size: (1, 1),
            bloom_cut: null_mut(),
            blur_h: null_mut(),
            blur_v: null_mut(),
            light_buffer: null_mut(),
            blur19_h: null_mut(),
            blur19_v: null_mut(),
            vpx_ball: null_mut(),
            ball_env: null_mut(),
            ball_env_owned: false,
            vpx_ball_env: null_mut(),
            no_decal: null_mut(),
            ball_decal: null_mut(),
            ball_decal_owned: false,
            ball_override_env: null_mut(),
            ball_override_decal: null_mut(),
            ball_lighting_off: false,
            ball_anti_stretch: false,
            prev_hdr: null_mut(),
            prev_valid: std::cell::Cell::new(false),
            mirror_textures: Vec::new(),
            vpx_mirror_add: null_mut(),
            normal_textures: Vec::new(),
            blur_kernel_h: null_mut(),
            blur_kernel_v: null_mut(),
            mirror_blur: null_mut(),
            refraction_textures: Vec::new(),
            refraction_depths: Vec::new(),
            refraction_blur: null_mut(),
            vpx_refract: null_mut(),
            vpx_refract_depth: null_mut(),
            refraction_copy: null_mut(),
            scene: SceneDraws {
                draws: scene.draws.clone(),
                animations: scene.animations.clone(),
                ramps: scene.ramps.clone(),
                materials: scene.materials.clone(),
                ball: scene.ball,
                lights: scene.lights,
                light_parts: scene.light_parts.clone(),
                reels: scene.reels.clone(),
                backdrop_lights: scene.backdrop_lights.clone(),
                text_boxes: scene.text_boxes.clone(),
                backdrop: scene.backdrop,
                flashers: scene.flashers.clone(),
                table_images: scene.table_images.clone(),
                image_alpha_test: scene.image_alpha_test.clone(),
                image_has_alpha: scene.images.iter().map(|i| i.has_alpha).collect(),
                picture_alpha_test: scene.picture_alpha_test.clone(),
                mirrors: scene.mirrors.clone(),
                refractions: scene.refractions.clone(),
            },
        };
        unsafe {
            check(SDL_ClaimWindowForGPUDevice(device, window), "claim the window")?;
            r.claimed = true;
            // the lowest-latency mode the window supports; vsync is always there
            let mut mode = SDL_GPU_PRESENTMODE_VSYNC;
            let candidates: &[(SDL_GPUPresentMode, &'static str)] = match presentation {
                Presentation::Immediate => &[(SDL_GPU_PRESENTMODE_IMMEDIATE, "immediate"), (SDL_GPU_PRESENTMODE_MAILBOX, "mailbox")],
                Presentation::TearFree => &[(SDL_GPU_PRESENTMODE_MAILBOX, "mailbox")],
                Presentation::Queued | Presentation::Paced => &[],
            };
            for &(m, name) in candidates {
                if SDL_WindowSupportsGPUPresentMode(device, window, m) {
                    mode = m;
                    r.present_mode = name;
                    break;
                }
            }
            if presentation == Presentation::Paced {
                r.present_mode = "paced";
            }
            // VPX's look on a screen in HDR: VPX 5436's HDR10 output, its white and peak the
            // screen's as SDL reports them (VPX's Window); else linear output, encoded to sRGB by
            // the swapchain
            let props = SDL_GetWindowProperties(window);
            if r.hdr
                && let Some(request) = hdr
                && SDL_GetBooleanProperty(props, SDL_PROP_WINDOW_HDR_ENABLED_BOOLEAN, false)
                && SDL_WindowSupportsGPUSwapchainComposition(device, window, SDL_GPU_SWAPCHAINCOMPOSITION_HDR10_ST2084)
            {
                r.hdr10 = Some(Hdr10::new(
                    SDL_GetFloatProperty(props, SDL_PROP_WINDOW_SDR_WHITE_LEVEL_FLOAT, 1.0),
                    SDL_GetFloatProperty(props, SDL_PROP_WINDOW_HDR_HEADROOM_FLOAT, 1.0),
                    request,
                ));
            }
            let composition = if r.hdr10.is_some() {
                SDL_GPU_SWAPCHAINCOMPOSITION_HDR10_ST2084
            } else if SDL_WindowSupportsGPUSwapchainComposition(device, window, SDL_GPU_SWAPCHAINCOMPOSITION_SDR_LINEAR) {
                SDL_GPU_SWAPCHAINCOMPOSITION_SDR_LINEAR
            } else {
                SDL_GPU_SWAPCHAINCOMPOSITION_SDR
            };
            check(SDL_SetGPUSwapchainParameters(device, window, composition, mode), "set the present mode")?;
            // one frame queued at most: the lowest latency, VPX's default too (`[Player]`
            // `MaxPrerenderedFrames`, 1 to 3, which the settings may raise). SDL3 sizes the swap
            // chain by the frames in flight, and mailbox needs three buffers (one shown, one
            // waiting, one being drawn); with two it waits for the display like vsync
            let in_flight = FRAMES_IN_FLIGHT.load(std::sync::atomic::Ordering::Relaxed).clamp(1, 3);
            SDL_SetGPUAllowedFramesInFlight(device, if mode == SDL_GPU_PRESENTMODE_MAILBOX { 3 } else { in_flight });
            r.format = SDL_GetGPUSwapchainTextureFormat(device, window);
            r.scene_format = if r.hdr { HDR_FORMAT } else { r.format };
            // as many samples as asked, or the most the graphics card has below that
            while r.samples != SDL_GPU_SAMPLECOUNT_1
                && !(SDL_GPUTextureSupportsSampleCount(r.device, r.scene_format, r.samples) && SDL_GPUTextureSupportsSampleCount(r.device, DEPTH_FORMAT, r.samples))
            {
                let fewer = if r.samples == SDL_GPU_SAMPLECOUNT_8 { SDL_GPU_SAMPLECOUNT_4 } else { SDL_GPU_SAMPLECOUNT_1 };
                eprintln!("vpi-player: the graphics card can't smooth with {} samples; {} instead", samples_of(r.samples), samples_of(fewer));
                r.samples = fewer;
            }
        }
        r.create_pipelines()?;
        r.upload(scene, sources)?;
        // VPX reads the colour grade as linear values (SetTexture's linear RGB), filtered, clamped
        if r.hdr
            && let Some(image) = scene.tone.color_grade.and_then(|i| scene.images.get(i))
        {
            r.color_grade = r.unorm_picture(image.width, image.height, &image.rgba8(), false, "upload the colour grade")?;
        }
        if r.hdr {
            r.prepare_environment(scene)?;
            r.prepare_ball(scene)?;
            // the normal maps, as plain values (VPX's SetTexture with linear on)
            r.normal_textures = vec![null_mut(); scene.images.len()];
            for i in scene.draws.iter().filter_map(|d| d.normal_map) {
                if r.normal_textures[i].is_null() {
                    let image = &scene.images[i];
                    r.normal_textures[i] = r.unorm_picture(image.width, image.height, &image.rgba8(), true, "upload a normal map")?;
                }
            }
        }
        Ok(r)
    }

    /// Gives the score display a window of its own (VPX's score view
    /// window), drawn along with the playfield's frames whenever the display
    /// changes. It never makes a frame wait: when the window has no image
    /// free, a later frame draws it. With `hdr`, on a screen with HDR on it
    /// shows as VPX 5436's does there.
    ///
    /// # Safety
    /// `window` must be a live SDL window, created on this thread, that
    /// outlives the renderer.
    pub unsafe fn add_display_window(&mut self, window: *mut SDL_Window, hdr: bool) -> Result<(), String> {
        unsafe {
            check(SDL_ClaimWindowForGPUDevice(self.device, window), "claim the score display's window")?;
            let mode = [SDL_GPU_PRESENTMODE_MAILBOX, SDL_GPU_PRESENTMODE_IMMEDIATE]
                .into_iter()
                .find(|&m| SDL_WindowSupportsGPUPresentMode(self.device, window, m))
                .unwrap_or(SDL_GPU_PRESENTMODE_VSYNC);
            // on a screen with HDR on, when the settings ask (`[VPI]` `HDRScoreView`): VPX 5436's look there, its graphics library giving a window on that
            // screen the HDR10 format while VPX draws ordinary sRGB values into it, so the screen
            // shows them brighter and more saturated than they are; else linear output, encoded to
            // sRGB by the swapchain, as the playfield's
            let composition = if hdr
                && SDL_GetBooleanProperty(SDL_GetWindowProperties(window), SDL_PROP_WINDOW_HDR_ENABLED_BOOLEAN, false)
                && SDL_WindowSupportsGPUSwapchainComposition(self.device, window, SDL_GPU_SWAPCHAINCOMPOSITION_HDR10_ST2084)
            {
                SDL_GPU_SWAPCHAINCOMPOSITION_HDR10_ST2084
            } else if SDL_WindowSupportsGPUSwapchainComposition(self.device, window, SDL_GPU_SWAPCHAINCOMPOSITION_SDR_LINEAR) {
                SDL_GPU_SWAPCHAINCOMPOSITION_SDR_LINEAR
            } else {
                SDL_GPU_SWAPCHAINCOMPOSITION_SDR
            };
            if let Err(e) = check(SDL_SetGPUSwapchainParameters(self.device, window, composition, mode), "set the score display's present mode") {
                SDL_ReleaseWindowFromGPUDevice(self.device, window);
                return Err(e);
            }
            let format = SDL_GetGPUSwapchainTextureFormat(self.device, window);
            if let Err(e) = self.add_display_surface(Surface::Window(window), format) {
                SDL_ReleaseWindowFromGPUDevice(self.device, window);
                return Err(e);
            }
        }
        Ok(())
    }

    /// The score display drawn on `surface`, pictures of `format`.
    fn add_display_surface(&mut self, surface: Surface, format: SDL_GPUTextureFormat) -> Result<(), String> {
        let pipeline = self.display_pipeline(format)?;
        self.display = Some(DisplayWindow { surface, format, pipeline, shown: None, panel: None, pup: None });
        Ok(())
    }

    /// The display window's pipeline, for pictures of `format`: no vertex
    /// buffer, no depth, no blending (VPX's `DrawMatrixDisplay`).
    fn display_pipeline(&self, format: SDL_GPUTextureFormat) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(DISPLAY_HLSL, c"display_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 1)?;
        let fs = match self.shader(DISPLAY_HLSL, c"display_dmd_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 2, 1) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = format;
            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create the score display's pipeline")
        }
    }

    fn shader(&self, source: &str, entry: &CStr, target: &CStr, stage: SDL_GPUShaderStage, samplers: u32, uniforms: u32) -> Result<*mut SDL_GPUShader, String> {
        let code = fxc::compile(source, entry, target)?;
        unsafe {
            let mut info: SDL_GPUShaderCreateInfo = zeroed();
            info.code_size = code.len();
            info.code = code.as_ptr();
            info.entrypoint = entry.as_ptr();
            info.format = SDL_GPU_SHADERFORMAT_DXBC;
            info.stage = stage;
            info.num_samplers = samplers;
            info.num_uniform_buffers = uniforms;
            non_null(SDL_CreateGPUShader(self.device, &info), "create shader")
        }
    }

    /// A pipeline for parts' meshes drawing both faces, and its two twins dropping the faces VPX's
    /// cull modes drop (`CULL_CCW`, VPX's default, and `CULL_CW`), kept in `culled`: our meshes keep
    /// VPX's winding, so VPX's counter-clockwise faces are SDL's front ones
    unsafe fn part_pipeline(&mut self, info: &SDL_GPUGraphicsPipelineCreateInfo) -> *mut SDL_GPUGraphicsPipeline {
        unsafe {
            let both = SDL_CreateGPUGraphicsPipeline(self.device, info);
            if !both.is_null() {
                let mut twin = *info;
                twin.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_FRONT;
                let ccw = SDL_CreateGPUGraphicsPipeline(self.device, &twin);
                twin.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_BACK;
                let cw = SDL_CreateGPUGraphicsPipeline(self.device, &twin);
                if !ccw.is_null() && !cw.is_null() {
                    self.culled.insert(both, [ccw, cw]);
                }
            }
            both
        }
    }

    fn create_pipelines(&mut self) -> Result<(), String> {
        let vs = self.shader(SCENE_HLSL, c"vs_main", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 2)?;
        let fs = self.shader(SCENE_HLSL, c"fs_main", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)?;
        let light_fs = self.shader(SCENE_HLSL, c"fs_light", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)?;
        // the bulbs, flashers and added parts read VPX's ball shadows from the third slot
        let bulb_fs = self.shader(SCENE_HLSL, c"fs_bulb", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 3)?;
        let ovs = self.shader(OVERLAY_HLSL, c"overlay_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 1)?;
        let ofs = self.shader(OVERLAY_HLSL, c"overlay_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 0)?;
        // over VPX's HDR10 output, the readout encoded as the picture is
        let pq_fs = match self.hdr10 {
            Some(h) => {
                let source = format!("#define OVERLAY_PQ_WHITE {:.9}\n{OVERLAY_HLSL}", h.sdr_white / 10000.0);
                self.shader(&source, c"overlay_pq_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 0)?
            }
            None => null_mut(),
        };
        let sfs = self.shader(OVERLAY_HLSL, c"sprite_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 0)?;
        let blfs = self.shader(OVERLAY_HLSL, c"backdrop_light_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)?;
        let bbfs = self.shader(OVERLAY_HLSL, c"backdrop_bulb_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)?;
        let flasher_fs = self.shader(SCENE_HLSL, c"fs_flasher", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 2, 3)?;
        let unshaded_fs = self.shader(SCENE_HLSL, c"fs_unshaded", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 3)?;
        let dmd_flasher_fs = self.shader(&format!("#define DMD_SCENE\n{SCENE_HLSL}\n{DMD_HLSL}"), c"fs_dmd_flasher", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)?;
        let dmd_backdrop_fs = self.shader(&format!("#define DMD_OVERLAY\n{OVERLAY_HLSL}\n{DMD_HLSL}"), c"dmd_backdrop_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 1)?;
        unsafe {
            let mut buffer: SDL_GPUVertexBufferDescription = zeroed();
            buffer.slot = 0;
            buffer.pitch = size_of::<Vertex>() as u32;
            buffer.input_rate = SDL_GPU_VERTEXINPUTRATE_VERTEX;
            let mut attributes: [SDL_GPUVertexAttribute; 3] = zeroed();
            for (n, (format, offset)) in [
                (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT3, 0),
                (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT3, 12),
                (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT2, 24),
            ]
            .into_iter()
            .enumerate()
            {
                attributes[n].location = n as u32;
                attributes[n].format = format;
                attributes[n].offset = offset;
            }
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = self.scene_format;
            let color = color;
            // VPX's look marks the table's solid parts in the picture's alpha, for tonemapping
            // (VPX: where its depth buffer was written); everything else keeps the alpha below
            let hdr = self.hdr;
            let keep_alpha = |mut d: SDL_GPUColorTargetDescription| {
                if hdr {
                    if !d.blend_state.enable_blend {
                        d.blend_state.enable_blend = true;
                        d.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                        d.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
                        d.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
                    }
                    d.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
                    d.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                    d.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
                }
                d
            };
            let flat = keep_alpha(color);
            // VPX's bulb lights and additive flashers: dst (1 - src) - src alpha, with negative
            // colours (source alpha, one minus source colour, reverse subtract)
            let mut vpx_light = color;
            vpx_light.blend_state.enable_blend = true;
            vpx_light.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_SRC_ALPHA;
            vpx_light.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE_MINUS_SRC_COLOR;
            vpx_light.blend_state.color_blend_op = SDL_GPU_BLENDOP_REVERSE_SUBTRACT;
            vpx_light.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
            vpx_light.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            vpx_light.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;

            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.vertex_input_state.vertex_buffer_descriptions = &buffer;
            info.vertex_input_state.num_vertex_buffers = 1;
            info.vertex_input_state.vertex_attributes = attributes.as_ptr();
            info.vertex_input_state.num_vertex_attributes = 3;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            // what lies past the near and far planes is cut off, as Direct3D does by default and
            // VPX's renderer with it; SDL's zero clamps instead, drawing such parts at the far
            // plane's depth (Star Gazer's floor, far below the table, showed to the back)
            info.rasterizer_state.enable_depth_clip = true;
            info.multisample_state.sample_count = self.samples;
            info.depth_stencil_state.compare_op = SDL_GPU_COMPAREOP_LESS_OR_EQUAL;
            info.depth_stencil_state.enable_depth_test = true;
            info.depth_stencil_state.enable_depth_write = true;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            info.target_info.depth_stencil_format = DEPTH_FORMAT;
            info.target_info.has_depth_stencil_target = true;
            self.opaque = self.part_pipeline(&info);

            let mut blend = color;
            blend.blend_state.enable_blend = true;
            blend.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_SRC_ALPHA;
            blend.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE_MINUS_SRC_ALPHA;
            blend.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
            blend.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            blend.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE_MINUS_SRC_ALPHA;
            blend.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            // the readout over VPX's tonemapped picture blends as it always has
            let readout_blend = SDL_GPUColorTargetDescription { format: self.format, ..blend };
            let blend = keep_alpha(blend);
            info.target_info.color_target_descriptions = &blend;
            info.depth_stencil_state.enable_depth_write = false;
            self.blended = self.part_pipeline(&info);
            // VPX keeps writing depth for see-through parts (its render state's default)
            let mut depth = info;
            depth.depth_stencil_state.enable_depth_write = true;
            self.blended_depth = self.part_pipeline(&depth);
            if hdr {
                // VPX's look: the same, with VPX's basic material
                let vpx_fs = self.shader(MATERIAL_HLSL, c"fs_vpx", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 6, 3)?;
                let mut vpx = info;
                vpx.fragment_shader = vpx_fs;
                self.vpx_blended = self.part_pipeline(&vpx);
                // a see-through part writing depth marks its pixels for tonemapping too, as VPX
                // tonemaps wherever its depth buffer was written: the alpha keeps its highest value,
                // and any above 0 counts
                let mut marking = blend;
                marking.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                marking.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                marking.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_MAX;
                vpx.target_info.color_target_descriptions = &marking;
                vpx.depth_stencil_state.enable_depth_write = true;
                self.vpx_blended_depth = self.part_pipeline(&vpx);
                vpx.target_info.color_target_descriptions = &color;
                self.vpx_opaque = self.part_pipeline(&vpx);
                // a solid part with its depth mask off (Primitive::Render's ZWRITEENABLE): what's
                // drawn behind it later still shows over it, as in VPX; its pixels left marked for
                // tonemapping as the parts below marked them (VPX tonemaps where depth was written)
                let mut unmarked = color;
                unmarked.blend_state.enable_color_write_mask = true;
                unmarked.blend_state.color_write_mask = SDL_GPU_COLORCOMPONENT_R | SDL_GPU_COLORCOMPONENT_G | SDL_GPU_COLORCOMPONENT_B;
                let mut no_depth = vpx;
                no_depth.target_info.color_target_descriptions = &unmarked;
                no_depth.depth_stencil_state.enable_depth_write = false;
                self.vpx_opaque_no_depth = self.part_pipeline(&no_depth);
                // a kicker's plate: VPX's material over whatever is there, at the depth of a point 30
                // below (Kicker::Render's Z_ALWAYS, vs_basic.sc's KICKER)
                let kicker_vs = self.shader(SCENE_HLSL, c"vs_kicker", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 2)?;
                let mut plate = vpx;
                plate.vertex_shader = kicker_vs;
                plate.depth_stencil_state.compare_op = SDL_GPU_COMPAREOP_ALWAYS;
                self.vpx_kicker_plate = self.part_pipeline(&plate);
                SDL_ReleaseGPUShader(self.device, kicker_vs);
                SDL_ReleaseGPUShader(self.device, vpx_fs);
                // a part drawn bent (VPX's refraction): unblended, what's behind it mixed in by
                // its own opacity, writing depth or not as the part says
                let refract_fs = self.shader(&format!("#define REFRACTION\n{MATERIAL_HLSL}"), c"fs_vpx", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 8, 4)?;
                let mut refract = vpx;
                refract.fragment_shader = refract_fs;
                self.vpx_refract_depth = self.part_pipeline(&refract);
                refract.depth_stencil_state.enable_depth_write = false;
                self.vpx_refract = self.part_pipeline(&refract);
                SDL_ReleaseGPUShader(self.device, refract_fs);
                // a see-through part's mirror, added over it, no depth written (VPX's
                // reflection-only pass: source alpha, one)
                let reflection_fs = self.shader(MATERIAL_HLSL, c"fs_vpx_reflection", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 6, 2)?;
                let mut over = color;
                over.blend_state.enable_blend = true;
                over.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_SRC_ALPHA;
                over.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                over.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
                over.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
                over.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
                over.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
                let mut mirror_add = vpx;
                mirror_add.fragment_shader = reflection_fs;
                mirror_add.target_info.color_target_descriptions = &over;
                mirror_add.depth_stencil_state.enable_depth_write = false;
                self.vpx_mirror_add = self.part_pipeline(&mirror_add);
                SDL_ReleaseGPUShader(self.device, reflection_fs);
                // the ball: solid, VPX's ball shader
                let ball_fs = self.shader(&format!("#define BALL\n{MATERIAL_HLSL}"), c"fs_vpx_ball", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 5, 2)?;
                vpx.fragment_shader = ball_fs;
                self.vpx_ball = SDL_CreateGPUGraphicsPipeline(self.device, &vpx);
                SDL_ReleaseGPUShader(self.device, ball_fs);
            }

            let mut add = color;
            add.blend_state.enable_blend = true;
            add.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            add.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            add.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
            add.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
            add.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            add.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            info.target_info.color_target_descriptions = &add;
            info.fragment_shader = light_fs;
            self.glow = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            if hdr {
                // VPX's look: a classic light with a picture of its own, added the same way, its
                // picture lit by VPX's material (fs_classic_light.sc)
                let classic_fs =
                    self.shader(&format!("#define CLASSIC_LIGHT
{MATERIAL_HLSL}"), c"fs_vpx_classic_light", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 3, 4)?;
                let mut classic = info;
                classic.fragment_shader = classic_fs;
                self.vpx_classic_light = SDL_CreateGPUGraphicsPipeline(self.device, &classic);
                SDL_ReleaseGPUShader(self.device, classic_fs);
            }
            // bulb halos: what's below times (1 + m L), then plus (1 - m) L
            info.fragment_shader = bulb_fs;
            let mut multiply = color;
            multiply.blend_state.enable_blend = true;
            multiply.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_DST_COLOR;
            multiply.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            multiply.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
            multiply.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
            multiply.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            multiply.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            info.target_info.color_target_descriptions = &multiply;
            self.bulb_modulate_3d = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &add;
            self.bulb_add_3d = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            if hdr {
                info.target_info.color_target_descriptions = &vpx_light;
                self.bulb_vpx_3d = SDL_CreateGPUGraphicsPipeline(self.device, &info);
                // the bulb light buffer: the same, into a small picture with no depth
                let mut light_buffer = info;
                let mut target = vpx_light;
                target.format = HDR_FORMAT;
                light_buffer.target_info.color_target_descriptions = &target;
                light_buffer.target_info.has_depth_stencil_target = false;
                light_buffer.target_info.depth_stencil_format = SDL_GPU_TEXTUREFORMAT_INVALID;
                light_buffer.depth_stencil_state.enable_depth_test = false;
                light_buffer.depth_stencil_state.enable_depth_write = false;
                light_buffer.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
                self.light_buffer = SDL_CreateGPUGraphicsPipeline(self.device, &light_buffer);
            }
            // flashers: no depth written, both faces (VPX's Flasher::Render)
            info.fragment_shader = flasher_fs;
            info.target_info.color_target_descriptions = &blend;
            self.flasher_blend = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &multiply;
            self.flasher_modulate = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &add;
            self.flasher_add = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            if hdr {
                info.target_info.color_target_descriptions = &vpx_light;
                self.flasher_vpx = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            }
            // primitives set to add their light: unlit, no depth written (VPX's
            // Primitive::Render with m_addBlend: source alpha, one)
            let mut added = color;
            added.blend_state.enable_blend = true;
            added.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_SRC_ALPHA;
            added.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            added.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
            added.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
            added.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            added.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            info.fragment_shader = unshaded_fs;
            info.target_info.color_target_descriptions = &added;
            self.unshaded_add = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            // flashers showing the score display: no depth written; drawn over what's below,
            // blended, or added (VPX's Flasher::Render, DMD mode: EnableAlphaBlend)
            info.fragment_shader = dmd_flasher_fs;
            info.target_info.color_target_descriptions = &flat;
            self.dmd_opaque = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &blend;
            self.dmd_blend = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &added;
            self.dmd_add = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &blend;

            let mut obuffer: SDL_GPUVertexBufferDescription = zeroed();
            obuffer.pitch = size_of::<OverlayVertex>() as u32;
            obuffer.input_rate = SDL_GPU_VERTEXINPUTRATE_VERTEX;
            let mut oattributes: [SDL_GPUVertexAttribute; 3] = zeroed();
            for (n, (format, offset)) in [
                (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT2, 0),
                (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT2, 8),
                (SDL_GPU_VERTEXELEMENTFORMAT_FLOAT4, 16),
            ]
            .into_iter()
            .enumerate()
            {
                oattributes[n].location = n as u32;
                oattributes[n].format = format;
                oattributes[n].offset = offset;
            }
            info.vertex_shader = ovs;
            info.fragment_shader = ofs;
            info.vertex_input_state.vertex_buffer_descriptions = &obuffer;
            info.vertex_input_state.vertex_attributes = oattributes.as_ptr();
            info.depth_stencil_state.enable_depth_test = false;
            info.depth_stencil_state.enable_depth_write = false;
            self.overlay = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            if hdr {
                // the readout, drawn over the tonemapped picture in the window's own colours
                let mut post = info;
                if !pq_fs.is_null() {
                    post.fragment_shader = pq_fs;
                }
                post.target_info.color_target_descriptions = &readout_blend;
                post.target_info.has_depth_stencil_target = false;
                post.target_info.depth_stencil_format = SDL_GPU_TEXTUREFORMAT_INVALID;
                post.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
                self.overlay_post = SDL_CreateGPUGraphicsPipeline(self.device, &post);
            }
            // pictures over the window replace what is below (no blending)
            info.fragment_shader = sfs;
            info.target_info.color_target_descriptions = &flat;
            self.sprite = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            // a text box showing the score display, likewise (VPX: no blending)
            info.fragment_shader = dmd_backdrop_fs;
            self.dmd_backdrop = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            // classic lights on the backdrop, drawn opaque (VPX: no blending)
            info.fragment_shader = blfs;
            self.backdrop_light = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            // bulb lights on the backdrop: what's below times (1 + m L), then plus (1 - m) L
            info.fragment_shader = bbfs;
            let mut modulate = color;
            modulate.blend_state.enable_blend = true;
            modulate.blend_state.src_color_blendfactor = SDL_GPU_BLENDFACTOR_DST_COLOR;
            modulate.blend_state.dst_color_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            modulate.blend_state.color_blend_op = SDL_GPU_BLENDOP_ADD;
            modulate.blend_state.src_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ZERO;
            modulate.blend_state.dst_alpha_blendfactor = SDL_GPU_BLENDFACTOR_ONE;
            modulate.blend_state.alpha_blend_op = SDL_GPU_BLENDOP_ADD;
            info.target_info.color_target_descriptions = &modulate;
            self.bulb_modulate = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            info.target_info.color_target_descriptions = &add;
            self.bulb_add = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            if hdr {
                info.target_info.color_target_descriptions = &vpx_light;
                self.bulb_vpx = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            }

            for s in [vs, fs, light_fs, bulb_fs, ovs, ofs, sfs, blfs, bbfs, flasher_fs, unshaded_fs, dmd_flasher_fs, dmd_backdrop_fs] {
                SDL_ReleaseGPUShader(self.device, s);
            }
            if !pq_fs.is_null() {
                SDL_ReleaseGPUShader(self.device, pq_fs);
            }
        }
        non_null(self.opaque, "create the scene pipeline")?;
        non_null(self.flasher_blend, "create the flasher pipeline")?;
        non_null(self.flasher_modulate, "create the flasher pipeline")?;
        non_null(self.flasher_add, "create the flasher pipeline")?;
        non_null(self.unshaded_add, "create the added primitive pipeline")?;
        non_null(self.dmd_opaque, "create the DMD pipeline")?;
        non_null(self.dmd_blend, "create the DMD pipeline")?;
        non_null(self.dmd_add, "create the DMD pipeline")?;
        non_null(self.dmd_backdrop, "create the backdrop DMD pipeline")?;
        non_null(self.blended, "create the transparent pipeline")?;
        non_null(self.glow, "create the light pipeline")?;
        non_null(self.bulb_modulate_3d, "create the bulb pipeline")?;
        non_null(self.bulb_add_3d, "create the bulb pipeline")?;
        non_null(self.overlay, "create the overlay pipeline")?;
        non_null(self.backdrop_light, "create the backdrop light pipeline")?;
        non_null(self.bulb_modulate, "create the backdrop bulb pipeline")?;
        non_null(self.bulb_add, "create the backdrop bulb pipeline")?;
        non_null(self.blended_depth, "create the transparent pipeline")?;
        if self.hdr {
            non_null(self.vpx_opaque, "create VPX's material pipeline")?;
            non_null(self.vpx_opaque_no_depth, "create VPX's material pipeline")?;
            non_null(self.vpx_kicker_plate, "create VPX's kicker plate pipeline")?;
            non_null(self.vpx_blended, "create VPX's material pipeline")?;
            non_null(self.vpx_blended_depth, "create VPX's material pipeline")?;
            non_null(self.vpx_classic_light, "create VPX's classic light pipeline")?;
            non_null(self.vpx_refract, "create VPX's refraction pipeline")?;
            non_null(self.vpx_refract_depth, "create VPX's refraction pipeline")?;
            non_null(self.vpx_mirror_add, "create VPX's mirror pipeline")?;
            non_null(self.vpx_ball, "create VPX's ball pipeline")?;
            non_null(self.bulb_vpx_3d, "create VPX's bulb pipeline")?;
            non_null(self.flasher_vpx, "create VPX's flasher pipeline")?;
            non_null(self.bulb_vpx, "create VPX's backdrop bulb pipeline")?;
            non_null(self.overlay_post, "create the readout's pipeline")?;
            self.tonemap = self.tonemap_pipeline()?;
            self.bloom_cut = self.bloom_pipeline(c"bloom_cut_fs")?;
            self.blur_h = self.bloom_pipeline(c"blur_h_fs")?;
            self.blur_v = self.bloom_pipeline(c"blur_v_fs")?;
            self.blur19_h = self.bloom_pipeline(c"blur19_h_fs")?;
            self.blur19_v = self.bloom_pipeline(c"blur19_v_fs")?;
            self.blur_kernel_h = self.blur_pipeline(c"blur_kernel_h_fs", 2)?;
            self.blur_kernel_v = self.blur_pipeline(c"blur_kernel_v_fs", 2)?;
            non_null(self.light_buffer, "create the bulb light buffer's pipeline")?;
        }
        non_null(self.sprite, "create the picture pipeline").map(|_| ())
    }

    /// VPX's tonemapping pass into the window: one triangle over it, no
    /// vertex buffer, no depth, no blending.
    fn tonemap_pipeline(&self) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        self.tonemap_pipeline_for(self.format)
    }

    /// The tonemapping pass into a picture of `format`.
    fn tonemap_pipeline_for(&self, format: SDL_GPUTextureFormat) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(TONEMAP_HLSL, c"tonemap_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 0)?;
        let fs = match self.shader(TONEMAP_HLSL, c"tonemap_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 4, 1) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = format;
            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create the tonemapping pipeline")
        }
    }

    /// VPX's look: the ball's pictures (`Ball::RenderSetup`): the table's ball
    /// picture, else VPX's default environment, kept as half floats; the
    /// table's scratches picture, else VPX's null picture (transparent black).
    /// VPX's own two are kept as well, for balls a script gives none.
    fn prepare_ball(&mut self, scene: &Scene) -> Result<(), String> {
        let decoded = image::load_from_memory_with_format(BALL_ENV, image::ImageFormat::OpenExr).map_err(|e| format!("VPX's ball picture: {e}"))?;
        let f = decoded.to_rgba32f();
        let bytes: Vec<u8> = f.as_raw().iter().flat_map(|&v| half::f16::from_f32(v.clamp(0.0, 65504.0)).to_bits().to_le_bytes()).collect();
        self.vpx_ball_env = self.picture_of(SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT, f.width(), f.height(), &bytes, true, "upload VPX's ball picture")?;
        self.no_decal = self.unorm_picture(1, 1, &[0, 0, 0, 0], false, "upload VPX's null picture")?;
        match self.tone.ball_image.filter(|&i| i < self.textures.len()) {
            Some(_) if scene.ball_hdr.is_some() => {
                let hdr = scene.ball_hdr.as_ref().expect("checked");
                let bytes: Vec<u8> = hdr.rgba.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect();
                self.ball_env = self.picture_of(SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT, hdr.width, hdr.height, &bytes, true, "upload the ball picture")?;
                self.ball_env_owned = true;
            }
            Some(i) => self.ball_env = self.textures[i],
            None => self.ball_env = self.vpx_ball_env,
        }
        self.ball_decal = match self.tone.ball_decal.filter(|&i| i < self.textures.len()) {
            Some(i) => self.textures[i],
            None => self.no_decal,
        };
        Ok(())
    }

    /// One of bloom's passes into a quarter-size half-float picture: one
    /// triangle over it, no vertex buffer, no depth, no blending.
    fn bloom_pipeline(&self, entry: &CStr) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        self.blur_pipeline(entry, 1)
    }

    /// A full-picture pass of `bloom.hlsl` into a half-float picture, its
    /// fragment stage with that many uniform buffers.
    fn blur_pipeline(&self, entry: &CStr, uniforms: u32) -> Result<*mut SDL_GPUGraphicsPipeline, String> {
        let vs = self.shader(BLOOM_HLSL, c"bloom_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 0)?;
        let fs = match self.shader(BLOOM_HLSL, entry, c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, uniforms) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = HDR_FORMAT;
            let mut info: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            info.vertex_shader = vs;
            info.fragment_shader = fs;
            info.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            info.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            info.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            info.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            info.target_info.color_target_descriptions = &color;
            info.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &info);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            non_null(pipeline, "create a bloom pipeline")
        }
    }

    /// VPX's look: the environment picture's sampler, and its irradiance,
    /// made once on the graphics card as VPX makes it (`Renderer`'s
    /// constructor, the `irradiance` pass): a picture 1/8 of the
    /// environment's height (at most 256) and twice as wide.
    fn prepare_environment(&mut self, scene: &Scene) -> Result<(), String> {
        let Some(env) = self.tone.env.filter(|&e| e < self.textures.len()) else {
            return Err("VPX's look: no environment picture".into());
        };
        let mut info: SDL_GPUSamplerCreateInfo = unsafe { zeroed() };
        info.min_filter = SDL_GPU_FILTER_LINEAR;
        info.mag_filter = SDL_GPU_FILTER_LINEAR;
        info.mipmap_mode = SDL_GPU_SAMPLERMIPMAPMODE_LINEAR;
        info.address_mode_u = SDL_GPU_SAMPLERADDRESSMODE_REPEAT;
        info.address_mode_v = SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE;
        info.address_mode_w = SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE;
        info.max_lod = 1000.0;
        self.env_sampler = non_null(unsafe { SDL_CreateGPUSampler(self.device, &info) }, "create the environment's sampler")?;

        // VPX keeps a high-range picture's range (its RGB_FP16 and FP32 textures)
        let env_height = if let Some(hdr) = &scene.env_hdr {
            let bytes: Vec<u8> = hdr.rgba.iter().flat_map(|v| v.to_bits().to_le_bytes()).collect();
            self.env_texture = self.picture_of(SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT, hdr.width, hdr.height, &bytes, true, "upload the environment picture")?;
            self.env_owned = true;
            hdr.height
        } else {
            self.env_texture = self.textures[env];
            scene.images[env].height
        };
        self.env_height = env_height as f32;
        let height = (env_height.min(256) / 8).max(1);
        let width = height * 2;
        self.irradiance = self.texture(IRRADIANCE_FORMAT, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
        let source = format!("#define IRRADIANCE\n{MATERIAL_HLSL}");
        let vs = self.shader(&source, c"irradiance_vs", c"vs_5_1", SDL_GPU_SHADERSTAGE_VERTEX, 0, 0)?;
        let fs = match self.shader(&source, c"irradiance_fs", c"ps_5_1", SDL_GPU_SHADERSTAGE_FRAGMENT, 1, 0) {
            Ok(fs) => fs,
            Err(e) => {
                unsafe { SDL_ReleaseGPUShader(self.device, vs) };
                return Err(e);
            }
        };
        unsafe {
            let mut color: SDL_GPUColorTargetDescription = zeroed();
            color.format = IRRADIANCE_FORMAT;
            let mut p: SDL_GPUGraphicsPipelineCreateInfo = zeroed();
            p.vertex_shader = vs;
            p.fragment_shader = fs;
            p.primitive_type = SDL_GPU_PRIMITIVETYPE_TRIANGLELIST;
            p.rasterizer_state.fill_mode = SDL_GPU_FILLMODE_FILL;
            p.rasterizer_state.cull_mode = SDL_GPU_CULLMODE_NONE;
            p.multisample_state.sample_count = SDL_GPU_SAMPLECOUNT_1;
            p.target_info.color_target_descriptions = &color;
            p.target_info.num_color_targets = 1;
            let pipeline = SDL_CreateGPUGraphicsPipeline(self.device, &p);
            SDL_ReleaseGPUShader(self.device, vs);
            SDL_ReleaseGPUShader(self.device, fs);
            let pipeline = non_null(pipeline, "create the irradiance pipeline")?;
            let command = SDL_AcquireGPUCommandBuffer(self.device);
            if command.is_null() {
                SDL_ReleaseGPUGraphicsPipeline(self.device, pipeline);
                return Err(format!("acquire command buffer: {}", sdl_error()));
            }
            let mut target: SDL_GPUColorTargetInfo = zeroed();
            target.texture = self.irradiance;
            target.load_op = SDL_GPU_LOADOP_DONT_CARE;
            target.store_op = SDL_GPU_STOREOP_STORE;
            let pass = SDL_BeginGPURenderPass(command, &target, 1, null());
            SDL_BindGPUGraphicsPipeline(pass, pipeline);
            let binding = SDL_GPUTextureSamplerBinding { texture: self.env_texture, sampler: self.env_sampler };
            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
            SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
            let submitted = SDL_SubmitGPUCommandBuffer(command);
            SDL_ReleaseGPUGraphicsPipeline(self.device, pipeline);
            check(submitted, "compute the environment's irradiance")
        }
    }

    /// A picture uploaded with its values as they are (no sRGB decoding),
    /// with its mipmaps when asked.
    pub(super) fn unorm_picture(&self, w: u32, h: u32, rgba: &[u8], mipmaps: bool, what: &str) -> Result<*mut SDL_GPUTexture, String> {
        self.picture_of(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM, w, h, rgba, mipmaps, what)
    }

    /// A picture of `format` uploaded from its bytes, with its mipmaps when
    /// asked: VPX's (`mips`) for the formats VPX makes them for, else the
    /// graphics card's.
    fn picture_of(&self, format: SDL_GPUTextureFormat, w: u32, h: u32, rgba: &[u8], mipmaps: bool, what: &str) -> Result<*mut SDL_GPUTexture, String> {
        let chain = match mips_texels(format) {
            Some(texels) if mipmaps => mips::chain(texels, rgba, w, h),
            _ => Vec::new(),
        };
        self.picture_with(format, w, h, rgba, mipmaps, &chain, what)
    }

    /// A picture of `format` uploaded from its bytes with the levels after
    /// the first, `chain` (none: the graphics card makes them, when asked).
    fn picture_with(&self, format: SDL_GPUTextureFormat, w: u32, h: u32, rgba: &[u8], mipmaps: bool, chain: &[mips::Level], what: &str) -> Result<*mut SDL_GPUTexture, String> {
        let (levels, usage) = if mipmaps {
            (32 - w.max(h).leading_zeros(), SDL_GPU_TEXTUREUSAGE_SAMPLER | SDL_GPU_TEXTUREUSAGE_COLOR_TARGET)
        } else {
            (1, SDL_GPU_TEXTUREUSAGE_SAMPLER)
        };
        let texture = self.texture(format, usage, w, h, levels, SDL_GPU_SAMPLECOUNT_1)?;
        let texel_bytes = unsafe { SDL_GPUTextureFormatTexelBlockSize(format) } as usize;
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
            info.size = chain_span(w, h, texel_bytes, chain) as u32;
            let upload = SDL_CreateGPUTransferBuffer(self.device, &info);
            let mapped = if upload.is_null() { null_mut() } else { SDL_MapGPUTransferBuffer(self.device, upload, false) as *mut u8 };
            if mapped.is_null() {
                if !upload.is_null() {
                    SDL_ReleaseGPUTransferBuffer(self.device, upload);
                }
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(format!("{what}: {}", sdl_error()));
            }
            put_chain(mapped, 0, rgba, w, h, texel_bytes, chain);
            SDL_UnmapGPUTransferBuffer(self.device, upload);
            let command = SDL_AcquireGPUCommandBuffer(self.device);
            if command.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, upload);
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(format!("acquire command buffer: {}", sdl_error()));
            }
            let copy = SDL_BeginGPUCopyPass(command);
            upload_chain(copy, upload, texture, 0, w, h, texel_bytes, chain);
            SDL_EndGPUCopyPass(copy);
            if mipmaps && chain.is_empty() {
                SDL_GenerateMipmapsForGPUTexture(command, texture);
            }
            let submitted = SDL_SubmitGPUCommandBuffer(command);
            SDL_ReleaseGPUTransferBuffer(self.device, upload);
            if let Err(e) = check(submitted, what) {
                SDL_ReleaseGPUTexture(self.device, texture);
                return Err(e);
            }
        }
        Ok(texture)
    }

    fn buffer(&self, usage: SDL_GPUBufferUsageFlags, size: usize) -> Result<*mut SDL_GPUBuffer, String> {
        unsafe {
            let mut info: SDL_GPUBufferCreateInfo = zeroed();
            info.usage = usage;
            info.size = size as u32;
            non_null(SDL_CreateGPUBuffer(self.device, &info), "create buffer")
        }
    }

    fn texture(&self, format: SDL_GPUTextureFormat, usage: SDL_GPUTextureUsageFlags, width: u32, height: u32, levels: u32, samples: SDL_GPUSampleCount) -> Result<*mut SDL_GPUTexture, String> {
        unsafe {
            let mut info: SDL_GPUTextureCreateInfo = zeroed();
            info.r#type = SDL_GPU_TEXTURETYPE_2D;
            info.format = format;
            info.usage = usage;
            info.width = width;
            info.height = height;
            info.layer_count_or_depth = 1;
            info.num_levels = levels;
            info.sample_count = samples;
            non_null(SDL_CreateGPUTexture(self.device, &info), "create texture")
        }
    }

    fn sampler(&self, filter: SDL_GPUFilter, address: SDL_GPUSamplerAddressMode, anisotropy: bool) -> Result<*mut SDL_GPUSampler, String> {
        unsafe {
            let mut info: SDL_GPUSamplerCreateInfo = zeroed();
            info.min_filter = filter;
            info.mag_filter = filter;
            info.mipmap_mode = SDL_GPU_SAMPLERMIPMAPMODE_LINEAR;
            info.address_mode_u = address;
            info.address_mode_v = address;
            info.address_mode_w = address;
            info.max_lod = 1000.0;
            info.enable_anisotropy = anisotropy;
            info.max_anisotropy = if anisotropy { 16.0 } else { 1.0 };
            non_null(SDL_CreateGPUSampler(self.device, &info), "create sampler")
        }
    }

    /// Uploads the scene's meshes and images, the white stand-in texture and
    /// the font, through one transfer buffer, and builds the images' mipmaps.
    /// The scene's meshes and pictures to the graphics card; `sources`, the
    /// table's pictures, for those decoded only now (`Image::pending`).
    fn upload(&mut self, scene: &Scene, sources: &[vpin::vpx::image::ImageData]) -> Result<(), String> {
        let vertex_bytes = bytes_of_slice(&scene.vertices);
        let index_bytes = bytes_of_slice(&scene.indices);
        self.vertex_buffer = self.buffer(SDL_GPU_BUFFERUSAGE_VERTEX, vertex_bytes.len())?;
        self.index_buffer = self.buffer(SDL_GPU_BUFFERUSAGE_INDEX, index_bytes.len())?;
        self.overlay_buffer = self.buffer(SDL_GPU_BUFFERUSAGE_VERTEX, MAX_OVERLAY_CHARS * 6 * size_of::<OverlayVertex>())?;
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
            info.size = (MAX_OVERLAY_CHARS * 6 * size_of::<OverlayVertex>()) as u32;
            self.overlay_transfer = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
            let animated: usize = scene.animations.iter().map(|a| a.uvs.len()).sum();
            if animated > 0 {
                info.size = (animated * size_of::<crate::scene::Vertex>()) as u32;
                self.anim_transfer = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
            }
        }

        let color_usage = SDL_GPU_TEXTUREUSAGE_SAMPLER | SDL_GPU_TEXTUREUSAGE_COLOR_TARGET;
        let srgb = SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB;
        for image in &scene.images {
            // a picture of light past white gets a texture of its own below, one still to decode
            // when it is
            if image.hdr.is_some() || image.pending.is_some() {
                self.textures.push(null_mut());
                continue;
            }
            let levels = 32 - image.width.max(image.height).max(1).leading_zeros();
            let t = self.texture(srgb, color_usage, image.width, image.height, levels, SDL_GPU_SAMPLECOUNT_1)?;
            self.textures.push(t);
        }
        self.white = self.texture(srgb, SDL_GPU_TEXTUREUSAGE_SAMPLER, 1, 1, 1, SDL_GPU_SAMPLECOUNT_1)?;
        let (font_w, font_h, font_rgba) = font_atlas();
        self.font = self.texture(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM, SDL_GPU_TEXTUREUSAGE_SAMPLER, font_w, font_h, 1, SDL_GPU_SAMPLECOUNT_1)?;
        self.sampler = self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_REPEAT, true)?;
        self.trilinear = [
            self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_REPEAT, false)?,
            self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE, false)?,
            self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_MIRRORED_REPEAT, false)?,
        ];
        self.font_sampler = self.sampler(SDL_GPU_FILTER_NEAREST, SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE, false)?;
        self.flat_sampler = self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE, false)?;
        self.clamp_sampler = self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_CLAMP_TO_EDGE, true)?;

        let white = [255u8; 4];
        let total = vertex_bytes.len() + index_bytes.len() + 4 + font_rgba.len();
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
            info.size = total as u32;
            let transfer = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
            let mapped = SDL_MapGPUTransferBuffer(self.device, transfer, false) as *mut u8;
            if mapped.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, transfer);
                return Err(format!("map transfer buffer: {}", sdl_error()));
            }
            let mut offset = 0;
            let mut put = |bytes: &[u8]| {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped.add(offset), bytes.len());
                offset += bytes.len();
                (offset - bytes.len()) as u32
            };
            let vertex_at = put(vertex_bytes);
            let index_at = put(index_bytes);
            let white_at = put(&white);
            let font_at = put(&font_rgba);
            SDL_UnmapGPUTransferBuffer(self.device, transfer);

            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            let copy = SDL_BeginGPUCopyPass(command);
            let mut source: SDL_GPUTransferBufferLocation = zeroed();
            source.transfer_buffer = transfer;
            let mut region: SDL_GPUBufferRegion = zeroed();
            for (buffer, at, bytes) in [(self.vertex_buffer, vertex_at, vertex_bytes), (self.index_buffer, index_at, index_bytes)] {
                source.offset = at;
                region.buffer = buffer;
                region.size = bytes.len() as u32;
                SDL_UploadToGPUBuffer(copy, &source, &region, false);
            }
            upload_texture(copy, transfer, self.white, white_at, 1, 1);
            upload_texture(copy, transfer, self.font, font_at, font_w, font_h);
            SDL_EndGPUCopyPass(copy);
            let submitted = SDL_SubmitGPUCommandBuffer(command);
            SDL_ReleaseGPUTransferBuffer(self.device, transfer);
            check(submitted, "upload the scene")?;
        }
        // the table's pictures, in batches, each waited for before the next, so the copies on their
        // way to the graphics card never hold more than a batch (a big table has gigabytes of them)
        let flat: Vec<usize> = (0..scene.images.len()).filter(|&n| scene.images[n].hdr.is_none() && scene.images[n].pending.is_none()).collect();
        let mut first = 0;
        while first < flat.len() {
            let (mut last, mut bytes) = (first, 0);
            while last < flat.len() && (last == first || bytes + scene.images[flat[last]].rgba.len() <= UPLOAD_BATCH) {
                bytes += scene.images[flat[last]].rgba.len();
                last += 1;
            }
            let batch = &flat[first..last];
            // VPX's mipmaps for each, made on a few threads
            let images = &scene.images;
            let chains = par_map(batch, |&n| {
                let image = &images[n];
                if image.width.max(image.height) > 1 { mips::chain(mips::Texels::Srgb8, &image.rgba, image.width, image.height) } else { Vec::new() }
            });
            let spans: Vec<usize> = batch.iter().zip(&chains).map(|(&n, chain)| chain_span(images[n].width, images[n].height, 4, chain)).collect();
            let bytes: usize = spans.iter().sum();
            unsafe {
                let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
                info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
                info.size = bytes as u32;
                let transfer = non_null(SDL_CreateGPUTransferBuffer(self.device, &info), "create transfer buffer")?;
                let mapped = SDL_MapGPUTransferBuffer(self.device, transfer, false) as *mut u8;
                if mapped.is_null() {
                    SDL_ReleaseGPUTransferBuffer(self.device, transfer);
                    return Err(format!("map transfer buffer: {}", sdl_error()));
                }
                let mut at = Vec::with_capacity(batch.len());
                let mut offset = 0;
                for (&n, chain) in batch.iter().zip(&chains) {
                    let image = &scene.images[n];
                    at.push(offset);
                    offset += put_chain(mapped, offset, &image.rgba, image.width, image.height, 4, chain);
                }
                SDL_UnmapGPUTransferBuffer(self.device, transfer);
                let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
                let copy = SDL_BeginGPUCopyPass(command);
                for ((&n, &offset), chain) in batch.iter().zip(&at).zip(&chains) {
                    let image = &scene.images[n];
                    upload_chain(copy, transfer, self.textures[n], offset, image.width, image.height, 4, chain);
                }
                SDL_EndGPUCopyPass(copy);
                let submitted = SDL_SubmitGPUCommandBuffer(command);
                SDL_ReleaseGPUTransferBuffer(self.device, transfer);
                check(submitted, "upload the table's pictures")?;
                SDL_WaitForGPUIdle(self.device);
            }
            first = last;
        }
        // pictures of light past white, each through its own transfer, waited for: linear half
        // floats, as VPX keeps them (RGB_FP16)
        for (n, image) in scene.images.iter().enumerate() {
            if let Some(hdr) = &image.hdr {
                self.textures[n] =
                    self.picture_of(SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT, image.width, image.height, bytes_of_slice(hdr), image.width.max(image.height) > 1, "upload a high-range picture")?;
                unsafe { SDL_WaitForGPUIdle(self.device) };
            }
        }
        self.upload_pending(scene, sources)
    }

    /// The pictures still to decode (`Image::pending`): decoded on up to eight
    /// threads at once, each uploaded as soon as it's decoded and let go, the
    /// decoders waiting while the pictures decoded but not yet uploaded would
    /// take more than `DECODE_BUDGET`: a big table's pictures never all sit
    /// decoded in memory at once, and every thread stays busy. A picture
    /// several slots share is decoded once.
    fn upload_pending(&mut self, scene: &Scene, sources: &[vpin::vpx::image::ImageData]) -> Result<(), String> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::{Condvar, Mutex};
        let mut groups: Vec<((usize, crate::scene::Decode), Vec<usize>)> = Vec::new();
        let mut group_of: std::collections::HashMap<(usize, crate::scene::Decode), usize> = std::collections::HashMap::new();
        for (n, image) in scene.images.iter().enumerate() {
            let Some(key) = image.pending else { continue };
            match group_of.get(&key) {
                Some(&g) => groups[g].1.push(n),
                None => {
                    group_of.insert(key, groups.len());
                    groups.push((key, vec![n]));
                }
            }
        }
        if groups.is_empty() {
            return Ok(());
        }
        // what each takes while it's decoded (one bigger than the budget goes alone); its mipmaps
        // add up to a third more while it waits, left out as the budget was measured without them
        let costs: Vec<usize> = groups.iter().map(|((t, _), _)| sources.get(*t).map_or(0, crate::table_meshes::decode_cost).min(DECODE_BUDGET)).collect();
        let free = Mutex::new(DECODE_BUDGET);
        let freed = Condvar::new();
        let next = AtomicUsize::new(0);
        let threads = std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get).clamp(1, 8).min(groups.len());
        let (send, received) = std::sync::mpsc::channel::<(usize, Option<crate::scene::Image>, Vec<mips::Level>)>();
        let mut failure = None;
        std::thread::scope(|scope| {
            for _ in 0..threads {
                let send = send.clone();
                let (groups, costs, free, freed, next) = (&groups, &costs, &free, &freed, &next);
                scope.spawn(move || {
                    loop {
                        let g = next.fetch_add(1, Ordering::Relaxed);
                        let Some(((t, how), _)) = groups.get(g) else { break };
                        // room for it among the pictures not yet uploaded
                        let mut room = free.lock().unwrap_or_else(|e| e.into_inner());
                        while *room < costs[g] {
                            room = freed.wait(room).unwrap_or_else(|e| e.into_inner());
                        }
                        *room -= costs[g];
                        drop(room);
                        let image = sources.get(*t).and_then(|s| crate::scene::Image::decode(s, *how));
                        // VPX's mipmaps, made here beside the decoding
                        let chain = image.as_ref().map(image_chain).unwrap_or_default();
                        if send.send((g, image, chain)).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(send);
            // each uploaded as it comes, then let go; the graphics card's copies waited for now
            // and then, so the uploads' own buffers don't pile up either
            let mut since_wait = 0usize;
            for (g, image, chain) in received {
                if failure.is_none()
                    && let Err(e) = self.upload_decoded(&groups[g].1, image.as_ref(), &chain, sources.get(groups[g].0.0))
                {
                    failure = Some(e);
                }
                drop(image);
                drop(chain);
                *free.lock().unwrap_or_else(|e| e.into_inner()) += costs[g];
                freed.notify_all();
                since_wait += costs[g];
                if since_wait > DECODE_BUDGET {
                    unsafe { SDL_WaitForGPUIdle(self.device) };
                    since_wait = 0;
                }
            }
        });
        unsafe { SDL_WaitForGPUIdle(self.device) };
        failure.map_or(Ok(()), Err)
    }

    /// One decoded picture's textures, a slot each (white for one that
    /// couldn't be decoded, as no picture), with its mipmaps, `chain`.
    fn upload_decoded(&mut self, slots: &[usize], image: Option<&crate::scene::Image>, chain: &[mips::Level], source: Option<&vpin::vpx::image::ImageData>) -> Result<(), String> {
        for &n in slots {
            self.textures[n] = match image {
                Some(image) => {
                    if let Some(a) = self.scene.image_has_alpha.get_mut(n) {
                        *a = image.has_alpha;
                    }
                    let mipmaps = image.width.max(image.height) > 1;
                    match &image.hdr {
                        Some(hdr) => self.picture_with(SDL_GPU_TEXTUREFORMAT_R16G16B16A16_FLOAT, image.width, image.height, bytes_of_slice(hdr), mipmaps, chain, "upload a high-range picture")?,
                        None => self.picture_with(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB, image.width, image.height, &image.rgba, mipmaps, chain, "upload a picture")?,
                    }
                }
                None => {
                    eprintln!("image {}: could not be decoded", source.map_or("?", |s| s.name.as_str()));
                    self.picture_of(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB, 1, 1, &[255; 4], false, "upload a stand-in picture")?
                }
            };
        }
        Ok(())
    }

    fn ensure_targets(&mut self, out_width: u32, out_height: u32, input: &FrameInput) -> Result<(), String> {
        // the table's pictures at its size (supersampled, VPX's AAFactor); bloom's from the window's
        let (width, height) = self.render_size(out_width, out_height);
        let wanted = self.mirrors_wanted(input);
        let refracted = self.refractions_wanted(input);
        let missing = wanted.iter().enumerate().any(|(k, &w)| w && self.mirror_textures.get(k).is_none_or(|t| t.is_null()))
            || refracted.iter().enumerate().any(|(r, &w)| w && self.refraction_textures.get(r).is_none_or(|t| t.is_null()));
        if self.target_size == (width, height) && !self.msaa.is_null() && !missing {
            return Ok(());
        }
        unsafe {
            for t in [self.msaa, self.depth, self.hdr_target, self.bloom[0], self.bloom[1]] {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t);
                }
            }
        }
        self.hdr_target = null_mut();
        self.bloom = [null_mut(); 2];
        unsafe {
            for t in self.mirror_textures.drain(..).chain([self.prev_hdr, self.mirror_blur, self.refraction_blur]) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t);
                }
            }
            for t in self.refraction_textures.drain(..).chain(self.refraction_depths.drain(..)) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t);
                }
            }
        }
        self.refraction_blur = null_mut();
        self.prev_hdr = null_mut();
        self.prev_valid.set(false);
        self.msaa = self.texture(self.scene_format, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET, width, height, 1, self.samples)?;
        // a refraction's copy reads the depth (only without multisampling: SDL can't read a multisampled one)
        let single = self.samples == SDL_GPU_SAMPLECOUNT_1;
        // and the post-processing passes read it, to leave what isn't the table untouched
        let read_depth = if single && (refracted.iter().any(|&r| r) || self.post_reads_depth() || self.ao_mode() == 2 || self.motion_blur_on()) {
            SDL_GPU_TEXTUREUSAGE_SAMPLER
        } else {
            SDL_GPUTextureUsageFlags(0)
        };
        self.depth = self.texture(DEPTH_FORMAT, SDL_GPU_TEXTUREUSAGE_DEPTH_STENCIL_TARGET | read_depth, width, height, 1, self.samples)?;
        if self.hdr {
            let usage = SDL_GPU_TEXTUREUSAGE_COLOR_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER;
            self.hdr_target = self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            // the previous frame's picture, for the ball's mirror of the playfield (VPX swaps two)
            self.prev_hdr = self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            // VPX's bloom pictures, a quarter of the window across and down, which also hold
            // the bulb light buffer while the table is drawn
            let (bw, bh) = ((out_width / 4).max(1), (out_height / 4).max(1));
            self.bloom_size = (bw, bh);
            for k in 0..2 {
                self.bloom[k] = self.texture(HDR_FORMAT, usage, bw, bh, 1, SDL_GPU_SAMPLECOUNT_1)?;
            }
            // a picture for each mirror some part shows (VPX's probes, at the picture's size; VPX
            // halves the roughest, here blurred twice as far instead), and one to blur them through
            let mut rough = false;
            for (k, &shown) in wanted.iter().enumerate() {
                let texture = if shown { self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)? } else { null_mut() };
                rough |= shown && mirror_blur(self.scene.mirrors[k].roughness).is_some();
                self.mirror_textures.push(texture);
            }
            self.mirror_blur = if rough { self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)? } else { null_mut() };
            // a picture and depth for each refraction probe a part shows, and one to blur them through
            let mut rough = false;
            for (r, &shown) in refracted.iter().enumerate() {
                let (color, depth) = if shown {
                    let depth_usage = SDL_GPU_TEXTUREUSAGE_DEPTH_STENCIL_TARGET | SDL_GPU_TEXTUREUSAGE_SAMPLER;
                    (
                        self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?,
                        if single { self.texture(DEPTH_FORMAT, depth_usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)? } else { null_mut() },
                    )
                } else {
                    (null_mut(), null_mut())
                };
                rough |= shown && refraction_blur(self.scene.refractions[r].roughness).is_some();
                self.refraction_textures.push(color);
                self.refraction_depths.push(depth);
            }
            self.refraction_blur = if rough { self.texture(HDR_FORMAT, usage, width, height, 1, SDL_GPU_SAMPLECOUNT_1)? } else { null_mut() };
            if single && self.refraction_copy.is_null() && refracted.iter().any(|&r| r) {
                self.refraction_copy = self.still_pipeline(c"still_restore_fs", 3)?;
            }
        }
        self.target_size = (width, height);
        Ok(())
    }

    /// VPX's supersampling (`AAFactor`, 0.5 to 2): the table drawn this many
    /// times the window's size across and down, then read down to it (in
    /// VPX's look; 1 draws it at the window's size, VPX's default).
    pub fn set_supersampling(&mut self, factor: f32) {
        self.aa_factor = factor.clamp(0.5, 2.0);
    }

    /// The table's picture's size for a window of `width` × `height`: as
    /// VPX sizes its back buffer, the window's times its AAFactor, rounded
    /// down.
    pub(super) fn render_size(&self, width: u32, height: u32) -> (u32, u32) {
        if !self.hdr || self.aa_factor == 1.0 {
            return (width, height);
        }
        (((width as f32 * self.aa_factor) as u32).max(1), ((height as f32 * self.aa_factor) as u32).max(1))
    }

    /// Switches the colour grade picture, as a script does (VPX's
    /// `ColorGradeImage`): a 256 x 16 picture, or none. SDL keeps the old
    /// picture until the frames drawing with it are done.
    pub fn set_color_grade(&mut self, image: Option<&crate::scene::Image>) -> Result<(), String> {
        if !self.hdr {
            return Ok(());
        }
        if !self.color_grade.is_null() {
            unsafe { SDL_ReleaseGPUTexture(self.device, self.color_grade) };
            self.color_grade = null_mut();
        }
        if let Some(image) = image {
            self.color_grade = self.unorm_picture(image.width, image.height, &image.rgba8(), false, "upload the colour grade")?;
        }
        Ok(())
    }

    /// Sets the bloom's strength, as a script does (VPX's `BloomStrength`;
    /// none at 0 or below).
    pub fn set_bloom_strength(&mut self, strength: f32) {
        self.tone.bloom_strength = strength;
    }

    /// No bloom at all (VPX's `ForceBloomOff`), or the table's again.
    pub fn set_bloom_off(&mut self, off: bool) {
        self.bloom_off = off;
    }

    /// The player's ball settings (VPX 5436's `Renderer::Renderer`): pictures of their own for every
    /// ball (`OverwriteBallImage` with `BallImage` and `DecalImage`; the room picture then read as a
    /// sphere map, as `Ball::RenderSetup` does), balls without lighting
    /// (`DisableLightingForBalls`) and squeezed back against the view's stretching (`BallAntiStretch`).
    /// A picture that won't load is left out, as VPX's `CreateFromFile` leaves it.
    pub fn set_ball_settings(&mut self, pictures: &crate::vpx_settings::BallOverride, lighting_off: bool, anti_stretch: bool) {
        let load = |path: &std::path::Path| -> Option<*mut SDL_GPUTexture> {
            let rgba = image::open(path).map_err(|e| eprintln!("ball picture {}: {e}", path.display())).ok()?.to_rgba8();
            self.picture_of(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB, rgba.width(), rgba.height(), rgba.as_raw(), true, "upload the player's ball picture").ok()
        };
        let (env, decal) = (pictures.image.as_deref().and_then(load), pictures.decal.as_deref().and_then(load));
        self.ball_override_env = env.unwrap_or(null_mut());
        self.ball_override_decal = decal.unwrap_or(null_mut());
        self.ball_lighting_off = lighting_off;
        self.ball_anti_stretch = anti_stretch;
    }

    /// The most a mirror reflects (`[Player]` `PFReflection`: 0 nothing, 1 balls, 2 the still parts,
    /// 3 still parts and balls, 4 still and moving parts unsynced, 5 everything, VPX's default).
    pub fn set_max_reflection_mode(&mut self, mode: u32) {
        self.max_reflection_mode = mode.min(5);
    }

    /// A mirror's reflection mode now: its own held to the player's most (VPX's
    /// `min(m_reflection_mode, GetMaxReflectionMode())`), where without the still parts drawn once
    /// a most of still parts or more means everything (`Renderer::GetMaxReflectionMode`).
    fn reflection_mode(&self, k: usize) -> u32 {
        let max = if (!self.still_wanted || self.view_changing) && self.max_reflection_mode >= 2 { 5 } else { self.max_reflection_mode };
        self.scene.mirrors[k].mode.min(5).min(max)
    }

    /// The parts' pictures read anisotropically (VPX's default) or, off, trilinearly (`[Player]`
    /// `ForceAnisotropicFiltering`; VPX's `Renderer::SetAnisoFiltering`, which changes the parts'
    /// default way of reading their pictures and no other).
    pub fn set_anisotropic(&mut self, on: bool) -> Result<(), String> {
        if on == self.anisotropic {
            return Ok(());
        }
        let sampler = self.sampler(SDL_GPU_FILTER_LINEAR, SDL_GPU_SAMPLERADDRESSMODE_REPEAT, on)?;
        // SAFETY: no frame is being recorded; the old sampler is ours
        unsafe { SDL_ReleaseGPUSampler(self.device, self.sampler) };
        self.sampler = sampler;
        self.anisotropic = on;
        Ok(())
    }

    /// VPX 5436's HDR10 output, when the window's screen has HDR on and the
    /// settings let it.
    pub fn hdr10(&self) -> Option<Hdr10> {
        self.hdr10
    }

    /// The window's size in pixels, as the swapchain will be.
    pub fn window_size(&self) -> (u32, u32) {
        let (mut w, mut h) = (0, 0);
        unsafe { SDL_GetWindowSizeInPixels(self.window, &mut w, &mut h) };
        (w.max(1) as u32, h.max(1) as u32)
    }

    /// Draws one frame if a swapchain image is free; never waits for one.
    pub fn frame(&mut self, input: &FrameInput) -> Result<FrameResult, String> {
        unsafe {
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            let mut swapchain = null_mut();
            let (mut width, mut height) = (0, 0);
            if !SDL_AcquireGPUSwapchainTexture(command, self.window, &mut swapchain, &mut width, &mut height) {
                SDL_CancelGPUCommandBuffer(command);
                return Err(format!("get the next frame: {}", sdl_error()));
            }
            if swapchain.is_null() {
                SDL_CancelGPUCommandBuffer(command);
                return Ok(FrameResult::Skipped);
            }
            let (rw, rh) = self.render_size(width, height);
            if let Err(e) = self
                .ensure_targets(width, height, input)
                .and_then(|_| self.ensure_post(width, height))
                .and_then(|_| self.ensure_effects(rw, rh))
                .and_then(|_| self.ensure_still(rw, rh))
                .and_then(|_| self.draw_still(input, width, height))
            {
                SDL_CancelGPUCommandBuffer(command);
                return Err(e);
            }
            self.update_text_boxes(command, input.text_boxes, width);
            self.update_dmd(command, input.dmd);
            self.update_animations(command, input);
            self.update_ramps(command, input);
            self.update_lights(input);
            // the windows drawn inside the table's first, which its frame takes in; their own after it
            self.present_extras(command, input, true);
            self.record(command, swapchain, width, height, input, self.stage());
            self.present_extras(command, input, false);
            check(SDL_SubmitGPUCommandBuffer(command), "submit the frame")?;
            Ok(FrameResult::Submitted { width, height })
        }
    }

    /// Draws the score display, the backglass and the topper on `command`: those drawn inside the
    /// table's window (`embedded`), or those with windows of their own.
    unsafe fn present_extras(&mut self, command: *mut SDL_GPUCommandBuffer, input: &FrameInput, embedded: bool) {
        // SAFETY: the windows outlive the renderer; `command` is recording
        unsafe {
            if self.display.as_ref().is_some_and(|d| d.surface.is_embedded() == embedded) {
                self.present_display(command, input.dmd, input.segments, input.backglass, input.pup, input.display_hidden);
            }
            if self.backglass.as_ref().is_some_and(|b| b.surface.is_embedded() == embedded) {
                self.present_backglass(command, input.backglass, input.dmd, input.pup);
            }
            if self.topper.as_ref().is_some_and(|t| t.surface.is_embedded() == embedded) {
                self.present_topper(command, input.pup);
            }
        }
    }

    /// Draws the score display's window on `command` when the display or
    /// the window's size changed since it was last shown, and the window has
    /// an image free: the backglass file's score panel when it shows there,
    /// else the machine's displays.
    unsafe fn present_display(
        &mut self,
        command: *mut SDL_GPUCommandBuffer,
        frame: Option<&crate::dmd::DmdFrame>,
        segments: Option<&crate::segments::SegmentsNow>,
        backglass: Option<&std::sync::Arc<crate::b2s::BackglassNow>>,
        pup: Option<&std::sync::Arc<crate::pup::draw::PupNow>>,
        hidden: bool,
    ) {
        // the floating score window (one screen, VPX 10.8.0's PinMAME window) hides while the
        // script hides PinMAME's window, as VPinMAME's does
        if self.floating_display && hidden {
            if let Some(d) = &mut self.display {
                d.shown = None;
                // SAFETY: the window outlives the renderer
                if let Some(window) = d.surface.window() {
                    unsafe {
                        if (SDL_GetWindowFlags(window) & SDL_WINDOW_HIDDEN).0 == 0 {
                            SDL_HideWindow(window);
                        }
                    }
                }
            }
            return;
        }
        // VPX's renderers for the window, in order: the backglass file's score panel, a PinUP
        // pack's score screens (first when preferred), then the machine's displays
        let panel = self.shows_panel(backglass);
        let layers = Self::pup_layers(pup, 2);
        if let Some(layers) = layers.filter(|_| panel.is_none() || self.pack_first[1]) {
            let Some(d) = &mut self.display else { return };
            d.shown = None;
            if let Some(p) = &mut d.panel {
                p.forget();
            }
            let (surface, format, kept) = (d.surface, d.format, d.pup.take());
            self.reveal(surface);
            let mut p = match kept {
                Some(p) => p,
                None => match self.page_pipeline(format) {
                    Ok(pipeline) => pup::PupDrawing::new(format, pipeline),
                    Err(e) => {
                        crate::script::log::once(&format!("PinUP in the score window: {e}"));
                        return;
                    }
                },
            };
            // SAFETY: the window outlives the renderer
            unsafe { self.present_pup(command, surface, &mut p, layers) };
            if let Some(d) = &mut self.display {
                d.pup = Some(p);
            }
            return;
        }
        if let Some(d) = &mut self.display
            && let Some(p) = &mut d.pup
        {
            p.forget();
        }
        if let Some(now) = panel {
            let Some(d) = &mut self.display else { return };
            d.shown = None;
            let surface = d.surface;
            self.reveal(surface);
            // SAFETY: the window outlives the renderer
            unsafe { self.present_panel(command, now, frame) };
            return;
        }
        let Some(d) = &mut self.display else { return };
        if let Some(p) = &mut d.panel {
            p.forget();
        }
        let surface = d.surface;
        let (w, h) = self.surface_size(surface);
        let key = (frame.map_or(0, |f| f.id), segments.map_or(0, |s| s.id), w, h);
        if self.display.as_ref().is_some_and(|d| d.shown == Some(key)) {
            return;
        }
        if frame.is_some() || segments.is_some() {
            self.reveal(surface);
        }
        let plan = self.plan_display(frame, segments, (key.2, key.3));
        // SAFETY: as above; the image is drawn on this command buffer
        unsafe {
            let Some((target, tw, th)) = self.acquire_surface(command, surface) else { return };
            self.record_display(command, target, tw, th, frame, &plan);
        }
        if let Some(d) = &mut self.display {
            d.shown = Some(key);
        }
    }

    /// What the score window and the backglass show of the DMD: the backglass
    /// file's score panel in the score window (None: the machine's displays
    /// alone) and where the DMD shows in it, and where it shows over the
    /// backglass (VPX's B2S DMD overlay settings).
    pub fn set_dmd_overlays(&mut self, score_panel: Option<crate::dmd_overlay::Overlay>, backglass: crate::dmd_overlay::Overlay) {
        self.score_panel = score_panel;
        self.backglass_dmd = backglass;
        if let Some(d) = &mut self.display {
            d.shown = None;
        }
    }

    /// Whether a PinUP pack's screens come first in the backglass and the score windows, before
    /// the backglass file's pictures (VPX: its renderers' priority settings, which put the file's
    /// first by default; ours puts the pack's first wherever it draws unless they're set).
    pub fn set_pack_first(&mut self, backglass: bool, score: bool) {
        self.pack_first = [backglass, score];
    }

    /// Whether the backglass, score and topper windows show once something first draws in them
    /// (VPX's `Renderer::RenderAncillaryWindow`: its windows are made hidden and shown then);
    /// test mode keeps them hidden.
    pub fn set_reveal_windows(&mut self, reveal: bool) {
        self.reveal_windows = reveal;
    }

    /// The score window floats over the table on one screen (VPX 10.8.0's PinMAME window): it
    /// shows only while the script leaves PinMAME's window shown.
    pub fn set_floating_display(&mut self, floating: bool) {
        self.floating_display = floating;
    }

    /// Shows a backglass, score or topper window that's still hidden, now that something draws
    /// in it, unless windows stay hidden; one drawn inside the table's has its place taken.
    fn reveal(&self, surface: Surface) {
        let window = match surface {
            Surface::Window(window) => window,
            Surface::Embedded(kind) => {
                self.mark_embedded(kind);
                return;
            }
        };
        if !self.reveal_windows || window.is_null() {
            return;
        }
        // SAFETY: the window outlives the renderer
        unsafe {
            if (SDL_GetWindowFlags(window) & SDL_WINDOW_HIDDEN).0 != 0 {
                SDL_ShowWindow(window);
            }
        }
    }

    /// Gives the score display's window the layouts to choose from (VPX's
    /// score view finds a table's own, else its defaults).
    pub fn set_score_layouts(&mut self, layouts: Vec<crate::scoreview::Layout>) {
        self.score_layouts = layouts;
        if let Some(d) = &mut self.display {
            d.shown = None;
        }
    }

    /// What the score display's window shows in `out` pixels, made ready
    /// before its pass: the layout VPX's score view would choose places the
    /// DMD and every digit; with none, the DMD isn't shown (as in VPX) and
    /// the digits take PinMAME's own places.
    fn plan_display(&mut self, frame: Option<&crate::dmd::DmdFrame>, segments: Option<&crate::segments::SegmentsNow>, out: (u32, u32)) -> DisplayPlan {
        let mut plan = DisplayPlan { dmd: None, digits: Vec::new(), pipeline: None };
        let Some(format) = self.display.as_ref().map(|d| d.format) else { return plan };
        let uploaded = frame.filter(|f| !self.dmd_texture.is_null() && self.dmd_id == Some(f.id));
        let counts: Vec<usize> = segments.map(|s| s.layout.displays.iter().map(|d| d.digits.len()).collect()).unwrap_or_default();
        let sources = crate::scoreview::Sources { dmd: uploaded.map(|f| (f.width, f.height, false)), segments: &counts };
        let firsts: Vec<usize> = counts.iter().scan(0, |at, &n| { let first = *at; *at += n; Some(first) }).collect();
        if let Some(layout) = crate::scoreview::select(&self.score_layouts, &sources, out) {
            plan.dmd = uploaded.and_then(|f| crate::scoreview::dmd(layout, (f.width, f.height), f.luminance, f.tint, out));
            if let Some(s) = segments {
                for p in crate::scoreview::digits(layout, &sources, out) {
                    let (Some(display), Some(&first)) = (s.layout.displays.get(p.display), firsts.get(p.display)) else { continue };
                    let (Some(digit), Some(&levels)) = (display.digits.get(p.digit), s.levels.get(first + p.digit)) else { continue };
                    let (style, hint) = crate::scoreview::digit_style(p.style, display.hardware);
                    plan.digits.push(segments::DigitDraw {
                        rect: p.rect,
                        kind: digit.kind,
                        hint,
                        levels,
                        look: crate::segments::look(style, [1.0; 3], 1.0),
                        glass_tint: p.glass_tint,
                        roughness: p.roughness,
                        glass: p.glass.map(|g| (g, p.ambient)),
                    });
                }
            }
        } else if let Some(s) = segments {
            let rects = crate::segments::place(&s.layout, out);
            for (((display, digit), &rect), &levels) in s.layout.digits().zip(&rects).zip(&s.levels) {
                let (style, hint) = crate::scoreview::digit_style(None, display.hardware);
                plan.digits.push(segments::DigitDraw {
                    rect,
                    kind: digit.kind,
                    hint,
                    levels,
                    look: crate::segments::look(style, [1.0; 3], 1.0),
                    glass_tint: [1.0; 3],
                    roughness: crate::segments::GLASS_ROUGHNESS,
                    glass: None,
                });
            }
        }
        if !plan.digits.is_empty() {
            match self.prepare_digits(format, &plan.digits) {
                Ok(pipeline) => plan.pipeline = Some(pipeline),
                Err(e) => crate::script::log::once(&format!("segment displays: {e}")),
            }
        }
        plan
    }

    /// Draws the score display into `target`, `width` × `height` pixels, as
    /// planned: black, with the DMD and the digits where the plan puts them.
    unsafe fn record_display(&self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32, frame: Option<&crate::dmd::DmdFrame>, plan: &DisplayPlan) {
        let Some(d) = &self.display else { return };
        unsafe {
            // the layout's glass pictures uploaded before the pass
            for picture in plan.dmd.and_then(|p| p.look.glass).into_iter().chain(plan.digits.iter().filter_map(|d| d.glass.map(|(g, _)| g))).map(|g| g.picture) {
                self.glass_texture(picture);
            }
            let mut color: SDL_GPUColorTargetInfo = zeroed();
            color.texture = target;
            color.load_op = SDL_GPU_LOADOP_CLEAR;
            color.store_op = SDL_GPU_STOREOP_STORE;
            color.clear_color = SDL_FColor { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
            let pass = SDL_BeginGPURenderPass(command, &color, 1, std::ptr::null());
            // VPX's Reinhard, left linear for a swap chain that encodes sRGB, else in sRGB values as VPX
            // writes them (the score window in HDR10, VPX's look on an HDR screen)
            let mode = if embed::holds_srgb_values(d.format) { 2.0 } else { 1.0 };
            if let Some(pipeline) = plan.pipeline {
                self.draw_digits(command, pass, pipeline, &plan.digits, (width as f32, height as f32), mode);
            }
            let placed = frame.filter(|f| !self.dmd_texture.is_null() && self.dmd_id == Some(f.id)).zip(plan.dmd);
            if let Some((f, p)) = placed {
                self.draw_dmd(command, pass, d.pipeline, p.rect, &p.look, f.luminance, (width as f32, height as f32), mode);
            }
            SDL_EndGPURenderPass(pass);
        }
    }

    /// A score view glass picture's texture, uploaded with its mipmaps the
    /// first time (null if it can't be): sRGB colour, its alpha the roughness.
    fn glass_texture(&self, picture: u32) -> *mut SDL_GPUTexture {
        if let Some(&t) = self.glass_textures.borrow().get(&picture) {
            return t;
        }
        let texture = crate::scoreview::glass_picture(picture)
            .map(|p| self.picture_of(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB, p.width, p.height, &p.rgba, true, "upload a glass picture"))
            .and_then(|r| r.map_err(|e| crate::script::log::once(&format!("score view glass: {e}"))).ok())
            .unwrap_or(null_mut());
        self.glass_textures.borrow_mut().insert(picture, texture);
        texture
    }

    /// Draws the uploaded DMD frame into `rect` (left, top, right and bottom,
    /// in pixels of a target `size`) with the display pipeline `pipeline`, as
    /// VPX's newer display renderer draws it, its light leaving the shader in
    /// `mode` (1: VPX's Reinhard, linear; 2: then sRGB).
    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_dmd(
        &self,
        command: *mut SDL_GPUCommandBuffer,
        pass: *mut SDL_GPURenderPass,
        pipeline: *mut SDL_GPUGraphicsPipeline,
        rect: [f32; 4],
        l: &crate::scoreview::Look,
        luminance: bool,
        size: (f32, f32),
        mode: f32,
    ) {
        let (w, h) = size;
        // the layout's glass picture over the display, as VPX's score view lays it (none on the backglass)
        let glass = l.glass.map(|g| (g, self.glass_texture(g.picture))).filter(|(_, t)| !t.is_null());
        let quad = DisplayQuadUniforms {
            rect: [rect[0] / w * 2.0 - 1.0, 1.0 - rect[1] / h * 2.0, rect[2] / w * 2.0 - 1.0, 1.0 - rect[3] / h * 2.0],
            glass_area: glass.map_or([0.0, 0.0, 1.0, 1.0], |(g, _)| g.area),
            glass_pad: [0.0; 4],
        };
        let uniforms = DisplayUniforms {
            lit_diffuse: [l.lit[0], l.lit[1], l.lit[2], l.diffuse],
            unlit_mode: [l.unlit[0], l.unlit[1], l.unlit[2], mode],
            glass_tint_roughness: [l.glass_tint[0], l.glass_tint[1], l.glass_tint[2], l.roughness],
            glass_ambient: [l.ambient[0], l.ambient[1], l.ambient[2], if glass.is_some() { 1.0 } else { 0.0 }],
            size: [l.dots[0], l.dots[1], 0.0, 0.0],
            properties: [if luminance { 0.0 } else { 1.0 }, l.sdf_offset, l.threshold, 0.0],
            seg_state: [[0.0; 4]; 4],
        };
        // SAFETY: the pass and pipeline are live, and the frame is uploaded
        unsafe {
            SDL_BindGPUGraphicsPipeline(pass, pipeline);
            SDL_PushGPUVertexUniformData(command, 0, (&raw const quad).cast(), size_of::<DisplayQuadUniforms>() as u32);
            SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<DisplayUniforms>() as u32);
            let bindings = [
                SDL_GPUTextureSamplerBinding { texture: self.dmd_texture, sampler: self.font_sampler },
                match glass {
                    Some((_, texture)) => SDL_GPUTextureSamplerBinding { texture, sampler: self.clamp_sampler },
                    None => SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.flat_sampler },
                },
            ];
            SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 2);
            SDL_DrawGPUPrimitives(pass, 6, 1, 0, 0);
        }
    }

    /// Redraws the pictures of text boxes whose text changed since they were
    /// last shown (VPX's `m_textureDirty`, handled when the box is drawn),
    /// and uploads them on `command` ahead of its frame.
    fn update_text_boxes(&mut self, command: *mut SDL_GPUCommandBuffer, boxes: &[crate::textbox::TextBoxNow], window_width: u32) {
        // each picture with VPX's mipmaps, which its trilinear sampler reads as the backdrop
        // shrinks the picture a little at most window sizes (`Renderer::DrawSprite`)
        let mut pictures: Vec<(usize, Vec<u8>, Vec<mips::Level>)> = Vec::new();
        for (k, look) in self.scene.text_boxes.iter().enumerate() {
            let Some(now) = boxes.iter().find(|b| b.part == look.part) else { continue };
            if !now.visible || now.display || self.text_box_textures[k].1 == Some(now.version) {
                continue;
            }
            let picture = self.painter.draw(look, now, window_width);
            let chain = mips::chain(mips::Texels::Srgb8, &picture, look.width, look.height);
            pictures.push((k, picture, chain));
            self.text_box_textures[k].1 = Some(now.version);
            if self.text_box_textures[k].0.is_null() {
                let levels = 32 - look.width.max(look.height).leading_zeros();
                match self.texture(SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB, SDL_GPU_TEXTUREUSAGE_SAMPLER, look.width, look.height, levels, SDL_GPU_SAMPLECOUNT_1) {
                    Ok(t) => self.text_box_textures[k].0 = t,
                    Err(e) => eprintln!("text box picture: {e}"),
                }
            }
        }
        if pictures.is_empty() {
            return;
        }
        let total: usize = pictures
            .iter()
            .map(|(k, _, chain)| {
                let look = &self.scene.text_boxes[*k];
                chain_span(look.width, look.height, 4, chain)
            })
            .sum();
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
            info.size = total as u32;
            let transfer = SDL_CreateGPUTransferBuffer(self.device, &info);
            if transfer.is_null() {
                return;
            }
            let mapped = SDL_MapGPUTransferBuffer(self.device, transfer, false) as *mut u8;
            if mapped.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, transfer);
                return;
            }
            let mut offsets = Vec::with_capacity(pictures.len());
            let mut offset = 0;
            for (k, p, chain) in &pictures {
                let look = &self.scene.text_boxes[*k];
                offsets.push(offset);
                offset += put_chain(mapped, offset, p, look.width, look.height, 4, chain);
            }
            SDL_UnmapGPUTransferBuffer(self.device, transfer);
            let copy = SDL_BeginGPUCopyPass(command);
            for ((k, _, chain), &at) in pictures.iter().zip(&offsets) {
                let texture = self.text_box_textures[*k].0;
                if texture.is_null() {
                    continue;
                }
                let look = &self.scene.text_boxes[*k];
                upload_chain(copy, transfer, texture, at, look.width, look.height, 4, chain);
            }
            SDL_EndGPUCopyPass(copy);
            SDL_ReleaseGPUTransferBuffer(self.device, transfer);
        }
    }

    /// How a mesh looks now: the table file's material and picture, with a
    /// script's swaps (`Image`, `Material`, `UpdateMaterial`,
    /// `BlendDisableLighting`) laid over them, then a primitive's colour,
    /// opacity and blending as VPX's `Primitive::Render` sets them: an added
    /// primitive is drawn unlit, its colour times its opacity (after its
    /// light map); any other draws blended when its opacity isn't 100.
    fn mesh_look(&self, draw: &crate::scene::Draw, input: &FrameInput) -> MeshLook {
        let mut look = self.script_look(draw, input);
        if let Some(pr) = draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.primitive) {
            look.static_color_alpha = pr.static_color_alpha();
            look.added = pr.add_blend;
            look.skip = pr.adds_nothing();
            // VPX: an opacity below 100 blends too, if the material's opacity is active
            look.blend |= look.opacity_active.unwrap_or(true) && !pr.add_blend && pr.alpha != 100.0;
        }
        look
    }

    /// The mirror a part shows now, and how strongly: the table file's, or
    /// the one its script named (a primitive's `ReflectionProbe`; none for
    /// a name the table lacks); the playfield always the playfield's
    /// (`Primitive::Render` for the playfield).
    fn mirror_shown(&self, draw: &crate::scene::Draw, input: &FrameInput) -> Option<(usize, f32)> {
        let named = draw.part.filter(|_| !draw.playfield).and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.look.as_ref()).and_then(|l| l.mirror);
        match named {
            Some(k) => k.map(|k| (k as usize, draw.mirror_strength)).filter(|&(k, s)| s > 0.0 && k < self.scene.mirrors.len()),
            None => draw.mirror,
        }
    }

    /// Where a part sorts among see-through parts now (VPX's depth bias): the table file's, or what
    /// its script set (a primitive's `DepthBias`, which VPX reads as it draws).
    fn depth_bias_now(&self, draw: &crate::scene::Draw, input: &FrameInput) -> f32 {
        draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.look.as_ref()).and_then(|l| l.depth_bias).unwrap_or(draw.depth_bias)
    }

    /// Drawn into mirrors now: as the table file says, or its script
    /// (`ReflectionEnabled`).
    fn reflected_now(&self, draw: &crate::scene::Draw, input: &FrameInput) -> bool {
        draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.look.as_ref()).and_then(|l| l.reflected).unwrap_or(draw.reflected)
    }

    /// The refraction probe a part shows now: the table file's, or the one its
    /// script named (a primitive's `RefractionProbe`; none for a name the table
    /// lacks).
    fn refraction_shown(&self, draw: &crate::scene::Draw, input: &FrameInput) -> Option<usize> {
        let named = draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.look.as_ref()).and_then(|l| l.refraction);
        match named {
            Some(r) => r.map(|r| r as usize).filter(|&r| r < self.scene.refractions.len()),
            None => draw.refraction,
        }
    }

    /// The refraction probes some part shows, in the table file or now: each
    /// gets a picture (VPX's look only).
    fn refractions_wanted(&self, input: &FrameInput) -> Vec<bool> {
        let mut wanted = vec![false; self.scene.refractions.len()];
        if self.hdr {
            for d in &self.scene.draws {
                for r in [d.refraction, self.refraction_shown(d, input)].into_iter().flatten() {
                    wanted[r] = true;
                }
            }
        }
        wanted
    }

    /// Copies the table as drawn so far, its picture and depth, into a
    /// refraction probe's (VPX's `BlitRenderTarget` of both), by one triangle
    /// as the still parts' are drawn back.
    unsafe fn copy_for_refraction(&self, command: *mut SDL_GPUCommandBuffer, color: *mut SDL_GPUTexture, depth: *mut SDL_GPUTexture, r: usize) {
        unsafe {
            let mut target: SDL_GPUColorTargetInfo = zeroed();
            target.texture = self.refraction_textures[r];
            target.load_op = SDL_GPU_LOADOP_DONT_CARE;
            target.store_op = SDL_GPU_STOREOP_STORE;
            let mut depth_target: SDL_GPUDepthStencilTargetInfo = zeroed();
            depth_target.texture = self.refraction_depths[r];
            depth_target.load_op = SDL_GPU_LOADOP_DONT_CARE;
            depth_target.store_op = SDL_GPU_STOREOP_STORE;
            depth_target.stencil_load_op = SDL_GPU_LOADOP_DONT_CARE;
            depth_target.stencil_store_op = SDL_GPU_STOREOP_DONT_CARE;
            let pass = SDL_BeginGPURenderPass(command, &target, 1, &depth_target);
            SDL_BindGPUGraphicsPipeline(pass, self.refraction_copy);
            let bindings = [
                SDL_GPUTextureSamplerBinding { texture: color, sampler: self.font_sampler },
                SDL_GPUTextureSamplerBinding { texture: depth, sampler: self.font_sampler },
            ];
            SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 2);
            SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
            SDL_EndGPURenderPass(pass);
        }
    }

    /// The mirrors some part shows, in the table file or now, that mirror
    /// something: each gets a picture.
    fn mirrors_wanted(&self, input: &FrameInput) -> Vec<bool> {
        let mut wanted = vec![false; self.scene.mirrors.len()];
        for d in &self.scene.draws {
            for (k, _) in [d.mirror, self.mirror_shown(d, input)].into_iter().flatten() {
                wanted[k] = true;
            }
        }
        for (k, w) in wanted.iter_mut().enumerate() {
            *w &= self.reflection_mode(k) > 0;
        }
        wanted
    }

    /// The table file's look with a script's swaps laid over it, blended
    /// as VPX's `Shader::SetMaterial` decides where the part's material is
    /// known (`Draw::material_alpha`).
    fn script_look(&self, draw: &crate::scene::Draw, input: &FrameInput) -> MeshLook {
        let m = &self.scene.materials[draw.material];
        let mut look = MeshLook {
            texture: m.image.map_or(self.white, |i| self.textures[i]),
            textured: m.image.is_some(),
            base_color: m.base_color,
            metallic: m.metallic,
            roughness: m.roughness,
            alpha_cutoff: m.alpha_cutoff.unwrap_or(0.0),
            blend: m.blend,
            disable_lighting: draw.disable_lighting,
            disable_lighting_below: draw.disable_lighting_below,
            static_color_alpha: [1.0; 4],
            added: false,
            skip: false,
            opacity_active: draw.material_alpha.map(|a| a.opacity_active),
            // VPX's material for the mesh, else one read from the export's
            shading: draw.shading.unwrap_or_else(|| crate::look::Shading {
                base: [m.base_color[0], m.base_color[1], m.base_color[2]],
                opacity: m.base_color[3],
                opacity_active: m.blend,
                metal: m.metallic > 0.5,
                roughness: 1.0 - m.roughness,
                ..crate::look::Shading::dummy()
            }),
        };
        // the picture's alpha and alpha test, and the material's opacity, as drawn now
        let mut texture_alpha = m.image.is_some_and(|i| self.scene.image_has_alpha[i]);
        let mut alpha_test = m.image.map_or(-1.0, |i| self.scene.picture_alpha_test[i]);
        let mut opacity = draw.material_alpha.map_or(1.0, |a| a.opacity);
        let changed = draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.look.as_ref());
        let swaps = match (changed, draw.slot) {
            (Some(changed), crate::scene::LookSlot::Main) => Some((changed, changed.image, changed.material)),
            (Some(changed), crate::scene::LookSlot::Side) => Some((changed, changed.side_image, changed.side_material)),
            _ => None,
        };
        if let Some((changed, image, material)) = swaps {
            // VPX lights all a part's meshes alike (a wall's sides as its top); light from below
            // stays off a part drawn ahead of time whatever its script says
            if let Some(d) = changed.disable_lighting {
                look.disable_lighting = d;
            }
            if let Some(d) = changed.disable_lighting_below.filter(|_| !draw.below_fixed) {
                look.disable_lighting_below = d;
            }
            match image {
                // a name the table doesn't have: VPX draws without a picture
                Some(None) => {
                    (look.texture, look.textured, texture_alpha, alpha_test) = (self.white, false, false, -1.0);
                }
                Some(Some(i)) => match self.scene.table_images.get(i as usize).copied().flatten() {
                    Some(s) => {
                        (look.texture, look.textured, texture_alpha) = (self.textures[s], true, self.scene.image_has_alpha[s]);
                        alpha_test = self.scene.image_alpha_test.get(i as usize).copied().unwrap_or(-1.0);
                    }
                    None => {
                        if self.missing_images.borrow_mut().insert(i) {
                            eprintln!("a script swapped in table image {i}, which wasn't decoded ahead: drawn with the table's picture");
                        }
                    }
                },
                None => {}
            }
            match material {
                Some(mat) => {
                    (look.base_color, look.metallic, look.roughness) = (mat.base_color, mat.metallic, mat.roughness);
                    look.shading = mat.shading;
                    look.blend = mat.blend || texture_alpha;
                    if look.opacity_active.is_some() {
                        (look.opacity_active, opacity) = (Some(mat.opacity_active), mat.base_color[3]);
                    }
                }
                None if image.is_some() => look.blend |= texture_alpha,
                None => {}
            }
        }
        // VPX's Shader::SetMaterial: blended only when the material's opacity is active and the
        // picture has alpha or the opacity is below 0.999; the picture's alpha test cuts either way
        if let Some(active) = look.opacity_active {
            look.blend = active && (texture_alpha || opacity < 0.999);
            look.alpha_cutoff = if look.textured && texture_alpha && alpha_test >= 0.0 { alpha_test + 1e-6 } else { 0.0 };
        }
        look
    }

    /// What scripts changed of how each light draws ([`crate::scene::LightLive`]), which VPX
    /// reads as it draws: where its glow is brightest, a bulb's halo height, its blend, the
    /// light it passes up, how far its glow reaches and how it fades, where it sorts.
    fn update_lights(&mut self, input: &FrameInput) {
        for (info, now) in self.scene.light_parts.iter_mut().zip(input.lights) {
            let live = now.live;
            info.center.x = live.center[0];
            info.center.y = live.center[1];
            info.halo_height = live.halo_height;
            info.modulate_vs_add = live.modulate_vs_add;
            info.transmission = live.transmission;
            // as the scene takes the table's at load (a glow's reach divides)
            info.falloff_radius = live.falloff.max(1.0);
            info.falloff_power = live.falloff_power;
            info.depth_bias = live.depth_bias;
            info.on_ball = live.on_ball;
            info.ball_shadows = live.ball_shadows;
        }
    }

    /// Shows each animated primitive's frame as the script left it (VPX's
    /// `UploadToVB` when its frame moved): its vertices made again and copied
    /// over its own in the vertex buffer, before anything is drawn.
    /// Builds again the ramps whose heights or widths a script changed since they were last shown (VPX's
    /// `m_dynamicVertexBufferRegenerate`), and uploads them on `command` ahead of its frame.
    fn update_ramps(&mut self, command: *mut SDL_GPUCommandBuffer, input: &FrameInput) {
        let changed: Vec<(usize, Option<[f32; 4]>)> = (self.scene.ramps.iter().enumerate())
            .filter_map(|(k, r)| {
                let want = input.part_render.get(r.part as usize).and_then(|p| p.ramp_shape);
                (want != self.ramp_shown[k]).then_some((k, want))
            })
            .collect();
        if changed.is_empty() {
            return;
        }
        let built: Vec<(usize, Vec<crate::scene::Vertex>)> = changed
            .iter()
            .map(|&(k, want)| {
                let r = &self.scene.ramps[k];
                (k, r.vertices(want.unwrap_or(r.file_shape())))
            })
            .collect();
        let stride = size_of::<crate::scene::Vertex>();
        let total: usize = built.iter().map(|(_, v)| v.len()).sum();
        unsafe {
            // room for every ramp at once, made the first time
            if self.ramp_transfer.is_null() {
                let most: usize = self.scene.ramps.iter().map(|r| r.vertices(r.file_shape()).len()).sum();
                let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
                info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
                info.size = (most.max(total).max(1) * stride) as u32;
                self.ramp_transfer = SDL_CreateGPUTransferBuffer(self.device, &info);
                if self.ramp_transfer.is_null() {
                    return;
                }
            }
            let mapped = SDL_MapGPUTransferBuffer(self.device, self.ramp_transfer, true) as *mut crate::scene::Vertex;
            if mapped.is_null() {
                return;
            }
            let mut regions = Vec::with_capacity(built.len());
            let mut at = 0;
            for (k, vertices) in &built {
                std::ptr::copy_nonoverlapping(vertices.as_ptr(), mapped.add(at), vertices.len());
                regions.push((at, self.scene.ramps[*k].base_vertex as usize, vertices.len()));
                at += vertices.len();
            }
            SDL_UnmapGPUTransferBuffer(self.device, self.ramp_transfer);
            let copy = SDL_BeginGPUCopyPass(command);
            for (from, to, count) in regions {
                let mut source: SDL_GPUTransferBufferLocation = zeroed();
                source.transfer_buffer = self.ramp_transfer;
                source.offset = (from * stride) as u32;
                let mut region: SDL_GPUBufferRegion = zeroed();
                region.buffer = self.vertex_buffer;
                region.offset = (to * stride) as u32;
                region.size = (count * stride) as u32;
                // the rest of the buffer stays: no cycling
                SDL_UploadToGPUBuffer(copy, &source, &region, false);
            }
            SDL_EndGPUCopyPass(copy);
        }
        for (k, want) in changed {
            self.ramp_shown[k] = want;
        }
    }

    fn update_animations(&mut self, command: *mut SDL_GPUCommandBuffer, input: &FrameInput) {
        if self.anim_transfer.is_null() {
            return;
        }
        let changed: Vec<(usize, f32)> = (self.scene.animations.iter().enumerate())
            .filter_map(|(k, a)| input.part_render.get(a.part as usize).and_then(|r| r.anim_frame).filter(|&f| f != self.anim_shown[k]).map(|f| (k, f)))
            .collect();
        if changed.is_empty() {
            return;
        }
        let stride = size_of::<crate::scene::Vertex>();
        unsafe {
            let mapped = SDL_MapGPUTransferBuffer(self.device, self.anim_transfer, true) as *mut crate::scene::Vertex;
            if mapped.is_null() {
                return;
            }
            let mut regions = Vec::with_capacity(changed.len());
            let mut at = 0;
            for &(k, frame) in &changed {
                let a = &self.scene.animations[k];
                let vertices = a.vertices(frame);
                std::ptr::copy_nonoverlapping(vertices.as_ptr(), mapped.add(at), vertices.len());
                regions.push((at, a.base_vertex as usize, vertices.len()));
                at += vertices.len();
                self.anim_shown[k] = frame;
            }
            SDL_UnmapGPUTransferBuffer(self.device, self.anim_transfer);
            let copy = SDL_BeginGPUCopyPass(command);
            for (from, to, count) in regions {
                let mut source: SDL_GPUTransferBufferLocation = zeroed();
                source.transfer_buffer = self.anim_transfer;
                source.offset = (from * stride) as u32;
                let mut region: SDL_GPUBufferRegion = zeroed();
                region.buffer = self.vertex_buffer;
                region.offset = (to * stride) as u32;
                region.size = (count * stride) as u32;
                // the rest of the buffer stays: no cycling
                SDL_UploadToGPUBuffer(copy, &source, &region, false);
            }
            SDL_EndGPUCopyPass(copy);
        }
    }

    /// Uploads the score display's frame when a new one comes, a dot a
    /// texel; the texture is made again when the frame's size changes.
    fn update_dmd(&mut self, command: *mut SDL_GPUCommandBuffer, frame: Option<&crate::dmd::DmdFrame>) {
        let Some(frame) = frame else { return };
        if self.dmd_id == Some(frame.id) || frame.width == 0 || frame.height == 0 || frame.dots.len() != (frame.width * frame.height) as usize {
            return;
        }
        let size = (frame.width, frame.height);
        unsafe {
            if self.dmd_size != size || self.dmd_texture.is_null() || self.dmd_transfer.is_null() {
                if !self.dmd_texture.is_null() {
                    SDL_ReleaseGPUTexture(self.device, self.dmd_texture);
                    self.dmd_texture = null_mut();
                }
                if !self.dmd_transfer.is_null() {
                    SDL_ReleaseGPUTransferBuffer(self.device, self.dmd_transfer);
                    self.dmd_transfer = null_mut();
                }
                match self.texture(SDL_GPU_TEXTUREFORMAT_R32G32B32A32_FLOAT, SDL_GPU_TEXTUREUSAGE_SAMPLER, size.0, size.1, 1, SDL_GPU_SAMPLECOUNT_1) {
                    Ok(t) => self.dmd_texture = t,
                    Err(e) => {
                        crate::script::log::once(&format!("no score display: {e}"));
                        return;
                    }
                }
                let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
                info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_UPLOAD;
                info.size = (frame.dots.len() * size_of::<[f32; 4]>()) as u32;
                self.dmd_transfer = SDL_CreateGPUTransferBuffer(self.device, &info);
                if self.dmd_transfer.is_null() {
                    return;
                }
                self.dmd_size = size;
            }
            let mapped = SDL_MapGPUTransferBuffer(self.device, self.dmd_transfer, true) as *mut [f32; 4];
            if mapped.is_null() {
                return;
            }
            std::ptr::copy_nonoverlapping(frame.dots.as_ptr(), mapped, frame.dots.len());
            SDL_UnmapGPUTransferBuffer(self.device, self.dmd_transfer);
            let copy = SDL_BeginGPUCopyPass(command);
            let mut source: SDL_GPUTextureTransferInfo = zeroed();
            source.transfer_buffer = self.dmd_transfer;
            source.pixels_per_row = size.0;
            source.rows_per_layer = size.1;
            let mut region: SDL_GPUTextureRegion = zeroed();
            region.texture = self.dmd_texture;
            region.w = size.0;
            region.h = size.1;
            region.d = 1;
            SDL_UploadToGPUTexture(copy, &source, &region, true);
            SDL_EndGPUCopyPass(copy);
            self.dmd_id = Some(frame.id);
        }
    }

    /// The score display's uniforms: VPX's `SetupDMDRender` for the legacy
    /// renderer. A brightness-only frame takes the tint; a coloured one
    /// keeps its colours (white).
    fn dmd_uniforms(&self, frame: &crate::dmd::DmdFrame, tint: [f32; 3], brightness: f32, alpha: f32) -> DmdUniforms {
        let t = if frame.luminance { tint } else { [1.0; 3] };
        DmdUniforms {
            tint: [t[0] * brightness, t[1] * brightness, t[2] * brightness, 0.0],
            res_alpha_time: [frame.width as f32, frame.height as f32, alpha, (self.frames_drawn.get() % 2048) as f32],
        }
    }

    /// A flasher's shader settings and its two pictures, as VPX's
    /// `Flasher::Render` sets them: its colour and opacity times its
    /// intensity, one picture (mode 0, B standing in for a missing A), two
    /// blended by its filter (mode 1) or none (mode 2), and the pictures'
    /// alpha tests unless it is additive.
    fn flasher_uniforms(&self, now: &crate::look::FlasherNow) -> (FlasherUniforms, [*mut SDL_GPUTexture; 2]) {
        let picture = |i: Option<u32>| i.and_then(|i| self.scene.table_images.get(i as usize).copied().flatten().map(|s| (i, self.textures[s])));
        let alpha_test = |i: u32| if now.add_blend { -1.0 } else { self.scene.image_alpha_test.get(i as usize).copied().unwrap_or(-1.0) };
        let (mode, textures, tests) = match (picture(now.image_a), picture(now.image_b)) {
            (Some((a, ta)), None) => (0.0, [ta, ta], [alpha_test(a), -1.0]),
            (None, Some((b, tb))) => (0.0, [tb, tb], [alpha_test(b), -1.0]),
            (Some((a, ta)), Some((b, tb))) => (1.0, [ta, tb], [alpha_test(a), alpha_test(b)]),
            (None, None) => (2.0, [self.white, self.white], [-1.0, -1.0]),
        };
        let c = |k: u32| ((now.color >> (8 * k)) & 0xFF) as f32 / 255.0;
        let uniforms = FlasherUniforms {
            static_color_alpha: [c(0), c(1), c(2), now.alpha * now.intensity_scale / 100.0],
            alpha_test_filter_add: [tests[0], tests[1], now.filter as f32, if now.add_blend { 1.0 } else { 0.0 }],
            // VPX keeps modulate vs add off 0 (it would switch the blend off) and off 1
            amount_mva_mode: [now.amount as f32 / 100.0, now.modulate_vs_add.clamp(0.00001, 0.9999), mode, 0.0],
            term: [0.0; 4],
        };
        (uniforms, textures)
    }

    /// Records a frame into `target`, through the multisampled targets.
    ///
    /// # Safety
    /// `command` must be an acquired command buffer and `target` a colour
    /// target of the swapchain's format and the given size.
    /// Which parts this frame draws: with the still parts drawn once, only
    /// the moving ones over their picture.
    fn stage(&self) -> Stage {
        if !self.view_changing && self.still.as_ref().is_some_and(|s| s.valid.get()) { Stage::Moving } else { Stage::Full }
    }

    unsafe fn record(&self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32, input: &FrameInput, stage: Stage) {
        unsafe {
            // the readout's vertices, then the pictures', uploaded before the render pass
            let mut overlay = overlay_vertices(input.text, input.text_turn, (width as f32, height as f32));
            let text_count = overlay.len().min(MAX_OVERLAY_CHARS * 6);
            overlay.truncate(text_count);
            // the table's size (supersampled, VPX's AAFactor), and the window's, for the passes after the tonemapping
            let (rw, rh) = self.render_size(width, height);
            let (w, h) = (rw as f32, rh as f32);
            let (out_w, out_h) = (width as f32, height as f32);
            // the nudge shake moves the flat pictures with the table; with the still parts drawn
            // once, the whole picture moves at the end instead, as VPX moves it
            let offset = if stage == Stage::Full { input.screen_offset } else { [0.0; 2] };
            let (dx, dy) = (offset[0] * w * 0.5, -offset[1] * h * 0.5);
            // the camera: shaken; unshaken; for a pass of the still parts, shifted by part of a texel
            let view_proj = match stage {
                Stage::Full => input.view_proj,
                Stage::Still([x, y]) => input.unshaken * Mat4::translate(x, y, 0.0),
                Stage::Moving => input.unshaken,
            };
            let still_pass = matches!(stage, Stage::Still(_));
            let still = self.still.as_ref();
            // the parts this frame draws: the still ones in their pass, the others each frame
            let drawn_now = |draw: &crate::scene::Draw| match stage {
                Stage::Full => true,
                Stage::Still(_) => draw.still,
                Stage::Moving => !draw.still,
            };
            // VPX's pictures without mipmaps in the still parts' passes
            let (part_sampler, backdrop_sampler) = match (still_pass, still) {
                (true, Some(s)) => (s.sampler, s.clamp_sampler),
                _ => (self.sampler, self.flat_sampler),
            };
            // VPX's look: a part's own way of reading its picture (`scene::PartFilter`): ramps and
            // decals trilinear; in the still passes, where every picture is read at full size, a
            // clamped or mirrored one clamped (the same within the half texel past its edge)
            let sampler_of = |draw: &crate::scene::Draw| {
                use crate::scene::PartFilter;
                match (still_pass, still, draw.filter) {
                    (_, _, PartFilter::Default) => part_sampler,
                    (true, Some(s), PartFilter::Trilinear) => s.sampler,
                    (true, Some(s), _) => s.clamp_sampler,
                    (_, _, PartFilter::Trilinear) => self.trilinear[0],
                    (_, _, PartFilter::TrilinearClamp) => self.trilinear[1],
                    (_, _, PartFilter::TrilinearMirror) => self.trilinear[2],
                }
            };
            let backdrop = self.scene.backdrop;
            let background = backdrop.image.map(|image| {
                let first = overlay.len() as u32;
                // VPX draws it without blending (DrawBackground) and, in 5436's BGFX build, with
                // its sprite shader's alpha test off: a see-through texel shows the colour stored
                // under it, as VPX's pictures show Special Force's display windows
                let c = backdrop.intensity;
                overlay.extend(quad([dx, dy, w + dx, h + dy], [0.0, 0.0, 1.0, 1.0], [c, c, c, -1.0]));
                (self.textures[image], first)
            });
            // VPX's sprites (reels, text boxes; the last flag marks a reel), drawn with the backdrop's
            // lights in VPX's order
            let mut sprite_draws = Vec::new();
            for look in &self.scene.reels {
                let shown = input.part_render.get(look.part as usize).is_none_or(|r| r.visible);
                let Some(values) = input.reels.iter().find(|r| r.part == look.part) else { continue };
                if !shown {
                    continue;
                }
                // the picture a script set, else the reel's own; none, nothing drawn (VPX's DispReel::Render)
                let image = match values.image {
                    Some(table_image) => table_image.and_then(|i| self.scene.table_images.get(i as usize).copied().flatten()),
                    None => Some(look.image),
                };
                let Some(image) = image else { continue };
                let first = overlay.len() as u32;
                let mut x = look.x;
                for &digit in &values.digits {
                    if let Some(&uv) = look.digits.get(usize::from(digit)) {
                        let rect = [x * w + dx, look.y * h + dy, (x + look.width) * w + dx, (look.y + look.height) * h + dy];
                        // VPX's reels: alpha test at 128/255
                        overlay.extend(quad(rect, uv, [1.0, 1.0, 1.0, 128.0 / 255.0]));
                    }
                    x += look.spacing_x + look.width;
                }
                sprite_draws.push((look.part, self.textures[image], first, overlay.len() as u32 - first, None, true));
            }
            for (k, look) in self.scene.text_boxes.iter().enumerate() {
                let Some(now) = input.text_boxes.iter().find(|b| b.part == look.part) else { continue };
                let [l, t, r, b] = now.rect;
                let rect = [l / 1000.0 * w + dx, t / 750.0 * h + dy, r / 1000.0 * w + dx, b / 750.0 * h + dy];
                if now.display {
                    // the score display in the box (VPX's Textbox::Render with is_dmd): tinted
                    // by its font colour times its intensity scale, drawn over what's below
                    let (Some(frame), false, true) = (input.dmd, self.dmd_texture.is_null(), now.visible) else { continue };
                    let c = |k: u32| ((now.font_color >> (8 * k)) & 0xFF) as f32 / 255.0;
                    let uniforms = self.dmd_uniforms(frame, [c(0), c(1), c(2)], now.intensity, 1.0);
                    let first = overlay.len() as u32;
                    overlay.extend(quad(rect, [0.0, 0.0, 1.0, 1.0], [1.0; 4]));
                    sprite_draws.push((look.part, self.dmd_texture, first, 6, Some(uniforms), false));
                    continue;
                }
                let texture = self.text_box_textures[k].0;
                if !now.visible || texture.is_null() {
                    continue;
                }
                let first = overlay.len() as u32;
                // VPX: the box's intensity scale, alpha test at 128/255
                let c = now.intensity;
                overlay.extend(quad(rect, [0.0, 0.0, 1.0, 1.0], [c, c, c, 128.0 / 255.0]));
                sprite_draws.push((look.part, texture, first, 6, None, false));
            }
            sprite_draws.sort_by_key(|d| d.0);
            // the backdrop's lights: their outlines on VPX's grid, in pixels
            // (x * width / 1000 - 0.5), each vertex's colour carrying its place
            // before the shake, for the glow
            let (xmult, ymult) = (w / 1000.0, h / 750.0);
            let mut light_draws = Vec::new();
            for (k, look) in self.scene.backdrop_lights.iter().enumerate() {
                if !input.part_render.get(look.part as usize).is_none_or(|r| r.visible) {
                    continue;
                }
                let first = overlay.len() as u32;
                for &i in &look.triangles {
                    let [x, y] = look.points[i as usize];
                    let (px, py) = (x * xmult - 0.5, y * ymult - 0.5);
                    overlay.push(OverlayVertex { position: [px + dx, py + dy], uv: [x / 1000.0, y / 750.0], color: [px, py, 0.0, 0.0] });
                }
                light_draws.push((k, first, overlay.len() as u32 - first));
            }
            let overlay_count = overlay.len().min(MAX_OVERLAY_CHARS * 6);
            if overlay_count > 0 {
                let mapped = SDL_MapGPUTransferBuffer(self.device, self.overlay_transfer, true) as *mut OverlayVertex;
                if !mapped.is_null() {
                    std::ptr::copy_nonoverlapping(overlay.as_ptr(), mapped, overlay_count);
                    SDL_UnmapGPUTransferBuffer(self.device, self.overlay_transfer);
                    let copy = SDL_BeginGPUCopyPass(command);
                    let mut source: SDL_GPUTransferBufferLocation = zeroed();
                    source.transfer_buffer = self.overlay_transfer;
                    let mut region: SDL_GPUBufferRegion = zeroed();
                    region.buffer = self.overlay_buffer;
                    region.size = (overlay_count * size_of::<OverlayVertex>()) as u32;
                    SDL_UploadToGPUBuffer(copy, &source, &region, true);
                    SDL_EndGPUCopyPass(copy);
                }
            }

            let mut color: SDL_GPUColorTargetInfo = zeroed();
            color.texture = self.msaa;
            // the backdrop's colour: VPX clears its linear picture with the
            // stored values as they are (Renderer::DrawBackground), so they
            // show brighter than the colour picked. Build 5436's BGFX path
            // also swaps red and blue there (RenderCommand's RC_CLEAR); its
            // DirectX 9 and OpenGL paths, which tables were made with, don't
            let [r, g, b] = backdrop.color;
            // VPX's look: the table into its half-float picture, nothing marked for tonemapping yet
            let scene_target = if self.hdr { self.hdr_target } else { target };
            color.clear_color = SDL_FColor { r, g, b, a: if self.hdr { 0.0 } else { 1.0 } };
            color.load_op = SDL_GPU_LOADOP_CLEAR;
            color.store_op = SDL_GPU_STOREOP_RESOLVE;
            color.resolve_texture = scene_target;
            // without multisampling, straight into the target
            if self.samples == SDL_GPU_SAMPLECOUNT_1 {
                color.texture = scene_target;
                color.store_op = SDL_GPU_STOREOP_STORE;
                color.resolve_texture = null_mut();
            }
            let mut depth: SDL_GPUDepthStencilTargetInfo = zeroed();
            depth.texture = self.depth;
            depth.clear_depth = 1.0;
            depth.load_op = SDL_GPU_LOADOP_CLEAR;
            depth.store_op = SDL_GPU_STOREOP_DONT_CARE;
            depth.stencil_load_op = SDL_GPU_LOADOP_DONT_CARE;
            depth.stencil_store_op = SDL_GPU_STOREOP_DONT_CARE;
            // the table's own pass: a pass of the still parts into their picture, kept with its depth
            let mut main_depth = depth;
            if let (true, Some(s)) = (still_pass, still) {
                if self.samples == SDL_GPU_SAMPLECOUNT_1 {
                    color.texture = s.scratch;
                    color.store_op = SDL_GPU_STOREOP_STORE;
                    color.resolve_texture = null_mut();
                } else {
                    // multisampled, resolved into the pass's picture; the depth kept multisampled
                    color.resolve_texture = s.scratch;
                }
                main_depth.texture = s.depth;
                main_depth.store_op = SDL_GPU_STOREOP_STORE;
            }
            // multisampled, a frame over the still parts loads their depth, copied over before its pass
            if stage == Stage::Moving && still.is_some() && self.samples != SDL_GPU_SAMPLECOUNT_1 {
                main_depth.load_op = SDL_GPU_LOADOP_LOAD;
            }
            // the post-processing passes, the occlusion and the motion blur read the frame's depth
            if !still_pass && (self.post_reads_depth() || self.ao_mode() == 2 || self.motion_blur_on()) {
                main_depth.store_op = SDL_GPU_STOREOP_STORE;
            }

            // VPX's camera-relative projection (vs_basic.sc, ModelViewProj::Update): each point
            // less the camera's place, then the view's turning, scaling and layback, the projection
            // and the frame's shift; the numbers stay small, so surfaces at the same depth tie as
            // they do in VPX
            let post = match stage {
                Stage::Full => input.shake,
                Stage::Still([x, y]) => Mat4::translate(x, y, 0.0),
                Stage::Moving => Mat4::IDENTITY,
            };
            let frame_for = |view: Mat4, eye: Vec3, clip: [f32; 4]| {
                let camera = view.inverse().position();
                FrameUniforms {
                    rot_view_proj: (view.rotation_part() * input.projection * post).m,
                    camera: [camera.x, camera.y, camera.z, 0.0],
                    eye: [eye.x, eye.y, eye.z, 1.0],
                    clip,
                }
            };
            let frame = frame_for(input.view, input.eye, [0.0, 0.0, 0.0, 1.0]);
            let light_now = |l: usize| input.lights.get(l).copied().unwrap_or_else(|| crate::scene::LightNow::at_start(&self.scene.light_parts[l]));
            let model_of = |draw: &crate::scene::Draw| {
                // a bumper's ring and skirt, as its animation has them
                if let Some(piece) = draw.bumper
                    && let Some([ring, rot_x, rot_y]) = draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.bumper)
                {
                    return piece.transform(ring, [rot_x, rot_y]);
                }
                draw.group
                    .and_then(|g| input.group_transforms.get(g).copied())
                    .or_else(|| draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.transform))
                    .unwrap_or(Mat4::IDENTITY)
            };
            // a wall's sides keep the table's setting (or the script's SideVisible); everything
            // else follows the part's Visible
            let shown = |draw: &crate::scene::Draw| {
                let render = draw.part.and_then(|p| input.part_render.get(p as usize));
                // a light's bulb and socket, while its ShowBulbMesh is off
                if draw.bulb_piece.is_some() && render.and_then(|r| r.light_live).is_some_and(|l| !l.bulb_mesh) {
                    return false;
                }
                // a plunger's frame, while its position picks it (VPX's Render)
                if let Some((k, count)) = draw.frame
                    && crate::shapes::plunger::frame_of(render.and_then(|r| r.plunger).unwrap_or(0.0), count as usize) != k as usize
                {
                    return false;
                }
                // a bumper's base, skirt, ring or cap, as the script left it (VPX's Render)
                if let (Some(k), Some(shown)) = (draw.bumper_piece, render.and_then(|r| r.bumper_shown))
                    && !shown[k as usize]
                {
                    return false;
                }
                match render {
                    Some(r) if !draw.wall_side => r.visible,
                    Some(r) => r.look.and_then(|l| l.side_visible).unwrap_or(draw.visible),
                    None => draw.visible,
                }
            };
            // VPX's look: the part drawn bent through a refraction probe this pass, and the probes
            // copied so far (each once, before the first part showing it); a pass they split keeps
            // what it drew for its continuation
            let refraction_of = |draw: &crate::scene::Draw| {
                self.refraction_shown(draw, input).filter(|&r| self.hdr && self.refraction_textures.get(r).is_some_and(|t| !t.is_null()))
            };
            let copied: Vec<std::cell::Cell<bool>> = (0..self.scene.refractions.len()).map(|_| std::cell::Cell::new(false)).collect();
            if self.scene.draws.iter().any(|d| shown(d) && refraction_of(d).is_some()) {
                if color.store_op == SDL_GPU_STOREOP_RESOLVE {
                    color.store_op = SDL_GPU_STOREOP_RESOLVE_AND_STORE;
                }
                main_depth.store_op = SDL_GPU_STOREOP_STORE;
            }
            let mut vb: SDL_GPUBufferBinding = zeroed();
            vb.buffer = self.vertex_buffer;
            let mut ib: SDL_GPUBufferBinding = zeroed();
            ib.buffer = self.index_buffer;

            // VPX's ball shadows (RenderDynamics' balls): the balls' places, none in a pass of the still
            // parts (drawn before any ball), and for each draw the light whose shadows fall on it
            let mut balls = [[-1000.0, -1000.0, -1000.0, 0.0f32]; 8];
            if !still_pass {
                for (slot, b) in balls.iter_mut().zip(input.balls) {
                    *slot = [b.position.x, b.position.y, b.position.z, b.radius];
                }
            }
            let shadow_now: std::cell::Cell<Option<Option<usize>>> = std::cell::Cell::new(None);
            let shadow = |light: Option<usize>| {
                let light = light.filter(|&l| self.scene.light_parts.get(l).is_some_and(|i| i.ball_shadows));
                if shadow_now.get() == Some(light) {
                    return;
                }
                shadow_now.set(Some(light));
                // where it shines from (Light::GetCurrentHeight: its surface plus its height)
                let place = light.map_or([0.0; 4], |l| {
                    let i = &self.scene.light_parts[l];
                    [i.center.x, i.center.y, i.center.z - 0.1 + i.height, 1.0]
                });
                let u = ShadowUniforms { light: place, balls };
                SDL_PushGPUFragmentUniformData(command, 2, (&raw const u).cast(), size_of::<ShadowUniforms>() as u32);
            };
            shadow(None);

            // VPX's look: the bulb light buffer (Renderer::DrawBulbLightBuffer), drawn first into the
            // small picture: the halos of the bulbs that pass light up, at their transmission scale,
            // only adding, without depth, then VPX's 19-texel blur; see-through parts add it
            let mut transmission = false;
            if self.hdr && !self.bloom[0].is_null() && !still_pass {
                let lit: Vec<(&crate::scene::Draw, usize)> = self
                    .scene
                    .draws
                    .iter()
                    .filter_map(|d| {
                        let l = d.glow?;
                        let info = &self.scene.light_parts[l];
                        let now = light_now(l);
                        let dark = now.intensity == 0.0 || (now.color == [0.0; 3] && now.color2 == [0.0; 3]);
                        (info.bulb && info.transmission > 0.0 && shown(d) && !dark).then_some((d, l))
                    })
                    .collect();
                // VPX draws them, and blurs them, only over the screen's area of interest (Light::Render
                // in its light buffer pass: every bulb passing light up bounds its falloff around its
                // centre, at its halo's height once placed; DrawBulbLightBuffer: the blur 0.05 wider;
                // RenderPass::Execute: a scissor); outside it the light buffer stays dark, the blur's
                // other picture keeping what it held (the last frame's bloom)
                let mut area: Option<[f32; 4]> = None;
                {
                    let raised = self.halo_raised.borrow();
                    for (l, info) in self.scene.light_parts.iter().enumerate() {
                        if !info.bulb || info.transmission == 0.0 {
                            continue;
                        }
                        let z = info.center.z + if raised[l] { info.halo_height } else { 0.0 };
                        let r = info.falloff_radius;
                        let (mut x0, mut y0, mut x1, mut y1) = (1.0f32, 1.0f32, -1.0f32, -1.0f32);
                        for i in 0..4 {
                            let x = info.center.x + if i & 1 != 0 { -r } else { r };
                            let y = info.center.y + if i & 2 != 0 { -r } else { r };
                            let m = &view_proj.m;
                            let xp = m[0][0] * x + m[1][0] * y + m[2][0] * z + m[3][0];
                            let yp = m[0][1] * x + m[1][1] * y + m[2][1] * z + m[3][1];
                            let wp = m[0][3] * x + m[1][3] * y + m[2][3] * z + m[3][3];
                            if wp > 0.0 {
                                (x0, y0, x1, y1) = (x0.min(xp / wp), y0.min(yp / wp), x1.max(xp / wp), y1.max(yp / wp));
                            } else {
                                (x0, y0, x1, y1) = (-1.0, -1.0, 1.0, 1.0);
                                break;
                            }
                        }
                        area = Some(area.map_or([x0, y0, x1, y1], |a| [a[0].min(x0), a[1].min(y0), a[2].max(x1), a[3].max(y1)]));
                    }
                }
                // the area as a scissor on the light buffer, pixels from the top; None when it's empty
                let (bw, bh) = self.bloom_size;
                let scissor = |a: [f32; 4]| {
                    let at = |v: f32, size: u32| (((0.5 + v * 0.5) * size as f32) as i32).clamp(0, size as i32);
                    let (left, bottom, right, top) = (at(a[0], bw), at(a[1], bh), at(a[2], bw), at(a[3], bh));
                    (left != right && bottom != top).then_some(SDL_Rect { x: left, y: bh as i32 - top, w: right - left, h: top - bottom })
                };
                let mut target: SDL_GPUColorTargetInfo = zeroed();
                target.texture = self.bloom[0];
                target.load_op = SDL_GPU_LOADOP_CLEAR;
                target.store_op = SDL_GPU_STOREOP_STORE;
                let below = SDL_BeginGPURenderPass(command, &target, 1, null());
                // (VPX skips a pass whose area is empty)
                let drawn = area.is_none_or(|a| scissor(a).is_some());
                if let Some(rect) = area.and_then(scissor) {
                    SDL_SetGPUScissor(below, &rect);
                }
                if !lit.is_empty() && drawn {
                    SDL_BindGPUGraphicsPipeline(below, self.light_buffer);
                    SDL_BindGPUVertexBuffers(below, 0, &vb, 1);
                    SDL_BindGPUIndexBuffer(below, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                    SDL_PushGPUVertexUniformData(command, 0, (&raw const frame).cast(), size_of::<FrameUniforms>() as u32);
                    let binding = SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler };
                    SDL_BindGPUFragmentSamplers(below, 0, &binding, 1);
                    for &(draw, l) in &lit {
                        let info = &self.scene.light_parts[l];
                        let now = light_now(l);
                        let surface = info.center.z - 0.1;
                        shadow(Some(l));
                        let model = Mat4::translate(0.0, 0.0, info.halo_height) * model_of(draw);
                        SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                        let uniforms = MaterialUniforms {
                            base_color: [now.color[0], now.color[1], now.color[2], now.intensity * info.transmission],
                            emissive: [now.color2[0], now.color2[1], now.color2[2], info.falloff_power],
                            params: [info.center.x, info.center.y, surface + info.height, 1.0 / info.falloff_radius],
                            light0: [0.0001, 2.0, 0.0, 0.0],
                            light1: [0.0; 4],
                            extra: [0.0; 4],
                            static_color_alpha: [1.0; 4],
                        };
                        SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                        SDL_DrawGPUIndexedPrimitives(below, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    }
                }
                SDL_EndGPURenderPass(below);
                // the halos drawn are placed at their height from now on (UpdateMeshBuffer)
                {
                    let mut raised = self.halo_raised.borrow_mut();
                    for &(_, l) in &lit {
                        raised[l] = true;
                    }
                }
                if !lit.is_empty() {
                    let small = [1.0 / bw as f32, 1.0 / bh as f32, 1.0, 0.0];
                    let blur_rect = area.map(|a| scissor([a[0] - 0.05, a[1] - 0.05, a[2] + 0.05, a[3] + 0.05]));
                    for (pipeline, source, dest) in [(self.blur19_h, self.bloom[0], self.bloom[1]), (self.blur19_v, self.bloom[1], self.bloom[0])] {
                        // (VPX skips a pass whose area is empty)
                        if matches!(blur_rect, Some(None)) {
                            continue;
                        }
                        let mut target: SDL_GPUColorTargetInfo = zeroed();
                        target.texture = dest;
                        target.load_op = if blur_rect.is_some() { SDL_GPU_LOADOP_LOAD } else { SDL_GPU_LOADOP_DONT_CARE };
                        target.store_op = SDL_GPU_STOREOP_STORE;
                        let blur = SDL_BeginGPURenderPass(command, &target, 1, null());
                        if let Some(Some(rect)) = blur_rect {
                            SDL_SetGPUScissor(blur, &rect);
                        }
                        SDL_BindGPUGraphicsPipeline(blur, pipeline);
                        let binding = SDL_GPUTextureSamplerBinding { texture: source, sampler: self.flat_sampler };
                        SDL_BindGPUFragmentSamplers(blur, 0, &binding, 1);
                        SDL_PushGPUFragmentUniformData(command, 0, small.as_ptr().cast(), 16);
                        SDL_DrawGPUPrimitives(blur, 3, 1, 0, 0);
                        SDL_EndGPURenderPass(blur);
                    }
                    transmission = true;
                }
            }

            let [l0, l1] = self.scene.lights;
            let lights = ([l0.x, l0.y, l0.z, 1.0], [l1.x, l1.y, l1.z, 1.0]);
            // VPX's look: solid parts mark their pixels for tonemapping
            let solid = if self.hdr { 1.0 } else { 0.0 };
            let mesh_look = |draw: &crate::scene::Draw| self.mesh_look(draw, input);
            // VPX's look: the mirrors drawn this frame (VPX draws a reflection probe when a part
            // showing it is drawn): those a shown part shows, mirroring something
            // With the still parts drawn once (VPX's RenderProbe with its static and dynamic masks): their
            // passes mirror the still parts, for the still parts showing a mirror of still parts (the
            // moving ones and the balls come each frame), and a mirror of everything has its still parts
            // drawn there for itself (PreRenderStaticReflectionProbe); each frame mirrors what moves, a
            // mirror of everything over its still parts' picture
            let mode_of = |k: usize| self.reflection_mode(k);
            let rendered: Vec<bool> = (0..self.scene.mirrors.len())
                .map(|k| {
                    let shows = |still_only: bool| self.scene.draws.iter().any(|d| self.mirror_shown(d, input).is_some_and(|(m, _)| m == k) && shown(d) && (!still_only || d.still));
                    self.mirror_textures.get(k).is_some_and(|t| !t.is_null())
                        && match stage {
                            Stage::Full => shows(false),
                            Stage::Still(_) if mode_of(k) == 5 => still.is_some_and(|s| s.mirror_color.get(k).is_some_and(|t| !t.is_null())),
                            Stage::Still(_) => matches!(mode_of(k), 2..=4) && shows(true),
                            Stage::Moving => matches!(mode_of(k), 1 | 3..=5) && shows(false),
                        }
                })
                .collect();
            let live: Vec<bool> = (0..rendered.len()).map(|k| rendered[k] && !(still_pass && mode_of(k) == 5)).collect();
            let mirror_of = |draw: &crate::scene::Draw| self.mirror_shown(draw, input).filter(|&(k, _)| live[k]);
            // a part (VPX's look: with the mirror it shows, if any)
            let draw_one = |pass: *mut SDL_GPURenderPass, draw: &crate::scene::Draw, look: &MeshLook, model: &Mat4, mirror: Option<(usize, f32)>| {
                // a light map's light's ball shadows
                shadow(draw.lightmap);
                let m = &self.scene.materials[draw.material];
                let mut emissive = m.emissive;
                if let Some(l) = draw.bulb {
                    let now = light_now(l);
                    let k = (now.intensity * 0.1).min(1.5);
                    emissive = [now.color[0] * k, now.color[1] * k, now.color[2] * k];
                }
                SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                // see-through parts blend by their alpha (those writing depth mark their pixels
                // through their blend), so only solid ones set the mark here
                let mark = if look.blend || look.added { 0.0 } else { solid };
                if self.hdr && !look.added {
                    // VPX's look: VPX's basic material (Shader::SetMaterial)
                    let sh = &look.shading;
                    let alpha = if sh.opacity_active { sh.opacity } else { 1.0 };
                    let glossy = if sh.metal { [0.0; 4] } else { [sh.glossy[0], sh.glossy[1], sh.glossy[2], sh.glossy_image_lerp] };
                    // the table's normal map, or the one a script set (VPX's Primitive::put_NormalMap: none,
                    // or a picture the table prepared as a normal map)
                    let script_normal = draw.part.and_then(|p| input.part_render.get(p as usize)).and_then(|r| r.look.as_ref()).and_then(|l| l.normal_map);
                    let normal_slot = match script_normal {
                        Some(table_image) => table_image.and_then(|i| self.scene.table_images.get(i as usize).copied().flatten()),
                        None => draw.normal_map,
                    };
                    let normal_map = normal_slot.and_then(|i| self.normal_textures.get(i).copied()).unwrap_or(null_mut());
                    // the mirror's picture, and its plane's normal with the part's strength: the normal
                    // turned as VPX turns it, by the part's own matrix's inverse transpose (a
                    // primitive's rows, then where a script moved it; Primitive::Render)
                    let (reflection, mirror_normal_factor) = match mirror {
                        Some((k, strength)) => {
                            let [x, y, z, _] = self.scene.mirrors[k].plane;
                            let mut n = [x, y, z];
                            if let Some(rows) = draw.normal_rows {
                                n = [0, 1, 2].map(|j| x * rows[0][j] + y * rows[1][j] + z * rows[2][j]);
                            }
                            if *model != Mat4::IDENTITY {
                                let inv = model.inverse();
                                n = [0, 1, 2].map(|j| n[0] * inv.m[j][0] + n[1] * inv.m[j][1] + n[2] * inv.m[j][2]);
                            }
                            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(f32::MIN_POSITIVE);
                            (self.mirror_textures[k], [n[0] / len, n[1] / len, n[2] / len, strength])
                        }
                        None => (self.white, [0.0; 4]),
                    };
                    let uniforms = VpxMaterialUniforms {
                        base_alpha: [sh.base[0], sh.base[1], sh.base[2], alpha],
                        glossy_image_lerp: glossy,
                        clearcoat_edge_alpha: [sh.clearcoat[0], sh.clearcoat[1], sh.clearcoat[2], sh.edge_alpha],
                        roughness_wrap_edge_thickness: [(10.0 * sh.roughness + 1.0).exp2(), sh.wrap_lighting, sh.edge, sh.thickness],
                        disable_lighting_top_below: [look.disable_lighting, look.disable_lighting_below, if draw.degenerate { 1.0 } else { 0.0 }, 0.0],
                        static_color_alpha: look.static_color_alpha,
                        flags: [if sh.metal { 1.0 } else { 0.0 }, if look.textured { 1.0 } else { 0.0 }, look.alpha_cutoff, mark],
                        mirror_normal_factor,
                        normal_flags: [if normal_map.is_null() { 0.0 } else { 1.0 }, if draw.object_normal.is_some() { 1.0 } else { 0.0 }, 0.0, 0.0],
                        object_normal: draw.object_normal.map_or([[0.0; 4]; 3], |rows| rows.map(|[x, y, z]| [x, y, z, 0.0])),
                    };
                    SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<VpxMaterialUniforms>() as u32);
                    let env = if self.env_texture.is_null() { self.white } else { self.env_texture };
                    let below = if transmission { self.bloom[0] } else { self.white };
                    let bindings = [
                        SDL_GPUTextureSamplerBinding { texture: look.texture, sampler: sampler_of(draw) },
                        SDL_GPUTextureSamplerBinding { texture: env, sampler: self.env_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.irradiance, sampler: self.env_sampler },
                        SDL_GPUTextureSamplerBinding { texture: below, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: reflection, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: if normal_map.is_null() { self.white } else { normal_map }, sampler: part_sampler },
                    ];
                    SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 6);
                } else {
                    let uniforms = MaterialUniforms {
                        base_color: look.base_color,
                        emissive: [emissive[0], emissive[1], emissive[2], if look.textured { 1.0 } else { 0.0 }],
                        params: [look.alpha_cutoff, look.metallic, look.roughness, 0.0],
                        light0: lights.0,
                        light1: lights.1,
                        extra: [look.disable_lighting, mark, if draw.degenerate { 1.0 } else { 0.0 }, 0.0],
                        static_color_alpha: look.static_color_alpha,
                    };
                    SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                    let binding = SDL_GPUTextureSamplerBinding { texture: look.texture, sampler: part_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                }
                SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
            };
            // VPX's look: the table's light for its material, once a frame, with or without the
            // bulb light buffer's light from below
            let scene_light = |lit_below: bool, view: &Mat4| {
                if self.hdr {
                    let [l0, l1] = self.scene.lights;
                    let e = self.tone.light_emission;
                    let light = VpxSceneUniforms {
                        env_emission_height: [self.tone.env_emission, self.env_height, 0.0, 0.0],
                        // VPX 5436 sets these for its ball shader only: the basic shader's stay zero, so
                        // the point lights add nothing to parts (measured: VPX's pictures match this)
                        ambient_range: [0.0; 4],
                        light_pos: [[l0.x, l0.y, l0.z, 1.0], [l1.x, l1.y, l1.z, 1.0]],
                        light_emission: [[e[0], e[1], e[2], 1.0]; 2],
                        w_h_transmission: [1.0 / w, 1.0 / h, if transmission && lit_below { 1.0 } else { 0.0 }, 0.0],
                        view_rows: [0, 1, 2].map(|i| [view.m[i][0], view.m[i][1], view.m[i][2], 0.0]),
                    };
                    SDL_PushGPUFragmentUniformData(command, 1, (&raw const light).cast(), size_of::<VpxSceneUniforms>() as u32);
                }
            };
            // VPX's look: the balls (VPX's ball, its nearest lights and its mirror of the playfield),
            // seen through the camera's view, or a mirror's (where VPX leaves out a ball off the
            // playfield: on a ramp, or below it)
            let (ball_first, ball_count, ball_base) = self.scene.ball;
            let draw_balls = |pass: *mut SDL_GPURenderPass, view: Mat4, view_proj: Mat4, mirrored: bool| {
                let balls: Vec<&BallDraw> = input
                    .balls
                    .iter()
                    // VPX's Ball::Render: in the reflection, a ball whose reflection is on, and only on
                    // the playfield unless forced
                    .filter(|b| !mirrored || (b.reflected && (b.force_reflection || (!b.locked && b.position.z <= b.radius + 3.0 && b.position.z >= b.radius - 0.1))))
                    .collect();
                if balls.is_empty() {
                    return;
                }
                SDL_BindGPUGraphicsPipeline(pass, self.vpx_ball);
                SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                let prev = self.prev_valid.get();
                let ([l0, l1], e) = (self.scene.lights, self.tone.light_emission);
                // a table image as decoded, if it was
                let slot = |t: u32| self.scene.table_images.get(t as usize).copied().flatten().filter(|&s| s < self.textures.len());
                for ball in balls {
                    // the ball's own pictures (VPX's Ball::Render): the room it mirrors (none: VPX's
                    // own, equirectangular; the table's as uploaded, light past white kept) and its decal
                    let (env, spherical) = match ball.image.map(slot) {
                        // the player's own picture, for every ball, read as a sphere map
                        _ if !self.ball_override_env.is_null() => (self.ball_override_env, true),
                        None => (self.vpx_ball_env, false),
                        Some(Some(s)) if Some(s) != self.tone.ball_image => (self.textures[s], ball.spherical),
                        Some(_) => (self.ball_env, ball.spherical && self.tone.ball_image.is_some()),
                    };
                    let decal = if self.ball_override_decal.is_null() { ball.decal.and_then(slot).map_or(self.no_decal, |s| self.textures[s]) } else { self.ball_override_decal };
                    let bindings = [
                        SDL_GPUTextureSamplerBinding { texture: env, sampler: self.env_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.irradiance, sampler: self.env_sampler },
                        SDL_GPUTextureSamplerBinding { texture: decal, sampler: self.sampler },
                        SDL_GPUTextureSamplerBinding { texture: if prev { self.prev_hdr } else { self.white }, sampler: self.flat_sampler },
                    ];
                    SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 5);
                    // VPX's Ball::Render: the eight nearest lights that shine on balls
                    let mut near: Vec<(f32, usize)> = self
                        .scene
                        .light_parts
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.on_ball)
                        .map(|(k, l)| {
                            let surface = l.center.z - 0.1;
                            let (dx, dy, dz) = (l.center.x - ball.position.x, l.center.y - ball.position.y, l.mesh_radius + surface - ball.position.z);
                            (dx * dx + dy * dy + dz * dz, k)
                        })
                        .collect();
                    near.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let mut light_pos = [[-100_000.0, -100_000.0, -100_000.0, 1.0]; 10];
                    let mut light_emission = [[0.0f32; 4]; 10];
                    (light_pos[0], light_pos[1]) = ([l0.x, l0.y, l0.z, 1.0], [l1.x, l1.y, l1.z, 1.0]);
                    (light_emission[0], light_emission[1]) = ([e[0], e[1], e[2], 1.0], [e[0], e[1], e[2], 1.0]);
                    for (slot, &(_, k)) in near.iter().take(8).enumerate() {
                        let l = &self.scene.light_parts[k];
                        let now = light_now(k);
                        let surface = l.center.z - 0.1;
                        light_pos[2 + slot] = [l.center.x, l.center.y, surface + l.height, 1.0];
                        // VPX's map_bulblight_to_emission, times the ball's bulb intensity scale
                        let c = now.intensity * (l.falloff_radius * 0.6).powf(l.falloff_power * 0.6).clamp(0.0, 23000.0) * ball.bulb_scale;
                        light_emission[2 + slot] = [now.color[0] * c, now.color[1] * c, now.color[2] * c, 1.0];
                    }
                    // VPX's hack: rougher the farther the nearest light, for a bulb's size
                    let roughness = near.first().map_or(0.8, |&(d2, _)| (d2.sqrt() * 0.006).clamp(0.4, 0.8));
                    // VPX's Ball::Render: turned as it has rolled, sized and placed (its orientation is
                    // the physics' turn, transposed into VPX's row-vector matrix)
                    let o = ball.orientation;
                    let turn = Mat4 { m: [[o[0][0], o[1][0], o[2][0], 0.0], [o[0][1], o[1][1], o[2][1], 0.0], [o[0][2], o[1][2], o[2][2], 0.0], [0.0, 0.0, 0.0, 1.0]] };
                    let [ax, ay] = if self.ball_anti_stretch { anti_stretch(&view_proj, ball.position, ball.radius, w, h, input.view_rotation) } else { [1.0, 1.0] };
                    let model = turn * Mat4::scale(ball.radius * ax, ball.radius * ay, ball.radius * ay) * Mat4::translate(ball.position.x, ball.position.y, ball.position.z);
                    let uniforms = VpxBallUniforms {
                        base_alpha: [ball.color[0], ball.color[1], ball.color[2], 1.0],
                        roughness_wrap_edge_thickness: [(10.0 * roughness + 1.0).exp2(), 0.0, 1.0, 0.05],
                        ambient_range: [self.tone.ambient[0], self.tone.ambient[1], self.tone.ambient[2], self.tone.light_range],
                        // VPX: the table's strength times the ball's
                        inv_table_res_reflection: [self.tone.ball_inv_table[0], self.tone.ball_inv_table[1], (self.tone.ball_reflection * ball.reflection).clamp(0.0, 1.0), 0.0],
                        w_h_disable_lighting: [1.5 / w, 1.5 / h, if self.ball_lighting_off { 1.0 } else { 0.0 }, if prev { 1.0 } else { 0.0 }],
                        flags: [if spherical { 1.0 } else { 0.0 }, if ball.decal_mode { 1.0 } else { 0.0 }, 0.0, 0.0],
                        view: view.m,
                        view_proj: view_proj.m,
                        light_pos,
                        light_emission,
                    };
                    SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                    SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<VpxBallUniforms>() as u32);
                    SDL_DrawGPUIndexedPrimitives(pass, ball_count, 1, ball_first, ball_base, 0);
                }
            };
            // see-through parts and flashers in VPX's order; in the main picture, a see-through
            // part showing a mirror gets it added over it in a pass of its own (VPX's
            // Primitive::Render: an additive part shows none)
            // VPX's refraction (RenderProbe::RenderScreenSpaceTransparency, split before the first part
            // showing a probe in RenderFrame's sorted order): the table as drawn so far copied into the
            // probe's picture, its depth too, blurred by the probe's roughness; then the pass goes on
            let split_for = |pass: *mut SDL_GPURenderPass, r: usize| -> *mut SDL_GPURenderPass {
                SDL_EndGPURenderPass(pass);
                if self.samples == SDL_GPU_SAMPLECOUNT_1 {
                    self.copy_for_refraction(command, color.texture, main_depth.texture, r);
                } else {
                    // multisampled: the picture as resolved when the pass stopped (its depth can't be read)
                    let copy = SDL_BeginGPUCopyPass(command);
                    let mut from: SDL_GPUTextureLocation = zeroed();
                    from.texture = color.resolve_texture;
                    let mut to: SDL_GPUTextureLocation = zeroed();
                    to.texture = self.refraction_textures[r];
                    SDL_CopyGPUTextureToTexture(copy, &from, &to, rw, rh, 1, false);
                    SDL_EndGPUCopyPass(copy);
                }
                if let (Some(size), false) = (refraction_blur(self.scene.refractions[r].roughness), self.refraction_blur.is_null()) {
                    let kernel = KernelUniforms::vpx(size);
                    let texel = [1.0 / w, 1.0 / h, 0.0, 0.0];
                    for (pipeline, source, dest) in [(self.blur_kernel_h, self.refraction_textures[r], self.refraction_blur), (self.blur_kernel_v, self.refraction_blur, self.refraction_textures[r])] {
                        let mut target: SDL_GPUColorTargetInfo = zeroed();
                        target.texture = dest;
                        target.load_op = SDL_GPU_LOADOP_DONT_CARE;
                        target.store_op = SDL_GPU_STOREOP_STORE;
                        let blur = SDL_BeginGPURenderPass(command, &target, 1, null());
                        SDL_BindGPUGraphicsPipeline(blur, pipeline);
                        let binding = SDL_GPUTextureSamplerBinding { texture: source, sampler: self.flat_sampler };
                        SDL_BindGPUFragmentSamplers(blur, 0, &binding, 1);
                        SDL_PushGPUFragmentUniformData(command, 0, texel.as_ptr().cast(), 16);
                        SDL_PushGPUFragmentUniformData(command, 1, (&raw const kernel).cast(), size_of::<KernelUniforms>() as u32);
                        SDL_DrawGPUPrimitives(blur, 3, 1, 0, 0);
                        SDL_EndGPURenderPass(blur);
                    }
                }
                let mut again = color;
                again.load_op = SDL_GPU_LOADOP_LOAD;
                again.cycle = false;
                let mut again_depth = main_depth;
                again_depth.load_op = SDL_GPU_LOADOP_LOAD;
                again_depth.cycle = false;
                let pass = SDL_BeginGPURenderPass(command, &again, 1, &again_depth);
                // the table's light for VPX's material, pushed again for the passes after
                scene_light(true, &input.view);
                pass
            };
            // a part drawn bent: VPX's material over what's behind it, which shows through by
            // its opacity, sampled where the ray bent by its thickness lands
            let draw_refracting = |pass: *mut SDL_GPURenderPass, draw: &crate::scene::Draw, look: &MeshLook, model: &Mat4, r: usize| {
                let t = look.shading.refraction_tint;
                let uniforms = RefractionUniforms {
                    view_proj: view_proj.m,
                    tint_thickness: [t[0], t[1], t[2], draw.refraction_thickness],
                    flags: [if self.refraction_depths[r].is_null() { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                };
                SDL_PushGPUFragmentUniformData(command, 3, (&raw const uniforms).cast(), size_of::<RefractionUniforms>() as u32);
                let depth = if self.refraction_depths[r].is_null() { self.white } else { self.refraction_depths[r] };
                let bindings = [
                    SDL_GPUTextureSamplerBinding { texture: self.refraction_textures[r], sampler: self.clamp_sampler },
                    SDL_GPUTextureSamplerBinding { texture: depth, sampler: self.font_sampler },
                ];
                SDL_BindGPUFragmentSamplers(pass, 6, bindings.as_ptr(), 2);
                draw_one(pass, draw, look, model, None);
            };
            // VPX's legacy flag for a part (DrawMesh's, which RenderPass::SortCommands reads, not
            // whether the part blends): marked see-through, it's sorted far first with the halos
            // and flashers; else with the solid parts. The first look sorts by blending, as it was.
            let marked_see_through = |draw: &crate::scene::Draw, look: &MeshLook| {
                if !self.hdr {
                    return look.blend || look.added;
                }
                let opacity_active = look.opacity_active.unwrap_or(look.blend);
                match draw.sorting {
                    crate::scene::Sorting::Solid => false,
                    crate::scene::Sorting::SeeThrough => true,
                    crate::scene::Sorting::ByMaterial => opacity_active,
                    crate::scene::Sorting::ByMaterialUnlessStill => look.added || (opacity_active && !draw.still),
                }
            };
            // VPX's shader technique for a solid part, by its place in VPX's list (VPX's BGFX build
            // has three for the basic shader, Shader::SetTechniqueMaterial: textured, alpha-tested,
            // untextured)
            let technique = |look: &MeshLook| -> i32 {
                if !look.textured {
                    40
                } else if look.alpha_cutoff > 0.0 {
                    24
                } else {
                    8
                }
            };
            let pipeline_of = |draw: &crate::scene::Draw, look: &MeshLook| match (look.added, look.blend, self.hdr, draw.depth_mask) {
                (true, ..) => self.unshaded_add,
                (false, false, true, true) => self.vpx_opaque,
                (false, false, true, false) => self.vpx_opaque_no_depth,
                (false, false, false, _) => self.opaque,
                (false, true, true, true) => self.vpx_blended_depth,
                (false, true, true, false) => self.vpx_blended,
                (false, true, false, true) => self.blended_depth,
                (false, true, false, false) => self.blended,
            };
            // VPX's cull modes for a part as drawn now (each part's Render): 0 both faces, 1 VPX's
            // default (CULL_CCW), 2 the other way; the passes, one or two (a primitive with its back
            // faces on and a see-through material: the back first); in a mirror's pass VPX turns
            // its default the other way, the mirror reversing the faces
            // (RenderProbe::DoRenderReflectionProbe)
            let culls = |draw: &crate::scene::Draw, look: &MeshLook, mirrored: bool| -> ([u8; 2], usize) {
                use crate::scene::Culling;
                let (front, back) = if mirrored { (2, 1) } else { (1, 2) };
                let see_through = look.opacity_active.unwrap_or(look.blend);
                let one = |mode: u8| ([mode, 0], 1);
                if look.added {
                    return one(0);
                }
                match draw.culling {
                    Culling::None => one(0),
                    Culling::Front => one(front),
                    Culling::BothWhenSeeThrough => one(if see_through { 0 } else { front }),
                    Culling::WallTop { dynamic } => one(if see_through || !dynamic { 0 } else { front }),
                    Culling::WallSide { dynamic, top_visible } => one(if see_through || !dynamic || top_visible { 0 } else { front }),
                    Culling::Primitive { depth_mask: false, .. } => one(0),
                    Culling::Primitive { backfaces: true, .. } if see_through => ([back, front], 2),
                    Culling::Primitive { .. } => one(front),
                }
            };
            let culled = |pipeline: *mut SDL_GPUGraphicsPipeline, mode: u8| {
                if mode == 0 { pipeline } else { self.culled.get(&pipeline).map_or(pipeline, |twins| twins[mode as usize - 1]) }
            };
            // the parts VPX marks solid, in its order (RenderPass::SortCommands): by shader, a
            // technique later in VPX's list first, then near first (by the sort key), else in the
            // table's order (a stable sort); blended or not, a blended one with its mirror in a
            // pass of its own
            let draw_solid = |pass: *mut SDL_GPURenderPass, solids: &mut Vec<(i32, f32, &crate::scene::Draw, MeshLook)>, main: bool| {
                solids.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.total_cmp(&b.1)).then(a.2.part.unwrap_or(u32::MAX).cmp(&b.2.part.unwrap_or(u32::MAX))));
                let mut bound: *mut SDL_GPUGraphicsPipeline = null_mut();
                for (_, _, draw, look) in solids.iter() {
                    let (modes, passes) = culls(draw, look, !main);
                    for &mode in &modes[..passes] {
                        let pipeline = culled(pipeline_of(draw, look), mode);
                        if bound != pipeline {
                            SDL_BindGPUGraphicsPipeline(pass, pipeline);
                            SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                            SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                            bound = pipeline;
                        }
                        let model = model_of(draw);
                        let mirror = if main { mirror_of(draw) } else { None };
                        if look.blend && !look.added {
                            draw_one(pass, draw, look, &model, None);
                            if self.hdr
                                && let Some(mirror) = mirror
                            {
                                let add = culled(self.vpx_mirror_add, mode);
                                SDL_BindGPUGraphicsPipeline(pass, add);
                                bound = add;
                                draw_one(pass, draw, look, &model, Some(mirror));
                            }
                        } else {
                            draw_one(pass, draw, look, &model, mirror);
                        }
                    }
                }
            };
            // a classic light's glow added onto the table, its pipeline (`self.glow`) bound by the caller
            let classic_glow = |pass: *mut SDL_GPURenderPass, draw: &crate::scene::Draw, l: usize| {
                let now = light_now(l);
                let info = &self.scene.light_parts[l];
                let uniforms = MaterialUniforms {
                    // VPX's look: VPX's own strength (fs_classic_light.sc: the colour times its falloff
                    // and intensity); the first look keeps the softer glow it was tuned with
                    base_color: [now.color[0], now.color[1], now.color[2], now.intensity * if self.hdr { 1.0 } else { 0.12 }],
                    emissive: [now.color2[0], now.color2[1], now.color2[2], info.falloff_power],
                    // shining from its height above its surface (Light::GetCurrentHeight; the glow's
                    // mesh sits 0.1 above the surface)
                    params: [info.center.x, info.center.y, info.center.z - 0.1 + info.height, 1.0 / info.falloff_radius],
                    light0: [1.0, 0.0, 0.0, 0.0],
                    light1: [0.0; 4],
                    extra: [0.0; 4],
                    static_color_alpha: [1.0; 4],
                };
                let model = model_of(draw);
                SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                let binding = SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler };
                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
            };
            // VPX's look: a classic light with a picture of its own (light.cpp's light_with_texture,
            // fs_classic_light.sc): the picture lit as what the light sits on is, its glow added and
            // laid over the picture, the whole added onto the table, lit or not; its pipeline
            // (`self.vpx_classic_light`), the table's light and no ball shadow set by the caller
            let classic_picture = |pass: *mut SDL_GPURenderPass, draw: &crate::scene::Draw, l: usize| {
                let now = light_now(l);
                let info = &self.scene.light_parts[l];
                let look = mesh_look(draw);
                let sh = &info.surface_shading;
                let see_through = sh.opacity_active && sh.opacity < 0.999;
                let uniforms = VpxMaterialUniforms {
                    base_alpha: [sh.base[0], sh.base[1], sh.base[2], if see_through { sh.opacity } else { 1.0 }],
                    glossy_image_lerp: [sh.glossy[0], sh.glossy[1], sh.glossy[2], sh.glossy_image_lerp],
                    clearcoat_edge_alpha: [sh.clearcoat[0], sh.clearcoat[1], sh.clearcoat[2], sh.edge_alpha],
                    roughness_wrap_edge_thickness: [(10.0 * sh.roughness + 1.0).exp2(), sh.wrap_lighting, sh.edge, sh.thickness],
                    // the parts drawn before it leave their lighting switched on
                    disable_lighting_top_below: [0.0; 4],
                    static_color_alpha: [1.0; 4],
                    flags: [if sh.metal { 1.0 } else { 0.0 }, 1.0, 0.0, 0.0],
                    mirror_normal_factor: [0.0; 4],
                    normal_flags: [0.0; 4],
                    object_normal: [[0.0; 4]; 3],
                };
                let light = ClassicLightUniforms {
                    center_range: [info.center.x, info.center.y, info.center.z - 0.1 + info.height, 1.0 / info.falloff_radius],
                    color2_falloff_power: [now.color2[0], now.color2[1], now.color2[2], info.falloff_power],
                    color_intensity: [now.color[0], now.color[1], now.color[2], now.intensity],
                    lighting_off: [if info.image_mode { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                };
                let model = model_of(draw);
                SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<VpxMaterialUniforms>() as u32);
                SDL_PushGPUFragmentUniformData(command, 3, (&raw const light).cast(), size_of::<ClassicLightUniforms>() as u32);
                let env = if self.env_texture.is_null() { self.white } else { self.env_texture };
                // its picture clamped, filtered as the table's (SA_CLAMP, SF_TRILINEAR)
                let bindings = [
                    SDL_GPUTextureSamplerBinding { texture: look.texture, sampler: self.clamp_sampler },
                    SDL_GPUTextureSamplerBinding { texture: env, sampler: self.env_sampler },
                    SDL_GPUTextureSamplerBinding { texture: self.irradiance, sampler: self.env_sampler },
                ];
                SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 3);
                SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
            };
            // VPX leaves out a classic light with a picture of its own only when its picture is what it
            // sits on, it isn't in image mode, and it gives no light (`Light::Render`'s early out)
            let classic_picture_dark = |l: usize| {
                let now = light_now(l);
                let info = &self.scene.light_parts[l];
                info.picture_is_surface && !info.image_mode && (now.intensity == 0.0 || (now.color == [0.0; 3] && now.color2 == [0.0; 3]))
            };
            let draw_see_through = |pass: *mut SDL_GPURenderPass, transparent: &[(f32, Transparent)], main: bool| -> *mut SDL_GPURenderPass {
                let pass_now = std::cell::Cell::new(pass);
                let bound = std::cell::Cell::new(null_mut::<SDL_GPUGraphicsPipeline>());
                let bind = |pipeline: *mut SDL_GPUGraphicsPipeline| {
                    if bound.get() != pipeline {
                        let pass = pass_now.get();
                        SDL_BindGPUGraphicsPipeline(pass, pipeline);
                        SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                        SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                        bound.set(pipeline);
                    }
                };
                for (_, item) in transparent {
                    let pass = pass_now.get();
                    match item {
                        Transparent::Draw(draw, look) if main && !look.added && refraction_of(draw).is_some() => {
                            let r = refraction_of(draw).unwrap_or_default();
                            if !copied[r].get() {
                                copied[r].set(true);
                                pass_now.set(split_for(pass, r));
                                bound.set(null_mut());
                            }
                            let (modes, passes) = culls(draw, look, !main);
                            for &mode in &modes[..passes] {
                                bind(culled(if draw.depth_mask { self.vpx_refract_depth } else { self.vpx_refract }, mode));
                                draw_refracting(pass_now.get(), draw, look, &model_of(draw), r);
                            }
                        }
                        // marked see-through but drawn solid (a material whose opacity is on, at full)
                        Transparent::Draw(draw, look) if !look.blend && !look.added => {
                            let (modes, passes) = culls(draw, look, !main);
                            for &mode in &modes[..passes] {
                                bind(culled(pipeline_of(draw, look), mode));
                                draw_one(pass, draw, look, &model_of(draw), if main { mirror_of(draw) } else { None });
                            }
                        }
                        Transparent::Draw(draw, look) => {
                            let (modes, passes) = culls(draw, look, !main);
                            for &mode in &modes[..passes] {
                                bind(culled(
                                    match (look.added, self.hdr, draw.depth_mask) {
                                        (true, ..) => self.unshaded_add,
                                        (false, true, true) => self.vpx_blended_depth,
                                        (false, true, false) => self.vpx_blended,
                                        (false, false, true) => self.blended_depth,
                                        (false, false, false) => self.blended,
                                    },
                                    mode,
                                ));
                                let model = model_of(draw);
                                draw_one(pass, draw, look, &model, None);
                                if main
                                    && self.hdr
                                    && !look.added
                                    && let Some(mirror) = mirror_of(draw)
                                {
                                    bind(culled(self.vpx_mirror_add, mode));
                                    draw_one(pass, draw, look, &model, Some(mirror));
                                }
                            }
                        }
                        Transparent::Halo(draw, l) => {
                            let info = &self.scene.light_parts[*l];
                            let now = light_now(*l);
                            // VPX skips a bulb that gives no light
                            if now.intensity == 0.0 || (now.color == [0.0; 3] && now.color2 == [0.0; 3]) {
                                continue;
                            }
                            self.halo_raised.borrow_mut()[*l] = true;
                            // the glow's mesh sits 0.1 above the surface; VPX's light shines from its height above it
                            let surface = info.center.z - 0.1;
                            shadow(Some(*l));
                            let model = Mat4::translate(0.0, 0.0, info.halo_height) * model_of(draw);
                            SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                            let binding = SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler };
                            let terms: &[(*mut SDL_GPUGraphicsPipeline, f32)] =
                                if self.hdr { &[(self.bulb_vpx_3d, 2.0)] } else { &[(self.bulb_modulate_3d, 0.0), (self.bulb_add_3d, 1.0)] };
                            for &(pipeline, term) in terms {
                                bind(pipeline);
                                let uniforms = MaterialUniforms {
                                    base_color: [now.color[0], now.color[1], now.color[2], now.intensity],
                                    emissive: [now.color2[0], now.color2[1], now.color2[2], info.falloff_power],
                                    params: [info.center.x, info.center.y, surface + info.height, 1.0 / info.falloff_radius],
                                    light0: [info.modulate_vs_add.clamp(0.0001, 0.9999), term, 0.0, 0.0],
                                    light1: [0.0; 4],
                                    extra: [0.0; 4],
                                    static_color_alpha: [1.0; 4],
                                };
                                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                                SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                            }
                        }
                        Transparent::MirrorOnly(draw, look, mirror) => {
                            // VPX's reflection-only pass (Primitive::Render after its prepass): the
                            // mirror's moving parts added over a still part
                            let (modes, passes) = culls(draw, look, !main);
                            for &mode in &modes[..passes] {
                                bind(culled(self.vpx_mirror_add, mode));
                                let model = model_of(draw);
                                draw_one(pass, draw, look, &model, Some(*mirror));
                            }
                        }
                        Transparent::ClassicLight(draw, l) => {
                            let info = &self.scene.light_parts[*l];
                            if info.plain {
                                if light_now(*l).intensity > 0.0 {
                                    bind(self.glow);
                                    classic_glow(pass, draw, *l);
                                }
                            } else if !classic_picture_dark(*l) {
                                bind(self.vpx_classic_light);
                                scene_light(true, &input.view);
                                shadow(None);
                                classic_picture(pass, draw, *l);
                            }
                        }
                        Transparent::BulbHull(draw, l) => {
                            // VPX: the bulb light's shader over the bulb's mesh, at 2% of its
                            // intensity, only adding (modulate vs add 0.0001), in its single pass,
                            // without ball shadows
                            let info = &self.scene.light_parts[*l];
                            let now = light_now(*l);
                            if !info.bulb || now.intensity == 0.0 || (now.color == [0.0; 3] && now.color2 == [0.0; 3]) {
                                continue;
                            }
                            shadow(None);
                            bind(self.bulb_vpx_3d);
                            let model = model_of(draw);
                            SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                            let surface = info.center.z - 0.1;
                            let uniforms = MaterialUniforms {
                                base_color: [now.color[0], now.color[1], now.color[2], now.intensity * 0.02],
                                emissive: [now.color2[0], now.color2[1], now.color2[2], info.falloff_power],
                                params: [info.center.x, info.center.y, surface + info.height, 1.0 / info.falloff_radius],
                                light0: [0.0001, 2.0, 0.0, 0.0],
                                light1: [0.0; 4],
                                extra: [0.0; 4],
                                static_color_alpha: [1.0; 4],
                            };
                            SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                            let binding = SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler };
                            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                            SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                        }
                        Transparent::Flasher(f, now) => {
                            shadow(f.lightmap);
                            let model = f.transform(now);
                            SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                            let (uniforms, textures) = self.flasher_uniforms(now);
                            let bindings = textures.map(|texture| SDL_GPUTextureSamplerBinding { texture, sampler: self.clamp_sampler });
                            let terms: &[(*mut SDL_GPUGraphicsPipeline, f32)] = match (now.add_blend, self.hdr) {
                                (true, true) => &[(self.flasher_vpx, 3.0)],
                                (true, false) => &[(self.flasher_modulate, 1.0), (self.flasher_add, 2.0)],
                                (false, _) => &[(self.flasher_blend, 0.0)],
                            };
                            for &(pipeline, term) in terms {
                                bind(pipeline);
                                let u = FlasherUniforms { term: [term, 0.0, 0.0, 0.0], ..uniforms };
                                SDL_PushGPUFragmentUniformData(command, 0, (&raw const u).cast(), size_of::<FlasherUniforms>() as u32);
                                SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 2);
                                SDL_DrawGPUIndexedPrimitives(pass, f.index_count, 1, f.first_index, f.base_vertex, 0);
                            }
                        }
                        Transparent::DmdFlasher(f, now) => {
                            let Some(frame) = input.dmd else { continue };
                            let model = f.transform(now);
                            SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                            // VPX's Flasher::Render in DMD mode: the dots tinted by the flasher's colour,
                            // as bright as its opacity times its intensity scale, the alpha its modulate
                            // vs add; blended below 1 (added when it adds), else drawn over what's below
                            let c = |k: u32| ((now.color >> (8 * k)) & 0xFF) as f32 / 255.0;
                            let uniforms = self.dmd_uniforms(frame, [c(0), c(1), c(2)], now.alpha * now.intensity_scale / 100.0, now.modulate_vs_add);
                            bind(if now.modulate_vs_add >= 1.0 {
                                self.dmd_opaque
                            } else if now.add_blend {
                                self.dmd_add
                            } else {
                                self.dmd_blend
                            });
                            SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<DmdUniforms>() as u32);
                            let binding = SDL_GPUTextureSamplerBinding { texture: self.dmd_texture, sampler: self.font_sampler };
                            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                            SDL_DrawGPUIndexedPrimitives(pass, f.index_count, 1, f.first_index, f.base_vertex, 0);
                        }
                    }
                }
                pass_now.get()
            };

            // VPX's look: the mirrors, first (RenderProbe::RenderReflectionProbe): the table drawn
            // again from the camera mirrored through each mirror's plane, keeping what's above it,
            // into the mirror's picture; lit as the table is, so the mirrored camera only changes
            // where it's seen from. The main picture's multisampled buffers serve, cleared after
            for (k, mirror) in self.scene.mirrors.iter().enumerate() {
                if !rendered[k] {
                    continue;
                }
                let [x, y, z, d] = mirror.plane;
                let len = (x * x + y * y + z * z).sqrt();
                if len == 0.0 {
                    continue;
                }
                let n = Vec3::new(x / len, y / len, z / len);
                let reflect = Mat4::plane_reflection(n, d);
                let eye = reflect.transform_no_perspective(input.eye);
                let mirrored = frame_for(reflect * input.view, eye, [n.x, n.y, n.z, d]);
                // what VPX's reflection mode mirrors: balls, parts that never move, every part; with the
                // still parts drawn once, the still ones in their passes and the rest each frame
                let mode = self.reflection_mode(k);
                let (parts, balls, moving) = match stage {
                    Stage::Full => (mode >= 2, mode != 2, mode >= 4),
                    Stage::Still(_) => (true, false, false),
                    Stage::Moving => (mode >= 4, true, mode >= 4),
                };
                // a mirror of everything's still parts kept with their depth, for each frame to start from
                let prerender = still_pass && mode == 5;
                let mut mirror_depth = depth;
                if let (true, Some(s)) = (prerender, still) {
                    mirror_depth.texture = s.mirror_depth[k];
                    mirror_depth.store_op = SDL_GPU_STOREOP_STORE;
                }
                let mut target: SDL_GPUColorTargetInfo = zeroed();
                target.load_op = SDL_GPU_LOADOP_CLEAR;
                if self.samples == SDL_GPU_SAMPLECOUNT_1 {
                    target.texture = self.mirror_textures[k];
                    target.store_op = SDL_GPU_STOREOP_STORE;
                } else {
                    target.texture = self.msaa;
                    target.store_op = SDL_GPU_STOREOP_RESOLVE;
                    target.resolve_texture = self.mirror_textures[k];
                }
                // multisampled, the still parts' depth copied over first
                if let (Stage::Moving, 5, Some(s), false) = (stage, mode, still, self.samples == SDL_GPU_SAMPLECOUNT_1) {
                    self.copy_still_depth(command, s.mirror_depth[k], rw, rh);
                    mirror_depth.load_op = SDL_GPU_LOADOP_LOAD;
                }
                let mpass = SDL_BeginGPURenderPass(command, &target, 1, &mirror_depth);
                if let (Stage::Moving, 5, Some(s)) = (stage, mode, still) {
                    self.restore_still(mpass, s.mirror_color[k], s.mirror_depth[k]);
                }
                SDL_PushGPUVertexUniformData(command, 0, (&raw const mirrored).cast(), size_of::<FrameUniforms>() as u32);
                // no light from below in a mirror: VPX's playfield mirror leaves lights out (PinTable's
                // load sets it so), so its own bulb light buffer pass draws none and binds its black
                // null picture (Light::Render with DISABLE_LIGHTMAPS, DrawBulbLightBuffer); a mirror
                // set to reflect lights would draw the glows and a mirrored light buffer, not ported
                // (no test table has one)
                scene_light(false, &(reflect * input.view));
                let mut solids: Vec<(i32, f32, &crate::scene::Draw, MeshLook)> = Vec::new();
                let mut see_through: Vec<(f32, Transparent)> = Vec::new();
                // the parts drawn into mirrors: not a part showing this mirror (VPX: no self
                // reflection), nor lights' glows (light maps, which the playfield's leaves out)
                let reflected = |d: &&crate::scene::Draw| {
                    parts
                        && self.reflected_now(d, input)
                        && shown(d)
                        && drawn_now(d)
                        && d.glow.is_none()
                        && self.mirror_shown(d, input).is_none_or(|(m, _)| m != k)
                        && (moving || (d.group.is_none() && d.bumper.is_none()))
                };
                for draw in self.scene.draws.iter().filter(reflected) {
                    let look = mesh_look(draw);
                    if look.skip {
                        continue;
                    }
                    // VPX's DrawMesh, in a reflection pass: by the depth bias plus the height
                    let at = model_of(draw).transform_no_perspective(draw.sort_point);
                    let key = self.depth_bias_now(draw, input) + at.z;
                    if marked_see_through(draw, &look) {
                        see_through.push((key, Transparent::Draw(draw, look)));
                        if let Some(l) = draw.bulb {
                            see_through.push((key, Transparent::BulbHull(draw, l)));
                        }
                    } else {
                        solids.push((technique(&look), key, draw, look));
                    }
                }
                draw_solid(mpass, &mut solids, false);
                if balls {
                    draw_balls(mpass, reflect * input.view, reflect * view_proj, true);
                }
                see_through.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.part().cmp(&b.1.part())));
                draw_see_through(mpass, &see_through, false);
                SDL_EndGPURenderPass(mpass);
                // a rough mirror blurred by VPX's kernel for its roughness, across then down
                // (RenderProbe::ApplyRoughness); VPX draws the roughest at half size
                if let (Some((size, half)), false, false) = (mirror_blur(mirror.roughness), self.mirror_blur.is_null(), prerender) {
                    let kernel = KernelUniforms::vpx(size);
                    let reach = if half { 2.0 } else { 1.0 };
                    let texel = [reach / w, reach / h, 0.0, 0.0];
                    for (pipeline, source, dest) in [(self.blur_kernel_h, self.mirror_textures[k], self.mirror_blur), (self.blur_kernel_v, self.mirror_blur, self.mirror_textures[k])] {
                        let mut target: SDL_GPUColorTargetInfo = zeroed();
                        target.texture = dest;
                        target.load_op = SDL_GPU_LOADOP_DONT_CARE;
                        target.store_op = SDL_GPU_STOREOP_STORE;
                        let blur = SDL_BeginGPURenderPass(command, &target, 1, null());
                        SDL_BindGPUGraphicsPipeline(blur, pipeline);
                        let binding = SDL_GPUTextureSamplerBinding { texture: source, sampler: self.flat_sampler };
                        SDL_BindGPUFragmentSamplers(blur, 0, &binding, 1);
                        SDL_PushGPUFragmentUniformData(command, 0, texel.as_ptr().cast(), 16);
                        SDL_PushGPUFragmentUniformData(command, 1, (&raw const kernel).cast(), size_of::<KernelUniforms>() as u32);
                        SDL_DrawGPUPrimitives(blur, 3, 1, 0, 0);
                        SDL_EndGPURenderPass(blur);
                    }
                }
            }

            // multisampled, the still parts' depth copied over first (the pass loads it)
            if let (Stage::Moving, Some(s), false) = (stage, still, self.samples == SDL_GPU_SAMPLECOUNT_1) {
                self.copy_still_depth(command, s.depth, rw, rh);
            }
            let mut pass = SDL_BeginGPURenderPass(command, &color, 1, &main_depth);
            if let (Stage::Moving, Some(s)) = (stage, still) {
                self.restore_still(pass, s.color, s.depth);
            }
            let mut ob: SDL_GPUBufferBinding = zeroed();
            ob.buffer = self.overlay_buffer;
            let screen = [2.0 / w, -2.0 / h, 0.0, 0.0];
            // the readout's, in the window's pixels
            let screen_out = [2.0 / out_w, -2.0 / out_h, 0.0, 0.0];
            // the backdrop image behind everything (a still part, VPX's DrawBackground in its prepass)
            if let Some((texture, first)) = background
                && first + 6 <= overlay_count as u32
                && stage != Stage::Moving
            {
                SDL_BindGPUGraphicsPipeline(pass, self.sprite);
                SDL_BindGPUVertexBuffers(pass, 0, &ob, 1);
                SDL_PushGPUVertexUniformData(command, 0, (&raw const screen).cast(), 16);
                let binding = SDL_GPUTextureSamplerBinding { texture, sampler: backdrop_sampler };
                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                SDL_DrawGPUPrimitives(pass, 6, 1, first, 0);
            }
            SDL_PushGPUVertexUniformData(command, 0, (&raw const frame).cast(), size_of::<FrameUniforms>() as u32);
            let mut transparent: Vec<(f32, Transparent)> = Vec::new();

            // the playfield before everything else, blended or not, as VPX draws it (its depth bias
            // of 100000 puts it first: "playfield meshes were always drawn before other transparent
            // parts until 10.8.0", PinTable's load; RenderPass::SortCommands); a blended one gets
            // its mirror in a pass of its own
            scene_light(true, &input.view);
            for draw in self.scene.draws.iter().filter(|d| d.playfield && shown(d) && (drawn_now(d) || stage == Stage::Moving)) {
                let look = mesh_look(draw);
                if look.skip {
                    continue;
                }
                if !drawn_now(draw) {
                    over_still(&mut transparent, draw, look, &model_of(draw), mirror_of(draw), self.hdr, self.depth_bias_now(draw, input));
                    continue;
                }
                let (modes, passes) = culls(draw, &look, false);
                for &mode in &modes[..passes] {
                    SDL_BindGPUGraphicsPipeline(pass, culled(pipeline_of(draw, &look), mode));
                    SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                    SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                    let model = model_of(draw);
                    if look.blend && !look.added {
                        draw_one(pass, draw, &look, &model, None);
                        if self.hdr
                            && let Some(mirror) = mirror_of(draw)
                        {
                            SDL_BindGPUGraphicsPipeline(pass, culled(self.vpx_mirror_add, mode));
                            draw_one(pass, draw, &look, &model, Some(mirror));
                        }
                    } else {
                        draw_one(pass, draw, &look, &model, mirror_of(draw));
                    }
                }
            }
            // the rest (VPX's look: with VPX's material, and the table's light for it) in VPX's
            // order (RenderPass::SortCommands): those VPX marks solid first, by shader, then near
            // first; those it marks see-through with the halos and flashers, far first, below
            let mut solids: Vec<(i32, f32, &crate::scene::Draw, MeshLook)> = Vec::new();
            for draw in self.scene.draws.iter().filter(|d| shown(d) && !d.playfield) {
                if let Some(l) = draw.glow {
                    // lights' glows come each frame
                    if still_pass {
                        continue;
                    }
                    // a bulb's halo is drawn among the see-through parts, sorted by its depth bias less
                    // the height it floats at, its surface's plus its halo height (VPX's Light::Render:
                    // haloPos, after UpdateMeshBuffer raised m_surfaceHeight; DrawMesh); classic lights below
                    let info = &self.scene.light_parts[l];
                    if info.bulb {
                        transparent.push((info.depth_bias - (info.center.z - 0.1 + info.halo_height), Transparent::Halo(draw, l)));
                    } else if self.hdr && info.surface_shading.opacity_active {
                        // VPX's look: a classic light on a surface whose material is marked see-through
                        // is drawn among the see-through parts (Light::Render: DrawMesh's transparent
                        // flag), by its depth bias less its surface's height (haloPos)
                        transparent.push((info.depth_bias - (info.center.z - 0.1), Transparent::ClassicLight(draw, l)));
                    }
                    continue;
                }
                let look = mesh_look(draw);
                if look.skip {
                    continue;
                }
                let model = model_of(draw);
                match stage {
                    Stage::Still(_) if !draw.still => continue,
                    Stage::Moving if draw.still => {
                        over_still(&mut transparent, draw, look, &model, mirror_of(draw), self.hdr, self.depth_bias_now(draw, input));
                        continue;
                    }
                    _ => {}
                }
                // VPX's DrawMesh: by the depth bias less the part's height
                let at = model.transform_no_perspective(draw.sort_point);
                let key = self.depth_bias_now(draw, input) - at.z;
                if marked_see_through(draw, &look) || refraction_of(draw).is_some() {
                    // (a part drawn bent too: "refractions must be rendered back to front")
                    transparent.push((key, Transparent::Draw(draw, look)));
                    // VPX blends the lit bulb's own glow over its glass, at the same place in the order
                    // (each frame: it's lit and unlit as the light is)
                    if self.hdr
                        && !still_pass
                        && let Some(l) = draw.bulb
                    {
                        transparent.push((key, Transparent::BulbHull(draw, l)));
                    }
                } else {
                    solids.push((technique(&look), key, draw, look));
                }
            }
            // VPX's look: the kickers' holes before the other solid parts (RenderPass::SortCommands: kickers
            // before every other draw, the farthest first): each plate over what's there, at the depth of a
            // point 30 below, so the cup drawn next shows sunk in the playfield (Kicker::Render's kickerBoolean)
            if self.hdr && !self.vpx_kicker_plate.is_null() {
                let mut plates: Vec<(f32, &crate::scene::Draw, MeshLook)> = Vec::new();
                solids.retain(|&(_, key, draw, look)| {
                    if draw.kicker_plate {
                        plates.push((key, draw, look));
                    }
                    !draw.kicker_plate
                });
                plates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.part.cmp(&b.1.part)));
                for (_, draw, look) in &plates {
                    let (modes, passes) = culls(draw, look, false);
                    for &mode in &modes[..passes] {
                        SDL_BindGPUGraphicsPipeline(pass, culled(self.vpx_kicker_plate, mode));
                        SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                        SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                        draw_one(pass, draw, look, &model_of(draw), None);
                    }
                }
            }
            // VPX's look: the backdrop's score reels before the solid parts drawn now, where VPX's order
            // puts them (RenderPass::SortCommands: solid quads of the shader technique it sorts first,
            // DispReel::Render's basic_noDMD), writing no depth: a part drawn now over the backdrop
            // covers them, as Flash Gordon's room does in VPX's pictures, its script not run
            let reels_first = self.hdr && !still_pass;
            if reels_first {
                let mut bound = false;
                for &(_, texture, first, count, _, reel) in &sprite_draws {
                    if !reel || first + count > overlay_count as u32 {
                        continue;
                    }
                    if !bound {
                        SDL_BindGPUGraphicsPipeline(pass, self.sprite);
                        SDL_BindGPUVertexBuffers(pass, 0, &ob, 1);
                        SDL_PushGPUVertexUniformData(command, 0, (&raw const screen).cast(), 16);
                        bound = true;
                    }
                    let binding = SDL_GPUTextureSamplerBinding { texture, sampler: self.flat_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                    SDL_DrawGPUPrimitives(pass, count, 1, first, 0);
                }
                if bound {
                    SDL_PushGPUVertexUniformData(command, 0, (&raw const frame).cast(), size_of::<FrameUniforms>() as u32);
                }
            }
            draw_solid(pass, &mut solids, true);
            // classic lights with no picture: VPX draws their shape in the light's colour, lit or not
            // (light.cpp's light_without_texture); VPX's look shades it as the material of what it
            // sits on is shaded, the light's colour for its picture (fs_classic_light.sc), blended
            // only where that material is see-through, writing no depth
            if self.hdr {
                SDL_BindGPUGraphicsPipeline(pass, self.vpx_blended);
                SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
            }
            for draw in self.scene.draws.iter().filter(|d| shown(d) && !still_pass) {
                let Some(l) = draw.glow else { continue };
                let info = &self.scene.light_parts[l];
                if !info.plain {
                    continue;
                }
                let now = light_now(l);
                if self.hdr {
                    let (sh, c) = (&info.surface_shading, now.color);
                    let see_through = sh.opacity_active && sh.opacity < 0.999;
                    let uniforms = VpxMaterialUniforms {
                        base_alpha: [c[0] * sh.base[0], c[1] * sh.base[1], c[2] * sh.base[2], if see_through { sh.opacity } else { 1.0 }],
                        glossy_image_lerp: if sh.metal { [0.0; 4] } else { [c[0] * sh.glossy[0], c[1] * sh.glossy[1], c[2] * sh.glossy[2], sh.glossy_image_lerp] },
                        clearcoat_edge_alpha: [sh.clearcoat[0], sh.clearcoat[1], sh.clearcoat[2], sh.edge_alpha],
                        roughness_wrap_edge_thickness: [(10.0 * sh.roughness + 1.0).exp2(), sh.wrap_lighting, sh.edge, sh.thickness],
                        // its lighting on, no light from below
                        disable_lighting_top_below: [0.0, 1.0, 0.0, 0.0],
                        static_color_alpha: [1.0; 4],
                        flags: [if sh.metal { 1.0 } else { 0.0 }, 0.0, 0.0, if see_through { 0.0 } else { solid }],
                        mirror_normal_factor: [0.0; 4],
                        normal_flags: [0.0; 4],
                        object_normal: [[0.0; 4]; 3],
                    };
                    // (VPX compiles its classic light shader without ball shadows)
                    shadow(None);
                    let model = model_of(draw);
                    SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                    SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<VpxMaterialUniforms>() as u32);
                    let env = if self.env_texture.is_null() { self.white } else { self.env_texture };
                    let bindings = [
                        SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler },
                        SDL_GPUTextureSamplerBinding { texture: env, sampler: self.env_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.irradiance, sampler: self.env_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.flat_sampler },
                        SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler },
                    ];
                    SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 6);
                    SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    continue;
                }
                let uniforms = MaterialUniforms {
                    base_color: [now.color[0], now.color[1], now.color[2], 1.0],
                    emissive: [0.0; 4],
                    params: [0.0, 0.0, 0.5, 0.0],
                    light0: lights.0,
                    light1: lights.1,
                    extra: [0.0, solid, 0.0, 0.0],
                    static_color_alpha: [1.0; 4],
                };
                let model = model_of(draw);
                SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                let binding = SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler };
                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                SDL_DrawGPUIndexedPrimitives(pass, draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
            }
            // the balls (VPX's look: VPX's ball, its nearest lights and its mirror of the playfield)
            if self.hdr && !still_pass {
                draw_balls(pass, input.view, view_proj, false);
            }
            let (first, count, base) = self.scene.ball;
            for ball in input.balls.iter().filter(|_| !self.hdr && !still_pass) {
                let model = Mat4::scale(ball.radius, ball.radius, ball.radius) * Mat4::translate(ball.position.x, ball.position.y, ball.position.z);
                let uniforms = MaterialUniforms {
                    base_color: [0.55, 0.55, 0.58, 1.0],
                    emissive: [0.0; 4],
                    params: [0.0, 1.0, 0.1, 1.0],
                    light0: lights.0,
                    light1: lights.1,
                    extra: [0.0, solid, 0.0, 0.0],
                    static_color_alpha: [1.0; 4],
                };
                SDL_PushGPUVertexUniformData(command, 1, (&raw const model.m).cast(), 64);
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<MaterialUniforms>() as u32);
                let binding = SDL_GPUTextureSamplerBinding { texture: self.white, sampler: self.sampler };
                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                SDL_DrawGPUIndexedPrimitives(pass, count, 1, first, base, 0);
            }
            // the classic lights' glows, added onto the table (VPX adds it in the light's own pass;
            // VPX's look: those with no picture of their own, the others below)
            SDL_BindGPUGraphicsPipeline(pass, self.glow);
            SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
            SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
            for draw in self.scene.draws.iter().filter(|_| !still_pass) {
                let Some(l) = draw.glow else { continue };
                let now = light_now(l);
                let info = &self.scene.light_parts[l];
                // (VPX's look: those on a see-through surface come with the see-through parts)
                if !shown(draw) || now.intensity <= 0.0 || info.bulb || (self.hdr && (!info.plain || info.surface_shading.opacity_active)) {
                    continue;
                }
                classic_glow(pass, draw, l);
            }
            // VPX's look: the classic lights with a picture of their own
            if self.hdr && !still_pass {
                let mut bound = false;
                for draw in self.scene.draws.iter() {
                    let Some(l) = draw.glow else { continue };
                    let info = &self.scene.light_parts[l];
                    // (those on a see-through surface come with the see-through parts)
                    if info.bulb || info.plain || !shown(draw) || info.surface_shading.opacity_active {
                        continue;
                    }
                    if classic_picture_dark(l) {
                        continue;
                    }
                    if !bound {
                        SDL_BindGPUGraphicsPipeline(pass, self.vpx_classic_light);
                        SDL_BindGPUVertexBuffers(pass, 0, &vb, 1);
                        SDL_BindGPUIndexBuffer(pass, &ib, SDL_GPU_INDEXELEMENTSIZE_32BIT);
                        scene_light(true, &input.view);
                        shadow(None);
                        bound = true;
                    }
                    classic_picture(pass, draw, l);
                }
            }
            // the flashers: VPX skips one that is hidden, black, or without opacity or intensity
            for f in self.scene.flashers.iter().filter(|_| !still_pass) {
                let r = input.part_render.get(f.part as usize);
                let now = r.and_then(|r| r.flasher).unwrap_or(f.start);
                if !r.is_none_or(|r| r.visible) || now.color == 0 || now.alpha == 0.0 || now.intensity_scale == 0.0 {
                    continue;
                }
                if f.dmd {
                    // VPX shifts a DMD flasher's depth bias by -10000, so it comes after the
                    // other see-through parts, as it did when it was drawn in a pass of its own
                    if input.dmd.is_some() && !self.dmd_texture.is_null() {
                        transparent.push((now.depth_bias - 10000.0 - now.height, Transparent::DmdFlasher(f, now)));
                    }
                    continue;
                }
                transparent.push((now.depth_bias - now.height, Transparent::Flasher(f, now)));
            }
            // see-through parts and flashers in VPX's order: far first by the depth bias less
            // the height, the table's order where those are equal (a stable sort)
            transparent.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.part().cmp(&b.1.part())));
            pass = draw_see_through(pass, &transparent, true);
            // the backdrop in VPX's order (RenderPass::SortCommands), over the table: its score reels
            // first (in VPX's look drawn above, before the solid parts); then the classic lights, solid
            // too, which VPX writes at the nearest depth, so nothing after covers them; then, see-through
            // at one depth, the bulbs and the text boxes in the table's order, as VPX draws them — a
            // bulb lighting the reel picture below it
            let backdrop_light = |k: usize, term: f32| {
                let look = &self.scene.backdrop_lights[k];
                let now = light_now(look.light);
                BackdropLightUniforms {
                    color_intensity: [now.color[0], now.color[1], now.color[2], now.intensity],
                    color2_power: [now.color2[0], now.color2[1], now.color2[2], look.falloff_power],
                    center_range: [look.center[0] * xmult - 0.5, look.center[1] * ymult - 0.5, 0.0, 1.0 / look.falloff.max(0.1)],
                    mode: [if look.image.is_some() { 1.0 } else { 0.0 }, look.modulate_vs_add.clamp(0.0001, 0.9999), term, 0.0],
                }
            };
            if !still_pass {
                enum BackdropDraw {
                    Sprite(usize),
                    Light(usize, u32, u32),
                }
                let mut order: Vec<(u8, u32, BackdropDraw)> = Vec::new();
                for (n, d) in sprite_draws.iter().enumerate().filter(|(_, d)| !(d.5 && reels_first)) {
                    order.push((if d.5 { 0 } else { 2 }, d.0, BackdropDraw::Sprite(n)));
                }
                for &(k, first, count) in &light_draws {
                    let look = &self.scene.backdrop_lights[k];
                    order.push((if look.bulb { 2 } else { 1 }, look.part, BackdropDraw::Light(k, first, count)));
                }
                order.sort_by_key(|&(group, part, _)| (group, part));
                let mut bound: *mut SDL_GPUGraphicsPipeline = null_mut();
                for (_, _, item) in &order {
                    match *item {
                        BackdropDraw::Light(k, first, count) => {
                            let look = &self.scene.backdrop_lights[k];
                            if first + count > overlay_count as u32 {
                                continue;
                            }
                            // VPX skips a bulb that gives no light
                            let now = light_now(look.light);
                            if look.bulb && (now.intensity == 0.0 || (now.color == [0.0; 3] && now.color2 == [0.0; 3])) {
                                continue;
                            }
                            let texture = look.image.map_or(self.white, |i| self.textures[i]);
                            let steps: &[(*mut SDL_GPUGraphicsPipeline, f32)] = match (look.bulb, self.hdr) {
                                (true, true) => &[(self.bulb_vpx, 2.0)],
                                (true, false) => &[(self.bulb_modulate, 0.0), (self.bulb_add, 1.0)],
                                (false, _) => &[(self.backdrop_light, 0.0)],
                            };
                            for &(pipeline, term) in steps {
                                SDL_BindGPUGraphicsPipeline(pass, pipeline);
                                SDL_BindGPUVertexBuffers(pass, 0, &ob, 1);
                                SDL_PushGPUVertexUniformData(command, 0, (&raw const screen).cast(), 16);
                                let uniforms = backdrop_light(k, term);
                                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<BackdropLightUniforms>() as u32);
                                let binding = SDL_GPUTextureSamplerBinding { texture, sampler: self.flat_sampler };
                                SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                                SDL_DrawGPUPrimitives(pass, count, 1, first, 0);
                            }
                            bound = null_mut();
                        }
                        BackdropDraw::Sprite(n) => {
                            let (_, texture, first, count, dmd, _) = sprite_draws[n];
                            if first + count > overlay_count as u32 {
                                continue;
                            }
                            let pipeline = if dmd.is_some() { self.dmd_backdrop } else { self.sprite };
                            if bound != pipeline {
                                SDL_BindGPUGraphicsPipeline(pass, pipeline);
                                SDL_BindGPUVertexBuffers(pass, 0, &ob, 1);
                                SDL_PushGPUVertexUniformData(command, 0, (&raw const screen).cast(), 16);
                                bound = pipeline;
                            }
                            // the display's dots, a texel each: unfiltered (VPX's tex_dmd)
                            let sampler = if let Some(u) = dmd {
                                SDL_PushGPUFragmentUniformData(command, 0, (&raw const u).cast(), size_of::<DmdUniforms>() as u32);
                                self.font_sampler
                            } else {
                                self.flat_sampler
                            };
                            let binding = SDL_GPUTextureSamplerBinding { texture, sampler };
                            SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                            SDL_DrawGPUPrimitives(pass, count, 1, first, 0);
                        }
                    }
                }
            }
            // the readout (in VPX's look, over the tonemapped picture)
            let readout = |pass: *mut SDL_GPURenderPass, pipeline: *mut SDL_GPUGraphicsPipeline| {
                if text_count > 0 {
                    SDL_BindGPUGraphicsPipeline(pass, pipeline);
                    SDL_BindGPUVertexBuffers(pass, 0, &ob, 1);
                    SDL_PushGPUVertexUniformData(command, 0, (&raw const screen_out).cast(), 16);
                    let binding = SDL_GPUTextureSamplerBinding { texture: self.font, sampler: self.font_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                    SDL_DrawGPUPrimitives(pass, text_count as u32, 1, 0, 0);
                }
            };
            if !self.hdr {
                readout(pass, self.overlay);
            }
            SDL_EndGPURenderPass(pass);
            if still_pass {
                return;
            }
            // the windows drawn inside the table's (VPX 5436's embedded output): in the first look
            // over the finished picture; in VPX's look their places cleared before the occlusion
            // and the bloom read the picture, their pictures taken in after the bloom
            if self.hdr {
                self.record_embedded_clear(command, (rw, rh), (width, height));
            } else {
                self.record_embedded_copy(command, target, (width, height), (width, height));
            }
            // VPX's ball motion blur, from this frame's picture and the last one's (so before this
            // frame's is kept as the last), and its dynamic occlusion
            let blur_regions = if self.motion_blur_on() && self.prev_valid.get() { self.record_motion_blur(command, input, (rw, rh)) } else { Vec::new() };
            if self.ao_mode() == 2 {
                self.record_frame_ao(command, (rw, rh));
                if let Some(e) = &self.effects {
                    self.record_embedded_ao(command, e.ao_latest_picture(), (rw, rh), (width, height));
                }
            }
            // none on an HDR screen, as VPX 5436 turns its bloom off there
            let bloom = self.hdr && self.hdr10.is_none() && !self.bloom[0].is_null() && self.tone.bloom_strength > 0.0 && !self.bloom_off;
            if bloom {
                // VPX's bloom (Renderer::UpdateBloom): the bright parts' cut-off into a
                // quarter-size picture, then the 39-texel blur across and down
                let full = [1.0 / w, 1.0 / h, self.tone.bloom_strength, 2.25];
                let (bw, bh) = (self.bloom_size.0 as f32, self.bloom_size.1 as f32);
                let small = [1.0 / bw, 1.0 / bh, 1.0, 0.0];
                for (pipeline, source, dest, uniforms) in [
                    (self.bloom_cut, self.hdr_target, self.bloom[0], full),
                    (self.blur_h, self.bloom[0], self.bloom[1], small),
                    (self.blur_v, self.bloom[1], self.bloom[0], small),
                ] {
                    let mut target: SDL_GPUColorTargetInfo = zeroed();
                    target.texture = dest;
                    target.load_op = SDL_GPU_LOADOP_DONT_CARE;
                    target.store_op = SDL_GPU_STOREOP_STORE;
                    let pass = SDL_BeginGPURenderPass(command, &target, 1, null());
                    SDL_BindGPUGraphicsPipeline(pass, pipeline);
                    let binding = SDL_GPUTextureSamplerBinding { texture: source, sampler: self.flat_sampler };
                    SDL_BindGPUFragmentSamplers(pass, 0, &binding, 1);
                    SDL_PushGPUFragmentUniformData(command, 0, uniforms.as_ptr().cast(), 16);
                    SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
                    SDL_EndGPURenderPass(pass);
                }
            }
            if self.hdr {
                self.record_embedded_copy(command, target, (rw, rh), (width, height));
            }
            if self.hdr && !self.prev_hdr.is_null() {
                // the ball mirrors the playfield from the frame before (VPX's swapped back buffers,
                // which hold the embedded windows too)
                let copy = SDL_BeginGPUCopyPass(command);
                let mut from: SDL_GPUTextureLocation = zeroed();
                from.texture = self.hdr_target;
                let mut to: SDL_GPUTextureLocation = zeroed();
                to.texture = self.prev_hdr;
                SDL_CopyGPUTextureToTexture(copy, &from, &to, rw, rh, 1, false);
                SDL_EndGPUCopyPass(copy);
                self.prev_valid.set(true);
            }
            if self.hdr {
                // VPX's tonemapping (Renderer::SetupTonemapping, its SDR path): the
                // table's exposure and tone mapper, dither, and its colour grade
                // into the window, or with VPX's post-processing after it, into its first picture
                let (tonemapped, tonemap) = self.tonemap_target(target);
                let chained = tonemapped != target;
                let mut post: SDL_GPUColorTargetInfo = zeroed();
                post.texture = tonemapped;
                post.load_op = SDL_GPU_LOADOP_DONT_CARE;
                post.store_op = SDL_GPU_STOREOP_STORE;
                let pass = SDL_BeginGPURenderPass(command, &post, 1, null());
                SDL_BindGPUGraphicsPipeline(pass, tonemap);
                let lut = if self.color_grade.is_null() { self.white } else { self.color_grade };
                // the nudge shake (VPX's screen offset), in pixels: the picture moved and filtered
                let shake = if stage == Stage::Moving { [input.screen_offset[0] * out_w * 0.5, -input.screen_offset[1] * out_h * 0.5] } else { [0.0; 2] };
                let shaken = shake != [0.0; 2];
                // supersampled, the bigger picture read down filtered, as VPX 10.8.0 reads it
                let supersampled = (rw, rh) != (width, height);
                // VPX's dynamic occlusion, the newest
                let ao = self.effects.as_ref().filter(|_| self.ao_mode() == 2).map(|e| e.ao_latest_picture());
                let picture_sampler = if shaken || supersampled { self.flat_sampler } else { self.font_sampler };
                let mut bindings = [
                    SDL_GPUTextureSamplerBinding { texture: self.hdr_target, sampler: picture_sampler },
                    SDL_GPUTextureSamplerBinding { texture: lut, sampler: self.flat_sampler },
                    SDL_GPUTextureSamplerBinding { texture: if bloom { self.bloom[0] } else { self.white }, sampler: self.flat_sampler },
                    SDL_GPUTextureSamplerBinding { texture: ao.unwrap_or(self.white), sampler: self.flat_sampler },
                ];
                SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 4);
                // the dither follows the frame's number, as VPX's does, so a picture is the same on every run
                let frame = (self.frames_drawn.get() % 2048) as f32;
                // the post-processing picture takes sRGB values as they are, as VPX's does
                let srgb_target = !chained && (self.format == SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM_SRGB || self.format == SDL_GPU_TEXTUREFORMAT_R8G8B8A8_UNORM_SRGB);
                // VPX's HDR10 output: its exposure times HDRGlobalExposure, and no dither
                let (hdr, spline1, spline2) = self.hdr10.map_or(([0.0; 4], [0.0; 4], [0.0; 4]), |h| h.uniforms());
                let exposure = self.tone.exposure * self.hdr10.map_or(1.0, |h| h.request.exposure);
                let uniforms = TonemapUniforms {
                    exposure_mapper: [exposure, self.tone.mapper as f32, shake[0], shake[1]],
                    flags: [
                        if self.hdr10.is_some() { 0.0 } else { 1.0 },
                        if self.color_grade.is_null() { 0.0 } else { 1.0 },
                        if srgb_target { 1.0 } else { 0.0 },
                        if bloom { 1.0 } else { 0.0 },
                    ],
                    frame: [frame, 1.0 / out_w, 1.0 / out_h, if supersampled { 1.0 } else { 0.0 }],
                    ao: [if ao.is_some() { 1.0 } else { 0.0 }, 1.0 / w, 1.0 / h, 0.0],
                    hdr,
                    spline1,
                    spline2,
                };
                SDL_PushGPUFragmentUniformData(command, 0, (&raw const uniforms).cast(), size_of::<TonemapUniforms>() as u32);
                SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
                // the blurred balls' parts drawn again from the blur's picture, where the shake puts them
                // (VPX's ApplyBallMotionBlur, its second half)
                if let (Some(e), false) = (self.effects.as_ref(), blur_regions.is_empty()) {
                    bindings[0].texture = e.blur;
                    SDL_BindGPUFragmentSamplers(pass, 0, bindings.as_ptr(), 4);
                    for &region in &blur_regions {
                        let Some(mut rect) = effects::scissor_of(region, (width, height)) else { continue };
                        rect.x += shake[0].round() as i32;
                        rect.y += shake[1].round() as i32;
                        SDL_SetGPUScissor(pass, &rect);
                        SDL_DrawGPUPrimitives(pass, 3, 1, 0, 0);
                    }
                    SDL_SetGPUScissor(pass, &SDL_Rect { x: 0, y: 0, w: width as i32, h: height as i32 });
                }
                if chained {
                    // VPX's antialiasing and sharpening passes, the last into the window, the readout over it
                    SDL_EndGPURenderPass(pass);
                    let last = self.record_post(command, target, width, height);
                    if !last.is_null() {
                        readout(last, self.overlay_post);
                        SDL_EndGPURenderPass(last);
                    }
                } else {
                    readout(pass, self.overlay_post);
                    SDL_EndGPURenderPass(pass);
                }
            }
            // the pause menu over everything, when it shows
            self.record_menu(command, target, width, height, input.text_turn);
            self.frames_drawn.set(self.frames_drawn.get().wrapping_add(1));
        }
    }

    /// Draws one frame off screen and returns it as RGBA pixels, for
    /// pictures taken by the player's test mode.
    pub fn capture(&mut self, input: &FrameInput, width: u32, height: u32) -> Result<Vec<u8>, String> {
        self.ensure_targets(width, height, input)?;
        self.ensure_post(width, height)?;
        let (rw, rh) = self.render_size(width, height);
        self.ensure_effects(rw, rh)?;
        self.ensure_still(rw, rh)?;
        self.draw_still(input, width, height)?;
        if self.hdr && !self.prev_valid.get() {
            // VPX's look: the ball mirrors the playfield from the frame before, which a picture of
            // another size than the window's has none of; one is drawn first, the frame counter kept.
            // With VPX's dynamic occlusion, which builds up frame by frame, 30, as VPX's own pictures
            // are drawn after 30 frames, its frame counter held (tools/vpx-tester)
            let warm_frames = if self.ao_mode() == 2 { 30 } else { 1 };
            let warm = self.texture(self.format, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
            let frames = self.frames_drawn.get();
            unsafe {
                let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
                self.update_text_boxes(command, input.text_boxes, width);
                self.update_dmd(command, input.dmd);
                self.update_animations(command, input);
                self.update_ramps(command, input);
                self.update_lights(input);
                self.present_extras(command, input, true);
                for _ in 0..warm_frames {
                    self.record(command, warm, width, height, input, self.stage());
                    self.frames_drawn.set(frames);
                }
                let submitted = SDL_SubmitGPUCommandBuffer(command);
                SDL_ReleaseGPUTexture(self.device, warm);
                check(submitted, "draw the frame before the picture")?;
            }
        }
        let target = self.texture(self.format, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
        unsafe {
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            self.update_text_boxes(command, input.text_boxes, width);
            self.update_dmd(command, input.dmd);
            self.update_animations(command, input);
            self.update_ramps(command, input);
            self.update_lights(input);
            self.present_extras(command, input, true);
            self.record(command, target, width, height, input, self.stage());
            // a screen in HDR: its picture made an ordinary sRGB one, as a screenshot is kept
            self.read_back(command, target, width, height, self.format, self.hdr10.map(|h| h.sdr_white))
        }
    }

    /// Sets the frame's number, which the dither and the occlusion's noise
    /// follow: the player's test mode numbers its pictures by the game's
    /// clock, so a picture is the same however many frames came before it.
    pub fn set_frame_number(&self, frame: u32) {
        self.frames_drawn.set(frame);
    }

    /// Forgets the frames drawn so far, so the next picture taken starts
    /// afresh: the frame before it and the occlusion built up over 30 frames
    /// drawn first, as VPX's own pictures are, whatever came before.
    pub fn start_afresh(&self) {
        self.prev_valid.set(false);
    }

    /// Draws the score display's window off screen and returns it as RGBA
    /// pixels, for pictures taken by the player's test mode.
    pub fn capture_display(
        &mut self,
        frame: Option<&crate::dmd::DmdFrame>,
        segments: Option<&crate::segments::SegmentsNow>,
        backglass: Option<&std::sync::Arc<crate::b2s::BackglassNow>>,
        pup: Option<&std::sync::Arc<crate::pup::draw::PupNow>>,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, String> {
        let format = self.display.as_ref().map(|d| d.format).ok_or("the score display has no window")?;
        let panel = self.shows_panel(backglass);
        if let Some(layers) = Self::pup_layers(pup, 2).filter(|_| panel.is_none() || self.pack_first[1]) {
            let mut p = match self.display.as_mut().and_then(|d| d.pup.take()) {
                Some(p) => p,
                None => pup::PupDrawing::new(format, self.page_pipeline(format)?),
            };
            let rgba = self.capture_pup(&mut p, layers, width, height, false);
            if let Some(d) = &mut self.display {
                d.pup = Some(p);
            }
            return rgba;
        }
        if let Some(now) = panel {
            return self.capture_panel(now, frame, width, height);
        }
        let target = self.texture(format, SDL_GPU_TEXTUREUSAGE_COLOR_TARGET, width, height, 1, SDL_GPU_SAMPLECOUNT_1)?;
        unsafe {
            let command = non_null(SDL_AcquireGPUCommandBuffer(self.device), "acquire command buffer")?;
            self.update_dmd(command, frame);
            let plan = self.plan_display(frame, segments, (width, height));
            self.record_display(command, target, width, height, frame, &plan);
            self.read_back(command, target, width, height, format, None)
        }
    }

    /// Submits `command`, whose drawing ends in `target`, waits for it, and
    /// returns `target` as RGBA pixels; `target` is released.
    /// The picture drawn into `target` read back as RGBA, 8 bits a colour: a window in HDR10's format
    /// (10 bits a colour) made an ordinary picture, its PQ values decoded with the screen's SDR white
    /// (`pq_white`, nits) or, without it, the sRGB values VPX's HDR look writes there shortened.
    unsafe fn read_back(&self, command: *mut SDL_GPUCommandBuffer, target: *mut SDL_GPUTexture, width: u32, height: u32, format: SDL_GPUTextureFormat, pq_white: Option<f32>) -> Result<Vec<u8>, String> {
        let size = width * height * 4;
        unsafe {
            let mut info: SDL_GPUTransferBufferCreateInfo = zeroed();
            info.usage = SDL_GPU_TRANSFERBUFFERUSAGE_DOWNLOAD;
            info.size = size;
            let transfer = SDL_CreateGPUTransferBuffer(self.device, &info);
            if transfer.is_null() {
                SDL_CancelGPUCommandBuffer(command);
                SDL_ReleaseGPUTexture(self.device, target);
                return Err(format!("create transfer buffer: {}", sdl_error()));
            }
            let copy = SDL_BeginGPUCopyPass(command);
            let mut region: SDL_GPUTextureRegion = zeroed();
            region.texture = target;
            region.w = width;
            region.h = height;
            region.d = 1;
            let mut destination: SDL_GPUTextureTransferInfo = zeroed();
            destination.transfer_buffer = transfer;
            SDL_DownloadFromGPUTexture(copy, &region, &destination);
            SDL_EndGPUCopyPass(copy);
            let fence = non_null(SDL_SubmitGPUCommandBufferAndAcquireFence(command), "submit the capture")?;
            SDL_WaitForGPUFences(self.device, true, &fence, 1);
            SDL_ReleaseGPUFence(self.device, fence);
            let mapped = SDL_MapGPUTransferBuffer(self.device, transfer, false) as *const u8;
            let mut rgba = if mapped.is_null() { Vec::new() } else { std::slice::from_raw_parts(mapped, size as usize).to_vec() };
            SDL_UnmapGPUTransferBuffer(self.device, transfer);
            SDL_ReleaseGPUTransferBuffer(self.device, transfer);
            SDL_ReleaseGPUTexture(self.device, target);
            if rgba.is_empty() {
                return Err(format!("read the capture: {}", sdl_error()));
            }
            if format == SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM || format == SDL_GPU_TEXTUREFORMAT_B8G8R8A8_UNORM_SRGB {
                rgba.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
            } else if format == SDL_GPU_TEXTUREFORMAT_R10G10B10A2_UNORM {
                hdr10::to_srgb8(&mut rgba, pq_white);
            }
            Ok(rgba)
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            SDL_WaitForGPUIdle(self.device);
            let text_boxes: Vec<*mut SDL_GPUTexture> = self.text_box_textures.drain(..).map(|(t, _)| t).collect();
            let env = if self.env_owned { self.env_texture } else { null_mut() };
            let ball_env = if self.ball_env_owned { self.ball_env } else { null_mut() };
            let ball_decal = if self.ball_decal_owned { self.ball_decal } else { null_mut() };
            let (vpx_ball_env, no_decal) = (self.vpx_ball_env, self.no_decal);
            let mut mirrors = std::mem::take(&mut self.mirror_textures);
            mirrors.push(self.mirror_blur);
            mirrors.push(self.refraction_blur);
            mirrors.extend(self.refraction_textures.drain(..).chain(self.refraction_depths.drain(..)));
            for t in self.textures.drain(..).chain(text_boxes).chain(mirrors).chain([self.white, self.font, self.msaa, self.depth, self.dmd_texture, self.hdr_target, self.color_grade, self.irradiance, env, self.bloom[0], self.bloom[1], ball_env, ball_decal, vpx_ball_env, no_decal, self.ball_override_env, self.ball_override_decal, self.prev_hdr]) {
                if !t.is_null() {
                    SDL_ReleaseGPUTexture(self.device, t);
                }
            }
            for s in [self.sampler, self.font_sampler, self.flat_sampler, self.clamp_sampler, self.env_sampler].into_iter().chain(self.trilinear) {
                if !s.is_null() {
                    SDL_ReleaseGPUSampler(self.device, s);
                }
            }
            for b in [self.vertex_buffer, self.index_buffer, self.overlay_buffer] {
                if !b.is_null() {
                    SDL_ReleaseGPUBuffer(self.device, b);
                }
            }
            if !self.dmd_transfer.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, self.dmd_transfer);
            }
            if !self.anim_transfer.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, self.anim_transfer);
            }
            if !self.ramp_transfer.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, self.ramp_transfer);
            }
            if !self.overlay_transfer.is_null() {
                SDL_ReleaseGPUTransferBuffer(self.device, self.overlay_transfer);
            }
            for p in [
                self.opaque,
                self.blended,
                self.glow,
                self.bulb_modulate_3d,
                self.bulb_add_3d,
                self.overlay,
                self.sprite,
                self.backdrop_light,
                self.bulb_modulate,
                self.bulb_add,
                self.flasher_blend,
                self.flasher_modulate,
                self.flasher_add,
                self.unshaded_add,
                self.dmd_backdrop,
                self.dmd_opaque,
                self.dmd_blend,
                self.dmd_add,
                self.tonemap,
                self.overlay_post,
                self.vpx_opaque,
                self.vpx_opaque_no_depth,
                self.vpx_kicker_plate,
                self.vpx_blended,
                self.vpx_blended_depth,
                self.vpx_classic_light,
                self.blended_depth,
                self.bloom_cut,
                self.blur_h,
                self.blur_v,
                self.light_buffer,
                self.blur19_h,
                self.blur19_v,
                self.vpx_ball,
                self.vpx_mirror_add,
                self.vpx_refract,
                self.vpx_refract_depth,
                self.refraction_copy,
                self.blur_kernel_h,
                self.blur_kernel_v,
                self.bulb_vpx_3d,
                self.flasher_vpx,
                self.bulb_vpx,
            ] {
                if !p.is_null() {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, p);
                }
            }
            for (_, twins) in self.culled.drain() {
                for p in twins {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, p);
                }
            }
            self.release_backglass();
            self.release_topper();
            self.release_post();
            self.release_effects();
            self.release_still();
            self.release_segments();
            for (_, texture) in self.glass_textures.get_mut().drain() {
                if !texture.is_null() {
                    SDL_ReleaseGPUTexture(self.device, texture);
                }
            }
            if let Some(mut d) = self.display.take() {
                if let Some(p) = d.panel.take() {
                    self.release_page(p);
                }
                if let Some(p) = d.pup.take() {
                    self.release_pup(p);
                }
                if !d.pipeline.is_null() {
                    SDL_ReleaseGPUGraphicsPipeline(self.device, d.pipeline);
                }
                if let Some(window) = d.surface.window() {
                    SDL_ReleaseWindowFromGPUDevice(self.device, window);
                }
            }
            self.release_embedded();
            self.release_menu();
            if self.claimed {
                SDL_ReleaseWindowFromGPUDevice(self.device, self.window);
            }
            SDL_DestroyGPUDevice(self.device);
        }
    }
}

/// A still part, each frame: its mirror's moving reflections added over it
/// (VPX's reflection-only pass, among the see-through parts), and in VPX's
/// look a lit bulb's glow over its glass.
fn over_still<'a>(out: &mut Vec<(f32, Transparent<'a>)>, draw: &'a crate::scene::Draw, look: MeshLook, model: &Mat4, mirror: Option<(usize, f32)>, hdr: bool, depth_bias: f32) {
    let at = model.transform_no_perspective(draw.sort_point);
    let key = depth_bias - at.z;
    if hdr
        && !look.added
        && let Some(mirror) = mirror
    {
        out.push((key, Transparent::MirrorOnly(draw, look, mirror)));
    }
    if hdr && let Some(l) = draw.bulb {
        out.push((key, Transparent::BulbHull(draw, l)));
    }
}


/// Samples per pixel, as a number.
fn samples_of(count: SDL_GPUSampleCount) -> u32 {
    match count {
        SDL_GPU_SAMPLECOUNT_8 => 8,
        SDL_GPU_SAMPLECOUNT_4 => 4,
        SDL_GPU_SAMPLECOUNT_2 => 2,
        _ => 1,
    }
}

fn bytes_of_slice<T: Copy>(s: &[T]) -> &[u8] {
    // SAFETY: plain-old-data vertex and index types, read as bytes
    unsafe { std::slice::from_raw_parts(s.as_ptr().cast(), std::mem::size_of_val(s)) }
}

const ATLAS_COLUMNS: u32 = 16;

/// The font as a texture: 16 glyphs per row, white with the glyph in alpha.
fn font_atlas() -> (u32, u32, Vec<u8>) {
    let count = u32::from(font::LAST - font::FIRST) + 1;
    let rows = count.div_ceil(ATLAS_COLUMNS);
    let (w, h) = (ATLAS_COLUMNS * 8, rows * 8);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for g in 0..count {
        let glyph = font::glyph(font::FIRST + g as u8);
        let (gx, gy) = ((g % ATLAS_COLUMNS) * 8, (g / ATLAS_COLUMNS) * 8);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    let at = (((gy + row as u32) * w + gx + col) * 4) as usize;
                    rgba[at..at + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    (w, h, rgba)
}

/// A rectangle in pixels (left, top, right, bottom) showing part of a
/// picture (u0, v0, u1, v1), as two triangles.
fn quad(r: [f32; 4], uv: [f32; 4], color: [f32; 4]) -> [OverlayVertex; 6] {
    let v = |x: f32, y: f32, u: f32, w: f32| OverlayVertex { position: [x, y], uv: [u, w], color };
    [
        v(r[0], r[1], uv[0], uv[1]),
        v(r[2], r[1], uv[2], uv[1]),
        v(r[0], r[3], uv[0], uv[3]),
        v(r[2], r[1], uv[2], uv[1]),
        v(r[2], r[3], uv[2], uv[3]),
        v(r[0], r[3], uv[0], uv[3]),
    ]
}

/// Two triangles per character, each with a dark shadow behind it; turned `turn` quarter turns
/// into the window (`window`: its size in pixels), as VPX's LiveUI turns its overlay — laid out
/// in the turned screen's own width and height, then rotated and moved back into the window
/// (`matRotate * matTranslate`).
fn overlay_vertices(text: &[Text], turn: u32, window: (f32, f32)) -> Vec<OverlayVertex> {
    let (w, h) = window;
    let place = |[x, y]: [f32; 2]| match turn % 4 {
        1 => [w - y, x],
        2 => [w - x, h - y],
        3 => [y, h - x],
        _ => [x, y],
    };
    let count = u32::from(font::LAST - font::FIRST) + 1;
    let rows = count.div_ceil(ATLAS_COLUMNS) as f32;
    let mut out = Vec::new();
    for line in text {
        for (color, offset) in [([0.0, 0.0, 0.0, 0.8], line.scale), (line.color, 0.0)] {
            let mut x = line.x + offset;
            let y = line.y + offset;
            let size = 8.0 * line.scale;
            for c in line.text.bytes() {
                if (font::FIRST..=font::LAST).contains(&c) {
                    let g = u32::from(c - font::FIRST);
                    let (u0, v0) = ((g % ATLAS_COLUMNS) as f32 / ATLAS_COLUMNS as f32, (g / ATLAS_COLUMNS) as f32 / rows);
                    let (u1, v1) = (u0 + 1.0 / ATLAS_COLUMNS as f32, v0 + 1.0 / rows);
                    let quad = [
                        ([x, y], [u0, v0]),
                        ([x + size, y], [u1, v0]),
                        ([x, y + size], [u0, v1]),
                        ([x + size, y], [u1, v0]),
                        ([x + size, y + size], [u1, v1]),
                        ([x, y + size], [u0, v1]),
                    ];
                    out.extend(quad.iter().map(|&(position, uv)| OverlayVertex { position: place(position), uv, color }));
                }
                x += size;
            }
        }
    }
    out
}
