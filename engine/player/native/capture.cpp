// Visual Pinball Infinite (GPL-3.0-or-later): a part of the desktop captured as it shows, for a
// table's preview made from its own program's windows (a Future Pinball table, which the player
// doesn't draw). Windows' Desktop Duplication (DXGI): the screen holding the part duplicated, each
// new picture of it copied out as RGBA, rows top first, turned back where the screen is turned
// (Desktop Duplication hands a turned screen's picture as the screen's own, unturned). The mouse's
// pointer isn't in its pictures. Ours.

#include <stdint.h>
#include <string.h>
#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>

#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")

struct VpiCapture
{
   ID3D11Device *device = nullptr;
   ID3D11DeviceContext *context = nullptr;
   IDXGIOutput1 *output = nullptr;
   IDXGIOutputDuplication *dup = nullptr;
   ID3D11Texture2D *staging = nullptr;
   // the part, as shown: its place on the screen's own picture and its size
   int x = 0, y = 0, width = 0, height = 0;
   // the screen's turn, and its picture's size as Desktop Duplication hands it (unturned)
   DXGI_MODE_ROTATION rotation = DXGI_MODE_ROTATION_IDENTITY;
   int screenW = 0, screenH = 0;
   // the copied part's box on the unturned picture, and its size
   D3D11_BOX box = {};
   int partW = 0, partH = 0;
   // since when Windows has kept the screen from being duplicated again (0: it hasn't)
   ULONGLONG lostAt = 0;
};

// How long Windows may keep a screen from being duplicated again before it's given up: a full-screen
// game changing the screen's mode takes it away for a moment.
static const ULONGLONG LOST_FOR_MS = 10000;

static void release(VpiCapture *c)
{
   if (c->staging) c->staging->Release();
   if (c->dup) c->dup->Release();
   if (c->output) c->output->Release();
   if (c->context) c->context->Release();
   if (c->device) c->device->Release();
   c->staging = nullptr;
   c->dup = nullptr;
   c->output = nullptr;
   c->context = nullptr;
   c->device = nullptr;
}

static void say(char *error, int32_t len, const char *why)
{
   if (error && len > 0)
      strncpy_s(error, (size_t)len, why, _TRUNCATE);
}

// Desktop Duplication on the screen holding the part, from scratch (again, once Windows has taken
// it away: another mode, a full-screen game's own, a secure desktop).
static bool duplicate(VpiCapture *c, int left, int top, char *error, int32_t error_len)
{
   release(c);
   IDXGIFactory1 *factory = nullptr;
   if (FAILED(CreateDXGIFactory1(__uuidof(IDXGIFactory1), (void **)&factory)))
   {
      say(error, error_len, "DXGI couldn't be reached");
      return false;
   }
   IDXGIAdapter1 *adapter = nullptr;
   IDXGIOutput *found = nullptr;
   DXGI_OUTPUT_DESC desc = {};
   for (UINT a = 0; !found && factory->EnumAdapters1(a, &adapter) != DXGI_ERROR_NOT_FOUND; a++)
   {
      IDXGIOutput *out = nullptr;
      for (UINT o = 0; adapter->EnumOutputs(o, &out) != DXGI_ERROR_NOT_FOUND; o++)
      {
         DXGI_OUTPUT_DESC d = {};
         out->GetDesc(&d);
         const RECT &r = d.DesktopCoordinates;
         if (d.AttachedToDesktop && left >= r.left && left < r.right && top >= r.top && top < r.bottom)
         {
            found = out;
            desc = d;
            break;
         }
         out->Release();
      }
      if (!found)
      {
         adapter->Release();
         adapter = nullptr;
      }
   }
   factory->Release();
   if (!found)
   {
      say(error, error_len, "no screen shows that part of the desktop");
      return false;
   }
   HRESULT hr = D3D11CreateDevice(adapter, D3D_DRIVER_TYPE_UNKNOWN, nullptr, 0, nullptr, 0, D3D11_SDK_VERSION, &c->device, nullptr, &c->context);
   adapter->Release();
   if (FAILED(hr))
   {
      found->Release();
      say(error, error_len, "Direct3D 11 couldn't be started on that screen's graphics card");
      return false;
   }
   hr = found->QueryInterface(__uuidof(IDXGIOutput1), (void **)&c->output);
   found->Release();
   if (FAILED(hr))
   {
      say(error, error_len, "the screen can't be duplicated (DXGI 1.2 is needed)");
      release(c);
      return false;
   }
   hr = c->output->DuplicateOutput(c->device, &c->dup);
   if (FAILED(hr))
   {
      say(error, error_len, hr == E_ACCESSDENIED ? "Windows won't let the screen be captured now (a secure desktop?)" : hr == DXGI_ERROR_NOT_CURRENTLY_AVAILABLE ? "too many programs capture that screen already" : "the screen couldn't be duplicated");
      release(c);
      return false;
   }
   DXGI_OUTDUPL_DESC dd = {};
   c->dup->GetDesc(&dd);
   c->rotation = dd.Rotation;
   c->screenW = (int)dd.ModeDesc.Width;
   c->screenH = (int)dd.ModeDesc.Height;
   // the part on the screen as shown, then on its unturned picture
   const RECT &r = desc.DesktopCoordinates;
   const int rx = left - r.left, ry = top - r.top, rw = c->width, rh = c->height;
   const int W = c->screenW, H = c->screenH;
   int bl, bt, bw, bh;
   switch (c->rotation)
   {
   case DXGI_MODE_ROTATION_ROTATE90: bl = ry; bt = H - (rx + rw); bw = rh; bh = rw; break;
   case DXGI_MODE_ROTATION_ROTATE270: bl = W - (ry + rh); bt = rx; bw = rh; bh = rw; break;
   case DXGI_MODE_ROTATION_ROTATE180: bl = W - (rx + rw); bt = H - (ry + rh); bw = rw; bh = rh; break;
   default: bl = rx; bt = ry; bw = rw; bh = rh; break;
   }
   if (bl < 0 || bt < 0 || bl + bw > W || bt + bh > H)
   {
      say(error, error_len, "that part reaches past its screen's edge");
      release(c);
      return false;
   }
   c->box = D3D11_BOX { (UINT)bl, (UINT)bt, 0, (UINT)(bl + bw), (UINT)(bt + bh), 1 };
   c->partW = bw;
   c->partH = bh;
   D3D11_TEXTURE2D_DESC td = {};
   td.Width = (UINT)bw;
   td.Height = (UINT)bh;
   td.MipLevels = 1;
   td.ArraySize = 1;
   td.Format = DXGI_FORMAT_B8G8R8A8_UNORM;
   td.SampleDesc.Count = 1;
   td.Usage = D3D11_USAGE_STAGING;
   td.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
   if (FAILED(c->device->CreateTexture2D(&td, nullptr, &c->staging)))
   {
      say(error, error_len, "a texture to copy it into couldn't be made");
      release(c);
      return false;
   }
   return true;
}

// The part of the desktop at `left`, `top`, `width` × `height` (desktop pixels, on one screen),
// to capture: null where it can't be, `error` saying why.
extern "C" VpiCapture *vpi_capture_open(int32_t left, int32_t top, int32_t width, int32_t height, char *error, int32_t error_len)
{
   // the desktop in its own pixels, whatever Windows scales a screen by
   SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
   if (width <= 0 || height <= 0)
   {
      say(error, error_len, "an empty part");
      return nullptr;
   }
   VpiCapture *c = new VpiCapture();
   c->x = left;
   c->y = top;
   c->width = width;
   c->height = height;
   if (!duplicate(c, left, top, error, error_len))
   {
      delete c;
      return nullptr;
   }
   return c;
}

// Windows' duplication of the screen let go and asked for again at the next ask, the moment noted.
static void lost(VpiCapture *c)
{
   release(c);
   if (c->lostAt == 0)
      c->lostAt = GetTickCount64();
}

// Its next picture, as RGBA (`width` × `height` × 4 bytes, rows top first), waiting up to
// `timeout_ms`: 1 with a new picture in `rgba`, 0 when the screen showed nothing new meanwhile
// (`rgba` as it was) — also while Windows keeps it from being duplicated again for a moment, after
// a mode change or a full screen of a game's own — -1 when it can't be captured any more.
extern "C" int32_t vpi_capture_frame(VpiCapture *c, uint8_t *rgba, int32_t timeout_ms)
{
   if (!c)
      return -1;
   if (!c->dup)
   {
      if (!duplicate(c, c->x, c->y, nullptr, 0))
      {
         // `duplicate` let everything go: kept from it for now, or for good
         if (c->lostAt == 0)
            c->lostAt = GetTickCount64();
         if (GetTickCount64() - c->lostAt > LOST_FOR_MS)
            return -1;
         Sleep((DWORD)(timeout_ms <= 0 ? 0 : timeout_ms < 100 ? timeout_ms : 100));
         return 0;
      }
      c->lostAt = 0;
   }
   DXGI_OUTDUPL_FRAME_INFO info = {};
   IDXGIResource *resource = nullptr;
   HRESULT hr = c->dup->AcquireNextFrame((UINT)timeout_ms, &info, &resource);
   if (hr == DXGI_ERROR_WAIT_TIMEOUT)
      return 0;
   if (FAILED(hr))
   {
      // made again at the next ask: Windows took it away (a mode change, a full screen of its own)
      lost(c);
      return 0;
   }
   int32_t result = 0;
   // only the pointer moved: the picture as it was
   if (info.LastPresentTime.QuadPart != 0 || info.AccumulatedFrames > 0)
   {
      ID3D11Texture2D *texture = nullptr;
      if (SUCCEEDED(resource->QueryInterface(__uuidof(ID3D11Texture2D), (void **)&texture)))
      {
         c->context->CopySubresourceRegion(c->staging, 0, 0, 0, 0, texture, 0, &c->box);
         texture->Release();
         D3D11_MAPPED_SUBRESOURCE mapped = {};
         if (SUCCEEDED(c->context->Map(c->staging, 0, D3D11_MAP_READ, 0, &mapped)))
         {
            const uint8_t *src = (const uint8_t *)mapped.pData;
            const int w = c->width, h = c->height, pw = c->partW, ph = c->partH;
            for (int y = 0; y < h; y++)
            {
               uint8_t *row = rgba + (size_t)y * w * 4;
               for (int x = 0; x < w; x++)
               {
                  // the shown pixel's place on the copied part of the unturned picture
                  int tx, ty;
                  switch (c->rotation)
                  {
                  case DXGI_MODE_ROTATION_ROTATE90: tx = y; ty = ph - 1 - x; break;
                  case DXGI_MODE_ROTATION_ROTATE270: tx = pw - 1 - y; ty = x; break;
                  case DXGI_MODE_ROTATION_ROTATE180: tx = pw - 1 - x; ty = ph - 1 - y; break;
                  default: tx = x; ty = y; break;
                  }
                  const uint8_t *p = src + (size_t)ty * mapped.RowPitch + (size_t)tx * 4;
                  row[x * 4 + 0] = p[2];
                  row[x * 4 + 1] = p[1];
                  row[x * 4 + 2] = p[0];
                  row[x * 4 + 3] = 255;
               }
            }
            c->context->Unmap(c->staging, 0);
            result = 1;
         }
      }
   }
   resource->Release();
   c->dup->ReleaseFrame();
   return result;
}

extern "C" void vpi_capture_close(VpiCapture *c)
{
   if (!c)
      return;
   release(c);
   delete c;
}
