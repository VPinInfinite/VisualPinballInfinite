# vendor

Third-party source we build from a copy of our own: changed by us (SDL, SDL_ttf's binding), or
needed to build and not fetched (FFmpeg's headers).

## sdl3-src — SDL 3.4.18, with four changes of ours

The `sdl3-src` crate (3.4.18, the source of [SDL](https://github.com/libsdl-org/SDL) that
`sdl3-sys` builds) as published on crates.io, used in place of the published crate through
`[patch.crates-io]` in the workspace's `Cargo.toml`. Left out of the copy, as a CMake build
doesn't use them: SDL's `VisualC`, `VisualC-GDK`, `Xcode`, `android-project` and `docs` folders,
`Android.mk`, and its notes for AI tools (`AGENTS.md` and the like). SDL is under the zlib
license (`sdl3-src/LICENSE.md`, `sdl3-src/SDL/LICENSE.txt`); as it asks, this is an altered
version, marked as such: two changes are in `SDL/src/gpu/d3d12/SDL_gpu_d3d12.c` and two in
`SDL/CMakeLists.txt`, each marked `Visual Pinball Infinite: altered from SDL 3.4.18`.

SDL's Direct3D 12 code copies each draw's descriptors (the records telling the graphics card
which pictures and samplers a shader reads) into heaps of 65,536 and 2,048. Before a copy it only
checked that the heap wasn't already full, so a draw binding six samplers with two places left
wrote four past the heap's end; the graphics driver then wrote outside the heap, and the player
crashed now and then (about one picture in 40, back to back) with an invalid memory write in
NVIDIA's driver. The changes:

1. `D3D12_INTERNAL_BindGraphicsResources` counts the descriptors a draw will write first; when
   they don't fit in the bound heaps, new heaps are bound before any is written, and every
   descriptor table is written again into them (a table written before the switch would point
   into heaps no longer bound).
2. `D3D12_INTERNAL_WriteGPUDescriptors` switches heaps when a write wouldn't fit, not only when
   the heap is full (a safety net for the other paths that write descriptors).

The third is in SDL's build file. With Visual Studio's compiler, which has no `__FILE_NAME__`,
SDL's assertions name each source file by its full path as built, so the folder the program was
built in went into it. `/d1trimfile` makes the compiler record the paths from SDL's own folder down
instead (`src\gpu\SDL_gpu.c`).

A fourth, beside it: cmake-rs hands Visual Studio's Release configuration flags of its own, which
leave out CMake's `/O2 /Ob2 /DNDEBUG`, so SDL was built unoptimised, its debug assertions on; they're
added back. Measured before it became the default: every table's picture the same, no frame rate
or key-to-frame difference on a fast PC.

**Updating SDL:** copy the new `sdl3-src` crate from the Cargo registry over this folder (leaving
out the folders above), re-apply the four marked changes, build, and run the picture check.
Cargo doesn't see a change to SDL's own files here (the crate's Rust part reads none of them), so
after one, clear SDL's last build first — `cargo clean -p sdl3-sys --release`, with
`--target-dir target\release-build` for the release build — and the next build makes SDL again. Drop
the copy (and the `[patch]`) once SDL itself writes past no heap's end. SDL asks not to be sent
AI-generated reports or code; we send none.

## ffmpeg — FFmpeg's headers, for the PinUP media bridge

`ffmpeg/include` holds the 38 public headers of [FFmpeg](https://ffmpeg.org) (8.0: libavutil 60,
libavcodec 62, libavformat 62, libswscale 9, libswresample 6) that our bridge to FFmpeg
(`engine/player/native/pup_av.cpp`) includes, unchanged, copied from the third-party package of
VPX 5436's build (`5436-third-party-windows-x64-Release/include`), whose FFmpeg libraries the
player loads when a PinUP pack plays. Only the headers the bridge reaches are kept (`cl
/showIncludes` lists them). They're under the GNU LGPL 2.1 or later, as each file says; the
license's text is beside them (`COPYING.LGPLv2.1`, FFmpeg's own, from its repository). The bridge
is compiled against them and calls the libraries through a table it fills at run time, so FFmpeg
is never linked: the player runs without it, and a pack's media plays once its libraries are found
(beside the player; a development build also looks in our own build of them, `tools/thirdparty`,
and in that package's `runtime-libs/windows-x64`).

**Updating FFmpeg:** copy the new version's headers for the same list over these, change the
libraries' version numbers if their majors moved (the bridge reads them from the headers), and
check a pack plays.

## sdl3-ttf-sys — the SDL_ttf binding, with three changes of ours

The `sdl3-ttf-sys` crate (0.7.1, SDL_ttf 3.2.2; zlib, `sdl3-ttf-sys/LICENSE.md`) as published on
crates.io, used through `[patch.crates-io]`: it builds SDL_ttf with its own FreeType and HarfBuzz
(`sdl3-ttf-src`, fetched as usual) into the player for the labels of PinUP packs. Its build script
has one change, marked `Visual Pinball Infinite: altered from sdl3-ttf-sys 0.7.1`: on Windows, when
the program links the C runtime into itself (`crt-static`, `.cargo/config.toml`), SDL_ttf,
FreeType and HarfBuzz are built for that runtime too (`CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded`);
CMake otherwise picks the runtime kept in a DLL, and the player doesn't link. A second change, marked
the same way: a release build defines `NDEBUG` for SDL_ttf, FreeType and HarfBuzz, as CMake's own
Release configuration does (cmake-rs replaces CMake's flags with its own, which leave it out), so
their C asserts, which carry each source file's full path, aren't built in. A third, beside it: with
Visual Studio's compiler a release build is optimised too (`/O2 /Ob2`, also left out by cmake-rs).

**Updating it:** copy the new crate from the Cargo registry over this folder, re-apply the marked
changes, build the player, and check a pack's labels (`cargo test -p player fonts`).

