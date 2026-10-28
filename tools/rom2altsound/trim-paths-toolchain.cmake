# Handed to CMake by the cmake crate as rom2altsound builds its PinMAME (setup.ps1 sets the
# CMAKE_TOOLCHAIN_FILE environment variable): nothing about the compiler changed, only
# trim-paths.cmake beside it included after each project().
set(CMAKE_PROJECT_INCLUDE "${CMAKE_CURRENT_LIST_DIR}/trim-paths.cmake")
