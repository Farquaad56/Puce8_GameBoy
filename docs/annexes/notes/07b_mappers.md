# Note 07b - Mappers (MBC) de cartouche, DMG

Tache D_15. Sources : refs/pandocs/src/{nombc,MBCs,MBC1,MBC2,MBC3,MBC5}.md et refs/pandocs/historical/2001-Oct-pandocs.txt.
Le type de mapper est choisi par l'octet 0147 de la ROM (voir note 07a). Le DMG n'a pas de mode CGB ; les faits marques "CGB only" ne concernent que le double speed.

## No-MBC / ROM-only (32 KiB)
Fait : une cartouche sans MBC a au plus 32 KiB de ROM, mappee directement en 0000-7FFF ; jusqu'a 8 KiB de RAM peuvent etre branchees en A000-BFFF via un decodeur logique discret (pas de puce MBC).
Source : refs/pandocs/src/nombc.md#No-MBC (L3-L8) ; historical/2001-Oct-pandocs.txt L2453-L2459
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche/mapper (cas 0147 = $00/$08/$09 : pas de banking ROM)
Statut : CONFIRME

## Choix du mapper par l'octet 0147
Fait : 0147 code le type de cartouche ; les codes pertinents pour le DMG sont $00 ROM only, $01-$03 MBC1 (+RAM/+BATTERY), $05/$06 MBC2 (+BATTERY), $0F/$10/$13 MBC3+TIMER(+RAM)(+BATTERY), $19-$1E MBC5 (+RAM/RUMBLE/BATTERY). Le bit "battery" n'indique que la presence d'une pile, pas un comportement different.
Source : refs/pandocs/src/MBCs.md#MBCs (L8-L12) ; historical/2001-Oct-pandocs.txt L2364-L2378
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche/mapper (table de decodage 0147 -> type de mapper + tailles ROM/RAM)
Statut : CONFIRME

## MBC1 : configuration par defaut et wiring alternatif
Fait : en configuration par defaut le MBC1 supporte jusqu'a 512 KiB de ROM avec jusqu'a 32 KiB de RAM banked ; les cartouches d'au moins 1 MiB re-cablent le registre a 2 bits (normalement la banque RAM) comme extension du registre de banque ROM, au prix d'une RAM fixe de 8 KiB.
Source : refs/pandocs/src/MBC1.md#MBC1 (L3-L13) ; historical/2001-Oct-pandocs.txt L2461-L2479
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC1 (deux modes de cablage selon la taille ROM declaree)
Statut : CONFIRME

## MBC1 : registre et plages d'adresses
Fait : 0000-1FFF = enable RAM ; 2000-3FFF = banque ROM (5 bits, plage $01-$1F) ; 4000-5FFF = banque RAM ($00-$03) OU les 2 bits superieurs de la banque ROM selon le mode ; 6000-7FFF = selection du mode de banking. Tous ces registres valent $00 au power-up, sauf que la banque ROM traite $00 comme $01.
Source : refs/pandocs/src/MBC1.md#Registers (L42-L98) ; historical/2001-Oct-pandocs.txt L2486-L2517
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC1 (quatre registres ecris dans 0000-7FFF, lus via la ROM)
Statut : CONFIRME

## MBC1 : particularites de banque 0 et des banques $20/$40/$60
Fait : ecrire $00 dans le registre de banque ROM donne la banque $01 (on ne peut pas dupliquer la banque 0 en 4000-7FFF) ; sur les cartouches >512 KiB, selectionner les banques $20/$40/$60 (seuls bits superieurs poses) renvoie a $21/$41/$61 au lieu. Le registre de 5 bits complet est compare pour la conversion 00->01, meme si moins de 5 bits servent a choisir la banque.
Source : refs/pandocs/src/MBC1.md#2000-3FFF (L57-L86) ; historical/2001-Oct-pandocs.txt L2496-L2504
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC1 (regle 00->01 sur le registre complet + masque des bits superieurs)
Statut : CONFIRME

## MBC1 : enable de la RAM externe
Fait : ecrire une valeur dont les 4 bits bas valent $A en 0000-1FFF active la RAM externe, toute autre valeur la desactive ; la RAM est desactivee par defaut et il est recommande de la desactiver apres usage pour proteger le contenu au power-down.
Source : refs/pandocs/src/MBC1.md#0000-1FFF (L46-L55) ; historical/2001-Oct-pandocs.txt L2486-L2494
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC1 (flag RAM enable = (val & $F) == $A)
Statut : CONFIRME

## MBC1 : mode de banking (simple / avance)
Fait : le bit du registre 6000-7FFF choisit entre mode simple (defaut : 0000-3FFF et A000-BFFF verrouilles sur la banque 0) et mode avance (les deux plages peuvent etre banked via le registre a 2 bits) ; on peut basculer librement entre les deux modes.
Source : refs/pandocs/src/MBC1.md#6000-7FFF (L98-L124) ; historical/2001-Oct-pandocs.txt L2510-L2517
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC1 (mode bit controle le role du registre 4000-5FFF et le verrouillage de la banque 0)
Statut : CONFIRME

## MBC2 : RAM integree et map memoire
Fait : le MBC2 a au plus 256 KiB de ROM (16 banques, 4000-7FFF = $01-$0F) et n'a pas de RAM externe ; il embarque 512 demi-octets (512x4 bits) de RAM integree en A000-A1FF, repetee ("echo") jusqu'a BFFF, ne conservant que les 4 bits bas de chaque octet ; une pile externe reste necessaire pour conserver la donnee.
Source : refs/pandocs/src/MBC2.md#Memory (L5-L26) ; historical/2001-Oct-pandocs.txt L2519-L2528
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC2 (RAM 4 bits en A000-A1FF avec echo, pas de banking RAM externe)
Statut : CONFIRME

## MBC2 : registre et particularites
Fait : le bit 8 de l'adresse ecrise dans 0000-3FFF decide du role : bit 8 a 0 = enable/desable la RAM (activee si les 4 bits bas valent $A), bit 8 a 1 = choisit la banque ROM (4 bits bas) ; ecrire la banque 0 donne la banque 1. Par defaut la RAM est desactivee et la banque ROM vaut 1.
Source : refs/pandocs/src/MBC2.md#Registers (L29-L57) ; historical/2001-Oct-pandocs.txt L2535-L2548
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC2 (role du registre depend du bit 8 de l'adresse ecrise)
Statut : CONFIRME

## MBC3 : vue d'ensemble (ROM + RAM + RTC)
Fait : le MBC3 supporte jusqu'a 2 MiB de ROM (128 banques), 32 KiB de RAM en 4 banques de 8 KiB, et embarque un horloge temps reel (RTC) ; le RTC exige un oscillateur quartz externe a 32.768 kHz et une pile pour continuer a tourner a l'extinction.
Source : refs/pandocs/src/MBC3.md#MBC3 (L3-L7) ; historical/2001-Oct-pandocs.txt L2550-L2553
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC3 (banking ROM + RAM 4 banques + bloc RTC)
Statut : CONFIRME

## MBC3 : registre et plages d'adresses
Fait : 0000-1FFF = enable RAM+timer ($0A active, $00 desactive) ; 2000-3FFF = banque ROM (7 bits ecris directement, $00 donne $01) ; 4000-5FFF = selection de ce qui est mappe en A000-BFFF ($00-$07 = banque RAM, $08-$0C = registre RTC) ; 6000-7FFF = latch des donnees horloge.
Source : refs/pandocs/src/MBC3.md#Registers (L27-L65) ; historical/2001-Oct-pandocs.txt L2571-L2589
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC3 (registre de selection 4000-5FFF bascule entre RAM et RTC en A000-BFFF)
Statut : CONFIRME

## MBC3 : registres du RTC ($08-$0C)
Fait : le registre RTC mappe en A000-BFFF (lu/ecrit a n'importe quelle adresse de la plage, typiquement A000) est : $08 secondes 0-59, $09 minutes 0-59, $0A heures 0-23, $0B 8 bits bas du compteur de jours (0-FF), $0C bit superieur du compteur de jours + bit de carry + flag halt (bit6 : 0=actif/1=arret).
Source : refs/pandocs/src/MBC3.md#Clock-Counter-Registers (L67-L77) ; historical/2001-Oct-pandocs.txt L2597-L2608
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC3 RTC (cinq registres $08-$0C avec halt/carry sur $0C)
Statut : CONFIRME

## MBC3 : latch et compteur de jours
Fait : ecrire $00 puis $01 dans 6000-7FFF fige l'heure courante dans les registres RTC (qui ne changent plus jusqu'a un nouveau latch), permettant de lire le temps pendant que l'horloge tourne ; le compteur de jours fait 9 bits (0-511) et son bit de carry reste pose apres depassement jusqu'a ce que le programme le reinitialise, le flag halt devant etre pose avant ecrire dans les registres RTC.
Source : refs/pandocs/src/MBC3.md#Latch-Clock-Data + #The-Day-Counter (L59-L89) ; historical/2001-Oct-pandocs.txt L2589-L2621
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC3 RTC (latch $00->$01 + compteur de jours 9 bits avec carry persistant)
Statut : CONFIRME

## MBC3 : delai entre acces aux registres RTC (conflit)
Fait : le doc historique recommande un delai de "4ms (4 Cycles en Normal Speed Mode)" entre deux acces aux registres RTC, tandis que la page actuelle dit "4 us (4 M-cycles en Normal Speed Mode)" ; les deux valeurs sont documentees et aucune ROM test locale ne mesure directement ce delai inter-acces.
Source : historical/2001-Oct-pandocs.txt L2618-L2620 vs refs/pandocs/src/MBC3.md#Delays (L91-L95) ; a trancher par roms/test-roms/rtc3test (RTC MBC3)
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC3 RTC (delai minimum a modeler entre acces consecutifs aux registres $08-$0C)
Statut : CONFLIT

## MBC5 : vue d'ensemble (ROM 8 MiB + RAM banked)
Fait : le MBC5 mappe jusqu'a 64 Mbits (8 MiB) de ROM, soit les banques $00-$1FF en 4000-7FFF, et des tailles de RAM externe de 8/32/128 KiB ; c'est la premiere puce MBC garantie par Nintendo pour le double speed CGB.
Source : refs/pandocs/src/MBC5.md#MBC5 (L3-L21) ; historical/2001-Oct-pandocs.txt L2636
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC5 (banking ROM 9 bits + RAM banked)
Statut : CONFIRME

## MBC5 : registre et bit 9 de banque + rumble
Fait : 0000-1FFF = enable RAM ($0A active, $00 desactive) ; 2000-2FFF = 8 bits bas de la banque ROM (ecrire 0 donne bien la banque 0, contrairement aux autres MBC) ; 3000-3FFF = bit 9 de la banque ROM ; 4000-5FFF = banque RAM ($00-$0F), dont le bit 3 pilote le moteur rumble sur les cartouches equippees.
Source : refs/pandocs/src/MBC5.md#Registers + #Rumble (L23-L60) ; historical/2001-Oct-pandocs.txt L2373-L2375
Fiabilite : communautaire
Impact code : puce8gb-core, module mapper MBC5 (registre de banque ROM 9 bits sur deux plages + bit rumble dans la banque RAM)
Statut : CONFIRME

## Omissions / a confirmer
- Delai exact entre acces aux registres RTC du MBC3 (4ms vs 4us) : voir le fait CONFLIT ci-dessus et docs/annexes/open_questions.md. Statut : CONFLIT - to confirm.
- Comportement precis de l'acces a une banque RAM non mappee (wrap-around) sur chaque puce MBC : formule donnee generalement dans refs/pandocs/src/MBCs.md#MBC-Unmapped-RAM-Bank-Access mais non detaillee par puce ; sans effet majeur sur le DMG. Statut : UNKNOWN - to confirm.
