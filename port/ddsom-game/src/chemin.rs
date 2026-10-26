//! Durée exacte d'un chemin d'exécution du code d'origine, pour les routines natives qui ne se
//! réduisent pas à une ligne droite (appels de sous-routines, boucles `dbf`).
//!
//! La routine native connaît, mieux que personne, les décisions que prend le code d'origine : elle
//! les enregistre dans un `Chemin` (dans l'ordre où l'original les rencontre : une décision par
//! branchement conditionnel `Bcc` ou par `DBcc`), puis `duree` parcourt le code d'origine en
//! additionnant les durées de l'interpréteur. Les résultats sont mis en cache.
//! `timing::path_cycles` fait de même pour les routines sans appel ni boucle.

use std::cell::RefCell;
use std::collections::HashMap;

use cps2::Cps2Bus;
use m68k::decode::{Ea, Op, ShiftCount, Size};

/// Décisions des branchements rencontrés : vrai = branchement pris (pour `dbf` : boucle poursuivie).
#[derive(Default, Clone, Copy, Debug)]
pub struct Chemin {
    bits: u128,
    n: u32,
}

thread_local! {
    static CACHE: RefCell<HashMap<(u32, u32, u128, u32), u64>> = RefCell::new(HashMap::new());
}

impl Chemin {
    /// Enregistre la décision du prochain branchement conditionnel.
    pub fn pris(&mut self, pris: bool) {
        assert!(self.n < 128, "Chemin : trop de décisions");
        self.bits |= (pris as u128) << self.n;
        self.n += 1;
    }

    /// Enregistre `n` décisions identiques.
    pub fn repete(&mut self, n: u32, pris: bool) {
        for _ in 0..n {
            self.pris(pris);
        }
    }

    /// Durée du code d'origine depuis `entree` jusqu'au RTS (ou au JMP final, compté) en suivant ces décisions.
    pub fn duree(&self, b: &mut Cps2Bus, entree: u32) -> u64 {
        self.duree_entre(b, entree, None)
    }

    /// Idem, mais s'arrête dès que le PC atteint `arret` (après au moins une instruction) : durée d'un
    /// tour de boucle ou d'un tronçon, sans compter la suite.
    pub fn duree_entre(&self, b: &mut Cps2Bus, depart: u32, arret: Option<u32>) -> u64 {
        self.duree_hors(b, depart, arret, &[])
    }

    /// Idem, mais les appels (JSR/BSR) vers les adresses `externes` ne comptent que l'instruction
    /// d'appel : la routine native les exécute elle-même par `call`, qui ajoute leur durée.
    pub fn duree_hors(&self, b: &mut Cps2Bus, depart: u32, arret: Option<u32>, externes: &[u32]) -> u64 {
        // sans 68000, pas de durée : le code d'origine n'est pas lu (jeu sans ROM)
        if crate::sans68k::actif() {
            return 0;
        }
        let cle = (depart, arret.unwrap_or(0), self.bits, self.n);
        if let Some(v) = CACHE.with(|c| c.borrow().get(&cle).copied()) {
            return v;
        }
        let mut suite = Suite { bits: self.bits, restantes: self.n };
        let v = marche(b, depart, arret, &mut suite, externes);
        assert_eq!(suite.restantes, 0, "{depart:06x} : décisions non consommées");
        CACHE.with(|c| c.borrow_mut().insert(cle, v));
        v
    }
}

struct Suite {
    bits: u128,
    restantes: u32,
}

impl Suite {
    fn suivante(&mut self, pc: u32) -> bool {
        assert!(self.restantes > 0, "chemin : branchement sans décision en {pc:06x}");
        self.restantes -= 1;
        let pris = self.bits & 1 != 0;
        self.bits >>= 1;
        pris
    }
}

fn marche(b: &mut Cps2Bus, mut pc: u32, arret: Option<u32>, suite: &mut Suite, externes: &[u32]) -> u64 {
    let mut cycles = 0u64;
    for tour in 0..100_000 {
        if tour > 0 && Some(pc) == arret {
            return cycles;
        }
        let ins = m68k::decode(b, pc);
        cycles += m68k::timing::base_cycles(&ins) as u64;
        let prochain = pc.wrapping_add(ins.len);
        match ins.op {
            Op::Rts => return cycles,
            Op::Bra(t) => {
                // saut vers une routine exécutée à part (appel de queue) : le parcours s'arrête
                if externes.contains(&t) {
                    return cycles;
                }
                pc = t;
                continue;
            }
            Op::Bcc(_, t) => {
                if suite.suivante(pc) {
                    cycles += 2;
                    pc = t;
                    continue;
                } else if ins.opcode & 0xff == 0 {
                    cycles += 4;
                }
            }
            Op::Dbcc(_, _, t) => {
                // condition fausse : le compteur décrémente ; boucle poursuivie = 10 cycles, fin = 14
                if suite.suivante(pc) {
                    cycles -= 2;
                    pc = t;
                    continue;
                }
                cycles += 2;
            }
            Op::Bsr(t) | Op::Jsr(Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t)) => {
                if !externes.contains(&t) {
                    cycles += marche(b, t, None, suite, externes);
                }
            }
            Op::Jsr(_) => panic!("chemin : JSR non statique en {pc:06x}"),
            Op::Jmp(_) => return cycles,
            Op::Movem { size, mask, .. } => {
                cycles += mask.count_ones() as u64 * if size == Size::Long { 8 } else { 4 }
            }
            Op::Shift { count: ShiftCount::Imm(n), .. } => cycles += 2 * n as u64,
            Op::Shift { .. } | Op::Mulu(..) | Op::Muls(..) | Op::Divu(..) | Op::Divs(..) => {
                panic!("chemin : durée dépendant des données en {pc:06x}")
            }
            _ => {}
        }
        pc = prochain;
    }
    panic!("chemin : pas de RTS depuis {pc:06x}");
}
