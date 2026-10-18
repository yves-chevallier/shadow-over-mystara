//! Scénarios d'entrées (format de mame/scenario.lua : « image champ valeur »).
//! Comme dans MAME, une entrée posée après l'image n prend effet à l'image n + 2.
//!
//! Format : une action par ligne, `image champ valeur`.
//!   * `900 Coin 1 1` / `906 Coin 1 0` : entrée MAME (appuyée = 1, relâchée = 0).
//!   * `quit` : ligne finale ignorée ici (utilisée par MAME).
//!   * **`2000 poke8 ff8034 5`**, `poke16`, `poke32` : écrit `valeur` (décimal ou `0x…`) à
//!     l'adresse hexadécimale `ff0000-ffffff` de la RAM de travail, en big-endian, juste avant
//!     l'exécution de l'image `n + 1` (même instant qu'une entrée). Les deux machines de
//!     `validate` appliquent la même écriture. Adresse ou valeur invalide : ligne ignorée.
//!     MAME ignore ces lignes.
//!   * Les lignes vides et celles commençant par `#` sont ignorées.
//! Les scénarios ne contiennent que des entrées et des adresses/valeurs de RAM, jamais de données de ROM.

use std::io;
use std::path::Path;

pub struct Scenario {
    actions: Vec<(usize, u8, u16, bool)>, // (image, port, bit, appuyé)
    pokes: Vec<(usize, u32, u32, u8)>,    // (image, adresse, valeur, taille en octets)
}

impl Scenario {
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut actions = Vec::new();
        let mut pokes = Vec::new();
        for line in std::fs::read_to_string(path)?.lines() {
            let mut it = line.trim().splitn(2, ' ');
            let (Some(f), Some(rest)) = (it.next(), it.next()) else { continue };
            let (Ok(f), Some((name, v))) = (f.parse::<usize>(), rest.rsplit_once(' ')) else { continue };
            if let Some((kind, addr)) = name.split_once(' ') {
                let size = match kind { "poke8" => 1, "poke16" => 2, "poke32" => 4, _ => 0 };
                if size != 0 {
                    let val = match v.strip_prefix("0x") {
                        Some(h) => u32::from_str_radix(h, 16).ok(),
                        None => v.parse::<u32>().ok(),
                    };
                    let a = u32::from_str_radix(addr.trim_start_matches("0x"), 16).ok();
                    if let (Some(a), Some(val)) = (a, val) {
                        if (0xff0000..=0xffffff).contains(&a) {
                            pokes.push((f, a, val, size));
                        }
                    }
                    continue;
                }
            }
            let Ok(v) = v.parse::<u32>() else { continue };
            if let Some((port, bit)) = input_bit(name) {
                actions.push((f, port, bit, v != 0));
            }
        }
        Ok(Scenario { actions, pokes })
    }

    /// Applique les entrées dues avant d'exécuter l'image d'indice `f` (0 = première image).
    pub fn apply(&self, f: usize, m: &mut cps2::Machine) {
        for &(af, addr, val, size) in &self.pokes {
            if af + 1 == f {
                poke(m, addr, val, size);
            }
        }
        for &(af, port, bit, down) in &self.actions {
            if af + 1 == f {
                set_bit(m, port, bit, down);
            }
        }
    }
}

/// Écrit `valeur` (big-endian, `taille` octets) en RAM de travail (FF0000-FFFFFF).
pub fn poke(m: &mut cps2::Machine, addr: u32, val: u32, size: u8) {
    for k in 0..size as u32 {
        let a = (addr + k - 0xff0000) as usize;
        let byte = (val >> (8 * (size as u32 - 1 - k))) as u8;
        let w = &mut m.bus.ram[a / 2];
        *w = if a % 2 == 0 { *w & 0x00ff | (byte as u16) << 8 } else { *w & 0xff00 | byte as u16 };
    }
}

fn set_bit(m: &mut cps2::Machine, port: u8, bit: u16, down: bool) {
    let reg = match port {
        0 => &mut m.bus.inputs.in0,
        1 => &mut m.bus.inputs.in1,
        _ => &mut m.bus.inputs.in2,
    };
    if down { *reg &= !bit } else { *reg |= bit }
}

/// Appuie (`down`) ou relâche l'entrée MAME `name` (ex. « P1 Button 2 ») ; faux si le nom est inconnu.
pub fn set_input(m: &mut cps2::Machine, name: &str, down: bool) -> bool {
    match input_bit(name) {
        Some((port, bit)) => {
            set_bit(m, port, bit, down);
            true
        }
        None => false,
    }
}

/// Nom d'entrée MAME -> (port, bit) pour cps2_4p4b (actif à 0).
fn input_bit(name: &str) -> Option<(u8, u16)> {
    if let Some(rest) = name.strip_prefix('P') {
        let (p, what) = rest.split_once(' ')?;
        let shift = if p == "1" || p == "3" { 0 } else { 8 };
        let b = match what {
            "Right" => 0x01,
            "Left" => 0x02,
            "Down" => 0x04,
            "Up" => 0x08,
            "Button 1" => 0x10,
            "Button 2" => 0x20,
            "Button 3" => 0x40,
            "Button 4" => 0x80,
            _ => return None,
        };
        return Some((if p == "3" || p == "4" { 1 } else { 0 }, b << shift));
    }
    Some(match name {
        "Coin 1" => (2, 0x1000),
        "Coin 2" => (2, 0x2000),
        "Coin 3" => (2, 0x4000),
        "Coin 4" => (2, 0x8000),
        "1 Player Start" => (2, 0x0100),
        "2 Players Start" => (2, 0x0200),
        "3 Players Start" => (2, 0x0400),
        "4 Players Start" => (2, 0x0800),
        "Service 1" => (2, 0x0004),
        "Service Mode" => (2, 0x0002),
        _ => return None,
    })
}
