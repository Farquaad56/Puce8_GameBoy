# Inventaire des sources

## E00_02 - Documentation et ROMs de test (2026-10-05)

| Source | URL | Commit / version | Notes |
|---|---|---|---|
| Pan Docs | https://github.com/gbdev/pandocs | `git rev-parse HEAD` = 0191af06ac49661587dcde3d57a241a626b8df75 (cloned 2026-10-05, depth 1) | Cloned into `refs/pandocs` (git-ignored). Hardware reference docs. |
| Game Boy test ROMs | https://github.com/c-sp/game-boy-test-roms | Repo HEAD = f8c3da8431dc60d752007bce571f50fd380b445e (master, cloned 2026-10-05, depth 1); compiled ROMs from release v7.0 archive `game-boy-test-roms-v7.0.zip` | Cloned into `roms/test-roms` (git-ignored). The repo itself contains no .gb files; the v7.0 release archive was downloaded and unzipped in place, adding 18 test suites (gambatte, gbmicrotest, mooneye-test-suite, blargg, same-suite, age-test-roms, etc.). |

## D_01 - Inventaire complet des sources (2026-10-05)

Legende : type = nature du fichier ; lignes = nombre de lignes (wc -l) ; fiabilite =
officielle / communautaire / testee sur ROM / deduite ; feeds note = note(s) que la
source alimente (voir docs/annexes/notes/). Les titres sont paraphrases, aucun texte
copie.

### Pan Docs (refs/pandocs) - 79 fichiers .md

| Fichier | Type | Lignes | Sujets | Fiabilite | Feeds note |
|---|---|---|---|---|---|
| `refs/pandocs/CONTRIBUTING.md` | Pan Docs (md) | 289 | contribution guide | communautaire | - |
| `refs/pandocs/DEPLOY.md` | Pan Docs (md) | 163 | site deployment | communautaire | - |
| `refs/pandocs/README.md` | Pan Docs (md) | 15 | Pan Docs index | communautaire | - |
| `refs/pandocs/src/404.md` | Pan Docs (md) | 1 | site 404 page | communautaire | - |
| `refs/pandocs/src/About.md` | Pan Docs (md) | 34 | project overview | communautaire | - |
| `refs/pandocs/src/Accessing_VRAM_and_OAM.md` | Pan Docs (md) | 92 | VRAM/OAM access rules, OAM lock window | communautaire | 04a_video_regs, 03b_io_registers |
| `refs/pandocs/src/Audio.md` | Pan Docs (md) | 120 | audio overview (channels, wave/noise) | communautaire | 05a_audio_channels |
| `refs/pandocs/src/Audio_Registers.md` | Pan Docs (md) | 423 | audio registers NR1-NR5 | communautaire | 05a_audio_channels, 03b_io_registers |
| `refs/pandocs/src/Audio_details.md` | Pan Docs (md) | 273 | audio channel details, output filter @4194304 Hz | communautaire | 05a_audio_channels, 05b_audio_mixing |
| `refs/pandocs/src/Authors.md` | Pan Docs (md) | 16 | authors | communautaire | - |
| `refs/pandocs/src/CGB_Registers.md` | Pan Docs (md) | 305 | CGB-only registers (palettes, BG/OBJ palettes) | communautaire | 04a_video_regs, 03b_io_registers (CGB variant) |
| `refs/pandocs/src/CPU_Comparison_with_Z80.md` | Pan Docs (md) | 72 | SM83 vs Z80 differences | communautaire | 02a_cpu_core |
| `refs/pandocs/src/CPU_Instruction_Set.md` | Pan Docs (md) | 192 | CPU instruction set / opcodes | communautaire | 02a_cpu_core, 02c_cpu_opcodes_src |
| `refs/pandocs/src/CPU_Registers_and_Flags.md` | Pan Docs (md) | 57 | CPU registers and flags | communautaire | 02a_cpu_core |
| `refs/pandocs/src/External_Connectors.md` | Pan Docs (md) | 61 | external connectors (peripheral) | communautaire | - |
| `refs/pandocs/src/Four_Player_Adapter.md` | Pan Docs (md) | 328 | 4-player adapter (peripheral) | communautaire | - |
| `refs/pandocs/src/GBC_Approval_Process.md` | Pan Docs (md) | 36 | GBC approval process | communautaire | - |
| `refs/pandocs/src/Gameboy_Camera.md` | Pan Docs (md) | 566 | Game Boy Camera (peripheral) | communautaire | - |
| `refs/pandocs/src/Gameboy_Printer.md` | Pan Docs (md) | 135 | Game Boy Printer (peripheral) | communautaire | - |
| `refs/pandocs/src/Graphics.md` | Pan Docs (md) | 85 | graphics overview | communautaire | 04b_video_render, 04a_video_regs |
| `refs/pandocs/src/Hardware_Reg_List.md` | Pan Docs (md) | 128 | hardware register list (I/O map) | communautaire | 03b_io_registers |
| `refs/pandocs/src/History.md` | Pan Docs (md) | 21 | hardware history | communautaire | - |
| `refs/pandocs/src/HuC1.md` | Pan Docs (md) | 53 | (unmapped) | communautaire | - |
| `refs/pandocs/src/HuC3.md` | Pan Docs (md) | 166 | (unmapped) | communautaire | - |
| `refs/pandocs/src/IR.md` | Pan Docs (md) | 65 | infrared port (peripheral) | communautaire | - |
| `refs/pandocs/src/Interrupt_Sources.md` | Pan Docs (md) | 81 | interrupt sources, IME, IF flags | communautaire | 02b_cpu_interrupts |
| `refs/pandocs/src/Interrupts.md` | Pan Docs (md) | 95 | interrupt handling / vector table | communautaire | 02b_cpu_interrupts |
| `refs/pandocs/src/Joypad_Input.md` | Pan Docs (md) | 33 | joypad input register P1 | communautaire | 06_input_serial, 03b_io_registers |
| `refs/pandocs/src/LCDC.md` | Pan Docs (md) | 135 | LCD controller registers LCDC/SCY/SCX/BG/NW/OB | communautaire | 04a_video_regs |
| `refs/pandocs/src/M161.md` | Pan Docs (md) | 43 | M161 mapper (Nintendo) | communautaire | 07b_mappers |
| `refs/pandocs/src/MBC1.md` | Pan Docs (md) | 208 | MBC1 mapper | communautaire | 07b_mappers |
| `refs/pandocs/src/MBC2.md` | Pan Docs (md) | 55 | MBC2 mapper | communautaire | 07b_mappers |
| `refs/pandocs/src/MBC3.md` | Pan Docs (md) | 103 | MBC3 mapper (RTC/RAM) | communautaire | 07b_mappers, 01b_timer |
| `refs/pandocs/src/MBC5.md` | Pan Docs (md) | 61 | MBC5 mapper (+rumble) | communautaire | 07b_mappers |
| `refs/pandocs/src/MBC6.md` | Pan Docs (md) | 178 | MBC6 mapper | communautaire | 07b_mappers |
| `refs/pandocs/src/MBC7.md` | Pan Docs (md) | 145 | MBC7 mapper (accelerometer) | communautaire | 07b_mappers |
| `refs/pandocs/src/MBCs.md` | Pan Docs (md) | 28 | mapper overview | communautaire | 07b_mappers |
| `refs/pandocs/src/MMM01.md` | Pan Docs (md) | 415 | MMM01 mapper (Nintendo) | communautaire | 07b_mappers |
| `refs/pandocs/src/Memory_Map.md` | Pan Docs (md) | 162 | memory map / address regions, boot ROM mapping | communautaire | 03a_memory_map, 01_timing, 08_boot_reset |
| `refs/pandocs/src/OAM.md` | Pan Docs (md) | 133 | OAM / sprite memory layout | communautaire | 04a_video_regs, 03a_memory_map |
| `refs/pandocs/src/OAM_Corruption_Bug.md` | Pan Docs (md) | 111 | OAM corruption bug (line 91/92) | communautaire | 04b_video_render |
| `refs/pandocs/src/OAM_DMA_Transfer.md` | Pan Docs (md) | 95 | OAM DMA transfer (OB) | communautaire | 04a_video_regs, 03b_io_registers |
| `refs/pandocs/src/Palettes.md` | Pan Docs (md) | 143 | DMG palettes / BGP/OGP | communautaire | 04a_video_regs, 04b_video_render |
| `refs/pandocs/src/Power_Up_Sequence.md` | Pan Docs (md) | 411 | power-up/reset sequence, post-boot register values | communautaire | 08_boot_reset, 03a_memory_map |
| `refs/pandocs/src/Reducing_Power_Consumption.md` | Pan Docs (md) | 128 | low-power modes (STOP/HALT) | communautaire | 02b_cpu_interrupts, 01_timing |
| `refs/pandocs/src/References.md` | Pan Docs (md) | 19 | source references (incl. boot ROM disassembly link) | communautaire | - |
| `refs/pandocs/src/Rendering.md` | Pan Docs (md) | 74 | frame rendering overview / LCD modes | communautaire | 04b_video_render |
| `refs/pandocs/src/SGB_Color_Palettes.md` | Pan Docs (md) | 77 | SGB palettes | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Attribute.md` | Pan Docs (md) | 145 | SGB attribute command | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Border.md` | Pan Docs (md) | 100 | SGB border command | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Multiplayer.md` | Pan Docs (md) | 49 | SGB multiplayer command | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Packet.md` | Pan Docs (md) | 59 | SGB packet format | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Palettes.md` | Pan Docs (md) | 115 | SGB palette commands | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Prototype.md` | Pan Docs (md) | 98 | SGB prototype command | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Sound.md` | Pan Docs (md) | 174 | SGB sound command | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Summary.md` | Pan Docs (md) | 32 | SGB command summary | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_System.md` | Pan Docs (md) | 158 | SGB system command | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Command_Undocumented.md` | Pan Docs (md) | 13 | SGB undocumented commands | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Functions.md` | Pan Docs (md) | 104 | SGB functions overview | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_Unlocking.md` | Pan Docs (md) | 55 | SGB unlock sequence | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/SGB_VRAM_Transfer.md` | Pan Docs (md) | 43 | SGB VRAM transfer | communautaire | - (Super Game Boy) |
| `refs/pandocs/src/STAT.md` | Pan Docs (md) | 42 | STAT register / line interrupts | communautaire | 04a_video_regs, 02b_cpu_interrupts |
| `refs/pandocs/src/SUMMARY.md` | Pan Docs (md) | 100 | doc summary index | communautaire | - |
| `refs/pandocs/src/Scrolling.md` | Pan Docs (md) | 28 | scroll registers SCX/SCY | communautaire | 04a_video_regs, 04b_video_render |
| `refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md` | Pan Docs (md) | 110 | serial / link cable transfer (SC) | communautaire | 06_input_serial, 03b_io_registers |
| `refs/pandocs/src/Shark_Cheats.md` | Pan Docs (md) | 40 | cheat codes | communautaire | - |
| `refs/pandocs/src/Specifications.md` | Pan Docs (md) | 77 | hardware specifications / clocks | communautaire | 01_timing, 03a_memory_map |
| `refs/pandocs/src/The_Cartridge_Header.md` | Pan Docs (md) | 454 | cartridge header layout (logo, flags) | communautaire | 07a_cart_header |
| `refs/pandocs/src/Tile_Data.md` | Pan Docs (md) | 100 | tile data format | communautaire | 04b_video_render, 03a_memory_map |
| `refs/pandocs/src/Tile_Maps.md` | Pan Docs (md) | 113 | tile map / BG windows | communautaire | 04a_video_regs, 04b_video_render |
| `refs/pandocs/src/Timer_Obscure_Behaviour.md` | Pan Docs (md) | 94 | timer obscure behaviour | communautaire | 01b_timer |
| `refs/pandocs/src/Timer_and_Divider_Registers.md` | Pan Docs (md) | 64 | timer/divider registers (DIV/TIMA/TIM0) | communautaire | 01b_timer, 03b_io_registers |
| `refs/pandocs/src/Window.md` | Pan Docs (md) | 54 | window layer registers (WY/WX) | communautaire | 04a_video_regs, 04b_video_render |
| `refs/pandocs/src/halt.md` | Pan Docs (md) | 35 | HALT instruction behaviour / halt bug | communautaire | 02b_cpu_interrupts |
| `refs/pandocs/src/nombc.md` | Pan Docs (md) | 8 | no-MBC ROMs | communautaire | 07b_mappers |
| `refs/pandocs/src/othermbc.md` | Pan Docs (md) | 113 | (unmapped) | communautaire | - |
| `refs/pandocs/src/partials/dandocs_notice.md` | Pan Docs (md) | 1 | docs notice partial | communautaire | - |
| `refs/pandocs/src/pixel_fifo.md` | Pan Docs (md) | 260 | pixel FIFO / pixel fetcher (mode 3) | communautaire | 01_timing, 04b_video_render |
| `refs/pandocs/src/single.md` | Pan Docs (md) | 1 | single-page build stub | communautaire | - |

### Suites de ROMs de test (roms/test-roms/) - 18 suites

| Suite | Type | Lignes | Sujets | Fiabilite | Feeds note |
|---|---|---|---|---|---|
| `roms/test-roms/age-test-roms/` | test ROM suite (47 .gb) | - | AGE test roms: CPU/graphics/audio edge cases | testee sur ROM | 02a_cpu_core, 04b_video_render, 05a_audio_channels |
| `roms/test-roms/blargg/` | test ROM suite (58 .gb) | - | Blargg: cpu_instrs, instr_timing, mem_timing, interrupt_time, halt_bug, sound (DMG/CGB) | testee sur ROM | 01_timing, 02a_cpu_core, 02b_cpu_interrupts, 05a_audio_channels |
| `roms/test-roms/bully/` | test ROM suite (1 .gb) | - | Bully: echo RAM / memory-map stress test | testee sur ROM | 03a_memory_map |
| `roms/test-roms/cgb-acid2/` | test ROM suite (0 .gb) | - | cgb-acid2: CGB feature stress (palette, VRAM bank) | testee sur ROM | - (CGB variant) |
| `roms/test-roms/cgb-acid-hell/` | test ROM suite (0 .gb) | - | cgb-acid-hell: CGB edge-case stress | testee sur ROM | - (CGB variant) |
| `roms/test-roms/dmg-acid2/` | test ROM suite (1 .gb) | - | dmg-acid2: DMG feature/edge-case stress | testee sur ROM | 04b_video_render, 03a_memory_map, 05a_audio_channels |
| `roms/test-roms/gambatte/` | test ROM suite (102 .gb) | - | Gambatte test suite (CPU/video/audio) | testee sur ROM | 02a_cpu_core, 04b_video_render, 05a_audio_channels |
| `roms/test-roms/gbmicrotest/` | test ROM suite (513 .gb) | - | GBMicrotest: micro-behaviour of OAM/VRAM/TIMA etc. | testee sur ROM | 04a_video_regs, 01b_timer, 03a_memory_map |
| `roms/test-roms/little-things-gb/` | test ROM suite (2 .gb) | - | little-things-gb: first-white / telling-ly's visual tests | testee sur ROM | 04b_video_render |
| `roms/test-roms/mbc3-tester/` | test ROM suite (1 .gb) | - | MBC3 Tester: MBC3 RTC/RAM behaviour | testee sur ROM | 07b_mappers, 01b_timer |
| `roms/test-roms/mealybug-tearoom-tests/` | test ROM suite (35 .gb) | - | Mealybug Tearoom Tests: CGB/DMG edge cases (screenshot-based) | testee sur ROM | - (CGB variant), 04b_video_render |
| `roms/test-roms/mooneye-test-suite/` | test ROM suite (115 .gb) | - | Mooneye Test Suite: CPU/video/audio correctness (Fibonacci pass code) | testee sur ROM | 02a_cpu_core, 04b_video_render, 05a_audio_channels |
| `roms/test-roms/mooneye-test-suite-wilbertpol/` | test ROM suite (121 .gb) | - | Mooneye (wilbertpol fork): extended logic-analysis tests | testee sur ROM | 02a_cpu_core, 04b_video_render |
| `roms/test-roms/rtc3test/` | test ROM suite (1 .gb) | - | rtc3test: MBC3 RTC accuracy | testee sur ROM | 07b_mappers, 01b_timer |
| `roms/test-roms/same-suite/` | test ROM suite (78 .gb) | - | SameSuite: CPU/video/audio correctness (Fibonacci pass code) | testee sur ROM | 02a_cpu_core, 04b_video_render, 05a_audio_channels |
| `roms/test-roms/scribbltests/` | test ROM suite (8 .gb) | - | Scribbltests: visual/edge-case tests (screenshot-based) | testee sur ROM | 04b_video_render |
| `roms/test-roms/strikethrough/` | test ROM suite (1 .gb) | - | Strikethrough: single-rom edge-case test | testee sur ROM | 02a_cpu_core, 04b_video_render |
| `roms/test-roms/turtle-tests/` | test ROM suite (2 .gb) | - | TurtleTests: window-Y trigger / off-screen WX behaviour | testee sur ROM | 04a_video_regs, 04b_video_render |

### Fichiers de niveau repo (signal pass/echec par suite)

| Fichier | Type | Lignes | Sujets | Fiabilite | Feeds note |
|---|---|---|---|---|---|
| `roms/test-roms/README.md` | repo README (128 lines) | 128 | lists all suites + per-suite howto links; exit-condition/success-failure model | communautaire | 09_test_roms |
| `roms/test-roms/src/howto/*.md` | per-suite how-to (18 files, ~570 lines) | - | each suite's device compatibility, exit condition, success/failure signal | communautaire | 09_test_roms |

Signal pass/echec (paraphrase des howto, voir roms/test-roms/src/howto/*.md) :
- age-test-roms: auto tests set a known value; any different value = failure (some are manual).
- blargg: run for N emulated seconds then compare register/checksum to expected.
- bully: fails on DMG-C with 'Bad Echo RAM Reads' (echo-RAM stress).
- cgb-acid2 / cgb-acid-hell / dmg-acid2: screenshot comparison against reference images.
- gambatte: per-test success/failure determined by the suite's own check (see howto).
- gbmicrotest: 0xFF82 == 0x01 on pass, 0xFF on fail (some tests use 0xFF80==0xFF81 on failure).
- little-things-gb: requires emulating all button presses; then a 'pass' screen is shown.
- mbc3-tester: success/failure determined by screenshot comparison.
- mealybug-tearoom-tests: screenshot comparison only.
- mooneye-test-suite / -wilbertpol: CPU registers hold Fibonacci B=3,C=5,D=8,E=13,H=21,L=34 on pass; opcode 0x40 (LD B,B) marks end; max runtime 120 s. manual-only/sprite_priority.gb is screenshot-based.
- rtc3test: RTC accuracy check (see howto).
- same-suite: CPU registers hold Fibonacci B=3,C=5,D=8,E=13,H=21,L=34 on pass; opcode 0x40 marks end.
- scribbltests: screenshot comparison (no screenshots yet for failrylake, winpos).
- strikethrough: single-rom edge-case check (see howto).
- turtle-tests: window-Y trigger / off-screen WX behaviour check (see howto).

### Manquants (aucune source dans refs/pandocs ni roms/test-roms)

Sujets sans source dediee ; ajoutes a docs/annexes/open_questions.md.

| Sujet | Statut |
|---|---|
| Nombre de cycles par ligne / par frame (DMG) - Specifications.md donne la frequence maitresse (4.194304 MHz, system clock = 1/4) mais aucun doc Pan Docs ne donne le nombre de cycles par ligne ou par frame | UNKNOWN - to confirm |
| Table des cycles par instruction SM83 (micro-op = 1 bus access) - CPU_Instruction_Set.md ne liste pas les cycles par opcode | UNKNOWN - to confirm |
| Code complet du boot ROM DMG (disassembly inline) - Power_Up_Sequence.md donne les valeurs de registres post-boot mais renvoie vers une disassembly externe (ISSOtm/gb-bootroms, non clonee) | UNKNOWN - to confirm |

