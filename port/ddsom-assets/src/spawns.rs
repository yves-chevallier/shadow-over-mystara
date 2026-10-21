//! Scripts d'apparition des scènes (vagues d'ennemis, décors, objets) : `assets/data/spawns.json`, lus par
//! `ddsom-game/src/scene/script.rs` (0xf2e5c). Voir docs/assets.md et docs/scenes.md §2.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Entrée d'un script (le mot `+2` les distingue : 0xffff pour un appel).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Entry {
    /// Appel (10 octets) : à l'abscisse `x`, appelle la routine `call` avec D0 = `arg`.
    Call {
        x: i16,
        #[serde(with = "hex")]
        call: u32,
        arg: u16,
    },
    /// Apparition (0x12 octets) : à l'abscisse de caméra `x`, crée un objet de la classe `class` (octet pair,
    /// index des allocateurs `0xf2f38` : ENEMY, SET, EFFCT, BOX, SOBJ, BOSS, ITEM, ESHL...), de type `kind` et
    /// variante `variant`, en X = `x + dx`, Z = `z`, Y = `y`, si le rang du jeu est au plus `rank` et s'il y a au
    /// moins `players` joueurs ; `f2c`, `f2f` et `f42` sont recopiés en `+0x2c`, `+0x2f` et `+0x42` (mot) de
    /// l'objet ; `unused` (octet `+0xa`) n'est pas lu.
    Spawn {
        x: i16,
        dx: i16,
        z: i16,
        y: i16,
        rank: u8,
        players: u8,
        unused: u8,
        class: u8,
        f2c: u8,
        kind: u8,
        variant: u8,
        f2f: u8,
        f42: u16,
    },
}

impl Entry {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        match *self {
            Entry::Call { x, call, arg } => {
                b.extend(x.to_be_bytes());
                b.extend([0xff, 0xff]);
                b.extend(call.to_be_bytes());
                b.extend(arg.to_be_bytes());
            }
            Entry::Spawn { x, dx, z, y, rank, players, unused, class, f2c, kind, variant, f2f, f42 } => {
                for w in [x, dx, z, y] {
                    b.extend(w.to_be_bytes());
                }
                b.extend([rank, players, unused, class, f2c, kind, variant, f2f]);
                b.extend(f42.to_be_bytes());
            }
        }
        b
    }
}

/// Un script : entrées jusqu'au mot 0xffff final (non écrit), puis `pad` : le mot de bourrage `0x4e71` qui
/// sépare certains scripts du suivant.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Script {
    #[serde(with = "hex")]
    pub rom: u32,
    pub entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pad: Vec<u8>,
}

impl Script {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b: Vec<u8> = self.entries.iter().flat_map(|e| e.bytes()).collect();
        b.extend([0xff, 0xff]);
        b.extend(&self.pad);
        b
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Spawns {
    /// Table des scripts (un long par scène, espace données).
    #[serde(with = "hex")]
    pub table: u32,
    /// Script de chaque scène (adresse).
    #[serde(with = "hex_vec")]
    pub scenes: Vec<u32>,
    /// Scripts distincts, par adresse croissante.
    pub scripts: Vec<Script>,
}

impl Spawns {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Script de la scène `s`.
    pub fn of_scene(&self, s: usize) -> Option<&Script> {
        let a = *self.scenes.get(s)?;
        self.scripts.iter().find(|x| x.rom == a)
    }
}
