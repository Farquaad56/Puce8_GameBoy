# Regles_Emulateur.md

Guide de référence et règles pour un agent LLM qui développe un émulateur de console ou de borne d'arcade (NES, SNES, Game Boy, Galaxian, Galaga, Pac-Man, etc.) en **Rust** avec **eframe/egui**.

Ce document est normatif. Les mots **DOIT**, **NE DOIT PAS**, **DEVRAIT** ont leur sens habituel. En cas de doute, appliquer la règle la plus stricte et poser la question à l'utilisateur.

---

## Table des matières

1. Comment utiliser ce document
2. Règles générales de l'agent
3. Baseline technique (non négociable)
4. Architecture du workspace
5. Modèle d'émulation cycle par cycle
6. Particularités par machine
7. Frontend eframe/egui
8. Plan d'étapes standard
9. Gabarit de sous-tâche
10. Informations techniques à récupérer
11. Classification et format des notes
12. Pièges à éviter
13. Stratégie de tests
14. Protocole de validation (GATE)
15. Checklists
16. Quand l'agent est bloqué

---

## 1. Comment utiliser ce document

- Lire les sections 2 et 3 au début de **chaque** session.
- Lire uniquement les autres sections utiles à la sous-tâche en cours (section 12 pour les pièges du domaine concerné, section 10 pour savoir quelles informations chercher).
- Une session de travail = **une seule sous-tâche**.
- Ce document définit **comment** travailler. Les faits spécifiques à la machine (opcodes, timings, registres) viennent des notes extraites de la documentation de la machine, jamais de la mémoire de l'agent.

---

## 2. Règles générales de l'agent

### 2.1 Périmètre d'écriture
1. Écrire uniquement dans les fichiers et dossiers nommés par la sous-tâche en cours.
2. Ne jamais modifier les sources de documentation fournies par l'utilisateur.
3. Ne jamais écrire de secrets (tokens, clés, mots de passe) dans un fichier, un log ou un commit.
4. Ne jamais ajouter de ROM, BIOS ou image de jeu au dépôt. Le dossier `roms/` est dans `.gitignore`.

### 2.2 Budget de contexte
1. Ne jamais lire en entier un fichier de plus de 150 lignes : utiliser `grep`, `head`, `sed -n 'a,bp'`.
2. Tronquer les sorties de commandes (`| head -40`).
3. Lire au maximum 3 fichiers de notes par sous-tâche, avec plages de lignes.
4. Les fichiers marqués « copier sans lire » (tables d'opcodes générées, palettes) sont copiés tels quels, pas ouverts.

### 2.3 Zéro invention
1. Si une information n'est pas dans les sources, écrire `UNKNOWN - to confirm` et l'ajouter à `open_questions.md`.
2. Ne jamais combler un trou avec des souvenirs approximatifs d'un autre émulateur.
3. En cas de conflit entre deux sources, **ne pas choisir en silence** : documenter le conflit et le trancher par une ROM de test.
4. Toute valeur numérique (cycles, adresses, fréquences) doit avoir une référence de source.

### 2.4 Langue
1. Code, identifiants, commentaires, messages de commit : **anglais, ASCII uniquement** (pas d'accents, pour que la recherche de texte reste fiable).
2. Documentation et échanges avec l'utilisateur : **français**.

### 2.5 Droit d'auteur
1. Les notes sont des **paraphrases** avec référence (`fichier#section` ou `url + date`).
2. Citer au maximum quelques mots d'une source.
3. Ne jamais reproduire de contenu long d'une documentation ou d'un code tiers.

### 2.6 Git
1. Chaque sous-tâche se termine par : build, tests, commit, push sur la branche courante.
2. Message de commit : `Exxy: <titre de la sous-tâche>`.
3. **Jamais de force push.**
4. Un commit = un état vert (fmt, clippy, tests passent).

### 2.7 Interaction avec l'utilisateur
1. Poser **une seule question à la fois**, seulement si la réponse n'est pas déductible.
2. Après un GATE (section 14), **s'arrêter et attendre**. Le silence n'est pas une approbation.
3. Seuls `ok`, `validate`, `go` ou un équivalent explicite autorisent la suite.

---

## 3. Baseline technique (non négociable)

| Élément | Choix |
|---|---|
| Langage | Rust (édition stable la plus récente) |
| Précision | **Cycle-accurate** |
| Crate core | `<name>-core` : aucune dépendance externe, pas d'I/O, pas de threads, déterministe |
| Crate CLI | `<name>-cli` : exécution headless (test ROMs, logs, hash d'écran) |
| Crate desktop | `<name>-desktop` : `eframe`/`egui` (backend `wgpu`), `gilrs` (manettes), `cpal` (audio) |
| Lints | `cargo fmt`, `cargo clippy -- -D warnings` |
| Sûreté | `#![forbid(unsafe_code)]` dans le core |

Règles Rust pour le core :
1. Arithmétique matérielle : utiliser `wrapping_add`, `wrapping_sub`, `overflowing_*` explicitement. Jamais d'opérateurs `+`/`-` qui paniqueraient en debug et wrapperaient en release.
2. Aucune source de non-déterminisme : pas d'horloge système, pas de `HashMap` itéré, pas de hasard non seedé.
3. Pas de `panic!` sur entrée externe : une ROM invalide renvoie un `Result<_, LoadError>`.
4. Pas de `Rc<RefCell<...>>` pour le bus (voir section 5).
5. Pas d'allocation dans `tick()` (pas de `Vec::push`, `String`, `Box` dans le chemin chaud).
6. Les tables (opcodes, palettes) sont des `const`/`static` générés et vérifiés, pas reconstruits à l'exécution.

---

## 4. Architecture du workspace

```
<name>/
  Cargo.toml                   # [workspace]
  rust-toolchain.toml
  .gitignore                   # roms/, target/
  crates/
    <name>-core/
      src/
        lib.rs
        machine.rs             # Machine : possede tout, expose tick()
        bus.rs                 # lecture/ecriture, open bus, DMA
        cpu/
          mod.rs
          micro_ops.rs
          instructions/        # un fichier par groupe d'instructions
          interrupts.rs
        video/                 # ppu.rs ou video.rs, framebuffer
        audio/                 # apu.rs ou wsg.rs
        input.rs               # etat des boutons, sans gilrs
        media/
          cartridge.rs         # consoles
          mappers/             # un fichier par mapper
          romset.rs            # arcade : fichiers ROM, offsets, CRC
        savestate.rs
    <name>-cli/
    <name>-desktop/
  roms/                        # hors git
  docs/
    annexes/
      notes/                   # un fichier par sujet (section 11)
      decisions.md
      open_questions.md
      sources_inventory.md
  PROGRESS.md
```

Règle d'or : **le core ne connaît ni egui, ni cpal, ni gilrs, ni le système de fichiers.** Il reçoit des octets et produit un framebuffer et des échantillons.

Ne pas chercher un trait `Cpu` universel pour 6502, 65816, SM83 et Z80 : un core par machine, avec seulement les utilitaires vraiment communs partagés.

---

## 5. Modèle d'émulation cycle par cycle

### 5.1 Interface de la machine

```rust
pub trait Machine {
    fn reset(&mut self);
    fn tick(&mut self);                    // avance d'UN cycle maitre
    fn run_frame(&mut self);               // tick() jusqu'a frame complete
    fn framebuffer(&self) -> &[u32];       // RGBA
    fn drain_audio(&mut self, out: &mut Vec<f32>);
    fn set_input(&mut self, port: usize, state: u32);
    fn save_state(&self) -> Vec<u8>;
    fn load_state(&mut self, data: &[u8]) -> Result<(), StateError>;
}
```

### 5.2 Règles de timing
1. `Machine::tick()` avance **exactement un cycle** de l'horloge de référence de la machine (maître ou CPU, selon la décision d'architecture approuvée).
2. `Cpu::tick()` exécute **une micro-op = un accès bus** (lecture, écriture ou cycle interne).
3. Les autres puces sont avancées depuis `Machine::tick()` à leur **ratio entier exact** (ex. NES : PPU x3 pour 1 cycle CPU). Jamais de flottants pour les ratios.
4. L'ordre d'exécution des puces à l'intérieur d'un tick est **défini, documenté dans `decisions.md`, et stable**.
5. Une instruction est décrite comme une liste de micro-ops, pas comme une fonction monolithique.

### 5.3 Propriété du bus
1. La `Machine` possède le bus et toutes les puces.
2. Le CPU reçoit `&mut Bus` à chaque tick.
3. Interdit : `Rc<RefCell<Bus>>`, `Arc<Mutex<Bus>>`, références croisées entre puces.

### 5.4 Comportements à modéliser explicitement
- DMA qui vole des cycles au CPU
- Wait states et contention
- Open bus (dernière valeur présente sur le bus)
- Lectures à effet de bord (registre de statut qui efface un flag)
- Dummy reads et dummy writes

---

## 6. Particularités par machine

| Machine | CPU | Points d'attention |
|---|---|---|
| NES | 6502 (2A03) | PPU 3:1, APU, mappers (NROM, MMC1, MMC3...), OAM DMA qui vole des cycles, opcodes illégaux, pas de mode décimal |
| SNES | 65816 | PPU1/PPU2, SPC700 + DSP audio séparés, DMA/HDMA, modes d'adressage larges, coprocesseurs (SuperFX...) |
| Game Boy | SM83 | PPU en modes 0-3, timers/DIV, MBC1/3/5, interruptions, HALT bug, ROM de boot |
| Galaxian | Z80 | Pas de cartouche : jeu d'EPROM + câblage carte ; tilemap, sprites, starfield ; sons discrets |
| Galaga | 3x Z80 | RAM partagée, puces Namco custom (06xx, 51xx, 54xx), starfield, WSG |
| Pac-Man | Z80 | Z80 cadencé depuis l'horloge maître, WSG 3 voix, vecteur d'interruption posé via un port OUT |

Conséquences de design :
- **Consoles** : `media/cartridge.rs` + `media/mappers/`.
- **Arcade** : `media/romset.rs` (fichiers, offsets, tailles, CRC, parent/clone). Le `Machine` est une carte spécifique. Plusieurs jeux du même matériel = plusieurs romsets, pas des forks.
- **Arcade** : exposer coin, start, service, test et **DIP switches** dans l'interface, gérer le watchdog et l'orientation de l'écran (rotation).
- Vérifier les checksums au chargement et signaler un romset incorrect.

---

## 7. Frontend eframe/egui

### 7.1 Boucle et cadence
1. L'émulation tourne dans un thread dédié (ou, à défaut, `run_frame()` depuis `update()`).
2. La cadence est pilotée par l'**audio** (le callback `cpal` consomme un ring buffer), à défaut par un accumulateur de temps. Jamais par le simple refresh de l'écran.
3. Appeler `ctx.request_repaint()` pour que l'affichage se mette à jour en continu.

### 7.2 Affichage
1. `egui::ColorImage` -> `TextureHandle::set()` sur **la même texture** à chaque frame (ne pas la recréer).
2. Filtre `Nearest`, mise à l'échelle **entière** par défaut.
3. Gérer ratio de pixels et rotation (arcade en écran vertical).

### 7.3 Audio
1. Ring buffer **lock-free** (`ringbuf`, `rtrb`) entre émulateur et callback `cpal`.
2. Aucun `Mutex` dans le callback audio.
3. Resampling avec filtre passe-bas, pas de décimation naïve.
4. Stratégie explicite pour l'underrun (silence) et l'overflow (rattrapage).

### 7.4 Entrées
1. Clavier via egui uniquement quand aucun widget texte n'a le focus.
2. Manettes via `gilrs`, avec branchement/débranchement à chaud.
3. Conversion vers l'état abstrait du core via `set_input()`.

### 7.5 Interface minimale
Menu fichier, pause/reprise, reset, pas-à-pas d'une frame, FPS, volume, plein écran, configuration des touches. Ensuite : viewers (tuiles, palettes, sprites), éditeur mémoire, trace CPU.

---

## 8. Plan d'étapes standard

Chaque étape finit par quelque chose de **lançable et testable**. Adapter à la machine (supprimer ou ajouter selon la documentation).

| Étape | Contenu | Test de sortie typique |
|---|---|---|
| E00 | Workspace, CI, règles, `.gitignore` | `cargo build && cargo test` verts |
| E01 | Squelette core, bus, chargement média/romset | Une ROM valide se charge, une invalide renvoie une erreur |
| E02 | CPU : micro-ops, puis groupes d'instructions, puis interruptions | Test ROM CPU passe, trace identique à la référence |
| E03 | Vidéo | Hash de framebuffer conforme après N frames |
| E04 | Audio | Échantillons conformes (hash ou comparaison) |
| E05 | Entrées + variantes/mappers | Jeu de test jouable en headless avec entrées scriptées |
| E06 | Harnais CLI + runner de test ROMs | Suite de test ROMs automatisée |
| E07 | Frontend eframe/egui | Jeu jouable avec image, son, entrées |
| E08 | Outils de debug | Viewers et trace fonctionnels |
| E09 | Save states + finitions | Sauvegarde/chargement reproductible bit à bit |
| Eopt | Étapes optionnelles | Selon besoin |

Règle d'ordre : ne jamais attaquer une étape dont les notes de documentation sont « partielles » ou « manquantes » pour un sujet bloquant (timing, CPU, carte mémoire).

---

## 9. Gabarit de sous-tâche

Chaque sous-tâche est un fichier de **moins de 70 lignes**, autosuffisant, réalisable en une session. Si elle est plus lourde, la découper et renuméroter.

```
# Exxy - <titre>
Goal: <une phrase>
Prerequisites: <ids de sous-taches DONE>
Read (max 3 fichiers, avec plages de lignes): <notes>
Copy without reading: <fichiers generes>   (si besoin)
Write only in: <chemins de crates/modules>
## Work
1. <action concrete, nommer les types/fonctions a creer>
2. ...
## Rules
- Identifiants et commentaires en anglais, ASCII uniquement.
- Cycle accuracy: <contrainte de timing applicable ici>
## Exit tests
- <commande> -> <resultat attendu>
- Non-regression: rejouer tous les tests de sortie precedents.
## CLOSURE
cargo fmt && cargo clippy -- -D warnings && cargo test
Mettre a jour PROGRESS.md, commit "Exxy: <titre>", push (sans force).
```

---

## 10. Informations techniques à récupérer

Pour chaque sujet : lister ce qu'il faut chercher dans la documentation. Un champ absent des sources devient `UNKNOWN - to confirm`.

### 10.1 Timing
- Fréquence de l'horloge maître et diviseurs vers chaque puce (CPU, vidéo, audio)
- Durée d'une frame : cycles, lignes, pixels par ligne (visibles, blanking, sync)
- Fréquence de rafraîchissement, par région (NTSC/PAL pour consoles)
- Ordre des événements à l'intérieur d'un cycle
- Cycles volés par DMA, wait states, contention mémoire

### 10.2 CPU
- Jeu d'instructions complet : opcode, mnémonique, mode d'adressage, taille, **cycles par accès bus** (pas seulement le total)
- Flags : effet exact de chaque instruction, y compris comportements non documentés
- Registres, valeurs au reset, vecteur de démarrage
- Interruptions : types, vecteurs, masquage, latence, priorité, moment d'acquittement
- Opcodes illégaux ou non documentés
- Cas limites : HALT, STOP, instructions qui retardent la prise en compte d'une interruption

### 10.3 Carte mémoire
- Plan d'adressage : plage, taille, type (ROM, RAM, registre, miroir, non mappé)
- Miroirs et leur règle exacte
- Registres mappés : adresse, nom, bits détaillés, lecture/écriture, effets de bord
- Comportement open bus
- Bus séparés ou partagés (RAM partagée en multi-CPU)

### 10.4 Vidéo
- Résolution, orientation de l'écran, ratio de pixels
- Modèle de rendu : tilemaps, sprites (taille, nombre max, nombre par ligne), priorités, palettes
- Format des données graphiques (tuiles, bitplanes, encodage des couleurs)
- Table de couleurs ou conversion PROM vers RGB
- Registres vidéo et moment où leurs changements prennent effet (y compris en milieu de frame)
- Flags et interruptions vidéo (VBlank, HBlank, collision de sprite...)
- Effets matériels spéciaux : starfield, scrolling, inversion écran

### 10.5 Audio
- Nombre de voix, type (carré, triangle, bruit, wavetable, PCM)
- Registres, enveloppes, formules de conversion en fréquence
- Fréquence native de génération, mixage, filtres analogiques
- Extensions audio apportées par les mappers ou cartes

### 10.6 Entrées
- Joypads et protocole de lecture (strobe, latch, registres)
- Arcade : coin, start, service, test, DIP switches (signification de chaque bit)
- Polarité des signaux (actif bas ou haut)

### 10.7 Média et mappers
- Format de fichier (en-tête iNES, cartouche GB/SNES, romset MAME)
- Mappers/MBC : registres de banking, plages commutables, RAM sauvegardée
- Arcade : liste des fichiers ROM, offsets, tailles, CRC, parent/clone, PROMs
- Détection automatique de la variante

### 10.8 Boot et reset
- État initial de chaque registre et de la RAM (connu ou indéfini)
- Séquence de démarrage, ROM de boot (Game Boy, SNES)
- Watchdog (arcade), différence power-on / reset

### 10.9 Test ROMs et références
- Test ROMs applicables, ce qu'elles valident, critère de réussite
- Traces de référence (logs CPU d'un émulateur de référence)
- Hash d'écran attendus

---

## 11. Classification et format des notes

### 11.1 Principe
Classer par **sujet matériel** (ce que le code va implémenter), pas par source. Une même source alimente plusieurs notes ; chaque note alimente un module du core.

```
annexes/
  sources_inventory.md       # toutes les sources : type, taille, sujets, fiabilite
  notes/
    01_timing.md             -> machine.rs (scheduler)
    02_cpu.md                -> cpu/
    03_memory_map.md         -> bus.rs
    04_video.md              -> video/
    05_audio.md              -> audio/
    06_input.md              -> input.rs
    07_media.md              -> media/
    08_boot_reset.md         -> machine.rs (reset)
    09_test_roms.md          -> cli + tests
  decisions.md               # choix d'architecture approuves
  open_questions.md          # tout ce qui est UNKNOWN
  code/                      # fichiers generes et verifies, "copier sans lire"
  sources_cache/             # snapshots d'URL (url + date en en-tete)
```

L'ordre ci-dessus est l'ordre de priorité : le timing conditionne tout le reste.

### 11.2 Format d'un fait

```
## <Sujet precis>
Fait : <enonce court, chiffre>
Source : <fichier#section ou url + date>
Fiabilite : officielle | communautaire | testee sur ROM | deduite
Impact code : <module et fonction concernes>
Statut : CONFIRME | CONFLIT | UNKNOWN - to confirm
```

Exemple de conflit :

```
Conflit : source A indique 7 cycles, source B indique 6.
Decision : en attente -> tester avec <ROM de test>
```

### 11.3 Hiérarchie de fiabilité
1. Résultat d'une test ROM
2. Documentation officielle du constructeur
3. Documentation communautaire reconnue
4. Code d'un autre émulateur
5. Mémoire ou déduction (à éviter, à marquer comme telle)

### 11.4 Règles de tri
1. Une information = un seul emplacement ; utiliser des renvois, pas des doublons.
2. Séparer **faits** (notes) et **choix** (`decisions.md`).
3. Notes courtes (150 lignes maximum) ; si une note grossit, la découper par sous-sujet.
4. Paraphraser, garder la référence précise.
5. Tout manque va dans `open_questions.md`.

### 11.5 Matrice de couverture (à maintenir)

| Sujet | Couvert | Partiel | Manquant | Sources |
|---|---|---|---|---|
| Timing | | | | |
| CPU | | | | |
| Carte mémoire | | | | |
| Vidéo | | | | |
| Audio | | | | |
| Entrées | | | | |
| Média/mappers | | | | |
| Boot/reset | | | | |
| Test ROMs | | | | |

Tant qu'un sujet bloquant est « partiel » ou « manquant », ne pas coder ce qui en dépend.

### 11.6 Données tabulaires
Les tables machine-lisibles (opcodes, palettes, catalogue de ROMs) sont transformées en fichiers de code générés, **vérifiés contre une référence** (trace, checksum, test ROM), puis marquées avec la vérification effectuée. Les sous-tâches les référencent en « copier sans lire ».

---

## 12. Pièges à éviter

### 12.1 Architecture
- **Mélanger core et frontend** : dès qu'egui, cpal ou `std::fs` entre dans le core, les tests headless et le déterminisme sont perdus.
- **`Rc<RefCell<>>` pour le bus** : panics au runtime et code illisible. La `Machine` possède le bus.
- **Émuler par instruction** quand on vise le cycle-accurate : ajouter le timing après coup équivaut à une réécriture. Partir des micro-ops.
- **Horloges approximatives** : ratios entiers exacts, jamais de `f64`.
- **Généraliser trop tôt** : un trait CPU universel pour des architectures différentes gêne tout le monde.

### 12.2 CPU
- **Flags** : comportements non documentés (Z80 X/Y et `DAA`, 6502 flag B et `BRK`, SM83 et `DAA`). Valider avec des ROMs dédiées, pas avec sa lecture de la documentation.
- **Opcodes illégaux** dont certains jeux dépendent (NES).
- **Interruptions** : timing d'acquittement, instruction suivante exécutée avant prise en compte (`EI` Z80/SM83), NMI manquée ou doublée.
- **Overflow arithmétique Rust** : panique en debug, wrap en release. Utiliser `wrapping_*` partout où le matériel wrappe.
- **Page crossing** (6502) et dummy reads : comptent dans le timing et peuvent avoir des effets de bord sur les registres.

### 12.3 Bus et mémoire
- **Open bus** : une adresse non mappée ne renvoie pas forcément 0.
- **Mirroring** incorrect (RAM, VRAM, registres).
- **Lectures à effet de bord** : le debugger doit utiliser un mode « peek » sans effet.
- **DMA instantané** : il vole des cycles, le modéliser explicitement.

### 12.4 Vidéo et audio
- **Rendre la frame d'un coup** : beaucoup de jeux modifient scroll, palette ou sprites en milieu de frame. Suivre le faisceau (scanline ou pixel).
- **Ordre des événements dans un cycle** (CPU avant ou après PPU) : source de bugs d'un cycle très difficiles à localiser.
- **Resampling naïf** : aliasing ; utiliser un filtre passe-bas.
- **Buffer audio** : underrun = craquements, overflow = latence.
- **Couleurs, palette, gamma, ratio de pixels, rotation de l'écran** (arcade vertical : Galaxian, Galaga, Pac-Man).

### 12.5 Frontend eframe/egui
- Émulation dans `update()` sans contrôle du temps : la vitesse dépend du refresh écran.
- Oubli de `ctx.request_repaint()` : image figée.
- Texture en filtre linéaire : pixels flous ; utiliser `Nearest` et échelle entière.
- Recréation de la texture à chaque frame au lieu d'un `set()`.
- `Mutex` dans le callback audio : glitches ; utiliser un ring buffer lock-free.
- Clavier lu alors qu'un champ texte a le focus ; manettes non gérées à chaud.

### 12.6 Arcade
- **Romsets MAME** : noms de fichiers, CRC, ordre de chargement, versions parent/clone ; vérifier les checksums.
- **DIP switches, coin, service** à exposer, sinon le jeu reste en mode test ou sans crédit.
- **Multi-CPU (Galaga)** : ordre de tick entre CPU défini et stable, RAM partagée correctement arbitrée.
- **Puces custom Namco** : documentation incomplète, marquer `UNKNOWN - to confirm` plutôt qu'inventer.
- **Watchdog et reset** : ne pas l'oublier, le jeu redémarre en boucle sinon.

### 12.7 Tests et méthode
- **Pas de test ROM tôt** : les bugs CPU se découvrent par des jeux qui plantent, très difficiles à localiser.
- **Tester uniquement avec des jeux commerciaux** : ils passent avec des bugs que les test ROMs détectent.
- **Hacks spécifiques à un jeu** : corriger la cause matérielle.
- **Pas de harnais headless** : le hash de framebuffer après N frames est le meilleur filet de non-régression.
- **Non-déterminisme** : casse les save states et les tests reproductibles.

### 12.8 Process
- Sous-tâches trop grosses : une session = un objectif testable.
- Lire toute la documentation avant de coder : extraire des notes par sujet, avancer par étape.
- Ne pas commit à chaque état vert : perte du point de retour lors d'un refactor.
- Corriger un bug en modifiant plusieurs modules à la fois : isoler d'abord la cause avec un test.

---

## 13. Stratégie de tests

### 13.1 Niveaux
1. **Unitaires** : une instruction, un registre, un mapper.
2. **Test ROMs** : résultat lu en mémoire ou à l'écran, automatisé en headless.
3. **Comparaison de trace** : log CPU contre une référence, ligne à ligne.
4. **Hash de framebuffer** : après N frames d'une ROM donnée, comparé à une valeur de référence.
5. **Non-régression** : toute sous-tâche rejoue tous les tests de sortie précédents.

### 13.2 Test ROMs par machine (à confirmer dans les sources)
- **NES / 6502** : nestest (trace), ROMs blargg.
- **Z80 (arcade)** : zexdoc / zexall.
- **Game Boy** : blargg (cpu_instrs, instr_timing), suites mooneye.
- **SNES** : ROMs de test CPU/PPU/SPC de la communauté.

### 13.3 Règles
1. Poser le harnais de test ROM dès l'étape CPU (E02), pas à la fin.
2. Chaque test a un critère de réussite explicite (code de sortie, hash, trace identique).
3. Un test qui échoue n'est jamais désactivé pour « avancer » : il est documenté dans `open_questions.md` avec sa cause probable.
4. Les résultats déterministes sont une exigence : deux exécutions identiques donnent le même hash.

---

## 14. Protocole de validation (GATE)

Chaque phase de travail se termine par un bloc GATE, puis l'agent **s'arrête**.

```
=== GATE <phase> ===
Done: <2 a 4 lignes>
Written: <liste des fichiers>
Checks: [x] passe / [ ] echec  (liste)
Open questions: <numerotees, ou "none">
Reply: "ok" pour continuer | ou indiquer quoi changer.
=== STOP ===
```

Règles :
1. Corrections demandées par l'utilisateur : les appliquer, réimprimer le GATE, attendre de nouveau.
2. Une décision d'architecture n'est valide qu'une fois approuvée à un GATE et inscrite dans `decisions.md`.
3. L'état d'avancement est tenu dans `PROGRESS.md` (ligne « Current sub-task » + tableau de statut) ; en reprise de session, le lire en premier.

### Décisions d'architecture à faire approuver (une à la fois)
Pour chacune : options, recommandation, conséquence.
- Horloge maître, ratios entre puces, ordonnanceur de ticks
- Conception du bus (propriété, open bus, DMA)
- Modèle de micro-ops du CPU
- Buffers vidéo/audio, cadence, frontière core/frontend
- Save states et règles de déterminisme
- Variantes supportées (régions, médias, mappers) et première cible
- Stratégie de test (ROMs, logs, critères de réussite)

---

## 15. Checklists

### 15.1 Avant de commencer une sous-tâche
- [ ] `PROGRESS.md` lu, sous-tâche courante identifiée
- [ ] Prérequis tous à l'état DONE
- [ ] Au plus 3 notes lues, avec plages de lignes
- [ ] Périmètre d'écriture clair
- [ ] Aucune information manquante masquée (sinon `UNKNOWN - to confirm`)

### 15.2 Avant de déclarer une sous-tâche terminée
- [ ] Tests de sortie de la sous-tâche passent
- [ ] Tous les tests de sortie précédents passent (non-régression)
- [ ] `cargo fmt` propre
- [ ] `cargo clippy -- -D warnings` sans avertissement
- [ ] `cargo test` vert
- [ ] Code et commentaires en anglais ASCII
- [ ] Aucun `unsafe`, aucun `panic!` sur entrée externe, aucune allocation dans `tick()`
- [ ] Aucun secret ni ROM dans le dépôt
- [ ] `PROGRESS.md` mis à jour
- [ ] Commit `Exxy: <titre>` puis push, sans force

### 15.3 Revue de sujet « cycle-accurate »
- [ ] Chaque micro-op fait exactement un accès bus
- [ ] Ratios d'horloge entiers et documentés
- [ ] Ordre des puces dans un tick défini et stable
- [ ] DMA, wait states et dummy reads modélisés
- [ ] Interruptions : moment d'échantillonnage et d'acquittement conformes aux sources

### 15.4 Vérification de la documentation d'une sous-tâche
- [ ] Moins de 70 lignes
- [ ] Sections `## Exit tests` et `## CLOSURE` présentes
- [ ] Références de source présentes pour chaque valeur numérique
- [ ] Les liens vers les notes existent

Script de contrôle (à lancer à la racine de la documentation) :

```bash
# sous-taches de plus de 69 lignes
find tasks -name '*.md' -exec awk 'END{if(NR>69)print FILENAME": "NR" lines"}' {} \;
# caracteres non ASCII dans le code genere
grep -rnP '[^\x00-\x7F]' annexes/code 2>/dev/null | head
# CLOSURE et Exit tests presents
for f in tasks/*/*.md; do grep -q '^## CLOSURE' "$f" || echo "no CLOSURE: $f"; \
  grep -q '^## Exit tests' "$f" || echo "no exit tests: $f"; done
# secrets
grep -rnEi 'ghp_|password|token *[:=]' . | head
```

---

## 16. Quand l'agent est bloqué

1. **Information absente** : écrire `UNKNOWN - to confirm`, l'ajouter à `open_questions.md`, continuer sur ce qui ne dépend pas de cette information, ou poser **une** question à l'utilisateur.
2. **Sources en conflit** : documenter les deux valeurs avec références, proposer un test discriminant (ROM de test), attendre le résultat.
3. **Test qui échoue** : isoler la cause avec le test le plus petit possible (une instruction, un registre, un cycle) avant de modifier le code. Ne jamais désactiver le test.
4. **Sous-tâche trop lourde** : la découper, renuméroter, mettre à jour l'index de l'étape et `PROGRESS.md`, demander validation.
5. **Tentation de hack** : si une correction est spécifique à un jeu, s'arrêter, rechercher la cause matérielle dans les notes, sinon l'inscrire comme question ouverte.
6. **Hors périmètre** : ne pas modifier des fichiers non listés par la sous-tâche ; signaler le besoin à l'utilisateur.

---

*Fin du document. En cas de contradiction entre ce guide et une demande explicite de l'utilisateur, demander confirmation avant d'agir.*
