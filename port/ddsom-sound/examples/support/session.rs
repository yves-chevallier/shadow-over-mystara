//! Validation runs: simulated 68000 commands, original Z80 and Rust driver executed side by
//! side, comparison of the writes and of the internal state.

use super::{reference::RefZ80, Report, SoundRom as Rom};
use ddsom_sound::{Driver, Write, SHARED_SIZE};

/// Pseudo-random generator (xorshift64*).
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    pub fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
}

/// Number of consecutive interrupts during which the Z80 may stay busy before the run is
/// abandoned (a normal tick only needs two or three).
pub const MAX_BUSY: usize = 10;

/// What the 68000 does during a run.
pub struct Session {
    /// ROM patches: (offset in the audiocpu region, bytes). They add synthetic songs
    /// and effects that the game does not use.
    pub patches: Vec<(usize, Vec<u8>)>,
    /// Stereo setting (0 = mono).
    pub stereo: u8,
    /// Commands (interrupt from which posting is attempted).
    pub commands: Vec<(usize, [u8; 16])>,
    pub irqs: usize,
}

/// "Song or effect" command with plausible pan parameters.
pub fn play_command(rng: &mut Rng, number: u16) -> [u8; 16] {
    let mut c = [0u8; 16];
    c[0] = (number >> 8) as u8;
    c[1] = number as u8;
    for b in &mut c[2..6] {
        *b = rng.byte();
    }
    c[6] = rng.below(8) as u8; // pan evolution type
    c[7] = rng.below(0x28) as u8;
    c[8] = rng.below(0x28) as u8;
    for b in &mut c[9..13] {
        *b = if rng.below(2) == 0 { rng.below(8) as u8 } else { rng.byte() };
    }
    c[13] = rng.byte();
    c[14] = rng.byte();
    c
}

/// System command with plausible parameters.
pub fn system_command(rng: &mut Rng) -> [u8; 16] {
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

pub fn song_count(rom: &Rom) -> u16 {
    rom.data.sounds.len() as u16
}

pub fn run_session(rom: &Rom, s: &Session, dump: Option<(usize, usize)>, coverage: Option<&mut [bool]>) -> Report {
    let patched;
    let rom = if s.patches.is_empty() {
        rom
    } else {
        let mut region = rom.region.clone();
        for (at, bytes) in &s.patches {
            region[*at..*at + bytes.len()].copy_from_slice(bytes);
        }
        patched = Rom::from_region(region);
        &patched
    };
    let mut real = RefZ80::new(rom);
    let mut driver = Driver::new(rom.data.clone());
    let mut dshared = vec![0u8; SHARED_SIZE];
    let mut report = Report::default();
    let boot = real.run_interval();
    let mut mine = Vec::new();
    driver.reset(&mut dshared, &mut mine);
    report.add_interval(0, boot, mine);
    let mut next = 0;
    let mut busy = 0;
    for k in 1..=s.irqs {
        // The 68000 answers the handshake, prepares the mailbox, posts the commands.
        if k == 10 {
            for sh in [real.shared(), &mut dshared[..]] {
                sh[0xfff] = 0xff;
                sh[0xffd] = 0x88;
                sh[0xffe] = s.stereo;
                sh[0xf] = 0xff;
            }
        }
        // The 68000 only posts a command if the Z80 has caught up: otherwise the order between
        // the command and the pending work depends on the Z80 execution time.
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
                    "--- interrupt {k} (Z80: f000-f003 {:02x?} queue {:02x}/{:02x}; Rust: {:02x?} queue {:02x}/{:02x}; idle: {})",
                    &ram[..4], ram[0x15], ram[0x16], &img[..4], img[0x15], img[0x16], real.is_idle()
                );
                print_writes("Z80   ", &w_real);
                print_writes("Rust  ", &w_mine);
                if std::env::var("FUZZ_PROFILE").is_ok() {
                    let mut top: Vec<(u32, usize)> = real.hits.iter().enumerate().map(|(a, &n)| (n, a)).filter(|x| x.0 > 0).collect();
                    top.sort_by(|a, b| b.cmp(a));
                    let list: Vec<String> = top.iter().take(10).map(|(n, a)| format!("{a:04x}:{n}")).collect();
                    println!("  most executed addresses: {}", list.join(" "));
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
                    .map(|&i| format!("{:04x} ({}): Z80 {:02x} / Rust {:02x}", 0xf000 + i, describe(i), z[i], r[i]))
                    .collect();
                report.first_divergence = Some(format!("different state after interrupt {k}: {}", list.join(" ; ")));
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

/// Bytes of the Z80 RAM that the driver does not model (current bank, temporary
/// variables, stack, unused voice bytes).
pub fn ignored(i: usize) -> bool {
    let a = 0xf000 + i;
    matches!(a, 0xf005 | 0xf006 | 0xf007 | 0xf00d | 0xf01a) || (0xf050..0xf200).contains(&a) || (0xff00..0x10000).contains(&a)
        || (0xf200..0xfc00).contains(&a) && matches!((a - 0xf200) % 0x50, 0x21..=0x24 | 0x32 | 0x45)
}

/// Bytes that differ between the Z80 RAM and the driver image. A free voice only matters through
/// its state and priority: the rest is stale and is not compared.
pub fn differences(z: &[u8], r: &[u8]) -> Vec<usize> {
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

/// Name of a byte of the Z80 RAM.
pub fn describe(i: usize) -> String {
    let a = 0xf000 + i;
    match a {
        0xf200..=0xf6ff => format!("effect voice {}, +{:#04x}", (a - 0xf200) / 0x50, (a - 0xf200) % 0x50),
        0xf700..=0xfbff => format!("music voice {}, +{:#04x}", (a - 0xf700) / 0x50, (a - 0xf700) % 0x50),
        0xfc00..=0xfc47 => format!("ADPCM voice {}, +{:#04x}", (a - 0xfc00) / 0x18, (a - 0xfc00) % 0x18),
        _ => "global".to_string(),
    }
}

pub fn print_writes(who: &str, w: &[Write]) {
    for x in w {
        println!("{who} {x:?}");
    }
}

/// "One song" run: the command, then a second sound and a system command.
pub fn song_session(rom: &Rom, n: usize, irqs: usize) -> Session {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ ((n as u64 + 1) * 0x1234_5679));
    let mut commands = vec![(12, play_command(&mut rng, n as u16))];
    let other = rng.below(u64::from(song_count(rom))) as u16;
    commands.push((irqs / 3, play_command(&mut rng, other)));
    commands.push((2 * irqs / 3, system_command(&mut rng)));
    Session { patches: Vec::new(), stereo: if n.is_multiple_of(5) { 0 } else { 0xff }, commands, irqs }
}

/// Random run: a sequence of commands of all kinds.
pub fn random_session(rom: &Rom, seed: usize, i: usize, irqs: usize) -> Session {
    let mut rng = Rng(0x2545_f491 ^ ((seed as u64 + 1) * 0x9e37_79b9 + i as u64 * 0x85eb_ca6b));
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

/// Run with synthetic songs and effects (random but well-formed bytes), added
/// in a free area of the ROM and referenced by numbers 1100 and up.
pub fn synth_session(rom: &Rom, seed: usize, i: usize, irqs: usize) -> Session {
    synth_session_with(rom, seed, i, irqs, 160)
}

/// Short tracks: a divergence is then easier to locate.
pub fn synth_short_session(rom: &Rom, seed: usize, i: usize, irqs: usize) -> Session {
    synth_session_with(rom, seed, i, irqs, 8)
}

pub fn synth_session_with(rom: &Rom, seed: usize, i: usize, irqs: usize, track_len: u64) -> Session {
    let mut rng = Rng(0x3c6e_f372 ^ ((seed as u64 + 1) * 0x9e37_79b9 + i as u64 * 0xc2b2_ae35));
    let mut patches = Vec::new();
    let mut numbers = Vec::new();
    // Every other time the data straddles a bank boundary (4C000), to exercise the
    // switch to the next bank during reading.
    let mut at = if rng.below(2) == 0 { 0x48000 } else { 0x4c000 - 0x20 - rng.below(0x300) as usize };
    for n in 0..6 {
        let data = synth_song(&mut rng, track_len);
        if at + data.len() >= 0x4f000 {
            break;
        }
        let number = 1100 + n as u16;
        // Entry of the music table: 3-byte linear address.
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

/// Definition of a synthetic sound: song, multi-voice effect, simple note or ADPCM.
pub fn synth_song(rng: &mut Rng, track_len: u64) -> Vec<u8> {
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
            // Simple note: voice, priority (bit 7: continuous), duration, hold, amplitude, instrument, note, detune.
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
            // ADPCM voice: priority, 4 ignored bytes, amplitude, sample.
            let mut d = vec![0x90 + rng.below(3) as u8, rng.byte() & if rng.below(3) == 0 { 0xff } else { 0x7f }];
            d.extend([rng.byte(), rng.byte(), rng.byte(), rng.byte()]);
            d.push(rng.below(128) as u8);
            d.extend((rng.below(290) as u16).to_be_bytes());
            d
        }
    }
}

/// Synthetic track: notes and instructions with plausible parameters. Loops always
/// contain a note: the original Z80 takes real time to run through a noteless loop
/// (a 20,000-pass loop blocks it for dozens of ticks), which the Rust driver, having no notion
/// of execution time, does instantly.
pub fn synth_track(rng: &mut Rng, track_len: u64) -> Vec<u8> {
    let mut d = Vec::new();
    // An instrument first, so that the notes sound.
    d.extend([8, rng.byte()]);
    // Start of the instructions and notes already written; position of the last note.
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
                    // Loop: jump back to an instruction that precedes a note.
                    14..=17 => {
                        let Some(note) = last_note else { continue };
                        let candidates: Vec<usize> = starts.iter().copied().filter(|&s| s <= note).collect();
                        let target = candidates[rng.below(candidates.len() as u64) as usize];
                        let after = d.len() + 4;
                        d.extend([op, 1 + rng.below(3) as u8]);
                        d.extend((target as i64 - after as i64).to_be_bytes()[6..].iter());
                    }
                    // Loop exit and jump: forward only.
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

