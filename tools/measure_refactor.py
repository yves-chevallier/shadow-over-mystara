#!/usr/bin/env python3
"""Measures the 68000 shape of the ddsom-game Rust code, per zone (docs/refactorisation.md).

    tools/measure_refactor.py [--zones] [--doublons N] [--consts]

No option: table per zone (src/ directory, or single file) of the 68000 convention marks.
Each column counts occurrences (patterns below); a refactoring batch brings them down:

  lines      non-empty lines
  regs       register accesses: `.d[n]`, `.a[n]`
  flags      flags: SR_C, SR_Z, cond(), set_ccr, flags_logic, addsub, ops::
  addr       calls by original address: call(, call_at(, .jsr(, .rom(, natif(, deleguer(, tail(
  stack      68000 stack: a[7], Unwound, epilogue_etranger, push_return, resume!
  timing     hybrid mode durations: cost, exact_cost, Chemin, walk_cycles, path_cycles, .pris(, a_blanc, Vue
  raw        raw memory accesses: rd8/rd16/rd32/wr8/wr16/wr32(
  a5         variables designated by an offset: `A5 + 0x`, a5_u8(0x, ram16(0x...
  natives    native declarations: `Native {`, gestionnaire!(, handler macro, resume!(

--doublons N: windows of N normalized lines (literals replaced) present in several files;
--consts    : original addresses declared as a constant in several files.
"""
import re
import sys
from collections import defaultdict
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "port/ddsom-game/src"

# The patterns match Rust identifiers of the port (some still in French).
PATTERNS = {
    "regs": r"\.[da]\[\d\]",
    "flags": r"SR_[CZNVX]\b|\.cond\(|set_ccr|flags_logic|\baddsub\(|ops::",
    "addr": r"\bcall\(|\bcall_at\(|\.jsr\(|\.rom\(|\.rom_d6\(|\.natif\(|\.deleguer\(|\.tail\(|jsr_origine",
    "stack": r"a\[7\]|Unwound|epilogue_etranger|push_return|resume!",
    "timing": r"\bcost\b|exact_cost|Chemin|walk_cycles|path_cycles|\.pris\(|a_blanc|\bVue\b",
    "raw": r"\b(rd8|rd16|rd32|wr8|wr16|wr32)\(",
    "a5": r"A5 \+ 0x|A5\.wrapping_add|\b(a5_?u(8|16|32)|set_a5_?u(8|16|32)|ram(8|16|32)|set_ram(8|16|32))\(0x",
    "natives": r"Native \{|gestionnaire!\(|handlers!|resume!\(",
}
COLUMNS = ["lines"] + list(PATTERNS)


def zone(p: Path) -> str:
    rel = p.relative_to(SRC)
    return rel.parts[0] if len(rel.parts) > 1 else rel.stem


def code(p: Path):
    """Code lines (without line comments or empty lines)."""
    for l in p.read_text().splitlines():
        s = l.strip()
        if s and not s.startswith("//"):
            yield s


def zones():
    tot = defaultdict(lambda: defaultdict(int))
    for p in sorted(SRC.rglob("*.rs")):
        z = zone(p)
        for s in code(p):
            tot[z]["lines"] += 1
            for k, m in PATTERNS.items():
                tot[z][k] += len(re.findall(m, s))
    total = defaultdict(int)
    print("| zone | " + " | ".join(COLUMNS) + " |")
    print("|---|" + "---:|" * len(COLUMNS))
    for z, v in sorted(tot.items(), key=lambda kv: -kv[1]["lines"]):
        for k in COLUMNS:
            total[k] += v[k]
        print(f"| {z} | " + " | ".join(str(v[k]) for k in COLUMNS) + " |")
    print("| **total** | " + " | ".join(f"**{total[k]}**" for k in COLUMNS) + " |")


def normalize(s: str) -> str:
    s = re.sub(r"0x[0-9a-fA-F_]+|\b\d+\b", "N", s)
    s = re.sub(r'"[^"]*"', "S", s)
    return s


def duplicates(n: int):
    """Windows of n normalized lines seen in at least two files; lines covered per zone."""
    seen = defaultdict(set)
    files = {}
    for p in sorted(SRC.rglob("*.rs")):
        lines = [normalize(s) for s in code(p)]
        files[p] = lines
        for i in range(len(lines) - n + 1):
            win = lines[i : i + n]
            # trivial windows (braces, lone `}`) ignored
            if sum(len(x) > 3 for x in win) < n - 1:
                continue
            seen["\n".join(win)].add(p)
    multiple = {k for k, v in seen.items() if len(v) > 1}
    covered = defaultdict(int)
    total = defaultdict(int)
    for p, lines in files.items():
        mark = [False] * len(lines)
        for i in range(len(lines) - n + 1):
            if "\n".join(lines[i : i + n]) in multiple:
                for j in range(i, i + n):
                    mark[j] = True
        covered[zone(p)] += sum(mark)
        total[zone(p)] += len(lines)
    print(f"Code lines covered by a window of {n} lines (normalized literals) present in another file:")
    print("| zone | lines | duplicated | % |")
    print("|---|---:|---:|---:|")
    st = sc = 0
    for z in sorted(total, key=lambda z: -covered[z]):
        st += total[z]
        sc += covered[z]
        print(f"| {z} | {total[z]} | {covered[z]} | {100 * covered[z] / max(total[z], 1):.0f} |")
    print(f"| **total** | **{st}** | **{sc}** | **{100 * sc / st:.0f}** |")


def consts():
    """Addresses (>= 0x1000, < 0x100000: original routines) declared as a constant in several files."""
    by_value = defaultdict(set)
    for p in sorted(SRC.rglob("*.rs")):
        for s in code(p):
            m = re.match(r"(?:pub(?:\(crate\))? )?const (\w+): u32 = 0x([0-9a-fA-F_]+);", s)
            if m:
                v = int(m.group(2).replace("_", ""), 16)
                if 0x400 <= v < 0x100000:
                    by_value[v].add((m.group(1), str(p.relative_to(SRC))))
    multi = {v: e for v, e in by_value.items() if len({f for _, f in e}) > 1}
    decl = sum(len(e) for e in multi.values())
    print(f"{len(multi)} addresses declared in several files ({decl} declarations). The most repeated:")
    for v, e in sorted(multi.items(), key=lambda kv: -len(kv[1]))[:25]:
        names = sorted({n for n, _ in e})
        print(f"  {v:#07x} ×{len(e):2}  {', '.join(names[:6])}{' ...' if len(names) > 6 else ''}")


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a or "--zones" in a:
        zones()
    if "--doublons" in a:
        duplicates(int(a[a.index("--doublons") + 1]))
    if "--consts" in a:
        consts()
