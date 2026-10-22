#!/usr/bin/env python3
"""Generates docs/annexes/test_manifest_battery.txt: CGB tests whose result is read from the
CPU registers (Fibonacci B=3 C=5 D=8 E=13 H=21 L=34 after LD B,B). Only ROMs meant for CGB
revision E, or generic ones, are kept (names tagged -cgb0B, -cgbBC, -A, -GS are skipped)."""
import os
import re

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BASE = os.path.join(ROOT, "roms", "test-roms")
OUT = os.path.join(ROOT, "docs", "annexes", "test_manifest_battery.txt")
TAG = r"(?:(?:cgb|ncm|dmg)[0-9A-Z]*|ds|nocgb|A|GS|C|S)"
SUITES = [
    ("same-suite", "same-suite", ["apu", "dma", "interrupt", "ppu"]),
    ("age", "age-test-roms", ["halt", "lcd-align-ly", "ly", "oam", "speed-switch",
                              "stat-interrupt", "stat-mode", "stat-mode-sprites",
                              "stat-mode-window", "vram"]),
]
EXTRA = [
    ("mooneye cgb misc/boot_regs-cgb", "mooneye-test-suite/misc/", "boot_regs-cgb.gb"),
    ("mooneye cgb misc/boot_div-cgbABCDE", "mooneye-test-suite/misc/", "boot_div-cgbABCDE.gb"),
    ("mooneye cgb misc/bits/unused_hwio-C", "mooneye-test-suite/misc/bits/", "unused_hwio-C.gb"),
    ("mooneye cgb misc/ppu/vblank_stat_intr-C", "mooneye-test-suite/misc/ppu/", "vblank_stat_intr-C.gb"),
]


def keep(stem):
    m = re.search(r"((?:-" + TAG + r")+)$", stem)
    tags = m.group(1).split("-")[1:] if m else []
    cgbish = [t for t in tags if t.startswith(("cgb", "ncm"))]
    other = [t for t in tags if not t.startswith(("cgb", "ncm", "dmg")) and t not in ("ds", "nocgb")]
    if other:
        return False
    if cgbish:
        return all("E" in t[3:] for t in cgbish)
    return not any(t.startswith("dmg") for t in tags)


rows = []
for alias, folder, subs in SUITES:
    for sub in subs:
        top = os.path.join(BASE, folder, sub)
        for dirpath, dirnames, filenames in os.walk(top):
            dirnames[:] = sorted(d for d in dirnames if d != "caution")
            for fname in sorted(filenames):
                if not fname.endswith(".gb") or not keep(fname[:-3]):
                    continue
                rel = os.path.relpath(dirpath, BASE).replace(os.sep, "/")
                rel_in = os.path.relpath(dirpath, os.path.join(BASE, folder)).replace(os.sep, "/")
                rows.append("%s %s/%s|%s/|%s|mooneye|100000000|-|cgb" % (alias, rel_in, fname[:-3], rel, fname))
for name, directory, fname in EXTRA:
    if os.path.exists(os.path.join(BASE, directory, fname)):
        rows.append("%s|%s|%s|mooneye|100000000|-|cgb" % (name, directory, fname))
rows.sort()
with open(OUT, "w", encoding="utf-8", newline="\n") as f:
    f.write("# name|dir-pattern|file-prefix|kind|max-cycles|param|model\n")
    for row in rows:
        f.write(row + "\n")
counts = {}
for row in rows:
    key = "/".join(row.split("|")[0].split(" ")[0:2])[:40]
    counts[key] = counts.get(key, 0) + 1
print("rows", len(rows))
for key in sorted(counts):
    print(key, counts[key])
