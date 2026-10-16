//! FNV-1a 64-bit hashing of raw byte slices and framebuffer pixel arrays.

/// Computes the FNV-1a 64-bit hash of a byte slice.
#[allow(dead_code)] // exercised only by the unit tests below
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Computes the FNV-1a 64-bit hash of a framebuffer, feeding each pixel's four bytes in little-endian order.
pub fn frame_hash(frame: &[u32]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for pixel in frame {
        for b in pixel.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// Formats the framebuffer hash as a single line: `hash:` followed by 16 lowercase hex digits.
pub fn hash_line(frame: &[u32]) -> String {
    format!("hash: {:016x}", frame_hash(frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e03_07_fnv1a64_empty_is_the_offset_basis() {
        assert_eq!(fnv1a64(&[]), 0xcbf29ce484222325);
    }

    #[test]
    fn e03_07_fnv1a64_of_a_matches_reference() {
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }

    #[test]
    fn e03_07_fnv1a64_of_foobar_matches_reference() {
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn e03_07_frame_hash_uses_little_endian_bytes() {
        let expected = fnv1a64(&[0x61, 0, 0, 0, 0x62, 0, 0, 0]);
        assert_eq!(frame_hash(&[0x61, 0x62]), expected);
    }

    #[test]
    fn e03_07_frame_hash_is_deterministic_and_sensitive() {
        let a = [0u32; 8];
        let mut b = a;
        b[3] = 1;
        assert_eq!(frame_hash(&a), frame_hash(&a));
        assert_ne!(frame_hash(&a), frame_hash(&b));
    }

    #[test]
    fn e03_07_hash_line_of_empty_frame_is_the_fnv_offset_basis() {
        assert_eq!(hash_line(&[]), "hash: cbf29ce484222325");
    }

    #[test]
    fn e03_07_hash_line_has_16_hex_digits() {
        let s = hash_line(&[1, 2, 3]);
        assert_eq!(s.len(), 6 + 16);
        for c in s["hash: ".len()..].chars() {
            assert!(matches!(c, '0'..='9' | 'a'..='f'));
        }
    }
}
