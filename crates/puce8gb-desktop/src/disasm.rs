//! Disassembly of traced instructions from the generated SM83 opcode table (E08_04).

use crate::opcodes::{CB_OPCODES, OPCODES};
use puce8gb_core::trace::TraceEntry;

/// Length in bytes of the instruction starting with `bytes[0]` (the CB prefix counts 2).
pub fn instruction_len(bytes: [u8; 3]) -> usize {
    if bytes[0] == 0xCB {
        2
    } else {
        usize::from(OPCODES[usize::from(bytes[0])].bytes)
    }
}

/// Mnemonic text of the instruction at `pc` with its operands filled in.
/// n16/a16 -> $XXXX, n8 -> $XX, (a8) -> ($FFXX), JR e8 -> target address, other e8 -> signed offset.
pub fn disassemble(pc: u16, bytes: [u8; 3]) -> String {
    if bytes[0] == 0xCB {
        return CB_OPCODES[usize::from(bytes[1])].mnemonic.to_string();
    }
    let info = &OPCODES[usize::from(bytes[0])];
    let imm8 = bytes[1];
    let imm16 = u16::from_le_bytes([bytes[1], bytes[2]]);
    let rel = bytes[1] as i8;
    let out = info
        .mnemonic
        .replace("n16", &format!("${imm16:04X}"))
        .replace("a16", &format!("${imm16:04X}"))
        .replace("(a8)", &format!("($FF{imm8:02X})"))
        .replace("n8", &format!("${imm8:02X}"));
    if info.mnemonic.starts_with("JR") {
        let target = pc.wrapping_add(2).wrapping_add(rel as u16);
        out.replace("e8", &format!("${target:04X}"))
    } else {
        let signed = format!("{rel:+}");
        out.replace("+e8", &signed).replace("e8", &signed)
    }
}

/// One trace window line: `PC  bytes  mnemonic  AF BC DE HL SP` (registers before the instruction).
pub fn trace_row(entry: &TraceEntry) -> String {
    let len = instruction_len(entry.bytes);
    let hex: Vec<String> = entry
        .bytes
        .iter()
        .take(len)
        .map(|b| format!("{b:02X}"))
        .collect();
    let r = &entry.regs;
    format!(
        "{:04X}  {:<8}  {:<16}  AF={:02X}{:02X} BC={:02X}{:02X} DE={:02X}{:02X} HL={:02X}{:02X} SP={:04X}",
        entry.pc,
        hex.join(" "),
        disassemble(entry.pc, entry.bytes),
        r.a,
        r.f,
        r.b,
        r.c,
        r.d,
        r.e,
        r.h,
        r.l,
        r.sp
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use puce8gb_core::debug::CpuRegs;

    #[test]
    fn e08_04_disasm_immediates_and_addresses() {
        let cases: [(u16, [u8; 3], &str); 11] = [
            (0x0100, [0x00, 0xC3, 0x50], "NOP"),
            (0x0100, [0x01, 0x34, 0x12], "LD BC,$1234"),
            (0x0100, [0x06, 0x7F, 0x00], "LD B,$7F"),
            (0x0100, [0x08, 0x00, 0xC0], "LD ($C000),SP"),
            (0x0100, [0x10, 0x00, 0x00], "STOP $00"),
            (0x0100, [0xC3, 0x50, 0x01], "JP $0150"),
            (0x0100, [0xCD, 0x00, 0x40], "CALL $4000"),
            (0x0100, [0xE0, 0x40, 0x00], "LDH ($FF40),A"),
            (0x0100, [0xFA, 0x44, 0xFF], "LD A,($FF44)"),
            (0x0100, [0xCF, 0x00, 0x00], "RST $08"),
            (0x0100, [0xE3, 0x00, 0x00], "ILLEGAL_E3"),
        ];
        for (pc, bytes, expected) in cases {
            assert_eq!(disassemble(pc, bytes), expected);
        }
    }

    #[test]
    fn e08_04_disasm_relative_jumps_and_sp_offsets() {
        assert_eq!(disassemble(0x0100, [0x18, 0xFE, 0x00]), "JR $0100");
        assert_eq!(disassemble(0x0109, [0x18, 0xF9, 0x00]), "JR $0104");
        assert_eq!(disassemble(0x0200, [0x20, 0x05, 0x00]), "JR NZ,$0207");
        assert_eq!(disassemble(0xFFFE, [0x18, 0x02, 0x00]), "JR $0002");
        assert_eq!(disassemble(0x0100, [0xF8, 0x05, 0x00]), "LD HL,SP+5");
        assert_eq!(disassemble(0x0100, [0xF8, 0xFB, 0x00]), "LD HL,SP-5");
    }

    #[test]
    fn e08_04_disasm_cb_prefix() {
        assert_eq!(disassemble(0x0100, [0xCB, 0x18, 0x00]), "RR B");
        assert_eq!(disassemble(0x0100, [0xCB, 0xC3, 0x00]), "SET 0,E");
        assert_eq!(disassemble(0x0100, [0xCB, 0xC6, 0x00]), "SET 0,(HL)");
    }

    #[test]
    fn e08_04_instruction_len_values() {
        assert_eq!(instruction_len([0x00, 0, 0]), 1);
        assert_eq!(instruction_len([0x06, 0, 0]), 2);
        assert_eq!(instruction_len([0x01, 0, 0]), 3);
        assert_eq!(instruction_len([0xCD, 0, 0]), 3);
        assert_eq!(instruction_len([0xCB, 0x18, 0]), 2);
    }

    #[test]
    fn e08_04_opcode_table_integrity() {
        assert!(OPCODES.iter().all(|o| (1..=3).contains(&o.bytes)));
        assert!(CB_OPCODES.iter().all(|o| o.bytes == 2));
        assert_eq!(OPCODES[0xCB].mnemonic, "PREFIX");
        assert_eq!(OPCODES.iter().filter(|o| o.illegal).count(), 11);
    }

    #[test]
    fn e08_04_trace_row_format() {
        let entry = TraceEntry {
            pc: 0x0104,
            bytes: [0x04, 0x80, 0xEA],
            regs: CpuRegs {
                a: 0x01,
                f: 0x80,
                b: 0x00,
                c: 0x13,
                d: 0x00,
                e: 0xD8,
                h: 0x01,
                l: 0x4D,
                sp: 0xFFFE,
                pc: 0x0104,
            },
        };
        assert_eq!(
            trace_row(&entry),
            format!(
                "0104  {:<8}  {:<16}  AF=0180 BC=0013 DE=00D8 HL=014D SP=FFFE",
                "04", "INC B"
            )
        );
    }
}
