//! Instruction trace ring (E08_04). Preallocated when enabled and filled only while enabled.
//! It only reads the machine (peek and register copies), so it never changes timing or results.
use crate::debug::CpuRegs;
use crate::machine::Dmg;

/// Entries kept in the ring.
pub const TRACE_CAPACITY: usize = 256;

/// One traced instruction: PC, the 3 bytes at PC when it started, and the registers before it ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TraceEntry {
    pub pc: u16,
    pub bytes: [u8; 3],
    pub regs: CpuRegs,
}

/// Fixed-size ring of the most recent instructions. No allocation after `set_enabled(true)`.
#[derive(Debug, Default)]
pub struct Trace {
    entries: Vec<TraceEntry>,
    head: usize,
    len: usize,
    enabled: bool,
}

impl Trace {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Enabling allocates the ring once; disabling keeps the stored entries.
    pub fn set_enabled(&mut self, on: bool) {
        if on && self.entries.is_empty() {
            self.entries = vec![TraceEntry::default(); TRACE_CAPACITY];
        }
        self.enabled = on;
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Stores an entry, overwriting the oldest one when full. Ignored while not allocated.
    pub fn push(&mut self, entry: TraceEntry) {
        let cap = self.entries.len();
        if cap == 0 {
            return;
        }
        let tail = (self.head + self.len) % cap;
        self.entries[tail] = entry;
        if self.len == cap {
            self.head = (self.head + 1) % cap;
        } else {
            self.len += 1;
        }
    }

    /// Copies the most recent min(out.len(), len) entries into `out`, oldest first.
    /// Returns how many entries were written.
    pub fn copy_recent(&self, out: &mut [TraceEntry]) -> usize {
        let n = out.len().min(self.len);
        let cap = self.entries.len();
        let first = self.len - n;
        for (i, slot) in out.iter_mut().take(n).enumerate() {
            *slot = self.entries[(self.head + first + i) % cap];
        }
        n
    }
}

impl Dmg {
    pub fn set_trace_enabled(&mut self, on: bool) {
        self.trace.set_enabled(on);
    }

    pub fn trace_enabled(&self) -> bool {
        self.trace.enabled()
    }

    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    /// Copies the most recent traced instructions into `out`, oldest first; returns the count.
    pub fn debug_trace(&self, out: &mut [TraceEntry]) -> usize {
        self.trace.copy_recent(out)
    }

    /// Called by Machine::tick right before the CPU tick of an M-cycle: records the instruction
    /// that is about to start. Read-only on the machine; does nothing while the trace is off.
    pub(crate) fn trace_instruction_start(&mut self) {
        if !self.trace.enabled() || !self.cpu.will_fetch(&self.bus) {
            return;
        }
        let pc = self.cpu.pc;
        let bytes = [
            self.bus.peek(pc),
            self.bus.peek(pc.wrapping_add(1)),
            self.bus.peek(pc.wrapping_add(2)),
        ];
        let regs = self.debug_regs();
        self.trace.push(TraceEntry { pc, bytes, regs });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;

    /// LD A,1 ; LD B,0 ; loop at $0104: INC B ; ADD A,B ; LD ($C000),A ; JR loop.
    fn program_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x10B].copy_from_slice(&[
            0x3E, 0x01, 0x06, 0x00, 0x04, 0x80, 0xEA, 0x00, 0xC0, 0x18, 0xF9,
        ]);
        rom
    }

    fn fingerprint(dmg: &Dmg) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |b: u8| {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        };
        for px in dmg.framebuffer() {
            for b in px.to_le_bytes() {
                feed(b);
            }
        }
        let r = dmg.debug_regs();
        for b in [r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l] {
            feed(b);
        }
        for b in r.sp.to_le_bytes() {
            feed(b);
        }
        for b in r.pc.to_le_bytes() {
            feed(b);
        }
        for addr in 0xC000u16..0xC100 {
            feed(dmg.peek(addr));
        }
        for addr in 0xFF00u16..=0xFFFF {
            feed(dmg.peek(addr));
        }
        for b in dmg.frame_count().to_le_bytes() {
            feed(b);
        }
        h
    }

    fn run_frames(trace: bool) -> Dmg {
        let mut dmg = Dmg::new(&program_rom()).unwrap();
        dmg.set_trace_enabled(trace);
        for _ in 0..3 {
            dmg.run_frame();
        }
        dmg
    }

    #[test]
    fn e08_04_ring_wraps_keeps_newest() {
        let mut trace = Trace::default();
        trace.set_enabled(true);
        for i in 0..(TRACE_CAPACITY + 5) {
            trace.push(TraceEntry {
                pc: i as u16,
                ..TraceEntry::default()
            });
        }
        assert_eq!(trace.len(), TRACE_CAPACITY);
        let mut all = [TraceEntry::default(); TRACE_CAPACITY + 8];
        assert_eq!(trace.copy_recent(&mut all), TRACE_CAPACITY);
        assert_eq!(all[0].pc, 5);
        assert_eq!(all[TRACE_CAPACITY - 1].pc, (TRACE_CAPACITY + 4) as u16);
        let mut last = [TraceEntry::default(); 3];
        assert_eq!(trace.copy_recent(&mut last), 3);
        let base = (TRACE_CAPACITY + 2) as u16;
        assert_eq!(
            [last[0].pc, last[1].pc, last[2].pc],
            [base, base + 1, base + 2]
        );
    }

    #[test]
    fn e08_04_ring_is_allocated_on_enable_only() {
        let mut trace = Trace::default();
        trace.push(TraceEntry::default());
        assert!(trace.is_empty());
        trace.set_enabled(true);
        trace.push(TraceEntry::default());
        assert_eq!(trace.len(), 1);
        trace.clear();
        assert!(trace.is_empty());
        trace.set_enabled(false);
        assert!(!trace.enabled());
    }

    #[test]
    fn e08_04_trace_on_or_off_gives_identical_results() {
        let off = run_frames(false);
        let on = run_frames(true);
        assert!(off.frame_count() >= 2);
        assert_eq!(fingerprint(&off), fingerprint(&on));
        let mut buf = [TraceEntry::default(); 4];
        assert!(!off.trace_enabled());
        assert_eq!(off.debug_trace(&mut buf), 0);
        assert_eq!(on.debug_trace(&mut buf), 4);
    }

    #[test]
    fn e08_04_entries_follow_the_program() {
        let mut dmg = Dmg::new(&program_rom()).unwrap();
        dmg.set_trace_enabled(true);
        for _ in 0..400 {
            dmg.tick();
        }
        let mut e = [TraceEntry::default(); TRACE_CAPACITY];
        let n = dmg.debug_trace(&mut e);
        assert!(n >= 7);
        assert_eq!(e[0].pc, 0x0100);
        assert_eq!(e[0].bytes, [0x3E, 0x01, 0x06]);
        assert_eq!(e[0].regs.pc, 0x0100);
        assert_eq!(e[0].regs.a, 0x01);
        assert_eq!(e[0].regs.f, 0x80);
        assert_eq!(e[0].regs.sp, 0xFFFE);
        assert_eq!(e[1].pc, 0x0102);
        assert_eq!(e[1].bytes, [0x06, 0x00, 0x04]);
        assert_eq!(e[2].pc, 0x0104);
        assert_eq!(e[2].bytes, [0x04, 0x80, 0xEA]);
        assert_eq!(e[2].regs.b, 0);
        assert_eq!(e[3].pc, 0x0105);
        assert_eq!(e[3].bytes, [0x80, 0xEA, 0x00]);
        assert_eq!(e[3].regs.b, 1);
        assert_eq!(e[4].pc, 0x0106);
        assert_eq!(e[4].bytes, [0xEA, 0x00, 0xC0]);
        assert_eq!(e[4].regs.a, 2);
        assert_eq!(e[5].pc, 0x0109);
        assert_eq!(e[5].bytes, [0x18, 0xF9, 0x00]);
        assert_eq!(e[6].pc, 0x0104);
        assert_eq!(e[6].regs.a, 2);
        assert_eq!(e[6].regs.b, 1);
    }

    #[test]
    fn e08_04_halt_idle_cycles_are_not_recorded() {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100] = 0x76; // HALT
        let mut dmg = Dmg::new(&rom).unwrap();
        dmg.bus.io[0xFF] = 0x00; // IE = 0: nothing can wake the CPU
        dmg.set_trace_enabled(true);
        for _ in 0..400 {
            dmg.tick();
        }
        let mut e = [TraceEntry::default(); 8];
        assert_eq!(dmg.debug_trace(&mut e), 1);
        assert_eq!(e[0].pc, 0x0100);
        assert_eq!(e[0].bytes[0], 0x76);
        assert!(dmg.cpu.halted());
    }
}
