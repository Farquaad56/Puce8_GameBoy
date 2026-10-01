# 04_video - Video DMG - PPU, LCDC, STAT, tuiles, OBJ, fenetre

Module cible : video/
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## LCDC ($FF40)
Fait : Bit7 LCD on/off ; 6 carte fenetre (0=$9800, 1=$9C00) ; 5 fenetre on ; 4 donnees BG/fenetre (0=$8800-97FF, 1=$8000-8FFF) ; 3 carte BG ; 2 taille OBJ (0=8x8, 1=8x16) ; 1 OBJ on ; 0 BG/fenetre on (DMG).
Source : pandocs/src/LCDC.md#FF40
Fiabilite : officielle
Impact code : video/regs.rs
Statut : CONFIRME

## LCD eteint
Fait : Desactiver le LCD donne acces complet a VRAM/OAM ; l'ecran est blanc (plus blanc que la couleur 0 sur DMG). Comportement de LY/STAT LCD off : lire STAT.md avant E05.
Source : pandocs/src/LCDC.md#LCDC.7
Fiabilite : officielle
Impact code : video/ppu.rs
Statut : CONFIRME (details LY : UNKNOWN - to confirm)

## STAT ($FF41), LY, LYC
Fait : STAT : bits 6 LYC int, 5 mode2 int, 4 mode1 int, 3 mode0 int, 2 LYC==LY (lecture seule), 1-0 mode (0 si PPU off). LY 0..153 (144..153 = VBlank). LYC compare en continu.
Source : pandocs/src/STAT.md
Fiabilite : officielle
Impact code : video/regs.rs
Statut : CONFIRME

## Ligne d'interruption STAT
Fait : Les sources actives (mode 0/1/2, LYC) sont combinees en OU dans une ligne ; l'interruption se declenche sur front montant. 'STAT blocking' : pas d'interruption si la ligne est deja haute.
Source : pandocs/src/Interrupt_Sources.md#INT $48
Fiabilite : officielle
Impact code : video/stat.rs
Statut : CONFIRME

## Ecriture STAT parasite (DMG)
Fait : Ecrire STAT pendant mode 2, mode 0, mode 1 ou LY=LYC peut declencher une interruption (comme si $FF etait ecrit un M-cycle).
Source : pandocs/src/STAT.md#Spurious STAT interrupts
Fiabilite : officielle
Impact code : video/stat.rs
Statut : CONFIRME

## Acces VRAM / OAM
Fait : VRAM inaccessible en mode 3 ; OAM inaccessible en modes 2 et 3 ; ecritures ignorees, lectures ~$FF.
Source : pandocs/src/Rendering.md#PPU modes ; Accessing_VRAM_and_OAM.md
Fiabilite : officielle
Impact code : bus.rs + video/ppu.rs
Statut : CONFIRME

## Format des tuiles
Fait : Tuile = 16 octets, 2 octets par ligne : 1er octet = bit de poids faible de l'index couleur, 2eme = poids fort. 3 blocs de 128 tuiles : $8000, $8800, $9000.
Source : pandocs/src/Tile_Data.md#Data format
Fiabilite : officielle
Impact code : video/tiles.rs
Statut : CONFIRME

## Adressage des tuiles BG
Fait : LCDC.4 = 1 : IDs non signes depuis $8000. LCDC.4 = 0 : plage $8800-97FF (lire la table de Tile_Data.md pour la correspondance exacte des IDs).
Source : pandocs/src/LCDC.md#LCDC.4 ; Tile_Data.md#VRAM Tile Data
Fiabilite : officielle
Impact code : video/tiles.rs
Statut : CONFIRME (table IDs a relire en E05.12)

## Defilement
Fait : SCY/SCX = coin haut-gauche dans la carte 256x256, avec wrap. Les registres sont relus a chaque fetch de tuile, sauf SCX bas 3 bits lus au debut de ligne.
Source : pandocs/src/Scrolling.md
Fiabilite : officielle
Impact code : video/fetcher.rs
Statut : CONFIRME

## Fenetre
Fait : Visible si WX dans [0;166] et WY dans [0;143] ; WX=7,WY=0 = coin haut-gauche. Condition Y : effacee au VBlank, vraie si WY == LY au debut d'une ligne. Compteur pixel : +7 avant le 1er pixel ; quand egal a WX (Y vrai, fenetre on) le fetch repart sur la carte fenetre. WX=0 : decalage de SCX%8. Bogues DMG : WX=166 ; fenetre desactivee pixel parasite.
Source : pandocs/src/Window.md
Fiabilite : officielle
Impact code : video/window.rs
Statut : CONFIRME

## OAM : 40 entrees x 4 octets
Fait : Y = ecran+16 ; X = ecran+8 ; tuile ; attributs : bit7 priorite (BG 1-3 par-dessus), bit6 Y flip, bit5 X flip, bit4 palette DMG (OBP0/OBP1). 8x16 : bit 0 de l'index ignore. Max 10 OBJ par ligne ; X=0 ou >=168 compte quand meme dans les 10.
Source : pandocs/src/OAM.md
Fiabilite : officielle
Impact code : video/oam_scan.rs
Statut : CONFIRME

## Palettes DMG
Fait : BGP/OBP0/OBP1 : 2 bits par index couleur (0 blanc, 1 gris clair, 2 gris fonce, 3 noir). OBP : bits 1-0 ignores (index 0 transparent).
Source : pandocs/src/Palettes.md#LCD Monochrome Palettes
Fiabilite : officielle
Impact code : video/palette.rs
Statut : CONFIRME

## Penalites du mode 3
Fait : Longueur min = 172 dots (160 + 12). Penalites : SCX%8 dots au debut ; 6 dots pour la fenetre ; chaque OBJ 6 a 11 dots (algorithme OBJ penalty, OBJ a X=0 : 11 dots).
Source : pandocs/src/Rendering.md#Mode 3 length
Fiabilite : officielle
Impact code : video/ppu.rs
Statut : CONFIRME

## Pixel FIFO
Fait : Description detaillee du fetcher et du FIFO : fichier pixel_fifo.md (260 lignes), a lire par plages dans E05.
Source : pandocs/src/pixel_fifo.md
Fiabilite : officielle
Impact code : video/fetcher.rs
Statut : UNKNOWN - to confirm (extraction E05.20)

## Couleurs de comparaison des tests
Fait : DMG : #000000 #555555 #AAAAAA #FFFFFF (noir a blanc) pour les 4 nuances des captures attendues.
Source : game-boy-test-roms/src/howto/blargg.md#Test Success/Failure
Fiabilite : testee sur ROM
Impact code : cli : fn shade_to_rgb
Statut : CONFIRME
