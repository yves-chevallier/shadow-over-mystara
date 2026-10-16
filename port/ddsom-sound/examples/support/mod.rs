#![allow(dead_code)]
//! Outils communs aux exemples : comparaison de suites d'écritures et Z80 de référence.

use ddsom_sound::Write;
use std::collections::VecDeque;

/// Écritures d'un côté, avec le numéro d'interruption où elles ont eu lieu.
#[derive(Default)]
pub struct Side {
    /// Écritures de la boucle principale (QSound et rapport de pan en C005).
    pub main: VecDeque<(usize, Write)>,
    /// Acquittements de commande (C00F <- FF) de la routine d'interruption.
    pub acks: VecDeque<(usize, Write)>,
    pub total: usize,
    pub qsound: usize,
}

/// Résultats de la comparaison de deux exécutions.
#[derive(Default)]
pub struct Report {
    pub real: Side,
    pub drv: Side,
    pub matched_main: usize,
    pub matched_acks: usize,
    pub first_divergence: Option<String>,
    /// Interruptions où les écritures de la boucle principale coïncident exactement.
    pub exact_intervals: usize,
    /// Interruptions où le travail déborde sur la suivante dans l'un des deux côtés.
    pub spill_intervals: usize,
    /// Interruptions sans aucune écriture des deux côtés.
    pub idle_intervals: usize,
    /// Le Z80 a été occupé trop longtemps pour que le pilote Rust, qui ne modélise pas la durée
    /// d'exécution, puisse l'être comparé (essai abandonné).
    pub slow: bool,
}

/// L'acquittement de commande est écrit par la routine d'interruption, à l'instant de
/// l'interruption : son rang par rapport aux écritures de la boucle principale (dont le travail
/// peut déborder sur l'interruption suivante) dépend de la durée des instructions du Z80.
pub fn is_ack(w: &Write) -> bool {
    matches!(w, Write::Shared { offset: 0xf, value: 0xff })
}

impl Side {
    fn push(&mut self, irq: usize, writes: Vec<Write>) {
        self.total += writes.len();
        for w in writes {
            if matches!(w, Write::Qsound { .. }) {
                self.qsound += 1;
            }
            let q = if is_ack(&w) { &mut self.acks } else { &mut self.main };
            q.push_back((irq, w));
        }
    }
}

impl Report {
    /// Range les écritures d'une interruption (Z80 d'origine, pilote Rust) et compare.
    pub fn add_interval(&mut self, irq: usize, real: Vec<Write>, drv: Vec<Write>) {
        let main = |v: &[Write]| v.iter().filter(|w| !is_ack(w)).copied().collect::<Vec<_>>();
        if real.is_empty() && drv.is_empty() {
            self.idle_intervals += 1;
        } else if main(&real) == main(&drv) {
            self.exact_intervals += 1;
        }
        self.real.push(irq, real);
        self.drv.push(irq, drv);
        if self.real.main.len() != self.drv.main.len() {
            self.spill_intervals += 1;
        }
        self.drain();
    }

    /// Compare les écritures en attente des deux côtés, dans l'ordre.
    fn drain(&mut self) {
        for acks in [false, true] {
            loop {
                let (r, d) = if acks {
                    (self.real.acks.front().copied(), self.drv.acks.front().copied())
                } else {
                    (self.real.main.front().copied(), self.drv.main.front().copied())
                };
                let (Some((ri, r)), Some((di, d))) = (r, d) else { break };
                if r != d && self.first_divergence.is_none() {
                    let n = if acks { self.matched_acks } else { self.matched_main };
                    self.first_divergence = Some(format!(
                        "{} n°{n} : Z80 {r:?} (interruption {ri}) / pilote {d:?} (interruption {di})",
                        if acks { "acquittement" } else { "écriture" }
                    ));
                }
                if self.first_divergence.is_some() {
                    return;
                }
                if acks {
                    self.matched_acks += 1;
                    self.real.acks.pop_front();
                    self.drv.acks.pop_front();
                } else {
                    self.matched_main += 1;
                    self.real.main.pop_front();
                    self.drv.main.pop_front();
                }
            }
        }
    }

    /// Écritures de la boucle principale que l'un des deux côtés n'a pas encore produites quand
    /// l'essai s'arrête (le Z80 est au milieu d'un tic, ou le pilote a une interruption d'avance).
    pub fn tail(&self) -> usize {
        self.real.main.len().abs_diff(self.drv.main.len())
    }

    /// Les deux suites sont-elles identiques de bout en bout, à l'inachevé final près ?
    pub fn identical(&self) -> bool {
        self.first_divergence.is_none() && self.tail() <= MAX_TAIL
    }
}

/// Un tic complet de 16 voies écrit au plus quelques dizaines de registres : au-delà, ce n'est
/// plus une simple fin d'essai en cours de tic.
const MAX_TAIL: usize = 120;

/// Z80 d'origine seul (sans 68000 ni QSound) : exécute la ROM du pilote et relève ses écritures.
/// La QSound répond toujours « prête ».
pub mod reference {
    use ddsom_sound::{Rom, Write};

    const IRQ_PERIOD: u64 = 32_000;

    struct Bus {
        region: Vec<u8>,
        bank: usize,
        ram: Vec<u8>,
        shared: Vec<u8>,
        latch: [u8; 2],
        out: Vec<Write>,
        irq_acked: bool,
    }

    impl z80::Bus for Bus {
        fn read(&mut self, a: u16) -> u8 {
            match a {
                0x0000..=0x7fff => self.region[a as usize],
                0x8000..=0xbfff => {
                    let i = 0x10000 + self.bank * 0x4000 + (a as usize - 0x8000);
                    *self.region.get(i).unwrap_or(&0xff)
                }
                0xc000..=0xcfff => self.shared[(a - 0xc000) as usize],
                0xd007 => 0x80,
                0xf000..=0xffff => self.ram[(a - 0xf000) as usize],
                _ => 0xff,
            }
        }
        fn write(&mut self, a: u16, v: u8) {
            match a {
                0xc000..=0xcfff => {
                    self.shared[(a - 0xc000) as usize] = v;
                    self.out.push(Write::Shared { offset: a - 0xc000, value: v });
                }
                0xd000 | 0xd001 => self.latch[(a - 0xd000) as usize] = v,
                0xd002 => self.out.push(Write::Qsound { reg: v, data: u16::from(self.latch[0]) << 8 | u16::from(self.latch[1]) }),
                0xd003 => {
                    let bank = (v & 0x0f) as usize;
                    self.bank = if 0x10000 + bank * 0x4000 >= self.region.len() { 0 } else { bank };
                }
                0xf000..=0xffff => self.ram[(a - 0xf000) as usize] = v,
                _ => {}
            }
        }
        fn input(&mut self, _port: u16) -> u8 {
            0xff
        }
        fn output(&mut self, _port: u16, _v: u8) {}
        fn irq_ack(&mut self) -> u8 {
            self.irq_acked = true;
            0xff
        }
    }

    pub struct RefZ80 {
        cpu: z80::Z80,
        bus: Bus,
        next_irq: u64,
        /// Adresses d'instructions exécutées (couverture du programme du Z80).
        pub coverage: Vec<bool>,
        /// Nombre d'exécutions de chaque adresse depuis la dernière remise à zéro (profil).
        pub hits: Vec<u32>,
        /// Point d'arrêt de débogage : affiche IX et l'enregistrement de voie.
        pub break_at: Option<u16>,
        break_count: usize,
        /// Surveille une adresse de la RAM du Z80 (débogage).
        watch: Option<u16>,
    }

    impl RefZ80 {
        pub fn new(rom: &Rom) -> Self {
            let mut cpu = z80::Z80::new();
            cpu.reset();
            let bus = Bus {
                region: rom.region().to_vec(),
                bank: 0,
                ram: vec![0; 0x1000],
                shared: vec![0; ddsom_sound::SHARED_SIZE],
                latch: [0; 2],
                out: Vec::new(),
                irq_acked: false,
            };
            RefZ80 { cpu, bus, next_irq: IRQ_PERIOD, coverage: vec![false; 0x10000], hits: vec![0; 0x10000], break_at: std::env::var("FUZZ_BREAK").ok().and_then(|v| u16::from_str_radix(&v, 16).ok()), break_count: 0, watch: std::env::var("FUZZ_WATCH").ok().and_then(|v| u16::from_str_radix(&v, 16).ok()) }
        }

        /// RAM partagée vue par le Z80 (le 68000 y écrit).
        pub fn shared(&mut self) -> &mut [u8] {
            &mut self.bus.shared
        }

        /// La boucle principale est-elle au repos (le Z80 vient d'être interrompu dans sa boucle
        /// d'attente) ? On le lit dans l'adresse de retour empilée par l'interruption.
        pub fn is_idle(&self) -> bool {
            let sp = usize::from(self.cpu.sp);
            if sp < 0xf000 {
                return false;
            }
            let ret = u16::from(self.bus.ram[sp - 0xf000]) | u16::from(self.bus.ram[sp - 0xf000 + 1]) << 8;
            let pending = self.bus.ram[1] | self.bus.ram[2] | self.bus.ram[3];
            (0x017d..=0x019a).contains(&ret) && !matches!(ret, 0x0185 | 0x0190 | 0x019b) && pending == 0
        }

        /// RAM du Z80 (F000-FFFF) : pour le débogage.
        pub fn ram(&self) -> &[u8] {
            &self.bus.ram
        }

        /// Exécute jusqu'à l'acquittement de l'interruption suivante (non compris) et rend les
        /// écritures faites depuis le précédent. Les interruptions demandées pendant que le Z80
        /// les masque (initialisation) ne produisent qu'un seul acquittement.
        pub fn run_interval(&mut self) -> Vec<Write> {
            loop {
                if self.cpu.cycles >= self.next_irq {
                    self.cpu.irq_line = true;
                    self.next_irq += IRQ_PERIOD;
                }
                self.coverage[usize::from(self.cpu.pc)] = true;
                self.hits[usize::from(self.cpu.pc)] += 1;
                if let Some(b) = self.break_at {
                    if self.cpu.pc == b && std::env::var("FUZZ_BREAK_SHORT").is_ok() {
                        println!("  [{b:04x}] cycle {} f000 {:02x} f001-3 {:02x?} file {:02x}/{:02x}", self.cpu.cycles, self.bus.ram[0], &self.bus.ram[1..4], self.bus.ram[0x15], self.bus.ram[0x16]);
                    } else if self.cpu.pc == b && self.break_count < 6 {
                        self.break_count += 1;
                        let ix = usize::from(self.cpu.ix);
                        let rec: Vec<String> = (0..0x50).map(|i| format!("{:02x}", self.bus.ram.get(ix + i - 0xf000).copied().unwrap_or(0))).collect();
                        println!("point d'arrêt {b:04x} : IX={ix:04x} DE={:04x} enregistrement {}", self.cpu.de(), rec.join(" "));
                    }
                }
                let watch_before = self.watch.map(|w| self.bus.ram[usize::from(w) - 0xf000]);
                let pc_before = self.cpu.pc;
                self.cpu.step(&mut self.bus);
                if let (Some(w), Some(before)) = (self.watch, watch_before) {
                    let after = self.bus.ram[usize::from(w) - 0xf000];
                    if after != before {
                        println!("  [écriture {w:04x}] pc {pc_before:04x} cycle {} : {before:02x} -> {after:02x}", self.cpu.cycles);
                    }
                }
                if self.bus.irq_acked {
                    self.cpu.irq_line = false;
                    self.bus.irq_acked = false;
                    return std::mem::take(&mut self.bus.out);
                }
            }
        }
    }
}
