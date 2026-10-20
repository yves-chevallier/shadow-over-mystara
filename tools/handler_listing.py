#!/usr/bin/env python3
"""Listing lisible du code atteignable d'un gestionnaire 68000, tables de sauts résolues.

    python3 tools/handler_listing.py <début> <fin> [entrée_supplémentaire ...]     (adresses en hexadécimal)

Prépare la réécriture d'un type d'objet : suit le code par descente récursive (`examples/dis_flow`, qui
suit aussi les `lea (6,PC),A6` + `jmp (A6)` des préambules) et affiche chaque instruction avec le texte de
`listing.txt` quand il le connaît (sinon la forme `Debug` du décodeur, entiers en hexadécimal, marquée `??`).
Puis, pour chaque `jmp/jsr (table,PC,Dn)`, la table : `décalage:cible`, `!` pour une cible qui n'est pas une
instruction connue. **Une table s'arrête au premier code qui la suit** : le nombre d'entrées est
(adresse du premier code - base) / 2, les entrées au-delà sont des octets de code.

Variables d'environnement : DDSOM_ROMS (défaut `rom`), SOM_ROOT (dossier du dépôt principal, qui contient
`listing.txt` et `prog_dec.bin` : défaut le dossier courant). Prérequis : `cargo build --release -p ddsom-game
--examples`. Les lectures `(table,PC,Dn)` se font dans `prog_dec.bin` (espace des opcodes) ; les autres
lectures de tables (A4 chargé par `lea`) lisent les données en clair (`Cx::data8/data16`).
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
        text[a] = fmt(re.sub(r"\b\d+\b", lambda m: hex(int(m.group(0))), addrs[a])) + "   ; (hors listing)"
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
