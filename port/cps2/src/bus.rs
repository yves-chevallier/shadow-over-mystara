//! Carte mémoire du 68000 CPS2 (cps2_state::cps2_map de MAME).

use std::sync::Arc;

use m68k::decode::Fetch;

use crate::eeprom::Eeprom93c46;

/// Entrées du panneau (actives à 0, comme sur le matériel).
#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub in0: u16,
    pub in1: u16,
    /// Bits 1..15 de IN2 (le bit 0 vient de l'EEPROM).
    pub in2: u16,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs { in0: 0xffff, in1: 0xffff, in2: 0xffff }
    }
}

/// État d'un jeu porté gardé hors de la RAM émulée (par exemple la table des tâches en Rust de ddsom) : opaque pour
/// la machine, copié avec le bus.
pub trait Extension: std::any::Any + Send {
    fn clone_box(&self) -> Box<dyn Extension>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any + Send + Clone> Extension for T {
    fn clone_box(&self) -> Box<dyn Extension> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Clone for Box<dyn Extension> {
    fn clone(&self) -> Self {
        (**self).clone_box()
    }
}

#[derive(Clone)]
pub struct Cps2Bus {
    /// ROM programme, vue données (en clair), mots big-endian. Partagée entre copies du bus.
    pub rom: Arc<Vec<u16>>,
    /// ROM programme, vue espace programme (opcodes et lectures relatives au PC déchiffrés).
    pub rom_dec: Arc<Vec<u16>>,
    pub ram: Vec<u16>,       // FF0000-FFFFFF
    pub extra_ram: Vec<u16>, // 660000-664001
    pub gfxram: Vec<u16>,    // 900000-92FFFF
    pub objram: [Vec<u16>; 2],
    pub objram_bank: u16,
    pub output: [u16; 6], // 400000-40000B
    pub cps_a: [u16; 32],
    pub cps_b: [u16; 32],
    /// RAM partagée avec le Z80 (C000-CFFF côté Z80), un octet par mot côté 68000.
    pub qram: Vec<u8>,
    pub eeprom: Eeprom93c46,
    pub eeprom_out: u16,
    pub inputs: Inputs,
    /// Palette copiée à l'écriture du registre CPS-A 0x0a (6 pages de 0x200 mots).
    pub palette: Vec<u16>,
    /// Ligne vblank/raster IRQ demandée (niveau 68000), effacée à l'acquittement.
    pub irq_vblank: bool,
    pub irq_raster: bool,
    /// Compteurs raster CPS-B (registres 0x0e/0x10/0x12).
    pub raster_counter: [u16; 3],
    pub raster_reload: [u16; 3],
    /// Ligne courante (pour la lecture des compteurs raster).
    pub scanline: u32,
    /// Image en cours depuis la mise sous tension (`Machine::frame`) : avec `scanline`, position du faisceau.
    pub frame: u64,
    /// Le Z80 est maintenu en reset tant que le bit 3 de 804041 vaut 0.
    pub z80_running: bool,
    /// Premier cycle CPU où une interruption peut être levée (fin d'image, ou fin de ligne si les
    /// compteurs raster sont armés). Un traitement atomique qui s'achève avant reste exact.
    pub irq_horizon: u64,
    /// État du jeu porté hors de la RAM émulée (voir [`Extension`]).
    pub ext: Option<Box<dyn Extension>>,
}

impl Cps2Bus {
    pub fn new(rom: Vec<u16>, rom_dec: Vec<u16>, eeprom: Eeprom93c46) -> Self {
        Cps2Bus {
            rom: Arc::new(rom),
            rom_dec: Arc::new(rom_dec),
            ram: vec![0; 0x8000],
            extra_ram: vec![0; 0x2001],
            gfxram: vec![0; 0x18000],
            objram: [vec![0; 0x1000], vec![0; 0x1000]],
            objram_bank: 0,
            output: [0; 6],
            cps_a: [0; 32],
            cps_b: [0; 32],
            qram: vec![0; 0x1000],
            eeprom,
            eeprom_out: 0,
            inputs: Inputs::default(),
            palette: vec![0; 6 * 0x200],
            irq_vblank: false,
            irq_raster: false,
            raster_counter: [0; 3],
            raster_reload: [0x1ff, 0x1ff, 0],
            scanline: 0,
            frame: 0,
            // le Z80 démarre à la mise sous tension ; le 68000 le met en reset au boot
            z80_running: true,
            irq_horizon: 0,
            ext: None,
        }
    }

    /// Niveau d'interruption présenté au 68000 (IPL1 = vblank -> 2, IPL2 = raster -> 4, les deux -> 6).
    pub fn irq_level(&self) -> u8 {
        (self.irq_vblank as u8) * 2 + (self.irq_raster as u8) * 4
    }

    fn latch_palette(&mut self) {
        let base = ((self.cps_a[0x0a / 2] as usize * 256) & !0x3ff) & 0x3ffff;
        let n = self.gfxram.len();
        for i in 0..self.palette.len() {
            self.palette[i] = self.gfxram[(base / 2 + i) % n];
        }
    }

    fn read_word(&mut self, a: u32) -> u16 {
        match a {
            0x000000..=0x3fffff => self.rom[(a >> 1) as usize],
            0x400000..=0x40000b => self.output[((a - 0x400000) >> 1) as usize],
            0x618000..=0x619fff => 0xff00 | self.qram[((a - 0x618000) >> 1) as usize] as u16,
            0x660000..=0x664001 => self.extra_ram[((a - 0x660000) >> 1) as usize],
            0x708000..=0x70ffff => {
                let bank = (self.objram_bank ^ 1) as usize & 1;
                self.objram[bank][((a & 0x1fff) >> 1) as usize]
            }
            0x800140..=0x80017f => self.cps_b_read(((a - 0x800140) >> 1) as usize),
            0x804140..=0x80417f => self.cps_b_read(((a - 0x804140) >> 1) as usize),
            0x804000..=0x804001 => self.inputs.in0,
            0x804010..=0x804011 => self.inputs.in1,
            0x804020..=0x804021 => (self.inputs.in2 & !1) | self.eeprom.do_read() as u16,
            0x804030..=0x804031 => 0xe021, // volume numérique au maximum, pas de RAM additionnelle
            0x8040b0..=0x8040b3 => 0xffff, // DIP (cartes de développement seulement)
            0x900000..=0x92ffff => self.gfxram[((a - 0x900000) >> 1) as usize],
            0xff0000..=0xffffff => self.ram[((a - 0xff0000) >> 1) as usize],
            _ => 0, // non mappé
        }
    }

    fn cps_b_read(&self, off: usize) -> u16 {
        match off {
            // multiplicateur matériel (CPS_B_21_DEF : facteurs 0x00/0x02, résultat 0x04/0x06)
            2 => (self.cps_b[0] as u32 * self.cps_b[1] as u32) as u16,
            3 => ((self.cps_b[0] as u32 * self.cps_b[1] as u32) >> 16) as u16,
            7 => ((self.raster_counter[2] << 1) & 0x1fe) | (self.cps_b[7] & 1),
            8 | 9 => self.raster_counter[off & 1] & 0x1ff,
            _ => 0xffff,
        }
    }

    fn cps_b_write(&mut self, off: usize, v: u16, mask: u16) {
        let v = (self.cps_b[off] & !mask) | (v & mask);
        self.cps_b[off] = v;
        match off {
            7 => self.raster_reload[2] = (v >> 1) & 0xff,
            8 | 9 => {
                self.raster_reload[off & 1] = v & 0x1ff;
                if v & 0x8000 != 0 {
                    self.raster_counter[off & 1] = self.raster_reload[off & 1];
                }
            }
            _ => {}
        }
    }

    /// Écriture d'un mot avec masque d'octets (COMBINE_DATA).
    fn write_word(&mut self, a: u32, v: u16, mask: u16) {
        let comb = |old: u16| (old & !mask) | (v & mask);
        match a {
            0x400000..=0x40000b => {
                let i = ((a - 0x400000) >> 1) as usize;
                self.output[i] = comb(self.output[i]);
            }
            0x618000..=0x619fff => {
                if mask & 0xff != 0 {
                    self.qram[((a - 0x618000) >> 1) as usize] = v as u8;
                }
            }
            0x660000..=0x664001 => {
                let i = ((a - 0x660000) >> 1) as usize;
                self.extra_ram[i] = comb(self.extra_ram[i]);
            }
            0x700000..=0x701fff => {
                let bank = self.objram_bank as usize & 1;
                let i = ((a & 0x1fff) >> 1) as usize;
                self.objram[bank][i] = comb(self.objram[bank][i]);
            }
            0x708000..=0x70ffff => {
                let bank = (self.objram_bank ^ 1) as usize & 1;
                let i = ((a & 0x1fff) >> 1) as usize;
                self.objram[bank][i] = comb(self.objram[bank][i]);
            }
            0x800100..=0x80013f | 0x804100..=0x80413f => {
                let i = ((a & 0x3f) >> 1) as usize;
                self.cps_a[i] = comb(self.cps_a[i]);
                if i == 0x0a / 2 {
                    self.latch_palette();
                }
            }
            0x800140..=0x80017f | 0x804140..=0x80417f => self.cps_b_write(((a & 0x3f) >> 1) as usize, v, mask),
            0x804040..=0x804041 => {
                self.eeprom_out = comb(self.eeprom_out);
                if mask & 0xff00 != 0 {
                    let d = self.eeprom_out;
                    self.eeprom.write_lines(d & 0x1000 != 0, d & 0x2000 != 0, d & 0x4000 != 0);
                }
                if mask & 0x00ff != 0 {
                    self.z80_running = self.eeprom_out & 0x08 != 0;
                }
            }
            0x8040e0..=0x8040e1 => {
                if mask & 0xff != 0 {
                    self.objram_bank = v & 1;
                }
            }
            0x900000..=0x92ffff => {
                let i = ((a - 0x900000) >> 1) as usize;
                self.gfxram[i] = comb(self.gfxram[i]);
            }
            0xff0000..=0xffffff => {
                let i = ((a - 0xff0000) >> 1) as usize;
                self.ram[i] = comb(self.ram[i]);
            }
            _ => {}
        }
    }
}

impl Fetch for Cps2Bus {
    fn fetch16(&mut self, a: u32) -> u16 {
        let a = a & 0xff_ffff;
        if a < 0x400000 {
            self.rom_dec[(a >> 1) as usize]
        } else {
            self.read_word(a & !1)
        }
    }
}

impl m68k::Bus for Cps2Bus {
    fn read8(&mut self, a: u32) -> u8 {
        let w = self.read_word(a & !1);
        if a & 1 == 0 { (w >> 8) as u8 } else { w as u8 }
    }
    fn read16(&mut self, a: u32) -> u16 {
        self.read_word(a & !1)
    }
    fn write8(&mut self, a: u32, v: u8) {
        if a & 1 == 0 {
            self.write_word(a & !1, (v as u16) << 8, 0xff00);
        } else {
            self.write_word(a & !1, v as u16, 0x00ff);
        }
    }
    fn write16(&mut self, a: u32, v: u16) {
        self.write_word(a & !1, v, 0xffff);
    }
    fn irq_ack(&mut self, _level: u8) {
        // l'acquittement efface IPL1 et IPL2 (cps_state::irqack_r)
        self.irq_vblank = false;
        self.irq_raster = false;
    }
}
