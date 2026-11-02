#!/usr/bin/env python3
"""Bisection of a poisoned validation (`DDSOM_POISON`, docs/guide-refactorisation.md, dead registers section).

    tools/bisect_poison.py <validate> <scenario> <frames> <poison> [guards...]

Runs the scenario with the `poison` list and the poisoning trace (DDSOM_POISON_TRACE); if it fails, searches
by bisection for the responsible "label@site" entry among the sites poisoned before the mismatch, then the
register(s) read back (N/Z/V/C and X separate), spares it ("label@site#registers") and starts again until VALID.
Prints each reader found, then the "GUARDS" line of the entries to add. The `=…#…` entries of the starting list
(registers spared without poisoning) apply to every trial.

Variables: DDSOM_ROMS (rom), DDSOM_EEPROM (testdata/eeprom_ddsom.bin); the scenario is relative to the repository root.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ROM = os.environ.get("DDSOM_ROMS", os.path.join(ROOT, "rom"))
EEPROM = os.environ.get("DDSOM_EEPROM", os.path.join(ROOT, "testdata/eeprom_ddsom.bin"))
REGS = [f"d{i}" for i in range(8)] + [f"a{i}" for i in (0, 1, 2, 3, 4, 6)] + ["nzvc", "x"]


def run(val, scen, frames, poison, trace=False):
    env = dict(os.environ)
    env["DDSOM_POISON"] = poison
    if trace:
        env["DDSOM_POISON_TRACE"] = "1"
    else:
        env.pop("DDSOM_POISON_TRACE", None)
    return subprocess.run([val, ROM, EEPROM, scen, str(frames)], cwd=ROOT, env=env, capture_output=True, text=True)


def verdict(r):
    return r.stdout.strip().split("\n")[-1]


def main():
    val, scen, frames, base = sys.argv[1:5]
    guards = list(sys.argv[5:])
    base_guards = [e for e in base.split(",") if "#" in e]
    limit = [frames]

    def fails(entries):
        if not entries:
            return False
        return not verdict(run(val, scen, limit[0], ",".join(entries + base_guards))).startswith("VALID")

    while True:
        limit[0] = frames
        r = run(val, scen, frames, ",".join([base] + guards), trace=True)
        v = verdict(r)
        print("start:", v, flush=True)
        if v.startswith("VALID"):
            break
        m = re.search(r"(?:image|frame) (\d+)", v)
        if m:
            limit[0] = int(m.group(1)) + 1
        candidates = []
        for line in r.stderr.split("\n"):
            m = re.match(r"POISON (\S+) (\S+)", line)
            if m and f"{m.group(1)}@{m.group(2)}" not in candidates:
                candidates.append(f"{m.group(1)}@{m.group(2)}")
        print(len(candidates), "candidates", flush=True)
        if not fails(candidates + guards):
            print("mismatch not reproduced by the candidates alone", flush=True)
            break
        low, high = 0, len(candidates)
        while high - low > 1:
            mid = (low + high) // 2
            if fails(candidates[:mid] + guards):
                high = mid
            else:
                low = mid
        culprit = candidates[high - 1]
        read = [reg for reg in REGS
                if fails(candidates[:high - 1] + [f"{culprit}#{'+'.join(x for x in REGS if x != reg)}"] + guards)]
        if read:
            print("reader:", culprit, "registers read:", "+".join(read), flush=True)
            guards.append(f"{culprit}#{'+'.join(read)}")
        else:
            print("reader:", culprit, "(combination of registers)", flush=True)
            guards.append("!" + culprit)
    print("GUARDS", " ".join(guards))


if __name__ == "__main__":
    main()
