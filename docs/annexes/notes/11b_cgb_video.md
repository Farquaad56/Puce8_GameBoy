# Note 11b - Video CGB (palettes, attributs, priorites)

Sources : refs/pandocs/src/Palettes.md (PAL), Tile_Maps.md (TMAP), OAM.md (OAMD), LCDC.md (LCDC), Power_Up_Sequence.md (PWUP), CGB_Registers.md (CGBR).

Fait : une couleur CGB est en RGB555 : 3 composantes de 5 bits (rouge, vert, bleu) dans les 15 bits bas d'un mot de 16 bits ; le bit 15 est ignore par le rendu (le blanc canonique est $7FFF, $FFFF est traite pareil).
Source : PAL L42-L43 ; PAL L58-L60
Statut : CONFIRME

Fait : les deux octets d'une couleur sont stockes en little-endian (octet bas en premier).
Source : PAL L49
Statut : CONFIRME

Fait : il y a 8 palettes BG (fond et fenetre) et 8 palettes OBJ de 4 couleurs ; chaque banque de palette (CRAM) fait 64 octets (2 x 4 x 8) ; la palette est choisie par les attributs de map ou d'OAM.
Source : PAL L34 ; PAL L47 ; PAL L62-L63
Statut : CONFIRME

Fait : la CRAM n'est pas dans la memoire adressable ; chaque banque s'acces par un registre d'index et un registre de donnees ; pendant le mode 3 les ecritures de donnees sont ignorees et les lectures renvoient $FF.
Source : PAL L54-L56
Statut : CONFIRME

Fait : BCPS/BGPI $FF68 : bit 7 = auto-increment (1 = actif), bits 5-0 = adresse de l'octet (0-63) ; le registre d'index reste accessible hors VBlank et HBlank.
Source : PAL L65-L75
Statut : CONFIRME

Fait : BCPD/BGPD $FF69 : l'adresse de BCPS est incrementee apres chaque ecriture (63 revient a 0), meme si l'ecriture echoue car la CRAM est inaccessible ; une lecture ne declenche jamais l'auto-increment.
Source : PAL L77-L82
Statut : CONFIRME

Fait : OCPS/OBPI $FF6A et OCPD/OBPD $FF6B fonctionnent comme BCPS et BCPD avec 64 octets separes pour les objets ; la couleur 0 d'une palette OBJ n'est jamais utilisee (transparente).
Source : PAL L84-L88
Statut : CONFIRME

Fait : en mode CGB, une seconde map 32x32 d'attributs est dans la banque VRAM 1 aux memes adresses que la tile map de la banque 0 (1:$9800 donne les attributs de la tuile de 0:$9800).
Source : TMAP L20-L25
Statut : CONFIRME

Fait : attributs de BG map : bit 7 = priorite, bit 6 = flip Y, bit 5 = flip X, bit 3 = banque VRAM de la tuile, bits 2-0 = palette couleur (BGP0-7).
Source : TMAP L26-L35
Statut : CONFIRME

Fait : attributs d'OAM (octet 3) : bit 7 = priorite (1 = les index de couleur 1-3 du BG/fenetre passent devant l'objet), bit 6 = flip Y, bit 5 = flip X, bit 4 = palette DMG (hors mode CGB), bit 3 = banque VRAM (mode CGB), bits 2-0 = palette CGB (mode CGB).
Source : OAMD L53-L61
Statut : CONFIRME

Fait : en mode CGB, LCDC bit 0 est la priorite maitre du BG et de la fenetre : a 0, les objets passent toujours devant, quels que soient les bits de priorite d'OAM et de map ; a 1, la priorite suit les regles de BG vers OBJ.
Source : LCDC L108-L114
Statut : CONFIRME

Fait : regles BG vers OBJ en mode CGB : (1) index BG 0 = l'objet gagne ; (2) sinon LCDC bit 0 a 0 = l'objet gagne ; (3) sinon si les bits 7 d'attribut BG et d'attribut OAM sont tous deux a 0 = l'objet gagne ; (4) sinon le BG gagne.
Source : TMAP L41-L52
Statut : CONFIRME

Fait : le bit 7 d'attribut OAM donne la priorite a l'objet quand il est a 0 ; la priorite entre objets est resolue avant la priorite avec le BG.
Source : TMAP L72-L74 ; OAMD L113-L124
Statut : CONFIRME

Fait : priorite entre objets : en mode non CGB, plus petite coordonnee X = plus haute priorite ; en mode CGB, seule la position dans l'OAM compte (le plus tot gagne).
Source : OAMD L105-L109
Statut : CONFIRME

Fait : OPRI $FF6C bit 0 : 0 = priorite style CGB (position OAM), 1 = style DMG (coordonnee X) ; le boot ROM ecrit $01 pour un jeu DMG seulement.
Source : CGBR L250-L269 ; PWUP L90-L91
Statut : CONFIRME

Fait : l'effet de OPRI quand il est ecrit apres le demontage du boot ROM n'est pas verifie dans pandocs ("TO BE VERIFIED").
Source : CGBR L256-L260
Statut : UNKNOWN - to confirm

Fait : en mode de compatibilite (jeu DMG), le boot ROM ecrit $04 dans KEY0 pour activer le mode de compatibilite DMG du CPU, ecrit $01 dans OPRI pour la priorite OBJ style DMG, et ecrit les palettes de compatibilite ; pour un jeu CGB, l'octet de compatibilite est ecrit directement dans KEY0.
Source : PWUP L89-L91
Statut : CONFIRME

Fait : apres le boot ROM, toutes les couleurs de fond sont initialisees a blanc ; en mode de compatibilite, le fond utilise la palette BG 0 (l'attribut map etant probablement mis a zero) et les objets utilisent la palette OBJ 0 ou 1 selon le bit 4 de leurs attributs ; BGP, OBP0 et OBP1 indexent alors les palettes CGB au lieu des gris DMG.
Source : PAL L38 ; PWUP L111-L112
Statut : CONFIRME
