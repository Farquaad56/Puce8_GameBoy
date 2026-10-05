# Decisions d'architecture

Une decision n'est valide que si elle est approuvee a un GATE (Statut : APPROUVE).
Format : voir AGENTS.md. Sections ajoutees par les taches A_01 a A_07.

## A_01

Decision: master clock, tick granularity, ratios, order of chips inside a tick.
Questions a trancher : (1) Un Machine::tick vaut-il un T-cycle (dot) ou un M-cycle ?
(2) Comment le rythme des micro-ops CPU se mappe-t-il sur les ticks ?
(3) Ratios entiers exacts pour PPU, timer, APU, serial.
(4) Ordre d'avancement de CPU, timer, PPU, APU, DMA, serial dans un tick, et pourquoi.
(5) Cycles par frame.

Options :

Option A - tick = dot (T-cycle), toutes les puces avancees a chaque dot.
Le noyau avance une fois par dot (1/4194304 s). Chaque puce porte son propre compteur
interne et n'agit que quand sa periode s'ecoule. Le CPU consomme un micro-op toutes
les 4 dots (un M-cycle) et effectue exactement un acces bus par micro-op, a un dot
fixe du M-cycle.
Pros : granularite maximale ; tous les ratios sont entiers en dots - PPU X = 1/dot,
ligne = 456 dots, frame = 70224 dots (note 01_timing.md) ; DIV = +1 / 1024 dots
(note 01b_timer.md) ; serial = 1 bit / 512 dots (refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md,
section Internal Clock : "internal clock of 8192Hz" en mode normal DMG) ; APU pulse
divider = 1/4 dots, wave = 1/2 dots (refs/pandocs/src/Audio_Registers.md, Sound-Channel-1 /
Wave). Represente exactement les evenements sub-M-cycle : transition de mode PPU
(note 01_timing.md), conflit bus DMA (note 01_timing.md), retard +1 M-cycle du overflow
timer (note 01b_timer.md), tick sur falling edge DIV/TAC (note 01b_timer.md). Deterministe,
sans allocation dans tick().
Cons : plus d'etat a porter par tick (compteurs PPU/timer/APU/serial) ; le CPU ne fait
rien sur 3 dots sur 4, il faut donc un compteur de M-cycle pour savoir quand executer la
prochaine micro-op.

Option B - tick = M-cycle (4 dots), sous-tick interne pour PPU et serial.
Le noyau avance une fois par M-cycle ; le CPU = 1 tick / micro-op (un acces bus). La PPU
et le serial portent un compteur de dot interne (0..3) pour atteindre la resolution
sub-M-cycle.
Pros : moins de ticks/frame (70224/4 = 17556) ; rythme CPU trivial (1 tick = 1 micro-op).
Cons : il faut quand meme un sous-tick de 4 dots a l'interieur du tick pour la PPU
(X position, mode boundaries - note 01_timing.md) et le serial (periode 512 dots), donc on
ne gagne pas reellement en travail ; on ajoute juste un compteur imbrique. Les evenements
qui tombent sur un dot precis (retard +1 M-cycle du overflow timer, conflit bus DMA)
deviennent plus delicats a placer.

Option C - tick = dot, avancement event-driven des puces.
Comme A, mais une puce n'est avancee que quand sa periode s'ecoule ou qu'elle a un etat en
attente ; les dots "vides" pour une puce sont sautes.
Pros : evite de toucher les compteurs d'une puce inactive sur la plupart des dots (ex. le
serial ne bouge que 1 bit / 512 dots).
Cons : plus de branches et de cas limites ; risque de manquer un point ou deux puces
interagissent sur le meme dot - conflit bus DMA + lecture OAM PPU, overflow timer + ecrire
TIMA/TMA (note 01b_timer.md), falling edge DIV/TAC (note 01b_timer.md). Moins deterministe
a verifier.

Recommandation :
Option A - tick = dot (T-cycle), avancement uniforme de toutes les puces dans un ordre fixe,
CPU = une micro-op / 4 dots (un acces bus). C'est la seule option qui represente tous les
evenements documentes a leur granularite exacte sans sous-tick imbrique ni cas limites
event-driven.

Consequence :
- Le noyau porte un compteur de dot global ; Machine::tick() est appele une fois par dot
  (70224 appels/frame).
- Ratios entiers en dots (tous CONFIRMEES) : PPU X = 1/dot, ligne = 456 dots, frame =
  70224 dots (note 01_timing.md) ; DIV = +1 / 1024 dots (note 01b_timer.md) ; TIMA period
  selon TAC[1:0] = 1024/16/64/256 dots, gate par TAC bit 2 (note 01b_timer.md) ; APU pulse
  divider = 1/4 dots, wave = 1/2 dots, noise LFSR = 262144/(divider x 2^shift) Hz, DIV-APU
  counter = 1/8192 dots (envelope sweep /65536, sound length /16384, CH1 freq sweep /32768)
  (refs/pandocs/src/Audio_Registers.md ; refs/pandocs/src/Audio_details.md, section DIV-APU) ;
  serial internal clock = 1 bit / 512 dots (refs/pandocs/src/Serial_Data_Transfer_(Link_Cable).md).
- Ordre fixe dans un tick (jamais change silencieusement) : CPU -> Timer -> DMA -> PPU -> APU
  -> Serial. CPU d'abord pour qu'une ecriture (ex. FF46 qui demarre le DMA, ou DIV/TAC qui
  declenche un falling-edge tick - note 01b_timer.md) soit visible des puces dans le meme dot ;
  Timer avant APU car le compteur DIV-APU alimente les evenements APU ; DMA avant PPU pour
  etablir l'etat de conflit bus (CPU ne voit que HRAM) avant que la PPU echantillonne OAM
  (note 01_timing.md) ; Serial en dernier, plus lent (periode 512 dots), son interrupt de fin
  etant vu par le CPU au prochain frontiere d'instruction.
- Cycles par frame : 70224 dots = 70224 T-cycles = 17556 M-cycles (note 01_timing.md).
- Le point precis du dot ou la micro-op CPU effectue son acces bus (lecture vs ecriture) sera
  fixe par les taches CPU suivantes ; A_01 ne le decide pas.

Statut : PROPOSE
