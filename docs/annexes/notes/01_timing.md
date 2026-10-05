# Note 01 - Horloge, PPU et cycles (DMG)

Note de reference pour le timing. DMG uniquement : on ignore le double speed CGB.
Unite de base = un dot (= un T-cycle). Voir aussi les cycles par instruction plus bas.

## Frequence maitresse et horloge CPU
Fait : L'horloge maitresse du DMG est 4194304 Hz (4,194304 MHz), soit 2^22 Hz.
Source : refs/pandocs/src/Specifications.md#Specifications (ligne "Master Clock") ; refs/pandocs/src/Rendering.md#Terminology ("dot = un intervalle de 2^22 Hz (env. 4,194 MHz)")
Fiabilite : officielle
Impact code : puce8gb-core - compteur de dots/clock du noyau, derive de toutes les horloges
Statut : CONFIRME

## Le dot vaut un T-cycle ; M-cycle = 4 dots
Fait : Sur le DMG (et CGB normal speed), un point/dot est la plus petite periode PPU et vaut exactement 1 T-cycle. A vitesse normale, 1 M-cycle (cycle machine) = 4 dots = 1048576 Hz (2^20).
Source : refs/pandocs/src/STAT.md#Terminology ; refs/pandocs/src/Rendering.md#Terminology ("4 dots per normal-speed M-cycle") ; refs/pandocs/src/Audio_Registers.md#Sound-Channel-1 ("clocked at 1048576 Hz, once per four dots")
Fiabilite : officielle
Impact code : puce8gb-core - conversion T-cycle <-> M-cycle dans le bus et l'audio (eviter de melanger les deux)
Statut : CONFIRME

## Dots par ligne d'ecran = 456
Fait : Une ligne ecran (scanline) dure 456 dots. La VBlank vaut 10 lignes = 4560 dots, donc 4560/10 = 456 dots/ligne ; le dessin PPU affiche aussi "456 dots" par ligne et un total de frame de 70224 dots.
Source : refs/pandocs/src/Rendering.md#PPU-modes (ligne mode 1 "4560 dots (10 scanlines)") ; refs/pandocs/src/imgs/src/ppu_modes_timing.svg ("456 dots", total "70224 dots")
Fiabilite : deduite
Impact code : puce8gb-core - compteur de X position du PPU, largeur d'une ligne
Statut : CONFIRME

## Lignes par frame (154) et periode VBlank
Fait : Une image complete dure 154 scanlines ; les 144 premieres sont affichees, les 10 suivantes sont la VBlank. LY tient de 0 a 153, avec 144-153 qui indiquent la VBlank.
Source : refs/pandocs/src/Rendering.md#Terminology ("un frame consiste en 154 scanlines") ; refs/pandocs/src/STAT.md#FF44-LY ("LY ... 0 to 153, values from 144 to 153 indicating the VBlank period")
Fiabilite : officielle
Impact code : puce8gb-core - compteur de Y position (LY) et generation du flag VBlank
Statut : CONFIRME

## Cycles par frame (70224) et frequence d'actualisation (~59,73 Hz)
Fait : Une frame complete = 456 dots x 154 lignes = 70224 dots/cycles. A la horloge maitresse, la frequence d'actualisation est 4194304/70224 = 59,7275... Hz (env. 59,73 Hz), soit ~16,74 ms/frame (le DMG tourne legerement sous 60 Hz).
Source : refs/pandocs/src/imgs/src/ppu_modes_timing.svg (total frame "70224 dots") ; refs/pandocs/src/Specifications.md#Specifications ("Vertical sync 59.73 Hz") ; refs/pandocs/src/Rendering.md#Terminology (~16,74 ms/frame). Controle : 456*154 = 70224 ; 4194304/70224 = 59,7275.
Fiabilite : deduite (valeurs de base CONFIRMEES)
Impact code : puce8gb-core - duree d'une frame pour le PPU et l'horloge des timers
Statut : CONFIRME

## Durees des modes PPU par ligne (DMG, vitesse normale)
Fait : Par ligne (456 dots total), la PPU passe par 4 modes : Mode 2 "OAM scan" = 80 dots ; Mode 3 "dessin des pixels" = variable entre 172 et 289 dots ; Mode 0 "HBlank, attente fin de ligne" = 376 - duree du mode 3 (donc la ligne = 80 + mode3 + mode0) ; Mode 1 VBlank = 4560 dots sur les 10 dernieres lignes. Le minimum du mode 3 est 172 = 160 pixels + 12 dots de fetch initial des tuiles ; les penalites (scroll SCX%8, fenetre +6, OBJ +6 a +11) allongent le mode 3 et raccourcissent d'autant le mode 0.
Source : refs/pandocs/src/Rendering.md#PPU-modes (tableau modes 2/3/0/1) ; refs/pandocs/src/Rendering.md#Mode-3-length (minimum 172 = 160+12, penalites)
Fiabilite : officielle
Impact code : puce8gb-core - etat des STAT (modes 0-3), timing de la PPU par ligne
Statut : CONFIRME

## Horloge du timer DIV (FF04) = 16384 Hz
Fait : Le registre DIV s'incremente a 16384 Hz sur le DMG (= une fois toutes les 256 M-cycles). Ce n'est pas un timer reprogrammable : il compte en permanence, independamment de TAC.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF04-DIV ("incremented at a rate of 16384Hz") ; memesfichier table "256 M-cycles -> 16384"
Fiabilite : officielle
Impact code : puce8gb-core - horloge du registre FF04 (timer)
Statut : CONFIRME

## Transfert OAM DMA = 160 M-cycles (640 dots)
Fait : Ecrire FF46 demarre un transfert OAM de 160 M-cycles = 640 dots (~1,4 lignes) a vitesse normale. Pendant ce temps la PPU ne lit pas correctement l'OAM et le CPU n'accede plus qu'a HRAM sur le DMG ; d'ou executer le DMA depuis HRAM dans VBlank.
Source : refs/pandocs/src/OAM_DMA_Transfer.md#FF46-DMA ("The transfer takes 160 M-cycles: 640 dots") ; memesfichier #OAM-DMA-bus-conflicts
Fiabilite : officielle
Impact code : puce8gb-core - etat du bus / PPU pendant un OAM DMA (FF46)
Statut : CONFIRME

## Cycles par instruction (unite = T-cycle/dot, pour les taches CPU suivantes)
Fait : Source unique des durees d'instruction : gbdev.io/gb-opcodes/Opcodes.json (copie locale docs/annexes/Opcodes.json), unite = T-cycle. Les ops conditionnelles listent deux valeurs [pris, non pris] : JR cc 12/8 ; JP cc 16/12 ; CALL cc 24/12 ; RETI 16 (opcode D9). Sans condition : JR 12, JP a16 16, JP HL 4, CALL 24, RET 16, RST 16.
Source : docs/annexes/Opcodes.json = gbdev.io/gb-opcodes/Opcodes.json (table "unprefixed" : C3=JP[16], E9=JP HL[4], CD=CALL[24], C9=RET[16], D9=RETI[16], 07..FF=RST[16])
Fiabilite : communautaire (reference gbdev)
Impact code : puce8gb-core - table de cycles du decodeur d'instructions (taches CPU futures)
Statut : CONFIRME

## Couts de prefixe CB deja inclus dans la table cbprefixed
Fait : Le cout du prefixe 0xCB est DEJA compte dans la table "cbprefixed" de Opcodes.json, il ne faut PAS rajouter l'entree PREFIX 4 cycles. Formes a registre = 8 dots ; forme memoire (HL) = 16 dots ; BIT n,(HL) = 12 dots (seuls cas a 12).
Source : docs/annexes/Opcodes.json (table "cbprefixed" : registres [8], (HL)-form [16] dont 24 entrees, BIT n,(HL) [12] = 8 entrees ; table "unprefixed" 0xCB=PREFIX[4])
Fiabilite : communautaire (reference gbdev)
Impact code : puce8gb-core - cycles des instructions CB (taches CPU futures)
Statut : CONFIRME

## OpCodes illegaux : CONFLIT JSON (4 dots) vs materiel veritable (lock up)
Fait : Opcodes.json liste 11 opcodes illegaux (D3 DB DD E3 E4 EB EC ED F4 FC FD) a 4 dots, mais le CPU SM83 veritable s'arrete/verrouille ("locks up") sur ces opcodes. Deux sources en desaccord : garder la valeur JSON (4 dots) pour le decodeur et modeler un lock-up pour l'emulation exacte ; une test ROM decide laquelle est exacte.
Source : docs/annexes/Opcodes.json (entrees ILLEGAL_*, cycles [4]) vs refs/pandocs/src/CPU_Comparison_with_Z80.md ("unused (-) opcodes will lock up the Game Boy CPU")
Fiabilite : communautaire vs officielle
Impact code : puce8gb-core - gestion des opcodes illegaux (taches CPU futures)
Statut : CONFLIT

## Test ROM qui tranche le conflit d'opcodes illegaux (a produire)
Fait : Aucune test ROM existante sous roms/test-roms/ ne teste les 11 opcodes illegaux ; il faudra en ajouter une (suite mooneye-test-suite ou dmg-acid2) qui execute D3 DB DD E3 E4 EB EC ED F4 FC FD et observe si la machine freeze. Statut : UNKNOWN - to confirm, pointe dans open_questions.md.
Source : inventaire roms/test-roms/ (aucun .gb dedie aux illegal opcodes) + refs/pandocs/src/CPU_Comparison_with_Z80.md#CPU-Instruction-Set (comportement lock up)
Fiabilite : testee sur ROM (a produire)
Impact code : puce8gb-core - decodeur d'instructions ; suite de tests non-regression
Statut : UNKNOWN - to confirm
