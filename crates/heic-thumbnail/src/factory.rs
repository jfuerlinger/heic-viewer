use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_POINTER, E_UNEXPECTED};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows_core::{implement, IUnknown, Interface, Ref, Result, BOOL, GUID};

use crate::exports::{add_server_ref, release_server_ref, ServerRef};
use crate::provider::ThumbnailProvider;

#[implement(IClassFactory)]
pub struct ClassFactory {
    _server_ref: ServerRef,
}

impl ClassFactory {
    pub fn new() -> Self {
        ClassFactory { _server_ref: ServerRef::new() }
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(&self, punkouter: Ref<IUnknown>, riid: *const GUID, ppvobject: *mut *mut c_void) -> Result<()> {
        if ppvobject.is_null() || riid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *ppvobject = std::ptr::null_mut() };
        if punkouter.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        catch_unwind(AssertUnwindSafe(|| {
            let unknown: IUnknown = ThumbnailProvider::new().into();
            unsafe { unknown.query(riid, ppvobject) }.ok()
        }))
        .unwrap_or_else(|_| Err(E_UNEXPECTED.into()))
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            add_server_ref();
        } else {
            release_server_ref();
        }
        Ok(())
    }
}
