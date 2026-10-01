# decisions.md

Statut initial de toutes les decisions : PROPOSEE. Elles ne deviennent APPROUVEE qu'apres un GATE (sous-tache E00.07), une a la fois (regles section 14).
Format : options, recommandation, consequence.

## D01 - Horloge de reference, ratios, ordonnanceur
Options : (a) Machine::tick = 1 T-cycle (dot) ; (b) Machine::tick = 1 M-cycle, PPU avance de 4 dots par tick.
Recommandation : (b). Le CPU, le timer, le DMA et le bus s'executent par M-cycle (une micro-op = un acces bus) ; la PPU est avancee de 4 dots exacts, l'APU de 1 pas de son horloge (1048576 Hz) par tick. Ratios entiers (01_timing : 1 M-cycle = 4 T-cycles, 4 dots).
Consequence : un acces bus survient a granularite M-cycle ; les effets sub-M-cycle (ecritures PPU en milieu de ligne) se resolvent par l'ordre d'avancement des dots dans le tick (voir D02).

## D02 - Ordre des puces dans un tick
Options : (a) CPU puis timer, DMA, PPU, APU ; (b) timer/PPU puis CPU.
Recommandation : (a), puis ajuster uniquement si un test ROM l'exige. Documenter l'ordre exact ici une fois valide.
Consequence : valeurs de lecture d'un registre dans le meme cycle qu'un evenement (STAT, LY, TIMA) en dependent ; les tests mooneye/mealybug tranchent (note 01_timing : UNKNOWN).

## D03 - Echantillonnage et acquittement des interruptions
Options : (a) controle a la frontiere d'instruction ; (b) controle par cycle avec dispatch de 5 M-cycles.
Recommandation : (a) avec dispatch en 5 M-cycles (note 01_timing). Le cycle exact d'effacement de IF est ajuste par les tests d'interruption du zip mooneye.
Consequence : cpu/interrupts.rs ; E04.04 et E04.05 documentent le point retenu ici.

## D04 - Conception du bus
Options : (a) Rc<RefCell<Bus>> (interdit) ; (b) Machine possede Cpu et Bus ; le Bus possede memoire, cartouche, timer, PPU, APU, DMA, joypad, serie ; le CPU recoit &mut Bus.
Recommandation : (b).
Consequence : pas de references croisees ; les interruptions remontent par IF dans le Bus.

## D05 - Buffers video/audio et frontiere core/frontend
Options : audio core a 1048576 Hz stereo (i16) reechantillonne dans le frontend ; ou resampling dans le core.
Recommandation : core = echantillon stereo par M-cycle (1048576 Hz) ; frontend = filtre passe-bas + ring buffer lock-free (regles 7.3). Video : framebuffer 160x144 u32 RGBA, nuances 0..3 converties via palette.
Consequence : le core reste deterministe et sans filtre dependant du peripherique.

## D06 - Etat initial, save states, determinisme
Options : RAM aleatoire ou zero-fill.
Recommandation : zero-fill deterministe (note 08_boot_reset : contenu initial UNKNOWN). Save states : en-tete + version + empreinte de ROM ; RTC piloté par cycles emules.
Consequence : deux executions identiques donnent le meme hash ; revisiter si un test ROM exige un etat de RAM particulier.

## D07 - Cible, variantes et strategie de test
Options : DMG seul d'abord ; DMG+CGB des le depart.
Recommandation : DMG (monochrome) d'abord : cartouche sans MBC, MBC1, MBC2, MBC3, MBC5 ; sans boot ROM (etat post-boot). CGB, SGB, double vitesse, MMM01/MBC6/MBC7 : etapes optionnelles Eopt apres E11.
Consequence : les phases T1 a T8 (AGENT_PROTOCOL.md) ; harnais en E02 (regle 13.3).
