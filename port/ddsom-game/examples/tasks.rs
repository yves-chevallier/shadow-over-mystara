//! Task table at a given frame, in the original machine (records FF0064) and in the game (Rust
//! table), then their differences: tasks <roms> <eeprom> <scenario> <frame>
use ddsom_game::tasks::table::{gaps, record, read, Resume};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let load = || {
        let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
        m.set_overclock(16.0);
        m
    };
    let (mut original, mut game) = (load(), load());
    ddsom_game::install_reference(&mut original);
    ddsom_game::install(&mut game);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    for f in 0..n {
        for m in [&mut original, &mut game] {
            s.apply(f, m);
            m.run_frame();
        }
    }
    println!("origine (RAM) :");
    for k in 0..16 {
        let t = record(k);
        let w = |o: u32| original.bus.ram[((t + o - 0xff0000) / 2) as usize];
        println!("  task {k:2}: state {:04x} pc {:04x}{:04x} usp {:04x}{:04x} stack {:04x}{:04x}", w(0), w(4), w(6), w(8), w(10), w(12), w(14));
    }
    let table = read(&game.bus).expect("game task table");
    println!("jeu (table Rust, courante {}) :", table.current);
    for (k, e) in table.slots.iter().enumerate() {
        let resume = match e.resume {
            Resume::Entry(task) => format!("entry {task:?} ({:x})", task.address()),
            Resume::Step(step) => format!("step {step:?} ({:x})", step.address()),
            Resume::Address(pc) => format!("address {pc:x}"),
        };
        println!("  task {k:2}: {:?} {:?} {} stack {:x} {resume}", e.task, e.state, e.count, e.stack);
    }
    for e in gaps(table, &original.bus.ram, true) {
        println!("mismatch: {e}");
    }
}
