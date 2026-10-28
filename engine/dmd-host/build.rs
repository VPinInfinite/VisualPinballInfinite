//! Builds the host's one piece of C++: the bridge to libdmdutil (`native/dmdutil.cpp`),
//! whose interface is C++ classes. The bridge is compiled against libdmdutil's headers and linked
//! by its import library, the library itself loaded only at its first use (delay-loaded), so the
//! program can say where it is first. Found, in order: `VPI_DMDUTIL` (a folder holding `include`
//! and the import library), our own build of it (`vpx-build\libdmdutil`), or VPX 5436's library
//! bundle; with none, the bridge is built without it and says so.

use std::path::{Path, PathBuf};

/// libdmdutil's headers and import library (`dmdutil64.lib`): their folders.
fn find(root: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut places: Vec<(PathBuf, PathBuf)> = Vec::new();
    if let Some(d) = std::env::var_os("VPI_DMDUTIL").map(PathBuf::from) {
        places.push((d.join("include"), d.clone()));
    }
    let own = root.join(r"vpx-build\libdmdutil");
    places.push((own.join("include"), own.join(r"build\Release")));
    let bundle = root.join(r"vpx-build\downloads\5436-third-party-windows-x64-Release");
    places.push((bundle.join("include"), bundle.join(r"build-libs\windows-x64")));
    places.into_iter().find(|(include, lib)| include.join(r"DMDUtil\DMDUtil.h").is_file() && lib.join("dmdutil64.lib").is_file())
}

fn main() {
    println!("cargo:rerun-if-changed=native/dmdutil.cpp");
    println!("cargo:rerun-if-env-changed=VPI_DMDUTIL");
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it")).join(r"..\..");
    let mut build = cc::Build::new();
    build.cpp(true).file("native/dmdutil.cpp").flag_if_supported("/std:c++20").flag_if_supported("/EHsc").warnings(true);
    match find(&root) {
        Some((include, lib)) => {
            println!("cargo:rerun-if-changed={}", lib.join("dmdutil64.lib").display());
            build.include(&include);
            println!("cargo:rustc-link-search=native={}", lib.display());
            println!("cargo:rustc-link-lib=dylib=dmdutil64");
            println!("cargo:rustc-link-lib=delayimp");
            println!("cargo:rustc-link-arg=/DELAYLOAD:dmdutil64.dll");
        }
        None => {
            build.define("VPI_NO_DMDUTIL", None);
            println!("cargo:warning=libdmdutil wasn't found (VPI_DMDUTIL, vpx-build\\libdmdutil or VPX 5436's bundle): vpi-dmd is built without it");
        }
    }
    build.compile("vpi_dmdutil");
}
