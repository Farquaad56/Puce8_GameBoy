//! PPU / video (decisions A_01/A_02): framebuffer for now; mode tracking and rendering
//! arrive with later tasks.

use crate::machine::FRAMEBUFFER_SIZE;

/// Video state: framebuffer of 2-bit pixel indices, 160 x 144 (decision A_04).
#[derive(Debug)]
pub struct Video {
    pub frame: [u8; FRAMEBUFFER_SIZE],
}

impl Default for Video {
    fn default() -> Self {
        Video {
            frame: [0x00; FRAMEBUFFER_SIZE],
        }
    }
}
