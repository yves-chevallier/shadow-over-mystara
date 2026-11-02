#!/usr/bin/env python3
"""Switches to `#[sans]` (Corps::Gestionnaire, without registers) every `handlers!` / `entry!` entry of the families whose
handler is generic over the register bench.

Checked by the compiler: at the end of each file, one check function per entry calls the handler
with a `Gx`; those that do not compile point at the handlers still on the processor. The checks are
then removed, and the other entries marked `#[sans]`.

    regs_without.py <excluded addresses, comma-separated>   (handlers whose output is read back: `Restes`)
"""
import os, re, subprocess, sys, json

SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port/ddsom-game/src/families')
PORT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'port')
ENV = dict(os.environ)  # CARGO_TARGET_DIR, DDSOM_ROMS: the caller's
excluded = {int(a, 16) for a in sys.argv[1].split(',') if a} if len(sys.argv) > 1 else set()

ENTRY = re.compile(r'^(\s*)(0x[0-9a-f]+) ("[^"]*") ([\w:]+),')                 # handlers! { 0x.. "name" path, }
ENTRY_SET = re.compile(r'^(\s*entry!\()(0x[0-9a-f]+) ("[^"]*"), ([\w:]+)\),')  # set.rs
MARKER = '// --- check without registers (regs_without.py) ---'

def files():
    out = []
    for root, _, fs in os.walk(SRC):
        for f in fs:
            if f.endswith('.rs') and f != 'cx.rs':
                out.append(os.path.join(root, f))
    return sorted(out)

def in_handlers(lines, i):
    for k in range(i, -1, -1):
        if 'handlers! {' in lines[k]:
            return True
        if lines[k].strip() == '}' and k != i:
            return False
    return False

def entries(lines):
    for i, l in enumerate(lines):
        m = ENTRY.match(l)
        if m and in_handlers(lines, i):
            yield i, int(m[2], 16), m[4], 'h'
        m = ENTRY_SET.match(l)
        if m:
            yield i, int(m[2], 16), m[4], 's'

# 1. checks
checks = {}  # (file, check line) -> (file, entry line)
for f in files():
    lines = open(f).read().split('\n')
    es = [e for e in entries(lines) if e[1] not in excluded]
    if not es:
        continue
    block = ['', MARKER]
    base = len(lines) + 2
    for k, (i, addr, path, _) in enumerate(es):
        block.append(f'#[allow(dead_code)] fn __check_without_regs_{k}(x: &mut crate::families::cx::Gx) {{ {path}(x); }}')
        checks[(f, base + 1 + k)] = (f, i)
    open(f, 'w').write('\n'.join(lines + block))

p = subprocess.run(['cargo', 'build', '--release', '-p', 'ddsom-game', '--lib', '--message-format=json'], cwd=PORT, env=ENV,
                   capture_output=True, text=True)
failures = set()
for l in p.stdout.splitlines():
    try:
        j = json.loads(l)
    except ValueError:
        continue
    if j.get('reason') != 'compiler-message' or j['message']['level'] != 'error':
        continue
    for sp in j['message']['spans']:
        key = (os.path.join(PORT, sp['file_name']), sp['line_start'])
        if key in checks:
            failures.add(checks[key])

# 2. remove the checks, mark the entries
n = 0
for f in files():
    s = open(f).read()
    if MARKER not in s:
        continue
    s = s[:s.index('\n' + MARKER) - 1] if s[s.index('\n' + MARKER) - 1] == '\n' else s[:s.index('\n' + MARKER)]
    lines = s.split('\n')
    for i, addr, path, kind in list(entries(lines)):
        if addr in excluded or (f, i) in failures or '#[sans]' in lines[i]:
            continue
        if kind == 'h':
            m = ENTRY.match(lines[i])
            lines[i] = f'{m[1]}{m[2]} {m[3]} #[sans] {m[4]},' + lines[i][m.end():]
        else:
            m = ENTRY_SET.match(lines[i])
            lines[i] = f'{m[1]}{m[2]} {m[3]}, #[sans] {m[4]}),' + lines[i][m.end():]
        n += 1
    open(f, 'w').write('\n'.join(lines))
print('checks:', len(checks), '; on the processor:', len(failures), '; switched to no registers:', n)
