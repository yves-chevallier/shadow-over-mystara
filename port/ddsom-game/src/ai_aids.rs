//! Aides d'IA des ennemis et des boss (suite de `ai`) : tirages d'attitude par tables, test de la
//! boîte devant un adversaire.
//!
//! Ces routines gardent le contrat des originaux : un résultat dans D0 (1 : oui, 0 : non) et les
//! drapeaux d'un `moveq`, que les handlers testent juste après l'appel (`beq`).

use cps2::Cps2Bus;
use m68k::decode::{Fetch, Size};
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::ops::logique;
use crate::objects::Obj;
use crate::rng;
use crate::objects::pools;

/// Compteur de l'image courante, qui sert d'index dans les tables d'attitude.
const TICK: u32 = A5 + 0x7f92;
/// État du générateur pseudo-aléatoire.

/// Champs de l'ennemi (A0).
mod f {
    pub const X: u32 = 0x08;
    pub const DEPTH: u32 = 0x10;
    pub const FACING: u32 = 0x2c;
    /// Objet visé (pointeur de 16 bits).
    pub const TARGET: u32 = 0x50;
    pub const STATE: u32 = 0x05;
    pub const SUB_STATE: u32 = 0x06;
    pub const SUB_STEP: u32 = 0x07;
    /// Numéros de tables : attitude (0x70), temporisation (0x72, 0x73), boîte d'attaque (0x74, 0x75).
    pub const STANCE_TABLE: u32 = 0x70;
    pub const STANCE_VALUE: u32 = 0x71;
    pub const BOX_CHANCE: u32 = 0x74;
    pub const BOX_ANIMATION: u32 = 0x75;
}

/// Tables de pointeurs en ROM (un long par numéro de table).
const STANCE_POINTERS: u32 = 0xa48c;
const BOX_CHANCE_POINTERS: u32 = 0x7fec;
const BOX_ANIMATION_POINTERS: u32 = 0x800c;

/// `moveq #v,D0` : D0 et les drapeaux N, Z (V et C à zéro).
fn moveq(c: &mut Cpu, v: i32) {
    c.d[0] = v as u32;
    logique(c, v as u32, Size::Long);
}

/// Mot de la table de pointeurs `table` désigné par `index`, étendu en signe comme le fait un index
/// `Dn.w` du 68000.
fn offset(index: u16) -> u32 {
    index as i16 as i32 as u32
}


/// 0xa470 : tire l'attitude de l'ennemi `o` (octet de la table `+0x70`, indexée par le compteur d'images), la
/// range en `+0x71` et la rend (le `beq` des appelants teste sa nullité).
pub fn stance(b: &mut Cps2Bus, o: Obj) -> u8 {
    stance_at(b, o).1
}

/// (table lue, attitude).
fn stance_at(b: &mut Cps2Bus, o: Obj) -> (u32, u8) {
    let number = rd8(b, o.0 + f::STANCE_TABLE);
    let table = crate::mem::rd32(b, STANCE_POINTERS + 4 * number as u32);
    let tick = rd16(b, TICK);
    let v = rd8(b, table.wrapping_add(offset(tick)));
    wr8(b, o.0 + f::STANCE_VALUE, v);
    (table, v)
}


/// Remplace l'octet bas / le mot bas d'un registre (les autres bits restent).
fn set_b(r: &mut u32, v: u8) {
    *r = *r & !0xff | v as u32;
}
fn set_w(r: &mut u32, v: u16) {
    *r = *r & !0xffff | v as u32;
}


/// Boîte devant un objet : décalage en x (vers l'avant), demi-largeur, demi-profondeur (D0, D1, D2 de l'original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Boite {
    pub decalage: u16,
    pub demi_largeur: u16,
    pub demi_profondeur: u16,
}

/// Registres de travail que laissent les tests de boîte de l'original (mots bas, s'ils ont été posés).
#[derive(Default)]
struct Restes {
    d0: Option<u16>,
    d1: Option<u16>,
    d2: Option<u16>,
    d3: Option<u16>,
    d2_octet: Option<u8>,
    d3_octet: Option<u8>,
    d4_octet: Option<u8>,
}

impl Restes {
    fn poser(&self, c: &mut Cpu) {
        // dans l'ordre de l'original : octets puis mots, chacun à sa place
        if let Some(v) = self.d3_octet {
            set_b(&mut c.d[3], v);
        }
        if let Some(v) = self.d4_octet {
            set_b(&mut c.d[4], v);
        }
        for (r, v) in [(0, self.d0), (1, self.d1), (2, self.d2), (3, self.d3)] {
            if let Some(v) = v {
                set_w(&mut c.d[r], v);
            }
        }
        if let Some(v) = self.d2_octet {
            set_b(&mut c.d[2], v);
        }
    }
}

/// L'objet `other` est-il dans la boîte devant `me` ? Écarts biaisés comparés en non signé, comme l'original.
fn dans_la_boite(b: &mut Cps2Bus, me: u32, other: u32, boite: Boite, r: &mut Restes) -> bool {
    let facing = rd8(b, me + f::FACING);
    let offset_x = if facing != 0 { boite.decalage.wrapping_neg() } else { boite.decalage };
    let other_x = rd16(b, other + f::X);
    r.d3 = Some(other_x);
    let half_x = boite.demi_largeur;
    let reach_x = offset_x.wrapping_add(rd16(b, me + f::X)).wrapping_sub(other_x).wrapping_add(half_x);
    r.d0 = Some(reach_x);
    r.d1 = Some(half_x.wrapping_mul(2));
    if reach_x > half_x.wrapping_mul(2) {
        return false;
    }
    let other_depth = rd16(b, other + f::DEPTH);
    r.d1 = Some(other_depth);
    let half_depth = boite.demi_profondeur;
    let reach_depth = rd16(b, me + f::DEPTH).wrapping_sub(other_depth).wrapping_add(half_depth);
    r.d0 = Some(reach_depth);
    r.d2 = Some(half_depth.wrapping_mul(2));
    reach_depth <= half_depth.wrapping_mul(2)
}

/// 0x7f0c (`strict`) et 0x7ef4 : un adversaire (l'objet visé `+0x50`) est-il dans la boîte devant l'ennemi `o`, de
/// face, au sol et dans l'état 12 (ou 14 si `strict`) ? Si oui, après un tirage de probabilité (table `+0x74`),
/// l'ennemi passe à l'état 6 avec une animation tirée au hasard (table `+0x75`) et le résultat est vrai.
pub fn in_front(b: &mut Cps2Bus, o: Obj, strict: bool, boite: Boite) -> bool {
    in_front_restes(b, o.0, strict, boite).0
}

/// (résultat, restes, adversaire lu (A1), table lue (A4), D2 et D0 longs du tirage).
fn in_front_restes(b: &mut Cps2Bus, me: u32, strict: bool, boite: Boite) -> (bool, Restes, Option<u32>, Option<u32>, Option<(u32, u32)>) {
    let mut r = Restes::default();
    let target = rd16(b, me + f::TARGET);
    if target == 0 {
        return (false, r, None, None, None);
    }
    let other = object(target);
    let state = rd8(b, other + f::SUB_STATE);
    let refuse = if strict { rd16(b, other + 0x0c) != 0 || (state != 12 && state != 14) } else { state != 12 };
    if refuse {
        return (false, r, Some(other), None, None);
    }
    let facing = rd8(b, me + f::FACING);
    r.d3_octet = Some(facing);
    if facing == rd8(b, other + f::FACING) {
        return (false, r, Some(other), None, None);
    }
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    r.d3 = Some(gap);
    let side = if (gap as i16) >= 0 { 0 } else { 32 };
    r.d4_octet = Some(side);
    if side != facing || !dans_la_boite(b, me, other, boite, &mut r) {
        return (false, r, Some(other), None, None);
    }
    let number = rd8(b, me + f::BOX_CHANCE);
    let chances = crate::mem::rd32(b, BOX_CHANCE_POINTERS + 4 * number as u32);
    let tick = rd16(b, TICK);
    let masque = rd32(b, chances.wrapping_add(offset(tick << 2)));
    let roll = rng::random(b) as u32 & 31;
    if masque >> roll & 1 == 0 {
        return (false, r, Some(other), Some(chances), Some((masque, roll)));
    }
    let number = rd8(b, me + f::BOX_ANIMATION);
    // l'octet `+0x75` est déjà un multiple de 4 : pas de décalage
    let animations = rd32(b, BOX_ANIMATION_POINTERS.wrapping_add(number as u32)).wrapping_add(offset((tick >> 1) << 5));
    let pick = rng::random(b) as u32 & 31;
    wr8(b, me + f::STATE, 6);
    let v = rd8(b, animations.wrapping_add(pick));
    wr8(b, me + f::SUB_STATE, v);
    wr8(b, me + f::SUB_STEP, 0);
    (true, r, Some(other), Some(animations), Some((masque, roll)))
}

/// Position de l'objet par rapport à la boîte devant un autre (0x7f19c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DansBoite {
    /// Hors de la boîte, ou en l'air (D0 = 0).
    Non,
    /// Dans la boîte, de face (D0 = 1).
    Face,
    /// Dans la boîte, de dos (D0 = 2).
    Dos,
}

/// 0x7f19c : l'objet `other` est-il dans la boîte devant l'objet `me` ?
pub fn box_around(b: &mut Cps2Bus, me: Obj, other: Obj, boite: Boite) -> DansBoite {
    box_reach(b, me.0, other.0, boite, &mut Restes::default())
}

fn box_reach(b: &mut Cps2Bus, me: u32, other: u32, boite: Boite, r: &mut Restes) -> DansBoite {
    if rd16(b, other + 0x0c) != 0 || !dans_la_boite(b, me, other, boite, r) {
        return DansBoite::Non;
    }
    let facing = rd8(b, me + f::FACING);
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    r.d1 = Some(gap);
    let side = if (gap as i16) < 0 { 32 } else { 0 };
    r.d2_octet = Some(side);
    if side == facing {
        DansBoite::Face
    } else {
        DansBoite::Dos
    }
}

/// 0xb5e0 (`ai::target_in_front`) avec les registres de l'original : A1 = la cible, D0 = 0/1/2 et drapeaux de
/// `moveq`, D1-D3 laissés par les tests de la boîte. Pour les appelants dont la suite relit ces restes (boss 11 :
/// D2, glitch d'origine).
pub fn target_in_front_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, boite: Boite) -> DansBoite {
    let pointer = rd16(b, o.0 + f::TARGET);
    let mut restes = Restes::default();
    let found = if pointer == 0 {
        DansBoite::Non
    } else {
        c.a[1] = object(pointer);
        box_reach(b, o.0, object(pointer), boite, &mut restes)
    };
    restes.poser(c);
    moveq(c, found as i32);
    found
}

/// 0x7f19c (`box_around`) avec les registres de l'original : D0 = 0/1/2 et drapeaux de `moveq`, D1-D3 laissés par
/// les tests de la boîte.
pub fn box_around_68k(c: &mut Cpu, b: &mut Cps2Bus, me: Obj, other: Obj, boite: Boite) -> DansBoite {
    let mut restes = Restes::default();
    let found = box_reach(b, me.0, other.0, boite, &mut restes);
    restes.poser(c);
    moveq(c, found as i32);
    found
}

/// 0x7f0c / 0x7ef4 (`in_front`) avec les registres de l'original : D0 (1 / 0) et drapeaux de `moveq`, D1-D4, A1
/// et A4 laissés comme l'original.
pub fn in_front_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj, strict: bool, boite: Boite) -> bool {
    let (oui, restes, autre, table, tirage) = in_front_restes(b, o.0, strict, boite);
    if let Some(autre) = autre {
        c.a[1] = autre;
    }
    restes.poser(c);
    if let Some(table) = table {
        c.a[4] = table;
    }
    if let Some((masque, roll)) = tirage {
        c.d[2] = masque;
        c.d[0] = roll;
    }
    moveq(c, oui as i32);
    oui
}

/// Profils de poussée : table d'octets (indexée par l'écart en x / 16) en espace programme, puis
/// entrées de 8 octets (hauteur maximale, écart en profondeur maximal, poussée) en espace de données.
const PUSH_PROFILE_INDEX: u32 = 0x1168e;
const PUSH_PROFILES: u32 = 0x116ae;

/// Octet de l'espace programme (table adressée relativement au PC).
fn prog8(b: &mut Cps2Bus, a: u32) -> u8 {
    let word = b.fetch16(a & !1);
    if a & 1 == 0 {
        (word >> 8) as u8
    } else {
        word as u8
    }
}

/// Poussée (16.16) que le boss `me` applique au joueur `player` pour un écart en x `gap` : 0 si le
/// joueur est trop haut ou trop loin en profondeur.
fn push_amount_for(b: &mut Cps2Bus, me: u32, player: u32, gap: u16) -> u32 {
    if gap as i16 >= 512 {
        return 0;
    }
    let profile = prog8(b, PUSH_PROFILE_INDEX.wrapping_add(offset(gap >> 4))) as u16;
    let entry = PUSH_PROFILES.wrapping_add(offset(profile << 3));
    let max_height = rd16(b, entry) as i16;
    let max_depth = rd16(b, entry + 2) as i16;
    let depth = (rd16(b, player + f::DEPTH).wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs();
    if max_height >= rd16(b, player + 0x0c) as i16 && depth < max_depth {
        rd32(b, entry + 4)
    } else {
        0
    }
}

/// Le boss `me` pousse le joueur `player` en x, du côté vers lequel il regarde ; vrai si poussé.
fn push_player_once(b: &mut Cps2Bus, me: u32, player: u32) -> bool {
    let gap = (rd16(b, player + f::X).wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let push = push_amount_for(b, me, player, gap);
    let facing = rd8(b, me + f::FACING);
    let side = if (rd16(b, me + f::X).wrapping_sub(rd16(b, player + f::X)) as i16) < 0 { 32 } else { 0 };
    if push == 0 || side != facing {
        return false;
    }
    let push = if facing == 0 { push.wrapping_neg() } else { push };
    let x = rd32(b, player + f::X).wrapping_add(push);
    wr32(b, player + f::X, x);
    true
}

// ---------------------------------------------------------------------------------------------
// Animations par table et tests d'orientation (0x103e0-0x105d0). Les fonctions « corps » ne font pas
// le RTS : `leaf!` l'ajoute pour la native.
// ---------------------------------------------------------------------------------------------

const ANIM_MARK: u32 = 0x3e;

/// Adresse de l'objet désigné par un pointeur de 16 bits (étendu en signe).
fn object(pointer: u16) -> u32 {
    pointer as i16 as i32 as u32
}

// ---------------------------------------------------------------------------------------------
// Attaques sur les joueurs (0xfc44-0xfda0)
// ---------------------------------------------------------------------------------------------

/// Boîtes d'attaque des ennemis (entrées de 8 octets : décalage et demi-étendue en x, z, y) et boîtes de
/// corps des joueurs (une table par personnage, entrées de 8 octets), en espace de données.
const ATTACK_BOXES: u32 = 0x123602;
const BODY_BOX_TABLES: u32 = 0xfd4c;

/// Octet signé de l'espace de données.
fn rd8s(b: &mut Cps2Bus, a: u32) -> i16 {
    rd8(b, a) as i8 as i16
}

/// Le joueur à `player` est-il touché par la boîte d'attaque `attack` (adresse de l'entrée) de l'objet `me` ?
/// Le joueur doit être présent, visible, non protégé, ni à terre ni mort, et avoir une boîte de corps.
fn player_hit(b: &mut Cps2Bus, me: u32, attack: u32, player: u32) -> bool {
    let flags = rd8(b, player);
    if flags & 1 == 0 || flags & 2 != 0 || rd8(b, player + 0x188) != 0 {
        return false;
    }
    if rd16(b, player + 4) == 0x204 || rd8(b, player + 4) == 4 {
        return false;
    }
    let body = rd8(b, player + 0x38);
    if body == 0 || rd8(b, player + 0x7e) != 0 || rd16(b, player + 0x196) != 0 {
        return false;
    }
    let character = rd8(b, player + 3) & 14;
    let table = rd32(b, BODY_BOX_TABLES + 2 * character as u32);
    let mut mine = attack;
    let mut theirs = table.wrapping_add(8 * body as u32);
    // x, puis z, puis y : (décalage, étendue) de chaque boîte ; seul x tient compte du sens regardé
    for (axis, mirror) in [(0x08, true), (0x0c, false), (0x10, false)] {
        let (mut d0, mut d1) = (rd8s(b, mine), rd8s(b, theirs));
        if mirror {
            if rd8(b, me + f::FACING) != 0 {
                d0 = -d0;
            }
            if rd8(b, player + f::FACING) != 0 {
                d1 = -d1;
            }
        }
        let gap = d0.wrapping_add(rd16(b, me + axis) as i16).wrapping_sub(d1.wrapping_add(rd16(b, player + axis) as i16));
        let reach = rd8(b, mine + 1) as i16 + rd8(b, theirs + 1) as i16;
        mine += 2;
        theirs += 2;
        if reach < gap.wrapping_abs() {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------------------------
// Choix d'un point où marcher autour de la cible (0xe886, 0xeff2, 0xf084)
// ---------------------------------------------------------------------------------------------

/// Tables de points candidats par numéro (`+0x79`) : pointeur, puis nombre de points - 1 et paires
/// (décalage x, décalage profondeur) autour de la cible.
pub(crate) const WALK_POINTS: u32 = 0xe93e;
const MIRRORED_WALK_POINTS: u32 = 0xf06a;

/// Écart « en ville » entre deux points : |dx| + |dy| (mots).
fn manhattan(b: &mut Cps2Bus, me: u32, x: u16, depth: u16) -> u16 {
    let dx = (x.wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let dy = (depth.wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs() as u16;
    dx.wrapping_add(dy)
}

// ---------------------------------------------------------------------------------------------
// Divers : tests d'écran, cap aléatoire, prise (0x104a2-0x105d0, 0xfda2, 0xff54)
// ---------------------------------------------------------------------------------------------

/// Origine de l'écran en x (mot de A5), selon la caméra utilisée (`A5+0x36`).
fn screen_origin(b: &mut Cps2Bus) -> u16 {
    let offset = if rd8(b, A5 + 0x36) == 0 { 0x5a6 } else { 0x5ae };
    rd16(b, A5 + offset)
}

/// Table de déplacement des prises (octets signés : décalage x, décalage de hauteur, orientation, sens).
const SEIZE_OFFSETS: u32 = 0x123652;

/// 0xfe34 : l'objet A0 est ramené dans l'écran de la caméra. Rend vrai s'il l'a été.
fn keep_inside_camera(b: &mut Cps2Bus, me: u32) -> bool {
    let left = if rd8(b, A5 + 0x36) == 0 { rd16(b, A5 + 0x506) } else { rd16(b, A5 + 0x556) };
    let x = rd16(b, me + f::X) as i16;
    if left as i16 > x {
        wr16(b, me + f::X, left);
        return true;
    }
    let right = left.wrapping_add(384);
    if right as i16 >= x {
        return false;
    }
    wr16(b, me + f::X, right);
    true
}

// ---------------------------------------------------------------------------------------------
// Choix d'une cible selon un critère imposé (0x43bc)
// ---------------------------------------------------------------------------------------------


// ---------------------------------------------------------------------------------------------
// Tirages par tables : attitude, riposte, esquive (0x788c, 0x79ce, 0x79f6, 0xb048, 0xb068, 0xf118,
// 0x10112). Une table de pointeurs (longs) est indexée par un octet de l'objet ; la ligne choisie est
// lue au hasard (octets) ou par le compteur d'images (masques de 32 bits, un bit tiré au hasard).
// ---------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------
// Débris et butin (0x11a36, 0x1172e, 0xb648, 0xb780, 0xbad6, 0xe2f6)
// ---------------------------------------------------------------------------------------------

/// Alloue par `alloc` (0x2318, 0x23d8) : l'objet, ou `None` si le pool est plein.
fn alloc(b: &mut Cps2Bus, pool: &pools::Pool) -> Option<u32> {
    pool.alloc(b).map(|o| o.0)
}

/// Copie la position de `from` dans `to`, posée 16 pixels plus haut et juste devant.
fn place_above(b: &mut Cps2Bus, from: u32, to: u32) {
    for field in [0x08, 0x0c, 0x10] {
        let v = rd16(b, from + field);
        wr16(b, to + field, v);
    }
    let z = rd16(b, to + 0x0c).wrapping_add(0x10);
    wr16(b, to + 0x0c, z);
    let depth = rd16(b, to + 0x10).wrapping_sub(1);
    wr16(b, to + 0x10, depth);
}

/// Débris (EFFCT de type 0x32, variante 0xc) : `+0x58`/`+0x18` puis vitesses lus dans la table A2,
/// décalage et vitesse en x retournés si l'objet regarde à gauche. Une ligne n'est consommée que si
/// l'effet a pu être créé.
fn debris(b: &mut Cps2Bus, fx: u32, me: u32) {
    wr8(b, fx, 1);
    wr8(b, fx + 2, 0x32);
    wr8(b, fx + 3, 0xc);
    let x = rd16(b, me + 0x08);
    wr16(b, fx + 0x08, x);
}

/// Butin lancé en l'air (ITEM de genre `kind`, variante `variant`) : vitesses (±0x140, -0x28 en z...),
/// retournées si `mirrored`.
fn tossed_item(b: &mut Cps2Bus, me: u32, item: u32, kind: u8, variant: u8, vy: u16, mirrored: bool) {
    wr8(b, item, 1);
    wr8(b, item + 2, kind);
    wr8(b, item + 3, variant);
    place_above(b, me, item);
    wr16(b, item + 0x18, 0x300);
    wr16(b, item + 0x1a, (-0x28i16) as u16);
    wr16(b, item + 0x14, 0x140);
    wr16(b, item + 0x1c, vy);
    if mirrored {
        for field in [0x14, 0x1c] {
            let v = rd16(b, item + field).wrapping_neg();
            wr16(b, item + field, v);
        }
    }
}

/// Nombre de tirages de butin : deux à plus de trois joueurs en vie, sinon un.
fn loot_rolls(b: &mut Cps2Bus) -> u16 {
    if rd8(b, A5 + 0x35) > 3 {
        2
    } else {
        1
    }
}

/// Objet spécial tiré par 0xbad6 : genre et variante.
const DRAWN_KIND: u32 = A5 - 0x7196;
const DRAWN_VARIANT: u32 = A5 - 0x7195;

/// Tables de 0xbad6 (lues comme données, A1 = `lea` de la table) : ligne `+0x80` (paires genre, variante), puis par difficulté (`A5+0x49`).
mod special {
    pub const ROWS: u32 = 0xbf26;
    pub const COMMON: u32 = 0xc4c6;
    pub const PAIRS: u32 = 0xca86;
    pub const KIND_2: u32 = 0xd606;
    pub const BY_CLASS: u32 = 0xd046;
    pub const CLERIC_OR_MAGE: u32 = 0xd776;
    pub const MAGES: u32 = 0xda56;
    pub const CLERICS: u32 = 0xdbc6;
    pub const MAGE_OR_CLERIC: u32 = 0xdd36;
}

/// Classes de personnage (`+3` / 2 d'un joueur) qui comptent pour le butin spécial.
const CLERIC: u8 = 1;
const ELF: u8 = 2;
const MAGE: u8 = 4;

/// Étapes du tirage de 0xbad6 (les étiquettes de l'original).
#[derive(Clone, Copy)]
enum Draw {
    /// 0xbaf2 : ligne de l'objet.
    Row,
    /// 0xbb20 : objet commun, ou de genre 7 si un clerc, un elfe ou un magicien est en jeu.
    Common,
    /// 0xbba8 : paire (genre, variante) tirée.
    Pair,
    /// 0xbbda : genre 2.
    Kind2,
    /// 0xbc02 : selon les classes en jeu.
    ByClass,
    /// 0xbcb4 : certains objets sont réservés à des classes ; sinon l'objet est lâché.
    Restrict,
    Spawn,
    /// 0xbf20 : tirage suivant.
    Next,
}

fn drawn(b: &mut Cps2Bus) -> (u8, u8) {
    (rd8(b, DRAWN_KIND), rd8(b, DRAWN_VARIANT))
}
fn set_drawn(b: &mut Cps2Bus, kind: u8, variant: u8) {
    wr8(b, DRAWN_KIND, kind);
    wr8(b, DRAWN_VARIANT, variant);
}
/// Nouvelle variante de genre 4 tirée dans `at` : 0 annule le tirage, 1 et 2 renvoient aux tirages `on_1`, `on_2`.
fn redraw(b: &mut Cps2Bus, at: u32, on_1: Draw, on_2: Draw) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    match variant {
        0 => Draw::Next,
        1 => on_1,
        2 => on_2,
        _ => Draw::Restrict,
    }
}

fn reroll_or_none(b: &mut Cps2Bus, at: u32) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    if variant == 0 { Draw::Next } else { Draw::Spawn }
}

/// Routines d'atterrissage par type (longs du code) : ennemis (classe 2) aux indices 0..31, boss ensuite.
const LANDING_BY_KIND: u32 = 0x3e49c;


// ---------------------------------------------------------------------------------------------
// Cœurs du lot IA : chaque routine d'origine est une fonction Rust sur l'objet traité, sans registre.
// ---------------------------------------------------------------------------------------------

/// Champs de l'ennemi propres à ces aides.
mod g {
    /// Clé d'animation de la variante (7 bits) et durée de l'entrée courante.
    pub const VARIANT_KEY: u32 = 0x33;
    pub const ANIM_TIMER: u32 = 0x30;
    /// Marque de l'entrée d'animation (type de coup), recopiée en `+0x44` après chaque pas.
    pub const ANIM_MARK: u32 = 0x3e;
    pub const ANIM_MARK_COPY: u32 = 0x44;
    /// Maître (pointeur de 16 bits) dont la variante ou la marque choisit l'animation.
    pub const MASTER: u32 = 0xb2;
    /// Point visé (x, profondeur), jeu de points et numéro du dernier point retenu.
    pub const GOAL_X: u32 = 0x7a;
    pub const GOAL_DEPTH: u32 = 0x7c;
    pub const POINT_SET: u32 = 0x79;
    pub const LAST_POINT: u32 = 0x99;
    /// Paliers de PV : palier courant, nombre de paliers, colonne (rang de PV).
    pub const LEVEL: u32 = 0xaa;
    pub const LEVELS: u32 = 0xab;
    /// Paramètre de comportement (et compte à rebours de 0xff54).
    pub const BEHAVIOUR: u32 = 0x98;
    /// Lignes des tables de tirage : `+0x6a` → `+0x6b`, `+0x6c` (masques) et `+0x6d` → `+0x6e`, `+0x8e` →
    /// `+0x85` (`+0x84` : hors de l'écran).
    pub const ROW_6A: u32 = 0x6a;
    pub const DRAW_6B: u32 = 0x6b;
    pub const ROW_6C: u32 = 0x6c;
    pub const ROW_6D: u32 = 0x6d;
    pub const DRAW_6E: u32 = 0x6e;
    pub const OFFSCREEN_84: u32 = 0x84;
    pub const DRAW_85: u32 = 0x85;
    pub const ROW_8E: u32 = 0x8e;
    /// Niveau de parade (0x10112).
    pub const PARRY_LEVEL: u32 = 0x68;
    /// Butin : ligne des tables (`+0x80`), masque et drapeau de l'objet lâché une seule fois.
    pub const LOOT_ROW: u32 = 0x80;
    pub const ONCE_MASK: u32 = 0x9c;
    pub const ONCE_DONE: u32 = 0x9d;
}

/// L'objet désigné par le mot `pointer` (étendu en signe, comme `movea.w`).
fn linked(pointer: u16) -> Obj {
    Obj(object(pointer))
}

// --- animations par table (0x103e0, 0x10428, 0x10450) ----------------------------------------------

/// Première entrée (16 octets) du jeu `set` dont l'octet `+3` vaut `key` sous `mask`.
fn find_animation(b: &mut Cps2Bus, set: u32, key: u8, mask: u8) -> u32 {
    let mut at = set;
    while rd8(b, at + 3) & mask != key & mask {
        at = at.wrapping_add(16);
    }
    at
}

/// Recopie de la marque d'animation (0x1046c).
fn copy_mark(b: &mut Cps2Bus, o: Obj) {
    let v = o.u8(b, g::ANIM_MARK);
    o.set_u8(b, g::ANIM_MARK_COPY, v);
}

/// 0x103e0 : démarre l'animation du jeu `set` de même clé que la variante courante (`+0x33`), en gardant la durée
/// de l'entrée courante.
pub fn animation_by_variant(b: &mut Cps2Bus, o: Obj, set: u32) {
    let timer = o.u16(b, g::ANIM_TIMER);
    let key = o.u8(b, g::VARIANT_KEY);
    let at = find_animation(b, set, key, 0x7f);
    crate::objects::anim::start_entry(b, o, at);
    o.set_u16(b, g::ANIM_TIMER, timer);
    copy_mark(b, o);
}

/// 0x10428 : démarre l'animation du jeu `set` de même clé que la variante du maître (`+0xb2`).
pub fn animation_of_master(b: &mut Cps2Bus, o: Obj, set: u32) {
    let master = linked(o.u16(b, g::MASTER));
    let key = master.u8(b, g::VARIANT_KEY);
    let at = find_animation(b, set, key, 0x7f);
    crate::objects::anim::start_entry(b, o, at);
    copy_mark(b, o);
}

/// 0x10450 : démarre l'animation du jeu `set` dont la marque (`+3`) est celle de l'entrée courante du maître.
pub fn animation_of_mark(b: &mut Cps2Bus, o: Obj, set: u32) {
    let master = linked(o.u16(b, g::MASTER));
    let key = master.u8(b, g::ANIM_MARK);
    let at = find_animation(b, set, key, 0xff);
    crate::objects::anim::start_entry(b, o, at);
}

// --- joueurs et paliers (0x10474, 0x104a2, 0x105b6) -----------------------------------------------

/// 0x10474 : au moins un joueur est en vie.
pub fn players_alive(b: &mut Cps2Bus) -> bool {
    crate::ram::JOUEURS_EN_VIE.get(b) != 0
}

/// 0x104a2 : `o` est-il hors de la bande `[écran + left, écran + right]` (bornes exclues de l'intérieur) ? L'écran
/// est l'origine de la caméra d'affichage (`A5+0x5a6`, ou `A5+0x5ae` en mode deux caméras).
pub fn outside_band(b: &mut Cps2Bus, o: Obj, left: u16, right: u16) -> bool {
    let origin = screen_origin(b);
    let x = o.u16(b, Obj::X) as i16;
    let (low, high) = (left.wrapping_add(origin) as i16, right.wrapping_add(origin) as i16);
    low >= x || high <= x
}

/// 0x105b6 : franchit le palier de PV suivant (`+0xaa` < `+0xab`) si son seuil dans `table` (ligne du palier,
/// colonne `+0xac`, mots) atteint les PV (`+0x62`). Vrai si un palier est passé.
pub fn next_threshold(b: &mut Cps2Bus, o: Obj, table: u32) -> bool {
    let level = o.u8(b, g::LEVEL) as i8;
    if level >= o.u8(b, g::LEVELS) as i8 {
        return false;
    }
    let column = o.u8(b, crate::ai::f::HP_RANK) as i8 as i16;
    let index = (level as i16).wrapping_shl(7).wrapping_add(column);
    let threshold = rd16(b, table.wrapping_add(offset(index as u16))) as i16;
    if threshold < o.u16(b, crate::ai::f::HP) as i16 {
        return false;
    }
    o.set_u8(b, g::LEVEL, level.wrapping_add(1) as u8);
    true
}

// --- points de marche autour de la cible (0xe76c, 0xe7fa, 0xe886, 0xe914, 0xeff2, 0xf084) ------------

/// Le point de marche retenu : le plus proche ou le plus éloigné de `o`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    Nearest,
    Farthest,
}

/// Retient, parmi les points de la table de `o` (`+0x79`) autour de sa cible, celui qui est le plus proche (à
/// égalité, le dernier) ou le plus éloigné (à égalité, le premier) de `o`, et le range en `+0x7a`/`+0x7c` (la
/// position de la cible si aucun). `skip` : numéro de point sauté sans lire sa paire (glitch d'origine : les
/// points suivants prennent la paire décalée) ; le point retenu est alors noté en `+0x99`. `mirror` : décalages en
/// x retournés si la cible regarde à gauche. Rend le point retenu.
fn pick_point(b: &mut Cps2Bus, o: Obj, table: u32, pick: Pick, skip: Option<u8>, mirror: bool) -> (u16, u16) {
    let target = linked(o.u16(b, crate::ai::f::TARGET));
    let set = o.u8(b, g::POINT_SET) as u32;
    let mut at = rd32(b, table + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    o.set_u16(b, g::GOAL_X, target_x);
    o.set_u16(b, g::GOAL_DEPTH, target_depth);
    let mut best: u16 = if pick == Pick::Farthest { 0 } else { 0x7fff };
    for point in (0..=last).rev() {
        if skip == Some(point as u8) {
            continue;
        }
        let mut dx = rd16(b, at);
        let dy = rd16(b, at + 2);
        at += 4;
        if mirror && target.u8(b, f::FACING) != 0 {
            dx = dx.wrapping_neg();
        }
        let (x, depth) = (dx.wrapping_add(target_x), dy.wrapping_add(target_depth));
        let distance = manhattan(b, o.0, x, depth);
        let better = match pick {
            Pick::Farthest => distance > best,
            Pick::Nearest => distance <= best,
        };
        if better {
            best = distance;
            o.set_u16(b, g::GOAL_X, x);
            o.set_u16(b, g::GOAL_DEPTH, depth);
            if skip.is_some() {
                o.set_u8(b, g::LAST_POINT, point as u8);
            }
        }
    }
    (o.u16(b, g::GOAL_X), o.u16(b, g::GOAL_DEPTH))
}

/// `o` est-il à plus de 384 pixels à droite du bord gauche de la caméra en service ?
fn far_from_camera(b: &mut Cps2Bus, o: Obj) -> bool {
    let (camera, _) = crate::ram::camera(b);
    o.u16(b, Obj::X).wrapping_sub(camera) > 0x180
}

/// 0xf084 : vise le point le plus éloigné autour de la cible et tourne d'un cran vers lui.
pub fn walk_far_point(b: &mut Cps2Bus, o: Obj) {
    let goal = pick_point(b, o, WALK_POINTS, Pick::Farthest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe76c : vise le point le plus proche autour de la cible (loin de la caméra : le plus éloigné, 0xf084) et
/// tourne d'un cran vers lui. Rien sans cible.
pub fn approach_target(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let goal = pick_point(b, o, WALK_POINTS, Pick::Nearest, None, false);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xeff2 : comme `approach_target` (sans le cas lointain), les décalages retournés selon le sens de la cible.
pub fn walk_near_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    let goal = pick_point(b, o, MIRRORED_WALK_POINTS, Pick::Nearest, None, true);
    crate::ai::turn_toward(b, o, goal);
}

/// 0xe886 : retient le point le plus éloigné autour de la cible, sauf le dernier retenu (`+0x99`) ; loin de la
/// caméra, vise tout de suite un point lointain (0xf084). Rien sans cible.
pub fn choose_walk_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    let last = o.u8(b, g::LAST_POINT);
    pick_point(b, o, WALK_POINTS, Pick::Farthest, Some(last), false);
}

/// 0xe914 : le point visé est-il atteint (à 8 pixels près sur chaque axe) ? Alors un autre est choisi (0xe886).
pub fn walk_point_reached(b: &mut Cps2Bus, o: Obj) {
    let near = |goal: u16, here: u16| goal.wrapping_sub(here).wrapping_add(8) <= 16;
    let (gx, x) = (o.u16(b, g::GOAL_X), o.u16(b, Obj::X));
    let (gy, y) = (o.u16(b, g::GOAL_DEPTH), o.u16(b, Obj::Y));
    if near(gx, x) && near(gy, y) {
        choose_walk_point(b, o);
    }
}

/// 0xe7fa : retient le point le plus proche autour de la cible et note son numéro (`+0x99`, 0xff si aucun) ; loin
/// de la caméra, vise un point lointain (0xf084). Rien sans cible.
pub fn choose_near_walk_point(b: &mut Cps2Bus, o: Obj) {
    if o.u16(b, crate::ai::f::TARGET) == 0 {
        return;
    }
    if far_from_camera(b, o) {
        return walk_far_point(b, o);
    }
    o.set_u8(b, g::LAST_POINT, 0xff);
    let target = linked(o.u16(b, crate::ai::f::TARGET));
    let set = o.u8(b, g::POINT_SET) as u32;
    let mut at = rd32(b, WALK_POINTS + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (target.u16(b, Obj::X), target.u16(b, Obj::Y));
    o.set_u16(b, g::GOAL_X, target_x);
    o.set_u16(b, g::GOAL_DEPTH, target_depth);
    let mut best: u16 = 0x7fff;
    for point in (0..=last).rev() {
        let (x, depth) = (rd16(b, at).wrapping_add(target_x), rd16(b, at + 2).wrapping_add(target_depth));
        at += 4;
        let distance = manhattan(b, o.0, x, depth);
        if distance <= best {
            best = distance;
            o.set_u16(b, g::GOAL_X, x);
            o.set_u16(b, g::GOAL_DEPTH, depth);
            o.set_u8(b, g::LAST_POINT, point as u8);
        }
    }
}

// --- prises (0xfc44, 0xfd64, 0xfd8c, 0xfda2) --------------------------------------------------------

/// 0xfc44 : un joueur est-il touché par la boîte d'attaque `attack` de `o` ? Rend le premier touché.
pub fn attack_hits_a_player(b: &mut Cps2Bus, o: Obj, attack: u8) -> Option<Obj> {
    let boxes = ATTACK_BOXES.wrapping_add(offset((attack as u16) << 3));
    crate::ram::JOUEURS.iter().find(|p| player_hit(b, o.0, boxes, p.0))
}

/// 0xfd64 : le joueur `player` est saisi par `o` (animation de prise `anim` de `o`).
pub fn seize_player(b: &mut Cps2Bus, o: Obj, player: Obj, anim: u8) {
    player.set_u32(b, Obj::STATE, 0x0204_3200);
    player.set_u8(b, Obj::ACTIVE, 2);
    player.set_u16(b, 0x186, o.word());
    player.set_u8(b, 0x188, 1);
    o.set_u16(b, crate::ai::f::TARGET, player.word());
    o.set_u32(b, Obj::STATE, 0x020c_0000);
    o.set_u8(b, f::SUB_STATE, anim);
}

/// 0xfd8c : le joueur `player` est relâché (action 0x34).
pub fn release_player(b: &mut Cps2Bus, player: Obj) {
    player.set_u32(b, Obj::STATE, 0x0204_0000);
    player.set_u8(b, f::SUB_STATE, 0x34);
    player.set_u8(b, 0x188, 0);
}

/// 0xfda2 : `o` est ramené dans l'écran, puis l'objet qu'il tient (`+0x50`) le suit selon l'étape (`+0x3e`) de
/// l'animation de prise (table des déplacements : x, hauteur, orientation, sens de `+0x12`).
pub fn hold_seized(b: &mut Cps2Bus, o: Obj) {
    let me = o.0;
    keep_inside_camera(b, me);
    let pointer = rd16(b, me + f::TARGET);
    if (pointer as i16) >= 0 {
        return;
    }
    let held = object(pointer);
    let step = rd8(b, me + ANIM_MARK) as u32;
    if step == 0 {
        return;
    }
    let mut at = SEIZE_OFFSETS + 4 * step;
    let mirrored = rd8(b, me + f::FACING) != 0;
    let mut dx = rd8s(b, at);
    if mirrored {
        dx = -dx;
    }
    let x = (dx as u16).wrapping_add(rd16(b, me + f::X));
    wr16(b, held + f::X, x);
    let rise = (rd8s(b, at + 1) as u16).wrapping_add(rd16(b, me + 0x0c));
    let height = if (rise as i16) < 0 {
        let z = rd16(b, me + 0x0c).wrapping_sub(rise);
        wr16(b, me + 0x0c, z);
        0
    } else {
        rise
    };
    wr16(b, held + 0x0c, height);
    let depth = rd16(b, me + f::DEPTH);
    wr16(b, held + f::DEPTH, depth);
    wr16(b, me + 0x12, 1);
    wr16(b, held + 0x12, 1);
    at += 2;
    let faces = rd8(b, at) != 0;
    let mut facing: u8 = if faces { 32 } else { 0 };
    if mirrored {
        facing = if facing != 0 { 0 } else { 32 };
    }
    wr8(b, held + f::FACING, facing);
    let direction: i16 = if (rd8(b, at + 1) as i8) < 0 { 1 } else { -1 };
    let v = rd16(b, me + 0x12).wrapping_add(direction as u16);
    wr16(b, me + 0x12, v);
}

// --- états et paramètres (0xff54, 0xfffc) --------------------------------------------------------------

/// Masques (mots, par joueurs en vie et rang) de 0xff54.
const COUNTDOWN_MASKS: u32 = 0xff9a;

/// 0xff54 : compte à rebours `+0x98` ; à son terme, et si le bit tiré dans le masque des joueurs en vie et du rang
/// est levé, état 0x020e0000 ; sinon `o` reste (ou redevient) actif à l'état 0x04040200.
pub fn countdown_then_state(b: &mut Cps2Bus, o: Obj) {
    let left = o.u8(b, g::BEHAVIOUR).wrapping_sub(1);
    o.set_u8(b, g::BEHAVIOUR, left);
    let again = |b: &mut Cps2Bus| {
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u32(b, Obj::STATE, 0x0404_0200);
    };
    if left as i8 > 0 {
        return again(b);
    }
    let row = ((crate::ram::JOUEURS_EN_VIE.get(b) as u16) << 4).wrapping_add((rd16(b, TICK) >> 2) << 1);
    let mask = crate::mem::prog16(b, COUNTDOWN_MASKS.wrapping_add(offset(row))) as u32;
    let roll = rng::random(b) as u32 & 15;
    if mask >> roll & 1 == 0 {
        return again(b);
    }
    o.set_u32(b, Obj::STATE, 0x020e_0000);
}

/// Paramètre de comportement par type de 0xfffc.
const BEHAVIOUR_BY_KIND: u32 = 0x1000a;

/// 0xfffc : paramètre de comportement `+0x98` de `o`, selon son type.
pub fn behaviour_by_kind(b: &mut Cps2Bus, o: Obj) {
    let kind = o.u8(b, Obj::KIND) as u32;
    let v = crate::mem::prog8(b, BEHAVIOUR_BY_KIND + kind);
    o.set_u8(b, g::BEHAVIOUR, v);
}

// --- tirages par tables (0x788c, 0x79ce, 0x79f6, 0xb048, 0xb068, 0xf118, 0x10112) ---------------------

/// Masque de 32 bits de la ligne `number` de la table `pointers`, choisi par le rang ; un de ses bits est tiré :
/// vrai s'il est levé.
fn rank_mask_roll(b: &mut Cps2Bus, pointers: u32, number: u8) -> bool {
    let row = rd32(b, pointers + 4 * number as u32);
    let tick = rd16(b, TICK);
    let mask = rd32(b, row.wrapping_add(offset(tick << 2)));
    let roll = rng::random(b) as u32 & 31;
    mask >> roll & 1 != 0
}

/// Octet tiré au hasard (32 choix) dans la ligne `number` de la table `pointers`.
fn random_byte(b: &mut Cps2Bus, pointers: u32, number: u8) -> u8 {
    let row = rd32(b, pointers + 4 * number as u32);
    let roll = rng::random(b) as u32 & 31;
    rd8(b, row + roll)
}

/// 0x788c : `+0x6b` tiré dans la ligne `+0x6a`.
pub fn draw_6b(b: &mut Cps2Bus, o: Obj) {
    let number = o.u8(b, g::ROW_6A);
    let v = random_byte(b, 0x78ae, number);
    o.set_u8(b, g::DRAW_6B, v);
}

/// 0x79f6 : dans la zone de jeu, `+0x6e` est tiré dans la ligne `+0x6d` ; sinon il vaut 1.
pub fn draw_6e(b: &mut Cps2Bus, o: Obj) {
    let v = if crate::objects::cull::in_arena(b, o, 0) {
        let number = o.u8(b, g::ROW_6D);
        random_byte(b, 0x7a50, number)
    } else {
        1
    };
    o.set_u8(b, g::DRAW_6E, v);
}

/// 0x79ce : si le bit tiré dans le masque de la ligne `+0x6c` est levé, `draw_6e` et vrai ; sinon faux.
pub fn maybe_draw_6e(b: &mut Cps2Bus, o: Obj) -> bool {
    let number = o.u8(b, g::ROW_6C);
    if !rank_mask_roll(b, 0x7a34, number) {
        return false;
    }
    draw_6e(b, o);
    true
}

/// Les deux tables de parade : ligne `+0x68` (0xb048) ou `+0x69` (0xb068) de la victime.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Parry {
    A,
    B,
}

/// 0xb048 / 0xb068 : la victime `victim` pare-t-elle le coup de `attacker` ? Il faut qu'elle sache parer (`+0x67`),
/// soit au sol et face à l'agresseur ; un bit est alors tiré dans le masque de sa ligne de parade.
pub fn parries(b: &mut Cps2Bus, attacker: Obj, victim: Obj, table: Parry) -> bool {
    let facing = attacker.u8(b, f::FACING);
    if victim.u8(b, 0x67) == 0 || victim.u16(b, Obj::Z) != 0 || facing == victim.u8(b, f::FACING) {
        return false;
    }
    let field = match table {
        Parry::A => 0x68,
        Parry::B => 0x69,
    };
    let number = victim.u8(b, field);
    rank_mask_roll(b, 0xb0b0, number)
}

/// 0xf118 : hors de la zone de jeu, `+0x84` = 1 ; sinon `+0x84` = 0 et `+0x85` est tiré dans la ligne `+0x8e`, bloc
/// choisi par le rang (32 octets par paire de rangs).
pub fn draw_84(b: &mut Cps2Bus, o: Obj) {
    if !crate::objects::cull::in_arena(b, o, 0) {
        o.set_u16(b, g::OFFSCREEN_84, 1);
        return;
    }
    let number = o.u8(b, g::ROW_8E) as u32;
    let block = (rd16(b, TICK) >> 1) << 5;
    let row = rd32(b, 0xf160 + 4 * number).wrapping_add(offset(block));
    let roll = rng::random(b) as u32 & 31;
    let v = rd8(b, row + roll);
    o.set_u8(b, g::DRAW_85, v);
    o.set_u8(b, g::OFFSCREEN_84, 0);
}

/// Niveaux de parade par niveau de scène et joueurs en vie de 0x10112.
const PARRY_LEVELS: u32 = 0x10144;

/// 0x10112 : niveau de parade `+0x68` = 8 sans niveau de scène (`A5+0x5a`), sinon lu selon lui (plafonné à 4) et le
/// nombre de joueurs en vie.
pub fn set_68(b: &mut Cps2Bus, o: Obj) {
    let level = rd8(b, A5 + 0x5a);
    let v = if level == 0 {
        8
    } else {
        let players = crate::ram::JOUEURS_EN_VIE.get(b).wrapping_mul(4);
        let row = ((level.wrapping_sub(1) as i8).min(3) as u8).wrapping_add(players);
        crate::mem::prog8(b, PARRY_LEVELS + row as u32)
    };
    o.set_u8(b, g::PARRY_LEVEL, v);
}

// --- poussée des joueurs (0x115e2) -----------------------------------------------------------------------

/// 0x115e2 : le boss `o` pousse chaque joueur présent devant lui, sauf le voleur ou le nain dans leur état 26
/// (`+0x6a`), un joueur en sortie (`+0x66`) ou protégé (`+0x188`).
pub fn push_players(b: &mut Cps2Bus, o: Obj) {
    for p in crate::ram::JOUEURS.iter() {
        let player = p.0;
        if rd8(b, player) == 0 {
            continue;
        }
        if rd8(b, player + 3) >> 1 == 5 && rd8(b, player + 0x6a) == 26 {
            continue;
        }
        if rd8(b, player + 0x66) == 0 && rd8(b, player + 0x188) == 0 {
            push_player_once(b, o.0, player);
        }
    }
}

// --- débris et butin (0x1172e, 0x11a36, 0xb648, 0xb780, 0xbad6, 0xe2f6) --------------------------------

/// Jeux de débris de 0x1172e.
const DEBRIS_SETS: u32 = 0x1178a;

/// 0x1172e : jeu de débris n° `set` (table : nombre - 1, puis lignes de 4 mots : décalage x, vitesse x, `+0x18`,
/// `+0x58`) autour de `o`, retournés si `o` regarde à gauche. Une ligne n'est lue que si l'effet a pu être créé.
pub fn debris_set(b: &mut Cps2Bus, o: Obj, set: u16) {
    let mut at = crate::mem::prog32(b, DEBRIS_SETS.wrapping_add(offset(set.wrapping_mul(4))));
    let mut next = |b: &mut Cps2Bus| {
        let v = rd16(b, at);
        at += 2;
        v
    };
    let count = next(b);
    let mirrored = o.u8(b, f::FACING) != 0;
    for _ in 0..=count {
        let Some(fx) = alloc(b, &pools::EFFCT) else { continue };
        debris(b, fx, o.0);
        let z = o.u16(b, Obj::Z);
        wr16(b, fx + 0x0c, z);
        let depth = o.u32(b, Obj::Y).wrapping_sub(1);
        wr32(b, fx + 0x10, depth);
        let (mut dx, mut vx) = (next(b), next(b));
        let v18 = next(b);
        wr16(b, fx + 0x18, v18);
        let v58 = next(b);
        wr16(b, fx + 0x58, v58);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
        }
        let x = rd16(b, fx + 0x08).wrapping_add(dx);
        wr16(b, fx + 0x08, x);
        wr16(b, fx + 0x14, vx);
    }
}

/// Lignes des débris au sol de 0x11a36 (la seconde si `second`).
const GROUND_DEBRIS: [u32; 2] = [0x11a9e, 0x11ada];

/// 0x11a36 : cinq débris au sol (profondeur `A5+0x5aa`), lignes de 6 mots (décalage x, vitesse x, vitesse z,
/// `+0x18`, décalage en profondeur, `+0x1c`), retournés si `o` regarde à gauche ; puis le son 0xede08.
pub fn ground_debris(b: &mut Cps2Bus, o: Obj, second: bool) {
    let mut at = GROUND_DEBRIS[second as usize];
    let mut next = |b: &mut Cps2Bus| {
        let v = rd16(b, at);
        at += 2;
        v
    };
    let mirrored = o.u8(b, f::FACING) != 0;
    for _ in 0..5 {
        let Some(fx) = alloc(b, &pools::EFFCT) else { continue };
        debris(b, fx, o.0);
        let ground = rd16(b, A5 + 0x5aa);
        wr16(b, fx + 0x10, ground);
        let (mut dx, mut vx, mut vz) = (next(b), next(b), next(b));
        let v18 = next(b);
        wr16(b, fx + 0x18, v18);
        let ddepth = next(b);
        let v1c = next(b);
        wr16(b, fx + 0x1c, v1c);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            vz = vz.wrapping_neg();
        }
        let x = rd16(b, fx + 0x08).wrapping_add(dx);
        wr16(b, fx + 0x08, x);
        let v14 = rd16(b, fx + 0x14).wrapping_add(vx);
        wr16(b, fx + 0x14, v14);
        wr16(b, fx + 0x16, vz);
        let depth = rd16(b, fx + 0x10).wrapping_add(ddepth);
        wr16(b, fx + 0x10, depth);
    }
    crate::sound::jouer(b, 0xede08, o);
}

/// Objet lâché une seule fois par type de 0xb648 (genre 2 ; 0 : rien), et masques de chance.
const ONCE_VARIANTS: u32 = 0xb6e4;
const ONCE_MASKS: u32 = 0xb6fc;

/// Lance un objet en l'air au-dessus de `o` (genre `kind`, variante `variant`) : vitesses ±0x140 en x, 0x300 vers
/// le haut, `vy` en profondeur, retournées si `mirrored`.
fn toss_item(b: &mut Cps2Bus, o: Obj, item: u32, kind: u8, variant: u8, vy: u16, mirrored: bool) {
    tossed_item(b, o.0, item, kind, variant, vy, mirrored);
}

/// 0xb648 : une seule fois par objet (`+0x9d`), si le bit tiré dans le masque `+0x9c` est levé, lâche l'objet de
/// genre 2 propre à son type (rien pour une variante nulle), lancé d'un côté tiré au hasard.
pub fn drop_once(b: &mut Cps2Bus, o: Obj) {
    if o.u8(b, g::ONCE_DONE) != 0 {
        return;
    }
    let roll = rng::random(b) as u32 & 31;
    let line = o.u8(b, g::ONCE_MASK) as u32;
    let mask = rd32(b, ONCE_MASKS + 4 * line);
    if mask >> roll & 1 == 0 {
        return;
    }
    let kind = o.u8(b, Obj::KIND) as u32;
    let variant = crate::mem::prog8(b, ONCE_VARIANTS + kind);
    if variant == 0 {
        return;
    }
    let Some(item) = alloc(b, &pools::ITEM) else { return };
    o.set_u8(b, g::ONCE_DONE, 1);
    toss_item(b, o, item, 2, variant, (-0x40i16) as u16, false);
    if rng::random(b) & 1 != 0 {
        for field in [Obj::VX, Obj::VY] {
            let v = rd16(b, item + field).wrapping_neg();
            wr16(b, item + field, v);
        }
    }
}

/// Butin par ligne (`+0x80`, 16 tirages) de 0xb780 ; variante négative : rien.
const LOOT: u32 = 0xb806;

/// 0xb780 : butin de `o`, un ou deux tirages selon le nombre de joueurs ; le second lancé de l'autre côté.
pub fn drop_loot(b: &mut Cps2Bus, o: Obj) {
    let row = o.u8(b, g::LOOT_ROW) as u16;
    if row == 0 {
        return;
    }
    for left in (0..loot_rolls(b)).rev() {
        let roll = (rng::random(b) & 15).wrapping_add(row << 4);
        let variant = crate::mem::prog8(b, LOOT.wrapping_add(offset(roll)));
        if (variant as i8) < 0 {
            return;
        }
        let Some(item) = alloc(b, &pools::ITEM) else { return };
        toss_item(b, o, item, 0, variant, 0x40, left != 0);
    }
}

/// Trésors de 0xe2f6 : ligne (`+0x80`, 16 tirages) → liste (longs) d'objets de genre 5, terminée par un octet
/// négatif.
const TREASURE_ROWS: u32 = 0xe372;
const TREASURE_LISTS: u32 = 0xe642;

/// 0xe2f6 : trésor de `o` (un ou deux tirages) ; les objets d'une liste sont numérotés à partir de 1 (`+0x2f`) et
/// posés au-dessus de `o`. Rend le mot bas de D2 que l'original laisse (la ligne, × 16 si elle est non nulle) et son
/// D5 (le compteur de tirages de la boucle `dbf`, 0x0000ffff à la fin), s'il l'a écrit.
pub fn drop_treasure(b: &mut Cps2Bus, o: Obj) -> (u16, Option<u32>) {
    let row = o.u8(b, g::LOOT_ROW) as u16;
    if row == 0 {
        return (0, None);
    }
    let mut rank: u8 = 1;
    for left in (0..loot_rolls(b)).rev() {
        let roll = (rng::random(b) & 15).wrapping_add(row << 4);
        let list = crate::mem::prog8(b, TREASURE_ROWS.wrapping_add(offset(roll)));
        if (list as i8) < 0 {
            return (row << 4, Some(left as u32));
        }
        let mut at = rd32(b, TREASURE_LISTS + 4 * list as u32);
        loop {
            let variant = rd8(b, at);
            at += 1;
            if (variant as i8) < 0 {
                break;
            }
            let Some(item) = alloc(b, &pools::ITEM) else { return (row << 4, Some(left as u32)) };
            wr8(b, item, 1);
            wr8(b, item + 2, 5);
            wr8(b, item + 3, variant);
            wr8(b, item + 0x2f, rank);
            rank = rank.wrapping_add(1);
            place_above(b, o.0, item);
        }
    }
    (row << 4, Some(0xffff))
}

/// Les joueurs en jeu et en vie (présents, PV non nuls), dans l'ordre, avec leur classe (`+3` / 2).
fn party_classes(b: &mut Cps2Bus) -> Vec<u8> {
    let mut classes = Vec::with_capacity(4);
    for p in crate::ram::JOUEURS.iter() {
        if p.u8(b, Obj::ACTIVE) != 0 && p.u16(b, crate::ai::f::HP) != 0 {
            classes.push(p.u8(b, Obj::VARIANT) >> 1);
        }
    }
    classes
}

/// Tire une entrée de `table` à `base` + (tirage & `mask`) × `scale`.
fn special_entry(b: &mut Cps2Bus, table: u32, base: u16, mask: u16, scale: u16) -> u32 {
    let roll = (rng::random(b) & mask).wrapping_mul(scale).wrapping_add(base);
    table.wrapping_add(offset(roll))
}

/// Début de la ligne de difficulté (`A5+0x49`) de `size` octets.
fn difficulty_line(b: &mut Cps2Bus, size: u16) -> u16 {
    rd8(b, A5 + 0x49) as u16 * size
}

/// 0xbad6 : objet spécial lâché par `o` (ligne `+0x80` de la table 0xbf26 : paires genre et variante, 16 tirages ;
/// genre négatif : rien), un ou deux tirages selon le nombre de joueurs. Une variante 0 à 3 de genre 0 renvoie à
/// des tables par difficulté (`A5+0x49`), certaines selon les classes en jeu (clerc, elfe, magicien) ; certains
/// objets sont réservés à un groupe d'un seul genre de lanceurs de sorts (`restrict_draw`). L'objet est lancé en
/// l'air comme le butin. Rend le mot bas de D3 que l'original laisse s'il l'a écrit (ligne de difficulté, ou
/// classes en jeu).
pub fn drop_special(b: &mut Cps2Bus, o: Obj) -> Option<u16> {
    let row = o.u8(b, g::LOOT_ROW) as u16;
    if row == 0 {
        return None;
    }
    let mut d3 = None;
    let mut left = (crate::ram::JOUEURS_EN_VIE.get(b) > 3) as u16;
    let mut step = Draw::Row;
    loop {
        step = match step {
            Draw::Row => {
                let at = special_entry(b, special::ROWS, row << 5, 15, 2);
                let (kind, variant) = (rd8(b, at), rd8(b, at + 1));
                set_drawn(b, kind, variant);
                match (kind as i8, variant) {
                    (k, _) if k < 0 => return d3,
                    (0, 0) => Draw::Common,
                    (0, 1) => Draw::Pair,
                    (0, 2) => Draw::Kind2,
                    (0, _) => Draw::ByClass,
                    _ => Draw::Restrict,
                }
            }
            Draw::Common => {
                let line = difficulty_line(b, 64);
                d3 = Some(line);
                let at = special_entry(b, special::COMMON, line, 31, 1);
                let caster = party_classes(b).iter().any(|&k| matches!(k, CLERIC | ELF | MAGE));
                if caster {
                    let variant = rd8(b, at);
                    set_drawn(b, 7, variant);
                    Draw::Restrict
                } else {
                    redraw(b, at + 32, Draw::Pair, Draw::Restrict)
                }
            }
            Draw::Pair => {
                let line = difficulty_line(b, 64);
                d3 = Some(line);
                let at = special_entry(b, special::PAIRS, line, 31, 2);
                let (kind, variant) = (rd8(b, at), rd8(b, at + 1));
                set_drawn(b, kind, variant);
                Draw::Restrict
            }
            Draw::Kind2 => {
                let line = difficulty_line(b, 16);
                d3 = Some(line);
                let at = special_entry(b, special::KIND_2, line, 15, 1);
                let variant = rd8(b, at);
                set_drawn(b, 2, variant);
                Draw::Restrict
            }
            Draw::ByClass => {
                let line = difficulty_line(b, 64);
                let at = special_entry(b, special::BY_CLASS, line, 15, 1);
                let classes = party_classes(b).iter().fold(0u8, |bits, &k| {
                    bits | match k {
                        CLERIC => 1,
                        ELF => 2,
                        MAGE => 4,
                        _ => 0,
                    }
                });
                d3 = Some(classes as u16);
                if classes == 4 {
                    let variant = rd8(b, at);
                    set_drawn(b, 3, variant);
                    Draw::Restrict
                } else {
                    let shift = match classes {
                        0 => 48,
                        1 => 16,
                        _ => 32,
                    };
                    redraw(b, at + shift, Draw::Common, Draw::Pair)
                }
            }
            Draw::Restrict => restrict_draw(b, &mut d3),
            Draw::Spawn => {
                let (kind, variant) = drawn(b);
                let Some(item) = alloc(b, &pools::ITEM) else { return d3 };
                toss_item(b, o, item, kind, variant, 0x40, left == 0);
                Draw::Next
            }
            Draw::Next => {
                if left == 0 {
                    return d3;
                }
                left -= 1;
                Draw::Row
            }
        }
    }
}

/// 0xbcb4 : objets réservés. Genre 3 variante 5 : clercs et magiciens seulement (table 0xd776) ; genre 4 variante
/// 15 : magiciens seulement (0xda56) ; genre 4 variantes 8 et 9 : clercs seulement (0xdbc6) ; genre 4 variantes 10
/// et 11 : selon les magiciens et les clercs en jeu (0xdd36). Un groupe qui ne convient pas garde l'objet tiré.
fn restrict_draw(b: &mut Cps2Bus, d3: &mut Option<u16>) -> Draw {
    let (kind, variant) = drawn(b);
    match (kind, variant) {
        (3, 5) => {
            let classes = party_classes(b);
            // la boucle de l'original s'arrête au premier joueur ni clerc ni magicien
            let other = classes.iter().any(|&k| k != CLERIC && k != MAGE);
            let mut bits = 0u8;
            for &k in classes.iter().take_while(|&&k| k == CLERIC || k == MAGE) {
                bits |= if k == CLERIC { 1 } else { 2 };
            }
            if other || bits == 0 {
                return Draw::Spawn;
            }
            let shift = if bits == 1 { 0 } else { 16 };
            let line = difficulty_line(b, 32);
            *d3 = Some(line);
            let at = special_entry(b, special::CLERIC_OR_MAGE, line, 15, 1);
            let variant = rd8(b, at + shift);
            wr8(b, DRAWN_VARIANT, variant);
            match variant {
                0 => Draw::Next,
                1 => Draw::Common,
                2 => Draw::Pair,
                3 => Draw::ByClass,
                _ => Draw::Spawn,
            }
        }
        (4, 15) => only_class_draw(b, d3, MAGE, special::MAGES),
        (4, 8 | 9) => only_class_draw(b, d3, CLERIC, special::CLERICS),
        (4, 10 | 11) => {
            let bits = party_classes(b).iter().fold(0u8, |bits, &k| {
                bits | if k == MAGE { 1 } else { 0 } | if k == CLERIC { 2 } else { 0 }
            });
            let shift = match bits {
                0 => return Draw::Spawn,
                1 => 0,
                2 => 16,
                _ => 32,
            };
            let line = difficulty_line(b, 16) + shift;
            *d3 = Some(line);
            let at = special_entry(b, special::MAGE_OR_CLERIC, line, 15, 1);
            reroll_or_none(b, at)
        }
        _ => Draw::Spawn,
    }
}

/// Tous les joueurs en jeu sont de la classe `class` : nouvelle variante de genre 4 tirée dans `table`.
fn only_class_draw(b: &mut Cps2Bus, d3: &mut Option<u16>, class: u8, table: u32) -> Draw {
    if party_classes(b).iter().any(|&k| k != class) {
        return Draw::Spawn;
    }
    let line = difficulty_line(b, 16);
    *d3 = Some(line);
    let at = special_entry(b, table, line, 15, 1);
    reroll_or_none(b, at)
}

// --- éclair et secousse (0x103ba), atterrissage par type (0x3e480) -----------------------------------

/// 0x103ba : éclair rouge, secousse de 8 images (`A5+0x4be`) et étincelle d'impact (0xb141a, vitesses `vx`, `ax`)
/// dont la variante est remise à zéro.
/// Glitch d'origine : l'original ne pose pas les vitesses de l'étincelle ; elle prend les D2 et D3 que laissent les
/// routines appelées avant (les appelants les passent en clair).
pub fn flash_and_shake(b: &mut Cps2Bus, o: Obj, vx: u16, ax: u16) {
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_RED);
    wr16(b, A5 + 0x4be, 8);
    if let Some(spark) = crate::families::effects_creators::impact_spark(b, o, vx, ax) {
        spark.set_u8(b, Obj::VARIANT, 0);
    }
}

/// 0x3e480 : la routine d'atterrissage propre au type de `o` (ennemis : classe 2, types 0..31 ; boss ensuite) et
/// l'index lu dans la table, que l'original laisse dans D0 en sautant à la routine (A1). `None` pour les types sans
/// routine (un simple `rts` : 0x3e2aa, 0x3bb9a).
pub fn landing_routine(b: &mut Cps2Bus, o: Obj) -> Option<(u32, u16)> {
    let base: u8 = if o.u8(b, Obj::CLASS) == 2 { 0 } else { 32 };
    let index = base.wrapping_add(o.u8(b, Obj::KIND)) as u16 * 4;
    let routine = crate::mem::prog32(b, LANDING_BY_KIND.wrapping_add(offset(index)));
    (routine != 0x3e2aa && routine != 0x3bb9a).then_some((routine, index))
}
