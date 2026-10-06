
## C_00

Decision: CPU restart - moteur M-cycle ecrit a la main (remplace A_03 pour le CPU). Les ROMs de test Blargg (port serie) sont l'oracle.
Questions a trancher : (1) Unite d'un tick CPU. (2) Etat du moteur et representation d'une instruction. (3) Decodage. (4) Fetch / chevauchement. (5) Instruction non implantee. (6) Interrupts, HALT, STOP. (7) Outil de test.

Options :

Option A - Instruction executee d'un coup, retour du nombre de cycles. Simple, mais mem_timing exige l'ordre exact des acces bus : refactor garanti plus tard.
Option B - Machine a etats par M-cycle : un tick = un M-cycle = au plus un acces bus ; l'instruction avance d'une etape par tick (compteur `step`), code ecrit a la main par groupes d'opcodes (match sur les champs de bits), aucune table generee dans le CPU.
Option C - Idem B avec fetch de l'opcode suivant chevauche sur le dernier M-cycle de l'instruction. Plus exact pour certaines ROMs mooneye, mais l'effet sur EI et DMA est UNKNOWN (open_questions D_06/D_07) ; difficile a debugger avec une IA locale.

Recommandation :
Option B. Le chevauchement (option C) est reporte ; il ne changera pas l'API publique.

Consequence :
- Un tick CPU = un M-cycle (4 dots, appele par Dmg::tick tous les 4 dots) = AU PLUS un acces bus. Les M-cycles internes (ex. INC rr, ADD HL,rr, le M-cycle interne de CALL/PUSH/RET cc) n'ont aucun acces bus. La regle d'AGENTS.md "one micro-op = exactly one bus access" est remplacee par "at most one bus access per M-cycle".
- Etat prive du Cpu (taille fixe, Copy, aucune allocation) : opcode: u8, step: u8, lo: u8, hi: u8 (verrous d'operandes), cb: bool, ime, ime_pending, halted, unimplemented: Option<(u8,u16)>. Les registres a f b c d e h l sp pc restent des champs publics (machine.rs et les tests E01 les utilisent) ; Cpu::reset(checksum), Cpu::default(), Cpu::regs() restent identiques.
- tick() : si step == 0 => (a) frontiere d'instruction : verifier interrupt/halt (tache C01_32/33), sinon (b) fetch : opcode = read(pc), pc+1, step = 1. Sinon step += 1. Puis la chaine de dispatch appelle les groupes dans l'ordre : chaque groupe `exec_xxx(&mut self, bus) -> bool` renvoie true s'il reconnait l'opcode. Etape 1 = le M-cycle du fetch vient d'avoir lieu : travail registre seulement, AUCUN acces bus. Les etapes >= 2 font au plus un acces bus. Fin d'instruction : `self.done()` met step a 0.
- Un NOP dure 1 tick (le fetch). Le total de M-cycles d'une instruction = valeur de la note 02c (le fetch compte). Opcode CB : etape 1 = prefixe fetch ; etape 2 = fetch du second octet (acces bus) ; les formes (HL) ajoutent lecture puis ecriture.
- Decodage par champs de bits : x = opcode >> 6, y = (opcode >> 3) & 7, z = opcode & 7, p = y >> 1, q = y & 1. Ordres a verifier dans Pan Docs (grep) : r8 = B C D E H L (HL) A ; rr = BC DE HL SP ; push/pop = BC DE HL AF ; cc = NZ Z NC C.
- Opcode non reconnu par aucun groupe : enregistre dans `unimplemented = Some((opcode, adresse_de_l_opcode))`, coute 1 M-cycle, step revient a 0, pas de panic. Le CLI s'arrete dessus (code retour 4) : c'est la boussole pour avancer ROM par ROM.
- Les operations ALU et de flags sont des fonctions pures (testables sans bus). F a toujours son quartet bas a 0.
- Outil de test : `#[cfg(test)] cpu::testutil::exec(cpu, bus, &[u8]) -> u32` place le code en WRAM ($C000), execute jusqu'a la frontiere et renvoie le nombre de ticks.
- Interrupts, HALT, STOP, EI retarde : comme A_03 (notes 02b), implantes aux taches C01_32 et C01_33 ; l'interrupt est servi a la frontiere d'instruction avant le fetch.
- Critere de validation : serie Blargg "Passed" dans l'ordre de docs/tasks/C01 (06, 03, 04, 05, 01, 09, 10, 11, 07, 08, 02, puis instr_timing, mem_timing 01-03, halt_bug).

Statut : APPROUVE
