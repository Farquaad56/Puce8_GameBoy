# Reset CPU Puce8_GameBoy - mode d'emploi et prompts

Roles : ton IA locale (opencode / Hermes) fait TOUT le code et lance les commandes. Claude (moi) ne fait que
fournir les fichiers, relire les resultats que tu me colles, et rediger le prompt de correction suivant.

## 1. Installation (une seule fois, a la racine du depot)

    python3 _pack_cpu_reset/apply.py --commit
    python3 docs/tools/task.py status

Effet : les anciennes taches E02_02..E02_46 sont rangees dans docs/tasks/_old_e02 et docs/checks/_old_e02,
41 nouvelles taches C01_01..C01_41 sont installees (E02_01 registres reste DONE), la decision C_00 (moteur M-cycle)
est ajoutee a docs/annexes/decisions.md, AGENTS.md est corrige (1 acces bus MAX par M-cycle).
Sauvegardes : PROGRESS.json.bak_cpu_reset et INDEX.tsv.bak_cpu_reset. `--commit` evite que le controle de
perimetre de task.py voie ces fichiers comme "hors zone".

## 2. Boucle normale (une session = une tache)

Automatique : `docs/tools/drive.sh` (s'arrete tout seul aux GATE, BLOCKED, FINISHED).
Manuel : colle le PROMPT A dans une session neuve de ton IA locale.

### PROMPT A - session standard (a coller tel quel)

    Start with: python3 docs/tools/task.py show
    If the first line is not "STATE: RUN", say so in French and STOP.
    Do ONLY that task, following AGENTS.md and the decision C_00 in docs/annexes/decisions.md.
    Unit test first, small diffs, never more than 150 lines of a file opened at once.
    Finish with: python3 docs/tools/task.py check (repeat until all lines are [ok]), then python3 docs/tools/task.py done.
    Never run approve / reopen / unblock / addfix. Speak French to the user.

### Aux GATE (taches C01_04, puis chaque ROM Blargg)
L'IA affiche le bloc GATE et s'arrete. C'est ici que tu me parles : colle-moi le bloc GATE
(+ la sortie de `cargo run -q --release -p puce8gb-cli -- run <rom> --max-cycles 100000000` si utile).
Je reponds par "ok" ou par un prompt de correction. Quand tu es d'accord : `python3 docs/tools/task.py approve`.

## 3. Prompts de secours

### PROMPT B - une ROM Blargg echoue (la tache TEST n'arrive pas a SCORE plein)

    Task <ID> is a TEST task. Do NOT rewrite the CPU. Follow the "Work" section of the task file strictly:
    1) run the ROM with the CLI and read the exit code and printed text first;
    2) exit 4 = implement exactly the printed opcode (unit test first); exit 2 = run with --trace 300000 2> trace.txt
       and read only tail -n 40 trace.txt, name the looping PC and the polled address; exit 1 = read the failure text;
    3) one minimal reproduction as a unit test named c01_<NN>_*, one fix in ONE module, rerun.
    Same command failing twice without any change: stop and run task.py blocked "<reason>". Speak French.

### PROMPT C - tache BLOCKED (apres 5 checks rates)
Ne relance pas l'IA. Colle-moi : la raison de `task.py blocked`, les 40 dernieres lignes de `task.py check`,
et docs/annexes/open_questions.md (tail -n 20). Je te renvoie un prompt cible + eventuellement une tache corrigee.
Puis, toi : `python3 docs/tools/task.py unblock`.

### PROMPT D - regression (NON-REGRESSION FAILED)

    The non-regression step failed for: <ids>. Do not edit older check scripts and do not weaken any test.
    Run each failing check script from docs/checks/, read the [FAIL] lines, find the hardware cause in docs/annexes/notes,
    fix it in the smallest module, then run task.py check and task.py done again. Speak French.

### PROMPT E - l'IA derive / tourne en boucle
Interromps la session. Dans une session neuve : `python3 docs/tools/task.py blocked "model looping on <ID>"` puis
voir PROMPT C. (Regle AGENTS.md : meme commande en echec 2 fois sans changement = on change d'approche.)

## 4. Ce que tu me colles pour la boucle question/reponse

- le bloc GATE (ou la raison BLOCKED) ;
- `python3 docs/tools/task.py status | tail -15` ;
- si ROM en echec : la ligne `UNIMPLEMENTED opcode ...` ou les 40 dernieres lignes de trace.

## 5. Ordre des taches (resume)

C01_01 squelette CPU | 02 Dmg pilote le CPU | 03 serie | 04 CLI run (GATE)
05-14 instructions (LD, PUSH/POP, JP/JR, CALL/RET, INC/DEC, ALU) | 15 stub LY | 16 ROM 06 (GATE)
17-18 16 bits + SP | 19-21 ROM 03, 04, 05 | 22-23 acc + DAA | 24 ROM 01 | 25-26 CB
27-31 ROM 09, 10, 11, 07, 08 | 32 EI/DI/interrupts | 33 HALT/STOP | 34-35 timer | 36 ROM 02
37 instr_timing | 38-40 mem_timing 01-03 | 41 halt_bug (GATE final)

Notes de conception :
- cpu_instrs.gb (version 64 Ko combinee) n'est PAS ciblee : elle demande un MBC1 que le bus n'a pas. On utilise les 11 ROMs individuelles.
- interrupt_time est une ROM CGB seulement (voir game-boy-test-roms-howto.md) : volontairement absente.
- Si une ROM boucle sur un registre qui n'existe pas encore (LY, timer, serie), le PROMPT B demande de bloquer et de me
  donner le PC et l'adresse : on ajoutera alors une petite tache dediee plutot que de bricoler dans le CPU.
