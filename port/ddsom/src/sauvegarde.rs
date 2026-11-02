//! Sauvegarde rapide (F5) et reprise (F9) par rejeu : la partie garde ses entrées depuis le démarrage et l'EEPROM
//! de départ ; reprendre, c'est refaire une machine neuve et rejouer ces entrées sans affichage jusqu'à l'image
//! sauvegardée (le jeu est déterministe : c'est ce que vérifie `validate`). Le fichier est un scénario (fin
//! `N quit`) et son EEPROM de départ (`.eeprom`) : `validate <roms> fichier.eeprom fichier N` le rejoue aussi.

use std::io;
use std::path::Path;

use cps2::Machine;
use ddsom_game::scenario::{Enregistreur, Scenario};

pub struct Partie {
    eeprom: Vec<u8>,
    entrees: Enregistreur<Vec<u8>>,
}

impl Partie {
    /// Partie qui commence sur `m` neuve.
    pub fn nouvelle(m: &Machine) -> Self {
        Partie { eeprom: m.bus.eeprom.to_bytes(), entrees: Enregistreur::new(Vec::new()) }
    }

    /// À appeler juste avant chaque image exécutée, entrées posées.
    pub fn image(&mut self, m: &Machine) {
        self.entrees.image(m).expect("écriture en mémoire");
    }

    pub fn images(&self) -> usize {
        self.entrees.images()
    }

    /// Écrit `fichier` (scénario) et `fichier.eeprom`.
    pub fn sauve(&self, fichier: &Path) -> io::Result<()> {
        if let Some(dir) = fichier.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(eeprom_de(fichier), &self.eeprom)?;
        let mut texte = self.entrees.sortie().clone();
        texte.extend(format!("{} quit\n", self.images()).bytes());
        std::fs::write(fichier, texte)
    }

    /// Rejoue `fichier` sur une machine neuve (`machine` la fait à partir de l'EEPROM de départ) jusqu'à l'image
    /// sauvegardée.
    pub fn charge(fichier: &Path, machine: impl FnOnce(&[u8]) -> Machine) -> io::Result<(Machine, Partie)> {
        let eeprom = std::fs::read(eeprom_de(fichier))?;
        let texte = std::fs::read_to_string(fichier)?;
        let fin = texte.lines().rev().find_map(|l| l.strip_suffix(" quit")?.trim().parse::<usize>().ok());
        let Some(fin) = fin else {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "ligne finale « N quit » absente"));
        };
        let scenario = Scenario::load(fichier)?;
        let mut m = machine(&eeprom);
        m.rendu = false;
        for f in 0..fin {
            scenario.apply(f, &mut m);
            m.run_frame();
            m.sound.audio.clear();
        }
        m.rendu = true;
        let entrees = texte.lines().filter(|l| !l.ends_with(" quit")).flat_map(|l| [l, "\n"]).collect::<String>();
        let entrees = Enregistreur::reprend(entrees.into_bytes(), fin, &m);
        Ok((m, Partie { eeprom, entrees }))
    }
}

fn eeprom_de(fichier: &Path) -> std::path::PathBuf {
    let mut f = fichier.as_os_str().to_owned();
    f.push(".eeprom");
    f.into()
}
