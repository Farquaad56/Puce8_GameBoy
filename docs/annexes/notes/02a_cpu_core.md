# Note 02a - CPU core SM83 : registres, flags, DAA, reset

Sources (grep + sed) : refs/pandocs/src/CPU_Registers_and_Flags.md,
refs/pandocs/src/CPU_Instruction_Set.md, refs/pandocs/src/CPU_Comparison_with_Z80.md,
refs/pandocs/src/Power_Up_Sequence.md, refs/pandocs/historical/2001-Oct-pandocs.txt.
Les cycles des tables historiques sont des T-cycles (M-cycle = T/4, voir note 01).

## Registres : AF BC DE HL SP PC, dont quatre coupables en deux octets
Fait : La SM83 possede six registres 16-bit : AF (accumulateur + flags), BC, DE, HL, SP (stack pointer) et PC. Seuls AF, BC, DE, HL s'accedent aussi comme paires d'octets (A/F, B/C, D/E, H/L) ; SP et PC ne se coupent pas en deux.
Source : refs/pandocs/src/CPU_Registers_and_Flags.md#Registers (L5-L15)
Fiabilite : communautaire
Impact code : puce8gb-core - struct des registres CPU + acces 8/16 bit par paire.
Statut : CONFIRME

## Bits du registre de flags F : z n h c en bits 7 a 4, bits 3-0 inutilises
Fait : Le flag register (bas de AF) porte z au bit 7, n au bit 6, h au bit 5, cy au bit 4 ; la source historique ajoute que les bits 3-0 sont "not used (always zero)". Les deux sources pandocs concordent sur ce placement (le task D_05 evocant le nibble bas : voir open_questions pour confirmation materielle).
Source : refs/pandocs/src/CPU_Registers_and_Flags.md#The-Flags-Register (L17-L24) ; refs/pandocs/historical/2001-Oct-pandocs.txt (L2087-L2093)
Fiabilite : communautaire
Impact code : puce8gb-core - masques de bits du registre F dans ALU, DAA, reset.
Statut : CONFIRME

## Signification des flags Z, C, N, H
Fait : Z est set si et seulement si le resultat vaut zero (sert aux jumps conditionnels). C est set quand un ajout 8-bit depasse $FF ou un ajout 16-bit depasse $FFFF, quand une soustraction/comparaison donne moins que zero, ou quand un rotate/shift ejecte un "1". N et H ne servent qu'a DAA : N indique l'operation de soustraction precedente, H le half carry des 4 bits bas du resultat.
Source : refs/pandocs/src/CPU_Registers_and_Flags.md#The-Zero-Flag + #The-Carry-Flag + #The-BCD-Flags (L29-L50) ; historique (L2087-L2120)
Fiabilite : communautaire
Impact code : puce8gb-core - calcul des flags dans les ops ALU, CP et rotate/shift.
Statut : CONFIRME

## Paires 16-bit r16 et push/pop sur {bc de hl af}
Fait : Les groupements distinguent r8 (b c d e h l [hl] a), r16 (bc de hl sp) et r16stk (bc de hl af). Seules les paires bc/de/hl/af peuvent etre push/pop ; push fait SP=SP-2 puis ecrit (16 T-cycles), pop lit puis SP=SP+2 (12 T-cycles).
Source : refs/pandocs/src/CPU_Instruction_Set.md#CPU-Instruction-Set (L20-L30, L145-L147) ; historique (L2154-L2157)
Fiabilite : communautaire
Impact code : puce8gb-core - decode push/pop r16stk et gestion de pile.
Statut : CONFIRME

## POP AF restaure F depuis la pile sans calculer les flags
Fait : pop rr (0xCx, 12 T-cycles) pose rr=(SP), SP=SP+2 ; la colonne flags porte "(AF)", c'est-a-dire que quand on pop AF le registre de flags est restaure tel quel depuis la pile, aucun flag n'etant recalcule.
Source : refs/pandocs/historical/2001-Oct-pandocs.txt (L2156) ; refs/pandocs/src/CPU_Instruction_Set.md#block-3 (L145-L147)
Fiabilite : communautaire
Impact code : puce8gb-core - pop af = restauration de F, a ne pas passer par le calculeur de flags.
Statut : CONFIRME

## Differences SM83 vs Z80 / 8080
Fait : La CPU se rapproche plutot du 8080 (JR supporte, presque tous les opcodes CB). Pas de bus I/O ni d'IN/OUT : les ports passent par des LD classiques ou par $E0/$F0 ($FF00+n) et $E2/$F2 ($FF00+C). Absents : sign et parity flags (et leurs RET/CALL/JP conditionnels), EX (SP),HL, EX DE,HL, second register set, prefixes DD/FD (IX/IY), tous les opcodes ED ; les block instructions sont remplaces par les auto-incrementing HL accesses [hl+]/[hl-] ($2A/$3A LDI, $22/$32 LDD).
Source : refs/pandocs/src/CPU_Comparison_with_Z80.md#comparison-with-8080 + #comparison-with-Z80 (L5-L32), table (L41-L69) ; historique (section CPU Comparison with Z80, ~L2257+)
Fiabilite : communautaire
Impact code : puce8gb-core - decode strictement SM83 ; ne jamais importer de mnemonic Z80.
Statut : CONFIRME

## Cadence : ~4 MHz comme une Z80, durees multiples de 4 cycles
Fait : La Game Boy CPU opere a la vitesse d'une Z80 ~4 MHz (8 MHz en CGB double speed) et toutes les durees d'instruction sont arrondies au multiple de 4 cycles ; la doc historique donne la frequence 4.194304 MHz.
Source : refs/pandocs/src/CPU_Comparison_with_Z80.md#comparison-with-Z80 (L34-L36) ; historique (L76, L2126-L2127)
Fiabilite : communautaire
Impact code : puce8gb-core - 1 M-cycle = 4 T-cycles (convention deja enote dans note 01).
Statut : CONFIRME

## DAA (opcode $27, 4 T-cycles) : ajustement BCD
Fait : N, H et C sont utilises uniquement par DAA. Apres ajout/soustraction de deux nombres BCD ($00-$99), DAA convertit le resultat en forme BCD ; il est inefficace pour les ops 16-bit (4 chiffres) et ses usages avec INC/DEC sont limits car ces ops n'affectent pas C.
Source : refs/pandocs/src/CPU_Registers_and_Flags.md#The-BCD-Flags (L46-L57) ; historique (L2109-L2120, ligne daa L2188)
Fiabilite : communautaire
Impact code : puce8gb-core - implementation de DAA a partir des flags N/H/C.
Statut : CONFIRME

## CONFLIT : C-flag de DAA, "upper 4 bits" vs "upper 8bits"
Fait : La doc courante dit que le C flag "must indicate carry for the upper 4 bits", la version historique dit "carry for upper 8bits". Les deux valeurs sont documentees ; l'implementation doit choisir selon un test ROM.
Deciding test ROM : blargg cpu_instrs (roms/test-roms/blargg/cpu_instrs).
Source : refs/pandocs/src/CPU_Registers_and_Flags.md#The-BCD-Flags (L50-L51) vs refs/pandocs/historical/2001-Oct-pandocs.txt (L2113)
Fiabilite : communautaire
Impact code : puce8gb-core - correction BCD de DAA (quels carries a traiter).
Statut : CONFLIT

## ADD SP,e ($E8) et LD HL,SP+e ($F8) : e signe 8 bits, flags "00hc"
Fait : $E8 add sp,imm8 vaut 16 T-cycles, $F8 ld hl,sp+imm8 12 ; l'octane est un nombre signe sur 8 bits (ajout ou soustraction). La table historique porte "00hc" pour les deux : Z et N remis a zero, H et C calcules ; add HL,rr ($09 family) porte "-0hc".
Source : refs/pandocs/src/CPU_Instruction_Set.md#block-3 (L163-L164) ; historique (L2192-L2196)
Fiabilite : communautaire
Impact code : puce8gb-core - regles de flags des deux ops basees sur SP.
Statut : CONFIRME

## Blocs d'opcodes, cas HALT/STOP, prefixe CB
Fait : L'espace des opcodes est decoupe en 4 blocs sur les 2 bits hauts : Block 0 = ld r16/r8/mem, inc/dec r16/r8, add hl,r16, ops un octet (dont DAA $27, CPL $2F, SCF $37, CCF $3F), JR et STOP ; Block 1 = ld r8,r8, avec HALT $76 qui est l'encodage de `ld [hl],[hl]` inexistante ; Block 2 = ALU a,r8 (add/adc/sub/sbc/and/xor/or/cp) ; Block 3 = ALU a,imm8, RET/RETI/JP/CALL/RST, push/pop, prefixe CB $CB. STOP $10 est souvent traite en deux octets mais le second n'est pas toujours ignore.
Source : refs/pandocs/src/CPU_Instruction_Set.md#CPU-Instruction-Set (L42-L43, L44, L88-L92, L94-L103, L106, L119, L150)
Fiabilite : communautaire
Impact code : puce8gb-core - decode par bloc ; HALT/STOP a traiter dans la CPU loop.
Statut : CONFIRME

## Les 11 opcodes illegaux font hard-lock the CPU
Fait : Les opcodes $D3, $DB, $DD, $E3, $E4, $EB, $EC, $ED, $F4, $FC, $FD sont invalides et verrouillent la CPU jusqu'au power off ; aucun d'eux n'existe en variante CB.
Source : refs/pandocs/src/CPU_Instruction_Set.md#CPU-Instruction-Set (L173) ; refs/pandocs/src/CPU_Comparison_with_Z80.md (table L41-L69, note L71)
Fiabilite : communautaire
Impact code : puce8gb-core - flag OpInfo.illegal deja genere par note 02c.
Statut : CONFIRME

## Valeurs de reset DMG des registres CPU
Fait : Au power-up DMG : A=$01, B=$00, C=$13, D=$00, E=$D8, H=$01, L=$4D, PC=$0100, SP=$FFFE ; F = Z=1 N=0 avec H et C dependant du header checksum $014D (clears si $00, set otherwise). En MGB, A=$FF. La CPU commence a $0000 sur le boot ROM grave, qui s'unmappe puis passe la main a $0100.
Source : refs/pandocs/src/Power_Up_Sequence.md#CPU-registers (L223-L237) ; boot ROM (L3-L6, L34-L37)
Fiabilite : communautaire
Impact code : puce8gb-core - reset() du CPU + emulation du handoff boot ROM.
Statut : CONFIRME

## CONFLIT : valeur de F au reset DMG ($01B0 vs H/C conditionnels)
Fait : La doc historique donne AF=$01B0 (soit Z=1 N=0 H=1 C=0 dans le layout pandocs, avec BC=$0013 DE=$00D8 HL=$014D SP=$FFFE) alors que la doc courante laisse H/C ouverts selon $014D. A trancher par une test ROM qui verifie les registres au boot DMG.
Deciding test ROM : mooneye-test-suite acceptance/boot_regs-dmgABC.gb (roms/test-roms/mooneye-test-suite/acceptance/boot_regs-dmgABC.gb).
Source : refs/pandocs/historical/2001-Oct-pandocs.txt (L2695-L2701) vs refs/pandocs/src/Power_Up_Sequence.md#CPU-registers (L223-L237)
Fiabilite : communautaire
Impact code : puce8gb-core - valeur initiale du registre F a l'initialisation.
Statut : CONFLIT

## Laisse hors de la note (limite 150 lignes)
- Table des durees par instruction (couverte par notes 01/02c via Opcodes.json).
- Details internes du bloc CB au-dela du prefixe et des groupements.
- Comportement halt bug / ei delay (refs/pandocs/src/halt.md, Interrupts.md) : noter dans une note CPU dediee.
