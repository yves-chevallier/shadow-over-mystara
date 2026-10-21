//! Données diverses : `data/misc.json` — octets et noms des scripts du visualiseur de sprites (menu de test
//! caché), entrées enregistrées des démonstrations de l'attract, générique de fin.

use ddsom_assets::data::{Region, TextBlock};

use crate::data::{data, region, text, Spec};
use crate::rom::Program;

/// Entrées des démonstrations : 30 suites d'enregistrements de 14 octets (durée en images, puis 3 octets
/// d'entrées par joueur, comparés et recopiés en `+0x64a..+0x64c` des quatre joueurs ; enregistreur 0xe0698).
/// Chaque suite commence par un en-tête (durée 0, entrées 0xff) et finit par une durée 0xffff ; entre les
/// suites, des mots de bourrage (`0x4e71`, une fois 0) et un enregistrement de remplissage (0xff).
const DEMO: (u32, u32) = (0x2b04de, 0x2b45d8);
const DEMO_FORMAT: &str = "u16 x8*12";
const DEMO_FIELDS: &[&str] =
    &["frames", "p1a", "p1b", "p1c", "p2a", "p2b", "p2c", "p3a", "p3b", "p3c", "p4a", "p4b", "p4c"];

fn demo(rom: &Program, s: &mut Vec<Spec>) {
    let bytes = |a: u32, n: u32| (a..a + n).map(|x| rom.rd8(x)).collect::<Vec<u8>>();
    let header: Vec<u8> = [0, 0].into_iter().chain([0xff; 12]).collect();
    let (mut p, mut k) = (DEMO.0, 0);
    while p < DEMO.1 {
        if bytes(p, 14) == header {
            let start = p;
            p += 14;
            while rom.rd16(p) != 0xffff {
                p += 14;
            }
            p += 14;
            s.push(data(&format!("demo/{k}"), start, DEMO_FORMAT, DEMO_FIELDS, ((p - start) / 14) as usize));
            k += 1;
        } else if bytes(p, 14) == [0xff; 14] {
            s.push(data(&format!("filler/{p:#x}"), p, DEMO_FORMAT, DEMO_FIELDS, 1));
            p += 14;
        } else {
            assert!(matches!(rom.rd16(p), 0x4e71 | 0), "démonstrations : {p:#x} inattendu");
            s.push(data(&format!("pad/{p:#x}"), p, "x16", &[], 1));
            p += 2;
        }
    }
    assert_eq!(p, DEMO.1);
}

pub fn specs(rom: &Program) -> (Vec<Region>, Vec<Spec>, Vec<TextBlock>) {
    let mut s = vec![data("viewer_counts", 0x2a85fc, "u8", &[], 2512)];
    demo(rom, &mut s);
    let texts = vec![
        text(rom, "viewer_names", 0x2a8fcc, 0x2ae81e, 0xff),
        text(rom, "credits", 0xd01de, 0xd06e5, b'/'),
    ];
    let regions = vec![region("viewer", 0x2a85fc, 0x2ae81e), region("demo", DEMO.0, DEMO.1)];
    (regions, s, texts)
}
