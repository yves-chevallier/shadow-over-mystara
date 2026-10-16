//! Arithmétique du pilote et adresses des tables de la ROM du Z80.

/// Table de 128 mots : amplitude des voies (instruction 7, commandes d'effets).
pub const TABLE_AMPLITUDE: u32 = 0x33e1;
/// Table de mots : niveaux généraux (volumes de base, cibles des rampes).
pub const TABLE_LEVELS: u32 = 0x34e1;
/// Table de mots : vitesses d'attaque des enveloppes.
pub const TABLE_ATTACK: u32 = 0x35e1;
/// Table de mots : vitesses de chute des enveloppes (decay, release).
pub const TABLE_DECAY: u32 = 0x3661;
/// Table de mots : vitesses du LFO (vibrato et trémolo).
pub const TABLE_LFO_RATE: u32 = 0x36e1;
/// Table de mots : profondeurs de vibrato.
pub const TABLE_VIBRATO: u32 = 0x37e1;
/// Table de mots : profondeurs de trémolo.
pub const TABLE_TREMOLO: u32 = 0x38e1;
/// Adresse (mot) de l'en-tête des musiques.
pub const HEADER_PTR: u32 = 0x39e1;
/// Adresse (mot) de la table des échantillons : fiches de 8 octets [banque, début, début de
/// boucle, fin, note de référence].
pub const SAMPLE_PTR: u32 = 0x39e3;
/// Adresse (mot) de la table des bancs d'instruments.
pub const INSTRUMENT_BANKS_PTR: u32 = 0x39e5;

/// Octave de chaque demi-ton (0-107), puis début d'octave en demi-tons.
pub const TABLE_OCTAVE: u32 = 0x1fc6;
pub const TABLE_OCTAVE_START: u32 = 0x2032;
/// Taux de base de chaque demi-ton (mots), et interpolation fine (12 x 256 octets).
pub const TABLE_RATE: u32 = 0x203b;
pub const TABLE_FINE: u32 = 0x2113;
/// Durées des notes (mots) selon le mode (normal, pointé, triolet).
pub const TABLE_DURATION: u32 = 0x1119;
/// Décalage d'octave de la voie (4 bits bas des drapeaux).
pub const TABLE_OCTAVE_BASE: u32 = 0x1149;
/// Valeurs de pan : registre = PAN_BASE + position (0-0x20).
pub const PAN_BASE: u16 = 0x110;

/// Produit 16 x 16 bits, mots de poids fort (le Z80 le fait par décalages et additions).
pub fn hi16(a: u16, b: u16) -> u16 {
    ((u32::from(a) * u32::from(b)) >> 16) as u16
}

/// Octet étendu en mot signé.
pub fn sext(b: u8) -> u16 {
    i16::from(b as i8) as u16
}

/// Borne un pan à 0x20.
pub fn clamp_pan(p: u8) -> u8 {
    p.min(0x20)
}
