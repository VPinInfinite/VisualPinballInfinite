# Included after the project() of rom2altsound's PinMAME: the compiler records its source files'
# paths from rom2altsound's own folder down (VPI_TRIM_ROOT, which setup.ps1 sets), not the folder it
# was built in. PinMAME keeps each machine driver's source file name (__FILE__) in the program, and
# its sources sit outside the CMake project's own folder, so tools\cmake\trim-paths.cmake's prefix
# wouldn't reach them.
include_guard(GLOBAL)
if(DEFINED ENV{VPI_TRIM_ROOT} AND (CMAKE_C_COMPILER_ID STREQUAL "MSVC" OR CMAKE_CXX_COMPILER_ID STREQUAL "MSVC"))
  file(TO_NATIVE_PATH "$ENV{VPI_TRIM_ROOT}/" vpi_trim_root)
  add_compile_options("/d1trimfile:${vpi_trim_root}")
endif()
