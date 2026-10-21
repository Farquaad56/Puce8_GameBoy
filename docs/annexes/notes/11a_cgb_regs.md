# Note 11a - Registres CGB (mode, banques, vitesse)

Sources : refs/pandocs/src/CGB_Registers.md (CGBR), Memory_Map.md (MMAP), Hardware_Reg_List.md (HWRL), The_Cartridge_Header.md (HDR), Power_Up_Sequence.md (PWUP).

Fait : le mode CGB est choisi par l'octet $0143 de l'en-tete : $80 = jeu CGB compatible monochrome, $C0 = jeu CGB seulement (le materiel ignore le bit 6, meme effet que $80).
Source : HDR L47-L57 ; CGBR L9-L16
Statut : CONFIRME

Fait : sans $80 ou $C0 en $0143, la CGB fonctionne en mode monochrome de compatibilite ("Non CGB") et les registres CGB ne sont pas debloques.
Source : CGBR L11-L16
Statut : CONFIRME

Fait : le bit 7 de $0143 declenche l'ecriture de cet octet dans KEY0 ($FF4C) par le boot ROM CGB ; sinon $04 est ecrit dans KEY0, $01 dans OPRI et les palettes de compatibilite sont chargees.
Source : HDR L59 ; PWUP L89-L91
Statut : CONFIRME

Fait : un programme detecte la CGB par A = $11 juste apres le demarrage ; le bit 0 de B distingue CGB (0) et GBA (1).
Source : CGBR L18-L25
Statut : CONFIRME

Fait : KEY0 $FF4C (non officiel) n'est ecrit que par le boot ROM CGB, puis verrouille apres la fin du boot ; bit 2 = mode compatibilite DMG (0 = CGB complet, 1 = jeu DMG seulement).
Source : CGBR L224-L248
Statut : CONFIRME

Fait : la valeur lue dans KEY0 et le role du bit 3 (mode PGB) ne sont pas documentes.
Source : CGBR L224-L248 ; PWUP L317
Statut : UNKNOWN - to confirm

Fait : KEY1 $FF4D : bit 7 = vitesse courante (lecture seule, 0 normale, 1 double), bit 0 = bascule armee (lecture/ecriture) ; la bascule s'execute par l'instruction STOP bit 0 a 1, puis le bit 0 retombe a 0.
Source : CGBR L147-L170
Statut : CONFIRME

Fait : la CGB demarre en vitesse normale ; en double vitesse CPU, timer, serie et DMA OAM vont 2x plus vite ; LCD, HDMA et son restent a vitesse normale ; le CPU s'arrete 2050 M-cycles apres STOP.
Source : CGBR L172-L196
Statut : CONFIRME

Fait : en cas de STOP de bascule, la possibilite d'interruptions pendant la pause n'est pas documentee dans pandocs.
Source : CGBR L197
Statut : UNKNOWN - to confirm

Fait : VBK $FF4F : seul le bit 0 compte (banque VRAM 0 ou 1) ; la lecture renvoie le numero de banque en bit 0 et tous les autres bits a 1.
Source : CGBR L132-L145
Statut : CONFIRME

Fait : la banque VRAM 1 n'existe qu'en CGB (mode CGB seulement) ; chaque banque fait 8 KiB en $8000-$9FFF ; la banque 1 contient aussi des tuiles en $8000-$97FF et les attributs de tile map en $9800-$9FFF.
Source : MMAP L9 ; MMAP L47-L50 ; CGBR L137-L142
Statut : CONFIRME

Fait : HDMA1-HDMA2 $FF51-$FF52 (source) et HDMA3-HDMA4 $FF53-$FF54 (destination) sont en ecriture seule ; les 4 bits bas sont ignores ; source $0000-$7FF0 ou $A000-$DFF0 ; destination dans $8000-$9FF0 (bits 12-4 seulement).
Source : CGBR L33-L47
Statut : CONFIRME

Fait : HDMA5 $FF55 : bits 0-6 = longueur (divisee par $10, moins 1), bit 7 = mode (0 = general purpose, 1 = HBlank, $10 octets par HBlank) ; lecture : $FF = termine ; ecrire 0 au bit 7 arrete un transfert HBlank.
Source : CGBR L49-L94
Statut : CONFIRME

Fait : le comportement de HDMA quand l'adresse de destination deborde (le transfert s'arrete, etat des registres inconnu) n'est pas documente.
Source : CGBR L101-L103
Statut : UNKNOWN - to confirm

Fait : RP $FF56 (infrarouge) : bits 7-6 = lecture activee (3 = active), bit 1 = reception (lecture seule, 1 = normal), bit 0 = emission LED.
Source : CGBR L200-L222
Statut : CONFIRME

Fait : OPRI $FF6C bit 0 : 0 = priorite des objets style CGB (position OAM), 1 = style DMG (coordonnee X) ; mis a 1 par le boot ROM pour un jeu DMG seulement.
Source : CGBR L250-L269 ; PWUP L90-L91
Statut : CONFIRME

Fait : SVBK $FF70 bits 2-0 : la CGB a 32 KiB de WRAM en 8 banques de 4 KiB ; la banque 0 est fixe en $C000-$CFFF, les banques 1-7 se mappent en $D000-$DFFF ; ecrire 0 selectionne la banque 1.
Source : CGBR L271-L288 ; MMAP L11-L12
Statut : CONFIRME

Fait : les registres de palette couleur BCPS $FF68, BCPD $FF69, OCPS $FF6A, OCPD $FF6B sont en lecture/ecriture, CGB seulement ; le detail des bits n'est pas dans ce chapitre.
Source : HWRL L59-L62
Statut : UNKNOWN - to confirm

Fait : registres non documentes : $FF72 et $FF73 lecture/ecriture (valeur initiale $00) ; $FF74 lecture/ecriture en mode CGB (initial $00), sinon lecture seule verrouillee a $FF ; $FF75 seuls les bits 4-6 sont lecture/ecriture.
Source : CGBR L290-L306
Statut : CONFIRME

Fait : valeurs lues apres boot CGB (bits inutilises a 1) : KEY1 $7E, RP $3E, SVBK $F8.
Source : PWUP L318 ; PWUP L326 ; PWUP L331
Statut : CONFIRME

Fait : Echo RAM $E000-$FDFF reflete $C000-$DDFF : lectures et ecritures ont le meme effet que sur $C000-$DDFF.
Source : MMAP L133-L139
Statut : CONFIRME
