//! Builds the player's pieces of C++: the bridge to FFmpeg for PinUP packs' media
//! (`native/pup_av.cpp`), compiled against FFmpeg's headers in `vendor/ffmpeg` — FFmpeg itself
//! isn't linked: the bridge loads its libraries when a pack plays — and the desktop's capture for
//! media made from another program's windows (`native/capture.cpp`, Windows' Desktop Duplication).

fn main() {
    println!("cargo:rerun-if-changed=native/pup_av.cpp");
    println!("cargo:rerun-if-changed=native/capture.cpp");
    println!("cargo:rerun-if-changed=../../vendor/ffmpeg/include");
    println!("cargo:rustc-link-lib=d3d11");
    println!("cargo:rustc-link-lib=dxgi");
    cc::Build::new()
        .cpp(true)
        .file("native/pup_av.cpp")
        .file("native/capture.cpp")
        .include("../../vendor/ffmpeg/include")
        .flag_if_supported("/std:c++17")
        .warnings(true)
        .compile("vpi_pup_av");
}
