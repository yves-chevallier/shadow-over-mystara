//! Désassemblage par descente récursive : complète `listing.txt` (qui perd du code après les tables
//! de données) en suivant les branchements, les `bsr`/`jsr` locaux et les tables de sauts
//! `jmp/jsr (table,PC,Dn)` à partir de points d'entrée.
//!   dis_flow <roms> <début> <fin> <entrée>...      (adresses en hexadécimal)
//! Seul le code de la plage [début, fin) est suivi ; la sortie est triée par adresse.

use m68k::decode::{Ea, Op};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage : dis_flow <roms> <début> <fin> <entrée>...");
        std::process::exit(2);
    }
    let hex = |s: &String| u32::from_str_radix(s, 16).unwrap();
    let (lo, hi) = (hex(&a[2]), hex(&a[3]));
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), None).expect("roms");
    let mut todo: Vec<u32> = a[4..].iter().map(hex).collect();
    let mut seen: BTreeMap<u32, (u16, String, u32)> = BTreeMap::new();
    let mut tables: BTreeSet<(u32, u32)> = BTreeSet::new();
    let inside = |t: u32| (lo..hi).contains(&t) && t % 2 == 0;
    while let Some(start) = todo.pop() {
        let mut pc = start;
        while inside(pc) && !seen.contains_key(&pc) {
            let ins = m68k::decode(&mut m.bus, pc);
            seen.insert(pc, (ins.opcode, format!("{:?}", ins.op), ins.len));
            let next = pc + ins.len.max(2);
            let mut falls = true;
            match ins.op {
                Op::Bra(t) => {
                    todo.push(t);
                    falls = false;
                }
                Op::Bcc(_, t) | Op::Bsr(t) => todo.push(t),
                Op::Dbcc(_, _, t) => todo.push(t),
                Op::Rts | Op::Rte | Op::Rtr | Op::Illegal => falls = false,
                Op::Jmp(ea) | Op::Jsr(ea) => {
                    let is_jmp = matches!(ins.op, Op::Jmp(_));
                    match ea {
                        Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t) => todo.push(t),
                        Ea::PcIdx(base, _) => {
                            // table de mots relatifs à `base` : elle finit où commence le premier code
                            let mut end = hi;
                            let mut at = base;
                            while at < end && at < base + 0x200 {
                                let w = {
                                    use m68k::decode::Fetch;
                                    m.bus.fetch16(at) as i16 as i32
                                };
                                let t = (base as i32 + w) as u32;
                                if inside(t) && t > at {
                                    end = end.min(t);
                                    todo.push(t);
                                }
                                at += 2;
                            }
                            tables.insert((base, at));
                        }
                        _ => {}
                    }
                    if is_jmp {
                        falls = false;
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
    let mut last_end = 0;
    for (pc, (opcode, op, len)) in &seen {
        if *pc != last_end {
            println!();
        }
        for (b, e) in &tables {
            if *b == *pc {
                println!("; table de sauts {b:06x}..{e:06x}");
            }
        }
        println!("{pc:06x}  {opcode:04x}  {op}");
        last_end = pc + len.max(&2);
    }
}
