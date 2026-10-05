# Note 01b - Timer et registres diviseur (DMG)

Registres : FF04 DIV, FF05 TIMA, FF06 TMA, FF07 TAC. DMG uniquement ; double speed SGB/CGB ignoree (comme note 01). Unite de base = un dot = un T-cycle ; M-cycle = 4 dots.
Le timer ci-dessous est le timer interne du Game Boy, pas le RTC a batterie des MBC3 (objet distinct, voir la section MBCs).

## FF04 DIV - horloge du diviseur = 16384 Hz
Fait : DIV s'increment a 16384 Hz sur DMG, soit une fois toutes les 256 M-cycles. Toute ecriture dans DIV le remet a $00. Le diviseur compte en permanence, independamment du bit enable de TAC ; il n'est arrete que par le mode STOP.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF04-DIV ("incremented at a rate of 16384Hz", "Writing any value to this register resets it to $00") ; memes fichier #FF07-TAC ("DIV is always counting, regardless of this bit")
Fiabilite : officielle
Impact code : puce8gb-core - horloge du registre FF04 dans le module timer ; reset a 0 sur toute ecriture CPU
Statut : CONFIRME

## Mode STOP arrete puis redemarre DIV (system counter)
Fait : Executer l'instruction stop remet DIV a $00 et il ne recommence a tiquer qu'a la fin du mode STOP. Le system counter interne (dont DIV n'est que la partie visible, ses 8 bits bas) s'increment tous les M-cycles sauf en mode STOP.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF04-DIV ("reset when executing the stop instruction... only begins ticking again once stop mode ends") ; refs/pandocs/src/Timer_Obscure_Behaviour.md, tip "System counter" ("constantly incrementing every M-cycle, unless the CPU is in STOP mode")
Fiabilite : officielle
Impact code : puce8gb-core - gate du system counter et de DIV par l'etat STOP (tache STOP future) ; reset du compteur a 0 sur stop
Statut : CONFIRME

## FF05 TIMA - increment selon TAC, reload + interrupt a chaque overflow
Fait : TIMA s'increment au rythme choisi dans TAC. Quand il depasse $FF (overflow), il est reinitialise a la valeur courante de TMA (FF06) et un interrupt timer (bit 2 du registre IF FF0F, vecteur $50) est demande. Ecrire $FF dans TIMA ne provoque ni overflow ni interrupt : seul l'increment du hardware les declenche.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF05-TIMA ; memes fichier #FF06-TMA ("When TIMA overflows, it is reset to the value in this register and an interrupt is requested") ; refs/pandocs/src/Interrupt_Sources.md#INT-50-Timer-interrupt ("requested every time that the timer overflows (that is, when TIMA exceeds $FF)"); refs/pandocs/src/Timer_Obscure_Behaviour.md#Timer-overflow-behavior ("only happens when TIMA overflows from incrementing")
Fiabilite : officielle
Impact code : puce8gb-core - increment de FF05 selon TAC[1:0] et reload par la valeur courante de FF06 + set du bit 2 IF a l'overflow
Statut : CONFIRME

## FF07 TAC bits 1-0 - periodes d'increment de TIMA (DMG)
Fait : Les clock select font avancer TIMA respectivement toutes les : 00 = 256 M-cycles (4096 Hz), 01 = 4 M-cycles (262144 Hz), 10 = 16 M-cycles (65536 Hz), 11 = 64 M-cycles (16384 Hz) sur DMG a vitesse normale. Les deux sources Pan Docs actuelles donnent exactement les memes quatre paires ; l'historique 2001 confirme aussi.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF07-TAC (tableau "Increment every" / frequence DMG) ; refs/pandocs/historical/2001-Oct-pandocs.txt section Timer and Divider Registers, lignes 1076-1079
Fiabilite : officielle
Impact code : puce8gb-core - table des periodes du timer pour les 4 valeurs de TAC[1:0] (256/4/16/64 M-cycles)
Statut : CONFIRME

## FF07 TAC bit 2 - enable du timer seulement, pas de DIV
Fait : Le bit 2 de TAC ("Enable" dans la source actuelle, nomme "Timer Stop, 0=Stop 1=Start" dans l'historique 2001) controle uniquement si TIMA est incremente. Il n'a aucun effet sur DIV : le diviseur continue de tourner que le timer soit active ou non.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF07-TAC ("Enable: Controls whether TIMA is incremented") ; refs/pandocs/historical/2001-Oct-pandocs.txt ligne 1074 (Bit 2 "Timer Stop")
Fiabilite : officielle
Impact code : puce8gb-core - gate de l'increment TIMA par TAC bit 2 uniquement, DIV jamais gate
Statut : CONFIRME

## FF06 TMA - rechargement a l'overflow et division effective de l'horloge
Fait : A chaque overflow, la valeur courante de TMA (pas une valeur figee) est copie dans TIMA et un interrupt timer est demande. Cela donne les usages cites : TMA=$FF donne un interrupt a chaque increment ; TMA=$FE n'en demande qu'un toutes les 2 increments (horloge divisee par 2) ; TMA=$FD la divise par 3, etc.
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF06-TMA ("Example of use: if TMA is set to $FF... only requested every two increments")
Fiabilite : officielle
Impact code : puce8gb-core - le rechargement de FF05 doit lire la valeur courante de FF06 a l'overflow ; periode effective = (TMA+1) * periode choisie dans TAC
Statut : CONFIRME

## CONFLIT : ecriture TMA pendant le cycle du reload (ancienne vs nouvelle valeur)
Fait : Si une ecriture CPU dans TMA tombe sur le meme M-cycle que le transfert automatique de TMA vers TIMA (d'un overflow), les deux sections Pan Docs actuelles ne sont pas d'accord. Timer_and_Divider_Registers.md dit explicitement "the old value is transferred to TIMA" ; Timer_Obscure_Behaviour.md, point 3, dit l'inverse : la nouvelle valeur de TMA est copie dans TIMA sur le meme M-cycle (emulable comme si TMA etait copie a la fin du cycle). La test ROM tma_write_reloading.gb (roms/test-roms/mooneye-test-suite/acceptance/timer/) tranche laquelle des deux est exacte (voir open_questions.md).
Source : refs/pandocs/src/Timer_and_Divider_Registers.md#FF06-TMA ("If a TMA write is executed on the same M-cycle... the old value is transferred to TIMA") vs refs/pandocs/src/Timer_Obscure_Behaviour.md#Timer-overflow-behavior point 3 (la nouvelle valeur est copie dans TIMA sur le meme M-cycle)
Fiabilite : testee sur ROM (a trancher par la ROM)
Impact code : puce8gb-core - ordre de l'ecriture CPU de TMA par rapport au reload de TIMA dans le module timer ; doit dependre du resultat de tma_write_reloading.gb
Statut : CONFLIT

## Falling edge du circuit interne : une ecriture DIV/TAC peut envoyer un tick
Fait : Le circuit interne n'est pas un simple compteur : c'est un multiplexeur selectionne par TAC[1:0], suivi d'un detecteur de descente (falling edge) sur le bit selectionne. Consequences sur DMG : ecrire dans DIV reinitialise le system counter et, si cette ecriture fait passer le bit selectionne de 1 a 0, un "Timer tick" est envoye immediatement ; changer TAC[1:0] d'un bit a 1 vers un bit a 0 envoie egalement un "Timer tick". Exemple cite par Pan Docs : avec system counter = $3FF0 et TAC=$FC, ecrire $05 ou $06 dans TAC envoie immediatement un tick ; ecrire $04 ou $07 ne fait rien. Enfin, sur DMG uniquement (pas on les consoles CGB), desactiver le timer alors que le bit selectionne est a 1 produit un "Timer tick" unique.
Source : refs/pandocs/src/Timer_Obscure_Behaviour.md#Relation-between-Timer-and-Divider-register (points 1-3 et exemple $3FF0/$FC) ; memes fichier, tip "System counter" + schema timer_tac_bug_dmg.svg
Fiabilite : testee sur ROM
Impact code : puce8gb-core - le module timer doit modeler un tick de falling edge du circuit interne (pas a simple compteur) ; genere par une ecriture DIV ou TAC qui fait descendre le bit selectionne, ainsi que par la desactivation du timer sur DMG alors que le bit selectionne vaut 1.
Statut : CONFIRME

## Retard d'un M-cycle : reload TMA->TIMA et bit 2 IF poses un M-cycle APRES l'increment qui cause le overflow
Fait : Quand TIMA overflows par increment, le reload de TMA vers TIMA ainsi que le set du bit 2 IF arrivent un M-cycle APRES l'increment qui a cause le overflow. Concretement : sur le M-cycle juste apres l'overflow, TIMA = $00 (la valeur rechargee n'est PAS encore visible) et IF bit 2 n'est PAS encore set ; sur le cycle suivant, le bit 2 IF est pose et TIMA affiche sa nouvelle valeur. Ce delai ne s'applique qu'aux overflow par increment du hardware, jamais a une ecriture manuelle de $FF dans TIMA. Exemple Pan Docs (TAC=$FD) : cycle A = overflow ($FF), cycle B = TIMA=00 et IF bit 2 set ; la valeur courante de TMA n'apparait dans TIMA qu'a partir du cycle suivant B (ou immediatement si TMA elle-meme est ecrise sur ce meme M-cycle).
Source : refs/pandocs/src/Timer_Obscure_Behaviour.md#Timer-overflow-behavior ("the timer flag is set in IF, but one M-cycle later. This means that TIMA is equal to $00 for the M-cycle after it overflows" ; exemple tableau cycle A/B)
Fiabilite : testee sur ROM
Impact code : puce8gb-core - sequencer du reload TMA->TIMA et du flag IF bit 2 a +1 M-cycle apres l'overflow par increment, pas immediat
Statut : CONFIRME

## Ecrire dans TIMA sur le M-cycle A annule le overflow : pas de flag, pas de reload (DIV/TAC n'annulent rien)
Fait : Ecrire dans TIMA sur le M-cycle A exact ou se produit l'overflow agit comme si le overflow ne s'etait pas produit : TMA n'est PAS copie dans TIMA, et IF bit 2 n'est PAS set. En revanche, ecrire dans DIV, TAC ou un autre registre n'empeche ni le set du flag IF ni le reload de TIMA. De plus, ecrire dans TIMA sur cycle B est ignore entierement (TIMA finit egal a TMA de toute facon), tandis qu'ecrire dans TMA sur cycle B fait copier la nouvelle valeur dans TIMA sur ce meme M-cycle.
Source : refs/pandocs/src/Timer_Obscure_Behaviour.md#Timer-overflow-behavior points 1-3 ("Writing to TIMA during cycle A acts as if the overflow didn't happen!... Writing to DIV, TAC, or other registers won't prevent the IF flag from being set")
Fiabilite : testee sur ROM
Impact code : puce8gb-core - cas special : l'ecriture CPU de TIMA qui tombe sur le meme M-cycle que le overflow par increment annule le set du flag ainsi que le reload, contrairement aux ecritures dans DIV/TAC, qui n'ont aucun tel effet.
Statut : CONFIRME

## Interrupt timer (vecteur $50, condition et timing)
Fait : Le interrupt timer est demande chaque fois que TIMA overflows, c'est-a-dire quand il depasse $FF ; il correspond au bit 2 du registre IF et porte vers le vecteur $50. La request est delayee d'exactly un M-cycle after l'increment qui cause le overflow (voir fait "Retard d'un M-cycle" ci-dessus), et n'est pas simultane avec the overflow lui-meme.
Source : refs/pandocs/src/Interrupt_Sources.md#INT-50-Timer-interrupt ; refs/pandocs/src/Timer_Obscure_Behaviour.md#Timer-overflow-behavior (delay de 1 M-cycle)
Fiabilite : officielle
Impact code : puce8gb-core - condition et timing exact du flag IF bit 2 / vecteur $50 dans le module timer, delaye d'un M-cycle par rapport a l'increment qui cause le overflow
Statut : CONFIRME

## Largeur du system counter interne : UNKNOWN
Fait : Pan Docs dit que DIV n'est "just the visible part" of an internal "system counter" qui s'increment tous les M-cycles, mais ne donne pas la largeur totale de ce compteur. Seule l'existence de ses 8 bits bas correspondant a DIV (FF04) est explicite ; rien dans refs/pandocs/src ni historique ne dit si ce system counter complet fait bien 16-bit ou plus. Statut : UNKNOWN - to confirm, pointe dans open_questions.md.
Source : refs/pandocs/src/Timer_Obscure_Behaviour.md, tip "System counter" ("DIV is just the visible part of the system counter") ; memes fichier example tableau ("SYS represents the lower 8 bits of the system counter")
Fiabilite : officielle ; largeur exacte du compteur : UNKNOWN - to confirm
Impact code : puce8gb-core - taille du compteur interne a modeler dans le module timer en attendant confirmation de sa largeur
Statut : UNKNOWN - to confirm

## Elements non developpes ici (au-dela des 150 lignes max)
- Comportement propre au CGB du timer (mode double speed ; differences entre les consoles SGB1/SGB2/CGB dans le cas d'une ecriture TAC qui active le timer alors qu'un bit est at 1) : hors scope DMG.
- Event DIV-APU (refs/pandocs/src/Audio_details.md : le "DIV-APU counter" s'increment quand le bit 4 de DIV passe de 1 to 0, a 512 Hz quel que soit le mode de vitesse) : appartient au module audio, pas au sujet de cette note.
