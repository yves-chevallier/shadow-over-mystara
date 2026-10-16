# Concatène les ROMs programme CPS2 (ROM_LOAD16_WORD_SWAP) en une image 68000 big-endian (chiffrée pour les opcodes).
import sys, pathlib
romdir = pathlib.Path(sys.argv[1]); out = sys.argv[2]
names = ["dd2e.03e","dd2e.04e","dd2e.05e","dd2e.06e","dd2e.07","dd2e.08","dd2e.09","dd2e.10"]
img = bytearray()
for n in names:
    d = (romdir/n).read_bytes()
    img += bytes(b for i in range(0, len(d), 2) for b in (d[i+1], d[i]))
open(out, "wb").write(img)
print(len(img))
