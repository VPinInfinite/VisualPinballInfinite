//! Builds the player's one piece of C++: the bridge to FFmpeg for PinUP packs' media
//! (`native/pup_av.cpp`), compiled against FFmpeg's headers in `vendor/ffmpeg`. FFmpeg
//! itself isn't linked: the bridge loads its libraries when a pack plays.

fn main() {
    println!("cargo:rerun-if-changed=native/pup_av.cpp");
    println!("cargo:rerun-if-changed=../../vendor/ffmpeg/include");
    cc::Build::new()
        .cpp(true)
        .file("native/pup_av.cpp")
        .include("../../vendor/ffmpeg/include")
        .flag_if_supported("/std:c++17")
        .warnings(true)
        .compile("vpi_pup_av");
}
