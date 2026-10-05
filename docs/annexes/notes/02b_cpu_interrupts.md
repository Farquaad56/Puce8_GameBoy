# Note 02b - CPU SM83 interrupts (IME/IE/IF), HALT and its bug, STOP
Sources (grep + sed) : refs/pandocs/src/{Interrupts.md, Interrupt_Sources.md, halt.md,
CPU_Instruction_Set.md, Reducing_Power_Consumption.md}, refs/pandocs/historical/
{1995-Jan-28-GAMEBOY.txt, 2001-Oct-pandocs.txt, 2008-Mar-7-gbspec.txt}.
Tables historiques en T-cycles : 4 T = 1 M-cycle (voir note 01).

## IME : flag maitre interne, non lisible
Fait : IME est un flag interne au CPU qui decide si un handler peut etre appele, quel que soit IE ; il ne se lit pas. Seuls ei (IME=1), di (IME=0), reti et le service d'un interrupt le modifient (ce dernier remet IME a 0 avant d'appeler le handler). Au boot, les interrupts sont desactives (IME=0).
Source : refs/pandocs/src/Interrupts.md#ime (L3-L13) ; refs/pandocs/historical/2008-Mar-7-gbspec.txt (L526)
Fiabilite : communautaire
Impact code : puce8gb-core - etat ime du cpu : set par ei/reti, clear par di, au reset et au service d'un interrupt.
Statut : CONFIRME

## IE ($FFFF) et IF ($FF0F) : meme layout de 5 bits
Fait : les deux registres partagent l'ordre des bits : 0=vblank, 1=stat (LCDC), 2=debordement timer (overflow), 3=fin transfert serial, 4=joypad (P10-P13 high-to-low). Un bit IF set est une simple demande ; le handler n'est execute que si IME et ce bit IE sont tous deux a 1.
Source : refs/pandocs/historical/1995-Jan-28-GAMEBOY.txt (L341-L360) ; refs/pandocs/src/Interrupts.md#ffff + #ff0f (L19-L48)
Fiabilite : officielle
Impact code : puce8gb-core - map memoire FF0F/FFFF et masque par bit de [IE]&[IF] dans la decision d'interrupt.
Statut : CONFIRME

## Vecteurs $40/$48/$50/$58/$60, priorite bit 0 la plus haute
Fait : les vecteurs sont vblank $40, stat $48, timer $50, serial $58, joypad $60. Si plusieurs bits IF sont set en meme temps, seul l'interrupt de plus haute priorite est acknowledge : bit 0 (vblank) d'abord, bit 4 (joypad) dernier.
Source : refs/pandocs/historical/1995-Jan-28-GAMEBOY.txt (L372-L386) ; refs/pandocs/src/Interrupts.md#interrupt-priorities (L74-L86) ; historical/2008-Mar-7-gbspec.txt (L172-L180)
Fiabilite : officielle
Impact code : puce8gb-core - table des vecteurs d'interrupt et scan de priorite du bit 0 au bit 4.
Statut : CONFIRME

## Instant de declenchement de chaque source
Fait : vblank est demande a l'entree en mode 1 (LY=144, ~59,7 Hz sur DMG). STAT : les 4 sources sont OR'ees sur une ligne unique ; seul un front montant la re-demande ("STAT blocking" si deja haute). Timer : overflow de TIMA. Serial : apres 8 cycles d'horloge depuis le set du bit 7 de SC (donnees dans SB). Joypad : transition high-to-low des P10-P13, avec rebond mecanique.
Source : refs/pandocs/src/Interrupt_Sources.md#int-40 ... #int-60 (L5-L68)
Fiabilite : communautaire
Impact code : puce8gb-core - mode 1 a LY=144, front montant de la ligne STAT OR'ee, overflow timer, compteur d'horloge serial, detection d'arret joypad.
Statut : CONFIRME

## Echantillonnage et effacement des bits IF
Fait : le CPU evalue en continu [IE]&[IF] (condition de reveil) ; un bit est set par le front montant de sa ligne et reste en attente tant que IME/IE bloquent le service. Au service, le CPU efface le bit IF correspondant ("acknowledge") et remets IME a 0 : les autres demandes restent posees mais bloquees jusqu'a rei (ou ei, pour nicher des interrupts).
Source : refs/pandocs/src/halt.md#halt (L3-L6) ; refs/pandocs/src/Interrupts.md#ff0f + #interrupt-handling + #nested-interrupt-handling (L43-L62, L88-L95)
Fiabilite : communautaire
Impact code : puce8gb-core - echantillonnage [IE]&[IF] a chaque tick, effacement du bit IF et de IME a l'entree d'interrupt, pas de reactivation automatique.
Statut : CONFIRME

## Entree d'interrupt : 5 M-cycles au total (call normal)
Fait : le CPU appelle le vecteur comme un call ordinaire : 2 M-cycles d'attente, push du PC en 2 M-cycles, chargement de l'adresse du handler ($40/$48/$50/$58/$60) en 1 M-cycle ; la sequence complete coute 5 M-cycles.
Source : refs/pandocs/src/Interrupts.md#interrupt-handling (L61-L72)
Fiabilite : communautaire
Impact code : puce8gb-core - cout de la sequence d'entree d'interrupt ; testable avec roms/test-roms/blargg/interrupt_time/interrupt_time.gb.
Statut : CONFIRME

## EI : delai d'une instruction
Fait : l'effet de ei (IME=1) est retarde d'une instruction : un di immediat n'autorise aucun interrupt entre les deux, et si le halt suit immediatement un ei, il s'execute avec IME encore a 0 (voir bug HALT ci-dessous). La frontiere exacte en cycles (pendant ou apres la prochaine instruction) n'est pas precisee dans refs/pandocs (voir open_questions).
Source : refs/pandocs/src/Interrupts.md#ime (L15-L17) ; refs/pandocs/src/halt.md#halt-bug (L29) ; historical/2001-Oct-pandocs.txt (ei = 4 T, L2235)
Fiabilite : communautaire
Impact code : puce8gb-core - ime=1 visible a partir de l'instruction suivante ; testable avec roms/test-roms/same-suite/interrupt/ei_delay_halt.gb.
Statut : CONFIRME

## DI : efface IME, aucun interrupt entre ei et di consecutifs
Fait : di remets IME a 0 (4 T = 1 M dans la table historique). Aucun interrupt ne peut etre pris entre un ei et un di consecutifs ; le point exact (quel T-cycle) ou IME=0 devient visible par rapport a l'instruction suivante n'est pas precise plus avant dans refs/pandocs (voir open_questions).
Source : refs/pandocs/src/Interrupts.md#ime (L8-L9, L15-L16) ; historical/2001-Oct-pandocs.txt (di = 4 T, L2234)
Fiabilite : communautaire
Impact code : puce8gb-core - ime=0 par di ; interrupts bloquees jusqu'au prochain ei ou reti.
Statut : CONFIRME

## RETI : ret + IME=1, cout 4 M-cycles
Fait : reti pose PC depuis la pile comme un ret et remets IME a 1 (equivalent d'un ei immediatement suivi d'un ret) ; la table historique donne 16 T = 4 M. C'est le re-activation standard en fin de handler ; un ei en plein handler autorise aussi le nichage.
Source : refs/pandocs/src/Interrupts.md#ime + #nested-interrupt-handling (L8-L13, L92-L95) ; historical/2001-Oct-pandocs.txt (reti = 16 T, L2247)
Fiabilite : communautaire
Impact code : puce8gb-core - decode de reti : pc=(sp), sp+=2, ime=1 sur 4 M-cycles.
Statut : CONFIRME

## HALT ($76) : CPU arrete tant que [IE]&[IF] est nul
Fait : halt arrete le CPU en bas conso (N*4 T dans la table historique) et se reveille des que [IE]&[IF] != 0, quel que soit IME. Si IME=1 : le handler est appele normalement avant l'instruction suivant le halt, puis l'execution repart a cette instruction. Si IME=0 sans demande au debut du halt : le CPU reste arrete ; il se reveille sans servir un interrupt quand une demande apparait plus tard.
Source : refs/pandocs/src/halt.md#halt (L1-L20) ; Reducing_Power_Consumption.md#using-the-halt-instruction (L7-L58) ; historical/2001-Oct-pandocs.txt (L2232)
Fiabilite : communautaire
Impact code : puce8gb-core - etat halt dans le tick cpu, condition de reveil [IE]&[IF] != 0.
Statut : CONFIRME

## Bug HALT : IME=0 + demande pendante au debut du halt, pc non incremente
Fait : si halt s'execute avec IME=0 et [IE]&[IF] != 0, il se termine immediatement mais le pc n'est pas normalement incremente : l'octet apres le halt est re-lu. Variante 1 : ei puis halt ; le halt voit encore IME a 0 (delai de ei), l'interrupt est servi ensuite et un RETI revient sur le halt, qui s'execute encore une fois en attendant un autre interrupt. Variante 2 : halt suivi d'un rst ; l'adresse de retour du rst pointe sur le rst lui-meme, donc ret le re-ecute. Si ei precede et rst suit, "the former wins". La spec officielle 2008 confirme sur GB/GBP/SGB (absent du GBC) : avec DI, l'instruction suivant HALT est skippee.
Source : refs/pandocs/src/halt.md#halt-bug (L24-L35) ; historical/2008-Mar-7-gbspec.txt (L526-L541)
Fiabilite : officielle
Impact code : puce8gb-core - halt avec IME=0 et [IE]&[IF] != 0 : pas d'increment de pc, octet suivant re-lu ; testable avec roms/test-roms/blargg/halt_bug.gb et mooneye acceptance halt_ime0_ei.gb / halt_ime1_timing.gb.
Statut : CONFIRME

## STOP ($FB + 2e byte) : veille a tres basse conso
Fait : stop est normalement une instruction sur 2 bytes (le second, souvent $00, n'est pas toujours ignore). Le systeme entre en standby de tres faible conso jusqu'a ce que P10-P13 passe bas ; il faut donc activer les boutons/d-pad via P1 ($FF00) avant d'y entrer. Sur DMG avec LCD desactive : ligne noire a l'ecran ; sur CGB avec LCD active : ecran noir sauf en mode 3. Sur CGB, un stop apres avoir set KEY1 declenche le switch de vitesse du CPU. Le comportement exact (STOP vs HALT vs NOP, execution du second byte, glitch) depend d'une matrice de conditions documentee par un diagramme (L. Halphon), image seule dans refs/pandocs ; la table historique ne donne aucun cout ("?").
Source : Reducing_Power_Consumption.md#using-the-stop-instruction + #the-bizarre-case-of-the-game-boy-stop-instruction-before-even-considering-timing (L59-L84) ; CPU_Instruction_Set.md#cpu-instruction-set (L89-L92) ; historical/2001-Oct-pandocs.txt (L2233)
Fiabilite : communautaire
Impact code : puce8gb-core - etat stop + traitement du second byte dans le tick cpu ; cout d'entree UNKNOWN.
Statut : CONFIRME

## $93 et $FF : aucune variante halt documentee dans pandocs
Fait : refs/pandocs ne documente ni $93 ni $FF comme variantes d'opcode illegales de type halt : l'instruction-set liste uniquement $D3, $DB, $DD, $E3, $E4, $EB, $EC, $ED, $F4, $FC et $FD comme opcodes invalides qui verrouillent le CPU jusqu'a power-off ; aucun grep pour '$93'/'0x93' ne retourne rien dans tout refs/pandocs.
Source : refs/pandocs/src/CPU_Instruction_Set.md#cpu-instruction-set (L173) ; grep -rn '$93\|0x93' sur refs/pandocs, aucun resultat (2026-10-05)
Fiabilite : communautaire
Impact code : puce8gb-core - table de decode ; comportement DMG de $93/$FF a confirmer.
Statut : UNKNOWN - to confirm

Sujets non couverts ici (voir open_questions.md) : frontiere exacte en cycles du delai ei et du set IME=0 par di, cout d'entree de STOP ("?"), matrice complete des comportements de STOP (diagramme Halphon), comportement DMG de $93/$FF.
