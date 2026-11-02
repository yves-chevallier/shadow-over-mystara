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
//!   `scène:durée:f` force à la place la sortie de la scène (phase FF802C ← 0x10, comme `tools/gen_scene_end.py`) ;
//!   `scène:durée:t` la force à l'état de fondu de la séquence 18, qui joue le script de transition de la scène (comme
//!   les `trans_*`) ; le bot attend alors la scène suivante choisie par le jeu (votes de chemin), puis 400 images.
//! * `--spawn famille:type:variante:image` : à l'image indiquée, crée à la main un objet `enemy` ou `boss` du type
//!   et de la variante donnés, à 0x60 pixels à droite du joueur 1 (comme `creatures spawn`), pour les types que les
//!   parcours ne font jamais apparaître ; les écritures RAM font partie du scénario produit.
//! * `--door image:x:y:touche[:durée]` : à l'image donnée, tous les joueurs sont posés au sol en (x, y) et tiennent la
//!   touche (bit de `+0x6c` : 0 droite, 1 gauche, 2 bas, 3 haut) `durée` images (60 par défaut), puis le bot reprend : pour
//!   traverser les portes de passage (SET 27 et 29) et de sortie de zone (SET 7), voir `tools/gen_long.py`.
//! * `--wander` : dans les boutiques (scènes 0x45 à 0x52) et pendant un vote de chemin (EFFCT 23), chaque joueur tient
//!   une direction tirée au hasard pendant 20 à 90 images et tape parfois un bouton (curseurs de boutique : articles,
//!   dons d'or, sortie ; curseurs de vote : choix variés, ex æquo) ; à chaque vote, un joueur sur quatre s'abstient (fin
//!   du compte à rebours). Voir `tools/gen_long.py` (`long_votes_*`, `long_boutiques_*`).
//! * `--frail N` : toutes les N images, les joueurs 3 et 4 en vie (état < 4) sont ramenés à 1 PV : ils tombent au
//!   prochain coup (reprises pendant les combats de boss, interface F3 : effacement du cadre quand un boss est là).
//! * `--patience N` : un objet ou une caisse poursuivi plus de N images est ignorée 2 000 images
//!   (sinon le bot reste bloqué devant un objet hors d'atteinte et la scène n'avance plus).
//! * `--cast N`, `--charges V` : magies et objets fréquents (voir `Magic`).
//! * `--offset N` : la partie commence N images plus tard (attract d'abord) : la scène du plan est écrite à
//!   l'image 2400 + N et le bot ne joue qu'après 2600 + N (base produite avec `bot_lib.build(offset=N)`).
//! Il tourne comme `validate` (68000 surcadencé ×16, machine B) : le scénario est donc reproduit à l'identique.
use std::path::Path;

const OVERCLOCK: f64 = 16.0;
const P0: u32 = 0xff85de;

struct Bot {
    m: cps2::Machine,
    f: usize,
    held: std::collections::HashMap<String, bool>,
    out: Vec<(usize, String)>,
    /// `--patience N` : objet poursuivi par chaque joueur et depuis combien d'images ; objets ignorés jusqu'à une image
    patience: usize,
    chase: [(u32, usize); 4],
    ignore: std::collections::HashMap<u32, usize>,
    /// `--spells a,b,..` `--spell-period N` : identifiants posés tour à tour dans la 1re ligne d'équipement, lancés devant un ennemi
    spells: Vec<u32>,
    spell_period: usize,
    spell_next: usize,
    /// image du dernier identifiant posé par joueur (une pose par période, quand le joueur est au repos)
    spell_at: [usize; 4],
    rod_at: [usize; 4],
    /// `--rods N` : toutes les N images, le magicien change de baguette (`WEAPON_B` 0x24..0x27, réserve pleine ou vide)
    rods: usize,
    /// `--door` : (image, x, y, touche, durée)
    doors: Vec<(usize, u32, u32, u32, usize)>,
    /// `--wander` : (début et fin du geste, touches, abstention au vote en cours) par joueur
    wander: bool,
    wander_keys: [(usize, usize, [bool; 8], bool); 4],
    voting: bool,
    frail: usize,
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
    let (mut players, mut god, mut plan, mut seed, mut weaken) = (1usize, 0u32, Vec::<(u32, usize, u8)>::new(), 1u64, 0usize);
    let mut offset = 0usize;
    let mut magic = Magic::default();
    let mut patience = 0usize;
    let mut spawn: Option<(bool, u32, u32, usize)> = None;
    let (mut spells, mut spell_period, mut rods) = (Vec::<u32>::new(), 0usize, 0usize);
    let mut doors = Vec::<(usize, u32, u32, u32, usize)>::new();
    let mut wander = false;
    let mut frail = 0usize;
    let mut i = 6;
    while i < a.len() {
        match a[i].as_str() {
            "--players" => players = a[i + 1].parse().unwrap(),
            "--god" => god = u32::from_str_radix(a[i + 1].trim_start_matches("0x"), 16).unwrap(),
            "--weaken" => weaken = a[i + 1].parse().unwrap(),
            "--seed" => seed = a[i + 1].parse().unwrap(),
            "--offset" => offset = a[i + 1].parse().unwrap(),
            "--patience" => patience = a[i + 1].parse().unwrap(),
            "--cast" => magic.period = a[i + 1].parse().unwrap(),
            "--charges" => magic.charges = a[i + 1].parse().unwrap(),
            "--spells" => {
                spells = a[i + 1].split(',').map(|t| u32::from_str_radix(t.trim_start_matches("0x"), 16).unwrap()).collect()
            }
            "--spell-period" => spell_period = a[i + 1].parse().unwrap(),
            "--door" => {
                let t: Vec<usize> = a[i + 1].split(':').map(|v| usize::from_str_radix(v.trim_start_matches("0x"), if v.starts_with("0x") { 16 } else { 10 }).unwrap()).collect();
                doors.push((t[0], t[1] as u32, t[2] as u32, t[3] as u32, t.get(4).copied().unwrap_or(60)));
            }
            "--rods" => rods = a[i + 1].parse().unwrap(),
            "--frail" => frail = a[i + 1].parse().unwrap(),
            "--wander" => {
                wander = true;
                i -= 1;
            }
            "--spawn" => {
                let t: Vec<&str> = a[i + 1].split(':').collect();
                spawn = Some((t[0] == "boss", t[1].parse().unwrap(), t[2].parse().unwrap(), t[3].parse().unwrap()));
            }
            "--plan" => {
                plan = a[i + 1]
                    .split(',')
                    .map(|t| {
                        let v: Vec<&str> = t.split(':').collect();
                        (v[0].parse().unwrap(), v[1].parse().unwrap(), v.get(2).map_or(0, |m| m.as_bytes()[0]))
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
    let mut b = Bot { m, f: 0, held: Default::default(), out: Vec::new(), patience, chase: [(0, 0); 4], ignore: Default::default(), spells, spell_period, spell_next: 0, spell_at: [usize::MAX; 4], rod_at: [usize::MAX; 4], rods, doors, wander, wander_keys: [(0, 0, [false; 8], false); 4], voting: false, frail };
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut rnd = move |n: u32| {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 33) % n as u64) as u32
    };
    let (mut k, mut since, mut trans): (usize, usize, usize) = (0, 0, 0);
    let mut forced = false;
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
        if f == 2400 + offset && !plan.is_empty() {
            b.poke(0xff8034, plan[0].0, 1);
        }
        if let Some((boss, kind, variant, at)) = spawn {
            if f == at {
                let (pool, n) = if boss { (0xffba1eu32, 3) } else { (0xffa99e, 22) };
                if let Some(slot) = (0..n).map(|i| pool + 0xc0 * i).find(|&o| b.rd8(o) == 0) {
                    let (px, py) = (b.rd16(P0 + 8), b.rd16(P0 + 0x10));
                    b.poke(slot + 0x28, if boss { 4 } else { 2 }, 1);
                    b.poke(slot + 2, kind, 1);
                    b.poke(slot + 3, variant, 1);
                    b.poke(slot + 8, (px + 0x60) & 0xffff, 2);
                    b.poke(slot + 0x10, py, 2);
                    if !boss {
                        let free = b.rd16(0xff8000 + 0x7e62);
                        b.poke(0xff8000 + 0x7e62, free.wrapping_sub(1) & 0xffff, 2);
                    }
                    b.poke(slot, 1, 1);
                }
            }
        }
        let active = b.rd8(0xff8030) != 0;
        let select_end = phase == 0x04 && b.rd8(0xff802d) > 0x0c;
        if f > 2600 + offset && select_end {
            // fin de la sélection (compte à rebours) : aucune touche
            for pl in 0..players {
                apply_keys(&mut b, pl, [false; 8]);
            }
        } else if f > 2600 + offset && (!active || phase == 0x04) && phase != 0x08 {
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
        } else if f > 2600 + offset && active && b.rd8(0xff8031) != 0 {
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
            // (un vote de chemin en cours, avec --wander, ne compte pas : il dure jusqu'à 20 s après ses dialogues)
            trans = if (phase == 0x08 || phase == 0x10) && !(b.wander && vote_open(&b)) { trans + 1 } else { 0 };
            // (sortie forcée : dialogues et vote peuvent durer bien plus)
            let trans_max = if forced { 6000 } else { 1500 };
            if !plan.is_empty() && (trans > trans_max || (phase != 0x08 && phase != 0x10)) {
                // un boss vivant prolonge le séjour (jusqu'à 5 fois la durée prévue)
                let boss = (0..3).any(|i| b.rd8(0xffba1e + 0xc0 * i) != 0);
                let limit = if boss { 5 * plan[k].1 } else { plan[k].1 };
                let timeout = trans > trans_max || if scene == plan[k].0 && !forced { since >= limit } else { since >= 400 };
                if plan[k].2 != 0 && !forced && scene == plan[k].0 && since >= limit {
                    // sortie de scène forcée (gen_scene_end.py) : séquence de la scène, ou script de transition
                    forced = true;
                    eprintln!("image {f} : sortie forcée de la scène {}", plan[k].0);
                    b.poke(0xff802c, 0x10, 1);
                    b.poke(0xff802d, 0, 1);
                    b.poke(0xff802e, 0, 2);
                    if plan[k].2 == b't' {
                        b.poke(0xff2a36, 0x0224_1400, 4);
                        for r in 0..4 {
                            b.poke(0xff2ae0 + 4 * r, 0, 4);
                        }
                    }
                    phase = 0x10;
                } else if timeout && k + 1 < plan.len() {
                    k += 1;
                    since = 0;
                    trans = 0;
                    forced = false;
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
                } else if let Some(&(at, x, y, key, dur)) = b.doors.iter().find(|d| f >= d.0 && f < d.0 + d.4) {
                    // porte : les joueurs vivants sont posés devant et tiennent la touche
                    let base = P0 + 0x200 * pl as u32;
                    if f == at && b.rd8(base) != 0 {
                        b.poke(base + 8, x, 2);
                        b.poke(base + 0x10, y, 2);
                        b.poke(base + 0xc, 0, 2);
                    }
                    let _ = dur;
                    let mut keys = [false; 8];
                    keys[[0, 1, 3, 2][key as usize]] = f > at + 2;
                    apply_keys(&mut b, pl, keys);
                } else if b.wander && (matches!(scene, 0x45..=0x52) || vote_open(&b)) {
                    wander_step(&mut b, pl, &mut rnd);
                } else {
                    policy(&mut b, pl, &mut rnd, god, magic);
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

/// Magies et objets : `--cast N` choisit la ligne suivante (Button 4) puis l'utilise (Button 3) toutes les N images
/// devant un ennemi ; `--charges V` remet à V les charges (réserve `+0x150`) des lignes d'équipement occupées.
#[derive(Clone, Copy, Default)]
struct Magic {
    period: usize,
    charges: u32,
}

fn policy(b: &mut Bot, pl: usize, rnd: &mut impl FnMut(u32) -> u32, god: u32, magic: Magic) {
    let base = P0 + 0x200 * pl as u32;
    if b.rd8(base) == 0 {
        apply_keys(b, pl, [false; 8]);
        return;
    }
    let f = b.f + 7 * pl;
    if magic.charges != 0 && f % 50 == 0 {
        // trois lignes de 16 octets à +0x120 : identifiant (0x56 : vide), octet +8 = indice de la réserve
        for line in 0..3 {
            let row = base + 0x120 + 16 * line;
            let slot = base + 0x150 + b.rd8(row + 8);
            if b.rd8(row) != 0x56 && b.rd8(row + 8) < 9 && b.rd8(slot) < magic.charges {
                b.poke(slot, magic.charges, 1);
            }
        }
    }
    let quiet = b.rd16(base + 4) == 0x200 && !matches!(b.rd8(base + 6), 0xc | 0xe | 0x14 | 0x22) && b.rd8(base + 0x15c) == 0;
    if b.spell_period != 0 && !b.spells.is_empty() && f % b.spell_period < 100 && quiet
        && b.spell_at[pl] / b.spell_period != f / b.spell_period
    {
        b.spell_at[pl] = f;
        // pose l'identifiant suivant en tête de la 1re ligne, chargé, et la choisit (`0x15d` = 0)
        let id = b.spells[b.spell_next % b.spells.len()];
        b.spell_next += 1;
        b.poke(base + 0x120, id, 1);
        b.poke(base + 0x128, 0, 1);
        b.poke(base + 0x150, 5, 1);
        b.poke(base + 0x15d, 0, 1);
    }
    if b.rods != 0 && f % b.rods < 100 && b.rd8(base + 3) >> 1 == 4 && quiet && b.rod_at[pl] / b.rods != f / b.rods {
        // une baguette (0x24 à 0x27), à tour de rôle pleine (1), à six charges (la dernière avertit l'interface) et épuisée
        b.rod_at[pl] = f;
        let n = (f / b.rods) as u32;
        b.poke(base + 0x10e, 0x24 + n % 4, 1);
        for flag in 0x18b..0x192 {
            b.poke(base + flag, [1, 6, 0][(n as usize / 4) % 3], 1);
        }
    }
    if b.frail != 0 && pl >= 2 && b.f % b.frail == 0 && b.rd8(base + 4) < 4 && b.rd16(base + 0x62) > 1 {
        b.poke(base + 0x62, 1, 2);
    }
    let hp = b.rd16(base + 0x62);
    if god != 0 && hp < god - 8 && hp > 0 && b.rd8(base + 4) < 4 {
        b.poke(base + 0x62, god, 2);
    }
    let (px, py) = (b.rd16(base + 8) as i32, b.rd16(base + 0x10) as i32);
    // cible : ennemi / boss vivants, sinon objets et caisses
    let mut best: Option<(i32, i32, i32, bool, u32)> = None; // (distance, x, y, ennemi, objet)
    let now = b.f;
    let scan = |b: &Bot, start: u32, n: u32, size: u32, hostile: bool, best: &mut Option<(i32, i32, i32, bool, u32)>| {
        for i in 0..n {
            let o = start + i * size;
            if b.rd8(o) == 0 || (hostile && b.rd16(o + 0x62) == 0) || b.ignore.get(&o).is_some_and(|&u| u > now) {
                continue;
            }
            let (x, y) = (b.rd16(o + 8) as i32, b.rd16(o + 0x10) as i32);
            let d = (x - px).abs() + 2 * (y - py).abs();
            if best.is_none_or(|t| (hostile, -d) > (t.3, -t.0)) {
                *best = Some((d, x, y, hostile, o));
            }
        }
    };
    scan(b, 0xffba1e, 3, 0xc0, true, &mut best);
    scan(b, 0xffa99e, 22, 0xc0, true, &mut best);
    if best.is_none() {
        scan(b, 0xffe2de, 32, 0x60, false, &mut best);
        scan(b, 0xffeede, 21, 0x80, false, &mut best);
    }
    // objet ou caisse poursuivi trop longtemps (hors d'atteinte : derrière un mur, décor) : ignoré 2 000 images
    if b.patience != 0 {
        let o = best.filter(|t| !t.3).map_or(0, |t| t.4);
        if b.chase[pl].0 != o {
            b.chase[pl] = (o, 0);
        }
        b.chase[pl].1 += 1;
        if o != 0 && b.chase[pl].1 > b.patience {
            b.ignore.insert(o, now + 2000);
            b.chase[pl] = (0, 0);
            best = None;
        }
    }
    if pl == 0 && b.f % 200 == 0 && std::env::var("BOTDEBUG").is_ok() {
        eprintln!("f {} pos {px},{py} hp {hp} cible {best:?}", b.f);
    }
    let (mut right, mut left, mut up, mut down, mut b1, mut b2, mut b3, mut b4) = (false, false, false, false, false, false, false, false);
    match best {
        Some((_, tx, ty, hostile, _)) => {
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
                if magic.period != 0 && f % magic.period < 4 { b4 = true }
                if magic.period != 0 && f % magic.period >= 12 && f % magic.period < 16 { b3 = true }
            }
        }
        None => {
            right = true;
            if f % 90 < 4 { b1 = true }
            if f % 150 < 3 { if (f / 150) % 2 == 0 { up = true } else { down = true } }
            if f % 700 < 4 { b2 = true }
        }
    }
    if b.spell_period != 0 && matches!(f.wrapping_sub(b.spell_at[pl]), 2..=5 | 42..=45 | 82..=85) { b4 = true } // attaque spéciale du --spells
    // objets : choisir (Button 4) puis utiliser (Button 3)
    if f % 1100 < 4 { b4 = true }
    if f % 1100 >= 20 && f % 1100 < 24 { b3 = true }
    // variété : quelques actions aléatoires
    if rnd(400) == 0 { b2 = true }
    apply_keys(b, pl, [right, left, up, down, b1, b2, b3, b4]);
}

/// Un vote de chemin est-il ouvert ? (un EFFCT de type 23 en vie, FFD49E : 38 emplacements de 0x60 octets)
fn vote_open(b: &Bot) -> bool {
    (0..38).any(|i| {
        let o = 0xffd49e + 0x60 * i;
        b.rd8(o) != 0 && b.rd8(o + 2) == 23
    })
}

/// `--wander` : gestes au hasard (direction tenue jusqu'à la fin du geste, bouton tapé pendant ses quatre premières images).
fn wander_step(b: &mut Bot, pl: usize, rnd: &mut impl FnMut(u32) -> u32) {
    let vote = vote_open(b);
    if vote && !b.voting {
        // nouveau vote : abstentions tirées pour tous les joueurs
        for k in 0..4 {
            b.wander_keys[k].3 = rnd(4) == 0;
        }
    }
    if pl == 0 {
        b.voting = vote;
    }
    let (_, until, _, abstain) = b.wander_keys[pl];
    if b.f >= until {
        let mut keys = [false; 8];
        match rnd(9) {
            0 => keys[0] = true,
            1 => keys[1] = true,
            2 => keys[2] = true,
            3 => keys[3] = true,
            4 => (keys[0], keys[2]) = (true, true),
            5 => (keys[1], keys[3]) = (true, true),
            _ => {}
        }
        if !(vote && abstain) {
            match rnd(16) {
                0..=5 => keys[4] = true,
                6 | 7 => keys[5] = true,
                8 | 9 => keys[6] = true,
                10 => keys[7] = true,
                _ => {}
            }
        }
        b.wander_keys[pl] = (b.f, b.f + 20 + rnd(70) as usize, keys, abstain);
    }
    let (start, _, mut keys, _) = b.wander_keys[pl];
    if b.f >= start + 4 {
        keys[4..].fill(false);
    }
    apply_keys(b, pl, keys);
}

fn apply_keys(b: &mut Bot, pl: usize, keys: [bool; 8]) {
    let names = ["Right", "Left", "Up", "Down", "Button 1", "Button 2", "Button 3", "Button 4"];
    for (n, v) in names.iter().zip(keys) {
        b.set(format!("P{} {n}", pl + 1), v);
    }
}
