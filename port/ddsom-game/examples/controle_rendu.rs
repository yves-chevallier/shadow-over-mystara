//! Contrôle bit à bit du rendu et de l'audio (phase d'optimisation, lot F).
//!
//! controle_rendu <roms> <eeprom> <scénario> <images> [sortie]
//!
//! Fait tourner le jeu sans 68000 avec le rendu et l'audio (`DDSOM_SON_RUST=1` : pilote son Rust ; sinon Z80 émulé)
//! et imprime une ligne par image : hachage des pixels, hachage des échantillons audio produits. Avec `sortie`, écrit
//! ces lignes dans un fichier ; sinon seule la ligne de synthèse est imprimée. Comparer deux sorties avec `cmp`/`diff`.

use std::hash::Hasher;
use std::io::Write;
use std::path::Path;

/// FNV-1a 64 bits : simple, stable, sans dépendance.
struct Fnv(u64);
impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage : controle_rendu <roms> <eeprom> <scenario> <images> [sortie]");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    if std::env::var_os("DDSOM_SON_RUST").is_some() {
        m.sound.use_rust_driver();
    }
    ddsom_game::install_sans_68k(&mut m);
    m.rendu = true;
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).expect("scénario");
    let frames: usize = a[4].parse().unwrap();
    let mut out: Option<std::io::BufWriter<std::fs::File>> = a.get(5).map(|p| std::io::BufWriter::new(std::fs::File::create(p).unwrap()));
    let mut total = Fnv(0xcbf29ce484222325);
    let t0 = std::time::Instant::now();
    for f in 0..frames {
        script.apply(f, &mut m);
        let img = m.run_frame();
        let mut hi = Fnv(0xcbf29ce484222325);
        for p in &img {
            hi.write(p);
        }
        let mut ha = Fnv(0xcbf29ce484222325);
        for s in m.sound.audio.drain(..) {
            ha.write(&s[0].to_le_bytes());
            ha.write(&s[1].to_le_bytes());
        }
        if let Some(o) = out.as_mut() {
            writeln!(o, "{f} {:016x} {:016x}", hi.finish(), ha.finish()).unwrap();
        }
        total.write(&hi.finish().to_le_bytes());
        total.write(&ha.finish().to_le_bytes());
    }
    println!("{} images, synthèse {:016x}, {:.0} images/s", frames, total.finish(), frames as f64 / t0.elapsed().as_secs_f64());
}
