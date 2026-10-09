//! Machine trait and the Dmg (DMG) machine: a pure function of ticks, no wall clock, no I/O
//! (decisions A_01/A_04).

use crate::audio::Audio;
use crate::bus::{Bus, ROM_HEADER_CHECKSUM};
use crate::cpu::Cpu;
use crate::input::Input;
use crate::media::Media;
use crate::savestate::StateError;
use crate::video::Video;

/// Dots per frame: 70224 (note 01_timing.md, decision A_01).
pub const FRAME_DOTS: u32 = 70224;

/// Framebuffer size in pixels: 160 x 144 of 2-bit indices (decision A_04).
pub const FRAMEBUFFER_SIZE: usize = 23040;

/// Audio ring capacity in interleaved samples: 4096 stereo pairs at 32768 Hz, ~0.125 s
/// (decision A_04).
pub const AUDIO_RING_SAMPLES: usize = 2 * 4096;

/// Error returned when a ROM cannot be loaded (bad ROM => Result, never panic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// The ROM is empty or shorter than the cartridge header ($0100-$014F, note 07a).
    TooShort,
}

/// Core machine interface (decisions A_01/A_04/A_05).
pub trait Machine {
    /// Reset to the documented post-boot state at PC=$0100 without running the boot ROM
    /// (note 08, decision A_06).
    fn reset(&mut self);

    /// Advance by one dot (T-cycle) (decision A_01). No allocation.
    fn tick(&mut self);

    /// Advance exactly one frame: 70224 dots (note 01_timing.md, decision A_01).
    fn run_frame(&mut self) {
        for _ in 0..FRAME_DOTS {
            self.tick();
        }
    }

    /// Framebuffer of 2-bit pixel indices, 160 x 144 (decision A_04).
    fn framebuffer(&self) -> &[u8; FRAMEBUFFER_SIZE];

    /// Drain up to `into.len()` interleaved stereo samples from the ring at 32768 Hz
    /// (decision A_04). Returns the number of samples drained.
    fn drain_audio(&mut self, into: &mut [i16]) -> usize;

    /// Drain up to `out.len()` bytes sent on the serial port SB ($FF01), oldest first
    /// (C01_03); returns the number of bytes drained. Test ROMs print through it.
    fn take_serial_output(&mut self, out: &mut [u8]) -> usize;

    /// Set the current button state; the frontend drains it at frame boundaries
    /// (decision A_04).
    fn set_input(&mut self, input: Input);

    /// Serialize the full machine state into a caller-provided buffer (decision A_05).
    fn save_state(&self, buf: &mut [u8]) -> Result<(), StateError>;

    /// Restore the full machine state from a buffer produced by `save_state` (decision A_05).
    fn load_state(&mut self, buf: &[u8]) -> Result<(), StateError>;
}

/// DMG machine owning every component (decisions A_02/A_04).
/// Invariant: `bus.rom.len() >= Dmg::MIN_ROM_LEN`.
#[derive(Debug)]
pub struct Dmg {
    pub bus: Bus,
    pub cpu: Cpu,
    pub video: Video,
    pub audio: Audio,
    pub media: Media,
    /// Current button state (decision A_04).
    pub input: Input,
    /// Dot phase within the current M-cycle; 0..3, wraps to 0 every 4 dots (decision A_01).
    dot_phase: u8,
}

impl Dmg {
    /// Minimum ROM length: the header occupies $0100-$014F of bank 0 (note 07a).
    pub const MIN_ROM_LEN: usize = 0x150;

    /// Load a ROM image and build the machine in its post-boot state.
    pub fn new(rom: &[u8]) -> Result<Dmg, LoadError> {
        if rom.len() < Self::MIN_ROM_LEN {
            return Err(LoadError::TooShort);
        }
        let mut dmg = Dmg {
            bus: Bus::new(rom.to_vec()),
            cpu: Cpu::default(),
            video: Video::default(),
            audio: Audio::default(),
            media: Media,
            input: Input::default(),
            dot_phase: 0,
        };
        dmg.reset();
        Ok(dmg)
    }
}

impl Machine for Dmg {
    fn reset(&mut self) {
        // Header checksum byte $014D (note 07a); the invariant guarantees it is present.
        let checksum = self.bus.rom[ROM_HEADER_CHECKSUM];
        self.cpu = Cpu::reset(checksum);
        self.bus.reset();
        self.video.reset(); // LY and dot counter restart at line 0 (C01_15)
        self.audio.head = 0;
        self.audio.tail = 0;
        self.dot_phase = 0;
    }

    fn tick(&mut self) {
        // Chip order CPU -> Timer -> DMA -> PPU -> APU -> Serial (decision A_01).
        // The CPU advances one M-cycle every 4 dots (note 01_timing.md); the other chips
        // are filled in by later tasks.
        self.dot_phase = (self.dot_phase + 1) % 4;
        if self.dot_phase == 0 {
            self.cpu.tick(&mut self.bus);
        }
        // PPU advances one dot per Machine::tick() (decision A_01, C01_15). LY is mirrored
        // into the bus so $FF44 reads are side-effect-free; entry into VBlank (LY 143 -> 144)
        // raises the IF vblank bit once per frame (note 02b "vblank", note 01_timing.md).
        let was_vblank = self.video.vblank();
        let line_changed = self.video.tick();
        self.bus.ly = self.video.ly();
        if line_changed && !was_vblank && self.video.vblank() {
            self.bus.io[0x0F] |= 0x01; // IF bit 0: vblank request (note 02b)
        }
    }

    fn framebuffer(&self) -> &[u8; FRAMEBUFFER_SIZE] {
        &self.video.frame
    }

    fn drain_audio(&mut self, into: &mut [i16]) -> usize {
        self.audio.drain(into)
    }

    fn take_serial_output(&mut self, out: &mut [u8]) -> usize {
        // Forwards to the bus capture buffer (C01_03).
        self.bus.take_serial(out)
    }

    fn set_input(&mut self, input: Input) {
        self.input = input;
    }

    fn save_state(&self, _buf: &mut [u8]) -> Result<(), StateError> {
        // Binary format (magic + version + cartridge type 0147) arrives with later tasks
        // (decision A_05).
        Ok(())
    }

    fn load_state(&mut self, _buf: &[u8]) -> Result<(), StateError> {
        // Binary format (magic + version + cartridge type 0147) arrives with later tasks
        // (decision A_05).
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM image of `len` bytes, all zero except the header checksum byte $014D.
    fn rom(len: usize, checksum: u8) -> Vec<u8> {
        let mut r = vec![0x00u8; len];
        if len > ROM_HEADER_CHECKSUM {
            r[ROM_HEADER_CHECKSUM] = checksum;
        }
        r
    }

    /// Post-boot IO register values from note 08 (offset in the IO file, value).
    const EXPECTED_IO: [(u8, u8); 42] = [
        (0x00, 0xCF), // P1
        (0x01, 0x00), // SB
        (0x02, 0x7E), // SC
        (0x04, 0xAB), // DIV
        (0x05, 0x00), // TIMA
        (0x06, 0x00), // TMA
        (0x07, 0xF8), // TAC
        (0x0F, 0xE1), // IF
        (0x10, 0x80), // NR10
        (0x11, 0xBF), // NR11
        (0x12, 0xF3), // NR12
        (0x13, 0xFF), // NR13
        (0x14, 0xBF), // NR14
        (0x15, 0x3F), // NR21
        (0x16, 0x00), // NR22
        (0x17, 0xFF), // NR23
        (0x18, 0xBF), // NR24
        (0x19, 0x7F), // NR30
        (0x1A, 0xFF), // NR31
        (0x1B, 0x9F), // NR32
        (0x1C, 0xFF), // NR33
        (0x1D, 0xBF), // NR34
        (0x20, 0xFF), // NR41
        (0x21, 0x00), // NR42
        (0x22, 0x00), // NR43
        (0x23, 0xBF), // NR44
        (0x24, 0x77), // NR50
        (0x25, 0xF3), // NR51
        (0x26, 0xF1), // NR52
        (0x40, 0x91), // LCDC
        (0x41, 0x85), // STAT
        (0x42, 0x00), // SCY
        (0x43, 0x00), // SCX
        (0x44, 0x00), // LY
        (0x45, 0x00), // LYC
        (0x46, 0xFF), // DMA
        (0x47, 0xFC), // BGP
        (0x48, 0xFF), // OBP0 (specs value; CONFLIT with pandocs, note 08)
        (0x49, 0xFF), // OBP1 (specs value; CONFLIT with pandocs, note 08)
        (0x4A, 0x00), // WY
        (0x4B, 0x00), // WX
        (0xFF, 0x00), // IE
    ];

    #[test]
    fn e01_01_valid_rom_loads() {
        let r = rom(32 * 1024, 0x00);
        let dmg = Dmg::new(&r).expect("a 32 KiB ROM loads");
        assert_eq!(dmg.bus.rom.len(), 32 * 1024);
        assert_eq!(dmg.cpu.pc, 0x0100);
    }

    #[test]
    fn e01_01_empty_rom_rejected() {
        assert!(matches!(Dmg::new(&[]), Err(LoadError::TooShort)));
    }

    #[test]
    fn e01_01_short_rom_rejected() {
        let r = rom(100, 0x00);
        assert!(matches!(Dmg::new(&r), Err(LoadError::TooShort)));
    }

    #[test]
    fn e01_01_reset_cpu_registers_checksum_zero() {
        let dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        assert_eq!(dmg.cpu.a, 0x01);
        // Z set, N clear; H and C clear because the checksum $014D is $00.
        assert_eq!(dmg.cpu.f, 0x80);
        assert_eq!(dmg.cpu.b, 0x00);
        assert_eq!(dmg.cpu.c, 0x13);
        assert_eq!(dmg.cpu.d, 0x00);
        assert_eq!(dmg.cpu.e, 0xD8);
        assert_eq!(dmg.cpu.h, 0x01);
        assert_eq!(dmg.cpu.l, 0x4D);
        assert_eq!(dmg.cpu.sp, 0xFFFE);
        assert_eq!(dmg.cpu.pc, 0x0100);
    }

    #[test]
    fn e01_01_reset_f_flags_follow_checksum() {
        let dmg = Dmg::new(&rom(32 * 1024, 0x5A)).expect("ROM loads");
        // Z set, N clear; H and C both set because the checksum $014D is not $00.
        assert_eq!(dmg.cpu.f, 0xB0);
    }

    #[test]
    fn e01_01_reset_io_registers() {
        let dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for (offset, value) in EXPECTED_IO {
            assert_eq!(dmg.bus.io[usize::from(offset)], value);
        }
    }

    #[test]
    fn e01_01_reset_ram_zeroed() {
        let dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        assert!(dmg.bus.wram.iter().all(|&b| b == 0x00));
        assert!(dmg.bus.hram.iter().all(|&b| b == 0x00));
    }

    #[test]
    fn e01_01_reset_restores_after_mutation() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x5A)).expect("ROM loads");
        dmg.cpu.a = 0x00;
        dmg.bus.io[0x40] = 0x00;
        dmg.reset();
        assert_eq!(dmg.cpu.a, 0x01);
        assert_eq!(dmg.cpu.f, 0xB0);
        assert_eq!(dmg.bus.io[0x40], 0x91);
    }

    #[test]
    fn e01_01_machine_trait_implemented() {
        fn assert_machine<M: Machine>(_m: &mut M) {}
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        assert_machine(&mut dmg);
    }

    #[test]
    fn e01_01_drain_audio_empty_ring() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        let mut buf = [0i16; 8];
        assert_eq!(dmg.drain_audio(&mut buf), 0);
    }

    #[test]
    fn e01_01_set_input_stores_state() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        let input = Input {
            a: true,
            ..Input::default()
        };
        dmg.set_input(input);
        assert!(dmg.input.a);
    }

    #[test]
    fn c01_02_four_n_ticks_fetch_n_bytes() {
        // A ROM of all-zero bytes is full of unknown opcodes: each M-cycle fetches one.
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        const N: u16 = 5;
        for _ in 0..(4 * N) {
            dmg.tick();
        }
        assert_eq!(dmg.cpu.pc, 0x0100 + N);
    }

    #[test]
    fn c01_02_three_ticks_do_not_advance_pc() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..3 {
            dmg.tick();
        }
        assert_eq!(dmg.cpu.pc, 0x0100);
    }

    #[test]
    fn c01_02_reset_restarts_phase() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Advance 3 dots: the phase is now 3 and the CPU has not ticked.
        for _ in 0..3 {
            dmg.tick();
        }
        assert_eq!(dmg.cpu.pc, 0x0100);

        // Reset restarts the phase at 0; without it the next dot would tick the CPU.
        dmg.reset();
        for _ in 0..3 {
            dmg.tick();
        }
        assert_eq!(dmg.cpu.pc, 0x0100);
    }

    #[test]
    fn c01_03_take_serial_output_drains_bus_capture() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Send two bytes through the serial port (note 06 "SB"/"SC").
        for byte in [0x41u8, 0x42] {
            dmg.bus.write(0xFF01, byte);
            dmg.bus.write(0xFF02, 0x81); // internal clock: captured at once (C01_03)
        }
        let mut out = [0u8; 8];
        assert_eq!(dmg.take_serial_output(&mut out), 2);
        assert_eq!(&out[..2], &[0x41, 0x42]); // oldest first
        assert_eq!(dmg.take_serial_output(&mut out), 0); // the buffer is drained
    }

    #[test]
    fn c01_03_take_serial_output_empty_after_reset() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF01, 0x41);
        dmg.bus.write(0xFF02, 0x81);
        assert_eq!(dmg.bus.serial_len, 1);
        dmg.reset(); // the capture buffer is part of the post-boot state (C01_03)
        let mut out = [0u8; 4];
        assert_eq!(dmg.take_serial_output(&mut out), 0);
    }

    #[test]
    fn c01_15_ly_visible_through_bus_read() {
        use crate::video::{LINE_DOTS, VBLANK_FIRST_LINE};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        assert_eq!(dmg.bus.read(0xFF44), 0x00); // post-boot LY (note 08)
                                                // Advance to the start of line 5: $FF44 must read 5 through the bus.
        for _ in 0..(LINE_DOTS * 5) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF44), 5);
        // Advance into VBlank (line 144): $FF44 reads the line index.
        for _ in 0..(LINE_DOTS * u32::from(VBLANK_FIRST_LINE - 5)) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF44), VBLANK_FIRST_LINE);
    }

    #[test]
    fn c01_15_vblank_if_bit_raised_once_per_frame() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // IF bit 0 (vblank) is set at power-up (note 08: IF = $E1).
        assert_eq!(dmg.bus.io[0x0F] & 0x01, 0x01);

        // Clear the request (as a ROM would by reading IF), then run exactly one frame and
        // count rising edges of the vblank bit: it must be raised exactly once per frame.
        dmg.bus.io[0x0F] &= !0x01;
        let mut raises = 0u32;
        for _ in 0..FRAME_DOTS {
            let before = dmg.bus.io[0x0F] & 0x01;
            dmg.tick();
            if before == 0 && dmg.bus.io[0x0F] & 0x01 != 0 {
                raises += 1; // a clear -> set transition (note 02b)
            }
        }
        assert_eq!(raises, 1); // exactly one vblank request per frame (note 02b)
    }

    #[test]
    fn c01_15_reset_restarts_ly_and_vblank() {
        use crate::video::{LINE_DOTS, VBLANK_FIRST_LINE};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..(LINE_DOTS * u32::from(VBLANK_FIRST_LINE)) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF44), VBLANK_FIRST_LINE); // in VBlank before reset
        dmg.reset();
        assert_eq!(dmg.bus.read(0xFF44), 0x00); // LY restarts at line 0 (note 08)
    }
}
