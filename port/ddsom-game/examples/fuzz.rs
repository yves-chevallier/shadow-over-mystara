//! Validation différentielle sur des états aléatoires : chaque routine native d'un groupe est appelée
//! en mode contrôle depuis des états tirés au sort ; l'original s'exécute à côté et les deux sont
//! comparés (mémoire, registres du contrat, durée si elle est déclarée exacte). Exerce les chemins
//! que les scénarios n'atteignent pas.
//!
//!   fuzz <groupe> <roms> <eeprom> <scénario> <images de chauffe> <tours>
//!
//! Groupes : son (0x68c, 0xe0836-0xee000), u1 (RNG et files d'événements 0x19a80-0x1a000),
//! c1 (monnayeurs 0x13c00-0x14e50), m2 (EEPROM et configuration 0x2d300-0x2ede0),
//! video (primitives vidéo, palettes et fondus : natives `video_*` et `palette_*`).

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
}

fn b_prog16(b: &mut Bus, a: u32) -> u16 {
    m68k::Bus::read_prog16(b, a)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, groupe, roms, eeprom, scenario, chauffe, tours] = &a[..] else {
        eprintln!("usage : fuzz <groupe> <roms> <eeprom> <scénario> <images de chauffe> <tours>");
        std::process::exit(2);
    };
    let (alea_etat, plage): (fn(&mut Cpu, &mut Bus, &mut Alea), std::ops::Range<u32>) = match groupe.as_str() {
        "son" => (alea_son, 0x68c..0xee000),
        "u1" => (alea_u1, 0x19a80..0x1a000),
        "c1" => (alea_c1, 0x13c00..0x14e50),
        "m2" => (alea_m2, 0x2d300..0x2ede0),
        "video" => (alea_video, 0xb40..0x1d100),
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
        .filter(|e| *groupe != "video" || matches!(*e, 0xb40 | 0xb46 | 0xb96 | 0xbac | 0xbc6 | 0xbfc | 0xc58 | 0xca8 | 0xcc8 | 0xce8 | 0x158e | 0x15c8 | 0x1602 | 0x164e | 0x1a296 | 0x1a350 | 0x1a360 | 0x1a366 | 0x1a3d4 | 0x1a608 | 0x1a60a | 0x1a678 | 0x1a67e | 0x1a8ac | 0x1a8b2 | 0x1cd32 | 0x1cd7c | 0x1cdd6 | 0x1cf3c | 0x1cfb8 | 0x1cfc8 | 0x1d00e | 0x1d042 | 0x24a8 | 0x24b0 | 0x260c | 0x26d4 | 0x2766))
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
