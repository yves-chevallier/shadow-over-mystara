//! Validation différentielle sans le jeu : le Z80 d'origine seul et le pilote Rust reçoivent les
//! mêmes commandes synthétiques dans la RAM partagée, et on compare leurs écritures. Cela exerce
//! toute la ROM (toutes les musiques et tous les effets, toutes les commandes système) et pas
//! seulement ce que le scénario du jeu envoie.
//!
//! fuzz <roms> songs <premier> <dernier> <interruptions>   un essai par numéro de musique/effet
//! fuzz <roms> random <graine> <essais> <interruptions>    suites aléatoires de commandes
//! fuzz <roms> synth <graine> <essais> <interruptions>    musiques et effets synthétiques (ajoutés à la ROM)
//! fuzz <roms> one <numéro> <graine> <interruptions> [de a]  un essai, avec détail des écritures

#[path = "support/mod.rs"]
mod support;

use ddsom_sound::{Driver, Rom, Write, SHARED_SIZE};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::{reference::RefZ80, Report};

/// Générateur pseudo-aléatoire (xorshift64*).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
}

/// Nombre d'interruptions consécutives pendant lesquelles le Z80 peut rester occupé avant qu'on
/// abandonne l'essai (un tic normal n'en demande que deux ou trois).
const MAX_BUSY: usize = 10;

/// Ce que fait le 68000 pendant un essai.
struct Session {
    /// Modifications de la ROM : (offset dans la région audiocpu, octets). Elles ajoutent des
    /// musiques et des effets synthétiques que le jeu n'utilise pas.
    patches: Vec<(usize, Vec<u8>)>,
    /// Réglage stéréo (0 = mono).
    stereo: u8,
    /// Commandes (interruption à partir de laquelle on tente de les poster).
    commands: Vec<(usize, [u8; 16])>,
    irqs: usize,
}

/// Commande « musique ou effet » avec des paramètres de pan plausibles.
fn play_command(rng: &mut Rng, number: u16) -> [u8; 16] {
    let mut c = [0u8; 16];
    c[0] = (number >> 8) as u8;
    c[1] = number as u8;
    for b in &mut c[2..6] {
        *b = rng.byte();
    }
    c[6] = rng.below(8) as u8; // type d'évolution du pan
    c[7] = rng.below(0x28) as u8;
    c[8] = rng.below(0x28) as u8;
    for b in &mut c[9..13] {
        *b = if rng.below(2) == 0 { rng.below(8) as u8 } else { rng.byte() };
    }
    c[13] = rng.byte();
    c[14] = rng.byte();
    c
}

/// Commande système avec des paramètres plausibles.
fn system_command(rng: &mut Rng) -> [u8; 16] {
    let mut c = [0u8; 16];
    c[0] = 0xff;
    c[1] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12, 13][rng.below(12) as usize];
    c[2] = rng.byte();
    c[3] = if rng.below(2) == 0 { 0 } else { 0xff };
    c[4] = match c[1] {
        2 => rng.below(16) as u8,
        4 => rng.below(3) as u8,
        _ => rng.byte(),
    };
    c
}

fn song_count(rom: &Rom) -> u16 {
    let header = u32::from(rom.word(0x39e1));
    u16::from(rom.byte(header)) << 8 | u16::from(rom.byte(header + 1))
}

fn run_session(rom: &Rom, s: &Session, dump: Option<(usize, usize)>, coverage: Option<&mut [bool]>) -> Report {
    let patched;
    let rom = if s.patches.is_empty() {
        rom
    } else {
        let mut region = rom.region().to_vec();
        for (at, bytes) in &s.patches {
            region[*at..*at + bytes.len()].copy_from_slice(bytes);
        }
        patched = Rom::from_region(region);
        &patched
    };
    let mut real = RefZ80::new(rom);
    let mut driver = Driver::new(rom.clone());
    let mut dshared = vec![0u8; SHARED_SIZE];
    let mut report = Report::default();
    let boot = real.run_interval();
    let mut mine = Vec::new();
    driver.reset(&mut dshared, &mut mine);
    report.add_interval(0, boot, mine);
    let mut next = 0;
    let mut busy = 0;
    for k in 1..=s.irqs {
        // Le 68000 répond à la poignée de main, prépare la boîte aux lettres, poste les commandes.
        if k == 10 {
            for sh in [real.shared(), &mut dshared[..]] {
                sh[0xfff] = 0xff;
                sh[0xffd] = 0x88;
                sh[0xffe] = s.stereo;
                sh[0xf] = 0xff;
            }
        }
        // Le 68000 ne poste une commande que si le Z80 a rattrapé son retard : sinon l'ordre entre
        // la commande et le travail en attente dépend de la durée d'exécution du Z80.
        if k > 10 && next < s.commands.len() && s.commands[next].0 <= k && real.shared()[0xf] == 0xff && real.is_idle() {
            let c = s.commands[next].1;
            for sh in [real.shared(), &mut dshared[..]] {
                sh[..16].copy_from_slice(&c);
                sh[0xf] = 0;
            }
            next += 1;
        }
        let w_real = real.run_interval();
        let mut w_mine = Vec::new();
        driver.interrupt(&mut dshared, &mut w_mine);
        if let Some((from, to)) = dump {
            if (from..=to).contains(&k) {
                let ram = real.ram();
                let img = driver.ram_image();
                println!(
                    "--- interruption {k} (Z80 : f000-f003 {:02x?} file {:02x}/{:02x} ; Rust : {:02x?} file {:02x}/{:02x} ; inactif : {})",
                    &ram[..4], ram[0x15], ram[0x16], &img[..4], img[0x15], img[0x16], real.is_idle()
                );
                print_writes("Z80   ", &w_real);
                print_writes("Rust  ", &w_mine);
                if std::env::var("FUZZ_PROFILE").is_ok() {
                    let mut top: Vec<(u32, usize)> = real.hits.iter().enumerate().map(|(a, &n)| (n, a)).filter(|x| x.0 > 0).collect();
                    top.sort_by(|a, b| b.cmp(a));
                    let list: Vec<String> = top.iter().take(10).map(|(n, a)| format!("{a:04x}:{n}")).collect();
                    println!("  adresses les plus exécutées : {}", list.join(" "));
                    real.hits.iter_mut().for_each(|h| *h = 0);
                }
            }
        }
        if std::env::var("FUZZ_STATE").is_ok() && real.is_idle() && report.first_divergence.is_none() {
            let (z, r) = (real.ram(), driver.ram_image());
            let diffs = differences(z, &r);
            if !diffs.is_empty() {
                let list: Vec<String> = diffs
                    .iter()
                    .take(6)
                    .map(|&i| format!("{:04x} ({}) : Z80 {:02x} / Rust {:02x}", 0xf000 + i, describe(i), z[i], r[i]))
                    .collect();
                report.first_divergence = Some(format!("état différent après l'interruption {k} : {}", list.join(" ; ")));
                break;
            }
        }
        report.add_interval(k, w_real, w_mine);
        busy = if real.is_idle() { 0 } else { busy + 1 };
        if busy >= MAX_BUSY {
            report.slow = true;
            break;
        }
    }
    if let Some(cov) = coverage {
        for (c, r) in cov.iter_mut().zip(&real.coverage) {
            *c |= r;
        }
    }
    report
}

/// Octets de la RAM du Z80 que le pilote ne modélise pas (banque courante, variables
/// temporaires, pile, octets inutilisés des voies).
fn ignored(i: usize) -> bool {
    let a = 0xf000 + i;
    matches!(a, 0xf005 | 0xf006 | 0xf007 | 0xf00d | 0xf01a) || (0xf050..0xf200).contains(&a) || (0xff00..0x10000).contains(&a)
        || (0xf200..0xfc00).contains(&a) && matches!((a - 0xf200) % 0x50, 0x21..=0x24 | 0x32 | 0x45)
}

/// Octets différents entre la RAM du Z80 et l'image du pilote. Une voie libre n'a de sens que par
/// son état et sa priorité : le reste est périmé et n'est pas comparé.
fn differences(z: &[u8], r: &[u8]) -> Vec<usize> {
    (0..0x1000)
        .filter(|&i| {
            if z[i] == r[i] || ignored(i) {
                return false;
            }
            let a = 0xf000 + i;
            if (0xf200..0xfc00).contains(&a) {
                let start = i - (a - 0xf200) % 0x50;
                if z[start] == 0 && r[start] == 0 {
                    return matches!((a - 0xf200) % 0x50, 0x00 | 0x40);
                }
            }
            if (0xfc00..0xfc48).contains(&a) {
                let start = i - (a - 0xfc00) % 0x18;
                if z[start + 2] | z[start + 3] == 0 && r[start + 2] | r[start + 3] == 0 {
                    return matches!((a - 0xfc00) % 0x18, 0x02 | 0x03 | 0x10);
                }
            }
            true
        })
        .collect()
}

/// Nom d'un octet de la RAM du Z80.
fn describe(i: usize) -> String {
    let a = 0xf000 + i;
    match a {
        0xf200..=0xf6ff => format!("voie d'effet {}, +{:#04x}", (a - 0xf200) / 0x50, (a - 0xf200) % 0x50),
        0xf700..=0xfbff => format!("voie de musique {}, +{:#04x}", (a - 0xf700) / 0x50, (a - 0xf700) % 0x50),
        0xfc00..=0xfc47 => format!("voie ADPCM {}, +{:#04x}", (a - 0xfc00) / 0x18, (a - 0xfc00) % 0x18),
        _ => "globale".to_string(),
    }
}

fn print_writes(who: &str, w: &[Write]) {
    for x in w {
        println!("{who} {x:?}");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage : fuzz <roms> songs|random|one ... (voir l'en-tête du fichier)");
        std::process::exit(2);
    }
    let rom = Rom::load(Path::new(&a[1])).expect("rom son");
    let num = |i: usize| a[i].parse::<usize>().expect("nombre");
    match a[2].as_str() {
        "songs" => {
            let (first, last, irqs) = (num(3), num(4), num(5));
            run_many(&rom, first..=last, |n| song_session(&rom, n, irqs));
        }
        "random" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| random_session(&rom, seed, i, irqs));
        }
        "synth" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| synth_session(&rom, seed, i, irqs));
        }
        "short" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| synth_short_session(&rom, seed, i, irqs));
        }
        "one" => {
            // one songs <numéro> <interruptions> [de a] | one random <graine> <essai> <interruptions> [de a]
            let dump_at = |i| (a.len() > i + 1).then(|| (num(i), num(i + 1)));
            let (s, dump) = match a[3].as_str() {
                "songs" => (song_session(&rom, num(4), num(5)), dump_at(6)),
                "synth" => (synth_session(&rom, num(4), num(5), num(6)), dump_at(7)),
                "short" => (synth_short_session(&rom, num(4), num(5), num(6)), dump_at(7)),
                _ => (random_session(&rom, num(4), num(5), num(6)), dump_at(7)),
            };
            for (at, c) in &s.commands {
                println!("interruption {at} : commande {c:02x?}");
            }
            if std::env::var("FUZZ_SHOW").is_ok() {
                for (at, bytes) in s.patches.iter().filter(|p| p.0 >= 0x48000) {
                    println!("données à la région {at:#x} (adresse linéaire {:#x}) : {}", at - 0x8000, bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
                }
            }
            summary(&run_session(&rom, &s, dump, None));
        }
        _ => eprintln!("mode inconnu"),
    }
}

/// Essai « une musique » : la commande, puis un deuxième son et une commande système.
fn song_session(rom: &Rom, n: usize, irqs: usize) -> Session {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ (n as u64 + 1) * 0x1234_5679);
    let mut commands = vec![(12, play_command(&mut rng, n as u16))];
    let other = rng.below(u64::from(song_count(rom))) as u16;
    commands.push((irqs / 3, play_command(&mut rng, other)));
    commands.push((2 * irqs / 3, system_command(&mut rng)));
    Session { patches: Vec::new(), stereo: if n % 5 == 0 { 0 } else { 0xff }, commands, irqs }
}

/// Essai aléatoire : une suite de commandes de tous types.
fn random_session(rom: &Rom, seed: usize, i: usize, irqs: usize) -> Session {
    let mut rng = Rng(0x2545_f491 ^ (seed as u64 + 1) * 0x9e37_79b9 + i as u64 * 0x85eb_ca6b);
    let mut commands = Vec::new();
    let mut at = 12;
    for _ in 0..4 + rng.below(8) {
        let song = rng.below(u64::from(song_count(rom))) as u16;
        let c = if rng.below(4) == 0 { system_command(&mut rng) } else { play_command(&mut rng, song) };
        commands.push((at, c));
        at += rng.below(irqs as u64 / 6 + 1) as usize;
    }
    Session { patches: Vec::new(), stereo: if rng.below(4) == 0 { 0 } else { 0xff }, commands, irqs }
}

/// Essai avec des musiques et effets synthétiques (octets aléatoires mais bien formés), ajoutés
/// dans une zone libre de la ROM et référencés par les numéros 1100 et suivants.
fn synth_session(rom: &Rom, seed: usize, i: usize, irqs: usize) -> Session {
    synth_session_with(rom, seed, i, irqs, 160)
}

/// Pistes courtes : une divergence est alors plus facile à localiser.
fn synth_short_session(rom: &Rom, seed: usize, i: usize, irqs: usize) -> Session {
    synth_session_with(rom, seed, i, irqs, 8)
}

fn synth_session_with(rom: &Rom, seed: usize, i: usize, irqs: usize, track_len: u64) -> Session {
    let mut rng = Rng(0x3c6e_f372 ^ (seed as u64 + 1) * 0x9e37_79b9 + i as u64 * 0xc2b2_ae35);
    let mut patches = Vec::new();
    let mut numbers = Vec::new();
    // Une fois sur deux les données chevauchent une limite de banque (4C000), pour exercer le
    // passage à la banque suivante en cours de lecture.
    let mut at = if rng.below(2) == 0 { 0x48000 } else { 0x4c000 - 0x20 - rng.below(0x300) as usize };
    for n in 0..6 {
        let data = synth_song(&mut rng, track_len);
        if at + data.len() >= 0x4f000 {
            break;
        }
        let number = 1100 + n as u16;
        // Entrée de la table des musiques : adresse linéaire sur 3 octets.
        let linear = (at - 0x8000) as u32;
        patches.push((0x9006 + 0x8000 + 4 * usize::from(number), vec![(linear >> 16) as u8, (linear >> 8) as u8, linear as u8, 0]));
        at += data.len();
        patches.push((at - data.len(), data));
        numbers.push(number);
    }
    let mut commands = Vec::new();
    let mut when = 12;
    for _ in 0..3 + rng.below(8) {
        let song = numbers[rng.below(numbers.len() as u64) as usize];
        let c = if rng.below(5) == 0 { system_command(&mut rng) } else { play_command(&mut rng, song) };
        commands.push((when, c));
        when += rng.below(irqs as u64 / 5 + 1) as usize;
    }
    let _ = rom;
    Session { patches, stereo: if rng.below(4) == 0 { 0 } else { 0xff }, commands, irqs }
}

/// Définition d'un son synthétique : musique, effet à plusieurs voies, note simple ou ADPCM.
fn synth_song(rng: &mut Rng, track_len: u64) -> Vec<u8> {
    let multi = |rng: &mut Rng, kind: u8| {
        let mut d = vec![kind];
        d.extend([0u8; 32]);
        for v in 0..16 {
            if rng.below(3) != 0 {
                let offset = d.len() as u16;
                d[1 + 2 * v] = (offset >> 8) as u8;
                d[2 + 2 * v] = offset as u8;
                d.extend(synth_track(rng, track_len));
            }
        }
        d
    };
    match rng.below(100) {
        0..=24 => multi(rng, 0),
        25..=54 => {
            let priority = 1 + rng.below(0x7f) as u8;
            multi(rng, priority)
        }
        55..=84 => {
            // Note simple : voie, priorité (bit 7 : continue), durée, tenue, amplitude, instrument, note, détune.
            let mut d = vec![0x80 | rng.below(16) as u8, rng.byte() & if rng.below(4) == 0 { 0xff } else { 0x7f }];
            let duration = 8 + rng.below(300) as u16;
            d.extend(duration.to_be_bytes());
            let gate = rng.below(u64::from(duration) + 20) as u16;
            d.extend(gate.to_be_bytes());
            d.push(rng.below(128) as u8);
            let instrument = if rng.below(2) == 0 {
                rng.below(290) as u16
            } else {
                0x8000 | (rng.below(2) as u16) << 8 | u16::from(rng.byte())
            };
            d.extend(instrument.to_be_bytes());
            d.push(rng.below(90) as u8);
            d.push(rng.byte());
            d
        }
        _ => {
            // Voie ADPCM : priorité, 4 octets ignorés, amplitude, échantillon.
            let mut d = vec![0x90 + rng.below(3) as u8, rng.byte() & if rng.below(3) == 0 { 0xff } else { 0x7f }];
            d.extend([rng.byte(), rng.byte(), rng.byte(), rng.byte()]);
            d.push(rng.below(128) as u8);
            d.extend((rng.below(290) as u16).to_be_bytes());
            d
        }
    }
}

/// Piste synthétique : notes et instructions avec des paramètres plausibles. Les boucles
/// contiennent toujours une note : le Z80 d'origine met un temps réel à parcourir une boucle sans
/// note (une boucle de 20 000 tours le bloque pendant des dizaines de tics), ce que le pilote Rust,
/// qui n'a pas de notion de durée d'exécution, fait instantanément.
fn synth_track(rng: &mut Rng, track_len: u64) -> Vec<u8> {
    let mut d = Vec::new();
    // Un instrument d'abord, pour que les notes sonnent.
    d.extend([8, rng.byte()]);
    // Début des instructions et des notes déjà écrites ; position de la dernière note.
    let mut starts: Vec<usize> = Vec::new();
    let mut last_note: Option<usize> = None;
    for _ in 0..4 + rng.below(track_len) {
        starts.push(d.len());
        match rng.below(100) {
            0..=49 => {
                last_note = Some(d.len());
                d.push(0x20 + rng.below(0xe0) as u8);
            }
            50..=54 => d.push(0x20 * (1 + rng.below(7) as u8)),
            _ => {
                let op = rng.below(32) as u8;
                match op {
                    0..=3 | 23 => d.push(op),
                    5 => {
                        d.push(op);
                        d.extend((0x80 + rng.below(0x600) as u16).to_be_bytes());
                    }
                    9 => d.extend([op, rng.below(8) as u8]),
                    // Boucle : retour à une instruction qui précède une note.
                    14..=17 => {
                        let Some(note) = last_note else { continue };
                        let candidates: Vec<usize> = starts.iter().copied().filter(|&s| s <= note).collect();
                        let target = candidates[rng.below(candidates.len() as u64) as usize];
                        let after = d.len() + 4;
                        d.extend([op, 1 + rng.below(3) as u8]);
                        d.extend((target as i64 - after as i64).to_be_bytes()[6..].iter());
                    }
                    // Sortie de boucle et saut : vers l'avant seulement.
                    18..=21 => {
                        d.extend([op, rng.byte() & 0x68]);
                        d.extend((1 + rng.below(30) as u16).to_be_bytes());
                    }
                    22 => {
                        d.push(op);
                        d.extend((1 + rng.below(30) as u16).to_be_bytes());
                    }
                    24 => d.extend([op, rng.below(0x21) as u8]),
                    31 => d.extend([op, rng.below(2) as u8]),
                    _ => d.extend([op, rng.byte()]),
                }
            }
        }
    }
    if rng.below(2) == 0 {
        d.push(23);
    }
    d
}

fn summary(r: &Report) {
    println!(
        "Z80 {} écritures, Rust {}, identiques {} (+{} acquittements), débordements {}",
        r.real.total, r.drv.total, r.matched_main, r.matched_acks, r.spill_intervals
    );
    match &r.first_divergence {
        None if r.identical() => println!("IDENTIQUE"),
        None => println!("préfixe identique, longueurs différentes"),
        Some(d) => println!("DIVERGENCE : {d}"),
    }
}

/// Exécute les essais `range` en parallèle et résume.
fn run_many(rom: &Rom, range: std::ops::RangeInclusive<usize>, make: impl Fn(usize) -> Session + Sync) {
    let next = AtomicUsize::new(*range.start());
    let last = *range.end();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let results = std::sync::Mutex::new(Vec::new());
    let coverage = std::sync::Mutex::new(vec![false; 0x10000]);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i > last {
                    break;
                }
                let s = make(i);
                let mut cov = vec![false; 0x10000];
                let r = run_session(rom, &s, None, Some(&mut cov));
                for (c, x) in coverage.lock().unwrap().iter_mut().zip(&cov) {
                    *c |= x;
                }
                results.lock().unwrap().push((i, r.identical(), r.matched_main + r.matched_acks, r.first_divergence, r.slow));
            });
        }
    });
    if let Ok(path) = std::env::var("FUZZ_COVERAGE") {
        let bytes: Vec<u8> = coverage.into_inner().unwrap().iter().map(|&c| c as u8).collect();
        std::fs::write(path, bytes).unwrap();
    }
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    let slow = results.iter().filter(|r| r.4 && r.3.is_none()).count();
    let ok = results.iter().filter(|r| r.1 || (r.4 && r.3.is_none())).count() - slow;
    let writes: usize = results.iter().map(|r| r.2).sum();
    for (i, identical, _, div, slow) in &results {
        if !identical && !(*slow && div.is_none()) {
            println!("essai {i} : {}", div.as_deref().unwrap_or("longueurs différentes"));
        }
    }
    println!(
        "{ok} essais identiques sur {} ({writes} écritures comparées) ; {slow} abandonnés : le Z80 est resté occupé plus de {MAX_BUSY} interruptions (durée d'exécution non modélisée)",
        results.len()
    );
}
