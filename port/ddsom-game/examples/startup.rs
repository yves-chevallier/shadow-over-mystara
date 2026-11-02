//! Startup timeline (reset 0x350, RAM test 0x182a, Z80 handshake, `boot_init` 0x492):
//! frame, line and cycle of the first pass at each milestone, then registers at the scheduler's entry.
//!   startup <roms> <eeprom> [frames] [natives=0]
//!   startup <roms> <eeprom> <frames> compare: rewritten game and original code side by side; at each frame, number
//!   of different words (RAM outside the zones ignored by `validate`, GFX RAM, objects, palette) and sound state (shared RAM,
//!   Z80 RAM and registers), which `validate` does not compare.

use std::cell::RefCell;

/// Startup milestones (block start addresses).
const MILESTONES: &[(u32, &str)] = &[
    (0x350, "reset"),
    (0x182a, "test de RAM"),
    (0x1884, "test GFX 0x914000"),
    (0x189c, "test GFX 0x918000"),
    (0x18b4, "work RAM test"),
    (0x18cc, "Z80 released"),
    (0x18f2, "0x77 received, shared RAM test"),
    (0x190a, "end of the test"),
    (0x492, "boot_init"),
    (0x928, "ordonnanceur"),
    (0x574, "premier VBLANK"),
    (0x11b16, "startup task"),
];

thread_local! {
    static SEEN: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static IMAGE: RefCell<u64> = const { RefCell::new(0) };
    static NEXT: RefCell<Option<cps2::BlockExecutor>> = const { RefCell::new(None) };
}

fn note(pc: u32, c: &m68k::Cpu, b: &cps2::Cps2Bus) {
    if let Some(&(_, name)) = MILESTONES.iter().find(|j| j.0 == pc) {
        let new = SEEN.with(|v| {
            let mut v = v.borrow_mut();
            !v.contains(&pc) && {
                v.push(pc);
                true
            }
        });
        if new {
            let f = IMAGE.with(|i| *i.borrow());
            println!("{pc:06x} {name:36} frame {f} line {} cycle {}", b.scanline, c.cycles);
            if pc == 0x928 || pc == 0x492 {
                let r: Vec<String> = c.d.iter().chain(&c.a).map(|r| format!("{r:08x}")).collect();
                println!("    D0-D7 A0-A7 {} sr={:04x} usp={:08x}", r.join(" "), c.sr, c.other_sp);
            }
        }
    }
}

fn executor(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    note(pc, c, b);
    NEXT.with(|s| s.borrow().unwrap())(pc, c, b, dl)
}

fn ignored(word: usize) -> bool {
    let a = 2 * word;
    ddsom_game::tasks::table::kernel_ram(a as u32) || (0x2900..0x2a1a).contains(&a)
}

fn different(x: &[u16], y: &[u16]) -> usize {
    x.iter().zip(y).filter(|(p, q)| p != q).count()
}

fn compare(roms: &str, ee: Option<&[u8]>, images: u64) {
    let load = || {
        let mut m = cps2::Machine::load_ddsom(std::path::Path::new(roms), ee).unwrap();
        m.set_overclock(16.0);
        m
    };
    let (mut ma, mut mb) = (load(), load());
    ddsom_game::install(&mut ma);
    ddsom_game::install_reference(&mut mb);
    for f in 0..images {
        ma.run_frame();
        mb.run_frame();
        let ram = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !ignored(i)).count();
        let gfx = different(&ma.bus.gfxram, &mb.bus.gfxram);
        let obj = different(&ma.obj_buffer, &mb.obj_buffer) + (0..2).map(|k| different(&ma.bus.objram[k], &mb.bus.objram[k])).sum::<usize>();
        let pal = different(&ma.bus.palette, &mb.bus.palette);
        let qram = ma.bus.qram.iter().zip(&mb.bus.qram).filter(|(p, q)| p != q).count();
        let zram = ma.sound.ram.iter().zip(&mb.sound.ram).filter(|(p, q)| p != q).count();
        let z80 = format!("{:?}", ma.sound.z80) == format!("{:?}", mb.sound.z80);
        println!("frame {}: ram {ram} gfx {gfx} objects {obj} palette {pal} | shared {qram} Z80 ram {zram} Z80 registers {}", f + 1, if z80 { "identical" } else { "different" });
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    if a.get(4).is_some_and(|v| v == "compare") {
        return compare(&a[1], ee.as_deref(), a[3].parse().unwrap());
    }
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let images: u64 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(12);
    if a.get(4).is_some_and(|v| v == "1") {
        ddsom_game::install(&mut m);
    } else {
        ddsom_game::install_reference(&mut m);
    }
    m.set_overclock(16.0);
    NEXT.with(|s| *s.borrow_mut() = m.executor);
    m.executor = Some(executor);
    m.step_hook = Some(Box::new(|c, b, pc| note(pc, c, b)));
    println!("SSP initial {:08x} PC initial {:06x}", m.cpu.a[7], m.cpu.pc);
    for f in 0..images {
        IMAGE.with(|i| *i.borrow_mut() = f);
        m.run_frame();
        println!(
            "end of frame {f}: pc {:06x} sr {:04x} CFFD {:02x} CFFF {:02x} Z80 {} IRQ {}",
            m.cpu.pc,
            m.cpu.sr,
            m.bus.qram[0xffd],
            m.bus.qram[0xfff],
            if m.bus.z80_running { "running" } else { "in reset" },
            m.bus.irq_vblank
        );
    }
}
