#!/usr/bin/env python
"""Convert a binary PPM (P6) to PNG, upscaled. Stdlib only.
Usage: python ppm2png.py in.ppm out.png [scale]   (default scale 4)"""
import struct, sys, zlib

def main():
    src, dst = sys.argv[1], sys.argv[2]
    k = int(sys.argv[3]) if len(sys.argv) > 3 else 4
    d = open(src, "rb").read()
    parts, i = [], 0
    while len(parts) < 4:                       # magic, width, height, maxval
        while d[i:i + 1].isspace():
            i += 1
        j = i
        while not d[j:j + 1].isspace():
            j += 1
        parts.append(d[i:j]); i = j
    i += 1                                      # single whitespace after maxval
    if parts[0] != b"P6" or parts[3] != b"255":
        sys.exit("not a P6 255 PPM")
    w, h = int(parts[1]), int(parts[2])
    px = d[i:i + w * h * 3]
    if len(px) != w * h * 3:
        sys.exit("truncated PPM")
    raw = bytearray()
    for y in range(h):
        row = bytearray()
        for x in range(w):
            row += px[(y * w + x) * 3:(y * w + x) * 3 + 3] * k
        for _ in range(k):
            raw += b"\x00" + row
    def chunk(t, b):
        c = struct.pack(">I", len(b)) + t + b
        return c + struct.pack(">I", zlib.crc32(t + b) & 0xFFFFFFFF)
    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w * k, h * k, 8, 2, 0, 0, 0)) \
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
    open(dst, "wb").write(png)
    print("wrote", dst, w * k, "x", h * k)

main()
