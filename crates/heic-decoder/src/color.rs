//! Conversion of decoded RGB (in the source color space) to sRGB.
//!
//! - ICC profiles (e.g. Apple "Display P3") are handled by moxcms.
//! - nclx/CICP signalling is handled here: primaries via chromaticity matrices (+ Bradford
//!   adaptation), transfer functions incl. PQ/HLG with a simple highlight roll-off so HDR
//!   content lands in SDR sRGB.
//!
//! Any failure falls back to treating the data as sRGB instead of failing the thumbnail.

use std::sync::Arc;

use moxcms::{ColorProfile, Layout, TransformExecutor, TransformOptions};

#[derive(Debug, Clone, PartialEq)]
pub struct Nclx {
    pub primaries: u16,
    pub transfer: u16,
    /// rx, ry, gx, gy, bx, by, wx, wy as decoded by libheif.
    pub chromaticities: [f32; 8],
}

#[derive(Debug, Clone, PartialEq)]
pub enum SourceProfile {
    Unknown,
    Icc(Vec<u8>),
    Nclx(Nclx),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Transfer {
    Srgb,
    Gamma(f32),
    Linear,
    Pq,
    Hlg,
}

/// SDR reference white in nits (ITU-R BT.2408).
const REFERENCE_WHITE_NITS: f32 = 203.0;
/// Peak luminance assumed for HLG content.
const HLG_PEAK_NITS: f32 = 1000.0;
/// Linear values above this knee get compressed smoothly towards 1.0.
const TONEMAP_KNEE: f32 = 0.75;

pub(crate) enum Converter {
    Identity,
    Icc(Arc<dyn TransformExecutor<f32> + Send + Sync>),
    Nclx { transfer: Transfer, matrix: Option<[[f32; 3]; 3]> },
}

impl Converter {
    pub fn new(profile: &SourceProfile) -> Converter {
        match profile {
            SourceProfile::Unknown => Converter::Identity,
            SourceProfile::Icc(icc) => icc_converter(icc).unwrap_or(Converter::Identity),
            SourceProfile::Nclx(n) => nclx_converter(n),
        }
    }

    #[cfg(test)]
    pub fn is_identity(&self) -> bool {
        matches!(self, Converter::Identity)
    }

    /// Converts one row of straight-alpha RGBA f32 in place. `scratch` must have the same length.
    pub fn convert_row(&self, row: &mut [f32], scratch: &mut Vec<f32>) {
        match self {
            Converter::Identity => {}
            Converter::Icc(t) => {
                scratch.resize(row.len(), 0.0);
                if t.transform(row, scratch).is_ok() {
                    for (dst, src) in row.as_chunks_mut::<4>().0.iter_mut().zip(scratch.as_chunks::<4>().0) {
                        dst[0] = src[0].clamp(0.0, 1.0);
                        dst[1] = src[1].clamp(0.0, 1.0);
                        dst[2] = src[2].clamp(0.0, 1.0);
                    }
                }
            }
            Converter::Nclx { transfer, matrix } => {
                for px in row.as_chunks_mut::<4>().0 {
                    let mut rgb =
                        [to_linear(*transfer, px[0]), to_linear(*transfer, px[1]), to_linear(*transfer, px[2])];
                    if let Some(m) = matrix {
                        rgb = mul(m, rgb);
                    }
                    if matches!(transfer, Transfer::Pq | Transfer::Hlg) {
                        rgb = tonemap(rgb);
                    }
                    px[0] = srgb_encode(rgb[0]);
                    px[1] = srgb_encode(rgb[1]);
                    px[2] = srgb_encode(rgb[2]);
                }
            }
        }
    }
}

fn icc_converter(icc: &[u8]) -> Option<Converter> {
    let src = ColorProfile::new_from_slice(icc).ok()?;
    if src.color_space != moxcms::DataColorSpace::Rgb {
        return None;
    }
    let dst = ColorProfile::new_srgb();
    let t = src.create_transform_f32(Layout::Rgba, &dst, Layout::Rgba, TransformOptions::default()).ok()?;
    Some(Converter::Icc(t))
}

fn nclx_converter(n: &Nclx) -> Converter {
    let transfer = match n.transfer {
        8 => Transfer::Linear,
        4 => Transfer::Gamma(2.2),
        5 => Transfer::Gamma(2.8),
        16 => Transfer::Pq,
        18 => Transfer::Hlg,
        // 1, 6, 14, 15 (BT.709 family), 13 (sRGB), 2 (unspecified) and anything exotic.
        _ => Transfer::Srgb,
    };
    let matrix = match n.primaries {
        1 | 2 => None,
        _ => primaries_to_srgb_matrix(&n.chromaticities),
    };
    if transfer == Transfer::Srgb && matrix.is_none() {
        Converter::Identity
    } else {
        Converter::Nclx { transfer, matrix }
    }
}

fn to_linear(t: Transfer, v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    match t {
        Transfer::Srgb => srgb_decode(v),
        Transfer::Gamma(g) => v.powf(g),
        Transfer::Linear => v,
        Transfer::Pq => pq_eotf_nits(v) / REFERENCE_WHITE_NITS,
        Transfer::Hlg => hlg_to_display_nits(v) / REFERENCE_WHITE_NITS,
    }
}

pub(crate) fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub(crate) fn srgb_encode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// SMPTE ST 2084 EOTF, returns absolute luminance in nits.
pub(crate) fn pq_eotf_nits(v: f32) -> f32 {
    const M1: f32 = 2610.0 / 16384.0;
    const M2: f32 = 2523.0 / 4096.0 * 128.0;
    const C1: f32 = 3424.0 / 4096.0;
    const C2: f32 = 2413.0 / 4096.0 * 32.0;
    const C3: f32 = 2392.0 / 4096.0 * 32.0;
    let p = v.powf(1.0 / M2);
    let num = (p - C1).max(0.0);
    let den = C2 - C3 * p;
    10000.0 * (num / den).powf(1.0 / M1)
}

/// HLG inverse OETF followed by a per-channel approximation of the BT.2100 OOTF (gamma 1.2).
fn hlg_to_display_nits(v: f32) -> f32 {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_7;
    let scene = if v <= 0.5 { v * v / 3.0 } else { (((v - C) / A).exp() + B) / 12.0 };
    HLG_PEAK_NITS * scene.powf(1.2)
}

/// Keeps values below the knee untouched and rolls highlights off asymptotically towards 1.0.
/// Applied on the max channel to preserve hue.
fn tonemap(rgb: [f32; 3]) -> [f32; 3] {
    let m = rgb[0].max(rgb[1]).max(rgb[2]);
    if m <= TONEMAP_KNEE {
        return rgb;
    }
    let x = (m - TONEMAP_KNEE) / (1.0 - TONEMAP_KNEE);
    let mapped = TONEMAP_KNEE + (1.0 - TONEMAP_KNEE) * (x / (1.0 + x));
    let s = mapped / m;
    [rgb[0] * s, rgb[1] * s, rgb[2] * s]
}

type M3 = [[f32; 3]; 3];

fn mul(m: &M3, v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn mat_mul(a: &M3, b: &M3) -> M3 {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn invert(m: &M3) -> Option<M3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-9 || !det.is_finite() {
        return None;
    }
    let d = 1.0 / det;
    Some([
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d,
        ],
    ])
}

fn xy_to_xyz(x: f32, y: f32) -> [f32; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

fn rgb_to_xyz(c: &[f32; 8]) -> Option<M3> {
    if c.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v >= 1.0) {
        return None;
    }
    let (r, g, b, w) = (xy_to_xyz(c[0], c[1]), xy_to_xyz(c[2], c[3]), xy_to_xyz(c[4], c[5]), xy_to_xyz(c[6], c[7]));
    let p = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let s = mul(&invert(&p)?, w);
    Some([
        [p[0][0] * s[0], p[0][1] * s[1], p[0][2] * s[2]],
        [p[1][0] * s[0], p[1][1] * s[1], p[1][2] * s[2]],
        [p[2][0] * s[0], p[2][1] * s[1], p[2][2] * s[2]],
    ])
}

const SRGB_CHROMATICITIES: [f32; 8] = [0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290];

fn bradford(src_white: [f32; 3], dst_white: [f32; 3]) -> Option<M3> {
    const B: M3 = [[0.8951, 0.2664, -0.1614], [-0.7502, 1.7135, 0.0367], [0.0389, -0.0685, 1.0296]];
    let s = mul(&B, src_white);
    let d = mul(&B, dst_white);
    let scale = [[d[0] / s[0], 0.0, 0.0], [0.0, d[1] / s[1], 0.0], [0.0, 0.0, d[2] / s[2]]];
    Some(mat_mul(&invert(&B)?, &mat_mul(&scale, &B)))
}

/// Linear source RGB -> linear sRGB.
pub(crate) fn primaries_to_srgb_matrix(src: &[f32; 8]) -> Option<M3> {
    let src_to_xyz = rgb_to_xyz(src)?;
    let xyz_to_srgb = invert(&rgb_to_xyz(&SRGB_CHROMATICITIES)?)?;
    let src_white = xy_to_xyz(src[6], src[7]);
    let d65 = xy_to_xyz(SRGB_CHROMATICITIES[6], SRGB_CHROMATICITIES[7]);
    let adapt = if (src[6] - SRGB_CHROMATICITIES[6]).abs() > 1e-3 || (src[7] - SRGB_CHROMATICITIES[7]).abs() > 1e-3 {
        bradford(src_white, d65)?
    } else {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    };
    Some(mat_mul(&xyz_to_srgb, &mat_mul(&adapt, &src_to_xyz)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const P3_D65: [f32; 8] = [0.680, 0.320, 0.265, 0.690, 0.150, 0.060, 0.3127, 0.3290];

    #[test]
    fn srgb_roundtrip() {
        for i in 0..=255 {
            let v = i as f32 / 255.0;
            assert!((srgb_encode(srgb_decode(v)) - v).abs() < 1e-5);
        }
    }

    #[test]
    fn p3_matrix_matches_reference() {
        // Well-known Display P3 -> sRGB linear matrix.
        let expected = [[1.2249, -0.2247, 0.0], [-0.0420, 1.0419, 0.0], [-0.0197, -0.0786, 1.0979]];
        let m = primaries_to_srgb_matrix(&P3_D65).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!((m[i][j] - expected[i][j]).abs() < 2e-3, "{m:?}");
            }
        }
    }

    #[test]
    fn srgb_nclx_is_identity() {
        for primaries in [1, 2] {
            for transfer in [1, 2, 6, 13] {
                let n = Nclx { primaries, transfer, chromaticities: SRGB_CHROMATICITIES };
                assert!(Converter::new(&SourceProfile::Nclx(n)).is_identity());
            }
        }
    }

    #[test]
    fn pq_reference_points() {
        assert!(pq_eotf_nits(0.0) < 1e-3);
        assert!((pq_eotf_nits(1.0) - 10000.0).abs() < 1.0);
        // 0.58 code value is ~203 nits (BT.2408 reference white).
        assert!((pq_eotf_nits(0.5806) - 203.0).abs() < 3.0);
    }

    #[test]
    fn pq_bt2020_white_and_highlights_stay_in_range() {
        let bt2020 = [0.708, 0.292, 0.170, 0.797, 0.131, 0.046, 0.3127, 0.3290];
        let conv = Converter::new(&SourceProfile::Nclx(Nclx { primaries: 9, transfer: 16, chromaticities: bt2020 }));
        let mut row = vec![0.5806, 0.5806, 0.5806, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        conv.convert_row(&mut row, &mut Vec::new());
        // Reference white stays a bright neutral, peak white maps to (nearly) 1.0, black stays black.
        assert!(row[0] > 0.85 && (row[0] - row[1]).abs() < 1e-3 && (row[1] - row[2]).abs() < 1e-3);
        assert!(row[4] > 0.97 && row[4] <= 1.0);
        assert_eq!(&row[8..11], &[0.0, 0.0, 0.0]);
        assert_eq!(row[3], 1.0);
    }

    #[test]
    fn tonemap_is_monotonic_and_bounded() {
        let mut last = 0.0;
        for i in 0..1000 {
            let v = i as f32 * 0.05;
            let out = tonemap([v, 0.0, 0.0])[0];
            assert!(out >= last && out < 1.0 + 1e-6);
            last = out;
        }
    }

    #[test]
    fn broken_icc_falls_back_to_identity() {
        assert!(Converter::new(&SourceProfile::Icc(vec![1, 2, 3])).is_identity());
    }
}
