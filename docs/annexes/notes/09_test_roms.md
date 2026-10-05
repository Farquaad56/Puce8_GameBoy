# Note 09 - Catalogue des ROMs de test (roms/test-roms, archive v7.0)

Tache D_17. Sources : roms/test-roms/README.md, roms/test-roms/src/howto/*.md,
roms/test-roms/mooneye-test-suite/README.markdown, arborescence locale (v7.0).
Portee : DMG d'origine ; les variantes CGB/SGB sont citees quand elles changent le signal.

## Inventaire des suites
Fait : roms/test-roms contient 18 suites compilees (archive v7.0) : age-test-roms, blargg, bully, cgb-acid2, cgb-acid-hell, dmg-acid2, gambatte, gbmicrotest, little-things-gb, mbc3-tester, mealybug-tearoom-tests, mooneye-test-suite (+ fork wilbertpol), rtc3test, same-suite, scribbltests, strikethrough, turtle-tests. Chaque suite a un howto dans src/howto/ ; le README impose 3 questions par ROM : device compatible, exit condition, success/failure signal.
Source : roms/test-roms/README.md (L10-L100)
Fiabilite : communautaire
Impact code : test runner : dispatcher par suite + harnais de pass/fail.
Statut : CONFIRME

## Blargg : contenu et nommage des fichiers
Fait : blargg/ contient 8 suites DMG/CGB : cpu_instrs, instr_timing, mem_timing (+mem_timing-2), dmg_sound, cgb_sound, halt_bug, interrupt_time, oam_bug. Nommage exact : <suite>.gb a la racine de chaque sous-dossier (ex. blargg/cpu_instrs/cpu_instrs.gb) ; les ROMs individuelles sont dans <suite>/individual/NN-nom.gb et <suite>/rom_singles/ pour sound. Les 11 cpu_instrs individual (chemins exacts) :
roms/test-roms/blargg/cpu_instrs/individual/01-special.gb
roms/test-roms/blargg/cpu_instrs/individual/02-interrupts.gb
roms/test-roms/blargg/cpu_instrs/individual/03-op sp,hl.gb
roms/test-roms/blargg/cpu_instrs/individual/04-op r,imm.gb
roms/test-roms/blargg/cpu_instrs/individual/05-op rp.gb
roms/test-roms/blargg/cpu_instrs/individual/06-ld r,r.gb
roms/test-roms/blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb
roms/test-roms/blargg/cpu_instrs/individual/08-misc instrs.gb
roms/test-roms/blargg/cpu_instrs/individual/09-op r,r.gb
roms/test-roms/blargg/cpu_instrs/individual/10-bit ops.gb
roms/test-roms/blargg/cpu_instrs/individual/11-op a,(hl).gb
Source : roms/test-roms/src/howto/blargg.md ; find roms/test-roms -name '*.gb' | grep cpu_instrs (v7.0 locale)
Fiabilite : communautaire + testee sur ROM (arborescence verifiee localement)
Impact code : test runner : liste des ROMs blargg a executer.
Statut : CONFIRME

## Blargg : durees d'execution par test
Fait : temps emule requis, DMG-C : cpu_instrs 55 s, dmg_sound 36 s, halt_bug 2 s, instr_timing 1 s, interrupt_time 2 s, mem_timing 3 s, mem_timing-2 4 s, oam_bug 21 s. CGB (B/C/E) : cgb_sound 37 s, cpu_instrs 31 s, halt_bug 2 s, instr_timing 1 s, interrupt_time 2 s, mem_timing 3 s, mem_timing-2 4 s.
Source : roms/test-roms/src/howto/blargg.md#Exit-Condition (tableaux DMG-C et CGB)
Fiabilite : communautaire
Impact code : test runner : duree d'execution par ROM blargg.
Statut : CONFIRME

## Blargg : signal pass/echec
Fait : le test reussit si l'ecran correspond a la capture de reference dans src/blargg-expected (ex. cpu_instrs/cpu_instrs-dmg-cgb.png, dmg_sound/dmg_sound-dmg.png). dmg_sound et cgb_sound bouclent a l'infini sur le hardware ; les captures ont ete faites avec SameBoy qui arrete le test correctement. interrupt_time est une ROM CGB-only : elle echoue volontairement sur DMG-C (checksum 7F8F4AAF).
Source : roms/test-roms/src/howto/blargg.md#Test-SuccessFailure + notes 1-4 des tableaux
Fiabilite : communautaire
Impact code : video : palette ; test runner : arret par capture d'ecran.
Statut : CONFIRME

## Palette commune des captures d'ecran
Fait : pour comparer les captures, l'emulateur doit utiliser #000000/#555555/#AAAAAA/#FFFFFF pour les 4 teintes DMG ; chaque canal CGB 5 bits est converti en 8 bits par (X << 3) | (X >> 2) ; mode compatibilite CGB : fond #000000/#0063C6/#7BFF31/#FFFFFF, sprites #000000/#943939/#FF8484/#FFFFFF.
Source : roms/test-roms/src/howto/blargg.md#Test-SuccessFailure (idem mooneye-test-suite.md, dmg-acid2.md)
Fiabilite : communautaire
Impact code : video : conversion de palette pour le harnais screenshot.
Statut : CONFIRME

## Mooneye : fin de test et signal pass/echec
Fait : chaque ROM termine en executant l'opcode 0x40 (LD B,B) ; duree max 120 s emulees. Reussite : registres Fibonacci B=3, C=5, D=8, E=13, H=21, L=34 ; echec : les 6 registres valent 0x42 et le port serie envoie 0x42 six fois (pas d'interruption serie requise, attente par busy loop). Le fork wilbertpol utilise l'opcode indefini 0xED a la place de LD B,B.
Source : roms/test-roms/src/howto/mooneye-test-suite.md#Exit-Condition + #Test-SuccessFailure ; mooneye-test-suite/README.markdown#Pass-fail-reporting ; howto/mooneye-test-suite-wilbertpol.md
Fiabilite : communautaire
Impact code : cpu : detection de l'opcode 0x40 ; serial : port serie ; test runner : lecture des registres.
Statut : CONFIRME

## Mooneye : suffixes hardware dans les noms de fichiers
Fait : un suffixe -dmg/-mgb/-sgb/-sgb2/-cgb/-agb/-ags limite le test a ce modele, avec revisions SoC (DMG : 0, A, B, C ; CGB : 0..E) ; les groupes G=dmg+mgb, S=sgb+sgb2, C=cgb+agb+ags s'additionnent (ex. -GS). Pour un emulateur DMG : executer les ROMs sans suffixe + celles en -dmgX (revision X) et groupe G ; ignorer mgb/sgb/cgb/agb/ags. Exemples locaux : acceptance/boot_regs-dmgABC.gb, boot_div2-S.gb, boot_regs-mgb.gb.
Source : roms/test-roms/mooneye-test-suite/README.markdown#Test-naming ; howto/mooneye-test-suite.md (ex. boot_regs-dmgABC)
Fiabilite : communautaire + testee sur ROM (noms verifies localement)
Impact code : test runner : filtre des ROMs mooneye par variante DMG.
Statut : CONFIRME

## Mooneye : tests basees capture d'ecran
Fait : seul manual-only/sprite_priority.gb est compare par capture (hors suite automatique). Captures de reference en palette commune : roms/test-roms/mooneye-test-suite/manual-only/sprite_priority-dmg.png et sprite_priority-cgb.png ; aussi madness/mgb_oam_dma_halt_sprites_expected.png.
Source : roms/test-roms/src/howto/mooneye-test-suite.md#Screenshot-based-tests ; find *.png (v7.0 locale)
Fiabilite : communautaire + testee sur ROM
Impact code : video : harnais screenshot pour sprite_priority.
Statut : CONFIRME

## dmg-acid2
Fait : stress PPU DMG, sans exigence de precision T-cycle ; termine par LD B,B (0x40) ; pass/echec = comparaison des captures roms/test-roms/dmg-acid2/dmg-acid2-dmg.png et dmg-acid2-cgb.png. Duree d'execution : non documentee dans le README ni le howto.
Source : roms/test-roms/src/howto/dmg-acid2.md ; roms/test-roms/dmg-acid2/README.md#Reference-Image
Fiabilite : communautaire
Impact code : video : rendu PPU ; test runner : screenshot.
Statut : CONFIRME (duree : UNKNOWN - to confirm)

## Autres suites : signal pass/echec et durees
Fait : gbmicrotest (513 ROMs NN-nom.gb) ecris le resultat en RAM 0xFF80-0xFF82 ; 0xFF82=0x01 = pass, 0xFF = fail ; ~2 frames suffisent sauf is_if_set_during_ime0.gb (~380 ms). same-suite et age-test-roms : LD B,B + registres Fibonacci (age : toute autre valeur = echec ; certains tests manuels par capture). gambatte : toutes les ROMs finissent apres 15 frames (1053360 cycles, ~252 ms) ; tests audio _outaudio0/1 (silence ou son), resultats hexadecimaux _out<hex> compares a un motif monochrome, sinon capture .png a cote de la ROM.
Source : roms/test-roms/src/howto/{gbmicrotest,same-suite,age-test-roms,gambatte}.md
Fiabilite : communautaire
Impact code : test runner : harnais par type (RAM 0xFF82, registres, frames, audio).
Statut : CONFIRME

## Suites basees capture d'ecran (autres)
Fait : bully (0.5 s ; src/bully-expected/bully.png), little-things-gb (firstwhite ~0.5 s ; tellinglys = appuyer sur tous les boutons puis ~5 s ; firstwhite-dmg-cgb.png, tellinglys-{dmg,cgb}.png), mbc3-tester (boucle infinie, verifier apres 40 frames ; mbc3-tester-{dmg,cgb}.png), mealybug-tearoom-tests (LD B,B + captures ppu/*.png par revision), scribbltests (~10 frames, sauf statcount_auto ~270 frames/4.5 s ; captures dans src/scribbltests-expected/, aucune pour failrylake et winpos), strikethrough (0.5 s ; strikethrough-{dmg,cgb}.png), turtle-tests (0.5 s/~30 frames ; window_y_trigger.png, window_y_trigger_wx_offscreen.png), rtc3test (selection par boutons : A=13 s basic, down+A=8 s range, down+down+A=26 s sub-second writes ; captures rtc3test-*-tests-{dmg,cgb}.png).
Source : roms/test-roms/src/howto/{bully,little-things-gb,mbc3-tester,mealybug-tearoom-tests,scribbltests,strikethrough,turtle-tests,rtc3test}.md ; find *.png (v7.0 locale)
Fiabilite : communautaire + testee sur ROM
Impact code : input : emulation de boutons (tellinglys, rtc3test) ; video : harnais screenshot.
Statut : CONFIRME

## Points non documentes
Fait : le README et les howto ne disent pas comment signaler pass/echec des 11 cpu_instrs individual ROMs separement (seule la capture de l'ensemble est fournie), ni la duree d'execution de dmg-acid2, ni celle des suites cgb-acid2/cgb-acid-hell (0 .gb local).
Source : grep complet roms/test-roms/README.md + src/howto/*.md ; find *.gb (v7.0 locale)
Fiabilite : communautaire (absence de donnee)
Impact code : test runner : a trancher par execution des ROMs.
Statut : UNKNOWN - to confirm

Laisse hors note : details des suites CGB-only (cgb-acid2, cgb-acid-hell), fork wilbertpol au-dela de l'opcode 0xED, howto bully/strikethrough au-dela du signal.
