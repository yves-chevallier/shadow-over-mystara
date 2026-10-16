//! ROM du pilote son : programme et données de la région « audiocpu » de MAME.
//!
//! Le Z80 voit 0000-7FFF fixe et une fenêtre de 16 Ko en 8000-BFFF dont la banque est choisie en
//! D003. Le pilote manipule des pointeurs « linéaires » : l'adresse Z80 d'un octet de la banque `b`
//! à l'offset `o` de la fenêtre est `0x8000 + 0x4000 * b + o`, c'est-à-dire l'offset dans le fichier
//! `dd2.01` (puis `dd2.02` à sa suite). Lire avec un pointeur linéaire revient à ce que fait le Z80
//! en passant à la banque suivante chaque fois qu'il atteint C000.

use std::io;
use std::path::Path;

/// Taille de la région audiocpu de MAME.
const REGION_SIZE: usize = 0x50000;

#[derive(Clone)]
pub struct Rom {
    region: Vec<u8>,
}

impl Rom {
    /// Charge `dd2.01` et `dd2.02` comme MAME : dd2.01 = 0x8000 octets fixes puis 0x18000 octets
    /// de banques à partir de 0x10000 ; dd2.02 à partir de 0x28000.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let mut region = vec![0u8; REGION_SIZE];
        let d1 = std::fs::read(dir.join("dd2.01"))?;
        region[..0x8000].copy_from_slice(&d1[..0x8000]);
        region[0x10000..0x28000].copy_from_slice(&d1[0x8000..0x20000]);
        let d2 = std::fs::read(dir.join("dd2.02"))?;
        region[0x28000..0x48000].copy_from_slice(&d2);
        Ok(Rom { region })
    }

    /// Construit la ROM depuis une région audiocpu déjà assemblée.
    pub fn from_region(region: Vec<u8>) -> Self {
        Rom { region }
    }

    /// Région audiocpu complète (outils de test : Z80 de référence).
    pub fn region(&self) -> &[u8] {
        &self.region
    }

    /// Octet à l'adresse linéaire `p` (0xFF hors ROM, comme le bus ouvert du Z80).
    pub fn byte(&self, p: u32) -> u8 {
        let p = p & 0xff_ffff;
        let i = if p < 0x8000 { p } else { p + 0x8000 } as usize;
        self.region.get(i).copied().unwrap_or(0xff)
    }

    /// Mot petit-boutiste à l'adresse linéaire `p`.
    pub fn word(&self, p: u32) -> u16 {
        u16::from(self.byte(p)) | u16::from(self.byte(p + 1)) << 8
    }

    /// Entrée `i` d'une table de mots petit-boutistes située en `base`.
    pub fn table(&self, base: u32, i: u8) -> u16 {
        self.word(base + 2 * u32::from(i))
    }
}
