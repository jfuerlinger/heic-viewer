//! "HEIC Explorer Preview": Windows Explorer thumbnail provider for .heic/.heif/.hif.
//!
//! COM in-process server exposing one class (IThumbnailProvider + IInitializeWithStream).
//! Registration is done by the installer, not by DllRegisterServer.
//! Every entry point catches panics; nothing unwinds across the FFI boundary.

#[cfg_attr(not(windows), allow(dead_code))]
mod pixels;

#[cfg(windows)]
mod factory;
#[cfg(windows)]
mod loader;
#[cfg(windows)]
mod provider;

#[cfg(windows)]
pub use exports::*;

#[cfg(windows)]
mod exports {
    use std::ffi::c_void;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::ptr;
    use std::sync::atomic::{AtomicIsize, Ordering};

    use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_POINTER, E_UNEXPECTED, S_FALSE, S_OK};
    use windows::Win32::System::Com::IClassFactory;
    use windows_core::{Interface, GUID, HRESULT};

    use crate::factory::ClassFactory;

    /// {218C7AE3-FDC4-4381-ABCD-D7385AFB7A4A} – must match installer/heic-explorer-preview.iss.
    pub const CLSID_HEIC_THUMBNAIL_PROVIDER: GUID = GUID::from_u128(0x218c7ae3_fdc4_4381_abcd_d7385afb7a4a);

    /// Live COM objects plus IClassFactory::LockServer locks.
    static SERVER_REFS: AtomicIsize = AtomicIsize::new(0);

    pub(crate) fn add_server_ref() {
        SERVER_REFS.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn release_server_ref() {
        SERVER_REFS.fetch_sub(1, Ordering::SeqCst);
    }

    /// Increments the server reference count for the lifetime of a COM object.
    pub(crate) struct ServerRef;
    impl ServerRef {
        pub(crate) fn new() -> Self {
            add_server_ref();
            ServerRef
        }
    }
    impl Drop for ServerRef {
        fn drop(&mut self) {
            release_server_ref();
        }
    }

    /// # Safety
    /// Standard COM export; pointers come from the COM runtime.
    #[no_mangle]
    pub unsafe extern "system" fn DllGetClassObject(
        rclsid: *const GUID,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> HRESULT {
        if ppv.is_null() {
            return E_POINTER;
        }
        unsafe { *ppv = ptr::null_mut() };
        if rclsid.is_null() || riid.is_null() {
            return E_POINTER;
        }
        if unsafe { *rclsid } != CLSID_HEIC_THUMBNAIL_PROVIDER {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        catch_unwind(AssertUnwindSafe(|| {
            let factory: IClassFactory = ClassFactory::new().into();
            unsafe { factory.query(riid, ppv) }
        }))
        .unwrap_or(E_UNEXPECTED)
    }

    #[no_mangle]
    pub extern "system" fn DllCanUnloadNow() -> HRESULT {
        // Timed-out decode threads may still run code from this DLL.
        if SERVER_REFS.load(Ordering::SeqCst) <= 0 && heic_decoder::active_workers() == 0 {
            S_OK
        } else {
            S_FALSE
        }
    }
}
