//! Variante de fuzz.rs du lot ENEMY (table ENEMY, états valides par type, maîtres et cibles fabriqués).
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

#[path = "fuzz_enemy/lot3.rs"]
mod lot3;

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
const DDSOM_TRACE_LEN: u32 = 200000;
/// États de départ (`+4`, `+5`, `+6`) que la table de sauts de chaque type ESHL 15..39 accepte
/// (au-delà, l'original sort du code) : listes de valeurs possibles par octet.
/// États (`+4`, `+5`, `+6`) valides d'un type ENEMY : ceux que les tables de sauts de l'original couvrent.
fn etats_enemy(kind: u8, variante: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
    const Z: &[u8] = &[0];
    match kind {
        // Riyakaa : sous-états et sous-sous-états que les tables de sauts acceptent (l'arrivée dépend
        // de la variante)
        16 => match variante {
            8 | 10 => &[
                (&[0], Z, Z),
                (&[2], &[0], &[0, 2, 4, 6]),
                (&[2], &[2], &[0, 2, 4]),
                (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
                (&[2], &[6], &[0, 2, 4, 6, 8, 10]),
                (&[2], &[8], &[0, 2, 4, 6]),
                (&[2], &[10], &[0, 2]),
                (&[4], &[0], &[0]),
                (&[4], &[2], &[0, 2]),
                (&[4], &[4], &[0, 2, 4]),
                (&[4], &[6], &[0]),
                (&[4], &[8], &[0, 2, 4]),
                (&[6], Z, Z),
            ],
            _ => &[
                (&[0], Z, Z),
                (&[2], &[0], &[0, 2, 4]),
                (&[2], &[2], &[0, 2, 4]),
                (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
                (&[2], &[6], &[0, 2, 4, 6, 8, 10]),
                (&[2], &[8], &[0, 2, 4, 6]),
                (&[2], &[10], &[0, 2]),
                (&[4], &[0], &[0]),
                (&[4], &[2], &[0, 2]),
                (&[4], &[4], &[0, 2, 4]),
                (&[4], &[6], &[0]),
                (&[4], &[8], &[0, 2, 4]),
                (&[6], Z, Z),
            ],
        },
        22..=24 => &[(&[0, 2, 4, 6], Z, Z)],
        26 => &[(&[0, 2, 4, 6], Z, Z)],
        7 => &[
            (&[0], Z, Z),
            (&[2], &[0, 6, 10], &[0, 2]),
            (&[2], &[2], &[0, 2, 4]),
            (&[2], &[4, 8], &[0, 2, 4, 6, 8]),
            (&[4, 6], Z, Z),
        ],
        28 => &[(&[0], Z, Z), (&[2], &[0, 2, 4, 6], Z), (&[4, 6], Z, Z)],
        20 => &[(&[0], Z, Z), (&[2], &[0, 2], Z), (&[4], &[0], &[0, 2]), (&[4], &[2], &[0, 2, 4, 6, 8, 10]), (&[6], Z, Z)],
        // DarkWarrior 1 : bras liés à un hôte, l'étape `+5` dépend de l'état de l'hôte
        25 => &[(&[0], Z, Z), (&[2], &[0, 2, 4, 6], Z), (&[4], Z, Z), (&[6], Z, Z)],
        // Harpy : états que les tables de sauts de l'original acceptent
        21 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2], &[0, 2, 4, 6]),
            (&[2], &[4], Z),
            (&[2], &[6], Z),
            (&[2], &[8], &[0, 2, 4]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2]),
            (&[6], Z, Z),
        ],
        // Owlbear : même charpente que le Riyakaa
        4 => &[
            (&[0], Z, Z),
            (&[2], &[0], Z),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18]),
            (&[2], &[6], &[0, 2, 4]),
            (&[2], &[8], &[0, 2, 4, 6]),
            (&[2], &[10], Z),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6], Z),
            (&[4], &[8], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // Displacer Beast : un groupe de comportements (+5) par état actif, ses étapes (+6)
        19 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22]),
            (&[2], &[2], &[0, 2, 4, 6]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36]),
            (&[2], &[6], &[0, 2, 4, 6, 8, 10, 12]),
            (&[2], &[8], &[0, 2, 4, 6]),
            (&[2], &[10], &[0, 2, 4]),
            (&[2], &[12], &[0, 2, 4, 6, 8]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2]),
            (&[4], &[4], &[0, 2, 4, 6]),
            (&[4], &[6], Z),
            (&[4], &[8], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[6], Z, Z),
        ],
        _ => &[(&[0, 2, 4, 6], &[0, 2, 4], &[0, 2, 4])],
    }
}

/// Étape `+7` valide de l'Owlbear (type 4) pour un état `(+4, +5, +6)`.
fn etape_owlbear(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n = match (s4, s5, s6) {
        (2, 0, _) => 4,
        (2, 2, _) => 2,
        (2, 4, 0) => 6,
        (2, 4, 2 | 4 | 12) => 12,
        (2, 4, 6) => 18,
        (2, 4, 8) => 6,
        (2, 4, 10) => 8,
        (2, 4, 14) => 16,
        (2, 4, 16 | 18) => 10,
        (2, 6, 2) => 12,
        (2, 6, 4) => 6,
        (2, 8, _) => 2,
        (2, 10, _) => 4,
        _ => 0,
    };
    2 * (alea.next() % (n / 2 + 1)) as u8
}

/// Champs du bras (type 25) et de son hôte à des valeurs que les tables de l'original acceptent :
/// état de l'hôte `+0xa2` (0 à 8), sous-état `+5` valide pour le chemin choisi, positions proches.
fn prep_warrior(b: &mut cps2::Cps2Bus, alea: &mut Alea, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    let host = rd16(b, obj + 0x46) as u32 | 0xff0000;
    let hs = (alea.next() % 9) as u8;
    wr8(b, host + 0xa2, hs);
    wr8(b, host + 0x5, 2 * (alea.next() % 4) as u8);
    wr8(b, host + 0x2, if alea.next() % 2 == 0 { 0xf } else { 0x19 });
    wr8(b, host + 0x42, (alea.next() % 2) as u8);
    for off in [0xa8u32, 0xaa, 0xac] {
        wr16(b, host + off, (0x100 + alea.next() % 0x80) as u16);
    }
    wr16(b, host + 0xa6, (alea.next() % 3) as u16);
    wr16(b, host + 0x62, (alea.next() % 40) as u16);
    if alea.next() % 4 != 0 {
        wr8(b, obj + 0xa2, hs);
    }
    let variant = rd8(b, obj + 3);
    let swings = (hs == 4 && variant == 0) || (hs == 5 && variant != 0);
    let drifts = (hs == 4 && variant != 0) || (hs == 5 && variant == 0) || ((hs == 6 && variant != 0) || (hs == 7 && variant == 0));
    let rises = matches!(hs, 6..=8) && !drifts;
    let sub = if swings || rises { 2 * (alea.next() % 4) as u8 } else if drifts { 2 * (alea.next() % 2) as u8 } else { (alea.next() % 3) as u8 * 2 };
    wr8(b, obj + 5, sub);
    wr16(b, obj + 0xae, (alea.next() % 6) as u16);
    for off in [0xb4u32, 0xb6, 0xb8] {
        wr16(b, obj + off, (alea.next() % 0x40) as u16);
    }
    wr32(b, obj + 0xb0, if alea.next() % 2 == 0 { 0x0200_0000 } else { 0 });
    wr16(b, obj + 0xc, (alea.next() % 3) as u16);
    wr16(b, obj + 0x16, (alea.next() % 0x200) as u16);
    wr16(b, obj + 0x1a, 0xffc8);
    wr8(b, obj + 0xa4, (alea.next() % 2) as u8);
    wr16(b, obj + 0x60, (alea.next() % 40) as u16);
    wr8(b, obj + 0x34, 0);
}

/// Champs de la Harpy (type 21) à des valeurs que ses tables acceptent.
fn prep_harpy(b: &mut cps2::Cps2Bus, alea: &mut Alea, obj: u32) {
    use ddsom_game::mem::{wr16, wr32, wr8};
    wr8(b, obj + 0x43, (alea.next() % 2) as u8);
    wr8(b, obj + 0x42, [0u8, 4, 2, 6][alea.next() as usize % 4]);
    for off in [0xa0u32, 0xa1, 0xa2, 0xa3, 0xa4, 0x77] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    wr8(b, obj + 0xa2, (alea.next() % 2) as u8);
    // l'angle de vol reste sous 0x40 (le dispatch par variante lit D0 laissé par la table de vitesses)
    wr8(b, obj + 0x2d, (alea.next() % 0x20) as u8);
    wr16(b, obj + 0xb0, (alea.next() % 0x200) as u16);
    wr8(b, obj + 0xb1, [0u8, 2, 6][alea.next() as usize % 3]);
    wr8(b, obj + 0xa5, (alea.next() % 2) as u8);
    wr16(b, obj + 0xae, [0x100u16, 0x280, 0xff00][alea.next() as usize % 3]);
    // le compagnon est un objet existant ; le tir (+0xa6) un autre
    wr16(b, obj + 0xaa, 0x85de + 0x200 * (alea.next() % 4) as u16);
    wr32(b, obj + 0xa6, 0xff0000 | (0x85de + 0x200 * (alea.next() % 4)));
    wr8(b, obj + 0x49, 4 * (alea.next() % 43) as u8);
}

/// Champs de l'Owlbear à des valeurs que ses tables acceptent.
fn prep_owlbear(b: &mut cps2::Cps2Bus, alea: &mut Alea, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    if s4 == 2 {
        let etape = etape_owlbear(alea, s4, s5, s6);
        wr8(b, obj + 7, etape);
    }
    wr8(b, obj + 0x49, 4 * (alea.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (alea.next() % 2) as u8);
    // souvent au ras du sol, en train de retomber : les réceptions de bonds
    if alea.next() % 2 == 0 {
        wr16(b, obj + 0xc, (alea.next() % 3) as u16);
        wr16(b, obj + 0x18, 0xff00);
        wr16(b, obj + 0x1a, 0xffc8);
    }
    for off in [0x6b, 0x6e, 0x71, 0x77, 0xa0, 0xac, 0xad, 0xaa] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0xa5, (alea.next() % 2) as u8);
    // animations voulue et en cours : souvent identiques (répétitions des coups)
    const ANIMS: [u8; 14] = [0, 2, 4, 6, 8, 0xa, 0xc, 0xe, 0x15, 0x16, 0x1a, 0x1c, 0x7f, 0xff];
    let anim = ANIMS[alea.next() as usize % ANIMS.len()];
    wr8(b, obj + 0xa1, anim);
    wr8(b, obj + 0xa2, if alea.next() % 2 == 0 { anim } else { ANIMS[alea.next() as usize % ANIMS.len()] });
    // état à reprendre après un demi-tour
    let reprises: [u32; 3] = [0x0202_0000, 0x0204_0000, 0x0206_0000];
    wr32(b, obj + 0xa6, reprises[alea.next() as usize % 3]);
    wr8(b, obj + 0xa4, 2 * (alea.next() % 3) as u8);
    wr16(b, obj + 0x50, 0x85de + 0x200 * (alea.next() % 4) as u16);
    // un hôte éventuel : orientation
    let hote = rd16(b, obj + 0x46) as u32 | 0xff0000;
    wr8(b, hote + 0x2c, 0x20 * (alea.next() % 2) as u8);
}

/// Étape `+7` valide du Displacer Beast (type 19) pour un état `(+4, +5, +6)` : celles que les tables
/// de sauts de l'original acceptent.
fn etape_dbeast(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n = match (s4, s5, s6) {
        (2, 0, 4 | 10 | 12 | 14 | 16 | 20 | 22) => 4,
        (2, 0, 6 | 18) | (2, 0, 0) => 2,
        (2, 2, _) => 2,
        (2, 4, 4 | 8) => 8,
        (2, 4, 10 | 18 | 20) => 10,
        (2, 4, 12) => 12,
        (2, 4, 16 | 30 | 36) => 14,
        (2, 4, _) => 6,
        (2, 6, 0) => 8,
        (2, 6, 2) => 6,
        (2, 6, 4 | 6 | 10) => 10,
        (2, 6, 8) => 12,
        (2, 6, 12) => 2,
        (2, 8, 0 | 6) => 4,
        (2, 8, _) => 8,
        _ => 0,
    };
    2 * (alea.next() % (n / 2 + 1)) as u8
}

/// Champs d'IA d'un ennemi à comportement complexe (type 16) à des valeurs que ses tables acceptent :
/// compteurs courts, sous-état à reprendre, état de repli, réaction au dernier coup, cible = un joueur.
fn prep_ennemi_ia(b: &mut cps2::Cps2Bus, alea: &mut Alea, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    for off in [0x6b, 0x6e, 0x71, 0x77, 0xa2, 0x90] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    // compteur de coups de patte restants : 1 au plus, et seulement au sous-sous-état 4
    if rd8(b, obj + 5) == 4 {
        let n = if rd8(b, obj + 6) == 4 { (alea.next() % 2) as u8 } else { 0 };
        wr8(b, obj + 0xa2, n);
    }
    wr8(b, obj + 0xa3, 2 * (alea.next() % 8) as u8);
    wr8(b, obj + 0xa0, alea.next() as u8);
    wr8(b, obj + 0xa1, (alea.next() % 0x20) as u8);
    wr8(b, obj + 0xac, (alea.next() % 3) as u8);
    wr8(b, obj + 0xad, (alea.next() % 2) as u8);
    wr8(b, obj + 0xae, (alea.next() % 2) as u8);
    wr8(b, obj + 0x7, 2 * (alea.next() % 3) as u8);
    wr32(b, obj + 0xa8, if alea.next() % 2 == 0 { 0x0204_0200 } else { 0x0202_0000 });
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x49, 4 * (alea.next() % 41) as u8);
    wr8(b, obj + 0x4a, 0x20 * (alea.next() % 2) as u8);
    // paramètres d'IA de l'initialisation du type 16 (masques de décision, durées)
    for (off, v) in [(0x68, 2), (0x69, 2), (0x70, 0), (0x6a, 1), (0x6c, 2), (0x6d, 4), (0x72, 2), (0x73, 2), (0x74, 2), (0x75, 0), (0x76, 3), (0x79, 6), (0x80, 7), (0x81, 6)] {
        wr8(b, obj + off, v);
    }
    wr8(b, obj + 0x72, (alea.next() % 4) as u8);
    wr8(b, obj + 0x73, (alea.next() % 4) as u8);
    // un hôte qui regarde dans un sens ou l'autre (saisie par un joueur)
    let hote = rd16(b, obj + 0x46) as u32 | 0xff0000;
    wr8(b, hote + 0x2c, 0x20 * (alea.next() % 2) as u8);
    // la cible (+0x50) est un des joueurs
    wr16(b, obj + 0x50, 0x85de + 0x200 * (alea.next() % 4) as u16);
}

/// Champs du Displacer Beast (type 19) à des valeurs que ses tables acceptent : étape `+7`, code de
/// réaction, compteurs courts, animation en cours, état à reprendre après un demi-tour.
fn prep_dbeast(b: &mut cps2::Cps2Bus, alea: &mut Alea, obj: u32) {
    use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8};
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    if s4 == 2 {
        let etape = etape_dbeast(alea, s4, s5, s6);
        wr8(b, obj + 7, etape);
    }
    wr8(b, obj + 0x49, 4 * (alea.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (alea.next() % 2) as u8);
    // souvent au ras du sol, en train de retomber : les réceptions de bonds
    if alea.next() % 2 == 0 {
        wr16(b, obj + 0xc, (alea.next() % 3) as u16);
        wr16(b, obj + 0x18, 0xff00);
        wr16(b, obj + 0x1a, 0xffc8);
    }
    for off in [0x6b, 0x6e, 0x71, 0x77, 0x90, 0xa0, 0xab, 0xae, 0xb5] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    wr8(b, obj + 0xbf, (alea.next() % 16) as u8);
    // pièce rattachée (variante 0 à 3 : rang) ou lâchée
    wr8(b, obj + 0xbc, if alea.next() % 3 == 0 { 0xff } else { (alea.next() % 8) as u8 });
    // état à reprendre après un demi-tour : un état actif valide
    let reprises: [u32; 4] = [0x0202_0000, 0x0204_0000, 0x0206_0000, 0x0208_0004];
    wr32(b, obj + 0xa6, reprises[alea.next() as usize % 4]);
    wr8(b, obj + 0xa4, 2 * (alea.next() % 3) as u8);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    // animation en cours : souvent celle que l'étape voudra lancer (répétitions des coups)
    const ANIMS: [u8; 22] = [0, 5, 6, 7, 0xc, 0xd, 0xe, 0xf, 0x16, 0x18, 0x19, 0x1c, 0x1d, 0x1e, 0x28, 0x2a, 0x2b, 0x2d, 0x2e, 0x2f, 0x7f, 0xff];
    wr8(b, obj + 0xa2, ANIMS[alea.next() as usize % ANIMS.len()]);
    wr8(b, obj + 0xaf, ANIMS[alea.next() as usize % ANIMS.len()]);
    // la cible est un joueur
    wr16(b, obj + 0x50, 0x85de + 0x200 * (alea.next() % 4) as u16);
    // le parent (+0x42) : état, variante, indice d'animation, compteur de bras
    let parent = rd16(b, obj + 0x42) as u32 | 0xff0000;
    if rd16(b, obj + 0x42) != 0 {
        const ETATS: [u32; 6] = [0x0402_0000, 0x0402_0c00, 0x0200_0600, 0x0202_0000, 0x0402_0200, 0x0408_0000];
        wr32(b, parent + 4, ETATS[alea.next() as usize % 6]);
        wr8(b, parent + 3, (alea.next() % 3) as u8);
        wr8(b, parent + 0xa1, (alea.next() % 0x20) as u8);
        wr8(b, parent + 0xaa, (alea.next() % 3) as u8);
        wr8(b, parent + 0x2c, 0x20 * (alea.next() % 2) as u8);
    }
}

/// États du maître d'un ennemi de type 26 (+0xb8 à +0xbb, plus +4/+5 et le code de réaction +0x49) que
/// les tables de sauts de l'original couvrent.
fn etat_maitre_26(b: &mut cps2::Cps2Bus, alea: &mut Alea, maitre: u32) {
    use ddsom_game::mem::{wr16, wr32, wr8};
    let pair = |alea: &mut Alea, n: u32| 2 * (alea.next() % n) as u8;
    let groupe = pair(alea, 4);
    let (mode, phase, etape);
    if groupe == 4 {
        mode = pair(alea, 6);
        (phase, etape) = match mode {
            // les routines de réaction ne sont sûres qu'en leur état de départ
            0 => (0, 0),
            2 => (pair(alea, 3), 0),
            4 => (pair(alea, 5), 0),
            _ => (pair(alea, 4), 0),
        };
    } else {
        mode = pair(alea, 8);
        (phase, etape) = match mode {
            0 => (pair(alea, 8), 0),
            2 => {
                let ph = pair(alea, 6);
                (ph, if ph >= 4 { pair(alea, 5) } else { 0 })
            }
            4 => {
                let ph = pair(alea, 6);
                (ph, if ph == 8 { pair(alea, 3) } else if ph == 10 { pair(alea, 5) } else { 0 })
            }
            _ => (0, 0),
        };
    }
    wr8(b, maitre + 0xb8, groupe);
    wr8(b, maitre + 0xb9, mode);
    wr8(b, maitre + 0xba, phase);
    wr8(b, maitre + 0xbb, etape);
    wr8(b, maitre + 0x33, (alea.next() % 2) as u8);
    // code de réaction : multiple de 4 dans la table (45 entrées)
    wr8(b, maitre + 0x49, 4 * (alea.next() % 45) as u8);
    for off in [0x14u32, 0x18, 0x1c] {
        wr32(b, maitre + off, (alea.next() % 0x400) as u32);
    }
    wr16(b, maitre + 0x16, (alea.next() % 0x40) as u16);
    // état du maître : 0x406 (en cours de coup) déclenche la disparition du double
    wr16(b, maitre + 4, if alea.next() % 12 == 0 { 0x406 } else { 0x200 });
}

/// États du maître d'un ennemi de type 24 (octets +4 à +7) que les tables de sauts de l'original couvrent.
fn etat_maitre_24(b: &mut cps2::Cps2Bus, alea: &mut Alea, maitre: u32) {
    use ddsom_game::mem::wr8;
    let pair = |alea: &mut Alea, n: u32| 2 * (alea.next() % n) as u8;
    let etat = pair(alea, 4);
    let (sub, s6, s7);
    if etat == 4 {
        sub = pair(alea, 6);
        s6 = pair(alea, 3);
        s7 = pair(alea, 3);
    } else {
        sub = pair(alea, 7);
        s6 = match sub {
            0 => pair(alea, 3),
            2 => pair(alea, 6),
            _ => pair(alea, 7),
        };
        s7 = match (sub, s6) {
            (4, 0) => pair(alea, 2),
            (4, 8) => pair(alea, 7),
            (4, 2 | 4 | 6) | (2, 10) => pair(alea, 5),
            (2, 8) => pair(alea, 3),
            (4, 10) => pair(alea, 5),
            (4, 12) => pair(alea, 6),
            _ => pair(alea, 3),
        };
    }
    wr8(b, maitre + 4, etat);
    wr8(b, maitre + 5, sub);
    wr8(b, maitre + 6, s6);
    wr8(b, maitre + 7, s7);
    wr8(b, maitre + 0xad, (alea.next() % 3 == 0) as u8);
}

fn etats_eshl(kind: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
    const T: &[u8] = &[2];
    const TQ: &[u8] = &[2, 4];
    const Z: &[u8] = &[0];
    const Z2: &[u8] = &[0, 2];
    match kind {
        15 => &[(T, &[0], &[0, 2, 4, 6, 8, 10]), (T, &[2], Z), (T, &[4], &[0, 2, 4]), (&[4], Z, Z)],
        16 => &[(TQ, Z2, Z)],
        17 => &[(T, Z, Z), (T, &[2], Z2), (T, &[4], Z), (&[4], Z, Z)],
        18 | 24 | 25 | 28 | 38 => &[(TQ, Z, Z)],
        19 => &[(TQ, Z, Z)],
        20 => &[(T, Z, Z2), (T, &[2], &[0, 2, 4, 6])],
        26 => &[(T, &[0, 2, 4], Z), (&[4], Z, Z)],
        30 => &[(TQ, &[0, 2, 4, 6], Z)],
        32 | 39 => &[(T, &[0, 2, 4], Z)],
        33 => &[(T, Z2, Z), (T, &[4], Z2)],
        21 | 22 | 34 | 37 => &[(T, Z2, Z)],
        _ => &[(TQ, Z2, Z)],
    }
}

/// Perturbe les variables globales que lisent les contrôleurs SET / SOBJ : mode de jeu, nombre de
/// joueurs, caméras, rang, et pour chaque joueur son état, sa vie, ses touches et sa position
/// autour de l'objet `obj`.
fn perturber_globales(b: &mut cps2::Cps2Bus, alea: &mut Alea, pas: u32, obj: u32) {
    use ddsom_game::mem::{rd16, wr8};
    if pas != 0 && alea.next() % 4 != 0 {
        return;
    }
    // octets de la RAM de travail (A5+) et valeurs possibles
    let octets: [(u32, &[u8]); 16] = [
        (0x2c, &[0xc, 0xc, 0xc, 0, 8]),
        (0x30, &[0, 1, 1, 1]),
        (0x34, &[0, 1, 4, 6, 7, 0x1c, 0x1f, 0x2b, 0x45, 0x50]),
        (0x35, &[0, 1, 2, 3, 4]),
        (0x36, &[0, 0, 1]),
        (0x42, &[0, 0, 0, 1]),
        (0x4e, &[0, 1, 4]),
        (0x50, &[0, 0, 0, 1]),
        (0x51, &[0, 1, 2]),
        (0x58, &[0, 0, 1]),
        (0x59, &[0, 1]),
        (0x6f, &[0, 1, 7, 8, 0x10]),
        (0x48e, &[0, 1, 0xc, 0xf]),
        (0x48f, &[0, 1, 7]),
        (0x53a, &[0, 0, 0, 1]),
        (0x70, &[0, 0, 1]),
    ];
    for (off, valeurs) in octets {
        if alea.next() % 3 == 0 {
            wr8(b, A5 + off, valeurs[alea.next() as usize % valeurs.len()]);
        }
    }
    // caméras, rang
    for off in [0x506u32, 0x556] {
        if alea.next() % 2 == 0 {
            wr16(b, A5 + off, [0, 0x100, 0x180, 0x1c0, 0x200, 0x2b0, 0x300, 0xa00][alea.next() as usize % 8]);
        }
    }
    for off in [0x50au32, 0x55a] {
        if alea.next() % 2 == 0 {
            wr16(b, A5 + off, (alea.next() % 0x200) as u16);
        }
    }
    if alea.next() % 3 == 0 {
        wr16(b, A5 + 0x7f92, (alea.next() % 32) as u16);
    }
    // joueurs : actif, état, vie, touches, position proche de l'objet
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if alea.next() % 3 == 0 {
            wr8(b, p, [0, 1, 1, 1, 2][alea.next() as usize % 5]);
        }
        if alea.next() % 4 == 0 {
            wr8(b, p + 4, [0, 1, 2, 2, 3, 4][alea.next() as usize % 6]);
            wr8(b, p + 5, 0);
            wr8(b, p + 6, 0);
        }
        if alea.next() % 3 == 0 {
            wr16(b, p + 0x62, [0, 0x08, 0x20, 0x40, 0x80][alea.next() as usize % 5]);
        }
        if alea.next() % 3 == 0 {
            wr8(b, p + 0x6c, alea.next() as u8);
            wr8(b, 0xff800c + j, alea.next() as u8 & 0xf0);
        }
        if alea.next() % 3 == 0 {
            let x = rd16(b, obj + 8).wrapping_add((alea.next() % 0x60) as u16).wrapping_sub(0x30);
            let y = rd16(b, obj + 0x10).wrapping_add((alea.next() % 0x30) as u16).wrapping_sub(0x18);
            wr16(b, p + 8, x);
            wr16(b, p + 0x10, y);
            wr16(b, p + 0xc, if alea.next() % 4 == 0 { 0x30 } else { 0 });
        }
        if alea.next() % 4 == 0 {
            wr8(b, p + 3, (alea.next() % 12) as u8);
        }
    }
}

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
    let graine: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut alea = Alea(0x1234_5678_9abc_def1 ^ graine.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let tables: [(&Pool, u32, u32); 4] = [(&ESHL, 0x39b96, 64), (&SET, 0x39eac, 34), (&SOBJ, 0x39fa8, 16), (&ENEMY, 0x39a78, 29)];
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
    let duree: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
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
            if pool.class == 2 && (matches!(kind, 4 | 19 | 21 | 25) || lot3::gere(kind)) {
                // un propriétaire distinct de l'objet : ses états sont réécrits au hasard par la suite
                proprietaires.retain(|&o| o != obj.0);
            }
            let mut owner = proprietaires[alea.next() as usize % proprietaires.len()];
            if pool.class == 2 {
                // le maître n'est pas l'objet lui-même : on prend un autre ennemi, sinon un joueur
                proprietaires.retain(|&o| o != obj.0 && o >= ENEMY.slot(0).0 && o < ENEMY.slot(ENEMY.slots).0);
                if proprietaires.is_empty() {
                    // aucun autre ennemi en scène : on en fabrique un, inactif à tous égards
                    let autre = ENEMY.slot((alea.next() % ENEMY.slots).min(ENEMY.slots - 1));
                    let autre = if autre.0 == obj.0 { ENEMY.slot(0) } else { autre };
                    let autre = if autre.0 == obj.0 { ENEMY.slot(1) } else { autre };
                    for off in (0..ENEMY.stride).step_by(2) {
                        wr16(&mut b, autre.0 + off, 0);
                    }
                    wr8(&mut b, autre.0, 1);
                    wr8(&mut b, autre.0 + 2, 25);
                    wr8(&mut b, autre.0 + 0x28, 2);
                    proprietaires.push(autre.0);
                }
                owner = proprietaires[alea.next() as usize % proprietaires.len()];
            }
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, (alea.next() % variantes) as u8);
            if pool.class == 2 && kind == 16 {
                wr8(&mut b, obj.0 + 3, 2 * (alea.next() % 6) as u8);
            }
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, (alea.next() & 1) as u8);
            wr8(&mut b, obj.0 + 0x2f, if alea.next() % 4 == 0 { alea.next() as u8 % 3 } else { 0 });
            if pool.class == 2 && kind == 21 {
                // la série d'attaque indexe des tables de mots : 0, 2 ou 4
                let serie = 2 * (rd8(&mut b, obj.0 + 0x2f) % 3);
                wr8(&mut b, obj.0 + 0x2f, serie);
            }
            if pool.class == 2 && kind == 19 {
                // le profil d'IA n'a que deux valeurs (la table de sauts de l'original n'en a pas d'autre)
                let profil = rd8(&mut b, obj.0 + 0x2f) % 2;
                wr8(&mut b, obj.0 + 0x2f, profil);
            }
            wr16(&mut b, obj.0 + 0x50, owner as u16);
            wr16(&mut b, obj.0 + 0x46, owner as u16);
            wr16(&mut b, obj.0 + 0x5a, owner as u16);
            // ESHL 15..39 : créateur en +0x5c, hôte en +0x42, pointeurs longs (types 17 et 19), délais,
            // variables de niveau lues par les tirs
            if pool.class == 8 && kind >= 15 {
                wr16(&mut b, obj.0 + 0x5c, owner as u16);
                if alea.next() % 2 == 0 {
                    wr16(&mut b, obj.0 + 0x42, owner as u16);
                }
                wr16(&mut b, obj.0 + 0x58, (alea.next() % 6) as u16);
                if kind == 17 || kind == 19 {
                    wr32(&mut b, obj.0 + 0x50, owner);
                    wr32(&mut b, obj.0 + 0x56, owner);
                    wr8(&mut b, obj.0 + 0x43, [0, 1, 0xff][alea.next() as usize % 3]);
                    wr8(&mut b, obj.0 + 0x42, 4 * (alea.next() % 2) as u8);
                }
                if std::env::var("DDSOM_ETATS").is_ok() && alea.next() % 2 == 0 {
                    let boites = etats_eshl(kind);
                    let (s4, s5, s6) = boites[alea.next() as usize % boites.len()];
                    wr8(&mut b, obj.0 + 4, s4[alea.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[alea.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[alea.next() as usize % s6.len()]);
                }
                // le plus souvent, le propriétaire est à l'écran (sinon le tir est aussitôt retiré par le culling)
                if alea.next() % 4 != 0 {
                    let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
                    wr16(&mut b, owner + 8, cx.wrapping_add(0x60 + (alea.next() % 0x80) as u16));
                    wr16(&mut b, owner + 0x10, cy.wrapping_add(0x60 + (alea.next() % 0x40) as u16));
                }
                wr16(&mut b, A5 + 0x7f92, (alea.next() % 8) as u16);
                wr8(&mut b, A5 + 0x35, (alea.next() % 5) as u8);
                wr8(&mut b, A5 + 0x36, (alea.next() % 2) as u8);
            }
            // ENEMY : hôte en +0x42, PV, états tirés (DDSOM_ETATS), joueurs proches
            if pool.class == 2 {
                wr16(&mut b, obj.0 + 0xb2, owner as u16);
                if kind == 26 {
                    wr16(&mut b, obj.0 + 0xb0, owner as u16);
                    etat_maitre_26(&mut b, &mut alea, owner);
                } else if matches!(kind, 22..=24) {
                    etat_maitre_24(&mut b, &mut alea, owner);
                }
                if alea.next() % 2 == 0 {
                    wr16(&mut b, obj.0 + 0x42, owner as u16);
                }
                wr16(&mut b, obj.0 + 0x62, (alea.next() % 40) as u16);
                // hôte d'une partie composite (type 7) : son type, ses états et son dernier coup
                if kind == 7 {
                    wr8(&mut b, owner + 2, [0xe, 0x13, 5][alea.next() as usize % 3]);
                    wr8(&mut b, owner + 0xa2, (alea.next() % 7) as u8);
                    wr8(&mut b, owner + 0x3e, if alea.next() % 3 == 0 { 0xff } else { 0 });
                    wr8(&mut b, owner + 0x2c, (alea.next() % 2) as u8 * 0x20);
                }
                // suivi de vie des doubles (type 23) : PV minimum connu, délai, drapeau « blessé »
                wr16(&mut b, obj.0 + 0x64, (alea.next() % 50) as u16);
                wr16(&mut b, obj.0 + 0xb4, (alea.next() % 3) as u16);
                wr8(&mut b, obj.0 + 0xaf, (alea.next() % 3 == 0) as u8);
                wr8(&mut b, obj.0 + 0xb6, (alea.next() % 3 == 0) as u8);
                // parties liées (halos, flammes, bras, ombre) : objets existants tirés au hasard
                if kind == 20 && alea.next() % 4 != 0 {
                    for off in [0xa4u32, 0xa6, 0xa8, 0xaa, 0xac, 0xae, 0xb4] {
                        wr16(&mut b, obj.0 + off, proprietaires[alea.next() as usize % proprietaires.len()] as u16);
                    }
                    wr8(&mut b, obj.0 + 0xb0, (alea.next() % 4) as u8);
                }
                wr16(&mut b, obj.0 + 0x60, 40);
                if std::env::var("DDSOM_ETATS").is_ok() && alea.next() % 2 == 0 && !lot3::gere(kind) {
                    let boites = etats_enemy(kind, rd8(&mut b, obj.0 + 3));
                    let (s4, s5, s6) = boites[alea.next() as usize % boites.len()];
                    wr8(&mut b, obj.0 + 4, s4[alea.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[alea.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[alea.next() as usize % s6.len()]);
                }
                if kind == 16 {
                    prep_ennemi_ia(&mut b, &mut alea, obj.0);
                }
                if kind == 19 {
                    prep_dbeast(&mut b, &mut alea, obj.0);
                }
                if kind == 4 {
                    prep_owlbear(&mut b, &mut alea, obj.0);
                }
                if kind == 21 {
                    prep_harpy(&mut b, &mut alea, obj.0);
                }
                if kind == 25 {
                    prep_warrior(&mut b, &mut alea, obj.0);
                }
                if alea.next() % 4 != 0 {
                    let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
                    wr16(&mut b, obj.0 + 8, cx.wrapping_add(0x60 + (alea.next() % 0x80) as u16));
                    wr16(&mut b, obj.0 + 0x10, cy.wrapping_add(0x60 + (alea.next() % 0x40) as u16));
                    wr16(&mut b, owner + 8, cx.wrapping_add(0x60 + (alea.next() % 0x80) as u16));
                    wr16(&mut b, owner + 0x10, cy.wrapping_add(0x60 + (alea.next() % 0x40) as u16));
                }
            }
            if entree == 0xd5f5a {
                // barre de vie (SET 20) : joueur suivi en +0x20, second joueur en +0x34
                wr16(&mut b, obj.0 + 0x20, 0x85de + 0x200 * (alea.next() % 4) as u16);
                wr16(&mut b, obj.0 + 0x34, 0x85de + 0x200 * (alea.next() % 4) as u16);
            }
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, owner + off) as u32) << 16 | (alea.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            wr16(&mut b, obj.0 + 0xc, (alea.next() % 0x60) as u16);
            // décompte des places libres cohérent
            // (au moment du chauffage le pool peut être plein : les gestionnaires testent ce compteur)
            wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
            if pool.class == SET.class && matches!(kind, 2 | 15) {
                etat_scene(&mut b, &mut alea, obj.0, kind, true);
            }
            for pas in 0..duree {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if pas > 0 {
                    if pool.class == SET.class && matches!(kind, 2 | 15) && alea.next() % 6 == 0 {
                        etat_scene(&mut b, &mut alea, obj.0, kind, false);
                    }
                    if alea.next() % 6 == 0 {
                        wr8(&mut b, obj.0 + 0x57, [0x08, 0x20, 0x28, 0x10, 0x02, 0x40][alea.next() as usize % 6]);
                    }
                    if alea.next() % 8 == 0 {
                        wr8(&mut b, obj.0 + 0x33, alea.next() as u8 & 1);
                    }
                    if alea.next() % 10 == 0 && !(pool.class == 2 && matches!(kind, 22..=24 | 26)) {
                        wr8(&mut b, owner + 4, (alea.next() % 8) as u8);
                    }
                    if pool.class == 2 && matches!(kind, 22..=24 | 26) && alea.next() % 3 == 0 {
                        if kind == 26 {
                            etat_maitre_26(&mut b, &mut alea, owner);
                        } else {
                            etat_maitre_24(&mut b, &mut alea, owner);
                        }
                    }
                    // le propriétaire change d'état : états de l'automate, drapeaux d'attaque, compteurs
                    if alea.next() % 3 == 0 && !(pool.class == 2 && matches!(kind, 22..=24 | 26)) {
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
                if pool.class == 2 {
                    // compteurs de places libres de tous les pools, cohérents avec leur contenu (les ennemis
                    // composites les lisent pour savoir s'ils peuvent créer leurs parties)
                    for q in ddsom_game::objects::pools::POOLS {
                        let libres = (0..q.slots).filter(|&i| rd8(&mut b, q.slot(i).0) == 0).count();
                        wr16(&mut b, A5 + q.free, libres as u16);
                    }
                }
                if pool.class == 2 && alea.next() % 5 == 0 {
                    // difficulté, scène et joueurs en vie lus par les tables de décision
                    wr16(&mut b, A5 + 0x7f92, (alea.next() % 8) as u16);
                    wr8(&mut b, A5 + 0x34, if alea.next() % 4 == 0 { 4 } else { (alea.next() % 12) as u8 });
                    wr8(&mut b, A5 + 0x35, (alea.next() % 5) as u8);
                }
                if pool.class == 2 && alea.next() % 2 == 0 {
                    // joueurs actifs, tantôt près de l'ennemi tantôt loin
                    for j in 0..4u32 {
                        let q = 0xff85de + 0x200 * j;
                        if alea.next() % 3 != 0 {
                            wr8(&mut b, q, 1);
                        }
                        let x = rd16(&mut b, obj.0 + 8).wrapping_add((alea.next() % 0x90) as u16).wrapping_sub(0x48);
                        let y = rd16(&mut b, obj.0 + 0x10).wrapping_add((alea.next() % 0x40) as u16).wrapping_sub(0x20);
                        wr16(&mut b, q + 8, x);
                        wr16(&mut b, q + 0x10, y);
                        wr16(&mut b, q + 0xc, (alea.next() % 0x30) as u16);
                    }
                }
                if pool.class == 2 && lot3::gere(kind) && (pas == 0 || alea.next() % 3 == 0) {
                    // état et champs valides du troisième lot, tirés à nouveau de temps en temps
                    lot3::prep(&mut b, &mut alea, kind, obj.0, pas == 0);
                }
                if pool.class == 2 && matches!(kind, 4 | 19 | 21 | 25) && pas > 0 && alea.next() % 3 == 0 {
                    // nouvel état valide à chaque fois : visite beaucoup plus d'étapes que la seule trajectoire
                    let boites = etats_enemy(kind, rd8(&mut b, obj.0 + 3));
                    let (s4, s5, s6) = boites[alea.next() as usize % boites.len()];
                    wr8(&mut b, obj.0 + 4, s4[alea.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[alea.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[alea.next() as usize % s6.len()]);
                    if kind == 25 {
                        prep_warrior(&mut b, &mut alea, obj.0);
                    } else if kind == 21 {
                        prep_harpy(&mut b, &mut alea, obj.0);
                    } else if kind == 4 {
                        prep_owlbear(&mut b, &mut alea, obj.0);
                    } else {
                        prep_dbeast(&mut b, &mut alea, obj.0);
                    }
                }
                if pool.class == 2 && (matches!(kind, 4 | 19) || lot3::gere(kind)) && alea.next() % 2 == 0 {
                    // des joueurs dans les boîtes de portée de la bête (largeur étroite, profondeur ±8)
                    for j in 0..4u32 {
                        let q = 0xff85de + 0x200 * j;
                        if alea.next() % 4 != 0 {
                            wr8(&mut b, q, 1);
                        }
                        let dx = (alea.next() % 0x180) as u16;
                        let dy = (alea.next() % 0x12) as u16;
                        let (ox, oy) = (rd16(&mut b, obj.0 + 8), rd16(&mut b, obj.0 + 0x10));
                        wr16(&mut b, q + 8, ox.wrapping_add(dx).wrapping_sub(0x100));
                        wr16(&mut b, q + 0x10, oy.wrapping_add(dy).wrapping_sub(9));
                        wr16(&mut b, q + 0xc, (alea.next() % 4) as u16);
                    }
                }
                if pool.class == 2 && kind == 7 && alea.next() % 4 == 0 {
                    wr8(&mut b, owner + 0xa2, (alea.next() % 7) as u8);
                    wr8(&mut b, owner + 0x3e, if alea.next() % 3 == 0 { 0xff } else { 0 });
                    wr8(&mut b, owner + 0xa3, (alea.next() % 2) as u8);
                    if alea.next() % 5 == 0 {
                        wr8(&mut b, owner, 0);
                    }
                }
                if pool.class == 2 && pas > 0 && !lot3::gere(kind) {
                    for off in [0x33u32, 0x53, 0x56, 0x57, 0xa2, 0xa3, 0xa4, 0xb0, 0xb2, 0xb6, 0x20, 0x21, 0x24, 0x26, 0x30, 0x32, 0x3e] {
                        // +0x24 et +0x26 sont les liens des listes triées : seul le type 20 les
                        // utilise comme compteurs (parent de type 19), on ne les corrompt que pour lui
                        let liens = matches!(off, 0x20 | 0x24 | 0x26) || (kind == 16 && matches!(off, 0xa2 | 0xa3)) || (kind == 19 && off == 0xa4) || (kind == 4 && matches!(off, 0xa4 | 0xa3)) || (kind == 21 && off == 0xa2);
                        if alea.next() % 9 == 0 && !(matches!(kind, 22..=24 | 26) && matches!(off, 0xb0 | 0xb2)) && !(liens && (kind != 20 || off == 0xa3)) {
                            let v = match alea.next() % 4 {
                                0 => 0,
                                1 => 1,
                                2 => (alea.next() % 4) as u8,
                                _ => alea.next() as u8,
                            };
                            wr8(&mut b, obj.0 + off, v);
                            // l'état de l'hôte d'une partie (type 7) reste dans la table de la ROM
                            if alea.next() % 3 == 0 && !(kind == 7 && matches!(off, 0xa2 | 0xa3)) && !(kind == 25 && off == 0xa2) {
                                wr8(&mut b, owner + off, v);
                            }
                        }
                    }
                    if alea.next() % 15 == 0 {
                        wr16(&mut b, obj.0 + 0x62, (alea.next() % 5) as u16);
                        wr16(&mut b, obj.0 + 0x64, (alea.next() % 50) as u16);
                    }
                }
                // SET et SOBJ : contrôleurs de scène et d'interface, qui lisent des variables globales
                // (mode de jeu, nombre de joueurs, caméras, joueurs) : on les tire aussi
                if pool.class == 0x10 || pool.class == 0x14 {
                    perturber_globales(&mut b, &mut alea, pas, obj.0);
                }
                // jeu en pause (rare) ou non ; objet suspendu (octet +0x40) très rarement
                wr8(&mut b, A5 + 0x3d, (alea.next() % 10 == 0) as u8);
                if pool.class == 8 && kind >= 15 {
                    wr8(&mut b, A5 + 0x6f, pas as u8);
                    wr16(&mut b, A5 + 0x6e, alea.next() as u16);
                    wr8(&mut b, A5 + 0x42, (alea.next() % 2) as u8);
                    wr8(&mut b, A5 + 0x4e, (alea.next() % 16 == 0) as u8);
                }
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
                if std::env::var("DDSOM_MAITRE").is_ok() && pool.class == 2 {
                    let m = rd16(&mut b, obj.0 + 0xb2) as u32 | 0xff0000;
                    let o: Vec<u8> = (0..8).map(|i| rd8(&mut b, m + i)).collect();
                    println!("pas {pas} maître {m:06x} : {o:02x?} obj {:06x}", obj.0);
                }
                let avant_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                if std::env::var("DDSOM_PAS_ETAT").is_ok() {
                    println!("pas {pas} variante {:02x} états {:02x?} +2f {:02x}", avant_obj[3], &avant_obj[4..8], avant_obj[0x2f]);
                }
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
                if std::env::var("DDSOM_PAS_ETAT").is_ok() {
                    let apres: Vec<u8> = (4..8).map(|i| rd8(&mut b, obj.0 + i)).collect();
                    println!("   après : {apres:02x?}");
                }
                if r.is_err() {
                    // l'original est sorti des rails (entrée incohérente) : on abandonne la trajectoire
                    let _ = (cs, bs);
                    break;
                }
                if std::env::var("DDSOM_MAITRE").is_ok() && pool.class == 2 {
                    let m = rd16(&mut b, obj.0 + 0xb2) as u32 | 0xff0000;
                    let o: Vec<u8> = (0..8).map(|i| rd8(&mut b, m + i)).collect();
                    println!("   après : {o:02x?}");
                }
                let apres = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                if apres > avant && affiches < 12 {
                    affiches += 1;
                    let dernier = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                    println!("écart en {entree:06x} type {kind} pas {pas} obj {:06x} : {}", obj.0, dernier.unwrap_or_default());
                    println!("  objet avant : {:02x?}", avant_obj);
                    if pool.class == 2 {
                        let m = ((avant_obj[0xb2] as u32) << 8 | avant_obj[0xb3] as u32) | 0xff0000;
                        let octets: Vec<u8> = (0..8).map(|i| rd8(&mut bs.clone(), m + i)).collect();
                        println!("  maître {m:06x} avant : {octets:02x?}");
                    }
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        // exécution pas à pas du code d'origine seul depuis l'état d'avant
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        let sp = c2.a[7];
                        c2.pc = entree;
                        c2.a[7] = sp.wrapping_sub(4);
                        ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                        for _ in 0..DDSOM_TRACE_LEN {
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

/// Contrôleur de scène (SET 2) et écrans de fin de niveau (SET 15) : ils dépendent de variables de
/// partie (niveau, difficulté, scène figée...), de la variante (script) et de l'état des joueurs,
/// que le fuzz générique ne fait pas varier. `depart` : état initial de l'objet (variante, état).
fn etat_scene(b: &mut Bus, alea: &mut Alea, obj: u32, kind: u8, depart: bool) {
    use ddsom_game::mem::{rd16, rd32, rd8};
    wr8(b, A5 + 0x34, if alea.next() % 3 == 0 { 4 } else { 1 + (alea.next() % 7) as u8 });
    wr8(b, A5 + 0x35, if alea.next() % 10 == 0 { 0 } else { 1 + (alea.next() % 4) as u8 });
    wr8(b, A5 + 0x2c, [0xc, 4, 8, 0, 0x18][alea.next() as usize % 5]);
    wr8(b, A5 + 0x42, (alea.next() % 8 == 0) as u8);
    wr8(b, A5 + 0x36, (alea.next() % 4 == 0) as u8);
    wr8(b, A5 + 0x50, (alea.next() % 2) as u8);
    wr8(b, A5 + 0x66, (alea.next() % 2) as u8);
    wr16(b, A5 + 0x7f92, (alea.next() % 6) as u16);
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if alea.next() % 3 == 0 {
            wr16(b, p + 4, [0x200, 0x200, 0, 0x600, 0x204][alea.next() as usize % 5]);
            wr8(b, p + 6, (alea.next() % 3 == 0) as u8);
            wr16(b, p + 0x62, [0, 5, 100][alea.next() as usize % 3]);
        }
    }
    if kind == 15 {
        // variables lues par les écrans de fin : fondu terminé, caméras, score, rng, objets du joueur
        wr8(b, A5 + 0x48f, (alea.next() % 3 == 0) as u8);
        wr8(b, A5 + 0x6d, alea.next() as u8);
        wr16(b, A5 + 0x6e, alea.next() as u16 & [0xf, 0xff, 0][alea.next() as usize % 3]);
        for off in [0x506u32, 0x50a, 0x556, 0x55a] {
            wr16(b, A5 + off, [0, 0, 1, 0x70, alea.next() as u16 & 0x1ff][alea.next() as usize % 5]);
        }
        wr16(b, A5 + 0x38, [0x80, 0x120, 0x1f0, 0x220, 0x300][alea.next() as usize % 5]);
        wr8(b, A5 + 0x5d, (alea.next() % 2) as u8);
        for j in 0..4u32 {
            let p = 0xff85de + 0x200 * j;
            wr8(b, p + 0x18a, (alea.next() % 2) as u8);
            wr8(b, p + 3, (alea.next() % 6) as u8);
            wr8(b, p + 0x22, (alea.next() % 2) as u8);
            wr16(b, p + 0x60, [50, 100, 160][alea.next() as usize % 3]);
            wr16(b, p + 0x64, (alea.next() % 20) as u16);
            if alea.next() % 4 == 0 {
                wr8(b, p + 0x118, [0xff, 0xf9, 0xfa, 0x10, 0][alea.next() as usize % 5]);
            }
        }
        if depart {
            if alea.next() % 4 == 0 {
                wr8(b, obj + 0x29, 0x1f);
                wr16(b, obj + 0x24, 1 + (alea.next() % 2) as u16);
            }
            for off in [0x14u32, 0x18, 0x22, 0x23, 0x24, 0x2a, 0x2b, 0x30, 0x31, 0x42, 0x43] {
                if alea.next() % 3 == 0 {
                    wr8(b, obj + off, [0, 1, 2, 3, alea.next() as u8][alea.next() as usize % 5]);
                }
            }
        }
    }
    if depart && kind == 15 {
        // écrans de fin : variante, états, chef d'orchestre (+0x4c, un autre objet SET)
        wr8(b, obj + 3, (alea.next() % 25) as u8);
        wr8(b, obj + 4, [0u8, 0, 2, 2, 4, 6][alea.next() as usize % 6]);
        // sous-états valides de chaque variante (les autres font sauter l'original dans des données)
        let variante = rd8(b, obj + 3);
        let sous_etats = match variante {
            1 => 5,
            9 | 10 | 11 | 17 => 3,
            7 | 14 | 16 => 2,
            18 | 22 => 9,
            _ => 1,
        };
        wr8(b, obj + 5, 2 * (alea.next() % sous_etats) as u8);
        // joueur visé par les variantes 18 et 22, objet cible de la 17
        let joueur = 0xff85de + 0x200 * (alea.next() % 4);
        wr16(b, obj + 0x20, joueur as u16);
        wr16(b, obj + 0x26, joueur as u16);
        wr16(b, obj + 0x46, joueur as u16);
        // fenêtre de dialogue (variante 7) : joueur visé, message à composer, tampon de texte
        wr16(b, obj + 0x34, joueur as u16);
        wr32(b, obj + 8, 0xcec00 + 2 * (alea.next() % 0x400));
        wr32(b, obj + 0xc, 0xff2072);
        wr8(b, A5 + 0x73, [2, 1, 0][alea.next() as usize % 3]);
        wr16(b, A5 + 0x72, 2 + (alea.next() % 3) as u16);
        let chef = if obj == 0xffca9e { 0xffcaee } else { 0xffca9e };
        wr32(b, obj + 0x4c, chef);
        wr8(b, chef + 8, [0xff, 0, 1, 2, 3][alea.next() as usize % 5]);
        wr8(b, chef + 3, [0, 2, 3, 1][alea.next() as usize % 4]);
        wr8(b, chef + 0x3f, (alea.next() % 4) as u8);
        wr8(b, chef + 0xc, (alea.next() % 2) as u8);
        wr8(b, obj + 0x2f, (alea.next() % 8) as u8);
        return;
    }
    if !depart || kind != 2 {
        return;
    }
    let variante = alea.next() % 0x78;
    wr8(b, obj + 3, variante as u8);
    let script = rd32(b, 0xbd276 + 4 * variante);
    let etat = [0u8, 0, 2, 2, 2, 4, 4, 6][alea.next() as usize % 8];
    wr8(b, obj + 4, etat);
    if etat == 2 {
        wr16(b, obj + 0x18, 0xffff);
        let premier = rd16(b, script);
        wr16(b, obj + 0x24, premier);
        wr32(b, obj + 0x20, script + 2);
    } else if etat == 4 {
        wr16(b, obj + 0x24, (alea.next() % 4) as u16);
        wr16(b, obj + 0x2a, (alea.next() % 4 == 0) as u16);
        if alea.next() % 2 == 0 {
            // rideau (action de fin 6) : déjà armé, décompte court, curseur dans la table
            wr8(b, obj + 5, 2);
            wr16(b, obj + 0x14, (alea.next() % 3) as u16);
            wr16(b, obj + 0x26, 6 * (alea.next() % 9) as u16);
        }
    }
}
