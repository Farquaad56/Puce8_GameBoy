# Hint sheet - DMG timing values to CHECK (not a source)

RULES: every value below must be re-found in refs/pandocs with the grep given.
Found => write it with a Source line (pandocs file#section). Not found after 3 greps => Statut UNKNOWN - to confirm.
Never cite this sheet. Never write a value that grep did not show.

## Values for task D_02 (DMG only, ignore double speed)
| Value | Expected | grep string to try |
|---|---|---|
| Master clock | 4194304 Hz (4.194304 MHz) | grep -rn -i "4194304\|4.194304" refs/pandocs |
| Dot = T-cycle | 1 dot = 1 T-cycle | grep -rn -i "dot" refs/pandocs \| head -20 |
| M-cycle | 4 T-cycles (1048576 Hz) | grep -rn -i "m-cycle\|machine cycle\|1048576" refs/pandocs |
| Dots per line | 456 | grep -rn "456" refs/pandocs \| head -20 |
| Lines per frame | 154 (144 visible + 10 VBlank, LY 144-153) | grep -rn -i "154\|vblank" refs/pandocs \| head -20 |
| Dots per frame | 70224 (456 x 154) | grep -rn "70224" refs/pandocs |
| Refresh rate | about 59.7275 Hz (4194304 / 70224) | grep -rn -i "59\.7\|refresh" refs/pandocs |
| Mode 2 (OAM scan) | 80 dots | grep -rn -i "OAM scan" refs/pandocs |
| Mode 3 (drawing) | 172 to 289 dots, variable | grep -rn -i "172\|289" refs/pandocs |
| Mode 0 (HBlank) | rest of the line, up to 376 minus mode 3 | grep -rn -i "376" refs/pandocs |
| Mode 1 (VBlank) | 4560 dots (10 lines x 456) | grep -rn "4560" refs/pandocs |
| DIV timer | 16384 Hz | grep -rn -i "16384" refs/pandocs |

Arithmetic check (do it, it is a free test): 456 x 154 = 70224 ; 4194304 / 70224 = 59.7275...

## Instruction cycles (for later CPU tasks, source: gbdev Opcodes.json)
Unit = T-cycles. Conditional ops list two values [taken, not taken]:
JR cc 12/8 ; JP cc 16/12 ; CALL cc 24/12 ; RET cc 20/8.
Unconditional: JR 12, JP a16 16, JP HL 4, CALL 24, RET 16, RETI 16, RST 16.
CB prefix cost is ALREADY included in the cbprefixed table: register 8, (HL) 16, BIT n,(HL) 12.
Do not add the 4 cycles of the 0xCB PREFIX entry on top.
Illegal opcodes (D3 DB DD E3 E4 EB EC ED F4 FC FD): the JSON says 4, real hardware locks up.

## Common timing bugs
1. T-cycles mixed with M-cycles. 2. Taken/not-taken value swapped. 3. CB prefix counted twice.
4. (HL) CB op at 8 instead of 16 (or 12 for BIT).
