//! Bytes of 68000 instructions executed by the reference machine (recompiled original code, overclocked 68000,
//! like machine B of `validate`): the measurement of the code that separates, in the program region, code from data
//! (`port/extract/code_68k.txt`, docs/sans-rom.md "full data coverage").
//!
//!   executed_code <roms> <eeprom> <output.txt> <scenario>:<frames> ...
//!
//! Each recompiled block executed is recorded by its head address, then extended in a straight line up to
//! the instruction that ends a basic block (branch, jump, call, return, trap, chk, SR write): a basic
//! block executes entirely once started. A call ends the block: the bytes stored after a `jsr` (data
//! that the called routine reads then skips) are counted only if execution comes back to them. The instructions done by
//! the interpreter are recorded one by one. The output gives the ranges of executed bytes `start end` (hexadecimal, end
//! excluded), sorted and merged.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use m68k::decode::Op;

thread_local! {
    static HEADS: RefCell<BTreeSet<u32>> = const { RefCell::new(BTreeSet::new()) };
}

/// Reference executor that records the head of each executed block.
fn executor(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    let r = ddsom_rc::run_block(pc, c, b, dl);
    if r.is_some() {
        HEADS.with(|t| t.borrow_mut().insert(pc));
    }
    r
}

/// Does the instruction end a basic block? (like `recomp`, plus the invalid instructions)
fn block_end(op: &Op) -> bool {
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
        eprintln!("usage: executed_code <roms> <eeprom> <output.txt> <scenario>:<frames> ...");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let mut bytes = vec![false; 0x400000];
    for spec in &a[4..] {
        let (path, frames) = spec.rsplit_once(':').expect("scenario:frames");
        let frames: usize = frames.parse().expect("frames");
        let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
        m.set_overclock(16.0);
        ddsom_game::install_reference(&mut m);
        m.executor = Some(executor);
        m.rendering = false;
        let interpreted = std::rc::Rc::new(RefCell::new(BTreeSet::new()));
        let notes = interpreted.clone();
        m.step_hook = Some(Box::new(move |_: &m68k::Cpu, _: &cps2::Cps2Bus, pc: u32| {
            notes.borrow_mut().insert(pc);
        }));
        let script = ddsom_game::core::scenario::Scenario::load(Path::new(path)).expect("scenario");
        for f in 0..frames {
            script.apply(f, &mut m);
            m.run_frame();
        }
        m.step_hook = None;
        let heads = HEADS.with(|t| std::mem::take(&mut *t.borrow_mut()));
        let mut mark = |pc: u32, len: u32| {
            for o in pc..pc + len {
                if let Some(x) = bytes.get_mut(o as usize) {
                    *x = true;
                }
            }
        };
        for &pc in interpreted.borrow().iter() {
            let ins = m68k::decode(&mut m.bus, pc);
            mark(pc, ins.len);
        }
        for pc in heads {
            let mut pc = pc;
            loop {
                let ins = m68k::decode(&mut m.bus, pc);
                mark(pc, ins.len);
                if block_end(&ins.op) || pc >= 0x400000 {
                    break;
                }
                pc += ins.len;
            }
        }
        eprintln!("{path}: {frames} frames");
    }
    let mut out = String::new();
    let mut start = None;
    for (i, &x) in bytes.iter().chain([&false]).enumerate() {
        match (x, start) {
            (true, None) => start = Some(i),
            (false, Some(d)) => {
                out += &format!("{d:06x} {i:06x}\n");
                start = None;
            }
            _ => {}
        }
    }
    std::fs::write(&a[3], out).expect("output");
}
