//! RGBA (straight alpha) -> BGRA as expected by a 32-bit top-down DIB section.

/// Writes BGRA pixels into `dst` (len = width*height*4). Returns true if the image has any
/// transparency; in that case the colors are premultiplied (required for WTSAT_ARGB),
/// otherwise alpha is forced to 255 (WTSAT_RGB).
pub fn rgba_to_bgra(src: &[u8], dst: &mut [u8]) -> bool {
    debug_assert_eq!(src.len(), dst.len());
    let transparent = src.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
    for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
        if transparent {
            let a = s[3] as u32;
            d[0] = premultiply(s[2], a);
            d[1] = premultiply(s[1], a);
            d[2] = premultiply(s[0], a);
            d[3] = s[3];
        } else {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 255;
        }
    }
    transparent
}

fn premultiply(c: u8, a: u32) -> u8 {
    ((c as u32 * a + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_is_swizzled_only() {
        let mut out = [0u8; 4];
        assert!(!rgba_to_bgra(&[10, 20, 30, 255], &mut out));
        assert_eq!(out, [30, 20, 10, 255]);
    }

    #[test]
    fn transparent_is_premultiplied() {
        let mut out = [0u8; 8];
        assert!(rgba_to_bgra(&[255, 0, 0, 255, 0, 0, 255, 128], &mut out));
        assert_eq!(out, [0, 0, 255, 255, 128, 0, 0, 128]);
    }
}
