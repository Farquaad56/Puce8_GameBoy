//! Binary PPM (P6) writer for the framebuffer (E03_16).

/// Write `frame` (0xAARRGGBB pixels, 160 x 144) as a binary PPM: header `P6`, `160 144`, `255`,
/// then R, G, B bytes per pixel; the alpha byte is dropped.
pub fn write_ppm<W: std::io::Write>(frame: &[u32], out: &mut W) -> std::io::Result<()> {
    out.write_all(b"P6\n160 144\n255\n")?;
    let mut bytes = Vec::with_capacity(frame.len() * 3);
    for &px in frame {
        bytes.push((px >> 16) as u8);
        bytes.push((px >> 8) as u8);
        bytes.push(px as u8);
    }
    out.write_all(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e03_16_ppm_header_and_size() {
        let frame = vec![0xFFFF_FFFFu32; 23040];
        let mut out: Vec<u8> = Vec::new();
        write_ppm(&frame, &mut out).expect("write succeeds");
        assert_eq!(&out[..15], b"P6\n160 144\n255\n");
        assert_eq!(out.len(), 15 + 69120);
    }

    #[test]
    fn e03_16_ppm_pixel_conversion() {
        let mut frame = vec![0xFFFF_FFFFu32; 23040];
        frame[0] = 0xFF12_3456;
        let mut out: Vec<u8> = Vec::new();
        write_ppm(&frame, &mut out).expect("write succeeds");
        assert_eq!(&out[15..18], [0x12, 0x34, 0x56]);
        assert_eq!(&out[18..21], [0xFF, 0xFF, 0xFF]);
    }
}
