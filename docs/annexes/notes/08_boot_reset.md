# 08_boot_reset - Etat apres boot ROM (DMG)

Module cible : machine.rs (reset)
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Registres CPU (DMG)
Fait : A=$01 F: Z=1 N=0 (H et C = 0 si checksum d'en-tete = $00, sinon les deux a 1) B=$00 C=$13 D=$00 E=$D8 H=$01 L=$4D PC=$0100 SP=$FFFE.
Source : pandocs/src/Power_Up_Sequence.md#CPU registers
Fiabilite : officielle
Impact code : cpu/mod.rs : Registers::post_boot_dmg
Statut : CONFIRME

## Registres materiels (DMG)
Fait : P1 $CF, SB $00, SC $7E, DIV $AB, TIMA $00, TMA $00, TAC $F8, IF $E1, LCDC $91, STAT $85, SCY $00, SCX $00, LY $00, LYC $00, DMA $FF, BGP $FC, WY $00, WX $00. OBP0/OBP1 : inconnus (??).
Source : pandocs/src/Power_Up_Sequence.md#Hardware registers
Fiabilite : officielle
Impact code : bus.rs : fn post_boot_io
Statut : CONFIRME (OBP0, OBP1 : UNKNOWN - to confirm)

## Valeurs reset audio
Fait : Voir 05_audio.md#Valeurs de reset audio.
Source : renvoi
Fiabilite : officielle
Impact code : audio/mod.rs
Statut : CONFIRME

## Boot ROM
Fait : La boot ROM DMG (256 octets) lit le logo, verifie logo et checksum, puis ecrit dans $FF50 (dernier acces a $00FE) ; premiere instruction cartouche en $0100. Decision : demarrer directement a l'etat post-boot (pas de boot ROM, non distribuable).
Source : pandocs/src/Power_Up_Sequence.md#Monochrome models
Fiabilite : officielle
Impact code : machine.rs
Statut : CONFIRME

## WRAM / VRAM / HRAM au demarrage
Fait : Contenu initial non extrait (presume indefini).
Source : aucune
Fiabilite : deduite
Impact code : bus.rs : Default
Statut : UNKNOWN - to confirm (decision : zero-fill deterministe, D06)
