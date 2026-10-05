# Note 02c - Source de la table d'opcodes SM83 (seed/Opcodes.json -> opcodes.rs)

Table machine-readable : docs/annexes/seed/Opcodes.json, source gbdev "gb-opcodes"
(https://gbdev.io/gb-opcodes/Opcodes.json), commitee dans f703d42 le 2026-10-05.
Le fichier genere docs/annexes/code/opcodes.rs via docs/annexes/code/gen_opcodes.py
(re-executable, deterministe, stdlib seul). Structure JSON : deux tables "unprefixed" et
"cbprefixed", cles 0x00..0xFF, chaque entree porte mnemonic, bytes, cycles, operands, flags.

## Les cycles du JSON sont des T-cycles ; M-cycle = T/4
Fait : Chaque valeur de "cycles" est un multiple de 4 (verifie par assertion sur les 512 entrees) et vaut des T-cycles. La table historique pandocs le confirme explicitement ("all gameboy timings are divideable by 4"), d'ou la convention M-cycle = T/4 deja enote dans note 01. Exemple : nop = 4 T = 1 M, jr nz e8 = [12;8] T = [3;2] M.
Source : refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (L2127-2130) ; docs/annexes/seed/Opcodes.json (commit f703d42, 2026-10-05)
Fiabilite : communautaire
Impact code : puce8gb-core - OpInfo.m_taken / m_not_taken sont des M-cycles ; gen_opcodes.py divise par 4 avec assertion de divisibilite.
Statut : CONFIRME

## Ordre des deux valeurs de cycles : [branch taken, branch not taken]
Fait : Quand "cycles" porte deux valeurs (16 opcodes de la table base : JR/RET/CALL conditionnes), le JSON les donne dans l'ordre [taken, not-taken]. Verifie croise avec pandocs : jr f,PC+dd = 12;8 et ret f = 20;8 correspondent aux paires JSON [12,8] (JR NZ) et [20,8] (RET NZ). Une valeur unique = cout fixe, dupliquee dans m_taken et m_not_taken.
Source : refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (L2242 jr 12;8, L2246 ret f 20;8) ; docs/annexes/seed/Opcodes.json (0x20 -> [12,8], 0xC0 -> [20,8])
Fiabilite : communautaire
Impact code : puce8gb-core - affectation de m_taken / m_not_taken dans gen_opcodes.py.
Statut : CONFIRME

## Les cycles de la table CB incluent deja le fetch du prefixe 0xCB
Fait : Les valeurs "cycles" de cbprefixed comptent le byte 0xCB ; il ne faut jamais y ajouter l'entree OPCODES[0xCB] (mnemonic PREFIX, 1 M). Couts pandocs : registre = 8 T = 2 M, (HL) = 16 T = 4 M, BIT n,(HL) = 12 T = 3 M ; le JSON donne exactement [8], [16], [12] pour RLC r / RLC (HL) / BIT 0,(HL).
Source : refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (L2203 rlc r CB 0x 8, L2204 rlc (HL) CB 06 16, L2222 bit n,(HL) CB xx 12) ; docs/annexes/seed/Opcodes.json (CB 0x06 -> [16], CB 0x46 -> [12])
Fiabilite : communautaire
Impact code : puce8gb-core - cout d'execution des instructions 0xCB dans le CPU, sans prefixe additionnel.
Statut : CONFIRME

## Les 11 opcodes illegaux font hard-locker le CPU ; leur cout JSON est a ignorer
Fait : Les opcodes illegaux de la table base sont D3 DB DD E3 E4 EB EC ED F4 FC FD (mnemonic ILLEGAL_xx dans le JSON, aucun en CB). La doc pandocs liste exactement ces 11 adresses et dit que le CPU se verrouille jusqu'au power-off. Le JSON leur attribue 1 M-cycle (cycles [4]) ; ne pas s'appuyer sur ce cout.
Source : refs/pandocs/src/CPU_Instruction_Set.md#CPU-Instruction-Set (L173, liste invalid + hard-lock) ; docs/annexes/seed/Opcodes.json (11 entrees ILLEGAL_xx, cycles [4])
Fiabilite : communautaire
Impact code : puce8gb-core - flag OpInfo.illegal (assertion dans gen_opcodes.py que la liste coincide et que cbprefixed en porte 0).
Statut : CONFIRME

## Notation des flags du JSON
Fait : Les valeurs de flags {Z,N,H,C} sont restreintes a "-" (inchange), "0", "1" ou la lettre elle-meme (calculee par l'instruction) ; verifie sur les 512 entrees. La colonne de flags de la table pandocs utilise le meme alphabet dans l'ordre znhc, ce qui permet une comparaison caractere a caractere avec le JSON.
Source : docs/annexes/seed/Opcodes.json (alphabet {-,0,1,Z,N,H,C} sur toutes les entrees) ; refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (L2126 "affected flags (ordered as znhc)")
Fiabilite : communautaire
Impact code : puce8gb-core - champs OpInfo.z / n / h / c ; gen_opcodes.py asserte l'alphabet.
Statut : CONFIRME

## La table suit le layout SM83 de pandocs, pas celui du Z80
Fait : L'expansion des templates {{#bits}} de la doc pandocs couvre exactement les entrees du JSON aux adresses testees (cf. spot-checks), y compris la difference avec le Z80 : JP HL vaut 0xE9 dans les deux sources, LDH (C),A vaut 0xE2 et LDH A,(C) vaut 0xF2, LD (a16),SP vaut 0x08.
Source : refs/pandocs/src/CPU_Instruction_Set.md#CPU-Instruction-Set (L54 "ld [imm16], sp" -> 0x08 ; L138 "jp hl" -> 0xE9 ; bloc L154-L159 pour E0/F2) ; refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (E0/E2/F2 dans la table des 8-bit loads)
Fiabilite : communautaire
Impact code : puce8gb-core - decodage des opcodes par le CPU ; ne pas importer de mnemonic Z80.
Statut : CONFIRME

## Spot-checks manuels du JSON contre pandocs (10 opcodes) : aucun conflit
Fait : Comparaison a la main (grep + sed) de 10 opcodes entre seed/Opcodes.json et la table historique pandocs, bytes / M-cycles / flags : 0x00 NOP [4T=1M], 0x20 JR NZ,e8 [12;8 T = 3;2 M], 0x34 INC (HL) [12 T = 3 M, z0h-], 0x76 HALT [base 1 M ; la doc donne N*4 car l'attente d'interrupt est variable, pas un conflit], 0xC0 RET NZ [20;8 T = 5;2 M], 0xCD CALL a16 [24 T = 6 M, 3 bytes], 0xE8 ADD SP,e8 [16 T = 4 M, flags 00hc], 0xF8 LD HL,SP+e8 [12 T = 3 M, flags 00hc], CB 0x06 RLC (HL) [16 T = 4 M, z00c], CB 0x46 BIT 0,(HL) [12 T = 3 M, z01-]. Les 10 valeurs concordent ; gen_opcodes.py les re-verifie par assertions. Aucun CONFLIT : la resolution par ROM de test (blargg cpu_instrs / instr_timing, presentes dans roms/test-roms) n'a pas ete necessaire.
Source : refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (L2185 inc(HL), L2195 add SP,dd E8, L2196 ld HL,SP+dd F8, L2204 rlc(HL) CB 06, L2222 bit n,(HL), L2231 nop, L2232 halt N*4, L2242 jr f 12;8, L2243 call nn CD 24, L2246 ret f 20;8) ; docs/annexes/seed/Opcodes.json
Fiabilite : communautaire
Impact code : puce8gb-core - SPOT_CHECKS de gen_opcodes.py (bytes, M-cycles, flags des 10 entrees).
Statut : CONFIRME

## Regle de construction des textes mnemoniques et cas special 0xF8
Fait : Le texte d'une instruction = mnemonic + " " + operande(s) separes par "," ; un operande non immediate s'ecrit entre parentheses, suffixe "+" (increment) ou "-" (decrement), d'ou INC (HL), LDH (C),A. Cas special 0xF8 : l'operande SP+ suivi de e8 forme "SP+e8" sans virgule, si bien que 0xF8 lit exactement "LD HL,SP+e8". gen_opcodes.py asserte la presence des textes requis JP HL, LDH (C),A, LDH (a8),A, LD (a16),SP et du texte de 0xF8 dans la sortie.
Source : docs/annexes/seed/Opcodes.json (structure operands de 0xF8) ; refs/pandocs/historical/2001-Oct-pandocs.txt#CPU-Instruction-Set (L2196 "ld HL, SP+dd")
Fiabilite : deduite
Impact code : puce8gb-core - OpInfo.mnemonic (chaines &'static str) ; regle encodee dans mnemonic_text() de gen_opcodes.py.
Statut : CONFIRME

## Proprietes du fichier genere opcodes.rs
Fait : opcodes.rs contient pub struct OpInfo et les constantes OPCODES / CB_OPCODES, chacune avec 256 entrees dans l'ordre d'index 0x00..0xFF (garanti par construction + assertion sur les cles). Pas de logique Rust, pas d'allocation, aucune dependance ; fichier purement ASCII (assertion), header "generated by gen_opcodes.py from seed/Opcodes.json, do not edit". Verifie par recompilation avec rustc en crate lib.
Source : docs/annexes/code/gen_opcodes.py (sections build_entries / render / main) ; docs/annexes/code/opcodes.rs (fichier genere)
Fiabilite : deduite
Impact code : puce8gb-core - le fichier sera copie sans lecture dans le crate core lors de E02.
Statut : CONFIRME
