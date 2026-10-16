//! Platform-independent HEIC/HEIF decoder core: bytes in, 8-bit sRGB RGBA out.
//!
//! Used by the Windows thumbnail provider; designed so a future preview handler can reuse it
//! (see [`decode_image`]). Only the primary image is decoded (no bursts, depth maps, gain maps
//! or other auxiliary images). Rotation/mirroring (irot/imir) is applied by libheif; the EXIF
//! orientation tag is intentionally not applied on top, as HEIF transformative properties
//! take precedence and applying both would rotate twice.

mod color;
mod error;
mod ffi;
mod heif;
mod resize;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub use error::DecodeError;

/// 8-bit sRGB pixels, RGBA order, straight (non-premultiplied) alpha, rows top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSource {
    /// The thumbnail stored inside the HEIF file was used.
    EmbeddedThumbnail,
    /// The primary image was decoded (and scaled down).
    PrimaryImage,
}

#[derive(Debug, Clone)]
pub struct Decoded {
    pub image: RgbaImage,
    pub source: ImageSource,
    /// True if the image carries an alpha channel (pixels may still all be opaque).
    pub has_alpha: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_input_bytes: usize,
    /// Maximum width * height of any image that gets decoded.
    pub max_pixels: u64,
    /// Only honored by [`decode_thumbnail_with_timeout`].
    pub timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_input_bytes: 512 * 1024 * 1024, max_pixels: 100_000_000, timeout: Duration::from_secs(20) }
    }
}

/// Version string of the libheif that was loaded at runtime.
pub fn libheif_version() -> String {
    heif::version()
}

/// Decodes a thumbnail that fits into `max_edge` x `max_edge` (aspect ratio kept, never upscaled).
/// Uses the embedded thumbnail when it is at least `max_edge` large, otherwise the primary image.
pub fn decode_thumbnail(data: &[u8], max_edge: u32, limits: &Limits) -> Result<Decoded, DecodeError> {
    guarded(|| decode_impl(data, max_edge, limits, None))
}

/// Decodes the full-resolution primary image (for a future preview handler).
pub fn decode_image(data: &[u8], limits: &Limits) -> Result<Decoded, DecodeError> {
    guarded(|| decode_impl(data, 0, limits, None))
}

static ACTIVE_WORKERS: AtomicUsize = AtomicUsize::new(0);

/// Number of decode worker threads still running (also ones that already timed out).
/// A DLL host must not be unloaded while this is non-zero.
pub fn active_workers() -> usize {
    ACTIVE_WORKERS.load(Ordering::SeqCst)
}

/// Like [`decode_thumbnail`] but runs on a worker thread and gives up after `limits.timeout`.
/// On timeout, libheif is asked to cancel (checked between grid tiles); the worker then
/// finishes in the background.
pub fn decode_thumbnail_with_timeout(data: Arc<[u8]>, max_edge: u32, limits: Limits) -> Result<Decoded, DecodeError> {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let result = run_with_timeout(limits.timeout, cancel, move || {
        guarded(|| decode_impl(&data, max_edge, &limits, Some(&worker_cancel)))
    });
    result.and_then(|r| r)
}

fn run_with_timeout<T: Send + 'static>(
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, DecodeError> {
    struct WorkerGuard;
    impl Drop for WorkerGuard {
        fn drop(&mut self) {
            ACTIVE_WORKERS.fetch_sub(1, Ordering::SeqCst);
        }
    }

    let (tx, rx) = mpsc::channel();
    ACTIVE_WORKERS.fetch_add(1, Ordering::SeqCst);
    let spawned = std::thread::Builder::new().name("heic-decode".into()).spawn(move || {
        let _guard = WorkerGuard;
        let _ = tx.send(catch_unwind(AssertUnwindSafe(work)));
    });
    if let Err(e) = spawned {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::SeqCst);
        return Err(DecodeError::Internal(format!("cannot spawn decode thread: {e}")));
    }
    match rx.recv_timeout(timeout) {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(panic)) => Err(DecodeError::Internal(panic_message(&panic))),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancel.store(true, Ordering::Relaxed);
            Err(DecodeError::Timeout)
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(DecodeError::Internal("decode thread vanished".into())),
    }
}

fn guarded<T>(f: impl FnOnce() -> Result<T, DecodeError>) -> Result<T, DecodeError> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|p| Err(DecodeError::Internal(panic_message(&p))))
}

fn panic_message(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".into())
}

fn check_size(width: u32, height: u32, limits: &Limits) -> Result<(), DecodeError> {
    let pixels = width as u64 * height as u64;
    if pixels == 0 || pixels > limits.max_pixels {
        return Err(DecodeError::ImageTooLarge { width, height });
    }
    Ok(())
}

fn same_aspect(a: (u32, u32), b: (u32, u32)) -> bool {
    let ra = a.0 as f64 / a.1.max(1) as f64;
    let rb = b.0 as f64 / b.1.max(1) as f64;
    (ra / rb - 1.0).abs() < 0.05
}

fn decode_impl(
    data: &[u8],
    max_edge: u32,
    limits: &Limits,
    cancel: Option<&AtomicBool>,
) -> Result<Decoded, DecodeError> {
    if data.is_empty() || data.len() > limits.max_input_bytes {
        return Err(DecodeError::InputSize(data.len()));
    }
    heif::init();
    let libheif_edge = (limits.max_pixels as f64).sqrt().ceil() as u32;
    let ctx = heif::Context::read(data, libheif_edge)?;
    let primary = ctx.primary_image()?;
    let primary_size = primary.size();
    check_size(primary_size.0, primary_size.1, limits)?;

    if max_edge > 0 {
        // Smallest embedded thumbnail that is large enough and has the primary's orientation
        // (guards against thumbnails that lack the rotation of the main image).
        let candidate = primary
            .thumbnails()
            .into_iter()
            .filter(|t| {
                let (w, h) = t.size();
                w.max(h) >= max_edge && same_aspect((w, h), primary_size) && check_size(w, h, limits).is_ok()
            })
            .min_by_key(|t| {
                let (w, h) = t.size();
                w as u64 * h as u64
            });
        if let Some(thumb) = candidate {
            // Thumbnails rarely carry their own colr box; fall back to the primary's profile.
            let profile = match thumb.color_profile() {
                color::SourceProfile::Unknown => primary.color_profile(),
                p => p,
            };
            if let Ok(image) = render(&thumb, &profile, max_edge, cancel) {
                return Ok(Decoded { image, source: ImageSource::EmbeddedThumbnail, has_alpha: thumb.has_alpha() });
            }
        }
    }

    let image = render(&primary, &primary.color_profile(), max_edge, cancel)?;
    Ok(Decoded { image, source: ImageSource::PrimaryImage, has_alpha: primary.has_alpha() })
}

fn render(
    handle: &heif::Handle<'_>,
    profile: &color::SourceProfile,
    max_edge: u32,
    cancel: Option<&AtomicBool>,
) -> Result<RgbaImage, DecodeError> {
    let has_alpha = handle.has_alpha();
    let high_bit_depth = handle.luma_bits() > 8;
    let decoded = handle.decode(has_alpha, high_bit_depth, cancel)?;
    let plane = decoded.plane()?;

    let (dw, dh) = resize::fit_within(plane.width as u32, plane.height as u32, max_edge);
    let (dw, dh) = (dw as usize, dh as usize);
    let converter = color::Converter::new(profile);
    let mut pixels = vec![0u8; dw * dh * 4];
    let mut scratch = Vec::new();

    resize::downscale(&plane, dw, dh, handle.is_premultiplied(), |y, row| {
        converter.convert_row(row, &mut scratch);
        let out = &mut pixels[y * dw * 4..(y + 1) * dw * 4];
        for (o, v) in out.iter_mut().zip(row.iter()) {
            *o = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    });

    Ok(RgbaImage { width: dw as u32, height: dh as u32, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_fires_and_sets_cancel_flag() {
        let cancel = Arc::new(AtomicBool::new(false));
        let observed = cancel.clone();
        let r = run_with_timeout(Duration::from_millis(20), cancel, move || {
            while !observed.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        assert_eq!(r, Err(DecodeError::Timeout));
    }

    #[test]
    fn worker_panic_becomes_error() {
        let r: Result<(), _> =
            run_with_timeout(Duration::from_secs(5), Arc::new(AtomicBool::new(false)), || panic!("boom"));
        assert_eq!(r, Err(DecodeError::Internal("boom".into())));
    }

    #[test]
    fn cancel_flag_aborts_grid_decode() {
        // with_thumbnail.heic is a tiled (grid) image; libheif polls the cancel callback per tile.
        // This also verifies our mirror of the heif_decoding_options layout.
        let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/with_thumbnail.heic")).unwrap();
        let cancel = AtomicBool::new(true);
        let err = decode_impl(&data, 0, &Limits::default(), Some(&cancel)).unwrap_err();
        assert!(matches!(err, DecodeError::Heif { .. }), "{err:?}");
    }

    #[test]
    fn empty_input_is_rejected_without_touching_libheif() {
        assert_eq!(decode_impl(&[], 256, &Limits::default(), None).unwrap_err(), DecodeError::InputSize(0));
    }
}
