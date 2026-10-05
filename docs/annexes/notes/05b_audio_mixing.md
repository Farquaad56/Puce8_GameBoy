# Note 05b - Audio mixing DMG (frame sequencer, NR50/NR51/NR52, DACs, HPF)

Tache D_12. Sources : refs/pandocs/src/{Audio, Audio_details, Audio_Registers, Power_Up_Sequence}.md
Le terme "frame sequencer" n'apparait pas dans refs/pandocs ; le compteur DIV-APU joue ce role (fait 1).

## Frame sequencer = compteur DIV-APU (512 Hz)
Fait : un compteur interne "DIV-APU" s'incremente a chaque front descendant du bit4 de DIV, soit 512 Hz, quel que soit le mode double vitesse CGB ; ecrire dans DIV pendant que ce bit est a 1 force un increment supplementaire. Les evenements APU en sont derives : envelope/sweep tous les 8 ticks (64 Hz), length timer tous les 2 ticks (256 Hz), sweep de frequence CH1 tous les 4 ticks (128 Hz).
Source : refs/pandocs/src/Audio_details.md#DIV-APU (L87-L96)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (compteur a 512 Hz + diviseurs 2/4/8 pour length/sweep/envelope)
Statut : CONFIRME

## Power on/off de l'APU (NR52 bit7)
Fait : NR52 bit7 allume ou eteint toute l'APU. A l'off, tous les registres APU sont effaces et deviennent read-only jusqu'a re-allumage, sauf NR52 ; la Wave RAM FF30-FF3F et le compteur DIV-APU ne sont pas affectes. Valeurs au power-up DMG/MGB : NR50=$77 (volume maitre max des deux cotes), NR51=$F3 (tous canaux routes a gauche, CH1/CH2 egalement a droite), NR52=$F1 (APU allumee).
Source : refs/pandocs/src/Audio_Registers.md#FF26 - NR52: Audio master control ; Power_Up_Sequence.md#Hardware-registers (L302-L304)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (reset APU + handler on/off)
Statut : CONFIRME

## Statut des canaux dans NR52 (bits 3-0)
Fait : les bits 3-0 de NR52 sont read-only et rapportent l'etat du circuit de generation de chaque canal, pas celui du DAC ; ecrire ces bits n'a aucun effet. Un canal est actif s'il a ete declenche (bit7 de NRx4) avec son DAC allume ; il s'eteint quand le length timer (s'il est active dans NRx4) expire, quand le sweep CH1 depasse $7FF, ou quand son DAC s'eteint ; l'envelope qui atteint volume 0 n'eteint pas le canal.
Source : refs/pandocs/src/Audio_Registers.md#FF26 - NR52: Audio master control ; Audio_details.md#Channels (L131-L140)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (bits de statut read-only)
Statut : CONFIRME

## DAC enable par canal
Fait : le DAC d'un canal est allume si et seulement si NRx2 & $F8 != 0 ; CH3 fait exception, son DAC etant controle directement par bit7 de NR30. L'envelope change le volume mais pas la valeur stockee dans NRx2, donc n'eteint jamais un DAC. Le numerique 0-15 est traduit lineairement en analogique -1..+1 avec pente negative (numerique 0 = analogique +1) ; un DAC eteint s'estompe vers l'analogique 0 (= numerique 7.5), la nature de cet estompage variant selon le modele.
Source : refs/pandocs/src/Audio_details.md#DACs (L118-L130)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (enable DAC par canal + conversion numerique/analogique)
Statut : CONFIRME

## Pan NR51 (FF25) et volume maitre NR50 (FF24)
Fait : NR51 : bits 7-4 = enable gauche CH4..CH1, bits 3-0 = enable droit ; un canal n'est route vers une sortie que si son bit est a 1. NR50 : bits 6-4 / 2-0 = volume maitre gauche/droite (valeur 0 traitee comme volume 1, valeur 7 comme volume 8 ; l'amplificateur ne mute jamais un signal non silencieux) ; bits 7 et 3 = enable VIN gauche/droite.
Source : refs/pandocs/src/Audio_Registers.md#FF25 - NR51: Sound panning (L54-L65) ; #FF24 - NR50: Master volume & VIN panning (L66-L77)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (mixer gauche/droite + volumes maitres)
Statut : CONFIRME

## Mixage des 4 canaux (+VIN)
Fait : chaque canal produit une valeur analogique dans -1..+1 ; le mixer les additionne selectivement selon NR51 en deux sorties gauche/droite, d'ou une plage de -4..+4 par sortie (VIN agit comme un 5e canal controle par NR50 au lieu de NR51). Chaque sortie est ensuite scalee en volume par NR50 puis par le bouton du console.
Source : refs/pandocs/src/Audio_details.md (L48-L67) ; Audio.md#Architecture
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (somme des 4 canaux + VIN, plage -4..+4, scale NR50)
Statut : CONFIRME

## Filtre passe-haut du mixer
Fait : chaque sortie passe par un filtre passe-haut qui tire continuellement le signal vers l'analogique 0 ; il supprime les offsets DC crees par des canaux inactifs dont le DAC reste allume et par les formes d'onde decalees. Quand les 4 DACs sont tous eteints, les volumes maitres sont decouples et la sortie vaut 0 ; quand au moins un DAC est allume, le condensateur du filtre est branche (DMG : facteur de charge 0.999958 par echantillon a 4194304 Hz, soit 0.996 a 44100 Hz ; MGB/CGB : 0.998943). Le filtre est plus agressif sur GBA que sur CGB, et plus sur CGB que sur DMG. Allumer/eteindre un DAC, changer NR51 ou le volume NR50 provoque un "pop" audio (changement d'offset DC lisse par le HPF mais audible).
Source : refs/pandocs/src/Audio_details.md#Mixer (L101-L117) ; #Obscure Behavior (L254-L273)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (HPF par sortie + etat "tous DACs eteints")
Statut : CONFIRME

## Echantillonnage natif de l'APU
Fait : l'APU est horlogee par le master clock (4194304 Hz) et reste synchro avec CPU/PPU ; ses ticks ne sont pas affectes par la double vitesse CGB. Le diviseur de periode des pulse (CH1/CH2) est horloge a 1048576 Hz (un tick par 4 dots), forme d'onde de 8 echantillons, sample rate = 1048576/(2048-v) Hz ; le diviseur wave (CH3) est horloge a 2097152 Hz (un tick par 2 dots), forme d'onde de 32 echantillons, sample rate = 2097152/(2048-v) Hz.
Source : refs/pandocs/src/Audio.md#APU (L46-L70) ; Audio_Registers.md#FF13 - NR13: Channel 1 period low (L180-L181) ; #FF1D - NR33: Channel 3 period low (L282-L283)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (diviseurs de periode par canal + cadence d'echantillonnage)
Statut : CONFIRME

## Laisse hors note
Laisse hors note : details du trigger et des timers par canal (voir 05a), registres PCM CGB FF76-FF77, Obscure Behavior (zombie mode NRx2, length clocking a l'ecriture de NRx4, premier duty step joue comme 0, corruption Wave RAM au re-trigger DMG). Voir refs/pandocs/src/Audio_details.md#Obscure Behavior + #PCM registers.
