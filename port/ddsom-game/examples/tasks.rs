//! État de la table des tâches à une image donnée : tasks <roms> <eeprom> <scenario> <image>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_rc::install(&mut m);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    for f in 0..n {
        s.apply(f, &mut m);
        m.run_frame();
    }
    for k in 0..16u32 {
        let t = 0xff0064 + 16 * k;
        let w = |o: u32| m.bus.ram[((t + o - 0xff0000) / 2) as usize];
        println!("tâche {k:2} : état {:04x} pc {:04x}{:04x} usp {:04x}{:04x} pile {:04x}{:04x}", w(0), w(4), w(6), w(8), w(10), w(12), w(14));
    }
}
