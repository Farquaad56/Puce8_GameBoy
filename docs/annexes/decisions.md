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

Statut : APPROUVE

## A_03

Decision: CPU micro-op model - representation of instructions as micro-ops with one bus access each.
Questions a trancher : (1) Representation sans allocation des micro-ops (tableau fixe ou machine a etats). (2) Decodage au moment du fetch. (3) Le fetch de l'opcode suivant chevauche-t-il le dernier M-cycle de l'instruction precedente, et quel effet sur la temporisation EI/interrupts. (4) Dispatch des interrupts, HALT et STOP comme sequences speciales. (5) Utilisation de la table d'opcodes generees.

Options :

Option A - Tableau fixe de micro-ops emis au decodage ("instruction precompilee").
Au fetch, le CPU lit l'octet opcode, cherche OpInfo dans OPCODES[]/CB_OPCODES[], puis emet un tableau de taille fixe (ex. [MicroOp; 8]) couvrant les phases d'operands + execution ; chaque M-cycle execute une entree avec exactement un acces bus.
Pros : "one micro-op = one bus access" (AGENTS.md) verifiable trivialement ; tests unitaires par instruction directs (comparer la sequence emise a la table - note 02c) ; boucle chaude simple et deterministe.
Cons : la taille du tableau doit couvrir l'instruction normale la plus longue (CALL a16 = 6 M-cycles, spot-checks note 02c) alors que la plupart n'en utilisent 1 a 3 ; les instructions de duree variable (HALT attente N*4, STOP, entree d'interrupt 5 M-cycles - notes 02a/02b) ne tiennent pas dans le tableau et doivent etre special-casees quand meme ; une instruction de branchement change PC en milieu de tableau, il faut donc arreter l'execution a ce point.

Option B - Machine a etats : "instruction en cours" = OpInfo + octets d'operands [u8;2] + compteur de M-cycles restants, avancee d'une phase par M-cycle.
Decodage au moment du fetch (le premier M-cycle lit l'octet opcode -> OpInfo depuis la table) ; les M-cycles suivants lisent les operands selon OpInfo.bytes ; les phases d'execution suivent le cout de la table (m_taken/m_not_taken pour les branches conditionnelles - note 02c). Sans chevauchement par defaut : l'opcode suivant est lu au premier M-cycle de l'instruction suivante.
Pros : etat minimal sans allocation, colle exactement a A_01/A_02 (un acces bus par M-cycle, a un dot fixe) ; les instructions de duree variable sont naturelles (HALT/STOP = etats qui n'avancent pas PC jusqu'au reveil - note 02b) ; la table pilote toute la temporisation, aucun cout d'instruction en dur dans le code ; l'entree d'interrupt est une sequence d'etats de 5 M-cycles inseree a la frontiere d'instruction (note 02b).
Cons : l'executeur doit garantir exactement un acces bus pour chaque transition d'etat - revue soigneuse + tests requis ; plus de branches dans la boucle chaude que l'iteration d'un tableau.

Option C - Pipeline de fetch chevauche : precharger toujours l'opcode suivant pendant le dernier M-cycle de l'instruction en cours, pour que les instructions se suivent sans M-cycle vide.
Pros : correspond a l'indice indirect selon lequel le DMA "starts right after instruction" (refs/pandocs/src/OAM_DMA_Transfer.md#best-practices) si le chevauchement est reel ; moins de M-cycles vides entre instructions.
Cons : refs/pandocs ne dit rien du chevauchement du fetch (open_questions D_07, Statut : UNKNOWN - to confirm) ; la frontiere exacte du delai ei est egalement inconnue (D_06) - un pipeline change le point ou IME=1 devient visible et risque de fausser la temporisation des interrupts avant que les ROM tests ne tranchent ; etat plus complexe (fetch en vol + execution en vol), plus difficile de garder "one micro-op = one bus access" exact.

Recommandation :
Option B - machine a etats avec decodage au moment du fetch, sans chevauchement par defaut. La table generees pilote toute la temporisation (question 5) ; le dispatch des interrupts, HALT et STOP sont des etats/sequences speciales ; la question du chevauchement est differee jusqu'a ce que D_07 soit tranchee par les ROM tests (blargg mem_timing / mooneye-test-suite), sans changer la forme publique de ce modele.

Consequence :
- Etat CPU (tout en taille fixe, aucune allocation dans tick()) : registres AF BC DE HL SP PC (note 02a) ; flag IME (note 02b) ; "instruction en cours" = OpInfo (struct Copy issue de la table) + octets d'operands [u8;2] + compteur de M-cycles restants + phase.
- Chaque M-cycle (tous les 4 dots, decision A_01) : exactement un bus.read ou bus.write via &mut Bus (decision A_02), a un dot fixe du M-cycle que les taches CPU suivantes fixeront.
- Decodage : OPCODES[byte] / CB_OPCODES[byte] depuis opcodes.rs genere (note 02c) - aucune logique de decodage au runtime, aucune allocation ; au fetch de 0xCB, bascule vers la table CB pour l'octet suivant ; les cycles CB incluent deja le fetch du prefixe, aucun M-cycle additionnel n'est ajoute (note 02c).
- Branches conditionnelles : m_taken vs m_not_taken choisi a l'execution selon la condition evaluee (ordre [taken, not-taken] - note 02c) ; une valeur unique est dupliquee dans les deux champs.
- Dispatch des interrupts : a la frontiere d'instruction, avant de demarrer l'instruction suivante, si IME=1 et [IE]&[IF]!=0 (priorite bit 0 d'abord - note 02b), le CPU execute la sequence d'entree fixe de 5 M-cycles (attente 2M + push PC 2M + chargement de l'adresse vecteur 1M) et efface le bit IF correspondant ainsi que IME avant d'appeler le handler ; RETI = ret + IME=1 sur 4 M-cycles.
- EI/DI : le set/clear de ime prend effet a partir de l'instruction suivante ("retarde d'une instruction" - note 02b) ; ei pose un flag pending, et ime devient 1 au demarrage de l'instruction qui suit (un halt immediatement apres ei s'execute donc avec IME encore a 0). La frontiere exacte en cycles est UNKNOWN (open_questions D_06), a confirmer par same-suite ei_delay_halt.gb.
- HALT ($76) : etat d'attente de N M-cycles tant que [IE]&[IF]==0 ; reveil des qu'une demande apparait, quel que soit IME ; si IME=1 le handler est servi avant l'instruction suivant le halt (note 02b). Variante bug : IME=0 + demande pendante au debut du halt -> pc non incremente, l'octet suivant est re-lu (spec officielle 2008 - note 02b).
- STOP ($FB) : consomme son second octet ; le cout d'entree en veille est UNKNOWN - to confirm (open_questions D_06) ; modele comme etat qui attend que P10-P13 passent bas.
- Opcodes illegaux (les 11, aucune variante CB - notes 02a/02c) : etat verrouille jusqu'au power-off ; le cout JSON est ignore.

Statut : APPROUVE

## A_04

Decision: buffers, cadence, core/frontend boundary - framebuffer format and size, audio sample production (integer cycle count between output samples and resulting rate), pacing (audio-driven vs accumulator, one frame = N cycles), and the thread model of the desktop app.
Questions a trancher : (1) Framebuffer format and size. (2) Audio: integer cycle count between output samples and the resulting rate; who resamples. (3) Pacing: audio-driven vs accumulator; one frame = N cycles. (4) Thread model of the desktop app and how input/state cross it without a Mutex in the audio callback.

Options :

Option A - Core pur, cadence par accumulateur d'entier dans le frontend ; la callback audio ne fait que lire un anneau de samples.
Le noyau reste une fonction pure des ticks (Machine::tick() = 1 dot, decision A_01) sans horloge murale ni I/O (AGENTS.md). Le thread principal du frontend porte un accumulateur de dots entier (nanosecondes * 4194304 / 1e9) et appelle Machine::tick() autant de fois ; une frame = exactement 70224 ticks (note 01_timing.md, section "Cycles par frame"). Le PPU ecrirait un index de pixel 2 bits dans un tableau fixe [u8; 23040] (160 x 144 pixels affiches - note 01_timing.md), sans allocation dans tick(). L'APU produit une paire stereo toutes les 128 dots = exactement 32768 Hz (ratio entier de l'horloge maitre, note 05b_audio_mixing.md "echantillonnage natif") et la depose dans un anneau fixe [i16; 2 x 4096] ; le frontend resample 32768 Hz -> taux du device (cpal). La callback audio ne lit que l'anneau, jamais l'etat du noyau.
Pros : determinisme total - le noyau ne voit ni horloge murale ni taux de device, donc deux runs identiques sont bit-identiques ; tous les ratios restent entiers en dots (4194304/128 = 32768 exactement) ; pas de Mutex autour du Machine (un seul thread le touche - AGENTS.md "no Rc<RefCell>/Arc<Mutex> for the bus") et la callback audio ne peut ni bloquer ni deadllock ; pause, save-state, rewind sont triviaux car la machine n'est que des donnees ; le frontend choisit librement le taux du device.
Cons : le thread principal doit suivre le temps reel (70224 ticks/frame a 59,73 Hz - note 01_timing.md) ; s'il derape, il faut limiter les batches de rattrapage et laisser tomber des samples audio plutot que ralentir la machine ; deux horloges a reconcilier (l'accumulateur entier borne le drift par arrondi).

Option B - Cadence audio-driven : la callback cpal avance elle-meme le noyau.
La stream callback calcule les dots ecoules depuis l'appel precedent et appelle Machine::tick() directement, produisant les samples a la demande ; la video est un sous-produit de l'horloge audio.
Pros : synchronisation A/V parfaite par construction (pas d'accumulateur ni de drift) ; le code de temporisation du frontend est minimal.
Cons : le noyau s'execute sur le thread audio, donc tout etat partage avec le thread UI (input, pause, save-state) exige un Mutex autour du Machine - en contradiction avec "no Rc<RefCell>/Arc<Mutex> for the bus" et "no threads in core" (AGENTS.md) ; la callback a une deadline stricte, il faut plafonner les ticks par appel ou risquer des glitches audio ; le timing devient dependant du driver (periode de callback variable), ce qui casse la reproductibilite du rythme d'execution.

Option C - Cadence frame-locked : un batch fixe par frame, audio decouple et lisse.
Le thread principal avance exactement une frame par vsync ou par intervalle fixe (~16,74 ms) ; le noyau produit 23040 pixels + les samples de la frame, et un resampler independant du frontend lisse vers le taux du device.
Pros : boucle la plus simple (un batch par frame), affichage "frame N" trivial, batches deterministes.
Cons : 70224/128 = 548,625 - les samples par frame ne sont pas entiers, donc un "N samples per frame" fixe casse le ratio exact de 32768 Hz ; la cadence quantisee a ~16,7 ms donne un jitter visible sur des ecrans non-60 Hz et un drift A/V que seul le resampler masque ; le taux vsync n'etant pas 59,73 Hz (note 01_timing.md), il faut quand meme un accumulateur continu pour rester fidele.

Recommandation :
Option A - noyau pur + accumulateur de dots entier dans le thread principal + anneau fixe de samples stereo a 32768 Hz consomme par la callback audio, resampling fait cote frontend. C'est la seule option qui garde les quatre contraintes ensemble : cycle accuracy (tick = dot, decision A_01), ratios entiers (4194304/128 = 32768 exactement), determinisme (le noyau ne voit ni horloge murale ni taux de device) et pas de Mutex dans la callback audio (le Machine est possede par un seul thread, la callback ne lit que l'anneau).

Consequence :
- Framebuffer : puce8gb-core porte [u8; 23040] d'indices de pixels 2 bits (160 x 144 - note 01_timing.md) ; le PPU ecrit un index par dot en mode 3 (note 01_timing.md, durees des modes). Le frontend convertit l'index en couleur via BGP/OBP0/OBP1 (refs/pandocs/src/Palettes.md#FF47-BGP) et echelle pour l'affichage ; le noyau ne stocke jamais de RGB.
- Audio : l'APU produit une paire stereo toutes les 128 dots = exactement 32768 Hz (ratio entier de l'horloge maitre - note 05b_audio_mixing.md, section "Echantillonnage natif") ; le mixage suit NR50/NR51 + filtre passe-haut par sortie (note 05b_audio_mixing.md) avec un facteur de charge a 32768 Hz = 0.999958^128 ~ 0.9946 (formule 0.999958^(4194304/rate), note 05b_audio_mixing.md) implemente en point fixe entier dans le noyau ; l'arrondi exact reste a confirmer par un test ROM audio plus tard. Les samples vont dans un anneau fixe [i16; 2 x 4096] (~0,125 s de latence) avec head/tail atomiques - aucune allocation dans tick(). Le frontend (cpal) resample 32768 Hz -> taux du device ; le noyau ne connait ni cpal ni le taux du device.
- Pacing : le thread principal porte un accumulateur de dots entier derive d'une horloge monotone et appelle Machine::tick() autant de fois que de dots ecoules ; une frame = exactement 70224 ticks (note 01_timing.md). En cas de retard, il plafonne la taille du batch de rattrapage et laisse tomber des samples audio dans l'anneau plutot que de ralentir la machine.
- Thread model : le thread principal possede le Machine en exclusif (&mut, aucun Mutex) ; les evenements d'input (clavier/gilrs) traversent vers lui via une file bornee videe aux frontieres de frame ; la callback audio ne lit que l'anneau de samples et ne touche jamais l'etat du noyau - donc pas de lock dans la callback, pas de Rc<RefCell>/Arc<Mutex> (AGENTS.md), et le noyau reste sans thread ni I/O.

Statut : APPROUVE

## A_05

Decision: save states and determinism - serialization format and rules for deterministic execution.
Questions a trancher : (1) Format binaire versionne ecrit a la main, sans dependance. (2) Ce qui fait partie de l'etat. (3) Regles de determinisme (RTC compte en cycles emules, pas d'horloge systeme, pas d'iteration sur hash). (4) Fichiers RAM batterie separes des save states.

Options :

Option A - Format binaire versionne ecrit a la main, sans dependance ; SRAM batterie stockee dans un fichier propre.
Le noyau expose Machine::save(&self, &mut [u8]) -> Result<(), SaveError> et Machine::load(...) : ordre de champs fixe documente en commentaires, entiers little-endian, en-tete = magic 4 octets + version u32 + type cartouche 0147 (note 07a), puis les sections d'etat dans un ordre fixe. Aucune crate externe, aucun I/O dans le noyau (AGENTS.md) - l'ecriture des fichiers est faite par le frontend (cli/desktop). La SRAM batterie (MBC1/2/3/5, note 07b) vit dans son propre fichier par cartouche, charge avant le boot et jamais embarquee dans les save states.
Pros : zero dependance externe (contrainte AGENTS.md du noyau) ; layout de bytes deterministe, unit-testable ; taille petite, adaptee aux saves frequents / rewind (decision A_04) ; le champ version permet de rejeter un format inconnu avec LoadError au lieu d'un panic (AGENTS.md "bad ROM => Result") ; la separation de la RAM batterie colle au hardware - le bit battery du 0147 n'indique que la presence d'une pile (note 07b), et la SRAM persiste a travers les cycles d'alimentation alors qu'un save state est un instantane.
Cons : format ecrit a la main, il faut de la discipline sur l'ordre des champs et le versioning ; non lisible par un humain ; tout changement d'etat machine doit etre passe en revue pour le format.

Option B - Fichier unique auto-suffisant : SRAM batterie embarquee dans chaque save state.
Pros : un seul fichier par slot de sauvegarde, UI plus simple (pas de fichiers batterie orphelins).
Cons : couple la duree de vie de la SRAM a un instantane donne : charger n'importe quel save ecrase le contenu "reel" de la pile ; les jeux qui lisent la SRAM au boot avant tout chargement de save ont besoin d'une source separee quand meme ; contredit le modele hardware ou la pile persiste independamment des instantanes (note 07b) ; fichiers plus gros.

Option C - Format texte/JSON, ou crate externe de serialisation.
Pros : lisible et debogable par un humain ; une crate gererait le versioning a notre place.
Cons : viole "zero external dependencies" du noyau si on utilise une crate ; un JSON ecrit a la main ajoute allocation + parsing sur le chemin de save (lent, source d'erreurs) ; mal adapte aux saves frequents / rewind au rythme audio de 32768 Hz (decision A_04).

Recommandation :
Option A - format binaire versionne ecrit a la main sans dependance, SRAM batterie dans des fichiers separes. C'est la seule option qui garde les quatre contraintes ensemble : zero dependance + aucun I/O dans le noyau (AGENTS.md), layout de bytes deterministe pour des saves bit-a-bit identiques, taille petite pour le rewind, et separation hardware-fidele entre RAM batterie persistante et instantanes.

Consequence :
- Format : en-tete = magic 4 octets + version u32 (debut a 1) + type cartouche 0147 (note 07a) ; puis sections dans un ordre fixe, entiers little-endian, sans compression ; toute entree inconnue ou tronquee renvoie LoadError, jamais de panic (AGENTS.md).
- Ce qui fait partie de l'etat : registres CPU AF BC DE HL SP PC + flag IME + instruction en cours (OpInfo + octets d'operands [u8;2] + compteur M-cycles + phase) + verrou d'opcode illegal (decision A_03) ; toute la memoire du Bus - ROM banks, VRAM 8000-9FFF, WRAM C000-DFFF, OAM FE00-FE9F, HRAM FF80-FFFE, fichier I/O FF00-FFFF y compris IF/IE et Wave RAM FF30-FF3F (decision A_02) ; mode PPU + position dot ; timer DIV/TIMA/TMA + retard d'overflow de 1 M-cycle + tick sur falling edge en attente (note 01b) ; DMA flag active + cycles restants (note 03a) ; etat des canaux APU y compris LFSR CH4, tous les timers length/envelope/sweep et la phase d'echantillonnage du rythme de 128 dots (notes 05a/05b, decision A_04) ; serial SC/SB + compteur de bits du transfert + file d'octets (note 06) ; registres du mapper selon le type (banque ROM/RAM, bit mode, enable RAM - note 07b) et registres RTC MBC3 $08-$0C y compris flag halt et carry des jours (note 07b). Le contenu de la SRAM batterie ne fait pas partie de l'etat.
- Regles de determinisme : le noyau ne lit jamais d'horloge systeme (AGENTS.md) ; le RTC MBC3 n'avance qu'en dots emules - une seconde par 4194304 ticks, arrete par le flag halt (note 07b) - donc deux runs avec la meme sequence d'inputs sont bit-a-bit identiques ; tous les compteurs et ratios restent entiers (decision A_01) ; aucune iteration HashMap/HashSet dans tick() ni save/load (pas de hash iteration), tout l'etat serialise est des tableaux a ordre fixe ; la RAM au power-up est remplie deterministiquement ($00 pour WRAM/HRAM/SRAM, note 08) au lieu du bruit aleatoire.
- RAM batterie : le noyau porte un tableau [u8] dimensionne par la cartouche (type 0147 + taille 0149 - notes 07a/07b), lu/ecrit via le Bus ; le frontend la persiste dans son propre fichier par cartouche, charge avant le boot et ecris a l'extinction / fermeture ; sans fichier existant, la SRAM demarre a $00 (substitut deterministe du bruit - note 08). Les save states ne la contiennent jamais.
- Code : puce8gb-core expose save/load comme fonctions pures sur un tampon fourni par l'appelant (pas de std::fs, pas d'I/O - AGENTS.md) ; les crates cli/desktop gerent chemins et noms de fichiers ; aucune dependance nouvelle dans le noyau.

Statut : APPROUVE

## A_06

Decision: supported variants and first target - portee du hardware emule (DMG seul), ordre des mappers implementes, politique de boot ROM, et premieres cibles ROM pour E02.
Questions a trancher : (1) DMG d'origine seulement (pas CGB, pas SGB) ? (2) Ordre des mappers (ROM only en premier) ? (3) Politique boot ROM : demarrer dans l'etat post-boot documente, boot ROM optionnelle plus tard ? (4) Premieres cibles ROM pour E02 ?

Options :

Option A - DMG seul ; mapper ROM only ($00) en premier ; reset direct a l'etat post-boot documente sans faire tourner la boot ROM ; premieres cibles = mooneye acceptance + blargg cpu_instrs.
Portee : Game Boy d'origine (DMG) uniquement. Le drapeau CGB 0143 et le drapeau SGB 0146 sont lus mais ignores (note 07a, sections "Titre / fabricant / drapeau CGB" et "Drapeau SGB") ; pas de mode couleur, pas de palettes ni paquets de commande Super Game Boy. Ordre des mappers : ROM only ($00) en premier - toutes les cibles test d'E02/E03 (blargg cpu_instrs, mooneye-test-suite, same-suite) sont de petites cartouches qui tournent dessus ; puis MBC1/MBC2 (+RAM/batterie), puis MBC3 avec RTC, puis les codes restants 0147 au besoin (note 07a, section "Type de cartouche"). Politique boot : Machine::reset() place directement l'etat post-boot documente du DMG a PC=$0100 sans faire tourner la boot ROM (note 08, section "Ce qu'il faut emuler sans boot ROM") : A=$01 F=Z=1 N=0 H/C selon le checksum d'en-tete $014D, B=$00 C=$13 DE=$00D8 HL=$014D SP=$FFFE + valeurs I/O (note 08) ; WRAM/HRAM/SRAM a $00 pour la determinisme (decision A_05). L'emulation de la boot ROM (animation du logo, verrouillage sur logo ou checksum invalide) est differee en fonction optionnelle.
Pros : portee minimale conforme a AGENTS.md ("Hardware: original Game Boy (DMG)") ; le travail CPU d'E02 n'est pas bloque par des mappers ni par du code boot ; l'etat post-boot est documente et testable avec les ROMs acceptance mooneye (boot_regs-dmgABC, boot_hwio-dmgABCmgb - note 08) ; reset deterministe sans dump de boot ROM.
Cons : pas d'animation du logo ni de verrouillage sur cartouche invalide (note 08, section "Comportement de la boot ROM") ; les jeux qui s'appuient sur des valeurs exactes au power-up ne sont couverts qu'a l'etat documente pres ; MBC3 RTC et mappers suivants arrivent apres E02/E03.

Option B - DMG + CGB compatible : emuler les deux variantes avec mode couleur, tous les mappers d'un coup, boot ROM complete emulee des le debut.
Pros : couvre plus de cartouches commerciales (drapeau CGB $80/$C0 - note 07a) ; une seule base de code pour les deux consoles ; la plus fidele au hardware reel si la boot ROM de 256 octets est faite tourner (note 08, section "Boot ROM : existence et taille").
Cons : contredit AGENTS.md qui fixe la cible a la DMG d'origine ; double la portee : registres palettes CGB, KEY0/OPBI, tuiles 4 bpp, boot ROM differente de 256+1792 octets (note 08, section "Laisse de cote") ; retarde le travail CPU d'E02 ; la duree de la boot ROM est UNKNOWN - to confirm (note 08), donc une emulation cycle-accurate ne peut pas s'appuyer sur les sources.

Option C - DMG seul mais boot ROM complete emulee des le debut : faire tourner une reimplementation de la boot ROM de 256 octets puis hand-off a la cartouche a $0100 ; tous les mappers d'un coup.
Pros : la plus fidele au hardware reel : animation du logo, son a deux notes, verrouillage sur logo ou checksum d'en-tete invalide (note 08, sections "Comportement de la boot ROM" et "Hand-off vers la ROM de cartouche") ; l'etat post-boot emerge de la simulation plutot que d'etre code en dur.
Cons : la duree de la boot ROM est UNKNOWN - to confirm (note 08), donc le timing cycle-accurate ne peut pas s'appuyer sur les sources ; il faut un dump ou une reimplementation du code de la boot ROM, absent de refs/pandocs ; retarde le travail CPU d'E02 ; l'etat de hand-off (PC=$0100, A=$01 - note 08) est deja atteignable directement au reset avec moins de risque.

Recommandation :
Option A - DMG seul, mapper ROM only ($00) en premier, reset direct a l'etat post-boot documente sans faire tourner la boot ROM, premieres cibles = mooneye acceptance + blargg cpu_instrs. C'est la seule option qui garde la portee conforme a AGENTS.md (DMG d'origine), ne bloque pas le travail CPU d'E02 sur du hardware non emule, et ancre chaque valeur dans les notes (etat post-boot CONFIRME - note 08 ; codes mappers - note 07a).

Consequence :
- Portee : puce8gb-core n'emule que la DMG d'origine. Le drapeau CGB 0143, le drapeau SGB 0146 et les codes licensee sont lus mais ignores (note 07a) ; pas de mode couleur ; OBP0/OBP1 gardent leurs valeurs de reset documentees (note 08).
- Ordre des mappers : ROM only ($00) en premier - les cibles test d'E02 tournent dessus ; puis MBC1/MBC2 (+RAM/batterie), puis MBC3 avec RTC, puis les codes restants 0147 au besoin (note 07a) ; un code inconnu ou non supporte renvoie LoadError (AGENTS.md : bad ROM => Result).
- Politique boot : Machine::reset() place directement l'etat post-boot documente du DMG a PC=$0100 sans faire tourner la boot ROM (note 08, section "Ce qu'il faut emuler sans boot ROM") : A=$01 F=Z=1 N=0 H/C selon le checksum d'en-tete $014D (CONFLIT specs vs pandocs - a trancher par mooneye boot_regs-dmgABC), B=$00 C=$13 D=$00 E=$D8 H=$01 L=$4D SP=$FFFE, valeurs I/O selon note 08 ; WRAM/HRAM/SRAM a $00 (deterministe - decision A_05). L'emulation de la boot ROM (animation du logo + verrouillage) est differee en fonction optionnelle apres E02/E03 ; sa duree etant UNKNOWN - to confirm (note 08), elle ne peut pas etre cycle-accurate sans source.
- Premieres cibles ROM pour E02 : mooneye-test-suite acceptance boot_regs-dmgABC.gb et boot_hwio-dmgABCmgb.gb (tranchent le CONFLIT F/TAC/OBP0/OBP1 de note 08) puis blargg cpu_instrs (E02_13 a E02_15), toutes des cartouches ROM only.

Statut : PROPOSE
