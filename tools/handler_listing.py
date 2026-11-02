#!/usr/bin/env python3
"""Readable listing of the reachable code of a 68000 handler, with jump tables resolved.

    python3 tools/handler_listing.py <start> <end> [extra_entry ...]     (addresses in hexadecimal)

Prepares the rewrite of an object type: follows the code by recursive descent (`examples/dis_flow`, which
also follows the `lea (6,PC),A6` + `jmp (A6)` of preambles) and prints each instruction with the text of
`listing.txt` when it knows it (otherwise the decoder's `Debug` form, integers in hexadecimal, marked `??`).
Then, for each `jmp/jsr (table,PC,Dn)`, the table: `offset:target`, `!` for a target that is not a
known instruction. **A table stops at the first code that follows it**: the number of entries is
(address of the first code - base) / 2, entries beyond that are code bytes.

Environment variables: DDSOM_ROMS (default `rom`), SOM_ROOT (main repository folder, holding
`listing.txt` and `prog_dec.bin`: default the current folder). Prerequisite: `cargo build --release -p ddsom-game
--examples`. `(table,PC,Dn)` reads are done in `prog_dec.bin` (opcode space); the other
table reads (A4 loaded by `lea`) read the plain data (`Cx::data8/data16`).
"""
import os
import re
import struct
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from dis_fmt import fmt  # noqa: E402

root = os.environ.get("SOM_ROOT", ".")
roms = os.environ.get("DDSOM_ROMS", "rom")
here = os.path.dirname(os.path.abspath(__file__))
dis_flow = os.path.join(here, "..", "port", "target", "release", "examples", "dis_flow")
lo, hi = sys.argv[1], sys.argv[2]
entries = set([lo] + sys.argv[3:])
dec = open(os.path.join(root, "prog_dec.bin"), "rb").read()
listing = {}
for line in open(os.path.join(root, "listing.txt")):
    parts = line.split(None, 1)
    if len(parts) == 2 and re.fullmatch(r"[0-9a-f]{6}", parts[0]):
        listing[parts[0]] = parts[1].rstrip()


def run_dis(ents):
    return subprocess.run([dis_flow, roms, lo, hi] + sorted(ents), capture_output=True, text=True).stdout


while True:
    out = run_dis(entries)
    new = set()
    for line in out.splitlines():
        m = re.match(r"([0-9a-f]{6})  ", line)
        if not m:
            continue
        mm = re.match(r"lea \(0x([0-9a-f]+),PC\),A6", listing.get(m.group(1), ""))
        if mm:
            new.add("%x" % (int(m.group(1), 16) + 2 + int(mm.group(1), 16)))
    if new <= entries:
        break
    entries |= new

addrs = {}
for line in out.splitlines():
    m = re.match(r"([0-9a-f]{6})  ([0-9a-f]{4})  (.*)", line)
    if m:
        addrs[m.group(1)] = m.group(3)
text = {}
for a in sorted(addrs):
    if a in listing:
        text[a] = listing[a]
    else:
        text[a] = fmt(re.sub(r"\b\d+\b", lambda m: hex(int(m.group(0))), addrs[a])) + "   ; (not in listing)"
    print(a, text[a])
print("---- tables (jmp/jsr (x,PC,Dn))")
bases = set()
for a in sorted(addrs):
    m = re.match(r"(?:j(?:mp|sr)|move\.w) \(0x([0-9a-f]+),PC,", text.get(a, ""))
    if m:
        bases.add(int(m.group(1), 16))
for base in sorted(bases):
    ents = []
    at = base
    end = 1 << 30
    while at < end and at < base + 0x100:
        target = base + struct.unpack(">h", dec[at:at + 2])[0]
        ok = target % 2 == 0 and target >= base and "%06x" % target in addrs
        if ok and at < target < end:
            end = target
        ents.append("%d:%x%s" % (len(ents) * 2, target, "" if ok else "!"))
        at += 2
    print("TABLE %06x:" % base, " ".join(ents))
