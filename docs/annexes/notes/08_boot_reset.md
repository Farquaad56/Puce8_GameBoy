# Note 08 - Boot and reset (DMG)

Tache D_16. Sources : refs/pandocs/src/Power_Up_Sequence.md (principale), refs/pandocs/historical/{2001-Oct-pandocs.txt, 1998-Mar-17-Gbspec.txt}.
Portee : DMG d'origine (monochrome). Les valeurs CGB/SGB ne sont citees que quand elles creent un conflit.

## Boot ROM : existence et taille
Fait : au power-up, le CPU demarre a $0000, pas a $0100 ; une boot ROM gravee dans la console est mappee par-dessus la ROM de cartouche. La boot ROM DMG fait 256 octets (DMG0/DMG/MGB/SGB/SGB2 font tous 256).
Source : Power_Up_Sequence.md#Power-Up-Sequence L3-L18
Fiabilite : communautaire
Impact code : bus + cpu : au reset, mapper la boot ROM sur $0000-$00FF et demarrer PC a $0000.
Statut : CONFIRME

## Comportement de la boot ROM (DMG)
Fait : la boot ROM DMG lit le logo de l'en-tete ($0104-$0133), le decomprime dans VRAM et le fait defiler lentement vers le bas ; elle joue alors un son a deux notes ("ba-ding"), relit le logo et le compare a une copie interne, puis calcule la somme des octets $0134-$014D + 25 (decimal) et la compare au checksum de l'en-tete ($014D). Si l'un des deux echecs survient, la boot ROM se verrouille : le controle n'est jamais passe a la cartouche.
Source : Power_Up_Sequence.md#Monochrome-models L26-L32 ; historical/2001-Oct-pandocs.txt#Power-Up-Sequence L2679-L2705
Fiabilite : communautaire (pandocs) + officielle (specs 2001, concordantes)
Impact code : boot : verifier logo + checksum ; en cas d'echec, arreter le CPU au lieu de hand-off.
Statut : CONFIRME

## Hand-off vers la ROM de cartouche
Fait : le dernier acte de la boot ROM est une ecriture dans $FF50 (BANK) qui la demappe ; l'instruction `ldh [$FF50],a` etant a $00FE, la premiere instruction executee depuis la ROM de cartouche est a $0100. Le registre A garde la valeur ecrise : DMG ecrit $01 (MGB ecrit $FF).
Source : Power_Up_Sequence.md#Monochrome-models L34-L37
Fiabilite : communautaire
Impact code : boot : apres hand-off, PC=$0100 et A=$01 ; demapper la boot ROM.
Statut : CONFIRME

## Duree de la boot ROM (DMG)
Fait : refs/pandocs ne donne aucun chiffre exact (cycles ou frames) pour la duree de la boot ROM DMG ; seule la boot ROM SGB est decrite comme dependante du contenu de l'en-tete (attente de 4 VBlanks par paquet envoye). L'animation DMG est "lente" mais sans valeur numerique dans le corpus.
Source : Power_Up_Sequence.md#Super-Game-Boy L59-L62 ; grep complet du fichier : aucune duree DMG trouvee
Fiabilite : communautaire (absence de donnee)
Impact code : boot : si l'animation est emulee, sa duree n'est pas fixee par les sources ; si elle est sautee, seul l'etat final compte.
Statut : UNKNOWN - to confirm

## Registres CPU apres boot (DMG)
Fait : a PC=$0100 sur DMG : A=$01, F=Z=1 N=0 H=? C=? (H et C poses si le checksum $014D n'est pas $00, effaces s'il vaut $00), B=$00, C=$13, D=$00, E=$D8, H=$01, L=$4D, PC=$0100, SP=$FFFE. Variantes : DMG0 a A=$01 F=Z=0 N=0 H=0 C=0 B=$FF C=$13 E=$C1 HL=$8403 ; MGB a A=$FF (sinon identique a DMG).
Source : Power_Up_Sequence.md#CPU-registers L223-L237 (tableau + note de bas de page dmg_c)
Fiabilite : testee sur ROM (tests Mooneye-GB boot_regs-dmgABC/dmg0/mgb cites a L268)
Impact code : cpu : etat de reset = ces valeurs quand la boot ROM est emulee.
Statut : CONFIRME

## Registres I/O apres boot (DMG)
Fait : valeurs enregistrees a PC=$0100 sur DMG/MGB : P1=$CF SB=$00 SC=$7E DIV=$AB TIMA=$00 TMA=$00 TAC=$F8 IF=$E1 NR10=$80 NR11=$BF NR12=$F3 NR13=$FF NR14=$BF NR21=$3F NR22=$00 NR23=$FF NR24=$BF NR30=$7F NR31=$FF NR32=$9F NR33=$FF NR34=$BF NR41=$FF NR42=$00 NR43=$00 NR44=$BF NR50=$77 NR51=$F3 NR52=$F1 LCDC=$91 STAT=$85 SCY=$00 SCX=$00 LY=$00 LYC=$00 DMA=$FF BGP=$FC WY=$00 WX=$00 IE=$00. OBP0/OBP1 ($FF48/$FF49) sont laisses non initialises (souvent $00 ou $FF, jamais fiable).
Source : Power_Up_Sequence.md#Hardware-registers L272-L345
Fiabilite : testee sur ROM (tests Mooneye-GB boot_hwio-dmgABCmgb cites a L352)
Impact code : io : valeurs de reset des registres I/O.
Statut : CONFIRME

## RAM apres power-up
Fait : WRAM et HRAM contiennent des donnees aleatoires au power-up (les motifs varient selon le modele et la temperature) ; la SRAM de cartouche est egalement du bruit a sa premiere lecture. Les emulators remplissent generalement la RAM a $00 ; un jeu ne doit pas s'appuyer sur ces valeurs.
Source : Power_Up_Sequence.md#Common-remarks L211-L217 ; historical/2001-Oct-pandocs.txt L2738-L2743
Fiabilite : communautaire + officielle (concordantes)
Impact code : ram : au reset, remplir WRAM/HRAM a $00 (ou aleatoirement) ; ne pas dependre de la valeur.
Statut : CONFIRME

## CONFLIT : registre F et OBP0/OBP1 (specs vs pandocs)
Fait : les specs 2001 donnent AF=$01B0 (H=1 C=1 fixes), TAC=$00, OBP0=$FF et OBP1=$FF sur DMG ; pandocs donne H/C dependants du checksum de l'en-tete ($014D), TAC=$F8 (timer deja actif) et OBP0/OBP1 non initialises. Les deux sources concordent par ailleurs sur A, B, C, D, E, HL, SP et la plupart des registres audio/LCD.
Source : historical/2001-Oct-pandocs.txt#Power-Up-Sequence L2697-L2735 vs Power_Up_Sequence.md#CPU-registers L226 et #Hardware-registers L282, L313-L345
Fiabilite : CONFLIT entre les deux sources
Impact code : cpu + io : choix des valeurs de reset en cas de conflit.
Statut : CONFLIT - a trancher par roms/test-roms/mooneye-test-suite/acceptance/boot_regs-dmgABC.gb (registre F) et boot_hwio-dmgABCmgb.gb (TAC, OBP0/OBP1).

## Ce qu'il faut emuler sans boot ROM
Fait : si l'emulateur ne fait pas tourner la boot ROM, il doit livrer l'etat post-boot du modele cible a PC=$0100 : sur DMG, A=$01 F=Z=1 (H/C selon le checksum), B=$00 C=$13 DE=$00D8 HL=$014D SP=$FFFE plus les valeurs I/O ci-dessus et WRAM/HRAM a zero ou aleatoires. Les specs 2001 avertissent que ces valeurs peuvent changer d'une version de console a l'autre, un jeu ne doit donc pas s'y fier.
Source : Power_Up_Sequence.md#Console-state-after-boot-ROM-hand-off L197-L205 ; historical/2001-Oct-pandocs.txt L2736-L2740
Fiabilite : communautaire + officielle
Impact code : reset : etat par defaut quand aucune boot ROM n'est chargee.
Statut : CONFIRME

## Laisse de cote (non necessaire au code DMG)
- Details des boot ROM CGB/AGB/SGB (taille 256+1792, palettes de compatibilite, ecritures KEY0/OPRI, regles du registre B).
- Specificites DMG0 (ecran qui clignote en cas d'echec, pas de symbole de marque, valeurs de registres differentes).
- Boot ROM speciale de Pokemon Stadium 2.
