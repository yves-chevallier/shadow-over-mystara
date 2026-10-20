//! Validation de la réécriture sur un scénario, image par image.
//!
//! Deux machines rejouent les mêmes entrées, 68000 surcadencé (le jeu finit toujours son travail
//! dans l'image : il est alors déterminé image par image, quelle que soit la durée de chaque
//! routine) :
//!   A : jeu réécrit (routines Rust, code d'origine pour le reste) ;
//!   B : code d'origine seul.
//! À chaque image, l'état du jeu (RAM de travail hors piles, GFX RAM, table de sprites, palette)
//! doit être identique. `DDSOM_CHECK=1` active en plus le contrôle appel par appel des routines
//! natives (diagnostic : registres et mémoire comparés à l'original sur une copie).
//!
//! validate <roms> <eeprom> <scenario> <images>

use std::path::Path;

/// Fréquence du 68000, en multiple de celle de l'arcade.
const OVERCLOCK: f64 = 16.0;

/// Mots de RAM sans signification propre : piles des tâches (FF0900-FF0FFF : la pile de la première tâche
/// déborde sous FF0A00 quand les appels sont profonds), pile logicielle de
/// l'interruption (FF2900-FF2A19), et le mot d'état de chaque tâche (FF0064 + 16 k), qui vaut
/// « prête » ou « en cours » selon l'instant exact de la fin d'image pour une tâche de fond, et la
/// dernière tâche lancée par l'ordonnanceur (FF0058, écrit en 0x96c : même raison).
fn ignored(word: usize) -> bool {
    let a = 2 * word;
    a == 0x58 || (0x0900..0x1000).contains(&a) || (0x2900..0x2a1a).contains(&a) || ((0x64..0x164).contains(&a) && (a - 0x64) % 16 < 4)
}

fn first_diff(a: &[u16], b: &[u16]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : validate <roms> <eeprom> <scenario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let load = || {
        let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
        m.set_overclock(std::env::var("DDSOM_OVERCLOCK").ok().and_then(|v| v.parse().ok()).unwrap_or(OVERCLOCK));
        m
    };
    let (mut ma, mut mb) = (load(), load());
    ddsom_game::install(&mut ma);
    ddsom_game::install_reference(&mut mb);
    mb.idle = Some(ddsom_game::idle);
    if std::env::var("DDSOM_NOIDLE").is_ok() {
        ma.idle = None;
        mb.idle = None;
    }
    let check = std::env::var("DDSOM_CHECK").is_ok();
    ddsom_game::set_check(check);
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    ddsom_game::set_count(true);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    let t0 = std::time::Instant::now();
    let mut failure = None;
    let lent: Option<u128> = std::env::var("DDSOM_LENT").ok().and_then(|v| v.parse().ok());
    // DDSOM_CHECK_FRAME=n : contrôle de tous les registres de chaque appel natif pendant l'image n
    let check_frame: Option<usize> = std::env::var("DDSOM_CHECK_FRAME").ok().and_then(|v| v.parse().ok());
    for f in 0..frames {
        if check_frame == Some(f + 1) {
            ddsom_game::set_check(true);
            ddsom_game::set_check_all(true);
        }
        script.apply(f, &mut ma);
        script.apply(f, &mut mb);
        let t = std::time::Instant::now();
        ma.run_frame();
        let ta = t.elapsed();
        mb.run_frame();
        let tb = t.elapsed() - ta;
        // DDSOM_LENT=ms : signale les images plus longues que ms millisecondes (par machine)
        if lent.is_some() && (f + 1) % 1000 == 0 {
            println!("image {} : {:.1?}", f + 1, t0.elapsed());
        }
        if let Some(seuil) = lent {
            if ta.as_millis() > seuil || tb.as_millis() > seuil {
                println!("image {} lente : réécrit {:.0?}, origine {:.0?}", f + 1, ta, tb);
            }
        }
        let ram: Vec<usize> = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !ignored(i)).collect();
        let mut what = Vec::new();
        for &i in ram.iter().take(10) {
            what.push(format!("ram {:06x} : {:04x} au lieu de {:04x}", 0xff0000 + 2 * i, ma.bus.ram[i], mb.bus.ram[i]));
        }
        if let Some(i) = first_diff(&ma.bus.gfxram, &mb.bus.gfxram) {
            what.push(format!("gfxram {:06x} : {:04x} au lieu de {:04x}", 0x900000 + 2 * i, ma.bus.gfxram[i], mb.bus.gfxram[i]));
        }
        if let Some(i) = first_diff(&ma.obj_buffer, &mb.obj_buffer) {
            what.push(format!("sprites, mot {i}"));
        }
        // les deux banques de la RAM d'objets (la banque lue par le jeu est celle de l'image précédente)
        for k in 0..2 {
            if let Some(i) = first_diff(&ma.bus.objram[k], &mb.bus.objram[k]) {
                what.push(format!("RAM d'objets, banque {k}, mot {i} : {:04x} au lieu de {:04x}", ma.bus.objram[k][i], mb.bus.objram[k][i]));
            }
        }
        if let Some(i) = first_diff(&ma.bus.palette, &mb.bus.palette) {
            what.push(format!("palette, mot {i}"));
        }
        if !what.is_empty() {
            println!("image {} : {} mots de RAM différents", f + 1, ram.len());
            for w in &what {
                println!("  {w}");
            }
            failure = Some(f + 1);
            break;
        }
    }
    println!("{frames} images en {:.1?}", t0.elapsed());
    let report = ddsom_game::report();
    for (name, st) in &report {
        let errors = if check {
            format!(" {:>5} écarts, cycles {} (natif {})", st.mismatches, st.original_cycles, st.native_cycles)
        } else {
            String::new()
        };
        println!("  {name:28} {:>8} appels{errors}{}", st.calls, st.first.as_ref().map(|f| format!(" — {f}")).unwrap_or_default());
    }
    if std::env::var("DDSOM_CHECK_ALL").is_ok() {
        // registres de sortie qui diffèrent de l'original, par routine native
        for (name, st) in &report {
            let liste: Vec<String> = st
                .regs
                .iter()
                .enumerate()
                .filter(|(_, &n)| n > 0)
                .map(|(i, n)| format!("{}{}:{n}", if i < 8 { 'd' } else { 'a' }, i % 8))
                .collect();
            if !liste.is_empty() {
                println!("REGS {name} {} appels : {}", st.calls, liste.join(" "));
            }
        }
    }
    match failure {
        None => println!("VALIDE : état identique sur {frames} images"),
        Some(f) => println!("ÉCHEC : première différence à l'image {f}"),
    }
}
