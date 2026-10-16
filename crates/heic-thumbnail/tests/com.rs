//! Drives the COM entry points in-process exactly like the Explorer thumbnail host does:
//! DllGetClassObject -> IClassFactory -> IInitializeWithStream::Initialize -> GetThumbnail.
//! Requires heif.dll on PATH (CI adds the vcpkg bin directory).
#![cfg(windows)]

use std::ffi::c_void;
use std::path::PathBuf;

use heic_thumbnail::{DllGetClassObject, CLSID_HEIC_THUMBNAIL_PROVIDER};
use windows::Win32::Foundation::CLASS_E_CLASSNOTAVAILABLE;
use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HBITMAP, HGDIOBJ};
use windows::Win32::System::Com::{CoInitializeEx, IClassFactory, COINIT_APARTMENTTHREADED, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithStream;
use windows::Win32::UI::Shell::{
    IThumbnailProvider, SHCreateMemStream, WTSAT_ARGB, WTSAT_RGB, WTSAT_UNKNOWN, WTS_ALPHATYPE,
};
use windows_core::{IUnknown, Interface, GUID};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(name)).unwrap()
}

fn factory() -> IClassFactory {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut ptr: *mut c_void = std::ptr::null_mut();
        DllGetClassObject(&CLSID_HEIC_THUMBNAIL_PROVIDER, &IClassFactory::IID, &mut ptr).ok().unwrap();
        IClassFactory::from_raw(ptr)
    }
}

fn initialized_provider(data: &[u8]) -> windows_core::Result<IThumbnailProvider> {
    unsafe {
        let provider: IThumbnailProvider = factory().CreateInstance(None::<&IUnknown>)?;
        let stream = SHCreateMemStream(Some(data)).expect("SHCreateMemStream");
        provider.cast::<IInitializeWithStream>()?.Initialize(&stream, STGM_READ.0)?;
        Ok(provider)
    }
}

struct Thumb {
    width: i32,
    height: i32,
    alpha: WTS_ALPHATYPE,
    bgra: Vec<u8>,
}

impl Thumb {
    fn px(&self, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.bgra[i], self.bgra[i + 1], self.bgra[i + 2], self.bgra[i + 3]]
    }
}

fn thumbnail(data: &[u8], cx: u32) -> windows_core::Result<Thumb> {
    let provider = initialized_provider(data)?;
    unsafe {
        let mut bitmap = HBITMAP::default();
        let mut alpha = WTSAT_UNKNOWN;
        provider.GetThumbnail(cx, &mut bitmap, &mut alpha)?;
        let mut bm = BITMAP::default();
        let n =
            GetObjectW(HGDIOBJ(bitmap.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut c_void));
        assert!(n > 0 && !bm.bmBits.is_null());
        assert_eq!(bm.bmBitsPixel, 32);
        let len = (bm.bmWidthBytes * bm.bmHeight) as usize;
        let bgra = std::slice::from_raw_parts(bm.bmBits as *const u8, len).to_vec();
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        Ok(Thumb { width: bm.bmWidth, height: bm.bmHeight, alpha, bgra })
    }
}

fn assert_bgr(t: &Thumb, x: i32, y: i32, bgr: [u8; 3]) {
    let p = t.px(x, y);
    for c in 0..3 {
        assert!(p[c].abs_diff(bgr[c]) <= 16, "pixel ({x},{y}) = {p:?}, expected ~{bgr:?}");
    }
}

#[test]
fn rotated_photo_produces_portrait_bitmap() {
    let t = thumbnail(&fixture("quadrants_orientation6.heic"), 256).unwrap();
    assert_eq!((t.width, t.height), (64, 96));
    assert_eq!(t.alpha, WTSAT_RGB);
    assert_bgr(&t, 16, 24, [255, 0, 0]); // blue (BGR order)
    assert_bgr(&t, 48, 24, [0, 0, 255]); // red
}

#[test]
fn embedded_thumbnail_is_scaled_to_cx() {
    let t = thumbnail(&fixture("with_thumbnail.heic"), 256).unwrap();
    assert_eq!((t.width, t.height), (256, 192));
}

#[test]
fn alpha_is_premultiplied_argb() {
    let t = thumbnail(&fixture("alpha.heic"), 256).unwrap();
    assert_eq!(t.alpha, WTSAT_ARGB);
    let p = t.px(56, 32);
    assert!(p[3].abs_diff(128) <= 6 && p[0].abs_diff(p[3]) <= 6 && p[2] <= 10, "{p:?}");
}

#[test]
fn corrupt_file_returns_error_hresult() {
    let err = thumbnail(&fixture("corrupt_truncated.heic"), 256).err().expect("must fail");
    assert!(err.code().is_err());
    assert!(thumbnail(b"not an image", 256).is_err());
}

#[test]
fn double_initialize_is_rejected() {
    let data = fixture("quadrants_srgb.heic");
    let provider = initialized_provider(&data).unwrap();
    unsafe {
        let stream = SHCreateMemStream(Some(&data)).unwrap();
        assert!(provider.cast::<IInitializeWithStream>().unwrap().Initialize(&stream, STGM_READ.0).is_err());
    }
}

#[test]
fn unknown_clsid_is_refused() {
    let mut ptr: *mut c_void = std::ptr::null_mut();
    let other = GUID::from_u128(0x12345678_1234_1234_1234_123456789abc);
    let hr = unsafe { DllGetClassObject(&other, &IClassFactory::IID, &mut ptr) };
    assert_eq!(hr, CLASS_E_CLASSNOTAVAILABLE);
    assert!(ptr.is_null());
}
