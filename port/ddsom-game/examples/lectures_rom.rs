//! Lectures de la ROM programme (0x000000-0x3fffff) faites comme données par le code Rust, hors exécution
//! d'instructions 68000 : `prog8`/`prog16`/`prog32` (espace programme déchiffré), `rd8`/`data16`/`data32`
//! et lectures du bus (espace données).
//!
//!   cargo build --release -p ddsom-game --example lectures_rom --features rom-trace
//!   lectures_rom <roms> <eeprom> <sortie.txt> <scénario>:<images> ...
//!
//! Variables : `DDSOM_MASQUE=n` décale de n bits les numéros de scénario du masque (pour fusionner plusieurs
//! exécutions) ; `DDSOM_COUVERT=fichier` (écrit par `couverture_rom`) : pour chaque mot lu hors des zones
//! couvertes, la sortie ajoute une ligne `s d|p adresse fichier:ligne;...` : les trois premiers appelants dans
//! `ddsom-game/src` de la première lecture.
//!
//! Chaque scénario est rejoué sur la machine hybride (natives Rust, le reste interprété une instruction à la
//! fois), ou, avec `DDSOM_SANS_68K=1`, sur la machine sans 68000 (natives seules : toute lecture compte ; un arrêt
//! faute de native finit le scénario à cette image, signalé sur la sortie d'erreur). La sortie donne, pour chaque zone dont tous les mots sont lus par les mêmes scénarios :
//! `d|p début fin masque lectures` (hexadécimal, `d` : espace données, `p` : espace programme ; `fin` exclue ;
//! `masque` : bit n = scénario n). Les zones sont comparées aux ressources par l'exemple `couverture_rom`
//! de `ddsom-assets` (docs/assets.md).

use std::fmt::Write as _;
use std::path::Path;

thread_local! {
    static COUVERT: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    static ORIGINES: std::cell::RefCell<std::collections::BTreeMap<(bool, u32), String>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Appelé à la première lecture d'un mot : si les ressources ne le couvrent pas, note qui l'a lu.
fn origine(programme: bool, adresse: u32) {
    let couvert = COUVERT.with(|c| {
        let c = c.borrow();
        c.is_empty() || c[programme as usize * 0x200000 + (adresse >> 1) as usize] != 0
    });
    if couvert {
        return;
    }
    let trace = std::backtrace::Backtrace::force_capture().to_string();
    let mut sites = Vec::new();
    for l in trace.lines() {
        if let Some(at) = l.trim().strip_prefix("at ") {
            if let Some(i) = at.find("ddsom-game/src/") {
                let site = at[i + "ddsom-game/src/".len()..].trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');
                let ligne = at.rsplitn(3, ':').nth(1).unwrap_or("");
                sites.push(format!("{site}:{ligne}"));
                if sites.len() == 6 {
                    break;
                }
            }
        }
    }
    ORIGINES.with(|o| o.borrow_mut().insert((programme, adresse), sites.join(";")));
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage : lectures_rom <roms> <eeprom> <sortie.txt> <scénario>:<images> ...");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let base: usize = std::env::var("DDSOM_MASQUE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    if let Some(f) = std::env::var_os("DDSOM_COUVERT") {
        COUVERT.with(|c| *c.borrow_mut() = std::fs::read(f).expect("fichier de couverture"));
        cps2::bus::rom_trace::set_hook(origine);
    }
    let sans_68k = std::env::var("DDSOM_SANS_68K").is_ok_and(|v| v == "1");
    let mut mask = vec![[0u32; 2]; 0x200000];
    let mut reads = vec![[0u64; 2]; 0x200000];
    for (n, spec) in a[4..].iter().enumerate() {
        let (path, frames) = spec.rsplit_once(':').expect("scénario:images");
        let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
        if sans_68k {
            ddsom_game::install_sans_68k(&mut m);
        } else {
            m.set_overclock(16.0);
            ddsom_game::install(&mut m);
            m.executor = Some(ddsom_game::run_traced);
        }
        let script = ddsom_game::scenario::Scenario::load(Path::new(path)).expect("scénario");
        cps2::bus::rom_trace::take();
        for f in 0..frames.parse::<usize>().unwrap() {
            script.apply(f, &mut m);
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())) {
                let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
                eprintln!("{path} : ARRÊT à l'image {f} : {}", msg.chars().take(300).collect::<String>());
                break;
            }
        }
        let r = cps2::bus::rom_trace::take();
        for v in 0..2 {
            for (w, &c) in r.0[v].iter().enumerate().filter(|(_, &c)| c > 0) {
                mask[w][v] |= 1 << (base + n);
                reads[w][v] += c as u64;
            }
        }
        eprintln!("{path} : {} images", frames);
    }
    let mut out = String::new();
    for (v, name) in [(0, 'd'), (1, 'p')] {
        let mut w = 0;
        while w < mask.len() {
            if mask[w][v] == 0 {
                w += 1;
                continue;
            }
            let (start, m, mut n) = (w, mask[w][v], 0);
            while w < mask.len() && mask[w][v] == m {
                n += reads[w][v];
                w += 1;
            }
            writeln!(out, "{name} {:06x} {:06x} {m:x} {n}", 2 * start, 2 * w).unwrap();
        }
    }
    ORIGINES.with(|o| {
        for (&(p, adresse), sites) in o.borrow().iter() {
            writeln!(out, "s {} {adresse:06x} {sites}", if p { 'p' } else { 'd' }).unwrap();
        }
    });
    std::fs::write(&a[3], out).unwrap();
}
