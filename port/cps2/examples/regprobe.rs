//! Registres à la fin de chaque image (même format que le script Lua regprobe).
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let n: u32 = a[3].parse().unwrap();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    for f in 1..=n {
        m.run_frame();
        let c = &m.cpu;
        let mut s = format!("R {f} {:06x} {:04x}", c.pc, c.sr);
        for v in c.d.iter().chain(c.a.iter()) {
            s += &format!(" {v:08x}");
        }
        println!("{s}");
    }
}
