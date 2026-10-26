//! Tables lues par les routines natives (`ddsom-game`) que les autres fichiers ne décrivent pas : `data/natives.json`.
//!
//! Les zones viennent d'une mesure (`zones_natives.txt`, régénéré par `tools/zones_natives.py` depuis la sortie de
//! `lectures_rom` en mode sans 68000 sur tous les scénarios ; docs/sans-rom.md) : chaque ligne donne l'espace
//! (`d` données, `p` programme déchiffré), le début, la fin exclue et le fichier Rust qui lit la zone. Ce sont de
//! petites tables disséminées (vitesses, décisions, positions, chaînes, adresses de routines lues par les tables
//! d'aiguillage, vecteurs d'interruption...) dont le format n'est pas encore décrit : elles sont rangées en mots
//! (ou en octets si la zone est impaire), sous le nom `<fichier>/<adresse>`. Une zone qu'une table d'un autre
//! fichier décrit déjà n'y est pas.

use crate::data::{data, prog, Spec};

const ZONES: &str = include_str!("../zones_natives.txt");

pub fn specs() -> Vec<Spec> {
    let mut s = Vec::new();
    for l in ZONES.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let t: Vec<&str> = l.split_whitespace().collect();
        let [espace, debut, fin, lecteur] = t[..] else { panic!("zones_natives.txt : ligne illisible « {l} »") };
        let (debut, fin) = (u32::from_str_radix(debut, 16).unwrap(), u32::from_str_radix(fin, 16).unwrap());
        let name = format!("{lecteur}/{debut:#x}");
        let (format, rows) = if debut % 2 == 0 && fin % 2 == 0 { ("x16", (fin - debut) / 2) } else { ("u8", fin - debut) };
        s.push(match espace {
            "d" => data(&name, debut, format, &[], rows as usize),
            "p" => prog(&name, debut, format, &[], rows as usize),
            _ => panic!("zones_natives.txt : espace « {espace} »"),
        });
    }
    s
}
