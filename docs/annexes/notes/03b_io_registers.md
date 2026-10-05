# Note 03b - I/O registers FF00-FFFF (DMG)

Tache D_08. Sources : refs/pandocs/src/{Hardware_Reg_List, Memory_Map, Power_Up_Sequence, Joypad_Input, Serial_Data_Transfer_(Link_Cable), Timer_and_Divider_Registers, Interrupts, Audio_Registers, LCDC, STAT, Window, Palettes, OAM_DMA_Transfer, CGB_Registers, IR}.md
"power-up value*" = valeur du tableau Power_Up_Sequence.md#Hardware-registers, enregistree a PC=$0100, donc apres le passage de la boot ROM (L272).

## Table FF00-FFFF
| Adresse | Nom | R/W | Bits inutilises (lisent 1) | Effet d'ecriture | Valeur au power-up* |
|---|---|---|---|---|---|
$FF00 | P1/JOYP joypad | mixte (nib. haut W, nib. bas R) | - ; si aucun groupe selectionne ($30 ecrit), le nibble bas lit $F | aucune : la lecture depend des bits de selection precedemment ecrits | $CF
$FF01 | SB serial data | R/W | - | pendant un transfert : registre decalage melange octet sortant et entrant | $00
$FF02 | SC serial control | mixte | 6-2 | bit7=1 demarre le transfert (maitre ecrit $81, esclave s'active a $80) ; en fin de transfert bit7 se decale tout seul + INT3 demande | $7E
$FF03 | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF04 | DIV divider | R/W | - (8 bits utilises) | toute ecriture le remet a $00 ; egalement remis par l'instruction stop | $AB
$FF05 | TIMA timer counter | R/W | - | aucune : auto-increment selon l'horloge TAC ; au depassement, rechargement depuis TMA + INT2 demande | $00
$FF06 | TMA timer modulo | R/W | - | aucune : copie dans TIMA au depassement (avec INT2) | $00
$FF07 | TAC timer control | mixte | 7-3 | une ecriture peut incrementer TIMA d'une fois | $F8
$FF08-$FF0E | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF0F | IF interrupt flags | R/W | 7-5 (lisent haut) | les bits de drapeaux sont poses/effaces automatiquement par le hardware ; ecriture manuelle possible aussi | $E1
$FF10 | NR10 ch1 sweep | R/W | bit 7 | - (balayage actif seulement quand le canal est declenche) | $80
$FF11 | NR11 ch1 length + duty | mixte | - (tous bits nommes) | - | $BF
$FF12 | NR12 ch1 volume/envelope | R/W | - (tous bits nommes) | - | $F3
$FF13 | NR13 ch1 period low | W only | n/a (write-only, lecture non documentee) | - | $FF
$FF14 | NR14 ch1 period high + control | mixte | 5-3 ; bit7 write-only | ecriture avec bit7=1 declenche le canal : l'active, fixe la periode depuis NR13+NR14, remet a zero le timer d'enveloppe | $BF
$FF15 | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF16 | NR21 (comme NR11) | mixte | - (tous bits nommes) | - | $3F
$FF17 | NR22 (comme NR12) | R/W | - (tous bits nommes) | - | $00
$FF18 | NR23 ch2 period low | W only | n/a (write-only) | - | $FF
$FF19 | NR24 (comme NR14) | mixte | 5-3 ; bit7 write-only | ecriture avec bit7=1 declenche le canal (ch2) | $BF
$FF1A | NR30 ch3 DAC enable | R/W | 6-0 | - (bit7 = DAC on/off) | $7F
$FF1B | NR31 ch3 length timer | W only | n/a (write-only) | - | $FF
$FF1C | NR32 ch3 output level | R/W | tous sauf 6-5 (level) | - | $9F
$FF1D | NR33 ch3 period low | W only | n/a (write-only) | - | $FF
$FF1E | NR34 ch3 period high + control | mixte | 5-3 ; bit7 write-only | ecriture avec bit7=1 declenche le canal : l'active, remet a zero l'index de Wave RAM sans la recharger | $BF
$FF20 | NR41 ch4 length timer | W only | 7-6 | - | $FF
$FF21 | NR42 (comme NR12) | R/W | - (tous bits nommes) | - | $00
$FF22 | NR43 ch4 noise control | R/W | - (tous bits nommes ; clock shift 14/15 arrete l'horloge du canal) | - | $00
$FF23 | NR44 ch4 period high + control | mixte | 5-0 ; bit7 write-only | ecriture avec bit7=1 declenche le canal : l'active, remet a zero LFSR/enveloppe/length, volume = initial de NR42 | $BF
$FF24 | NR50 master volume / VIN panning | R/W | - (tous bits nommes) | - | $77
$FF25 | NR51 sound pan | R/W | - (tous bits nommes) | - | $F3
$FF26 | NR52 sound on/off | mixte | 6-4 | bit7 = on/off de l'audio maitre : masque tous les canaux | $F1
$FF27-$FF2F | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF30-$FF3F | Wave pattern RAM (ch3) | R/W | - | - ; le canal 3 y lit l'echantillon en cours, comportement de lecture depend du modele (voir source) | UNKNOWN - to confirm
$FF40 | LCDC LCD control | R/W | - (tous bits nommes) | chaque bit bascule immediatement un element affiche (BG/OBJ/window enable + zones de tiles) ; l'effet est vivant a toute ecriture | $91
$FF41 | STAT LCD status | mixte | 7 ; bit2 et 1-0 read-only | sur DMG, une ecriture pendant OAM scan / HBlank / VBlank ou LY=LYC peut declencher un INT1 parasite (comme si $FF fut ecrite pendant 1 M-cycle) ; absent du GBC en mode DMG | $85
$FF42 | SCY viewport Y | R/W | - | - | $00
$FF43 | SCX viewport X | R/W | - | - | $00
$FF44 | LY LCD line | read-only | - | - (auto-increment PPU ; 144-153 = VBlank) | $00 (col. DMG/MGB ; col. DMG0=$91, depend du header)
$FF45 | LYC compare | R/W | - | - : compare continuellement a LY ; quand egaux pose le bit2 de STAT et demande INT1 si selectionne | $00
$FF46 | DMA OAM DMA source + start | R/W | - | toute ecriture demarre un OAM DMA : copie $XX00-$XX9F -> $FE00-$FE9F (octet ecrit = adresse source / $100) en 160 M-cycles ; sur DMG le CPU est limite a HRAM pendant le transfert | $FF
$FF47 | BGP BG palette (DMG) | R/W | - | - | $FC
$FF48 | OBP0 OBJ palette 0 (DMG) | R/W | - | - | initialisee a power-up (voir fait UNKNOWN)
$FF49 | OBP1 OBJ palette 1 (DMG) | R/W | - | - | non initialisee a power-up (voir fait UNKNOWN)
$FF4A | WY window Y | R/W | - | - | $00
$FF4B | WX window X + 7 | R/W | layout de bits non documente dans ce pandocs ; le power-up $00 contredit la regle "unused = high" (voir fait UNKNOWN) | - | $00
$FF4C | KEY0/SYS CPU mode select (CGB only) | mixte (CGB only, bit2 = compat DMG) | n/a sur DMG | n/a sur DMG | lecture DMG : UNKNOWN - to confirm ; col. CGB ?? (depend du header)
$FF4D | KEY1/SPD speed switch (CGB only) | mixte (bits 7 et 0 definis en mode CGB) | n/a sur DMG | n/a sur DMG | lit $FF en non-CGB mode ; power-up CGB : $7E
$FF4E | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF4F | VBK VRAM bank (CGB only) | R/W (CGB only) | n/a sur DMG | n/a sur DMG | lit $FF en non-CGB mode ; power-up CGB : $FE
$FF50 | BANK boot ROM mapping control | W only (tous modes) | lecture non documentee -> UNKNOWN - to confirm | une ecriture decale la boot ROM (la boot ROM DMG ecris $01 a sa fin) | pas de valeur dans le tableau
$FF51-$FF54 | HDMA1-4 VRAM DMA source/dest (CGB only, write-only) | W only (CGB only) | n/a sur DMG ; en CGB les 4 bits bas et les 3 bits hauts de dest sont ignores/traites comme 0 | n/a sur DMG | lit $FF en non-CGB mode
$FF55 | HDMA5 VRAM DMA length/mode/start (CGB only) | R/W (CGB only) | - | ecriture avec bit7=1 demarre un HBlank DMA ; la lecture rend la longueur restante (en blocs de $10 moins 1), $FF si termine | lit $FF en non-CGB mode ; power-up CGB : $FF
$FF56 | RP infrared port (CGB only) | mixte (bits CGB : 7-6 read enable, bit1 receiving, bit0 emitting) | n/a sur DMG | ecriture avec bit0=1 emet de l'IR | lit $FF en non-CGB mode ; power-up CGB : $3E
$FF57-$FF67 | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF68-$FF69 | BGPS/BGPI + BCPD/BGPD palette spec/data (CGB only) | R/W en CGB ; le registre data lit $FF / ignore les ecritures quand la PPU le lit (mode 3), en CGB seulement | n/a sur DMG | n/a sur DMG | lecture DMG : UNKNOWN - to confirm
$FF6A-$FF6B | OCPS/OBPI + OCPD/OBPD palette spec/data (CGB only) | R/W en CGB ; comme FF68-FF69 | n/a sur DMG | n/a sur DMG | lecture DMG : UNKNOWN - to confirm
$FF6C | OPRI object priority mode (CGB only) | R/W (bit0 = 1 : priorite style DMG, pose par la boot ROM CGB si le jeu est compatible) | n/a sur DMG | aucune effet instantane en mode DMG/CGB apres unmapping de la boot ROM (a verifier) | lecture DMG : UNKNOWN - to confirm
$FF6D-$FF6F | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF70 | SVBK/WBK WRAM bank (CGB only) | R/W (bits 2-0 ; ecrire 0 mappe le bank 1) | n/a sur DMG | n/a sur DMG | lit $FF en non-CGB mode ; power-up CGB : $F8
$FF71 | - non attribue DMG | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF72-$FF73 | bits non documentes (CGB only) | R/W en CGB, valeur initiale $00 ; pas d'enonce explicite pour la lecture DMG -> UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF74 | bit non documente (CGB only) | R/W en CGB (initiale $00) ; hors mode CGB elle est read-only et figee a $FF (enonce explicite) | n/a sur DMG : lit $FF | n/a sur DMG | $FF (DMG, explicite)
$FF75 | bits 4-6 non documentes (CGB only) | seulement les bits 4-6 R/W en CGB (initiale 0) ; pas d'enonce explicite pour la lecture DMG -> UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FF78-$FFFE | - non attribue DMG (sauf FFFF ci-dessous) | lecture : UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm | UNKNOWN - to confirm
$FFFF | IE interrupt enable | R/W | 7-5 selon le diagramme, mais voir fait CONFLIT ci-dessous (power-up $00) | - ; un bit pose autorise le drapeau IF correspondant a lancer l'handler si IME est aussi pose | $00

Source : refs/pandocs/src/Hardware_Reg_List.md#Hardware-Registers L4-L66 pour FF00-$FFFF : adresses, noms, colonne R/W et colonnes modeles.
Source : refs/pandocs/src/Power_Up_Sequence.md#Hardware-registers L274-L350 pour la colonne "power-up value*" (valeurs enregistrees a PC=$0100, apres boot ROM).
Source : refs/pandocs/src/IR.md L41 pour la regle generale "unused bits read high" appliquee dans la colonne des bits inutilises.
Source : refs/pandocs/src/{Joypad_Input, Serial_Data_Transfer_(Link_Cable), Timer_and_Divider_Registers, Interrupts}.md#FF00..#FFFF pour $FF00-$FF0F : details joypad, serial, timer, interruptions (colonnes effet d'ecriture).
Source : refs/pandocs/src/Audio_Registers.md L21-L423 + #FF30-FF3F-Wave-pattern-RAM L337-L351 pour $FF10-$FF3F : canaux audio et Wave RAM.
Source : refs/pandocs/src/LCDC.md#FF40-LCDC L3-L21 et STAT.md#FF41..#FF45 L17-L42 pour $FF40-$FF45 ; Window.md#FF4A-FF4B-WY-WX L3-L45 pour $FF4A-$FF4B ; Palettes.md#FF47-BGP L10 et #FF48-FF49-OBP0 L27-L30 pour $FF47-$FF49 ; OAM_DMA_Transfer.md#FF46-DMA L4-L28 pour $FF46.
Source : refs/pandocs/src/CGB_Registers.md (KEY1 L147, RP L200-L223, KEY0 L224-L249, OPRI L250-L268, SVBK L271-L279, HDMA L33-L126, non documentes FF72-FF75 L283-L305) + Power_Up_Sequence.md#Power-Up-Sequence L34-L37 pour $FF4C-$FF75 et FFFF.

## Bits inutilis des registres I/O lisent haut - CONFLIT sur IE
Fait : pandocs affirme que, comme les autres registres MMIO de Game Boy, les bits inutilises d'un registre I/O lisent haut (pose a 1) ; le tableau power-up confirme pour IF=$E1 (bits 7-6 = 1), NR30=$7F, TAC=$F8, NR52=$F1. En revanche ce meme tableau donne IE=$00 a PC=$0100 alors que Interrupts.md#FFFF-IE ne nomme que les bits 4-0 : si la regle generale s'appliquait a FFFF, il lirait >= $E0. Les deux valeurs sont documentees ci-dessus ; ROM qui tranche : roms/test-roms/mooneye-test-suite (tests boot_hwio).
Source : refs/pandocs/src/IR.md L41 + Power_Up_Sequence.md#Hardware-registers L283, L304, L332 + Interrupts.md#FFFF-IE-Interrupt-enable L19-L30.
Fiabilite : communautaire
Impact code : bus io_read (defaut pour bits non mappes), registre IE de la CPU
Statut : CONFLIT

## Joypad FF00 : bits de selection et valeur lue
Fait : les 8 boutons forment une matrice 2x4 ; on ecrit le nibble haut pour selectionner soit action soit direction, puis on lit le nibble bas (read-only) ou un bouton presse = bit a 0 (pas a 1). Si aucun groupe n'est selectionne ($30 ecrit), le nibble bas lit $F comme si tout etait relache. Valeur power-up a PC=$0100 : $CF (les deux selects = 1, aucun bouton).
Source : refs/pandocs/src/Joypad_Input.md#FF00-P1-JOYP-Joypad L5-L26 + Power_Up_Sequence.md L276.
Fiabilite : communautaire
Impact code : module input/joypad (io_read 0xFF00)
Statut : CONFIRME

## Effets d'ecriture qui changent un etat hors du bit ecrit
Fait : FF04 toute ecriture remet DIV a $00 (et l'instruction stop aussi). FF07 une ecriture peut incrementer TIMA d'une fois. FF02 bit7=1 demarre le transfert serial (maitre ecris $81, esclave s'active a SC=$80) ; en fin de transfert bit7 se decale tout seul et INT3 est demande. NR14/NR24/NR34/NR44 : ecriture avec bit7=1 declenche le canal (l'active, remet a zero timers d'enveloppe/length ; NR34 egalement index Wave RAM sans rechargement ; NR44 egalement LFSR). FF46 toute ecriture demarre un OAM DMA de 160 M-cycles (copie $XX00-$XX9F -> $FE00-$FE9F, octet = source / $100) ; sur DMG le CPU est limite a HRAM pendant.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF04-DIV L15-L16 et #FF07-TAC L63 + Serial_Data_Transfer_(Link_Cable).md#FF02-SC L32-L49 + Audio_Registers.md L203-L212, L304-L315, L411-L423 + OAM_DMA_Transfer.md#FF46-DMA L4-L28.
Fiabilite : communautaire
Impact code : bus io_write dispatch ; trigger audio (NRx4) ; demarrage oam_dma du ppu (io_write 0xFF46)
Statut : CONFIRME

## Quirk STAT sur DMG + comparaison LYC/LY
Fait : ecrire FF41 pendant OAM scan, HBlank, VBlank ou LY=LYC peut declencher un interrupt LCD parasite sur les Game Boy monochromes (comportement comme si $FF fut ecrite pendant 1 M-cycle puis la valeur reelle) ; le GBC en mode DMG n'a pas ce quirk. Separement : FF45 est compare continuellement a LY ; quand egaux, le bit2 de STAT (LYC==LY) est pose et INT1 demande si selectionne (bits 6-3).
Source : refs/pandocs/src/STAT.md#Spurious-STAT-interrupts L38-L42 + #FF45-LYC L17-L20 + #FF41-STAT L25-L36.
Fiabilite : communautaire
Impact code : chemin d'ecriture ppu stat (io_write 0xFF41) ; comparaison ly/lyc a chaque tick ; demande interrupt INT1
Statut : CONFIRME

## Valeurs power-up : caveats du tableau
Fait : le tableau est enregistre a PC=$0100, donc apres la boot ROM (ce n'est pas necessairement l'etat de reset brut du hardware) ; DIV/STAT/LY dependent du header sur les colonnes SGB/CGB ; les registres marques CGB-only lisent $FF en non-CGB mode. Consequence pour notre emulateur DMG : utiliser la colonne "DMG / MGB" comme etat io par defaut a l'init systeme.
Source : refs/pandocs/src/Power_Up_Sequence.md#Hardware-registers L270-L352 (tableau + notes [^unk], [^unk_pad], [^cgb_only] L334-L350).
Fiabilite : testee sur ROM (valeurs obtenues des tests boot_hwio de Mooneye-GB, citees meme page)
Impact code : init systeme (etat de tous les registres io au reset)
Statut : CONFIRME

## Power-up non initialise / non specifie
Fait : OBP0 ($FF48) et OBP1 ($FF49) sont "left entirely uninitialized" a power-up (valeur souvent $00 ou $FF, jamais fiablee) ; la Wave pattern RAM FF30-$FF3F n'a pas d'entree dans le tableau ; le comportement de lecture des registres write-only ($FF13/$FF18/$FF1B/$FF1D et HDMA en CGB) n'est pas documente. Enregistre dans open_questions.md (section D_08).
Source : refs/pandocs/src/Power_Up_Sequence.md L342-L345 + L274-L332 + Hardware_Reg_List.md#Hardware-Registers L17-L66 (flag write-only).
Fiabilite : communautaire
Impact code : init systeme (io 0xFF30-$FF49, registres write-only)
Statut : UNKNOWN - to confirm

## Lecture DMG des adresses non attribuees et CGB-only
Fait : seules les lignes marques [^cgb_only] du tableau lisent $FF en non-CGB mode ($FF4D, $FF4F, $FF51-$FF56). FF4C (KEY0), $FF68-$FF6C (palette spec/data + OPRI) ne sont pas couvertes par cette note ; les adresses DMG entierement non attribuees ($FF03, $FF08-$FF0E, $FF15, $FF27-$FF2F, $FF4E, $FF57-$FF67, $FF6D-$FF6F, $FF71, $FF78-$FFFE) n'ont aucune valeur de lecture documentee dans cette version de pandocs - la regle "unused bits read high" suggere $FF mais n'est pas etablie explicitement pour elles. Enregistre dans open_questions.md (section D_08).
Source : refs/pandocs/src/Power_Up_Sequence.md L350 + Memory_Map.md#I-O-Ranges L24-L41 (liste des plages attribuees) + IR.md L41 (regle generale).
Fiabilite : communautaire
Impact code : defaut du bus io_read pour toute adresse FF00-$FFFF non mappee a un registre precis
Statut : UNKNOWN - to confirm

## Omitted (garde court pour le code)
Formules de frequence/LFSR audio, timings et details HBlank DMA CGB, colonnes SGB/DMG0 du tableau power-up, comportement bit-a-bit RP ; voir les pages pandocs citees ci-dessus si une tache ulterieure en a besoin.
