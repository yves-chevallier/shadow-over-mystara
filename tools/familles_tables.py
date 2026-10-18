"""familles_tables.py : adresses des gestionnaires des familles ESHL, SET, SOBJ (tables de sauts
du dispatch, lues dans prog_dec.bin) ; `familles_tables.py tsv` écrit handlers.tsv (pour `couverture`)."""
import os, sys
SRC = os.environ.get('DDSOM_SRC', '/home/ycr/som')
d = open(SRC + '/prog_dec.bin', 'rb').read()
T = {'ESHL': (0x39b96, 64), 'SET': (0x39eac, 34), 'SOBJ': (0x39fa8, 16)}


def tab(base, n):
    return [int.from_bytes(d[base + 4 * i:base + 4 * i + 4], 'big') for i in range(n)]


if __name__ == '__main__':
    for k, (b, n) in T.items():
        print(k, ' '.join(f"{i}:{a:x}" for i, a in enumerate(tab(b, n))))
    if len(sys.argv) > 1 and sys.argv[1] == 'tsv':
        with open('handlers.tsv', 'w') as f:
            f.write('adresse\n')
            for k, (b, n) in T.items():
                for a in sorted(set(tab(b, n))):
                    f.write(f"{a:06x}\n")
