//! Sauvegarde rapide (F5) et reprise (F9) par rejeu : la partie garde ses entrées depuis le démarrage et l'EEPROM
//! de départ ; reprendre, c'est refaire une machine neuve et rejouer ces entrées sans affichage jusqu'à l'image
//! sauvegardée (le jeu est déterministe : c'est ce que vérifie `validate`). Le fichier est un scénario (fin
//! `N quit`) et son EEPROM de départ (`.eeprom`) : `validate <roms> file.eeprom file N` le rejoue aussi.

use std::io;
use std::path::Path;

use cps2::Machine;
use ddsom_game::scenario::{Recorder, Scenario};

pub struct Game {
    eeprom: Vec<u8>,
    inputs: Recorder<Vec<u8>>,
}

impl Game {
    /// Partie qui commence sur `m` neuve.
    pub fn new(m: &Machine) -> Self {
        Game { eeprom: m.bus.eeprom.to_bytes(), inputs: Recorder::new(Vec::new()) }
    }

    /// À appeler juste avant chaque image exécutée, entrées posées.
    pub fn image(&mut self, m: &Machine) {
        self.inputs.image(m).expect("écriture en mémoire");
    }

    pub fn images(&self) -> usize {
        self.inputs.images()
    }

    /// Écrit `file` (scénario) et `file.eeprom`.
    pub fn saved(&self, file: &Path) -> io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(eeprom_of(file), &self.eeprom)?;
        let mut text = self.inputs.exit().clone();
        text.extend(format!("{} quit\n", self.images()).bytes());
        std::fs::write(file, text)
    }

    /// Rejoue `file` sur une machine neuve (`machine` la fait à partir de l'EEPROM de départ) jusqu'à l'image
    /// sauvegardée.
    pub fn charge(file: &Path, machine: impl FnOnce(&[u8]) -> Machine) -> io::Result<(Machine, Game)> {
        let eeprom = std::fs::read(eeprom_of(file))?;
        let text = std::fs::read_to_string(file)?;
        let end = text.lines().rev().find_map(|l| l.strip_suffix(" quit")?.trim().parse::<usize>().ok());
        let Some(end) = end else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "ligne finale « N quit » absente"));
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
