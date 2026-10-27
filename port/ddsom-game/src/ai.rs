//! Aides d'IA communes aux ennemis et aux boss (sous-système E1) : choix d'un joueur comme cible, points de vie
//! et immunités initiaux, décisions pseudo-aléatoires par tables, tests de position, maintien dans l'écran.
//!
//! Chaque routine d'origine est une fonction Rust ordinaire sur l'objet traité (`Obj`, l'A0 de l'original) : ses
//! entrées en arguments, son résultat en valeur Rust. Les gestionnaires (`families::enemy_lib`) les appellent
//! directement ; aucune ne reproduit plus les registres de l'original (preuve : `DDSOM_POISON`, voir
//! `docs/refactorisation.md`, lot IA).
//!
//! Joueurs : 4 enregistrements de 0x200 octets à A5+0x5de (`ram::JOUEURS`) ; un joueur est « ciblable » s'il est
//! actif (`+0`), d'état principal < 4 (`+4`), et non masqué (bit 1 de `+1`), avec une règle sur `+0x182`
//! ([`Rule`]).

use cps2::Cps2Bus;

use crate::mem::{prog32, prog8, rd16, rd32, rd8};
use crate::objects::Obj;
use crate::player::Player;
use crate::ram::{self, Mot, Octet, JOUEURS, JOUEURS_EN_VIE};
use crate::rng;

/// Rang (difficulté dynamique 0..31, `objects::rank`) : il indexe les tables de décision et de PV.
pub const RANG: Mot = Mot::a5(0x7f92);
/// Niveau de difficulté du réglage (h) : au-delà de 5, les PV initiaux viennent de la seconde moitié de la table.
const NIVEAU: Octet = Octet::a5(0x48);

/// Champs de l'ennemi lus ou écrits par ces aides.
pub mod f {
    /// Cible : mot bas de l'adresse d'un joueur ou d'un objet (0 : aucune).
    pub const TARGET: u32 = 0x50;
    /// Orientation : 0 regarde à droite, 0x20 à gauche.
    pub const FLIP: u32 = 0x2c;
    /// Cap (0..31, index de la table de vitesses : `Obj::SPEED_INDEX`).
    pub const HEADING: u32 = 0x2d;
    /// Points de vie : maximum, courants, précédents.
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const HP_PREV: u32 = 0x64;
    /// Lignes des tables de décision par masque (0xa4b4, 0xa4c2).
    pub const DECISION_A: u32 = 0x72;
    pub const DECISION_B: u32 = 0x73;
    /// Ligne de la table d'octets par rang et octet tiré (0xac3a).
    pub const BYTE_TABLE: u32 = 0x76;
    pub const BYTE: u32 = 0x77;
    /// Immunités (long, bits par sorte d'attaque).
    pub const IMMUNITIES: u32 = 0x94;
    /// Rang de PV : (joueurs en vie - 1) × 32 + rang pair (colonne des tables de paliers).
    pub const HP_RANK: u32 = 0xac;
}

// ---------------------------------------------------------------------------------------------
// Choix d'un joueur comme cible (0x424e, 0x43ae, 0x43bc, 0x44ee)

/// Règle d'éligibilité d'un joueur.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    /// 0x424e, 0x44ee : `+0x182` non nul (aura) exclut le joueur.
    Strict,
    /// 0x43ae, 0x43bc : `+0x182` non nul dispense du test « masqué ».
    Lenient,
}

/// Critère de choix parmi les joueurs ciblables (D0 de 0x43bc : 0, 2, 4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Criterion {
    /// Le plus proche (|dx| + |dy|) ; à égalité, le premier.
    Nearest,
    /// Celui qui a le moins de PV ; à égalité, le dernier.
    LowestHp,
    /// Le plus éloigné ; à égalité, le dernier.
    Farthest,
}

impl Criterion {
    /// Octet de critère des tables de l'original (0, 2 ou 4).
    pub fn from_byte(v: u8) -> Criterion {
        match v {
            0 => Criterion::Nearest,
            2 => Criterion::LowestHp,
            4 => Criterion::Farthest,
            _ => unreachable!("critère de cible {v} : les tables n'ont que 0, 2 et 4"),
        }
    }
}

/// Le joueur `p` peut-il être pris pour cible ?
pub fn targetable(b: &mut Cps2Bus, p: Obj, rule: Rule) -> bool {
    let aura = p.u8(b, Player::AURA) != 0;
    let hidden = p.u8(b, Player::FLAGS) & 2 != 0;
    let visible = match rule {
        Rule::Strict => !aura && !hidden,
        Rule::Lenient => aura || !hidden,
    };
    p.u8(b, Player::ACTIVE) != 0 && visible && (p.u8(b, Player::MAIN) as i8) < 4
}

/// Écart |dx| + |dy| (mots, comme l'original : `sub`, `neg` si négatif, `add`).
fn manhattan(b: &mut Cps2Bus, a: Obj, other: Obj) -> u16 {
    let dx = other.u16(b, Obj::X).wrapping_sub(a.u16(b, Obj::X)) as i16;
    let dy = other.u16(b, Obj::Y).wrapping_sub(a.u16(b, Obj::Y)) as i16;
    (dx.wrapping_abs() as u16).wrapping_add(dy.wrapping_abs() as u16)
}

/// Corps de 0x427a / 0x42e0 / 0x4338 (règle stricte) et 0x43ca / 0x4430 / 0x4488 (règle large) : le joueur qui
/// répond le mieux au critère devient la cible de `o` (`+0x50`, 0 si aucun).
pub fn target_by(b: &mut Cps2Bus, o: Obj, criterion: Criterion, rule: Rule) -> Option<Obj> {
    let mut best: u16 = match criterion {
        Criterion::Nearest => 0xffff,
        Criterion::LowestHp => 0x7fff,
        Criterion::Farthest => 0,
    };
    let mut found = None;
    for p in JOUEURS.iter() {
        if !targetable(b, p, rule) {
            continue;
        }
        let (value, better) = match criterion {
            Criterion::LowestHp => {
                let hp = p.u16(b, Player::HP);
                (hp, hp as i16 <= best as i16)
            }
            Criterion::Nearest => {
                let d = manhattan(b, o, p);
                (d, d < best)
            }
            Criterion::Farthest => {
                let d = manhattan(b, o, p);
                (d, d >= best)
            }
        };
        if better {
            best = value;
            found = Some(p);
        }
    }
    o.set_u16(b, f::TARGET, found.map_or(0, Obj::word));
    found
}

/// Tables des critères tirés au hasard (16 octets : 10 fois le plus proche, 4 le moins de PV, 2 le plus éloigné).
const CRITERIA_STRICT: u32 = 0x426a;
const CRITERIA_LENIENT: u32 = 0x439e;

/// 0x424e (règle stricte) / 0x43ae (règle large) : critère tiré au hasard, puis `target_by`.
pub fn pick_target(b: &mut Cps2Bus, o: Obj, rule: Rule) -> Option<Obj> {
    let table = match rule {
        Rule::Strict => CRITERIA_STRICT,
        Rule::Lenient => CRITERIA_LENIENT,
    };
    let roll = rng::random(b) & 15;
    let criterion = Criterion::from_byte(prog8(b, table + roll as u32));
    target_by(b, o, criterion, rule)
}

/// 0x44ee : un joueur ciblable (règle stricte) tiré au hasard devient la cible : trois essais, chacun sur le joueur
/// qui suit le précédent. Sans joueur en vie, ni tirage ni écriture.
pub fn random_player(b: &mut Cps2Bus, o: Obj) -> Option<Obj> {
    if JOUEURS_EN_VIE.get(b) == 0 {
        return None;
    }
    let mut k = rng::random(b);
    for _ in 0..3 {
        k = k.wrapping_add(1) & 3;
        let p = JOUEURS.nth(k as u32);
        if targetable(b, p, Rule::Strict) {
            o.set_u16(b, f::TARGET, p.word());
            return Some(p);
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Valeurs initiales : PV (0x453c, 0x4544), immunités (0xfe5a), rang de PV (0x1059e)

/// Tables des PV initiaux : boss (0x453c) et ennemis (0x4544). Mots, par type (256 octets), niveau, joueurs, rang.
pub const HP_TABLE_BOSS: u32 = 0x458c;
pub const HP_TABLE_ENEMY: u32 = 0x5c8c;

/// 0x453c / 0x4544 : PV initiaux de `o` lus dans `table` (bloc du type `+2` ; seconde moitié au-delà du niveau 5 ;
/// ligne du nombre de joueurs en vie ; colonne du rang / 2), copiés en `+0x60`, `+0x62` et `+0x64`.
pub fn initial_hp(b: &mut Cps2Bus, o: Obj, table: u32) {
    let mut index = (o.u8(b, Obj::KIND) as u16) << 8;
    if NIVEAU.get(b) > 5 {
        index = index.wrapping_add(0x80);
    }
    index = index.wrapping_add(((JOUEURS_EN_VIE.get(b).wrapping_sub(1) & 3) as u16) << 5);
    index = index.wrapping_add((RANG.get(b) >> 1 & 15) * 2);
    let hp = rd16(b, table.wrapping_add(index as i16 as u32));
    for field in [f::HP_MAX, f::HP, f::HP_PREV] {
        o.set_u16(b, field, hp);
    }
}

/// Immunités par type (longs) : ennemis (classe 2) puis boss (classe 4, 32 types plus loin).
const IMMUNITY_TABLE: u32 = 0xfe78;

/// 0xfe5a : immunités de `o` (`+0x94`) selon sa classe et son type. Rend l'index (octets) lu dans la table.
pub fn set_immunities(b: &mut Cps2Bus, o: Obj) -> u16 {
    let base: u8 = if o.u8(b, Obj::CLASS) == 4 { 32 } else { 0 };
    let index = base.wrapping_add(o.u8(b, Obj::KIND)) as u16 * 4;
    let v = prog32(b, IMMUNITY_TABLE + index as u32);
    o.set_u32(b, f::IMMUNITIES, v);
    index
}

/// 0x1059e : rang de PV de `o` (`+0xac`) = (joueurs en vie - 1) × 32 + (rang & 0x1e), sur un octet.
pub fn hp_rank(b: &mut Cps2Bus, o: Obj) {
    let players = JOUEURS_EN_VIE.get(b).wrapping_sub(1) << 5;
    let v = players.wrapping_add(RANG.get(b) as u8 & 0x1e);
    o.set_u8(b, f::HP_RANK, v);
}

// ---------------------------------------------------------------------------------------------
// Décisions par tables (0xa4b4, 0xa4c2, 0xac3a)

/// Tables de lignes de masques (longs) des deux décisions.
const DECISION_TABLE_A: u32 = 0xa4f2;
const DECISION_TABLE_B: u32 = 0xa516;

/// Décision par masque : la ligne `+0x72` (`decision_a`, 0xa4b4) ou `+0x73` (`decision_b`, 0xa4c2) donne une
/// table de masques de 32 bits indexée par le rang ; un bit y est tiré au hasard. Vrai s'il est levé.
fn decision(b: &mut Cps2Bus, o: Obj, field: u32, table: u32) -> bool {
    let line = (o.u8(b, field) as u16) << 2;
    let masks = rd32(b, table.wrapping_add(line as i16 as u32));
    let rang = RANG.get(b);
    let mask = rd32(b, masks.wrapping_add((rang << 2) as i16 as u32));
    let bit = rng::random(b) & 31;
    mask >> bit & 1 != 0
}

/// 0xa4b4 : l'IA agit-elle à cette image (ligne `+0x72`) ?
pub fn decision_a(b: &mut Cps2Bus, o: Obj) -> bool {
    decision(b, o, f::DECISION_A, DECISION_TABLE_A)
}

/// 0xa4c2 : idem avec la ligne `+0x73`.
pub fn decision_b(b: &mut Cps2Bus, o: Obj) -> bool {
    decision(b, o, f::DECISION_B, DECISION_TABLE_B)
}

/// Tables d'octets par rang de 0xac3a.
const BYTE_TABLES: u32 = 0xac58;

/// 0xac3a : `+0x77` = octet de la table de ligne `+0x76`, à l'index du rang.
pub fn pick_byte(b: &mut Cps2Bus, o: Obj) {
    let line = (o.u8(b, f::BYTE_TABLE) as u16) << 2;
    let table = rd32(b, BYTE_TABLES.wrapping_add(line as i16 as u32));
    let rang = RANG.get(b);
    let v = rd8(b, table.wrapping_add(rang as i16 as u32));
    o.set_u8(b, f::BYTE, v);
}

// ---------------------------------------------------------------------------------------------
// Position de la cible (0xb5e0, 0x1050c, 0x1052e, 0x104e0, 0x1047a, 0x10560)

/// La cible de `o` (`+0x50`), si elle est désignée par un mot négatif (un objet de la RAM haute).
fn linked_target(b: &mut Cps2Bus, o: Obj) -> Option<Obj> {
    let w = o.u16(b, f::TARGET);
    ((w as i16) < 0).then(|| Obj::from_word(w))
}

/// Orientation qui regarde de `o` vers `other` : 0x20 (gauche) si `other` est à gauche (`o.x - other.x` > 0
/// est faux pour un écart négatif : l'original teste le signe de la soustraction).
fn side_of(b: &mut Cps2Bus, o: Obj, other: Obj) -> u8 {
    if (o.u16(b, Obj::X).wrapping_sub(other.u16(b, Obj::X)) as i16) < 0 {
        0x20
    } else {
        0
    }
}

/// 0xb5e0 : la cible de `o` (`+0x50`, au sol) est-elle dans la boîte `boite` devant `o` (0x7f19c sur la cible) ?
pub fn target_in_front(b: &mut Cps2Bus, o: Obj, boite: crate::ai_aids::Boite) -> crate::ai_aids::DansBoite {
    match o.u16(b, f::TARGET) {
        0 => crate::ai_aids::DansBoite::Non,
        w => crate::ai_aids::box_around(b, o, Obj::from_word(w), boite),
    }
}

/// 0x1050c : `o` regarde-t-il vers sa cible ? Vrai aussi sans cible (mot positif).
pub fn facing_target(b: &mut Cps2Bus, o: Obj) -> bool {
    let Some(t) = linked_target(b, o) else { return true };
    side_of(b, o, t) == o.u8(b, f::FLIP)
}

/// 0x1052e : la cible regarde-t-elle vers `o` ?
pub fn target_faces_me(b: &mut Cps2Bus, o: Obj) -> bool {
    let Some(t) = linked_target(b, o) else { return false };
    let gap = o.u16(b, Obj::X).wrapping_sub(t.u16(b, Obj::X)) & 0x8000;
    let flip = (o.u8(b, f::FLIP) as u16).rotate_right(6);
    let wanted = if (flip & gap) as i16 >= 0 { 0x20 } else { 0 };
    wanted == t.u8(b, f::FLIP)
}

/// 0x104e0 (Z clair) : la cible regarde du même côté que `o`, qui lui tourne le dos.
pub fn back_to_target(b: &mut Cps2Bus, o: Obj) -> bool {
    let Some(t) = linked_target(b, o) else { return false };
    let flip = o.u8(b, f::FLIP);
    if flip != t.u8(b, f::FLIP) {
        return false;
    }
    let wanted = if side_of(b, o, t) == 0x20 { 0 } else { 0x20 };
    wanted != flip
}

/// Geste choisi par `turn_decision` (D0 de 0x1047a).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Turn {
    None,
    /// Face à face, décision A acceptée.
    FaceToFace,
    /// Même côté, décision B acceptée.
    SameSide,
}

/// 0x1047a : décision de l'IA selon le côté de la cible.
pub fn turn_decision(b: &mut Cps2Bus, o: Obj) -> Turn {
    if target_faces_me(b, o) {
        if decision_a(b, o) {
            Turn::FaceToFace
        } else {
            Turn::None
        }
    } else if back_to_target(b, o) && decision_b(b, o) {
        Turn::SameSide
    } else {
        Turn::None
    }
}

/// 0x10560 : le sens de la vitesse en X (`+0x14`) est-il celui de l'orientation ?
pub fn moving_forward(b: &mut Cps2Bus, o: Obj) -> bool {
    let flip = (o.u8(b, f::FLIP) as u16).rotate_right(6);
    o.u16(b, Obj::VX) & 0x8000 == flip
}

// ---------------------------------------------------------------------------------------------
// Rester dans l'écran (0xfbae, 0xfbb0)

/// Marge de 0xfbae (le test de zone de fin d'image des gestionnaires).
pub const ZONE_MARGIN: u16 = 0x30;

/// Ramène un axe (`field`) de `o` dans la fenêtre `[origin - margin, origin + span + margin]` : une position
/// négative devient 0, une position hors de la fenêtre est posée au bord le plus proche. Vrai si elle a changé.
fn pull_inside(b: &mut Cps2Bus, o: Obj, field: u32, origin: u16, span: u16, margin: u16) -> bool {
    let pos = o.u16(b, field);
    if (pos as i16) < 0 {
        o.set_u16(b, field, 0);
        return true;
    }
    let offset = pos.wrapping_sub(origin).wrapping_add(margin);
    if offset <= span.wrapping_add(margin).wrapping_add(margin) {
        return false;
    }
    let edge = if (offset as i16) < 0 { margin.wrapping_neg() } else { span.wrapping_add(margin) };
    o.set_u16(b, field, origin.wrapping_add(edge));
    true
}

/// 0xfbb0 : garde `o` dans l'écran de la caméra en service, à `margin` pixels près en X (16 pixels de moins que la
/// hauteur de l'écran en profondeur). Vrai s'il a été déplacé.
pub fn keep_on_screen(b: &mut Cps2Bus, o: Obj, margin: u16) -> bool {
    let (x, y) = ram::camera(b);
    let moved_x = pull_inside(b, o, Obj::X, x, 0x180, margin);
    let moved_y = pull_inside(b, o, Obj::Y, y, 0x100, (-16i16) as u16);
    moved_x | moved_y
}

/// 0xfbae : `keep_on_screen` avec la marge de 0x30 pixels.
pub fn keep_in_zone(b: &mut Cps2Bus, o: Obj) -> bool {
    keep_on_screen(b, o, ZONE_MARGIN)
}

// ---------------------------------------------------------------------------------------------
// Cap (0xf0e8, 0x10574)

/// 0xf0e8 : tourne d'un cran le cap de `o` (`+0x2d`, 0..31) vers le point `vers` (direction de 0x137e), par le
/// plus court.
pub fn turn_toward(b: &mut Cps2Bus, o: Obj, vers: (u16, u16)) {
    let wanted = crate::aides::direction_from(b, o, vers);
    let heading = o.u8(b, f::HEADING);
    let diff = wanted.wrapping_sub(heading) & 0x1f;
    if diff == 0 {
        return;
    }
    let turned = if diff & 0x10 == 0 { heading.wrapping_add(1) } else { heading.wrapping_sub(1) };
    o.set_u8(b, f::HEADING, turned & 0x1f);
}

/// Pas de cap tirés au hasard (octets signés) de 0x10574.
const DRIFT_STEPS: u32 = 0x1058e;

/// 0x10574 : le cap de `o` change d'au plus un cran, tiré au hasard.
pub fn drift_heading(b: &mut Cps2Bus, o: Obj) {
    let roll = rng::random(b) & 15;
    let step = prog8(b, DRIFT_STEPS + roll as u32);
    let heading = o.u8(b, f::HEADING).wrapping_add(step) & 0x1f;
    o.set_u8(b, f::HEADING, heading);
}
