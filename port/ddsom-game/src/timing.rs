//! Durée d'une routine d'origine, lue dans le code 68000 lui-même.
//!
//! On parcourt les instructions depuis l'entrée en suivant le chemin indiqué pour chaque
//! branchement conditionnel, jusqu'au RTS, en additionnant les durées de l'interpréteur.
//! Les résultats sont mis en cache par (entrée, chemin).

use std::cell::RefCell;
use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::decode::{Op, ShiftCount};

thread_local! {
    static CACHE: RefCell<HashMap<(u32, u64), u64>> = RefCell::new(HashMap::new());
}

/// Durée du chemin de `entry` au RTS. `taken(pc)` dit si le branchement conditionnel en `pc`
/// est pris ; `path` identifie le chemin pour le cache (un bit par décision, au choix de l'appelant).
pub fn path_cycles(b: &mut Cps2Bus, entry: u32, path: u64, taken: impl Fn(u32) -> bool) -> u64 {
    // sans 68000, pas de durée : le code d'origine n'est pas lu (jeu sans ROM)
    if crate::sans68k::actif() {
        return 0;
    }
    if let Some(v) = CACHE.with(|c| c.borrow().get(&(entry, path)).copied()) {
        return v;
    }
    let mut pc = entry;
    let mut cyc = 0u64;
    for _ in 0..10_000 {
        let ins = m68k::decode(b, pc);
        cyc += m68k::timing::base_cycles(&ins) as u64;
        let next = pc.wrapping_add(ins.len);
        match ins.op {
            Op::Rts => {
                CACHE.with(|c| c.borrow_mut().insert((entry, path), cyc));
                return cyc;
            }
            Op::Bra(t) => {
                pc = t;
                continue;
            }
            Op::Bcc(_, t) => {
                if taken(pc) {
                    cyc += 2;
                    pc = t;
                    continue;
                } else if ins.opcode & 0xff == 0 {
                    cyc += 4;
                }
            }
            Op::Shift { count: ShiftCount::Imm(n), .. } => cyc += 2 * n as u64,
            Op::Shift { .. } | Op::Dbcc(..) | Op::Movem { .. } | Op::Mulu(..) | Op::Muls(..)
            | Op::Divu(..) | Op::Divs(..) | Op::Bsr(_) | Op::Jsr(_) | Op::Jmp(_) | Op::Bit(..)
            | Op::Scc(..) => panic!("path_cycles : durée dépendant des données non gérée en {pc:06x} ({:?})", ins.op),
            _ => {}
        }
        pc = next;
    }
    panic!("path_cycles : pas de RTS depuis {entry:06x}");
}
