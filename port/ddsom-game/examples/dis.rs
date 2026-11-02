//! Disassembles a range of the program (opcode space) with the interpreter's decoder.
//! Completes `listing.txt` where it is incomplete (misalignments).
//!   dis <roms> <start hex> <end hex>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, from, to] = &a[..] else {
        eprintln!("usage: dis <roms> <start> <end>");
        std::process::exit(2);
    };
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), None).expect("roms");
    let (mut pc, end) = (u32::from_str_radix(from, 16).unwrap(), u32::from_str_radix(to, 16).unwrap());
    while pc < end {
        let ins = m68k::decode(&mut m.bus, pc);
        println!("{pc:06x}  {:04x}  {:?}", ins.opcode, ins.op);
        pc += ins.len.max(2);
    }
}
