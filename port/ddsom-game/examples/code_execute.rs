//! Octets d'instructions 68000 exécutées par la machine de référence (code d'origine recompilé, 68000 surcadencé,
//! comme la machine B de `validate`) : la mesure du code qui sépare, dans la région programme, le code des données
//! (`port/extract/code_68k.txt`, docs/sans-rom.md « Couverture complète des données »).
//!
//!   code_execute <roms> <eeprom> <sortie.txt> <scénario>:<images> ...
//!
//! Chaque bloc recompilé exécuté est noté par son adresse de tête, puis étendu en ligne droite jusqu'à
//! l'instruction qui termine un bloc de base (branchement, saut, appel, retour, trap, chk, écriture de SR) : un bloc
//! de base s'exécute en entier une fois commencé. Un appel termine le bloc : les octets rangés après un `jsr` (données
//! que la routine appelée lit puis saute) ne sont comptés que si l'exécution y revient. Les instructions faites par
//! l'interpréteur sont notées une à une. La sortie donne les plages d'octets exécutés `début fin` (hexadécimal, fin
//! exclue), triées et réunies.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use m68k::decode::Op;

thread_local! {
    static TETES: RefCell<BTreeSet<u32>> = const { RefCell::new(BTreeSet::new()) };
}

/// Exécuteur de la référence qui note la tête de chaque bloc exécuté.
fn executeur(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    let r = ddsom_rc::run_block(pc, c, b, dl);
    if r.is_some() {
        TETES.with(|t| t.borrow_mut().insert(pc));
    }
    r
}

/// L'instruction termine-t-elle un bloc de base ? (comme `recomp`, plus les instructions invalides)
fn fin_de_bloc(op: &Op) -> bool {
    use Op::*;
    matches!(
        op,
        Bra(_) | Bsr(_) | Bcc(..) | Dbcc(..) | Jmp(_) | Jsr(_) | Rts | Rte | Rtr | Trap(_) | Trapv | Chk(..) | Illegal
            | LineA | LineF | Stop(_) | Reset | MoveToSr(_) | AndiSr(_) | OriSr(_) | EoriSr(_)
    )
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage : code_execute <roms> <eeprom> <sortie.txt> <scénario>:<images> ...");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let mut octets = vec![false; 0x400000];
    for spec in &a[4..] {
        let (path, frames) = spec.rsplit_once(':').expect("scénario:images");
        let frames: usize = frames.parse().expect("images");
        let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
        m.set_overclock(16.0);
        ddsom_game::install_reference(&mut m);
        m.executor = Some(executeur);
        m.rendu = false;
        let interpretes = std::rc::Rc::new(RefCell::new(BTreeSet::new()));
        let notes = interpretes.clone();
        m.step_hook = Some(Box::new(move |_: &m68k::Cpu, _: &cps2::Cps2Bus, pc: u32| {
            notes.borrow_mut().insert(pc);
        }));
        let script = ddsom_game::scenario::Scenario::load(Path::new(path)).expect("scénario");
        for f in 0..frames {
            script.apply(f, &mut m);
            m.run_frame();
        }
        m.step_hook = None;
        let tetes = TETES.with(|t| std::mem::take(&mut *t.borrow_mut()));
        let mut marque = |pc: u32, len: u32| {
            for o in pc..pc + len {
                if let Some(x) = octets.get_mut(o as usize) {
                    *x = true;
                }
            }
        };
        for &pc in interpretes.borrow().iter() {
            let ins = m68k::decode(&mut m.bus, pc);
            marque(pc, ins.len);
        }
        for pc in tetes {
            let mut pc = pc;
            loop {
                let ins = m68k::decode(&mut m.bus, pc);
                marque(pc, ins.len);
                if fin_de_bloc(&ins.op) || pc >= 0x400000 {
                    break;
                }
                pc += ins.len;
            }
        }
        eprintln!("{path} : {frames} images");
    }
    let mut out = String::new();
    let mut debut = None;
    for (i, &x) in octets.iter().chain([&false]).enumerate() {
        match (x, debut) {
            (true, None) => debut = Some(i),
            (false, Some(d)) => {
                out += &format!("{d:06x} {i:06x}\n");
                debut = None;
            }
            _ => {}
        }
    }
    std::fs::write(&a[3], out).expect("sortie");
}
