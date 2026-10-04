# Questions ouvertes (UNKNOWN, conflits, taches bloquees)

## D_01 - Manquants de sources (2026-10-05)
Sujets sans source dans refs/pandocs ni roms/test-roms (voir docs/annexes/sources_inventory.md).

- Nombre de cycles par ligne / par frame (DMG) : Specifications.md donne la frequence maitresse (4.194304 MHz, system clock = 1/4) mais aucun doc Pan Docs ne donne le nombre de cycles par ligne ou par frame. Statut : UNKNOWN - to confirm.
- Table des cycles par instruction SM83 (micro-op = 1 bus access) : CPU_Instruction_Set.md ne liste pas les cycles par opcode. Statut : UNKNOWN - to confirm.
- Code complet du boot ROM DMG (disassembly inline) : Power_Up_Sequence.md donne les valeurs de registres post-boot mais renvoie vers une disassembly externe (ISSOtm/gb-bootroms, non clonee). Statut : UNKNOWN - to confirm.

