// license:GPL-3.0-or-later
//
// The bridge to libdof (vpinball/libdof, GPL-3.0-or-later): the VPX team's C++ port of the
// DirectOutput Framework, as VPX build 5436 ships it (dof64.dll, libdof 0.4.7, with its USB
// libraries beside it). Its interface is two C++ classes (DOF::DOF and DOF::Config, its
// include/DOF/DOF.h and Config.h); this loads the library, finds their members by the names the
// library exports them under, and calls them for Rust from C, catching anything they throw, so a
// fault in libdof ends no more than this program. Part of vpi-dof (Visual Pinball Infinite).
//
// On 64-bit Windows a member function is called as a plain function with the object first, which
// is how the members are called here.

#include <windows.h>

#include <cstdarg>
#include <cstdio>

namespace {

// Config.h's log callback: a level (DOF_LogLevel: 0 info, 1 warning, 2 error, 3 debug), a printf
// format and its arguments
typedef void(__stdcall* LogCallback)(int level, const char* format, va_list args);

typedef void* (*GetInstanceFn)();
typedef void (*SetBasePathFn)(void* config, const char* basePath);
typedef void (*SetLogCallbackFn)(void* config, LogCallback callback);
typedef void* (*ConstructFn)(void* self);
typedef void (*DestructFn)(void* self);
typedef void (*InitFn)(void* self, const char* tableFilename, const char* romName);
typedef void (*DataReceiveFn)(void* self, char type, int number, int value);
typedef void (*FinishFn)(void* self);

struct Api {
    GetInstanceFn getInstance = nullptr;
    SetBasePathFn setBasePath = nullptr;
    SetLogCallbackFn setLogCallback = nullptr;
    ConstructFn construct = nullptr;
    DestructFn destruct = nullptr;
    InitFn init = nullptr;
    DataReceiveFn dataReceive = nullptr;
    FinishFn finish = nullptr;
};

Api api;

// The DOF object: DOF.h gives it one member, a pointer; room to spare
alignas(16) unsigned char dof[64];
bool constructed = false;

typedef void (*Sink)(int level, const char* text);
Sink sink = nullptr;

void __stdcall OnLog(int level, const char* format, va_list args)
{
    char text[4096];
    if (vsnprintf(text, sizeof(text), format, args) < 0)
        return;
    if (sink)
        sink(level, text);
}

template <typename T> bool Find(HMODULE module, const char* name, T& out)
{
    out = reinterpret_cast<T>(GetProcAddress(module, name));
    return out != nullptr;
}

}

// Loads the library at `path` (UTF-16), its own libraries found beside it, and finds every member:
// 1, else 0.
extern "C" int vpi_dof_load(const wchar_t* path)
{
    HMODULE module = LoadLibraryExW(path, nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
    if (module == nullptr)
        return 0;
    Api a;
    // the names libdof 0.4.7 exports its members under (MSVC's, as `dumpbin /exports` lists them)
    const bool found = Find(module, "?GetInstance@Config@DOF@@SAPEAV12@XZ", a.getInstance)
        && Find(module, "?SetBasePath@Config@DOF@@QEAAXPEBD@Z", a.setBasePath)
        && Find(module, "?SetLogCallback@Config@DOF@@QEAAXP6AXW4DOF_LogLevel@@PEBDPEAD@Z@Z", a.setLogCallback)
        && Find(module, "??0DOF@0@QEAA@XZ", a.construct)
        && Find(module, "??1DOF@0@QEAA@XZ", a.destruct)
        && Find(module, "?Init@DOF@1@QEAAXPEBD0@Z", a.init)
        && Find(module, "?DataReceive@DOF@1@QEAAXDHH@Z", a.dataReceive)
        && Find(module, "?Finish@DOF@1@QEAAXXZ", a.finish);
    if (!found)
        return 0;
    api = a;
    return 1;
}

// Starts libdof as VPX 5436's DOF plugin does: its log to `logSink`, its configuration in
// `basePath` (UTF-8, ending in a separator: libdof reads `<basePath>directoutputconfig\`), then the
// table `tableFilename` named `romName`. 1, else 0 (thrown, or not loaded).
extern "C" int vpi_dof_start(const char* basePath, Sink logSink, const char* tableFilename, const char* romName)
{
    if (!api.init)
        return 0;
    sink = logSink;
    try {
        void* config = api.getInstance();
        api.setLogCallback(config, OnLog);
        api.setBasePath(config, basePath);
        api.construct(dof);
        constructed = true;
        api.init(dof, tableFilename, romName);
        return 1;
    } catch (...) {
        return 0;
    }
}

// One of the game's events: its letter, number and value. 1, else 0 (thrown, or not started).
extern "C" int vpi_dof_data(char type, int number, int value)
{
    if (!constructed)
        return 0;
    try {
        api.dataReceive(dof, type, number, value);
        return 1;
    } catch (...) {
        return 0;
    }
}

// Ends the table's toys and frees libdof's object.
extern "C" void vpi_dof_finish()
{
    if (!constructed)
        return;
    try {
        api.finish(dof);
    } catch (...) {
    }
    try {
        api.destruct(dof);
    } catch (...) {
    }
    constructed = false;
}
