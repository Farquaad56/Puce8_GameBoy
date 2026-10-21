//! Machine trait and the Dmg (DMG) machine: a pure function of ticks, no wall clock, no I/O
//! (decisions A_01/A_04).

use crate::audio::Audio;
use crate::bus::{Bus, ROM_HEADER_CHECKSUM};
use crate::cpu::Cpu;
use crate::input::Input;
use crate::media::mapper::cartridge_has_battery;
use crate::media::{BatteryError, Media};
use crate::savestate::StateError;
use crate::trace::Trace;
use crate::video::{Ppu, VBLANK_FIRST_LINE};

/// Dots per frame: 70224 (note 01_timing.md, decision A_01).
pub const FRAME_DOTS: u32 = 70224;

/// Framebuffer size in pixels: 160 x 144 of 2-bit indices (decision A_04).
pub const FRAMEBUFFER_SIZE: usize = 23040;

/// Audio ring capacity in interleaved samples: 4096 stereo pairs at 32768 Hz, ~0.125 s
/// (decision A_04).
pub const AUDIO_RING_SAMPLES: usize = 2 * 4096;
#[cfg(test)]
mod determinism;
mod state;

/// Error returned when a ROM cannot be loaded (bad ROM => Result, never panic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    /// The ROM is empty or shorter than the cartridge header ($0100-$014F, note 07a).
    TooShort,
    /// The cartridge has no battery-backed RAM to load into (E05_06).
    NoBatteryRam,
    /// The battery RAM data length differs from the RAM size (E05_06).
    BatteryRamSizeMismatch { expected: usize, actual: usize },
    /// The boot ROM image does not have the expected length (E14_03).
    BadBootRom,
}

/// Console model emulated by a machine (decision A_11, E14_04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Model {
    #[default]
    Dmg,
    Cgb,
    Sgb,
}

impl Model {
    /// Byte stored in the save-state extension block: 0 Dmg, 1 Cgb, 2 Sgb (E14_04).
    pub fn to_byte(self) -> u8 {
        match self {
            Model::Dmg => 0,
            Model::Cgb => 1,
            Model::Sgb => 2,
        }
    }

    /// Inverse of `to_byte`; None for any other value (E14_04).
    pub fn from_byte(byte: u8) -> Option<Model> {
        match byte {
            0 => Some(Model::Dmg),
            1 => Some(Model::Cgb),
            2 => Some(Model::Sgb),
            _ => None,
        }
    }
}

impl Model {
    /// Expected boot ROM length in bytes (E15_04): Dmg 256, Sgb 256, Cgb 2304.
    /// The Cgb length is UNKNOWN - to confirm (note 10 says 2048, the CLI/desktop code uses 2304).
    pub fn boot_rom_len(self) -> usize {
        match self {
            Model::Dmg | Model::Sgb => Dmg::DMG_BOOT_ROM_LEN,
            Model::Cgb => 2304,
        }
    }
}

/// Options used to build a machine (E15_04).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineConfig {
    /// Console model to emulate.
    pub model: Model,
    /// Optional boot ROM image; its length must equal `Model::boot_rom_len`.
    pub boot_rom: Option<Vec<u8>>,
}

/// Picks the model from the cartridge header (E15_04): Cgb when bit 7 of byte $0143 is set,
/// otherwise Dmg. Never returns Sgb. A ROM shorter than $0144 bytes gives Dmg.
pub fn auto_model(rom: &[u8]) -> Model {
    match rom.get(0x0143) {
        Some(flag) if flag & 0x80 != 0 => Model::Cgb,
        _ => Model::Dmg,
    }
}

/// CGB post-boot CPU registers when no boot ROM runs (E15_05, note 10 "Registres CPU au
/// hand-off : CGB et AGB", Power_Up_Sequence.md CPU registers). PC and SP keep the common
/// post-boot values already set by `Cpu::reset`. The ROM is at least `Dmg::MIN_ROM_LEN` long.
fn apply_cgb_cpu_regs(cpu: &mut Cpu, rom: &[u8]) {
    cpu.a = 0x11; // A=$11 identifies CGB hardware
    cpu.f = 0x80; // Z=1 N=0 H=0 C=0
    cpu.c = 0x00;
    if rom[0x0143] & 0x80 != 0 {
        // CGB mode: B=$00 D=$FF E=$56 H=$00 L=$0D
        cpu.b = 0x00;
        cpu.d = 0xFF;
        cpu.e = 0x56;
        cpu.h = 0x00;
        cpu.l = 0x0D;
    } else {
        // DMG compatibility mode: D=$00 E=$08; B is the title sum for a Nintendo licensee, else $00;
        // HL is $991A when B is $43 or $58 (CGB), else $007C.
        let old = rom[0x014B];
        let nintendo = old == 0x01 || (old == 0x33 && rom[0x0144] == 0x30 && rom[0x0145] == 0x31);
        let b = if nintendo {
            rom[0x0134..=0x0143]
                .iter()
                .fold(0u8, |sum, &byte| sum.wrapping_add(byte))
        } else {
            0x00
        };
        cpu.b = b;
        cpu.d = 0x00;
        cpu.e = 0x08;
        if b == 0x43 || b == 0x58 {
            cpu.h = 0x99;
            cpu.l = 0x1A;
        } else {
            cpu.h = 0x00;
            cpu.l = 0x7C;
        }
    }
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
    /// Instruction trace ring (E08_04); empty and inactive until enabled.
    pub(crate) trace: Trace,
    /// Console model (E14_04, E15_04).
    model: Model,
    /// Copy of the installed boot ROM image, kept so a loaded state can rebuild it (E14_04).
    boot_image: Option<Vec<u8>>,
}

impl Dmg {
    /// Minimum ROM length: the header occupies $0100-$014F of bank 0 (note 07a).
    pub const MIN_ROM_LEN: usize = 0x150;

    /// Length of a monochrome boot ROM image in bytes (note 10).
    pub const DMG_BOOT_ROM_LEN: usize = 256;

    /// Sets the four output colors of shades 0..3 (E11_01); not part of the save state.
    pub fn set_dmg_palette(&mut self, colors: [u32; 4]) {
        self.ppu.set_dmg_palette(colors);
    }

    /// Current output colors of shades 0..3 (E11_01).
    pub fn dmg_palette(&self) -> [u32; 4] {
        self.ppu.dmg_palette()
    }

    /// Battery-backed cartridge RAM for the frontend to save (E05_06): the persistent bytes of
    /// the mapper's RAM. None when the cartridge type has no battery or no RAM is present. File
    /// access stays in the frontends.
    pub fn battery_ram(&self) -> Option<Vec<u8>> {
        if cartridge_has_battery(self.bus.peek(0x0147)) {
            self.bus.mapper.battery_payload()
        } else {
            None
        }
    }

    /// Load battery-backed RAM saved earlier (E05_06). Fails with `NoBatteryRam` when the
    /// cartridge has none and with `BatteryRamSizeMismatch` when the length differs; the RAM is
    /// left untouched on error.
    pub fn load_battery_ram(&mut self, data: &[u8]) -> Result<(), LoadError> {
        if !cartridge_has_battery(self.bus.peek(0x0147))
            || self.bus.mapper.battery_payload().is_none()
        {
            return Err(LoadError::NoBatteryRam);
        }
        match self.bus.mapper.load_battery_payload(data) {
            Ok(()) => Ok(()),
            Err(BatteryError::NoBatteryData) => Err(LoadError::NoBatteryRam),
            Err(BatteryError::SizeMismatch { expected, actual }) => {
                Err(LoadError::BatteryRamSizeMismatch { expected, actual })
            }
        }
    }

    /// Load a ROM image and build a Dmg machine in its post-boot state.
    pub fn new(rom: &[u8]) -> Result<Dmg, LoadError> {
        Dmg::with_config(rom, MachineConfig::default())
    }

    /// Builds a machine for `cfg.model` (E15_04). With a boot ROM the machine starts from the
    /// power-on state at PC=$0000. A Cgb machine still behaves like a Dmg for now.
    pub fn with_config(rom: &[u8], cfg: MachineConfig) -> Result<Dmg, LoadError> {
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
            trace: Trace::default(),
            model: cfg.model,
            boot_image: None,
        };
        dmg.bus.cgb = cfg.model == Model::Cgb; // CGB post-boot I/O values (E15_05)
                                               // DMG compatibility mode (E15_13): a Cgb machine running a cartridge whose $0143 bit 7 is clear.
        dmg.bus.compat = cfg.model == Model::Cgb && rom[0x0143] & 0x80 == 0;
        if let Some(boot) = cfg.boot_rom {
            dmg.bus
                .set_boot_rom(&boot, cfg.model.boot_rom_len())
                .map_err(|_| LoadError::BadBootRom)?;
            dmg.boot_image = Some(boot);
        }
        dmg.reset(); // with a boot ROM installed this restarts from the power-on state
        Ok(dmg)
    }

    /// Builds a machine that starts from the power-on state at PC=$0000 with the boot ROM
    /// mapped (E14_03). The boot ROM must be exactly `DMG_BOOT_ROM_LEN` bytes.
    pub fn with_boot_rom(rom: &[u8], boot: &[u8]) -> Result<Dmg, LoadError> {
        let cfg = MachineConfig {
            model: Model::Dmg,
            boot_rom: Some(boot.to_vec()),
        };
        Dmg::with_config(rom, cfg)
    }

    /// Power-on state used when a boot ROM is installed (E14_03, note 10): CPU registers, SP and
    /// PC at 0, every I/O register 0 (UNKNOWN values are 0), LCD off, APU powered off, boot ROM
    /// mapped.
    fn apply_power_on(&mut self) {
        self.cpu = Cpu::default();
        self.bus.power_on();
        self.bus.apu_write = Some((0xFF26, 0x00)); // the APU model starts powered off like NR52
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
    /// only the retained LY==LYC coincidence flag can keep the line high when its select bit
    /// is set (mooneye stat_lyc_onoff, FIX_08).
    fn stat_line(&self) -> bool {
        if !self.ppu.lcd_on() {
            // LCD off: only the retained coincidence flag keeps the line high, so switching the LCD back on does not create a false rising edge (mooneye stat_lyc_onoff, FIX_08).
            return self.bus.io[0x41] & 0x40 != 0 && self.bus.lyc_flag();
        }
        let stat = self.bus.io[0x41]; // STAT ($FF41): select bits 6..3 as written (E03_08 spec)
        (stat & 0x40 != 0 && self.ppu.ly() == self.bus.io[0x45]) // bit 6: LYC == LY coincidence
            || (stat & 0x20 != 0 && self.ppu.stat_mode() == 2) // mode 2 irq follows the 4-dot lagged mode (mooneye intr_2_*, FIX_09)
            || (stat & 0x20 != 0 && self.ppu.ly() == VBLANK_FIRST_LINE && self.ppu.line_dot() < 4) // bit 5 also fires at the very start of line 144, together with vblank (mooneye vblank_stat_intr-GS, FIX_07); how long the line stays high: UNKNOWN - to confirm, 4 dots used
            || (stat & 0x10 != 0 && self.ppu.stat_mode() == 1) // bit 4: VBlank
            || (stat & 0x08 != 0 && self.ppu.stat_mode() == 0) // bits 3/4/5 irqs all follow the 4-dot lagged mode (mooneye intr_1_2 / intr_2_0, FIX_09)
    }

    /// True when this is a Cgb machine in DMG compatibility mode (E15_13): the cartridge has
    /// $0143 bit 7 clear, so the DMG rendering path and the user's DMG palette are used. This is a
    /// deliberate simplification: the real boot ROM picks colors from a title hash table (decision A_13).
    pub fn compat(&self) -> bool {
        self.bus.compat
    }

    /// Console model of this machine (E14_04).
    pub fn model(&self) -> Model {
        self.model
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
        if self.model == Model::Cgb {
            apply_cgb_cpu_regs(&mut self.cpu, &self.bus.rom); // E15_05
        }
        self.bus.reset();
        self.audio.apu.reset(self.bus.timer.div());
        self.audio.reset_mixer();
        self.ppu.reset(); // LY, dot counter and mode restart at the start of line 0 (E03_01)
        self.audio.head = 0;
        self.audio.tail = 0;
        self.dot_phase = 0;
        // The STAT interrupt line is low at power-up: no pending request (E03_08).
        self.stat_line_prev = false;
        self.frame_count = 0;
        self.trace.clear();
        // Mirror the PPU's CPU access state for VRAM/OAM at power-up and reset (E03_10);
        // Dmg::new ends by calling this, so both paths are covered.
        self.bus
            .set_video_access(self.ppu.vram_accessible(), self.ppu.oam_accessible());
        self.bus
            .set_video_write_access(self.ppu.vram_writable(), self.ppu.oam_writable());
        // CPU write gate (FIX_10)
        if self.bus.has_boot_rom() {
            self.apply_power_on(); // boot ROM installed: restart from the power-on state (E14_03)
        }
    }

    fn tick(&mut self) {
        // Chip order CPU -> Timer -> DMA -> PPU -> APU -> Serial (decision A_01).
        // The CPU advances one M-cycle every 4 dots (note 01_timing.md); the other chips
        // are filled in by later tasks.
        self.dot_phase = (self.dot_phase + 1) % 4;
        if self.dot_phase == 0 {
            // A running HDMA transfer suspends the CPU (E15_14, note 11c); the other chips keep running.
            if !self.bus.hdma_stalled() {
                self.trace_instruction_start();
                self.cpu.tick(&mut self.bus);
            }
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
            if let Some(i) = self.audio.apu.wave.pending_corruption.take() {
                self.bus.corrupt_wave_ram(i); // DMG retrigger corruption (E04_07)
            }
            self.audio.apu.wave.ram = self.bus.wave_ram();
            self.audio.apu.tick(self.bus.timer.div());
            self.bus.set_apu_status(self.audio.apu.status());
            self.bus.wave_access = self.audio.apu.wave.access(); // wave RAM access rule (E04_07)
            let (nr50, nr51) = self.bus.audio_pan();
            self.audio.mix_tick(nr50, nr51);
            self.bus.mapper.tick(4); // cartridge clock: one M-cycle = 4 dots (E05_07)
        }
        // PPU advances one dot per Machine::tick() (decision A_01, E03_01). LY is mirrored
        // into the bus so $FF44 reads are side-effect-free; entry into VBlank (LY 143 -> 144)
        // raises the IF vblank bit once per frame (note 02b "vblank", note 01_timing.md).
        let was_vblank = self.ppu.vblank();
        // LCD enable from LCDC ($FF40) bit 7, applied before the PPU advances so a CPU write
        // in this same dot takes effect immediately (decision A_01: CPU first; E03_09 spec).
        let lcd_was_on = self.ppu.lcd_on();
        let flag_before = self.bus.ly == self.bus.io[0x45]; // LY==LYC just before a possible LCD switch-off (FIX_08)
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
        // Cgb color mode (E15_13): a Cgb machine whose cartridge is not DMG-only renders with the
        // attributes of VRAM bank 1 and the palette RAM; every other machine keeps the DMG path.
        let color_mode = self.bus.cgb && !self.bus.compat;
        if color_mode {
            let vram1 = self.bus.vram1();
            self.ppu.fetch_pixels_cgb(
                &self.bus.vram,
                Some(vram1),
                self.bus.io[0x43],
                self.bus.io[0x42],
            );
        } else {
            self.ppu
                .fetch_pixels(&self.bus.vram, self.bus.io[0x43], self.bus.io[0x42]);
        }
        // One dot of window fetching per dot (E03_12): when the pixel position has reached WX - 7
        // with the window line condition met, the pixels from that position on come from the
        // window tile map row win_line. WY/WX are $FF4A/$FF4B (note 04a).
        if color_mode {
            let vram1 = self.bus.vram1();
            self.ppu.fetch_window_cgb(
                &self.bus.vram,
                Some(vram1),
                self.bus.io[0x4B],
                self.bus.io[0x4A],
            );
            self.ppu.render_sprites_cgb(
                &self.bus.oam,
                &self.bus.vram,
                Some(vram1),
                self.bus.opri_dmg_style(),
            );
            self.ppu
                .commit_line_cgb(&self.bus.bg_pal, &self.bus.obj_pal); // BG and OBJ palette RAM (E15_08, E15_10)
        } else {
            self.ppu
                .fetch_window(&self.bus.vram, self.bus.io[0x4B], self.bus.io[0x4A]);
            self.ppu.render_sprites(&self.bus.oam, &self.bus.vram);
            self.ppu
                .commit_line(self.bus.io[0x47], self.bus.io[0x48], self.bus.io[0x49]);
            // BGP, OBP0, OBP1 (E03_05 spec)
        }
        self.bus.ly = self.ppu.ly(); // LY ($FF44) mirror (E03_01); the coincidence flag reads it too (E03_08)
        self.bus.ppu_mode = self.ppu.stat_mode(); // PPU mode mirror for the STAT readback (E03_08 spec, decision A_02; FIX_08)
                                                  // LY==LYC flag: frozen while the LCD is off, recomputed live as soon as it is on (mooneye stat_lyc_onoff, FIX_08).
        if self.ppu.lcd_on() {
            self.bus.lyc_frozen = None;
        } else if lcd_was_on {
            self.bus.lyc_frozen = Some(flag_before);
        }
        self.bus.lyc_flag_hold = self.ppu.line_dot() < 4; // LY==LYC flag reads 0 during the first 4 dots of a line (FIX_09)
        self.bus
            .set_video_access(self.ppu.vram_accessible(), self.ppu.oam_accessible()); // CPU access gate (E03_10)
        self.bus
            .set_video_write_access(self.ppu.vram_writable(), self.ppu.oam_writable()); // CPU write gate (FIX_10)
                                                                                        // HBlank HDMA (E15_15, note 11c): one block on each entry into mode 0 of a visible line; mode 1 (lines 144-153) and an LCD that is off never match.
        self.bus
            .hdma_hblank_tick(self.ppu.lcd_on() && self.ppu.mode() == 0);
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
        self.bus.set_buttons(input);
    }

    fn save_state(&self, buf: &mut [u8]) -> Result<(), StateError> {
        // Binary format of decision A_05 (E09_02); the caller provides the buffer.
        self.write_machine_state(&mut crate::savestate::StateWriter::new(buf))
    }

    fn load_state(&mut self, buf: &[u8]) -> Result<(), StateError> {
        // Atomic: on any error the machine is left unchanged (E09_02). Bytes after the state
        // are ignored.
        self.read_machine_state(&mut crate::savestate::StateReader::new(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wave::WaveAccess;

    /// 32 KiB ROM with `flag` at $0143, a jump to $0150 at $0100 and `program` at $0150 (E15_13).
    fn e15_13_rom(flag: u8, program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x0143] = flag;
        rom[0x0100..0x0103].copy_from_slice(&[0xC3, 0x50, 0x01]);
        rom[0x0150..0x0150 + program.len()].copy_from_slice(program);
        rom
    }

    /// Stripe pattern in VRAM tile 0, then a loop copying LY into SCX (a picture that depends on the timing).
    const E15_13_SCROLL_PROGRAM: [u8; 17] = [
        0x21, 0x00, 0x80, // LD HL,$8000
        0x3E, 0xAA, // LD A,$AA
        0x0E, 0x20, // LD C,$20
        0x22, // LD (HL+),A
        0x0D, // DEC C
        0x20, 0xFC, // JR NZ,-4
        0xF0, 0x44, // LDH A,($44)
        0xE0, 0x43, // LDH ($43),A
        0x18, 0xFA, // JR -6
    ];

    /// Waits for LY = $90 (VBlank, palette RAM is accessible), then sets BG palette 0 color 0 to
    /// pure red ($001F) through BCPS/BCPD with auto-increment, then loops forever.
    const E15_13_PALETTE_PROGRAM: [u8; 22] = [
        0xF0, 0x44, // LDH A,($44)
        0xFE, 0x90, // CP $90
        0x20, 0xFA, // JR NZ,-6
        0x3E, 0x80, // LD A,$80
        0xE0, 0x68, // LDH ($68),A   BCPS = $80
        0x3E, 0x1F, // LD A,$1F
        0xE0, 0x69, // LDH ($69),A   low byte
        0x3E, 0x00, // LD A,$00
        0xE0, 0x69, // LDH ($69),A   high byte
        0x18, 0xFE, // JR -2
        0x00, 0x00, // padding
    ];

    /// FNV-1a hash of every framebuffer pixel over `frames` frames (E15_13).
    fn e15_13_hash(dmg: &mut Dmg, frames: usize) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for _ in 0..frames {
            dmg.run_frame();
            for pixel in dmg.framebuffer() {
                for byte in pixel.to_le_bytes() {
                    hash ^= u64::from(byte);
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        hash
    }

    #[test]
    fn e15_15_machine_copies_one_block_per_hblank() {
        let rom = e15_13_rom(0x80, &E15_13_SCROLL_PROGRAM);
        let mut cgb = e15_13_machine(Model::Cgb, &rom);
        for i in 0..32u16 {
            cgb.bus.write(0xC100 + i, 0x40 + i as u8);
        }
        cgb.bus.write(0xFF51, 0xC1);
        cgb.bus.write(0xFF52, 0x00);
        cgb.bus.write(0xFF53, 0x01);
        cgb.bus.write(0xFF54, 0x00);
        cgb.bus.write(0xFF55, 0x81); // 2 blocks: source $C100, destination $8100
        assert_eq!(cgb.bus.read(0xFF55), 0x01);
        cgb.run_frame();
        for i in 0..32u16 {
            assert_eq!(cgb.bus.vram_bank(0, 0x8100 + i), 0x40 + i as u8);
        }
        assert_eq!(cgb.bus.vram_bank(0, 0x8120), 0x00);
        assert_eq!(cgb.bus.read(0xFF55), 0xFF);
    }

    fn e15_13_machine(model: Model, rom: &[u8]) -> Dmg {
        let cfg = MachineConfig {
            model,
            boot_rom: None,
        };
        Dmg::with_config(rom, cfg).expect("valid")
    }

    #[test]
    fn e15_13_cgb_with_a_dmg_cartridge_renders_like_the_dmg() {
        let rom = e15_13_rom(0x00, &E15_13_SCROLL_PROGRAM);
        let mut dmg = e15_13_machine(Model::Dmg, &rom);
        let mut cgb = e15_13_machine(Model::Cgb, &rom);
        assert_eq!(e15_13_hash(&mut dmg, 10), e15_13_hash(&mut cgb, 10));
    }

    #[test]
    fn e15_13_compat_flag_follows_model_and_cartridge() {
        let dmg_cart = e15_13_rom(0x00, &E15_13_SCROLL_PROGRAM);
        let cgb_cart = e15_13_rom(0x80, &E15_13_SCROLL_PROGRAM);
        let only_cgb_cart = e15_13_rom(0xC0, &E15_13_SCROLL_PROGRAM);
        assert!(e15_13_machine(Model::Cgb, &dmg_cart).compat());
        assert!(!e15_13_machine(Model::Cgb, &cgb_cart).compat());
        assert!(!e15_13_machine(Model::Cgb, &only_cgb_cart).compat());
        assert!(!e15_13_machine(Model::Dmg, &dmg_cart).compat());
        assert!(!e15_13_machine(Model::Dmg, &cgb_cart).compat());
    }

    #[test]
    fn e15_13_opri_is_one_and_fixed_in_compat_mode() {
        let mut compat = e15_13_machine(Model::Cgb, &e15_13_rom(0x00, &E15_13_SCROLL_PROGRAM));
        assert_eq!(compat.bus.read(0xFF6C), 0xFF);
        compat.bus.write(0xFF6C, 0x00);
        assert_eq!(compat.bus.read(0xFF6C), 0xFF);
        assert!(compat.bus.opri_dmg_style());
        let mut color = e15_13_machine(Model::Cgb, &e15_13_rom(0x80, &E15_13_SCROLL_PROGRAM));
        assert_eq!(color.bus.read(0xFF6C), 0xFE);
        color.bus.write(0xFF6C, 0x01);
        assert_eq!(color.bus.read(0xFF6C), 0xFF);
        color.bus.write(0xFF6C, 0x00);
        assert_eq!(color.bus.read(0xFF6C), 0xFE);
        assert!(!color.bus.opri_dmg_style());
    }

    #[test]
    fn e15_13_color_mode_renders_through_the_palette_ram() {
        let rom = e15_13_rom(0x80, &E15_13_PALETTE_PROGRAM);
        let mut cgb = e15_13_machine(Model::Cgb, &rom);
        cgb.run_frame();
        cgb.run_frame();
        assert_eq!(cgb.bus.bg_pal[0], 0x1F);
        assert_eq!(cgb.bus.bg_pal[1], 0x00);
        assert_eq!(cgb.framebuffer()[0], 0xFFFF_0000); // BG palette 0 color 0 = red
    }

    #[test]
    fn e15_13_compat_mode_accepts_palette_writes_without_visual_effect() {
        let rom = e15_13_rom(0x00, &E15_13_PALETTE_PROGRAM);
        let mut cgb = e15_13_machine(Model::Cgb, &rom);
        cgb.run_frame();
        cgb.run_frame();
        assert_eq!(cgb.bus.bg_pal[0], 0x1F); // the write was accepted
        assert_eq!(cgb.framebuffer()[0], 0xFFFF_FFFF); // DMG path: BGP color 0 = white (dmg_palette[0])
    }

    /// 32 KiB cartridge image of zeros whose CGB flag byte $0143 is `flag` (E15_05).
    fn e15_05_cart(flag: u8) -> Vec<u8> {
        let mut image = vec![0u8; 32 * 1024];
        image[0x0143] = flag;
        image
    }

    fn e15_05_machine(model: Model, image: &[u8]) -> Dmg {
        let cfg = MachineConfig {
            model,
            boot_rom: None,
        };
        Dmg::with_config(image, cfg).expect("valid")
    }

    #[test]
    fn e15_05_cgb_mode_cpu_registers() {
        let dmg = e15_05_machine(Model::Cgb, &e15_05_cart(0x80));
        let r = dmg.cpu.regs();
        assert_eq!(r.a, 0x11); // note 10 CGB hand-off: A=$11
        assert_eq!(r.f, 0x80); // note 10: Z=1 N=0 H=0 C=0
        assert_eq!(r.b, 0x00); // note 10: B=$00
        assert_eq!(r.c, 0x00); // note 10: C=$00
        assert_eq!(r.d, 0xFF); // note 10: D=$FF
        assert_eq!(r.e, 0x56); // note 10: E=$56
        assert_eq!(r.h, 0x00); // note 10: H=$00
        assert_eq!(r.l, 0x0D); // note 10: L=$0D
        assert_eq!(r.sp, 0xFFFE); // note 10: SP=$FFFE
        assert_eq!(r.pc, 0x0100); // note 10: PC=$0100
    }

    #[test]
    fn e15_05_cgb_dmg_compatibility_mode_registers() {
        // Nintendo old licensee $01: B is the sum of the 16 title bytes ($43 here).
        let mut image = e15_05_cart(0x00);
        image[0x014B] = 0x01;
        image[0x0134] = 0x43;
        let r = e15_05_machine(Model::Cgb, &image).cpu.regs();
        assert_eq!(
            (r.a, r.f, r.b, r.c, r.d, r.e),
            (0x11, 0x80, 0x43, 0x00, 0x00, 0x08)
        ); // note 10 DMG mode
        assert_eq!((r.h, r.l), (0x99, 0x1A)); // note 10: B=$43 gives HL=$991A
                                              // Other title sum: HL=$007C.
        image[0x0134] = 0x10;
        let r = e15_05_machine(Model::Cgb, &image).cpu.regs();
        assert_eq!(r.b, 0x10); // note 10: B is the title sum
        assert_eq!((r.h, r.l), (0x00, 0x7C)); // note 10: otherwise HL=$007C
                                              // Not a Nintendo licensee: B=$00.
        image[0x014B] = 0x00;
        let r = e15_05_machine(Model::Cgb, &image).cpu.regs();
        assert_eq!(r.b, 0x00); // note 10: B=$00 otherwise
        assert_eq!((r.h, r.l), (0x00, 0x7C));
        // New licensee "01" with old licensee $33.
        image[0x014B] = 0x33;
        image[0x0144] = 0x30;
        image[0x0145] = 0x31;
        image[0x0134] = 0x58;
        let r = e15_05_machine(Model::Cgb, &image).cpu.regs();
        assert_eq!(r.b, 0x58); // note 10: new licensee "01" counts as Nintendo
        assert_eq!((r.h, r.l), (0x99, 0x1A)); // note 10: B=$58 gives HL=$991A
    }

    #[test]
    fn e15_05_dmg_post_boot_registers_are_unchanged() {
        // A Dmg machine ignores the CGB flag and keeps the DMG values.
        let r = e15_05_machine(Model::Dmg, &e15_05_cart(0x80)).cpu.regs();
        assert_eq!(
            (r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l),
            (0x01, 0x80, 0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D)
        ); // note 10 DMG hand-off, header checksum $00 keeps H and C clear
        assert_eq!((r.sp, r.pc), (0xFFFE, 0x0100));
        let mut image = e15_05_cart(0x00);
        image[0x014D] = 0x01;
        let r = e15_05_machine(Model::Dmg, &image).cpu.regs();
        assert_eq!(r.f, 0xB0); // note 10: non-zero header checksum sets H and C
    }

    #[test]
    fn e15_05_cgb_io_registers_after_reset() {
        let dmg = e15_05_machine(Model::Cgb, &e15_05_cart(0x80));
        let io = &dmg.bus.io;
        assert_eq!(io[0x02], 0x7F); // SC, note 10 CGB column
        assert_eq!(io[0x46], 0x00); // DMA, note 10 CGB column
        assert_eq!(io[0x4D], 0x7E); // KEY1
        assert_eq!(io[0x4F], 0xFE); // VBK
        assert_eq!(&io[0x51..=0x55], &[0xFF; 5]); // HDMA1-HDMA5
        assert_eq!(io[0x56], 0x3E); // RP
        assert_eq!(io[0x70], 0xF8); // SVBK
        assert_eq!(io[0x40], 0x91); // LCDC unchanged
        assert_eq!(io[0x47], 0xFC); // BGP unchanged
        assert_eq!(io[0x26], 0xF1); // NR52 unchanged
        assert_eq!(io[0xFF], 0x00); // IE unchanged
    }

    #[test]
    fn e15_05_dmg_io_registers_and_cgb_reads_are_unchanged() {
        let dmg = e15_05_machine(Model::Dmg, &e15_05_cart(0x80));
        assert_eq!(dmg.bus.io[0x02], 0x7E); // SC, DMG value
        assert_eq!(dmg.bus.io[0x46], 0xFF); // DMA, DMG value
        assert_eq!(dmg.bus.io[0x4D], 0xFF);
        assert_eq!(dmg.bus.io[0x4F], 0xFF);
        assert_eq!(dmg.bus.io[0x70], 0xFF);
        for addr in [
            0xFF4Du16, 0xFF4F, 0xFF51, 0xFF52, 0xFF53, 0xFF54, 0xFF55, 0xFF68, 0xFF69, 0xFF6A,
            0xFF6B, 0xFF6C, 0xFF70,
        ] {
            assert_eq!(dmg.bus.read(addr), 0xFF, "dmg {addr:#06x}");
            // Cgb reads are tested per register by the e15_06 and later tests, as each register gets implemented.
        }
    }

    #[test]
    fn e15_05_cgb_with_boot_rom_starts_from_power_on() {
        let cfg = MachineConfig {
            model: Model::Cgb,
            boot_rom: Some(vec![0u8; 2304]),
        };
        let dmg = Dmg::with_config(&e15_05_cart(0x80), cfg).expect("valid");
        let r = dmg.cpu.regs();
        assert_eq!((r.a, r.pc), (0x00, 0x0000)); // power-on state, not the CGB hand-off values
        assert_eq!(dmg.bus.io[0x4D], 0x00); // all I/O are 0 at power-on
    }

    #[test]
    fn e15_04_auto_model_from_cgb_flag() {
        for (flag, expected) in [
            (0x00u8, Model::Dmg),
            (0x80, Model::Cgb),
            (0xC0, Model::Cgb),
            (0x42, Model::Dmg),
        ] {
            let mut image = vec![0u8; 0x150];
            image[0x0143] = flag;
            assert_eq!(auto_model(&image), expected, "flag {flag:#04x}");
        }
        assert_eq!(auto_model(&[0xFFu8; 0x100]), Model::Dmg);
        assert_eq!(auto_model(&[0x80u8; 0x143]), Model::Dmg);
        assert_eq!(auto_model(&[]), Model::Dmg);
    }

    #[test]
    fn e15_04_boot_rom_len_per_model() {
        assert_eq!(Model::Dmg.boot_rom_len(), 256);
        assert_eq!(Model::Sgb.boot_rom_len(), 256);
        assert_eq!(Model::Cgb.boot_rom_len(), 2304);
    }

    #[test]
    fn e15_04_with_config_cgb_reports_model() {
        let cfg = MachineConfig {
            model: Model::Cgb,
            boot_rom: None,
        };
        let dmg = Dmg::with_config(&e14_03_cart(), cfg).expect("valid");
        assert_eq!(dmg.model(), Model::Cgb);
    }

    #[test]
    fn e15_04_with_config_default_matches_new() {
        let plain = Dmg::new(&e14_03_cart()).expect("valid");
        let configured = Dmg::with_config(&e14_03_cart(), MachineConfig::default()).expect("valid");
        assert_eq!(configured.model(), Model::Dmg);
        assert_eq!(configured.cpu.regs(), plain.cpu.regs());
    }

    #[test]
    fn e15_04_with_config_checks_boot_rom_length() {
        let bad = MachineConfig {
            model: Model::Dmg,
            boot_rom: Some(vec![0u8; 255]),
        };
        assert_eq!(
            Dmg::with_config(&e14_03_cart(), bad).err(),
            Some(LoadError::BadBootRom)
        );
        let wrong_for_cgb = MachineConfig {
            model: Model::Cgb,
            boot_rom: Some(vec![0u8; 256]),
        };
        assert_eq!(
            Dmg::with_config(&e14_03_cart(), wrong_for_cgb).err(),
            Some(LoadError::BadBootRom)
        );
        let good = MachineConfig {
            model: Model::Dmg,
            boot_rom: Some(e14_03_boot()),
        };
        assert!(Dmg::with_config(&e14_03_cart(), good).is_ok());
    }

    /// 32 KiB ROM-only cartridge image: byte 0 is $AA, the rest is $00 (E14_03).
    fn e14_03_cart() -> Vec<u8> {
        let mut image = vec![0x00u8; 32 * 1024];
        image[0] = 0xAA;
        image
    }

    /// Synthetic boot ROM: NOPs, JP $00FC at $0000, then LD A,$01 and LDH ($50),A at $00FC (E14_03).
    fn e14_03_boot() -> Vec<u8> {
        let mut boot = vec![0x00u8; 256];
        boot[0x00] = 0xC3;
        boot[0x01] = 0xFC;
        boot[0x02] = 0x00;
        boot[0xFC] = 0x3E;
        boot[0xFD] = 0x01;
        boot[0xFE] = 0xE0;
        boot[0xFF] = 0x50;
        boot
    }

    /// Runs at most 40 M-cycles until the boot ROM is unmapped; true when it happened (E14_03).
    fn e14_03_run_until_unmapped(dmg: &mut Dmg) -> bool {
        for _ in 0..40 {
            for _ in 0..4 {
                dmg.tick();
            }
            if !dmg.bus.boot_mapped() {
                return true;
            }
        }
        false
    }

    #[test]
    fn e14_03_boot_rom_runs_then_hands_off() {
        let mut dmg = Dmg::with_boot_rom(&e14_03_cart(), &e14_03_boot()).expect("valid boot ROM");
        assert_eq!(dmg.cpu.pc, 0x0000);
        assert!(dmg.bus.boot_mapped());
        assert_eq!(dmg.bus.read(0x0000), 0xC3);
        assert!(e14_03_run_until_unmapped(&mut dmg));
        assert_eq!(dmg.cpu.pc, 0x0100);
        assert_eq!(dmg.cpu.a, 0x01);
        assert_eq!(dmg.bus.read(0x0000), 0xAA);
    }

    #[test]
    fn e14_03_bad_boot_size_is_an_error() {
        let result = Dmg::with_boot_rom(&e14_03_cart(), &[0u8; 255]);
        assert_eq!(result.err(), Some(LoadError::BadBootRom));
    }

    #[test]
    fn e14_03_new_without_boot_rom_is_unchanged() {
        let dmg = Dmg::new(&e14_03_cart()).expect("valid cartridge");
        assert_eq!(dmg.cpu.pc, 0x0100);
        assert!(!dmg.bus.boot_mapped());
        assert_eq!(dmg.bus.read(0x0000), 0xAA);
    }

    #[test]
    fn e14_03_power_on_state() {
        let dmg = Dmg::with_boot_rom(&e14_03_cart(), &e14_03_boot()).expect("valid boot ROM");
        assert_eq!(dmg.cpu.pc, 0x0000);
        assert_eq!(dmg.cpu.sp, 0x0000);
        assert_eq!(dmg.cpu.a, 0x00);
        assert_eq!(dmg.cpu.f, 0x00);
        assert_eq!(dmg.bus.io[0x40], 0x00);
        assert_eq!(dmg.bus.io[0x0F], 0x00);
    }

    #[test]
    fn e14_03_reset_restarts_from_the_boot_rom() {
        let mut dmg = Dmg::with_boot_rom(&e14_03_cart(), &e14_03_boot()).expect("valid boot ROM");
        assert!(e14_03_run_until_unmapped(&mut dmg));
        dmg.reset();
        assert!(dmg.bus.boot_mapped());
        assert_eq!(dmg.cpu.pc, 0x0000);
        assert_eq!(dmg.bus.read(0x0000), 0xC3);
    }

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
            (0x40u8, LINE_DOTS * 2, 1u32), // coincidence: high all of line 0 (LY == LYC == 0), falls on line 1
            (0x20, LINE_DOTS - 1, 1), // mode 2: power-up OAM window; stop before the wrap re-enters mode 2
            (0x10, FRAME_DOTS, 1),    // VBlank: one rise entering vblank per frame
            (0x08, LINE_DOTS, 2), // HBlank: one rise on line 0's HBlank The lagged mode reads 0 for the first 4 dots after power-up (rise 1), then the real HBlank (rise 2) (FIX_09).
        ];
        for (stat, n, expected) in cases {
            let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
            dmg.bus.write(0xFF41, stat); // enable only this source
            assert_eq!(
                count_stat_raises(&mut dmg, n),
                expected,
                "source {stat:02X} raise count"
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
        assert_eq!(raises, 2); // coincidence clears at line 1 dot 0, the 4-dot lagged mode 2 re-raises at dot 4 (FIX_09)
    }

    #[test]
    fn e03_08_new_request_after_the_line_fell() {
        use crate::video::{LINE_DOTS, MODE2_DOTS};
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // Enable only mode 2 (OAM scan): the line is high for dots 0..79 of each visible line.
        dmg.bus.write(0xFF41, 0x20);
        let raises = count_stat_raises(&mut dmg, LINE_DOTS * 3 + 8); // 1 edge at the STAT write (power-up is in mode 2) + 1 per line wrap at ticks 456, 912, 1368 (the lagged mode 2 rises 4 dots after each wrap, FIX_09)
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
        dmg.bus.write(0xFF45, 0x05); // LYC != LY(0): the LY==LYC flag frozen at switch-off is false, so enabling the STAT sources raises nothing (FIX_08)
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
                                     // The first line after LCD on has no OAM scan phase: STAT reads mode 0 (FIX_08).
        dmg.tick(); // the transition is applied in Machine::tick
        assert_eq!(dmg.bus.read(0xFF44), 0x00); // restarted at line 0
        let stat = dmg.bus.read(0xFF41);
        assert_eq!(stat & 0x03, 0x00); // first line after LCD on has no OAM scan phase: mode 0 (FIX_08)
    }

    #[test]
    fn fix_08_first_line_after_lcd_on_reads_mode_0() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // LCD off, then on: the PPU restarts at line 0 (E03_09 spec).
        dmg.bus.write(0xFF40, 0x11);
        dmg.tick();
        dmg.bus.write(0xFF40, 0x91);
        for _ in 0..12 {
            dmg.tick(); // a few dots into the first line after LCD on (FIX_08)
        }
        assert_eq!(dmg.bus.read(0xFF44), 0x00); // still line 0
        assert_eq!(dmg.bus.read(0xFF41) & 0x03, 0x00); // STAT reads mode 0: no OAM scan phase (FIX_08)
    }

    #[test]
    fn fix_08_second_line_after_lcd_on_reads_mode_2_again() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // LCD off, then on: the PPU restarts at line 0 (E03_09 spec).
        dmg.bus.write(0xFF40, 0x11);
        dmg.tick();
        dmg.bus.write(0xFF40, 0x91);
        // Advance to line 1.
        let mut ticks = 0u32;
        while dmg.ppu.ly() != 1 {
            dmg.tick();
            ticks += 1;
            assert!(ticks <= 600, "line 1 not reached within 600 ticks");
        }
        for _ in 0..8 {
            dmg.tick(); // a few dots into line 1 (FIX_08)
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x03, 0x02); // normal start-of-line mode: OAM scan again (E03_09 spec; FIX_08)
    }

    #[test]
    fn fix_08_first_line_mode_3_still_starts_at_dot_80() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        // LCD off, then on: the PPU restarts at line 0 (E03_09 spec).
        dmg.bus.write(0xFF40, 0x11);
        dmg.tick();
        dmg.bus.write(0xFF40, 0x91);
        // Advance to dot 100 of the first line after LCD on.
        let mut ticks = 0u32;
        while !(dmg.ppu.line_dot() == 100 && dmg.ppu.ly() == 0) {
            dmg.tick();
            ticks += 1;
            assert!(
                ticks <= 600,
                "dot 100 of line 0 not reached within 600 ticks"
            );
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x03, 0x03); // mode 3 starts at dot 80 even on the first line (E03_09 spec; FIX_08)
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
        for _ in 0..4 {
            dmg.tick(); // CPU access follows the mode lagged by 4 dots (FIX_09)
        }
        dmg.bus.write(0x8000, 0xA5);
        assert_eq!(dmg.bus.read(0x8000), 0xA5); // VRAM free (E03_10 spec)
        dmg.bus.write(0xFE00, 0x90);
        assert_eq!(dmg.bus.read(0xFE00), 0x90); // OAM free (E03_10 spec)
                                                // Mode 1 (VBlank): line 150.
        let target1 = LINE_DOTS * 150 + MODE2_DOTS;
        for _ in 0..(target1 - target0 - 4) {
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
    fn e11_01_machine_forwards_the_palette_to_the_ppu() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        let colors = [0xFF01_0203, 0xFF04_0506, 0xFF07_0809, 0xFF0A_0B0C];
        dmg.set_dmg_palette(colors);
        assert_eq!(dmg.dmg_palette(), colors);
        assert_eq!(dmg.ppu.dmg_palette(), colors);
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
        for _ in 0..8 {
            dmg.tick(); // 2 M-cycles: the DMA start delay (mooneye oam_dma_start, FIX_02)
        }
        assert_eq!(dmg.bus.oam[0], 0); // nothing copied yet during the start delay
        assert!(dmg.bus.dma_active()); // the transfer is active from the 2nd M-cycle
        for _ in 0..4 {
            dmg.tick(); // 3rd M-cycle: the first byte is copied
        }
        assert_eq!(dmg.bus.oam[0], 0x5A); // first OAM byte copied on the 3rd M-cycle
        assert_eq!(dmg.bus.oam[1], 0); // second byte not copied yet: one byte per M-cycle (E03_06 spec)
        assert!(dmg.bus.dma_active()); // transfer still running

        for _ in 0..632 {
            dmg.tick(); // 644 dots total = 161 M-cycles: 159 bytes copied, still running
        }
        assert!(dmg.bus.dma_active()); // 159 of 160 bytes copied: one step left

        for _ in 0..4 {
            dmg.tick(); // 648 dots total = 162 M-cycles: the final byte is copied
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

    #[test]
    fn e04_03_machine_relays_wave_registers_and_wave_ram() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF30, 0x12);
        apu_write(&mut dmg, 0xFF1A, 0x80);
        apu_write(&mut dmg, 0xFF1C, 0x20);
        apu_write(&mut dmg, 0xFF1D, 0xFF);
        apu_write(&mut dmg, 0xFF1E, 0x87);
        assert_eq!(dmg.audio.apu.wave.ram[0], 0x12);
        assert!(dmg.audio.apu.wave.enabled);
        assert_eq!(dmg.bus.read(0xFF26) & 0x07, 0x05);
    }

    #[test]
    fn e04_03_length_expiry_clears_nr52_bit_2() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00);
        apu_write(&mut dmg, 0xFF1A, 0x80);
        apu_write(&mut dmg, 0xFF1B, 0xFF);
        apu_write(&mut dmg, 0xFF1C, 0x20);
        apu_write(&mut dmg, 0xFF1D, 0x00);
        apu_write(&mut dmg, 0xFF1E, 0xC0);
        assert_eq!(dmg.bus.read(0xFF26) & 0x04, 0x04);
        for _ in 0..8192 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF26) & 0x07, 0x01);
    }

    #[test]
    fn e04_03_wave_ram_survives_apu_power_off() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF30, 0x12);
        apu_write(&mut dmg, 0xFF26, 0x00);
        assert_eq!(dmg.bus.read(0xFF30), 0x12);
        assert_eq!(dmg.audio.apu.wave.ram[0], 0x12);
    }

    #[test]
    fn e04_04_machine_relays_noise_registers() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF21, 0xF0);
        apu_write(&mut dmg, 0xFF22, 0x00);
        apu_write(&mut dmg, 0xFF23, 0x80);
        assert!(dmg.audio.apu.noise.enabled);
        assert_eq!(dmg.bus.read(0xFF26) & 0x0F, 0x09);
    }

    #[test]
    fn e04_04_length_expiry_clears_nr52_bit_3() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00);
        apu_write(&mut dmg, 0xFF20, 0x3F);
        apu_write(&mut dmg, 0xFF21, 0xF0);
        apu_write(&mut dmg, 0xFF22, 0x00);
        apu_write(&mut dmg, 0xFF23, 0xC0);
        assert_eq!(dmg.bus.read(0xFF26) & 0x08, 0x08);
        for _ in 0..8192 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF26) & 0x0F, 0x01);
    }

    #[test]
    fn e04_04_power_off_clears_channel_4() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF21, 0xF0);
        apu_write(&mut dmg, 0xFF23, 0x80);
        assert!(dmg.audio.apu.noise.enabled);
        apu_write(&mut dmg, 0xFF26, 0x00);
        assert_eq!(dmg.bus.read(0xFF26), 0x70);
        assert!(!dmg.audio.apu.noise.enabled);
    }

    fn drain_all(dmg: &mut Dmg) -> Vec<i16> {
        let mut buf = vec![0i16; 4096];
        let n = dmg.drain_audio(&mut buf);
        buf.truncate(n);
        buf
    }

    #[test]
    fn e04_05_one_frame_of_ticks_yields_548_stereo_frames() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..70224 {
            dmg.tick();
        }
        assert_eq!(drain_all(&mut dmg).len(), 1096);
    }

    #[test]
    fn e04_05_apu_power_off_gives_silence() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF26, 0x00);
        for _ in 0..1280 {
            dmg.tick();
        }
        let samples = drain_all(&mut dmg);
        assert_eq!(samples.len(), 20);
        assert!(samples.iter().all(|s| *s == 0));
    }

    #[test]
    fn e04_05_panning_left_only_leaves_the_right_side_silent() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF25, 0x10);
        for _ in 0..2560 {
            dmg.tick();
        }
        let samples = drain_all(&mut dmg);
        assert_eq!(samples.len(), 40);
        assert_ne!(samples[0], 0);
        assert!(samples.chunks(2).all(|c| c[1] == 0));
    }

    #[test]
    fn e04_05_panning_right_only_leaves_the_left_side_silent() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF25, 0x01);
        for _ in 0..2560 {
            dmg.tick();
        }
        let samples = drain_all(&mut dmg);
        assert_eq!(samples.len(), 40);
        assert_ne!(samples[1], 0);
        assert!(samples.chunks(2).all(|c| c[0] == 0));
    }

    #[test]
    fn e04_05_reset_restarts_the_sampling_phase() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..124 {
            dmg.tick();
        }
        dmg.reset();
        for _ in 0..8 {
            dmg.tick();
        }
        assert_eq!(drain_all(&mut dmg).len(), 0);
    }

    #[test]
    fn e04_07_length_written_while_off_survives_power_cycle() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF26, 0x00);
        apu_write(&mut dmg, 0xFF11, 0x3E);
        apu_write(&mut dmg, 0xFF26, 0x80);
        assert_eq!(dmg.audio.apu.pulse1.length, 2);
    }

    #[test]
    fn e04_07_machine_mirrors_wave_access_to_the_bus() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        apu_write(&mut dmg, 0xFF1A, 0x80);
        apu_write(&mut dmg, 0xFF1D, 0xFD);
        apu_write(&mut dmg, 0xFF1E, 0x87);
        let mut seen_open = false;
        for _ in 0..16 {
            for _ in 0..4 {
                dmg.tick();
            }
            if matches!(dmg.bus.wave_access, WaveAccess::Open(_)) {
                seen_open = true;
            }
        }
        assert!(seen_open);
        apu_write(&mut dmg, 0xFF26, 0x00);
        apu_write(&mut dmg, 0xFF26, 0x00);
        assert_eq!(dmg.bus.wave_access, WaveAccess::Free);
    }

    #[test]
    fn e05_01_set_input_reaches_p1_and_requests_the_joypad_interrupt() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.io[0x0F] &= !0x10;
        assert_eq!(dmg.bus.read(0xFF00), 0xCF);
        dmg.set_input(Input {
            start: true,
            ..Input::default()
        });
        assert_eq!(dmg.bus.read(0xFF00), 0xC7);
        assert_eq!(dmg.bus.io[0x0F] & 0x10, 0x10);
    }

    /// A 32 KiB ROM with the cartridge type at $0147 and the RAM size code at $0149.
    fn battery_rom(cart_type: u8, ram_code: u8) -> Vec<u8> {
        let mut r = rom(32 * 1024, 0x00);
        r[0x147] = cart_type;
        r[0x149] = ram_code;
        r
    }

    #[test]
    fn e05_06_rom_only_cartridge_has_no_battery_ram() {
        let mut dmg = Dmg::new(&battery_rom(0x00, 0x00)).expect("ROM loads");
        assert!(dmg.battery_ram().is_none());
        assert_eq!(
            dmg.load_battery_ram(&[0u8; 8192]),
            Err(LoadError::NoBatteryRam)
        );
    }

    #[test]
    fn e05_06_ram_without_battery_is_not_exposed() {
        let mut dmg = Dmg::new(&battery_rom(0x02, 0x02)).expect("ROM loads");
        assert!(dmg.battery_ram().is_none());
        assert_eq!(
            dmg.load_battery_ram(&[0u8; 8192]),
            Err(LoadError::NoBatteryRam)
        );
    }

    #[test]
    fn e05_06_battery_type_without_ram_reports_none() {
        let mut dmg = Dmg::new(&battery_rom(0x03, 0x00)).expect("ROM loads");
        assert!(dmg.battery_ram().is_none());
        assert_eq!(
            dmg.load_battery_ram(&[0u8; 8192]),
            Err(LoadError::NoBatteryRam)
        );
    }

    #[test]
    fn e05_06_battery_ram_round_trips() {
        let image = battery_rom(0x03, 0x02);
        let mut dmg = Dmg::new(&image).expect("ROM loads");
        assert_eq!(dmg.battery_ram().map(|v| v.len()), Some(8192));
        dmg.bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        dmg.bus.write(0xA000, 0x5A);
        dmg.bus.write(0xBFFF, 0x77);
        let saved = dmg.battery_ram().expect("battery").to_vec();
        assert_eq!(saved[0], 0x5A);
        assert_eq!(saved[8191], 0x77);
        let mut fresh = Dmg::new(&image).expect("ROM loads");
        assert_eq!(fresh.battery_ram().expect("battery")[0], 0x00);
        assert_eq!(fresh.load_battery_ram(&saved), Ok(()));
        fresh.bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        assert_eq!(fresh.bus.read(0xA000), 0x5A);
        assert_eq!(fresh.bus.read(0xBFFF), 0x77);
    }

    #[test]
    fn e05_06_load_battery_ram_rejects_a_size_mismatch_and_keeps_the_ram() {
        let mut dmg = Dmg::new(&battery_rom(0x03, 0x02)).expect("ROM loads");
        dmg.bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        dmg.bus.write(0xA000, 0x11);
        assert_eq!(
            dmg.load_battery_ram(&[0u8; 100]),
            Err(LoadError::BatteryRamSizeMismatch {
                expected: 8192,
                actual: 100
            })
        );
        assert_eq!(
            dmg.load_battery_ram(&vec![0u8; 8193]),
            Err(LoadError::BatteryRamSizeMismatch {
                expected: 8192,
                actual: 8193
            })
        );
        assert_eq!(dmg.bus.read(0xA000), 0x11);
    }

    /// ROM image with the cartridge header fields set for mapper selection (E05_07): type at
    /// $0147, ROM size code at $0148, RAM size code at $0149 and a valid checksum at $014D.
    fn e05_07_rom(cart_type: u8, rom_size_code: u8, ram_code: u8, banks: usize) -> Vec<u8> {
        let mut rom = vec![0x00u8; banks * 0x4000];
        rom[0x147] = cart_type;
        rom[0x148] = rom_size_code;
        rom[0x149] = ram_code;
        let mut x = 0u8;
        for &b in &rom[0x134..=0x14C] {
            x = x.wrapping_sub(b).wrapping_sub(1);
        }
        rom[0x14D] = x; // header checksum (note 07a)
        rom
    }

    #[test]
    fn e05_07_battery_ram_round_trips_through_the_mapper() {
        let mut dmg = Dmg::new(&e05_07_rom(0x03, 0x00, 0x02, 2)).expect("ROM loads");
        dmg.bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        dmg.bus.write(0xA000, 0x5A);
        let saved = dmg.battery_ram().expect("battery");
        assert_eq!(saved[0], 0x5A);
        let mut fresh = Dmg::new(&e05_07_rom(0x03, 0x00, 0x02, 2)).expect("ROM loads");
        assert_eq!(fresh.load_battery_ram(&saved), Ok(()));
        fresh.bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        assert_eq!(fresh.bus.read(0xA000), 0x5A);
    }

    #[test]
    fn e05_07_mbc3_clock_advances_with_dmg_tick() {
        // All-zero ROM: the CPU loops on BR $0100 and never touches the cartridge.
        let mut dmg = Dmg::new(&e05_07_rom(0x13, 0x00, 0x02, 2)).expect("ROM loads");
        let mut p = dmg.battery_ram().expect("battery");
        let start = p.len() - 8; // dot_acc: last 8 bytes of the payload, u64 little-endian (media/mbc3.rs)
        p[start..].copy_from_slice(&(4_194_304u64 - 100).to_le_bytes());
        dmg.load_battery_ram(&p).unwrap();
        dmg.bus.write(0x0000, 0x0A); // enable the MBC3 RAM/RTC (note 07b)
        dmg.bus.write(0x4000, 0x08); // select the seconds register
        dmg.bus.write(0x6000, 0x00); // arm the latch
        dmg.bus.write(0x6000, 0x01); // latch: seconds is still 0 (99 dots short of a second)
        assert_eq!(dmg.bus.read(0xA000), 0);
        for _ in 0..128 {
            dmg.tick(); // one dot each; 32 M-cycles x 4 dots = 128 >= 100 (E05_07)
        }
        dmg.bus.write(0x6000, 0x00); // arm the latch again
        dmg.bus.write(0x6000, 0x01); // latch: the seconds counter has advanced by one
        assert_eq!(dmg.bus.read(0xA000), 1);
    }

    #[test]
    fn fix_07_mode2_select_raises_stat_at_the_start_of_line_144() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF41, 0x20); // select bit 5 (OAM scan) in STAT ($FF41)
        dmg.bus.io[0x0F] = 0; // clear all pending requests
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_07)
        assert_eq!(dmg.bus.io[0x0F] & 0x03, 0x03); // vblank and STAT requested on the same dot (FIX_07)
    }

    #[test]
    fn fix_07_without_the_select_bit_only_vblank_is_raised() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF41, 0x00); // no STAT select bits: only vblank can request (FIX_07)
        dmg.bus.io[0x0F] = 0; // clear all pending requests
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_07)
        assert_eq!(dmg.bus.io[0x0F] & 0x03, 0x01); // only the vblank request is raised (FIX_07)
    }

    #[test]
    fn fix_07_line_144_start_does_not_block_a_later_mode2_request() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF41, 0x20); // select bit 5 (OAM scan) in STAT ($FF41)
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_07)
        dmg.bus.io[0x0F] = 0; // clear all pending requests at the start of line 144
        for _ in 0..10_000 {
            if dmg.ppu.mode() == 2 && dmg.ppu.ly() == 0 {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 2); // mode 2 reached on line 0 of the next frame (FIX_07)
        for _ in 0..4 {
            dmg.tick(); // the mode 2 irq follows the mode lagged by 4 dots (FIX_09)
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x02, 0x02); // a later mode-2 request still works (FIX_07)
    }

    #[test]
    fn fix_08_lyc_flag_is_retained_while_the_lcd_is_off() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_08)
        dmg.bus.write(0xFF45, 0x90); // LYC ($FF45) = LY: coincidence flag live-true
        for _ in 0..8 {
            dmg.tick();
        }
        dmg.bus.write(0xFF40, 0x00); // LCD off (LCDC bit 7 cleared)
        for _ in 0..4 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x04); // flag retained while the LCD is off (FIX_08)
        dmg.bus.write(0xFF45, 0x01); // LYC no longer equals LY: a live compare would clear it
        for _ in 0..4 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x04); // flag still retained (FIX_08)
    }

    #[test]
    fn fix_08_enabling_the_lcd_recomputes_the_flag() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_08)
        dmg.bus.write(0xFF45, 0x90); // LYC ($FF45) = LY: coincidence flag live-true
        for _ in 0..8 {
            dmg.tick();
        }
        dmg.bus.write(0xFF40, 0x00); // LCD off (LCDC bit 7 cleared)
        for _ in 0..4 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x04); // flag retained while the LCD is off (FIX_08)
        dmg.bus.write(0xFF45, 0x01); // LYC no longer equals LY: a live compare would clear it
        for _ in 0..4 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x04); // flag still retained (FIX_08)
        dmg.bus.write(0xFF40, 0x80); // LCD on again: the flag is recomputed live
        for _ in 0..8 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x00); // LY != LYC now, so the flag is clear (FIX_08)
    }

    #[test]
    fn fix_08_no_stat_interrupt_when_the_flag_stays_true() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_08)
        dmg.bus.write(0xFF41, 0x40); // select bit 6 (LYC coincidence) in STAT ($FF41)
        dmg.bus.write(0xFF45, 0x90); // LYC ($FF45) = LY: coincidence flag live-true
        for _ in 0..8 {
            dmg.tick();
        }
        dmg.bus.write(0xFF40, 0x00); // LCD off (LCDC bit 7 cleared): the line stays high via the retained flag
        for _ in 0..4 {
            dmg.tick();
        }
        dmg.bus.io[0x0F] = 0; // clear all pending requests
        dmg.bus.write(0xFF45, 0x00); // LYC no longer equals LY: a live compare would fall
        for _ in 0..4 {
            dmg.tick();
        }
        dmg.bus.write(0xFF40, 0x80); // LCD on again: LY restarts at line 0 and equals LYC
        for _ in 0..8 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x02, 0x00); // no STAT request: the line never fell (FIX_08)
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x04); // flag recomputed live: LY == LYC again (FIX_08)
    }

    #[test]
    fn fix_08_stat_interrupt_when_the_flag_rises_at_lcd_on() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        for _ in 0..FRAME_DOTS {
            if dmg.ppu.ly() == VBLANK_FIRST_LINE {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), VBLANK_FIRST_LINE); // reached line 144 within one frame (FIX_08)
        dmg.bus.write(0xFF41, 0x40); // select bit 6 (LYC coincidence) in STAT ($FF41)
        dmg.bus.write(0xFF45, 0x00); // LYC ($FF45) = 0: no coincidence while line 144 is shown
        for _ in 0..8 {
            dmg.tick();
        }
        dmg.bus.write(0xFF40, 0x00); // LCD off (LCDC bit 7 cleared): the retained flag stays false
        for _ in 0..4 {
            dmg.tick();
        }
        dmg.bus.io[0x0F] = 0; // clear all pending requests
        dmg.bus.write(0xFF40, 0x80); // LCD on again: LY restarts at line 0 and equals LYC
        for _ in 0..8 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x02, 0x02); // STAT request raised when the flag rises (FIX_08)
    }

    #[test]
    fn fix_09_lcdon_timing_matches_mooneye() {
        // Replica of the reads of mooneye acceptance/ppu/lcdon_timing-GS: pass p enables the LCD
        // (LCDC=$81) then waits p+N nops before one read; a read happens 4*(p+N+2) dots after the
        // write (FIX_09). Tables copied from the mooneye source.
        const CYCLES: [[u32; 8]; 3] = [
            [0, 17, 60, 110, 130, 174, 224, 244],
            [1, 18, 61, 111, 131, 175, 225, 245],
            [2, 19, 62, 112, 132, 176, 226, 246],
        ];
        const LY: [[u8; 8]; 3] = [
            [0, 0, 0, 0, 1, 1, 1, 2],
            [0, 0, 0, 1, 1, 1, 2, 2],
            [0, 0, 0, 1, 1, 1, 2, 2],
        ];
        const STAT0: [[u8; 8]; 3] = [
            [0x84, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82],
            [0x84, 0x87, 0x84, 0x80, 0x82, 0x80, 0x80, 0x82],
            [0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x83],
        ];
        const STAT1: [[u8; 8]; 3] = [
            [0x80, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82],
            [0x80, 0x83, 0x80, 0x80, 0x86, 0x84, 0x80, 0x82],
            [0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x83],
        ];
        const OAM: [[u8; 8]; 3] = [
            [0, 0, 0xFF, 0, 0xFF, 0xFF, 0, 0xFF],
            [0, 0xFF, 0, 0xFF, 0xFF, 0, 0xFF, 0xFF],
            [0, 0xFF, 0, 0xFF, 0xFF, 0, 0xFF, 0xFF],
        ];
        const VRAM: [[u8; 8]; 3] = [
            [0, 0, 0xFF, 0, 0, 0xFF, 0, 0],
            [0, 0xFF, 0, 0, 0xFF, 0, 0, 0xFF],
            [0, 0xFF, 0, 0, 0xFF, 0, 0, 0xFF],
        ];

        let mut bad: Vec<String> = Vec::new();
        for pass in 0..3 {
            for i in 0..8 {
                for kind in 0..5 {
                    let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
                    dmg.bus.write(0xFF40, 0x11); // LCD off (LCDC)
                    dmg.tick();
                    if kind == 2 {
                        dmg.bus.write(0xFF45, 0x01); // LYC = 1
                    } else {
                        dmg.bus.write(0xFF45, 0x00); // LYC = 0
                    }
                    dmg.bus.write(0xFF40, 0x81); // LCD on (LCDC)
                    for _ in 0..(4 * (CYCLES[pass][i] + 2)) {
                        dmg.tick(); // CYCLES already includes the pass nops: read at 4*(N+2) dots
                    }
                    let expected = match kind {
                        0 => LY[pass][i],
                        1 => STAT0[pass][i],
                        2 => STAT1[pass][i],
                        3 => OAM[pass][i],
                        _ => VRAM[pass][i],
                    };
                    let got = match kind {
                        0 => dmg.bus.read(0xFF44),     // LY
                        1 | 2 => dmg.bus.read(0xFF41), // STAT
                        3 => dmg.bus.read(0xFE00),     // OAM
                        _ => dmg.bus.read(0x8000),     // VRAM
                    };
                    if got != expected {
                        let name = match kind {
                            0 => "LY",
                            1 => "STAT lyc0",
                            2 => "STAT lyc1",
                            3 => "OAM",
                            _ => "VRAM",
                        };
                        bad.push(format!(
                            "{} pass{} cycle={} dots={}: expected {:#04x} got {:#04x}",
                            name,
                            pass + 1,
                            CYCLES[pass][i],
                            4 * (CYCLES[pass][i] + 2),
                            expected,
                            got
                        ));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "\n{}", bad.join("\n"));
    }

    #[test]
    fn fix_09_first_line_after_lcd_on_ends_449_ticks_after_the_write() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF40, 0x11); // LCD off (LCDC)
        dmg.tick();
        dmg.bus.write(0xFF40, 0x81); // LCD on (LCDC): the line restarts at dot 3, so the first tick after this write is dot 4 (FIX_12)
        for _ in 0..448 {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), 0); // LY still 0 after 448 ticks (dot 451): the first line runs from dot 3 to dot 452 (FIX_09, FIX_12)
        dmg.tick();
        assert_eq!(dmg.ppu.ly(), 1); // the 449th tick ends the first line (FIX_09, FIX_12)
    }

    #[test]
    fn fix_09_oam_is_accessible_during_the_first_line_scan_window() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.oam[0] = 0x77;
        dmg.bus.write(0xFF40, 0x11); // LCD off (LCDC)
        dmg.tick();
        dmg.bus.write(0xFF40, 0x81); // LCD on (LCDC)
        for _ in 0..12 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFE00), 0x77); // no OAM scan on the first line: OAM stays accessible (FIX_09)
    }

    #[test]
    fn fix_09_oam_is_blocked_in_the_scan_window_of_later_lines() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.oam[0] = 0x77;
        dmg.bus.write(0xFF40, 0x11); // LCD off (LCDC)
        dmg.tick();
        dmg.bus.write(0xFF40, 0x81); // LCD on (LCDC)
        for _ in 0..600 {
            if dmg.ppu.ly() == 1 {
                break;
            }
            dmg.tick();
        }
        assert_eq!(dmg.ppu.ly(), 1);
        for _ in 0..12 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFE00), 0xFF); // line 1 scans OAM normally: the read is blocked (FIX_09)
    }

    #[test]
    fn fix_09_lyc_flag_is_clear_during_the_first_4_dots_of_a_line() {
        let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
        dmg.bus.write(0xFF45, 0x01); // LYC = 1 (E03_08)
        for _ in 0..crate::video::LINE_DOTS {
            dmg.tick(); // reach line 1 dot 0
        }
        assert_eq!(dmg.ppu.ly(), 1);
        assert_eq!(dmg.ppu.line_dot(), 0);
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x00); // LY==LYC flag reads 0 during the first 4 dots (FIX_09)
        for _ in 0..4 {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF41) & 0x04, 0x04); // flag set again once past the first 4 dots (FIX_09)
    }

    #[test]
    fn fix_10_lcdon_write_timing_matches_mooneye() {
        // Replica of the writes of mooneye acceptance/ppu/lcdon_write_timing-GS: with the LCD off,
        // seed OAM/VRAM to $00, turn the LCD on (LCDC=$81), wait NOPS[i] nops, write $81 to the
        // target, then read it back. Tables copied from the mooneye source.
        const NOPS: [u32; 19] = [
            0, 17, 18, 60, 61, 110, 111, 112, 130, 131, 132, 174, 175, 224, 225, 226, 244, 245, 246,
        ];
        const OAM_OK: [u8; 19] = [1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0];
        const VRAM_OK: [u8; 19] = [1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 0];

        let mut bad = String::new();
        for i in 0..NOPS.len() {
            for (addr, ok) in [(0xFE00u16, OAM_OK), (0x8000u16, VRAM_OK)] {
                let mut dmg = Dmg::new(&rom(32 * 1024, 0x00)).expect("ROM loads");
                // LCD off so the seed writes land
                dmg.bus.write(0xFF40, 0x00);
                for _ in 0..4 {
                    dmg.tick(); // let the PPU see the LCD switched off before anything else
                }
                dmg.bus.write(0xFE00, 0x00);
                dmg.bus.write(0x8000, 0x00);
                dmg.bus.write(0xFF40, 0x81); // LCD on (LCDC)
                for _ in 0..(4 * (NOPS[i] + 2)) {
                    dmg.tick();
                }
                dmg.bus.write(addr, 0x81);
                dmg.bus.write(0xFF40, 0x00); // LCD off so the read lands
                for _ in 0..4 {
                    dmg.tick(); // let the LCD-off access mirrors update before reading back
                }
                let got = dmg.bus.read(addr);
                let expected = if ok[i] == 1 { 0x81 } else { 0x00 };
                if got != expected {
                    let name = if addr == 0xFE00 { "OAM" } else { "VRAM" };
                    bad.push_str(&format!(
                        "{} nops={} d={}: expected {:#04x} got {:#04x}\n",
                        name,
                        NOPS[i],
                        4 * (NOPS[i] + 2),
                        expected,
                        got
                    ));
                }
            }
        }
        if !bad.is_empty() {
            panic!("{}", bad);
        }
    }
}
