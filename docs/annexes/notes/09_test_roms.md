# 09_test_roms - Test ROMs et criteres de reussite

Module cible : cli + tests
Format : regles 11.2 (Fait / Source / Fiabilite / Impact code / Statut).
Chemins `pandocs/src/...` = depot https://github.com/gbdev/pandocs ; `game-boy-test-roms/...` = https://github.com/c-sp/game-boy-test-roms.

## Source des binaires
Fait : Collection compilee : releases de c-sp/game-boy-test-roms (zip). Placer dans roms/tests/ (hors git).
Source : game-boy-test-roms/README.md
Fiabilite : communautaire
Impact code : cli : manifest
Statut : CONFIRME

## Mooneye - fin et reussite
Fait : Fin = execution de l'opcode $40 (LD B,B). Succes = B=3 C=5 D=8 E=13 H=21 L=34. Delai max 120 secondes emulees. Suffixes de nom (ex. -dmgABC) limitent le modele.
Source : game-boy-test-roms/src/howto/mooneye-test-suite.md
Fiabilite : testee sur ROM
Impact code : cli : fn check_mooneye
Statut : CONFIRME

## Blargg - duree et reussite
Fait : Reussite = capture d'ecran identique a l'attendue. Secondes emulees sur DMG-C : cpu_instrs 55, dmg_sound 36, halt_bug 2, instr_timing 1, interrupt_time 2 (marque en echec dans la table, note 4), mem_timing 3, mem_timing-2 4. Captures dans src/blargg-expected/.
Source : game-boy-test-roms/src/howto/blargg.md
Fiabilite : testee sur ROM
Impact code : cli : fn run_for_seconds
Statut : CONFIRME (footnote 4 interrupt_time : lire avant d'en faire un critere)

## Blargg - sortie serie
Fait : Lecture du texte de resultat via le port serie non documentee dans les sources lues.
Source : aucune
Fiabilite : deduite
Impact code : cli
Statut : UNKNOWN - to confirm (lire howto/blargg.md et le README du depot retrio/gb-test-roms)

## dmg-acid2
Fait : Fin = opcode $40 (LD B,B). Reussite = capture identique. Nuances DMG #000000 #555555 #AAAAAA #FFFFFF. Captures dans les archives du depot.
Source : game-boy-test-roms/src/howto/dmg-acid2.md
Fiabilite : testee sur ROM
Impact code : cli : fn compare_png
Statut : CONFIRME

## Autres suites disponibles
Fait : AGE, Bully, Scribbltests, Strikethrough, Mealybug Tearoom, Gambatte, GBMicrotest, MBC3 Tester, SameSuite, rtc3test, TurtleTests, little-things-gb : un howto par suite dans game-boy-test-roms/src/howto/.
Source : game-boy-test-roms/src/howto/
Fiabilite : communautaire
Impact code : cli : manifest
Statut : CONFIRME
