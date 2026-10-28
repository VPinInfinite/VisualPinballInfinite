//! Pictures decoded by Windows' GDI+, as FlexDMD decodes every picture (.NET's
//! `Bitmap` is GDI+'s): the same decoders, so the same pixels — a PNG's
//! gamma applied as GDI+ applies it, a JPEG's rounding as its decoder rounds.
//! Other decoders differ by a level or two here and there, which FlexDMD's
//! filters (a threshold, dots summed) can turn into whole dots of difference.
//! GDI+ is part of Windows, so nothing is added to install; where it can't
//! read a picture, the `image` crate does.

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::OnceLock;

#[repr(C)]
struct StartupInput {
    version: u32,
    debug_event_callback: *mut c_void,
    suppress_background_thread: i32,
    suppress_external_codecs: i32,
}

#[repr(C)]
struct BitmapData {
    width: u32,
    height: u32,
    stride: i32,
    pixel_format: i32,
    scan0: *mut u8,
    reserved: usize,
}

#[repr(C)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[link(name = "gdiplus")]
unsafe extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const StartupInput, output: *mut c_void) -> i32;
    fn GdipCreateBitmapFromStream(stream: *mut c_void, bitmap: *mut *mut c_void) -> i32;
    fn GdipGetImageWidth(image: *mut c_void, width: *mut u32) -> i32;
    fn GdipGetImageHeight(image: *mut c_void, height: *mut u32) -> i32;
    fn GdipGetImagePixelFormat(image: *mut c_void, format: *mut i32) -> i32;
    fn GdipBitmapLockBits(bitmap: *mut c_void, rect: *const Rect, flags: u32, format: i32, data: *mut BitmapData) -> i32;
    fn GdipBitmapUnlockBits(bitmap: *mut c_void, data: *mut BitmapData) -> i32;
    fn GdipDisposeImage(image: *mut c_void) -> i32;
}

#[link(name = "shlwapi")]
unsafe extern "system" {
    fn SHCreateMemStream(data: *const u8, len: u32) -> *mut c_void;
}

/// GDI+'s `ImageLockModeRead` and `PixelFormat32bppARGB`.
const LOCK_READ: u32 = 1;
const FORMAT_32BPP_ARGB: i32 = 0x0026_200A;

/// Starts GDI+ once for the process; false when it can't start.
fn started() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let input = StartupInput { version: 1, debug_event_callback: null_mut(), suppress_background_thread: 0, suppress_external_codecs: 0 };
        let mut token = 0usize;
        // SAFETY: GDI+'s documented start; it stays up for the process
        unsafe { GdiplusStartup(&mut token, &input, null_mut()) == 0 }
    })
}

/// Releases a COM object (its `IUnknown::Release`).
///
/// # Safety
/// `object` must be a live COM object the caller holds a reference to.
unsafe fn release(object: *mut c_void) {
    unsafe {
        let vtable = *(object as *const *const unsafe extern "system" fn(*mut c_void) -> u32);
        (*vtable.add(2))(object);
    }
}

/// A picture's pixels as GDI+ decodes it: width, height, its rows, top
/// first, as GDI+'s 32-bit ARGB holds them in memory (blue, green, red,
/// alpha), and the pixel format GDI+ decoded it to. None when GDI+ can't
/// read it.
pub fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<[u8; 4]>, i32)> {
    if !started() {
        return None;
    }
    // SAFETY: the stream and the bitmap are GDI+'s and Windows', released below; the locked
    // rows are read within their width and height before unlocking
    unsafe {
        let stream = SHCreateMemStream(bytes.as_ptr(), u32::try_from(bytes.len()).ok()?);
        if stream.is_null() {
            return None;
        }
        let mut bitmap = null_mut();
        let out = if GdipCreateBitmapFromStream(stream, &mut bitmap) == 0 && !bitmap.is_null() {
            let (mut w, mut h, mut format) = (0u32, 0u32, 0i32);
            GdipGetImageWidth(bitmap, &mut w);
            GdipGetImageHeight(bitmap, &mut h);
            GdipGetImagePixelFormat(bitmap, &mut format);
            let rect = Rect { x: 0, y: 0, width: w as i32, height: h as i32 };
            let mut data = BitmapData { width: 0, height: 0, stride: 0, pixel_format: 0, scan0: null_mut(), reserved: 0 };
            let read = if w > 0 && h > 0 && GdipBitmapLockBits(bitmap, &rect, LOCK_READ, FORMAT_32BPP_ARGB, &mut data) == 0 {
                let mut px = Vec::with_capacity((w * h) as usize);
                for y in 0..h as isize {
                    let row = data.scan0.offset(y * data.stride as isize);
                    for x in 0..w as usize {
                        let p = row.add(x * 4);
                        px.push([*p, *p.add(1), *p.add(2), *p.add(3)]);
                    }
                }
                GdipBitmapUnlockBits(bitmap, &mut data);
                Some((w, h, px, format))
            } else {
                None
            };
            GdipDisposeImage(bitmap);
            read
        } else {
            None
        };
        release(stream);
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_jpeg_decodes_as_gdi_plus_holds_it() {
        // an 8x8 grey JPEG, made by the `image` crate
        let mut bytes = Vec::new();
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb([200, 120, 40]));
        image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Jpeg).expect("a JPEG");
        let (w, h, px, _) = super::decode(&bytes).expect("GDI+ reads it");
        assert_eq!((w, h, px.len()), (8, 8, 64));
        // blue, green, red, alpha in memory; a flat colour comes back within a level or two
        let p = px[27];
        assert!(p[3] == 255 && p[2].abs_diff(200) <= 3 && p[1].abs_diff(120) <= 3 && p[0].abs_diff(40) <= 3, "{p:?}");
        assert!(super::decode(b"not a picture").is_none());
    }
}
