#!/usr/bin/env python3
"""Input register readers: Rust functions (families) that read a processor register (x.c.d[k],
x.c.a[k], c.d[k]...) before writing it. Textual approximation (text order, not control flow).

readers.py <src root> [--famille pattern] [--resume]
"""
import re, sys, pathlib, collections

root = pathlib.Path(sys.argv[1])
pattern = None
if '--famille' in sys.argv:
    pattern = sys.argv[sys.argv.index('--famille') + 1]
summary = '--resume' in sys.argv

FN = re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+(\w+)\s*(<[^>]*>)?\s*\(([^)]*)\)', re.M)
REG = re.compile(r'\b(?:x\.c|c|self\.c)\.(d|a)\[(\d)\]')

def body(src, start):
    i = src.index('{', start)
    n = 0
    for j in range(i, len(src)):
        if src[j] == '{':
            n += 1
        elif src[j] == '}':
            n -= 1
            if n == 0:
                return src[i:j + 1]
    return src[i:]

def access_kind(text, end):
    """pure write, read-write, or read, depending on what follows the access."""
    rest = text[end:end + 6]
    if re.match(r'\s*=[^=]', rest):
        return 'w'
    if re.match(r'\s*(\|=|&=|\+=|-=|\^=|<<=|>>=)', rest):
        return 'rw'
    return 'r'

results = []
for p in sorted(root.rglob('*.rs')):
    rel = str(p.relative_to(root))
    if pattern and pattern not in rel:
        continue
    src = p.read_text()
    # strip line comments
    src_nc = re.sub(r'//[^\n]*', lambda m: ' ' * len(m.group(0)), src)
    for m in FN.finditer(src_nc):
        name, gen, params = m.group(1), m.group(2) or '', m.group(3)
        if 'Cpu' not in params and 'Cx' not in params:
            continue
        if 'Cx' in params and '<' in params.split('Cx', 1)[1][:3]:
            continue  # generic Cx<'_, R>: no registers
        try:
            b = body(src_nc, m.end())
        except ValueError:
            continue
        written, read = set(), []
        for r in REG.finditer(b):
            reg = f'{r.group(1).upper()}{r.group(2)}'
            # prefix: `let` or partial left-hand side `x.c.d[0] = x.c.d[0] & ...`: counted on the right
            k = access_kind(b, r.end())
            if reg in written:
                continue
            if k == 'w':
                # x.c.d[0] = x.c.d[0] & ...: the right-hand side reads first
                line_end = b.find(';', r.end())
                rhs = b[r.end():line_end]
                if re.search(r'\b(?:x\.c|c|self\.c)\.' + r.group(1) + r'\[' + r.group(2) + r'\]', rhs):
                    read.append(reg)
                written.add(reg)
            else:
                read.append(reg)
                written.add(reg)
        line = src.count('\n', 0, m.start()) + 1
        read = [r for r in dict.fromkeys(read) if r not in ('A0', 'A5', 'A6', 'A7')]
        if read:
            results.append((rel, line, name, read))

if summary:
    per = collections.Counter(r[0].split('/')[1] if r[0].startswith('families/') and '/' in r[0][9:] else r[0] for r in results)
    for k, v in sorted(per.items(), key=lambda kv: -kv[1]):
        print(f'{v:4} {k}')
    print(f'{len(results)} functions')
else:
    for rel, line, name, read in results:
        print(f'{rel}:{line} {name} {",".join(read)}')
