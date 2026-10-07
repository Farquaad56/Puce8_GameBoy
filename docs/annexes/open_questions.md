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

## D_08 - I/O registers FF00-FFFF : points a confirmer (2026-10-05)
Voir docs/annexes/notes/03b_io_registers.md.

- CONFLIT valeur power-up / lecture des bits 7-5 du registre IE ($FFFF) sur DMG : Power_Up_Sequence.md#Hardware-registers L332 donne $00 a PC=$0100, tandis que la regle generale IR.md L41 ("unused bits read high", confirme par IF=$E1 au meme endroit) implique >= $E0. A trancher par roms/test-roms/mooneye-test-suite (tests boot_hwio). Statut : CONFLIT - to confirm by running these ROMs.
- Valeur power-up d'OBP0 ($FF48) et OBP1 ($FF49) sur DMG : "left entirely uninitialized" selon Power_Up_Sequence.md L342-L345 (souvent $00 ou $FF, jamais fiable). Quelle valeur notre reset doit-il choisir ? Statut : UNKNOWN - to confirm.
- Valeur power-up de la Wave pattern RAM FF30-$FF3F sur DMG : aucune entree dans le tableau Power_Up_Sequence.md#Hardware-registers (ni enonce pandocs trouve par grep) ; aleatoire comme WRAM/HRAM, pattern fixe, ou ecrasee par la boot ROM ? Statut : UNKNOWN - to confirm.
- Valeur lue sur DMG des adresses I/O entierement non attribuees ($FF03, $FF08-$FF0E, $FF15, $FF27-$FF2F, $FF4E, $FF57-$FF67, $FF6D-$FF6F, $FF71, $FF78-$FFFE) et des registres CGB-only non marques [^cgb_only] dans le tableau ($FF4C KEY0/SYS, $FF68-$FF69, $FF6A-$FF6B, $FF6C OPRI) : pas d'enonce pandocs ; la regle "unused bits read high" (IR.md L41) suggere $FF mais n'est pas etablie explicitement pour ces adresses. Statut : UNKNOWN - to confirm.
- Comportement de lecture des registres write-only ($FF13 NR13, $FF18 NR23, $FF1B NR31, $FF1D NR33 sur DMG ; HDMA1-4 en CGB) : rendent-ils la derniere valeur ecrite ou $FF ? Le tableau power-up donne $FF pour FF13/FF18/FF1D mais n'explique pas le mecanisme. Statut : UNKNOWN - to confirm.

## D_09 - video registers : points a confirmer (2026-10-05)
Voir docs/annexes/notes/04a_video_regs.md.

- D_09 Regle d'increment de FF44 (LY) : refs/pandocs ne documente pas l'instant precis du frame ou LY passe a la valeur de la ligne suivante ; seuls sont etablis la portee 0-153 avec VBlank = 144-153 (STAT.md#FF44), la stabilite par ligne (CGB_Registers.md "Bit 7 = 1 — HBlank DMA", LY=0-143) et le test WY == LY en debut de scanline (Window.md#Window rendering criteria). A trancher par roms/test-roms/mooneye-test-suite/acceptance/ppu/stat_lyc_onoff.gb (+ vblank_stat_intr-GS.gb). Statut : UNKNOWN - to confirm.

## D_10 - video rendering : points a confirmer (2026-10-05)
Voir docs/annexes/notes/04b_video_render.md.

- D_10 Premiere ligne apres LCD on : refs/pandocs ne precise pas a quelle position de frame le PPU repart quand LCDC.7 repasse a 1 (LY reinitialisee a 0 ou continuation de la frame en cours), ni ce qui s'affiche exactement sur cette premiere ligne ; seuls sont etablis que le PPU repart immediatement et que l'ecran reste blanc pendant toute la premiere frame (LCDC.md#LCDC.7). A trancher par une test ROM qui re-activate LCDC.7 mid-frame and observe the first rendered line. Statut : UNKNOWN - to confirm.

## D_13 - Joypad et port serie : points a confirmer (2026-10-05)
Voir docs/annexes/notes/06_input_serial.md.

- Lecture des bits 7-6 du registre SC ($FF02) sur DMG : refs/pandocs ne documente que bit7 et bit0 ; la regle generale "unused bits read high" (IR.md L41) suggere $FE mais n'est pas etablie explicitement pour ce registre. A trancher par roms/test-roms/gambatte/serial/start_wait_read_sc_*.gbc (lisent SC apres un transfert). Statut : UNKNOWN - to confirm.
- C01_03 simplification documentee : l'ecriture de SC ($FF02) avec bit7 + bit0 (horloge interne) complete le transfert instantanement dans le bus (SB capture, bit7 efface d'un coup), au lieu des 4096 dots reels (note 06 "Duree d'un transfert"). La duree exacte et l'interrupt serie (IF/IE bit3) arrivent avec une tache serie ulterieure ; a trancher par roms/test-roms/gambatte/serial/. Statut : UNKNOWN - to confirm.

## D_14 - En-tete de cartouche : conflits et manquants (2026-10-05)
Voir docs/annexes/notes/07a_cart_header.md.

- CONFLIT codes MBC4 (0147 = $15/$16/$17) : historical/2001-Oct-pandocs.txt L2367-L2369 liste MBC4 / MBC4+RAM / MBC4+RAM+BATTERY, mais The_Cartridge_Header.md#0147 ne liste aucun code MBC4 (saut de $13 a $19). Aucune ROM test locale sous roms/test-roms/ ne trancher. Statut : CONFLIT - to confirm by a test ROM that reads 0147 on an MBC4 cartridge.
- CONFLIT valeur RAM size 0149 = $01 : historical/2001-Oct-pandocs.txt L2399 donne "2 KBytes", mais The_Cartridge_Header.md#0149 dit "unused" (aucune puce de 2K n'a jamais ete utilisee). Aucune ROM test locale ne trancher. Statut : CONFLIT - to confirm by a test ROM that reads 0149 on a cartridge with $01.
- E01_04 : le code RAM size 0149 = $01 est rejete par RomOnly::new (CartridgeLoadError::UnknownRamSize) car CONFLIT entre sources ; la valeur reelle (2 KiB ou absent) reste a trancher. Statut : UNKNOWN - to confirm.
- Comportement exact des valeurs 0147 = $08/$09 (ROM+RAM / ROM+RAM+BATTERY, jamais utilisees) : inconnu dans les sources. Statut : UNKNOWN - to confirm.

## D_15 - Mappers (MBC) : conflits et manquants (2026-10-05)
Voir docs/annexes/notes/07b_mappers.md.

- CONFLIT delai entre acces aux registres RTC du MBC3 : historical/2001-Oct-pandocs.txt L2618-L2620 recommande "4ms (4 Cycles in Normal Speed Mode)" tandis que refs/pandocs/src/MBC3.md#Delays dit "4 us (4 M-cycles in Normal Speed Mode)". A trancher par roms/test-roms/rtc3test (RTC MBC3). Statut : CONFLIT - to confirm by running this ROM.
- Comportement precis de l'acces a une banque RAM non mappee (wrap-around) sur chaque puce MBC : formule generale donnee dans refs/pandocs/src/MBCs.md#MBC-Unmapped-RAM-Bank-Access mais non detaillee par puce ; sans effet majeur sur le DMG. Statut : UNKNOWN - to confirm.

## D_16 - Boot and reset : points a confirmer (2026-10-05)
Voir docs/annexes/notes/08_boot_reset.md.

- Duree exacte de la boot ROM DMG en cycles ou frames : refs/pandocs ne donne aucun chiffre pour le modele DMG (seule la duree des boot ROM SGB est decrite comme dependante du header). A trancher par une test ROM qui mesure le temps entre power-up et PC=$0100, ou par la disassembly de la boot ROM DMG (ISSOtm/gb-bootroms, non clonee). Statut : UNKNOWN - to confirm.
- CONFLIT valeurs de reset F/TAC/OBP0/OBP1 entre les specs 2001 (AF=$01B0 fixes, TAC=$00, OBP0=OBP1=$FF) et Power_Up_Sequence.md (H/C dependants du checksum $014D, TAC=$F8, OBP0/OBP1 non initialises). A trancher par roms/test-roms/mooneye-test-suite/acceptance/boot_regs-dmgABC.gb et boot_hwio-dmgABCmgb.gb. Statut : CONFLIT - to confirm by running these ROMs.

## D_17 - Test ROM catalog : points a confirmer (2026-10-05)
Voir docs/annexes/notes/09_test_roms.md.

- Signal pass/echec des 11 cpu_instrs individual ROMs separement (seule la capture de l'ensemble blargg/cpu_instrs/cpu_instrs-dmg-cgb.png est fournie ; le howto ne decrit pas un signal par ROM individuelle). A trancher en executant les ROMs. Statut : UNKNOWN - to confirm.
- Duree d'execution de dmg-acid2 (ni README, ni howto, ni refs/pandocs ne donne de chiffre). A trancher en mesurant la duree jusqu'a l'opcode 0x40 (LD B,B) sur un emulator. Statut : UNKNOWN - to confirm.
- Durees d'execution des suites cgb-acid2 / cgb-acid-hell (aucune .gb locale dans v7.0 ; howto sans chiffre). Statut : UNKNOWN - to confirm.
