#!/usr/bin/env python3
"""Makes generic over the register bench (`Cx<'_, R>`) the family functions that do not use the processor.

All free functions `fn f(x: &mut Cx, ...)` become `fn f<R: Banc>(x: &mut Cx<'_, R>, ...)`; it compiles,
and each function where the compiler finds an error gets its original signature back; it starts again until
everything compiles. What stays generic is proven register-free by the compiler.

    regs_generic.py [files...]   (default: all files of families/)
"""
import os, re, subprocess, sys, json

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port/ddsom-game/src')
ENV = dict(os.environ)  # CARGO_TARGET_DIR, DDSOM_ROMS: the caller's
SIG = re.compile(r'^(\s*(?:pub(?:\([a-z]+\))?\s+)?fn\s+)(\w+)(<[^>()]*>)?\((\s*)(mut\s+)?x: &mut Cx(\s*[,)])')
GEN = re.compile(r'^(\s*(?:pub(?:\([a-z]+\))?\s+)?fn\s+)(\w+)<R: crate::families::cx::Banc(?:, ([^>]*))?>\((\s*)(mut\s+)?x: &mut Cx<\'_, R>(\s*[,)])')

def files():
    if len(sys.argv) > 1:
        return [os.path.abspath(f) for f in sys.argv[1:]]
    out = []
    for root, _, fs in os.walk(SRC + '/families'):
        for f in fs:
            if f.endswith('.rs') and f != 'cx.rs':
                out.append(os.path.join(root, f))
    return sorted(out)

def generalize(path):
    lines = open(path).read().split('\n')
    n = 0
    for i, l in enumerate(lines):
        m = SIG.match(l)
        if m:
            others = m[3][1:-1] if m[3] else None
            gen = '<R: crate::families::cx::Banc' + (', ' + others if others else '') + '>'
            lines[i] = l[:m.start(2)] + m[2] + gen + '(' + m[4] + (m[5] or '') + "x: &mut Cx<'_, R>" + m[6] + l[m.end():]
            n += 1
    if n:
        open(path, 'w').write('\n'.join(lines))
    return n

def add_import(s):
    # after the file's first `use` line (module level)
    m = re.search(r'^use [^\n]*\n', s, re.M)
    imp = 'use crate::families::cx::Banc;\n'
    if m:
        return s[:m.start()] + imp + s[m.start():]
    return imp + s

def revert(path, line):
    """Restores the original signature of the function containing the line (1-indexed). True if changed."""
    lines = open(path).read().split('\n')
    for i in range(line - 1, -1, -1):
        m = GEN.match(lines[i])
        if m:
            others = m[3]
            gen = '<' + others + '>' if others else ''
            lines[i] = lines[i][:m.start(2)] + m[2] + gen + '(' + m[4] + (m[5] or '') + 'x: &mut Cx' + m[6] + lines[i][m.end():]
            open(path, 'w').write('\n'.join(lines))
            return True
        if re.match(r'^\s*(pub(\([a-z]+\))?\s+)?fn\s', lines[i]) or re.match(r'^(impl|mod|pub mod)\b', lines[i]):
            return False
    return False

def build():
    p = subprocess.run(['cargo', 'build', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'],
                       cwd=os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port'), env=ENV, capture_output=True, text=True)
    errors = []
    for l in p.stdout.splitlines():
        try:
            j = json.loads(l)
        except ValueError:
            continue
        if j.get('reason') != 'compiler-message' or j['message']['level'] != 'error':
            continue
        for sp in j['message']['spans']:
            if not sp.get('is_primary'):
                continue
            errors.append((os.path.join(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port'), sp['file_name']), sp['line_start'], j['message']['message']))
    return p.returncode == 0, errors

import os as _os
FIRST = _os.environ.get('PREMIERE', '1') == '1'
if FIRST:
    total = sum(generalize(f) for f in files())
    print('generalized:', total, flush=True)
for round_ in range(60 if FIRST else 0):
    ok, errors = build()
    if ok:
        print('compiles at round', round_, flush=True)
        break
    changes = 0
    no_function = set()
    for f, l, msg in errors:
        if revert(f, l):
            changes += 1
        else:
            no_function.add((f, l, msg[:100]))
    print(f'round {round_}: {len(errors)} errors, {changes} functions returned to the processor', flush=True)
    if changes == 0:
        for e in sorted(no_function)[:30]:
            print('  ', e)
        break

# Second pass: generic functions passed as a `fn(&mut Cx)` pointer ("mismatched types" errors at the call
# site) get their signature back: the names of generic functions are looked for on the error line.
def generics():
    names = {}
    for f in files():
        for i, l in enumerate(open(f).read().split('\n'), 1):
            m = GEN.match(l)
            if m:
                names.setdefault(m[2], []).append((f, i))
    return names

for round_ in range(20):
    ok, errors = build()
    if ok:
        print('compiles (2nd pass) at round', round_, flush=True)
        break
    names = generics()
    changes = 0
    for f, l, msg in errors:
        if revert(f, l):
            changes += 1
            continue
        text = open(f).read().split('\n')[l - 1]
        for ident in re.findall(r'(?:\w+::)*(\w+)', text):
            for (g, i) in names.get(ident, []):
                if revert(g, i):
                    changes += 1
    print(f'2nd pass, round {round_}: {len(errors)} errors, {changes} functions returned', flush=True)
    if changes == 0:
        for e in errors[:20]:
            print('  ', e)
        break
