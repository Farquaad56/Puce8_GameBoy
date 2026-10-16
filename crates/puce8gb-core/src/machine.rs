//! Machine trait and the Dmg (DMG) machine: a pure function of ticks, no wall clock, no I/O
//! (decisions A_01/A_04).

use crate::audio::Audio;
use crate::bus::{Bus, ROM_HEADER_CHECKSUM};
use crate::cpu::Cpu;
use crate::input::Input;
use crate::media::Media;
use crate::savestate::StateError;
use crate::video::Ppu;

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

    /// Framebuffer of 0xAARRGGBB pixels, 160 x 144 (E03_05 spec).
    fn framebuffer(&self) -> &[u32];

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
    pub ppu: Ppu,
    pub audio: Audio,
    pub media: Media,
    /// Current button state (decision A_04).
    pub input: Input,
    /// Dot phase within the current M-cycle; 0..3, wraps to 0 every 4 dots (decision A_01).
    dot_phase: u8,
    /// Previous value of the STAT interrupt line for rising-edge detection (E03_08): IF bit 1
    /// is set only on a 0 -> 1 transition; while the line stays high no new request is raised.
    stat_line_prev: bool,
    /// Frames completed since reset, incremented at each frame boundary (E03_14 spec).
    frame_count: u64,
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
            ppu: Ppu::default(),
            audio: Audio::default(),
            media: Media,
            input: Input::default(),
            dot_phase: 0,
            stat_line_prev: false, // no STAT request pending at power-up (E03_08)
            frame_count: 0,
        };
        dmg.reset();
        Ok(dmg)
    }

    /// Debug tool for trace comparison, task C01_49: force every $FF44 read to `v`, or
    /// restore the normal PPU scanline with None.
    #[doc(hidden)]
    pub fn debug_set_ly_override(&mut self, v: Option<u8>) {
        self.bus.debug_set_ly_override(v);
    }

    /// Value of the STAT interrupt line from the current PPU mode and registers (E03_08):
    /// each source contributes only when its select bit in STAT ($FF41) is set, OR'ed into one
    /// shared line (note 04a "Ligne d'interrupt STAT"). The exact dot of the LYC compare and
    /// the STAT write quirk are UNKNOWN - to confirm: not implemented. While the LCD is off
    /// no VBlank or STAT request exists, so the line stays low (E03_09 spec).
    fn stat_line(&self) -> bool {
        if !self.ppu.lcd_on() {
            return false; // LCD off: no PPU mode and no coincidence can be active (E03_09 spec)
        }
        let stat = self.bus.io[0x41]; // STAT ($FF41): select bits 6..3 as written (E03_08 spec)
        let mode = self.ppu.mode();
        (stat & 0x40 != 0 && self.ppu.ly() == self.bus.io[0x45]) // bit 6: LYC == LY coincidence
            || (stat & 0x20 != 0 && mode == 2) // bit 5: OAM scan
            || (stat & 0x10 != 0 && mode == 1) // bit 4: VBlank
            || (stat & 0x08 != 0 && mode == 0) // bit 3: HBlank
    }

    /// Frames completed since reset, incremented at each frame boundary (E03_14 spec).
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Machine for Dmg {
    fn reset(&mut self) {
        // Header checksum byte $014D (note 07a); the invariant guarantees it is present.
        let checksum = self.bus.rom[ROM_HEADER_CHECKSUM];
        self.cpu = Cpu::reset(checksum);
        self.bus.reset();
        self.audio.apu.reset(self.bus.timer.div());
        self.ppu.reset(); // LY, dot counter and mode restart at the start of line 0 (E03_01)
        self.audio.head = 0;
        self.audio.tail = 0;
        self.dot_phase = 0;
        // The STAT interrupt line is low at power-up: no pending request (E03_08).
        self.stat_line_prev = false;
        self.frame_count = 0;
        // Mirror the PPU's CPU access state for VRAM/OAM at power-up and reset (E03_10);
        // Dmg::new ends by calling this, so both paths are covered.
        self.bus
            .set_video_access(self.ppu.vram_accessible(), self.ppu.oam_accessible());
    }

    fn tick(&mut self) {
        // Chip order CPU -> Timer -> DMA -> PPU -> APU -> Serial (decision A_01).
        // The CPU advances one M-cycle every 4 dots (note 01_timing.md); the other chips
        // are filled in by later tasks.
        self.dot_phase = (self.dot_phase + 1) % 4;
        if self.dot_phase == 0 {
            self.cpu.tick(&mut self.bus);
            // Timer advances one M-cycle per M-cycle, after the CPU so a DIV/TAC write is
            // visible in the same dot (decision A_01). The timer tick returns true when the
            // delayed request of an overflow by increment lands: one M-cycle after the
            // increment that overflows, TIMA takes TMA and IF bit 2 is set (note 01b "Retard
            // d'un M-cycle").
            if self.bus.timer.mcycle() {
                self.bus.io[0x0F] |= 0x04; // IF bit 2: timer interrupt request (note 01b)
            }
            // OAM DMA advances one byte per M-cycle, after the CPU and the timer (decision A_01 order, E03_06 spec).
            self.bus.dma_step();
            // APU frame sequencer: one step per falling edge of DIV bit 4, read after the timer ticked (E04_01 spec, note 05b); no dependency on the PPU, so it lives in the M-cycle block. The returned EV_* events are consumed by later tasks.
            if let Some((addr, v)) = self.bus.apu_write.take() {
                self.audio.apu.write_reg(addr, v);
            }
            self.audio.apu.tick(self.bus.timer.div());
            self.bus.set_apu_status(self.audio.apu.status());
        }
        // PPU advances one dot per Machine::tick() (decision A_01, E03_01). LY is mirrored
        // into the bus so $FF44 reads are side-effect-free; entry into VBlank (LY 143 -> 144)
        // raises the IF vblank bit once per frame (note 02b "vblank", note 01_timing.md).
        let was_vblank = self.ppu.vblank();
        // LCD enable from LCDC ($FF40) bit 7, applied before the PPU advances so a CPU write
        // in this same dot takes effect immediately (decision A_01: CPU first; E03_09 spec).
        self.ppu.set_lcd_enabled(self.bus.io[0x40]);
        let line_changed = self.ppu.tick();
        if line_changed && self.ppu.ly() == 0 {
            self.frame_count += 1; // line 153 -> 0: a frame boundary (E03_14 spec)
        }
        // Mode 2 OAM scan of the current line, performed once per visible line (E03_02): the
        // PPU reads the 40 OAM entries directly from the bus; CPU access blocking arrives with
        // E03_10. The result is identical whether the scan runs at dot 80 or all at once, so
        // it is done here on entry to mode 2 (E03_02 spec).
        if self.ppu.oam_scan_due() {
            self.ppu.scan_oam(&self.bus.oam);
            self.ppu.set_obj_penalty(&self.bus.oam, self.bus.io[0x43]);
        }
        // One dot of pixel fetching per dot (E03_03): the fetcher reads VRAM directly, so it is
        // not affected by the CPU access blocking set below. SCX/SCY are $FF43/$FF42 (note 04a).
        self.ppu
            .fetch_pixels(&self.bus.vram, self.bus.io[0x43], self.bus.io[0x42]);
        // One dot of window fetching per dot (E03_12): when the pixel position has reached WX - 7
        // with the window line condition met, the pixels from that position on come from the
        // window tile map row win_line. WY/WX are $FF4A/$FF4B (note 04a).
        self.ppu
            .fetch_window(&self.bus.vram, self.bus.io[0x4B], self.bus.io[0x4A]);
        self.ppu.render_sprites(&self.bus.oam, &self.bus.vram);
        self.ppu
            .commit_line(self.bus.io[0x47], self.bus.io[0x48], self.bus.io[0x49]); // BGP, OBP0, OBP1 (E03_05 spec)
        self.bus.ly = self.ppu.ly(); // LY ($FF44) mirror (E03_01); the coincidence flag reads it too (E03_08)
        self.bus.ppu_mode = self.ppu.mode(); // PPU mode mirror for the STAT readback (E03_08 spec, decision A_02)
        self.bus
            .set_video_access(self.ppu.vram_accessible(), self.ppu.oam_accessible()); // CPU access gate (E03_10)
        if line_changed && !was_vblank && self.ppu.vblank() {
            self.bus.io[0x0F] |= 0x01; // IF bit 0: vblank request (note 02b)
        }
        // STAT interrupt line, rising-edge detection (E03_08): the request is raised only on a
        // 0 -> 1 transition of the shared line; while it stays high no new request is emitted
        // ("STAT blocking", note 04a "Ligne d'interrupt STAT"). The previous value is kept in
        // stat_line_prev and compared after every step.
        let line_now = self.stat_line();
        if line_now && !self.stat_line_prev {
            self.bus.io[0x0F] |= 0x02; // IF bit 1: STAT request (note 04a)
        }
        self.stat_line_prev = line_now;
    }

    fn run_frame(&mut self) {
        for _ in 0..FRAME_DOTS {
            self.tick();
        }
    }

    fn framebuffer(&self) -> &[u32] {
        self.ppu.framebuffer()
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
        (0x16, 0x3F), // NR21
        (0x17, 0x00), // NR22
        (0x18, 0xFF), // NR23
        (0x19, 0xBF), // NR24
        (0x1A, 0x7F), // NR30
        (0x1B, 0xFF), // NR31
        (0x1C, 0x9F), // NR32
        (0x1D, 0xFF), // NR33
        (0x1E, 0xBF), // NR34
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
    fn e04_01_machine_clocks_the_frame_sequencer_from_div() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00);
        assert_eq!(dmg.audio.apu.step, 0);
        for _ in 0..8191 {
            dmg.tick();
        }
        assert_eq!(dmg.audio.apu.step, 0);
        dmg.tick();
        assert_eq!(dmg.audio.apu.step, 1);
        for _ in 0..57344 {
            dmg.tick();
        }
        assert_eq!(dmg.audio.apu.step, 0);
    }

    #[test]
    fn e04_01_reset_restarts_the_sequencer_at_step_0() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00);
        for _ in 0..8192 {
            dmg.tick();
        }
        assert_eq!(dmg.audio.apu.step, 1);
        dmg.reset();
        assert_eq!(dmg.audio.apu.step, 0);
    }

    #[test]
    fn e03_01_vblank_if_bit_requested_once_per_frame() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Clear the request (as a ROM would by reading IF), then drive the machine through one
        // full frame and count rising edges of the vblank bit: it must be raised exactly once,
        // on the dot where LY becomes 144 (E03_01).
        dmg.bus.io[0x0F] &= !0x01;
        let mut raises = 0u32;
        for _ in 0..FRAME_DOTS {
            let before = dmg.bus.io[0x0F] & 0x01;
            dmg.tick();
            if before == 0 && dmg.bus.io[0x0F] & 0x01 != 0 {
                raises += 1; // a clear -> set transition (note 02b)
            }
        }
        assert_eq!(raises, 1); // exactly one vblank request per frame (E03_01, note 02b)
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

    #[test]
    fn c01_49_ly_override_forces_ff44_reads() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.debug_set_ly_override(Some(0x90)); // debug tool for trace comparison (C01_49)
        assert_eq!(dmg.bus.read(0xFF44), 0x90); // forced before any tick
        assert_eq!(dmg.bus.peek(0xFF44), 0x90); // peek agrees with read
        for _ in 0..100_000 {
            dmg.tick(); // many M-cycles: the PPU advances LY underneath the override
        }
        assert_eq!(dmg.bus.read(0xFF44), 0x90); // still forced after the ticks
        dmg.debug_set_ly_override(None); // removes the override (call compiles)
        dmg.bus.ly = 0x37; // sentinel different from 0x90: normal LY logic applies again
        assert_eq!(dmg.bus.read(0xFF44), 0x37); // not forced anymore (no specific PPU LY asserted)
    }

    /// Count rising edges of the shared STAT line over `n` ticks, starting from a cleared request.
    /// Each observed request is acknowledged by clearing IF bit 1, so while the line stays high no
    /// new request is raised and only fresh edges are counted (note 04a "Ligne d'interrupt STAT").
    fn count_stat_raises(dmg: &mut Dmg, n: u32) -> u32 {
        dmg.bus.io[0x0F] &= !0x02; // clear the pending request (as a write to IF bit 1 would)
        let mut raises = 0u32;
        for _ in 0..n {
            let before = dmg.bus.io[0x0F] & 0x02;
            dmg.tick();
            if before == 0 && dmg.bus.io[0x0F] & 0x02 != 0 {
                raises += 1; // a rising edge of the STAT line (E03_08)
                dmg.bus.io[0x0F] &= !0x02; // acknowledge so the next edge is counted again
            }
        }
        raises
    }

    #[test]
    fn e03_08_lyc_coincidence_raises_if_bit1_once() {
        use crate::video::LINE_DOTS;
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // At power-up LY = 0 and LYC = 0: the coincidence source is active from the first dot.
        // Enable only bit 6 (LYC == LY) so no other source can contribute (E03_08 spec).
        dmg.bus.write(0xFF41, 0x40);
        let raises = count_stat_raises(&mut dmg, LINE_DOTS * 2); // two lines: coincidence active only on line 0
                                                                 // One rising edge at power-up; the line stays high through line 0 (LY == LYC == 0),
                                                                 // then falls when LY -> 1 and never re-rises over these two lines.
        assert_eq!(raises, 1); // a single request for the sustained-high coincidence window (note 04a "LYC")
    }

    #[test]
    fn e03_08_each_source_raises_if_bit1_once() {
        use crate::video::LINE_DOTS;
        // Each of the four select bits, alone, produces exactly one rising edge over a window that
        // captures its single high period (E03_08 spec: "each of the four sources raises IF bit 1 once").
        let cases = [
            (0x40u8, LINE_DOTS * 2), // coincidence: high all of line 0 (LY == LYC == 0), falls on line 1
            (0x20, LINE_DOTS - 1), // mode 2: power-up OAM window; stop before the wrap re-enters mode 2
            (0x10, FRAME_DOTS),    // VBlank: one rise entering vblank per frame
            (0x08, LINE_DOTS),     // HBlank: one rise on line 0's HBlank
        ];
        for (stat, n) in cases {
            let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
            dmg.bus.write(0xFF41, stat); // enable only this source
            assert_eq!(
                count_stat_raises(&mut dmg, n),
                1,
                "source {stat:02X} must raise once"
            );
        }
    }

    #[test]
    fn e03_08_two_overlapping_sources_raise_if_bit1_once() {
        use crate::video::{LINE_DOTS, MODE2_DOTS};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Enable coincidence (bit 6) and mode 2 (bit 5): both are high on line 0's OAM scan, so the
        // shared line is one continuous window from tick 1 through line 1's OAM scan ("STAT blocking",
        // note 04a). It falls only when line 1 leaves mode 3 and coincidence is false (LY = 1 != LYC = 0).
        dmg.bus.write(0xFF41, 0x60);
        let raises = count_stat_raises(&mut dmg, LINE_DOTS + MODE2_DOTS - 1); // stop before the fall at line 1 dot 80
        assert_eq!(raises, 1); // two overlapping sources merge into a single request (E03_08 spec)
    }

    #[test]
    fn e03_08_new_request_after_the_line_fell() {
        use crate::video::{LINE_DOTS, MODE2_DOTS};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Enable only mode 2 (OAM scan): the line is high for dots 0..79 of each visible line.
        dmg.bus.write(0xFF41, 0x20);
        let raises = count_stat_raises(&mut dmg, LINE_DOTS * 3); // 1 edge at the STAT write (power-up is in mode 2) + 1 per line wrap at ticks 456, 912, 1368
        assert_eq!(raises, 4); // a new request after each time the line fell (E03_08 spec)
        dmg.bus.io[0x0F] &= !0x02;
        for _ in 0..(LINE_DOTS - MODE2_DOTS) {
            dmg.tick(); // stay within mode 3/0 of this line: no rising edge, bit stays clear
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x02, 0x00); // still blocked while the line is low...
        for _ in 0..(LINE_DOTS - MODE2_DOTS) {
            dmg.tick(); // ...until the next OAM scan starts: one new request (E03_08 spec)
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x02, 0x02);
    }

    #[test]
    fn e03_09_video_registers_power_up_values() {
        let dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        assert_eq!(dmg.bus.read(0xFF40), 0x91); // LCDC (task spec)
        assert_eq!(dmg.bus.read(0xFF42), 0x00); // SCY (task spec)
        assert_eq!(dmg.bus.read(0xFF43), 0x00); // SCX (task spec)
        assert_eq!(dmg.bus.read(0xFF47), 0xFC); // BGP (task spec)
        assert_eq!(dmg.bus.read(0xFF48), 0xFF); // OBP0 (specs value; CONFLIT with pandocs, note 08)
        assert_eq!(dmg.bus.read(0xFF49), 0xFF); // OBP1 (specs value; CONFLIT with pandocs, note 08)
        assert_eq!(dmg.bus.read(0xFF4A), 0x00); // WY (task spec)
        assert_eq!(dmg.bus.read(0xFF4B), 0x00); // WX (task spec)
    }

    #[test]
    fn e03_09_video_registers_read_write_round_trip() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // The plain video registers (task spec): each is read/write with no side effect in this
        // task, so a written byte reads back unchanged. LCDC keeps bit 7 set to leave the LCD on.
        let cases: [(u16, u8); 8] = [
            (0xFF40, 0x91), // LCDC
            (0xFF42, 0x5A), // SCY
            (0xFF43, 0xA5), // SCX
            (0xFF47, 0x3C), // BGP
            (0xFF48, 0x78), // OBP0
            (0xFF49, 0xC6), // OBP1
            (0xFF4A, 0x92), // WY
            (0xFF4B, 0x4D), // WX
        ];
        for (addr, val) in cases {
            dmg.bus.write(addr, val);
            assert_eq!(
                dmg.bus.read(addr),
                val,
                "register at {addr:04X} round-trips"
            );
        }
    }

    #[test]
    fn e03_09_lcd_off_resets_ly_and_mode() {
        use crate::video::LINE_DOTS;
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Advance to line 5 (inside a visible line) so LY and the mode are non-zero.
        for _ in 0..(LINE_DOTS * 5 + 100) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF44), 0x05); // LY = 5 before turning the LCD off
        dmg.bus.write(0xFF40, 0x11); // LCDC bit 7 -> 0: LCD and PPU disabled (E03_09 spec)
        dmg.tick(); // the transition is applied in Machine::tick (decision A_01: CPU first)
        assert_eq!(dmg.bus.read(0xFF44), 0x00); // LY reset to line 0 (E03_09 spec)
        let stat = dmg.bus.read(0xFF41);
        assert_eq!(stat & 0x03, 0x00); // STAT mode bits read 0 while the PPU is disabled (note 04a "STAT")
    }

    #[test]
    fn e03_09_no_interrupt_while_lcd_off() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Turn the LCD off first so the PPU is frozen at line 0 (not in VBlank).
        dmg.bus.write(0xFF40, 0x11);
        dmg.tick();
        dmg.bus.io[0x0F] &= !0x03; // clear any pending vblank (bit 0) and STAT (bit 1) requests
                                   // Enable all four STAT sources: the shared line is low while off anyway.
        dmg.bus.write(0xFF41, 0x78);
        let raises = count_stat_raises(&mut dmg, FRAME_DOTS); // a whole frame with the LCD off
        assert_eq!(raises, 0); // no STAT request while the PPU is disabled (E03_09 spec)
        assert_eq!(dmg.bus.io[0x0F] & 0x01, 0x00); // and no VBlank request: the frozen line never enters VBlank
    }

    #[test]
    fn e03_09_lcd_on_restarts_at_line_zero() {
        use crate::video::LINE_DOTS;
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Advance to line 5 so the PPU is mid-frame.
        for _ in 0..(LINE_DOTS * 5 + 100) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF44), 0x05); // LY = 5 before the off/on cycle
        dmg.bus.write(0xFF40, 0x11); // LCD off (E03_09 spec)
        dmg.tick();
        dmg.bus.write(0xFF40, 0x91); // LCDC bit 7 -> 1: the PPU restarts at line 0, dot 0 (E03_09 spec)
                                     // What is displayed on that first line is UNKNOWN - to confirm (note 04b).
        dmg.tick(); // the transition is applied in Machine::tick
        assert_eq!(dmg.bus.read(0xFF44), 0x00); // restarted at line 0
        let stat = dmg.bus.read(0xFF41);
        assert_eq!(stat & 0x03, 0x02); // normal start-of-line mode: OAM scan (mode 2) (E03_09 spec)
    }

    #[test]
    fn e03_02_scan_selects_the_sprites_of_each_line_during_mode_2() {
        use crate::video::{LINE_DOTS, MODE2_DOTS};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // One sprite at Y = 16 in OAM entry 5: shown on lines 0..7 for 8x8 (E03_02 spec).
        dmg.bus.oam[5 * 4] = 16;
        dmg.bus.oam[5 * 4 + 1] = 40; // X = 40: visible on screen
                                     // Advance to an absolute position per line (E03_02 spec): the list is correct at dot 80, right after the OAM scan ends.
        let mut done = 0u32;
        for ly in 0..16u8 {
            let target = LINE_DOTS * u32::from(ly) + MODE2_DOTS; // start of mode 3 of this line
            for _ in 0..(target - done) {
                dmg.tick();
            }
            done = target;
            assert_eq!(dmg.ppu.ly(), ly);
            let sprites = dmg.ppu.line_sprites();
            if ly < 8 {
                assert_eq!(sprites, &[5], "line {ly} selects OAM entry 5"); // Y=16 (8x8): lines 0..7 only
            } else {
                assert!(sprites.is_empty(), "line {ly} selects no sprite");
            }
        }
    }

    #[test]
    fn e03_02_scan_reads_oam_directly_from_the_bus() {
        use crate::video::{LINE_DOTS, MODE2_DOTS};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // The scan reads OAM directly from the bus (E03_02 spec): a value written after the
        // machine starts is picked up by the next line's mode 2 scan. Advance to absolute positions.
        let done = LINE_DOTS * 2 + MODE2_DOTS;
        for _ in 0..done {
            dmg.tick(); // advance past line 1: its scan saw an empty OAM
        }
        assert!(dmg.ppu.line_sprites().is_empty()); // no sprite selected yet
                                                    // Write a sprite at Y = 32 (shown on lines 16..23 for 8x8) after the fact.
        dmg.bus.oam[0] = 32;
        dmg.bus.oam[1] = 40;
        let target = LINE_DOTS * 16 + MODE2_DOTS; // line 16, dot 80: its mode 2 scan has just run
        for _ in 0..(target - done) {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.line_sprites(), &[0]); // the new OAM value is selected (E03_02 spec)
    }

    #[test]
    fn e03_10_vram_and_oam_blocked_in_mode_3() {
        use crate::video::{LINE_DOTS, MODE2_DOTS, MODE3_LEN};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Line 5, dot in mode 3 (pixels): 80 <= dot < 80+172.
        for _ in 0..(LINE_DOTS * 5 + MODE2_DOTS + MODE3_LEN / 2) {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 3); // sanity: we are in mode 3 (E03_10 spec)
                                       // VRAM blocked: reads $FF, writes ignored.
        dmg.bus.write(0x8000, 0xA5);
        assert_eq!(dmg.bus.read(0x8000), 0xFF); // blocked read (E03_10 spec)
        assert_eq!(dmg.bus.vram[0], 0x00); // write ignored: underlying array unchanged (E03_10 spec)
                                           // OAM blocked too (mode >= 2): reads $FF, writes ignored.
        dmg.bus.write(0xFE00, 0x90);
        assert_eq!(dmg.bus.read(0xFE00), 0xFF); // blocked read (E03_10 spec)
        assert_eq!(dmg.bus.oam[0], 0x00); // write ignored: underlying array unchanged (E03_10 spec)
    }

    #[test]
    fn e03_10_oam_blocked_vram_free_in_mode_2() {
        use crate::video::{LINE_DOTS, MODE2_DOTS};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Line 5, dot in mode 2 (OAM scan): dot < 80.
        for _ in 0..(LINE_DOTS * 5 + MODE2_DOTS / 2) {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 2); // sanity: we are in mode 2 (E03_10 spec)
                                       // VRAM still accessible in mode 2.
        dmg.bus.write(0x8000, 0xA5);
        assert_eq!(dmg.bus.read(0x8000), 0xA5); // free access (E03_10 spec)
                                                // OAM blocked: reads $FF, writes ignored.
        dmg.bus.write(0xFE00, 0x90);
        assert_eq!(dmg.bus.read(0xFE00), 0xFF); // blocked read (E03_10 spec)
        assert_eq!(dmg.bus.oam[0], 0x00); // write ignored: underlying array unchanged (E03_10 spec)
    }

    #[test]
    fn e03_10_vram_and_oam_free_in_modes_0_and_1() {
        use crate::video::{LINE_DOTS, MODE2_DOTS, MODE3_LEN};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Mode 0 (HBlank): line 5, dot >= 80+172.
        let target0 = LINE_DOTS * 5 + MODE2_DOTS + MODE3_LEN;
        for _ in 0..target0 {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 0); // sanity: mode 0 (E03_10 spec)
        dmg.bus.write(0x8000, 0xA5);
        assert_eq!(dmg.bus.read(0x8000), 0xA5); // VRAM free (E03_10 spec)
        dmg.bus.write(0xFE00, 0x90);
        assert_eq!(dmg.bus.read(0xFE00), 0x90); // OAM free (E03_10 spec)
                                                // Mode 1 (VBlank): line 150.
        let target1 = LINE_DOTS * 150 + MODE2_DOTS;
        for _ in 0..(target1 - target0) {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 1); // sanity: mode 1 (E03_10 spec)
        dmg.bus.write(0x8000, 0x3C);
        assert_eq!(dmg.bus.read(0x8000), 0x3C); // VRAM free (E03_10 spec)
        dmg.bus.write(0xFE00, 0x88);
        assert_eq!(dmg.bus.read(0xFE00), 0x88); // OAM free (E03_10 spec)
    }

    #[test]
    fn e03_10_vram_and_oam_free_when_lcd_off() {
        use crate::video::{LINE_DOTS, MODE2_DOTS, MODE3_LEN};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // First get into mode 3 (pixels) with the LCD on.
        for _ in 0..(LINE_DOTS * 5 + MODE2_DOTS + MODE3_LEN / 2) {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 3); // sanity: mode 3, LCD on (E03_10 spec)
        assert_eq!(dmg.bus.read(0x8000), 0xFF); // blocked while in mode 3 (E03_10 spec)
                                                // Turn the LCD off via LCDC ($FF40): bit 7 -> 0. The PPU mode reads as 0, so access is free.
        dmg.bus.write(0xFF40, 0x11); // clear bit 7 (E03_09 spec)
        dmg.tick(); // apply the LCDC change: LCD off, mode reads 0 (E03_09 spec)
        assert_eq!(dmg.ppu.mode(), 0); // LCD off: mode reads 0 (note 04a "STAT")
        dmg.bus.write(0x8000, 0xA5);
        assert_eq!(dmg.bus.read(0x8000), 0xA5); // VRAM free when the LCD is off (E03_10 spec)
        dmg.bus.write(0xFE00, 0x90);
        assert_eq!(dmg.bus.read(0xFE00), 0x90); // OAM free when the LCD is off (E03_10 spec)
    }

    #[test]
    fn e03_14_frame_boundary_after_exactly_70224_ticks() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..70223 {
            dmg.tick();
        }
        assert_eq!(dmg.frame_count(), 0); // no frame boundary yet (E03_14 spec)
        assert_eq!(dmg.ppu.ly(), 153); // last line of the frame
                                       // The next dot wraps LY to 0: exactly one frame has completed.
        dmg.tick();
        assert_eq!(dmg.frame_count(), 1);
        assert_eq!(dmg.ppu.ly(), 0);
    }

    #[test]
    fn e03_14_frame_count_increments_once_per_run_frame() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.run_frame();
        assert_eq!(dmg.frame_count(), 1); // one frame boundary per run_frame (E03_14 spec)
        dmg.run_frame();
        assert_eq!(dmg.frame_count(), 2);
        dmg.run_frame();
        assert_eq!(dmg.frame_count(), 3);
    }

    #[test]
    fn e03_14_two_machines_give_identical_framebuffers() {
        let mut a = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        let mut b = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        a.bus.vram[0] = 0xFF; // identical starting state on both machines (E03_14 spec)
        a.bus.vram[1] = 0xFF;
        b.bus.vram[0] = 0xFF;
        b.bus.vram[1] = 0xFF;
        for _ in 0..3 {
            a.run_frame();
            b.run_frame();
        }
        assert_eq!(a.framebuffer(), b.framebuffer()); // deterministic: same image (E03_14 spec)
        assert_eq!(a.frame_count(), 3);
    }

    #[test]
    fn e03_14_machine_draws_a_tile_row() {
        use crate::video::PALETTE;
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Tile 0 row 0 is colour 3 (both bit planes all set); its other rows are 0.
        dmg.bus.vram[0] = 0xFF;
        dmg.bus.vram[1] = 0xFF;
        dmg.run_frame();
        let fb = dmg.framebuffer();
        // BGP post-boot 0xFC: index 3 -> shade 3, index 0 -> shade 0 (E03_14 spec).
        assert!(
            fb[0..160].iter().all(|&px| px == PALETTE[3]),
            "row 0 must be all shade 3"
        );
        assert!(
            fb[160..320].iter().all(|&px| px == PALETTE[0]),
            "row 1 must be all shade 0"
        );
        // The tile map repeats tile 0 every 8 lines, so row 8 shows the same tile row 0.
        assert!(
            fb[8 * 160..9 * 160].iter().all(|&px| px == PALETTE[3]),
            "row 8 must be all shade 3"
        );
    }

    #[test]
    fn e03_06_dma_runs_one_byte_per_m_cycle_and_ends_after_640_dots() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads"); // all-NOP ROM (task C01_35)
        for i in 0..160u16 {
            dmg.bus.write(0xC100 + i, (i as u8) ^ 0x5A); // source pattern in WRAM at $C100-$C19F
        }
        dmg.bus.write(0xFF46, 0xC1); // start the transfer from $C100 (E03_06 spec)
        for _ in 0..4 {
            dmg.tick(); // one M-cycle every 4 dots: after 4 dots exactly one byte is copied
        }
        assert_eq!(dmg.bus.oam[0], 0x5A); // first OAM byte copied on the first M-cycle
        assert_eq!(dmg.bus.oam[1], 0); // second byte not copied yet: one byte per M-cycle (E03_06 spec)
        assert!(dmg.bus.dma_active()); // transfer still running after 4 dots

        for _ in 0..632 {
            dmg.tick(); // 636 dots total = 159 M-cycles: the transfer is still running
        }
        assert!(dmg.bus.dma_active()); // 159 of 160 bytes copied: one step left

        for _ in 0..4 {
            dmg.tick(); // 640 dots total = 160 M-cycles: the final byte is copied
        }
        assert!(!dmg.bus.dma_active()); // transfer complete after exactly 160 M-cycles (E03_06 spec)
        for i in 0..160usize {
            assert_eq!(dmg.bus.oam[i], (i as u8) ^ 0x5A); // all 160 OAM bytes copied from WRAM
        }
    }

    #[test]
    fn e03_15_program_in_hram_polls_until_the_dma_ends() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // A program stored in HRAM: start the OAM DMA from $C100, poll until it ends,
        // then mark an HRAM byte and loop forever (E03_15 spec).
        let prog = [
            0x3E, 0xC1, // LD A,$C1
            0xE0, 0x46, // LDH ($FF46),A: start the OAM DMA from $C100 (E03_06 spec)
            0x3E, 0x28, // LD A,$28
            0x3D, // DEC A
            0x20, 0xFD, // JR NZ,-3: poll until the counter reaches zero
            0x3E, 0x5A, // LD A,$5A
            0xE0, 0x90, // LDH ($FF90),A: write HRAM while the DMA may still run
            0x18, 0xFE, // JR -2: loop forever in HRAM
        ];
        for (i, b) in prog.iter().enumerate() {
            dmg.bus.write(0xFF80 + i as u16, *b);
        }
        dmg.cpu.pc = 0xFF80;
        for _ in 0..1000 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.hram[0x10], 0x5A); // $FF90 was written by the program (E03_15 spec)
        assert!(!dmg.bus.dma_active()); // the transfer ended within 1000 M-cycles
    }

    /// Write an APU register through the bus, then let one M-cycle pass so the machine relays it.
    fn apu_write(dmg: &mut Dmg, addr: u16, v: u8) {
        dmg.bus.write(addr, v);
        for _ in 0..4 {
            dmg.tick();
        }
    }

    #[test]
    fn e04_02_machine_relays_register_writes_to_channel_2() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF17, 0xF0);
        apu_write(&mut dmg, 0xFF18, 0xFF);
        apu_write(&mut dmg, 0xFF19, 0x87);
        assert!(dmg.audio.apu.pulse2.enabled);
        assert_eq!(dmg.audio.apu.pulse2.freq, 2047);
        assert_eq!(dmg.bus.read(0xFF26) & 0x03, 0x03);
    }

    #[test]
    fn e04_02_channel_1_stays_on_after_boot_and_nr52_reads_f1() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..65536 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF26), 0xF1);
    }

    #[test]
    fn e04_02_length_expiry_clears_the_nr52_status_bit() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00);
        apu_write(&mut dmg, 0xFF16, 0x3F);
        apu_write(&mut dmg, 0xFF17, 0xF0);
        apu_write(&mut dmg, 0xFF18, 0x00);
        apu_write(&mut dmg, 0xFF19, 0xC0);
        assert_eq!(dmg.bus.read(0xFF26) & 0x03, 0x03);
        for _ in 0..8192 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF26) & 0x03, 0x01);
    }

    #[test]
    fn e04_02_power_off_clears_channels_and_ignores_register_writes() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF26, 0x00);
        assert_eq!(dmg.bus.read(0xFF26), 0x70);
        apu_write(&mut dmg, 0xFF17, 0xF0);
        assert!(!dmg.audio.apu.pulse2.dac_on);
    }

    #[test]
    fn e04_02_reset_stops_channel_2() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF17, 0xF0);
        apu_write(&mut dmg, 0xFF19, 0x80);
        assert!(dmg.audio.apu.pulse2.enabled);
        dmg.reset();
        assert!(!dmg.audio.apu.pulse2.enabled);
    }
}
