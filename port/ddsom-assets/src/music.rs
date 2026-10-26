//! Musiques et effets du pilote son (Z80) : table des sons, pistes, instruments, échantillons et tables du
//! pilote. Format : `assets/sound/music.json`, voir docs/assets.md et docs/son.md.
//!
//! Adresses « linéaires » du pilote (`ddsom_sound::Rom`) : décalage dans `dd2.01`, puis `dd2.02` à sa suite.
//!
//! Une piste est écrite comme une suite de jetons séparés par des espaces, un par instruction :
//! * note (octet >= 0x20) : `durée:note`, durée 1 à 7 (bits 7-5), note 0 à 31 (bits 4-0, 0 = silence) ;
//! * instruction (octet < 0x20) : son nom (`OPS`), suivi de `=` et de ses paramètres séparés par des
//!   virgules : octets en décimal, `step` en mot, déplacements des sauts (`loopK`, `exitK`, `jump`) en mot
//!   signé relatif à l'adresse qui suit le déplacement.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Noms des instructions 0 à 31 (docs/son.md §6).
pub const OPS: [&str; 32] = [
    "triplet", "tie", "dot", "up2", "mode", "step", "gate", "amp", "inst", "oct", "mtrans", "trans", "detune", "glide",
    "loop0", "loop1", "loop2", "loop3", "exit0", "exit1", "exit2", "exit3", "jump", "end", "pan", "echo", "vol", "vib",
    "trem", "lfo", "lfomode", "bank",
];

/// Octets de paramètres de l'instruction `op`.
pub fn param_len(op: u8) -> usize {
    match op {
        0..=3 | 23 => 0,
        5 | 22 => 2,
        14..=21 => 3,
        _ => 1,
    }
}

/// Jeton de l'instruction qui commence à `bytes[0]` (et sa longueur), `None` si les octets manquent.
pub fn decode_op(bytes: &[u8]) -> Option<(String, usize)> {
    let b = *bytes.first()?;
    if b >= 0x20 {
        return Some((format!("{}:{}", b >> 5, b & 0x1f), 1));
    }
    let n = param_len(b);
    let p = bytes.get(1..1 + n)?;
    let be = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
    let name = OPS[b as usize];
    let tok = match b {
        0..=3 | 23 => name.to_string(),
        5 => format!("{name}={}", be(0)),
        22 => format!("{name}={}", be(0) as i16),
        14..=21 => format!("{name}={},{}", p[0], be(1) as i16),
        _ => format!("{name}={}", p[0]),
    };
    Some((tok, 1 + n))
}

/// Octets d'un jeton.
pub fn encode_op(tok: &str) -> Option<Vec<u8>> {
    if let Some((d, n)) = tok.split_once(':') {
        let (d, n): (u8, u8) = (d.parse().ok()?, n.parse().ok()?);
        return (1..=7).contains(&d).then_some(vec![d << 5 | n & 0x1f]).filter(|_| n < 32);
    }
    let (name, args) = tok.split_once('=').unwrap_or((tok, ""));
    let op = OPS.iter().position(|&o| o == name)? as u8;
    let args: Vec<i32> = if args.is_empty() { Vec::new() } else { args.split(',').map(|a| a.parse().ok()).collect::<Option<_>>()? };
    let mut out = vec![op];
    match (op, args.as_slice()) {
        (0..=3 | 23, []) => {}
        (5 | 22, [v]) => out.extend((*v as u16).to_be_bytes()),
        (14..=21, [a, v]) => {
            out.push(*a as u8);
            out.extend((*v as u16).to_be_bytes());
        }
        (_, [v]) if param_len(op) == 1 => out.push(*v as u8),
        _ => return None,
    }
    Some(out)
}

/// Définition d'un son (entrée non nulle de la table des sons).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Song {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Premier octet : 0 musique ; 1 à 0x7f effet (priorité) ; 0x80-0x8f note simple (voie) ; 0x90-0x92 ADPCM.
    pub kind: u8,
    /// Musique, effet : 16 déplacements (depuis `rom`, 0 = pas de piste sur ce canal).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<u16>,
    /// Note simple (10 octets : drapeaux, durée, tenue, amplitude, instrument, note, accord) ou ADPCM
    /// (7 octets : drapeaux, 4 octets ignorés... voir docs/son.md §5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<u8>,
}

impl Song {
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = vec![self.kind];
        for t in &self.tracks {
            b.extend(t.to_be_bytes());
        }
        b.extend(&self.body);
        b
    }

    /// Adresses des pistes (canal, adresse).
    pub fn track_starts(&self) -> impl Iterator<Item = (usize, u32)> + '_ {
        self.tracks.iter().enumerate().filter(|(_, &o)| o != 0).map(|(c, &o)| (c, self.rom + o as u32))
    }
}

/// Instructions de pistes à la suite en ROM (tout ce qu'atteignent les pistes, en suivant les sauts).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CodeBlock {
    #[serde(with = "hex")]
    pub rom: u32,
    pub ops: String,
}

impl CodeBlock {
    pub fn bytes(&self) -> Option<Vec<u8>> {
        let mut b = Vec::new();
        for t in self.ops.split(' ') {
            b.extend(encode_op(t)?);
        }
        Some(b)
    }
}

/// Banc d'instruments : fiches de 8 octets (échantillon (mot petit-boutiste, bit 15 = suit la variation
/// de hauteur), accord fin, 5 index d'enveloppe : attaque, decay, tenue, chute lente, relâchement).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    #[serde(with = "hex")]
    pub rom: u32,
    pub instruments: Vec<[u8; 8]>,
}

/// Table du pilote (octets, ou mots petit-boutistes).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DriverTable {
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    pub words: bool,
    pub values: Vec<u16>,
}

impl DriverTable {
    pub fn bytes(&self) -> Vec<u8> {
        if self.words {
            self.values.iter().flat_map(|v| v.to_le_bytes()).collect()
        } else {
            self.values.iter().map(|&v| v as u8).collect()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Music {
    /// Mot (ROM fixe) qui donne l'adresse de l'en-tête des sons.
    #[serde(with = "hex")]
    pub header_ptr: u32,
    #[serde(with = "hex")]
    pub header: u32,
    /// En-tête : nombre de sons (mot grand-boutiste), index du facteur de volume de la musique et du volume
    /// de base des effets (table `levels`), deux octets non lus ; la table des sons suit (4 octets par son :
    /// adresse grand-boutiste sur 3 octets, 0).
    pub count: u16,
    pub music_factor: u8,
    pub sfx_volume: u8,
    pub header_rest: [u8; 2],
    /// Un son par numéro (`null` : entrée vide).
    pub songs: Vec<Option<Song>>,
    pub code: Vec<CodeBlock>,
    /// Table des bancs d'instruments (mots petit-boutistes, adresse de chaque banc).
    #[serde(with = "hex")]
    pub banks_table: u32,
    pub banks: Vec<Bank>,
    /// Table des échantillons : mot en `samples_ptr` ; fiches de 8 octets (banque, début, boucle, fin en
    /// mots petit-boutistes, note de référence).
    #[serde(with = "hex")]
    pub samples_ptr: u32,
    #[serde(with = "hex")]
    pub samples_rom: u32,
    pub samples: Vec<[u8; 8]>,
    /// Octets qui suivent la fin de chaque échantillon dans la ROM d'échantillons (hexadécimal, zéros finaux
    /// omis), jusqu'au prochain échantillon de la banque ou à la fin de la banque (`sample_tail_end`) : une
    /// voix qui ne boucle pas continue de lire après la fin tant que le Z80 ne l'arrête pas. Absents des
    /// fichiers WAV, qui s'arrêtent à la fin déclarée.
    #[serde(default)]
    pub sample_tails: Vec<String>,
    pub tables: Vec<DriverTable>,
}

impl Music {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Début (adresse dans la ROM d'échantillons) et fin incluse de l'échantillon `i`.
    pub fn sample_span(&self, i: usize) -> (usize, usize) {
        let e = &self.samples[i];
        let bank = (e[0] as usize) << 16;
        (bank | u16::from_le_bytes([e[1], e[2]]) as usize, bank | u16::from_le_bytes([e[5], e[6]]) as usize)
    }

    /// Fiche valide : banque dans la ROM, début, boucle et fin dans l'ordre (sinon aucun fichier WAV).
    pub fn sample_valid(&self, i: usize) -> bool {
        let e = &self.samples[i];
        let (start, end) = self.sample_span(i);
        let lp = (e[0] as usize) << 16 | u16::from_le_bytes([e[3], e[4]]) as usize;
        e[0] < 0x40 && start <= end && (start..=end).contains(&lp)
    }

    /// Fin (exclue) de la zone qui suit l'échantillon `i` : début du prochain échantillon de sa banque, sinon
    /// fin de la banque.
    pub fn sample_tail_end(&self, i: usize) -> usize {
        let end = self.sample_span(i).1;
        let limit = (end >> 16) + 1 << 16;
        (0..self.samples.len()).filter(|&k| self.sample_valid(k)).map(|k| self.sample_span(k).0).filter(|&s| s > end && s < limit).min().unwrap_or(limit)
    }

    pub fn table(&self, name: &str) -> Option<&DriverTable> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// Jetons de la piste qui commence en `at`, jusqu'à sa fin ou son premier saut inconditionnel.
    pub fn track(&self, at: u32) -> Option<Vec<&str>> {
        let i = self.code.partition_point(|b| b.rom <= at).checked_sub(1)?;
        let block = &self.code[i];
        let mut a = block.rom;
        let mut out = Vec::new();
        for t in block.ops.split(' ') {
            let n = encode_op(t)?.len() as u32;
            if a >= at {
                out.push(t);
                if t == "end" || t.starts_with("jump=") {
                    break;
                }
            }
            a += n;
        }
        (!out.is_empty()).then_some(out)
    }
}
