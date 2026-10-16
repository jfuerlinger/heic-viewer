//! Streaming area-average (box) downscaler.
//!
//! Reads an interleaved 8/16-bit plane once, row by row, and emits output rows as normalized
//! straight-alpha RGBA f32. Memory use is O(output width), independent of the source size.
//! Averaging happens on premultiplied values so transparent pixels do not bleed color.

use crate::heif::Plane;

/// Largest size that fits into `max_edge` x `max_edge` keeping the aspect ratio. Never upscales.
pub fn fit_within(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    if max_edge == 0 || (width <= max_edge && height <= max_edge) {
        return (width.max(1), height.max(1));
    }
    let scale = max_edge as f64 / width.max(height) as f64;
    let w = ((width as f64 * scale).round() as u32).clamp(1, max_edge);
    let h = ((height as f64 * scale).round() as u32).clamp(1, max_edge);
    (w, h)
}

pub fn downscale(
    plane: &Plane<'_>,
    dst_w: usize,
    dst_h: usize,
    premultiplied_src: bool,
    mut emit: impl FnMut(usize, &mut [f32]),
) {
    let (sw, sh) = (plane.width, plane.height);
    debug_assert!(dst_w >= 1 && dst_h >= 1 && dst_w <= sw && dst_h <= sh);

    let xmap: Vec<u32> = (0..sw).map(|x| (x * dst_w / sw) as u32).collect();
    let mut xcount = vec![0u32; dst_w];
    for &dx in &xmap {
        xcount[dx as usize] += 1;
    }

    let scale = if plane.bits == 0 { 1.0 / 255.0 } else { 1.0 / ((1u32 << plane.bits) - 1) as f32 };
    let has_alpha = plane.channels == 4;
    let mut acc = vec![0f32; dst_w * 4];
    let mut out = vec![0f32; dst_w * 4];
    let mut rows_in_acc = 0u32;
    let mut current_dy = 0usize;

    let mut flush = |acc: &mut [f32], rows: u32, dy: usize, out: &mut [f32]| {
        for dx in 0..dst_w {
            let n = (xcount[dx] * rows) as f32;
            let a = acc[dx * 4 + 3] / n;
            let o = &mut out[dx * 4..dx * 4 + 4];
            if a > 0.0 {
                o[0] = (acc[dx * 4] / n / a).min(1.0);
                o[1] = (acc[dx * 4 + 1] / n / a).min(1.0);
                o[2] = (acc[dx * 4 + 2] / n / a).min(1.0);
            } else {
                o[0] = 0.0;
                o[1] = 0.0;
                o[2] = 0.0;
            }
            o[3] = a.min(1.0);
        }
        emit(dy, out);
        acc.iter_mut().for_each(|v| *v = 0.0);
    };

    for y in 0..sh {
        let dy = y * dst_h / sh;
        if dy != current_dy {
            flush(&mut acc, rows_in_acc, current_dy, &mut out);
            rows_in_acc = 0;
            current_dy = dy;
        }
        let row = &plane.data[y * plane.stride..];
        for (x, &dx) in xmap.iter().enumerate() {
            let base = x * plane.channels;
            let sample = |c: usize| -> f32 {
                if plane.bits == 0 {
                    row[base + c] as f32 * scale
                } else {
                    let i = (base + c) * 2;
                    (u16::from_le_bytes([row[i], row[i + 1]]) as f32 * scale).min(1.0)
                }
            };
            let (mut r, mut g, mut b) = (sample(0), sample(1), sample(2));
            let a = if has_alpha { sample(3) } else { 1.0 };
            if !premultiplied_src {
                r *= a;
                g *= a;
                b *= a;
            }
            let o = dx as usize * 4;
            acc[o] += r;
            acc[o + 1] += g;
            acc[o + 2] += b;
            acc[o + 3] += a;
        }
        rows_in_acc += 1;
    }
    flush(&mut acc, rows_in_acc, current_dy, &mut out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &[u8], w: usize, h: usize, ch: usize, dw: usize, dh: usize) -> Vec<Vec<f32>> {
        let plane = Plane { data, stride: w * ch, width: w, height: h, channels: ch, bits: 0 };
        let mut rows = vec![Vec::new(); dh];
        downscale(&plane, dw, dh, false, |y, r| rows[y] = r.to_vec());
        rows
    }

    #[test]
    fn fit_within_keeps_aspect_and_never_upscales() {
        assert_eq!(fit_within(4032, 3024, 256), (256, 192));
        assert_eq!(fit_within(3024, 4032, 256), (192, 256));
        assert_eq!(fit_within(100, 50, 256), (100, 50));
        assert_eq!(fit_within(10000, 1, 256), (256, 1));
        assert_eq!(fit_within(640, 480, 0), (640, 480));
    }

    #[test]
    fn averages_2x2_blocks() {
        // 4x2 RGB: left block black/white mix, right block pure red.
        #[rustfmt::skip]
        let data = [
            0, 0, 0,   255, 255, 255,   255, 0, 0,   255, 0, 0,
            255, 255, 255,   0, 0, 0,   255, 0, 0,   255, 0, 0,
        ];
        let rows = run(&data, 4, 2, 3, 2, 1);
        let r = &rows[0];
        assert!((r[0] - 0.5).abs() < 1e-6 && (r[1] - 0.5).abs() < 1e-6);
        assert_eq!(&r[4..8], &[1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn transparent_pixels_do_not_bleed_color() {
        // Opaque blue next to fully transparent "red" garbage.
        let data = [0, 0, 255, 255, 255, 0, 0, 0];
        let rows = run(&data, 2, 1, 4, 1, 1);
        let p = &rows[0];
        assert_eq!(&p[0..3], &[0.0, 0.0, 1.0]);
        assert!((p[3] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn identity_size_is_a_copy_and_reads_16bit() {
        let v: u16 = 1023; // 10-bit full scale
        let px = [v.to_le_bytes(), 0u16.to_le_bytes(), 512u16.to_le_bytes()].concat();
        let plane = Plane { data: &px, stride: 6, width: 1, height: 1, channels: 3, bits: 10 };
        let mut got = Vec::new();
        downscale(&plane, 1, 1, false, |_, r| got = r.to_vec());
        assert_eq!(got[0], 1.0);
        assert_eq!(got[1], 0.0);
        assert!((got[2] - 512.0 / 1023.0).abs() < 1e-6);
        assert_eq!(got[3], 1.0);
    }
}
