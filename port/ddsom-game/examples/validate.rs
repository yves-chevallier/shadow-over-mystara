//! Validation de la réécriture sur un scénario, image par image.
//!
//! Deux machines rejouent les mêmes entrées, 68000 surcadencé (le jeu finit toujours son travail
//! dans l'image : il est alors déterminé image par image, quelle que soit la durée de chaque
//! routine) :
//!   A : jeu réécrit (routines Rust, code d'origine pour le reste) ;
//!   B : code d'origine seul.
//! À chaque image, l'état du jeu (RAM de travail hors piles, GFX RAM, table de sprites, palette)
//! doit être identique, et la table des tâches Rust du jeu doit valoir celle de la RAM de l'original. `DDSOM_CHECK=1` active en plus le contrôle appel par appel des routines
//! natives (diagnostic : registres et mémoire comparés à l'original sur une copie).
//!
//! `DDSOM_SANS_68K=1` : la machine A tourne sans 68000 (natives seules, boucle d'image du noyau,
//! `ddsom_game::install_sans_68k`) ; une adresse sans native l'arrête (« ÉCHEC : arrêt sans 68000 »).
//!
//! validate <roms> <eeprom> <scenario> <images>

use std::path::Path;

use ddsom_game::taches::table::{ecarts, lire, ram_du_noyau};

/// Fréquence du 68000, en multiple de celle de l'arcade.
const OVERCLOCK: f64 = 16.0;

/// Mots de RAM sans signification propre, ou que le jeu réécrit ne tient plus en RAM :
/// - le noyau (`ram_du_noyau`) : pile de l'ordonnanceur, tâche courante, tas des piles, enregistrements des tâches. Le
///   jeu les garde dans sa table des tâches en Rust, comparée à part à celle de la RAM de la machine d'origine
///   (`ecarts`) ;
/// - les piles des tâches (FF01A6-FF0DA5) et celle du superviseur (jusqu'à FF0E66), dans la même zone : le jeu n'y
///   sauve plus les registres des tâches qui cèdent la main (ils sont dans la table) et le reste n'est qu'adresses de
///   retour et sauvegardes de registres ; FF0E66-FF0FFF était déjà ignoré (débordement de la pile de la première
///   tâche) ;
/// - la pile logicielle de l'interruption (FF2900-FF2A19).
fn ignored(word: usize) -> bool {
    let a = 2 * word;
    ram_du_noyau(a as u32) || (0x2900..0x2a1a).contains(&a)
}

/// Images du démarrage (1 à 5) : le code d'origine y teste encore la RAM (`ddsom_game::demarrage`). Ce test est non
/// destructif mais, à la fin d'une image, le mot en cours de test peut porter un motif (0x8888, 0x4444...) au lieu de sa
/// valeur ; la version native n'a pas d'état intermédiaire. Pendant ces images, un mot de différence au plus est toléré,
/// en RAM de travail ou en GFX RAM ; tout le reste est comparé. En pratique seules les images 1 et 2 (GFX RAM) en ont un.
const DEMARRAGE: usize = 5;

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
    let sans_68k = std::env::var("DDSOM_SANS_68K").is_ok_and(|v| v == "1");
    if sans_68k {
        ddsom_game::install_sans_68k(&mut ma);
    } else {
        ddsom_game::install(&mut ma);
    }
    ddsom_game::install_reference(&mut mb);
    mb.idle = Some(ddsom_game::idle);
    if std::env::var("DDSOM_NOIDLE").is_ok() && !sans_68k {
        ma.idle = None;
        mb.idle = None;
    }
    let check = std::env::var("DDSOM_CHECK").is_ok();
    let taches_tout = std::env::var("DDSOM_TACHES_TOUT").is_ok();
    ddsom_game::set_check(check);
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    ddsom_game::set_count(true);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    let t0 = std::time::Instant::now();
    let mut failure = None;
    let mut arret: Option<String> = None;
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
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ma.run_frame())) {
            // arrêt du jeu sans 68000 (adresse sans native...) ; toute autre panique suit son cours
            match e.downcast_ref::<String>().and_then(|m| m.strip_prefix(ddsom_game::sans68k::ARRET)) {
                Some(m) => {
                    arret = Some(m.to_owned());
                    failure = Some(f + 1);
                    break;
                }
                None => std::panic::resume_unwind(e),
            }
        }
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
        let gfx = ma.bus.gfxram.iter().zip(&mb.bus.gfxram).filter(|(x, y)| x != y).count();
        let mot_en_test = f < DEMARRAGE && ram.len() + gfx <= 1;
        let mut what = Vec::new();
        for &i in ram.iter().take(10).filter(|_| !mot_en_test) {
            what.push(format!("ram {:06x} : {:04x} au lieu de {:04x}", 0xff0000 + 2 * i, ma.bus.ram[i], mb.bus.ram[i]));
        }
        if let Some(i) = first_diff(&ma.bus.gfxram, &mb.bus.gfxram).filter(|_| !mot_en_test) {
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
        // test différentiel de la table des tâches (DDSOM_TACHES_TOUT=1 : SR et registres sauvés aussi)
        if let Some(t) = lire(&ma.bus) {
            what.extend(ecarts(t, &mb.bus.ram, taches_tout).into_iter().take(10));
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
    if sans_68k {
        println!("sans 68000 : {} images arrêtées par le plafond de natives", ddsom_game::sans68k::bilan());
    }
    if let Some(m) = &arret {
        println!("ARRÊT {m}");
    }
    match failure {
        None => println!("VALIDE : état identique sur {frames} images"),
        Some(f) if arret.is_some() => println!("ÉCHEC : arrêt sans 68000 à l'image {f}"),
        Some(f) => println!("ÉCHEC : première différence à l'image {f}"),
    }
}
