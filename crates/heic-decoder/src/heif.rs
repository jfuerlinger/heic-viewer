//! Safe RAII wrappers around the libheif handles used by the decoder.

use std::ffi::CStr;
use std::marker::PhantomData;
use std::os::raw::{c_int, c_void};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use crate::color::{Nclx, SourceProfile};
use crate::error::DecodeError;
use crate::ffi;

fn check(err: ffi::heif_error) -> Result<(), DecodeError> {
    if err.code == 0 {
        return Ok(());
    }
    let message = if err.message.is_null() {
        String::new()
    } else {
        // SAFETY: libheif guarantees a valid NUL-terminated string for the error message.
        unsafe { CStr::from_ptr(err.message) }.to_string_lossy().into_owned()
    };
    Err(DecodeError::Heif { code: err.code, subcode: err.subcode, message })
}

pub fn init() {
    static INIT: Once = Once::new();
    // Reference-counted in libheif; we intentionally never call heif_deinit (unsafe at process exit).
    INIT.call_once(|| unsafe {
        let _ = ffi::heif_init(ptr::null_mut());
    });
}

pub fn version() -> String {
    // SAFETY: returns a static string.
    unsafe { CStr::from_ptr(ffi::heif_get_version()) }.to_string_lossy().into_owned()
}

/// A libheif context reading directly from `data` (no copy), hence the lifetime.
pub struct Context<'a> {
    ptr: NonNull<ffi::heif_context>,
    _data: PhantomData<&'a [u8]>,
}

impl<'a> Context<'a> {
    pub fn read(data: &'a [u8], max_edge: u32) -> Result<Self, DecodeError> {
        let raw = unsafe { ffi::heif_context_alloc() };
        let ptr = NonNull::new(raw).ok_or(DecodeError::Internal("heif_context_alloc failed".into()))?;
        let ctx = Context { ptr, _data: PhantomData };
        unsafe {
            // Caps width*height inside libheif (max_edge^2 pixels) before any allocation happens.
            ffi::heif_context_set_maximum_image_size_limit(ptr.as_ptr(), max_edge.min(i32::MAX as u32) as c_int);
            check(ffi::heif_context_read_from_memory_without_copy(
                ptr.as_ptr(),
                data.as_ptr() as *const c_void,
                data.len(),
                ptr::null(),
            ))?;
        }
        Ok(ctx)
    }

    pub fn primary_image(&self) -> Result<Handle<'_>, DecodeError> {
        let mut out = ptr::null_mut();
        check(unsafe { ffi::heif_context_get_primary_image_handle(self.ptr.as_ptr(), &mut out) })?;
        Handle::from_raw(out)
    }
}

impl Drop for Context<'_> {
    fn drop(&mut self) {
        unsafe { ffi::heif_context_free(self.ptr.as_ptr()) }
    }
}

pub struct Handle<'a> {
    ptr: NonNull<ffi::heif_image_handle>,
    _ctx: PhantomData<&'a ()>,
}

impl<'a> Handle<'a> {
    fn from_raw(raw: *mut ffi::heif_image_handle) -> Result<Self, DecodeError> {
        let ptr = NonNull::new(raw).ok_or(DecodeError::Internal("null image handle".into()))?;
        Ok(Handle { ptr, _ctx: PhantomData })
    }

    fn raw(&self) -> *const ffi::heif_image_handle {
        self.ptr.as_ptr()
    }

    /// Size after applying the transformative properties (irot/imir/clap).
    pub fn size(&self) -> (u32, u32) {
        unsafe {
            (
                ffi::heif_image_handle_get_width(self.raw()).max(0) as u32,
                ffi::heif_image_handle_get_height(self.raw()).max(0) as u32,
            )
        }
    }

    pub fn has_alpha(&self) -> bool {
        unsafe { ffi::heif_image_handle_has_alpha_channel(self.raw()) != 0 }
    }

    pub fn is_premultiplied(&self) -> bool {
        unsafe { ffi::heif_image_handle_is_premultiplied_alpha(self.raw()) != 0 }
    }

    pub fn luma_bits(&self) -> i32 {
        unsafe { ffi::heif_image_handle_get_luma_bits_per_pixel(self.raw()) }
    }

    pub fn thumbnails(&self) -> Vec<Handle<'a>> {
        unsafe {
            let n = ffi::heif_image_handle_get_number_of_thumbnails(self.raw());
            if n <= 0 {
                return Vec::new();
            }
            let mut ids = vec![0u32; n as usize];
            let n = ffi::heif_image_handle_get_list_of_thumbnail_IDs(self.raw(), ids.as_mut_ptr(), n);
            ids.truncate(n.max(0) as usize);
            ids.into_iter()
                .filter_map(|id| {
                    let mut out = ptr::null_mut();
                    check(ffi::heif_image_handle_get_thumbnail(self.raw(), id, &mut out)).ok()?;
                    Handle::from_raw(out).ok()
                })
                .collect()
        }
    }

    /// Color profile as signalled in the container. ICC wins over nclx.
    pub fn color_profile(&self) -> SourceProfile {
        unsafe {
            let kind = ffi::heif_image_handle_get_color_profile_type(self.raw());
            if kind == ffi::HEIF_COLOR_PROFILE_TYPE_PROF || kind == ffi::HEIF_COLOR_PROFILE_TYPE_RICC {
                let size = ffi::heif_image_handle_get_raw_color_profile_size(self.raw());
                if size > 0 && size <= 4 * 1024 * 1024 {
                    let mut icc = vec![0u8; size];
                    if check(ffi::heif_image_handle_get_raw_color_profile(self.raw(), icc.as_mut_ptr() as *mut c_void))
                        .is_ok()
                    {
                        return SourceProfile::Icc(icc);
                    }
                }
            }
            let mut nclx = ptr::null_mut();
            if check(ffi::heif_image_handle_get_nclx_color_profile(self.raw(), &mut nclx)).is_ok() && !nclx.is_null() {
                let n = &*nclx;
                let profile = Nclx {
                    primaries: n.color_primaries as u16,
                    transfer: n.transfer_characteristics as u16,
                    chromaticities: [
                        n.color_primary_red_x,
                        n.color_primary_red_y,
                        n.color_primary_green_x,
                        n.color_primary_green_y,
                        n.color_primary_blue_x,
                        n.color_primary_blue_y,
                        n.color_primary_white_x,
                        n.color_primary_white_y,
                    ],
                };
                ffi::heif_nclx_color_profile_free(nclx);
                return SourceProfile::Nclx(profile);
            }
            SourceProfile::Unknown
        }
    }

    /// Decodes to interleaved RGB(A), 8 bit or (for >8 bit sources) 16 bit little endian.
    /// Rotation/mirroring/cropping are applied by libheif.
    pub fn decode(
        &self,
        want_alpha: bool,
        high_bit_depth: bool,
        cancel: Option<&AtomicBool>,
    ) -> Result<Image, DecodeError> {
        let chroma = match (high_bit_depth, want_alpha) {
            (false, false) => ffi::HEIF_CHROMA_INTERLEAVED_RGB,
            (false, true) => ffi::HEIF_CHROMA_INTERLEAVED_RGBA,
            (true, false) => ffi::HEIF_CHROMA_INTERLEAVED_RRGGBB_LE,
            (true, true) => ffi::HEIF_CHROMA_INTERLEAVED_RRGGBBAA_LE,
        };
        unsafe {
            let options = ffi::heif_decoding_options_alloc();
            if let (Some(opts), Some(flag)) = (options.as_mut(), cancel) {
                if opts.version >= 6 {
                    opts.cancel_decoding = Some(cancel_callback);
                    opts.progress_user_data = flag as *const AtomicBool as *mut c_void;
                }
            }
            let mut out = ptr::null_mut();
            let err = ffi::heif_decode_image(self.raw(), &mut out, ffi::HEIF_COLORSPACE_RGB, chroma, options);
            if !options.is_null() {
                ffi::heif_decoding_options_free(options);
            }
            check(err)?;
            let ptr = NonNull::new(out).ok_or(DecodeError::Internal("null decoded image".into()))?;
            Ok(Image { ptr, channels: if want_alpha { 4 } else { 3 }, high_bit_depth })
        }
    }
}

impl Drop for Handle<'_> {
    fn drop(&mut self) {
        unsafe { ffi::heif_image_handle_release(self.raw()) }
    }
}

unsafe extern "C" fn cancel_callback(user_data: *mut c_void) -> c_int {
    match (user_data as *const AtomicBool).as_ref() {
        Some(flag) => flag.load(Ordering::Relaxed) as c_int,
        None => 0,
    }
}

pub struct Image {
    ptr: NonNull<ffi::heif_image>,
    pub channels: usize,
    pub high_bit_depth: bool,
}

/// Borrowed view on an interleaved plane.
pub struct Plane<'a> {
    pub data: &'a [u8],
    pub stride: usize,
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    /// 0 for 8-bit samples, otherwise the bit depth of 16-bit LE samples (e.g. 10).
    pub bits: u32,
}

impl Image {
    pub fn plane(&self) -> Result<Plane<'_>, DecodeError> {
        unsafe {
            let img = self.ptr.as_ptr() as *const ffi::heif_image;
            let ch = ffi::HEIF_CHANNEL_INTERLEAVED;
            let width = ffi::heif_image_get_width(img, ch);
            let height = ffi::heif_image_get_height(img, ch);
            let mut stride: c_int = 0;
            let data = ffi::heif_image_get_plane_readonly(img, ch, &mut stride);
            if data.is_null() || width <= 0 || height <= 0 || stride <= 0 {
                return Err(DecodeError::UnexpectedLayout("missing interleaved plane"));
            }
            let (width, height, stride) = (width as usize, height as usize, stride as usize);
            let bytes_per_sample = if self.high_bit_depth { 2 } else { 1 };
            if stride < width * self.channels * bytes_per_sample {
                return Err(DecodeError::UnexpectedLayout("stride smaller than row"));
            }
            let bits = if self.high_bit_depth {
                let b = ffi::heif_image_get_bits_per_pixel_range(img, ch);
                if !(9..=16).contains(&b) {
                    return Err(DecodeError::UnexpectedLayout("unsupported bit depth"));
                }
                b as u32
            } else {
                0
            };
            // The last row may be shorter than the stride.
            let len = stride * (height - 1) + width * self.channels * bytes_per_sample;
            Ok(Plane {
                data: std::slice::from_raw_parts(data, len),
                stride,
                width,
                height,
                channels: self.channels,
                bits,
            })
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe { ffi::heif_image_release(self.ptr.as_ptr()) }
    }
}
