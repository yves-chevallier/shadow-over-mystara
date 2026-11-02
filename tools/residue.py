#!/usr/bin/env python3
"""Reads of the residue (program ROM bytes that no typed table describes yet) over a validation.

    tools/residue.py LOG...              reads per missing key (table, key): reads, scenarios
    tools/residue.py --sprites LOG...    port/extract/code_sprites.txt completed with the frames, animation sets and
                                         entries that the code designates and the tables lack (the file only grows:
                                         a key it lists is no longer read in the residue)

The logs are those of `validate` (line `residue: ...`), for example `~/.cache/ddsom/validate/<hash>/*.log` after
`tools/validate_all.sh`. See docs/fin.md (`Residue`) and port/ddsom-game/src/data.rs.
"""
import re
import sys
from collections import Counter

ROM_END = 0x400000
CODE_SPRITES = __import__("pathlib").Path(__file__).resolve().parent.parent / "port/extract/code_sprites.txt"


def keys(logs):
    reads, scenarios = Counter(), Counter()
    for log in logs:
        for line in open(log):
            if line.startswith("residue:"):
                for table, key, n in re.findall(r", ([a-z ]+) (0x[0-9a-f]+) ×(\d+)", line):
                    reads[(table, int(key, 16))] += int(n)
                    scenarios[(table, int(key, 16))] += 1
    return reads, scenarios


def main(a):
    sprites = "--sprites" in a
    logs = [x for x in a if not x.startswith("--")]
    reads, scenarios = keys(logs)
    if sprites:
        known = set()
        if CODE_SPRITES.exists():
            for line in CODE_SPRITES.read_text().splitlines():
                if line and not line.startswith("#"):
                    kind, key = line.split()
                    known.add((kind, int(key, 16)))
        for table, key in reads:
            kind = {"frame": "frame", "anim set": "set", "anim entry": "entry"}.get(table)
            if kind and key < ROM_END:
                known.add((kind, key))
        lines = ["# kind address: frames, animation sets and entries designated by the code alone (measured: residue",
                 "# reads of the validation) — completed by tools/residue.py --sprites, do not edit"]
        lines += [f"{kind} {key:06x}" for kind, key in sorted(known)]
        CODE_SPRITES.write_text("\n".join(lines) + "\n")
        print(f"{CODE_SPRITES}: {len(known)} keys")
        return
    print(f"{sum(reads.values())} reads, {len(reads)} keys")
    for (table, key), n in sorted(reads.items()):
        print(f"{table:16} {key:#08x} {n:10} {scenarios[(table, key)]:4}")


if __name__ == "__main__":
    main(sys.argv[1:])
