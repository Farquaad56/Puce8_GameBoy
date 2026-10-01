# 07_media - Cartouche - en-tete et mappers

Module cible : media/
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Champs d'en-tete utiles
Fait : $0100-0103 entree ; $0104-0133 logo ; $0134-0143 titre ; $0147 type ; $0148 taille ROM ; $0149 taille RAM ; $014D checksum d'en-tete ; $014E-014F checksum global (big-endian, non verifie).
Source : pandocs/src/The_Cartridge_Header.md
Fiabilite : officielle
Impact code : media/header.rs
Statut : CONFIRME

## Checksum d'en-tete
Fait : checksum = 0 ; pour adresse de $0134 a $014C : checksum = checksum - rom[adresse] - 1 (8 bits). La boot ROM verrouille si different de rom[$014D].
Source : pandocs/src/The_Cartridge_Header.md#014D
Fiabilite : officielle
Impact code : media/header.rs : fn header_checksum
Statut : CONFIRME

## Taille ROM ($0148)
Fait : $00..$08 : 32 Kio x (1 << valeur), banques = 2 << valeur. Valeurs $52 $53 $54 : non officielles, a rejeter.
Source : pandocs/src/The_Cartridge_Header.md#0148
Fiabilite : officielle
Impact code : media/header.rs
Statut : CONFIRME

## Taille RAM ($0149)
Fait : $00 aucune ; $01 inutilise ; $02 8 Kio ; $03 32 Kio (4 banques) ; $04 128 Kio (16) ; $05 64 Kio (8). MBC2 : 512 x 4 bits internes, code 0.
Source : pandocs/src/The_Cartridge_Header.md#0149
Fiabilite : officielle
Impact code : media/header.rs
Statut : CONFIRME

## Type de cartouche ($0147)
Fait : $00 ROM seule ; $01-03 MBC1 ; $05-06 MBC2 ; $0F-13 MBC3 (0F/10 avec timer) ; $19-1E MBC5. Autres (MMM01 $0B-0D, MBC6 $20, MBC7 $22, $FC, $FD, $FE, $FF) : hors perimetre, renvoyer Unsupported.
Source : pandocs/src/The_Cartridge_Header.md#0147
Fiabilite : officielle
Impact code : media/header.rs : enum MapperKind
Statut : CONFIRME

## MBC1 - registres
Fait : Tous a $00 a l'allumage (banque ROM 0 traitee comme 1). 0000-1FFF : RAM active si quartet bas = $A. 2000-3FFF : banque ROM 5 bits, 0 -> 1, masquee a la taille. 4000-5FFF : 2 bits (banque RAM ou bits hauts ROM). Banque effective = (secondaire << 5) + principale. ROM >= 1 Mio : cablage alternatif. Registre de mode ($6000-7FFF) et MBC1M : a extraire.
Source : pandocs/src/MBC1.md#Registers
Fiabilite : officielle
Impact code : media/mappers/mbc1.rs
Statut : CONFIRME (mode 6000-7FFF : UNKNOWN - to confirm, lire MBC1.md l.95-150 en E07.04)

## MBC5 - registres
Fait : 0000-1FFF RAM enable ($0A actif ; en pratique quartet bas $A). 2000-2FFF 8 bits bas de banque ROM (0 = banque 0). 3000-3FFF 9e bit. 4000-5FFF banque RAM 00-0F (bit 3 = rumble si present). ROM jusqu'a 512 banques.
Source : pandocs/src/MBC5.md
Fiabilite : officielle
Impact code : media/mappers/mbc5.rs
Statut : CONFIRME

## MBC2 et MBC3 (RTC)
Fait : Non extraits (MBC2.md 55 lignes, MBC3.md 103 lignes : registres RTC 08-0C, latch 6000-7FFF, compteur de jours).
Source : pandocs/src/MBC2.md ; MBC3.md
Fiabilite : officielle
Impact code : media/mappers/
Statut : UNKNOWN - to confirm (extraction E07.08 et E07.11)
