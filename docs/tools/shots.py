#!/usr/bin/env python3
"""Screenshot battery: runs test ROMs headless with the CLI (--model cgb) and compares the last
frame with the reference PNG shipped next to the ROM. Python standard library only.
Status: EXACT (same pixels), LAYOUT (same shapes, other colors), FAIL, TIMEOUT, NOPPM, REFERR,
SIZE, DUMP (no reference: the frame is only kept for a visual check)."""
import argparse
import os
import re
import struct
import subprocess
import sys
import zlib
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BASE = os.path.join(ROOT, "roms", "test-roms")

# (group, folder, subdirs, rom extensions, reference regex on the text after the ROM stem, frames)
SPECS = [
    ("gambatte", "gambatte",
     ["bgen", "bgtiledata", "bgtilemap", "cgbpal_m3", "window", "sprites", "dma",
      "enable_display", "display_startstate", "scy", "vram_m3", "vramw_m3end", "lcd_offset"],
     (".gbc", ".gb"), r"_(cgb04c|xcgb)\.png", 15),
    ("mealybug", "mealybug-tearoom-tests", ["ppu"], (".gb",), r"_cgb_(c|d)\.png", 60),
    ("acid-hell", "cgb-acid-hell", [""], (".gbc",), r"\.png", 120),
    ("age-shots", "age-test-roms", ["m3-bg-bgp", "m3-bg-lcdc", "m3-bg-scx"], (".gb",),
     r"-(cgb|ncm)[0-9A-Z]*\.png", 60),
    ("age-screens", "age-test-roms", ["ly", "vram", "oam", "stat-mode", "halt"], (".gb",),
     None, 200),
]


def samples(line, count, depth):
    if depth == 8:
        return list(line[:count])
    if depth == 16:
        return [line[2 * i] for i in range(count)]
    out = []
    mask = (1 << depth) - 1
    for byte in line:
        for shift in range(8 - depth, -1, -depth):
            out.append((byte >> shift) & mask)
    return out[:count]


def read_png(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a png")
    pos = 8
    idat = b""
    plte = b""
    hdr = None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            hdr = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            plte = body
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    width, height, depth, ctype, _, _, interlace = hdr
    if interlace:
        raise ValueError("interlaced png")
    chans = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    bits = chans * depth
    bpp = max(1, bits // 8)
    stride = (width * bits + 7) // 8
    raw = zlib.decompress(idat)
    rows = []
    prev = bytearray(stride)
    p = 0
    for _ in range(height):
        ftype = raw[p]
        line = bytearray(raw[p + 1:p + 1 + stride])
        p += 1 + stride
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if ftype == 1:
                line[i] = (line[i] + a) & 255
            elif ftype == 2:
                line[i] = (line[i] + b) & 255
            elif ftype == 3:
                line[i] = (line[i] + ((a + b) >> 1)) & 255
            elif ftype == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 255
        rows.append(line)
        prev = line
    pixels = []
    top = (1 << depth) - 1 if depth < 16 else 255
    for line in rows:
        s = samples(line, width * chans, depth)
        for x in range(width):
            if ctype == 0:
                v = s[x] * 255 // top if depth < 8 else s[x]
                pixels.append((v, v, v))
            elif ctype == 4:
                v = s[2 * x] * 255 // top if depth < 8 else s[2 * x]
                pixels.append((v, v, v))
            elif ctype == 3:
                i = s[x]
                pixels.append(tuple(plte[3 * i:3 * i + 3]))
            else:
                k = chans * x
                pixels.append((s[k], s[k + 1], s[k + 2]))
    return width, height, pixels


def read_ppm(path):
    with open(path, "rb") as f:
        d = f.read()
    parts = []
    pos = 0
    while len(parts) < 4 and pos < len(d):
        while pos < len(d) and d[pos:pos + 1].isspace():
            pos += 1
        if d[pos:pos + 1] == b"#":
            pos = d.index(b"\n", pos)
            continue
        start = pos
        while pos < len(d) and not d[pos:pos + 1].isspace():
            pos += 1
        parts.append(d[start:pos])
    pos += 1
    width, height = int(parts[1]), int(parts[2])
    body = d[pos:pos + width * height * 3]
    return width, height, [tuple(body[i:i + 3]) for i in range(0, len(body), 3)]


def compare(ours, ref):
    if ours == ref:
        return "EXACT", 0
    diff = sum(1 for x, y in zip(ours, ref) if x != y)
    fwd, back = {}, {}
    for x, y in zip(ours, ref):
        if fwd.setdefault(x, y) != y or back.setdefault(y, x) != x:
            return "FAIL", diff
    return "LAYOUT", diff


def find_jobs(args):
    jobs = []
    for group, folder, subdirs, exts, refre, frames in SPECS:
        if args.group and not any(g in group for g in args.group):
            continue
        for sub in subdirs:
            top = os.path.join(BASE, folder, sub)
            for dirpath, dirnames, filenames in os.walk(top):
                dirnames.sort()
                for fname in sorted(filenames):
                    stem, ext = os.path.splitext(fname)
                    if ext not in exts:
                        continue
                    refs = []
                    if refre:
                        refs = [os.path.join(dirpath, o) for o in sorted(filenames)
                                if o.startswith(stem) and re.fullmatch(refre, o[len(stem):])]
                        if not refs:
                            continue
                    rel = os.path.relpath(os.path.join(dirpath, stem), BASE).replace(os.sep, "/")
                    if args.filter and args.filter not in rel:
                        continue
                    jobs.append((group, rel, os.path.join(dirpath, fname), refs,
                                 args.frames or frames))
    return jobs


def run_one(job, cli, outdir, keep_all):
    group, name, rom, refs, frames = job
    ppm = os.path.join(outdir, name.replace("/", "__") + ".ppm")
    cmd = [cli, "run", rom, "--model", "cgb", "--frames", str(frames), "--dump-ppm", ppm]
    try:
        subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120)
    except subprocess.TimeoutExpired:
        return group, name, "TIMEOUT", 0, ""
    if not os.path.exists(ppm):
        return group, name, "NOPPM", 0, ""
    if not refs:
        return group, name, "DUMP", 0, ""
    ours = read_ppm(ppm)
    best = None
    rank = {"EXACT": 0, "LAYOUT": 1, "FAIL": 2}
    for ref in refs:
        try:
            rw, rh, rpix = read_png(ref)
        except Exception as err:
            best = best or ("REFERR", 0, os.path.basename(ref) + " " + str(err))
            continue
        if (rw, rh) != (ours[0], ours[1]):
            best = best or ("SIZE", 0, os.path.basename(ref))
            continue
        status, diff = compare(ours[2], rpix)
        if best is None or best[0] in ("REFERR", "SIZE") or rank[status] < rank[best[0]]:
            best = (status, diff, os.path.basename(ref))
    status, diff, ref = best
    if status == "EXACT" and not keep_all:
        os.remove(ppm)
    return group, name, status, diff, ref


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cli", default=os.path.join(
        ROOT, "target", "release", "puce8gb-cli" + (".exe" if os.name == "nt" else "")))
    ap.add_argument("--out", default=os.path.join(ROOT, "..", "shots"))
    ap.add_argument("--report", default=os.path.join(ROOT, "..", "shots_report.tsv"))
    ap.add_argument("--group", action="append", default=[])
    ap.add_argument("--filter", default="")
    ap.add_argument("--frames", type=int, default=0)
    ap.add_argument("--jobs", type=int, default=16)
    ap.add_argument("--keep-all", action="store_true")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    jobs = find_jobs(args)
    if not jobs:
        print("no job found")
        return 1
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        results = list(pool.map(lambda j: run_one(j, args.cli, args.out, args.keep_all), jobs))
    results.sort()
    with open(args.report, "w", encoding="utf-8", newline="\n") as f:
        f.write("group\tname\tstatus\tdiff_pixels\tref\n")
        for r in results:
            f.write("%s\t%s\t%s\t%d\t%s\n" % r)
    table = {}
    for group, _, status, _, _ in results:
        row = table.setdefault(group, {})
        row[status] = row.get(status, 0) + 1
    for group in sorted(table):
        total = sum(table[group].values())
        cells = " ".join("%s=%d" % (k, table[group][k]) for k in sorted(table[group]))
        print("%-12s total=%d %s" % (group, total, cells))
    print("report:", os.path.abspath(args.report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
