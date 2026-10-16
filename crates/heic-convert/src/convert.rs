//! Single file conversion: HEIC -> JPEG next to the source, never overwriting anything.

use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use heic_decoder::{Limits, RgbaImage};
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

use crate::exif;

pub const JPEG_QUALITY: u8 = 92;

/// Extensions handled by the converter (compared case-insensitively).
pub const HEIC_EXTENSIONS: [&str; 3] = ["heic", "heif", "hif"];

pub fn is_heic(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| HEIC_EXTENSIONS.iter().any(|h| e.eq_ignore_ascii_case(h)))
}

#[derive(Debug)]
pub enum ConvertError {
    Io(io::Error),
    Decode(heic_decoder::DecodeError),
    Encode(String),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Io(e) => write!(f, "{e}"),
            ConvertError::Decode(e) => write!(f, "{e}"),
            ConvertError::Encode(e) => write!(f, "JPEG encoding failed: {e}"),
        }
    }
}

impl From<io::Error> for ConvertError {
    fn from(e: io::Error) -> Self {
        ConvertError::Io(e)
    }
}

/// Converts `src` and returns the path of the written JPEG.
pub fn convert_file(src: &Path) -> Result<PathBuf, ConvertError> {
    let limits = Limits::default();
    let data = fs::read(src)?;
    let decoded = heic_decoder::decode_image(&data, &limits).map_err(ConvertError::Decode)?;
    let exif = heic_decoder::read_exif(&data, &limits).ok().flatten();
    drop(data);
    let jpeg = encode_jpeg(&decoded.image, exif.as_deref())?;
    drop(decoded);
    write_next_to(src, &jpeg)
}

pub fn encode_jpeg(image: &RgbaImage, exif_tiff: Option<&[u8]>) -> Result<Vec<u8>, ConvertError> {
    let (w, h) = (image.width, image.height);
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(ConvertError::Encode(format!("{w}x{h} exceeds the JPEG size limit of 65535 pixels")));
    }
    let rgb = flatten_on_white(image);
    let mut out = Vec::with_capacity(rgb.len() / 6);
    let mut encoder = Encoder::new(&mut out, JPEG_QUALITY);
    encoder.set_sampling_factor(SamplingFactor::F_2_2);
    if let Some(payload) = exif_tiff.and_then(exif::app1_payload) {
        encoder.add_app_segment(1, payload).map_err(|e| ConvertError::Encode(e.to_string()))?;
    }
    encoder.encode(&rgb, w as u16, h as u16, ColorType::Rgb).map_err(|e| ConvertError::Encode(e.to_string()))?;
    Ok(out)
}

/// JPEG has no alpha: composite straight-alpha RGBA onto white.
fn flatten_on_white(image: &RgbaImage) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(image.pixels.len() / 4 * 3);
    for p in image.pixels.as_chunks::<4>().0 {
        let a = p[3] as u32;
        for &c in &p[..3] {
            rgb.push(((c as u32 * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    rgb
}

/// `IMG_0001.jpg`, then `IMG_0001 (1).jpg`, `IMG_0001 (2).jpg`, ...
pub fn candidate_name(stem: &str, index: u32) -> String {
    if index == 0 {
        format!("{stem}.jpg")
    } else {
        format!("{stem} ({index}).jpg")
    }
}

/// Writes to a temporary file first and only then publishes it under a free name, so neither a
/// crash nor a cancellation leaves a half-written JPEG, and no existing file is ever replaced.
pub fn write_next_to(src: &Path, jpeg: &[u8]) -> Result<PathBuf, ConvertError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = src.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("image").to_string();
    let temp = dir.join(format!(
        ".{stem}.{}-{}.heic-convert.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| {
        fs::write(&temp, jpeg)?;
        copy_file_times(src, &temp)?;
        publish(&temp, dir, &stem, jpeg)
    })();
    let _ = fs::remove_file(&temp);
    result
}

fn publish(temp: &Path, dir: &Path, stem: &str, jpeg: &[u8]) -> Result<PathBuf, ConvertError> {
    for index in 0..10_000 {
        let target = dir.join(candidate_name(stem, index));
        // A hard link is created atomically and fails if the target exists (no overwrite races
        // between parallel workers, e.g. IMG_1.heic and IMG_1.heif).
        match fs::hard_link(temp, &target) {
            Ok(()) => return Ok(target),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(_) => {}
        }
        // File systems without hard links (FAT32/exFAT, some network shares): reserve the name
        // atomically and write directly.
        match File::options().write(true).create_new(true).open(&target) {
            Ok(mut f) => {
                let written = io::Write::write_all(&mut f, jpeg).and_then(|_| f.sync_all());
                drop(f);
                if let Err(e) = written.and_then(|_| copy_file_times(temp, &target)) {
                    let _ = fs::remove_file(&target);
                    return Err(e.into());
                }
                return Ok(target);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free output file name").into())
}

/// Keeps the original's modification (and on Windows creation) time so date sorting still works.
fn copy_file_times(from: &Path, to: &Path) -> io::Result<()> {
    let meta = fs::metadata(from)?;
    let times = FileTimes::new().set_modified(meta.modified()?);
    #[cfg(windows)]
    let times = {
        use std::os::windows::fs::FileTimesExt;
        match meta.created() {
            Ok(created) => times.set_created(created),
            Err(_) => times,
        }
    };
    File::options().write(true).open(to)?.set_times(times)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heic_extensions_are_case_insensitive() {
        for name in ["a.heic", "B.HEIC", "c.Heif", "d.hif"] {
            assert!(is_heic(Path::new(name)), "{name}");
        }
        for name in ["a.jpg", "heic", "a.heic.txt", "a"] {
            assert!(!is_heic(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn alpha_is_flattened_on_white() {
        let image = RgbaImage { width: 3, height: 1, pixels: vec![255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 128] };
        assert_eq!(flatten_on_white(&image), vec![255, 0, 0, 255, 255, 255, 127, 127, 255]);
    }

    #[test]
    fn candidate_names() {
        assert_eq!(candidate_name("IMG_0001", 0), "IMG_0001.jpg");
        assert_eq!(candidate_name("IMG_0001", 2), "IMG_0001 (2).jpg");
    }
}
