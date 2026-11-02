//! RAM supplémentaire de la carte de développement (`A5+0x82` non nul ; jamais sur une borne).
//!
//! La carte de développement a, en 0x660000, une RAM de 16 Ko qui tient une comptabilité plus fine que
//! l'EEPROM : pièces, départs, parties payantes et gratuites, temps de jeu, parties par personnage, durées
//! extrêmes. Chaque zone existe en **trois exemplaires** (0x661xxx, 0x662xxx, 0x663xxx) : un vote
//! (`vote`, 0x2e41c) compare les octets bas des mots des trois exemplaires, répare l'exemplaire
//! minoritaire d'après les deux autres, et remet la zone à zéro si le désaccord est total. Les compteurs
//! sont des longs rangés « à la `movep` » : un octet par mot, aux adresses impaires.
//!
//! Zones (décalage dans l'exemplaire, mots, variable de travail qui en reçoit la valeur) :
//!
//! | Décalage | Mots | Contenu | Variable (A5 -) | Bornes |
//! |---|---|---|---|---|
//! | 0x1140 | 32 | signature « CAPCOM… Dungeons & Dragons 2 » propre à la région | - | recopiée de la ROM |
//! | 0x1020 | 4 | pièces insérées | 0x6164 | > 999 999 : 0 |
//! | 0x1028 | 4 | départs | 0x6160 | > 999 999 : 0 |
//! | 0x1030 | 4 | temps de jeu total | 0x615c | > 0x127a3980 : plafonné |
//! | 0x1038 | 4 | parties payantes | 0x6158 | > 0x4f5880 : plafonné |
//! | 0x1040 | 4 | parties gratuites | 0x6154 | > 0x4f5880 : plafonné |
//! | 0x1048 | 4 | cartes distribuées | 0x6150 | > 0x4f5880 : plafonné |
//! | 0x1200 | 48 | parties par personnage (12 compteurs de 8 octets) | 0x614c (48 octets) | - |
//! | 0x1260 | 8 | durée de partie maximale, puis minimale (secondes) | 0x611c, 0x6118 | - |
//! | 0x1270 | 2 | deux réglages d'un octet (0x1c3 par défaut) | A5+0x1a (2 octets) | octet nul : défaut |
//!
//! (Le temps de jeu moyen que montre GAME DATA est le quotient du temps total par les parties payantes.)
//! Particularités de l'original reproduites : la réparation ne recopie que les 8 premiers octets d'une
//! zone, même celles de 48 ou 16 octets ; la zone 0x1048 n'est pas remise à zéro par la réinitialisation
//! complète ; la zone 0x1240, qui a un vote mais aucun appelant, n'existe pas ici.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};

const RAM: u32 = 0x66_0000;
/// Écart entre deux exemplaires d'une zone.
const INSTANCE: u32 = 0x1000;

/// Début de la signature de région dans le programme (lue comme donnée) ; 16 octets par région.
const SIGNATURE: u32 = 0x2ebfc;
const REGION: u32 = 0x70;

/// Zone de la RAM supplémentaire : décalage de son premier exemplaire, et nombre de mots.
#[derive(Clone, Copy)]
struct Zone {
    debut: u32,
    words: u32,
}

const SIGNATURE_ZONE: Zone = Zone { debut: 0x1140, words: 32 };
const PIECES: Zone = Zone { debut: 0x1020, words: 4 };
const STARTS: Zone = Zone { debut: 0x1028, words: 4 };
const TIME: Zone = Zone { debut: 0x1030, words: 4 };
const PAID: Zone = Zone { debut: 0x1038, words: 4 };
const FREE: Zone = Zone { debut: 0x1040, words: 4 };
const CARDS: Zone = Zone { debut: 0x1048, words: 4 };
const CHARACTERS: Zone = Zone { debut: 0x1200, words: 48 };
const DURATIONS: Zone = Zone { debut: 0x1260, words: 8 };
const SETTINGS: Zone = Zone { debut: 0x1270, words: 2 };

/// Variables de travail qui reçoivent les valeurs (décalage depuis A5, négatif).
const VAR_PIECES: u32 = 0x6164;
const VAR_STARTS: u32 = 0x6160;
const VAR_TIME: u32 = 0x615c;
const VAR_PAID: u32 = 0x6158;
const VAR_FREE: u32 = 0x6154;
const VAR_CARDS: u32 = 0x6150;
const VAR_CHARACTERS: u32 = 0x614c;
const VAR_MAX_DURATION: u32 = 0x611c;
const VAR_MIN_DURATION: u32 = 0x6118;
const VAR_SETTINGS: u32 = 0x1a;

fn var(negative_offset: u32) -> u32 {
    A5.wrapping_sub(negative_offset)
}

impl Zone {
    /// Adresse de l'exemplaire `k` (0, 1 ou 2).
    fn instance(self, k: u32) -> u32 {
        RAM + self.debut + k * INSTANCE
    }
}

// ---------------------------------------------------------------------------------------------
// Valeurs « movep » : un octet par mot, aux adresses impaires

fn read_long(b: &mut Cps2Bus, address: u32) -> u32 {
    (0..4).fold(0, |v, i| v << 8 | rd8(b, address + 1 + 2 * i) as u32)
}

fn write_long(b: &mut Cps2Bus, address: u32, valeur: u32) {
    for i in 0..4 {
        wr8(b, address + 1 + 2 * i, (valeur >> (24 - 8 * i)) as u8);
    }
}

fn write_three(b: &mut Cps2Bus, zone: Zone, valeur: u32) {
    for k in 0..3 {
        write_long(b, zone.instance(k), valeur);
    }
}

// ---------------------------------------------------------------------------------------------
// Vote entre les trois exemplaires

/// Résultat du vote : quels exemplaires diffèrent (bits de `0x2e41c`).
#[derive(PartialEq, Eq, Clone, Copy)]
struct Vote(u16);

impl Vote {
    const B_DIFFERS_FROM_A: u16 = 2;
    const C_DIFFERS_FROM_A: u16 = 4;
    const C_DIFFERS_FROM_B: u16 = 8;
}

/// Compare les octets bas des mots des trois exemplaires (les écarts s'accumulent sur toute la zone) ;
/// s'arrête au premier mot où les trois diffèrent.
fn vote(b: &mut Cps2Bus, zone: Zone) -> Vote {
    let mut v = 0;
    for i in 0..zone.words {
        let [a, bb, c] = [0, 1, 2].map(|k| rd16(b, zone.instance(k) + 2 * i) as u8);
        if bb != a {
            v |= Vote::B_DIFFERS_FROM_A;
        }
        if c != a {
            v |= Vote::C_DIFFERS_FROM_A;
        }
        if c != bb {
            v |= Vote::C_DIFFERS_FROM_B;
        }
        if v == 0xe {
            break;
        }
    }
    Vote(v)
}

/// Exemplaire fautif quand deux exemplaires s'accordent contre le troisième.
enum Verdict {
    Good,
    /// Recopier l'exemplaire `de` sur l'exemplaire `to`.
    Repair { de: u32, to: u32 },
    /// Désaccord sans majorité : la zone est perdue.
    Lost,
}

fn verdict(v: Vote) -> Verdict {
    match v.0 {
        0 => Verdict::Good,
        6 => Verdict::Repair { de: 2, to: 0 },
        10 => Verdict::Repair { de: 0, to: 1 },
        12 => Verdict::Repair { de: 1, to: 2 },
        _ => Verdict::Lost,
    }
}

/// Recopie les 8 premiers octets d'un exemplaire sur un autre (toute la réparation de l'original).
fn repair(b: &mut Cps2Bus, zone: Zone, de: u32, to: u32) {
    for i in 0..2 {
        let v = rd32(b, zone.instance(de) + 4 * i);
        wr32(b, zone.instance(to) + 4 * i, v);
    }
}

/// Vote, puis réparation ou remise à zéro (`lost`) de la zone.
fn check(b: &mut Cps2Bus, zone: Zone, lost: fn(&mut Cps2Bus)) {
    match verdict(vote(b, zone)) {
        Verdict::Good => {}
        Verdict::Repair { de, to } => repair(b, zone, de, to),
        Verdict::Lost => lost(b),
    }
}

// ---------------------------------------------------------------------------------------------
// Réinitialisations

fn region(b: &mut Cps2Bus) -> u32 {
    rd16(b, A5 + REGION) as u32
}

/// Octet `i` de la signature de la région (le décalage `region << 4` est un mot signé, comme dans l'original).
fn signature_byte(b: &mut Cps2Bus, i: u32) -> u8 {
    let decalage = (region(b) << 4) as u16 as i16 as i32;
    rd8(b, SIGNATURE.wrapping_add(decalage as u32) + i)
}

fn write_signature(b: &mut Cps2Bus) {
    for k in 0..3 {
        for i in 0..SIGNATURE_ZONE.words {
            let o = signature_byte(b, i);
            wr16(b, SIGNATURE_ZONE.instance(k) + 2 * i, o as u16);
        }
    }
}

fn zero_pieces(b: &mut Cps2Bus) {
    zone_zero(b, PIECES, 8)
}
fn zero_starts(b: &mut Cps2Bus) {
    zone_zero(b, STARTS, 8)
}
fn zero_time(b: &mut Cps2Bus) {
    zone_zero(b, TIME, 8)
}
fn zero_paid(b: &mut Cps2Bus) {
    zone_zero(b, PAID, 8)
}
fn zero_free(b: &mut Cps2Bus) {
    zone_zero(b, FREE, 8)
}
fn zero_cards(b: &mut Cps2Bus) {
    zone_zero(b, CARDS, 8)
}
fn zero_characters(b: &mut Cps2Bus) {
    zone_zero(b, CHARACTERS, 96)
}
fn zero_durations(b: &mut Cps2Bus) {
    zone_zero(b, DURATIONS, 16)
}

/// Remet à zéro les `bytes` premiers octets de chaque exemplaire (longs pleins, pas « movep »).
fn zone_zero(b: &mut Cps2Bus, zone: Zone, bytes: u32) {
    for k in 0..3 {
        for i in 0..bytes / 4 {
            wr32(b, zone.instance(k) + 4 * i, 0);
        }
    }
}

/// Valeurs par défaut des deux réglages : le long 0x1c3 (écrit plein, pas « movep »).
fn default_settings(b: &mut Cps2Bus) {
    for k in 0..3 {
        wr32(b, SETTINGS.instance(k), 0x1c3);
    }
}

/// Réinitialisation complète (0x2e91a, 0x2e8e4) : toutes les zones, sauf les cartes distribuées.
fn reset_zones(b: &mut Cps2Bus) {
    zero_pieces(b);
    zero_starts(b);
    zero_time(b);
    zero_paid(b);
    zero_free(b);
    write_signature(b);
    zero_characters(b);
    zero_durations(b);
    default_settings(b);
}

// ---------------------------------------------------------------------------------------------
// Contrôle de la signature (0x2dd14)

/// Vote sur la signature, puis réparation en la comparant à celle de la ROM. Faux si la signature ne
/// correspond pas à la région (RAM vierge ou d'une autre version : tout est réinitialisé).
fn valid_signature(b: &mut Cps2Bus) -> bool {
    let (de, to) = match verdict(vote(b, SIGNATURE_ZONE)) {
        Verdict::Good => (0, 1),
        Verdict::Repair { de, to } => (de, to),
        Verdict::Lost => return false,
    };
    for i in 0..SIGNATURE_ZONE.words {
        let word = rd16(b, SIGNATURE_ZONE.instance(de) + 2 * i);
        if word as u8 != signature_byte(b, i) {
            return false;
        }
        wr16(b, SIGNATURE_ZONE.instance(to) + 2 * i, word);
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Contrôle des compteurs

/// Valeur maximale d'un compteur et ce qu'on fait au-delà.
enum Cabinet {
    /// Remis à zéro (pièces, départs).
    Zero(u32),
    /// Ramené au maximum.
    Cap(u32),
}

fn check_counter(b: &mut Cps2Bus, zone: Zone, lost: fn(&mut Cps2Bus), cabinet: Cabinet) {
    check(b, zone, lost);
    let valeur = read_long(b, zone.instance(0));
    match cabinet {
        Cabinet::Zero(max) if valeur > max => write_three(b, zone, 0),
        Cabinet::Cap(max) if valeur > max => write_three(b, zone, max),
        _ => {}
    }
}

fn check_coins(b: &mut Cps2Bus) {
    check_counter(b, PIECES, zero_pieces, Cabinet::Zero(999_999))
}
fn check_starts(b: &mut Cps2Bus) {
    check_counter(b, STARTS, zero_starts, Cabinet::Zero(999_999))
}
fn check_time(b: &mut Cps2Bus) {
    check_counter(b, TIME, zero_time, Cabinet::Cap(0x127a_3980))
}
fn check_paid(b: &mut Cps2Bus) {
    check_counter(b, PAID, zero_paid, Cabinet::Cap(0x4f_5880))
}
fn check_free(b: &mut Cps2Bus) {
    check_counter(b, FREE, zero_free, Cabinet::Cap(0x4f_5880))
}
fn check_cards(b: &mut Cps2Bus) {
    check_counter(b, CARDS, zero_cards, Cabinet::Cap(0x4f_5880))
}
fn check_characters(b: &mut Cps2Bus) {
    check(b, CHARACTERS, zero_characters)
}
fn check_durations(b: &mut Cps2Bus) {
    check(b, DURATIONS, zero_durations)
}

/// Les deux réglages : un octet nul (en 0x...1 ou 0x...3) fait remettre 0x1c3 dans les trois mots, en
/// « movep » (octets impairs 1, 3, 5, 7 : 0x01 0x01 0x01 0xc3).
fn check_settings(b: &mut Cps2Bus) {
    check(b, SETTINGS, default_settings);
    for byte in [1, 3] {
        if rd8(b, SETTINGS.instance(0) + byte) == 0 {
            // movep.w D1,(1,A1), (3,A1), (5,A1) : les écritures se recouvrent
            for decalage in [1, 3, 5] {
                wr8(b, SETTINGS.instance(0) + decalage, 0x01);
                wr8(b, SETTINGS.instance(0) + decalage + 2, 0xc3);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Variables de travail

fn copy_counter(b: &mut Cps2Bus, zone: Zone, variable: u32) {
    let v = read_long(b, zone.instance(0));
    wr32(b, var(variable), v);
}

fn copy_characters(b: &mut Cps2Bus) {
    for i in 0..CHARACTERS.words {
        let o = rd16(b, CHARACTERS.instance(0) + 2 * i) as u8;
        wr8(b, var(VAR_CHARACTERS) + i, o);
    }
}

fn copy_durations(b: &mut Cps2Bus) {
    let min_max = DURATIONS.instance(0);
    let (maxi, mini) = (read_long(b, min_max), read_long(b, min_max + 8));
    wr32(b, var(VAR_MAX_DURATION), maxi);
    wr32(b, var(VAR_MIN_DURATION), mini);
}

fn copy_settings(b: &mut Cps2Bus) {
    for i in 0..SETTINGS.words {
        let o = rd16(b, SETTINGS.instance(0) + 2 * i) as u8;
        wr8(b, A5 + VAR_SETTINGS + i, o);
    }
}

/// 0x2dcd2 : toutes les zones vers les variables de travail.
fn copy_all(b: &mut Cps2Bus) {
    copy_counter(b, PIECES, VAR_PIECES);
    copy_counter(b, STARTS, VAR_STARTS);
    copy_counter(b, PAID, VAR_PAID);
    copy_counter(b, TIME, VAR_TIME);
    copy_counter(b, FREE, VAR_FREE);
    copy_counter(b, CARDS, VAR_CARDS);
    copy_characters(b);
    copy_durations(b);
    copy_settings(b);
}

// ---------------------------------------------------------------------------------------------
// Points d'entrée

/// 0x2dc9a au reset (0x2dca6 puis 0x2dcd2) : contrôle de toute la RAM supplémentaire, puis variables.
pub fn start_up(b: &mut Cps2Bus) {
    if valid_signature(b) {
        check_coins(b);
        check_starts(b);
        check_time(b);
        check_paid(b);
        check_free(b);
        check_cards(b);
        check_characters(b);
        check_durations(b);
        check_settings(b);
    } else {
        reset_zones(b);
    }
    copy_all(b);
}

/// 0x2e8e4 (boutons tenus au reset) : réinitialisation de la RAM supplémentaire, puis variables.
pub fn reset(b: &mut Cps2Bus) {
    reset_zones(b);
    copy_all(b);
}

fn increment(b: &mut Cps2Bus, zone: Zone) {
    for k in 0..3 {
        let a = zone.instance(k);
        let v = read_long(b, a).wrapping_add(1);
        write_long(b, a, v);
    }
}

/// Un compteur de plus (pièce 0x2e456, départ 0x2e4a8, partie payante 0x2e4fc, gratuite 0x2e59a), sa zone
/// contrôlée, et sa variable de travail mise à jour. Rend la nouvelle valeur (dernier long chargé).
pub fn pieces(b: &mut Cps2Bus) -> u32 {
    count(b, PIECES, check_coins, VAR_PIECES)
}
pub fn starts(b: &mut Cps2Bus) -> u32 {
    count(b, STARTS, check_starts, VAR_STARTS)
}
pub fn paid_games(b: &mut Cps2Bus) -> u32 {
    count(b, PAID, check_paid, VAR_PAID)
}
pub fn free_games(b: &mut Cps2Bus) -> u32 {
    count(b, FREE, check_free, VAR_FREE)
}

fn count(b: &mut Cps2Bus, zone: Zone, control: fn(&mut Cps2Bus), variable: u32) -> u32 {
    increment(b, zone);
    control(b);
    copy_counter(b, zone, variable);
    rd32(b, var(variable))
}

/// 0x2e540 (une fois par seconde de jeu) : une seconde de plus au temps de jeu total.
pub fn play_time(b: &mut Cps2Bus) -> u32 {
    count(b, TIME, check_time, VAR_TIME)
}

/// 0x2e638 : une partie de plus pour le personnage `character` (mot signé ; les compteurs font 8 octets).
pub fn character_part(b: &mut Cps2Bus, character: u16) {
    let decalage = ((character as i16) << 3) as i32 as u32;
    for k in 0..3 {
        let a = CHARACTERS.instance(k).wrapping_add(decalage);
        let v = read_long(b, a).wrapping_add(1);
        write_long(b, a, v);
    }
    check_characters(b);
    copy_characters(b);
}

/// 0x2e69e : durée d'une partie (`images`, long signé), en secondes ; garde la plus longue et la plus courte.
pub fn game_duration(b: &mut Cps2Bus, images: u32) {
    let seconds = (images as i32).wrapping_div(60) as u32;
    if seconds > rd32(b, var(VAR_MAX_DURATION)) {
        write_three_durations(b, 0, seconds);
    }
    let mini = rd32(b, var(VAR_MIN_DURATION));
    if mini == 0 || seconds < mini {
        write_three_durations(b, 8, seconds);
    }
    check_durations(b);
    copy_durations(b);
}

fn write_three_durations(b: &mut Cps2Bus, decalage: u32, valeur: u32) {
    for k in 0..3 {
        write_long(b, DURATIONS.instance(k) + decalage, valeur);
    }
}
