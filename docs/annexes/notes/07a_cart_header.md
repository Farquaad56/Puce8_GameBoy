# Note 07a - En-tete de cartouche (cartridge header), DMG

Tache D_14. Sources : refs/pandocs/src/The_Cartridge_Header.md, refs/pandocs/src/Power_Up_Sequence.md, refs/pandocs/historical/2001-Oct-pandocs.txt.
L'en-tete occupe 0100-014F de la premiere banque ROM (80 octets). Le DMG n'a pas de mode CGB ; les faits marques "CGB only" ne concernent que le drapeau 0143 et la verification du logo.

## Zone 0100-014F : repartition des champs
Fait : l'en-tete contient, dans l'ordre : point d'entree (0100-0103), logo Nintendo (0104-0133), titre (0134-0143), code fabricant (013F-0142), drapeau CGB (0143), nouveau code licensee (0144-0145), drapeau SGB (0146), type de cartouche (0147), taille ROM (0148), taille RAM (0149), code destination (014A), ancien code licensee (014B), version Mask ROM (014C), checksum d'en-tete (014D), checksum global (014E-014F).
Source : refs/pandocs/src/The_Cartridge_Header.md#The-Cartridge-Header (L3) ; historical/2001-Oct-pandocs.txt L2307-L2312
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (lecture des octets 0147/0148/0149 pour choisir le mapper et les tailles)
Statut : CONFIRME

## Point d'entree 0100-0103
Fait : apres avoir affiche le logo, la boot ROM saute a $0100 ; cette zone de 4 octets doit ensuite sauter vers le vrai programme (generalement un NOP suivi de JP $0150).
Source : refs/pandocs/src/The_Cartridge_Header.md#0100-0103 - Entry point (L6-L9)
Fiabilite : communautaire
Impact code : puce8gb-core, module machine (PC initial = 0100 apres la boot ROM)
Statut : CONFIRME

## Logo Nintendo 0104-0133 et verification par la boot ROM
Fait : les octets 0104-0133 encodent le logo affiche au demarrage ; il doit correspondre a un dump fixe de 48 octets (CE ED 66 66 CC ... BB B9 33 3E), sinon la boot ROM se verrouille. Le DMG/MGB verifie les 0x30 octets complets ; le CGB et suivants ne verifient que les 0x18 premiers octets (moitie haute).
Source : refs/pandocs/src/The_Cartridge_Header.md#0104-0133 - Nintendo logo (L11-L32) ; historical/2001-Oct-pandocs.txt L2316-L2325
Fiabilite : communautaire
Impact code : puce8gb-core, module machine/boot (comparaison du logo charge depuis la ROM avant de laisser le controle a la cartouche)
Statut : CONFIRME

## Ce que la boot ROM verifie (logo + checksum d'en-tete)
Fait : apres avoir fait defiler le logo et joue le son, la boot ROM relit le logo pour le comparer a une copie stockee ET calcule le checksum d'en-tete ; si l'un des deux echechouit, elle se verrouille et ne passe jamais le controle a la cartouche. La boot ROM DMG0 effectue les deux verifications avant d'afficher quoi que ce soit (ecran qui clignote en cas d'echec).
Source : refs/pandocs/src/Power_Up_Sequence.md#Power-Up-Sequence (L26-L45) ; The_Cartridge_Header.md#014D - Header checksum (L433-L447)
Fiabilite : communautaire
Impact code : puce8gb-core, module machine/boot (deux controles a reproduire : logo + checksum 014D)
Statut : CONFIRME

## Titre / fabricant / drapeau CGB (0134-0143)
Fait : 0134-0143 porte le titre en ASCII majuscule (padded de $00). Sur les cartouches recentes, 013F-0142 devient un code fabricant a 4 caracteres et 0143 un drapeau CGB ; bit7 de 0143 active le mode couleur (valeurs typiques $80 = compatible mono+CGB, $C0 = CGB only). Le DMG ignore ce drapeau.
Source : refs/pandocs/src/The_Cartridge_Header.md#0134-0143 - Title / #0143 - CGB flag (L34-L59) ; historical/2001-Oct-pandocs.txt L2327-L2349
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (aucun effet sur le DMG ; a ignorer)
Statut : CONFIRME

## Drapeau SGB 0146 et codes licensee (0144-0145 / 014B)
Fait : 0146 vaut $03 si la cartouche supporte les fonctions SGB, sinon le SGB ignore ses paquets de commande. Le code licensee est l'ancien octet 014B (pre-SGB), sauf quand il vaut $33 auquel cas le nouveau code a deux caracteres 0144-0145 s'applique ; le SGB n'active ses fonctions que si 014B = $33.
Source : refs/pandocs/src/The_Cartridge_Header.md#0146 - SGB flag / #014B - Old licensee code (L135-L239) ; historical/2001-Oct-pandocs.txt L2356-L2362, L2411-L2416
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (sans effet sur le DMG ; a ignorer)
Statut : CONFIRME

## Type de cartouche 0147 (codes mapper)
Fait : 0147 indique le hardware present. Codes principaux : $00 ROM ONLY, $01 MBC1, $02 MBC1+RAM, $03 MBC1+RAM+BATTERY, $05 MBC2, $06 MBC2+BATTERY, $08/$09 ROM(+BATTERY)+RAM (jamais utilise), $0B-$0D MMM01, $0F-$13 MBC3 (+TIMER/+/BATTERY), $19-$1E MBC5 (+RUMBLE/+RAM/+BATTERY), $20 MBC6, $22 MBC7+SENSOR+RUMBLE+RAM+BATTERY, $FC POCKET CAMERA, $FD BANDAI TAMA5, $FE HuC3, $FF HuC1+RAM+BATTERY.
Source : refs/pandocs/src/The_Cartridge_Header.md#0147 - Cartridge type (L140-L180) ; historical/2001-Oct-pandocs.txt L2363-L2381
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (selection du mapper selon 0147)
Statut : CONFLIT

## Type de cartouche 0147 : conflit MBC4
Fait : le doc historique liste $15/$16/$17 comme MBC4 / MBC4+RAM / MBC4+RAM+BATTERY, mais la page actuelle The_Cartridge_Header.md ne liste aucun code MBC4 (elle saute de $13 a $19). Les deux valeurs sont documentees ; aucune ROM test locale sous roms/test-roms/ ne trancher.
Source : historical/2001-Oct-pandocs.txt L2367-L2369 vs The_Cartridge_Header.md#0147 - Cartridge type (L140-L180)
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (table des codes 0147 a completer ou non selon le mapper MBC4)
Statut : CONFLIT

## Taille ROM 0148
Fait : la taille ROM vaut generalement 32 KiB x (1 << valeur) : $00=32K(2 banques), $01=64K, $02=128K, $03=256K, $04=512K, $05=1M, $06=2M, $07=4M, $08=8M. Les valeurs $52/$53/$54 (1.1/1.2/1.5 Mo) n'apparaissent que dans des docs non officiels et ne sont connues d'aucune cartouche ; elles sont probablement inexactes.
Source : refs/pandocs/src/The_Cartridge_Header.md#0148 - ROM size (L181-L205) ; historical/2001-Oct-pandocs.txt L2382-L2395
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (nombre de banques ROM = 2^(valeur+1))
Statut : CONFIRME

## Taille RAM 0149
Fait : $00=aucune RAM, $02=8K(1 banque), $03=32K(4x8K), $04=128K(16x8K), $05=64K(8x8K). Si le type 0147 ne mentionne pas "RAM", ce champ vaut 0 (y compris MBC2, dont les 512x4 bits sont integres au mapper).
Source : refs/pandocs/src/The_Cartridge_Header.md#0149 - RAM size (L206-L228) ; historical/2001-Oct-pandocs.txt L2396-L2404
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (taille/banques de SRAM selon 0149)
Statut : CONFLIT

## Taille RAM 0149 : conflit valeur $01
Fait : le doc historique donne $01 = 2 KBytes, mais la page actuelle dit que $01 est "unused" (aucune puce de 2K n'a jamais ete utilisee) ; les ROMs homebownes "(PD)" qui utilisent $01 l'ont fait par erreur. Les deux valeurs sont documentees ; aucune ROM test locale ne trancher.
Source : historical/2001-Oct-pandocs.txt L2399 vs The_Cartridge_Header.md#0149 - RAM size (L206-L228)
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (interpretation du champ 0149 = $01)
Statut : CONFLIT

## Checksum d'en-tete 014D
Fait : 014D contient un checksum 8 bits calcule sur les octets 0134-014C par l'algo x=0; for i in 0134..014C: x = x - rom[i] - 1. La boot ROM le verifie ; si 014D ne correspond pas aux 8 bits bas du resultat, elle se verrouille et la cartouche ne demarre pas.
Source : refs/pandocs/src/The_Cartridge_Header.md#014D - Header checksum (L433-L447) ; historical/2001-Oct-pandocs.txt L2420-L2426
Fiabilite : communautaire
Impact code : puce8gb-core, module machine/boot (reproduire l'algo x = x - byte - 1 sur 0134-014C)
Statut : CONFIRME

## Checksum global 014E-014F
Fait : 014E-014F contient un checksum 16 bits (octet haut d'abord, big-endian) egal a la somme de tous les octets de la ROM sauf ces deux octets. Le hardware DMG ne le verifie pas ; seul l'emulateur "GB Tower" de Pokemon Stadium le controle.
Source : refs/pandocs/src/The_Cartridge_Header.md#014E-014F - Global checksum (L449-L453) ; historical/2001-Oct-pandocs.txt L2427-L2430
Fiabilite : communautaire
Impact code : puce8gb-core, module cartouche (aucune verification requise sur le DMG)
Statut : CONFIRME

## Omissions / a confirmer
- Codes licensee complets (tables 0144-0145 et 014B) : listes longues non reproduites ici ; sans effet sur le DMG. Statut : UNKNOWN - to confirm.
- Comportement exact des valeurs $08/$09 (ROM+RAM, jamais utilisees) : inconnu dans les sources. Statut : UNKNOWN - to confirm.
