//! Builds the host's one piece of C++: the bridge to libaltsound (`native/altsound.cpp`),
//! whose functions take C++ strings. libaltsound itself isn't linked: the bridge loads it when a
//! game has an AltSound pack.

fn main() {
    println!("cargo:rerun-if-changed=native/altsound.cpp");
    cc::Build::new().cpp(true).file("native/altsound.cpp").flag_if_supported("/std:c++17").warnings(true).compile("vpi_altsound");
}
