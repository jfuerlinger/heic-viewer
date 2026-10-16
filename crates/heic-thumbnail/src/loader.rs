//! Loads heif.dll (and, through LOAD_WITH_ALTERED_SEARCH_PATH, its dependency libde265.dll)
//! from the directory of this DLL before the first delay-loaded libheif call.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::OnceLock;

use windows::Win32::Foundation::{ERROR_MOD_NOT_FOUND, HMODULE};
use windows::Win32::System::LibraryLoader::{
    GetModuleFileNameW, GetModuleHandleExW, LoadLibraryExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, LOAD_LIBRARY_FLAGS, LOAD_WITH_ALTERED_SEARCH_PATH,
};
use windows_core::{HRESULT, HSTRING, PCWSTR};

pub const HEIF_DLL: &str = env!("HEIF_DLL_NAME");

static LOADED: OnceLock<Result<(), HRESULT>> = OnceLock::new();

pub fn ensure_libheif() -> windows_core::Result<()> {
    (*LOADED.get_or_init(load)).map_err(Into::into)
}

fn load() -> Result<(), HRESULT> {
    // The handle is intentionally leaked: libheif stays loaded as long as this DLL.
    if let Some(path) = own_directory().map(|d| d.join(HEIF_DLL)).filter(|p| p.is_file()) {
        let path = HSTRING::from(path.as_os_str());
        return unsafe { LoadLibraryExW(&path, None, LOAD_WITH_ALTERED_SEARCH_PATH) }.map(|_| ()).map_err(|e| e.code());
    }
    // Development/test fallback: regular search order (PATH).
    unsafe { LoadLibraryExW(&HSTRING::from(HEIF_DLL), None, LOAD_LIBRARY_FLAGS(0)) }
        .map(|_| ())
        .map_err(|_| HRESULT::from_win32(ERROR_MOD_NOT_FOUND.0))
}

fn own_directory() -> Option<PathBuf> {
    unsafe {
        let mut module = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(own_directory as *const () as *const u16),
            &mut module,
        )
        .ok()?;
        let mut buf = vec![0u16; 32768];
        let len = GetModuleFileNameW(Some(module), &mut buf) as usize;
        if len == 0 || len >= buf.len() {
            return None;
        }
        PathBuf::from(OsString::from_wide(&buf[..len])).parent().map(PathBuf::from)
    }
}
