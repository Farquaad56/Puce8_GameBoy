# coverage_matrix.md

| Sujet | Couvert | Partiel | Manquant | Sources / etape qui comble |
|---|---|---|---|---|
| Timing | horloges, frame, DIV/TIMA, dispatch | largeur compteur systeme, ordre des puces | | 01_timing ; D02 ; E04.17, E04.20 extraient Timer_Obscure |
| CPU | registres, flags, encodage, IME, halt | cycles et flags par instruction, F bas, stop | | 02_cpu ; E03.01 (snapshot optables), E03.63 |
| Carte memoire | plan, echo, FEA0, I/O | masques de lecture I/O | | 03_memory_map ; E01.15 |
| Video | LCDC, STAT, tuiles, OBJ, fenetre, penalites | pixel FIFO, priorites OBJ, LCD off | | 04_video ; E05.01-E05.03 |
| Audio | architecture, DIV-APU, pulse | registres NR2x/NR3x/NR4x, NR52, HPF | | 05_audio ; E08.01-E08.05 |
| Entrees | joypad, serie (registres) | IRQ joypad, duree transfert serie | | 06_input ; E06.08, E06.10 |
| Media / mappers | en-tete, MBC1 (3 registres), MBC5 | MBC1 mode/MBC1M, MBC2, MBC3 | | 07_media ; E07.03, E07.07, E07.09 |
| Boot / reset | registres CPU et I/O DMG | OBP0/OBP1, RAM initiale | | 08_boot_reset ; D06 |
| Test ROMs | mooneye, blargg (duree, ecran), dmg-acid2 | sortie serie blargg, interrupt_time (footnote 4) | | 09_test_roms ; E02, E05.41 |

Regle : tant qu'un sujet bloquant est Partiel, la sous-tache qui en depend commence par l'extraction listee (colonne de droite).
