# open_questions.md

Genere depuis les notes : tout fait au statut UNKNOWN. Format : Qnn | note#fait | resolution prevue.

- Q01 | 01_timing#Largeur du compteur systeme | UNKNOWN - to confirm (test ROMs timer du zip mooneye)
- Q02 | 01_timing#Ordre des puces dans un tick | UNKNOWN - to confirm (decision D02 + test ROMs)
- Q03 | 02_cpu#Bits bas de F | UNKNOWN - to confirm (test ROM cpu_instrs, sous-test pop af / push af)
- Q04 | 02_cpu#Cycles par instruction | UNKNOWN - to confirm (snapshoter l'url dans annexes/sources_cache/ avant E02.07)
- Q05 | 02_cpu#Effets sur les flags par instruction | UNKNOWN - to confirm
- Q06 | 02_cpu#stop | UNKNOWN - to confirm (lire Reducing_Power_Consumption.md l.59-110 en E02.42)
- Q07 | 03_memory_map#Lecture des I/O non mappees / bits inutilises | UNKNOWN - to confirm (E01.13 : lister l'adressage ; valider par mooneye)
- Q08 | 04_video#LCD eteint | CONFIRME (details LY : UNKNOWN - to confirm)
- Q09 | 04_video#Pixel FIFO | UNKNOWN - to confirm (extraction E05.20)
- Q10 | 05_audio#Registres et details des canaux 2, 3, 4, NR52 | UNKNOWN - to confirm (extraction en E08.01 a E08.05)
- Q11 | 06_input#Interruption joypad | UNKNOWN - to confirm (lire Interrupt_Sources.md en E06.03)
- Q12 | 06_input#Serie SB/SC | CONFIRME (duree d'un transfert : UNKNOWN - to confirm)
- Q13 | 07_media#MBC1 - registres | CONFIRME (mode 6000-7FFF : UNKNOWN - to confirm, lire MBC1.md l.95-150 en E07.04)
- Q14 | 07_media#MBC2 et MBC3 (RTC) | UNKNOWN - to confirm (extraction E07.08 et E07.11)
- Q15 | 08_boot_reset#Registres materiels (DMG) | CONFIRME (OBP0, OBP1 : UNKNOWN - to confirm)
- Q16 | 08_boot_reset#WRAM / VRAM / HRAM au demarrage | UNKNOWN - to confirm (decision : zero-fill deterministe, D06)
- Q17 | 09_test_roms#Blargg - sortie serie | UNKNOWN - to confirm (lire howto/blargg.md et le README du depot retrio/gb-test-roms)

Questions ajoutees pendant le developpement : une ligne par question, avec la sous-tache d'origine et la cause probable.
