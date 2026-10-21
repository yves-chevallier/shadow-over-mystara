//! Fichiers de données en JSON : adresses ROM en hexadécimal, une entrée par ligne.

use std::io;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Adresse ROM écrite `"0x2b45d8"`.
pub mod hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u32, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:#x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        let s = String::deserialize(d)?;
        parse(&s).ok_or_else(|| serde::de::Error::custom(format!("adresse invalide : {s}")))
    }

    pub fn parse(s: &str) -> Option<u32> {
        u32::from_str_radix(s.strip_prefix("0x")?, 16).ok()
    }
}

/// Liste d'adresses ROM.
pub mod hex_vec {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u32], s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|a| format!("{a:#x}")))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
        Vec::<String>::deserialize(d)?
            .iter()
            .map(|s| super::hex::parse(s).ok_or_else(|| serde::de::Error::custom(format!("adresse invalide : {s}"))))
            .collect()
    }
}

/// Adresse ROM facultative.
pub mod hex_opt {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<u32>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(a) => super::hex::serialize(a, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
        Option::<String>::deserialize(d)?
            .map(|s| super::hex::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("adresse invalide : {s}"))))
            .transpose()
    }
}

pub fn load<T: DeserializeOwned>(path: &Path) -> io::Result<T> {
    let f = io::BufReader::new(std::fs::File::open(path)?);
    serde_json::from_reader(f).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display())))
}

/// Écrit `v` en JSON indenté jusqu'à la profondeur `depth`, compact au-delà (et pour les listes de
/// nombres) : une image, une entrée d'animation ou une bande par ligne. Les champs gardent l'ordre des
/// structures.
pub fn save<T: Serialize>(path: &Path, v: &T, depth: usize) -> io::Result<()> {
    let compact = serde_json::to_string(v).map_err(io::Error::other)?;
    let mut out = layout(&compact, depth);
    out.push('\n');
    std::fs::write(path, out)
}

/// Réindente un JSON compact : retour à la ligne entre les éléments des objets et des listes d'objets
/// ou de listes de profondeur inférieure à `depth`.
fn layout(json: &str, depth: usize) -> String {
    let b = json.as_bytes();
    let mut out = String::with_capacity(json.len() + json.len() / 8);
    let mut open: Vec<bool> = Vec::new();
    let (mut in_str, mut escaped) = (false, false);
    let indent = |out: &mut String, n: usize| {
        out.push('\n');
        out.push_str(&"  ".repeat(n));
    };
    for (i, c) in json.char_indices() {
        if in_str {
            out.push(c);
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '{' | '[' => {
                let next = b.get(i + 1).copied();
                let empty = matches!(next, Some(b'}' | b']'));
                // liste dont les premiers éléments sont `null` : imbriquée si le premier élément non nul l'est
                let mut k = i + 1;
                while b[k..].starts_with(b"null,") {
                    k += 5;
                }
                let nested = c == '{' || matches!(b.get(k), Some(b'{' | b'['));
                let expand = open.len() < depth && nested && !empty;
                open.push(expand);
                out.push(c);
                if expand {
                    indent(&mut out, open.len());
                }
            }
            '}' | ']' => {
                if open.pop() == Some(true) {
                    indent(&mut out, open.len());
                }
                out.push(c);
            }
            ',' => {
                out.push(c);
                if open.last() == Some(&true) {
                    indent(&mut out, open.len());
                }
            }
            ':' => {
                out.push(c);
                if open.last() == Some(&true) {
                    out.push(' ');
                }
            }
            _ => out.push(c),
        }
    }
    out
}
