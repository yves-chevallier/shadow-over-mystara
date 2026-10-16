//! Décodage 68000 : opcode + mots d'extension -> instruction entièrement résolue.
//! Partagé par l'interpréteur et le recompilateur.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Byte,
    Word,
    Long,
}

impl Size {
    pub fn bytes(self) -> u32 {
        match self {
            Size::Byte => 1,
            Size::Word => 2,
            Size::Long => 4,
        }
    }
    pub fn mask(self) -> u32 {
        match self {
            Size::Byte => 0xff,
            Size::Word => 0xffff,
            Size::Long => 0xffff_ffff,
        }
    }
    pub fn msb(self) -> u32 {
        match self {
            Size::Byte => 0x80,
            Size::Word => 0x8000,
            Size::Long => 0x8000_0000,
        }
    }
}

/// Mode d'adressage effectif, avec ses mots d'extension déjà lus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ea {
    Dn(u8),
    An(u8),
    Ind(u8),
    PostInc(u8),
    PreDec(u8),
    Disp(u8, i16),
    /// (d8,An,Xn) : mot d'extension brut
    Idx(u8, u16),
    /// (xxx).w : adresse étendue en signe.
    AbsW(u32),
    /// (xxx).l
    AbsL(u32),
    /// (d16,PC) : adresse cible calculée au décodage. Lu en espace programme.
    PcDisp(u32),
    /// (d8,PC,Xn) : base (adresse du mot d'extension + d8) et mot d'extension. Espace programme.
    PcIdx(u32, u16),
    Imm(u32),
}

impl Ea {
    pub fn is_mem(self) -> bool {
        !matches!(self, Ea::Dn(_) | Ea::An(_) | Ea::Imm(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitOp {
    Btst,
    Bchg,
    Bclr,
    Bset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftKind {
    As,
    Ls,
    Rox,
    Ro,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftCount {
    Imm(u8),
    Reg(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    // arithmétique / logique : src, dst
    Add(Size, Ea, Ea),
    Adda(Size, Ea, u8),
    Addx(Size, Ea, Ea),
    Sub(Size, Ea, Ea),
    Suba(Size, Ea, u8),
    Subx(Size, Ea, Ea),
    Cmp(Size, Ea, Ea),
    Cmpa(Size, Ea, u8),
    And(Size, Ea, Ea),
    Or(Size, Ea, Ea),
    Eor(Size, Ea, Ea),
    AndiCcr(u8),
    AndiSr(u16),
    OriCcr(u8),
    OriSr(u16),
    EoriCcr(u8),
    EoriSr(u16),
    Neg(Size, Ea),
    Negx(Size, Ea),
    Not(Size, Ea),
    Clr(Size, Ea),
    Tst(Size, Ea),
    Tas(Ea),
    Ext(Size, u8),
    Swap(u8),
    Exg(u8, u8, u8), // mode (0x08 D-D, 0x09 A-A, 0x11 D-A), rx, ry
    Mulu(Ea, u8),
    Muls(Ea, u8),
    Divu(Ea, u8),
    Divs(Ea, u8),
    Abcd(Ea, Ea),
    Sbcd(Ea, Ea),
    Nbcd(Ea),
    Chk(Ea, u8),
    // transferts
    Move(Size, Ea, Ea),
    Movea(Size, Ea, u8),
    Moveq(i8, u8),
    Movem { size: Size, to_mem: bool, mask: u16, ea: Ea },
    Movep { size: Size, to_mem: bool, dn: u8, an: u8, disp: i16 },
    MoveFromSr(Ea),
    MoveToSr(Ea),
    MoveToCcr(Ea),
    MoveUsp { to_usp: bool, an: u8 },
    Lea(Ea, u8),
    Pea(Ea),
    Link(u8, i16),
    Unlk(u8),
    // bits / décalages
    Bit(BitOp, ShiftCount, Ea),
    Shift { kind: ShiftKind, left: bool, size: Size, count: ShiftCount, dn: u8 },
    ShiftMem { kind: ShiftKind, left: bool, ea: Ea },
    // flot de contrôle
    Bra(u32),
    Bsr(u32),
    Bcc(u8, u32),
    Dbcc(u8, u8, u32),
    Scc(u8, Ea),
    Jmp(Ea),
    Jsr(Ea),
    Rts,
    Rte,
    Rtr,
    Trap(u8),
    Trapv,
    Nop,
    Reset,
    Stop(u16),
    Illegal,
    LineA,
    LineF,
}

#[derive(Clone, Copy, Debug)]
pub struct Instr {
    pub op: Op,
    /// Adresse de l'opcode.
    pub pc: u32,
    /// Longueur en octets (opcode + extensions).
    pub len: u32,
    /// Mot d'opcode brut (distingue par ex. ADDQ d'ADDI pour le timing).
    pub opcode: u16,
}

/// Source des mots d'instruction (espace programme).
pub trait Fetch {
    fn fetch16(&mut self, addr: u32) -> u16;
}

struct Reader<'a, F: Fetch> {
    f: &'a mut F,
    pc: u32,
}

impl<F: Fetch> Reader<'_, F> {
    fn w(&mut self) -> u16 {
        let v = self.f.fetch16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        v
    }
    fn l(&mut self) -> u32 {
        let hi = self.w() as u32;
        (hi << 16) | self.w() as u32
    }

    /// Lit un mode d'adressage (mode, reg) ; None si invalide pour la taille.
    fn ea(&mut self, mode: u16, reg: u16, size: Size) -> Option<Ea> {
        let r = reg as u8;
        Some(match mode {
            0 => Ea::Dn(r),
            1 => Ea::An(r),
            2 => Ea::Ind(r),
            3 => Ea::PostInc(r),
            4 => Ea::PreDec(r),
            5 => Ea::Disp(r, self.w() as i16),
            6 => Ea::Idx(r, self.w()),
            7 => match reg {
                0 => Ea::AbsW(self.w() as i16 as i32 as u32),
                1 => Ea::AbsL(self.l()),
                2 => {
                    let at = self.pc;
                    Ea::PcDisp(at.wrapping_add(self.w() as i16 as i32 as u32))
                }
                3 => {
                    let at = self.pc;
                    let ext = self.w();
                    Ea::PcIdx(at.wrapping_add(ext as i8 as i32 as u32), ext)
                }
                4 => Ea::Imm(match size {
                    Size::Byte => (self.w() & 0xff) as u32,
                    Size::Word => self.w() as u32,
                    Size::Long => self.l(),
                }),
                _ => return None,
            },
            _ => unreachable!(),
        })
    }
}

// Classes de modes autorisés
fn is_data(ea: Ea) -> bool {
    !matches!(ea, Ea::An(_))
}
fn is_alterable(ea: Ea) -> bool {
    !matches!(ea, Ea::Imm(_) | Ea::PcDisp(_) | Ea::PcIdx(..))
}
fn is_data_alt(ea: Ea) -> bool {
    is_data(ea) && is_alterable(ea)
}
fn is_mem_alt(ea: Ea) -> bool {
    ea.is_mem() && is_alterable(ea)
}
fn is_control(ea: Ea) -> bool {
    matches!(ea, Ea::Ind(_) | Ea::Disp(..) | Ea::Idx(..) | Ea::AbsW(_) | Ea::AbsL(_) | Ea::PcDisp(_) | Ea::PcIdx(..))
}

fn size2(bits: u16) -> Option<Size> {
    match bits & 3 {
        0 => Some(Size::Byte),
        1 => Some(Size::Word),
        2 => Some(Size::Long),
        _ => None,
    }
}

/// Décode l'instruction à `pc`. Les encodages invalides donnent `Op::Illegal` (ou LineA/LineF).
pub fn decode<F: Fetch>(f: &mut F, pc: u32) -> Instr {
    let mut r = Reader { f, pc };
    let op = r.w();
    let o = decode_op(&mut r, op).unwrap_or(Op::Illegal);
    let len = r.pc.wrapping_sub(pc);
    // Une instruction invalide n'a que son opcode
    let len = if matches!(o, Op::Illegal | Op::LineA | Op::LineF) { 2 } else { len };
    Instr { op: o, pc, len, opcode: op }
}

fn decode_op<F: Fetch>(r: &mut Reader<F>, op: u16) -> Option<Op> {
    let mode = (op >> 3) & 7;
    let reg = op & 7;
    let rx = ((op >> 9) & 7) as u8;
    let ry = reg as u8;
    Some(match op >> 12 {
        0x0 => {
            if op & 0x0138 == 0x0108 {
                let disp = r.w() as i16;
                let size = if op & 0x40 != 0 { Size::Long } else { Size::Word };
                return Some(Op::Movep { size, to_mem: op & 0x80 != 0, dn: rx, an: ry, disp });
            }
            if op & 0x0100 != 0 {
                let kind = bitop(op);
                let ea = r.ea(mode, reg, Size::Byte)?;
                let ok = if kind == BitOp::Btst { is_data(ea) } else { is_data_alt(ea) };
                if !ok {
                    return None;
                }
                return Some(Op::Bit(kind, ShiftCount::Reg(rx), ea));
            }
            if op & 0xff00 == 0x0800 {
                let kind = bitop(op);
                let n = (r.w() & 0xff) as u8;
                let ea = r.ea(mode, reg, Size::Byte)?;
                let ok = if kind == BitOp::Btst {
                    is_data(ea) && !matches!(ea, Ea::Imm(_))
                } else {
                    is_data_alt(ea)
                };
                if !ok {
                    return None;
                }
                return Some(Op::Bit(kind, ShiftCount::Imm(n), ea));
            }
            // immédiats
            match op {
                0x003c => return Some(Op::OriCcr(r.w() as u8)),
                0x007c => return Some(Op::OriSr(r.w())),
                0x023c => return Some(Op::AndiCcr(r.w() as u8)),
                0x027c => return Some(Op::AndiSr(r.w())),
                0x0a3c => return Some(Op::EoriCcr(r.w() as u8)),
                0x0a7c => return Some(Op::EoriSr(r.w())),
                _ => {}
            }
            let size = size2(op >> 6)?;
            let imm = r.ea(7, 4, size)?;
            let dst = r.ea(mode, reg, size)?;
            match (op >> 9) & 7 {
                0 if is_data_alt(dst) => Op::Or(size, imm, dst),
                1 if is_data_alt(dst) => Op::And(size, imm, dst),
                2 if is_data_alt(dst) => Op::Sub(size, imm, dst),
                3 if is_data_alt(dst) => Op::Add(size, imm, dst),
                5 if is_data_alt(dst) => Op::Eor(size, imm, dst),
                6 if is_data_alt(dst) => Op::Cmp(size, imm, dst),
                _ => return None,
            }
        }
        0x1 | 0x2 | 0x3 => {
            let size = match op >> 12 {
                1 => Size::Byte,
                3 => Size::Word,
                _ => Size::Long,
            };
            let src = r.ea(mode, reg, size)?;
            if size == Size::Byte && matches!(src, Ea::An(_)) {
                return None;
            }
            let dmode = (op >> 6) & 7;
            if dmode == 1 {
                if size == Size::Byte {
                    return None;
                }
                return Some(Op::Movea(size, src, rx));
            }
            let dst = r.ea(dmode, rx as u16, size)?;
            if !is_data_alt(dst) {
                return None;
            }
            Op::Move(size, src, dst)
        }
        0x4 => decode_4(r, op, mode, reg, rx)?,
        0x5 => {
            if (op >> 6) & 3 == 3 {
                let cond = ((op >> 8) & 0xf) as u8;
                if mode == 1 {
                    let at = r.pc;
                    let disp = r.w() as i16 as i32 as u32;
                    return Some(Op::Dbcc(cond, ry, at.wrapping_add(disp)));
                }
                let ea = r.ea(mode, reg, Size::Byte)?;
                if !is_data_alt(ea) {
                    return None;
                }
                return Some(Op::Scc(cond, ea));
            }
            let size = size2(op >> 6)?;
            let q = if rx == 0 { 8 } else { rx as u32 };
            let dst = r.ea(mode, reg, size)?;
            if !is_alterable(dst) || (size == Size::Byte && matches!(dst, Ea::An(_))) {
                return None;
            }
            if let Ea::An(n) = dst {
                // ADDQ/SUBQ sur An : toujours long, sans flags
                return Some(if op & 0x100 == 0 {
                    Op::Adda(Size::Long, Ea::Imm(q), n)
                } else {
                    Op::Suba(Size::Long, Ea::Imm(q), n)
                });
            }
            if op & 0x100 == 0 {
                Op::Add(size, Ea::Imm(q), dst)
            } else {
                Op::Sub(size, Ea::Imm(q), dst)
            }
        }
        0x6 => {
            let at = r.pc;
            let d8 = op as u8;
            let disp = if d8 == 0 { r.w() as i16 as i32 } else { d8 as i8 as i32 };
            let target = at.wrapping_add(disp as u32);
            match (op >> 8) & 0xf {
                0 => Op::Bra(target),
                1 => Op::Bsr(target),
                c => Op::Bcc(c as u8, target),
            }
        }
        0x7 => {
            if op & 0x100 != 0 {
                return None;
            }
            Op::Moveq(op as u8 as i8, rx)
        }
        0x8 | 0xc => {
            let is_and = op >> 12 == 0xc;
            let opmode = (op >> 6) & 7;
            if opmode == 3 || opmode == 7 {
                let src = r.ea(mode, reg, Size::Word)?;
                if !is_data(src) {
                    return None;
                }
                return Some(match (is_and, opmode == 7) {
                    (false, false) => Op::Divu(src, rx),
                    (false, true) => Op::Divs(src, rx),
                    (true, false) => Op::Mulu(src, rx),
                    (true, true) => Op::Muls(src, rx),
                });
            }
            if op & 0x1f0 == 0x100 {
                let (s, d) = if mode & 1 == 0 {
                    (Ea::Dn(ry), Ea::Dn(rx))
                } else {
                    (Ea::PreDec(ry), Ea::PreDec(rx))
                };
                return Some(if is_and { Op::Abcd(s, d) } else { Op::Sbcd(s, d) });
            }
            if is_and && op & 0x130 == 0x100 {
                let m = (op >> 3) & 0x1f;
                return match m {
                    0x08 | 0x09 | 0x11 => Some(Op::Exg(m as u8, rx, ry)),
                    _ => None,
                };
            }
            let size = size2(opmode)?;
            let ea = r.ea(mode, reg, size)?;
            let (src, dst) = if opmode & 4 == 0 {
                if !is_data(ea) {
                    return None;
                }
                (ea, Ea::Dn(rx))
            } else {
                if !is_mem_alt(ea) {
                    return None;
                }
                (Ea::Dn(rx), ea)
            };
            if is_and {
                Op::And(size, src, dst)
            } else {
                Op::Or(size, src, dst)
            }
        }
        0x9 | 0xd => {
            let add = op >> 12 == 0xd;
            let opmode = (op >> 6) & 7;
            if opmode == 3 || opmode == 7 {
                let size = if opmode == 7 { Size::Long } else { Size::Word };
                let src = r.ea(mode, reg, size)?;
                return Some(if add { Op::Adda(size, src, rx) } else { Op::Suba(size, src, rx) });
            }
            let size = size2(opmode)?;
            if opmode & 4 != 0 && mode <= 1 {
                let (s, d) = if mode == 0 {
                    (Ea::Dn(ry), Ea::Dn(rx))
                } else {
                    (Ea::PreDec(ry), Ea::PreDec(rx))
                };
                return Some(if add { Op::Addx(size, s, d) } else { Op::Subx(size, s, d) });
            }
            let ea = r.ea(mode, reg, size)?;
            let (src, dst) = if opmode & 4 == 0 {
                if size == Size::Byte && matches!(ea, Ea::An(_)) {
                    return None;
                }
                (ea, Ea::Dn(rx))
            } else {
                if !is_mem_alt(ea) {
                    return None;
                }
                (Ea::Dn(rx), ea)
            };
            if add {
                Op::Add(size, src, dst)
            } else {
                Op::Sub(size, src, dst)
            }
        }
        0xa => Op::LineA,
        0xb => {
            let opmode = (op >> 6) & 7;
            if opmode == 3 || opmode == 7 {
                let size = if opmode == 7 { Size::Long } else { Size::Word };
                let src = r.ea(mode, reg, size)?;
                return Some(Op::Cmpa(size, src, rx));
            }
            let size = size2(opmode)?;
            if opmode & 4 == 0 {
                let src = r.ea(mode, reg, size)?;
                if size == Size::Byte && matches!(src, Ea::An(_)) {
                    return None;
                }
                return Some(Op::Cmp(size, src, Ea::Dn(rx)));
            }
            if mode == 1 {
                return Some(Op::Cmp(size, Ea::PostInc(ry), Ea::PostInc(rx)));
            }
            let dst = r.ea(mode, reg, size)?;
            if !is_data_alt(dst) {
                return None;
            }
            Op::Eor(size, Ea::Dn(rx), dst)
        }
        0xe => {
            let kind_of = |k: u16| match k & 3 {
                0 => ShiftKind::As,
                1 => ShiftKind::Ls,
                2 => ShiftKind::Rox,
                _ => ShiftKind::Ro,
            };
            let left = op & 0x100 != 0;
            if (op >> 6) & 3 == 3 {
                if op & 0x800 != 0 {
                    return None;
                }
                let ea = r.ea(mode, reg, Size::Word)?;
                if !is_mem_alt(ea) {
                    return None;
                }
                return Some(Op::ShiftMem { kind: kind_of(op >> 9), left, ea });
            }
            let size = size2(op >> 6)?;
            let count = if op & 0x20 != 0 {
                ShiftCount::Reg(rx)
            } else {
                ShiftCount::Imm(if rx == 0 { 8 } else { rx })
            };
            Op::Shift { kind: kind_of(op >> 3), left, size, count, dn: ry }
        }
        _ => Op::LineF,
    })
}

fn bitop(op: u16) -> BitOp {
    match (op >> 6) & 3 {
        0 => BitOp::Btst,
        1 => BitOp::Bchg,
        2 => BitOp::Bclr,
        _ => BitOp::Bset,
    }
}

fn decode_4<F: Fetch>(r: &mut Reader<F>, op: u16, mode: u16, reg: u16, rx: u8) -> Option<Op> {
    let ry = reg as u8;
    if op & 0x1c0 == 0x1c0 {
        let ea = r.ea(mode, reg, Size::Long)?;
        return if is_control(ea) { Some(Op::Lea(ea, rx)) } else { None };
    }
    if op & 0x1c0 == 0x180 {
        let ea = r.ea(mode, reg, Size::Word)?;
        return if is_data(ea) { Some(Op::Chk(ea, rx)) } else { None };
    }
    if op & 0x100 != 0 {
        return None;
    }
    match op {
        0x4afc => return Some(Op::Illegal),
        0x4e70 => return Some(Op::Reset),
        0x4e71 => return Some(Op::Nop),
        0x4e72 => return Some(Op::Stop(r.w())),
        0x4e73 => return Some(Op::Rte),
        0x4e75 => return Some(Op::Rts),
        0x4e76 => return Some(Op::Trapv),
        0x4e77 => return Some(Op::Rtr),
        _ => {}
    }
    match op & 0xfff0 {
        0x4e40 => return Some(Op::Trap((op & 0xf) as u8)),
        0x4e50 => {
            if op & 8 == 0 {
                return Some(Op::Link(ry, r.w() as i16));
            }
            return Some(Op::Unlk(ry));
        }
        0x4e60 => return Some(Op::MoveUsp { to_usp: op & 8 == 0, an: ry }),
        _ => {}
    }
    let size_bits = (op >> 6) & 3;
    match (op >> 8) & 0xf {
        0x0 | 0x2 | 0x4 | 0x6 if size_bits != 3 => {
            let size = size2(size_bits)?;
            let ea = r.ea(mode, reg, size)?;
            if !is_data_alt(ea) {
                return None;
            }
            Some(match (op >> 8) & 0xf {
                0x0 => Op::Negx(size, ea),
                0x2 => Op::Clr(size, ea),
                0x4 => Op::Neg(size, ea),
                _ => Op::Not(size, ea),
            })
        }
        0x0 => {
            let ea = r.ea(mode, reg, Size::Word)?;
            if is_data_alt(ea) { Some(Op::MoveFromSr(ea)) } else { None }
        }
        0x4 => {
            let ea = r.ea(mode, reg, Size::Word)?;
            if is_data(ea) { Some(Op::MoveToCcr(ea)) } else { None }
        }
        0x6 => {
            let ea = r.ea(mode, reg, Size::Word)?;
            if is_data(ea) { Some(Op::MoveToSr(ea)) } else { None }
        }
        0x8 => match size_bits {
            0 => {
                let ea = r.ea(mode, reg, Size::Byte)?;
                if is_data_alt(ea) { Some(Op::Nbcd(ea)) } else { None }
            }
            1 => {
                if mode == 0 {
                    return Some(Op::Swap(ry));
                }
                let ea = r.ea(mode, reg, Size::Long)?;
                if is_control(ea) { Some(Op::Pea(ea)) } else { None }
            }
            _ => {
                let size = if size_bits == 3 { Size::Long } else { Size::Word };
                if mode == 0 {
                    return Some(Op::Ext(size, ry));
                }
                let mask = r.w();
                let ea = r.ea(mode, reg, size)?;
                let ok = matches!(ea, Ea::PreDec(_)) || (is_control(ea) && is_alterable(ea));
                if ok { Some(Op::Movem { size, to_mem: true, mask, ea }) } else { None }
            }
        },
        0xa => {
            if size_bits == 3 {
                let ea = r.ea(mode, reg, Size::Byte)?;
                return if is_data_alt(ea) { Some(Op::Tas(ea)) } else { None };
            }
            let size = size2(size_bits)?;
            let ea = r.ea(mode, reg, size)?;
            // 68000 : TST n'accepte que les modes données altérables (An, PC et # viennent du 68020)
            if !is_data_alt(ea) {
                return None;
            }
            Some(Op::Tst(size, ea))
        }
        0xc => {
            if size_bits < 2 {
                return None;
            }
            let size = if size_bits == 3 { Size::Long } else { Size::Word };
            let mask = r.w();
            let ea = r.ea(mode, reg, size)?;
            let ok = matches!(ea, Ea::PostInc(_)) || is_control(ea);
            if ok { Some(Op::Movem { size, to_mem: false, mask, ea }) } else { None }
        }
        0xe => {
            let ea = r.ea(mode, reg, Size::Long)?;
            if !is_control(ea) {
                return None;
            }
            match size_bits {
                2 => Some(Op::Jsr(ea)),
                3 => Some(Op::Jmp(ea)),
                _ => None,
            }
        }
        _ => None,
    }
}
