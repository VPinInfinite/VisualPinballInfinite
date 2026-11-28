//! Whether a VR headset answers through the PC's VR program (its OpenXR runtime), as VPX looks for
//! one: VPX's own OpenXR loader (`openxr_loader64.dll` beside its BGFX program) loaded, an instance
//! made with no extension, the runtime's name and version asked, then its head-mounted system and
//! that system's name — nothing drawn, no session begun. The runtime is the one Windows has active,
//! or the one `XR_RUNTIME_JSON` names for this program alone. `vpi-player --vr-check --loader <dll>`
//! prints one line of JSON: `runtime`, `version`, `headset`, `ready`, `problem`. Ours.

use std::ffi::{c_char, c_void};

const XR_TYPE_INSTANCE_CREATE_INFO: i32 = 3;
const XR_TYPE_SYSTEM_GET_INFO: i32 = 4;
const XR_TYPE_SYSTEM_PROPERTIES: i32 = 5;
const XR_TYPE_INSTANCE_PROPERTIES: i32 = 32;
const XR_FORM_FACTOR_HEAD_MOUNTED_DISPLAY: i32 = 1;
const XR_ERROR_RUNTIME_FAILURE: i32 = -2;
const XR_ERROR_FORM_FACTOR_UNAVAILABLE: i32 = -35;
const XR_ERROR_RUNTIME_UNAVAILABLE: i32 = -51;

/// OpenXR 1.0, which every runtime takes.
const API_VERSION: u64 = 1 << 48;

#[repr(C)]
struct ApplicationInfo {
    application_name: [u8; 128],
    application_version: u32,
    engine_name: [u8; 128],
    engine_version: u32,
    api_version: u64,
}

#[repr(C)]
struct InstanceCreateInfo {
    ty: i32,
    next: *const c_void,
    create_flags: u64,
    application_info: ApplicationInfo,
    enabled_api_layer_count: u32,
    enabled_api_layer_names: *const *const c_char,
    enabled_extension_count: u32,
    enabled_extension_names: *const *const c_char,
}

#[repr(C)]
struct InstanceProperties {
    ty: i32,
    next: *mut c_void,
    runtime_version: u64,
    runtime_name: [u8; 128],
}

#[repr(C)]
struct SystemGetInfo {
    ty: i32,
    next: *const c_void,
    form_factor: i32,
}

#[repr(C)]
struct SystemProperties {
    ty: i32,
    next: *mut c_void,
    system_id: u64,
    vendor_id: u32,
    system_name: [u8; 256],
    max_swapchain_image_height: u32,
    max_swapchain_image_width: u32,
    max_layer_count: u32,
    orientation_tracking: u32,
    position_tracking: u32,
}

type CreateInstance = unsafe extern "system" fn(*const InstanceCreateInfo, *mut *mut c_void) -> i32;
type GetInstanceProperties = unsafe extern "system" fn(*mut c_void, *mut InstanceProperties) -> i32;
type GetSystem = unsafe extern "system" fn(*mut c_void, *const SystemGetInfo, *mut u64) -> i32;
type GetSystemProperties = unsafe extern "system" fn(*mut c_void, u64, *mut SystemProperties) -> i32;
type DestroyInstance = unsafe extern "system" fn(*mut c_void) -> i32;

/// What the check found.
#[derive(Debug, Default)]
pub struct Found {
    pub runtime: Option<String>,
    pub version: Option<String>,
    pub headset: Option<String>,
    pub ready: bool,
    pub problem: Option<String>,
}

fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn name(into: &mut [u8], what: &str) {
    let room = into.len() - 1;
    for (d, s) in into.iter_mut().zip(what.bytes().take(room)) {
        *d = s;
    }
}

/// The headset checked through the OpenXR loader at `loader`.
#[cfg(windows)]
pub fn check(loader: &std::path::Path) -> Found {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let wide: Vec<u16> = loader.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect();
    // SAFETY: a NUL-ended path; the library stays loaded for the process's short life
    let module = unsafe { LoadLibraryW(wide.as_ptr()) };
    if module.is_null() {
        return Found { problem: Some(format!("VPX's OpenXR library couldn't be loaded: {}", loader.display())), ..Default::default() };
    }
    // SAFETY: the loader's own exported functions, each looked up by its NUL-ended name and called
    // with its own signature (OpenXR's core API)
    unsafe {
        let find = |n: &std::ffi::CStr| GetProcAddress(module, n.as_ptr().cast());
        let (create, props, system, system_props, destroy) = (find(c"xrCreateInstance"), find(c"xrGetInstanceProperties"), find(c"xrGetSystem"), find(c"xrGetSystemProperties"), find(c"xrDestroyInstance"));
        if [create, props, system, system_props, destroy].iter().any(|f| f.is_null()) {
            return Found { problem: Some("VPX's OpenXR library lacks OpenXR's functions".into()), ..Default::default() };
        }
        let create: CreateInstance = std::mem::transmute(create);
        let props: GetInstanceProperties = std::mem::transmute(props);
        let system: GetSystem = std::mem::transmute(system);
        let system_props: GetSystemProperties = std::mem::transmute(system_props);
        let destroy: DestroyInstance = std::mem::transmute(destroy);
        let mut app = ApplicationInfo { application_name: [0; 128], application_version: 1, engine_name: [0; 128], engine_version: 1, api_version: API_VERSION };
        name(&mut app.application_name, "Visual Pinball Infinite headset check");
        name(&mut app.engine_name, "Visual Pinball Infinite");
        let info = InstanceCreateInfo {
            ty: XR_TYPE_INSTANCE_CREATE_INFO,
            next: std::ptr::null(),
            create_flags: 0,
            application_info: app,
            enabled_api_layer_count: 0,
            enabled_api_layer_names: std::ptr::null(),
            enabled_extension_count: 0,
            enabled_extension_names: std::ptr::null(),
        };
        let mut instance: *mut c_void = std::ptr::null_mut();
        let made = create(&info, &mut instance);
        if made < 0 || instance.is_null() {
            let problem = match made {
                XR_ERROR_RUNTIME_UNAVAILABLE => "no VR program is set up on this PC".to_string(),
                XR_ERROR_RUNTIME_FAILURE => "the VR program didn't answer: is it running?".to_string(),
                n => format!("the VR program wouldn't start (OpenXR's error {n})"),
            };
            return Found { problem: Some(problem), ..Default::default() };
        }
        let mut found = Found::default();
        let mut ip = InstanceProperties { ty: XR_TYPE_INSTANCE_PROPERTIES, next: std::ptr::null_mut(), runtime_version: 0, runtime_name: [0; 128] };
        if props(instance, &mut ip) >= 0 {
            found.runtime = Some(text(&ip.runtime_name));
            let v = ip.runtime_version;
            found.version = Some(format!("{}.{}.{}", v >> 48, (v >> 32) & 0xFFFF, v & 0xFFFF_FFFF));
        }
        let get = SystemGetInfo { ty: XR_TYPE_SYSTEM_GET_INFO, next: std::ptr::null(), form_factor: XR_FORM_FACTOR_HEAD_MOUNTED_DISPLAY };
        let mut id = 0u64;
        match system(instance, &get, &mut id) {
            r if r >= 0 => {
                let mut sp = SystemProperties { ty: XR_TYPE_SYSTEM_PROPERTIES, next: std::ptr::null_mut(), system_id: 0, vendor_id: 0, system_name: [0; 256], max_swapchain_image_height: 0, max_swapchain_image_width: 0, max_layer_count: 0, orientation_tracking: 0, position_tracking: 0 };
                if system_props(instance, id, &mut sp) >= 0 {
                    found.headset = Some(text(&sp.system_name)).filter(|n| !n.is_empty());
                }
                found.ready = true;
            }
            XR_ERROR_FORM_FACTOR_UNAVAILABLE => found.problem = Some("no headset answered: is it on, and connected to the VR program?".into()),
            n => found.problem = Some(format!("the VR program found no headset (OpenXR's error {n})")),
        }
        destroy(instance);
        found
    }
}

#[cfg(not(windows))]
pub fn check(_loader: &std::path::Path) -> Found {
    Found { problem: Some("VR is checked on Windows".into()), ..Default::default() }
}

/// `vpi-player --vr-check --loader <dll>`: the check's line of JSON.
pub fn run(args: &[String]) -> Result<(), String> {
    let loader = args.iter().position(|a| a == "--loader").and_then(|i| args.get(i + 1)).ok_or("vpi-player --vr-check --loader <openxr_loader64.dll>")?;
    let f = check(std::path::Path::new(loader));
    println!("{}", serde_json::json!({ "runtime": f.runtime, "version": f.version, "headset": f.headset, "ready": f.ready, "problem": f.problem }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openxrs_structures_laid_out_as_its_header_has_them() {
        assert_eq!(std::mem::size_of::<ApplicationInfo>(), 272);
        assert_eq!(std::mem::size_of::<InstanceCreateInfo>(), 328);
        assert_eq!(std::mem::size_of::<InstanceProperties>(), 152);
        assert_eq!(std::mem::size_of::<SystemGetInfo>(), 24);
        assert_eq!(std::mem::size_of::<SystemProperties>(), 304);
        assert_eq!(text(b"Meta Quest 3\0\0junk"), "Meta Quest 3");
    }
}
