//! Interpréteur Zilog Z80 (CPU son du CPS2).
//!
//! Comportements non documentés inclus (bits X/Y, registre interne WZ/MEMPTR, Q pour SCF/CCF),
//! validés contre les tests SingleStepTests/z80. Durées en T-états.

pub const FC: u8 = 0x01;
pub const FN: u8 = 0x02;
pub const FPV: u8 = 0x04;
pub const FX: u8 = 0x08;
pub const FH: u8 = 0x10;
pub const FY: u8 = 0x20;
pub const FZ: u8 = 0x40;
pub const FS: u8 = 0x80;

pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, v: u8);
    fn input(&mut self, port: u16) -> u8;
    fn output(&mut self, port: u16, v: u8);
    /// Acquittement d'interruption : valeur placée sur le bus de données (0xff par défaut).
    fn irq_ack(&mut self) -> u8 {
        0xff
    }
}

#[derive(Clone, Debug, Default)]
pub struct Z80 {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub af_: u16,
    pub bc_: u16,
    pub de_: u16,
    pub hl_: u16,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    pub i: u8,
    pub r: u8,
    pub wz: u16,
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8,
    pub halted: bool,
    /// EI vient d'être exécuté : les interruptions ne sont pas acceptées avant l'instruction suivante.
    pub ei_delay: bool,
    /// Drapeaux écrits par la dernière instruction (0 sinon) : utilisé par SCF/CCF.
    pub q: u8,
    /// Ligne INT maintenue par le matériel.
    pub irq_line: bool,
    pub cycles: u64,
}

#[derive(Clone, Copy, PartialEq)]
enum Idx {
    Hl,
    Ix,
    Iy,
}

const fn parity(v: u8) -> bool {
    v.count_ones() % 2 == 0
}

fn szxy(v: u8) -> u8 {
    let mut f = v & (FS | FX | FY);
    if v == 0 {
        f |= FZ;
    }
    f
}

fn szxyp(v: u8) -> u8 {
    szxy(v) | if parity(v) { FPV } else { 0 }
}

impl Z80 {
    pub fn new() -> Self {
        Z80 { a: 0xff, f: 0xff, sp: 0xffff, ..Default::default() }
    }

    pub fn reset(&mut self) {
        self.pc = 0;
        self.i = 0;
        self.r = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.im = 0;
        self.halted = false;
        self.ei_delay = false;
    }

    pub fn bc(&self) -> u16 {
        u16::from_be_bytes([self.b, self.c])
    }
    pub fn de(&self) -> u16 {
        u16::from_be_bytes([self.d, self.e])
    }
    pub fn hl(&self) -> u16 {
        u16::from_be_bytes([self.h, self.l])
    }
    pub fn af(&self) -> u16 {
        u16::from_be_bytes([self.a, self.f])
    }
    pub fn set_bc(&mut self, v: u16) {
        [self.b, self.c] = v.to_be_bytes();
    }
    pub fn set_de(&mut self, v: u16) {
        [self.d, self.e] = v.to_be_bytes();
    }
    pub fn set_hl(&mut self, v: u16) {
        [self.h, self.l] = v.to_be_bytes();
    }
    pub fn set_af(&mut self, v: u16) {
        [self.a, self.f] = v.to_be_bytes();
    }

    fn inc_r(&mut self) {
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7f);
    }

    fn fetch8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }
    fn fetch16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch8(bus) as u16;
        lo | (self.fetch8(bus) as u16) << 8
    }
    fn read16<B: Bus>(&mut self, bus: &mut B, a: u16) -> u16 {
        bus.read(a) as u16 | (bus.read(a.wrapping_add(1)) as u16) << 8
    }
    fn write16<B: Bus>(&mut self, bus: &mut B, a: u16, v: u16) {
        bus.write(a, v as u8);
        bus.write(a.wrapping_add(1), (v >> 8) as u8);
    }
    fn push<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, (v >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, v as u8);
    }
    fn pop<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let v = self.read16(bus, self.sp);
        self.sp = self.sp.wrapping_add(2);
        v
    }

    fn set_f(&mut self, f: u8) {
        self.f = f;
        self.q = f;
    }

    fn idx(&self, x: Idx) -> u16 {
        match x {
            Idx::Hl => self.hl(),
            Idx::Ix => self.ix,
            Idx::Iy => self.iy,
        }
    }
    fn set_idx(&mut self, x: Idx, v: u16) {
        match x {
            Idx::Hl => self.set_hl(v),
            Idx::Ix => self.ix = v,
            Idx::Iy => self.iy = v,
        }
    }

    /// Registre 8 bits n°r (B C D E H L - A), H/L remplacés par IXH/IXL sous préfixe.
    fn reg8(&self, r: u8, x: Idx) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => (self.idx(x) >> 8) as u8,
            5 => self.idx(x) as u8,
            7 => self.a,
            _ => unreachable!(),
        }
    }
    fn set_reg8(&mut self, r: u8, x: Idx, v: u8) {
        match r {
            0 => self.b = v,
            1 => self.c = v,
            2 => self.d = v,
            3 => self.e = v,
            4 => {
                let w = (self.idx(x) & 0x00ff) | (v as u16) << 8;
                self.set_idx(x, w)
            }
            5 => {
                let w = (self.idx(x) & 0xff00) | v as u16;
                self.set_idx(x, w)
            }
            7 => self.a = v,
            _ => unreachable!(),
        }
    }

    /// Paires BC DE HL SP (SP pour p=3), HL remplacé par IX/IY.
    fn rp(&self, p: u8, x: Idx) -> u16 {
        match p {
            0 => self.bc(),
            1 => self.de(),
            2 => self.idx(x),
            _ => self.sp,
        }
    }
    fn set_rp(&mut self, p: u8, x: Idx, v: u16) {
        match p {
            0 => self.set_bc(v),
            1 => self.set_de(v),
            2 => self.set_idx(x, v),
            _ => self.sp = v,
        }
    }
    /// Paires BC DE HL AF (PUSH/POP).
    fn rp2(&self, p: u8, x: Idx) -> u16 {
        if p == 3 { self.af() } else { self.rp(p, x) }
    }
    fn set_rp2(&mut self, p: u8, x: Idx, v: u16) {
        if p == 3 { self.set_af(v) } else { self.set_rp(p, x, v) }
    }

    fn cond(&self, c: u8) -> bool {
        let f = self.f;
        match c {
            0 => f & FZ == 0,
            1 => f & FZ != 0,
            2 => f & FC == 0,
            3 => f & FC != 0,
            4 => f & FPV == 0,
            5 => f & FPV != 0,
            6 => f & FS == 0,
            _ => f & FS != 0,
        }
    }

    // ---------- ALU ----------

    fn alu(&mut self, op: u8, v: u8) {
        let a = self.a;
        match op {
            0 | 1 => {
                let c = (op == 1 && self.f & FC != 0) as u16;
                let r = a as u16 + v as u16 + c;
                let rb = r as u8;
                let mut f = szxy(rb);
                if (a ^ v ^ rb) & 0x10 != 0 {
                    f |= FH;
                }
                if (!(a ^ v) & (a ^ rb) & 0x80) != 0 {
                    f |= FPV;
                }
                if r > 0xff {
                    f |= FC;
                }
                self.a = rb;
                self.set_f(f);
            }
            2 | 3 | 7 => {
                let c = (op == 3 && self.f & FC != 0) as u16;
                let r = (a as u16).wrapping_sub(v as u16).wrapping_sub(c);
                let rb = r as u8;
                let mut f = (rb & FS) | FN;
                if rb == 0 {
                    f |= FZ;
                }
                if (a ^ v ^ rb) & 0x10 != 0 {
                    f |= FH;
                }
                if ((a ^ v) & (a ^ rb) & 0x80) != 0 {
                    f |= FPV;
                }
                if r > 0xff {
                    f |= FC;
                }
                if op == 7 {
                    // CP : X/Y viennent de l'opérande
                    f |= v & (FX | FY);
                } else {
                    f |= rb & (FX | FY);
                    self.a = rb;
                }
                self.set_f(f);
            }
            4 => {
                self.a &= v;
                self.set_f(szxyp(self.a) | FH);
            }
            5 => {
                self.a ^= v;
                self.set_f(szxyp(self.a));
            }
            _ => {
                self.a |= v;
                self.set_f(szxyp(self.a));
            }
        }
    }

    fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        let mut f = (self.f & FC) | szxy(r);
        if r & 0x0f == 0 {
            f |= FH;
        }
        if r == 0x80 {
            f |= FPV;
        }
        self.set_f(f);
        r
    }

    fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        let mut f = (self.f & FC) | szxy(r) | FN;
        if v & 0x0f == 0 {
            f |= FH;
        }
        if v == 0x80 {
            f |= FPV;
        }
        self.set_f(f);
        r
    }

    fn add16(&mut self, a: u16, b: u16) -> u16 {
        let r = a as u32 + b as u32;
        let rw = r as u16;
        let mut f = self.f & (FS | FZ | FPV);
        f |= ((rw >> 8) as u8) & (FX | FY);
        if (a ^ b ^ rw) & 0x1000 != 0 {
            f |= FH;
        }
        if r > 0xffff {
            f |= FC;
        }
        self.wz = a.wrapping_add(1);
        self.set_f(f);
        rw
    }

    fn adc16(&mut self, a: u16, b: u16) -> u16 {
        let c = (self.f & FC) as u32;
        let r = a as u32 + b as u32 + c;
        let rw = r as u16;
        let mut f = ((rw >> 8) as u8) & (FS | FX | FY);
        if rw == 0 {
            f |= FZ;
        }
        if (a ^ b ^ rw) & 0x1000 != 0 {
            f |= FH;
        }
        if (!(a ^ b) & (a ^ rw) & 0x8000) != 0 {
            f |= FPV;
        }
        if r > 0xffff {
            f |= FC;
        }
        self.wz = a.wrapping_add(1);
        self.set_f(f);
        rw
    }

    fn sbc16(&mut self, a: u16, b: u16) -> u16 {
        let c = (self.f & FC) as u32;
        let r = (a as u32).wrapping_sub(b as u32).wrapping_sub(c);
        let rw = r as u16;
        let mut f = (((rw >> 8) as u8) & (FS | FX | FY)) | FN;
        if rw == 0 {
            f |= FZ;
        }
        if (a ^ b ^ rw) & 0x1000 != 0 {
            f |= FH;
        }
        if ((a ^ b) & (a ^ rw) & 0x8000) != 0 {
            f |= FPV;
        }
        if r > 0xffff {
            f |= FC;
        }
        self.wz = a.wrapping_add(1);
        self.set_f(f);
        rw
    }

    /// Rotations/décalages CB (RLC RRC RL RR SLA SRA SLL SRL).
    fn rot(&mut self, op: u8, v: u8) -> u8 {
        let c = self.f & FC != 0;
        let (r, co) = match op {
            0 => (v.rotate_left(1), v & 0x80 != 0),
            1 => (v.rotate_right(1), v & 1 != 0),
            2 => ((v << 1) | c as u8, v & 0x80 != 0),
            3 => ((v >> 1) | (c as u8) << 7, v & 1 != 0),
            4 => (v << 1, v & 0x80 != 0),
            5 => ((v >> 1) | (v & 0x80), v & 1 != 0),
            6 => ((v << 1) | 1, v & 0x80 != 0),
            _ => (v >> 1, v & 1 != 0),
        };
        self.set_f(szxyp(r) | co as u8);
        r
    }

    fn bit(&mut self, n: u8, v: u8, xy: u8) {
        let r = v & (1 << n);
        let mut f = (self.f & FC) | FH | (xy & (FX | FY));
        if r == 0 {
            f |= FZ | FPV;
        }
        if n == 7 && r != 0 {
            f |= FS;
        }
        self.set_f(f);
    }

    fn daa(&mut self) {
        let a = self.a;
        let mut corr = 0u8;
        let mut carry = self.f & FC != 0;
        if self.f & FH != 0 || a & 0x0f > 9 {
            corr |= 0x06;
        }
        if carry || a > 0x99 {
            corr |= 0x60;
            carry = true;
        }
        let r = if self.f & FN != 0 { a.wrapping_sub(corr) } else { a.wrapping_add(corr) };
        let mut f = szxyp(r) | (self.f & FN) | carry as u8;
        if (a ^ r) & 0x10 != 0 {
            f |= FH;
        }
        self.a = r;
        self.set_f(f);
    }

    // ---------- exécution ----------

    /// Exécute une instruction (ou prend une interruption). Renvoie les T-états consommés.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let start = self.cycles;
        if self.irq_line && self.iff1 && !self.ei_delay {
            self.interrupt(bus);
            return (self.cycles - start) as u32;
        }
        self.ei_delay = false;
        if self.halted {
            self.inc_r();
            self.cycles += 4;
            return 4;
        }
        let q_before = self.q;
        self.q = 0;
        let op = self.fetch_m1(bus);
        self.exec_main(bus, op, Idx::Hl, q_before);
        (self.cycles - start) as u32
    }

    fn fetch_m1<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.inc_r();
        self.cycles += 4;
        self.fetch8(bus)
    }

    fn interrupt<B: Bus>(&mut self, bus: &mut B) {
        self.halted = false;
        self.iff1 = false;
        self.iff2 = false;
        self.inc_r();
        self.q = 0;
        let data = bus.irq_ack();
        match self.im {
            2 => {
                self.push(bus, self.pc);
                let v = (self.i as u16) << 8 | data as u16;
                self.pc = self.read16(bus, v);
                self.cycles += 19;
            }
            _ => {
                // IM 1, et IM 0 avec RST 38h sur le bus (0xff)
                self.push(bus, self.pc);
                self.pc = if self.im == 1 { 0x38 } else { (data & 0x38) as u16 };
                self.cycles += 13;
            }
        }
        self.wz = self.pc;
    }

    /// Adresse (HL) ou (IX+d)/(IY+d) ; lit le déplacement si préfixé.
    fn mem_addr<B: Bus>(&mut self, bus: &mut B, x: Idx) -> u16 {
        match x {
            Idx::Hl => self.hl(),
            _ => {
                let d = self.fetch8(bus) as i8 as i16 as u16;
                let a = self.idx(x).wrapping_add(d);
                self.wz = a;
                a
            }
        }
    }

    fn exec_main<B: Bus>(&mut self, bus: &mut B, op: u8, x: Idx, q_before: u8) {
        let xx = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        let p = y >> 1;
        let qb = y & 1;
        let pre = x != Idx::Hl;
        match xx {
            0 => match z {
                0 => match y {
                    0 => {}
                    1 => {
                        let t = self.af();
                        self.set_af(self.af_);
                        self.af_ = t;
                    }
                    2 => {
                        // DJNZ
                        self.cycles += 1;
                        let d = self.fetch8(bus) as i8;
                        self.b = self.b.wrapping_sub(1);
                        if self.b != 0 {
                            self.pc = self.pc.wrapping_add(d as i16 as u16);
                            self.wz = self.pc;
                            self.cycles += 8;
                        } else {
                            self.cycles += 3;
                        }
                    }
                    3 => {
                        let d = self.fetch8(bus) as i8;
                        self.pc = self.pc.wrapping_add(d as i16 as u16);
                        self.wz = self.pc;
                        self.cycles += 8;
                    }
                    _ => {
                        let d = self.fetch8(bus) as i8;
                        if self.cond(y - 4) {
                            self.pc = self.pc.wrapping_add(d as i16 as u16);
                            self.wz = self.pc;
                            self.cycles += 8;
                        } else {
                            self.cycles += 3;
                        }
                    }
                },
                1 => {
                    if qb == 0 {
                        let v = self.fetch16(bus);
                        self.set_rp(p, x, v);
                        self.cycles += 6;
                    } else {
                        let r = self.add16(self.idx(x), self.rp(p, x));
                        self.set_idx(x, r);
                        self.cycles += 7;
                    }
                }
                2 => {
                    self.cycles += 3;
                    match (qb, p) {
                        (0, 0) | (0, 1) => {
                            let a = if p == 0 { self.bc() } else { self.de() };
                            bus.write(a, self.a);
                            self.wz = ((self.a as u16) << 8) | (a.wrapping_add(1) & 0xff);
                        }
                        (1, 0) | (1, 1) => {
                            let a = if p == 0 { self.bc() } else { self.de() };
                            self.a = bus.read(a);
                            self.wz = a.wrapping_add(1);
                        }
                        (0, 2) => {
                            let a = self.fetch16(bus);
                            let v = self.idx(x);
                            self.write16(bus, a, v);
                            self.wz = a.wrapping_add(1);
                            self.cycles += 9;
                        }
                        (1, 2) => {
                            let a = self.fetch16(bus);
                            let v = self.read16(bus, a);
                            self.set_idx(x, v);
                            self.wz = a.wrapping_add(1);
                            self.cycles += 9;
                        }
                        (0, _) => {
                            let a = self.fetch16(bus);
                            bus.write(a, self.a);
                            self.wz = ((self.a as u16) << 8) | (a.wrapping_add(1) & 0xff);
                            self.cycles += 6;
                        }
                        _ => {
                            let a = self.fetch16(bus);
                            self.a = bus.read(a);
                            self.wz = a.wrapping_add(1);
                            self.cycles += 6;
                        }
                    }
                }
                3 => {
                    let v = self.rp(p, x);
                    self.set_rp(p, x, if qb == 0 { v.wrapping_add(1) } else { v.wrapping_sub(1) });
                    self.cycles += 2;
                }
                4 | 5 => {
                    if y == 6 {
                        let a = self.mem_addr(bus, x);
                        let v = bus.read(a);
                        let r = if z == 4 { self.inc8(v) } else { self.dec8(v) };
                        bus.write(a, r);
                        self.cycles += if pre { 15 } else { 7 };
                    } else {
                        let v = self.reg8(y, x);
                        let r = if z == 4 { self.inc8(v) } else { self.dec8(v) };
                        self.set_reg8(y, x, r);
                    }
                }
                6 => {
                    if y == 6 {
                        let a = self.mem_addr(bus, x);
                        let v = self.fetch8(bus);
                        bus.write(a, v);
                        self.cycles += if pre { 11 } else { 6 };
                    } else {
                        let v = self.fetch8(bus);
                        self.set_reg8(y, x, v);
                        self.cycles += 3;
                    }
                }
                _ => match y {
                    0..=3 => {
                        let a = self.a;
                        let (r, c) = match y {
                            0 => (a.rotate_left(1), a & 0x80 != 0),
                            1 => (a.rotate_right(1), a & 1 != 0),
                            2 => ((a << 1) | (self.f & FC), a & 0x80 != 0),
                            _ => ((a >> 1) | (self.f & FC) << 7, a & 1 != 0),
                        };
                        self.a = r;
                        self.set_f((self.f & (FS | FZ | FPV)) | (r & (FX | FY)) | c as u8);
                    }
                    4 => self.daa(),
                    5 => {
                        self.a = !self.a;
                        self.set_f((self.f & (FS | FZ | FPV | FC)) | FH | FN | (self.a & (FX | FY)));
                    }
                    6 => {
                        let xy = ((q_before ^ self.f) | self.a) & (FX | FY);
                        self.set_f((self.f & (FS | FZ | FPV)) | xy | FC);
                    }
                    _ => {
                        let xy = ((q_before ^ self.f) | self.a) & (FX | FY);
                        let c = self.f & FC;
                        let h = if c != 0 { FH } else { 0 };
                        self.set_f((self.f & (FS | FZ | FPV)) | xy | h | (c ^ FC));
                    }
                },
            },
            1 => {
                if y == 6 && z == 6 {
                    self.halted = true;
                } else if y == 6 {
                    let a = self.mem_addr(bus, x);
                    bus.write(a, self.reg8(z, Idx::Hl));
                    self.cycles += if pre { 11 } else { 3 };
                } else if z == 6 {
                    let a = self.mem_addr(bus, x);
                    let v = bus.read(a);
                    self.set_reg8(y, Idx::Hl, v);
                    self.cycles += if pre { 11 } else { 3 };
                } else {
                    let v = self.reg8(z, x);
                    self.set_reg8(y, x, v);
                }
            }
            2 => {
                let v = if z == 6 {
                    let a = self.mem_addr(bus, x);
                    self.cycles += if pre { 11 } else { 3 };
                    bus.read(a)
                } else {
                    self.reg8(z, x)
                };
                self.alu(y, v);
            }
            _ => match z {
                0 => {
                    self.cycles += 1;
                    if self.cond(y) {
                        self.pc = self.pop(bus);
                        self.wz = self.pc;
                        self.cycles += 6;
                    }
                }
                1 => {
                    if qb == 0 {
                        let v = self.pop(bus);
                        self.set_rp2(p, x, v);
                        self.cycles += 6;
                    } else {
                        match p {
                            0 => {
                                self.pc = self.pop(bus);
                                self.wz = self.pc;
                                self.cycles += 6;
                            }
                            1 => {
                                let t = (self.bc(), self.de(), self.hl());
                                self.set_bc(self.bc_);
                                self.set_de(self.de_);
                                self.set_hl(self.hl_);
                                (self.bc_, self.de_, self.hl_) = t;
                            }
                            2 => self.pc = self.idx(x),
                            _ => {
                                self.sp = self.idx(x);
                                self.cycles += 2;
                            }
                        }
                    }
                }
                2 => {
                    let a = self.fetch16(bus);
                    self.wz = a;
                    if self.cond(y) {
                        self.pc = a;
                    }
                    self.cycles += 6;
                }
                3 => match y {
                    0 => {
                        self.pc = self.fetch16(bus);
                        self.wz = self.pc;
                        self.cycles += 6;
                    }
                    1 => self.exec_cb(bus, x),
                    2 => {
                        let n = self.fetch8(bus);
                        let port = (self.a as u16) << 8 | n as u16;
                        bus.output(port, self.a);
                        self.wz = ((self.a as u16) << 8) | (n.wrapping_add(1) as u16);
                        self.cycles += 7;
                    }
                    3 => {
                        let n = self.fetch8(bus);
                        let port = (self.a as u16) << 8 | n as u16;
                        self.a = bus.input(port);
                        self.wz = port.wrapping_add(1);
                        self.cycles += 7;
                    }
                    4 => {
                        let v = self.read16(bus, self.sp);
                        let r = self.idx(x);
                        self.write16(bus, self.sp, r);
                        self.set_idx(x, v);
                        self.wz = v;
                        self.cycles += 15;
                    }
                    5 => {
                        let t = self.de();
                        self.set_de(self.hl());
                        self.set_hl(t);
                    }
                    6 => {
                        self.iff1 = false;
                        self.iff2 = false;
                    }
                    _ => {
                        self.iff1 = true;
                        self.iff2 = true;
                        self.ei_delay = true;
                    }
                },
                4 => {
                    let a = self.fetch16(bus);
                    self.wz = a;
                    if self.cond(y) {
                        self.push(bus, self.pc);
                        self.pc = a;
                        self.cycles += 13;
                    } else {
                        self.cycles += 6;
                    }
                }
                5 => {
                    if qb == 0 {
                        self.cycles += 1;
                        let v = self.rp2(p, x);
                        self.push(bus, v);
                        self.cycles += 6;
                    } else {
                        match p {
                            0 => {
                                let a = self.fetch16(bus);
                                self.wz = a;
                                self.push(bus, self.pc);
                                self.pc = a;
                                self.cycles += 13;
                            }
                            // le préfixe compte comme une instruction : Q repart de 0
                            1 => self.prefix(bus, Idx::Ix, 0),
                            2 => self.exec_ed(bus),
                            _ => self.prefix(bus, Idx::Iy, 0),
                        }
                    }
                }
                6 => {
                    let v = self.fetch8(bus);
                    self.alu(y, v);
                    self.cycles += 3;
                }
                _ => {
                    self.cycles += 1;
                    self.push(bus, self.pc);
                    self.pc = (y as u16) * 8;
                    self.wz = self.pc;
                    self.cycles += 6;
                }
            },
        }
    }

    /// Préfixe DD/FD : l'opcode suivant utilise IX/IY.
    fn prefix<B: Bus>(&mut self, bus: &mut B, x: Idx, q_before: u8) {
        let op = self.fetch_m1(bus);
        match op {
            // préfixe suivant : il remplace celui-ci
            0xdd | 0xfd | 0xed => {
                self.pc = self.pc.wrapping_sub(1);
                self.r = (self.r & 0x80) | (self.r.wrapping_sub(1) & 0x7f);
                self.cycles -= 4;
            }
            0xcb => self.exec_ddcb(bus, x),
            _ => self.exec_main(bus, op, x, q_before),
        }
    }

    fn exec_cb<B: Bus>(&mut self, bus: &mut B, _x: Idx) {
        let op = self.fetch_m1(bus);
        let xx = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        if z == 6 {
            let a = self.hl();
            let v = bus.read(a);
            match xx {
                0 => {
                    let r = self.rot(y, v);
                    bus.write(a, r);
                    self.cycles += 7;
                }
                1 => {
                    self.bit(y, v, (self.wz >> 8) as u8);
                    self.cycles += 4;
                }
                2 => {
                    bus.write(a, v & !(1 << y));
                    self.cycles += 7;
                }
                _ => {
                    bus.write(a, v | (1 << y));
                    self.cycles += 7;
                }
            }
        } else {
            let v = self.reg8(z, Idx::Hl);
            match xx {
                0 => {
                    let r = self.rot(y, v);
                    self.set_reg8(z, Idx::Hl, r);
                }
                1 => self.bit(y, v, v),
                2 => self.set_reg8(z, Idx::Hl, v & !(1 << y)),
                _ => self.set_reg8(z, Idx::Hl, v | (1 << y)),
            }
        }
    }

    fn exec_ddcb<B: Bus>(&mut self, bus: &mut B, x: Idx) {
        let d = self.fetch8(bus) as i8 as i16 as u16;
        let op = self.fetch8(bus);
        let a = self.idx(x).wrapping_add(d);
        self.wz = a;
        let xx = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        let v = bus.read(a);
        if xx == 1 {
            self.bit(y, v, (a >> 8) as u8);
            self.cycles += 12;
            return;
        }
        let r = match xx {
            0 => self.rot(y, v),
            2 => v & !(1 << y),
            _ => v | (1 << y),
        };
        bus.write(a, r);
        // copie non documentée dans un registre
        if z != 6 {
            self.set_reg8(z, Idx::Hl, r);
        }
        self.cycles += 15;
    }

    fn exec_ed<B: Bus>(&mut self, bus: &mut B) {
        let op = self.fetch_m1(bus);
        let xx = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;
        let p = y >> 1;
        let qb = y & 1;
        if xx == 1 {
            match z {
                0 => {
                    let v = bus.input(self.bc());
                    self.wz = self.bc().wrapping_add(1);
                    if y != 6 {
                        self.set_reg8(y, Idx::Hl, v);
                    }
                    self.set_f((self.f & FC) | szxyp(v));
                    self.cycles += 4;
                }
                1 => {
                    let v = if y == 6 { 0 } else { self.reg8(y, Idx::Hl) };
                    bus.output(self.bc(), v);
                    self.wz = self.bc().wrapping_add(1);
                    self.cycles += 4;
                }
                2 => {
                    let hl = self.hl();
                    let v = self.rp(p, Idx::Hl);
                    let r = if qb == 0 { self.sbc16(hl, v) } else { self.adc16(hl, v) };
                    self.set_hl(r);
                    self.cycles += 7;
                }
                3 => {
                    let a = self.fetch16(bus);
                    self.wz = a.wrapping_add(1);
                    if qb == 0 {
                        let v = self.rp(p, Idx::Hl);
                        self.write16(bus, a, v);
                    } else {
                        let v = self.read16(bus, a);
                        self.set_rp(p, Idx::Hl, v);
                    }
                    self.cycles += 12;
                }
                4 => {
                    let v = self.a;
                    self.a = 0;
                    self.alu(2, v);
                }
                5 => {
                    // RETN / RETI
                    self.pc = self.pop(bus);
                    self.wz = self.pc;
                    self.iff1 = self.iff2;
                    self.cycles += 6;
                }
                6 => {
                    self.im = match y & 3 {
                        0 | 1 => 0,
                        2 => 1,
                        _ => 2,
                    };
                }
                _ => match y {
                    0 => {
                        self.i = self.a;
                        self.cycles += 1;
                    }
                    1 => {
                        self.r = self.a;
                        self.cycles += 1;
                    }
                    2 | 3 => {
                        self.a = if y == 2 { self.i } else { self.r };
                        let mut f = (self.f & FC) | szxy(self.a);
                        if self.iff2 {
                            f |= FPV;
                        }
                        self.set_f(f);
                        self.cycles += 1;
                    }
                    4 | 5 => {
                        // RRD / RLD
                        let hl = self.hl();
                        let m = bus.read(hl);
                        let a = self.a;
                        let (na, nm) = if y == 4 {
                            ((a & 0xf0) | (m & 0x0f), (a << 4) | (m >> 4))
                        } else {
                            ((a & 0xf0) | (m >> 4), (m << 4) | (a & 0x0f))
                        };
                        bus.write(hl, nm);
                        self.a = na;
                        self.wz = hl.wrapping_add(1);
                        self.set_f((self.f & FC) | szxyp(na));
                        self.cycles += 10;
                    }
                    _ => {}
                },
            }
            return;
        }
        if xx == 2 && z <= 3 && y >= 4 {
            self.block(bus, y, z);
        }
        // autres opcodes ED : NOP de 8 T-états
    }

    fn block<B: Bus>(&mut self, bus: &mut B, y: u8, z: u8) {
        let inc = y & 1 == 0;
        let repeat = y >= 6;
        let step = |v: u16| if inc { v.wrapping_add(1) } else { v.wrapping_sub(1) };
        match z {
            0 => {
                // LDI/LDD/LDIR/LDDR
                let v = bus.read(self.hl());
                bus.write(self.de(), v);
                self.set_hl(step(self.hl()));
                self.set_de(step(self.de()));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                let n = v.wrapping_add(self.a);
                let mut f = (self.f & (FS | FZ | FC)) | (n & FX) | ((n & 0x02) << 4);
                if bc != 0 {
                    f |= FPV;
                }
                self.cycles += 8;
                if repeat && bc != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    self.wz = self.pc.wrapping_add(1);
                    f = (f & !(FX | FY)) | ((self.pc >> 8) as u8 & (FX | FY));
                    self.cycles += 5;
                }
                self.set_f(f);
            }
            1 => {
                // CPI/CPD/CPIR/CPDR
                let v = bus.read(self.hl());
                let r = self.a.wrapping_sub(v);
                self.set_hl(step(self.hl()));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                self.wz = step(self.wz);
                let h = (self.a ^ v ^ r) & 0x10 != 0;
                let n = r.wrapping_sub(h as u8);
                let mut f = (self.f & FC) | FN | (r & FS) | (n & FX) | ((n & 0x02) << 4);
                if r == 0 {
                    f |= FZ;
                }
                if h {
                    f |= FH;
                }
                if bc != 0 {
                    f |= FPV;
                }
                self.cycles += 8;
                if repeat && bc != 0 && r != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    self.wz = self.pc.wrapping_add(1);
                    f = (f & !(FX | FY)) | ((self.pc >> 8) as u8 & (FX | FY));
                    self.cycles += 5;
                }
                self.set_f(f);
            }
            2 | 3 => {
                // INI/IND/INIR/INDR, OUTI/OUTD/OTIR/OTDR
                let (v, k) = if z == 2 {
                    self.cycles += 1;
                    let v = bus.input(self.bc());
                    self.wz = step(self.bc());
                    bus.write(self.hl(), v);
                    self.b = self.b.wrapping_sub(1);
                    self.set_hl(step(self.hl()));
                    let c = if inc { self.c.wrapping_add(1) } else { self.c.wrapping_sub(1) };
                    (v, v as u16 + c as u16)
                } else {
                    self.cycles += 1;
                    let v = bus.read(self.hl());
                    self.b = self.b.wrapping_sub(1);
                    self.wz = step(self.bc());
                    bus.output(self.bc(), v);
                    self.set_hl(step(self.hl()));
                    (v, v as u16 + self.l as u16)
                };
                let b = self.b;
                let mut f = szxy(b);
                if v & 0x80 != 0 {
                    f |= FN;
                }
                if k > 0xff {
                    f |= FH | FC;
                }
                let mut pv = parity(((k & 7) as u8) ^ b);
                self.cycles += 7;
                if repeat && b != 0 {
                    self.pc = self.pc.wrapping_sub(2);
                    self.wz = self.pc.wrapping_add(1);
                    f = (f & !(FX | FY)) | ((self.pc >> 8) as u8 & (FX | FY));
                    // effets supplémentaires de la répétition (P. Rak)
                    if f & FC != 0 {
                        if v & 0x80 != 0 {
                            pv ^= !parity(b.wrapping_sub(1) & 7);
                            f = (f & !FH) | if b & 0x0f == 0 { FH } else { 0 };
                        } else {
                            pv ^= !parity(b.wrapping_add(1) & 7);
                            f = (f & !FH) | if b & 0x0f == 0x0f { FH } else { 0 };
                        }
                    } else {
                        pv ^= !parity(b & 7);
                    }
                    self.cycles += 5;
                }
                if pv {
                    f |= FPV;
                }
                self.set_f(f);
            }
            _ => unreachable!(),
        }
    }
}
