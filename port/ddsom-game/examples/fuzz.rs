//! Validation différentielle sur des états aléatoires : chaque routine native d'un groupe est appelée
//! en mode contrôle depuis des états tirés au sort ; l'original s'exécute à côté et les deux sont
//! comparés (mémoire, registres du contrat, durée si elle est déclarée exacte). Exerce les chemins
//! que les scénarios n'atteignent pas.
//!
//!   fuzz <groupe> <roms> <eeprom> <scénario> <images de chauffe> <tours>
//!
//! Groupes : son (0x68c, 0xe0836-0xee000), u1 (RNG et files d'événements 0x19a80-0x1a000),
//! c1 (monnayeurs 0x13c00-0x14e50), m2 (EEPROM et configuration 0x2d300-0x2ede0).

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
        wr16(b, A5 + 0x20e + 8 * i, if alea.next() % 3 == 0 { alea.next() as u16 } else { 0xffff });
    }
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
    registres(c, b, alea);
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
        .collect();
    let (c, b) = (&mut m.cpu, &mut m.bus);
    b.irq_horizon = u64::MAX;
    let mut affiches = 0;
    for _ in 0..tours.parse::<u32>().unwrap() {
        for &entree in &entrees {
            alea_etat(c, b, &mut alea);
            let (d0, a0) = (c.d[0], c.a[0]);
            let avant = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            ddsom_game::call(c, b, entree);
            let apres = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            if apres > avant && affiches < 12 {
                affiches += 1;
                let dernier = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                println!("écart en {entree:06x} (d0={d0:08x} a0={a0:06x}) : {}", dernier.unwrap_or_default());
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
