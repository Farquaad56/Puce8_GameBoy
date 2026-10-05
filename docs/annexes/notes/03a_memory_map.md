# Note 03a - Memory map DMG (ranges, echo RAM, unusable area, PPU/DMA access)
Sources : refs/pandocs/src/Memory_Map.md, OAM_DMA_Transfer.md, Accessing_VRAM_and_OAM.md,
Rendering.md ; refs/pandocs/historical/{1995-Jan-28-GAMEBOY.txt, 1998-Mar-12-Gbspec.txt}.

## Adresse bus : 16 bits, map complete 0x0000-0xFFFF avec tailles
Fait : le bus d'adresse a 16 bits et adresse ROM/RAM/I-O. Plages : 0000-3FFF = ROM bank 0 (16 Ko, fixe) ; 4000-7FFF = ROM banks 1-N (16 Ko, commutables via MBC) ; 8000-9FFF = VRAM (8 Ko, banks 0/1 en mode CGB) ; A000-BFFF = RAM externe cartrouche (8 Ko) ; C000-CFFF et D000-DFFF = WRAM (4 Ko chacun, bank D commutable vers 1-7 en mode CGB) ; E000-FDFF = echo RAM (miroir de C000-DDFF, usage interdit) ; FE00-FE9F = OAM ; FEA0-FEFF = inutilisable (usage interdit) ; FF00-FF7F = registres I-O ; FF80-FFFE = HRAM ; FFFF = registre IE.
Source : refs/pandocs/src/Memory_Map.md#memory-map (L3-L18)
Fiabilite : communautaire
Impact code : puce8gb-core - routage bus.read / bus.write par plage d'adresse vers rom, vram, wram, io, hram.
Statut : CONFIRME

## Echo RAM E000-FDFF : miroir du WRAM courant, usage interdit
Fait : seules les 13 bits bas de l'adresse sont connects ; l'adresse reboucle sur le bank WRAM en cours (fixe par un registre interne), donc tout read ou write a E000-FDFF a exactement le meme effet que sur C000-DDFF. Nintendo interdit cette plage ; le comportement est confirme sur toutes les consoles officielles, mais certaines cartouches flash la font entrer en conflit avec la SRAM cartrouche (A000-BFFF). Test logiciel : ecrire une valeur (pas 0x00/0xFF) puis verifier qu'elle est miroiree dans l'echo RAM et absente de la SRAM cartrouche.
Source : refs/pandocs/src/Memory_Map.md#echo-ram (L133-L146)
Fiabilite : communautaire
Impact code : puce8gb-core - bus.read / bus.write : plage E000-FDFF redirigee vers le bank WRAM en cours.
Statut : CONFIRME

## Plage inutilisable FEA0-FEFF (128 octets) : lectures selon la revision
Fait : Nintendo interdit cette plage ; une lecture y renvoie 0xFF quand l'OAM est bloque, sinon selon la revision : sur DMG/MGB/SGB/SGB2 elle renvoie 0x00 hors OAM block, mais une lecture pendant le OAM block declenche le bug de corruption OAM ; sur CGB rev 0-D c'est une zone RAM unique masquee par une valeur propre a la revision ; sur rev E/AGB/AGS/GBP elle renvoie la moitie haute du byte bas d'adresse deux fois (FFAx -> 0xAA, FFBx -> 0xBB).
Source : refs/pandocs/src/Memory_Map.md#fea0feff-range (L148-L162)
Fiabilite : communautaire
Impact code : puce8gb-core - bus.read DMG : plage FEA0-FEFF renvoie 0x00, ou 0xFF avec effet corruption OAM si l'OAM est bloque.
Statut : CONFIRME

## VRAM 8000-9FFF : acces CPU uniquement en modes PPU 0/1/2
Fait : le CPU n'accede a la VRAM que pendant HBlank (mode 0), VBlank (mode 1) et OAM search (mode 2). A l'interieur d'un mode, la fenetre dure seulement quelques cycles apres un test du bit 1 de STAT ($FF41), moins que la duree du mode 2 ; aucun interrupt ne doit survenir entre la boucle d'attente et l'acces. Durations : mode 0 = 376 - duree du mode 3, mode 1 = 4560 dots (10 lignes), mode 2 = 80 dots, mode 3 = 172 a 289 dots ; en mode 3 aucune memoire video n'est accessible. A l'affichage desactive (LCDC bit 7 a 0), VRAM et OAM sont accessibles.
Source : refs/pandocs/src/Accessing_VRAM_and_OAM.md#warning + #vram-memory-area-at-8000-9fff-is-accessible-during-modes-0-2 (L4-L52) ; refs/pandocs/src/Rendering.md#ppu-modes (L32-L43)
Fiabilite : communautaire
Impact code : puce8gb-core - porte d'acces VRAM selon mode PPU et cycle en mode 0 ; puce8gb-ppu expose le mode et la position dot.
Statut : CONFIRME

## OAM FE00-FE9F : acces CPU uniquement en modes PPU 0/1, DMA prioritaire
Fait : l'OAM est accessible directement ou via un DMA ($FF46) seulement pendant HBlank (mode 0) et VBlank (mode 1). Hors de ces modes, le DMA a priorite sur le PPU pour OAM ; quand le DMA est actif hors mode 0/1, le PPU lit 0xFF depuis OAM.
Source : refs/pandocs/src/Accessing_VRAM_and_OAM.md#oam-memory-area-at-fe00-fe9f-is-accessible-during-modes-0-1 (L54-L68)
Fiabilite : communautaire
Impact code : puce8gb-core - porte d'acces OAM selon mode PPU 0/1 ; le DMA surpasse le PPU quand les deux se disputent OAM.
Statut : CONFIRME

## HRAM FF80-FFFE (127 octets) + IE FFFF : seul acces CPU pendant un OAM DMA (DMG)
Fait : la HRAM couvre FF80-FFFE, FFFF = IE. Sur DMG, pendant un OAM DMA le CPU ne peut acceder qu'a la HRAM ; il faut donc copier une routine dans la HRAM et demarrer puis attendre le transfert depuis l'interieur de la HRAM. Sur CGB, cartrouche et WRAM sont sur des bus separes (ROM/SRAM cartrouche accessible pendant un DMA depuis WRAM, inversement), mais comme le stack vit en WRAM on attend quand meme dans la HRAM ; un interrupt pendant le DMA est dangereux car il pousse un return address en WRAM puis fetch le handler depuis ROM.
Source : refs/pandocs/src/Memory_Map.md#memory-map (L17-L18) ; refs/pandocs/src/OAM_DMA_Transfer.md#oam-dma-bus-conflicts (L19-L34) ; historical/1998-Mar-12-Gbspec.txt (L1669-L1670)
Fiabilite : communautaire
Impact code : puce8gb-core - bus gate : quand dma.active, tout acces CPU sauf HRAM renvoie des valeurs non fiablees.
Statut : CONFIRME

## OAM DMA ($FF46) : declenchement et plage de source en CONFLIT entre sources
Fait : ecrire dans $FF46 demarre un DMA ; la valeur ecrue vaut l'adresse source divisee par 0x100, d'ou une source XX00-XX9F vers la destination FE00-FE9F. pandocs limite XX a 0x00-0xDF (source dans 0x0000-0xDFFF) tandis que les specs officielles indiquent "internal ROM or RAM ($0000-$F19F)" avec un pas de 0x100 ; des ROM test existent pour des sources a 0x9000 (VRAM) et 0xE000 (echo RAM), hors de la plage pandocs.
Source : refs/pandocs/src/OAM_DMA_Transfer.md#ff46-dma-oam-dma-source-address-and-start (L4-L13) ; refs/pandocs/historical/1995-Jan-28-GAMEBOY.txt (L587-L605) ; historical/1998-Mar-12-Gbspec.txt (L1643-L1663)
Fiabilite : CONFLIT - communautaire vs officielle
Impact code : puce8gb-core - dma.source_bank = valeur de $FF46 ; a trancher par roms/test-roms/gbmicrotest/dma_0x9000.gb et dma_0xE000.gb.
Statut : CONFLIT

## OAM DMA : duree du bus en CONFLIT, 160 M-cycles = 640 dots (vitesse normale)
Fait : le transfert tient 160 M-cycles = 640 dots (1.4 lignes) a vitesse normale, soit 320 dots (0.7 ligne) en CGB double speed mode - bien plus rapide qu'une copie par CPU. Les specs officielles donnent "takes 160 nano-seconds" (1995) puis "microseconds" (1998), unites incoherentes avec la duree mesuree, d'ou un conflit sur l'ecelle de temps a trancher par ROM test.
Source : refs/pandocs/src/OAM_DMA_Transfer.md#ff46-dma-oam-dma-source-address-and-start (L15-L17) ; refs/pandocs/historical/1995-Jan-28-GAMEBOY.txt (L590-L591) ; historical/1998-Mar-12-Gbspec.txt (L1643-L1647)
Fiabilite : CONFLIT - communautaire vs officielle
Impact code : puce8gb-core - dma.active dure 160 cycles et gate le bus ; a trancher par roms/test-roms/mooneye-test-suite/acceptance/oam_dma_timing.gb (+ gbmicrotest/dma_timing_a.gb).
Statut : CONFLIT

## PPU pendant un OAM DMA : lecture de l'OAM corrompue, effet par mode
Fait : pendant le DMA, le PPU ne lit pas correctement l'OAM. En mode 2 (OAM scan), la majorite des revisions lisent chaque objet comme hors ecran et donc masque sur cette ligne ; en mode 3 (rendu), le PPU lit le mot de 16 bits que le DMA ecrivait au moment du fetch, ce qui donne un numero de tile et des attributs incorrects pour les objets deja declares dans la plage. C'est pourquoi on execute le DMA pendant VBlank (mode 1) ou, volontairement, sur modes 2/3.
Source : refs/pandocs/src/OAM_DMA_Transfer.md#oam-dma-bus-conflicts (L39-L54)
Fiabilite : communautaire
Impact code : puce8gb-ppu - en mode 2 avec dma.active les objets sont traites hors ecran ; en mode 3 le PPU lit les mots que le DMA ecrira.
Statut : CONFIRME

## Lectures de zones non mappees ou bloquees par la PPU : valeur renvoyee
Fait : quand la PPU accede a une memoire video, cette memoire est inaccessible au CPU : toute ecriture est ignoree (donnee inchangee) et toute lecture renvoie une donnee indefinie, habituellement 0xFF. C'est aussi le cas de l'interval FEA0-FEFF pendant un OAM block (renvoie 0xFF, voir plus haut).
Source : refs/pandocs/src/Rendering.md#ppu-modes (L32) ; refs/pandocs/src/Accessing_VRAM_and_OAM.md#warning (L9-L10) ; refs/pandocs/src/Memory_Map.md#fea0feff-range (L148-L162)
Fiabilite : communautaire
Impact code : puce8gb-core - bus.read : VRAM/OAM bloquees par la PPU renvoie 0xFF ; ecritures ignorees.
Statut : CONFIRME

## Fetch d'instruction vs dernier cycle de l'instruction precedente (DMA $FF46)
Fait : aucun document du corpus ne precise si le fetch de l'opcode suivant chevauche ou non le dernier M-cycle de l'instruction en cours. Seules indications indirectes : le DMA "starts right after instruction" apres une ecriture de $FF46, et la variante ret z existe pour eviter une lecture du stack sur le dernier M-cycle du DMA, ce qui suppose un alignement precis des cycles a verifier.
Source : refs/pandocs/src/OAM_DMA_Transfer.md#best-practices (L58-L91)
Fiabilite : deduite
Impact code : puce8gb-core - cycle de demarrage dma.active et chevauchement du fetch CPU avec les bus cycles ; a valider sur des ROM test.
Statut : UNKNOWN - to confirm
