"""Measures the 68000 instruction coverage of scenarios (example `cov`, in parallel).

    python3 tools/run_cov.py <output_dir> scenarios/a.txt scenarios/b.txt ...

Writes <output_dir>/<name>.bin (bitmap); merge and report: tools/cov_report.py.
DDSOM_ROMS / DDSOM_EEPROM as for validate_all.sh; JOBS = number of processes.
"""
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.join(os.path.dirname(__file__), "..")
COV = os.path.join(ROOT, "port/target/release/examples/cov")
ROMS = os.environ.get("DDSOM_ROMS", os.path.join(ROOT, "rom"))
EEPROM = os.environ.get("DDSOM_EEPROM", os.path.join(ROOT, "testdata/eeprom_ddsom.bin"))
FRAMES = os.environ.get("FRAMES", "30000")

out = sys.argv[1]
os.makedirs(out, exist_ok=True)


def run(sc):
    name = os.path.basename(sc)[:-4]
    r = subprocess.run([COV, ROMS, EEPROM, sc, FRAMES, os.path.join(out, name + ".bin")], capture_output=True, text=True)
    return name, r.stdout.strip() or r.stderr.strip()


with ThreadPoolExecutor(int(os.environ.get("JOBS", "6"))) as ex:
    for name, msg in ex.map(run, sys.argv[2:]):
        print(f"{name:28} {msg}")
