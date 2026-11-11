//! Follows the mode machine (FF0E70-72) and the game phase (A5+0x2c): one line at each change.
//! modes_trace <roms> <eeprom> <scenario> <images>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_game::install_reference(&mut m);
    m.set_overclock(16.0);
    let s = ddsom_game::core::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    let mut last = (0u16, 0u16, 0u16, 0u16);
    for f in 0..n {
        s.apply(f, &mut m);
        m.run_frame();
        let r = |addr: u32| m.bus.ram[((addr - 0xff0000) / 2) as usize];
        let now = (r(0xff0e70), r(0xff0e72), r(0xff802c), r(0xff8034));
        if now != last {
            println!("frame {:6}: mode {:04x} {:04x}  phase/step {:04x}  scene {:04x}", f + 1, now.0, now.1, now.2, now.3 >> 8);
            last = now;
        }
    }
}
