# Cherche dans un dump RAM (FF0000-FFFFFF, une image de 64 Ko par frame) les mots 16 bits
# dont la variation suit une fenêtre d'entrée : usage ramdiff.py ram.bin premiere_frame debut fin [signe]
import sys, struct
path, first, a, b = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
sign = int(sys.argv[5]) if len(sys.argv) > 5 else -1
data = open(path, 'rb').read()
N = len(data) // 0x10000
fr = lambda f: data[(f - first) * 0x10000:(f - first + 1) * 0x10000]
def words(f): return struct.unpack('>32768H', fr(f))
inside = [words(f) for f in range(a, b + 1)]
before = [words(f) for f in range(max(first, a - 60), a)]
res = []
for i in range(32768):
    seq = [w[i] for w in inside]
    d = [((y - x + 0x8000) & 0xffff) - 0x8000 for x, y in zip(seq, seq[1:])]
    moving = sum(1 for v in d if v * sign > 0)
    wrong = sum(1 for v in d if v * sign < 0)
    still = len(set(w[i] for w in before)) == 1 if before else True
    if moving > len(d) * float(sys.argv[6] if len(sys.argv) > 6 else 0.5) and wrong == 0:
        res.append((moving, still, 0xff0000 + 2 * i, seq[0], seq[-1]))
res.sort(reverse=True)
for m, s, addr, v0, v1 in res[:25]:
    print(f"{addr:06x}  {v0:04x} -> {v1:04x}  ({m} frames en mouvement, immobile avant: {s})")
