#!/usr/bin/env python
"""Compare a PPM (P6) dumped by the emulator with a reference PNG, shade by shade.
Usage: python compare_png_ppm.py <ours.ppm> <reference.png> [--ascii]
Colors are ranked by luminance in each image (darkest = highest shade), so the exact
palette does not matter. Exit 0 = identical shade maps, 1 = different, 2 = bad input.
Stdlib only."""
import struct, sys, zlib

def read_ppm(path):
    d = open(path, "rb").read()
    parts, i = [], 0
    while len(parts) < 4:
        while d[i:i + 1].isspace():
            i += 1
        j = i
        while not d[j:j + 1].isspace():
            j += 1
        parts.append(d[i:j]); i = j
    i += 1
    if parts[0] != b"P6" or parts[3] != b"255":
        sys.exit("ours: not a P6 255 PPM")
    w, h = int(parts[1]), int(parts[2])
    px = d[i:i + w * h * 3]
    if len(px) != w * h * 3:
        sys.exit("ours: truncated PPM")
    return w, h, [tuple(px[k:k + 3]) for k in range(0, len(px), 3)]

def read_png(path):
    d = open(path, "rb").read()
    if d[:8] != b"\x89PNG\r\n\x1a\n":
        sys.exit("reference: not a PNG")
    i, idat, plte = 8, b"", None
    w = h = depth = ctype = None
    while i < len(d):
        n, t = struct.unpack(">I4s", d[i:i + 8])
        body = d[i + 8:i + 8 + n]
        if t == b"IHDR":
            w, h, depth, ctype, _, _, il = struct.unpack(">IIBBBBB", body)
            if il != 0 or depth != 8 or ctype not in (0, 2, 3, 4, 6):
                sys.exit("reference: unsupported PNG (need 8 bit, not interlaced)")
        elif t == b"PLTE":
            plte = [tuple(body[k:k + 3]) for k in range(0, len(body), 3)]
        elif t == b"IDAT":
            idat += body
        i += 12 + n
    ch = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[ctype]
    raw = zlib.decompress(idat)
    stride = w * ch
    rows, prev = [], bytearray(stride)
    p = 0
    for _ in range(h):
        f = raw[p]; line = bytearray(raw[p + 1:p + 1 + stride]); p += 1 + stride
        for x in range(stride):
            a = line[x - ch] if x >= ch else 0
            b = prev[x]
            c = prev[x - ch] if x >= ch else 0
            if f == 1: line[x] = (line[x] + a) & 255
            elif f == 2: line[x] = (line[x] + b) & 255
            elif f == 3: line[x] = (line[x] + ((a + b) >> 1)) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        rows.append(line); prev = line
    px = []
    for line in rows:
        for x in range(w):
            q = line[x * ch:(x + 1) * ch]
            if ctype == 0 or ctype == 4: px.append((q[0], q[0], q[0]))
            elif ctype == 3: px.append(plte[q[0]])
            else: px.append((q[0], q[1], q[2]))
    return w, h, px

def lum(c):
    return c[0] * 299 + c[1] * 587 + c[2] * 114

def ranks(px):
    cols = sorted(set(px), key=lum, reverse=True)      # brightest first = shade 0
    m = {c: i for i, c in enumerate(cols)}
    return [m[c] for c in px], len(cols)

def main():
    if len(sys.argv) < 3:
        sys.exit(2)
    w1, h1, a = read_ppm(sys.argv[1])
    w2, h2, b = read_png(sys.argv[2])
    if (w1, h1) != (w2, h2):
        print("SIZE DIFFERENT: ours %dx%d, reference %dx%d" % (w1, h1, w2, h2)); sys.exit(1)
    ra, na = ranks(a); rb, nb = ranks(b)
    print("distinct shades: ours %d, reference %d" % (na, nb))
    diff = [i for i in range(len(ra)) if ra[i] != rb[i]]
    if na != nb:
        print("DIFFERENT number of shades")
    if not diff and na == nb:
        print("IDENTICAL"); sys.exit(0)
    if diff:
        xs = [i % w1 for i in diff]; ys = [i // w1 for i in diff]
        print("%d pixels differ; first at x=%d y=%d; box x %d..%d y %d..%d" %
              (len(diff), xs[0], ys[0], min(xs), max(xs), min(ys), max(ys)))
        if "--ascii" in sys.argv:
            x0, x1 = min(xs), min(max(xs), min(xs) + 99)
            y0, y1 = min(ys), min(max(ys), min(ys) + 39)
            ch = " .#@"
            for name, r in (("ours", ra), ("reference", rb)):
                print("--- %s (box) ---" % name)
                for y in range(y0, y1 + 1):
                    print("".join(ch[min(r[y * w1 + x], 3)] for x in range(x0, x1 + 1)))
    sys.exit(1)

main()
