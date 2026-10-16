//! Minimal Exif handling: the pixels are already rotated, so IFD0 Orientation must become 1.

const ORIENTATION_TAG: u16 = 0x0112;
/// A JPEG segment holds at most 65535 bytes including its 2-byte length field.
const MAX_APP1_PAYLOAD: usize = 65533;

/// Wraps a raw TIFF structure into an APP1 Exif payload with Orientation reset to 1.
/// Returns None if the data is malformed or too large for a single APP1 segment.
pub fn app1_payload(tiff: &[u8]) -> Option<Vec<u8>> {
    let mut tiff = tiff.to_vec();
    reset_orientation(&mut tiff)?;
    let mut payload = Vec::with_capacity(6 + tiff.len());
    payload.extend_from_slice(b"Exif\0\0");
    payload.extend_from_slice(&tiff);
    (payload.len() <= MAX_APP1_PAYLOAD).then_some(payload)
}

/// Sets IFD0 Orientation to 1 if present. Returns None for malformed TIFF data.
fn reset_orientation(tiff: &mut [u8]) -> Option<()> {
    let big_endian = match tiff.get(0..4)? {
        b"MM\0*" => true,
        b"II*\0" => false,
        _ => return None,
    };
    let u16_at = |t: &[u8], i: usize| -> Option<u16> {
        let b = [*t.get(i)?, *t.get(i + 1)?];
        Some(if big_endian { u16::from_be_bytes(b) } else { u16::from_le_bytes(b) })
    };
    let ifd0 = {
        let b = [*tiff.get(4)?, *tiff.get(5)?, *tiff.get(6)?, *tiff.get(7)?];
        (if big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }) as usize
    };
    let count = u16_at(tiff, ifd0)? as usize;
    for i in 0..count {
        let entry = ifd0 + 2 + i * 12;
        if u16_at(tiff, entry)? == ORIENTATION_TAG {
            // SHORT value stored inline in the first two bytes of the value field.
            let value = if big_endian { 1u16.to_be_bytes() } else { 1u16.to_le_bytes() };
            tiff.get_mut(entry + 8..entry + 10)?.copy_from_slice(&value);
            return Some(());
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiff_with_orientation(big_endian: bool, orientation: u16) -> Vec<u8> {
        let w16 = |v: u16| if big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        let w32 = |v: u32| if big_endian { v.to_be_bytes() } else { v.to_le_bytes() };
        let mut t = Vec::new();
        t.extend_from_slice(if big_endian { b"MM\0*" } else { b"II*\0" });
        t.extend_from_slice(&w32(8));
        t.extend_from_slice(&w16(1)); // one entry
        t.extend_from_slice(&w16(ORIENTATION_TAG));
        t.extend_from_slice(&w16(3)); // SHORT
        t.extend_from_slice(&w32(1));
        t.extend_from_slice(&w16(orientation));
        t.extend_from_slice(&[0, 0]);
        t.extend_from_slice(&w32(0)); // no next IFD
        t
    }

    #[test]
    fn orientation_is_reset_in_both_byte_orders() {
        for be in [true, false] {
            let payload = app1_payload(&tiff_with_orientation(be, 6)).unwrap();
            assert!(payload.starts_with(b"Exif\0\0"));
            assert_eq!(payload[6..], tiff_with_orientation(be, 1)[..]);
        }
    }

    #[test]
    fn malformed_or_oversized_data_is_dropped() {
        assert!(app1_payload(b"garbage!").is_none());
        let mut t = tiff_with_orientation(true, 1);
        t[9] = 200; // claims 200 entries, far beyond the end of the data ...
        t[11] = 0x01; // ... and the first entry is not the Orientation tag
        assert!(app1_payload(&t).is_none());
        let mut big = tiff_with_orientation(false, 1);
        big.resize(70_000, 0);
        assert!(app1_payload(&big).is_none());
    }
}
