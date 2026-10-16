# Mots RAM qui montent pendant la fenêtre A et descendent pendant la fenêtre B (et immobiles hors fenêtres).
# usage : ramcorr.py ram.bin premiere_frame a0 a1 b0 b1
import sys, struct
p, first, a0, a1, b0, b1 = sys.argv[1], *map(int, sys.argv[2:7])
d = open(p, 'rb').read()
W = lambda f: struct.unpack('>32768H', d[(f - first) * 0x10000:(f - first + 1) * 0x10000])
s = lambda x, y: ((y - x + 0x8000) & 0xffff) - 0x8000
A0, A1, B0, B1 = W(a0), W(a1), W(b0), W(b1)
out = []
for i in range(32768):
    da, db = s(A0[i], A1[i]), s(B0[i], B1[i])
    if da > 0 and db < 0:
        out.append((min(da, -db), 0xff0000 + 2 * i, A0[i], A1[i], B1[i]))
out.sort(reverse=True)
for m, addr, x, y, z in out[:30]:
    print(f"{addr:06x}  {x:04x} -> {y:04x} -> {z:04x}")
