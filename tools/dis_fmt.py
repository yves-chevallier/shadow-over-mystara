import re


def parse(s):
    s = s.strip()
    m = re.match(r'^([A-Za-z_]+)(.*)$', s, re.S)
    name, rest = m.group(1), m.group(2).strip()
    if not rest.startswith('(') and not rest.startswith('{'):
        return (name, [])
    inner = rest[1:-1]
    args = []
    depth = 0
    cur = ''
    for ch in inner:
        if ch in '({':
            depth += 1
        if ch in ')}':
            depth -= 1
        if ch == ',' and depth == 0:
            args.append(cur.strip())
            cur = ''
        else:
            cur += ch
    if cur.strip():
        args.append(cur.strip())
    return (name, args)


def num(s):
    s = s.strip()
    if s.startswith('-0x'):
        return -int(s[3:], 16)
    if s.startswith('0x'):
        return int(s, 16)
    return int(s)


def hx(n):
    return ('-0x%x' % -n) if n < 0 else '0x%x' % n


SZ = {'Byte': 'b', 'Word': 'w', 'Long': 'l'}


def ea(s):
    name, a = parse(s)
    if name == 'Dn':
        return 'D%d' % num(a[0])
    if name == 'An':
        return 'A%d' % num(a[0])
    if name == 'Ind':
        return '(A%d)' % num(a[0])
    if name == 'PostInc':
        return '(A%d)+' % num(a[0])
    if name == 'PreDec':
        return '-(A%d)' % num(a[0])
    if name == 'Imm':
        return '#' + hx(num(a[0]))
    if name == 'Disp':
        return '(%s,A%d)' % (hx(num(a[1])), num(a[0]))
    if name == 'AbsL':
        return '0x%08x.l' % num(a[0])
    if name == 'AbsW':
        return '0x%x.w' % num(a[0])
    if name == 'PcDisp':
        return '(0x%x,PC)' % num(a[0])
    if name in ('Idx', 'PcIdx'):
        ext = num(a[1])
        d8 = ext & 0xff
        if d8 >= 128:
            d8 -= 256
        xr = ('A' if ext >> 15 & 1 else 'D') + str(ext >> 12 & 7) + ('l' if ext >> 11 & 1 else 'w')
        if name == 'Idx':
            return '(%s,A%d,%s)' % (hx(d8), num(a[0]), xr)
        return '(0x%x,PC,%s)' % (num(a[0]), xr)
    if name == 'Reg':
        return 'D%d' % num(a[0])
    return s


CC = ['t', 'f', 'hi', 'ls', 'cc', 'cs', 'ne', 'eq', 'vc', 'vs', 'pl', 'mi', 'ge', 'lt', 'gt', 'le']


def fmt(dbg):
    name, a = parse(dbg)
    try:
        if name in ('Move', 'Add', 'Sub', 'Cmp', 'And', 'Or', 'Eor', 'Addx', 'Subx') and len(a) == 3:
            return '%s.%s %s,%s' % (name.lower(), SZ[a[0]], ea(a[1]), ea(a[2]))
        if name in ('Movea', 'Adda', 'Suba', 'Cmpa'):
            return '%s.%s %s,A%d' % (name.lower(), SZ[a[0]], ea(a[1]), num(a[2]))
        if name in ('Tst', 'Clr', 'Neg', 'Negx', 'Not'):
            return '%s.%s %s' % (name.lower(), SZ[a[0]], ea(a[1]))
        if name == 'Moveq':
            return 'moveq %s,D%d' % (hx(num(a[0])), num(a[1]))
        if name == 'Lea':
            return 'lea %s,A%d' % (ea(a[0]), num(a[1]))
        if name in ('Bsr', 'Bra'):
            return '%s 0x%x' % (name.lower(), num(a[0]))
        if name == 'Bcc':
            return 'b%s 0x%x' % (CC[num(a[0])], num(a[1]))
        if name in ('Jsr', 'Jmp'):
            return '%s %s' % (name.lower(), ea(a[0]))
        if name == 'Rts':
            return 'rts'
        if name == 'Bit':
            return '%s %s,%s' % (a[0].lower(), ea(a[1]), ea(a[2]))
    except Exception:
        pass
    return '?? ' + dbg
