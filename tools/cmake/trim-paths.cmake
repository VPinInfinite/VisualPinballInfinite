# Included after the first project() of a library we build (CMAKE_PROJECT_INCLUDE, which the
# setup scripts pass): the compiler records the source files' paths from the library's own folder
# down, not the folder it was built in, and the linker records its debug file by name alone, so the
# library carries no folder of the PC that built it. Visual Studio's compiler takes /d1trimfile
# (the folder given with a closing backslash); GCC and Clang take -ffile-prefix-map.
include_guard(GLOBAL)
if(CMAKE_C_COMPILER_ID STREQUAL "MSVC" OR CMAKE_CXX_COMPILER_ID STREQUAL "MSVC")
  file(TO_NATIVE_PATH "${CMAKE_SOURCE_DIR}/" vpi_source_prefix)
  add_compile_options("/d1trimfile:${vpi_source_prefix}")
  # the debug file's name written out by CMake: the linker's own %_PDB% reaches it as %%_PDB%%
  add_link_options("/PDBALTPATH:$<TARGET_PDB_FILE_NAME:$<TARGET_PROPERTY:NAME>>")
elseif(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang" OR CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
  # C and C++ only: with these compilers CMake hands compile options to the resource compiler too
  add_compile_options("$<$<COMPILE_LANGUAGE:C,CXX>:-ffile-prefix-map=${CMAKE_SOURCE_DIR}/=>")
endif()
