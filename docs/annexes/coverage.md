# Coverage matrix (D_18)

Date: 2026-10-05. Method: every note in docs/annexes/notes was listed, counted (lines and UNKNOWN markers),
and its facts were rated against refs/pandocs and roms/test-roms. Ratings are honest: partial means partial.
No hardware has been emulated yet (crates/puce8gb-core is a version stub only), so "Couvert" means the
documentation needed to code the subject exists in the notes; it does not mean the code exists.

Note inventory (lines / UNKNOWN markers):

| Note | Lines | UNKNOWN |
|---|---|---|
| 01_timing.md | 88 | 2 |
| 01b_timer.md | 92 | 4 |
| 02a_cpu_core.md | 111 | 0 |
| 02b_cpu_interrupts.md | 98 | 2 |
| 02c_cpu_opcodes_src.md | 70 | 0 |
| 03a_memory_map.md | 80 | 1 |
| 03b_io_registers.md | 134 | 22 |
| 04a_video_regs.md | 101 | 1 |
| 04b_video_render.md | 81 | 1 |
| 05a_audio_channels.md | 97 | 2 |
| 05b_audio_mixing.md | 63 | 0 |
| 06_input_serial.md | 70 | 0 |
| 07a_cart_header.md | 99 | 2 |
| 07b_mappers.md | 120 | 1 |
| 08_boot_reset.md | 72 | 1 |
| 09_test_roms.md | 102 | 2 |

## Matrix

| Sujet | Couvert | Partiel | Manquant | Sources |
|---|---|---|---|---|
| Timing | Clock 4.194304 MHz, dot = T-cycle, M-cycle = 4 dots, 456 dots/line, 154 lines/frame, VBlank 144-153, 70224 cycles/frame (~59.73 Hz), PPU mode durations per line (DMG normal speed), DIV clock 16384 Hz, OAM DMA = 160 M-cycles / 640 dots | Illegal opcodes: CONFLIT JSON (4 dots) vs real hardware (lock up); timer system counter width UNKNOWN; TMA/TIMA reload-on-overflow CONFLIT (old vs new value) | Cycles per line/frame not stated in pandocs (derived from 456 x 154); boot ROM duration in cycles/frames | notes 01_timing.md, 01b_timer.md; open_questions D_01, D_03 |
| CPU | Registers AF BC DE HL SP PC, F flag bits z n h c at 7-4 (bits 3-0 unused), r16 push/pop, POP AF, SM83 vs Z80 differences, ~4 MHz cadence multiples of 4 cycles, DAA, ADD SP,e / LD HL,SP+e, opcode blocks + HALT/STOP/CB prefix, DMG reset values, IME internal flag, IE/IF layout, vectors $40/$48/$50/$58/$60 with priority bit 0 highest, interrupt entry = 5 M-cycles, EI/DI/RETI timing, HALT bug (IME=0 + pending request), STOP; CB table cycles from Opcodes.json spot-checked against pandocs | F flag bit placement CONFIRME at source level only - to confirm on hardware via blargg cpu_instrs; DAA C-flag CONFLIT ("upper 4 bits" vs "upper 8bits"); ei delay boundary UNKNOWN (during or after next instruction); STOP cost in cycles UNKNOWN; $93/$FF halt-like variants UNKNOWN; exact point where IME=0 becomes visible after di UNKNOWN | Full per-instruction cycle table not present in pandocs (Opcodes.json is the source, spot-checked only) | notes 02a_cpu_core.md, 02b_cpu_interrupts.md, 02c_cpu_opcodes_src.md; open_questions D_05, D_06 |
| Carte memoire | Full 16-bit bus map 0x0000-0xFFFF with sizes, echo RAM E000-FDFF, unusable FEA0-FEFF (128 bytes), VRAM/OAM/HRAM CPU access windows per PPU mode, OAM DMA trigger on $FF46, readback values of unmapped/blocked regions; I/O table FF00-FFFF with unused-bits-read-high rule | OAM DMA source range CONFLIT (pandocs $00-$DF vs official $0000-$F19F); OAM DMA bus duration CONFLIT (160 M-cycles vs 160 ns/us); instruction fetch overlap with last M-cycle of previous instruction UNKNOWN; power-up values: OBP0/OBP1 uninitialized, Wave RAM FF30-FF3F unspecified, unassigned I/O addresses read value UNKNOWN, write-only register readback mechanism UNKNOWN; IE ($FFFF) bits 7-5 power-up CONFLIT | Exact DMG behavior of unassigned I/O reads and CGB-only registers not marked [^cgb_only] | notes 03a_memory_map.md, 03b_io_registers.md; open_questions D_07, D_08 |
| Video | LCDC/STAT/LY/LYC/SCX/SCY/WY/WX/BGP/OBP0/OBP1 registers (all R/W), OAM layout 40 x 4 bytes and readback per PPU mode, tile format 8x8 bit order, tile map selection (2 banks BG vs window), spurious STAT interrupt quirk on FF41 write; PPU mode durations + OBJ penalty algorithm (6-11 dots), pixel fetcher 5 stages with dot costs, Get Tile coordinates/tilemap choice, OAM scan mode 2 rule of 10 objects/line, sprite priority and pixel mixing DMG, window rules incl. edge cases, LCD off/on behavior | LY increment timing within the frame UNKNOWN (needs stat_lyc_onoff ROM); first line after LCDC.7 re-enabled mid-frame UNKNOWN; OBP0/OBP1 power-up values UNKNOWN | Exact scanline position of the first rendered line after LCD on | notes 04a_video_regs.md, 04b_video_render.md; open_questions D_09, D_10 |
| Audio | APU architecture and NRxy convention, NR52 master/status, NR51 pan + NR50 volume, CH1 sweep NR10, duty/length NR11/NR21, volume/envelope NR12/NR22/NR42, period/frequency NRx3/NRx4, trigger bit7 of NRx4, length timer and DIV-APU 512 Hz frame sequencer, CH3 wave DAC + Wave RAM, CH4 noise LFSR frequency, DAC power-up values, read masks; mixing: channel enable, pan/volume, high-pass filter, native APU sampling | Write-only register readback on DMG UNKNOWN (FF13/FF18/FF1B/FF1D); Wave RAM FF30-FF3F power-up content UNKNOWN | Mixer HPF details and audio pops left out of the notes (documented in pandocs Audio_details.md, not extracted) | notes 05a_audio_channels.md, 05b_audio_mixing.md; open_questions D_08 |
| Entrees | P1 joypad FF00: 2x4 matrix, select bits 5/4, inverted polarity, read-only low nibble; power-up values P1=$CF SB=$00 SC=$7E (DMG); SC ($FF02) control bits and transfer end detection; DMG internal serial clock 8192 Hz | Reading of SC bits 7-6 on DMG UNKNOWN (gambatte serial ROMs can decide it) | Nothing blocking for DMG; CGB fast serial clock out of scope | notes 06_input_serial.md; open_questions D_13 |
| Media/mappers | Header layout 0100-014F, entry point 0100-0103, Nintendo logo verification by boot ROM, title/manufacturer/CGB flag, SGB flag 0146, licensee fields, ROM size 0148, header checksum 014D and global checksum 014E-014F; No-MBC 32 KiB, mapper choice by 0147, MBC1 (default config, registers/ranges, bank 0 quirks, RAM enable, simple/advanced mode), MBC2 (RAM + map), MBC3 (ROM+RAM+RTC, RTC registers $08-$0C, latch and day counter), MBC5 (8 MiB ROM + banked RAM + rumble) | 0147 MBC4 codes CONFLIT (historical pandocs lists them, The_Cartridge_Header.md does not); 0149=$01 CONFLIT ("2 KBytes" vs "unused"); $08/$09 behavior UNKNOWN; MBC3 RTC access delay CONFLIT (4 ms vs 4 us); unmapped RAM bank wrap-around per chip UNKNOWN | Full licensee tables (no effect on DMG) | notes 07a_cart_header.md, 07b_mappers.md; open_questions D_14, D_15 |
| Boot/reset | Boot ROM existence and size, behavior, hand-off to PC=$0100, CPU registers after boot, I/O registers after boot, RAM state at power-up, what the emulator must reproduce without a boot ROM | Reset values F/TAC/OBP0/OBP1 CONFLIT (specs 2001 fixed AF=$01B0 TAC=$00 OBP0=OBP1=$FF vs pandocs H/C checksum-dependent, TAC=$F8, OBP uninitialized); boot ROM duration in cycles/frames UNKNOWN; Wave RAM power-up content UNKNOWN | DMG boot ROM disassembly (external repo ISSOtm/gb-bootroms not cloned) | notes 08_boot_reset.md; open_questions D_16 |
| Test ROMs | Suite inventory, blargg contents/naming/durations/pass-fail signal/common palette, mooneye end-of-test and pass/fail signal/hardware suffixes/capture-based tests, dmg-acid2, other suites' signals and durations | Per-ROM pass/fail signal of the 11 individual cpu_instrs ROMs UNKNOWN; dmg-acid2 execution duration UNKNOWN; cgb-acid2 / cgb-acid-hell durations UNKNOWN (CGB anyway) | Nothing blocking for DMG coverage | notes 09_test_roms.md; open_questions D_17 |

## Blocking subjects not fully covered

The three blocking subjects (timing, CPU, memory map) are NOT fully covered. Before the CODE phase can
start cycle-accurate work, these must be resolved:

Timing:
1. Illegal opcodes D3 DB DD E3 E4 EB EC ED F4 FC FD: lock up the real CPU or cost 4 dots (CONFLIT).
   A test ROM executing all 11 does not exist yet under roms/test-roms/ and must be produced or found.
2. Timer system counter width (DIV is only its visible low 8 bits): UNKNOWN, no pandocs statement.
3. TMA write landing on the overflow M-cycle: old vs new value transferred to TIMA (CONFLIT).
   Decidable by roms/test-roms/mooneye-test-suite/acceptance/timer/tma_write_reloading.gb.

CPU:
4. F flag bit placement (z n h c at bits 7-4): CONFIRME in sources, must be confirmed on hardware via
   blargg cpu_instrs flag tests before the flags register is coded as final.
5. DAA C-flag: "upper 4 bits" vs "upper 8bits" (CONFLIT) - needs a test ROM or source ruling.
6. ei delay boundary: during or after execution of the next instruction (UNKNOWN). Decidable by
   roms/test-roms/same-suite/interrupt/ei_delay_halt.gb.
7. STOP cost in cycles before sleep (UNKNOWN, table 2001 marks "?").
8. $93/$FF halt-like variants and exact point where IME=0 becomes visible after di (both UNKNOWN).

Memory map:
9. OAM DMA source range ($00-$DF vs $0000-$F19F) and bus duration (160 M-cycles vs 160 ns/us): both
   CONFLIT, decidable by gbmicrotest dma_0x9000.gb / dma_0xE000.gb / dma_timing_a.gb and mooneye
   oam_dma_timing.gb.
10. Instruction fetch overlap with the last M-cycle of the previous instruction (UNKNOWN).
11. Power-up values: IE bits 7-5 ($FFFF) CONFLIT; OBP0/OBP1, Wave RAM FF30-FF3F, unassigned I/O reads,
    write-only readback mechanism all UNKNOWN - decidable by mooneye boot_hwio and targeted test ROMs.

Non-blocking but tracked: video (LY increment timing, first line after LCD on), audio (write-only
readback), mappers (MBC4 codes, 0149=$01, MBC3 RTC delay), boot/reset (F/TAC/OBP reset values CONFLIT,
boot ROM duration).

## Open questions (copied from docs/annexes/open_questions.md)

1. Cycles per line / per frame (DMG): no Pan Docs statement; UNKNOWN - to confirm. [D_01]
2. SM83 cycles-per-instruction table: CPU_Instruction_Set.md lists none; Opcodes.json used as source, spot-checked only. [D_01]
3. Complete DMG boot ROM disassembly: external (ISSOtm/gb-bootroms), not cloned; UNKNOWN - to confirm. [D_01]
4. CONFLIT illegal opcodes: Opcodes.json gives 4 dots for the 11 opcodes, CPU_Comparison_with_Z80.md says real SM83 locks up; no local test ROM executes them yet. [D_02]
5. Timer system counter width (total bits of the internal counter whose low 8 bits are DIV): UNKNOWN - to confirm. [D_03]
6. CONFLIT TMA write on overflow M-cycle: old vs new value transferred to TIMA; decidable by mooneye tma_write_reloading.gb. [D_03]
7. F flag bit placement (z n h c at bits 7-4, low nibble unused): CONFIRME in two concordant pandocs sources, contradicts the D_05 task statement ("low nibble of F"); to confirm on hardware via blargg cpu_instrs. [D_05]
8. ei delay boundary: during or after execution of the next instruction; decidable by same-suite ei_delay_halt.gb. [D_06]
9. STOP cost in cycles before sleep: no pandocs figure, no dedicated local ROM. [D_06]
10. DMG behavior of $93/$FF halt-like variants: not in the invalid-opcode lock list; no dedicated local ROM. [D_06]
11. Exact point where IME=0 becomes visible after di: pandocs only states no interrupt between consecutive ei and di. [D_06]
12. Instruction fetch overlap with last M-cycle of previous instruction: no pandocs statement; indirect hint in OAM_DMA_Transfer.md#best-practices. [D_07]
13. CONFLIT OAM DMA source range: pandocs $00-$DF vs official $0000-$F19F (step 0x100); decidable by gbmicrotest dma_0x9000.gb / dma_0xE000.gb. [D_07]
14. CONFLIT OAM DMA duration: pandocs 160 M-cycles vs official "160 nano-seconds" (1995) / "microseconds" (1998); decidable by mooneye oam_dma_timing.gb + gbmicrotest dma_timing_a.gb. [D_07]
15. CONFLIT power-up/read value of IE ($FFFF) bits 7-5: Power_Up_Sequence.md gives $00 at PC=$0100, IR.md unused-bits-read-high implies >= $E0; decidable by mooneye boot_hwio tests. [D_08]
16. OBP0/OBP1 ($FF48/$FF49) power-up on DMG: "left entirely uninitialized"; which value must reset choose? UNKNOWN - to confirm. [D_08]
17. Wave pattern RAM FF30-FF3F power-up content: no entry in the power-up table; random, fixed, or boot-ROM-written? UNKNOWN - to confirm. [D_08]
18. DMG read value of unassigned I/O addresses ($FF03, $FF08-$FF0E, $FF15, $FF27-$FF2F, $FF4E, $FF57-$FF67, $FF6D-$FF6F, $FF71, $FF78-$FFFE) and CGB-only registers not marked [^cgb_only] ($FF4C, $FF68-$FF6B, $FF6C): "unused bits read high" suggests $FF but is not established. UNKNOWN - to confirm. [D_08]
19. Write-only register readback on DMG (NR13/NR23/NR31/NR33; HDMA1-4 CGB): last written value or $FF? Power-up table gives $FF without explaining the mechanism. UNKNOWN - to confirm. [D_08]
20. LY increment timing: exact frame instant when FF44 advances to the next line is not documented; decidable by mooneye stat_lyc_onoff.gb (+ vblank_stat_intr-GS.gb). [D_09]
21. First line after LCD on: pandocs does not state where in the frame the PPU resumes or what that first line shows; needs a test ROM re-activating LCDC.7 mid-frame. [D_10]
22. SC ($FF02) bits 7-6 read value on DMG: only bit7 and bit0 documented; "unused bits read high" suggests $FE but is not established; decidable by gambatte serial start_wait_read_sc ROMs. [D_13]
23. CONFLIT MBC4 codes at 0147 ($15/$16/$17): historical pandocs lists them, The_Cartridge_Header.md does not; no local ROM decides it. [D_14]
24. CONFLIT RAM size 0149=$01: "2 KBytes" (historical) vs "unused" (The_Cartridge_Header.md); no local ROM decides it. [D_14]
25. Behavior of 0147=$08/$09 (ROM+RAM, never used): unknown in sources. [D_14]
26. CONFLIT MBC3 RTC register access delay: "4 ms" (historical) vs "4 us / 4 M-cycles" (MBC3.md#Delays); decidable by roms/test-roms/rtc3test. [D_15]
27. Unmapped RAM bank wrap-around per MBC chip: general formula in MBCs.md, not detailed per chip; no major DMG effect. [D_15]
28. Boot ROM DMG duration in cycles/frames: no pandocs figure for DMG; needs a measuring test ROM or the external disassembly. [D_16]
29. CONFLIT reset values F/TAC/OBP0/OBP1: specs 2001 (AF=$01B0 fixed, TAC=$00, OBP0=OBP1=$FF) vs Power_Up_Sequence.md (H/C checksum-dependent, TAC=$F8, OBP uninitialized); decidable by mooneye boot_regs-dmgABC.gb and boot_hwio-dmgABCmgb.gb. [D_16]
30. Per-ROM pass/fail signal of the 11 individual blargg cpu_instrs ROMs: only the combined capture is provided; UNKNOWN - to confirm by execution. [D_17]
31. dmg-acid2 execution duration: no figure in README, howto or pandocs; measure until opcode 0x40 (LD B,B). [D_17]
32. cgb-acid2 / cgb-acid-hell durations: no local .gb in v7.0, howto without figures; UNKNOWN - to confirm. [D_17]
