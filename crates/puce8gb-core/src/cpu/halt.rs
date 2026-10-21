//! HALT ($76), STOP ($10) and the eleven illegal opcodes (task C01_33): low-power states
//! and CPU lock-up. Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// HALT ($76), STOP ($10) and the eleven illegal opcodes (task C01_33). Returns true when
    /// the current opcode is in this group, false otherwise. Behaviour per note 02b and the
    /// fixed hardware spec of task C01_33:
    /// - HALT ($76): one M-cycle total (the fetch), then three cases decided at its end: not
    ///   pending -> halted = true; IME set and pending -> no halt, the instruction ends and
    ///   the dispatch happens at the next boundary; IME clear and pending -> HALT bug, no
    ///   halt and the byte after HALT is fetched twice (halt_bug flag).
    /// - STOP ($10): a two-byte NOP: 2 M-cycles (opcode fetch, then read and ignore the next
    ///   byte), PC += 2. No low-power mode is emulated (task C01_33; note 02b "STOP").
    /// - The eleven illegal opcodes ($D3 $DB $DD $E3 $E4 $EB $EC $ED $F4 $FC $FD): the CPU
    ///   locks up. The fetch takes one M-cycle, sets locked = true and PC stays after the
    ///   opcode; every later tick is an idle M-cycle (note 02a "Les 11 opcodes illegaux font
    ///   hard-lock the CPU"). They are NOT recorded as unimplemented.
    pub(super) fn exec_halt_stop(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // HALT ($76): one M-cycle total (the fetch). The three cases are decided at the end of
        // that fetch (task C01_33 hardware spec); pending uses the same helper as dispatch.
        if op == 0x76 {
            let pending = bus.pending_interrupts() != 0;
            if !pending {
                self.halted = true; // case 1: wait for a request (IME=1 or IME=0)
            } else if self.ime {
                // Case 2: no halt; the instruction ends and the dispatch happens at the next
                // boundary, where PC already points at the byte after HALT and that address is pushed.
            } else {
                // Case 3 (HALT bug, note 02b): no halt; the byte after HALT is fetched twice.
                self.halt_bug = true;
            }
            // HALT is a complete instruction after its fetch (1 M-cycle total).
            self.done();
            return true;
        }

        // STOP ($10): a two-byte NOP (task C01_33 hardware spec). Step 2 reads and ignores the
        // second byte at PC, then ends. No low-power mode is emulated.
        if op == 0x10 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let _ = bus.read(self.pc); // second byte: read and ignored
                    self.pc = self.pc.wrapping_add(1);
                    bus.stop_speed_switch(); // Cgb speed switch when KEY1 is armed, otherwise nothing (E15_16)
                    self.done();
                    return true;
                }
            }
        }

        // The eleven illegal opcodes (note 02a "Les 11 opcodes illegaux font hard-lock the
        // CPU"): $D3 $DB $DD $E3 $E4 $EB $EC $ED $F4 $FC $FD. The fetch is the whole
        // instruction; it locks the CPU and is NOT recorded as unimplemented.
        if ILLEGAL_OPCODES.contains(&op) {
            self.locked = true;
            self.done();
            return true;
        }

        false
    }
}

/// The eleven illegal opcodes that lock the CPU (note 02a, task C01_33 hardware spec).
const ILLEGAL_OPCODES: [u8; 11] = [
    0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD,
];

/// True when the opcode must execute before any interrupt dispatch at its boundary (task C01_33):
/// HALT ($76) defers its case-2 dispatch to the next boundary (pushed PC = byte after HALT), and an
/// illegal opcode locks the CPU so no dispatch ever happens. STOP ($10) is NOT exempt: it behaves as
/// a plain 2-byte NOP that an interrupt preempts (C01_32). Used by `tick()` at the boundary.
pub(super) fn preempt_exempt(op: u8) -> bool {
    op == 0x76 || ILLEGAL_OPCODES.contains(&op)
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    /// Address where test code is placed (work RAM; the ROM area $0000-$7FFF ignores writes).
    const CODE: u16 = 0xC000;

    /// Place `code` at CODE (work RAM) and return a CPU with PC there.
    fn cpu_at_code(bus: &mut crate::bus::Bus, code: &[u8]) -> Cpu {
        for (i, &b) in code.iter().enumerate() {
            bus.write(CODE.wrapping_add(i as u16), b);
        }
        Cpu {
            pc: CODE,
            ..Cpu::default()
        }
    }

    /// Arm a vblank interrupt request (IE bit 0 enabled, IF bit 0 set).
    fn set_pending(bus: &mut crate::bus::Bus) {
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0x01; // IF bit 0 (vblank) requested
    }

    /// The eleven illegal opcodes that lock the CPU (note 02a "Les 11 opcodes illegaux").
    const ILLEGAL: [u8; 11] = [
        0xD3, 0xDB, 0xDD, 0xE3, 0xE4, 0xEB, 0xEC, 0xED, 0xF4, 0xFC, 0xFD,
    ];

    #[test]
    fn c01_33_power_on_not_halted_or_locked() {
        let cpu = Cpu::default();
        assert!(!cpu.halted()); // false at power-on
        assert!(!cpu.locked()); // false at power-on

        let cpu = Cpu::reset(0);
        assert!(!cpu.halted());
        assert!(!cpu.locked());
    }

    #[test]
    fn c01_33_halt_waits() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x76, 0x00]); // HALT, NOP at CODE
                                                            // IME=0, IE=$00 and no IF request pending (power-up state of the bus).

        cpu.tick(&mut bus); // tick 1: fetch HALT -> halted
        assert!(cpu.halted());

        for _ in 0..10 {
            cpu.tick(&mut bus); // ten idle M-cycles: no PC change, still halted
        }
        assert_eq!(cpu.pc, CODE.wrapping_add(1)); // PC unchanged during the halt
        assert!(cpu.halted());

        set_pending(&mut bus); // IE=$01, IF bit 0 set; IME still 0
        cpu.tick(&mut bus); // wake up in this tick and fetch the NOP in it
        assert!(!cpu.halted());
        assert_eq!(cpu.pc, CODE.wrapping_add(2)); // PC past HALT and the fetched NOP
        assert_eq!(bus.io[0x0F], 0x01); // IF bit not cleared: no dispatch with IME=0
        assert!(!cpu.ime); // IME still 0
    }

    #[test]
    fn c01_33_halt_ime_dispatch() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x76]); // HALT at CODE
        cpu.sp = 0xDFFE;
        cpu.ime = true;
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0; // IF clear at HALT time (reset leaves IF=$E1)

        cpu.tick(&mut bus); // fetch HALT: IME=1 but not pending -> halted
        assert!(cpu.halted());
        for _ in 0..3 {
            cpu.tick(&mut bus); // idle M-cycles while no request is pending
        }
        assert_eq!(cpu.pc, CODE.wrapping_add(1));

        set_pending(&mut bus); // IF bit 0 requested: pending now
        cpu.tick(&mut bus); // wake up in this tick and start the dispatch (M1)
        assert!(!cpu.halted());
        for _ in 0..4 {
            cpu.tick(&mut bus); // M2..M5 of the five-M-cycle entry
        }

        assert_eq!(cpu.pc, 0x0040); // vector of IF bit 0 (vblank)
        assert_eq!(cpu.sp, 0xDFFC); // SP = SP - 2
        let pushed = CODE.wrapping_add(1); // the byte after HALT was not fetched
        assert_eq!(bus.read(0xDFFD), (pushed >> 8) as u8); // high byte at [SP-1]
        assert_eq!(bus.read(0xDFFC), pushed as u8); // low byte at [SP-2]
        assert_eq!(bus.io[0x0F] & 0x1F, 0); // IF bit 0 acknowledged by the dispatch
        assert!(!cpu.ime); // IME cleared by the service of the interrupt
    }

    #[test]
    fn c01_33_halt_bug() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x76, 0x3C, 0x00]); // HALT, INC A, NOP at CODE
        cpu.ime = false;
        set_pending(&mut bus); // IE=$01, IF bit 0: a request is pending when HALT executes

        for _ in 0..3 {
            cpu.tick(&mut bus); // fetch HALT + the INC A byte read twice (the bug)
        }
        assert!(!cpu.halted()); // HALT must not set halted
        assert_eq!(cpu.a, 0x02); // INC A executed twice: its byte was read twice
        assert_eq!(cpu.pc, CODE.wrapping_add(2)); // PC now points at the NOP

        cpu.tick(&mut bus); // fetch and execute the NOP
        assert_eq!(cpu.pc, CODE.wrapping_add(3));
    }

    #[test]
    fn c01_33_halt_no_bug_with_ime() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x76]); // HALT at CODE
        cpu.sp = 0xDFFE;
        cpu.ime = true;
        set_pending(&mut bus); // IE=$01, IF bit 0: pending when HALT executes

        cpu.tick(&mut bus); // fetch HALT: IME=1 and pending -> no halt, instruction ends
        assert!(!cpu.halted()); // no HALT bug with IME=1
        for _ in 0..5 {
            cpu.tick(&mut bus); // the five M-cycles of the entry right after HALT
        }

        assert_eq!(cpu.pc, 0x0040); // vector of IF bit 0 (vblank)
        let pushed = CODE.wrapping_add(1); // PC already pointed past HALT at dispatch time
        assert_eq!(bus.read(0xDFFD), (pushed >> 8) as u8); // high byte at [SP-1]
        assert_eq!(bus.read(0xDFFC), pushed as u8); // low byte at [SP-2]: address+1
    }

    #[test]
    fn c01_33_stop_skips_byte() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x10, 0x00, 0x3C]); // STOP, $00, INC A at CODE

        for _ in 0..2 {
            cpu.tick(&mut bus); // STOP: opcode fetch + the second byte read and ignored
        }
        assert_eq!(cpu.pc, CODE.wrapping_add(2)); // PC advanced past both bytes
        assert!(cpu.at_boundary());
        assert_eq!(cpu.unimplemented(), None); // $10 is claimed by the halt group

        cpu.tick(&mut bus); // fetch and execute the INC A at CODE+2
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.pc, CODE.wrapping_add(3));
    }

    #[test]
    fn e15_16_stop_with_key1_armed_toggles_the_speed() {
        let mut bus = testutil::new_bus();
        bus.cgb = true;
        let mut cpu = cpu_at_code(&mut bus, &[0x10, 0x00, 0x00]);
        assert_eq!(bus.read(0xFF4D), 0x7E);
        bus.write(0xFF4D, 0x01);
        assert_eq!(bus.read(0xFF4D), 0x7F);
        for _ in 0..2 {
            cpu.tick(&mut bus);
        }
        assert_eq!(bus.read(0xFF4D), 0xFE); // bit 7 = 1 (double speed), bit 0 = 0
        assert!(bus.double_speed());
        cpu.pc = CODE;
        for _ in 0..2 {
            cpu.tick(&mut bus);
        }
        assert_eq!(bus.read(0xFF4D), 0xFE); // STOP without arming: speed unchanged
        assert!(bus.double_speed());
    }

    #[test]
    fn e15_16_dmg_model_stop_is_unchanged() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x10, 0x00, 0x00]);
        bus.write(0xFF4D, 0x01);
        for _ in 0..2 {
            cpu.tick(&mut bus);
        }
        assert_eq!(cpu.pc, CODE.wrapping_add(2));
        assert_eq!(bus.read(0xFF4D), 0xFF);
        assert!(!bus.double_speed());
    }

    #[test]
    fn c01_33_illegal_locks() {
        for op in ILLEGAL {
            let mut bus = testutil::new_bus();
            let mut cpu = cpu_at_code(&mut bus, &[op]); // one illegal opcode at CODE
            cpu.sp = 0xDFFE;
            cpu.ime = true;
            set_pending(&mut bus); // IE=$01, IF bit 0: a request is pending

            cpu.tick(&mut bus); // fetch the illegal opcode: the CPU locks up
            assert!(cpu.locked(), "opcode ${:02X} must lock the CPU", op);
            assert_eq!(cpu.pc, CODE.wrapping_add(1)); // PC stays after the opcode
            assert_eq!(cpu.unimplemented(), None); // not recorded as unimplemented

            for _ in 0..100 {
                cpu.tick(&mut bus); // every later tick is one idle M-cycle
            }
            assert_eq!(cpu.pc, CODE.wrapping_add(1)); // nothing changes
            assert_eq!(cpu.sp, 0xDFFE); // no dispatch: SP unchanged
            assert!(cpu.ime); // IME untouched: the interrupt was not served
            assert_eq!(bus.io[0x0F], 0x01); // IF bit still pending
        }
    }
}
