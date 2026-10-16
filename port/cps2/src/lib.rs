//! Machine CPS2 : 68000 + bus + vidéo, cadencée ligne par ligne comme dans MAME.
//! Son : Z80 + QSound (HLE), exécutés ligne par ligne en alternance avec le 68000.

pub mod bus;
pub mod crypt;
pub mod eeprom;
mod sboxes;
pub mod sound;

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

/// Exécuteur de blocs recompilés : (pc, cpu, bus, échéance) -> Some(pc suivant) ou None si aucun bloc.
pub type BlockExecutor = fn(u32, &mut Cpu, &mut Cps2Bus, u64) -> Option<u32>;

pub struct Machine {
    /// Code recompilé (None = tout interprété).
    pub executor: Option<BlockExecutor>,
    /// Adresses où aucun bloc recompilé n'existait (exécutées par l'interpréteur).
    pub misses: std::collections::BTreeMap<u32, u64>,
    /// Instructions 68000 exécutées par l'interpréteur (le reste l'est par le code recompilé).
    pub interp_steps: u64,
    /// Rappel après chaque instruction interprétée (outils de diagnostic) : (cpu, bus, pc).
    pub step_hook: Option<Box<dyn FnMut(&Cpu, &Cps2Bus, u32)>>,
    pub cpu: Cpu,
    pub bus: Cps2Bus,
    pub sound: sound::Sound,
    pub gfx: Vec<u8>,
    pub frame: u64,
    /// Table d'objets verrouillée au dernier vblank (affichée à l'image suivante).
    pub obj_buffer: Vec<u16>,
    pub pri_ctrl_buffer: [u16; 6],
    line_cycles: f64,
    /// Cycle CPU absolu de fin de la ligne en cours.
    cycle_debt: f64,
    /// Idem pour le Z80.
    z80_debt: f64,
}

/// Programme 68000 de ddsom : (vue données en clair, vue espace programme déchiffrée), mots big-endian.
pub fn load_program(romdir: &Path) -> io::Result<(Vec<u16>, Vec<u16>)> {
    let mut rom = Vec::with_capacity(0x200000);
    for name in DDSOM_PROG {
        let d = std::fs::read(romdir.join(name))?;
        // ROM_LOAD16_WORD_SWAP : mots stockés petit-boutiste dans la puce
        rom.extend(d.chunks_exact(2).map(|w| u16::from_le_bytes([w[0], w[1]])));
    }
    let key = std::fs::read(romdir.join("ddsom.key"))?;
    let (k, lower, upper) = crypt::parse_key(&key);
    let dec = crypt::decrypt(&rom, k, lower, upper);
    Ok((rom, dec))
}

impl Machine {
    /// Charge ddsom depuis un dossier de ROMs extraites. `eeprom` : contenu nvram MAME (128 octets).
    pub fn load_ddsom(romdir: &Path, eeprom: Option<&[u8]>) -> io::Result<Self> {
        let (rom, rom_dec) = load_program(romdir)?;
        let gfx = cps2_gfx::rom::load_gfx(romdir, cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE)?;
        // Z80 : dd2.01 = 0x8000 en 0, puis 0x18000 en 0x10000 (ROM_CONTINUE) ; dd2.02 en 0x28000
        let mut zrom = vec![0u8; 0x50000];
        let d1 = std::fs::read(romdir.join("dd2.01"))?;
        zrom[..0x8000].copy_from_slice(&d1[..0x8000]);
        zrom[0x10000..0x28000].copy_from_slice(&d1[0x8000..0x20000]);
        let d2 = std::fs::read(romdir.join("dd2.02"))?;
        zrom[0x28000..0x48000].copy_from_slice(&d2);
        // échantillons QSound : ROM_LOAD16_WORD_SWAP
        let mut samples = Vec::with_capacity(0x400000);
        for name in ["dd2.11m", "dd2.12m"] {
            let d = std::fs::read(romdir.join(name))?;
            samples.extend(d.chunks_exact(2).flat_map(|w| [w[1], w[0]]));
        }
        let dsp = std::fs::read(romdir.join("dl-1425.bin"))?;
        let snd = sound::Sound::new(zrom, qsound::QSound::new(&dsp, samples));
        let ee = match eeprom {
            Some(b) => Eeprom93c46::from_bytes(b),
            None => Eeprom93c46::new([0xffff; 64]),
        };
        let mut m = Machine {
            executor: None,
            misses: Default::default(),
            interp_steps: 0,
            step_hook: None,
            cpu: Cpu::new(),
            bus: Cps2Bus::new(rom, rom_dec, ee),
            sound: snd,
            gfx,
            frame: 0,
            obj_buffer: vec![0; 0x1000],
            pri_ctrl_buffer: [0; 6],
            line_cycles: CPU_HZ / LINE_HZ,
            cycle_debt: 0.0,
            z80_debt: 0.0,
        };
        m.reset();
        Ok(m)
    }

    pub fn reset(&mut self) {
        self.cpu = Cpu::new();
        self.cpu.reset(&mut self.bus);
    }

    /// Exécute le CPU pendant une ligne de balayage. La cible est absolue (le dépassement d'une
    /// ligne est retranché de la suivante) pour garder exactement 755,2 cycles par ligne en moyenne.
    fn run_line(&mut self) {
        self.cycle_debt += self.line_cycles;
        let target = self.cycle_debt as u64;
        while self.cpu.cycles < target {
            self.cpu.irq_level = self.bus.irq_level();
            if let Some(run) = self.executor {
                let mask = ((self.cpu.sr >> 8) & 7) as u8;
                let plain = self.cpu.irq_level <= mask && !self.cpu.stopped && !self.cpu.trace_pending
                    && self.cpu.sr & m68k::SR_T == 0;
                if plain {
                    if run(self.cpu.pc, &mut self.cpu, &mut self.bus, target).is_some() {
                        continue;
                    }
                    *self.misses.entry(self.cpu.pc).or_default() += 1;
                }
            }
            let pc = self.cpu.pc;
            self.cpu.step(&mut self.bus);
            self.interp_steps += 1;
            if let Some(h) = self.step_hook.as_mut() {
                h(&self.cpu, &self.bus, pc);
            }
        }
        // puis le Z80 sur la même tranche de temps (512 cycles à 8 MHz par ligne)
        self.sound.set_reset(!self.bus.z80_running);
        self.z80_debt += sound::Z80_HZ / LINE_HZ;
        self.sound.run(&mut self.bus.qram, self.z80_debt as u64, 1.0 / LINE_HZ);
    }

    /// Exécute une image comme MAME : au reset le faisceau est en début de vblank (ligne 240) et
    /// la notification d'image de MAME tombe au début de chaque vblank. Une image exécute donc les
    /// lignes 240..261 puis 0..239 ; elle se termine sur l'événement de vblank (rendu de l'écran,
    /// verrouillage de la table d'objets, IRQ2), traité au début de l'image suivante.
    /// Renvoie l'image 384x224 RGB.
    pub fn run_frame(&mut self) -> Vec<[u8; 3]> {
        let frame_end = (self.cycle_debt + self.line_cycles * LINES as f64) as u64;
        for i in 0..LINES {
            let line = (VBLANK_LINE + i) % LINES;
            self.bus.scanline = line;
            self.raster_tick(line);
            // compteurs raster armés (rechargement < nombre de lignes) : une IRQ peut tomber à
            // chaque ligne ; sinon la prochaine interruption possible est le vblank de fin d'image
            let raster = self.bus.raster_reload[..2].iter().any(|&r| (r as u32) < LINES);
            self.bus.irq_horizon = if raster { (self.cycle_debt + self.line_cycles) as u64 } else { frame_end };
            self.run_line();
        }
        let image = self.render();
        let bank = self.bus.objram_bank as usize & 1;
        self.obj_buffer.copy_from_slice(&self.bus.objram[bank]);
        self.pri_ctrl_buffer = self.bus.output;
        self.bus.irq_vblank = true;
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
