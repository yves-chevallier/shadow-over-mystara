//! Table des tâches à une image donnée, dans la machine d'origine (enregistrements FF0064) et dans le jeu (table
//! Rust), puis leurs écarts : tasks <roms> <eeprom> <scenario> <image>
use ddsom_game::taches::table::{ecarts, enregistrement, lire, Reprise};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let load = || {
        let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
        m.set_overclock(16.0);
        m
    };
    let (mut origine, mut jeu) = (load(), load());
    ddsom_game::install_reference(&mut origine);
    ddsom_game::install(&mut jeu);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    for f in 0..n {
        for m in [&mut origine, &mut jeu] {
            s.apply(f, m);
            m.run_frame();
        }
    }
    println!("origine (RAM) :");
    for k in 0..16 {
        let t = enregistrement(k);
        let w = |o: u32| origine.bus.ram[((t + o - 0xff0000) / 2) as usize];
        println!("  tâche {k:2} : état {:04x} pc {:04x}{:04x} usp {:04x}{:04x} pile {:04x}{:04x}", w(0), w(4), w(6), w(8), w(10), w(12), w(14));
    }
    let table = lire(&jeu.bus).expect("table des tâches du jeu");
    println!("jeu (table Rust, courante {}) :", table.courante);
    for (k, e) in table.emplacements.iter().enumerate() {
        let reprise = match e.reprise {
            Reprise::Entree(pc) => format!("entrée {pc:x}"),
            Reprise::Cession(c) => format!("reprise {:x} usp {:x}", c.pc, c.usp),
        };
        println!("  tâche {k:2} : {:?} {} pile {:x} {reprise}", e.etat, e.compte, e.pile);
    }
    for e in ecarts(table, &origine.bus.ram, true) {
        println!("écart : {e}");
    }
}
