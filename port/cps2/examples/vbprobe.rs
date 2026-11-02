//! Vblank counter (FF806C), SR and PC at the end of frames 60 to 100.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    for f in 1..=100u32 {
        m.run_frame();
        if f >= 60 {
            println!("V {f} cnt={:04x} sr={:04x} pc={:06x}", m.bus.ram[0x806c / 2], m.cpu.sr, m.cpu.pc);
        }
    }
}
