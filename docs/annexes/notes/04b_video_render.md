# Note 04b - Rendu video DMG (modes PPU, FIFO, window, sprites)

Tache D_10. Sources : refs/pandocs/src/{Rendering, pixel_fifo, Window, OAM, LCDC, STAT, Accessing_VRAM_and_OAM, OAM_Corruption_Bug}.md
Complete 04a_video_regs.md (registres). Ici : comportement du PPU en cours de frame.

## Durees des modes PPU par scanline
Fait : Une frame = 154 scanlines ; les 144 premieres sont dessinees, les 10 dernieres = VBlank. Par scanline dessinee (vitesse normale, 4 dots/M-cycle) : mode 2 OAM scan = 80 dots (= 20 M-cycles, le PPU lit une ligne OAM par M-cycle sur 20 lignes) ; mode 3 pixels = 172 a 289 dots ; mode 0 HBlank = 376 - duree(mode 3), donc total d'une scanline dessinee = 376 dots. Mode 1 VBlank = 4560 dots repartis sur les 10 lignes (soit 456 dots/ligne, plus long que 376). Pendant mode 3 la VRAM/OAM est inaccessible au CPU (lecture $FF, ecriture ignoree) ; modes 0-2 : VRAM accessible, OAM seulement en 0-1.
Source : refs/pandocs/src/Rendering.md section "Terminology" (L6 frame=154 scanlines, L12 4 dots/M-cycle) + section "PPU modes" (tableau L34-39) ; refs/pandocs/src/OAM_Corruption_Bug.md section "Corruption Patterns" (L57-59 : une ligne OAM par M-cycle, 20 lignes)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (pu mode par dot + timing de frame 144x376 + VBlank 4560)
Statut : CONFIRME

## Duree variable du mode 3 (penalties)
Fait : Minimum mode 3 = 160 pixels + 12 dots de fetch initial = 172 dots. Trois sources allongent le mode 3 (et raccourcissent d'autant le mode 0, la scanline restant a duree fixe) : (a) defilement BG - pause de SCX % 8 dots au tout debut du mode 3 pendant que les pixels gauches sont jetes ; (b) window - penalite de 6 dots apres le dernier pixel non-window, pour preparer le fetcher BG vers le window ; (c) objets - chaque objet dessine (memme partiellement) coute une penalite de 6 a 11 dots. La duree max du mode 3 est 289 dots.
Source : refs/pandocs/src/Rendering.md section "Mode 3 length" (L41-54) + note [^first12] (L70 : les 12 dots = 2 fetchs de tile en debut de ligne) ; bornes 172-289 dans le tableau "PPU modes" (L37)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (calcul duree mode 3 par scanline selon SCX/window/sprites)
Statut : CONFIRME

## Algorithme de penalite OBJ (6 a 11 dots)
Fait : Seule la colonne du pixel le plus a gauche de l'objet compte ("The Pixel", transparent ou non). On determine le tile BG/window qui contient The Pixel. Si ce tile n'a pas encore ete considere par un objet precedent : on compte les pixels de ce tile strictement a droite de The Pixel, on soustrait 2 (penalite = 0 si negatif), puis on ajoute une penalite fixe de 6 dots (fetch du tile OBJ). Exception : un objet a X=OAM (completement hors ecran a gauche) coute toujours 11 dots, quel que soit SCX. Les objets se considerent de gauche a droite, exaequo par index OAM croissant.
Source : refs/pandocs/src/Rendering.md section "OBJ penalty algorithm" (L56-67) + note [^order] (L74 : ordre gauche->droite, ties par adresse OAM la plus basse)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (penalite mode 3 par objet dessine)
Statut : CONFIRME

## FIFO de pixels (BG et OBJ independants)
Fait : Deux FIFO distincts, un pour les pixels BG/window et un pour les objets ; ils ne sont pas partages et ne se melangent qu'au pop. Chaque FIFO tient jusqu'a 16 pixels et le fetcher garantit toujours au moins 8 pixels (8 requis pour rendre). Les deux FIFO sont manipules uniquement pendant mode 3, et tous deux sont vides au debut du mode 3. Un pixel du FIFO porte : couleur 0-3, palette (DMG : seulement pour objets), priorite BG (valeur du bit de priorite OAM).
Source : refs/pandocs/src/pixel_fifo.md section "Introduction" (L9-23) + section "Mode 3 Operation" (L125-129 : FIFO vides au debut mode 3)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (deux fifo bg/obj de 16 px, min 8)
Statut : CONFIRME

## Etapes du pixel fetcher (5 etapes, cout en dots)
Fait : Le fetcher lit une ligne de 8 pixels BG/window. Il a 5 etapes dans l'ordre : Get tile, Get tile data low, Get tile data high, Sleep, Push. Les quatre premieres coutent 2 dots chacune ; la cinquieme (Push) est tentee chaque dot jusqu'a reussite. "Get Tile Data High" pousse aussi une ligne de pixels au FIFO, si bien qu'il y a 3 chances de pousser des pixels par cycle complet du fetcher.
Source : refs/pandocs/src/pixel_fifo.md section "FIFO Pixel Fetcher" (L28-36) + sections "Get Tile Data High" (L79-88) et "Push" (L90-102)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (machine a etapes du fetcher bg, 2 dots/etape sauf push)
Statut : CONFIRME

## Get Tile : coordonnees et choix de tilemap
Fait : Par defaut la tilemap est $9800 ; elle devient $9C00 si LCDC.3=1 et X hors window, ou si LCDC.6=1 et X dans le window. Coordonnee X du fetcher (BG) = ((SCX/8) + fetcherX) & $1F (donc 0-31) ; coordonnee Y = (scanline courante + SCY) & 255 (0-255). Get Tile Data Low verifie LCDC.4 (zone de tiles), la VRAM bank et le flip vertical, puis lit les donnees du tile. Si l'acces VRAM du PPU est bloque a ce moment, la valeur lue est $FF (tile index ou donnee).
Source : refs/pandocs/src/pixel_fifo.md section "Get Tile" (L38-74) + section "Get Tile Data Low" (L69-77) + section "VRAM Access" (L108-123)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (calcul tile bg/window par pixel + relecture SCX/SCY)
Statut : CONFIRME

## OAM scan (mode 2), regle des 10 objets/ligne
Fait : Pendant le mode 2, le PPU compare LY a la coordonnee Y de chaque objet (taille selon LCDC.2) en balayant l'OAM sequentiellement de $FE00 a $FE9F, et retient les 10 premiers objets convenablement positionnes sur cette ligne. Seule la coordonnee Y est testee : un objet hors ecran par X (X=0 ou X>=168) compte quand meme dans la limite des 10 ; pour ne pas bloquer d'autres objets, masquer via Y (Y=0 ou Y>=160). Le PPU lit une ligne OAM de 8 octets par M-cycle.
Source : refs/pandocs/src/OAM.md section "Selection priority" (L80-93) + intro (L1-10 : 40 objets max, 10/ligne) ; refs/pandocs/src/OAM_Corruption_Bug.md section "Corruption Patterns" (L57-59)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (selection des <=10 sprites par scanline pendant mode 2)
Statut : CONFIRME

## Priorite de dessin des sprites DMG + melange pixel
Fait : En mode non-CGB (DMG), la priorite entre objets opaqes est determinee par la coordonnee X : plus X est petit, plus l'objet est dessine au-dessus ; a X egal, l'index OAM le plus bas gagne. Au melange d'un pixel : si les deux FIFO ont un pixel et que le pixel OBJ n'est pas transparent (couleur != 0) avec LCDC.1=1, le pixel OBJ gagne quand sa priorite BG est egale ou superieure a celle du pixel BG ; sinon le pixel BG s'affiche. Sur DMG la couleur vient de BGP (BG) ou OBP0/OBP1 selon la propriete palette du pixel OBJ.
Source : refs/pandocs/src/OAM.md section "Drawing priority" (L98-108) + refs/pandocs/src/Rendering.md note [^order] (L74) ; refs/pandocs/src/pixel_fifo.md section "Pixel Rendering" (L201-225)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (resolution priorite obj vs bg + lookup palette DMG)
Statut : CONFIRME

## Regles du window (debut, ligne active, cas limites)
Fait : Le window est visible si 0<=WX<=166 et 0<=WY<=143 ; WX=7,WY=0 couvre tout l'ecran. "Condition Y" : reinitialisee a chaque VBlank, posee au debut de la premiere scanline ou WY==LY puis maintenue. Un compteur (init 0 par scanline) s'incremente a chaque pixel rendu et aussi 7 fois avant le premier pixel ; quand il atteint WX avec condition Y vraie et LCDC.5=1, le rendu BG repart de la ligne active du tilemap window qui s'incremente alors. WX=0 : le window debute avant le fine scroll, decalle a gauche de SCX%8 pixels (mode 3 raccourci de 1 dot si SCX&7>0). Bug DMG : WX=166 fait couvrir tout l'ecran avec un decalage vertical d'une scanline. Si le window est desactive via LCDC mais que les conditions sont reunies et qu'il aurait debute pile sur une frontiere de tile BG, un pixel unique de couleur 0 est insere, decalant le reste de la ligne (cas Star Trek 25th anniversary).
Source : refs/pandocs/src/Window.md sections "FF4A-FF4B" (L3-8) et "Window rendering criteria" (L18-46) ; refs/pandocs/src/pixel_fifo.md section "The Window" (L131-141 : FIFO bg vide + fetcher reset a l'etape 1, WX=0 raccourcit de 1 dot)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (module window : condition y + compteur wx + ligne active)
Statut : CONFIRME

## LCD off / on (LCDC.7), ecran et acces memoire
Fait : LCDC.7=0 coupe le LCD et le PPU ; l'acces complet a VRAM/OAM est immediatement disponible, et les bits de mode de STAT rapportent 0. L'ecran devient blanc (sur DMG un blanc "plus clair" que la couleur #0). Couper le LCD en dehors du VBlank peut graver une ligne noire ; il faut donc desactiver pendant VBlank seulement. Quand le LCD est coupe, les lectures VRAM du PPU sont bloquees (valeur $FF) et l'acces palette CGB aussi. A la reactivation, le PPU repart immediatement mais l'ecran reste blanc pendant toute la premiere frame.
Source : refs/pandocs/src/LCDC.md section "LCDC.7 - LCD enable" (L22-46) ; refs/pandocs/src/STAT.md section "FF41" (bit PPU mode rapporte 0 si PPU desactive, L35) ; refs/pandocs/src/pixel_fifo.md sections "VRAM Access" (L112) et "CGB Palette Access" (L233)
Fiabilite : communautaire
Impact code : puce8gb-core, module video (etat lcd on/off + gate acces vram/oam + ecran blanc)
Statut : CONFIRME

## Premiere ligne apres LCD on (position de scanline)
Fait : refs/pandocs ne precise pas a quelle position de frame le PPU repart quand LCDC.7 repasse a 1 (si LY est reinitialisee a 0 ou si la frame en cours continue), ni ce qui s'affiche exactement sur cette premiere ligne ; il n'est etabli que que le PPU repart immediatement et que l'ecran reste blanc pendant toute la premiere frame. A confirmer.
Source : absence d'enonce dans refs/pandocs (grep "re-enab"/"first frame" sur src/*.md) ; seule indication LCDC.md section "LCDC.7 - LCD enable" (L45-46)
Fiabilite : deduite
Impact code : puce8gb-core, module video (comportement a la reactivation du lcd)
Statut : UNKNOWN - to confirm
