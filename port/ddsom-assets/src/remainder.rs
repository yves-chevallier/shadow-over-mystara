//! Le reste de la région programme (`data/remainder.json`) : tous les octets que les autres fichiers ne décrivent pas,
//! sauf le code 68000.
//!
//! Le jeu sans 68000 n'exécute plus le code d'origine : tout le reste de la région programme est de la donnée qu'une
//! routine native peut lire. Plutôt que d'extraire les seules zones qu'une mesure a vu lire (`data/natives.json`),
//! ce fichier range tout ce qui n'est pas du code, ce qui rend la région programme complète par construction. Le
//! code exclu est celui que la machine de référence exécute sur tous les scénarios (`port/extract/code_68k.txt`,
//! docs/sans-rom.md « Couverture complète des données ») ; le code jamais exécuté est donc rangé ici aussi, comme
//! les tables rangées dans le code et les données qu'un appel saute.
//!
//! * `data` : blocs de l'espace données (lectures du 68000), `debut` puis les octets en hexadécimal, 32 par
//!   ligne ;
//! * `programme` : blocs de l'espace programme déchiffré (lectures relatives au PC), dans la zone chiffrée
//!   seulement : au-delà de `encrypted_end`, l'espace programme égale l'espace données et la reconstruction le
//!   recopie ;
//! * `code` : les plages de code exclues (`début-fin`, fin exclue). Les assets rendent tout le reste ; une partie
//!   de ce code est quand même décrite par d'autres fichiers (zones lues par les natives, `natives.json`).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json;

/// Octets par ligne d'un bloc.
pub const LINE: usize = 32;

#[derive(Serialize, Deserialize, Default)]
pub struct Remainder {
    pub description: String,
    /// Fin de la zone chiffrée du programme (exclue) : au-delà, l'espace programme égale l'espace données.
    #[serde(rename = "fin_chiffree", with = "json::hex")]
    pub encrypted_end: u32,
    /// Octets de code (total des plages de `code`).
    #[serde(rename = "octets_de_code")]
    pub code_bytes: u32,
    /// Plages de code exclues, `"0x400-0x4ec"` (fin exclue).
    pub code: Vec<String>,
    #[serde(rename = "donnees")]
    pub data: Vec<Block>,
    #[serde(rename = "programme")]
    pub programme: Vec<Block>,
}

/// Octets consécutifs à partir de `debut`.
#[derive(Serialize, Deserialize)]
pub struct Block {
    #[serde(rename = "debut", with = "json::hex")]
    pub debut: u32,
    /// Lignes de LINE octets (la dernière peut être plus courte), en hexadécimal.
    #[serde(rename = "octets")]
    pub bytes: Vec<String>,
}

impl Block {
    pub fn new(debut: u32, bytes: &[u8]) -> Self {
        let lines = bytes.chunks(LINE).map(|l| l.iter().map(|b| format!("{b:02x}")).collect()).collect();
        Block { debut, bytes: lines }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for l in &self.bytes {
            if l.len() % 2 != 0 {
                return Err(format!("bloc {:#x} : ligne impaire « {l} »", self.debut));
            }
            for i in (0..l.len()).step_by(2) {
                out.push(u8::from_str_radix(&l[i..i + 2], 16).map_err(|e| format!("bloc {:#x} : {e}", self.debut))?);
            }
        }
        Ok(out)
    }
}

impl Remainder {
    pub fn load(path: &Path) -> io::Result<Self> {
        json::load(path)
    }

    /// Écrit le fichier : une plage de code ou une ligne d'octets par ligne.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let string = |s: &str| serde_json::to_string(s).unwrap();
        let list = |v: &[String], n: usize| {
            let pad = "  ".repeat(n);
            let l: Vec<String> = v.iter().map(|x| format!("{pad}{}", string(x))).collect();
            if l.is_empty() { "[]".to_string() } else { format!("[\n{}\n{}]", l.join(",\n"), "  ".repeat(n - 1)) }
        };
        let blocks = |v: &[Block]| {
            let b: Vec<String> = v
                .iter()
                .map(|b| format!("    {{\"debut\": \"{:#x}\", \"octets\": {}}}", b.debut, list(&b.bytes, 3)))
                .collect();
            if b.is_empty() { "[]".to_string() } else { format!("[\n{}\n  ]", b.join(",\n")) }
        };
        let out = format!(
            "{{\n  \"description\": {},\n  \"fin_chiffree\": \"{:#x}\",\n  \"octets_de_code\": {},\n  \"code\": {},\n  \"donnees\": {},\n  \"programme\": {}\n}}\n",
            string(&self.description),
            self.encrypted_end,
            self.code_bytes,
            list(&self.code, 2),
            blocks(&self.data),
            blocks(&self.programme)
        );
        std::fs::write(path, out)
    }

    /// Plages de code `(début, fin exclue)`.
    pub fn code_ranges(&self) -> Result<Vec<(u32, u32)>, String> {
        self.code
            .iter()
            .map(|p| {
                let (a, b) = p.split_once('-').ok_or_else(|| format!("plage de code illisible « {p} »"))?;
                match (json::hex::parse(a), json::hex::parse(b)) {
                    (Some(a), Some(b)) if a < b => Ok((a, b)),
                    _ => Err(format!("plage de code illisible « {p} »")),
                }
            })
            .collect()
    }
}

/// Plages `(début, fin exclue)` des positions où `guard` est vrai.
pub fn ranges(n: u32, guard: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for a in 0..n {
        if guard(a) {
            match out.last_mut() {
                Some(r) if r.1 == a => r.1 += 1,
                _ => out.push((a, a + 1)),
            }
        }
    }
    out
}
