#!/usr/bin/env python3
"""Convertit les appels de sons par adresse en appels du cœur `sound` (lot S, docs/guide-refactorisation.md).

    tools/convertir_sons.py fichier.rs...      (chemins relatifs à port/ddsom-game/src ou absolus)

Motifs reconnus, quand l'argument désigne un son (wrapper, aiguillage, entrée « selon la version ») ou une commande
de service, par un littéral ou une constante résolue (même fichier, sinon nom unique dans le crate) :

  x.call(SON)            (gestionnaire `Cx` de families/)  ->  crate::sound::jouer(x.b, SON, x.o)
  g.rom(SON, p)          (joueurs)                          ->  g.son(SON, p)
  w.call_ao(SON, a0, a1) (combat)                           ->  crate::sound::jouer(w.b, SON, a0)
  call(c, b, SON), r.call(SON) (autres contextes)           ->  jouer_global(b, SON) si le son ne lit pas son objet,
                                                                sinon jouer(b, SON, Obj(c.a[0]))
  commandes de service (0xe0882, 0xe0a10...)                ->  crate::sound::service(b, Service::…)

Les sons qui lisent D4 ou A1 deviennent `jouer_avec(…, Source { … })` avec les registres : à remplacer à la main par
la valeur que l'appelant posait. Relire le diff, compiler, valider.
"""
import os
import re,sys,collections
SRC=os.path.join(os.path.dirname(os.path.abspath(__file__)),'..','port','ddsom-game','src')
EXCL=re.compile(r'^(families/(boss|enemy|reactions|effects|items|boxes|petites|creators)|effects_aids|sound)')
tab=open(SRC+'/sound_table.rs').read()
wr={int(m.group(1),16):m.group(2) for m in re.finditer(r'w\((0x[0-9a-f]+), (\w+)',tab)}
snd=open(SRC+'/sound.rs').read()
aig={int(m.group(1),16):m.group(2) for m in re.finditer(r'\((0x[0-9a-f]+), (\w+)\s*[\(\{]',snd[snd.index('const AIGUILLAGES'):snd.index('const AIGUILLAGES')+1500])}
sel={0xe2240,0xe22e8,0xe2430,0xe27da,0xe285c,0xe2a74}
JOUABLE=set(wr)|set(aig)|sel|{0xec238}
SPECIAL={e for e,k in wr.items() if k=='DuD4'}|{e for e,k in aig.items() if k in('ParD4','ParA1')}
SERVICE={0xe0882:'ArretGeneral',0xe0a10:'Ff05',0xe0a2e:'Ff06D1',0xe0a40:'Ff06',0xe0a70:'Ff07D1',0xe0a82:'Ff07',0xe0ad8:'Ff08',0xe0af8:'Ff09'}
files={}
for root,_,fs in os.walk(SRC):
    for f in fs:
        if f.endswith('.rs'):
            p=os.path.relpath(os.path.join(root,f),SRC)
            files[p]=open(os.path.join(root,f)).read()
consts=collections.defaultdict(dict)  # file -> name -> value
glob=collections.defaultdict(set)
for p,t in files.items():
    for m in re.finditer(r'const (\w+): u32 = (0x[0-9a-fA-F_]+);',t):
        v=int(m.group(2).replace('_',''),16)
        consts[p][m.group(1)]=v; glob[m.group(1)].add(v)
def resolve(p,arg):
    arg=arg.strip()
    if re.fullmatch(r'0x[0-9a-fA-F_]+',arg): return int(arg.replace('_',''),16)
    name=arg.split('::')[-1]
    if not re.fullmatch(r'[A-Z][A-Z0-9_]*',name): return None
    if 'son::' in arg and name in consts['player/externes.rs']: return consts['player/externes.rs'][name]
    if name in consts[p]: return consts[p][name]
    vs=glob.get(name,set())
    if len(vs)==1: return next(iter(vs))
    return None

wrr={}
for m in re.finditer(r'w\((0x[0-9a-f]+), (\w+)(?:\([^)]*\))?, 0x[0-9a-f]+, 0x[0-9a-f]+, ([^)]*)\)',tab):
    wrr[int(m.group(1),16)]=(m.group(2),m.group(3))
def reads_obj(v,depth=0):
    if v in wrr:
        k,o=wrr[v]; return 'PAN' in o or k in('ParObjet','ParOctet2')
    if v in aig: return aig[v] not in ('Musique',)
    if v==0xe2240: return reads_obj(0xe2248) or reads_obj(0xe2390)
    if v==0xe22e8: return reads_obj(0xe22f0) or reads_obj(0xe2390)
    if v==0xe2430: return reads_obj(0xe2438) or reads_obj(0xe24d8)
    if v==0xe27da: return reads_obj(0xe27e2) or reads_obj(0xe28a4)
    if v==0xe285c: return reads_obj(0xe2864) or reads_obj(0xe28a4)
    if v==0xe2a74: return reads_obj(0xe2a7a) or reads_obj(0xe2a34)
    if v==0xec238: return False
    return True

def args_at(t,i):
    # t[i]=='(' ; returns (end index after ')', list of args)
    depth=0; j=i; cur=''; out=[]
    while True:
        ch=t[j]
        if ch in '([{':
            depth+=1
            if depth>1: cur+=ch
        elif ch in ')]}':
            depth-=1
            if depth==0:
                out.append(cur.strip()); return j+1,out
            cur+=ch
        elif ch==',' and depth==1:
            out.append(cur.strip()); cur=''
        else: cur+=ch
        j+=1

SV=lambda name:'crate::sound::Service::'+name
def convert(p,t):
    pat=re.compile(r'\b(?:(\w+)\.(call|rom|call_ao)|(call))\(')
    out=[];last=0;manual=[]
    for m in pat.finditer(t):
        if m.start()<last: continue
        end,args=args_at(t,m.end()-1)
        recv,meth,bare=m.group(1),m.group(2),m.group(3)
        if bare:
            if args[:2]!=['c','b'] or len(args)!=3: continue
            arg=args[2]
        else: arg=args[0]
        v=resolve(p,arg)
        if v is None or not (v in JOUABLE or v in SERVICE): continue
        line=t.count('\n',0,m.start())+1
        rep=None
        if v in SERVICE:
            name=SERVICE[v]
            if bare: B,R='b','c'
            else: B,R=recv+'.b',recv+'.c'
            if meth not in (None,'call') and not bare: manual.append((line,'service via '+meth)); continue
            if name in('ArretGeneral','Ff05'): rep=f'crate::sound::service({B}, {SV(name)})'
            elif name=='Ff06D1': rep=f'crate::sound::service({B}, {SV("Ff06")} {{ verrou: {R}.d[1] as u8, ligne: {R}.d[0] as u16 }})'
            elif name=='Ff07D1': rep=f'crate::sound::service({B}, {SV("Ff07")} {{ verrou: {R}.d[1] as u8, ligne: {R}.d[0] as u16 }})'
            elif name=='Ff06': rep=f'crate::sound::service({B}, {SV("Ff06")} {{ verrou: 0xff, ligne: {R}.d[0] as u16 }})'
            elif name=='Ff07': rep=f'crate::sound::service({B}, {SV("Ff07")} {{ verrou: 0, ligne: {R}.d[0] as u16 }})'
            else: rep=f'crate::sound::service({B}, {SV(name)}({R}.d[0] as u16))'
        elif meth=='rom':
            if v in SPECIAL: manual.append((line,'rom special '+hex(v))); continue
            rep=f'{recv}.son({arg}, {args[1]})'
        elif meth=='call_ao':
            if v in SPECIAL: rep=f'crate::sound::jouer_avec({recv}.b, {arg}, crate::sound::Source {{ objet: Some({args[1]}), a1: Some({args[2]}.0), d4: Some({recv}.c.d[4]) }})'
            else: rep=f'crate::sound::jouer({recv}.b, {arg}, {args[1]})'
        else:
            cx = (p.startswith('families/') and recv=='x')
            B = 'b' if bare else recv+'.b'
            R = 'c' if bare else recv+'.c'
            obj = (recv+'.o') if cx else f'crate::objects::Obj({R}.a[0])'
            if v in SPECIAL: rep=f'crate::sound::jouer_avec({B}, {arg}, crate::sound::Source {{ objet: Some({obj}), a1: Some({R}.a[1]), d4: Some({R}.d[4]) }})'
            elif cx or reads_obj(v): rep=f'crate::sound::jouer({B}, {arg}, {obj})'
            else: rep=f'crate::sound::jouer_global({B}, {arg})'
        out.append(t[last:m.start()]); out.append(rep); last=end
    out.append(t[last:])
    return ''.join(out),manual


if __name__ == "__main__":
    for arg in sys.argv[1:]:
        p = os.path.relpath(os.path.abspath(arg), os.path.abspath(SRC)) if os.path.isabs(arg) or os.path.exists(arg) else arg
        t = files[p]
        n, manuel = convert(p, t)
        if n != t:
            open(os.path.join(SRC, p), "w").write(n)
            print("converti", p)
        for ligne, quoi in manuel:
            print(f"à la main : {p}:{ligne} ({quoi})")
