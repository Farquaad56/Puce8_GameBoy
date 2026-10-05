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

## D_05 - CPU core : placement des bits du registre F a confirmer sur le materiel (2026-10-05)
Voir docs/annexes/notes/02a_cpu_core.md. Les deux sources pandocs concordent entre elles (src/CPU_Registers_and_Flags.md#The-Flags-Register L17-L24 et historical/2001-Oct-pandocs.txt L2087-L2093) : z=bit 7, n=bit 6, h=bit 5, c=bit 4 du registre F, bits 3-0 "not used (always zero)". Ce placement contredit l'enonce du task D_05 ("low nibble of F") ; aucune source du corpus ne confirme les positions de bits sur le materiel. Statut : CONFIRME au niveau des sources - to confirm on hardware via blargg cpu_instrs flag tests (roms/test-roms/blargg/cpu_instrs).

## D_06 - Interrupts CPU / HALT bug / STOP : points a confirmer (2026-10-05)
Voir docs/annexes/notes/02b_cpu_interrupts.md.

- Frontiere exacte en cycles du delai de ei ("retarde d'une instruction" ; un halt suivant immediatement un ei voit encore IME=0) : pendant ou apres l'execution de l'instruction suivante ? ROM decisive locale : roms/test-roms/same-suite/interrupt/ei_delay_halt.gb. Statut : UNKNOWN - to confirm.
- Cout exact en cycles avant que STOP n'entree en veille (table 2001 marque "?") ; refs/pandocs ne donne aucun chiffre, le diagramme Halphon reste une image. Pas de ROM test locale dediee trouvee sous roms/test-roms/. Statut : UNKNOWN - to confirm.
- Comportement DMG des opcodes $93 et $FF (variantes halt-like non documentees) ; la liste des opcodes invalides qui verrouillent le CPU (CPU_Instruction_Set.md L173) ne les inclut pas. Pas de ROM test locale dediee trouvee sous roms/test-roms/ (blargg/cpu_instrs peut couvrir une partie). Statut : UNKNOWN - to confirm.
- Point exact ou IME=0 devient visible par rapport a l'instruction suivant un di ; refs/pandocs ne precise que qu'aucun interrupt n'est pris entre un ei et un di consecutifs. Statut : UNKNOWN - to confirm.

## D_07 - Memory map : points a confirmer (2026-10-05)
Voir docs/annexes/notes/03a_memory_map.md.

- Le fetch de l'opcode suivant chevauche-t-il le dernier M-cycle de l'instruction precedente ? refs/pandocs ne dit rien ; seule indication indirecte dans OAM_DMA_Transfer.md#best-practices (le DMA "starts right after instruction" et la variante ret z evite une lecture du stack sur le dernier M-cycle du DMA). Pas de ROM test locale dediee trouvee sous roms/test-roms/ (blargg/mem_timing ou mooneye-test-suite peuvent couvrir l'interleaving CPU/bus). Statut : UNKNOWN - to confirm.
- CONFLIT plage de source du OAM DMA ($FF46) : pandocs donne XX = $00 a $DF (source 0x0000-0xDFFF) tandis que les specs officielles donnent "$0000-$F19F" avec pas de 0x100. A trancher par roms/test-roms/gbmicrotest/dma_0x9000.gb et dma_0xE000.gb. Statut : CONFLIT - to confirm by running these ROMs.
- CONFLIT duree du OAM DMA : pandocs dit 160 M-cycles (= 640 dots a vitesse normale), les specs officielles disent "takes 160 nano-seconds" (1995) / "microseconds" (1998). A trancher par roms/test-roms/mooneye-test-suite/acceptance/oam_dma_timing.gb (+ gbmicrotest/dma_timing_a.gb). Statut : CONFLIT - to confirm by running this ROM.
