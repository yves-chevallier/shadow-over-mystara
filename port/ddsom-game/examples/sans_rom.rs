//! Le jeu sans ROM : une machine faite des seules ressources extraites joue-t-elle comme celle des ROMs ?
//!
//!   sans_rom <roms> <eeprom> <assets> <scénario>:<images> ...
//!
//! Deux machines sans 68000 (`ddsom_game::install_sans_68k`) avec le pilote son Rust rejouent chaque scénario :
//!   R : contenus lus dans les ROMs (`cps2::Contenus::depuis_roms`) ;
//!   A : contenus reconstruits depuis le dossier d'assets (`ddsom_assets::rebuild::contenus`), aucun fichier ROM
//!       ouvert : programme 68000 réduit à ses tables, région du Z80 sans programme, DSP réduit à ses tables.
//! À chaque image : l'image affichée, la RAM de travail, la GFX RAM, la RAM d'objets, la palette, la RAM partagée
//! et les échantillons audio doivent être identiques. Un arrêt du jeu sans 68000 (adresse sans native) sur les
//! deux machines à la même image clôt le scénario sans le compter en échec (trou du mode sans 68000, pas des
//! ressources). Sortie : « IDENTIQUE » ou la première différence, par scénario.

use std::path::Path;

use cps2::{Contenus, Machine};

fn machine(contenus: Contenus, eeprom: Option<&[u8]>) -> Machine {
    let mut m = Machine::ddsom(contenus, eeprom);
    ddsom_game::install_sans_68k(&mut m);
    m.sound.use_rust_driver();
    m
}

/// Une image, ou le message d'arrêt du jeu sans 68000.
fn image(m: &mut Machine) -> Result<Vec<[u8; 3]>, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())).map_err(|e| {
        let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
        match msg.strip_prefix(ddsom_game::sans68k::ARRET) {
            Some(m) => m.chars().take(200).collect(),
            None => std::panic::resume_unwind(e),
        }
    })
}

/// Première partie de l'état qui diffère entre les deux machines.
fn ecart(r: &Machine, a: &Machine) -> Option<String> {
    let premier = |x: &[u16], y: &[u16]| x.iter().zip(y).position(|(p, q)| p != q);
    if let Some(i) = premier(&r.bus.ram, &a.bus.ram) {
        return Some(format!("RAM {:06x} : {:04x} au lieu de {:04x}", 0xff0000 + 2 * i, a.bus.ram[i], r.bus.ram[i]));
    }
    if let Some(i) = premier(&r.bus.gfxram, &a.bus.gfxram) {
        return Some(format!("GFX RAM {:06x}", 0x900000 + 2 * i));
    }
    for k in 0..2 {
        if premier(&r.bus.objram[k], &a.bus.objram[k]).is_some() {
            return Some(format!("RAM d'objets, banque {k}"));
        }
    }
    if premier(&r.bus.palette, &a.bus.palette).is_some() {
        return Some("palette".into());
    }
    if let Some(i) = r.bus.qram.iter().zip(&a.bus.qram).position(|(p, q)| p != q) {
        return Some(format!("RAM partagée {i:03x}"));
    }
    None
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage : sans_rom <roms> <eeprom> <assets> <scénario>:<images> ...");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let mut echecs = 0;
    for spec in &a[4..] {
        let (path, frames) = spec.rsplit_once(':').expect("scénario:images");
        let frames: usize = frames.parse().expect("images");
        let script = ddsom_game::scenario::Scenario::load(Path::new(path)).expect("scénario");
        let roms = Contenus::depuis_roms(Path::new(&a[1]), None, None).expect("roms");
        let assets = ddsom_assets::rebuild::contenus(Path::new(&a[3])).expect("assets");
        let (mut mr, mut ma) = (machine(roms, ee.as_deref()), machine(assets, ee.as_deref()));
        let t0 = std::time::Instant::now();
        let mut verdict = format!("IDENTIQUE sur {frames} images");
        for f in 0..frames {
            script.apply(f, &mut mr);
            script.apply(f, &mut ma);
            let (ir, ia) = (image(&mut mr), image(&mut ma));
            let (sr, sa): (Vec<_>, Vec<_>) = (mr.sound.audio.drain(..).collect(), ma.sound.audio.drain(..).collect());
            let diff = match (&ir, &ia) {
                (Err(_), Err(_)) => {
                    verdict = format!("IDENTIQUE sur {f} images (puis arrêt sans 68000 des deux : {})", ir.unwrap_err());
                    break;
                }
                (Ok(x), Ok(y)) if x != y => Some("image affichée".to_string()),
                (Ok(_), Ok(_)) if sr != sa => Some("échantillons audio".to_string()),
                (Ok(_), Ok(_)) => ecart(&mr, &ma),
                (Err(m), Ok(_)) => Some(format!("arrêt de la machine des ROMs seule : {m}")),
                (Ok(_), Err(m)) => Some(format!("arrêt de la machine des assets seule : {m}")),
            };
            if let Some(d) = diff {
                verdict = format!("DIFFÉRENT à l'image {} : {d}", f + 1);
                echecs += 1;
                break;
            }
        }
        println!("{path} : {verdict} ({:.1?})", t0.elapsed());
    }
    std::process::exit(if echecs == 0 { 0 } else { 1 });
}
