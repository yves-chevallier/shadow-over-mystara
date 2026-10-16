//! Machine CPS2 : 68000 + bus + vidéo, cadencée ligne par ligne comme dans MAME.
//! Le Z80 / QSound n'est pas encore émulé : la RAM partagée est simulée au minimum.

pub mod bus;
pub mod crypt;
pub mod eeprom;
mod sboxes;

use std::io;
use std::path::Path;

pub use bus::{Cps2Bus, Inputs};
use eeprom::Eeprom93c46;
use m68k::Cpu;

/// Horloge 68000 : 16 MHz x 0,7375 (MAME compense ainsi les états d'attente non émulés).
pub const CPU_HZ: f64 = 16_000_000.0 * 0.7375;
/// 512 pixels par ligne à 8 MHz.
pub const LINE_HZ: f64 = 8_000_000.0 / 512.0;
pub const LINES: u32 = 262;
pub const VBLANK_LINE: u32 = 240;

/// Programme de ddsom (Europe 960619).
pub const DDSOM_PROG: [&str; 8] = [
    "dd2e.03e", "dd2e.04e", "dd2e.05e", "dd2e.06e", "dd2e.07", "dd2e.08", "dd2e.09", "dd2e.10",
];

pub struct Machine {
    pub cpu: Cpu,
    pub bus: Cps2Bus,
    pub gfx: Vec<u8>,
    pub frame: u64,
    /// Table d'objets verrouillée au dernier vblank (affichée à l'image suivante).
    pub obj_buffer: Vec<u16>,
    pub pri_ctrl_buffer: [u16; 6],
    line_cycles: f64,
    cycle_debt: f64,
}

impl Machine {
    /// Charge ddsom depuis un dossier de ROMs extraites. `eeprom` : contenu nvram MAME (128 octets).
    pub fn load_ddsom(romdir: &Path, eeprom: Option<&[u8]>) -> io::Result<Self> {
        let mut rom = Vec::with_capacity(0x200000);
        for name in DDSOM_PROG {
            let d = std::fs::read(romdir.join(name))?;
            // ROM_LOAD16_WORD_SWAP : mots stockés petit-boutiste dans la puce
            rom.extend(d.chunks_exact(2).map(|w| u16::from_le_bytes([w[0], w[1]])));
        }
        let key = std::fs::read(romdir.join("ddsom.key"))?;
        let (k, lower, upper) = crypt::parse_key(&key);
        let rom_dec = crypt::decrypt(&rom, k, lower, upper);
        let gfx = cps2_gfx::rom::load_gfx(romdir, cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE)?;
        let ee = match eeprom {
            Some(b) => Eeprom93c46::from_bytes(b),
            None => Eeprom93c46::new([0xffff; 64]),
        };
        let mut m = Machine {
            cpu: Cpu::new(),
            bus: Cps2Bus::new(rom, rom_dec, ee),
            gfx,
            frame: 0,
            obj_buffer: vec![0; 0x1000],
            pri_ctrl_buffer: [0; 6],
            line_cycles: CPU_HZ / LINE_HZ,
            cycle_debt: 0.0,
        };
        m.reset();
        Ok(m)
    }

    pub fn reset(&mut self) {
        self.cpu = Cpu::new();
        self.cpu.reset(&mut self.bus);
        // Z80 non émulé : il signale qu'il est prêt (le 68000 attend 0x77 en 619FFF au boot)
        self.bus.qram[0xfff] = 0x77;
    }

    /// Exécute le CPU pendant une ligne de balayage.
    fn run_line(&mut self) {
        self.cycle_debt += self.line_cycles;
        let target = self.cpu.cycles + self.cycle_debt as u64;
        self.cycle_debt -= self.cycle_debt.floor();
        while self.cpu.cycles < target {
            self.cpu.irq_level = self.bus.irq_level();
            self.cpu.step(&mut self.bus);
        }
    }

    /// Avance d'une image. Renvoie l'image rendue au début du vblank (384x224 RGB).
    pub fn run_frame(&mut self) -> Vec<[u8; 3]> {
        let mut image = Vec::new();
        for line in 0..LINES {
            self.bus.scanline = line;
            self.raster_tick(line);
            if line == VBLANK_LINE {
                image = self.render();
                // verrouillage de la table d'objets et des priorités (cps2_objram_latch)
                let bank = self.bus.objram_bank as usize & 1;
                self.obj_buffer.copy_from_slice(&self.bus.objram[bank]);
                self.pri_ctrl_buffer = self.bus.output;
                self.bus.irq_vblank = true;
            }
            self.run_line();
        }
        self.frame += 1;
        image
    }

    /// Compteurs raster du CPS-B (cps_state::raster_scanline). L'IRQ4 est déclenchée sans délai.
    fn raster_tick(&mut self, line: u32) {
        let b = &mut self.bus;
        for i in 0..2 {
            if line == 0 {
                b.raster_counter[i] = b.raster_reload[i];
            } else {
                b.raster_counter[i] = b.raster_counter[i].wrapping_sub(1) & 0x1ff;
            }
            if b.raster_counter[i] == 0 {
                b.irq_raster = true;
            }
        }
        b.raster_counter[2] = b.raster_reload[2];
    }

    pub fn render(&self) -> Vec<[u8; 3]> {
        let b = &self.bus;
        let v = cps2_gfx::VideoState {
            gfxram: &b.gfxram,
            cps_a: &b.cps_a,
            cps_b: &b.cps_b,
            objram: &self.obj_buffer,
            output: &self.pri_ctrl_buffer,
            palette_ram: Some(&b.palette),
        };
        cps2_gfx::render(&v, &self.gfx)
    }
}
