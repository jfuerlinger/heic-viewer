use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use heic_convert::{convert_all, convert_file, Batch, Outcome};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(name)
}

/// Fresh, empty scratch directory per test.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("heic-convert-test-{}-{test}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy(fixture_name: &str, dir: &Path, as_name: &str) -> PathBuf {
    let dst = dir.join(as_name);
    fs::copy(fixture(fixture_name), &dst).unwrap();
    dst
}

fn decode_jpeg(path: &Path) -> (u16, u16, Vec<u8>, Option<Vec<u8>>) {
    let mut d = jpeg_decoder::Decoder::new(fs::File::open(path).unwrap());
    let pixels = d.decode().unwrap();
    let info = d.info().unwrap();
    assert_eq!(info.pixel_format, jpeg_decoder::PixelFormat::RGB24);
    (info.width, info.height, pixels, d.exif_data().map(|e| e.to_vec()))
}

fn rgb_at(pixels: &[u8], width: u16, x: u16, y: u16) -> [u8; 3] {
    let i = (y as usize * width as usize + x as usize) * 3;
    [pixels[i], pixels[i + 1], pixels[i + 2]]
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 20)
}

/// IFD0 Orientation value of an Exif blob ("Exif\0\0" + TIFF or bare TIFF).
fn orientation(exif: &[u8]) -> Option<u16> {
    let t = exif.strip_prefix(b"Exif\0\0").unwrap_or(exif);
    let be = t.starts_with(b"MM");
    let u16_at =
        |i: usize| if be { u16::from_be_bytes([t[i], t[i + 1]]) } else { u16::from_le_bytes([t[i], t[i + 1]]) };
    let ifd = if be {
        u32::from_be_bytes(t[4..8].try_into().unwrap())
    } else {
        u32::from_le_bytes(t[4..8].try_into().unwrap())
    } as usize;
    (0..u16_at(ifd) as usize).map(|i| ifd + 2 + i * 12).find(|&e| u16_at(e) == 0x0112).map(|e| u16_at(e + 8))
}

fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect()
}

#[test]
fn rotated_photo_becomes_upright_jpeg_with_exif_orientation_1() {
    let dir = scratch("rotated");
    let src = copy("quadrants_orientation6.heic", &dir, "IMG_0001.HEIC");
    let out = convert_file(&src).unwrap();
    assert_eq!(out, dir.join("IMG_0001.jpg"));
    let (w, h, px, exif) = decode_jpeg(&out);
    assert_eq!((w, h), (64, 96));
    assert!(close(rgb_at(&px, w, 16, 24), [0, 0, 255]), "top-left should be blue: {:?}", rgb_at(&px, w, 16, 24));
    assert!(close(rgb_at(&px, w, 48, 24), [255, 0, 0]), "top-right should be red");
    let exif = exif.expect("Exif must be copied");
    assert_eq!(orientation(&exif), Some(1));
    assert!(src.exists(), "original must be kept");
    assert!(leftovers(&dir).is_empty());
}

#[test]
fn existing_files_are_never_overwritten() {
    let dir = scratch("conflict");
    let src = copy("quadrants_srgb.heic", &dir, "IMG.heic");
    fs::write(dir.join("IMG.jpg"), b"precious").unwrap();
    assert_eq!(convert_file(&src).unwrap(), dir.join("IMG (1).jpg"));
    assert_eq!(convert_file(&src).unwrap(), dir.join("IMG (2).jpg"));
    assert_eq!(fs::read(dir.join("IMG.jpg")).unwrap(), b"precious");
}

#[test]
fn same_stem_different_extension_in_parallel() {
    let dir = scratch("samestem");
    copy("quadrants_srgb.heic", &dir, "a.heic");
    copy("quadrants_srgb.heic", &dir, "a.heif");
    copy("quadrants_srgb.heic", &dir, "a.hif");
    let batch = convert_all(&[&dir]);
    assert_eq!(batch.progress().converted, 3);
    for name in ["a.jpg", "a (1).jpg", "a (2).jpg"] {
        assert!(dir.join(name).is_file(), "{name}");
    }
}

#[test]
fn transparency_becomes_white() {
    let dir = scratch("alpha");
    let out = convert_file(&copy("alpha.heic", &dir, "alpha.heic")).unwrap();
    let (w, _, px, _) = decode_jpeg(&out);
    assert!(close(rgb_at(&px, w, 8, 32), [255, 0, 0]));
    // 50 % blue over white.
    assert!(close(rgb_at(&px, w, 56, 32), [128, 128, 255]), "{:?}", rgb_at(&px, w, 56, 32));
}

#[test]
fn file_modification_time_is_preserved() {
    let dir = scratch("times");
    let src = copy("quadrants_srgb.heic", &dir, "old.heic");
    let past = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    fs::File::options().write(true).open(&src).unwrap().set_modified(past).unwrap();
    let out = convert_file(&src).unwrap();
    assert_eq!(fs::metadata(out).unwrap().modified().unwrap(), past);
}

#[test]
fn folder_batch_reports_each_file() {
    let dir = scratch("folder");
    copy("quadrants_srgb.heic", &dir, "one.heic");
    copy("with_thumbnail.heic", &dir, "two.HEIC");
    copy("corrupt_truncated.heic", &dir, "broken.heic");
    fs::write(dir.join("notes.txt"), b"x").unwrap();
    let batch = convert_all(&[&dir]);
    let p = batch.progress();
    assert!(p.done);
    assert_eq!((p.total, p.converted, p.failed, p.skipped), (3, 2, 1, 0));
    assert!(dir.join("one.jpg").is_file() && dir.join("two.jpg").is_file());
    assert!(!dir.join("broken.jpg").exists(), "no output for a broken file");
    assert!(leftovers(&dir).is_empty());
}

#[test]
fn non_heic_inputs_and_empty_folders_are_skipped() {
    let dir = scratch("skips");
    let txt = dir.join("readme.txt");
    fs::write(&txt, b"x").unwrap();
    let empty = dir.join("empty");
    fs::create_dir(&empty).unwrap();
    let batch = convert_all(&[txt.clone(), empty.clone()]);
    let results = batch.results();
    assert!(results.contains(&(txt, Outcome::NotHeic)));
    assert!(results.contains(&(empty, Outcome::NoHeicInFolder)));
    assert_eq!(batch.progress().converted, 0);
}

#[test]
fn duplicates_are_converted_once() {
    let dir = scratch("dupes");
    let src = copy("quadrants_srgb.heic", &dir, "x.heic");
    let batch = convert_all(&[src.clone(), src, dir.clone()]);
    assert_eq!(batch.progress().converted, 1);
}

#[test]
fn cancel_stops_queued_files() {
    let dir = scratch("cancel");
    for i in 0..5 {
        copy("quadrants_srgb.heic", &dir, &format!("{i}.heic"));
    }
    let batch = Batch::new();
    batch.add(&[&dir]);
    batch.cancel();
    for w in batch.spawn_workers(2) {
        w.join().unwrap();
    }
    let p = batch.progress();
    assert!(p.done && p.cancelled);
    assert_eq!(p.converted, 0);
    assert!(batch.results().iter().all(|(_, o)| *o == Outcome::Cancelled));
}

#[test]
fn batch_accepts_files_while_running() {
    let dir = scratch("growing");
    let batch = Batch::new();
    let workers = batch.spawn_workers(2);
    batch.add(&[copy("quadrants_srgb.heic", &dir, "a.heic")]);
    std::thread::sleep(Duration::from_millis(50));
    batch.add(&[copy("quadrants_srgb.heic", &dir, "b.heic")]);
    batch.close();
    for w in workers {
        w.join().unwrap();
    }
    assert_eq!(batch.progress().converted, 2);
}
