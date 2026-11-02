#!/usr/bin/env python3
"""ROM reads of the game sorted by reader: reference measure of the data batches (docs/fin.md, L0).

    tools/rom_readers.py list [frames]           scenarios as `rom_reads` takes them (`path:frames`), like
                                                 tools/validate_all.sh with DDSOM_SCENES=1 DDSOM_COV=1 DDSOM_LONG=1
    tools/rom_readers.py report OUT.txt...       words read per reader (Rust file) and per zone of docs/fin.md
    tools/rom_readers.py zones FILE OUT.txt...   zones read by one reader file (`objects/anim.rs`...)

The outputs come from `rom_reads` run with `DDSOM_READERS=1` (every word read gets its first caller in
`ddsom-game/src`). Measure (about 40 min with 10 processes):

    cd port && CARGO_TARGET_DIR=target/trace cargo build --release -p ddsom-game --example rom_reads --features rom-trace
    ../tools/rom_readers.py list | split -n r/10 - $SCRATCH/chunk_
    for c in $SCRATCH/chunk_*; do DDSOM_READERS=1 target/trace/release/examples/rom_reads <roms> <eeprom> \\
        $c.out $(cat $c) & done; wait
    ../tools/rom_readers.py report $SCRATCH/chunk_*.out
"""
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
from measure_end import ZONES, zone_of  # noqa: E402

# Generic accessors: the reader is the first caller outside them.
ACCESSORS = ("mem.rs", "families/cx.rs", "objects/current.rs", "machine.rs")


def scenarios(frames: int):
    s = ROOT / "scenarios"

    def own(d):
        for p in sorted((s / d).glob("*.txt")):
            yield p, int(p.with_suffix(".images").read_text())

    def fixed(d, n):
        for p in sorted((s / d).glob("*.txt") if d else s.glob("*.txt")):
            yield p, n

    yield from own("long")
    yield from fixed("", frames)
    for d in ("extra", "boss", "enemy", "modes"):
        yield from fixed(d, 12000)
    yield from fixed("scene", 6500)
    yield from fixed("scene_end", 9000)
    yield from fixed("cov", 30000)


def reads(outputs):
    """(space, address) -> reader file, over all the outputs (the first reader seen wins)."""
    out = {}
    for f in outputs:
        for line in Path(f).read_text().splitlines():
            if not line.startswith("s "):
                continue
            _, space, address, *sites = line.split(" ", 3)
            sites = sites[0].split(";") if sites else [""]
            files = [s.rsplit(":", 1)[0] for s in sites if s]
            reader = next((f for f in files if f not in ACCESSORS), files[0] if files else "?")
            out.setdefault((space, int(address, 16)), reader)
    return out


def report(outputs):
    words = reads(outputs)
    by_reader = defaultdict(int)
    by_zone = defaultdict(int)
    for (_, _), reader in words.items():
        by_reader[reader] += 1
        by_zone[zone_of(reader) if reader.endswith(".rs") else "?"] += 1
    print(f"{len(words)} words read ({2 * len(words)} bytes)\n")
    print("| zone | words |")
    print("|---|---:|")
    for name, _ in ZONES:
        if by_zone.get(name):
            print(f"| {name} | {by_zone[name]} |")
    if by_zone.get("?"):
        print(f"| ? | {by_zone['?']} |")
    print("\n| reader | words |")
    print("|---|---:|")
    for r, n in sorted(by_reader.items(), key=lambda kv: -kv[1]):
        print(f"| {r} | {n} |")


def zones(reader, outputs):
    words = sorted(k for k, r in reads(outputs).items() if r == reader)
    start = prev = None
    for space, a in words + [(None, None)]:
        if start and (space != start[0] or a != prev + 2):
            print(f"{start[0]} {start[1]:06x} {prev + 2:06x}")
            start = None
        if space and not start:
            start = (space, a)
        prev = a


if __name__ == "__main__":
    a = sys.argv[1:]
    if a and a[0] == "list":
        for p, n in scenarios(int(a[1]) if len(a) > 1 else 30000):
            print(f"{p}:{n}")
    elif a and a[0] == "report":
        report(a[1:])
    elif a and a[0] == "zones":
        zones(a[1], a[2:])
    else:
        print(__doc__)
