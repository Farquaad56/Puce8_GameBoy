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

Statut : APPROUVE

## A_02

Decision: bus design - ownership of memory regions, OAM DMA arbitration, PPU-mode access blocking, open bus / unmapped reads, and side-effect-free peek().
Questions a trancher : (1) Qui possede quoi (VRAM, OAM, IF/IE) et comment le CPU recoit-il &mut Bus. (2) Comment l'OAM DMA vole ou bloque les acces CPU. (3) Comment le blocage d'acces par mode PPU renvoie des valeurs. (4) Lectures non mappees / inutilisables. (5) peek() sans effets de bord pour les debogueurs.

Options :

Option A - Bus central qui possede toute la memoire adressable ; les puces ne portent que leur etat de controle.
Le Bus possede tous les tableaux du bus d'adresse a 16 bits (note 03a_memory_map.md, section "Adresse bus") : ROM bank 0 + banks commutables, VRAM 8000-9FFF, WRAM C000-DFFF + echo RAM E000-FDFF, OAM FE00-FE9F, HRAM FF80-FFFE, et le fichier de registres I/O FF00-FFFF (y compris IF $FF0F et IE $FFFF). Les puces PPU, APU, serial et DMA ne possedent que leur propre etat (mode PPU + position dot, canaux audio, compteur serial, cycles restants du DMA) ; pendant leur slot de tick (ordre fixe CPU -> Timer -> DMA -> PPU -> APU -> Serial, decision A_01), elles empruntent des tranches &mut du Bus. Le CPU recoit un &mut Bus par micro-op et effectue exactement un acces bus (bus.read / bus.write) par micro-op, comme exige par AGENTS.md ("one micro-op = one bus access"). Les effets de bord inter-puces (ecrire $FF46 demarre le DMA ; ecrire NR14 declenche un canal audio ; ecrire $FF41 affecte STAT - note 03b_io_registers.md, section "Effets d'ecriture") passent par un point unique : la dispatch io_write du Bus.
Pros : un seul proprietaire de memoire = pas de Rc<RefCell>/Arc<Mutex> (contrainte AGENTS.md), le borrow checker reste propre car les emprunts &mut sont sequencielles dans l'ordre fixe A_01 ; chaque micro-op CPU = exactement un acces bus ; tous les effets de bord I/O convergent vers une seule dispatch, facile a verifier et deterministe ; le blocage d'acces par mode PPU et par DMA devient une simple requete pure sur l'etat des puces (ppu.vram_accessible(), dma.active) que le Bus consulte avant de toucher aux tableaux.
Cons : la structure Bus est grande et centrale, elle porte toutes les regions memoire ; il faut passer des tranches &mut a chaque puce a chaque tick (un peu de plomberie) ; le fichier I/O avec ses effets de bord vit dans le Bus plutot que dans un module separe.

Option B - Possession par puce : chaque puce possede sa propre memoire, le Bus est un routeur mince.
La PPU possede VRAM + OAM, l'APU possede la Wave RAM FF30-FF3F et son etat audio, un module I/O possede FF00-FFFF, et un proprietaire separe porte ROM/WRAM. Le Bus ne fait que router les adresses vers le proprietaire de chaque plage (note 03a_memory_map.md) en conservant des references &mut vers la memoire des puces soeurs.
Pros : separation conceptuelle nette, chaque puce encapsule sa propre memoire ; le routage par plage d'adresse est direct.
Cons : le Bus doit conserver des references &mut vers la memoire de structures soeurs (PPU, APU), ce qui est difficile en Rust sans unsafe ou acrobaties de duree de vie - cela viole "no Rc<RefCell>/Arc<Mutex> for the bus" ; pour que le CPU lise la VRAM via bus.read(0x8000), le Bus doit atteindre dans le tableau de la PPU, ce qui est maladroit ; le DMA ecrivant dans OAM a besoin d'un chemin depuis la logique DMA vers l'OAM possede par la PPU.

Option C - Le Bus possede les tableaux memoire mais pas le fichier I/O : un module Io separe possede FF00-FFFF et ses hooks d'effets de bord.
Comme A pour la memoire (ROM, VRAM, WRAM+echo, OAM, HRAM), mais le fichier de registres I/O FF00-FFFF est possede par un composant Io distinct ; le Bus dellege toute adresse FF00-FFFF a io.read / io.write. IF ($FF0F) et IE ($FFFF) vivent dans ce module Io ; chaque puce demande une interruption via io.request_interrupt(bit).
Pros : le fichier I/O avec ses nombreux effets de bord (joypad, serial, timer, audio, LCDC/STAT, DMA - note 03b_io_registers.md) est isole dans un seul module testable ; le Bus reste plus petit.
Cons : deux points d'entree pour les acces memoire (Bus + Io), ce qui multiplie les chemins a verifier ; la dispatch des effets de bord inter-puces passe par l'Io plutot que par le Bus, ce qui eparpille un peu la logique ; IF/IE sont alors possedes par l'Io et non par le Bus.

Recommandation :
Option A - Bus central qui possede toute la memoire adressable (y compris le fichier I/O FF00-FFFF avec IF et IE), les puces ne portant que leur etat de controle et empruntant des tranches &mut dans l'ordre fixe de tick A_01. C'est la seule option qui satisfait a la fois "no Rc<RefCell>/Arc<Mutex> for the bus", "one micro-op = one bus access" et un point unique pour les effets de bord I/O, tout en gardant le blocage d'acces (mode PPU, DMA) comme des requetes pures sur l'etat des puces.

Consequence :
- Ownership : le Bus possede ROM banks, VRAM 8000-9FFF, WRAM C000-DFFF + echo RAM E000-FDFF (miroir du bank WRAM en cours - note 03a_memory_map.md), OAM FE00-FE9F, HRAM FF80-FFFE, et le fichier I/O FF00-FFFF. IF ($FF0F) est un octet porte par le Bus, pose/efface par les puces via une methode request_interrupt(bit) et lu/efface par le CPU ; IE ($FFFF) est un registre ordinaire du fichier I/O (note 03b_io_registers.md). Le PPU, l'APU, le serial et le DMA ne possedent que leur etat de controle.
- CPU &mut Bus : Machine::tick() appelle d'abord cpu.tick(&mut bus) ; le CPU consomme une micro-op toutes les 4 dots (decision A_01) et effectue exactement un acces bus par micro-op via bus.read(addr) / bus.write(addr, val). L'emprunt &mut est relache avant que la puce suivante ne s'execute dans l'ordre fixe, donc pas de conflit d'emprunt. Le point precis du dot ou l'acces a lieu sera fixe par les taches CPU suivantes (comme A_01).
- OAM DMA : le DMA porte un etat (active + cycles restants = 160 M-cycles = 640 dots a vitesse normale - note 03a_memory_map.md). Quand dma.active, le Bus gate les acces CPU : toute adresse hors HRAM FF80-FFFF renvoie une valeur non fiable (fixe a 0xFF pour la determinisme) et les ecritures sont ignorees ; seules HRAM + IE fonctionnent normalement (note 03a_memory_map.md, section "HRAM ... seul acces CPU pendant un OAM DMA"). La duree exacte du DMA est en CONFLIT entre sources (160 M-cycles vs specs officielles) et sera tranchee par ROM test ; le gate utilise la valeur de 160 M-cycles.
- PPU-mode blocking : le Bus consulte des requetes pures sur l'etat PPU - vram_accessible() (modes 0/1/2, avec une fenetre de quelques cycles en mode 0) et oam_accessible() (modes 0/1 seulement - note 03a_memory_map.md). Quand bloquees, bus.read renvoie 0xFF et bus.write est un no-op (donnees inchangees), comme documente pour les memoires video inaccessible au CPU (note 03a_memory_map.md, section "Lectures de zones non mappees ou bloquees").
- Unmapped / unusable reads : par defaut, une lecture d'une adresse non mappes renvoie 0xFF (open bus). Cas speciaux : la plage FEA0-FEFF renvoie 0x00 hors OAM block mais 0xFF pendant un OAM block (avec l'effet de corruption OAM) ; les registres I/O DMG entierement non attribues ($FF03, $FF08-$FF0E, etc.) et les registres CGB-only en mode non-CGB lisent $FF par defaut - la valeur exacte des adresses non attribuees est UNKNOWN - to confirm (note 03b_io_registers.md).
- peek() : une methode pure peek(addr) renvoie le contenu brut de la memoire a l'adresse, en contournant tout gate (mode PPU, DMA active) et sans aucun effet de bord ni avancement d'horloge ; elle lit directement les tableaux possedes par le Bus. C'est ce que le code debogueur / viewer utilise pour lire la memoire sans effets de bord (AGENTS.md).

Statut : PROPOSE
