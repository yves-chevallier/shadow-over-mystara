//! Durée des instructions en cycles (tables du manuel utilisateur 68000, sans états d'attente).
//! Les parts dynamiques (branchement pris, nombre de décalages, registres MOVEM) sont ajoutées
//! à l'exécution.

use crate::decode::{Ea, Instr, Op, Size};

/// Temps de calcul d'adresse effective (octet/mot, long).
fn ea_time(ea: Ea, size: Size) -> u32 {
    let long = size == Size::Long;
    let (bw, l) = match ea {
        Ea::Dn(_) | Ea::An(_) => (0, 0),
        Ea::Ind(_) | Ea::PostInc(_) => (4, 8),
        Ea::PreDec(_) => (6, 10),
        Ea::Disp(..) | Ea::PcDisp(_) => (8, 12),
        Ea::AbsW(_) => (8, 12),
        Ea::AbsL(_) => (12, 16),
        Ea::Idx(..) | Ea::PcIdx(..) => (10, 14),
        Ea::Imm(_) => (4, 8),
    };
    if long { l } else { bw }
}

/// Destination d'un MOVE : -(An) coûte comme (An).
fn move_dst_time(ea: Ea, size: Size) -> u32 {
    match ea {
        Ea::PreDec(_) => if size == Size::Long { 8 } else { 4 },
        _ => ea_time(ea, size),
    }
}

/// Adresse de contrôle pour LEA/PEA/JMP/JSR : (An), d16, d8+Xn, abs.w, abs.l.
fn ctrl_index(ea: Ea) -> usize {
    match ea {
        Ea::Ind(_) => 0,
        Ea::Disp(..) | Ea::PcDisp(_) => 1,
        Ea::Idx(..) | Ea::PcIdx(..) => 2,
        Ea::AbsW(_) => 3,
        _ => 4,
    }
}

/// Durée de base ; les parts dépendant des données sont ajoutées par l'exécuteur.
pub fn base_cycles(ins: &Instr) -> u32 {
    use Op::*;
    let reg = |ea: Ea| matches!(ea, Ea::Dn(_) | Ea::An(_));
    let quick = ins.opcode >> 12 == 5;
    match ins.op {
        Move(sz, s, d) => 4 + ea_time(s, sz) + move_dst_time(d, sz),
        Movea(sz, s, _) => 4 + ea_time(s, sz),
        Moveq(..) => 4,
        Add(sz, _, d) | Sub(sz, _, d) if quick => {
            if reg(d) { if sz == Size::Long { 8 } else { 4 } } else { (if sz == Size::Long { 12 } else { 8 }) + ea_time(d, sz) }
        }
        Add(sz, s, d) | Sub(sz, s, d) | And(sz, s, d) | Or(sz, s, d) | Eor(sz, s, d) => {
            match (s, d) {
                (Ea::Imm(_), Ea::Dn(_)) => if sz == Size::Long { 16 } else { 8 },
                (Ea::Imm(_), _) => (if sz == Size::Long { 20 } else { 12 }) + ea_time(d, sz),
                (_, Ea::Dn(_)) => {
                    if sz == Size::Long {
                        (if reg(s) || matches!(s, Ea::Imm(_)) { 8 } else { 6 }) + ea_time(s, sz)
                    } else {
                        4 + ea_time(s, sz)
                    }
                }
                _ => (if sz == Size::Long { 12 } else { 8 }) + ea_time(d, sz),
            }
        }
        Adda(..) | Suba(..) if quick => 8,
        Adda(sz, s, _) | Suba(sz, s, _) => match s {
            _ => if sz == Size::Long { (if reg(s) || matches!(s, Ea::Imm(_)) { 8 } else { 6 }) + ea_time(s, sz) } else { 8 + ea_time(s, sz) },
        },
        Cmp(sz, s, d) => match d {
            Ea::Dn(_) => (if sz == Size::Long { 6 } else { 4 }) + ea_time(s, sz),
            Ea::PostInc(_) if matches!(s, Ea::PostInc(_)) => if sz == Size::Long { 20 } else { 12 },
            _ => (if sz == Size::Long { 12 } else { 8 }) + ea_time(d, sz),
        },
        Cmpa(sz, s, _) => 6 + ea_time(s, sz),
        Addx(sz, s, _) | Subx(sz, s, _) => match s {
            Ea::Dn(_) => if sz == Size::Long { 8 } else { 4 },
            _ => if sz == Size::Long { 30 } else { 18 },
        },
        Abcd(s, _) | Sbcd(s, _) => if matches!(s, Ea::Dn(_)) { 6 } else { 18 },
        Nbcd(d) => if reg(d) { 6 } else { 8 + ea_time(d, Size::Byte) },
        Neg(sz, d) | Negx(sz, d) | Not(sz, d) | Clr(sz, d) => {
            if reg(d) { if sz == Size::Long { 6 } else { 4 } } else { (if sz == Size::Long { 12 } else { 8 }) + ea_time(d, sz) }
        }
        Tst(sz, d) => 4 + ea_time(d, sz),
        Tas(d) => if reg(d) { 4 } else { 10 + ea_time(d, Size::Byte) },
        Ext(..) | Swap(_) | Nop => 4,
        Exg(..) => 6,
        // + 2 par bit (MULU) / transition (MULS), + durée de division : ajoutés à l'exécution
        Mulu(s, _) | Muls(s, _) => 38 + ea_time(s, Size::Word),
        Divu(s, _) | Divs(s, _) => ea_time(s, Size::Word),
        Chk(s, _) => 10 + ea_time(s, Size::Word),
        AndiCcr(_) | OriCcr(_) | EoriCcr(_) | AndiSr(_) | OriSr(_) | EoriSr(_) => 20,
        MoveFromSr(d) => if reg(d) { 6 } else { 8 + ea_time(d, Size::Word) },
        MoveToSr(s) | MoveToCcr(s) => 12 + ea_time(s, Size::Word),
        MoveUsp { .. } => 4,
        Movem { to_mem, ea, .. } => {
            let i = if matches!(ea, Ea::PostInc(_) | Ea::PreDec(_)) { 0 } else { ctrl_index(ea) };
            if to_mem { [8, 12, 14, 12, 16][i] } else { [12, 16, 18, 16, 20][i] }
        }
        Movep { size, .. } => if size == Size::Long { 24 } else { 16 },
        Lea(ea, _) => [4, 8, 12, 8, 12][ctrl_index(ea)],
        Pea(ea) => [12, 16, 20, 16, 20][ctrl_index(ea)],
        Jmp(ea) => [8, 10, 14, 10, 12][ctrl_index(ea)],
        Jsr(ea) => [16, 18, 22, 18, 20][ctrl_index(ea)],
        Link(..) => 16,
        Unlk(_) => 12,
        Bit(kind, count, d) => {
            use crate::decode::{BitOp, ShiftCount};
            let imm = matches!(count, ShiftCount::Imm(_));
            if matches!(d, Ea::Dn(_)) {
                // BCHG/BSET/BCLR : +2 si le numéro de bit >= 16 (ajouté à l'exécution)
                let base = match kind {
                    BitOp::Btst => 6,
                    BitOp::Bclr => 8,
                    _ => 6,
                };
                base + if imm { 4 } else { 0 }
            } else {
                let base = if kind == BitOp::Btst { 4 } else { 8 };
                // BTST Dn,#imm : l'immédiat compte comme une lecture d'extension
                base + if imm { 4 } else { 0 } + if matches!(d, Ea::Imm(_)) { 6 } else { ea_time(d, Size::Byte) }
            }
        }
        Shift { size, .. } => if size == Size::Long { 8 } else { 6 },
        ShiftMem { ea, .. } => 8 + ea_time(ea, Size::Word),
        Bra(_) => 10,
        Bsr(_) => 18,
        Bcc(..) => 8,
        Dbcc(..) => 12,
        Scc(_, d) => if reg(d) { 4 } else { 8 + ea_time(d, Size::Byte) },
        Rts => 16,
        Rte | Rtr => 20,
        Trap(_) => 34,
        Trapv => 4,
        Reset => 132,
        Stop(_) => 4,
        Illegal | LineA | LineF => 34,
    }
}
