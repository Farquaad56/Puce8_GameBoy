# Note 10 - Boot ROM (tous modeles)

Tache E14_01. Source unique : refs/pandocs/src/Power_Up_Sequence.md (PU) et refs/pandocs/src/Memory_Map.md (MM). Complete la note 08 (DMG seul) ; ici tous les modeles. Aucun octet de boot ROM n'est copie.

## Modeles et tailles
Fait : 9 boot ROM officielles connues. DMG0, DMG, MGB, SGB, SGB2 font 256 octets ; CGB0, CGB, AGB0, AGB font 256 + 1792 octets (2048 au total).
Source : PU L8-L20 ; PU L68-L70
Statut : CONFIRME

## Adresses couvertes
Fait : sur les modeles monochromes la boot ROM couvre $0000-$00FF. Sur les modeles couleur elle est en deux parties : $0000-$00FF et $0200-$08FF, l'en-tete de cartouche ($0100-$014F) restant visible au milieu.
Source : PU L68-L72
Statut : CONFIRME

## Ecriture dans $FF50
Fait : une ecriture dans le registre BANK ($FF50) demappe la boot ROM. L'instruction `ldh [$FF50], a` est a $00FE (2 octets), donc la premiere instruction executee depuis the cartouche is at $0100.
Source : PU L34-L35 ; MM L36
Statut : CONFIRME

## Valeur ecrite dans $FF50
Fait : la valeur ecrite est celle of A au hand-off : DMG $01, MGB $FF, SGB $01, SGB2 $FF, et $11 for the boot ROM couleur.
Source : PU L37 ; PU L59 ; PU L94
Statut : CONFIRME

## Demapping reversible ou non
Fait : les sources ne disent pas if the demapping of the boot ROM by $FF50 can be annule (remapper the boot ROM) ni what becomes an ecriture ulterieure in $FF50.
Source : PU L34-L37 et MM L36 (aucune mention)
Statut : UNKNOWN - to confirm

## Comportement monochrome (DMG0, DMG, MGB)
Fait : la boot ROM lit the logo of l'en-tete, le decomprime in VRAM and le fait defiler slowly vers the bas. Once the defilement is fini, elle joue the son "ba-ding!", relit the logo and le compare a une copie interne, puis calcule the checksum d'en-tete and le compare a celui of $014D. Sans cartouche, les lectures give the most often $FF, d'ou un rectangle noir qui defile.
Source : PU L26-L32
Statut : CONFIRME

## Verrouillage sur logo or checksum errone (monochrome)
Fait : if the verification of the logo or the checksum echoue, la boot ROM se verrouille and ne passes jamais the main a the cartouche. The DMG0 does the two verifications before any affichage and then fait clignoter l'ecran between blanc and noir pendant the verrouillage.
Source : PU L31-L32 ; PU L44-L45
Statut : CONFIRME

## Super Game Boy (SGB, SGB2)
Fait : les boot ROM SGB ne font aucune verification. Elles installent the logo in VRAM, envoient tout l'en-tete au SGB BIOS by paquets (octets d'en-tete de paquet $F1, $F3, $F5, $F7, $F9, $FB dans cet ordre), puis se demappent. C'est le SGB BIOS (cote SNES) who verifie logo and checksum ; s'il echoue, il reinitialise sans fin the CPU SGB of the cartouche.
Source : PU L49-L57
Statut : CONFIRME

## Comportement des boot ROM couleur
Fait : elles decompressent the logo in VRAM and dans un tampon HRAM, jouent l'animation with "ba-ding!", puis verifient the logo from the tampon HRAM (seule la premiere moitie of the logo is verifiee) et le checksum d'en-tete. Elles fondent ensuite les palettes BG vers the blanc and reglent the mode materiel : l'octet de compatibilite CGB est ecrit in KEY0 s'il indique the compatibilite CGB, sinon $04 is ecrit in KEY0 and $01 in OPRI.
Source : PU L76-L92
Statut : CONFIRME

## Registres CPU au hand-off : DMG0, DMG, MGB, SGB, SGB2
Fait : a PC=$0100, SP=$FFFE for tous. DMG0 : A=$01 B=$FF C=$13 D=$00 E=$C1 H=$84 L=$03, flags Z=0 N=0 H=0 C=0. DMG : A=$01 B=$00 C=$13 D=$00 E=$D8 H=$01 L=$4D, flags Z=1 N=0, H et C poses if the checksum $014D is non nul. MGB : comme DMG but A=$FF. SGB : A=$01 B=$00 C=$14 D=$00 E=$00 H=$C0 L=$60, flags a 0. SGB2 : comme SGB but A=$FF.
Source : PU L221-L237
Statut : CONFIRME

## Registres CPU au hand-off : CGB et AGB
Fait : a PC=$0100, SP=$FFFE, A=$11 for tous. CGB (mode CGB) : flags Z=1 N=0 H=0 C=0, B=$00 C=$00 D=$FF E=$56 H=$00 L=$0D. AGB (mode CGB) : flags tous a 0, B=$01, the reste comme CGB. En mode de compatibilite DMG : C=$00 D=$00 E=$08, B depend du code de licence ($00 sinon la somme des 16 octets du titre), HL vaut $991A if B is $43 or $58 (CGB) / $44 or $59 (AGB), sinon $007C.
Source : PU L239-L266
Statut : CONFIRME

## Registres materiels au hand-off (DMG / MGB)
Fait : valeurs a PC=$0100 on DMG/MGB : LCDC=$91, BGP=$FC, IF=$E1, TAC=$F8, NR52=$F1, DIV=$AB, STAT=$85, LY=$00, DMA=$FF, IE=$00 ; OBP0 and OBP1 ne sont pas initialises. For SGB et CGB/AGB, DIV, STAT and LY dependent de la duree of the boot ROM, donc de l'en-tete.
Source : PU L270-L336
Statut : CONFIRME

## RAM a la mise sous tension
Fait : WRAM and HRAM are aleatoires a la mise sous tension and ne doivent pas etre utilisees ; la RAM externe of cartouche contains en general des donnees aleatoires au premier demarrage.
Source : PU L211-L217
Statut : CONFIRME

## Registres CPU avant the boot ROM (power-on)
Fait : les valeurs des registres CPU (A, F, BC, DE, HL, SP) before l'execution of the boot ROM a $0000 ne sont pas donnees by the sources.
Source : PU L1-L7 et L209-L266 (aucune mention)
Statut : UNKNOWN - to confirm

## Registres materiels avant the boot ROM (power-on)
Fait : les valeurs des registres d'E/S ($FF00-$FFFF) before l'execution of the boot ROM ne sont pas donnees by the sources.
Source : PU L270-L336 (valeurs donnees seulement a PC=$0100)
Statut : UNKNOWN - to confirm
