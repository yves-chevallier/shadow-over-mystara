#!/usr/bin/env python3
"""dis_plage.py DÉBUT FIN (hexa) : listing.txt sur une plage, tables de sauts `(tbl,PC,Dn)` résolues.

Le listing désassemble prog_dec.bin et est incomplet par endroits (désalignements) : les tables
de données y apparaissent comme des instructions absurdes (`ori.b #0x0,D0b`...). Pour une plage
douteuse, utiliser `examples/dis` (décodeur de l'interpréteur). Variables : DDSOM_SRC (dépôt
principal contenant listing.txt et prog_dec.bin, défaut /home/ycr/som).
"""
import os
import sys, re
SRC = os.environ.get('DDSOM_SRC', '/home/ycr/som')
lst = open(SRC+'/listing.txt').read().split('\n')
plain = open(SRC+'/prog_dec.bin','rb').read()
def w(a): return int.from_bytes(plain[a:a+2],'big')
def sw(a):
    v=w(a); return v-0x10000 if v&0x8000 else v
s=int(sys.argv[1],16); e=int(sys.argv[2],16)
rows=[]
for ln in lst:
    m=re.match(r'^([0-9a-f]{6})  (.*)$',ln)
    if m:
        a=int(m.group(1),16)
        if s<=a<e: rows.append((a,m.group(2)))
tabs={}
for a,t in rows:
    m=re.search(r'\(0x([0-9a-f]+),PC,D(\d)w\*0x1\)',t)
    if m and ('jmp' in t or 'jsr' in t or 'move.w' in t):
        tabs.setdefault(int(m.group(1),16),[]).append(a)
skip=set()
for tb in sorted(tabs):
    targets=[]
    n=0
    while True:
        t=tb+sw(tb+2*n)
        if t<tb or t>tb+0x4000: break
        targets.append(t); n+=1
        if tb+2*n >= min(targets): break
        if n>64: break
    if len(targets)>0 and all(t==tb+sw(tb) for t in targets[:1]) and False:
        pass
    if n and targets and tb+2*n<=min(targets) and 'jmp' in ''.join(t for a,t in rows if a in tabs[tb]) + ''.join(t for a,t in rows if a in tabs[tb]) or True:
        print(f"; TABLE {tb:06x}: " + ' '.join(f"{i*2:x}->{t:06x}" for i,t in enumerate(targets)))
        for a,_ in rows:
            if tb<=a<tb+2*n: skip.add(a)
for a,t in rows:
    if a in skip: continue
    print(f"{a:06x}  {t}")
