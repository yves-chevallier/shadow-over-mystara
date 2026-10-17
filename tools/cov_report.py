"""Fusionne les bitmaps de couverture de l'exemple `cov` et affiche un rapport.

    python3 tools/cov_report.py [--index decomp/index.tsv] [--listing listing.txt] [--regions] A.bin B.bin ...

Chaque .bin : 1 bit par mot du programme 0x000000-0x0FFFFF (instruction exécutée à cette adresse).
Le premier fichier est la référence (« bot initial ») ; le rapport donne pour chacun et pour
l'union : instructions distinctes, fonctions de index.tsv touchées (au moins une instruction),
part des instructions du listing (dénominateur approximatif : toutes les lignes d'instruction de
listing.txt, données mal désassemblées comprises).
"""
import argparse
import os
import re
import sys


def load(path):
    return bytearray(open(path, "rb").read())


def bit(bm, addr):
    w = addr >> 1
    return bm[w >> 3] >> (w & 7) & 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("--index", default="/home/ycr/som/decomp/index.tsv")
    ap.add_argument("--listing", default="/home/ycr/som/listing.txt")
    ap.add_argument("--regions", action="store_true", help="carte par blocs de 4 Ko")
    ap.add_argument("--missed", metavar="OUT", help="liste des fonctions de index.tsv jamais touchées")
    a = ap.parse_args()

    funcs = []
    if os.path.exists(a.index):
        for l in open(a.index).read().splitlines()[1:]:
            v = l.split("\t")
            funcs.append((int(v[0], 16), v[1], int(v[2])))
    funcs.sort()
    listing = set()
    if os.path.exists(a.listing):
        pat = re.compile(r"^([0-9a-f]{6})  \S")
        for l in open(a.listing, errors="replace"):
            m = pat.match(l)
            if m:
                listing.add(int(m.group(1), 16))

    maps = [(os.path.basename(f), load(f)) for f in a.files]
    union = bytearray(len(maps[0][1]))
    for _, bm in maps:
        for i, b in enumerate(bm):
            union[i] |= b
    maps.append(("UNION", union))

    def stats(bm):
        n = sum(bin(b).count("1") for b in bm)
        fh = 0
        for addr, _, size in funcs:
            if any(bit(bm, x) for x in range(addr, addr + size, 2)):
                fh += 1
        inl = sum(1 for x in listing if bit(bm, x))
        return n, fh, inl

    print(f"{'scénario':32} {'instr.':>8} {'fonctions':>10} {'% listing':>10}")
    for name, bm in maps:
        n, fh, inl = stats(bm)
        pct = 100.0 * inl / len(listing) if listing else 0
        print(f"{name[:32]:32} {n:8d} {fh:5d}/{len(funcs):<4d} {pct:9.1f}")
    if a.regions:
        print("\nCouverture de l'union par bloc de 4 Ko (instructions exécutées / lignes du listing) :")
        for base in range(0, 0x100000, 0x1000):
            tot = sum(1 for x in listing if base <= x < base + 0x1000)
            hit = sum(1 for x in listing if base <= x < base + 0x1000 and bit(union, x))
            if tot:
                print(f"  {base:06x} {hit:5d}/{tot:<5d} {100.0 * hit / tot:5.1f} %")
    if a.missed:
        with open(a.missed, "w") as f:
            for addr, name, size in funcs:
                if not any(bit(union, x) for x in range(addr, addr + size, 2)):
                    f.write(f"{addr:06x} {name} {size}\n")


main()
