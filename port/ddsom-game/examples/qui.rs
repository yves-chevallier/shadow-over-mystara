//! Qui entre dans une plage de code 68000 : pour chaque entrée (instruction de la plage précédée d'une
//! instruction 68000 hors de la plage, ou d'une routine native), l'image, l'adresse et la dernière
//! instruction 68000 exécutée avant (« natif » si une native s'est exécutée entre les deux).
//!
//!   qui <roms> <eeprom> <scénario> <images> <début> <fin> [max]

use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

static LAST: AtomicU32 = AtomicU32::new(0);
static FROM: AtomicU32 = AtomicU32::new(0);
static TO: AtomicU32 = AtomicU32::new(0);
static FRAME: AtomicU64 = AtomicU64::new(0);
static LEFT: AtomicU32 = AtomicU32::new(20);
static STEPS: AtomicU64 = AtomicU64::new(0);
static LAST_STEP: AtomicU64 = AtomicU64::new(0);
/// Natives en cours d'exécution (la dernière est l'appelante du code 68000 courant).
static NATIVES: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// Une native a commencé ou fini depuis la dernière instruction 68000.
static NATIVE_SEEN: AtomicU32 = AtomicU32::new(0);

fn hook(pc: u32, enter: bool) {
    let mut v = NATIVES.lock().unwrap();
    if enter {
        v.push(pc);
    } else {
        v.pop();
    }
    NATIVE_SEEN.store(1, Ordering::Relaxed);
}

fn trace(pc: u32) {
    let step = STEPS.fetch_add(1, Ordering::Relaxed);
    let (from, to) = (FROM.load(Ordering::Relaxed), TO.load(Ordering::Relaxed));
    let last = LAST.swap(pc, Ordering::Relaxed);
    LAST_STEP.store(step, Ordering::Relaxed);
    let contiguous = NATIVE_SEEN.swap(0, Ordering::Relaxed) == 0;
    let inside = |a: u32| (from..to).contains(&a);
    if inside(pc) && !inside(last) && LEFT.load(Ordering::Relaxed) > 0 {
        LEFT.fetch_sub(1, Ordering::Relaxed);
        let via = if contiguous {
            format!("{last:06x}")
        } else {
            let stack: Vec<String> = NATIVES.lock().unwrap().iter().map(|a| format!("{a:06x}")).collect();
            format!("natives [{}] (dernier 68000 {last:06x})", stack.join(" > "))
        };
        println!("image {} : {pc:06x} depuis {via}", FRAME.load(Ordering::Relaxed));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames, from, to, rest @ ..] = &a[..] else {
        eprintln!("usage : qui <roms> <eeprom> <scénario> <images> <début> <fin> [max]");
        std::process::exit(2);
    };
    FROM.store(u32::from_str_radix(from, 16).unwrap(), Ordering::Relaxed);
    TO.store(u32::from_str_radix(to, 16).unwrap(), Ordering::Relaxed);
    if let Some(n) = rest.first() {
        LEFT.store(n.parse().unwrap(), Ordering::Relaxed);
    }
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    m.executor = Some(ddsom_game::run_traced);
    ddsom_game::set_native_hook(hook);
    ddsom_game::set_trace(trace);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..frames.parse::<usize>().unwrap() {
        FRAME.store(f as u64, Ordering::Relaxed);
        script.apply(f, &mut m);
        m.run_frame();
        if LEFT.load(Ordering::Relaxed) == 0 {
            break;
        }
    }
}
