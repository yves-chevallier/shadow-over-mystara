//! Textes de la couche de caractères 8x8 (descripteurs lus par les routines V2, `ddsom-game/src/text.rs`) et
//! noms des bulles (`popups.rs`). Format : `assets/texts.json`, voir docs/assets.md.
//!
//! Les octets d'un texte sont des numéros de tuiles 8x8 : la police suit l'ASCII (0x20 à 0x7e), 0xfc est
//! le symbole ©. Dans le JSON, un texte d'octets est une chaîne : les caractères ASCII imprimables sont
//! écrits tels quels, sauf `{` et `}` ; tout autre octet s'écrit `{xx}` (hexadécimal), et l'octet qui suit
//! 0xff (nombre d'espaces) aussi. Le terminateur n'est pas écrit.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Routine qui lit un descripteur.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    /// 0xd5e (écrit), 0xd32 (efface), 0xdca (écrit sans effacer le fond) : colonne, ligne, attributs (mot),
    /// octets terminés par `@` ; `/` (0xdca : `_`) passe à la ligne, 0xff n écrit n espaces.
    Text,
    /// 0x1136 : colonne, ligne, attributs, mots terminés par `@@` ; chaque mot `t` écrit la tuile `t` et
    /// `t + 0x10` en dessous ; `##` saute une colonne, `/@` passe à la ligne, `//` deux lignes plus bas.
    Tall,
    /// 0x1234 : comme `Tall`, sans colonne ni ligne (position en D5, D6).
    TallAt,
    /// 0xf38 (dessine), 0xf4a (efface) : blocs de 2x2 tuiles `t, t+1, t+0x10, t+0x11` ; `##` saute deux
    /// colonnes, `//` deux lignes, 0xffff n : n blocs vides, `@@` fin.
    Blocks,
    /// 0x1324 : rectangle d'espaces (colonne, ligne, largeur, hauteur).
    Box,
}

/// Un descripteur. Champs selon la sorte : `col`, `row` (sauf `tall_at`), `attr` (sauf `box`), puis `text`
/// (`text`), `words` (`tall`, `tall_at`, `blocks`, sans le `@@` final) ou `size` (`box`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TextDesc {
    #[serde(with = "hex")]
    pub rom: u32,
    pub kind: TextKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attr: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u8; 2]>,
    /// Trouvé en suivant le code (`false`) ou seulement par la recherche de motifs (`true`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub scanned: bool,
}

impl TextDesc {
    /// Octets du descripteur en ROM, terminateur compris.
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        if let (Some(c), Some(r)) = (self.col, self.row) {
            b.extend([c, r]);
        }
        if let Some(a) = self.attr {
            b.extend(a.to_be_bytes());
        }
        match self.kind {
            TextKind::Text => {
                b.extend(decode(self.text.as_deref().unwrap_or("")));
                b.push(b'@');
            }
            TextKind::Tall | TextKind::TallAt | TextKind::Blocks => {
                for w in self.words.iter().flatten() {
                    b.extend(w.to_be_bytes());
                }
                b.extend([b'@', b'@']);
            }
            TextKind::Box => b.extend(self.size.unwrap_or_default()),
        }
        b
    }
}

/// Table de descripteurs lue par le code : longs (adresses) ou mots (déplacements depuis la table).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TextTable {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Lue en espace programme (accès relatif au PC).
    pub program: bool,
    /// Mots de déplacement depuis la table plutôt que longs.
    pub offsets: bool,
    /// Descripteurs désignés, dans l'ordre de la table.
    #[serde(with = "hex_vec")]
    pub entries: Vec<u32>,
}

/// Noms affichés dans les bulles (0x19fd2) : chaînes de tuiles terminées par `/`, désignées par numéro.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PopupNames {
    /// Table de déplacements (mots, espace programme), un par numéro de texte.
    #[serde(with = "hex")]
    pub table: u32,
    pub offsets: Vec<i16>,
    /// Les chaînes, à la suite à partir de `strings_rom`, sans le `/` final.
    #[serde(with = "hex")]
    pub strings_rom: u32,
    pub strings: Vec<String>,
}

/// Noms terminés par `@` (sans position ni attributs), à la suite en ROM, désignés par une table de longs :
/// noms des boss de la barre de vie (0xd6232, données), des personnages (0xb20e8, programme).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NameGroup {
    #[serde(with = "hex")]
    pub table: u32,
    pub program: bool,
    #[serde(with = "hex_vec")]
    pub entries: Vec<u32>,
    /// Les noms, à la suite à partir de `rom`, sans le `@` final (y compris ceux qu'aucune entrée ne
    /// désigne).
    #[serde(with = "hex")]
    pub rom: u32,
    pub names: Vec<String>,
}

impl NameGroup {
    /// Adresse de chaque nom.
    pub fn starts(&self) -> Vec<u32> {
        let mut a = self.rom;
        self.names
            .iter()
            .map(|n| {
                let s = a;
                a += decode(n).len() as u32 + 1;
                s
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Texts {
    pub texts: Vec<TextDesc>,
    pub tables: Vec<TextTable>,
    pub popups: PopupNames,
    pub names: Vec<NameGroup>,
}

impl Texts {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    pub fn by_rom(&self, rom: u32) -> Option<&TextDesc> {
        self.texts.binary_search_by_key(&rom, |t| t.rom).ok().map(|i| &self.texts[i])
    }
}

/// Octets -> chaîne du JSON (voir l'en-tête du module).
pub fn encode(bytes: &[u8]) -> String {
    let mut s = String::new();
    let mut count = false;
    for &b in bytes {
        if !count && (0x20..=0x7e).contains(&b) && b != b'{' && b != b'}' {
            s.push(b as char);
        } else {
            s.push_str(&format!("{{{b:02x}}}"));
        }
        count = !count && b == 0xff;
    }
    s
}

/// Chaîne du JSON -> octets.
pub fn decode(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '{' {
            let h: String = chars.by_ref().take(3).collect();
            out.push(u8::from_str_radix(&h[..2], 16).unwrap_or(0));
        } else {
            out.push(c as u8);
        }
    }
    out
}
