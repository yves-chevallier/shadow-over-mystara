//! Validation différentielle sur des états aléatoires : chaque routine native d'un groupe est appelée
//! en mode contrôle depuis des états tirés au sort ; l'original s'exécute à côté et les deux sont
//! comparés (mémoire, registres du contrat, durée si elle est déclarée exacte). Exerce les chemins
//! que les scénarios n'atteignent pas.
//!
//!   fuzz <groupe> <roms> <eeprom> <scénario> <images de chauffe> <tours>
//!
//! Groupes : son (0x68c, 0xe0836-0xee000), u1 (RNG et files d'événements 0x19a80-0x1a000),
//! c1 (monnayeurs 0x13c00-0x14e50), m2 (EEPROM et configuration 0x2d300-0x2ede0),
//! video (primitives vidéo, palettes et fondus : natives `video_*` et `palette_*`),
//! camera (recopie du décor par les caméras 1 et 2, 0x2f81c 0x2f86e 0x328a8, et décors à parallaxe 0x33178...).

use std::path::Path;

use ddsom_game::mem::{wr16, wr32, wr8, A5};

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    /// Vrai avec la probabilité 1/n.
    fn une_chance_sur(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

/// Registres quelconques ; A0 sur un enregistrement joueur.
fn registres(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    for r in 0..8 {
        c.d[r] = if alea.next() & 3 == 0 { alea.next() & 0xff } else { alea.next() };
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (alea.next() & 0xffff);
    }
    let obj = 0xff85de + 0x200 * (alea.next() % 4);
    for o in [2u32, 3, 0x7e] {
        wr8(b, obj + o, alea.next() as u8);
    }
    wr16(b, obj + 0x48, alea.next() as u16);
    wr16(b, obj + 8, alea.next() as u16 & 0x7ff);
    c.a[0] = obj;
}

fn alea_son(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    wr8(b, A5 + 0x27, alea.next() as u8 & 1);
    wr8(b, A5 + 0x30, alea.next() as u8 & 1);
    wr8(b, A5 + 0x28, alea.next() as u8 & 1);
    wr8(b, A5 + 0x36, alea.next() as u8 & 1);
    wr16(b, A5 + 0x7fcc, alea.une_chance_sur(3) as u16);
    wr16(b, A5 + 0x7fc2, if alea.une_chance_sur(2) { 0 } else { alea.next() as u16 & 7 });
    wr16(b, A5 + 0x7fc4, alea.next() as u16);
    wr8(b, 0xff2a1e, alea.une_chance_sur(4) as u8);
    wr16(b, 0xff2960, if alea.next() & 1 == 0 { 0 } else { 0xffff });
    wr16(b, A5 + 0x5a6, alea.next() as u16 & 0x3ff);
    wr16(b, A5 + 0x5ae, alea.next() as u16 & 0x3ff);
    // file : têtes quelconques, parfois pleine ou vide
    let lecture = (alea.next() as u16 & 0x3f) * 0x10;
    let ecriture = match alea.next() % 4 {
        0 => lecture,
        1 => lecture.wrapping_sub(0x10) & 0x3f0,
        _ => (alea.next() as u16 & 0x3f) * 0x10,
    };
    wr16(b, 0xff295c, lecture);
    wr16(b, 0xff295e, ecriture);
    wr8(b, 0x61801f, if alea.next() & 1 == 0 { 0xff } else { 0 });
    registres(c, b, alea);
}

/// Un compteur BCD : parfois proche du plafond, parfois n'importe quoi.
fn bcd(alea: &mut Alea) -> u32 {
    match alea.next() % 4 {
        0 => 0x0999_9990 + alea.next() % 10,
        1 => alea.next(),
        _ => (0..8).fold(0, |v, _| v << 4 | alea.next() % 10),
    }
}

fn anneau(b: &mut Bus, alea: &mut Alea, ecriture: u32, cases: u32, lecture: bool) {
    wr8(b, A5 + ecriture, (alea.next() as u8 & 0x3f) * 2);
    if lecture {
        wr8(b, A5 + ecriture + 1, (alea.next() as u8 & 0x3f) * 2);
    }
    for i in 0..64 {
        let v = if alea.next() % 3 == 0 { (alea.next() as u16 & 0x37f) | 0x0000 } else { 0xffff };
        wr16(b, A5 + cases + 2 * i, v);
    }
}

fn alea_u1(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    wr16(b, A5 + 0x1a, alea.next() as u16);
    anneau(b, alea, 0x108, 0x10a, true);
    anneau(b, alea, 0x18a, 0x18c, true);
    // pop-ups : entrées de 8 octets, index multiples de 8
    wr8(b, A5 + 0x50, alea.une_chance_sur(4) as u8);
    wr8(b, A5 + 0x20c, (alea.next() as u8 & 0x1f) * 8);
    wr8(b, A5 + 0x20d, (alea.next() as u8 & 0x1f) * 8);
    for i in 0..32 {
        // entrées plausibles : type (octet haut 0-3), code < 0x18, petits paramètres
        let plausible = (alea.next() % 4) as u16 * 0x100 | (alea.next() % 0x18) as u16;
        let base = A5 + 0x20e + 8 * i;
        wr16(b, base, if alea.next() % 3 == 0 { plausible } else { 0xffff });
        wr16(b, base + 2, alea.next() as u16 % 0x100);
        wr16(b, base + 4, if alea.next() & 1 == 0 { 0 } else { alea.next() as u16 % 0x100 });
        wr16(b, base + 6, 0x80);
    }
    // pop-up courant : durée restante 0, 1, 2, 3, 4, 5 ou longue
    let duree = [0u16, 1, 2, 3, 4, 5, 0x60][alea.next() as usize % 7];
    wr16(b, 0xff9180, (alea.next() % 4) as u16 * 0x100 | (alea.next() % 0x18) as u16);
    wr16(b, 0xff9182, alea.next() as u16 % 0x100);
    wr16(b, 0xff9184, alea.next() as u16 % 0x100);
    wr16(b, 0xff9186, duree);
    for j in 0..4 {
        wr32(b, 0xff85de + 0x200 * j + 0x110, bcd(alea));
        wr32(b, 0xff85de + 0x200 * j + 0x114, bcd(alea) & 0x000f_ffff);
    }
    registres(c, b, alea);
}

/// Monnayeurs : réglages, enregistrements et lignes d'entrée quelconques.
fn alea_c1(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    wr16(b, A5 + 0x7fcc, alea.une_chance_sur(8) as u16);
    wr8(b, A5 + 0x1e, 1 + alea.next() as u8 % 4);
    wr8(b, A5 + 0x1f, 1 + alea.next() as u8 % 4);
    wr8(b, A5 + 0x20, alea.une_chance_sur(2) as u8);
    let monnayeurs = [0u8, 1, 2, 3, 0xff, 7];
    wr8(b, A5 + 0x21, monnayeurs[alea.next() as usize % 6]);
    wr8(b, A5 + 0x22, alea.une_chance_sur(2) as u8);
    wr8(b, A5 + 0x25, alea.une_chance_sur(4) as u8);
    wr8(b, A5 + 0x30, alea.une_chance_sur(2) as u8);
    wr8(b, 0xff8ec4, alea.une_chance_sur(2) as u8);
    for k in 0..4 {
        let base = A5 + 0x44e + 0x10 * k;
        let antirebond = [0u8, 1, 2, 3, 49, 50, 51, 255];
        wr8(b, base, if alea.next() & 1 == 0 { antirebond[alea.next() as usize % 8] } else { alea.next() as u8 });
        wr8(b, base + 1, alea.next() as u8 % 6);
        wr8(b, base + 2, [0u8, 4, 8, 9, 10, 12, 0x80, 255][alea.next() as usize % 8]);
        wr8(b, base + 3, alea.next() as u8);
        wr8(b, base + 7, alea.une_chance_sur(2) as u8);
    }
    b.inputs.in2 = alea.next() as u16;
    registres(c, b, alea);
}

/// EEPROM et comptabilité : contenu de l'EEPROM, copies en RAM, paramètres de transfert, compteurs.
fn alea_m2(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    let mut donnees = [0u16; 64];
    for d in donnees.iter_mut() {
        *d = alea.next() as u16;
    }
    if alea.next() & 1 == 0 {
        // les deux exemplaires de la configuration concordent
        for i in 0..24 {
            donnees[24 + i] = donnees[i];
        }
    }
    b.eeprom = cps2::eeprom::Eeprom93c46::new(donnees);
    let mots = [0u16, 1, 2, 5, 23, 47, 63][alea.next() as usize % 7];
    let debut = 2 * (alea.next() as u16 % (64 - mots));
    wr16(b, A5 - 0x616e, mots);
    wr16(b, A5 - 0x616a, debut);
    wr16(b, A5 - 0x616c, mots);
    wr16(b, A5 - 0x6168, debut);
    wr32(b, A5 - 0x6172, if alea.next() & 1 == 0 { 0xff1d8e } else { 0xff1e0e });
    for i in 0..64 {
        let v = if alea.next() % 4 == 0 { donnees[i] ^ 1 } else { donnees[i] };
        wr16(b, 0xff1d8e + 2 * i as u32, if alea.next() % 3 == 0 { alea.next() as u16 } else { v });
        wr16(b, 0xff1e0e + 2 * i as u32, alea.next() as u16);
    }
    for (valeur, image) in [(0x76u32, 0xffff_9deeu32), (0x7a, 0xffff_9df2), (0x7e, 0xffff_9df6), (0x7fd4, 0xffff_9dfa)] {
        let v = alea.next() % 1000;
        wr32(b, A5 + valeur, v);
        wr32(b, A5.wrapping_add(image), if alea.next() & 1 == 0 { v + 1 } else { alea.next() % 1000 });
    }
    wr8(b, A5 + 0x82, alea.une_chance_sur(4) as u8);
    wr8(b, A5 + 0x25, alea.une_chance_sur(2) as u8);
    wr8(b, A5 + 0x74, 0);
    wr16(b, A5 + 0x16, alea.next() as u16 & 0x8f88);
    wr8(b, A5 + 0x18, alea.next() as u8 & 6);
    wr16(b, 0x664000, alea.next() as u16);
    // configuration : variante, copie en RAM (valeurs valides ou non), bloc par défaut dans A6
    let variante = 2 * (alea.next() as u16 % 6);
    wr16(b, A5 + 0x70, variante);
    for i in 0..0x80u32 {
        let petit = if alea.next() % 3 == 0 { alea.next() as u8 } else { (alea.next() % 20) as u8 };
        wr8(b, 0xff1d8e + i, petit);
    }
    let bloc = 0x2d656 + b_prog16(b, 0x2d656 + variante as u32) as i16 as u32;
    c.a[6] = if alea.next() & 3 == 0 { bloc + 0x30 } else { bloc };
    registres(c, b, alea);
}

/// Routine tirée au sort (pour les états qui dépendent de l'entrée appelée).
static ENTREE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Couleur de palette : luminosité souvent aux butées, composantes quelconques ou proches de 0 / 0xf.
fn couleur(alea: &mut Alea) -> u16 {
    let lum = match alea.next() % 4 {
        0 => 0,
        1 => 0xf000,
        _ => (alea.next() as u16 & 0xf) << 12,
    };
    let compo = |a: &mut Alea| match a.next() % 4 {
        0 => 0,
        1 => 0xf,
        _ => (a.next() & 0xf) as u16,
    };
    lum | compo(alea) << 8 | compo(alea) << 4 | compo(alea)
}

/// Primitives vidéo, palettes et fondus : palettes, état de fondu, scène et caméras quelconques.
fn alea_video(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    let entree = ENTREE.load(std::sync::atomic::Ordering::Relaxed);
    for i in 0..0x800u32 {
        let v = if alea.next() % 5 == 0 { couleur(alea) } else { couleur(alea) & 0xf0ff | (i as u16 & 0xf00) };
        wr16(b, 0x91c000 + 2 * i, v);
    }
    // palettes cibles en RAM (0xff1190, 512 couleurs)
    for i in 0..512u32 {
        wr16(b, 0xff1190 + 2 * i, couleur(alea));
    }
    wr8(b, A5 + 0x48e, alea.next() as u8 & 0xf);
    for k in 0..4u32 {
        let cible = match alea.next() % 3 {
            0 => 0xff1190,
            1 => 0x253000 + 0x20 * (alea.next() % 0x1000),
            _ => 0x91c000 + 0x400 * k,
        };
        // 0x1cdd6 lit un mot (couleur) : mêmes octets, interprétés autrement
        wr32(b, A5 + 0x492 + 4 * k, if entree == 0x1cdd6 { couleur(alea) as u32 } else if entree == 0x158e || entree == 0x15c8 { 0x91c000 + 0x400 * k } else { cible });
    }
    if entree == 0x15c8 {
        wr32(b, A5 + 0x492, 0x91c3a0);
    }
    wr8(b, A5 + 0x34, [2, 0x14, 0x2f, 0x42, 0x44, 0x45, 0x59, 0x5a, 0x3, 0x8][alea.next() as usize % 10]);
    if alea.next() % 3 == 0 {
        wr8(b, A5 + 0x34, alea.next() as u8 % 0x60);
    }
    wr8(b, A5 + 0x36, alea.une_chance_sur(2) as u8);
    for off in [0x506u32, 0x50a, 0x556, 0x55a] {
        wr16(b, A5 + off, alea.next() as u16);
    }
    for i in 0..0x2000u32 {
        wr16(b, 0x914000 + 2 * i, alea.next() as u16);
    }
    wr16(b, A5 + 0x7fd0, if alea.next() & 1 == 0 { 0x400 } else { 0 });
    wr8(b, 0xff0eab, alea.une_chance_sur(3) as u8);
    wr8(b, A5 + 0x18, alea.next() as u8);
    registres(c, b, alea);
    c.a[4] = A5 + 0x48e;
    // numéros de scène / d'ensemble valides (les tables de la ROM s'arrêtent à la scène 0x5a)
    c.d[0] = if alea.next() % 3 == 0 { alea.next() % 0x5b } else { alea.next() % 12 };
    c.d[1] = if alea.next() & 1 == 0 { 0x1000 } else { 0xffff_f000 };
    if entree == 0x1a60a {
        c.d[0] %= 3; // la table de la page 1 n'a que trois entrées
    }
    let page = 0x91c000 + 0x400 * (alea.next() % 4);
    c.a[0] = page;
    c.a[1] = match entree {
        0x1602 | 0x164e => page,
        0x1cfc8 => [0xff1190, 0x253000 + 0x20 * (alea.next() % 0x1000)][alea.next() as usize % 2],
        _ => 0x916000 + 0x400 * (alea.next() % 8),
    };
    if entree == 0x164e {
        c.a[1] = 0x91c3a0;
    }
    if matches!(entree, 0x24a8) {
        let obj = 0xff85de + 0x200 * (alea.next() % 4);
        wr16(b, obj + 8, alea.next() as u16);
        wr16(b, obj + 0x10, alea.next() as u16);
        c.a[0] = obj;
    }
    if entree == 0x1d042 {
        c.d[1] = couleur(alea) as u32;
    }
    if matches!(entree, 0x255a | 0x25e8) {
        c.d[0] = if alea.next() % 3 == 0 { alea.next() } else { alea.next() % 130 };
        c.a[1] = 0x912000 + 0x80 * (alea.next() % 40) + 4 * (alea.next() % 30);
    }
}

/// Caméras de défilement : positions cible et dessinée, butées et indicateurs quelconques mais plausibles
/// (les écarts vont du simple décalage à plusieurs pas, dans les quatre sens).
fn alea_camera(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    let entree = ENTREE.load(std::sync::atomic::Ordering::Relaxed);
    if matches!(entree, 0x33178 | 0x33196 | 0x331e8 | 0x3326c) {
        // décors à parallaxe : défilement courant et précédent, position de l'objet
        for off in [0x506u32, 0x50a, 0x524, 0x528] {
            wr32(b, A5 + off, alea.next() << 8 ^ alea.next());
        }
        registres(c, b, alea);
        for off in [0x08u32, 0x0c, 0x10] {
            wr32(b, c.a[0] + off, alea.next() << 8 ^ alea.next());
        }
        return;
    }
    let cam = if entree == 0x328a8 { A5 + 0x54e } else { A5 + 0x4fe };
    let pas: i32 = if entree == 0x328a8 { 0x20 } else { 0x10 };
    let ecart = |a: &mut Alea, portee: i32| -> i32 {
        match a.next() % 4 {
            0 => 0,
            1 => (a.next() as i32 % (2 * pas)) - pas,
            _ => (a.next() as i32 % (2 * portee)) - portee,
        }
    };
    // positions qui restent dans la carte du décor (au-delà, la ROM n'a plus de table : lectures sans sens)
    let dessine_x = 0x100 + (alea.next() % 0xb00) as i32;
    let dessine_y = 0x40 + (alea.next() % 0x260) as i32;
    let x = dessine_x + ecart(alea, 4 * pas);
    let y = dessine_y + ecart(alea, 3 * pas);
    wr16(b, cam + 0x10, dessine_x as u16);
    wr16(b, cam + 0x14, dessine_y as u16);
    wr16(b, cam + 0x08, x as u16);
    wr16(b, cam + 0x0c, y as u16);
    wr16(b, cam + 0x20, if alea.next() % 4 == 0 { 4 } else { 0 });
    wr16(b, cam + 0x2e, (x - alea.next() as i32 % 0x100) as u16);
    wr16(b, cam + 0x34, (x + (alea.next() as i32 % 0x200) - 0x40) as u16);
    wr8(b, cam + 0x3d, alea.une_chance_sur(3) as u8);
    wr8(b, cam + 0x3e, alea.une_chance_sur(8) as u8);
    wr8(b, A5 + 0x37, alea.une_chance_sur(4) as u8);
    registres(c, b, alea);
    c.a[0] = cam;
}

fn b_prog16(b: &mut Bus, a: u32) -> u16 {
    m68k::Bus::read_prog16(b, a)
}

/// Familles d'objets (ESHL, SET, SOBJ) : pour chaque gestionnaire, des trajectoires complètes. Un
/// objet neuf du type est créé à l'état 0 (propriétaire et variante tirés au sort) puis le
/// gestionnaire est appelé image après image, avec des perturbations aléatoires des entrées que le
/// moteur de collisions ou le créateur modifient (drapeaux de touche, propriétaire, positions).
/// Les états sont ainsi atteints par le code lui-même. Chaque appel est comparé à l'original.
/// `DDSOM_ENTREE=<adresse hexa>` restreint à un gestionnaire.
fn familles(roms: &str, eeprom: &str, scenario: &str, chauffe: u32, tours: u32, filtre: Option<u32>) {
    use ddsom_game::mem::{rd16, rd8};
    use ddsom_game::objects::pools::{Pool, ENEMY, ESHL, SET, SOBJ};
    use m68k::decode::Fetch;
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    std::panic::set_hook(Box::new(|_| {}));
    let mut alea = Alea(0x1234_5678_9abc_def1);
    let tables: [(&Pool, u32, u32); 3] = [(&ESHL, 0x39b96, 64), (&SET, 0x39eac, 34), (&SOBJ, 0x39fa8, 16)];
    let mut entrees: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        for k in 0..n {
            let a = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == a) && filtre.map_or(true, |f| f == a) {
                entrees.push((a, pool, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    if std::env::var("DDSOM_VERBOSE").is_ok() {
        let mut b = b0.clone();
        println!("places libres : ESHL {} SET {} SOBJ {}", rd16(&mut b, A5 + ESHL.free), rd16(&mut b, A5 + SET.free), rd16(&mut b, A5 + SOBJ.free));
    }
    // nombre de variantes tirées (3 par défaut : au-delà, beaucoup de types sortent de leurs tables)
    let variantes: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let mut affiches = 0;
    let mut essais = 0;
    // couverture du code d'origine des gestionnaires (DDSOM_COUVERTURE=fichier : écrit les adresses vues)
    let couverture = std::env::var("DDSOM_COUVERTURE").is_ok();
    let mut vus: std::collections::BTreeSet<u32> = Default::default();
    for _ in 0..tours {
        for &(entree, pool, kind) in &entrees {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            // propriétaires possibles : ennemis actifs, sinon joueurs
            let mut proprietaires: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
            proprietaires.extend((0..4).map(|j| 0xff85de + 0x200 * j));
            let obj = pool.slot(alea.next() % pool.slots);
            for off in (0..pool.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            let owner = proprietaires[alea.next() as usize % proprietaires.len()];
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, (alea.next() % variantes) as u8);
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, (alea.next() & 1) as u8);
            wr8(&mut b, obj.0 + 0x2f, if alea.next() % 4 == 0 { alea.next() as u8 % 3 } else { 0 });
            wr16(&mut b, obj.0 + 0x50, owner as u16);
            wr16(&mut b, obj.0 + 0x46, owner as u16);
            wr16(&mut b, obj.0 + 0x5a, owner as u16);
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, owner + off) as u32) << 16 | (alea.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            wr16(&mut b, obj.0 + 0xc, (alea.next() % 0x60) as u16);
            // décompte des places libres cohérent
            // (au moment du chauffage le pool peut être plein : les gestionnaires testent ce compteur)
            wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
            for pas in 0..60 {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if pas > 0 {
                    if alea.next() % 6 == 0 {
                        wr8(&mut b, obj.0 + 0x57, [0x08, 0x20, 0x28, 0x10, 0x02, 0x40][alea.next() as usize % 6]);
                    }
                    if alea.next() % 8 == 0 {
                        wr8(&mut b, obj.0 + 0x33, alea.next() as u8 & 1);
                    }
                    if alea.next() % 10 == 0 {
                        wr8(&mut b, owner + 4, (alea.next() % 8) as u8);
                    }
                    // le propriétaire change d'état : états de l'automate, drapeaux d'attaque, compteurs
                    if alea.next() % 3 == 0 {
                        for off in [4u32, 5, 6, 7] {
                            if alea.next() % 2 == 0 {
                                wr8(&mut b, owner + off, 2 * (alea.next() % 6) as u8);
                            }
                        }
                        for off in [0x53u32, 0xa0, 0xa1, 0xa8, 0xa9, 0xb0, 0xbd, 0x3e, 0x2f, 0x42, 0x5d] {
                            if alea.next() % 3 == 0 {
                                let v = match alea.next() % 4 {
                                    0 => 0,
                                    1 => 1,
                                    2 => (alea.next() % 4) as u8,
                                    _ => alea.next() as u8,
                                };
                                wr8(&mut b, owner + off, v);
                            }
                        }
                    }
                    // sa propre variante / drapeaux de contrôle
                    if alea.next() % 12 == 0 {
                        wr8(&mut b, obj.0 + 0x3e, alea.next() as u8 % 6);
                    }
                    if alea.next() % 10 == 0 {
                        wr8(&mut b, obj.0 + 0x53, (alea.next() % 5) as u8);
                    }
                }
                // jeu en pause (rare) ou non ; objet suspendu (octet +0x40) très rarement
                wr8(&mut b, A5 + 0x3d, (alea.next() % 10 == 0) as u8);
                wr8(&mut b, obj.0 + 0x40, (alea.next() % 40 == 0) as u8);
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = pas;
                // registres de la boucle de dispatch : D0 = type * 4 (octet 0 haut vide), A1 = gestionnaire
                c.d[0] = 4 * kind as u32;
                c.a[1] = entree;
                essais += 1;
                let avant = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                let (cs, bs) = (c.clone(), b.clone());
                if couverture {
                    // adresses des instructions que l'original exécute à cet appel (interpréteur)
                    let (mut c2, mut b2) = (cs.clone(), bs.clone());
                    c2.pc = entree;
                    c2.a[7] = cs.a[7].wrapping_sub(4);
                    ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                    for _ in 0..20_000 {
                        if c2.pc == 0xfffe {
                            break;
                        }
                        vus.insert(c2.pc);
                        c2.step(&mut b2);
                    }
                }
                let avant_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
                if r.is_err() {
                    // l'original est sorti des rails (entrée incohérente) : on abandonne la trajectoire
                    let _ = (cs, bs);
                    break;
                }
                let apres = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                if apres > avant && affiches < 12 {
                    affiches += 1;
                    let dernier = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                    println!("écart en {entree:06x} type {kind} pas {pas} : {}", dernier.unwrap_or_default());
                    println!("  objet avant : {:02x?}", avant_obj);
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        // exécution pas à pas du code d'origine seul depuis l'état d'avant
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        let sp = c2.a[7];
                        c2.pc = entree;
                        c2.a[7] = sp.wrapping_sub(4);
                        ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                        for _ in 0..400 {
                            if c2.pc == 0xfffe {
                                break;
                            }
                            let ins = m68k::decode(&mut b2, c2.pc);
                            println!("    {:06x} {:?}", c2.pc, ins.op);
                            c2.step(&mut b2);
                        }
                    }
                }
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    // calibrage des durées estimées des gestionnaires : écart moyen original - native par appel
    for (name, st) in ddsom_game::report() {
        if st.calls > 0 && (name.starts_with("eshl_") || name.starts_with("set_") || name.starts_with("sobj_")) {
            let ecart = (st.original_cycles as i64 - st.native_cycles as i64) as f64 / st.calls as f64;
            println!("  durée {name}: original {:.0} cycles/appel, écart à corriger de la durée estimée {ecart:+.0}", st.original_cycles as f64 / st.calls as f64);
        }
    }
    println!("{essais} appels de gestionnaires");
    if let Ok(fichier) = std::env::var("DDSOM_COUVERTURE") {
        let texte: Vec<String> = vus.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(fichier, texte.join("\n")).expect("écriture couverture");
    }
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, groupe, roms, eeprom, scenario, chauffe, tours] = &a[..] else {
        eprintln!("usage : fuzz <groupe> <roms> <eeprom> <scénario> <images de chauffe> <tours>");
        std::process::exit(2);
    };
    if groupe == "familles" {
        familles(roms, eeprom, scenario, chauffe.parse().unwrap(), tours.parse().unwrap(), std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()));
        return;
    }
    let (alea_etat, plage): (fn(&mut Cpu, &mut Bus, &mut Alea), std::ops::Range<u32>) = match groupe.as_str() {
        "son" => (alea_son, 0x68c..0xee000),
        "u1" => (alea_u1, 0x19a80..0x1a000),
        "c1" => (alea_c1, 0x13c00..0x14e50),
        "m2" => (alea_m2, 0x2d300..0x2ede0),
        "video" => (alea_video, 0xb40..0x1d100),
        "camera" => (alea_camera, 0x2f000..0x33400),
        _ => {
            eprintln!("groupe inconnu : {groupe}");
            std::process::exit(2);
        }
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    let mut alea = Alea(0x9e37_79b9_7f4a_7c15);
    let entrees: Vec<u32> = ddsom_game::all_natives()
        .map(|n| n.entry)
        .filter(|e| plage.contains(e) && (*groupe != "son" || *e == 0x68c || *e >= 0xe0836))
        .filter(|e| *groupe != "camera" || matches!(*e, 0x2f81c | 0x2f86e | 0x328a8 | 0x33178 | 0x33196 | 0x331e8 | 0x3326c))
        .filter(|e| *groupe != "video" || matches!(*e, 0xb40 | 0xb46 | 0xb96 | 0xbac | 0xbc6 | 0xbfc | 0xc58 | 0xca8 | 0xcc8 | 0xce8 | 0x158e | 0x15c8 | 0x1602 | 0x164e | 0x1a296 | 0x1a350 | 0x1a360 | 0x1a366 | 0x1a3d4 | 0x1a608 | 0x1a60a | 0x1a678 | 0x1a67e | 0x1a8ac | 0x1a8b2 | 0x1cd32 | 0x1cd7c | 0x1cdd6 | 0x1cf3c | 0x1cfb8 | 0x1cfc8 | 0x1d00e | 0x1d042 | 0x24a8 | 0x24b0 | 0x255a | 0x25e8 | 0x260c | 0x26d4 | 0x2766))
        .collect();
    let (c, b) = (&mut m.cpu, &mut m.bus);
    b.irq_horizon = u64::MAX;
    let mut affiches = 0;
    for _ in 0..tours.parse::<u32>().unwrap() {
        for &entree in &entrees {
            ENTREE.store(entree, std::sync::atomic::Ordering::Relaxed);
            alea_etat(c, b, &mut alea);
            let (d0, a0) = (c.d[0], c.a[0]);
            let contexte = if entree == 0x2d662 {
                let var = ddsom_game::mem::rd16(b, A5 + 0x70);
                let copie: Vec<u8> = (0..18).map(|i| ddsom_game::mem::rd8(b, 0xff1d8e + i)).collect();
                let bloc: Vec<u8> = (0..18).map(|i| ddsom_game::mem::rd8(b, c.a[6] + i)).collect();
                format!(" var={var} copie={copie:02x?} defauts={bloc:02x?}")
            } else if *groupe == "camera" {
                let cam = c.a[0];
                let champs: Vec<String> = [0x08u32, 0x0c, 0x10, 0x14, 0x20, 0x2e, 0x34, 0x40]
                    .iter()
                    .map(|o| format!("{o:02x}={:04x}", ddsom_game::mem::rd16(b, cam + o)))
                    .collect();
                format!(" scène={:02x} terrain={} {}", ddsom_game::mem::rd8(b, A5 + 0x34), ddsom_game::mem::rd8(b, A5 + 0x37), champs.join(" "))
            } else {
                String::new()
            };
            let avant = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            ddsom_game::call(c, b, entree);
            let apres = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            if apres > avant && affiches < 12 {
                affiches += 1;
                let dernier = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                println!("écart en {entree:06x} (d0={d0:08x} a0={a0:06x}){contexte} : {}", dernier.unwrap_or_default());
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}
