# Note 11c - CGB : vitesse double, HDMA, infrarouge, registres divers

Sources : refs/pandocs/src/CGB_Registers.md (CGBR), Timer_and_Divider_Registers.md (TIM), OAM_DMA_Transfer.md (ODMA), Serial_Data_Transfer_(Link_Cable).md (SER), Hardware_Reg_List.md (HWRL).

Fait : KEY1 $FF4D : bit 7 = vitesse courante (lecture seule, 0 = normale, 1 = double), bit 0 = bascule armee (lecture/ecriture).
Source : CGBR L147-L154
Statut : CONFIRME

Fait : la bascule s'execute par l'instruction STOP quand le bit 0 de KEY1 vaut 1 ; ensuite le bit 0 est remis a 0 automatiquement et la machine tourne a l'autre vitesse ; procedure conseillee : IE = $00, JOYP = $30, KEY1 = $01, STOP.
Source : CGBR L156-L171
Statut : CONFIRME

Fait : la CGB demarre en vitesse normale.
Source : CGBR L172-L174
Statut : CONFIRME

Fait : en double vitesse, vont 2x plus vite : le CPU (2,10 MHz, 1 M-cycle = environ 0,5 us), le timer et le diviseur, le port serie, le DMA OAM.
Source : CGBR L176-L184
Statut : CONFIRME

Fait : en double vitesse, continuent a vitesse normale : le controleur LCD (PPU), le transfert HDMA vers la VRAM, tous les timings et frequences du son.
Source : CGBR L186-L188
Statut : CONFIRME

Fait : le CPU est arrete 2050 M-cycles (8200 T-cycles) apres le STOP de bascule ; pendant ce temps DIV ne compte pas, donc certains evenements audio ne sont pas traites.
Source : CGBR L189-L194
Statut : CONFIRME

Fait : pendant cette pause l'acces memoire video est fige selon le mode PPU de depart : HBlank ou VBlank = le PPU ne lit aucune memoire video et produit des pixels noirs ; mode 2 = VRAM lisible mais pas l'OAM (fond sans objets) ; mode 3 = rendu normal.
Source : CGBR L194-L197
Statut : CONFIRME

Fait : DIV est remis a 0 par l'execution de STOP et ne recommence a compter qu'a la fin du mode stop, y compris pendant une bascule de vitesse ; ecrire n'importe quelle valeur dans DIV le remet aussi a $00.
Source : TIM L12-L19
Statut : CONFIRME

Fait : DIV s'incremente a 16384 Hz en vitesse normale et a 32768 Hz en double vitesse.
Source : TIM L12-L22
Statut : CONFIRME

Fait : l'effet de l'attente apres une bascule de vitesse sur DIV n'est pas documente (question ouverte dans pandocs).
Source : TIM L19
Statut : UNKNOWN - to confirm

Fait : la possibilite d'interruptions (par exemple joypad) pendant la pause de la bascule de vitesse n'est pas documentee.
Source : CGBR L198
Statut : UNKNOWN - to confirm

Fait : le DMA OAM dure 160 M-cycles : 640 dots (1,4 ligne) en vitesse normale, 320 dots (0,7 ligne) en double vitesse.
Source : ODMA L15-L16
Statut : CONFIRME

Fait : SC $FF02 bit 1 (CGB seulement) active l'horloge serie rapide ; horloges internes en mode CGB : 8192 Hz (bit 1 a 0, normale), 16384 Hz (bit 1 a 0, double), 262144 Hz (bit 1 a 1, normale), 524288 Hz (bit 1 a 1, double) ; hors CGB seulement 8192 Hz.
Source : SER L39 ; SER L54-L64
Statut : CONFIRME

Fait : HDMA1-HDMA2 $FF51-$FF52 (source haut/bas) et HDMA3-HDMA4 $FF53-$FF54 (destination haut/bas) sont en ecriture seule ; les 4 bits bas sont ignores ; source $0000-$7FF0 ou $A000-$DFF0 ; destination $8000-$9FF0 (seuls les bits 12-4 comptent, les 3 bits hauts sont ignores).
Source : CGBR L33-L56
Statut : CONFIRME

Fait : HDMA5 $FF55 : ecrire lance le transfert ; bits 0-6 = longueur (divisee par $10, moins 1) soit $10 a $800 octets pour les valeurs $00-$7F ; bit 7 = mode (0 = general purpose, 1 = HBlank).
Source : CGBR L58-L61
Statut : CONFIRME

Fait : mode general purpose (bit 7 a 0) : tout est copie d'un coup, le programme est suspendu jusqu'a la fin, la copie ne tient pas compte de l'acces PPU a la VRAM, et FF55 vaut ensuite $FF.
Source : CGBR L63-L72
Statut : CONFIRME

Fait : mode HBlank (bit 7 a 1) : $10 octets copies a chaque HBlank pour LY = 0-143, rien pendant le VBlank (LY 144-153), reprise a LY = 0 ; le programme est suspendu pendant chaque bloc et continue entre les blocs.
Source : CGBR L74-L80
Statut : CONFIRME

Fait : pendant un transfert HBlank il ne faut pas changer la banque VRAM destination (FF4F) ni la banque source ; l'instruction HALT suspend le transfert, qui reprend quand le CPU reprend ; un transfert HBlank ne doit pas etre demarre pendant un HBlank (mode STAT 0).
Source : CGBR L80-L87 ; CGBR L98-L99
Statut : CONFIRME

Fait : lecture de FF55 : longueur restante (divisee par $10, moins 1) ; $FF = termine ; ecrire 0 au bit 7 annule un transfert HBlank actif, alors la lecture donne les blocs restants moins 1 dans les 7 bits bas et le bit 7 a 1 ; l'arret ne remet pas HDMA1-4 a $FF ; bit 7 de FF55 : 1 = inactif, 0 = actif.
Source : CGBR L89-L94 ; CGBR L107-L112
Statut : CONFIRME

Fait : temps de transfert : environ 8 us par bloc de $10 octets dans les deux vitesses, soit 8 M-cycles en vitesse normale et 16 M-cycles rapides en double vitesse ; 2 octets par microseconde ; environ 2280 octets possibles pendant un VBlank.
Source : CGBR L114-L125
Statut : CONFIRME

Fait : une source HDMA situee en VRAM copie des donnees invalides ; le comportement avec une source en Echo RAM, OAM, $FExx, I/O ou HRAM n'a pas ete teste.
Source : CGBR L35-L40
Statut : UNKNOWN - to confirm

Fait : RP $FF56 (infrarouge) : bits 7-6 = lecture activee (0 = desactive, le bit 1 se lit alors 1 ; 3 = active), bit 1 = reception (lecture seule, 0 = signal recu, 1 = normal), bit 0 = LED d'emission (0 = eteinte, 1 = allumee) ; remettre le registre a $00 apres usage.
Source : CGBR L200-L222
Statut : CONFIRME

Fait : registres non documentes : $FF72 et $FF73 entierement lecture/ecriture, valeur initiale $00 ; $FF74 lecture/ecriture en mode CGB (initial $00), sinon lecture seule verrouillee a $FF ; $FF75 seuls les bits 4-6 sont lecture/ecriture, valeur initiale 0.
Source : CGBR L290-L306
Statut : CONFIRME

Fait : PCM12 $FF76 et PCM34 $FF77 sont des registres en lecture seule, CGB seulement, qui donnent les sorties numeriques des canaux audio 1-2 et 3-4 ; leur format de bits n'est pas decrit dans les fichiers lus.
Source : HWRL L64-L65
Statut : UNKNOWN - to confirm
