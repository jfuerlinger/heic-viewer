use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use heic_decoder::{DecodeError, Limits};
use windows::Win32::Foundation::{
    ERROR_ALREADY_INITIALIZED, ERROR_FILE_TOO_LARGE, ERROR_TIMEOUT, E_FAIL, E_OUTOFMEMORY, E_POINTER, E_UNEXPECTED,
    S_FALSE, S_OK,
};
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{IStream, STATFLAG_NONAME, STATSTG};
use windows::Win32::System::Diagnostics::Debug::OutputDebugStringW;
use windows::Win32::UI::Shell::PropertiesSystem::{IInitializeWithStream, IInitializeWithStream_Impl};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTSAT_ARGB, WTSAT_RGB, WTSAT_UNKNOWN, WTS_ALPHATYPE,
};
use windows_core::{implement, Error, Ref, Result, HRESULT, HSTRING};

use crate::exports::ServerRef;
use crate::{loader, pixels};

/// Upper bound for a single thumbnail edge; Explorer asks for at most 2560 today.
const MAX_THUMBNAIL_EDGE: u32 = 4096;
const DEFAULT_THUMBNAIL_EDGE: u32 = 256;

fn limits() -> Limits {
    Limits::default()
}

#[implement(IThumbnailProvider, IInitializeWithStream)]
pub struct ThumbnailProvider {
    data: Mutex<Option<Arc<[u8]>>>,
    _server_ref: ServerRef,
}

impl ThumbnailProvider {
    pub fn new() -> Self {
        ThumbnailProvider { data: Mutex::new(None), _server_ref: ServerRef::new() }
    }
}

/// Runs `f`, converting panics into E_UNEXPECTED so nothing unwinds into Explorer.
fn guard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        debug_log("panic caught");
        Err(E_UNEXPECTED.into())
    })
}

fn debug_log(msg: &str) {
    // Visible with Sysinternals DebugView; costs nothing when no debugger listens.
    let text = HSTRING::from(format!("[HEIC Explorer Preview] {msg}\n"));
    unsafe { OutputDebugStringW(&text) };
}

fn win32(code: u32) -> Error {
    HRESULT::from_win32(code).into()
}

fn read_stream(stream: &IStream, max_bytes: usize) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut stat = STATSTG::default();
    if unsafe { stream.Stat(&mut stat, STATFLAG_NONAME) }.is_ok() {
        if stat.cbSize > max_bytes as u64 {
            return Err(win32(ERROR_FILE_TOO_LARGE.0));
        }
        data.reserve_exact(stat.cbSize as usize);
    }
    let mut chunk = vec![0u8; 1 << 20];
    loop {
        let mut read = 0u32;
        let hr = unsafe { stream.Read(chunk.as_mut_ptr() as *mut c_void, chunk.len() as u32, Some(&mut read)) };
        if hr != S_OK && hr != S_FALSE {
            return Err(hr.into());
        }
        if read == 0 {
            break;
        }
        if data.len() + read as usize > max_bytes {
            return Err(win32(ERROR_FILE_TOO_LARGE.0));
        }
        data.extend_from_slice(&chunk[..read as usize]);
    }
    Ok(data)
}

fn to_hresult(err: &DecodeError) -> Error {
    match err {
        DecodeError::Timeout => win32(ERROR_TIMEOUT.0),
        DecodeError::InputSize(_) => win32(ERROR_FILE_TOO_LARGE.0),
        DecodeError::ImageTooLarge { .. } => E_OUTOFMEMORY.into(),
        DecodeError::Internal(_) => E_UNEXPECTED.into(),
        DecodeError::Heif { .. } | DecodeError::UnexpectedLayout(_) => E_FAIL.into(),
    }
}

fn create_bitmap(image: &heic_decoder::RgbaImage) -> Result<(HBITMAP, WTS_ALPHATYPE)> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: image.width as i32,
            biHeight: -(image.height as i32), // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
    if bits.is_null() {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
        }
        return Err(E_OUTOFMEMORY.into());
    }
    // 32 bpp rows are always DWORD-aligned, so the DIB stride is width * 4.
    let dst = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, image.pixels.len()) };
    let transparent = pixels::rgba_to_bgra(&image.pixels, dst);
    Ok((bitmap, if transparent { WTSAT_ARGB } else { WTSAT_RGB }))
}

impl IInitializeWithStream_Impl for ThumbnailProvider_Impl {
    fn Initialize(&self, pstream: Ref<IStream>, _grfmode: u32) -> Result<()> {
        guard(|| {
            let stream = pstream.ok()?;
            let mut slot = self.data.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
            if slot.is_some() {
                return Err(win32(ERROR_ALREADY_INITIALIZED.0));
            }
            *slot = Some(read_stream(stream, limits().max_input_bytes)?.into());
            Ok(())
        })
    }
}

impl IThumbnailProvider_Impl for ThumbnailProvider_Impl {
    fn GetThumbnail(&self, cx: u32, phbmp: *mut HBITMAP, pdwalpha: *mut WTS_ALPHATYPE) -> Result<()> {
        guard(|| {
            if phbmp.is_null() || pdwalpha.is_null() {
                return Err(E_POINTER.into());
            }
            unsafe {
                *phbmp = HBITMAP::default();
                *pdwalpha = WTSAT_UNKNOWN;
            }
            let data = self
                .data
                .lock()
                .map_err(|_| Error::from(E_UNEXPECTED))?
                .clone()
                .ok_or_else(|| Error::from(E_UNEXPECTED))?;

            loader::ensure_libheif().inspect_err(|e| debug_log(&format!("cannot load {}: {e}", loader::HEIF_DLL)))?;

            let edge = if cx == 0 { DEFAULT_THUMBNAIL_EDGE } else { cx.min(MAX_THUMBNAIL_EDGE) };
            let decoded = heic_decoder::decode_thumbnail_with_timeout(data, edge, limits()).map_err(|e| {
                debug_log(&format!("decode failed: {e}"));
                to_hresult(&e)
            })?;
            let (bitmap, alpha) = create_bitmap(&decoded.image)?;
            unsafe {
                *phbmp = bitmap;
                *pdwalpha = alpha;
            }
            Ok(())
        })
    }
}
