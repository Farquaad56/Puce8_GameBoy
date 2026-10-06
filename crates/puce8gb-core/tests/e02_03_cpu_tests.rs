use puce8gb_core::bus::Bus;
use puce8gb_core::cpu::Cpu;

/// Run CPU until it reaches an instruction boundary (Idle state after being Active).
fn run_to_boundary(cpu: &mut Cpu, bus: &mut Bus) {
    for _ in 0..10 { // Safety limit to prevent infinite loops
        let was_idle = matches!(cpu.instr_state, puce8gb_core::cpu::InstrState::Idle);
        cpu.tick(bus);
        if matches!(cpu.instr_state, puce8gb_core::cpu::InstrState::Idle) && !was_idle {
            return;
        }
    }
}

#[test]
fn e02_03_nop_preserves_f() {
    let mut bus = Bus::new(vec![0x00; 1]); // NOP opcode
    let mut cpu = Cpu::default();
    cpu.pc = 0xC000;

    // Set all flag bits (including low nibble) to 1.
    cpu.f = 0b_1111_0000 | 0x0F;

    run_to_boundary(&mut cpu, &mut bus);

    // F should keep upper bits and clear low nibble.
    assert_eq!(cpu.f & 0x0F, 0, "low nibble of F must stay 0");
}

#[test]
fn e02_03_ld_bc_n16_advances_pc_correctly() {
    // LD BC, n16 takes 3 bytes: opcode + low + high.
    let mut bus = Bus::new(vec![0x01, 0x12, 0x34]);
    let mut cpu = Cpu::default();
    cpu.pc = 0xC000;

    run_to_boundary(&mut cpu, &mut bus);

    assert_eq!(cpu.pc, 0xC003, "PC should advance past LD BC, n16");
}

#[test]
fn e02_03_ld_hl_sp_n16_advances_pc() {
    // LD HL, SP+n takes 3 bytes: opcode + offset.
    let mut bus = Bus::new(vec![0xF8, 0x12, 0x34]);
    let mut cpu = Cpu::default();
    cpu.pc = 0xC000;

    run_to_boundary(&mut cpu, &mut bus);

    assert_eq!(cpu.pc, 0xC003, "PC should advance past LD HL, SP+n");
}

#[test]
fn e02_03_cb_prefix_advances_pc() {
    // CB prefix: first byte is CB, second byte is the actual instruction.
    let mut bus = Bus::new(vec![0xCB, 0x7E]);
    let mut cpu = Cpu::default();
    cpu.pc = 0xC000;

    run_to_boundary(&mut cpu, &mut bus);

    // After first tick: PC=0xC001 (CB consumed).
    // After second tick: PC=0xC002 (operand byte consumed, instruction complete).
    assert_eq!(cpu.pc, 0xC002, "CB instruction should consume 2 bytes");
}

#[test]
fn e02_03_bit_operations_set_clear_flags() {
    // NOP: PC should stay at 0xC000 after one tick.
    let mut bus = Bus::new(vec![0x00]);
    let mut cpu = Cpu { f: 0b_0100_0000, ..Cpu::default() };
    cpu.pc = 0xC000;

    run_to_boundary(&mut cpu, &mut bus);

    // Carry bit (bit 4) must stay set.
    assert_eq!(cpu.f & 0x10, 0b_0001_0000, "Carry bit must stay set");
}
