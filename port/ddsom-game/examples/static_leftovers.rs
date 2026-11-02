//! Static analysis of the « leftover readers »: for each handler of the object tables, looks for
//! the registers D1-D6 and A2-A4 read before being written, on at least one path of the original code.
//! The dispatch loop only sets A0, A5, D7, D0 (type × 4) and A1 (handler address): the other
//! registers are those the previous handler left behind.
//!
//!   static_leftovers <roms>
//!
//! The analysis follows branches, jump tables and local `bsr`/`jsr` (the body of the
//! subroutine is analysed with the caller's state); a `jsr` to a common routine outside
//! the range is not followed (its possible reads escape: the dynamic check
//! `DDSOM_RESTES=1 fuzz families` covers them). A call is assumed to define D0, D1 and A1 (results).

use m68k::decode::{Ea, Fetch, Op, ShiftCount};
use std::collections::BTreeMap;
use std::path::Path;

const fn d(n: u8) -> u16 {
    1 << n
}
const fn a(n: u8) -> u16 {
    1 << (8 + n)
}

fn name(bit: u32) -> String {
    if bit < 8 { format!("D{bit}") } else { format!("A{}", bit - 8) }
}

/// Registers used by an operand (address, index, or value if register).
fn regs_ea(ea: Ea) -> u16 {
    let idx = |ext: u16| if ext & 0x8000 != 0 { a((ext >> 12 & 7) as u8) } else { d((ext >> 12 & 7) as u8) };
    match ea {
        Ea::Dn(n) => d(n),
        Ea::An(n) | Ea::Ind(n) | Ea::PostInc(n) | Ea::PreDec(n) | Ea::Disp(n, _) => a(n),
        Ea::Idx(n, ext) => a(n) | idx(ext),
        Ea::PcIdx(_, ext) => idx(ext),
        _ => 0,
    }
}

/// Register written if the operand is a data or address register.
fn written_ea(ea: Ea) -> u16 {
    match ea {
        Ea::Dn(n) => d(n),
        Ea::An(n) => a(n),
        _ => 0,
    }
}

/// (registers read, registers written) of an instruction.
fn effects(op: &Op) -> (u16, u16) {
    use Op::*;
    // read-modify-write destination
    let rmw = |src: Ea, dst: Ea| (regs_ea(src) | regs_ea(dst), written_ea(dst));
    // write-only destination: only the operand's address registers are read
    let wo = |src: u16, dst: Ea| (src | if matches!(dst, Ea::Dn(_) | Ea::An(_)) { 0 } else { regs_ea(dst) }, written_ea(dst));
    match *op {
        Add(_, s, t) | Sub(_, s, t) | And(_, s, t) | Or(_, s, t) | Eor(_, s, t) | Addx(_, s, t) | Subx(_, s, t) | Abcd(s, t)
        | Sbcd(s, t) => rmw(s, t),
        Cmp(_, s, t) => (regs_ea(s) | regs_ea(t), 0),
        Adda(_, s, n) | Suba(_, s, n) => (regs_ea(s) | a(n), a(n)),
        Cmpa(_, s, n) => (regs_ea(s) | a(n), 0),
        Neg(_, e) | Negx(_, e) | Not(_, e) | Nbcd(e) | Heap(e) => (regs_ea(e), written_ea(e)),
        Clr(_, e) => wo(0, e),
        Tst(_, e) | Pea(e) | Jmp(e) | Jsr(e) | MoveToSr(e) | MoveToCcr(e) => (regs_ea(e), 0),
        Ext(_, n) | Swap(n) => (d(n), d(n)),
        Exg(m, x, y) => {
            let (rx, ry) = match m {
                0x08 => (d(x), d(y)),
                0x09 => (a(x), a(y)),
                _ => (d(x), a(y)),
            };
            (rx | ry, rx | ry)
        }
        Mulu(e, n) | Muls(e, n) | Divu(e, n) | Divs(e, n) => (regs_ea(e) | d(n), d(n)),
        Chk(e, n) => (regs_ea(e) | d(n), 0),
        Move(_, s, t) => wo(regs_ea(s), t),
        Movea(_, s, n) => (regs_ea(s), a(n)),
        Moveq(_, n) => (0, d(n)),
        Lea(e, n) => (regs_ea(e), a(n)),
        Movem { to_mem, mask, ea, size: _ } => {
            // mask: bit 0 = D0 ... bit 15 = A7 (except predecrement, reversed order)
            let m = if matches!(ea, Ea::PreDec(_)) { mask.reverse_bits() } else { mask };
            if to_mem { (regs_ea(ea) | m, 0) } else { (regs_ea(ea), m) }
        }
        MoveFromSr(e) => wo(0, e),
        Scc(_, e) => wo(0, e),
        Bit(_, c, e) => (regs_ea(e) | if let ShiftCount::Reg(n) = c { d(n) } else { 0 }, 0),
        Shift { count, dn, .. } => (d(dn) | if let ShiftCount::Reg(n) = count { d(n) } else { 0 }, d(dn)),
        ShiftMem { ea, .. } => (regs_ea(ea), 0),
        Dbcc(_, n, _) => (d(n), d(n)),
        _ => (0, 0),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut m = cps2::Machine::load_ddsom(Path::new(&args[1]), None).expect("roms");
    let tables: [(&str, u32, u32); 6] = [
        ("ESHL", 0x39b96, 64),
        ("SET", 0x39eac, 34),
        ("SOBJ", 0x39fa8, 16),
        ("ITEM", 0x39f5a, 10),
        ("BOX", 0x39e76, 4),
        ("EFFCT", 0x39ce0, 64),
    ];
    // tracked registers: D1-D6, A2-A4 (D0, A0, A1, A5, D7 are set by the dispatch)
    let followed: u16 = (1..=6).fold(0, |s, n| s | d(n)) | a(2) | a(3) | a(4);
    let initial: u16 = d(0) | d(7) | a(0) | a(1) | a(5) | a(7);
    let (lo, hi) = (0x98000u32, 0xe1000u32);
    // all entries, sorted: a handler's range goes from its entry to the next one
    let mut all: Vec<u32> = Vec::new();
    for (_, table, n) in tables {
        for k in 0..n {
            all.push((m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32);
        }
    }
    all.sort();
    all.dedup();
    for (class, table, n) in tables {
        for k in 0..n {
            let entry = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if !(lo..hi).contains(&entry) || !ddsom_game::all_natives().any(|n| n.entry == entry) {
                continue;
            }
            let range_end = all.iter().copied().find(|&e| e > entry).unwrap_or(hi);
            let inside = |t: u32| (entry..range_end).contains(&t) && t % 2 == 0;
            // minimal « definitely written » state per address, fixed point
            let mut state: BTreeMap<u32, u16> = BTreeMap::new();
            let mut todo = vec![(entry, initial)];
            let mut reads: BTreeMap<(u32, u32), String> = BTreeMap::new();
            let mut budget = 20_000;
            while let Some((start, mut def)) = todo.pop() {
                let mut pc = start;
                loop {
                    budget -= 1;
                    if budget == 0 || !inside(pc) {
                        break;
                    }
                    match state.get(&pc) {
                        Some(&e) if e & !def == 0 => break, // already seen with an at least as poor state
                        Some(&e) => {
                            def &= e;
                            state.insert(pc, def);
                        }
                        None => {
                            state.insert(pc, def);
                        }
                    }
                    let ins = m68k::decode(&mut m.bus, pc);
                    let (read, written) = effects(&ins.op);
                    let unwritten = read & followed & !def;
                    for bit in 0..16 {
                        if unwritten >> bit & 1 != 0 {
                            reads.entry((bit, pc)).or_insert_with(|| format!("{:?}", ins.op));
                        }
                    }
                    def |= written;
                    let next = pc + ins.len.max(2);
                    let mut falls = true;
                    match ins.op {
                        Op::Bra(t) => {
                            todo.push((t, def));
                            falls = false;
                        }
                        Op::Bcc(_, t) | Op::Dbcc(_, _, t) => todo.push((t, def)),
                        Op::Bsr(t) => {
                            todo.push((t, def));
                            def |= d(0) | d(1) | a(1);
                        }
                        Op::Rts | Op::Rte | Op::Rtr | Op::Illegal => falls = false,
                        Op::Jsr(ea) | Op::Jmp(ea) => {
                            let jump = matches!(ins.op, Op::Jmp(_));
                            match ea {
                                Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t) => {
                                    if inside(t) {
                                        todo.push((t, def));
                                    }
                                }
                                Ea::PcIdx(base, _) => {
                                    let mut end = range_end;
                                    let mut at = base;
                                    while at < end && at < base + 0x200 {
                                        let t = (base as i32 + m.bus.fetch16(at) as i16 as i32) as u32;
                                        if inside(t) && t > at {
                                            end = end.min(t);
                                            todo.push((t, def));
                                        }
                                        at += 2;
                                    }
                                }
                                _ => {}
                            }
                            if jump {
                                falls = false;
                            } else {
                                def |= d(0) | d(1) | a(1);
                            }
                        }
                        _ => {}
                    }
                    if !falls {
                        break;
                    }
                    pc = next;
                }
            }
            if !reads.is_empty() {
                println!("{class} {k} ({entry:06x}) :");
                for ((bit, pc), op) in &reads {
                    println!("    {:3} lu en {pc:06x} : {op}", name(*bit));
                }
            }
        }
    }
}
