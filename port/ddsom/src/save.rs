//! Quick save (F5) and resume (F9) by replay: the game keeps its inputs since startup and the starting EEPROM;
//! resuming means building a fresh machine and replaying those inputs without display up to the saved
//! frame (the game is deterministic: this is what `validate` checks). The file is a scenario (ending
//! `N quit`) and its starting EEPROM (`.eeprom`): `validate <roms> file.eeprom file N` replays it too.

use std::io;
use std::path::Path;

use cps2::{Board, Machine};
use ddsom_game::core::scenario::{Recorder, Scenario};

pub struct Game {
    eeprom: Vec<u8>,
    inputs: Recorder<Vec<u8>>,
}

impl Game {
    /// Game that starts on the fresh `m`.
    pub fn new<B: Board>(m: &Machine<B>) -> Self {
        Game { eeprom: m.bus.hw().eeprom.to_bytes(), inputs: Recorder::new(Vec::new()) }
    }

    /// To be called just before each executed frame, inputs set.
    pub fn image<B: Board>(&mut self, m: &Machine<B>) {
        self.inputs.image(m).expect("in-memory write");
    }

    pub fn images(&self) -> usize {
        self.inputs.images()
    }

    /// Writes `file` (scenario) and `file.eeprom`.
    pub fn saved(&self, file: &Path) -> io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(eeprom_of(file), &self.eeprom)?;
        let mut text = self.inputs.exit().clone();
        text.extend(format!("{} quit\n", self.images()).bytes());
        std::fs::write(file, text)
    }

    /// Replays `file` on a fresh machine (`machine` builds it from the starting EEPROM) up to the saved
    /// frame.
    pub fn load<B: Board>(file: &Path, machine: impl FnOnce(&[u8]) -> Machine<B>) -> io::Result<(Machine<B>, Game)> {
        let eeprom = std::fs::read(eeprom_of(file))?;
        let text = std::fs::read_to_string(file)?;
        let end = text.lines().rev().find_map(|l| l.strip_suffix(" quit")?.trim().parse::<usize>().ok());
        let Some(end) = end else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "final line 'N quit' missing"));
        };
        let scenario = Scenario::load(file)?;
        let mut m = machine(&eeprom);
        m.rendering = false;
        for f in 0..end {
            scenario.apply(f, &mut m);
            m.run_frame();
            m.sound.audio.clear();
        }
        m.rendering = true;
        let inputs = text.lines().filter(|l| !l.ends_with(" quit")).flat_map(|l| [l, "\n"]).collect::<String>();
        let inputs = Recorder::resumes(inputs.into_bytes(), end, &m);
        Ok((m, Game { eeprom, inputs }))
    }
}

fn eeprom_of(file: &Path) -> std::path::PathBuf {
    let mut f = file.as_os_str().to_owned();
    f.push(".eeprom");
    f.into()
}
