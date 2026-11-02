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
//! camera (recopie du décor par les caméras 1 et 2, 0x2f81c 0x2f86e 0x328a8, et décors à parallaxe 0x33178...),
//! createurs (créateurs d'objets du lot B : `effects_creators`, `creators`, `petites`, `effects_aids`).

use std::path::Path;

use ddsom_game::mem::{wr16, wr32, wr8, A5};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    /// Vrai avec la probabilité 1/n.
    fn one_chance_in(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

/// Registres quelconques ; A0 sur un enregistrement joueur.
fn regs(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    for r in 0..8 {
        c.d[r] = if rng.next() & 3 == 0 { rng.next() & 0xff } else { rng.next() };
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (rng.next() & 0xffff);
    }
    let obj = 0xff85de + 0x200 * (rng.next() % 4);
    for o in [2u32, 3, 0x7e] {
        wr8(b, obj + o, rng.next() as u8);
    }
    wr16(b, obj + 0x48, rng.next() as u16);
    wr16(b, obj + 8, rng.next() as u16 & 0x7ff);
    c.a[0] = obj;
}

fn rng_sound(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    wr8(b, A5 + 0x27, rng.next() as u8 & 1);
    wr8(b, A5 + 0x30, rng.next() as u8 & 1);
    wr8(b, A5 + 0x28, rng.next() as u8 & 1);
    wr8(b, A5 + 0x36, rng.next() as u8 & 1);
    wr16(b, A5 + 0x7fcc, rng.one_chance_in(3) as u16);
    wr16(b, A5 + 0x7fc2, if rng.one_chance_in(2) { 0 } else { rng.next() as u16 & 7 });
    wr16(b, A5 + 0x7fc4, rng.next() as u16);
    wr8(b, 0xff2a1e, rng.one_chance_in(4) as u8);
    wr16(b, 0xff2960, if rng.next() & 1 == 0 { 0 } else { 0xffff });
    wr16(b, A5 + 0x5a6, rng.next() as u16 & 0x3ff);
    wr16(b, A5 + 0x5ae, rng.next() as u16 & 0x3ff);
    // file : têtes quelconques, parfois pleine ou vide
    let read = (rng.next() as u16 & 0x3f) * 0x10;
    let write = match rng.next() % 4 {
        0 => read,
        1 => read.wrapping_sub(0x10) & 0x3f0,
        _ => (rng.next() as u16 & 0x3f) * 0x10,
    };
    wr16(b, 0xff295c, read);
    wr16(b, 0xff295e, write);
    wr8(b, 0x61801f, if rng.next() & 1 == 0 { 0xff } else { 0 });
    regs(c, b, rng);
}

/// Un compteur BCD : parfois proche du plafond, parfois n'importe quoi.
fn bcd(rng: &mut Rng) -> u32 {
    match rng.next() % 4 {
        0 => 0x0999_9990 + rng.next() % 10,
        1 => rng.next(),
        _ => (0..8).fold(0, |v, _| v << 4 | rng.next() % 10),
    }
}

fn ring(b: &mut Bus, rng: &mut Rng, write: u32, cases: u32, read: bool) {
    wr8(b, A5 + write, (rng.next() as u8 & 0x3f) * 2);
    if read {
        wr8(b, A5 + write + 1, (rng.next() as u8 & 0x3f) * 2);
    }
    for i in 0..64 {
        let v = if rng.next() % 3 == 0 { (rng.next() as u16 & 0x37f) | 0x0000 } else { 0xffff };
        wr16(b, A5 + cases + 2 * i, v);
    }
}

fn rng_u1(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    wr16(b, A5 + 0x1a, rng.next() as u16);
    ring(b, rng, 0x108, 0x10a, true);
    ring(b, rng, 0x18a, 0x18c, true);
    // pop-ups : entrées de 8 octets, index multiples de 8
    wr8(b, A5 + 0x50, rng.one_chance_in(4) as u8);
    wr8(b, A5 + 0x20c, (rng.next() as u8 & 0x1f) * 8);
    wr8(b, A5 + 0x20d, (rng.next() as u8 & 0x1f) * 8);
    for i in 0..32 {
        // entrées plausibles : type (octet haut 0-3), code < 0x18, petits paramètres
        let plausible = (rng.next() % 4) as u16 * 0x100 | (rng.next() % 0x18) as u16;
        let base = A5 + 0x20e + 8 * i;
        wr16(b, base, if rng.next() % 3 == 0 { plausible } else { 0xffff });
        wr16(b, base + 2, rng.next() as u16 % 0x100);
        wr16(b, base + 4, if rng.next() & 1 == 0 { 0 } else { rng.next() as u16 % 0x100 });
        wr16(b, base + 6, 0x80);
    }
    // pop-up courant : durée restante 0, 1, 2, 3, 4, 5 ou longue
    let duration = [0u16, 1, 2, 3, 4, 5, 0x60][rng.next() as usize % 7];
    wr16(b, 0xff9180, (rng.next() % 4) as u16 * 0x100 | (rng.next() % 0x18) as u16);
    wr16(b, 0xff9182, rng.next() as u16 % 0x100);
    wr16(b, 0xff9184, rng.next() as u16 % 0x100);
    wr16(b, 0xff9186, duration);
    for j in 0..4 {
        wr32(b, 0xff85de + 0x200 * j + 0x110, bcd(rng));
        wr32(b, 0xff85de + 0x200 * j + 0x114, bcd(rng) & 0x000f_ffff);
    }
    regs(c, b, rng);
}

/// Monnayeurs : réglages, enregistrements et lignes d'entrée quelconques.
fn rng_c1(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    wr16(b, A5 + 0x7fcc, rng.one_chance_in(8) as u16);
    wr8(b, A5 + 0x1e, 1 + rng.next() as u8 % 4);
    wr8(b, A5 + 0x1f, 1 + rng.next() as u8 % 4);
    wr8(b, A5 + 0x20, rng.one_chance_in(2) as u8);
    let coin_slots = [0u8, 1, 2, 3, 0xff, 7];
    wr8(b, A5 + 0x21, coin_slots[rng.next() as usize % 6]);
    wr8(b, A5 + 0x22, rng.one_chance_in(2) as u8);
    wr8(b, A5 + 0x25, rng.one_chance_in(4) as u8);
    wr8(b, A5 + 0x30, rng.one_chance_in(2) as u8);
    wr8(b, 0xff8ec4, rng.one_chance_in(2) as u8);
    for k in 0..4 {
        let base = A5 + 0x44e + 0x10 * k;
        let debounce = [0u8, 1, 2, 3, 49, 50, 51, 255];
        wr8(b, base, if rng.next() & 1 == 0 { debounce[rng.next() as usize % 8] } else { rng.next() as u8 });
        wr8(b, base + 1, rng.next() as u8 % 6);
        wr8(b, base + 2, [0u8, 4, 8, 9, 10, 12, 0x80, 255][rng.next() as usize % 8]);
        wr8(b, base + 3, rng.next() as u8);
        wr8(b, base + 7, rng.one_chance_in(2) as u8);
    }
    b.inputs.in2 = rng.next() as u16;
    regs(c, b, rng);
}

/// EEPROM et comptabilité : contenu de l'EEPROM, copies en RAM, paramètres de transfert, compteurs.
fn rng_m2(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let mut data = [0u16; 64];
    for d in data.iter_mut() {
        *d = rng.next() as u16;
    }
    if rng.next() & 1 == 0 {
        // les deux exemplaires de la configuration concordent
        for i in 0..24 {
            data[24 + i] = data[i];
        }
    }
    b.eeprom = cps2::eeprom::Eeprom93c46::new(data);
    let words = [0u16, 1, 2, 5, 23, 47, 63][rng.next() as usize % 7];
    let debut = 2 * (rng.next() as u16 % (64 - words));
    wr16(b, A5 - 0x616e, words);
    wr16(b, A5 - 0x616a, debut);
    wr16(b, A5 - 0x616c, words);
    wr16(b, A5 - 0x6168, debut);
    wr32(b, A5 - 0x6172, if rng.next() & 1 == 0 { 0xff1d8e } else { 0xff1e0e });
    for i in 0..64 {
        let v = if rng.next() % 4 == 0 { data[i] ^ 1 } else { data[i] };
        wr16(b, 0xff1d8e + 2 * i as u32, if rng.next() % 3 == 0 { rng.next() as u16 } else { v });
        wr16(b, 0xff1e0e + 2 * i as u32, rng.next() as u16);
    }
    for (valeur, image) in [(0x76u32, 0xffff_9deeu32), (0x7a, 0xffff_9df2), (0x7e, 0xffff_9df6), (0x7fd4, 0xffff_9dfa)] {
        let v = rng.next() % 1000;
        wr32(b, A5 + valeur, v);
        wr32(b, A5.wrapping_add(image), if rng.next() & 1 == 0 { v + 1 } else { rng.next() % 1000 });
    }
    wr8(b, A5 + 0x82, rng.one_chance_in(4) as u8);
    wr8(b, A5 + 0x25, rng.one_chance_in(2) as u8);
    wr8(b, A5 + 0x74, 0);
    wr16(b, A5 + 0x16, rng.next() as u16 & 0x8f88);
    wr8(b, A5 + 0x18, rng.next() as u8 & 6);
    wr16(b, 0x664000, rng.next() as u16);
    // configuration : variante, copie en RAM (valeurs valides ou non), bloc par défaut dans A6
    let variant = 2 * (rng.next() as u16 % 6);
    wr16(b, A5 + 0x70, variant);
    for i in 0..0x80u32 {
        let small = if rng.next() % 3 == 0 { rng.next() as u8 } else { (rng.next() % 20) as u8 };
        wr8(b, 0xff1d8e + i, small);
    }
    let block = 0x2d656 + b_prog16(b, 0x2d656 + variant as u32) as i16 as u32;
    c.a[6] = if rng.next() & 3 == 0 { block + 0x30 } else { block };
    regs(c, b, rng);
}

/// Routine tirée au sort (pour les états qui dépendent de l'entrée appelée).
static ENTRY: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Couleur de palette : luminosité souvent aux butées, composantes quelconques ou proches de 0 / 0xf.
fn colour(rng: &mut Rng) -> u16 {
    let lum = match rng.next() % 4 {
        0 => 0,
        1 => 0xf000,
        _ => (rng.next() as u16 & 0xf) << 12,
    };
    let compo = |a: &mut Rng| match a.next() % 4 {
        0 => 0,
        1 => 0xf,
        _ => (a.next() & 0xf) as u16,
    };
    lum | compo(rng) << 8 | compo(rng) << 4 | compo(rng)
}

/// Primitives vidéo, palettes et fondus : palettes, état de fondu, scène et caméras quelconques.
fn rng_video(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let entry = ENTRY.load(std::sync::atomic::Ordering::Relaxed);
    for i in 0..0x800u32 {
        let v = if rng.next() % 5 == 0 { colour(rng) } else { colour(rng) & 0xf0ff | (i as u16 & 0xf00) };
        wr16(b, 0x91c000 + 2 * i, v);
    }
    // palettes cibles en RAM (0xff1190, 512 couleurs)
    for i in 0..512u32 {
        wr16(b, 0xff1190 + 2 * i, colour(rng));
    }
    wr8(b, A5 + 0x48e, rng.next() as u8 & 0xf);
    for k in 0..4u32 {
        let target = match rng.next() % 3 {
            0 => 0xff1190,
            1 => 0x253000 + 0x20 * (rng.next() % 0x1000),
            _ => 0x91c000 + 0x400 * k,
        };
        // 0x1cdd6 lit un mot (couleur) : mêmes octets, interprétés autrement
        wr32(b, A5 + 0x492 + 4 * k, if entry == 0x1cdd6 { colour(rng) as u32 } else if entry == 0x158e || entry == 0x15c8 { 0x91c000 + 0x400 * k } else { target });
    }
    if entry == 0x15c8 {
        wr32(b, A5 + 0x492, 0x91c3a0);
    }
    wr8(b, A5 + 0x34, [2, 0x14, 0x2f, 0x42, 0x44, 0x45, 0x59, 0x5a, 0x3, 0x8][rng.next() as usize % 10]);
    if rng.next() % 3 == 0 {
        wr8(b, A5 + 0x34, rng.next() as u8 % 0x60);
    }
    wr8(b, A5 + 0x36, rng.one_chance_in(2) as u8);
    for off in [0x506u32, 0x50a, 0x556, 0x55a] {
        wr16(b, A5 + off, rng.next() as u16);
    }
    for i in 0..0x2000u32 {
        wr16(b, 0x914000 + 2 * i, rng.next() as u16);
    }
    wr16(b, A5 + 0x7fd0, if rng.next() & 1 == 0 { 0x400 } else { 0 });
    wr8(b, 0xff0eab, rng.one_chance_in(3) as u8);
    wr8(b, A5 + 0x18, rng.next() as u8);
    regs(c, b, rng);
    c.a[4] = A5 + 0x48e;
    // numéros de scène / d'ensemble valides (les tables de la ROM s'arrêtent à la scène 0x5a)
    c.d[0] = if rng.next() % 3 == 0 { rng.next() % 0x5b } else { rng.next() % 12 };
    c.d[1] = if rng.next() & 1 == 0 { 0x1000 } else { 0xffff_f000 };
    if entry == 0x1a60a {
        c.d[0] %= 3; // la table de la page 1 n'a que trois entrées
    }
    let page = 0x91c000 + 0x400 * (rng.next() % 4);
    c.a[0] = page;
    c.a[1] = match entry {
        0x1602 | 0x164e => page,
        0x1cfc8 => [0xff1190, 0x253000 + 0x20 * (rng.next() % 0x1000)][rng.next() as usize % 2],
        _ => 0x916000 + 0x400 * (rng.next() % 8),
    };
    if entry == 0x164e {
        c.a[1] = 0x91c3a0;
    }
    if matches!(entry, 0x24a8) {
        let obj = 0xff85de + 0x200 * (rng.next() % 4);
        wr16(b, obj + 8, rng.next() as u16);
        wr16(b, obj + 0x10, rng.next() as u16);
        c.a[0] = obj;
    }
    if entry == 0x1d042 {
        c.d[1] = colour(rng) as u32;
    }
    if matches!(entry, 0x255a | 0x25e8) {
        c.d[0] = if rng.next() % 3 == 0 { rng.next() } else { rng.next() % 130 };
        c.a[1] = 0x912000 + 0x80 * (rng.next() % 40) + 4 * (rng.next() % 30);
    }
}

/// Caméras de défilement : positions cible et dessinée, butées et indicateurs quelconques mais plausibles
/// (les écarts vont du simple décalage à plusieurs pas, dans les quatre sens).
fn rng_camera(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    let entry = ENTRY.load(std::sync::atomic::Ordering::Relaxed);
    if matches!(entry, 0x33178 | 0x33196 | 0x331e8 | 0x3326c) {
        // décors à parallaxe : défilement courant et précédent, position de l'objet
        for off in [0x506u32, 0x50a, 0x524, 0x528] {
            wr32(b, A5 + off, rng.next() << 8 ^ rng.next());
        }
        regs(c, b, rng);
        for off in [0x08u32, 0x0c, 0x10] {
            wr32(b, c.a[0] + off, rng.next() << 8 ^ rng.next());
        }
        return;
    }
    let cam = if entry == 0x328a8 { A5 + 0x54e } else { A5 + 0x4fe };
    let step: i32 = if entry == 0x328a8 { 0x20 } else { 0x10 };
    let gap = |a: &mut Rng, portee: i32| -> i32 {
        match a.next() % 4 {
            0 => 0,
            1 => (a.next() as i32 % (2 * step)) - step,
            _ => (a.next() as i32 % (2 * portee)) - portee,
        }
    };
    // positions qui restent dans la carte du décor (au-delà, la ROM n'a plus de table : lectures sans sens)
    let drawn_x = 0x100 + (rng.next() % 0xb00) as i32;
    let drawn_y = 0x40 + (rng.next() % 0x260) as i32;
    let x = drawn_x + gap(rng, 4 * step);
    let y = drawn_y + gap(rng, 3 * step);
    wr16(b, cam + 0x10, drawn_x as u16);
    wr16(b, cam + 0x14, drawn_y as u16);
    wr16(b, cam + 0x08, x as u16);
    wr16(b, cam + 0x0c, y as u16);
    wr16(b, cam + 0x20, if rng.next() % 4 == 0 { 4 } else { 0 });
    wr16(b, cam + 0x2e, (x - rng.next() as i32 % 0x100) as u16);
    wr16(b, cam + 0x34, (x + (rng.next() as i32 % 0x200) - 0x40) as u16);
    wr8(b, cam + 0x3d, rng.one_chance_in(3) as u8);
    wr8(b, cam + 0x3e, rng.one_chance_in(8) as u8);
    wr8(b, A5 + 0x37, rng.one_chance_in(4) as u8);
    regs(c, b, rng);
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
/// `DDSOM_ENTREE=<address hexa>` restreint à un gestionnaire.
const DDSOM_TRACE_LEN: u32 = 200000;
/// États de départ (`+4`, `+5`, `+6`) que la table de sauts de chaque type ESHL 15..39 accepte
/// (au-delà, l'original sort du code) : listes de valeurs possibles par octet.
fn eshl_states(kind: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
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
fn perturb_globals(b: &mut cps2::Cps2Bus, rng: &mut Rng, step: u32, obj: u32) {
    use ddsom_game::mem::{rd16, wr8};
    if step != 0 && rng.next() % 4 != 0 {
        return;
    }
    // octets de la RAM de travail (A5+) et valeurs possibles
    let bytes: [(u32, &[u8]); 16] = [
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
    for (off, values) in bytes {
        if rng.next() % 3 == 0 {
            wr8(b, A5 + off, values[rng.next() as usize % values.len()]);
        }
    }
    // caméras, rang
    for off in [0x506u32, 0x556] {
        if rng.next() % 2 == 0 {
            wr16(b, A5 + off, [0, 0x100, 0x180, 0x1c0, 0x200, 0x2b0, 0x300, 0xa00][rng.next() as usize % 8]);
        }
    }
    for off in [0x50au32, 0x55a] {
        if rng.next() % 2 == 0 {
            wr16(b, A5 + off, (rng.next() % 0x200) as u16);
        }
    }
    if rng.next() % 3 == 0 {
        wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    }
    // joueurs : actif, état, vie, touches, position proche de l'objet
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if rng.next() % 3 == 0 {
            wr8(b, p, [0, 1, 1, 1, 2][rng.next() as usize % 5]);
        }
        if rng.next() % 4 == 0 {
            wr8(b, p + 4, [0, 1, 2, 2, 3, 4][rng.next() as usize % 6]);
            wr8(b, p + 5, 0);
            wr8(b, p + 6, 0);
        }
        if rng.next() % 3 == 0 {
            wr16(b, p + 0x62, [0, 0x08, 0x20, 0x40, 0x80][rng.next() as usize % 5]);
        }
        if rng.next() % 3 == 0 {
            wr8(b, p + 0x6c, rng.next() as u8);
            wr8(b, 0xff800c + j, rng.next() as u8 & 0xf0);
        }
        if rng.next() % 3 == 0 {
            let x = rd16(b, obj + 8).wrapping_add((rng.next() % 0x60) as u16).wrapping_sub(0x30);
            let y = rd16(b, obj + 0x10).wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18);
            wr16(b, p + 8, x);
            wr16(b, p + 0x10, y);
            wr16(b, p + 0xc, if rng.next() % 4 == 0 { 0x30 } else { 0 });
        }
        if rng.next() % 4 == 0 {
            wr8(b, p + 3, (rng.next() % 12) as u8);
        }
    }
}

/// Nombre de variantes (octet `+3`) que la table de sauts de l'original couvre (0xcc25a, ESHL 5, 9, 15). Au-delà, l'original
/// lit du code ou des données comme un décalage et saute n'importe où : ces cas sont hors contrat
/// (aucun objet du jeu n'a ces variantes) et ne sont pas tirés. 0xcc25a : une entrée de table (la
/// variante 1 retombe sur la même cible), la variante 2 saute au milieu d'un `jsr`.
fn table_variants(entry: u32) -> u32 {
    match entry {
        0xcc25a => 2,
        0x9e458 | 0x9ef46 => 3,
        0x9fe0a => 2,
        _ => u32::MAX,
    }
}

/// Registres que la boucle de dispatch ne fixe pas (elle pose A0, D7, A5, D0 = type × 4 et A1 =
/// gestionnaire) : ce sont les restes du gestionnaire précédent.
const LEFTOVER_NAMES: [&str; 10] = ["d1", "d2", "d3", "d4", "d5", "d6", "a2", "a3", "a4", "d0h"];

/// Exécute le code d'origine seul de `entry` jusqu'au `rts` (au plus 50 000 instructions).
fn run_original(c0: &Cpu, b0: &Bus, entry: u32, patch: &dyn Fn(&mut Cpu)) -> Option<(Cpu, Bus)> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (mut c, mut b) = (c0.clone(), b0.clone());
        patch(&mut c);
        c.pc = entry;
        c.a[7] = c.a[7].wrapping_sub(4);
        wr32(&mut b, c.a[7], 0xfffe);
        for _ in 0..50_000 {
            if c.pc == 0xfffe {
                c.a[7] = c.a[7].wrapping_add(4);
                return Some((c, b));
            }
            c.step(&mut b);
        }
        None
    }))
    .ok()
    .flatten()
}

/// Rejoue l'original deux fois en parallèle (registre de reste `r` de `LEFTOVER_NAMES` différent) et rend
/// l'adresse de la première instruction après laquelle l'état diverge (autre chose que le registre
/// lui-même) : c'est l'instruction qui lit le reste.
fn read_site(c: &Cpu, b: &Bus, entry: u32, r: usize, v1: &[u32; 10], v2: &[u32; 10], poser: fn(&mut Cpu, &[u32; 10])) -> Option<String> {
    let (mut c1, mut b1) = (c.clone(), b.clone());
    let (mut c2, mut b2) = (c.clone(), b.clone());
    poser(&mut c1, v1);
    poser(&mut c2, v2);
    for (c, b) in [(&mut c1, &mut b1), (&mut c2, &mut b2)] {
        c.pc = entry;
        c.a[7] = c.a[7].wrapping_sub(4);
        wr32(b, c.a[7], 0xfffe);
    }
    let mut story: Vec<u32> = Vec::new();
    for step in 0..20_000 {
        if c1.pc == 0xfffe && c2.pc == 0xfffe {
            return None;
        }
        let pc = c1.pc;
        story.push(pc);
        let ins = m68k::decode(&mut b1, pc);
        c1.step(&mut b1);
        c2.step(&mut b2);
        let mut d1 = c1.d;
        let mut d2 = c2.d;
        let (mut a1, mut a2) = (c1.a, c2.a);
        match r {
            0..=5 => {
                d1[r + 1] = 0;
                d2[r + 1] = 0;
            }
            6..=8 => {
                a1[r - 4] = 0;
                a2[r - 4] = 0;
            }
            _ => {
                d1[0] &= 0xffff;
                d2[0] &= 0xffff;
            }
        }
        let stack = |i: usize| {
            let a = 2 * i;
            (0x0a00..0x1000).contains(&a) || (0x2900..0x2a1a).contains(&a)
        };
        let ram = (0..b1.ram.len()).find(|&i| b1.ram[i] != b2.ram[i] && !stack(i));
        if c1.pc != c2.pc || d1 != d2 || a1[..7] != a2[..7] || ram.is_some() || b1.gfxram != b2.gfxram {
            let fin = story.len().saturating_sub(400);
            let h: Vec<String> = story[fin..].iter().map(|p| format!("{p:x}")).collect();
            return Some(format!("pas {step} en {pc:06x} {:?}{} après {}", ins.op, ram.map_or(String::new(), |i| format!(" (ram {:06x})", 0xff0000 + 2 * i)), h.join(" ")));
        }
    }
    None
}

/// Cherche les registres que le gestionnaire lit avant de les écrire : le code d'origine est exécuté
/// plusieurs fois depuis le même état de mémoire, en changeant un seul registre de reste à la fois
/// (D1-D6, A2-A4, octet haut de D0 que le dispatch met à zéro) ; si la mémoire finale change, le
/// gestionnaire dépend de ce registre. Rend un drapeau par registre de `LEFTOVER_NAMES` et l'état à la
/// fin de l'exécution de référence (pour poursuivre la trajectoire).
fn restes(c: &Cpu, b: &Bus, entry: u32, rng: &mut Rng) -> ([bool; 10], Option<(Cpu, Bus)>) {
    let draw = |rng: &mut Rng| -> [u32; 10] {
        let mut v = [0u32; 10];
        for (i, x) in v.iter_mut().enumerate() {
            *x = match i {
                0..=5 => if rng.next() & 3 == 0 { rng.next() & 0xff } else { rng.next() },
                6..=8 => 0xff0000 | (rng.next() & 0xfffe),
                _ => rng.next() & 0xff00,
            };
        }
        v
    };
    fn poser(c: &mut Cpu, v: &[u32; 10]) {
        for i in 0..6 {
            c.d[i + 1] = v[i];
        }
        for i in 0..3 {
            c.a[i + 2] = v[6 + i];
        }
        c.d[0] = (c.d[0] & 0xffff) | (v[9] << 16);
    }
    let base = draw(rng);
    let Some((cr, br)) = run_original(c, b, entry, &|c| poser(c, &base)) else {
        return ([false; 10], None);
    };
    let mut res = [false; 10];
    for r in 0..10 {
        for _ in 0..2 {
            let mut v = base;
            v[r] = draw(rng)[r];
            if v[r] == base[r] {
                continue;
            }
            if let Some((_, b2)) = run_original(c, b, entry, &|c| poser(c, &v)) {
                // les piles (registres sauvegardés par movem : pile du 68000 et pile du pilote de son à
                // 0xff2900-0xff2a1a) gardent les restes : comme `validate`, on les ignore
                let stack = |i: usize| {
                    let a = 2 * i;
                    (0x0a00..0x1000).contains(&a) || (0x2900..0x2a1a).contains(&a) || ((0x64..0x164).contains(&a) && (a - 0x64) % 16 < 4)
                };
                let ram_diff = (0..b2.ram.len()).any(|i| b2.ram[i] != br.ram[i] && !stack(i));
                if ram_diff || b2.gfxram != br.gfxram {
                    if std::env::var("DDSOM_RESTES_SITE").is_ok() {
                        if let Some(site) = read_site(c, b, entry, r, &base, &v, poser) {
                            eprintln!("RESTES_SITE {entry:06x} {} {site}", LEFTOVER_NAMES[r]);
                        }
                    }
                    if std::env::var("DDSOM_RESTES_TRACE").is_ok() {
                        let i = (0..b2.ram.len()).find(|&i| b2.ram[i] != br.ram[i] && !stack(i));
                        eprintln!("RESTES_TRACE {entry:06x} {} ram {:?} sp {:06x}", LEFTOVER_NAMES[r], i.map(|i| format!("{:06x} {:04x}/{:04x}", 0xff0000 + 2 * i, b2.ram[i], br.ram[i])), c.a[7]);
                    }
                    res[r] = true;
                }
            }
        }
    }
    (res, Some((cr, br)))
}

fn families(roms: &str, eeprom: &str, scenario: &str, warmup: u32, tours: u32, filter: Option<u32>) {
    use ddsom_game::mem::{rd16, rd8};
    use ddsom_game::objects::pools::{Pool, BOX, EFFCT, ENEMY, ESHL, ITEM, SET, SOBJ};
    use m68k::decode::Fetch;
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    // DDSOM_CHECK_ALL=1 : compare aussi tous les registres de sortie (lignes REGS du bilan)
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    std::panic::set_hook(Box::new(|_| {}));
    let mut rng = Rng(0x1234_5678_9abc_def1);
    // DDSOM_RESTES=1 : cherche les gestionnaires qui lisent un registre laissé par le précédent (voir
    // `restes`), dans les six tables ; sinon, contrôle de la native contre l'original
    let leftovers_active = std::env::var("DDSOM_RESTES").is_ok();
    let mut tables: Vec<(&Pool, u32, u32)> = vec![(&ESHL, 0x39b96, 64), (&SET, 0x39eac, 34), (&SOBJ, 0x39fa8, 16)];
    if leftovers_active {
        tables.extend([(&ITEM, 0x39f5a, 10), (&BOX, 0x39e76, 4), (&EFFCT, 0x39ce0, 64)]);
    }
    let mut leftovers_summary: std::collections::BTreeMap<u32, (u64, [u64; 10])> = Default::default();
    let mut inputs: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        for k in 0..n {
            let a = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == a) && filter.map_or(true, |f| f == a) {
                inputs.push((a, pool, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    if std::env::var("DDSOM_VERBOSE").is_ok() {
        let mut b = b0.clone();
        println!("places libres : ESHL {} SET {} SOBJ {}", rd16(&mut b, A5 + ESHL.free), rd16(&mut b, A5 + SET.free), rd16(&mut b, A5 + SOBJ.free));
    }
    // nombre de variantes tirées (3 par défaut : au-delà, beaucoup de types sortent de leurs tables)
    let variants: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let duration: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
    let mut shown = 0;
    let mut tests = 0;
    // couverture du code d'origine des gestionnaires (DDSOM_COUVERTURE=fichier : écrit les adresses vues)
    let coverage = std::env::var("DDSOM_COUVERTURE").is_ok();
    let mut seen: std::collections::BTreeSet<u32> = Default::default();
    for _ in 0..tours {
        for &(entry, pool, kind) in &inputs {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            // propriétaires possibles : ennemis actifs, sinon joueurs
            let mut owners: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
            owners.extend((0..4).map(|j| 0xff85de + 0x200 * j));
            let obj = pool.slot(rng.next() % pool.slots);
            for off in (0..pool.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            let owner = owners[rng.next() as usize % owners.len()];
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, (rng.next() % variants.min(table_variants(entry))) as u8);
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, (rng.next() & 1) as u8);
            wr8(&mut b, obj.0 + 0x2f, if rng.next() % 4 == 0 { rng.next() as u8 % 3 } else { 0 });
            wr16(&mut b, obj.0 + 0x50, owner as u16);
            wr16(&mut b, obj.0 + 0x46, owner as u16);
            wr16(&mut b, obj.0 + 0x5a, owner as u16);
            // ESHL 15..39 : créateur en +0x5c, hôte en +0x42, pointeurs longs (types 17 et 19), délais,
            // variables de niveau lues par les tirs
            if pool.class == 8 && kind >= 15 {
                wr16(&mut b, obj.0 + 0x5c, owner as u16);
                if rng.next() % 2 == 0 {
                    wr16(&mut b, obj.0 + 0x42, owner as u16);
                }
                wr16(&mut b, obj.0 + 0x58, (rng.next() % 6) as u16);
                if kind == 17 || kind == 19 {
                    wr32(&mut b, obj.0 + 0x50, owner);
                    wr32(&mut b, obj.0 + 0x56, owner);
                    wr8(&mut b, obj.0 + 0x43, [0, 1, 0xff][rng.next() as usize % 3]);
                    wr8(&mut b, obj.0 + 0x42, 4 * (rng.next() % 2) as u8);
                }
                if std::env::var("DDSOM_ETATS").is_ok() && rng.next() % 2 == 0 {
                    let hitboxes = eshl_states(kind);
                    let (s4, s5, s6) = hitboxes[rng.next() as usize % hitboxes.len()];
                    wr8(&mut b, obj.0 + 4, s4[rng.next() as usize % s4.len()]);
                    wr8(&mut b, obj.0 + 5, s5[rng.next() as usize % s5.len()]);
                    wr8(&mut b, obj.0 + 6, s6[rng.next() as usize % s6.len()]);
                }
                // le plus souvent, le propriétaire est à l'écran (sinon le tir est aussitôt retiré par le culling)
                if rng.next() % 4 != 0 {
                    let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
                    wr16(&mut b, owner + 8, cx.wrapping_add(0x60 + (rng.next() % 0x80) as u16));
                    wr16(&mut b, owner + 0x10, cy.wrapping_add(0x60 + (rng.next() % 0x40) as u16));
                }
                wr16(&mut b, A5 + 0x7f92, (rng.next() % 8) as u16);
                wr8(&mut b, A5 + 0x35, (rng.next() % 5) as u8);
                wr8(&mut b, A5 + 0x36, (rng.next() % 2) as u8);
            }
            if entry == 0xd5f5a {
                // barre de vie (SET 20) : joueur suivi en +0x20, second joueur en +0x34
                wr16(&mut b, obj.0 + 0x20, 0x85de + 0x200 * (rng.next() % 4) as u16);
                wr16(&mut b, obj.0 + 0x34, 0x85de + 0x200 * (rng.next() % 4) as u16);
            }
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, owner + off) as u32) << 16 | (rng.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            wr16(&mut b, obj.0 + 0xc, (rng.next() % 0x60) as u16);
            // décompte des places libres cohérent
            // (au moment du chauffage le pool peut être plein : les gestionnaires testent ce compteur)
            wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
            if pool.class == SET.class && matches!(kind, 2 | 15) {
                scene_state(&mut b, &mut rng, obj.0, kind, true);
            }
            for step in 0..duration {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if step > 0 {
                    if pool.class == SET.class && matches!(kind, 2 | 15) && rng.next() % 6 == 0 {
                        scene_state(&mut b, &mut rng, obj.0, kind, false);
                    }
                    if rng.next() % 6 == 0 {
                        wr8(&mut b, obj.0 + 0x57, [0x08, 0x20, 0x28, 0x10, 0x02, 0x40][rng.next() as usize % 6]);
                    }
                    if rng.next() % 8 == 0 {
                        wr8(&mut b, obj.0 + 0x33, rng.next() as u8 & 1);
                    }
                    if rng.next() % 10 == 0 {
                        wr8(&mut b, owner + 4, (rng.next() % 8) as u8);
                    }
                    // le propriétaire change d'état : états de l'automate, drapeaux d'attaque, compteurs
                    if rng.next() % 3 == 0 {
                        for off in [4u32, 5, 6, 7] {
                            if rng.next() % 2 == 0 {
                                wr8(&mut b, owner + off, 2 * (rng.next() % 6) as u8);
                            }
                        }
                        for off in [0x53u32, 0xa0, 0xa1, 0xa8, 0xa9, 0xb0, 0xbd, 0x3e, 0x2f, 0x42, 0x5d] {
                            if rng.next() % 3 == 0 {
                                let v = match rng.next() % 4 {
                                    0 => 0,
                                    1 => 1,
                                    2 => (rng.next() % 4) as u8,
                                    _ => rng.next() as u8,
                                };
                                wr8(&mut b, owner + off, v);
                            }
                        }
                    }
                    // sa propre variante / drapeaux de contrôle
                    if rng.next() % 12 == 0 {
                        wr8(&mut b, obj.0 + 0x3e, rng.next() as u8 % 6);
                    }
                    if rng.next() % 10 == 0 {
                        wr8(&mut b, obj.0 + 0x53, (rng.next() % 5) as u8);
                    }
                }
                // SET et SOBJ : contrôleurs de scène et d'interface, qui lisent des variables globales
                // (mode de jeu, nombre de joueurs, caméras, joueurs) : on les tire aussi
                if pool.class == 0x10 || pool.class == 0x14 {
                    perturb_globals(&mut b, &mut rng, step, obj.0);
                }
                // jeu en pause (rare) ou non ; objet suspendu (octet +0x40) très rarement
                wr8(&mut b, A5 + 0x3d, (rng.next() % 10 == 0) as u8);
                if pool.class == 8 && kind >= 15 {
                    wr8(&mut b, A5 + 0x6f, step as u8);
                    wr16(&mut b, A5 + 0x6e, rng.next() as u16);
                    wr8(&mut b, A5 + 0x42, (rng.next() % 2) as u8);
                    wr8(&mut b, A5 + 0x4e, (rng.next() % 16 == 0) as u8);
                }
                wr8(&mut b, obj.0 + 0x40, (rng.next() % 40 == 0) as u8);
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = step;
                // registres de la boucle de dispatch : D0 = type * 4 (octet 0 haut vide), A1 = gestionnaire
                c.d[0] = 4 * kind as u32;
                c.a[1] = entry;
                tests += 1;
                let before = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                let (cs, bs) = (c.clone(), b.clone());
                if coverage {
                    // adresses des instructions que l'original exécute à cet appel (interpréteur)
                    let (mut c2, mut b2) = (cs.clone(), bs.clone());
                    c2.pc = entry;
                    c2.a[7] = cs.a[7].wrapping_sub(4);
                    ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                    for _ in 0..20_000 {
                        if c2.pc == 0xfffe {
                            break;
                        }
                        seen.insert(c2.pc);
                        c2.step(&mut b2);
                    }
                }
                if leftovers_active {
                    let (res, next) = restes(&cs, &bs, entry, &mut rng);
                    let e = leftovers_summary.entry(entry).or_default();
                    e.0 += 1;
                    for (t, r) in e.1.iter_mut().zip(res) {
                        *t += r as u64;
                    }
                    match next {
                        Some((c2, b2)) => (c, b) = (c2, b2),
                        None => break,
                    }
                    continue;
                }
                let prev_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                if r.is_err() {
                    // l'original est sorti des rails (entrée incohérente) : on abandonne la trajectoire
                    let _ = (cs, bs);
                    break;
                }
                let after = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                if after > before && shown < 12 {
                    shown += 1;
                    let last = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                    println!("écart en {entry:06x} type {kind} pas {step} : {}", last.unwrap_or_default());
                    println!("  objet avant : {:02x?}", prev_obj);
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        // exécution pas à pas du code d'origine seul depuis l'état d'avant
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        let sp = c2.a[7];
                        c2.pc = entry;
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
    for (entry, (calls, regs)) in &leftovers_summary {
        let native_name = ddsom_game::all_natives().find(|n| n.entry == *entry).map_or("?", |n| n.name);
        let list: Vec<String> =
            regs.iter().enumerate().filter(|(_, &n)| n > 0).map(|(i, n)| format!("{}:{n}", LEFTOVER_NAMES[i])).collect();
        println!("RESTES {entry:06x} {native_name} {calls} appels : {}", if list.is_empty() { "aucun".into() } else { list.join(" ") });
    }
    if std::env::var("DDSOM_CHECK_ALL").is_ok() {
        for (name, st) in ddsom_game::report() {
            let list: Vec<String> = st
                .regs
                .iter()
                .enumerate()
                .filter(|(_, &n)| n > 0)
                .map(|(i, n)| format!("{}{}:{n}", if i < 8 { 'd' } else { 'a' }, i % 8))
                .collect();
            if !list.is_empty() {
                println!("REGS {name} {} appels : {}", st.calls, list.join(" "));
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{tests} appels de gestionnaires");
    if let Ok(file) = std::env::var("DDSOM_COUVERTURE") {
        let text: Vec<String> = seen.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(file, text.join("\n")).expect("écriture couverture");
    }
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}

/// Petites routines appelées par les gestionnaires des familles (créateurs, palettes, cases de tuiles, caméras
/// au retour de zone).
const SMALL: [u32; 8] = [0x101da, 0x1ac90, 0x4aed0, 0x53746, 0xb230e, 0xa462c, 0xa2b4a, 0x33b24];

fn rng_small_effects(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    use ddsom_game::objects::pools::{PEFF, PSHL, ESHL};
    regs(c, b, rng);
    let pool = [&PEFF, &PSHL, &ESHL][rng.next() as usize % 3];
    c.a[0] = if rng.one_chance_in(4) { 0xff85de + 0x200 * (rng.next() % 4) } else { pool.slot(rng.next() % pool.slots).0 };
    c.a[1] = 0x910000 + 4 * (rng.next() % 0xf00);
    c.a[5] = A5;
    wr8(b, A5 + 0x36, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x34, (rng.next() % 0x6c) as u8);
    for p in [&PEFF, &PSHL, &ESHL] {
        // places libres : parfois aucune (la création échoue en cours de route)
        let free = if rng.one_chance_in(5) { rng.next() % 3 } else { p.slots / 2 };
        wr16(b, A5 + p.free, free as u16);
    }
}

/// Créateurs d'objets du lot B (`families::effects_creators`, `creators`, `petites`, `effects_aids`) : créateur pris
/// dans un pool ou parmi les joueurs, registres et places libres des pools tirés (parfois aucune : la création échoue
/// en cours de route).
fn rng_creators(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    use ddsom_game::objects::pools::POOLS;
    regs(c, b, rng);
    let pool = POOLS[rng.next() as usize % POOLS.len()];
    c.a[0] = if rng.one_chance_in(4) { 0xff85de + 0x200 * (rng.next() % 4) } else { pool.slot(rng.next() % pool.slots).0 };
    c.a[1] = 0x910000 + 4 * (rng.next() % 0xf00);
    c.a[4] = 0xff0000 + (rng.next() % 0x8000);
    c.a[5] = A5;
    wr8(b, A5 + 0x36, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x34, (rng.next() % 0x6c) as u8);
    for p in POOLS {
        let free = if rng.one_chance_in(5) { rng.next() % 3 } else { p.slots / 2 };
        wr16(b, A5 + p.free, free as u16);
    }
}

/// Entrées du groupe `creator_entries` (sans les aiguillages par table et la remise à zéro de scène, qui appellent du code
/// choisi par l'état et ne sont pas des créateurs).
fn creator_entries() -> Vec<u32> {
    use ddsom_game::families::{creators, effects_creators, petites};
    // les étincelles posées entre deux objets lisent leurs boîtes de collision : sur des objets tirés au hasard,
    // l'original boucle sans fin (hors contrat), elles sont validées par les scénarios
    let excluded = [0xa8ae8, 0xa8af8, 0xd981a, 0xd9778, 0xcc0ca, 0x180ac, 0x1c44a, 0xaa96c, 0xaaa04, 0xaaa84];
    effects_creators::NATIVES.iter().chain(creators::NATIVES).chain(petites::NATIVES).chain(ddsom_game::effects_aids::NATIVES)
        .map(|n| n.entry)
        .filter(|e| !excluded.contains(e))
        .collect()
}

/// Boutique : l'article en vitrine (PEFF 42) et le comptoir (PEFF 43) avec l'interprète des dialogues du marchand, sur un client (objet
/// PEFF dont `+0x50` désigne un joueur) à l'inventaire, à la bourse et à l'état de menu tirés ; le script de
/// dialogue est écrit dans un emplacement ITEM (commandes valides, sauts vers l'avant, fin par un arrêt).
fn rng_shop(c: &mut Cpu, b: &mut Bus, rng: &mut Rng) {
    use ddsom_game::objects::pools::{ITEM, PEFF};
    let entry = ENTRY.load(std::sync::atomic::Ordering::Relaxed);
    regs(c, b, rng);
    let me = PEFF.slot(rng.next() % 24).0;
    let parent = PEFF.slot(24 + rng.next() % 12).0;
    let client = PEFF.slot(36 + rng.next() % 12).0;
    let player = 0xff85de + 0x200 * (rng.next() % 4);
    c.a[0] = me;
    c.a[2] = client;
    c.a[5] = A5;
    c.d[0] &= 0xff; // le comptoir laisse un petit écart de position dans D0.w
    wr16(b, client + 0x50, player as u16);
    wr8(b, client + 0x58, if rng.one_chance_in(4) { 0 } else { 1 + rng.next() as u8 % 3 });
    wr8(b, player + 0x3, (rng.next() % 12) as u8);
    wr8(b, player + 0x10d, [0, 8, 9, 0x1d, 0x1e, 3][rng.next() as usize % 6]);
    wr8(b, player + 0x167, [0, 1, 3, 4, 5, 8, 0xa, 0xff, 2][rng.next() as usize % 9]);
    for k in 0..6 {
        let item = [0x56, 0x56, 0x2b, 0x39, 0x4e, 0x51, 0x10, 0x20][rng.next() as usize % 8];
        wr8(b, player + 0x140 + k, item);
        wr8(b, player + 0x148 + k, (rng.next() % 11) as u8);
    }
    for k in 0..6 {
        wr8(b, player + 0x112 + k, bcd(rng) as u8);
    }
    wr16(b, A5 - 0x554e, if rng.one_chance_in(2) { 0 } else { 0x80 });
    wr8(b, A5 + 0x5e, (rng.next() % 2) as u8);
    if entry == 0x50b9c {
        wr8(b, me, 1);
        wr8(b, me + 2, 42);
        wr8(b, me + 3, (rng.next() % 12) as u8);
        wr8(b, me + 4, [0, 2, 2, 2][rng.next() as usize % 4]);
        wr16(b, me + 0x5c, parent as u16);
        wr8(b, parent + 6, if rng.one_chance_in(6) { 0 } else { 2 });
        wr8(b, A5 - 0x553f, (rng.next() % 2) as u8);
        wr8(b, A5 - 0x5540, (rng.next() % 2) as u8);
        wr8(b, me + 0x2f, [0, 0, 1, 2, 0xff][rng.next() as usize % 5]);
        wr8(b, me + 0x1a, (rng.next() % 2) as u8);
        for k in 0..6 {
            wr8(b, me + 0x4e + k, bcd(rng) as u8 & if k < 3 { 0 } else { 0xff });
        }
        // clients : la liste des quatre joueurs, le nôtre souvent dans la zone
        for k in 0..4u32 {
            let w = if k == 0 || rng.one_chance_in(3) { client as u16 } else { 0 };
            wr16(b, 0xff2aa4 + 2 * k, w);
        }
        for off in [0x8u32, 0xc, 0x10] {
            let v = ddsom_game::mem::rd16(b, me + off).wrapping_add((rng.next() % 0x20) as u16);
            wr16(b, client + off, v);
        }
        for (off, v) in [(0x14u32, 0x20u16), (0x16, 0x40), (0x54, 0x20), (0x56, 0x40)] {
            wr16(b, me + off, if rng.one_chance_in(5) { rng.next() as u16 & 0xff } else { v });
        }
        wr16(b, me + 0x5a, if rng.one_chance_in(2) { 0 } else { 0x90 });
    } else {
        let script = ITEM.slot(rng.next() % ITEM.slots).0;
        let n = 1 + rng.next() % 12;
        for k in 0..20u32 {
            let at = script + 4 * k;
            let (op, jump) = if k < n {
                ([0u8, 0, 4, 8, 0xc, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28][rng.next() as usize % 13], 4 * (rng.next() % 3) as u16)
            } else {
                (4, 0)
            };
            wr8(b, at, op);
            wr8(b, at + 1, (rng.next() % 2) as u8);
            wr16(b, at + 2, if op == 0 { 0x80 + rng.next() as u16 % 0x20 } else { jump });
        }
        // le comptoir PEFF 43 (0x51076) appelle l'interprète (0x514ac / 0x514cc, fonctions Rust) pour chaque joueur
        // prêt dans une de ses deux zones : un client dans la liste, à sa position, et le script dans les deux zones
        wr8(b, me, 1);
        wr8(b, me + 2, 43);
        wr8(b, me + 4, 2);
        for k in 0..4u32 {
            let w = if k == 0 || rng.one_chance_in(3) { client as u16 } else { 0 };
            wr16(b, 0xff2aa4 + 2 * k, w);
        }
        for off in [0x8u32, 0xc, 0x10] {
            let v = ddsom_game::mem::rd16(b, me + off).wrapping_add((rng.next() % 0x10) as u16);
            wr16(b, client + off, v);
        }
        for (off, v) in [(0x14u32, 0x20u16), (0x16, 0x40), (0x18, 0x20), (0x1a, 0x40), (0x50, 0x20), (0x52, 0x40), (0x54, 0x20), (0x56, 0x40)] {
            wr16(b, me + off, if rng.one_chance_in(5) { rng.next() as u16 & 0xff } else { v });
        }
        for field in [0x1c, 0x58] {
            wr32(b, me + field, script);
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, group, roms, eeprom, scenario, warmup, tours] = &a[..] else {
        eprintln!("usage : fuzz <groupe> <roms> <eeprom> <scénario> <images de chauffe> <tours>");
        std::process::exit(2);
    };
    if group == "familles" {
        families(roms, eeprom, scenario, warmup.parse().unwrap(), tours.parse().unwrap(), std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()));
        return;
    }
    let (rng_state, range): (fn(&mut Cpu, &mut Bus, &mut Rng), std::ops::Range<u32>) = match group.as_str() {
        "son" => (rng_sound, 0x68c..0xee000),
        "u1" => (rng_u1, 0x19a80..0x1a000),
        "c1" => (rng_c1, 0x13c00..0x14e50),
        "m2" => (rng_m2, 0x2d300..0x2ede0),
        "video" => (rng_video, 0xb40..0x1d100),
        "camera" => (rng_camera, 0x2f000..0x33400),
        "petites" => (rng_small_effects, 0x0..0x100000),
        "boutique" => (rng_shop, 0x50b9c..0x514d0),
        "createurs" => (rng_creators, 0x0..0x100000),
        _ => {
            eprintln!("groupe inconnu : {group}");
            std::process::exit(2);
        }
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..warmup.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    // les aiguillages du son choisissent leur cible dans une table selon un registre ou l'objet : tirés au
    // hasard, ces sélecteurs sortent des tables (hors contrat) ; ils sont validés par les scénarios
    let inputs: Vec<u32> = ddsom_game::all_natives()
        .filter(|n| n.name != "son_aiguillage")
        .map(|n| n.entry)
        .filter(|e| range.contains(e) && (*group != "son" || *e == 0x68c || *e >= 0xe0836))
        .filter(|e| *group != "petites" || SMALL.contains(e))
        .filter(|e| *group != "createurs" || creator_entries().contains(e))
        .filter(|e| *group != "boutique" || matches!(*e, 0x50b9c | 0x51076))
        .filter(|e| *group != "camera" || matches!(*e, 0x2f81c | 0x2f86e | 0x328a8 | 0x33178 | 0x33196 | 0x331e8 | 0x3326c))
        .filter(|e| *group != "video" || matches!(*e, 0xb40 | 0xb46 | 0xb96 | 0xbac | 0xbc6 | 0xbfc | 0xc58 | 0xca8 | 0xcc8 | 0xce8 | 0x158e | 0x15c8 | 0x1602 | 0x164e | 0x1a296 | 0x1a350 | 0x1a360 | 0x1a366 | 0x1a3d4 | 0x1a608 | 0x1a60a | 0x1a678 | 0x1a67e | 0x1a8ac | 0x1a8b2 | 0x1cd32 | 0x1cd7c | 0x1cdd6 | 0x1cf3c | 0x1cfb8 | 0x1cfc8 | 0x1d00e | 0x1d042 | 0x24a8 | 0x24b0 | 0x255a | 0x25e8 | 0x260c | 0x26d4 | 0x2766))
        .collect();
    let (c, b) = (&mut m.cpu, &mut m.bus);
    b.irq_horizon = u64::MAX;
    let mut shown = 0;
    for _ in 0..tours.parse::<u32>().unwrap() {
        for &entry in &inputs {
            ENTRY.store(entry, std::sync::atomic::Ordering::Relaxed);
            rng_state(c, b, &mut rng);
            let (d0, a0) = (c.d[0], c.a[0]);
            let context = if entry == 0x2d662 {
                let var = ddsom_game::mem::rd16(b, A5 + 0x70);
                let copy: Vec<u8> = (0..18).map(|i| ddsom_game::mem::rd8(b, 0xff1d8e + i)).collect();
                let block: Vec<u8> = (0..18).map(|i| ddsom_game::mem::rd8(b, c.a[6] + i)).collect();
                format!(" var={var} copie={copy:02x?} defauts={block:02x?}")
            } else if *group == "camera" {
                let cam = c.a[0];
                let fields: Vec<String> = [0x08u32, 0x0c, 0x10, 0x14, 0x20, 0x2e, 0x34, 0x40]
                    .iter()
                    .map(|o| format!("{o:02x}={:04x}", ddsom_game::mem::rd16(b, cam + o)))
                    .collect();
                format!(" scène={:02x} terrain={} {}", ddsom_game::mem::rd8(b, A5 + 0x34), ddsom_game::mem::rd8(b, A5 + 0x37), fields.join(" "))
            } else {
                String::new()
            };
            let before = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            ddsom_game::call(c, b, entry);
            let after = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            if after > before && shown < 12 {
                shown += 1;
                let last = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                println!("écart en {entry:06x} (d0={d0:08x} a0={a0:06x}){context} : {}", last.unwrap_or_default());
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
/// que le fuzz générique ne fait pas varier. `start` : état initial de l'objet (variante, état).
fn scene_state(b: &mut Bus, rng: &mut Rng, obj: u32, kind: u8, start: bool) {
    use ddsom_game::mem::{rd16, rd32, rd8};
    wr8(b, A5 + 0x34, if rng.next() % 3 == 0 { 4 } else { 1 + (rng.next() % 7) as u8 });
    wr8(b, A5 + 0x35, if rng.next() % 10 == 0 { 0 } else { 1 + (rng.next() % 4) as u8 });
    wr8(b, A5 + 0x2c, [0xc, 4, 8, 0, 0x18][rng.next() as usize % 5]);
    wr8(b, A5 + 0x42, (rng.next() % 8 == 0) as u8);
    wr8(b, A5 + 0x36, (rng.next() % 4 == 0) as u8);
    wr8(b, A5 + 0x50, (rng.next() % 2) as u8);
    wr8(b, A5 + 0x66, (rng.next() % 2) as u8);
    wr16(b, A5 + 0x7f92, (rng.next() % 6) as u16);
    for j in 0..4u32 {
        let p = 0xff85de + 0x200 * j;
        if rng.next() % 3 == 0 {
            wr16(b, p + 4, [0x200, 0x200, 0, 0x600, 0x204][rng.next() as usize % 5]);
            wr8(b, p + 6, (rng.next() % 3 == 0) as u8);
            wr16(b, p + 0x62, [0, 5, 100][rng.next() as usize % 3]);
        }
    }
    if kind == 15 {
        // variables lues par les écrans de fin : fondu terminé, caméras, score, rng, objets du joueur
        wr8(b, A5 + 0x48f, (rng.next() % 3 == 0) as u8);
        wr8(b, A5 + 0x6d, rng.next() as u8);
        wr16(b, A5 + 0x6e, rng.next() as u16 & [0xf, 0xff, 0][rng.next() as usize % 3]);
        for off in [0x506u32, 0x50a, 0x556, 0x55a] {
            wr16(b, A5 + off, [0, 0, 1, 0x70, rng.next() as u16 & 0x1ff][rng.next() as usize % 5]);
        }
        wr16(b, A5 + 0x38, [0x80, 0x120, 0x1f0, 0x220, 0x300][rng.next() as usize % 5]);
        wr8(b, A5 + 0x5d, (rng.next() % 2) as u8);
        for j in 0..4u32 {
            let p = 0xff85de + 0x200 * j;
            wr8(b, p + 0x18a, (rng.next() % 2) as u8);
            wr8(b, p + 3, (rng.next() % 6) as u8);
            wr8(b, p + 0x22, (rng.next() % 2) as u8);
            wr16(b, p + 0x60, [50, 100, 160][rng.next() as usize % 3]);
            wr16(b, p + 0x64, (rng.next() % 20) as u16);
            if rng.next() % 4 == 0 {
                wr8(b, p + 0x118, [0xff, 0xf9, 0xfa, 0x10, 0][rng.next() as usize % 5]);
            }
        }
        if start {
            if rng.next() % 4 == 0 {
                wr8(b, obj + 0x29, 0x1f);
                wr16(b, obj + 0x24, 1 + (rng.next() % 2) as u16);
            }
            for off in [0x14u32, 0x18, 0x22, 0x23, 0x24, 0x2a, 0x2b, 0x30, 0x31, 0x42, 0x43] {
                if rng.next() % 3 == 0 {
                    wr8(b, obj + off, [0, 1, 2, 3, rng.next() as u8][rng.next() as usize % 5]);
                }
            }
        }
    }
    if start && kind == 15 {
        // écrans de fin : variante, états, chef d'orchestre (+0x4c, un autre objet SET)
        wr8(b, obj + 3, (rng.next() % 25) as u8);
        wr8(b, obj + 4, [0u8, 0, 2, 2, 4, 6][rng.next() as usize % 6]);
        // sous-états valides de chaque variante (les autres font sauter l'original dans des données)
        let variant = rd8(b, obj + 3);
        let sub_states = match variant {
            1 => 5,
            9 | 10 | 11 | 17 => 3,
            7 | 14 | 16 => 2,
            18 | 22 => 9,
            _ => 1,
        };
        wr8(b, obj + 5, 2 * (rng.next() % sub_states) as u8);
        // joueur visé par les variantes 18 et 22, objet cible de la 17
        let player = 0xff85de + 0x200 * (rng.next() % 4);
        wr16(b, obj + 0x20, player as u16);
        wr16(b, obj + 0x26, player as u16);
        wr16(b, obj + 0x46, player as u16);
        // fenêtre de dialogue (variante 7) : joueur visé, message à composer, tampon de texte
        wr16(b, obj + 0x34, player as u16);
        wr32(b, obj + 8, 0xcec00 + 2 * (rng.next() % 0x400));
        wr32(b, obj + 0xc, 0xff2072);
        wr8(b, A5 + 0x73, [2, 1, 0][rng.next() as usize % 3]);
        wr16(b, A5 + 0x72, 2 + (rng.next() % 3) as u16);
        let leader = if obj == 0xffca9e { 0xffcaee } else { 0xffca9e };
        wr32(b, obj + 0x4c, leader);
        wr8(b, leader + 8, [0xff, 0, 1, 2, 3][rng.next() as usize % 5]);
        wr8(b, leader + 3, [0, 2, 3, 1][rng.next() as usize % 4]);
        wr8(b, leader + 0x3f, (rng.next() % 4) as u8);
        wr8(b, leader + 0xc, (rng.next() % 2) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 8) as u8);
        return;
    }
    if !start || kind != 2 {
        return;
    }
    let variant = rng.next() % 0x78;
    wr8(b, obj + 3, variant as u8);
    let script = rd32(b, 0xbd276 + 4 * variant);
    let state = [0u8, 0, 2, 2, 2, 4, 4, 6][rng.next() as usize % 8];
    wr8(b, obj + 4, state);
    if state == 2 {
        wr16(b, obj + 0x18, 0xffff);
        let first = rd16(b, script);
        wr16(b, obj + 0x24, first);
        wr32(b, obj + 0x20, script + 2);
    } else if state == 4 {
        wr16(b, obj + 0x24, (rng.next() % 4) as u16);
        wr16(b, obj + 0x2a, (rng.next() % 4 == 0) as u16);
        if rng.next() % 2 == 0 {
            // rideau (action de fin 6) : déjà armé, décompte court, curseur dans la table
            wr8(b, obj + 5, 2);
            wr16(b, obj + 0x14, (rng.next() % 3) as u16);
            wr16(b, obj + 0x26, 6 * (rng.next() % 9) as u16);
        }
    }
}
