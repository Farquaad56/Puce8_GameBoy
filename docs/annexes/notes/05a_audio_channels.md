# Note 05a - Audio channels DMG (NR10-NR44)

Tache D_11. Sources : refs/pandocs/src/{Audio, Audio_Registers, Audio_details, Hardware_Reg_List, Power_Up_Sequence}.md

## Architecture APU et convention NRxy
Fait : 4 canaux specialises + VIN (entree analogique cartouche) : CH1/CH2 pulse (duty), CH3 wave (echantillons arbitraires), CH4 noise (LFSR). Convention NRxy : x = canal (5 = global), y = id du registre ; NRx0 fonction propre, NRx1 length timer, NRx2 volume/envelope, NRx3 periode basse, NRx4 trigger + length enable + periode haute. L'APU est horlogee par le master clock et n'est pas affectee par la double vitesse CGB.
Source : refs/pandocs/src/Audio.md#Architecture (tableau des canaux) ; Audio_Registers.md intro ; Audio.md#APU
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (4 structs de canal + mixer)
Statut : CONFIRME

## NR52 (FF26) maitre on/off et statut des canaux
Fait : bit7 = APU on/off ; a l'off tous les registres APU sont effaces et deviennent read-only sauf NR52 (Wave RAM FF30-FF3F et le compteur DIV-APU ne sont pas affectes). Bits 3-0 = statut "canal actif", read-only : les ecrire n'active ni desactive un canal. Un canal est actif s'il a ete declenche (bit7 de NRx4) et son DAC allume ; il s'eteint quand le length timer expire, quand le sweep CH1 depasse $7FF, ou quand son DAC s'eteint ; l'envelope qui atteint volume 0 n'eteint PAS le canal.
Source : refs/pandocs/src/Audio_Registers.md#FF26 - NR52: Audio master control ; Audio_details.md#Channels
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (apu on/off + bits de statut)
Statut : CONFIRME

## NR51 (FF25) pan et NR50 (FF24) volume maitre
Fait : NR51 : bits 7..4 = enable gauche CH4..CH1, bits 3..0 = enable droit ; un canal n'est route vers une sortie que si son bit est a 1. NR50 : bits 6-4 / 2-0 = volume maitre gauche/droite (valeur 0 traitee comme volume 1, valeur 7 comme volume 8 ; l'amplificateur ne mute jamais un signal non silencieux) ; bits 7 et 3 = enable VIN gauche/droite.
Source : refs/pandocs/src/Audio_Registers.md#FF25 - NR51: Sound panning ; #FF24 - NR50: Master volume & VIN panning
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (mixer gauche/droite)
Statut : CONFIRME

## CH1 sweep NR10 (FF10)
Fait : bits 6-4 = pace en ticks de 128 Hz (7.8 ms), bit3 = direction (0 addition / 1 soustraction), bits 2-0 = step ; a chaque iteration nouvelle periode = ancienne +/- ancienne/2^step, ecrise dans NR13/NR14. Si le resultat depasse $7FF le canal s'eteint, meme si pace=0 ; une periode de 0 ne peut jamais etre modifiee par le sweep (pas de underflow). Le champ pace n'est pas relu jusqu'a la fin d'une iteration ou un re-trigger.
Source : refs/pandocs/src/Audio_Registers.md#FF10 - NR10: Channel 1 sweep ; Audio_details.md#Pulse channel with sweep (CH1)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio ch1 (timer de sweep + registre ombre)
Statut : CONFIRME

## Duty et length dans NR11/NR21 (FF11/FF16)
Fait : bits 7-6 = duty cycle : 00=12.5%, 01=25%, 10=50%, 11=75% (pas de difference auditive entre 25% et 75%). Bits 5-0 = length timer initial, write-only ; plus la valeur est haute, plus le canal est coupe tot.
Source : refs/pandocs/src/Audio_Registers.md#FF11 - NR11: Channel 1 length timer & duty cycle ; #Sound Channel 2 - Pulse
Fiabilite : communautaire
Impact code : puce8gb-core, module audio ch1/ch2 (LUT de duty + compteur length)
Statut : CONFIRME

## Volume et envelope dans NR12/NR22/NR42 (FF12/FF17/FF21)
Fait : bits 7-4 = volume initial (lisibles mais jamais mis a jour par l'envelope), bit3 = direction (0 decrease / 1 increase), bits 2-0 = pace : l'envelope tick a 64 Hz et change le volume tous les N ticks ; pace=0 desactive l'envelope. Bits 3-7 tous a 0 eteint le DAC (donc le canal). Ecrire NRx2 pendant que le canal est allume exige un re-trigger pour prendre effet.
Source : refs/pandocs/src/Audio_Registers.md#FF12 - NR12: Channel 1 volume & envelope ; Audio.md#Volume & envelope
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (timer d'envelope par canal)
Statut : CONFIRME

## Valeur de periode et frequence (NRx3/NRx4)
Fait : la periode fait 11 bits : NRx3 = 8 bas + NRx4[2:0] = 3 hauts. Le diviseur est un compteur montant qui se recharge depuis les registres quand il depasse $7FF, c'est-a-dire qu'il traite la valeur comme negative en complement a deux sur 11 bits : plus la valeur est haute, plus la frequence est haute. Pulse (CH1/CH2) : diviseur horloge a 1048576 Hz (un tick par M-cycle, soit 4 dots), forme d'onde de 8 echantillons ; sample rate = 1048576/(2048-v) Hz, tone frequency = 131072/(2048-v) Hz. Wave (CH3) : horloge a 2097152 Hz (un tick par 2 dots), forme d'onde de 32 echantillons ; sample rate = 2097152/(2048-v) Hz, tone frequency = 65536/(2048-v) Hz. Un changement de periode ne prend effet qu'a la fin de l'echantillon en cours.
Source : refs/pandocs/src/Audio_Registers.md#FF13 - NR13: Channel 1 period low ; #FF1D - NR33: Channel 3 period low ; Audio.md#Frequency
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (diviseur de periode par canal)
Statut : CONFIRME

## Triggering (bit7 de NRx4 : FF14/FF19/FF1E/FF23)
Fait : ecrire NRx4 avec bit7=1 declenche le canal : l'active (si son DAC est allume), remet a zero le length timer s'il avait expire, fixe le diviseur de periode depuis les registres, remet a zero le timer d'envelope, volume = valeur initiale de NRx2. CH1 copie en plus la periode dans le registre ombre du sweep et fait un calcul + test de depassement immediat ; CH3 remet a zero l'index Wave RAM sans recharger son tampon ; CH4 remet a zero le LFSR. Le bit6 (length enable) prend effet immediatement a l'ecriture.
Source : refs/pandocs/src/Audio_Registers.md#FF14 - NR14: Channel 1 period high & control ; #FF1E - NR34 ; #FF23 - NR44 ; Audio_details.md#Pulse channel with sweep (CH1)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (handler de trigger par canal)
Statut : CONFIRME

## Timing du length timer et DIV-APU
Fait : le compteur length tick a 256 Hz ; CH1/CH2/CH4 s'eteignent quand il atteint 64, CH3 quand il atteint 256. Le compteur DIV-APU s'incremente a 512 Hz (bascule du bit4 de DIV) : envelope tous les 8 ticks (64 Hz), length tous les 2 (256 Hz), sweep CH1 tous les 4 (128 Hz).
Source : refs/pandocs/src/Audio.md#Length timer ; Audio_details.md#DIV-APU
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (compteur div_apu + timers par canal)
Statut : CONFIRME

## CH3 wave channel : DAC, niveau, Wave RAM
Fait : NR30 bit7 = DAC on/off de CH3 (lisible) ; le DAC off eteint le canal. NR32 bits 6-5 = output level : 0=mute, 1=100%, 2=50% (decalage droit d'un), 3=25% (decalage droit de deux) ; ce decalage s'applique a la valeur numerique, pas au signal analogique ; CH3 n'a pas d'envelope. Wave RAM FF30-FF3F = 16 octets, chacun portant 2 echantillons de 4 bits (32 au total), lus de gauche a droite, nibble haut en premier ; mais le premier echantillon joue apres un trigger est l'index 1 (nibble bas de FF30), et le dernier echantillon lu reste emis jusqu'a la lecture suivante.
Source : refs/pandocs/src/Audio_Registers.md#FF1A - NR30: Channel 3 DAC enable ; #FF1C - NR32: Channel 3 output level ; #FF30-FF3F - Wave pattern RAM ; Audio_details.md#Wave channel (CH3) + Obscure Behavior
Fiabilite : communautaire
Impact code : puce8gb-core, module audio ch3 (index de lecture wave ram + tampon echantillon)
Statut : CONFIRME

## CH4 noise : LFSR et frequence
Fait : NR43 : bits 7-4 = clock shift, bit3 = largeur du LFSR (0=15-bit, 1=7-bit), bits 2-0 = divider ; le LFSR est horloge a 262144/(divider x 2^shift) Hz, un divider de 0 etant traite comme 0.5, et shift 14 ou 15 arrete l'horloge entierement. A chaque tick : bit15 <- (bit0 XNOR bit1), copie aussi dans bit7 en mode court, puis decalage droit ; la sortie vaut le volume de NR42 si bit0=1 sinon 0. Le LFSR est remis a zero au trigger ; passer de 15-bit a 7-bit quand les 7 bits bas sont tous a 1 le bloque (silencieux jusqu'au re-trigger).
Source : refs/pandocs/src/Audio_Registers.md#FF22 - NR43: Channel 4 frequency & randomness ; Audio_details.md#Noise channel (CH4)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio ch4 (etat LFSR + diviseur d'horloge)
Statut : CONFIRME

## DACs et valeurs au power-up
Fait : le DAC d'un canal est allume si et seulement si NRx2 & $F8 != 0 (CH3 : bit7 de NR30) ; l'envelope change le volume mais pas la valeur stockee dans NRx2, donc n'eteint jamais un DAC. Le numerique 0 correspond a l'analogique +1 (pente negative). Valeurs au power-up DMG/MGB (PC=$0100) : NR10=$80, NR11=$BF, NR12=$F3, NR13=$FF, NR14=$BF, NR21=$3F, NR22=$00, NR23=$FF, NR24=$BF, NR30=$7F, NR31=$FF, NR32=$9F, NR33=$FF, NR34=$BF, NR41=$FF, NR42=$00, NR43=$00, NR44=$BF, NR50=$77, NR51=$F3, NR52=$F1.
Source : refs/pandocs/src/Audio_details.md#DACs ; Power_Up_Sequence.md#Hardware-registers (L284-L304)
Fiabilite : communautaire
Impact code : puce8gb-core, module audio (enable DAC + valeurs de reset)
Statut : CONFIRME

## Masques de lecture et registres write-only
Fait : Hardware_Reg_List marque FF13/FF18/FF1B/FF1D comme W only ; les bits inutilises lisent haut : NR10 bit7, NR30 bits 6-0, NR32 tous sauf 6-5, NR41 bits 7-6, NR44 bits 5-0, NR52 bits 6-4 (regle generale "unused bits read high", IR.md L41). Ce que lit un registre write-only sur DMG n'est pas documente ; le tableau power-up donne $FF pour FF13/FF18/FF1D sans expliquer le mecanisme. Voir open_questions.md section D_08 (deja enregistree).
Source : refs/pandocs/src/Hardware_Reg_List.md#Hardware-registers ; refs/pandocs/src/IR.md (L41) ; Power_Up_Sequence.md#Hardware-registers
Fiabilite : communautaire
Impact code : puce8gb-core, module io (masques de lecture FF10-FF26 + valeurs write-only)
Statut : UNKNOWN - to confirm

## Laisse hors note
Laisse hors note : filtre HPF du mixer et audio pops, comportements GBA, Obscure Behavior (zombie mode NRx2, length clocking a l'ecriture de NRx4, premier duty step joue comme 0), references aux test ROMs SameSuite. Voir refs/pandocs/src/Audio_details.md#Mixer + #Obscure Behavior ; les points UNKNOWN ci-dessus sont deja dans open_questions.md (D_08).
