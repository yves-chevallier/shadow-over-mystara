//! Analyse statique des « lecteurs de restes » : pour chaque gestionnaire des tables d'objets, cherche
//! les registres D1-D6 et A2-A4 lus avant d'avoir été écrits, sur au moins un chemin du code d'origine.
//! La boucle de dispatch ne fixe que A0, A5, D7, D0 (type × 4) et A1 (adresse du gestionnaire) : les autres
//! registres sont ceux que le gestionnaire précédent a laissés.
//!
//!   restes_statique <roms>
//!
//! L'analyse suit les branchements, les tables de sauts et les `bsr`/`jsr` locaux (le corps de la
//! sous-routine est analysé avec l'état de l'appelant) ; un `jsr` vers une routine commune hors de
//! la plage n'est pas suivi (ses lectures éventuelles échappent : le contrôle dynamique
//! `DDSOM_RESTES=1 fuzz familles` les couvre). Un appel est supposé définir D0, D1 et A1 (résultats).

use m68k::decode::{Ea, Fetch, Op, ShiftCount};
use std::collections::BTreeMap;
use std::path::Path;

const fn d(n: u8) -> u16 {
    1 << n
}
const fn a(n: u8) -> u16 {
    1 << (8 + n)
}

fn nom(bit: u32) -> String {
    if bit < 8 { format!("D{bit}") } else { format!("A{}", bit - 8) }
}

/// Registres utilisés par un opérande (adresse, indice, ou valeur si registre).
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

/// Registre écrit si l'opérande est un registre de données ou d'adresses.
fn ecrit_ea(ea: Ea) -> u16 {
    match ea {
        Ea::Dn(n) => d(n),
        Ea::An(n) => a(n),
        _ => 0,
    }
}

/// (registres lus, registres écrits) d'une instruction.
fn effets(op: &Op) -> (u16, u16) {
    use Op::*;
    // destination en lecture-modification-écriture
    let rmw = |src: Ea, dst: Ea| (regs_ea(src) | regs_ea(dst), ecrit_ea(dst));
    // destination en écriture seule : seuls les registres d'adresse de l'opérande sont lus
    let wo = |src: u16, dst: Ea| (src | if matches!(dst, Ea::Dn(_) | Ea::An(_)) { 0 } else { regs_ea(dst) }, ecrit_ea(dst));
    match *op {
        Add(_, s, t) | Sub(_, s, t) | And(_, s, t) | Or(_, s, t) | Eor(_, s, t) | Addx(_, s, t) | Subx(_, s, t) | Abcd(s, t)
        | Sbcd(s, t) => rmw(s, t),
        Cmp(_, s, t) => (regs_ea(s) | regs_ea(t), 0),
        Adda(_, s, n) | Suba(_, s, n) => (regs_ea(s) | a(n), a(n)),
        Cmpa(_, s, n) => (regs_ea(s) | a(n), 0),
        Neg(_, e) | Negx(_, e) | Not(_, e) | Nbcd(e) | Tas(e) => (regs_ea(e), ecrit_ea(e)),
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
            // masque : bit 0 = D0 ... bit 15 = A7 (sauf prédécrément, ordre inversé)
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
    // registres suivis : D1-D6, A2-A4 (D0, A0, A1, A5, D7 sont posés par le dispatch)
    let suivis: u16 = (1..=6).fold(0, |s, n| s | d(n)) | a(2) | a(3) | a(4);
    let initial: u16 = d(0) | d(7) | a(0) | a(1) | a(5) | a(7);
    let (lo, hi) = (0x98000u32, 0xe1000u32);
    // toutes les entrées, triées : la plage d'un gestionnaire va de son entrée à la suivante
    let mut toutes: Vec<u32> = Vec::new();
    for (_, table, n) in tables {
        for k in 0..n {
            toutes.push((m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32);
        }
    }
    toutes.sort();
    toutes.dedup();
    for (classe, table, n) in tables {
        for k in 0..n {
            let entree = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if !(lo..hi).contains(&entree) || !ddsom_game::all_natives().any(|n| n.entry == entree) {
                continue;
            }
            let fin_plage = toutes.iter().copied().find(|&e| e > entree).unwrap_or(hi);
            let dedans = |t: u32| (entree..fin_plage).contains(&t) && t % 2 == 0;
            // état « définitivement écrit » minimal par adresse, point fixe
            let mut etat: BTreeMap<u32, u16> = BTreeMap::new();
            let mut a_faire = vec![(entree, initial)];
            let mut lectures: BTreeMap<(u32, u32), String> = BTreeMap::new();
            let mut budget = 20_000;
            while let Some((debut, mut def)) = a_faire.pop() {
                let mut pc = debut;
                loop {
                    budget -= 1;
                    if budget == 0 || !dedans(pc) {
                        break;
                    }
                    match etat.get(&pc) {
                        Some(&e) if e & !def == 0 => break, // déjà vu avec un état au moins aussi pauvre
                        Some(&e) => {
                            def &= e;
                            etat.insert(pc, def);
                        }
                        None => {
                            etat.insert(pc, def);
                        }
                    }
                    let ins = m68k::decode(&mut m.bus, pc);
                    let (lus, ecrits) = effets(&ins.op);
                    let non_ecrits = lus & suivis & !def;
                    for bit in 0..16 {
                        if non_ecrits >> bit & 1 != 0 {
                            lectures.entry((bit, pc)).or_insert_with(|| format!("{:?}", ins.op));
                        }
                    }
                    def |= ecrits;
                    let suite = pc + ins.len.max(2);
                    let mut tombe = true;
                    match ins.op {
                        Op::Bra(t) => {
                            a_faire.push((t, def));
                            tombe = false;
                        }
                        Op::Bcc(_, t) | Op::Dbcc(_, _, t) => a_faire.push((t, def)),
                        Op::Bsr(t) => {
                            a_faire.push((t, def));
                            def |= d(0) | d(1) | a(1);
                        }
                        Op::Rts | Op::Rte | Op::Rtr | Op::Illegal => tombe = false,
                        Op::Jsr(ea) | Op::Jmp(ea) => {
                            let saut = matches!(ins.op, Op::Jmp(_));
                            match ea {
                                Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t) => {
                                    if dedans(t) {
                                        a_faire.push((t, def));
                                    }
                                }
                                Ea::PcIdx(base, _) => {
                                    let mut fin = fin_plage;
                                    let mut at = base;
                                    while at < fin && at < base + 0x200 {
                                        let t = (base as i32 + m.bus.fetch16(at) as i16 as i32) as u32;
                                        if dedans(t) && t > at {
                                            fin = fin.min(t);
                                            a_faire.push((t, def));
                                        }
                                        at += 2;
                                    }
                                }
                                _ => {}
                            }
                            if saut {
                                tombe = false;
                            } else {
                                def |= d(0) | d(1) | a(1);
                            }
                        }
                        _ => {}
                    }
                    if !tombe {
                        break;
                    }
                    pc = suite;
                }
            }
            if !lectures.is_empty() {
                println!("{classe} {k} ({entree:06x}) :");
                for ((bit, pc), op) in &lectures {
                    println!("    {:3} lu en {pc:06x} : {op}", nom(*bit));
                }
            }
        }
    }
}
