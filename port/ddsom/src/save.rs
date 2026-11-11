//! Quick save (F5) and resume (F9) by replay: the session keeps its inputs since startup and the starting EEPROM;
//! resuming means building a fresh game and replaying those inputs without display up to the saved frame (the game
//! is deterministic: this is what `validate` checks). The file is a scenario (ending `N quit`) and its starting
//! EEPROM (`.eeprom`): `validate <roms> file.eeprom file N` replays it too. Only its inputs are replayed (the
//! application never writes RAM pokes).

use std::io;
use std::path::Path;

use ddsom_game::core::interface::Board;
use ddsom_game::core::scenario::{Recorder, Scenario};
use ddsom_game::{Game, Inputs};

pub struct Session {
    eeprom: Vec<u8>,
    inputs: Recorder<Vec<u8>>,
}

impl Session {
    /// Session that starts on the fresh `game`.
    pub fn new<B: Board>(game: &Game<B>) -> Self {
        Session { eeprom: game.eeprom(), inputs: Recorder::new(Vec::new()) }
    }

    /// To be called just before each executed frame, with its inputs.
    pub fn frame(&mut self, inputs: Inputs) {
        self.inputs.record(inputs).expect("in-memory write");
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

    /// Replays `file` on a fresh game (`new_game` builds it from the starting EEPROM) up to the saved frame.
    pub fn load<B: Board>(file: &Path, new_game: impl FnOnce(&[u8]) -> Game<B>) -> io::Result<(Game<B>, Session)> {
        let eeprom = std::fs::read(eeprom_of(file))?;
        let text = std::fs::read_to_string(file)?;
        let end = text.lines().rev().find_map(|l| l.strip_suffix(" quit")?.trim().parse::<usize>().ok());
        let Some(end) = end else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "final line 'N quit' missing"));
        };
        let scenario = Scenario::load(file)?;
        let mut game = new_game(&eeprom);
        let mut inputs = Inputs::default();
        for f in 0..end {
            scenario.apply_inputs(f, &mut inputs);
            game.step(inputs);
        }
        let lines = text.lines().filter(|l| !l.ends_with(" quit")).flat_map(|l| [l, "\n"]).collect::<String>();
        let recorder = Recorder::resumes(lines.into_bytes(), end, inputs);
        Ok((game, Session { eeprom, inputs: recorder }))
    }
}

fn eeprom_of(file: &Path) -> std::path::PathBuf {
    let mut f = file.as_os_str().to_owned();
    f.push(".eeprom");
    f.into()
}
