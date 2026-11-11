//! Input scenarios (format of mame/scenario.lua: "frame field value").
//! As in MAME, an input set after frame n takes effect at frame n + 2.
//!
//! Format: one action per line, `frame field value`.
//!   * `900 Coin 1 1` / `906 Coin 1 0`: MAME input (pressed = 1, released = 0).
//!   * `quit`: final line ignored here (used by MAME).
//!   * **`2000 poke8 ff8034 5`**, `poke16`, `poke32`: writes `value` (decimal or `0x…`) at
//!     the hexadecimal address `ff0000-ffffff` of the work RAM, big-endian, just before
//!     frame `n + 1` is executed (same instant as an input). Both `validate` machines
//!     apply the same write. Invalid address or value: line ignored.
//!     MAME ignores these lines.
//!   * Empty lines and those starting with `#` are ignored.
//!
//! Scenarios only contain inputs and RAM addresses/values, never ROM data.

use cps2::{Board, Machine};
use std::io;
use std::path::Path;

pub struct Scenario {
    actions: Vec<(usize, u8, u16, bool)>, // (frame, port, bit, pressed)
    pokes: Vec<(usize, u32, u32, u8)>,    // (frame, address, value, size in bytes)
}

impl Scenario {
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut actions = Vec::new();
        let mut pokes = Vec::new();
        for line in std::fs::read_to_string(path)?.lines() {
            let mut it = line.trim().splitn(2, ' ');
            let (Some(f), Some(rest)) = (it.next(), it.next()) else { continue };
            let (Ok(f), Some((name, v))) = (f.parse::<usize>(), rest.rsplit_once(' ')) else { continue };
            if let Some((kind, addr)) = name.split_once(' ') {
                let size = match kind { "poke8" => 1, "poke16" => 2, "poke32" => 4, _ => 0 };
                if size != 0 {
                    let val = match v.strip_prefix("0x") {
                        Some(h) => u32::from_str_radix(h, 16).ok(),
                        None => v.parse::<u32>().ok(),
                    };
                    let a = u32::from_str_radix(addr.trim_start_matches("0x"), 16).ok();
                    if let (Some(a), Some(val)) = (a, val) {
                        if (0xff0000..=0xffffff).contains(&a) {
                            pokes.push((f, a, val, size));
                        }
                    }
                    continue;
                }
            }
            let Ok(v) = v.parse::<u32>() else { continue };
            if let Some((port, bit)) = input_bit(name) {
                actions.push((f, port, bit, v != 0));
            }
        }
        Ok(Scenario { actions, pokes })
    }

    /// Applies the inputs due before executing the frame of index `f` (0 = first frame).
    pub fn apply<B: Board>(&self, f: usize, m: &mut Machine<B>) {
        for &(af, addr, val, size) in &self.pokes {
            if af + 1 == f {
                poke(m, addr, val, size);
            }
        }
        for &(af, port, bit, down) in &self.actions {
            if af + 1 == f {
                set_bit(m, port, bit, down);
            }
        }
    }
}

/// Writes `value` (big-endian, `size` bytes) to the work RAM (FF0000-FFFFFF).
pub fn poke<B: Board>(m: &mut Machine<B>, addr: u32, val: u32, size: u8) {
    for k in 0..size as u32 {
        let a = (addr + k - 0xff0000) as usize;
        let byte = (val >> (8 * (size as u32 - 1 - k))) as u8;
        let w = &mut m.bus.ram_mut()[a / 2];
        *w = if a.is_multiple_of(2) { *w & 0x00ff | (byte as u16) << 8 } else { *w & 0xff00 | byte as u16 };
    }
}

fn set_bit<B: Board>(m: &mut Machine<B>, port: u8, bit: u16, down: bool) {
    let inputs = &mut m.bus.hw_mut().inputs;
    let reg = match port {
        0 => &mut inputs.in0,
        1 => &mut inputs.in1,
        _ => &mut inputs.in2,
    };
    if down { *reg &= !bit } else { *reg |= bit }
}

/// Presses (`down`) or releases the MAME input `name` (e.g. "P1 Button 2"); false if the name is unknown.
pub fn set_input<B: Board>(m: &mut Machine<B>, name: &str, down: bool) -> bool {
    match input_bit(name) {
        Some((port, bit)) => {
            set_bit(m, port, bit, down);
            true
        }
        None => false,
    }
}

/// Records the inputs of a game as a scenario: one line per input that changes, replayable by `validate`.
pub struct Recorder<W: io::Write = std::fs::File> {
    out: W,
    inputs: Vec<(String, u8, u16)>,
    before: [u16; 3],
    image: usize,
}

impl Recorder {
    pub fn created(path: &Path) -> io::Result<Self> {
        Ok(Recorder::new(std::fs::File::create(path)?))
    }
}

impl<W: io::Write> Recorder<W> {
    pub fn new(out: W) -> Self {
        let mut inputs = Vec::new();
        for p in 1..=4 {
            for what in ["Right", "Left", "Down", "Up", "Button 1", "Button 2", "Button 3", "Button 4"] {
                inputs.push(format!("P{p} {what}"));
            }
        }
        inputs.extend(SYSTEM.map(String::from));
        let inputs = inputs.into_iter().filter_map(|n| input_bit(&n).map(|(port, bit)| (n, port, bit))).collect();
        Recorder { out, inputs, before: [0xffff; 3], image: 0 }
    }

    /// Resumes the recording of a replayed game up to frame `image` (already written to `out`): the inputs
    /// of `m` are those of the last replayed frame.
    pub fn resumes<B: Board>(out: W, image: usize, m: &Machine<B>) -> Self {
        let mut e = Recorder::new(out);
        let inputs = m.bus.hw().inputs;
        e.before = [inputs.in0, inputs.in1, inputs.in2];
        e.image = image;
        e
    }

    /// Recorded frames.
    pub fn images(&self) -> usize {
        self.image
    }

    pub fn exit(&self) -> &W {
        &self.out
    }

    /// To be called just before each executed frame, inputs set: `apply` replays them at the same instant.
    pub fn image<B: Board>(&mut self, m: &Machine<B>) -> io::Result<()> {
        let inputs = m.bus.hw().inputs;
        let ports = [inputs.in0, inputs.in1, inputs.in2];
        let f = self.image.saturating_sub(1);
        for (name, port, bit) in &self.inputs {
            let (a, b) = (self.before[*port as usize] & bit, ports[*port as usize] & bit);
            if a != b {
                writeln!(self.out, "{f} {name} {}", (b == 0) as u8)?;
            }
        }
        self.before = ports;
        self.image += 1;
        Ok(())
    }
}

const SYSTEM: [&str; 10] = [
    "Coin 1",
    "Coin 2",
    "Coin 3",
    "Coin 4",
    "1 Player Start",
    "2 Players Start",
    "3 Players Start",
    "4 Players Start",
    "Service 1",
    "Service Mode",
];

/// MAME input name -> (port, bit) for cps2_4p4b (active at 0).
fn input_bit(name: &str) -> Option<(u8, u16)> {
    if let Some(rest) = name.strip_prefix('P') {
        let (p, what) = rest.split_once(' ')?;
        let shift = if p == "1" || p == "3" { 0 } else { 8 };
        let b = match what {
            "Right" => 0x01,
            "Left" => 0x02,
            "Down" => 0x04,
            "Up" => 0x08,
            "Button 1" => 0x10,
            "Button 2" => 0x20,
            "Button 3" => 0x40,
            "Button 4" => 0x80,
            _ => return None,
        };
        return Some((if p == "3" || p == "4" { 1 } else { 0 }, b << shift));
    }
    Some(match name {
        "Coin 1" => (2, 0x1000),
        "Coin 2" => (2, 0x2000),
        "Coin 3" => (2, 0x4000),
        "Coin 4" => (2, 0x8000),
        "1 Player Start" => (2, 0x0100),
        "2 Players Start" => (2, 0x0200),
        "3 Players Start" => (2, 0x0400),
        "4 Players Start" => (2, 0x0800),
        "Service 1" => (2, 0x0004),
        "Service Mode" => (2, 0x0002),
        _ => return None,
    })
}
