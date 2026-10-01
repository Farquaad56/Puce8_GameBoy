# 06_input - Entrees - joypad et serie

Module cible : input.rs, serial.rs
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Joypad $FF00
Fait : Bit 5 = 0 selectionne les boutons (Start Select B A), bit 4 = 0 selectionne la croix. Bits 3-0 en lecture seule, 0 = appuye. Rien de selectionne : quartet bas = $F.
Source : pandocs/src/Joypad_Input.md#FF00
Fiabilite : officielle
Impact code : input.rs
Statut : CONFIRME

## Interruption joypad
Fait : Vecteur $60, bit 4 de IF. Condition exacte de declenchement non extraite.
Source : pandocs/src/Interrupt_Sources.md#INT $60
Fiabilite : officielle
Impact code : input.rs
Statut : UNKNOWN - to confirm (lire Interrupt_Sources.md en E06.03)

## Serie SB/SC
Fait : SB = octet a emettre/recu (decalage a gauche, bit entrant a droite). SC bit 7 = transfert actif, bit 0 = horloge interne (maitre) / externe. Bit 1 = vitesse (CGB).
Source : pandocs/src/Serial_Data_Transfer_(Link_Cable).md
Fiabilite : officielle
Impact code : serial.rs
Statut : CONFIRME (duree d'un transfert : UNKNOWN - to confirm)
