# Questions ouvertes (UNKNOWN, conflits, taches bloquees)

## D_01 - Manquants de sources (2026-10-05)
Sujets sans source dans refs/pandocs ni roms/test-roms (voir docs/annexes/sources_inventory.md).

- Nombre de cycles par ligne / par frame (DMG) : Specifications.md donne la frequence maitresse (4.194304 MHz, system clock = 1/4) mais aucun doc Pan Docs ne donne le nombre de cycles par ligne ou par frame. Statut : UNKNOWN - to confirm.
- Table des cycles par instruction SM83 (micro-op = 1 bus access) : CPU_Instruction_Set.md ne liste pas les cycles par opcode. Statut : UNKNOWN - to confirm.
- Code complet du boot ROM DMG (disassembly inline) : Power_Up_Sequence.md donne les valeurs de registres post-boot mais renvoie vers une disassembly externe (ISSOtm/gb-bootroms, non clonee). Statut : UNKNOWN - to confirm.


## D_02 - Conflit opcodes illegaux (2026-10-05)
Opcodes.json (gbdev.io/gb-opcodes/Opcodes.json, copie docs/annexes/Opcodes.json) donne 4 dots pour les 11 opcodes illegaux (D3 DB DD E3 E4 EB EC ED F4 FC FD), alors que refs/pandocs/src/CPU_Comparison_with_Z80.md dit que le CPU SM83 veritable se lock up sur ces opcodes. Statut : CONFLIT - a trancher par une test ROM qui execute les 11 opcodes (aucune n'existe encore sous roms/test-roms/). Voir docs/annexes/notes/01_timing.md.
