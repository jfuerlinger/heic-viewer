//! Decodes the HEIC files in /testdata (generated with tools/gen-fixtures.swift).
//! Quadrant pattern of the unrotated source: top-left red, top-right green,
//! bottom-left blue, bottom-right white.

use std::path::PathBuf;
use std::sync::Arc;

use heic_decoder::{
    decode_image, decode_thumbnail, decode_thumbnail_with_timeout, DecodeError, ImageSource, Limits, RgbaImage,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata").join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

const RED: [u8; 3] = [255, 0, 0];
const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];

fn assert_color(img: &RgbaImage, x: u32, y: u32, expected: [u8; 3], tolerance: u8) {
    let p = img.pixel(x, y);
    for c in 0..3 {
        assert!(
            p[c].abs_diff(expected[c]) <= tolerance,
            "pixel ({x},{y}) = {p:?}, expected ~{expected:?} (±{tolerance})"
        );
    }
}

/// Samples the centers of the four quadrants: [TL, TR, BL, BR].
fn assert_quadrants(img: &RgbaImage, expected: [[u8; 3]; 4], tolerance: u8) {
    let (w, h) = (img.width, img.height);
    assert_color(img, w / 4, h / 4, expected[0], tolerance);
    assert_color(img, 3 * w / 4, h / 4, expected[1], tolerance);
    assert_color(img, w / 4, 3 * h / 4, expected[2], tolerance);
    assert_color(img, 3 * w / 4, 3 * h / 4, expected[3], tolerance);
}

#[test]
fn libheif_is_loadable() {
    let v = heic_decoder::libheif_version();
    assert!(!v.is_empty());
    eprintln!("libheif {v}");
}

#[test]
fn small_image_is_not_upscaled() {
    let d = decode_thumbnail(&fixture("quadrants_srgb.heic"), 256, &Limits::default()).unwrap();
    assert_eq!((d.image.width, d.image.height), (96, 64));
    assert_eq!(d.source, ImageSource::PrimaryImage);
    assert!(!d.has_alpha);
    assert_quadrants(&d.image, [RED, GREEN, BLUE, WHITE], 12);
    assert!(d.image.pixels.chunks(4).all(|p| p[3] == 255));
}

#[test]
fn downscales_to_requested_edge() {
    let d = decode_thumbnail(&fixture("quadrants_srgb.heic"), 48, &Limits::default()).unwrap();
    assert_eq!((d.image.width, d.image.height), (48, 32));
    assert_quadrants(&d.image, [RED, GREEN, BLUE, WHITE], 12);
}

#[test]
fn orientation_is_applied() {
    // Orientation 6 = rotate 90° clockwise for display.
    let d = decode_thumbnail(&fixture("quadrants_orientation6.heic"), 256, &Limits::default()).unwrap();
    assert_eq!((d.image.width, d.image.height), (64, 96));
    assert_quadrants(&d.image, [BLUE, RED, WHITE, GREEN], 12);
}

#[test]
fn embedded_thumbnail_is_used_when_large_enough() {
    let data = fixture("with_thumbnail.heic");
    let d = decode_thumbnail(&data, 256, &Limits::default()).unwrap();
    assert_eq!(d.source, ImageSource::EmbeddedThumbnail);
    assert_eq!((d.image.width, d.image.height), (256, 192));
    assert_quadrants(&d.image, [RED, GREEN, BLUE, WHITE], 16);

    // The embedded thumbnail is 320x240, so 512 needs the primary image.
    let d = decode_thumbnail(&data, 512, &Limits::default()).unwrap();
    assert_eq!(d.source, ImageSource::PrimaryImage);
    assert_eq!((d.image.width, d.image.height), (512, 384));
    assert_quadrants(&d.image, [RED, GREEN, BLUE, WHITE], 12);
}

#[test]
fn embedded_thumbnail_respects_orientation() {
    let d = decode_thumbnail(&fixture("with_thumbnail_orientation6.heic"), 256, &Limits::default()).unwrap();
    assert_eq!(d.source, ImageSource::EmbeddedThumbnail);
    assert_eq!((d.image.width, d.image.height), (192, 256));
    assert_quadrants(&d.image, [BLUE, RED, WHITE, GREEN], 16);
}

fn p3_to_srgb_8bit(p3: [f32; 3]) -> [u8; 3] {
    let dec = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let enc = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.0031308 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    let l = [dec(p3[0]), dec(p3[1]), dec(p3[2])];
    let m = [[1.2249, -0.2247, 0.0], [-0.0420, 1.0419, 0.0], [-0.0197, -0.0786, 1.0979]];
    let mut out = [0u8; 3];
    for i in 0..3 {
        let v = m[i][0] * l[0] + m[i][1] * l[1] + m[i][2] * l[2];
        out[i] = (enc(v) * 255.0).round() as u8;
    }
    out
}

#[test]
fn display_p3_icc_is_converted_to_srgb() {
    let d = decode_thumbnail(&fixture("p3_color.heic"), 256, &Limits::default()).unwrap();
    let expected = p3_to_srgb_8bit([0.8, 0.2, 0.2]);
    let got = d.image.pixel(32, 32);
    eprintln!("P3 (0.8,0.2,0.2): got {got:?}, expected {expected:?}, unconverted would be [204, 51, 51]");
    assert_color(&d.image, 32, 32, expected, 6);
    // Make sure it really differs from the unconverted values.
    assert!(got[0].abs_diff(204) > 8 && got[1].abs_diff(51) > 8);
}

#[test]
fn ten_bit_p3_is_reduced_to_8bit_srgb() {
    let d = decode_thumbnail(&fixture("tenbit_p3.heic"), 256, &Limits::default()).unwrap();
    assert_eq!((d.image.width, d.image.height), (64, 48));
    // Saturated P3 primaries are clipped to the sRGB primaries.
    assert_quadrants(&d.image, [RED, GREEN, BLUE, WHITE], 14);
}

#[test]
fn alpha_is_preserved_unpremultiplied() {
    let d = decode_thumbnail(&fixture("alpha.heic"), 256, &Limits::default()).unwrap();
    assert!(d.has_alpha);
    let left = d.image.pixel(8, 32);
    let right = d.image.pixel(56, 32);
    assert!(left[3] >= 250 && left[0] >= 240 && left[2] <= 15, "left {left:?}");
    assert!(right[3].abs_diff(128) <= 6, "right {right:?}");
    assert!(right[2] >= 235 && right[0] <= 20, "right {right:?}");
}

#[test]
fn full_resolution_decode() {
    let d = decode_image(&fixture("with_thumbnail.heic"), &Limits::default()).unwrap();
    assert_eq!(d.source, ImageSource::PrimaryImage);
    assert_eq!((d.image.width, d.image.height), (1024, 768));
}

#[test]
fn corrupt_and_garbage_input_fail_cleanly() {
    let limits = Limits::default();
    assert!(matches!(
        decode_thumbnail(&fixture("corrupt_truncated.heic"), 256, &limits),
        Err(DecodeError::Heif { .. })
    ));
    assert!(decode_thumbnail(b"definitely not a heic file", 256, &limits).is_err());
    assert!(decode_thumbnail(&[0u8; 4096], 256, &limits).is_err());
    let mut flipped = fixture("quadrants_srgb.heic");
    for b in flipped.iter_mut().skip(200) {
        *b ^= 0x5a;
    }
    assert!(decode_thumbnail(&flipped, 256, &limits).is_err());
}

#[test]
fn pixel_limit_is_enforced() {
    let limits = Limits { max_pixels: 1000, ..Limits::default() };
    let err = decode_thumbnail(&fixture("quadrants_srgb.heic"), 256, &limits).unwrap_err();
    assert!(matches!(err, DecodeError::ImageTooLarge { .. } | DecodeError::Heif { .. }), "{err:?}");
}

#[test]
fn input_size_limit_is_enforced() {
    let limits = Limits { max_input_bytes: 100, ..Limits::default() };
    let data = fixture("quadrants_srgb.heic");
    assert_eq!(decode_thumbnail(&data, 256, &limits).unwrap_err(), DecodeError::InputSize(data.len()));
}

#[test]
fn threaded_decode_with_timeout() {
    let data: Arc<[u8]> = fixture("with_thumbnail.heic").into();
    let d = decode_thumbnail_with_timeout(data, 256, Limits::default()).unwrap();
    assert_eq!((d.image.width, d.image.height), (256, 192));
}

#[test]
fn exif_is_extracted_as_tiff() {
    let exif = heic_decoder::read_exif(&fixture("quadrants_orientation6.heic"), &Limits::default()).unwrap().unwrap();
    assert!(exif.starts_with(b"MM\0*") || exif.starts_with(b"II*\0"), "{:?}", &exif[..8]);
    assert_eq!(heic_decoder::read_exif(&fixture("quadrants_srgb.heic"), &Limits::default()).unwrap(), None);
}
