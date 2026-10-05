#!/usr/bin/env python3
"""Generate opcodes.rs from seed/Opcodes.json (gbdev gb-opcodes table).

Re-runnable and deterministic: the output depends only on the JSON content.
Stdlib only. Run from anywhere:  python docs/annexes/code/gen_opcodes.py
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SEED = os.path.join(HERE, "..", "seed", "Opcodes.json")
OUT = os.path.join(HERE, "opcodes.rs")

# Illegal opcodes per refs/pandocs/src/CPU_Instruction_Set.md (invalid list) and
# the seed table; the CPU hard-locks on them. The JSON gives each a 1 M-cycle cost; do not rely on it.
EXPECTED_ILLEGAL = ["D3", "DB", "DD", "E3", "E4", "EB", "EC", "ED", "F4", "FC", "FD"]

FLAG_ALPHABET = {"-", "0", "1", "Z", "N", "H", "C"}
NAME_CHARS = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789$(),+-_ ")


def load_seed():
    with open(SEED, encoding="utf-8") as f:
        d = json.load(f)
    assert list(d.keys()) == ["unprefixed", "cbprefixed"], d.keys()
    for name in ("unprefixed", "cbprefixed"):
        t = d[name]
        assert len(t) == 256, (name, len(t))
        assert sorted(t.keys()) == ["0x%02X" % i for i in range(256)], name + " keys not 0x00..0xFF"
    return d


def normalize_cycles(cycles):
    # The JSON gives T-cycles; M-cycle = T/4 (every value is divisible by 4).
    values = cycles if isinstance(cycles, list) else [cycles]
    assert len(values) in (1, 2), values
    for v in values:
        assert isinstance(v, int) and v % 4 == 0, ("T-cycles not divisible by 4", values)
        m = v // 4
        assert 1 <= m <= 255, ("M-cycle out of u8 range", values)
    if len(values) == 1:  # fixed cost => taken = not taken
        return (values[0] // 4, values[0] // 4)
    return (values[0] // 4, values[1] // 4)  # JSON order: [branch taken, branch not taken]


def operand_text(o):
    name = o["name"]
    if o.get("increment"):
        name += "+"
    elif o.get("decrement"):
        name += "-"
    return ("(%s)" % name) if not o.get("immediate") else name


def mnemonic_text(e):
    ops = e["operands"]
    parts, i = [], 0
    while i < len(ops):
        o = ops[i]
        t = operand_text(o)
        nxt = ops[i + 1] if i + 1 < len(ops) else None
        # Special case (e.g. F8 LD HL,SP+e8): an inc/dec register followed by a byte-counted
        # immediate is one combined operand, joined without a comma.
        if nxt is not None and (o.get("increment") or o.get("decrement")) and nxt.get("bytes"):
            t += operand_text(nxt)
            i += 2
        else:
            i += 1
        parts.append(t)
    return e["mnemonic"] + ((" " + ",".join(parts)) if parts else "")


def build_entries(table):
    entries = []
    for i in range(256):  # index order 0x00..0xFF by construction
        e = table["0x%02X" % i]
        assert isinstance(e["bytes"], int) and 1 <= e["bytes"] <= 255, (i, e)
        m_taken, m_not_taken = normalize_cycles(e["cycles"])
        flags = e["flags"]
        assert list(flags.keys()) == ["Z", "N", "H", "C"], (i, flags)
        for v in flags.values():
            assert v in FLAG_ALPHABET, (i, v)  # "-" unchanged, "0"/"1" forced, letter = computed
        text = mnemonic_text(e)
        for ch in text:
            assert ch in NAME_CHARS, ("bad char in mnemonic", i, text)
        entries.append({
            "idx": i,
            "mnemonic": text,
            "bytes": e["bytes"],
            "m_taken": m_taken,
            "m_not_taken": m_not_taken,
            "flags": [flags[k] for k in ("Z", "N", "H", "C")],
            "illegal": e["mnemonic"].startswith("ILLEGAL"),
        })
    return entries


def check_illegal(entries):
    ill = ["%02X" % e["idx"] for e in entries if e["illegal"]]
    return ill


# Hand spot-checks against refs/pandocs/historical/2001-Oct-pandocs.txt, section "CPU Instruction Set"
# (lines cited below; timings are T-cycles there: "all gameboy timings are divideable by 4").
# Each row: (opcode address in table index 0 = OPCODES, 1 = CB_OPCODES) -> bytes, m_taken, m_not_taken, flags Z N H C.
SPOT_CHECKS = [
    ((0, 0x00), 1, 1, 1, ("-", "-", "-", "-")),   # L2231 nop 00 4 ----
    ((0, 0x20), 2, 3, 2, ("-", "-", "-", "-")),   # L2242 jr f,PC+dd xx dd 12;8 ---- (taken first)
    ((0, 0x34), 1, 3, 3, ("Z", "0", "H", "-")),   # L2185 inc (HL) 34 12 z0h-
    ((0, 0x76), 1, 1, 1, ("-", "-", "-", "-")),   # L2232 halt 76 N*4 ---- (base cost = 1 M; wait is extra)
    ((0, 0xC0), 1, 5, 2, ("-", "-", "-", "-")),   # L2246 ret f xx 20;8 ----
    ((0, 0xCD), 3, 6, 6, ("-", "-", "-", "-")),   # L2243 call nn CD nn nn 24 ----
    ((0, 0xE8), 2, 4, 4, ("0", "0", "H", "C")),   # L2195 add SP,dd E8 16 00hc
    ((0, 0xF8), 2, 3, 3, ("0", "0", "H", "C")),   # L2196 ld HL,SP+dd F8 12 00hc (text must be LD HL,SP+e8)
    ((1, 0x06), 2, 4, 4, ("Z", "0", "0", "C")),   # L2204 rlc (HL) CB 06 16 z00c
    ((1, 0x46), 2, 3, 3, ("Z", "0", "1", "-")),   # L2222 bit n,(HL) CB xx 12 z01- (znhc order)
]


def check_spot(ops):
    tables = [ops["unprefixed_entries"], ops["cbprefixed_entries"]]
    for (tbl_idx, addr), nbytes, mtaken, mnottaken, flags in SPOT_CHECKS:
        e = tables[tbl_idx][addr]
        assert e["bytes"] == nbytes, (tbl_idx, "%02X" % addr, "bytes", e["bytes"], nbytes)
        assert (e["m_taken"], e["m_not_taken"]) == (mtaken, mnottaken), (tbl_idx, "%02X" % addr, e)
        assert tuple(e["flags"]) == flags, (tbl_idx, "%02X" % addr, "flags", e["flags"], flags)


def render(entries):
    lines = []
    for e in entries:
        z, n, h, c = e["flags"]
        lines.append(
            '    OpInfo { mnemonic: "%s", bytes: %d, m_taken: %d, m_not_taken: %d, '
            'z: "%s", n: "%s", h: "%s", c: "%s", illegal: %s }, // 0x%02X\n'
            % (e["mnemonic"], e["bytes"], e["m_taken"], e["m_not_taken"], z, n, h, c,
               "true" if e["illegal"] else "false", e["idx"]))
    return lines


def main():
    d = load_seed()
    up = build_entries(d["unprefixed"])
    cb = build_entries(d["cbprefixed"])

    # Step 6 script assertions.
    assert len(up) == 256 and len(cb) == 256
    ill_up = check_illegal(up)
    assert ill_up == EXPECTED_ILLEGAL, ("illegal unprefixed", ill_up)
    assert check_illegal(cb) == [], "cbprefixed must have no ILLEGAL entries"
    for e in up:  # the JSON gives illegal opcodes a 1 M-cycle cost; real hardware locks up instead
        if e["illegal"]:
            assert (e["m_taken"], e["m_not_taken"]) == (1, 1), ("illegal not 1 M", e)
    check_spot({"unprefixed_entries": {e["idx"]: e for e in up}, "cbprefixed_entries": {e["idx"]: e for e in cb}})

    texts = [e["mnemonic"] for e in up] + ["CB %s" % e["mnemonic"] for e in cb]
    for required in ("JP HL", "LDH (C),A", "LDH (a8),A", "LD (a16),SP"):
        assert required in texts, ("missing mnemonic text", required)
    assert any(e["idx"] == 0xF8 and e["mnemonic"] == "LD HL,SP+e8" for e in up), "F8 must read LD HL,SP+e8"

    header = (
        "// generated by gen_opcodes.py from seed/Opcodes.json, do not edit\n"
        "// SM83 opcode table for the DMG Game Boy. OPCODES is indexed by the first byte;\n"
        "// CB_OPCODES by the second byte following 0xCB (the 0xCB fetch cost is already included).\n"
        "// m_taken / m_not_taken are M-cycles (= T-cycles / 4); a fixed cost fills both fields.\n"
        "// Flags: '-' unchanged, '0'/'1' forced value, letter = computed by the instruction.\n")
    body_up = render(up)
    body_cb = render(cb)
    out = header
    out += ("pub struct OpInfo {\n"
            "    pub mnemonic: &'static str,\n"
            "    pub bytes: u8,\n"
            "    pub m_taken: u8,\n"
            "    pub m_not_taken: u8,\n"
            "    pub z: &'static str,\n"
            "    pub n: &'static str,\n"
            "    pub h: &'static str,\n"
            "    pub c: &'static str,\n"
            "    pub illegal: bool\n}\n\n")
    out += "pub const OPCODES: [OpInfo; 256] = [\n%s];\n\n" % "".join(body_up)
    out += "pub const CB_OPCODES: [OpInfo; 256] = [\n%s];\n" % "".join(body_cb)

    # Step 6 script assertions on the generated file itself.
    assert all(ord(ch) < 128 for ch in out), "generated file must be pure ASCII"
    with open(OUT, "w", encoding="ascii", newline="\n") as f:
        f.write(out)
    print("wrote %s (512 entries)" % OUT)


if __name__ == "__main__":
    sys.exit(main())
