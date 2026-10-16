//! Validation différentielle du son sur des états aléatoires : chaque routine native du son (wrappers
//! compris) est appelée en mode contrôle depuis des états tirés au sort (drapeaux de son, file
//! vide/pleine/quelconque, objet A0 quelconque, caméras...) ; l'original s'exécute à côté et les
//! deux sont comparés. Exerce les chemins que les scénarios n'atteignent pas.
//!
//!   fuzz_son <roms> <eeprom> <scénario> <images de chauffe> <tours>

use std::path::Path;

use ddsom_game::mem::{wr16, wr8, A5};

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

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, chauffe, tours] = &a[..] else {
        eprintln!("usage : fuzz_son <roms> <eeprom> <scénario> <images de chauffe> <tours>");
        std::process::exit(2);
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
        .filter(|&e| e == 0x68c || (0xe0836..0xee000).contains(&e))
        .collect();
    let (c, b) = (&mut m.cpu, &mut m.bus);
    b.irq_horizon = u64::MAX;
    let mut ecarts_affiches = 0;
    for _ in 0..tours.parse::<u32>().unwrap() {
        for &entree in &entrees {
            // drapeaux et compteurs de son
            wr8(b, A5 + 0x27, alea.next() as u8 & 1);
            wr8(b, A5 + 0x30, alea.next() as u8 & 1);
            wr8(b, A5 + 0x28, alea.next() as u8 & 1);
            wr8(b, A5 + 0x36, alea.next() as u8 & 1);
            wr16(b, A5 + 0x7fcc, if alea.une_chance_sur(3) { 1 } else { 0 });
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
            // objet A0 : un enregistrement joueur aux octets 2, 3 et à la position X tirés au sort
            let obj = 0xff85de + 0x200 * (alea.next() % 4);
            wr8(b, obj + 2, alea.next() as u8);
            wr8(b, obj + 3, alea.next() as u8);
            wr16(b, obj + 8, alea.next() as u16 & 0x7ff);
            c.a[0] = obj;
            for r in 0..8 {
                c.d[r] = alea.next();
            }
            for r in 1..5 {
                c.a[r] = 0xff0000 | (alea.next() & 0xffff);
            }
            let avant = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            ddsom_game::call(c, b, entree);
            let apres = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            if apres > avant && ecarts_affiches < 12 {
                ecarts_affiches += 1;
                let dernier = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                println!("écart en {entree:06x} : {}", dernier.unwrap_or_default());
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
