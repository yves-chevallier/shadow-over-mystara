#!/usr/bin/env python3
"""Makes family functions generic (Cx<'_, R: Banc>), then propagates to the called functions as long as
`cargo check` asks for it (error "expected &mut Cx<'_>, found &mut Cx<'_, R>" on a call argument).
Stops on the other errors (register accesses...), which it prints.

generic.py <port/> file.rs:function [...]   (paths relative to ddsom-game/src)
"""
import re, subprocess, sys, pathlib, json

port = pathlib.Path(sys.argv[1])
src = port / 'ddsom-game/src'

def make_generic(file, name):
    p = src / file
    s = p.read_text()
    pat = re.compile(r'\bfn ' + re.escape(name) + r'\(x: &mut Cx([,)])')
    s2, n = pat.subn(lambda m: f"fn {name}<R: Banc>(x: &mut Cx<'_, R>{m.group(1)}", s)
    if n == 0:
        # parameter named other than x
        pat = re.compile(r'\bfn ' + re.escape(name) + r'\((\w+): &mut Cx([,)])')
        s2, n = pat.subn(lambda m: f"fn {name}<R: Banc>({m.group(1)}: &mut Cx<'_, R>{m.group(2)}", s)
    if n and 'Banc' not in re.sub(r'fn \w+<R: Banc>', '', s2).split('fn ', 1)[0] and not re.search(r'use [^;]*\bBanc\b', s2):
        # import Banc
        s2 = re.sub(r'(\nuse [^\n]*\n)', r'\1use crate::families::cx::Banc;\n', s2, count=1)
    p.write_text(s2)
    return n

EXCLUDED = ('fn set_dw(',)

def make_line_generic(file, line):
    p = src / file
    lines = p.read_text().split('\n')
    l = lines[line - 1]
    if any(e in l for e in EXCLUDED):
        return 0
    l2, n = re.subn(r"\bfn (\w+)\((\w+): &mut Cx([,)])", lambda m: f"fn {m.group(1)}<R: Banc>({m.group(2)}: &mut Cx<'_, R>{m.group(3)}", l)
    if n == 0:
        return 0
    if re.search(r"(FnOnce|FnMut|Fn|fn)\(&mut Cx[,)]", l2):
        # function parameters: same lifetime as the context
        l2 = re.sub(r"(FnOnce|FnMut|Fn|fn)\(&mut Cx([,)])", lambda m: f"{m.group(1)}(&mut Cx<'a, R>{m.group(2)}", l2)
        l2 = l2.replace("<R: Banc>(", "<'a, R: Banc>(", 1).replace("&mut Cx<'_, R>", "&mut Cx<'a, R>", 1)
    lines[line - 1] = l2
    t = '\n'.join(lines)
    if not re.search(r'use [^;]*\bBanc\b', t):
        t = re.sub(r'(\nuse [^\n]*\n)', r'\1use crate::families::cx::Banc;\n', t, count=1)
    p.write_text(t)
    return n

def check():
    r = subprocess.run(['cargo', 'check', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'],
                       cwd=port, capture_output=True, text=True)
    errs = []
    for l in r.stdout.splitlines():
        try:
            m = json.loads(l)
        except Exception:
            continue
        if m.get('reason') == 'compiler-message' and m['message']['level'] == 'error':
            errs.append(m['message'])
    return errs

# definition of a function from its call path: looks for `fn name(` in the files
def find(name, call_path, caller_file):
    cands = []
    for p in src.rglob('*.rs'):
        t = p.read_text()
        if re.search(r'\bfn ' + re.escape(name) + r'\(\w+: &mut Cx[,)]', t):
            cands.append(str(p.relative_to(src)))
    if len(cands) == 1:
        return cands[0]
    # prefer the one the path suggests (super::enemy_ogre::hit::regroup)
    parts = [q for q in call_path.split('::')[:-1] if q not in ('super', 'crate', 'self', 'families')]
    best = [c for c in cands if all(q in c for q in parts)] if parts else []
    if len(best) == 1:
        return best[0]
    if not parts:
        # same module as the caller
        d = str(pathlib.Path(caller_file).parent)
        same = [c for c in cands if c == caller_file] or [c for c in cands if str(pathlib.Path(c).parent) == d]
        if len(same) == 1:
            return same[0]
    return None

for arg in sys.argv[2:]:
    f, n = arg.split(':')
    print(f'{f}:{n} -> {make_generic(f, n)}')

for round_ in range(200):
    errs = check()
    if not errs:
        print('compile')
        break
    progress = False
    others = []
    for e in errs:
        msg = e['rendered']
        sp = next((s for s in e['spans'] if s['is_primary']), None)
        if "expected `&mut Cx<'_>`, found `&mut Cx<'_, R>`" in msg or "expected `&mut cx::Cx<'_>`" in msg or ("expected mutable reference `&mut cx::Cx<'_, &mut m68k::Cpu>`" in msg and "found mutable reference `&mut cx::Cx<'_, R>`" in msg):
            # the faulty call: the text of the secondary span "arguments to this function"
            defs = [(sp2['file_name'], sp2['line_start']) for ch in e.get('children', []) if 'defined here' in ch['message'] for sp2 in ch['spans']]
            ok = False
            for (fn_, ln) in defs:
                if make_line_generic(fn_.replace('ddsom-game/src/', ''), ln):
                    print(f"  {fn_}:{ln}")
                    ok = True
            if ok:
                progress = True
            else:
                others.append(msg)
        else:
            others.append(msg)
    if not progress:
        print(f'{len(others)} errors left:')
        for a in others[:40]:
            print(a)
        break
