# 02_cpu - CPU SM83 - registres, flags, encodage, interruptions

Module cible : cpu/
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Registres
Fait : AF, BC, DE, HL (16 bits, moities 8 bits), SP, PC.
Source : pandocs/src/CPU_Registers_and_Flags.md#Registers
Fiabilite : officielle
Impact code : cpu/mod.rs : struct Registers
Statut : CONFIRME

## Flags
Fait : F bits 7..4 = Z, N, H, C. Z = resultat nul. C : addition > $FF/$FFFF, soustraction/comparaison < 0, bit sorti par rotation/decalage. N et H servent a DAA.
Source : pandocs/src/CPU_Registers_and_Flags.md#The Flags Register
Fiabilite : officielle
Impact code : cpu/alu.rs
Statut : CONFIRME

## Bits bas de F
Fait : Les sources lues ne disent pas si F[3:0] est toujours 0 (pop af).
Source : aucune
Fiabilite : deduite
Impact code : cpu/mod.rs : set_af
Statut : UNKNOWN - to confirm (test ROM cpu_instrs, sous-test pop af / push af)

## Encodage : tables de registres
Fait : r8 (3 bits) : 0=b 1=c 2=d 3=e 4=h 5=l 6=[hl] 7=a. r16 : bc de hl sp. r16stk : bc de hl af. r16mem : bc de hl+ hl-. cond : nz z nc c. tgt3 = adresse RST / 8.
Source : pandocs/src/CPU_Instruction_Set.md#placeholders
Fiabilite : officielle
Impact code : cpu/decode.rs
Statut : CONFIRME

## Encodage : blocs
Fait : Bloc 0 (01xxxxxx exclus) : nop, ld r16/imm16, ld [r16mem]/a, ld [imm16]/sp, inc/dec r16, add hl/r16, inc/dec r8, ld r8/imm8, rlca rrca rla rra daa cpl scf ccf, jr, jr cond, stop. Bloc 1 (01) : ld r8,r8 ; ld [hl],[hl] = halt. Bloc 2 (10) : add adc sub sbc and xor or cp avec r8. Bloc 3 (11) : alu imm8, ret/reti/jp/call/rst, pop/push, prefixe $CB, ldh/ld abs, add sp/ld hl,sp+e8/ld sp,hl, di, ei. Prefixe CB : rlc rrc rl rr sla sra swap srl / bit res set.
Source : pandocs/src/CPU_Instruction_Set.md#Block 0 a $CB prefix
Fiabilite : officielle
Impact code : cpu/decode.rs : match par blocs
Statut : CONFIRME

## Opcodes invalides
Fait : $D3 $DB $DD $E3 $E4 $EB $EC $ED $F4 $FC $FD : verrouillent le CPU jusqu'a l'extinction.
Source : pandocs/src/CPU_Instruction_Set.md (avant $CB prefix)
Fiabilite : officielle
Impact code : cpu/decode.rs : etat Locked
Statut : CONFIRME

## Cycles par instruction
Fait : Non fournis par pandocs : renvoi vers gbdev.io/gb-opcodes/optables et gbz80(7).
Source : pandocs/src/CPU_Instruction_Set.md (tip initial)
Fiabilite : officielle (externe)
Impact code : cpu/instructions/*.rs
Statut : UNKNOWN - to confirm (snapshoter l'url dans annexes/sources_cache/ avant E02.07)

## Effets sur les flags par instruction
Fait : Non fournis par pandocs (renvoi gbz80(7) et optables). ADD SP,e8 / LD HL,SP+e8 / DAA : valider par test ROM.
Source : pandocs/src/CPU_Instruction_Set.md (tip initial)
Fiabilite : officielle (externe)
Impact code : cpu/alu.rs
Statut : UNKNOWN - to confirm

## IME
Fait : IME est interne, non lisible. Modifie par ei, di, reti et l'entree dans un handler. L'effet de ei est retarde d'une instruction. IME = 0 au demarrage du jeu.
Source : pandocs/src/Interrupts.md#IME
Fiabilite : officielle
Impact code : cpu/interrupts.rs
Statut : CONFIRME

## IE / IF et priorite
Fait : Bits : 0 VBlank, 1 LCD, 2 Timer, 3 Serial, 4 Joypad. Vecteurs $40 $48 $50 $58 $60. Le bit de poids faible a la priorite. Execution si IME et (IE & IF) != 0. Prise en compte : bit IF efface, IME = 0, puis appel du vecteur.
Source : pandocs/src/Interrupts.md#IE, #IF, #Interrupt handling, #Interrupt priorities
Fiabilite : officielle
Impact code : cpu/interrupts.rs
Statut : CONFIRME

## halt
Fait : Reveil quand IE & IF != 0. Si IME = 1 : handler appele normalement. Si IME = 0 et rien en attente : reprise sans handler. Si IME = 0 et interruption deja en attente : halt bug.
Source : pandocs/src/halt.md
Fiabilite : officielle
Impact code : cpu/instructions/halt.rs
Statut : CONFIRME

## halt bug
Fait : PC n'est pas incremente : l'octet suivant est lu deux fois. Cas ei+halt : handler appele, retour sur le halt. Cas halt suivi de rst : l'adresse de retour pointe sur le rst. ei + rst : ei gagne.
Source : pandocs/src/halt.md#halt bug
Fiabilite : officielle
Impact code : cpu/instructions/halt.rs
Statut : CONFIRME

## stop
Fait : Comportement detaille non extrait : page Using the STOP Instruction (pas de ROM licenciee hors CGB speed switch). DIV est remis a 0 par stop.
Source : pandocs/src/Reducing_Power_Consumption.md#Using the STOP Instruction ; Timer_and_Divider_Registers.md#FF04
Fiabilite : officielle
Impact code : cpu/instructions/stop.rs
Statut : UNKNOWN - to confirm (lire Reducing_Power_Consumption.md l.59-110 en E02.42)
