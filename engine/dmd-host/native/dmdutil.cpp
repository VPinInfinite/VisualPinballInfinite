// license:GPL-3.0-or-later
//
// The bridge to libdmdutil (vpinball/libdmdutil, GPL-3.0-or-later): the VPX team's library that
// drives real DMD panels — ZeDMD, Pixelcade, PIN2DMD — as VPX build 5436 uses it (its DMDUtil
// plugin, plugins/dmdutil/DMDUtilPlugin.cpp, GPLv3+, which this follows). Its interface is C++
// classes (DMDUtil::Config and DMDUtil::DMD, its include/DMDUtil); this calls them for Rust from C,
// catching anything they throw, so a fault in libdmdutil ends no more than this program.
// Part of vpi-dmd (Visual Pinball Infinite).
//
// The library is linked by its import library and loaded only at its first use (delay-loaded), so
// this program can say where it is first: beside the program, our own build of it without its Serum
// and VNI parts (tools\dmdutil\setup.ps1), or, while developing, VPX 5436's library bundle. Built
// without the library (VPI_NO_DMDUTIL), every call says it isn't there.

#include <windows.h>

#include <cstdarg>
#include <cstdint>
#include <cstdio>
#include <cstring>

#ifndef VPI_NO_DMDUTIL
#include <delayimp.h>
#pragma warning(push)
#pragma warning(disable : 4251) // members needing a dll-interface, as VPX's plugin disables it
#include "DMDUtil/DMDUtil.h"
#pragma warning(pop)
#endif

extern "C" {

// The panel settings VPX 5436's plugin hands libdmdutil (its [Plugin.DMDUtil] settings).
struct VpiDmdSettings {
    int zedmd;
    const char* zedmd_device;
    int zedmd_debug;
    int zedmd_brightness;
    int zedmd_wifi;
    const char* zedmd_wifi_addr;
    int zedmd_spi;
    int zedmd_spi_speed;
    int zedmd_spi_frame_pause;
    int zedmd_spi_width;
    int zedmd_spi_height;
    int pixelcade;
    const char* pixelcade_device;
    int pin2dmd;
    int dmd_server;
    const char* dmd_server_addr;
    int dmd_server_port;
};

typedef void (*VpiDmdSink)(int level, const char* text);

}

namespace {

VpiDmdSink sink = nullptr;

#ifndef VPI_NO_DMDUTIL
DMDUtil::DMD* dmd = nullptr;
DMDUtil::RGB24DMD* panel = nullptr;

// libdmdutil's log, a line each, as VPX's plugin logs it
void DMDUTILCALLBACK OnLog(DMDUtil_LogLevel level, const char* format, va_list args)
{
    char text[4096];
    if (vsnprintf(text, sizeof(text), format, args) < 0)
        return;
    if (sink)
        sink(static_cast<int>(level), text);
}

// Loads the library and resolves everything it's asked for, catching the delay-load's failure
HRESULT LoadAll()
{
    __try {
        return __HrLoadAllImportsForDll("dmdutil64.dll");
    } __except (GetExceptionCode() == VcppException(ERROR_SEVERITY_ERROR, ERROR_MOD_NOT_FOUND) || GetExceptionCode() == VcppException(ERROR_SEVERITY_ERROR, ERROR_PROC_NOT_FOUND)
                    ? EXCEPTION_EXECUTE_HANDLER
                    : EXCEPTION_CONTINUE_SEARCH) {
        return E_FAIL;
    }
}
#endif

} // namespace

extern "C" {

// Whether this program was built with libdmdutil.
int vpi_dmd_built_in()
{
#ifdef VPI_NO_DMDUTIL
    return 0;
#else
    return 1;
#endif
}

// libdmdutil from `folder` (its DLLs together), loaded now: 1 when it loaded.
int vpi_dmd_load(const wchar_t* folder)
{
#ifdef VPI_NO_DMDUTIL
    (void)folder;
    return 0;
#else
    if (folder && !SetDllDirectoryW(folder))
        return 0;
    return SUCCEEDED(LoadAll()) ? 1 : 0;
#endif
}

// The panel settings and the log, as VPX's plugin sets them when it loads: 1 when set.
int vpi_dmd_configure(const VpiDmdSettings* s, VpiDmdSink log)
{
    sink = log;
#ifdef VPI_NO_DMDUTIL
    (void)s;
    return 0;
#else
    try {
        DMDUtil::Config* c = DMDUtil::Config::GetInstance();
        c->SetLogCallback(OnLog);
        c->SetZeDMD(s->zedmd != 0);
        c->SetZeDMDDevice(s->zedmd_device);
        c->SetZeDMDDebug(s->zedmd_debug != 0);
        c->SetZeDMDBrightness(s->zedmd_brightness);
        c->SetZeDMDWiFiEnabled(s->zedmd_wifi != 0);
        c->SetZeDMDWiFiAddr(s->zedmd_wifi_addr);
        c->SetZeDMDSpiEnabled(s->zedmd_spi != 0);
        c->SetZeDMDSpiSpeed(s->zedmd_spi_speed);
        c->SetZeDMDSpiFramePause(s->zedmd_spi_frame_pause);
        c->SetZeDMDWidth(s->zedmd_spi_width);
        c->SetZeDMDHeight(s->zedmd_spi_height);
        c->SetPixelcade(s->pixelcade != 0);
        c->SetPixelcadeDevice(s->pixelcade_device);
        c->SetPIN2DMD(s->pin2dmd != 0);
        c->SetDMDServer(s->dmd_server != 0);
        c->SetDMDServerAddr(s->dmd_server_addr);
        c->SetDMDServerPort(s->dmd_server_port);
        return 1;
    } catch (...) {
        return 0;
    }
#endif
}

// The DMD made, as VPX's plugin makes it with the first display: the panels looked for when
// `find`, libdmdutil's frame dumps when asked. 1 when made.
int vpi_dmd_start(int find, int dump_txt, int dump_raw)
{
#ifdef VPI_NO_DMDUTIL
    (void)find, (void)dump_txt, (void)dump_raw;
    return 0;
#else
    try {
        dmd = new DMDUtil::DMD();
        if (find)
            dmd->FindDisplays();
        if (dump_txt)
            dmd->DumpDMDTxt();
        if (dump_raw)
            dmd->DumpDMDRaw();
        return 1;
    } catch (...) {
        dmd = nullptr;
        return 0;
    }
#endif
}

// Whether libdmdutil is still looking for panels, and whether it found one.
int vpi_dmd_finding()
{
#ifdef VPI_NO_DMDUTIL
    return 0;
#else
    try {
        return DMDUtil::DMD::IsFinding() ? 1 : 0;
    } catch (...) {
        return 0;
    }
#endif
}

int vpi_dmd_has_display()
{
#ifdef VPI_NO_DMDUTIL
    return 0;
#else
    try {
        return dmd && dmd->HasDisplay() ? 1 : 0;
    } catch (...) {
        return 0;
    }
#endif
}

// A frame in 24-bit colour, red, green and blue a dot (VPX's SRGB888, and its tinted brightness).
int vpi_dmd_rgb24(const uint8_t* data, uint16_t width, uint16_t height)
{
#ifdef VPI_NO_DMDUTIL
    (void)data, (void)width, (void)height;
    return 0;
#else
    try {
        if (!dmd)
            return 0;
        dmd->UpdateRGB24Data(data, width, height);
        return 1;
    } catch (...) {
        return 0;
    }
#endif
}

// A frame in 16-bit colour (VPX's SRGB565).
int vpi_dmd_rgb16(const uint16_t* data, uint16_t width, uint16_t height)
{
#ifdef VPI_NO_DMDUTIL
    (void)data, (void)width, (void)height;
    return 0;
#else
    try {
        if (!dmd)
            return 0;
        dmd->UpdateRGB16Data(data, width, height);
        return 1;
    } catch (...) {
        return 0;
    }
#endif
}

// libdmdutil's own virtual panel in 24-bit colour, `width` × `height`, made for checks with no
// real panel (vpi-dmd --check): it gets every frame as a colour panel would. 1 when made.
int vpi_dmd_virtual(uint16_t width, uint16_t height)
{
#ifdef VPI_NO_DMDUTIL
    (void)width, (void)height;
    return 0;
#else
    try {
        if (!dmd)
            return 0;
        if (panel) {
            dmd->DestroyRGB24DMD(panel);
            panel = nullptr;
        }
        panel = dmd->CreateRGB24DMD(width, height);
        return panel ? 1 : 0;
    } catch (...) {
        return 0;
    }
#endif
}

// What the virtual panel shows now, red, green and blue a dot, into `out` (`capacity` bytes),
// its size into `width` and `height`. 1 when copied.
int vpi_dmd_virtual_read(uint8_t* out, int capacity, uint16_t* width, uint16_t* height)
{
#ifdef VPI_NO_DMDUTIL
    (void)out, (void)capacity, (void)width, (void)height;
    return 0;
#else
    try {
        if (!panel)
            return 0;
        const int length = panel->GetLength();
        if (length > capacity)
            return 0;
        const uint8_t* data = panel->GetData();
        if (!data)
            return 0;
        memcpy(out, data, length);
        *width = static_cast<uint16_t>(panel->GetWidth());
        *height = static_cast<uint16_t>(panel->GetHeight());
        return 1;
    } catch (...) {
        return 0;
    }
#endif
}

// The DMD ended, as VPX's plugin ends it with the game.
void vpi_dmd_finish()
{
#ifndef VPI_NO_DMDUTIL
    try {
        if (dmd && panel)
            dmd->DestroyRGB24DMD(panel);
        panel = nullptr;
        delete dmd;
    } catch (...) {
    }
    dmd = nullptr;
#endif
}

} // extern "C"
