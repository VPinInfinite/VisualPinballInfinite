// license:BSD-3-Clause
//
// The bridge to libaltsound (vpinball/libaltsound, BSD-3-Clause): PinMAME's alternate sound, the
// library VPX build 5436's AltSound plugin uses (altsound64.dll, commit f4b790a1). Its functions take
// C++ strings, which Rust can't make, so this loads the library and hands them over from C.
// Part of vpi-pinmame (Visual Pinball Infinite): BSD-3-Clause, no GPL code.

#include <windows.h>

#include <cstddef>
#include <cstdint>
#include <string>

namespace {

typedef void (*AudioCallback)(const float* samples, size_t frameCount, uint32_t sampleRate, uint32_t channels, void* userData);

// altsound.h's functions, as altsound64.dll exports them (extern "C", C++ strings by reference)
typedef void (*SetLoggerFn)(const std::string& logPath, int logLevel, bool console);
typedef bool (*InitFn)(const std::string& pinmamePath, const std::string& gameName, uint32_t sampleRate, uint32_t channels, uint32_t bufferSizeFrames);
typedef void (*SetHardwareGenFn)(uint64_t hardwareGen);
typedef void (*SetAudioCallbackFn)(AudioCallback callback, void* userData);
typedef bool (*ProcessCommandFn)(unsigned int cmd, int attenuation);
typedef bool (*ProcessBoardCommandFn)(unsigned int board, unsigned int cmd, int attenuation);
typedef void (*PauseFn)(bool pause);
typedef void (*ShutdownFn)();

struct Api {
    SetLoggerFn setLogger = nullptr;
    InitFn init = nullptr;
    SetHardwareGenFn setHardwareGen = nullptr;
    SetAudioCallbackFn setAudioCallback = nullptr;
    ProcessCommandFn processCommand = nullptr;
    // newer libaltsound only (System 11's two sound boards); else every command goes as board 0's
    ProcessBoardCommandFn processBoardCommand = nullptr;
    PauseFn pause = nullptr;
    ShutdownFn shutdown = nullptr;
};

Api api;

}

// Loads the library at `path` (UTF-16) and finds every function: 1, else 0.
extern "C" int vpi_altsound_load(const wchar_t* path)
{
    HMODULE module = LoadLibraryExW(path, nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (module == nullptr)
        return 0;
    Api a;
    a.setLogger = reinterpret_cast<SetLoggerFn>(GetProcAddress(module, "AltSoundSetLogger"));
    a.init = reinterpret_cast<InitFn>(GetProcAddress(module, "AltSoundInit"));
    a.setHardwareGen = reinterpret_cast<SetHardwareGenFn>(GetProcAddress(module, "AltSoundSetHardwareGen"));
    a.setAudioCallback = reinterpret_cast<SetAudioCallbackFn>(GetProcAddress(module, "AltSoundSetAudioCallback"));
    a.processCommand = reinterpret_cast<ProcessCommandFn>(GetProcAddress(module, "AltSoundProcessCommand"));
    a.processBoardCommand = reinterpret_cast<ProcessBoardCommandFn>(GetProcAddress(module, "AltSoundProcessBoardCommand"));
    a.pause = reinterpret_cast<PauseFn>(GetProcAddress(module, "AltSoundPause"));
    a.shutdown = reinterpret_cast<ShutdownFn>(GetProcAddress(module, "AltSoundShutdown"));
    if (!a.setLogger || !a.init || !a.setHardwareGen || !a.setAudioCallback || !a.processCommand || !a.pause || !a.shutdown)
        return 0;
    api = a;
    return 1;
}

// The log goes in folder `path` (UTF-8), at `level` (ALTSOUND_LOG_LEVEL).
extern "C" void vpi_altsound_set_logger(const char* path, int level)
{
    if (api.setLogger)
        api.setLogger(std::string(path), level, false);
}

// Starts AltSound for `game`, its pack in `pinmamePath\altsound\<game>` (both UTF-8): 1, else 0.
extern "C" int vpi_altsound_init(const char* pinmamePath, const char* game, uint32_t sampleRate, uint32_t channels, uint32_t bufferSizeFrames)
{
    if (!api.init)
        return 0;
    // reading a PinSound pack's folders moves the program's current folder into them (its Windows
    // opendir changes folder to read one), which is put back afterwards
    wchar_t cwd[MAX_PATH];
    const DWORD n = GetCurrentDirectoryW(MAX_PATH, cwd);
    const bool ok = api.init(std::string(pinmamePath), std::string(game), sampleRate, channels, bufferSizeFrames);
    if (n > 0 && n < MAX_PATH)
        SetCurrentDirectoryW(cwd);
    return ok ? 1 : 0;
}

extern "C" void vpi_altsound_set_hardware_gen(uint64_t gen)
{
    if (api.setHardwareGen)
        api.setHardwareGen(gen);
}

extern "C" void vpi_altsound_set_audio_callback(AudioCallback callback, void* userData)
{
    if (api.setAudioCallback)
        api.setAudioCallback(callback, userData);
}

extern "C" int vpi_altsound_process_command(unsigned int board, unsigned int cmd, int attenuation)
{
    if (api.processBoardCommand)
        return api.processBoardCommand(board, cmd, attenuation) ? 1 : 0;
    return api.processCommand && api.processCommand(cmd, attenuation) ? 1 : 0;
}

extern "C" void vpi_altsound_shutdown()
{
    if (api.shutdown)
        api.shutdown();
}
