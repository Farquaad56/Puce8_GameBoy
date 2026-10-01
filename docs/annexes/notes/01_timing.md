# 01_timing - Timing - horloges, frame, cadences

Module cible : machine.rs (scheduler)
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Horloge maitre et horloge systeme
Fait : Horloge maitre DMG = 4.194304 MHz ; l'horloge systeme vaut 1/4 de l'horloge maitre (1 M-cycle = 4 T-cycles).
Source : pandocs/src/Specifications.md#tableau (lignes Master Clock, System Clock)
Fiabilite : officielle
Impact code : machine.rs : constante T_CYCLES_PER_M = 4
Statut : CONFIRME

## Dot
Fait : Un dot = un T-cycle en vitesse normale ; 4 dots par M-cycle (vitesse double CGB hors perimetre).
Source : pandocs/src/Rendering.md#Terminology ; STAT.md#terminologie
Fiabilite : officielle
Impact code : machine.rs : ppu.tick_dot() appele 4 fois par M-cycle
Statut : CONFIRME

## Structure d'une frame
Fait : 154 scanlines, 144 visibles. Modes : 2 = 80 dots, 3 = 172 a 289 dots, 0 = 376 - duree mode 3, 1 = 4560 dots (10 lignes). Ligne = 80 + 376 = 456 dots ; frame = 154 x 456 = 70224 dots (valeur deduite du tableau, a valider par test ROM).
Source : pandocs/src/Rendering.md#PPU modes
Fiabilite : officielle (456 et 70224 : deduite)
Impact code : video/ppu.rs : constantes DOTS_PER_LINE, LINES_PER_FRAME
Statut : CONFIRME (70224 = deduite)

## Cadence d'affichage
Fait : Synchro verticale ~59.73 Hz ; une frame dure ~16.74 ms.
Source : pandocs/src/Specifications.md ; Rendering.md#Terminology
Fiabilite : officielle
Impact code : desktop : accumulateur de temps / cadence audio
Statut : CONFIRME

## DIV
Fait : DIV s'incremente a 16384 Hz (soit tous les 64 M-cycles en vitesse normale) ; c'est la partie visible du compteur systeme qui avance a chaque M-cycle (sauf en STOP).
Source : pandocs/src/Timer_and_Divider_Registers.md#FF04 ; Timer_Obscure_Behaviour.md#System counter
Fiabilite : officielle
Impact code : timer.rs : system_counter
Statut : CONFIRME

## Largeur du compteur systeme
Fait : Hypothese : compteur de 14 bits compte en M-cycles, DIV = bits 13..6. Coherent avec 64 M-cycles/increment et avec l'exemple 'SYS' de Timer_Obscure (TAC=$FD selectionne le bit 1).
Source : pandocs/src/Timer_Obscure_Behaviour.md#Timer overflow behavior
Fiabilite : deduite
Impact code : timer.rs : struct Timer
Statut : UNKNOWN - to confirm (test ROMs timer du zip mooneye)

## Frequences TIMA (TAC bits 1-0)
Fait : 00 = tous les 256 M-cycles ; 01 = 4 ; 10 = 16 ; 11 = 64. Le bit 2 active TIMA ; DIV compte toujours.
Source : pandocs/src/Timer_and_Divider_Registers.md#FF07
Fiabilite : officielle
Impact code : timer.rs : fn selected_bit(tac)
Statut : CONFIRME

## Dispatch d'interruption
Fait : 5 M-cycles : 2 cycles d'attente, 2 cycles d'empilement de PC, 1 cycle de chargement de PC.
Source : pandocs/src/Interrupts.md#Interrupt handling
Fiabilite : officielle
Impact code : cpu/interrupts.rs : micro-ops de dispatch
Statut : CONFIRME

## OAM DMA
Fait : Dure 160 M-cycles (640 dots) en vitesse normale.
Source : pandocs/src/OAM_DMA_Transfer.md#FF46
Fiabilite : officielle
Impact code : bus.rs / dma.rs
Statut : CONFIRME

## Ordre des puces dans un tick
Fait : Ordre CPU / timer / PPU / APU / DMA a l'interieur d'un M-cycle non precise par les sources.
Source : aucune
Fiabilite : deduite
Impact code : machine.rs : fn tick
Statut : UNKNOWN - to confirm (decision D02 + test ROMs)
