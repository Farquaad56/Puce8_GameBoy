# Systeme agentique - emulateur Game Boy (DMG) en Rust, pour Hermes Agent

Pipeline sequentiel : **un seul LLM (Nous Hermes) pilote par Hermes Agent, une seule tache a la fois, une session one-shot neuve par tache**.
Le LLM ne decide jamais de la suite : `docs/tools/task.py` le fait.

> CPU reel de la Game Boy d'origine : **Sharp SM83** (pas un Zilog Z80).

## Contenu
| Element | Role |
|---|---|
| `AGENTS.md` | Regles condensees, injectees automatiquement par Hermes Agent a chaque session (cwd = racine du depot) |
| `.hermes/skills/gb-{doc,arch,code,test}/SKILL.md` | 4 roles = skills de projet, precharges avec `-s` selon la tache |
| `PROGRESS.json` | Etat : tache courante, GATE en attente, echecs, statut de chaque tache |
| `docs/tasks/INDEX.tsv` + `docs/tasks/<phase>/*.md` | 85 sous-taches (moins de 70 lignes chacune) |
| `docs/checks/<id>.sh` | Autotest par **score** (`SCORE p/n`), un par tache |
| `docs/tools/task.py` | Pilote : show, check, done, blocked, approve, reopen, unblock, addfix, status |
| `docs/tools/drive.sh` | Boucle : `hermes chat --oneshot -Q -s gb-<role>[,opencode]` par tache |
| `opencode.json` | Config OpenCode du depot (point d'acces local, permissions) |
| `docs/tools/dashboard.py` + `docs/dashboard/index.html` | Tableau de bord temps reel (Tailwind CSS), lecture seule |
| `docs/annexes/` | notes, decisions.md, open_questions.md, coverage.md, baseline |

## Installation de Hermes Agent
```bash
curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash   # Linux, macOS, WSL2
source ~/.bashrc
hermes profile create gb-emu          # profil isole pour ce projet
hermes -p gb-emu model                # assistant : choisir le point d'acces custom / local (ton Hermes servi en local)
hermes -p gb-emu config set agent.max_turns 80
hermes -p gb-emu config set skills.write_approval true     # le "learning loop" ne cree pas de skill sans toi
hermes -p gb-emu config set memory.write_approval true     # idem pour la memoire
```
Dans le depot : `hermes -p gb-emu skills trust` (obligatoire : les skills de projet ne se chargent qu'apres cette confirmation).
Ne PAS utiliser `--ignore-rules` ni `--safe-mode` : ils desactivent AGENTS.md et les skills prechargees.
Le serveur local doit exposer le tool calling (ex. `llama-server --jinja -c <ctx> -m <hermes>.gguf`) ; donne a l'assistant `hermes model` la vraie taille de contexte.

## Mise en route
1. Copier ce dossier a la racine du depot, puis `git add -A && git commit -m "agent system"` (le controle de perimetre compare `git status`).
2. Authentification Git par SSH ou gestionnaire d'identifiants. **Jamais de jeton dans un fichier du depot.**
3. Premier lancement manuel (pour valider les approbations de commandes) :
   `hermes -p gb-emu -s gb-code` puis : `Start with: python3 docs/tools/task.py show. Do only that task, following AGENTS.md.`
   Approuve les commandes habituelles (cargo, git, python3 docs/tools/task.py) ; en cas de blocage en mode non interactif, regarde `hermes approvals` et la page Security de la doc.
4. Ensuite : `docs/tools/drive.sh` (variables : `HERMES_PROFILE`, `MAX_TURNS`).

## Hermes orchestre, OpenCode code (mode par defaut)
Hermes Agent embarque la skill `opencode` (autonomous-ai-agents/opencode : `opencode run '...'` depuis le terminal de Hermes).
- Taches **DOC** et **ARCH** : Hermes les fait lui-meme (lecture de Pan Docs, notes, decisions).
- Taches **CODE** et **TEST** : `drive.sh` charge `-s gb-code,opencode` ; Hermes lit la tache, lance `opencode run` sur le depot, puis verifie avec `task.py show` / `check`.
  OpenCode lit `AGENTS.md`, fait la tache, lance `check` puis `done`. En cas d'echec, Hermes relance OpenCode au plus 2 fois avec les seules lignes `[FAIL]`, puis `task.py blocked`.
- Un seul LLM, une seule requete active a la fois : pendant qu'OpenCode travaille, Hermes attend le resultat du terminal.
- `GB_CODER=self docs/tools/drive.sh` : Hermes code lui-meme (sans OpenCode). `GB_OPENCODE_MODEL=provider/modele` : autre modele pour OpenCode.
- Prerequis : `npm i -g opencode-ai@latest` (ou brew), `opencode.json` adapte (`baseURL`, modele, `limit.context`) ; `opencode auth list` doit montrer un fournisseur si tu n'utilises pas le point d'acces local.
- Si le terminal de Hermes coupe une longue execution, la skill `opencode` prevoit un mode en arriere-plan avec PTY.

## Cycle d'une tache
`show` -> l'agent travaille -> `check` (score, perimetre `Write only in`, fmt/clippy/test) -> `done` (non-regression des taches precedentes, commit `<id>: <titre>`, push sans force, tache suivante).
5 `check` rates ou 4 sessions sans fin : la tache passe en BLOCKED (anti-boucle) et une ligne va dans `open_questions.md`.

## GATE (tu valides)
D_18, A_01..A_07 et la derniere tache de chaque phase s'arretent. Les commandes ci-dessous demandent de **taper YES au clavier**
(lecture sur /dev/tty : l'agent ne peut pas repondre a ta place, et sans terminal la commande est refusee) :
- `python3 docs/tools/task.py approve` : valide (pour A_xx, `Statut : PROPOSE` devient `APPROUVE`)
- `python3 docs/tools/task.py reopen [id]` : rouvre la tache, puis donne tes corrections a l'agent
- `python3 docs/tools/task.py unblock` : debloque une tache BLOCKED
- `python3 docs/tools/task.py addfix "<nom du test>"` : cree une tache de correction ciblee (apres E06)

## Phases
E00 workspace/sources/CI - D_01..D_18 notes par sujet (Pan Docs) - A_01..A_07 decisions - E01 core/bus/cartouche - E02 CPU SM83 + timer + blargg -
E03 PPU - E04 APU - E05 joypad/MBC - E06 suite de tests + baseline - E07 eframe/egui + cpal + gilrs - E08 debug - E09 save states.

## Tableau de bord temps reel
`python3 docs/tools/dashboard.py` puis http://127.0.0.1:8765 (rafraichi toutes les 2 s, lecture seule, local uniquement).
Tailwind est charge via CDN : internet requis a l'ouverture de la page.

## Points a verifier chez toi
- Options Hermes utilisees (verifiees dans la doc CLI) : `-p`, `chat --oneshot -Q`, `-s`, `--max-turns`, `--query-file`. Si ta version differe, adapte `drive.sh`.
- Noms exacts des ROMs de test : D_17 les releve ; les checks les cherchent par prefixe (`01-`, `02-`...).
- D_04 pose une question si Pan Docs n'a pas de table d'opcodes avec cycles.
