# Note 04a - Registres video DMG (FF40-FF4B)

Tache D_09. Sources : refs/pandocs/src/{LCDC, STAT, Rendering, Scrolling, Window, Palettes, OAM, Tile_Data, Tile_Maps, Accessing_VRAM_and_OAM, Interrupt_Sources, Interrupts, CGB_Registers}.md

## LCDC bits (FF40), tous R/W
Fait : b7 LCD+PPU enable (0 = off, VRAM/OAM pleinement accessibles) ; b6 window tile map ($9800-$9BFF ou $9C00-$9FFF) ; b5 window enable ; b4 zone des tiles BG+window (1 = $8000-$8FFF, 0 = $8800-$97FF ; les objets utilisent toujours l'adressage $8000) ; b3 BG tile map ($9800 ou $9C00) ; b2 taille OBJ (8x8 / 8x16) ; b1 enable OBJ ; b0 DMG : off/on de BG+window. Le PPU ne verrouille jamais LCDC, chaque bit est modifiable en cours de frame.
Source : refs/pandocs/src/LCDC.md#FF40 — LCDC: LCD control (liste des bits) + sous-sections LCDC.7 a LCDC.0 ; section "Using LCDC"
Fiabilite : communautaire
Impact code : puce8gb-core, module video (registre lcdc + configuration du rendu)
Statut : CONFIRME

## STAT bits et modes PPU 0-3 avec durees de ligne
Fait : FF41 : b6..b3 = select d'interrupt (LYC=LY, mode 2, mode 1, mode 0), R/W ; b2 LYC==LY read-only (mise a jour continue) ; b1-b0 PPU mode read-only (rapporte 0 si le PPU est desactive). Durees en dots : mode 2 OAM scan = 80 dots (= 20 M-cycles DMG) ; mode 3 pixels = 172 a 289 dots ; mode 0 HBlank = 376 - duree(mode 3) dots ; mode 1 VBlank = 4560 dots (10 lignes, = 1140 M-cycles DMG). Sur DMG : 4 dots par M-cycle.
Source : refs/pandocs/src/STAT.md#FF41 — STAT: LCD status ; refs/pandocs/src/Rendering.md section "PPU modes" (tableau) + section "Terminology" ("4 dots per Normal Speed M-cycle")
Fiabilite : communautaire
Impact code : puce8gb-core, module video (pu mode par scanline + timing de frame)
Statut : CONFIRME

## LY (FF44), comportement general
Fait : FF44 read-only ; contient 0 a 153 ; valeurs 144-153 = VBlank. La valeur est stable pendant toute une ligne (le HDMA HBlank ne transfert que pour LY=0-143). A chaque debut de scanline le PPU teste WY == LY pour poser la condition Y du window ; pendant l'OAM scan il compare LY a la coordonnee Y de chaque objet (limite 10 objets/scanline, selection $FE00 -> $FE9F).
Source : refs/pandocs/src/STAT.md#FF44 — LY: LCD Y coordinate [read-only] ; refs/pandocs/src/CGB_Registers.md section "Bit 7 = 1 — HBlank DMA" (L77-78) ; refs/pandocs/src/OAM.md section "Selection priority"
Fiabilite : communautaire
Impact code : puce8gb-core, module video (compteur ly + comparaisons WY/LYC/objet)
Statut : CONFIRME

## Regle d'increment de LY (FF44)
Fait : refs/pandocs ne dit pas a quel point precis du frame FF44 change de valeur (fin de scanline, debut du mode suivant, etc.) ; on ne deduit que la stabilite par ligne (HDMA + condition Y du window testes en debut de scanline). A confirmer.
Source : absence d'enonce dans refs/pandocs (grep "LY" sur src/*.md) ; seule indication indirecte CGB_Registers.md section "Bit 7 = 1 — HBlank DMA"
Fiabilite : deduite
Impact code : puce8gb-core, module video (compteur ly)
Statut : UNKNOWN - to confirm

## LYC (FF45), comparaison et trigger STAT-LY
Fait : FF45 R/W ; le PPU compare en permanence LYC et LY ; si egaux, b2 de FF41 est pose et un interrupt STAT est demande si b6 = 1. Usage documente : WY = LYC + interrupt LY=LYC pour masquer les objets sur la ligne du window (text box).
Source : refs/pandocs/src/STAT.md#FF45 — LYC: LY compare ; refs/pandocs/src/Interrupt_Sources.md section "Using the STAT interrupt"
Fiabilite : communautaire
Impact code : puce8gb-core, module video (bit stat 2 + IF bit 1)
Statut : CONFIRME

## Ligne d'interrupt STAT et regle de blocage
Fait : Les conditions modes 0-2 et LYC=LY sont OR'ees dans une ligne d'interrupt partagee, chacune seulement si son bit FF41.b3-b6 = 1 ; l'interrupt est demande sur la tranche montant (low -> high) de cette ligne. Si deux sources consecutives sont enablees (ex. mode 0 + mode 1), pas de retour a low entre les deux : un seul interrupt est emette ("STAT blocking"). L'handler ne s'execute que si IE bit 1 = 1 et IME = 1 ; le flag IF bit 1 peut aussi etre pose par ecriture sur FF0F.
Source : refs/pandocs/src/Interrupt_Sources.md section "INT $48 — STAT interrupt" (edge + blocage, test ROM mooneye ppu/stat_irq_blocking) ; refs/pandocs/src/Interrupts.md sections "IME" et "FFFF — IE: Interrupt enable"
Fiabilite : communautaire (testee sur ROM via roms/test-roms/mooneye-test-suite/acceptance/ppu/stat_irq_blocking.gb)
Impact code : puce8gb-core, module interrupt (ligne stat + IF bit 1)
Statut : CONFIRME

## SCY / SCX (FF42-FF43), souse-pixel scrolling
Fait : FF42 = SCY, FF43 = SCX ; top-left du viewport 160x144 dans la carte BG 256x256, valeurs 0-255 avec wrap (bottom := (SCY+143) % 256 ; right := (SCX+159) % 256). Les 3 bits bas de SCX ne sont lus qu'en debut de scanline (decalage souse-pixel initial, mode 3 s'allonge de SCX % 8 dots) ; les bits hauts et SCY sont re-lus a chaque tile fetch, d'ou les effets "wavy". Pre-CGB-D : SCY lu une fois par bitplane.
Source : refs/pandocs/src/Scrolling.md section "FF42–FF43 — SCY, SCX: Background viewport Y position, X position" + section "Mid-frame behavior" ; refs/pandocs/src/Rendering.md section "Mode 3 length"
Fiabilite : communautaire
Impact code : puce8gb-core, module video (registres scroll re-lus par tile fetch)
Statut : CONFIRME

## WY / WX (FF4A-FF4B), enable + position window
Fait : Le window est visible si 0 <= WX <= 166 et 0 <= WY <= 143 ; WX=7, WY=0 couvre tout l'ecran depuis le coin haut-gauche. "Condition Y" : reset a chaque VBlank, posee a la premiere scanline ou WY == LY (puis maintenue). Un compteur de pixels (initialise a 0, pre-incremente de 7 avant la premiere pixel) declenche le debut du window quand il atteint WX ; la ligne du tilemap window s'incremente alors. WX=0 : le window debute avant le fine scroll, decalle vers la gauche de SCX % 8 pixels.
Source : refs/pandocs/src/Window.md section "FF4A–FF4B — WY, WX: Window Y position, X position plus 7" + section "Window rendering criteria" ; enable via LCDC bit 5 (refs/pandocs/src/LCDC.md#LCDC.5 — Window enable)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (module window + condition y)
Statut : CONFIRME

## BGP / OBP0 / OBP1 (FF47-FF49), format des palettes DMG
Fait : 3 registres DMG-only ; chacun = 4 entrees de 2 bits (b7-b6 = palette id 3, b5-b4 = id 2, b3-b2 = id 1, b1-b0 = id 0). BGP s'applique aux tiles BG et window ; OBP0/OBP1 pour les objets selon le bit 4 des attributs OAM (les 2 bits bas y sont ignores, index 0 transparent). Mapping : 0 = blanc, 1 = gris clair, 2 = gris fonce, 3 = noir.
Source : refs/pandocs/src/Palettes.md sections "FF47 — BGP" et "FF48–FF49 — OBP0, OBP1" (tableau de mapping des couleurs)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (lookup palette 2 bits -> couleur)
Statut : CONFIRME

## Layout d'une entree OAM ($FE00-$FE9F), 40 x 4 octets
Fait : Byte 0 = Y : position verticale + 16 (Y=16 -> ligne 0 ; Y=0 ou Y>=160 masque l'objet) ; byte 1 = X : position horizontale + 8 (X=0 ou X>=168 off-screen mais compte dans la limite de 10 objets/scanline) ; byte 2 = tile index $8000-$8FFF (mode 8x16 : bit 0 ignore, tile haute NN & $FE, tile basse NN | $01) ; byte 3 attributs : b7 priority (1 = couleur BG/window dessinee par-dessus l'OBJ), b6 Y-flip, b5 X-flip, b4 DMG palette (0=OBP0, 1=OBP1).
Source : refs/pandocs/src/OAM.md sections "Byte 0 — Y Position" a "Byte 3 — Attributes/Flags" ; section "Selection priority" (limite 10 objets)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (struct entree oam + selection par ligne)
Statut : CONFIRME

## Readback OAM selon mode PPU
Fait : Le CPU lit/ecrit OAM avec les vraies valeurs seulement pendant modes 0 et 1 ; pendant mode 2 le PPU scanne l'OAM et une lecture CPU retourne des donnees non definies (typiquement $FF) ; hors modes 0-1 un DMA surpasse le PPU pour l'OAM et ce dernier lit alors $FF. VRAM est accessible en modes 0-2, jamais pendant mode 3.
Source : refs/pandocs/src/Accessing_VRAM_and_OAM.md sections "OAM (memory area at $FE00-$FE9F) is accessible during Modes 0-1" et "VRAM ... Modes 0-2" ; refs/pandocs/src/Rendering.md section "PPU modes" (colonne "Accessible video memory")
Fiabilite : communautaire
Impact code : puce8gb-core, module bus (gate lecture/ecriture oam+vram selon pu mode)
Statut : CONFIRME

## Tile data format (8x8), ordre des bits
Fait : Chaque tile = 16 octets ; chaque ligne de pixels = 2 octets : le premier porte le bit bas du color id, le second le bit haut ; dans un octet, bit 7 = pixel le plus a gauche, bit 0 = le plus a droite. Les 2 bits combines donnent l'index de palette 0-3 (id 3 = noir, i.e. "foreground" sur DMG).
Source : refs/pandocs/src/Tile_Data.md section "Data format" ; refs/pandocs/src/Palettes.md section "FF47 — BGP" (mapping des couleurs)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (tile fetch -> pixel fifo)
Statut : CONFIRME

## Selection des tile maps (2 banques, BG vs window)
Fait : Deux cartes 32x32 en VRAM ($9800-$9BFF et $9C00-$9FFF), entrees = index de tile sur 1 octet (256x256 px par carte). Le BG utilise la banque choisie par LCDC.b3, le window celle de LCDC.b6 ; les deux peuvent partager la meme banque. La zone des tiles est commune a BG+window via LCDC.b4 ($8000-$8FFF ou $8800-$97FF), sauf objets qui restent en adressage $8000.
Source : refs/pandocs/src/Tile_Maps.md sections "VRAM Tile Maps" et "Tile Indexes" ; refs/pandocs/src/LCDC.md#LCDC.3 — BG tile map area, #LCDC.6 — Window tile map area, #LCDC.4 — BG and Window tile data area
Fiabilite : communautaire
Impact code : puce8gb-core, module video (pointeurs de map bg/window)
Statut : CONFIRME

## Spurious STAT interrupts (quirk DMG a l'ecriture de FF41)
Fait : Sur le DMG monochrome, ecrire FF41 pendant OAM scan, HBlank, VBlank ou LY=LYC peut declencher un interrupt parasite comme si $FF avait ete ecrit un M-cycle puis la valeur reelle le M-cycle suivant ; le GBC en mode DMG n'a pas ce quirk (jeux Road Rash et Xerd no Densetsu).
Source : refs/pandocs/src/STAT.md section "Spurious STAT interrupts"
Fiabilite : communautaire
Impact code : puce8gb-core, module video (quirk a l'ecriture de stat, DMG only)
Statut : CONFIRME
