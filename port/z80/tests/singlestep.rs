//! Tests SingleStepTests/z80 (JSON, licence MIT). Fichiers dans $Z80_TESTS (défaut ~/tools/z80-tests/v1).
//! Vérifie registres, RAM et nombre de T-états. cargo test --release -p z80 -- --nocapture

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;
use z80::{Bus, Z80};

struct Mem {
    ram: HashMap<u16, u8>,
    ports: Vec<(u16, u8)>,
}

impl Bus for Mem {
    fn read(&mut self, a: u16) -> u8 {
        *self.ram.get(&a).unwrap_or(&0)
    }
    fn write(&mut self, a: u16, v: u8) {
        self.ram.insert(a, v);
    }
    fn input(&mut self, port: u16) -> u8 {
        self.ports.iter().find(|p| p.0 == port).map(|p| p.1).unwrap_or(0xff)
    }
    fn output(&mut self, _port: u16, _v: u8) {}
}

fn u(v: &Value, k: &str) -> u64 {
    v[k].as_u64().unwrap_or_else(|| panic!("champ {k}"))
}

fn run(t: &Value) -> Vec<String> {
    let i = &t["initial"];
    let mut c = Z80::new();
    c.pc = u(i, "pc") as u16;
    c.sp = u(i, "sp") as u16;
    c.a = u(i, "a") as u8;
    c.f = u(i, "f") as u8;
    c.b = u(i, "b") as u8;
    c.c = u(i, "c") as u8;
    c.d = u(i, "d") as u8;
    c.e = u(i, "e") as u8;
    c.h = u(i, "h") as u8;
    c.l = u(i, "l") as u8;
    c.i = u(i, "i") as u8;
    c.r = u(i, "r") as u8;
    c.wz = u(i, "wz") as u16;
    c.ix = u(i, "ix") as u16;
    c.iy = u(i, "iy") as u16;
    c.af_ = u(i, "af_") as u16;
    c.bc_ = u(i, "bc_") as u16;
    c.de_ = u(i, "de_") as u16;
    c.hl_ = u(i, "hl_") as u16;
    c.im = u(i, "im") as u8;
    c.q = u(i, "q") as u8;
    c.iff1 = u(i, "iff1") != 0;
    c.iff2 = u(i, "iff2") != 0;
    let mut mem = Mem { ram: HashMap::new(), ports: Vec::new() };
    for e in i["ram"].as_array().unwrap() {
        mem.ram.insert(e[0].as_u64().unwrap() as u16, e[1].as_u64().unwrap() as u8);
    }
    if let Some(ps) = t["ports"].as_array() {
        for p in ps {
            if p[2].as_str() == Some("r") {
                mem.ports.push((p[0].as_u64().unwrap() as u16, p[1].as_u64().unwrap() as u8));
            }
        }
    }
    let cycles = c.step(&mut mem);

    let f = &t["final"];
    let mut errs = Vec::new();
    let got: [(&str, u64); 21] = [
        ("pc", c.pc as u64), ("sp", c.sp as u64), ("a", c.a as u64), ("f", c.f as u64), ("b", c.b as u64),
        ("c", c.c as u64), ("d", c.d as u64), ("e", c.e as u64), ("h", c.h as u64), ("l", c.l as u64),
        ("i", c.i as u64), ("r", c.r as u64), ("wz", c.wz as u64), ("ix", c.ix as u64), ("iy", c.iy as u64),
        ("af_", c.af_ as u64), ("bc_", c.bc_ as u64), ("de_", c.de_ as u64), ("hl_", c.hl_ as u64),
        ("im", c.im as u64), ("q", c.q as u64),
    ];
    for (k, g) in got {
        let e = u(f, k);
        if g != e {
            errs.push(format!("{k}={g:x} attendu {e:x}"));
        }
    }
    for (k, g) in [("iff1", c.iff1), ("iff2", c.iff2)] {
        if g as u64 != u(f, k) {
            errs.push(format!("{k}={g}"));
        }
    }
    for e in f["ram"].as_array().unwrap() {
        let a = e[0].as_u64().unwrap() as u16;
        let v = e[1].as_u64().unwrap() as u8;
        let g = mem.read(a);
        if g != v {
            errs.push(format!("ram[{a:04x}]={g:02x} attendu {v:02x}"));
        }
    }
    let expect = t["cycles"].as_array().unwrap().len() as u32;
    if cycles != expect {
        errs.push(format!("cycles={cycles} attendu {expect}"));
    }
    errs
}

#[test]
fn singlestep() {
    let dir = std::env::var_os("Z80_TESTS").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var("HOME").unwrap()).join("tools/z80-tests/v1")
    });
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("{} absent, test ignoré", dir.display());
        return;
    };
    let filter = std::env::var("Z80_FILTER").ok();
    let mut files: Vec<PathBuf> = rd.filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| filter.as_ref().is_none_or(|f| p.file_name().unwrap().to_string_lossy().starts_with(f.as_str())))
        .collect();
    files.sort();
    let (mut ok, mut fail, mut bad_files) = (0, 0, 0);
    for f in &files {
        let tests: Value = serde_json::from_slice(&std::fs::read(f).unwrap()).unwrap();
        let mut first = None;
        let mut nfail = 0;
        for t in tests.as_array().unwrap() {
            let e = run(t);
            if e.is_empty() {
                ok += 1;
            } else {
                nfail += 1;
                if first.is_none() {
                    first = Some(format!("{} : {}", t["name"].as_str().unwrap(), e.join(", ")));
                }
            }
        }
        if nfail > 0 {
            bad_files += 1;
            if bad_files <= 40 {
                println!("ÉCHEC {:12} {nfail} faux — ex. {}", f.file_name().unwrap().to_string_lossy(), first.unwrap());
            }
        }
        fail += nfail;
    }
    println!("TOTAL {ok} ok, {fail} faux, {} fichiers ({bad_files} en échec)", files.len());
    assert_eq!(fail, 0);
}
