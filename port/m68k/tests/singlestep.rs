//! Tests SingleStepTests/m68000 (générés avec le cœur 68000 microcodé de MAME, licence MIT).
//! Fichiers attendus dans $M68K_TESTS (défaut ~/tools/m68000-tests/*.json.bin) ; ignoré s'ils manquent.
//! Vérifie l'état final ET le nombre de cycles de chaque instruction.
//! Les cas déclenchant une erreur d'adresse (accès mot impair) sont ignorés : non modélisé.
//!
//! cargo test --release -p m68k --test singlestep -- --nocapture

use std::collections::BTreeMap;
use std::path::PathBuf;

use m68k::{decode::Fetch, Bus, Cpu, SR_S};

struct Mem {
    ram: Vec<u8>,
    touched: Vec<u32>,
}

impl Fetch for Mem {
    fn fetch16(&mut self, a: u32) -> u16 {
        self.read16(a)
    }
}

impl Bus for Mem {
    fn read8(&mut self, a: u32) -> u8 {
        self.ram[a as usize]
    }
    fn read16(&mut self, a: u32) -> u16 {
        u16::from_be_bytes([self.ram[a as usize], self.ram[(a as usize + 1) & 0xffffff]])
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.ram[a as usize] = v;
        self.touched.push(a);
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.write8(a, (v >> 8) as u8);
        self.write8((a + 1) & 0xffffff, v as u8);
    }
}

#[derive(Default, Clone)]
struct State {
    regs: [u32; 19], // d0-d7, a0-a6, usp, ssp, sr, pc
    prefetch: [u32; 2],
    ram: Vec<(u32, u8)>,
}

struct Case {
    name: String,
    initial: State,
    fin: State,
    address_error: bool,
    cycles: u32,
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl Rd<'_> {
    fn u8(&mut self) -> u8 {
        self.p += 1;
        self.b[self.p - 1]
    }
    fn u16(&mut self) -> u16 {
        self.p += 2;
        u16::from_le_bytes([self.b[self.p - 2], self.b[self.p - 1]])
    }
    fn u32(&mut self) -> u32 {
        self.p += 4;
        u32::from_le_bytes(self.b[self.p - 4..self.p].try_into().unwrap())
    }
    fn state(&mut self) -> State {
        self.u32();
        assert_eq!(self.u32(), 0x01234567);
        let mut s = State::default();
        for r in s.regs.iter_mut() {
            *r = self.u32();
        }
        s.prefetch = [self.u32(), self.u32()];
        let n = self.u32();
        for _ in 0..n {
            let addr = self.u32();
            let data = self.u16();
            s.ram.push((addr, (data >> 8) as u8));
            s.ram.push((addr | 1, data as u8));
        }
        s
    }
}

fn parse(b: &[u8]) -> Vec<Case> {
    let mut r = Rd { b, p: 0 };
    assert_eq!(r.u32(), 0x1A3F5D71);
    let n = r.u32();
    let mut out = Vec::new();
    for _ in 0..n {
        r.u32();
        assert_eq!(r.u32(), 0xABC12367);
        r.u32();
        assert_eq!(r.u32(), 0x89ABCDEF);
        let len = r.u32() as usize;
        let name = String::from_utf8_lossy(&b[r.p..r.p + len]).into_owned();
        r.p += len;
        let initial = r.state();
        let fin = r.state();
        r.u32();
        assert_eq!(r.u32(), 0x456789AB);
        let cycles = r.u32();
        let ntr = r.u32();
        let mut address_error = false;
        for _ in 0..ntr {
            let kind = r.u8();
            r.u32();
            if kind != 0 {
                r.p += 20;
                if kind == 4 || kind == 5 {
                    address_error = true;
                }
            }
        }
        out.push(Case { name, initial, fin, address_error, cycles });
    }
    out
}

const NAMES: [&str; 19] = [
    "d0", "d1", "d2", "d3", "d4", "d5", "d6", "d7", "a0", "a1", "a2", "a3", "a4", "a5", "a6", "usp", "ssp",
    "sr", "pc",
];

/// Exécute un cas ; renvoie la liste des écarts et les cycles comptés.
fn run(mem: &mut Mem, c: &Case) -> (Vec<String>, u64) {
    for &a in &mem.touched {
        mem.ram[a as usize] = 0;
    }
    mem.touched.clear();
    let i = &c.initial;
    let mut cpu = Cpu::new();
    cpu.d.copy_from_slice(&i.regs[0..8]);
    cpu.a[..7].copy_from_slice(&i.regs[8..15]);
    cpu.sr = i.regs[17] as u16;
    if cpu.sr & SR_S != 0 {
        cpu.a[7] = i.regs[16];
        cpu.other_sp = i.regs[15];
    } else {
        cpu.a[7] = i.regs[15];
        cpu.other_sp = i.regs[16];
    }
    let start = i.regs[18].wrapping_sub(4);
    for &(a, v) in &i.ram {
        mem.write8(a, v);
    }
    mem.write16(start & 0xffffff, i.prefetch[0] as u16);
    mem.write16((start + 2) & 0xffffff, i.prefetch[1] as u16);
    cpu.pc = start;
    let ins = cpu.step(mem);
    // après STOP le 68000 ne refait pas de prefetch : le PC du test n'a pas le +4 habituel
    let pc_bias: u32 = if matches!(ins.map(|i| i.op), Some(m68k::Op::Stop(_))) && cpu.stopped { 0 } else { 4 };

    let mut got = [0u32; 19];
    got[0..8].copy_from_slice(&cpu.d);
    got[8..15].copy_from_slice(&cpu.a[..7]);
    got[15] = cpu.usp();
    got[16] = cpu.ssp();
    got[17] = cpu.sr as u32;
    got[18] = cpu.pc.wrapping_add(pc_bias);
    let mut errs = Vec::new();
    for k in 0..19 {
        if got[k] != c.fin.regs[k] {
            errs.push(format!("{}={:08x} attendu {:08x}", NAMES[k], got[k], c.fin.regs[k]));
        }
    }
    for &(a, v) in &c.fin.ram {
        let g = mem.ram[a as usize];
        if g != v {
            errs.push(format!("ram[{a:06x}]={g:02x} attendu {v:02x}"));
        }
    }
    (errs, cpu.cycles)
}

#[test]
fn singlestep() {
    let dir = std::env::var_os("M68K_TESTS").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap()).join("tools/m68000-tests")
    });
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("{} absent, test ignoré", dir.display());
        return;
    };
    let filter = std::env::var("M68K_FILTER").ok();
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.to_string_lossy().ends_with(".json.bin"))
        .filter(|p| filter.as_ref().is_none_or(|f| p.to_string_lossy().contains(f.as_str())))
        .collect();
    files.sort();

    let mut mem = Mem { ram: vec![0; 0x1000000], touched: Vec::new() };
    let mut summary = BTreeMap::new();
    let mut total_fail = 0;
    for f in &files {
        let cases = parse(&std::fs::read(f).unwrap());
        let (mut ok, mut fail, mut skip) = (0, 0, 0);
        let mut first = None;
        let (mut cyc_ok, mut cyc_bad) = (0, 0);
        let mut cyc_first = None;
        for c in &cases {
            if c.address_error {
                skip += 1;
                continue;
            }
            let (errs, cycles) = run(&mut mem, c);
            if cycles == c.cycles as u64 {
                cyc_ok += 1;
            } else {
                cyc_bad += 1;
                if cyc_first.is_none() {
                    cyc_first = Some(format!("{} : {} cycles, attendu {}", c.name, cycles, c.cycles));
                }
                if std::env::var_os("M68K_CYCLES_ALL").is_some() && cyc_bad <= 12 {
                    println!("  {} : {} cycles, attendu {} (d={:08x?} sr={:04x})", c.name, cycles, c.cycles, &c.initial.regs[0..8], c.initial.regs[17]);
                }
            }
            if errs.is_empty() {
                ok += 1;
            } else {
                fail += 1;
                if first.is_none() {
                    first = Some(format!("{} : {}", c.name, errs.join(", ")));
                }
            }
        }
        let name = f.file_name().unwrap().to_string_lossy().replace(".json.bin", "");
        if fail > 0 {
            println!("ÉCHEC {name:14} {ok} ok, {fail} faux, {skip} ignorés — ex. {}", first.unwrap());
        }
        if cyc_bad > 0 {
            println!("CYCLES {name:14} {cyc_ok} ok, {cyc_bad} faux — ex. {}", cyc_first.unwrap());
        }
        total_fail += fail + cyc_bad;
        summary.insert(name, (ok, fail, skip));
    }
    let ok: usize = summary.values().map(|v| v.0).sum();
    let skip: usize = summary.values().map(|v| v.2).sum();
    println!("TOTAL {ok} ok, {total_fail} faux, {skip} ignorés (erreurs d'adresse), {} fichiers", files.len());
    assert_eq!(total_fail, 0);
}
