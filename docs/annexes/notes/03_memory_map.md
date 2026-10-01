# 03_memory_map - Carte memoire DMG

Module cible : bus.rs
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Plan d'adressage
Fait : 0000-3FFF ROM banque 0 ; 4000-7FFF ROM commutable ; 8000-9FFF VRAM 8 Kio ; A000-BFFF RAM externe ; C000-DFFF WRAM 8 Kio ; E000-FDFF echo de C000-DDFF ; FE00-FE9F OAM ; FEA0-FEFF inutilisable ; FF00-FF7F I/O ; FF80-FFFE HRAM ; FFFF IE.
Source : pandocs/src/Memory_Map.md (tableau principal)
Fiabilite : officielle
Impact code : bus.rs : fn read / write
Statut : CONFIRME

## Echo RAM
Fait : Lectures et ecritures en E000-FDFF ont le meme effet que C000-DDFF.
Source : pandocs/src/Memory_Map.md#Echo RAM
Fiabilite : officielle
Impact code : bus.rs
Statut : CONFIRME

## FEA0-FEFF
Fait : Sur DMG : lecture = $FF si l'OAM est bloquee (et declenche alors le bug de corruption OAM), sinon $00.
Source : pandocs/src/Memory_Map.md#FEA0-FEFF range
Fiabilite : officielle
Impact code : bus.rs
Statut : CONFIRME

## Plages I/O DMG
Fait : FF00 joypad ; FF01-02 serie ; FF04-07 timer ; FF0F IF ; FF10-26 audio ; FF30-3F wave RAM ; FF40-4B LCD (FF46 = DMA) ; FF50 boot ROM.
Source : pandocs/src/Memory_Map.md#I/O Ranges
Fiabilite : officielle
Impact code : bus.rs : dispatch I/O
Statut : CONFIRME

## Lecture des I/O non mappees / bits inutilises
Fait : Masques de lecture par registre non extraits (Hardware_Reg_List.md indique R/W/Mixed).
Source : pandocs/src/Hardware_Reg_List.md
Fiabilite : officielle
Impact code : bus.rs : read_io
Statut : UNKNOWN - to confirm (E01.13 : lister l'adressage ; valider par mooneye)

## Vecteurs
Fait : RST : $00 $08 $10 $18 $20 $28 $30 $38. Interruptions : $40 $48 $50 $58 $60. En-tete cartouche : $0100-$014F.
Source : pandocs/src/Memory_Map.md#Jump Vectors
Fiabilite : officielle
Impact code : cpu/
Statut : CONFIRME

## ID de tuile depuis l'adresse
Fait : ID = adresse / 16 mod 256. Apres les tuiles : deux cartes 32x32 ($9800, $9C00) ; X = adresse mod 32, Y = adresse / 32 mod 32.
Source : pandocs/src/Memory_Map.md#VRAM memory map
Fiabilite : officielle
Impact code : video/
Statut : CONFIRME
