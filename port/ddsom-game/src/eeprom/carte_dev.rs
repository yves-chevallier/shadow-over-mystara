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
const EXEMPLAIRE: u32 = 0x1000;

/// Début de la signature de région dans le programme (lue comme donnée) ; 16 octets par région.
const SIGNATURE: u32 = 0x2ebfc;
const REGION: u32 = 0x70;

/// Zone de la RAM supplémentaire : décalage de son premier exemplaire, et nombre de mots.
#[derive(Clone, Copy)]
struct Zone {
    debut: u32,
    mots: u32,
}

const SIGNATURE_ZONE: Zone = Zone { debut: 0x1140, mots: 32 };
const PIECES: Zone = Zone { debut: 0x1020, mots: 4 };
const DEPARTS: Zone = Zone { debut: 0x1028, mots: 4 };
const TEMPS: Zone = Zone { debut: 0x1030, mots: 4 };
const PAYANTES: Zone = Zone { debut: 0x1038, mots: 4 };
const GRATUITES: Zone = Zone { debut: 0x1040, mots: 4 };
const CARTES: Zone = Zone { debut: 0x1048, mots: 4 };
const PERSONNAGES: Zone = Zone { debut: 0x1200, mots: 48 };
const DUREES: Zone = Zone { debut: 0x1260, mots: 8 };
const REGLAGES: Zone = Zone { debut: 0x1270, mots: 2 };

/// Variables de travail qui reçoivent les valeurs (décalage depuis A5, négatif).
const VAR_PIECES: u32 = 0x6164;
const VAR_DEPARTS: u32 = 0x6160;
const VAR_TEMPS: u32 = 0x615c;
const VAR_PAYANTES: u32 = 0x6158;
const VAR_GRATUITES: u32 = 0x6154;
const VAR_CARTES: u32 = 0x6150;
const VAR_PERSONNAGES: u32 = 0x614c;
const VAR_DUREE_MAX: u32 = 0x611c;
const VAR_DUREE_MIN: u32 = 0x6118;
const VAR_REGLAGES: u32 = 0x1a;

fn var(decalage_negatif: u32) -> u32 {
    A5.wrapping_sub(decalage_negatif)
}

impl Zone {
    /// Adresse de l'exemplaire `k` (0, 1 ou 2).
    fn exemplaire(self, k: u32) -> u32 {
        RAM + self.debut + k * EXEMPLAIRE
    }
}

// ---------------------------------------------------------------------------------------------
// Valeurs « movep » : un octet par mot, aux adresses impaires

fn lire_long(b: &mut Cps2Bus, adresse: u32) -> u32 {
    (0..4).fold(0, |v, i| v << 8 | rd8(b, adresse + 1 + 2 * i) as u32)
}

fn ecrire_long(b: &mut Cps2Bus, adresse: u32, valeur: u32) {
    for i in 0..4 {
        wr8(b, adresse + 1 + 2 * i, (valeur >> (24 - 8 * i)) as u8);
    }
}

fn ecrire_trois(b: &mut Cps2Bus, zone: Zone, valeur: u32) {
    for k in 0..3 {
        ecrire_long(b, zone.exemplaire(k), valeur);
    }
}

// ---------------------------------------------------------------------------------------------
// Vote entre les trois exemplaires

/// Résultat du vote : quels exemplaires diffèrent (bits de `0x2e41c`).
#[derive(PartialEq, Eq, Clone, Copy)]
struct Vote(u16);

impl Vote {
    const B_DIFFERE_DE_A: u16 = 2;
    const C_DIFFERE_DE_A: u16 = 4;
    const C_DIFFERE_DE_B: u16 = 8;
}

/// Compare les octets bas des mots des trois exemplaires (les écarts s'accumulent sur toute la zone) ;
/// s'arrête au premier mot où les trois diffèrent.
fn vote(b: &mut Cps2Bus, zone: Zone) -> Vote {
    let mut v = 0;
    for i in 0..zone.mots {
        let [a, bb, c] = [0, 1, 2].map(|k| rd16(b, zone.exemplaire(k) + 2 * i) as u8);
        if bb != a {
            v |= Vote::B_DIFFERE_DE_A;
        }
        if c != a {
            v |= Vote::C_DIFFERE_DE_A;
        }
        if c != bb {
            v |= Vote::C_DIFFERE_DE_B;
        }
        if v == 0xe {
            break;
        }
    }
    Vote(v)
}

/// Exemplaire fautif quand deux exemplaires s'accordent contre le troisième.
enum Verdict {
    Bon,
    /// Recopier l'exemplaire `de` sur l'exemplaire `vers`.
    Reparer { de: u32, vers: u32 },
    /// Désaccord sans majorité : la zone est perdue.
    Perdue,
}

fn verdict(v: Vote) -> Verdict {
    match v.0 {
        0 => Verdict::Bon,
        6 => Verdict::Reparer { de: 2, vers: 0 },
        10 => Verdict::Reparer { de: 0, vers: 1 },
        12 => Verdict::Reparer { de: 1, vers: 2 },
        _ => Verdict::Perdue,
    }
}

/// Recopie les 8 premiers octets d'un exemplaire sur un autre (toute la réparation de l'original).
fn reparer(b: &mut Cps2Bus, zone: Zone, de: u32, vers: u32) {
    for i in 0..2 {
        let v = rd32(b, zone.exemplaire(de) + 4 * i);
        wr32(b, zone.exemplaire(vers) + 4 * i, v);
    }
}

/// Vote, puis réparation ou remise à zéro (`perdue`) de la zone.
fn controler(b: &mut Cps2Bus, zone: Zone, perdue: fn(&mut Cps2Bus)) {
    match verdict(vote(b, zone)) {
        Verdict::Bon => {}
        Verdict::Reparer { de, vers } => reparer(b, zone, de, vers),
        Verdict::Perdue => perdue(b),
    }
}

// ---------------------------------------------------------------------------------------------
// Réinitialisations

fn region(b: &mut Cps2Bus) -> u32 {
    rd16(b, A5 + REGION) as u32
}

/// Octet `i` de la signature de la région (le décalage `region << 4` est un mot signé, comme dans l'original).
fn octet_signature(b: &mut Cps2Bus, i: u32) -> u8 {
    let decalage = (region(b) << 4) as u16 as i16 as i32;
    rd8(b, SIGNATURE.wrapping_add(decalage as u32) + i)
}

fn ecrire_signature(b: &mut Cps2Bus) {
    for k in 0..3 {
        for i in 0..SIGNATURE_ZONE.mots {
            let o = octet_signature(b, i);
            wr16(b, SIGNATURE_ZONE.exemplaire(k) + 2 * i, o as u16);
        }
    }
}

fn zero_pieces(b: &mut Cps2Bus) {
    zone_zero(b, PIECES, 8)
}
fn zero_departs(b: &mut Cps2Bus) {
    zone_zero(b, DEPARTS, 8)
}
fn zero_temps(b: &mut Cps2Bus) {
    zone_zero(b, TEMPS, 8)
}
fn zero_payantes(b: &mut Cps2Bus) {
    zone_zero(b, PAYANTES, 8)
}
fn zero_gratuites(b: &mut Cps2Bus) {
    zone_zero(b, GRATUITES, 8)
}
fn zero_cartes(b: &mut Cps2Bus) {
    zone_zero(b, CARTES, 8)
}
fn zero_personnages(b: &mut Cps2Bus) {
    zone_zero(b, PERSONNAGES, 96)
}
fn zero_durees(b: &mut Cps2Bus) {
    zone_zero(b, DUREES, 16)
}

/// Remet à zéro les `octets` premiers octets de chaque exemplaire (longs pleins, pas « movep »).
fn zone_zero(b: &mut Cps2Bus, zone: Zone, octets: u32) {
    for k in 0..3 {
        for i in 0..octets / 4 {
            wr32(b, zone.exemplaire(k) + 4 * i, 0);
        }
    }
}

/// Valeurs par défaut des deux réglages : le long 0x1c3 (écrit plein, pas « movep »).
fn defaut_reglages(b: &mut Cps2Bus) {
    for k in 0..3 {
        wr32(b, REGLAGES.exemplaire(k), 0x1c3);
    }
}

/// Réinitialisation complète (0x2e91a, 0x2e8e4) : toutes les zones, sauf les cartes distribuées.
fn reinitialiser_zones(b: &mut Cps2Bus) {
    zero_pieces(b);
    zero_departs(b);
    zero_temps(b);
    zero_payantes(b);
    zero_gratuites(b);
    ecrire_signature(b);
    zero_personnages(b);
    zero_durees(b);
    defaut_reglages(b);
}

// ---------------------------------------------------------------------------------------------
// Contrôle de la signature (0x2dd14)

/// Vote sur la signature, puis réparation en la comparant à celle de la ROM. Faux si la signature ne
/// correspond pas à la région (RAM vierge ou d'une autre version : tout est réinitialisé).
fn signature_valide(b: &mut Cps2Bus) -> bool {
    let (de, vers) = match verdict(vote(b, SIGNATURE_ZONE)) {
        Verdict::Bon => (0, 1),
        Verdict::Reparer { de, vers } => (de, vers),
        Verdict::Perdue => return false,
    };
    for i in 0..SIGNATURE_ZONE.mots {
        let mot = rd16(b, SIGNATURE_ZONE.exemplaire(de) + 2 * i);
        if mot as u8 != octet_signature(b, i) {
            return false;
        }
        wr16(b, SIGNATURE_ZONE.exemplaire(vers) + 2 * i, mot);
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Contrôle des compteurs

/// Valeur maximale d'un compteur et ce qu'on fait au-delà.
enum Borne {
    /// Remis à zéro (pièces, départs).
    Zero(u32),
    /// Ramené au maximum.
    Plafond(u32),
}

fn controler_compteur(b: &mut Cps2Bus, zone: Zone, perdue: fn(&mut Cps2Bus), borne: Borne) {
    controler(b, zone, perdue);
    let valeur = lire_long(b, zone.exemplaire(0));
    match borne {
        Borne::Zero(max) if valeur > max => ecrire_trois(b, zone, 0),
        Borne::Plafond(max) if valeur > max => ecrire_trois(b, zone, max),
        _ => {}
    }
}

fn controler_pieces(b: &mut Cps2Bus) {
    controler_compteur(b, PIECES, zero_pieces, Borne::Zero(999_999))
}
fn controler_departs(b: &mut Cps2Bus) {
    controler_compteur(b, DEPARTS, zero_departs, Borne::Zero(999_999))
}
fn controler_temps(b: &mut Cps2Bus) {
    controler_compteur(b, TEMPS, zero_temps, Borne::Plafond(0x127a_3980))
}
fn controler_payantes(b: &mut Cps2Bus) {
    controler_compteur(b, PAYANTES, zero_payantes, Borne::Plafond(0x4f_5880))
}
fn controler_gratuites(b: &mut Cps2Bus) {
    controler_compteur(b, GRATUITES, zero_gratuites, Borne::Plafond(0x4f_5880))
}
fn controler_cartes(b: &mut Cps2Bus) {
    controler_compteur(b, CARTES, zero_cartes, Borne::Plafond(0x4f_5880))
}
fn controler_personnages(b: &mut Cps2Bus) {
    controler(b, PERSONNAGES, zero_personnages)
}
fn controler_durees(b: &mut Cps2Bus) {
    controler(b, DUREES, zero_durees)
}

/// Les deux réglages : un octet nul (en 0x...1 ou 0x...3) fait remettre 0x1c3 dans les trois mots, en
/// « movep » (octets impairs 1, 3, 5, 7 : 0x01 0x01 0x01 0xc3).
fn controler_reglages(b: &mut Cps2Bus) {
    controler(b, REGLAGES, defaut_reglages);
    for octet in [1, 3] {
        if rd8(b, REGLAGES.exemplaire(0) + octet) == 0 {
            // movep.w D1,(1,A1), (3,A1), (5,A1) : les écritures se recouvrent
            for decalage in [1, 3, 5] {
                wr8(b, REGLAGES.exemplaire(0) + decalage, 0x01);
                wr8(b, REGLAGES.exemplaire(0) + decalage + 2, 0xc3);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Variables de travail

fn copier_compteur(b: &mut Cps2Bus, zone: Zone, variable: u32) {
    let v = lire_long(b, zone.exemplaire(0));
    wr32(b, var(variable), v);
}

fn copier_personnages(b: &mut Cps2Bus) {
    for i in 0..PERSONNAGES.mots {
        let o = rd16(b, PERSONNAGES.exemplaire(0) + 2 * i) as u8;
        wr8(b, var(VAR_PERSONNAGES) + i, o);
    }
}

fn copier_durees(b: &mut Cps2Bus) {
    let min_max = DUREES.exemplaire(0);
    let (maxi, mini) = (lire_long(b, min_max), lire_long(b, min_max + 8));
    wr32(b, var(VAR_DUREE_MAX), maxi);
    wr32(b, var(VAR_DUREE_MIN), mini);
}

fn copier_reglages(b: &mut Cps2Bus) {
    for i in 0..REGLAGES.mots {
        let o = rd16(b, REGLAGES.exemplaire(0) + 2 * i) as u8;
        wr8(b, A5 + VAR_REGLAGES + i, o);
    }
}

/// 0x2dcd2 : toutes les zones vers les variables de travail.
fn copier_tout(b: &mut Cps2Bus) {
    copier_compteur(b, PIECES, VAR_PIECES);
    copier_compteur(b, DEPARTS, VAR_DEPARTS);
    copier_compteur(b, PAYANTES, VAR_PAYANTES);
    copier_compteur(b, TEMPS, VAR_TEMPS);
    copier_compteur(b, GRATUITES, VAR_GRATUITES);
    copier_compteur(b, CARTES, VAR_CARTES);
    copier_personnages(b);
    copier_durees(b);
    copier_reglages(b);
}

// ---------------------------------------------------------------------------------------------
// Points d'entrée

/// 0x2dc9a au reset (0x2dca6 puis 0x2dcd2) : contrôle de toute la RAM supplémentaire, puis variables.
pub fn demarrer(b: &mut Cps2Bus) {
    if signature_valide(b) {
        controler_pieces(b);
        controler_departs(b);
        controler_temps(b);
        controler_payantes(b);
        controler_gratuites(b);
        controler_cartes(b);
        controler_personnages(b);
        controler_durees(b);
        controler_reglages(b);
    } else {
        reinitialiser_zones(b);
    }
    copier_tout(b);
}

/// 0x2e8e4 (boutons tenus au reset) : réinitialisation de la RAM supplémentaire, puis variables.
pub fn reinitialiser(b: &mut Cps2Bus) {
    reinitialiser_zones(b);
    copier_tout(b);
}

fn incrementer(b: &mut Cps2Bus, zone: Zone) {
    for k in 0..3 {
        let a = zone.exemplaire(k);
        let v = lire_long(b, a).wrapping_add(1);
        ecrire_long(b, a, v);
    }
}

/// Un compteur de plus (pièce 0x2e456, départ 0x2e4a8, partie payante 0x2e4fc, gratuite 0x2e59a), sa zone
/// contrôlée, et sa variable de travail mise à jour. Rend la nouvelle valeur (dernier long chargé).
pub fn pieces(b: &mut Cps2Bus) -> u32 {
    compter(b, PIECES, controler_pieces, VAR_PIECES)
}
pub fn departs(b: &mut Cps2Bus) -> u32 {
    compter(b, DEPARTS, controler_departs, VAR_DEPARTS)
}
pub fn parties_payantes(b: &mut Cps2Bus) -> u32 {
    compter(b, PAYANTES, controler_payantes, VAR_PAYANTES)
}
pub fn parties_gratuites(b: &mut Cps2Bus) -> u32 {
    compter(b, GRATUITES, controler_gratuites, VAR_GRATUITES)
}

fn compter(b: &mut Cps2Bus, zone: Zone, controle: fn(&mut Cps2Bus), variable: u32) -> u32 {
    incrementer(b, zone);
    controle(b);
    copier_compteur(b, zone, variable);
    rd32(b, var(variable))
}

/// 0x2e540 (une fois par seconde de jeu) : une seconde de plus au temps de jeu total.
pub fn temps_de_jeu(b: &mut Cps2Bus) -> u32 {
    compter(b, TEMPS, controler_temps, VAR_TEMPS)
}

/// 0x2e638 : une partie de plus pour le personnage `personnage` (mot signé ; les compteurs font 8 octets).
pub fn partie_du_personnage(b: &mut Cps2Bus, personnage: u16) {
    let decalage = ((personnage as i16) << 3) as i32 as u32;
    for k in 0..3 {
        let a = PERSONNAGES.exemplaire(k).wrapping_add(decalage);
        let v = lire_long(b, a).wrapping_add(1);
        ecrire_long(b, a, v);
    }
    controler_personnages(b);
    copier_personnages(b);
}

/// 0x2e69e : durée d'une partie (`images`, long signé), en secondes ; garde la plus longue et la plus courte.
pub fn duree_de_partie(b: &mut Cps2Bus, images: u32) {
    let secondes = (images as i32).wrapping_div(60) as u32;
    if secondes > rd32(b, var(VAR_DUREE_MAX)) {
        ecrire_trois_durees(b, 0, secondes);
    }
    let mini = rd32(b, var(VAR_DUREE_MIN));
    if mini == 0 || secondes < mini {
        ecrire_trois_durees(b, 8, secondes);
    }
    controler_durees(b);
    copier_durees(b);
}

fn ecrire_trois_durees(b: &mut Cps2Bus, decalage: u32, valeur: u32) {
    for k in 0..3 {
        ecrire_long(b, DUREES.exemplaire(k) + decalage, valeur);
    }
}
