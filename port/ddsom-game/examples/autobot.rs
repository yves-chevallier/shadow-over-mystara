//! Enregistreur de bot : joue au jeu en boucle fermée (lit la RAM : joueurs, ennemis, boss, objets)
//! et écrit un scénario d'entrées pur (touches + écritures RAM `poke`), rejouable par `validate`.
//!
//!   autobot <roms> <eeprom> <base.txt> <images> <sortie.txt> [--players N] [--god PV]
//!           [--plan scène:durée,scène:durée,...] [--weaken N] [--seed S]
//!
//! * `base.txt` : entrées fixes (pièces, Start, choix des personnages), produit par
//!   `tools/make_bot.py --base-only`.
//! * Le bot rejoint l'ennemi le plus proche (ENEMY FFA99E, BOSS FFBA1E), s'aligne en profondeur, frappe,
//!   saute, lance une magie / un objet de temps en temps ; sans ennemi, il ramasse objets (ITEM) et caisses
//!   (BOX) puis avance à droite en tapant sur les boutons (menus, boutiques, choix de route).
//! * `--god PV` : remet les PV (+0x62 du joueur) à PV quand ils tombent sous la moitié.
//! * `--weaken N` : N images de jeu après l'entrée dans une scène, les PV des boss sont ramenés à 1
//!   (le bot les achève d'une touche : séquences de mort des boss, fin de scène naturelle).
//! * `--plan` : parcours de scènes. La première est écrite en FF8034 pendant la sélection du
//!   personnage ; ensuite, après `durée` images de jeu dans la scène (ou 400 images après une
//!   progression naturelle), le bot saute à la suivante (FF8034 ← scène, phase FF802C ← 8 : entrée de scène du jeu).
//! Il tourne comme `validate` (68000 surcadencé ×16, machine B) : le scénario est donc reproduit à l'identique.
use std::path::Path;

const OVERCLOCK: f64 = 16.0;
const P0: u32 = 0xff85de;

struct Bot {
    m: cps2::Machine,
    f: usize,
    held: std::collections::HashMap<String, bool>,
    out: Vec<(usize, String)>,
}

impl Bot {
    fn rd8(&self, a: u32) -> u32 {
        let o = (a - 0xff0000) as usize;
        let w = self.m.bus.ram[o / 2] as u32;
        if o % 2 == 0 { w >> 8 } else { w & 0xff }
    }
    fn rd16(&self, a: u32) -> u32 {
        self.rd8(a) << 8 | self.rd8(a + 1)
    }
    /// Entrée tenue ou relâchée à partir de l'image courante (effective pour l'image `f`).
    fn set(&mut self, name: String, down: bool) {
        let cur = self.held.get(&name).copied().unwrap_or(false);
        if cur != down {
            ddsom_game::scenario::set_input(&mut self.m, &name, down);
            self.out.push((self.f - 1, format!("{name} {}", down as u8)));
            self.held.insert(name, down);
        }
    }
    fn poke(&mut self, addr: u32, val: u32, size: u8) {
        ddsom_game::scenario::poke(&mut self.m, addr, val, size);
        self.out.push((self.f - 1, format!("poke{} {addr:x} {val}", 8 * size as u32)));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let base_path = &a[3];
    let frames: usize = a[4].parse().unwrap();
    let out_path = &a[5];
    let (mut players, mut god, mut plan, mut seed, mut weaken) = (1usize, 0u32, Vec::<(u32, usize)>::new(), 1u64, 0usize);
    let mut i = 6;
    while i < a.len() {
        match a[i].as_str() {
            "--players" => players = a[i + 1].parse().unwrap(),
            "--god" => god = u32::from_str_radix(a[i + 1].trim_start_matches("0x"), 16).unwrap(),
            "--weaken" => weaken = a[i + 1].parse().unwrap(),
            "--seed" => seed = a[i + 1].parse().unwrap(),
            "--plan" => {
                plan = a[i + 1]
                    .split(',')
                    .map(|t| {
                        let (s, d) = t.split_once(':').unwrap();
                        (s.parse().unwrap(), d.parse().unwrap())
                    })
                    .collect()
            }
            _ => panic!("option inconnue {}", a[i]),
        }
        i += 2;
    }
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(OVERCLOCK);
    ddsom_game::install_reference(&mut m);
    m.idle = Some(ddsom_game::idle);
    let base = ddsom_game::scenario::Scenario::load(Path::new(base_path)).unwrap();
    let mut b = Bot { m, f: 0, held: Default::default(), out: Vec::new() };
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut rnd = move |n: u32| {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 33) % n as u64) as u32
    };
    let (mut k, mut since, mut trans): (usize, usize, usize) = (0, 0, 0);
    let mut last_scene = 0;
    let mut visited = std::collections::BTreeSet::new();
    for f in 0..frames {
        b.f = f;
        base.apply(f, &mut b.m);
        let (scene, mut phase) = (b.rd8(0xff8034), b.rd8(0xff802c));
        if scene != last_scene {
            eprintln!("image {f} : scène {scene:#x}, phase {phase:#x}");
            last_scene = scene;
            since = 0;
            visited.insert(scene);
        }
        if f == 2400 && !plan.is_empty() {
            b.poke(0xff8034, plan[0].0, 1);
        }
        let active = b.rd8(0xff8030) != 0;
        let select_end = phase == 0x04 && b.rd8(0xff802d) > 0x0c;
        if f > 2600 && select_end {
            // fin de la sélection (compte à rebours) : aucune touche
            for pl in 0..players {
                apply_keys(&mut b, pl, [false; 8]);
            }
        } else if f > 2600 && (!active || phase == 0x04) && phase != 0x08 {
            // hors partie (nom du classement, écrans) ou sélection du personnage (après un continue / une
            // nouvelle partie) : tapes sur Button 1 et déplacements du curseur
            for pl in 0..players {
                let g = f + 17 * pl;
                let keys = [
                    g % 150 >= 20 && g % 150 < 24 && (g / 150 + pl) % 2 == 0,
                    g % 150 >= 20 && g % 150 < 24 && (g / 150 + pl) % 2 == 1,
                    g % 230 >= 30 && g % 230 < 34,
                    g % 230 >= 60 && g % 230 < 64,
                    g % 50 < 4,
                    false, false, false,
                ];
                apply_keys(&mut b, pl, keys);
            }
        } else if f > 2600 && active && b.rd8(0xff8031) != 0 {
            // --- parcours de scènes ; `since` = images écoulées dans la scène courante, hors transitions (phases 8 et 0x10)
            if phase != 0x08 && phase != 0x10 {
                since += 1;
            }
            if weaken != 0 && since >= weaken && since % 30 == 0 {
                // affaiblit les boss : la prochaine touche les achève
                for i in 0..3 {
                    let o = 0xffba1e + 0xc0 * i;
                    if b.rd8(o) != 0 && b.rd16(o + 0x62) > 1 {
                        b.poke(o + 0x62, 1, 2);
                    }
                }
            }
            // séquences de transition (phases 8 et 0x10) trop longues : on passe à la scène suivante
            trans = if phase == 0x08 || phase == 0x10 { trans + 1 } else { 0 };
            if !plan.is_empty() && k + 1 < plan.len() && (trans > 1500 || (phase != 0x08 && phase != 0x10)) {
                // un boss vivant prolonge le séjour (jusqu'à 5 fois la durée prévue)
                let boss = (0..3).any(|i| b.rd8(0xffba1e + 0xc0 * i) != 0);
                let limit = if boss { 5 * plan[k].1 } else { plan[k].1 };
                let timeout = trans > 1500 || if scene == plan[k].0 { since >= limit } else { since >= 400 };
                if timeout {
                    k += 1;
                    since = 0;
                    trans = 0;
                    eprintln!("image {f} : saut vers la scène {}", plan[k].0);
                    b.poke(0xff8034, plan[k].0, 1);
                    b.poke(0xff802c, 0x08, 1);
                    b.poke(0xff802d, 0, 1);
                    b.poke(0xff802e, 0, 1);
                    phase = 0x08;
                }
            }
            for pl in 0..players {
                if phase == 0x08 {
                    // entrée de scène : aucune touche (une entrée pendant cette phase fait diverger
                    // les deux machines de validate, voir docs/couverture.md)
                    apply_keys(&mut b, pl, [false; 8]);
                } else {
                    policy(&mut b, pl, &mut rnd, god);
                }
            }
        }
        b.m.run_frame();
    }
    // scénario = base + enregistrement (tri stable par image)
    let mut lines: Vec<(usize, String)> = std::fs::read_to_string(base_path)
        .unwrap()
        .lines()
        .filter_map(|l| Some((l.split(' ').next()?.parse().ok()?, l.to_string())))
        .collect();
    lines.extend(b.out.iter().map(|(f, s)| (*f, format!("{f} {s}"))));
    lines.sort_by_key(|l| l.0);
    let mut text: String = lines.iter().map(|l| l.1.clone() + "\n").collect();
    text += &format!("{} quit\n", frames + 1);
    std::fs::write(out_path, text).unwrap();
    eprintln!("scènes visitées : {:x?}", visited);
}

fn policy(b: &mut Bot, pl: usize, rnd: &mut impl FnMut(u32) -> u32, god: u32) {
    let base = P0 + 0x200 * pl as u32;
    if b.rd8(base) == 0 {
        apply_keys(b, pl, [false; 8]);
        return;
    }
    let f = b.f + 7 * pl;
    let hp = b.rd16(base + 0x62);
    if god != 0 && hp < god - 8 && hp > 0 && b.rd8(base + 4) < 4 {
        b.poke(base + 0x62, god, 2);
    }
    let (px, py) = (b.rd16(base + 8) as i32, b.rd16(base + 0x10) as i32);
    // cible : ennemi / boss vivants, sinon objets et caisses
    let mut best: Option<(i32, i32, i32, bool)> = None; // (distance, x, y, ennemi)
    let scan = |b: &Bot, start: u32, n: u32, size: u32, hostile: bool, best: &mut Option<(i32, i32, i32, bool)>| {
        for i in 0..n {
            let o = start + i * size;
            if b.rd8(o) == 0 || (hostile && b.rd16(o + 0x62) == 0) {
                continue;
            }
            let (x, y) = (b.rd16(o + 8) as i32, b.rd16(o + 0x10) as i32);
            let d = (x - px).abs() + 2 * (y - py).abs();
            if best.is_none_or(|t| (hostile, -d) > (t.3, -t.0)) {
                *best = Some((d, x, y, hostile));
            }
        }
    };
    scan(b, 0xffba1e, 3, 0xc0, true, &mut best);
    scan(b, 0xffa99e, 22, 0xc0, true, &mut best);
    if best.is_none() {
        scan(b, 0xffe2de, 32, 0x60, false, &mut best);
        scan(b, 0xffeede, 21, 0x80, false, &mut best);
    }
    if pl == 0 && b.f % 200 == 0 && std::env::var("BOTDEBUG").is_ok() {
        eprintln!("f {} pos {px},{py} hp {hp} cible {best:?}", b.f);
    }
    let (mut right, mut left, mut up, mut down, mut b1, mut b2, mut b3, mut b4) = (false, false, false, false, false, false, false, false);
    match best {
        Some((_, tx, ty, hostile)) => {
            let (dx, dy) = (tx - px, ty - py);
            let range = if hostile { 40 } else { 6 };
            if dx > range { right = true } else if dx < -range { left = true }
            if dy.abs() > 6 { if dy > 0 { up = true } else { down = true } }
            if hostile && dx.abs() <= range + 16 && dy.abs() <= 12 {
                b1 = (f / 4) % 2 == 0;
                // se tourne vers la cible un instant sur 16 images
                if f % 16 < 2 { if dx > 0 { right = true } else { left = true } }
                if f % 500 == 120 { b2 = true } // saut
                if f % 331 < 4 { b3 = true } // magie / objet
            }
        }
        None => {
            right = true;
            if f % 90 < 4 { b1 = true }
            if f % 150 < 3 { if (f / 150) % 2 == 0 { up = true } else { down = true } }
            if f % 700 < 4 { b2 = true }
        }
    }
    // objets : choisir (Button 4) puis utiliser (Button 3)
    if f % 1100 < 4 { b4 = true }
    if f % 1100 >= 20 && f % 1100 < 24 { b3 = true }
    // variété : quelques actions aléatoires
    if rnd(400) == 0 { b2 = true }
    apply_keys(b, pl, [right, left, up, down, b1, b2, b3, b4]);
}

fn apply_keys(b: &mut Bot, pl: usize, keys: [bool; 8]) {
    let names = ["Right", "Left", "Up", "Down", "Button 1", "Button 2", "Button 3", "Button 4"];
    for (n, v) in names.iter().zip(keys) {
        b.set(format!("P{} {n}", pl + 1), v);
    }
}
