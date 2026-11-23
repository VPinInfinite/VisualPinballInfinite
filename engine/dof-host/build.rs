//! Builds the host's one piece of C++: the bridge to libdof (`native/dof.cpp`), whose
//! interface is C++ classes. libdof itself isn't linked: the bridge loads it at start.

fn main() {
    println!("cargo:rerun-if-changed=native/dof.cpp");
    cc::Build::new().cpp(true).file("native/dof.cpp").flag_if_supported("/std:c++17").flag_if_supported("/EHsc").warnings(true).compile("vpi_dof");
}
