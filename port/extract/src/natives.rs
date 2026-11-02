//! Tables lues par les routines natives (`ddsom-game`) que les autres fichiers ne décrivent pas : `data/natives.json`.
//!
//! Les zones viennent d'une mesure (`zones_natives.txt`, régénéré par `tools/zones_natives.py` depuis la sortie de
//! `lectures_rom` en mode sans 68000 sur tous les scénarios ; docs/sans-rom.md) : chaque ligne donne l'espace
//! (`d` données, `p` programme déchiffré), le début, la fin exclue et le fichier Rust qui lit la zone. Ce sont de
//! petites tables disséminées (vitesses, décisions, positions, chaînes, adresses de routines lues par les tables
//! d'aiguillage, vecteurs d'interruption...) dont le format n'est pas encore décrit : elles sont rangées en mots
//! (ou en octets si la zone est impaire), sous le nom `<file>/<address>` (`prog/<file>/<address>` en espace
//! programme). Une zone qu'une table d'un autre
//! fichier décrit déjà n'y est pas.

use crate::data::{data, prog, Spec};

const ZONES: &str = include_str!("../zones_natives.txt");

pub fn specs() -> Vec<Spec> {
    let mut s = Vec::new();
    for l in ZONES.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let t: Vec<&str> = l.split_whitespace().collect();
        let [space, start, end, reader] = t[..] else { panic!("zones_natives.txt : ligne illisible « {l} »") };
        let (start, end) = (u32::from_str_radix(start, 16).unwrap(), u32::from_str_radix(end, 16).unwrap());
        // l'espace programme a ses propres noms : une même adresse peut être lue dans les deux espaces
        let name = if space == "p" { format!("prog/{reader}/{start:#x}") } else { format!("{reader}/{start:#x}") };
        let (format, rows) = if start % 2 == 0 && end % 2 == 0 { ("x16", (end - start) / 2) } else { ("u8", end - start) };
        s.push(match space {
            "d" => data(&name, start, format, &[], rows as usize),
            "p" => prog(&name, start, format, &[], rows as usize),
            _ => panic!("zones_natives.txt : espace « {space} »"),
        });
    }
    s
}
