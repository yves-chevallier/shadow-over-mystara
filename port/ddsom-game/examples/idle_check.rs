//! Skipping the busy wait (`ddsom_game::idle`) does not change the state: two reference machines (original code), one without `idle`, the other with it, compared at every frame.
//!   idle_check <roms> <eeprom> <scenario> <frames>
fn ignored(word: usize) -> bool {
    let a = 2 * word;
    a == 0x58 || (0x0900..0x1000).contains(&a) || (0x2900..0x2a1a).contains(&a) || ((0x64..0x164).contains(&a) && (a - 0x64) % 16 < 4)
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let load = || {
        let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
        m.set_overclock(16.0);
        m
    };
    let (mut ma, mut mb) = (load(), load());
    ddsom_game::install_reference(&mut ma);
    ddsom_game::install_reference(&mut mb);
    ma.idle = None;
    mb.idle = Some(ddsom_game::idle);
    let script = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    for f in 0..frames {
        script.apply(f, &mut ma);
        script.apply(f, &mut mb);
        let t = std::time::Instant::now();
        ma.run_frame();
        let ta = t.elapsed();
        mb.run_frame();
        let tb = t.elapsed() - ta;
        let diff: Vec<usize> = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !ignored(i)).collect();
        if !diff.is_empty() || ma.bus.gfxram != mb.bus.gfxram || ma.obj_buffer != mb.obj_buffer || ma.bus.palette != mb.bus.palette {
            println!("frame {}: {} different RAM words, first {:06x}", f + 1, diff.len(), diff.first().map_or(0, |i| 0xff0000 + 2 * i));
            return;
        }
        if f % 500 == 0 {
            println!("frame {}: without idle {:.0?}, with {:.0?}", f + 1, ta, tb);
        }
    }
    println!("IDENTICAL over {frames} frames");
}
