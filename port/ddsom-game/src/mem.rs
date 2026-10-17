//! Accès typés à la mémoire du jeu.

use cps2::Cps2Bus;
use m68k::Bus;

/// Base de la RAM de travail adressée par A5 dans tout le jeu.
pub const A5: u32 = 0xff8000;

pub fn rd8(b: &mut Cps2Bus, a: u32) -> u8 {
    b.read8(a & 0xff_ffff)
}
pub fn rd16(b: &mut Cps2Bus, a: u32) -> u16 {
    b.read16(a & 0xff_ffff)
}
pub fn rd32(b: &mut Cps2Bus, a: u32) -> u32 {
    (rd16(b, a) as u32) << 16 | rd16(b, a.wrapping_add(2)) as u32
}
pub fn wr8(b: &mut Cps2Bus, a: u32, v: u8) {
    b.write8(a & 0xff_ffff, v)
}
pub fn wr16(b: &mut Cps2Bus, a: u32, v: u16) {
    b.write16(a & 0xff_ffff, v)
}
pub fn wr32(b: &mut Cps2Bus, a: u32, v: u32) {
    wr16(b, a, (v >> 16) as u16);
    wr16(b, a.wrapping_add(2), v as u16);
}

/// Dépile l'adresse de retour (RTS).
pub fn rts(c: &mut m68k::Cpu, b: &mut Cps2Bus) {
    c.pc = rd32(b, c.a[7]);
    c.a[7] = c.a[7].wrapping_add(4);
}

/// Remplace l'octet bas d'un registre de données (move.b vers Dn).
pub fn set_low_byte(r: &mut u32, v: u8) {
    *r = (*r & !0xff) | v as u32;
}

/// Écarts entre deux bus (mode contrôle). `dead_stack` : zone de pile sous le pointeur de pile au
/// retour, dont le contenu (restes des appels internes) n'a plus de sens.
pub fn diff_bus(a: &Cps2Bus, b: &Cps2Bus, dead_stack: std::ops::Range<u32>, out: &mut Vec<String>) {
    fn first_diff(name: &str, base: u32, x: &[u16], y: &[u16], out: &mut Vec<String>) {
        if let Some(i) = x.iter().zip(y).position(|(p, q)| p != q) {
            out.push(format!("{name}[{:06x}]={:04x} au lieu de {:04x}", base + 2 * i as u32, x[i], y[i]));
        }
    }
    // pile privée des wrappers son (pointeur en FF2A1A) : les registres qu'ils y sauvegardent restent
    // en RAM sous le pointeur, sans signification (voir docs/son-68000.md)
    let prive = (a.ram[0x2a1a / 2] as u32) << 16 | a.ram[0x2a1c / 2] as u32;
    let dead_prive = (prive & 0xff_ffff).saturating_sub(0x400)..prive & 0xff_ffff;
    let live = |i: usize| {
        let adresse = 0xff0000 + 2 * i as u32;
        !dead_stack.contains(&adresse) && !dead_prive.contains(&adresse)
    };
    let verbose = std::env::var("DDSOM_VERBOSE").is_ok();
    for i in (0..a.ram.len()).filter(|&i| a.ram[i] != b.ram[i] && live(i)).take(if verbose { 12 } else { 1 }) {
        out.push(format!("ram[{:06x}]={:04x} au lieu de {:04x}", 0xff0000 + 2 * i as u32, a.ram[i], b.ram[i]));
    }
    first_diff("gfxram", 0x900000, &a.gfxram, &b.gfxram, out);
    first_diff("objram0", 0, &a.objram[0], &b.objram[0], out);
    first_diff("objram1", 0, &a.objram[1], &b.objram[1], out);
    first_diff("output", 0x400000, &a.output, &b.output, out);
    first_diff("cps_a", 0x804100, &a.cps_a, &b.cps_a, out);
    first_diff("cps_b", 0x804140, &a.cps_b, &b.cps_b, out);
    first_diff("extra_ram", 0x660000, &a.extra_ram, &b.extra_ram, out);
    first_diff("palette", 0, &a.palette, &b.palette, out);
    if a.qram != b.qram {
        out.push("qram".into());
    }
    if a.eeprom_out != b.eeprom_out || a.eeprom != b.eeprom {
        out.push("eeprom".into());
    }
    if a.objram_bank != b.objram_bank {
        out.push("objram_bank".into());
    }
}
