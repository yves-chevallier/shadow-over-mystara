//! Exécution des instructions décodées.

use crate::decode::{BitOp, Ea, Instr, Op, ShiftCount, ShiftKind, Size};
use crate::{Bus, Cpu, ADDR_MASK, SR_C, SR_N, SR_S, SR_V, SR_X, SR_Z};

/// Emplacement d'un opérande une fois son adresse effective calculée (effets de bord appliqués).
#[derive(Clone, Copy)]
enum Loc {
    D(u8),
    A(u8),
    Mem(u32),
    Prog(u32),
    Imm(u32),
}

fn sext(v: u32, size: Size) -> u32 {
    match size {
        Size::Byte => v as u8 as i8 as i32 as u32,
        Size::Word => v as u16 as i16 as i32 as u32,
        Size::Long => v,
    }
}

impl Cpu {
    fn index(&self, ext: u16) -> u32 {
        let r = ((ext >> 12) & 7) as usize;
        let v = if ext & 0x8000 != 0 { self.a[r] } else { self.d[r] };
        let v = if ext & 0x800 != 0 { v } else { v as u16 as i16 as i32 as u32 };
        v.wrapping_add(ext as i8 as i32 as u32)
    }

    /// Adresse effective d'un mode mémoire (sans effet de bord) : pour LEA/PEA/JMP/JSR.
    fn ea_address(&self, ea: Ea) -> u32 {
        match ea {
            Ea::Ind(n) => self.a[n as usize],
            Ea::Disp(n, d) => self.a[n as usize].wrapping_add(d as i32 as u32),
            Ea::Idx(n, ext) => self.a[n as usize].wrapping_add(self.index(ext)),
            Ea::Abs(a) => a,
            Ea::PcDisp(a) => a,
            Ea::PcIdx(base, ext) => base.wrapping_add(self.index(ext) - (ext as i8 as i32 as u32)),
            _ => unreachable!("mode sans adresse : {ea:?}"),
        }
    }

    fn resolve(&mut self, ea: Ea, size: Size) -> Loc {
        let step = |n: u8| if n == 7 && size == Size::Byte { 2 } else { size.bytes() };
        match ea {
            Ea::Dn(n) => Loc::D(n),
            Ea::An(n) => Loc::A(n),
            Ea::Imm(v) => Loc::Imm(v),
            Ea::PostInc(n) => {
                let a = self.a[n as usize];
                self.a[n as usize] = a.wrapping_add(step(n));
                Loc::Mem(a)
            }
            Ea::PreDec(n) => {
                let a = self.a[n as usize].wrapping_sub(step(n));
                self.a[n as usize] = a;
                Loc::Mem(a)
            }
            Ea::PcDisp(_) | Ea::PcIdx(..) => Loc::Prog(self.ea_address(ea)),
            _ => Loc::Mem(self.ea_address(ea)),
        }
    }

    fn read<B: Bus>(&mut self, bus: &mut B, loc: Loc, size: Size) -> u32 {
        match loc {
            Loc::D(n) => self.d[n as usize] & size.mask(),
            Loc::A(n) => self.a[n as usize] & size.mask(),
            Loc::Imm(v) => v & size.mask(),
            Loc::Mem(a) => read_mem(bus, a, size),
            Loc::Prog(a) => {
                let a = a & ADDR_MASK;
                match size {
                    Size::Byte => {
                        let w = bus.read_prog16(a & !1);
                        (if a & 1 == 0 { w >> 8 } else { w & 0xff }) as u32
                    }
                    Size::Word => bus.read_prog16(a) as u32,
                    Size::Long => {
                        (bus.read_prog16(a) as u32) << 16 | bus.read_prog16((a + 2) & ADDR_MASK) as u32
                    }
                }
            }
        }
    }

    fn write<B: Bus>(&mut self, bus: &mut B, loc: Loc, size: Size, v: u32) {
        match loc {
            Loc::D(n) => {
                let m = size.mask();
                let r = &mut self.d[n as usize];
                *r = (*r & !m) | (v & m);
            }
            Loc::A(n) => self.a[n as usize] = v,
            Loc::Mem(a) => write_mem(bus, a, size, v),
            Loc::Prog(_) | Loc::Imm(_) => unreachable!("écriture vers un mode non altérable"),
        }
    }

    fn read_ea<B: Bus>(&mut self, bus: &mut B, ea: Ea, size: Size) -> u32 {
        let l = self.resolve(ea, size);
        self.read(bus, l, size)
    }

    fn cond(&self, c: u8) -> bool {
        let sr = self.sr;
        let (n, z, v, cf) = (sr & SR_N != 0, sr & SR_Z != 0, sr & SR_V != 0, sr & SR_C != 0);
        match c {
            0 => true,
            1 => false,
            2 => !cf && !z,
            3 => cf || z,
            4 => !cf,
            5 => cf,
            6 => !z,
            7 => z,
            8 => !v,
            9 => v,
            10 => !n,
            11 => n,
            12 => n == v,
            13 => n != v,
            14 => !z && n == v,
            _ => z || n != v,
        }
    }

    fn set_flags(&mut self, mask: u16, values: u16) {
        self.sr = (self.sr & !mask) | (values & mask);
    }

    /// N et Z selon le résultat, V et C à zéro (opérations logiques, MOVE...).
    fn flags_logic(&mut self, r: u32, size: Size) {
        let mut f = 0;
        if r & size.mask() == 0 {
            f |= SR_Z;
        }
        if r & size.msb() != 0 {
            f |= SR_N;
        }
        self.set_flags(SR_N | SR_Z | SR_V | SR_C, f);
    }

    fn privileged<B: Bus>(&mut self, bus: &mut B, ins: &Instr) -> bool {
        if self.sr & SR_S == 0 {
            self.exception(bus, 8, ins.pc);
            return false;
        }
        true
    }

    /// Exécute `ins` (PC déjà avancé). Renvoie vrai si une exception a été prise.
    pub(crate) fn exec<B: Bus>(&mut self, bus: &mut B, ins: &Instr) -> bool {
        match ins.op {
            Op::Add(sz, s, d) | Op::Sub(sz, s, d) => {
                let sub = matches!(ins.op, Op::Sub(..));
                let sv = self.read_ea(bus, s, sz);
                let dl = self.resolve(d, sz);
                let dv = self.read(bus, dl, sz);
                let r = self.addsub(sub, sv, dv, false, sz, false);
                self.write(bus, dl, sz, r);
            }
            Op::Addx(sz, s, d) | Op::Subx(sz, s, d) => {
                let sub = matches!(ins.op, Op::Subx(..));
                let sv = self.read_ea(bus, s, sz);
                let dl = self.resolve(d, sz);
                let dv = self.read(bus, dl, sz);
                let x = self.sr & SR_X != 0;
                let r = self.addsub(sub, sv, dv, x, sz, true);
                self.write(bus, dl, sz, r);
            }
            Op::Cmp(sz, s, d) => {
                let sv = self.read_ea(bus, s, sz);
                let dv = self.read_ea(bus, d, sz);
                let x = self.sr & SR_X;
                self.addsub(true, sv, dv, false, sz, false);
                self.sr = (self.sr & !SR_X) | x;
            }
            Op::Adda(sz, s, n) | Op::Suba(sz, s, n) => {
                let sv = sext(self.read_ea(bus, s, sz), sz);
                let a = &mut self.a[n as usize];
                *a = if matches!(ins.op, Op::Adda(..)) { a.wrapping_add(sv) } else { a.wrapping_sub(sv) };
            }
            Op::Cmpa(sz, s, n) => {
                let sv = sext(self.read_ea(bus, s, sz), sz);
                let dv = self.a[n as usize];
                let x = self.sr & SR_X;
                self.addsub(true, sv, dv, false, Size::Long, false);
                self.sr = (self.sr & !SR_X) | x;
            }
            Op::And(sz, s, d) | Op::Or(sz, s, d) | Op::Eor(sz, s, d) => {
                let sv = self.read_ea(bus, s, sz);
                let dl = self.resolve(d, sz);
                let dv = self.read(bus, dl, sz);
                let r = match ins.op {
                    Op::And(..) => sv & dv,
                    Op::Or(..) => sv | dv,
                    _ => sv ^ dv,
                };
                self.flags_logic(r, sz);
                self.write(bus, dl, sz, r);
            }
            Op::AndiCcr(v) => self.set_ccr(self.ccr() & v as u16),
            Op::OriCcr(v) => self.set_ccr(self.ccr() | v as u16),
            Op::EoriCcr(v) => self.set_ccr(self.ccr() ^ v as u16),
            Op::AndiSr(v) | Op::OriSr(v) | Op::EoriSr(v) => {
                if !self.privileged(bus, ins) {
                    return true;
                }
                let r = match ins.op {
                    Op::AndiSr(_) => self.sr & v,
                    Op::OriSr(_) => self.sr | v,
                    _ => self.sr ^ v,
                };
                self.set_sr(r);
            }
            Op::Neg(sz, d) | Op::Negx(sz, d) => {
                let dl = self.resolve(d, sz);
                let dv = self.read(bus, dl, sz);
                let negx = matches!(ins.op, Op::Negx(..));
                let x = negx && self.sr & SR_X != 0;
                let r = self.addsub(true, dv, 0, x, sz, negx);
                self.write(bus, dl, sz, r);
            }
            Op::Not(sz, d) => {
                let dl = self.resolve(d, sz);
                let r = !self.read(bus, dl, sz) & sz.mask();
                self.flags_logic(r, sz);
                self.write(bus, dl, sz, r);
            }
            Op::Clr(sz, d) => {
                let dl = self.resolve(d, sz);
                if let Loc::Mem(_) = dl {
                    // le 68000 lit l'opérande avant de l'effacer
                    self.read(bus, dl, sz);
                }
                self.set_flags(SR_N | SR_Z | SR_V | SR_C, SR_Z);
                self.write(bus, dl, sz, 0);
            }
            Op::Tst(sz, d) => {
                let v = self.read_ea(bus, d, sz);
                self.flags_logic(v, sz);
            }
            Op::Tas(d) => {
                let dl = self.resolve(d, Size::Byte);
                let v = self.read(bus, dl, Size::Byte);
                self.flags_logic(v, Size::Byte);
                self.write(bus, dl, Size::Byte, v | 0x80);
            }
            Op::Ext(sz, n) => {
                let r = &mut self.d[n as usize];
                let v = if sz == Size::Word {
                    *r = (*r & 0xffff_0000) | (*r as u8 as i8 as i16 as u16 as u32);
                    *r & 0xffff
                } else {
                    *r = *r as u16 as i16 as i32 as u32;
                    *r
                };
                self.flags_logic(v, sz);
            }
            Op::Swap(n) => {
                let r = self.d[n as usize].rotate_left(16);
                self.d[n as usize] = r;
                self.flags_logic(r, Size::Long);
            }
            Op::Exg(mode, x, y) => {
                let (x, y) = (x as usize, y as usize);
                match mode {
                    0x08 => self.d.swap(x, y),
                    0x09 => self.a.swap(x, y),
                    _ => std::mem::swap(&mut self.d[x], &mut self.a[y]),
                }
            }
            Op::Mulu(s, n) | Op::Muls(s, n) => {
                let sv = self.read_ea(bus, s, Size::Word);
                let dv = self.d[n as usize];
                let r = if matches!(ins.op, Op::Mulu(..)) {
                    (sv & 0xffff) * (dv & 0xffff)
                } else {
                    ((sv as i16 as i32) * (dv as i16 as i32)) as u32
                };
                self.d[n as usize] = r;
                self.flags_logic(r, Size::Long);
                self.cycles += 34;
            }
            Op::Divu(s, n) | Op::Divs(s, n) => {
                let sv = self.read_ea(bus, s, Size::Word) & 0xffff;
                if sv == 0 {
                    self.div_zero_flags(ins, n);
                    self.exception(bus, 5, self.pc);
                    return true;
                }
                let dv = self.d[n as usize];
                if matches!(ins.op, Op::Divu(..)) {
                    let q = dv / sv;
                    let r = dv % sv;
                    if q > 0xffff {
                        self.set_flags(SR_N | SR_Z | SR_V | SR_C, SR_V | SR_N);
                    } else {
                        self.d[n as usize] = (r << 16) | q;
                        self.flags_logic(q, Size::Word);
                    }
                } else {
                    let (dv, sv) = (dv as i32 as i64, sv as i16 as i64);
                    let q = dv / sv;
                    let r = dv % sv;
                    if q > i16::MAX as i64 || q < i16::MIN as i64 {
                        self.set_flags(SR_N | SR_Z | SR_V | SR_C, SR_V | SR_N);
                    } else {
                        self.d[n as usize] = ((r as u32 & 0xffff) << 16) | (q as u32 & 0xffff);
                        self.flags_logic(q as u32, Size::Word);
                    }
                }
                self.cycles += 140;
            }
            Op::Abcd(s, d) | Op::Sbcd(s, d) => {
                let sv = self.read_ea(bus, s, Size::Byte);
                let dl = self.resolve(d, Size::Byte);
                let dv = self.read(bus, dl, Size::Byte);
                let r = if matches!(ins.op, Op::Abcd(..)) { self.abcd(sv, dv) } else { self.sbcd(sv, dv) };
                self.write(bus, dl, Size::Byte, r);
            }
            Op::Nbcd(d) => {
                let dl = self.resolve(d, Size::Byte);
                let dv = self.read(bus, dl, Size::Byte);
                let r = self.sbcd(dv, 0);
                self.write(bus, dl, Size::Byte, r);
            }
            Op::Chk(s, n) => {
                let bound = self.read_ea(bus, s, Size::Word) as u16 as i16;
                let v = self.d[n as usize] as u16 as i16;
                self.chk_flags(v, bound);
                if v < 0 || v > bound {
                    self.exception(bus, 6, self.pc);
                    return true;
                }
            }
            Op::Move(sz, s, d) => {
                let v = self.read_ea(bus, s, sz);
                let dl = self.resolve(d, sz);
                self.flags_logic(v, sz);
                self.write(bus, dl, sz, v);
            }
            Op::Movea(sz, s, n) => {
                let v = sext(self.read_ea(bus, s, sz), sz);
                self.a[n as usize] = v;
            }
            Op::Moveq(v, n) => {
                let v = v as i32 as u32;
                self.d[n as usize] = v;
                self.flags_logic(v, Size::Long);
            }
            Op::Movem { size, to_mem, mask, ea } => self.movem(bus, size, to_mem, mask, ea),
            Op::Movep { size, to_mem, dn, an, disp } => {
                let mut a = self.a[an as usize].wrapping_add(disp as i32 as u32);
                let n = size.bytes();
                if to_mem {
                    let v = self.d[dn as usize];
                    for i in (0..n).rev() {
                        bus.write8(a & ADDR_MASK, (v >> (8 * i)) as u8);
                        a = a.wrapping_add(2);
                    }
                } else {
                    let mut v = 0u32;
                    for _ in 0..n {
                        v = (v << 8) | bus.read8(a & ADDR_MASK) as u32;
                        a = a.wrapping_add(2);
                    }
                    let m = size.mask();
                    let r = &mut self.d[dn as usize];
                    *r = (*r & !m) | v;
                }
            }
            Op::MoveFromSr(d) => {
                let dl = self.resolve(d, Size::Word);
                if let Loc::Mem(_) = dl {
                    self.read(bus, dl, Size::Word);
                }
                self.write(bus, dl, Size::Word, self.sr as u32);
            }
            Op::MoveToCcr(s) => {
                let v = self.read_ea(bus, s, Size::Word);
                self.set_ccr(v as u16);
            }
            Op::MoveToSr(s) => {
                if !self.privileged(bus, ins) {
                    return true;
                }
                let v = self.read_ea(bus, s, Size::Word);
                self.set_sr(v as u16);
            }
            Op::MoveUsp { to_usp, an } => {
                if !self.privileged(bus, ins) {
                    return true;
                }
                if to_usp {
                    self.other_sp = self.a[an as usize];
                } else {
                    self.a[an as usize] = self.other_sp;
                }
            }
            Op::Lea(ea, n) => self.a[n as usize] = self.ea_address(ea),
            Op::Pea(ea) => {
                let a = self.ea_address(ea);
                self.push32(bus, a);
            }
            Op::Link(n, disp) => {
                // LINK A7 empile l'ancienne valeur de A7
                let v = self.a[n as usize];
                self.push32(bus, v);
                self.a[n as usize] = self.a[7];
                self.a[7] = self.a[7].wrapping_add(disp as i32 as u32);
            }
            Op::Unlk(n) => {
                self.a[7] = self.a[n as usize];
                let v = self.pop32(bus);
                self.a[n as usize] = v;
            }
            Op::Bit(kind, count, d) => {
                let bitn = match count {
                    ShiftCount::Imm(v) => v as u32,
                    ShiftCount::Reg(r) => self.d[r as usize],
                };
                let sz = if matches!(d, Ea::Dn(_)) { Size::Long } else { Size::Byte };
                let bitn = bitn % (sz.bytes() * 8);
                let dl = self.resolve(d, sz);
                let v = self.read(bus, dl, sz);
                let set = v >> bitn & 1 != 0;
                self.set_flags(SR_Z, if set { 0 } else { SR_Z });
                let r = match kind {
                    BitOp::Btst => return false,
                    BitOp::Bchg => v ^ (1 << bitn),
                    BitOp::Bclr => v & !(1 << bitn),
                    BitOp::Bset => v | (1 << bitn),
                };
                self.write(bus, dl, sz, r);
            }
            Op::Shift { kind, left, size, count, dn } => {
                let c = match count {
                    ShiftCount::Imm(v) => v as u32,
                    ShiftCount::Reg(r) => self.d[r as usize] & 63,
                };
                let v = self.d[dn as usize] & size.mask();
                let r = self.shift(kind, left, size, v, c);
                let m = size.mask();
                let reg = &mut self.d[dn as usize];
                *reg = (*reg & !m) | (r & m);
                self.cycles += 2 * c as u64;
            }
            Op::ShiftMem { kind, left, ea } => {
                let dl = self.resolve(ea, Size::Word);
                let v = self.read(bus, dl, Size::Word);
                let r = self.shift(kind, left, Size::Word, v, 1);
                self.write(bus, dl, Size::Word, r);
            }
            Op::Bra(t) => self.pc = t,
            Op::Bsr(t) => {
                self.push32(bus, self.pc);
                self.pc = t;
            }
            Op::Bcc(c, t) => {
                if self.cond(c) {
                    self.pc = t;
                }
            }
            Op::Dbcc(c, n, t) => {
                if !self.cond(c) {
                    let r = &mut self.d[n as usize];
                    let w = (*r as u16).wrapping_sub(1);
                    *r = (*r & 0xffff_0000) | w as u32;
                    if w != 0xffff {
                        self.pc = t;
                    }
                }
            }
            Op::Scc(c, d) => {
                let dl = self.resolve(d, Size::Byte);
                if let Loc::Mem(_) = dl {
                    self.read(bus, dl, Size::Byte);
                }
                let v = if self.cond(c) { 0xff } else { 0 };
                self.write(bus, dl, Size::Byte, v);
            }
            Op::Jmp(ea) => self.pc = self.ea_address(ea),
            Op::Jsr(ea) => {
                let t = self.ea_address(ea);
                self.push32(bus, self.pc);
                self.pc = t;
            }
            Op::Rts => self.pc = self.pop32(bus),
            Op::Rtr => {
                let ccr = self.pop16(bus);
                self.set_ccr(ccr);
                self.pc = self.pop32(bus);
            }
            Op::Rte => {
                if !self.privileged(bus, ins) {
                    return true;
                }
                let sr = self.pop16(bus);
                let pc = self.pop32(bus);
                self.set_sr(sr);
                self.pc = pc;
            }
            Op::Trap(n) => {
                self.exception(bus, 32 + n as u32, self.pc);
                return true;
            }
            Op::Trapv => {
                if self.sr & SR_V != 0 {
                    self.exception(bus, 7, self.pc);
                    return true;
                }
            }
            Op::Nop => {}
            Op::Reset => {
                if !self.privileged(bus, ins) {
                    return true;
                }
                bus.reset_devices();
            }
            Op::Stop(v) => {
                if !self.privileged(bus, ins) {
                    return true;
                }
                self.set_sr(v);
                self.stopped = true;
            }
            Op::Illegal => {
                self.exception(bus, 4, ins.pc);
                return true;
            }
            Op::LineA => {
                self.exception(bus, 10, ins.pc);
                return true;
            }
            Op::LineF => {
                self.exception(bus, 11, ins.pc);
                return true;
            }
        }
        false
    }

    /// Addition/soustraction avec tous les flags. `dv - sv` pour une soustraction.
    /// `ext` : variante X (ADDX/SUBX/NEGX) où Z n'est effacé que si le résultat est non nul.
    fn addsub(&mut self, sub: bool, sv: u32, dv: u32, x: bool, sz: Size, ext: bool) -> u32 {
        let m = sz.mask();
        let msb = sz.msb();
        let (sv, dv) = (sv & m, dv & m);
        let xi = x as u64;
        let (r, carry) = if sub {
            let r = (dv as u64).wrapping_sub(sv as u64).wrapping_sub(xi);
            (r as u32 & m, (sv as u64 + xi) > dv as u64)
        } else {
            let r = dv as u64 + sv as u64 + xi;
            (r as u32 & m, r > m as u64)
        };
        let overflow = if sub {
            ((dv ^ sv) & (dv ^ r) & msb) != 0
        } else {
            (!(dv ^ sv) & (dv ^ r) & msb) != 0
        };
        let mut f = 0;
        if r & msb != 0 {
            f |= SR_N;
        }
        if overflow {
            f |= SR_V;
        }
        if carry {
            f |= SR_C | SR_X;
        }
        let z = if ext {
            if r != 0 { 0 } else { self.sr & SR_Z }
        } else if r == 0 {
            SR_Z
        } else {
            0
        };
        self.sr = (self.sr & !0x1f) | f | z;
        r
    }

    /// ABCD : algorithme vérifié sur 68000 réel (Flamewing), y compris V et N « non définis ».
    fn abcd(&mut self, sv: u32, dv: u32) -> u32 {
        let x = (self.sr & SR_X != 0) as u32;
        let unadj = sv + dv + x;
        let mut adj = 0;
        if (sv & 0x0f) + (dv & 0x0f) + x > 9 {
            adj |= 0x06;
        }
        let c = unadj > 0x99;
        if c {
            adj |= 0x60;
        }
        let res = unadj + adj;
        let v = !unadj & res & 0x80 != 0;
        self.bcd_flags(res, c, v)
    }

    /// dv - sv - X en BCD (sert aussi à NBCD avec dv = 0).
    fn sbcd(&mut self, sv: u32, dv: u32) -> u32 {
        let x = (self.sr & SR_X != 0) as i32;
        let (sv, dv) = (sv as i32, dv as i32);
        let unadj = dv - sv - x;
        let mut adj = 0;
        if (dv & 0x0f) - (sv & 0x0f) - x < 0 {
            adj |= 0x06;
        }
        let c = unadj < 0 || unadj - adj < 0;
        if unadj < 0 {
            adj |= 0x60;
        }
        let res = unadj - adj;
        let v = unadj & !res & 0x80 != 0;
        self.bcd_flags(res as u32, c, v)
    }

    fn bcd_flags(&mut self, res: u32, c: bool, v: bool) -> u32 {
        let r = res & 0xff;
        let mut f = 0;
        if c {
            f |= SR_C | SR_X;
        }
        if v {
            f |= SR_V;
        }
        if r & 0x80 != 0 {
            f |= SR_N;
        }
        let z = if r != 0 { 0 } else { self.sr & SR_Z };
        self.sr = (self.sr & !0x1f) | f | z;
        r
    }

    fn chk_flags(&mut self, v: i16, bound: i16) {
        let mut f = 0;
        if v == 0 {
            f |= SR_Z;
        }
        if v < 0 {
            f |= SR_N;
        } else if v > bound {
            // N effacé
        }
        self.set_flags(SR_N | SR_Z | SR_V | SR_C, f);
    }

    fn div_zero_flags(&mut self, _ins: &Instr, _n: u8) {
        self.set_flags(SR_N | SR_Z | SR_V | SR_C, 0);
    }

    fn shift(&mut self, kind: ShiftKind, left: bool, sz: Size, v: u32, count: u32) -> u32 {
        let bits = sz.bytes() * 8;
        let m = sz.mask();
        let msb = sz.msb();
        let mut r = v & m;
        let mut c = false;
        let mut overflow = false;
        let mut x = self.sr & SR_X != 0;
        for _ in 0..count {
            match (kind, left) {
                (ShiftKind::As | ShiftKind::Ls, true) => {
                    c = r & msb != 0;
                    let nr = (r << 1) & m;
                    if (nr ^ r) & msb != 0 {
                        overflow = true;
                    }
                    r = nr;
                    x = c;
                }
                (ShiftKind::As, false) => {
                    c = r & 1 != 0;
                    r = (r >> 1) | (r & msb);
                    x = c;
                }
                (ShiftKind::Ls, false) => {
                    c = r & 1 != 0;
                    r >>= 1;
                    x = c;
                }
                (ShiftKind::Ro, true) => {
                    c = r & msb != 0;
                    r = ((r << 1) | c as u32) & m;
                }
                (ShiftKind::Ro, false) => {
                    c = r & 1 != 0;
                    r = (r >> 1) | if c { msb } else { 0 };
                }
                (ShiftKind::Rox, true) => {
                    c = r & msb != 0;
                    r = ((r << 1) | x as u32) & m;
                    x = c;
                }
                (ShiftKind::Rox, false) => {
                    c = r & 1 != 0;
                    r = (r >> 1) | if x { msb } else { 0 };
                    x = c;
                }
            }
        }
        let _ = bits;
        if count == 0 {
            // C effacé (ou copie de X pour ROXL/ROXR), X inchangé
            c = kind == ShiftKind::Rox && x;
        }
        let mut f = 0;
        if r & msb != 0 {
            f |= SR_N;
        }
        if r == 0 {
            f |= SR_Z;
        }
        if c {
            f |= SR_C;
        }
        if kind == ShiftKind::As && left && overflow {
            f |= SR_V;
        }
        let xbit = if kind == ShiftKind::Ro { self.sr & SR_X } else if x { SR_X } else { 0 };
        self.sr = (self.sr & !0x1f) | f | xbit;
        r
    }

    fn movem<B: Bus>(&mut self, bus: &mut B, size: Size, to_mem: bool, mask: u16, ea: Ea) {
        let n = size.bytes();
        if to_mem {
            if let Ea::PreDec(an) = ea {
                // masque inversé : bit 0 = A7 ... bit 15 = D0 ; on écrit de A7 vers D0
                let mut addr = self.a[an as usize];
                for i in 0..16 {
                    if mask >> i & 1 == 0 {
                        continue;
                    }
                    let reg = 15 - i;
                    let v = if reg < 8 { self.d[reg] } else { self.a[reg - 8] };
                    addr = addr.wrapping_sub(n);
                    write_mem(bus, addr, size, v);
                }
                self.a[an as usize] = addr;
            } else {
                let mut addr = self.ea_address(ea);
                for reg in 0..16 {
                    if mask >> reg & 1 == 0 {
                        continue;
                    }
                    let v = if reg < 8 { self.d[reg] } else { self.a[reg - 8] };
                    write_mem(bus, addr, size, v);
                    addr = addr.wrapping_add(n);
                }
            }
        } else {
            let (mut addr, post) = match ea {
                Ea::PostInc(an) => (self.a[an as usize], Some(an)),
                _ => (self.ea_address(ea), None),
            };
            let prog = matches!(ea, Ea::PcDisp(_) | Ea::PcIdx(..));
            for reg in 0..16 {
                if mask >> reg & 1 == 0 {
                    continue;
                }
                let loc = if prog { Loc::Prog(addr) } else { Loc::Mem(addr) };
                let v = sext(self.read(bus, loc, size), size);
                if reg < 8 {
                    self.d[reg] = v;
                } else {
                    self.a[reg - 8] = v;
                }
                addr = addr.wrapping_add(n);
            }
            if let Some(an) = post {
                self.a[an as usize] = addr;
            }
        }
    }
}

pub(crate) fn read_mem<B: Bus>(bus: &mut B, a: u32, size: Size) -> u32 {
    let a = a & ADDR_MASK;
    match size {
        Size::Byte => bus.read8(a) as u32,
        Size::Word => bus.read16(a) as u32,
        Size::Long => (bus.read16(a) as u32) << 16 | bus.read16((a + 2) & ADDR_MASK) as u32,
    }
}

pub(crate) fn write_mem<B: Bus>(bus: &mut B, a: u32, size: Size, v: u32) {
    let a = a & ADDR_MASK;
    match size {
        Size::Byte => bus.write8(a, v as u8),
        Size::Word => bus.write16(a, v as u16),
        Size::Long => {
            bus.write16(a, (v >> 16) as u16);
            bus.write16((a + 2) & ADDR_MASK, v as u16);
        }
    }
}
