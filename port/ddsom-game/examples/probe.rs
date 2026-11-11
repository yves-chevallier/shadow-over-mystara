//! Game state tracking: probe <roms> <eeprom> <scenario> <frames> [hex_address:size ...]
//! Prints at each change: mode (FF0E70-72), game phase (FF802C-E), active game/player
//! mask (FF8030-31), scene (FF8034), + the requested addresses (size 1, 2 or 4).
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_game::install_reference(&mut m);
    // like validate: overclocked 68000 (DDSOM_OC, default 16; 1 = arcade speed)
    m.set_overclock(std::env::var("DDSOM_OC").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0));
    let s = ddsom_game::core::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    let extra: Vec<(u32, u32)> = a[5..]
        .iter()
        .map(|t| {
            let (x, n) = t.split_once(':').unwrap_or((t, "1"));
            (u32::from_str_radix(x, 16).unwrap(), n.parse().unwrap())
        })
        .collect();
    let rd = |m: &cps2::Machine, ad: u32, n: u32| -> u32 {
        (0..n).fold(0, |v, k| {
            let o = (ad + k - 0xff0000) as usize;
            let w = m.bus.ram[o / 2];
            v << 8 | if o.is_multiple_of(2) { w >> 8 } else { w & 0xff } as u32
        })
    };
    let mut last = String::new();
    for f in 0..frames {
        s.apply(f, &mut m);
        m.run_frame();
        let mut st = format!(
            "mode {:02x}{:02x}{:02x} phase {:02x}/{:02x}/{:02x} active {:02x} players {:02x} scene {:02x} f35 {:02x}",
            rd(&m, 0xff0e70, 1), rd(&m, 0xff0e71, 1), rd(&m, 0xff0e72, 1),
            rd(&m, 0xff802c, 1), rd(&m, 0xff802d, 1), rd(&m, 0xff802e, 1),
            rd(&m, 0xff8030, 1), rd(&m, 0xff8031, 1), rd(&m, 0xff8034, 1), rd(&m, 0xff8035, 1));
        for &(ad, n) in &extra {
            st += &format!(" [{ad:x}]={:0w$x}", rd(&m, ad, n), w = 2 * n as usize);
        }
        // DUMP=hex_address:length:frame,frame,...: hexdump of the RAM at these frames
        if let Ok(d) = std::env::var("DUMP") {
            let p: Vec<&str> = d.split(':').collect();
            let ad = u32::from_str_radix(p[0], 16).unwrap();
            let len: u32 = p[1].parse().unwrap();
            if p[2].split(',').any(|x| x.parse::<usize>().ok() == Some(f + 1)) {
                for o in (0..len).step_by(16) {
                    let row: Vec<String> = (0..16.min(len - o)).map(|k| format!("{:02x}", rd(&m, ad + o + k, 1))).collect();
                    println!("{f:6} {:06x}: {}", ad + o, row.join(" "));
                }
            }
        }
        if st != last {
            println!("{:6} {st}", f + 1);
            last = st;
        }
    }
}
