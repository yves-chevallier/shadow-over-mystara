//! Durée des routines à boucles ou à chemins dépendant des données.
//!
//! `timing::path_cycles` ne sait pas suivre une boucle. Ici, la routine native décrit le chemin
//! pris par l'original sous forme d'une suite de décisions (une par branchement conditionnel ou
//! `dbf` rencontré, dans l'ordre) : `Trace`. `walk_cycles` parcourt le code d'origine en suivant ces
//! décisions et additionne les durées de l'interpréteur ; les appels `bsr`/`jsr` absolus sont suivis.

use std::cell::RefCell;
use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::decode::{Ea, Op, ShiftCount};

/// Décisions successives des branchements conditionnels (vrai = branchement pris, ou `dbf` qui boucle).
#[derive(Default)]
pub struct Trace(Vec<bool>);

impl Trace {
    pub fn new() -> Self {
        Trace(Vec::new())
    }

    /// Note la décision d'un branchement et la renvoie (pour écrire `if t.take(cond) { ... }`).
    pub fn take(&mut self, taken: bool) -> bool {
        self.0.push(taken);
        taken
    }

    fn key(&self) -> u64 {
        // FNV-1a sur les décisions
        self.0.iter().fold(0xcbf2_9ce4_8422_2325u64 ^ self.0.len() as u64, |h, &d| {
            (h ^ d as u64).wrapping_mul(0x100_0000_01b3)
        })
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(u32, u64), u64>> = RefCell::new(HashMap::new());
}

/// Durée de la routine d'origine depuis `entry` jusqu'à son RTS, en suivant `trace`.
pub fn walk_cycles(b: &mut Cps2Bus, entry: u32, trace: &Trace) -> u64 {
    let key = (entry, trace.key());
    if let Some(v) = CACHE.with(|c| c.borrow().get(&key).copied()) {
        return v;
    }
    let mut decisions = trace.0.iter().copied();
    let mut stack: Vec<u32> = Vec::new();
    let mut pc = entry;
    let mut cyc = 0u64;
    for _ in 0..100_000 {
        let ins = m68k::decode(b, pc);
        cyc += m68k::timing::base_cycles(&ins) as u64;
        let next = pc.wrapping_add(ins.len);
        match ins.op {
            Op::Rts => match stack.pop() {
                Some(ret) => {
                    pc = ret;
                    continue;
                }
                None => {
                    assert!(decisions.next().is_none(), "walk_cycles : trace trop longue pour {entry:06x}");
                    CACHE.with(|c| c.borrow_mut().insert(key, cyc));
                    return cyc;
                }
            },
            Op::Bra(t) => {
                pc = t;
                continue;
            }
            Op::Bsr(t) | Op::Jsr(Ea::AbsL(t)) => {
                stack.push(next);
                pc = t;
                continue;
            }
            Op::Jmp(Ea::AbsL(t)) => {
                pc = t;
                continue;
            }
            Op::Bcc(_, t) => {
                let taken = decisions.next().unwrap_or_else(|| panic!("walk_cycles : trace trop courte en {pc:06x}"));
                if taken {
                    cyc += 2;
                    pc = t;
                    continue;
                } else if ins.opcode & 0xff == 0 {
                    cyc += 4;
                }
            }
            Op::Dbcc(_, _, t) => {
                let taken = decisions.next().unwrap_or_else(|| panic!("walk_cycles : trace trop courte en {pc:06x}"));
                if taken {
                    cyc -= 2;
                    pc = t;
                    continue;
                }
                cyc += 2;
            }
            Op::Shift { count: ShiftCount::Imm(n), .. } => cyc += 2 * n as u64,
            _ => {}
        }
        pc = next;
    }
    panic!("walk_cycles : boucle infinie depuis {entry:06x}");
}

/// Somme des durées de base des instructions de `from` (inclus) à `to` (exclu), sans suivre les
/// branchements (estimation d'un tronçon de code linéaire).
pub fn range_cycles(b: &mut Cps2Bus, from: u32, to: u32) -> u64 {
    let mut pc = from;
    let mut cyc = 0u64;
    while pc < to {
        let ins = m68k::decode(b, pc);
        cyc += m68k::timing::base_cycles(&ins) as u64;
        pc = pc.wrapping_add(ins.len);
    }
    cyc
}
