//! Scénarios d'entrées (format de mame/scenario.lua : « image champ valeur »).
//! Comme dans MAME, une entrée posée après l'image n prend effet à l'image n + 2.

use std::io;
use std::path::Path;

pub struct Scenario {
    actions: Vec<(usize, u8, u16, bool)>, // (image, port, bit, appuyé)
}

impl Scenario {
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut actions = Vec::new();
        for line in std::fs::read_to_string(path)?.lines() {
            let mut it = line.trim().splitn(2, ' ');
            let (Some(f), Some(rest)) = (it.next(), it.next()) else { continue };
            let (Ok(f), Some((name, v))) = (f.parse::<usize>(), rest.rsplit_once(' ')) else { continue };
            let Ok(v) = v.parse::<u32>() else { continue };
            if let Some((port, bit)) = input_bit(name) {
                actions.push((f, port, bit, v != 0));
            }
        }
        Ok(Scenario { actions })
    }

    /// Applique les entrées dues avant d'exécuter l'image d'indice `f` (0 = première image).
    pub fn apply(&self, f: usize, m: &mut cps2::Machine) {
        for &(af, port, bit, down) in &self.actions {
            if af + 1 == f {
                let reg = match port {
                    0 => &mut m.bus.inputs.in0,
                    1 => &mut m.bus.inputs.in1,
                    _ => &mut m.bus.inputs.in2,
                };
                if down { *reg &= !bit } else { *reg |= bit }
            }
        }
    }
}

/// Nom d'entrée MAME -> (port, bit) pour cps2_4p4b (actif à 0).
fn input_bit(name: &str) -> Option<(u8, u16)> {
    if let Some(rest) = name.strip_prefix('P') {
        let (p, what) = rest.split_once(' ')?;
        let shift = if p == "1" { 0 } else { 8 };
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
        return Some((0, b << shift));
    }
    Some(match name {
        "Coin 1" => (2, 0x1000),
        "Coin 2" => (2, 0x2000),
        "1 Player Start" => (2, 0x0100),
        "2 Players Start" => (2, 0x0200),
        "Service 1" => (2, 0x0004),
        _ => return None,
    })
}
