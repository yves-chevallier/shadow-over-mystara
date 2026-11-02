# Compares the MAME coverage (words read in program space) with the static merged image.
import sys
cov = open(sys.argv[1], 'rb').read()
m = open('prog_merged.bin', 'rb').read(); d = open('prog_dec.bin', 'rb').read()
ENC = 0x100000
hit = [i * 2 for i, c in enumerate(cov) if c]
enc = [a for a in hit if a < ENC]
miss = [a for a in enc if m[a:a+2] != d[a:a+2]]
print(f"covered words: {len(hit)} (encrypted area {len(enc)}, outside {len(hit)-len(enc)})")
print(f"covered but plain in prog_merged (missed by the static pass): {len(miss)}")
runs, start, prev = [], None, None
for a in miss:
    if start is None or a != prev + 2:
        if start is not None: runs.append((start, prev))
        start = a
    prev = a
if start is not None: runs.append((start, prev))
for s, e in runs[:40]: print(f"  {s:06x}-{e+1:06x} ({(e-s)//2+1} words)")
print(f"  ... {len(runs)} ranges in total")
