//! Sous-système son CPS2 : Z80 à 8 MHz + QSound (qsound_sub_map de MAME).
//!
//! Z80 : 0000-7FFF ROM, 8000-BFFF banque de ROM, C000-CFFF RAM partagée avec le 68000,
//! D000-D002 écritures QSound, D003 banque, D007 état QSound, F000-FFFF RAM.

use qsound::QSound;

/// Z80 : 8 MHz.
pub const Z80_HZ: f64 = 8_000_000.0;
/// Interruption périodique : 8 MHz / 32000 = 250 Hz (irq0_line_hold).
pub const Z80_IRQ_PERIOD: u64 = 32_000;

/// Événement observable du Z80 (outils d'analyse et de validation du pilote son).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Interruption acceptée.
    Irq,
    /// Lecture du Z80 dans la RAM partagée (adresse 0-0xFFF, valeur lue).
    Read(u16, u8),
    /// Écriture du Z80 dans la RAM partagée.
    Write(u16, u8),
    /// Écriture du Z80 en D000-D003 : (décalage, octet). D000/D001 chargent le verrou de données,
    /// D002 écrit le registre QSound, D003 choisit la banque de ROM.
    Port(u8, u8),
}

pub struct Sound {
    /// Journal des événements, si activé.
    pub trace: Option<Vec<Event>>,
    /// Rappel avant chaque instruction du Z80 (couverture), si activé.
    pub pc_hook: Option<Box<dyn FnMut(&z80::Z80)>>,
    pub z80: z80::Z80,
    /// Région audiocpu (0x50000) : 0-7FFF fixe, banques de 0x4000 à partir de 0x10000.
    pub rom: Vec<u8>,
    pub bank: usize,
    pub ram: Vec<u8>, // F000-FFFF
    pub qsound: QSound,
    /// Le 68000 maintient le Z80 en reset (bit 3 de 804041 à 0).
    pub in_reset: bool,
    pub next_irq: u64,
    /// Échantillons stéréo produits (à vider par l'application).
    pub audio: Vec<[i16; 2]>,
    sample_acc: f64,
    irq_acked: bool,
}

/// Vue du bus Z80 : la RAM partagée appartient au bus 68000.
pub struct Z80Bus<'a> {
    pub s: &'a mut Sound,
    pub qram: &'a mut [u8],
}

impl Sound {
    pub fn new(rom: Vec<u8>, qsound: QSound) -> Self {
        let mut z80 = z80::Z80::new();
        z80.reset();
        Sound {
            trace: None,
            pc_hook: None,
            z80,
            rom,
            bank: 0,
            ram: vec![0; 0x1000],
            qsound,
            in_reset: false,
            next_irq: Z80_IRQ_PERIOD,
            audio: Vec::new(),
            sample_acc: 0.0,
            irq_acked: false,
        }
    }

    pub fn set_reset(&mut self, asserted: bool) {
        if asserted && !self.in_reset {
            self.z80.reset();
        }
        self.in_reset = asserted;
    }

    /// Exécute le Z80 jusqu'au cycle absolu `target`, puis produit les échantillons QSound
    /// correspondant à `seconds` de temps machine.
    pub fn run(&mut self, qram: &mut [u8], target: u64, seconds: f64) {
        if self.in_reset {
            // horloge arrêtée côté programme, mais le temps passe
            if self.z80.cycles < target {
                self.z80.cycles = target;
            }
            while self.next_irq <= target {
                self.next_irq += Z80_IRQ_PERIOD;
            }
        }
        // le Z80 est sorti de Sound le temps de l'exécution (le bus emprunte Sound)
        let mut cpu = std::mem::take(&mut self.z80);
        while cpu.cycles < target {
            if cpu.cycles >= self.next_irq {
                cpu.irq_line = true;
                self.next_irq += Z80_IRQ_PERIOD;
            }
            if let Some(h) = self.pc_hook.as_mut() {
                h(&cpu);
            }
            let mut bus = Z80Bus { s: self, qram };
            cpu.step(&mut bus);
            if self.irq_acked {
                cpu.irq_line = false;
                self.irq_acked = false;
            }
        }
        self.z80 = cpu;
        self.sample_acc += seconds * qsound::SAMPLE_RATE;
        while self.sample_acc >= 1.0 {
            self.sample_acc -= 1.0;
            self.qsound.update_sample();
            self.audio.push(self.qsound.out);
        }
    }
}

impl z80::Bus for Z80Bus<'_> {
    fn read(&mut self, a: u16) -> u8 {
        match a {
            0x0000..=0x7fff => self.s.rom[a as usize],
            0x8000..=0xbfff => {
                let i = 0x10000 + self.s.bank * 0x4000 + (a as usize - 0x8000);
                *self.s.rom.get(i).unwrap_or(&0xff)
            }
            0xc000..=0xcfff => {
                let v = self.qram[(a - 0xc000) as usize];
                if let Some(t) = self.s.trace.as_mut() {
                    t.push(Event::Read(a - 0xc000, v));
                }
                v
            }
            0xd007 => self.s.qsound.read(),
            0xf000..=0xffff => self.s.ram[(a - 0xf000) as usize],
            _ => 0xff,
        }
    }
    fn write(&mut self, a: u16, v: u8) {
        if let (Some(t), 0xd000..=0xd003) = (self.s.trace.as_mut(), a) {
            t.push(Event::Port((a - 0xd000) as u8, v));
        }
        match a {
            0xc000..=0xcfff => {
                self.qram[(a - 0xc000) as usize] = v;
                if let Some(t) = self.s.trace.as_mut() {
                    t.push(Event::Write(a - 0xc000, v));
                }
            }
            0xd000..=0xd002 => self.s.qsound.write((a - 0xd000) as u8, v),
            0xd003 => {
                let bank = (v & 0x0f) as usize;
                self.s.bank = if 0x10000 + bank * 0x4000 >= self.s.rom.len() { 0 } else { bank };
            }
            0xf000..=0xffff => self.s.ram[(a - 0xf000) as usize] = v,
            _ => {}
        }
    }
    fn input(&mut self, _port: u16) -> u8 {
        0xff
    }
    fn output(&mut self, _port: u16, _v: u8) {}
    fn irq_ack(&mut self) -> u8 {
        // irq0_line_hold : la ligne retombe à l'acquittement
        self.s.irq_acked = true;
        if let Some(t) = self.s.trace.as_mut() {
            t.push(Event::Irq);
        }
        0xff
    }
}
