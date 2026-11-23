#[cfg(feature = "build-from-source")]
const SOURCE_DIR: &str = sdl3_ttf_src::SOURCE_DIR;

const LINK_FRAMEWORK: bool = cfg!(feature = "link-framework");

include!("build-common.rs");

fn main() -> Result<(), Box<dyn Error>> {
    build(|config| {
        let _ = config;
        #[cfg(feature = "build-from-source")]
        {
            if let Some(sdl3_cmake_dir) = env::var_os("DEP_SDL3_CMAKE_DIR") {
                config.define("SDL3_DIR", sdl3_cmake_dir);
            }

            if LINK_FRAMEWORK {
                // !!!FIXME
                panic!(
                    "SDL3_ttf is currently missing a configuration option to build as a framework. You can download the official framework build from <https://github.com/libsdl-org/SDL_ttf/releases>."
                );
                //config.define("SDL_FRAMEWORK", "ON");
            } else if cfg!(feature = "link-static") {
                config.define("BUILD_SHARED_LIBS", "OFF");
            }

            config.define("SDLTTF_SAMPLES", "OFF");

            // Visual Pinball Infinite: altered from sdl3-ttf-sys 0.7.1. A program linking the C runtime
            // into itself (crt-static) needs SDL_ttf, FreeType and HarfBuzz built for that runtime too:
            // CMake picks the runtime from CMAKE_MSVC_RUNTIME_LIBRARY, not from the flags cmake-rs passes.
            if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
                && env::var("CARGO_CFG_TARGET_FEATURE").is_ok_and(|f| f.split(',').any(|f| f == "crt-static"))
            {
                config.define("CMAKE_MSVC_RUNTIME_LIBRARY", "MultiThreaded");
            }
            // Visual Pinball Infinite: altered from sdl3-ttf-sys 0.7.1. A release build leaves C's
            // asserts out, as CMake's own Release configuration does (cmake-rs sets its flags in place
            // of CMake's, /DNDEBUG among them): with them, MSVC's assert() writes each source file's
            // full path into the program.
            if env::var("PROFILE").as_deref() == Ok("release") {
                config.cflag("-DNDEBUG");
                config.cxxflag("-DNDEBUG");
                // Visual Pinball Infinite: altered from sdl3-ttf-sys 0.7.1. And optimised, as CMake's
                // own Release configuration is (/O2 /Ob2, which cmake-rs's flags leave out too).
                if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
                    for flag in ["-O2", "-Ob2"] {
                        config.cflag(flag);
                        config.cxxflag(flag);
                    }
                }
            }

            cmake_vars! { config =>
                SDLTTF_VENDORED,
                SDLTTF_HARFBUZZ,
                SDLTTF_PLUTOSVG,
            }
        }
        Ok(())
    })?;
    Ok(())
}
