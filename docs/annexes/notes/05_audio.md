# 05_audio - Audio - APU (extraction partielle)

Module cible : audio/
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Architecture
Fait : 4 canaux (2 pulse, 1 wave, 1 bruit). Chaque canal = generateur + DAC ; valeur numerique 0..15 convertie lineairement en -1..1. Le mixeur ajoute selectivement les canaux (NR51) vers gauche/droite ; volume NR50 ; puis filtre passe-haut.
Source : pandocs/src/Audio_details.md (intro)
Fiabilite : officielle
Impact code : audio/mixer.rs
Statut : CONFIRME

## DIV-APU
Fait : Compteur incremente sur front descendant du bit 4 de DIV (512 Hz). Tous les 8 ticks : enveloppe (64 Hz) ; tous les 2 : longueur (256 Hz) ; tous les 4 : balayage de periode CH1 (128 Hz). Ecrire DIV peut le faire avancer plus vite.
Source : pandocs/src/Audio_details.md#DIV-APU
Fiabilite : officielle
Impact code : audio/frame_seq.rs
Statut : CONFIRME

## Canaux pulse : periode
Fait : Periode 11 bits (NRx3 + NRx4[2:0]), diviseur compte vers le haut ; horloge 1048576 Hz ; forme d'onde de 8 echantillons ; frequence = 131072 / (2048 - periode) Hz.
Source : pandocs/src/Audio_Registers.md#FF13
Fiabilite : officielle
Impact code : audio/pulse.rs
Statut : CONFIRME

## Balayage CH1
Fait : Direction 0 = addition, 1 = soustraction. En addition, depassement > $7FF eteint le canal. Periode 0 : le balayage ne change plus rien.
Source : pandocs/src/Audio_Registers.md#FF10
Fiabilite : officielle
Impact code : audio/pulse.rs
Statut : CONFIRME

## Registres et details des canaux 2, 3, 4, NR52
Fait : Non extraits. Plages : Audio_Registers.md l.17-77 (globaux), l.78-218 (CH1), 219-227 (CH2), 228-359 (CH3), 360-423 (CH4).
Source : pandocs/src/Audio_Registers.md
Fiabilite : officielle
Impact code : audio/*.rs
Statut : UNKNOWN - to confirm (extraction en E08.01 a E08.05)

## Valeurs de reset audio
Fait : NR10 $80, NR11 $BF, NR12 $F3, NR13 $FF, NR14 $BF, NR21 $3F, NR22 $00, NR23 $FF, NR24 $BF, NR30 $7F, NR31 $FF, NR32 $9F, NR33 $FF, NR34 $BF, NR41 $FF, NR42 $00, NR43 $00, NR44 $BF, NR50 $77, NR51 $F3, NR52 $F1 (DMG).
Source : pandocs/src/Power_Up_Sequence.md#Hardware registers
Fiabilite : officielle
Impact code : audio/mod.rs : fn post_boot
Statut : CONFIRME
