//! Zones de la ROM programme lues par le code Rust (sortie de `lectures_rom` de ddsom-game) que les
//! ressources ne reconstruisent pas.
//!
//!   couverture_rom <assets> <lectures.txt> [écart]
//!   couverture_rom <assets> --octets <fichier> [--sans <fichier d'assets>]
//!
//! La seconde forme écrit la couverture mot par mot (un octet par mot, données puis programme, 1 = couvert)
//! pour `DDSOM_COUVERT` de `lectures_rom` ; `--sans data/natives.json` l'écrit sans ce fichier (pour
//! `tools/zones_natives.py`, qui le régénère).
//!
//! Affiche, par espace (données `d`, programme `p`), le total lu, la part couverte, puis les zones lues non
//! couvertes (regroupées si moins de `écart` octets les séparent, 0x10 par défaut) : début, fin exclue,
//! octets lus non couverts, lectures, masque des scénarios.

use std::path::Path;

use ddsom_assets::rebuild::Program;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if (a.len() == 4 || a.len() == 6 && a[4] == "--sans") && a[2] == "--octets" {
        let sans = a.get(5).cloned().unwrap_or_default();
        let p = Program::load_files(Path::new(&a[1]), &|f| f != sans).expect("ressources");
        let mut out = Vec::new();
        for img in [&p.data, &p.prog] {
            out.extend((0..0x200000u32).map(|w| (img.is_covered(2 * w) && img.is_covered(2 * w + 1)) as u8));
        }
        std::fs::write(&a[3], out).unwrap();
        return;
    }
    let [_, assets, lectures, ..] = &a[..] else {
        eprintln!("usage : couverture_rom <assets> <lectures.txt> [écart]");
        std::process::exit(2);
    };
    let gap: u32 = a.get(3).map_or(0x10, |g| u32::from_str_radix(g, 16).unwrap());
    let p = Program::load(Path::new(assets)).expect("ressources");
    // octets lus couverts par chaque fichier : (fichier, données, programme)
    let reads: Vec<(char, u32, u32)> = std::fs::read_to_string(lectures)
        .unwrap()
        .lines()
        .filter(|l| l.starts_with(['d', 'p']) && l.as_bytes()[1] == b' ')
        .map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            (f[0].chars().next().unwrap(), u32::from_str_radix(f[1], 16).unwrap(), u32::from_str_radix(f[2], 16).unwrap())
        })
        .collect();
    println!("# octets lus et couverts, par fichier de ressources (données, programme)");
    for rel in Program::FILES {
        let q = Program::load_files(Path::new(assets), &|f| f == rel).expect("ressources");
        let count = |view: char, img: &ddsom_assets::rebuild::Image| -> u64 {
            reads.iter().filter(|r| r.0 == view).map(|&(_, s, e)| (s..e).filter(|&a| img.is_covered(a)).count() as u64).sum()
        };
        println!("{rel} {} {}", count('d', &q.data), count('p', &q.prog));
    }
    for (view, name, img) in [('d', "données", &p.data), ('p', "programme", &p.prog)] {
        let (mut total, mut covered) = (0u64, 0u64);
        // zones non couvertes : (début, fin, lectures, masque)
        let mut miss: Vec<(u32, u32, u64, u32)> = Vec::new();
        for l in std::fs::read_to_string(lectures).unwrap().lines() {
            let f: Vec<&str> = l.split(' ').collect();
            if f[0].chars().next() != Some(view) {
                continue;
            }
            let (s, e) = (u32::from_str_radix(f[1], 16).unwrap(), u32::from_str_radix(f[2], 16).unwrap());
            let (mask, reads) = (u32::from_str_radix(f[3], 16).unwrap(), f[4].parse::<u64>().unwrap());
            total += (e - s) as u64;
            for w in (s..e).step_by(2) {
                if img.is_covered(w) && img.is_covered(w + 1) {
                    covered += 2;
                } else {
                    match miss.last_mut() {
                        Some(m) if w - m.1 <= gap => {
                            m.1 = w + 2;
                            m.2 += reads * 2 / (e - s) as u64;
                            m.3 |= mask;
                        }
                        _ => miss.push((w, w + 2, reads * 2 / (e - s) as u64, mask)),
                    }
                }
            }
        }
        println!("# espace {name} : {total} octets lus, {covered} couverts, {} zones non couvertes", miss.len());
        for (s, e, reads, mask) in &miss {
            println!("{view} {s:06x} {e:06x} {} {reads} {mask:x}", e - s);
        }
    }
}
