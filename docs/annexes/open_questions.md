# Questions ouvertes (UNKNOWN, conflits, taches bloquees)

## D_01 - Manquants de sources (2026-10-05)
Sujets sans source dans refs/pandocs ni roms/test-roms (voir docs/annexes/sources_inventory.md).

- Nombre de cycles par ligne / par frame (DMG) : Specifications.md donne la frequence maitresse (4.194304 MHz, system clock = 1/4) mais aucun doc Pan Docs ne donne le nombre de cycles par ligne ou par frame. Statut : UNKNOWN - to confirm.
- Table des cycles par instruction SM83 (micro-op = 1 bus access) : CPU_Instruction_Set.md ne liste pas les cycles par opcode. Statut : UNKNOWN - to confirm.
- Code complet du boot ROM DMG (disassembly inline) : Power_Up_Sequence.md donne les valeurs de registres post-boot mais renvoie vers une disassembly externe (ISSOtm/gb-bootroms, non clonee). Statut : UNKNOWN - to confirm.


## D_02 - Conflit opcodes illegaux (2026-10-05)
Opcodes.json (gbdev.io/gb-opcodes/Opcodes.json, copie docs/annexes/Opcodes.json) donne 4 dots pour les 11 opcodes illegaux (D3 DB DD E3 E4 EB EC ED F4 FC FD), alors que refs/pandocs/src/CPU_Comparison_with_Z80.md dit que le CPU SM83 veritable se lock up sur ces opcodes. Statut : CONFLIT - a trancher par une test ROM qui execute les 11 opcodes (aucune n'existe encore sous roms/test-roms/). Voir docs/annexes/notes/01_timing.md.

## D_03 - Timer : largeur du system counter UNKNOWN + CONFLIT TMA/TIMA (2026-10-05)
Voir docs/annexes/notes/01b_timer.md.

- Largeur exacte du "system counter" interne dont DIV n'est que la partie visible (8 bits bas = FF04) : Timer_Obscure_Behaviour.md ne dit pas combien de bits ce compteur a au total (ni s'il est 16-bit ou plus). Statut : UNKNOWN - to confirm.
- CONFLIT : si une ecriture CPU dans TMA tombe sur le meme M-cycle que le transfert automatique de TMA vers TIMA (overflow), Timer_and_Divider_Registers.md#FF06-TMA dit "the old value is transferred to TIMA" tandis que Timer_Obscure_Behaviour.md (Timer overflow behavior, point 3) dit que la nouvelle valeur est copie dans TIMA sur le meme M-cycle. A trancher par la test ROM mooneye-test-suite tma_write_reloading.gb (roms/test-roms/mooneye-test-suite/acceptance/timer/). Statut : CONFLIT - to confirm by running this ROM.
- D_03 blocked: note 01b_timer.md passes its own exit test (SCORE 1/1) but the scope check fails on pre-existing dirty files outside D_03 write scope: AGENTS.md and docs/tasks/D/D_02.md modified, plus untracked docs/annexes/Opcodes.json, docs/annexes/seed/, docs/annexes/timing_constantes.md (leftovers from the D_02 run). A human must commit or remove these before task.py done can pass; they are not part of D_03 and I may not touch them.
