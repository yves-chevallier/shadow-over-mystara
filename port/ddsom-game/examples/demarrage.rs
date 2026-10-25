//! Chronologie du démarrage (reset 0x350, test de RAM 0x182a, poignée de main du Z80, `boot_init` 0x492) :
//! image, ligne et cycle du premier passage en chaque jalon, puis registres à l'entrée de l'ordonnanceur.
//!   demarrage <roms> <eeprom> [images] [natives=0]
//!   demarrage <roms> <eeprom> <images> compare : jeu réécrit et code d'origine côte à côte ; à chaque image, nombre
//!   de mots différents (RAM hors zones ignorées par `validate`, GFX RAM, objets, palette) et état du son (RAM partagée,
//!   RAM et registres du Z80), que `validate` ne compare pas.

use std::cell::RefCell;

/// Jalons du démarrage (adresses de début de bloc).
const JALONS: &[(u32, &str)] = &[
    (0x350, "reset"),
    (0x182a, "test de RAM"),
    (0x1884, "test GFX 0x914000"),
    (0x189c, "test GFX 0x918000"),
    (0x18b4, "test de la RAM de travail"),
    (0x18cc, "Z80 relâché"),
    (0x18f2, "0x77 reçu, test de la RAM partagée"),
    (0x190a, "fin du test"),
    (0x492, "boot_init"),
    (0x928, "ordonnanceur"),
    (0x574, "premier VBLANK"),
    (0x11b16, "tâche de démarrage"),
];

thread_local! {
    static VUS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static IMAGE: RefCell<u64> = const { RefCell::new(0) };
    static SUIVANT: RefCell<Option<cps2::BlockExecutor>> = const { RefCell::new(None) };
}

fn noter(pc: u32, c: &m68k::Cpu, b: &cps2::Cps2Bus) {
    if let Some(&(_, nom)) = JALONS.iter().find(|j| j.0 == pc) {
        let nouveau = VUS.with(|v| {
            let mut v = v.borrow_mut();
            !v.contains(&pc) && {
                v.push(pc);
                true
            }
        });
        if nouveau {
            let f = IMAGE.with(|i| *i.borrow());
            println!("{pc:06x} {nom:36} image {f} ligne {} cycle {}", b.scanline, c.cycles);
            if pc == 0x928 || pc == 0x492 {
                let r: Vec<String> = c.d.iter().chain(&c.a).map(|r| format!("{r:08x}")).collect();
                println!("    D0-D7 A0-A7 {} sr={:04x} usp={:08x}", r.join(" "), c.sr, c.other_sp);
            }
        }
    }
}

fn executeur(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    noter(pc, c, b);
    SUIVANT.with(|s| s.borrow().unwrap())(pc, c, b, dl)
}

fn ignored(word: usize) -> bool {
    let a = 2 * word;
    ddsom_game::taches::table::ram_du_noyau(a as u32) || (0x2900..0x2a1a).contains(&a)
}

fn differents(x: &[u16], y: &[u16]) -> usize {
    x.iter().zip(y).filter(|(p, q)| p != q).count()
}

fn comparer(roms: &str, ee: Option<&[u8]>, images: u64) {
    let charger = || {
        let mut m = cps2::Machine::load_ddsom(std::path::Path::new(roms), ee).unwrap();
        m.set_overclock(16.0);
        m
    };
    let (mut ma, mut mb) = (charger(), charger());
    ddsom_game::install(&mut ma);
    ddsom_game::install_reference(&mut mb);
    for f in 0..images {
        ma.run_frame();
        mb.run_frame();
        let ram = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !ignored(i)).count();
        let gfx = differents(&ma.bus.gfxram, &mb.bus.gfxram);
        let obj = differents(&ma.obj_buffer, &mb.obj_buffer) + (0..2).map(|k| differents(&ma.bus.objram[k], &mb.bus.objram[k])).sum::<usize>();
        let pal = differents(&ma.bus.palette, &mb.bus.palette);
        let qram = ma.bus.qram.iter().zip(&mb.bus.qram).filter(|(p, q)| p != q).count();
        let zram = ma.sound.ram.iter().zip(&mb.sound.ram).filter(|(p, q)| p != q).count();
        let z80 = format!("{:?}", ma.sound.z80) == format!("{:?}", mb.sound.z80);
        println!("image {} : ram {ram} gfx {gfx} objets {obj} palette {pal} | partagée {qram} ram Z80 {zram} registres Z80 {}", f + 1, if z80 { "identiques" } else { "différents" });
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    if a.get(4).is_some_and(|v| v == "compare") {
        return comparer(&a[1], ee.as_deref(), a[3].parse().unwrap());
    }
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let images: u64 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(12);
    if a.get(4).is_some_and(|v| v == "1") {
        ddsom_game::install(&mut m);
    } else {
        ddsom_game::install_reference(&mut m);
    }
    m.set_overclock(16.0);
    SUIVANT.with(|s| *s.borrow_mut() = m.executor);
    m.executor = Some(executeur);
    m.step_hook = Some(Box::new(|c, b, pc| noter(pc, c, b)));
    println!("SSP initial {:08x} PC initial {:06x}", m.cpu.a[7], m.cpu.pc);
    for f in 0..images {
        IMAGE.with(|i| *i.borrow_mut() = f);
        m.run_frame();
        println!(
            "fin de l'image {f} : pc {:06x} sr {:04x} CFFD {:02x} CFFF {:02x} Z80 {} IRQ {}",
            m.cpu.pc,
            m.cpu.sr,
            m.bus.qram[0xffd],
            m.bus.qram[0xfff],
            if m.bus.z80_running { "actif" } else { "au reset" },
            m.bus.irq_vblank
        );
    }
}
