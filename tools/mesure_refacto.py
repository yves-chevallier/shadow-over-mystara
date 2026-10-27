#!/usr/bin/env python3
"""Mesure de la forme 68000 du code Rust de ddsom-game, par zone (docs/refactorisation.md).

    tools/mesure_refacto.py [--zones] [--doublons N] [--consts]

Sans option : tableau par zone (répertoire de src/, ou fichier isolé) des marques de la convention 68000.
Chaque colonne compte des occurrences (motifs ci-dessous) ; un lot de refactorisation les fait baisser :

  lignes     lignes non vides
  regs       accès aux registres : `.d[n]`, `.a[n]`
  drap       drapeaux : SR_C, SR_Z, cond(), set_ccr, flags_logic, addsub, ops::
  adr        appels par adresse d'origine : call(, call_at(, .jsr(, .rom(, natif(, deleguer(, tail(
  pile       pile 68000 : a[7], Unwound, epilogue_etranger, push_return, resume!
  duree      durées du mode hybride : cost, exact_cost, Chemin, walk_cycles, path_cycles, .pris(, a_blanc, Vue
  brut       accès mémoire bruts : rd8/rd16/rd32/wr8/wr16/wr32(
  a5         variables désignées par un décalage : `A5 + 0x`, a5_u8(0x, ram16(0x...
  natives    déclarations de natives : `Native {`, gestionnaire!(, handler macro, resume!(

--doublons N : fenêtres de N lignes normalisées (littéraux remplacés) présentes dans plusieurs fichiers ;
--consts     : adresses d'origine déclarées en constante dans plusieurs fichiers.
"""
import re
import sys
from collections import defaultdict
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "port/ddsom-game/src"

MOTIFS = {
    "regs": r"\.[da]\[\d\]",
    "drap": r"SR_[CZNVX]\b|\.cond\(|set_ccr|flags_logic|\baddsub\(|ops::",
    "adr": r"\bcall\(|\bcall_at\(|\.jsr\(|\.rom\(|\.rom_d6\(|\.natif\(|\.deleguer\(|\.tail\(|jsr_origine",
    "pile": r"a\[7\]|Unwound|epilogue_etranger|push_return|resume!",
    "duree": r"\bcost\b|exact_cost|Chemin|walk_cycles|path_cycles|\.pris\(|a_blanc|\bVue\b",
    "brut": r"\b(rd8|rd16|rd32|wr8|wr16|wr32)\(",
    "a5": r"A5 \+ 0x|A5\.wrapping_add|\b(a5_?u(8|16|32)|set_a5_?u(8|16|32)|ram(8|16|32)|set_ram(8|16|32))\(0x",
    "natives": r"Native \{|gestionnaire!\(|handlers!|resume!\(",
}
COLONNES = ["lignes"] + list(MOTIFS)


def zone(p: Path) -> str:
    rel = p.relative_to(SRC)
    return rel.parts[0] if len(rel.parts) > 1 else rel.stem


def code(p: Path):
    """Lignes de code (sans commentaires de ligne ni lignes vides)."""
    for l in p.read_text().splitlines():
        s = l.strip()
        if s and not s.startswith("//"):
            yield s


def zones():
    tot = defaultdict(lambda: defaultdict(int))
    for p in sorted(SRC.rglob("*.rs")):
        z = zone(p)
        for s in code(p):
            tot[z]["lignes"] += 1
            for k, m in MOTIFS.items():
                tot[z][k] += len(re.findall(m, s))
    somme = defaultdict(int)
    print("| zone | " + " | ".join(COLONNES) + " |")
    print("|---|" + "---:|" * len(COLONNES))
    for z, v in sorted(tot.items(), key=lambda kv: -kv[1]["lignes"]):
        for k in COLONNES:
            somme[k] += v[k]
        print(f"| {z} | " + " | ".join(str(v[k]) for k in COLONNES) + " |")
    print("| **total** | " + " | ".join(f"**{somme[k]}**" for k in COLONNES) + " |")


def normalise(s: str) -> str:
    s = re.sub(r"0x[0-9a-fA-F_]+|\b\d+\b", "N", s)
    s = re.sub(r'"[^"]*"', "S", s)
    return s


def doublons(n: int):
    """Fenêtres de n lignes normalisées vues dans au moins deux fichiers ; lignes couvertes par zone."""
    vues = defaultdict(set)
    fichiers = {}
    for p in sorted(SRC.rglob("*.rs")):
        lignes = [normalise(s) for s in code(p)]
        fichiers[p] = lignes
        for i in range(len(lignes) - n + 1):
            fen = lignes[i : i + n]
            # fenêtres triviales (accolades, `}` seules) ignorées
            if sum(len(x) > 3 for x in fen) < n - 1:
                continue
            vues["\n".join(fen)].add(p)
    multiples = {k for k, v in vues.items() if len(v) > 1}
    couvert = defaultdict(int)
    total = defaultdict(int)
    for p, lignes in fichiers.items():
        marque = [False] * len(lignes)
        for i in range(len(lignes) - n + 1):
            if "\n".join(lignes[i : i + n]) in multiples:
                for j in range(i, i + n):
                    marque[j] = True
        couvert[zone(p)] += sum(marque)
        total[zone(p)] += len(lignes)
    print(f"Lignes de code couvertes par une fenêtre de {n} lignes (littéraux normalisés) présente dans un autre fichier :")
    print("| zone | lignes | dupliquées | % |")
    print("|---|---:|---:|---:|")
    st = sc = 0
    for z in sorted(total, key=lambda z: -couvert[z]):
        st += total[z]
        sc += couvert[z]
        print(f"| {z} | {total[z]} | {couvert[z]} | {100 * couvert[z] / max(total[z], 1):.0f} |")
    print(f"| **total** | **{st}** | **{sc}** | **{100 * sc / st:.0f}** |")


def consts():
    """Adresses (>= 0x1000, < 0x100000 : routines d'origine) déclarées en constante dans plusieurs fichiers."""
    par_valeur = defaultdict(set)
    for p in sorted(SRC.rglob("*.rs")):
        for s in code(p):
            m = re.match(r"(?:pub(?:\(crate\))? )?const (\w+): u32 = 0x([0-9a-fA-F_]+);", s)
            if m:
                v = int(m.group(2).replace("_", ""), 16)
                if 0x400 <= v < 0x100000:
                    par_valeur[v].add((m.group(1), str(p.relative_to(SRC))))
    multi = {v: e for v, e in par_valeur.items() if len({f for _, f in e}) > 1}
    decl = sum(len(e) for e in multi.values())
    print(f"{len(multi)} adresses déclarées dans plusieurs fichiers ({decl} déclarations). Les plus répétées :")
    for v, e in sorted(multi.items(), key=lambda kv: -len(kv[1]))[:25]:
        noms = sorted({n for n, _ in e})
        print(f"  {v:#07x} ×{len(e):2}  {', '.join(noms[:6])}{' ...' if len(noms) > 6 else ''}")


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a or "--zones" in a:
        zones()
    if "--doublons" in a:
        doublons(int(a[a.index("--doublons") + 1]))
    if "--consts" in a:
        consts()
