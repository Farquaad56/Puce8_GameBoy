//! Save state of the CPU engine (E09_02, decision A_05): every field, fixed order, no padding.

use super::Cpu;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Cpu {
    /// Writes the full CPU state, 28 bytes, little-endian: a f b c d e h l, sp, pc, opcode,
    /// step, lo, hi, cb, ime, ei_delay, interrupt_step, interrupt_mask, halted, locked,
    /// halt_bug, then the unimplemented opcode as flag + opcode + address (zeros when absent).
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        let (flag, op, addr) = match self.unimplemented {
            Some((op, addr)) => (true, op, addr),
            None => (false, 0, 0),
        };
        w.put_u8(self.a)?;
        w.put_u8(self.f)?;
        w.put_u8(self.b)?;
        w.put_u8(self.c)?;
        w.put_u8(self.d)?;
        w.put_u8(self.e)?;
        w.put_u8(self.h)?;
        w.put_u8(self.l)?;
        w.put_u16(self.sp)?;
        w.put_u16(self.pc)?;
        w.put_u8(self.opcode)?;
        w.put_u8(self.step)?;
        w.put_u8(self.lo)?;
        w.put_u8(self.hi)?;
        w.put_bool(self.cb)?;
        w.put_bool(self.ime)?;
        w.put_u8(self.ei_delay)?;
        w.put_u8(self.interrupt_step)?;
        w.put_u8(self.interrupt_mask)?;
        w.put_bool(self.halted)?;
        w.put_bool(self.locked)?;
        w.put_bool(self.halt_bug)?;
        w.put_bool(flag)?;
        w.put_u8(op)?;
        w.put_u16(addr)
    }

    /// Reads what `write_state` wrote. The CPU is left unchanged when the data is truncated
    /// or holds an invalid flag byte. Ranges of step/interrupt_step: UNKNOWN - to confirm,
    /// not validated.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let mut next = *self;
        next.a = r.get_u8()?;
        next.f = r.get_u8()?;
        next.b = r.get_u8()?;
        next.c = r.get_u8()?;
        next.d = r.get_u8()?;
        next.e = r.get_u8()?;
        next.h = r.get_u8()?;
        next.l = r.get_u8()?;
        next.sp = r.get_u16()?;
        next.pc = r.get_u16()?;
        next.opcode = r.get_u8()?;
        next.step = r.get_u8()?;
        next.lo = r.get_u8()?;
        next.hi = r.get_u8()?;
        next.cb = r.get_bool()?;
        next.ime = r.get_bool()?;
        next.ei_delay = r.get_u8()?;
        next.interrupt_step = r.get_u8()?;
        next.interrupt_mask = r.get_u8()?;
        next.halted = r.get_bool()?;
        next.locked = r.get_bool()?;
        next.halt_bug = r.get_bool()?;
        let flag = r.get_bool()?;
        let op = r.get_u8()?;
        let addr = r.get_u16()?;
        next.unimplemented = if flag { Some((op, addr)) } else { None };
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BYTES: [u8; 28] = [
        0x12, 0xB0, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0xFF, 0x21, 0x43, 0xCB, 0x03, 0x11,
        0x22, 0x01, 0x01, 0x02, 0x04, 0x04, 0x01, 0x00, 0x01, 0x01, 0xD3, 0x50, 0x01,
    ];

    fn sample() -> Cpu {
        Cpu {
            a: 0x12,
            f: 0xB0,
            b: 0x34,
            c: 0x56,
            d: 0x78,
            e: 0x9A,
            h: 0xBC,
            l: 0xDE,
            sp: 0xFFF0,
            pc: 0x4321,
            opcode: 0xCB,
            step: 3,
            lo: 0x11,
            hi: 0x22,
            cb: true,
            ime: true,
            ei_delay: 2,
            interrupt_step: 4,
            interrupt_mask: 0x04,
            halted: true,
            locked: false,
            halt_bug: true,
            unimplemented: Some((0xD3, 0x0150)),
        }
    }

    fn saved(cpu: &Cpu) -> [u8; 28] {
        let mut buf = [0u8; 28];
        let mut w = StateWriter::new(&mut buf);
        cpu.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 28);
        buf
    }

    #[test]
    fn e09_02_cpu_layout_is_fixed() {
        assert_eq!(saved(&sample()), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_cpu_round_trip_is_byte_identical() {
        let mut loaded = Cpu::default();
        let mut r = StateReader::new(&SAMPLE_BYTES);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded, sample());
        assert_eq!(saved(&loaded), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_cpu_absent_unimplemented_clears_previous_value() {
        let empty = Cpu::default();
        let bytes = saved(&empty);
        assert_eq!(&bytes[24..], &[0, 0, 0, 0]);
        let mut cpu = sample();
        cpu.read_state(&mut StateReader::new(&bytes))
            .expect("valid");
        assert_eq!(cpu, empty);
    }

    #[test]
    fn e09_02_cpu_truncated_data_is_bad_size_and_leaves_cpu_unchanged() {
        for len in 0..28 {
            let mut cpu = Cpu::default();
            assert_eq!(
                cpu.read_state(&mut StateReader::new(&SAMPLE_BYTES[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(cpu, Cpu::default());
        }
    }

    #[test]
    fn e09_02_cpu_corrupted_flag_is_invalid_state() {
        for index in [16usize, 17, 21, 22, 23, 24] {
            let mut bytes = SAMPLE_BYTES;
            bytes[index] = 2;
            let mut cpu = Cpu::default();
            assert_eq!(
                cpu.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(cpu, Cpu::default());
        }
    }

    #[test]
    fn e09_02_cpu_write_to_small_buffer_fails() {
        let mut buf = [0u8; 27];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
