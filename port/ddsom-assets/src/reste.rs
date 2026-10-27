//! Le reste de la région programme (`data/reste.json`) : tous les octets que les autres fichiers ne décrivent pas,
//! sauf le code 68000.
//!
//! Le jeu sans 68000 n'exécute plus le code d'origine : tout le reste de la région programme est de la donnée qu'une
//! routine native peut lire. Plutôt que d'extraire les seules zones qu'une mesure a vu lire (`data/natives.json`),
//! ce fichier range tout ce qui n'est pas du code, ce qui rend la région programme complète par construction. Le
//! code exclu est celui que la machine de référence exécute sur tous les scénarios (`port/extract/code_68k.txt`,
//! docs/sans-rom.md « Couverture complète des données ») ; le code jamais exécuté est donc rangé ici aussi, comme
//! les tables rangées dans le code et les données qu'un appel saute.
//!
//! * `donnees` : blocs de l'espace données (lectures du 68000), `debut` puis les octets en hexadécimal, 32 par
//!   ligne ;
//! * `programme` : blocs de l'espace programme déchiffré (lectures relatives au PC), dans la zone chiffrée
//!   seulement : au-delà de `fin_chiffree`, l'espace programme égale l'espace données et la reconstruction le
//!   recopie ;
//! * `code` : les plages de code exclues (`début-fin`, fin exclue). Les assets rendent tout le reste ; une partie
//!   de ce code est quand même décrite par d'autres fichiers (zones lues par les natives, `natives.json`).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json;

/// Octets par ligne d'un bloc.
pub const LIGNE: usize = 32;

#[derive(Serialize, Deserialize, Default)]
pub struct Reste {
    pub description: String,
    /// Fin de la zone chiffrée du programme (exclue) : au-delà, l'espace programme égale l'espace données.
    #[serde(with = "json::hex")]
    pub fin_chiffree: u32,
    /// Octets de code (total des plages de `code`).
    pub octets_de_code: u32,
    /// Plages de code exclues, `"0x400-0x4ec"` (fin exclue).
    pub code: Vec<String>,
    pub donnees: Vec<Bloc>,
    pub programme: Vec<Bloc>,
}

/// Octets consécutifs à partir de `debut`.
#[derive(Serialize, Deserialize)]
pub struct Bloc {
    #[serde(with = "json::hex")]
    pub debut: u32,
    /// Lignes de LIGNE octets (la dernière peut être plus courte), en hexadécimal.
    pub octets: Vec<String>,
}

impl Bloc {
    pub fn new(debut: u32, octets: &[u8]) -> Self {
        let lignes = octets.chunks(LIGNE).map(|l| l.iter().map(|b| format!("{b:02x}")).collect()).collect();
        Bloc { debut, octets: lignes }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for l in &self.octets {
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

impl Reste {
    pub fn load(path: &Path) -> io::Result<Self> {
        json::load(path)
    }

    /// Écrit le fichier : une plage de code ou une ligne d'octets par ligne.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let chaine = |s: &str| serde_json::to_string(s).unwrap();
        let liste = |v: &[String], n: usize| {
            let pad = "  ".repeat(n);
            let l: Vec<String> = v.iter().map(|x| format!("{pad}{}", chaine(x))).collect();
            if l.is_empty() { "[]".to_string() } else { format!("[\n{}\n{}]", l.join(",\n"), "  ".repeat(n - 1)) }
        };
        let blocs = |v: &[Bloc]| {
            let b: Vec<String> = v
                .iter()
                .map(|b| format!("    {{\"debut\": \"{:#x}\", \"octets\": {}}}", b.debut, liste(&b.octets, 3)))
                .collect();
            if b.is_empty() { "[]".to_string() } else { format!("[\n{}\n  ]", b.join(",\n")) }
        };
        let out = format!(
            "{{\n  \"description\": {},\n  \"fin_chiffree\": \"{:#x}\",\n  \"octets_de_code\": {},\n  \"code\": {},\n  \"donnees\": {},\n  \"programme\": {}\n}}\n",
            chaine(&self.description),
            self.fin_chiffree,
            self.octets_de_code,
            liste(&self.code, 2),
            blocs(&self.donnees),
            blocs(&self.programme)
        );
        std::fs::write(path, out)
    }

    /// Plages de code `(début, fin exclue)`.
    pub fn plages_de_code(&self) -> Result<Vec<(u32, u32)>, String> {
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

/// Plages `(début, fin exclue)` des positions où `garde` est vrai.
pub fn plages(n: u32, garde: impl Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for a in 0..n {
        if garde(a) {
            match out.last_mut() {
                Some(r) if r.1 == a => r.1 += 1,
                _ => out.push((a, a + 1)),
            }
        }
    }
    out
}
