//! Affiche PC/D0/A0 à la fin des premières images (comparaison avec MAME).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    for f in 1..=6 {
        m.run_frame();
        println!("frame {f} pc={:06x} d0={:08x} a0={:08x} cycles={}", m.cpu.pc, m.cpu.d[0], m.cpu.a[0], m.cpu.cycles);
    }
}
