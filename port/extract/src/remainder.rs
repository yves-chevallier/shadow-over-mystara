//! The rest of the program region: `data/remainder.json` (`ddsom_assets::remainder`, docs/sans-rom.md « Couverture
//! complète des données »).
//!
//! Every byte of the program region that the other files do not describe is stored there, except 68000 code:
//! the bytes of the instructions that the reference machine executes over all the scenarios (`code_68k.txt`, measured
//! by the `code_execute` example of ddsom-game and gathered by `tools/code_68k.sh`). When in doubt a byte goes to the
//! data side: never-executed code, tables stored in the code and bytes that a call skips are therefore
//! in this file. The zones read by the natives in the code itself (`natives.json`, measured) remain covered.
//! Must be extracted after all the other files (it reads them to know what they describe).

use std::io;
use std::path::Path;

use ddsom_assets::remainder::{ranges, Block, Remainder};
use m68k::decode::{Ea, Fetch, Op};

const CODE: &str = include_str!("../code_68k.txt");

/// Ranges of executed bytes `(start, exclusive end)`.
pub fn code() -> Vec<(u32, u32)> {
    CODE.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let t: Vec<u32> = l.split_whitespace().map(|v| u32::from_str_radix(v, 16).expect("code_68k.txt")).collect();
            (t[0], t[1])
        })
        .collect()
}

/// Program space as seen by the decoder.
struct View<'a>(&'a [u16]);

impl Fetch for View<'_> {
    fn fetch16(&mut self, a: u32) -> u16 {
        self.0.get((a as usize & 0xff_ffff) >> 1).copied().unwrap_or(0)
    }
}

/// Memory operands of an instruction: (read or written as data, only computed by `lea`/`pea`).
/// Jumps and calls do not count: their target is code.
fn operands(op: &Op) -> (Vec<Ea>, Vec<Ea>) {
    use Op::*;
    let read = match *op {
        Add(_, a, b) | Addx(_, a, b) | Sub(_, a, b) | Subx(_, a, b) | Cmp(_, a, b) | And(_, a, b) | Or(_, a, b)
        | Eor(_, a, b) | Abcd(a, b) | Sbcd(a, b) | Move(_, a, b) => vec![a, b],
        Adda(_, a, _) | Suba(_, a, _) | Cmpa(_, a, _) | Neg(_, a) | Negx(_, a) | Not(_, a) | Clr(_, a) | Tst(_, a)
        | Heap(a) | Mulu(a, _) | Muls(a, _) | Divu(a, _) | Divs(a, _) | Nbcd(a) | Chk(a, _) | Movea(_, a, _)
        | MoveFromSr(a) | MoveToSr(a) | MoveToCcr(a) | Bit(_, _, a) | ShiftMem { ea: a, .. } | Scc(_, a)
        | Movem { ea: a, .. } => vec![a],
        _ => vec![],
    };
    let addresses = match *op {
        Lea(a, _) | Pea(a) => vec![a],
        _ => vec![],
    };
    (read, addresses)
}

/// Fixed ROM address of an operand (PC-relative or absolute).
fn address(ea: Ea) -> Option<u32> {
    match ea {
        Ea::PcDisp(a) | Ea::PcIdx(a, _) | Ea::AbsL(a) => Some(a & 0xff_ffff),
        Ea::AbsW(a) => Some(a & 0xff_ffff),
        _ => None,
    }
}

/// Data stored in executed code: an executed instruction reads (or computes by `lea`/`pea` outside an instruction
/// start) a fixed address that falls in the code. These bytes are data (for example a table indexed
/// that starts in the displacement of a `bra`): the code range is given back to the data of the target at its end.
/// Returns the ranges given back.
fn data_in_code(prog: &[u16], code: &[(u32, u32)], is_code: &[bool]) -> Vec<(u32, u32)> {
    let mut view = View(prog);
    let mut starts = vec![false; is_code.len()];
    let mut instructions = Vec::new();
    for &(d, f) in code {
        let mut pc = d;
        while pc < f {
            let ins = m68k::decode(&mut view, pc);
            starts[pc as usize] = true;
            instructions.push(ins);
            pc += ins.len;
        }
    }
    let mut returned = Vec::new();
    for ins in &instructions {
        let (read, addresses) = operands(&ins.op);
        let targets = read.into_iter().filter_map(address).chain(
            addresses.into_iter().filter_map(address).filter(|&t| (t as usize) < starts.len() && !starts[t as usize]),
        );
        for t in targets {
            if (t as usize) < is_code.len() && is_code[t as usize] {
                let &(_, end) = code.iter().find(|&&(d, f)| (d..f).contains(&t)).unwrap();
                returned.push((t, end));
            }
        }
    }
    returned
}

pub fn extract(roms: &Path, assets: &Path) -> io::Result<()> {
    let (data, prog) = cps2::load_program(roms)?;
    let byte = |v: &[u16], a: u32| v[(a / 2) as usize].to_be_bytes()[(a % 2) as usize];
    let size = 2 * data.len() as u32;
    let key = std::fs::read(roms.join("ddsom.key"))?;
    let (_, start, end) = cps2::crypt::parse_key(&key);
    assert_eq!(start, 0, "unexpected encrypted zone");
    // the decryption (as in MAME) includes the word of the upper bound
    let encrypted_end = 2 * end + 2;
    // outside the encrypted zone, the program space equals the data space (the rebuild copies it)
    if let Some(w) = (end as usize + 1..data.len()).find(|&w| data[w] != prog[w]) {
        panic!("program space differs from data space outside the encrypted zone, at {:#x}", 2 * w);
    }
    let mut is_code = vec![false; size as usize];
    for &(a, b) in &code() {
        is_code[a as usize..b as usize].fill(true);
    }
    let returned = data_in_code(&prog, &code(), &is_code);
    for &(a, b) in &returned {
        is_code[a as usize..b as usize].fill(false);
    }
    let others = ddsom_assets::rebuild::Program::load_files(assets, &|f| f != "data/reste.json")?;
    let blocks = |view: &[u16], covered: &dyn Fn(u32) -> bool, n: u32| -> Vec<Block> {
        ranges(n, |a| !is_code[a as usize] && !covered(a))
            .into_iter()
            .map(|(a, b)| Block::new(a, &(a..b).map(|x| byte(view, x)).collect::<Vec<u8>>()))
            .collect()
    };
    let data_blocks = blocks(&data, &|a| others.data.is_covered(a), size);
    let program = blocks(&prog, &|a| others.prog.is_covered(a), encrypted_end);
    let code = ranges(size, |a| is_code[a as usize]);
    let remainder = Remainder {
        description: "Program region outside 68000 code and outside the other files (docs/sans-rom.md)".into(),
        encrypted_end,
        code_bytes: code.iter().map(|(a, b)| b - a).sum(),
        code: code.iter().map(|(a, b)| format!("{a:#x}-{b:#x}")).collect(),
        data: data_blocks,
        program,
    };
    let path = assets.join("data/reste.json");
    remainder.save(&path)?;
    let n = |v: &[Block]| v.iter().map(|b| b.bytes().unwrap().len()).sum::<usize>();
    println!(
        "{}: {} blocks, {} bytes (data), {} blocks, {} bytes (program), code excluded {} bytes in {} ranges \
         ({} ranges given back to the data: operand targets in the code)",
        path.display(),
        remainder.data.len(),
        n(&remainder.data),
        remainder.program.len(),
        n(&remainder.program),
        remainder.code_bytes,
        remainder.code.len(),
        returned.len()
    );
    Ok(())
}
