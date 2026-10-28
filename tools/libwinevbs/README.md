# libwinevbs, built from source

[libwinevbs](https://github.com/vpinball/libwinevbs) is Wine's VBScript engine packaged as a library (LGPL-2.1-or-later). The player runs existing tables' scripts in it. We build it ourselves, with MSYS2's UCRT64 toolchain (GCC, CMake, Bison and Wine's `widl`) as its own automated build does, instead of using the copy from VPX's automated build. That lets us fix what our tables need, and later ship a DLL we built.

- `setup.ps1` — clones libwinevbs at the commit we follow (`e8f15a7`, its version 0.10.2, Wine 11.20's code) into `vpx-build\libwinevbs` (git-ignored), applies the patches, builds `winevbs64.dll` and puts MSYS2's `libwinpthread-1.dll` beside it. Needs MSYS2 (`tools\setup\install-dev-tools.ps1` installs it).
- `patches/` — our changes, each marked `Visual Pinball Infinite` in the code (the first, for `TypeName` on a `Scripting.Dictionary`, [vpinball/libwinevbs#39](https://github.com/vpinball/libwinevbs/issues/39), dropped since: libwinevbs's own fix does the same):
  2. A file fence the host sets (`libwinevbs_set_file_fence`): every path the file object opens, creates, lists or looks up is made absolute — a relative path taken from a base folder the host names — and handed to the host, which allows it or not; a refused path fails as if access were denied. The player allows writing only in its data folder for tables, and reading there, beside the table and in the shared scripts' folders (`engine/player/src/script/files.rs`). With it, `GetAbsolutePathName` keeps the rest of its path when a parent folder can't be looked up.
  3. A call's bracketed first argument joins the rest as VBScript joins it.
  4. `FormatNumber` of a number written with an exponent, such as a tiny nudge reading.

`e8f15a7` itself changes only libwinevbs's ATL header for programs built on it (VPX's own objects answer `TypeName` through it since Wine 11.20); the DLL doesn't compile that header, and our script objects answer `TypeName` through their type information (`engine/player/src/script/object.rs`).

The player looks for `winevbs64.dll` beside itself (how it will ship), then this build, then the copy from VPX's automated build in `vpx-build\winevbs`. Built unchanged, our DLL gave exactly the same results as VPX's copy on all 45 snippets of the VBScript engine comparison (`tools\vbs-engines`). Our changes stay under libwinevbs's LGPL, and the DLL stays a separate, replaceable file.
