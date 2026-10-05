# Note 06 - Joypad (P1) et port serie (SB/SC), DMG

Tache D_13. Sources : refs/pandocs/src/{Joypad_Input, Serial_Data_Transfer_(Link_Cable), Interrupt_Sources, Interrupts, Power_Up_Sequence}.md
Le DMG n'a pas de mode CGB ; les faits marques "CGB only" ne concernent que le port serie (bit 1 de SC).

## P1/JOYP ($FF00) : lignes de selection et polarite
Fait : les 8 boutons forment une matrice 2x4. Bit5 = 0 active la lecture des boutons d'action (A/B/Start/Select), bit4 = 0 active le pad directionnel ; les deux bits a 1 ($30) lit $F dans le bas de nibble (aucun bouton actif). Le bas de nibble est read-only et la polarite est inversee : un bouton presse lit 0, relache lit 1.
Source : refs/pandocs/src/Joypad_Input.md#FF00 - P1/JOYP: Joypad (L5-L18)
Fiabilite : communautaire
Impact code : puce8gb-core, module joypad (registre FF00 + etat des 8 boutons)
Statut : CONFIRME

## Valeurs power-up de P1/SB/SC sur DMG
Fait : a la main-off du boot ROM (PC = $0100), DMG/MGB lisent P1=$CF, SB=$00, SC=$7E ; SGB/CGB lisent P1=$C7 ou $CF et SC=$7F. Le bit6 de SC est donc 0 sur DMG a ce moment (bit1 = 0, bit0 = 1).
Source : refs/pandocs/src/Power_Up_Sequence.md#Hardware-registers (L276-L278) ; confirme par les tests mooneye boot_regs/boot_hwio cites au meme endroit (L268)
Fiabilite : testee sur ROM
Impact code : puce8gb-core, module machine (valeurs de reset des registres FF00-FF02)
Statut : CONFIRME

## Interrupt joypad (IF/IE bit4, INT $60)
Fait : l'interrupt joypad est demande quand un des bits 0-3 de P1 passe de haut a bas (bouton presse), a condition que la famille correspondante soit activee par bit5/bit4 ; le rebond du contact produit souvent plusieurs transitions. L'interrupt n'est utile qu'a une seule famille selectionnee, ou pour sortir du STOP ; il est le plus prioritaire des 5 sources et se masque via IE/IF bit4.
Source : refs/pandocs/src/Interrupt_Sources.md#INT $60 - Joypad interrupt (L62-L79) ; Interrupts.md#FFFF - IE / #FF0F - IF (L18-L43) ; L86
Fiabilite : communautaire
Impact code : puce8gb-core, module joypad + bus (detection de transition haut->bas sur bits 0-3, pose du bit4 de IF)
Statut : CONFIRME

## SB ($FF01) : donnees du port serie
Fait : avant un transfert, SB contient l'octet a envoyer ; pendant le transfert il est melange des octets sortant et entrant (le bit gauche part sur le fil, le bit recu entre par la droite, 8 shifts au total). Le DMG n'a pas de port physique attache : les test ROMs utilisent SB comme un canal de sortie vers l'host.
Source : refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md#FF01 - SB: Serial transfer data (L12-L30) ; roms/test-roms/mooneye-test-suite/README.markdown#Pass/fail reporting (L57-L84)
Fiabilite : communautaire + testee sur ROM
Impact code : puce8gb-core, module serial (registre FF01 + file d'octets sortants pour la capture)
Statut : CONFIRME

## SC ($FF02) : controle du port serie
Fait : bit7 = enable de transfert (1 = demande ou en cours), bit0 = selection d'horloge (0 = externe/esclave, 1 = interne/maitre). Le maitre ecrit SB puis SC=$81 ; l'esclave active le port avec SC=$80. A la fin du transfert, bit7 est efface sur les deux cotes et un interrupt serie est demande ; pour tester la fin il faut ne lire que bit7 de SC.
Source : refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md#FF02 - SC: Serial transfer control (L32-L51)
Fiabilite : communautaire
Impact code : puce8gb-core, module serial (registre FF02 + etat de transfert)
Statut : CONFIRME

## Horloge interne du DMG (8192 Hz)
Fait : en mode non-CGB le DMG ne fournit qu'une horloge serie interne de 8192 Hz (~1 Ko/s). Le bit1 de SC (horloge rapide, ~256 kHz) est CGB only et n'existe pas sur DMG.
Source : refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md#Internal Clock (L52-L64) ; #FF02 - SC: Serial transfer control (L39)
Fiabilite : communautaire
Impact code : puce8gb-core, module serial (1 bit par 4194304/8192 = 512 dots ; un octet = 4096 dots)
Statut : CONFIRME

## Duree d'un transfert et interrupt serie (INT $58)
Fait : le transfert dure exactement 8 cycles d'horloge serie, puis l'octet recu est dans SB et l'interrupt serie (IF/IE bit3) est demande. A l'horloge interne DMG cela fait 4096 dots (~1,2 ms). Les test ROMs mooneye attendent la fin par boucle d'attente sur SC bit7 sans utiliser l'interrupt ; les tests gambatte serial/ verifient le moment exact de la pose du IF bit3 et la lecture de SB/SC.
Source : refs/pandocs/src/Interrupt_Sources.md#INT $58 - Serial interrupt (L57-L60) ; roms/test-roms/mooneye-test-suite/README.markdown#Pass/fail reporting (L74-L79) ; roms/test-roms/gambatte/serial/ (noms des tests)
Fiabilite : communautaire + testee sur ROM
Impact code : puce8gb-core, module serial (compteur de 8 bits puis IF bit3 + file d'octets)
Statut : CONFIRME

## Horloge externe et temps mort
Fait : en mode esclave l'horloge vient du cable ; le DMG accepte des horloges externes jusqu'a ~500 kHz, sans regularite ni limite basse. Sans horloge (cable debranche ou second system off) le transfert ne se termine jamais, d'ou un compteur de timeout dans la procedure. Sur un cable debranche l'entree du maitre lit 1 (octets $FF).
Source : refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md#External Clock (L66-L75) ; #Timeouts (L77-L85) ; #Disconnects (L87-L94)
Fiabilite : communautaire
Impact code : puce8gb-core, module serial (mode esclave optionnel + timeout)
Statut : CONFIRME

## Usage des test ROMs du port serie
Fait : les suites mooneye et gambatte signalent le resultat par le port serie : un test qui passe envoie les nombres de Fibonacci 3/5/8/13/21/34 sur SB, un test qui echoue envoie $42 six fois ; l'attente se fait par boucle d'attente sur SC bit7 (pas d'interrupt). Le mooneye boot_sclk_align-dmgABCmgb.gb verifie l'alignement de l'horloge serie au power-up.
Source : roms/test-roms/mooneye-test-suite/README.markdown#Pass/fail reporting (L57-L84) ; roms/test-roms/mooneye-test-suite/acceptance/serial/boot_sclk_align-dmgABCmgb.gb ; roms/test-roms/gambatte/serial/
Fiabilite : testee sur ROM
Impact code : puce8gb-core, module serial + cli (capture des octets SB pour --expect-serial)
Statut : CONFIRME

## Laisse hors note
Laisse hors note : details du debranchement (montee 20 us, mesure CGB rev E), horloges internes CGB (16384/262144/524288 Hz en double speed), usage SGB du registre joypad pour les paquets de commande SNES et le four-player adapter (Four_Player_Adapter.md). Voir refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md#Disconnects + Joypad_Input.md#Usage in SGB software.
