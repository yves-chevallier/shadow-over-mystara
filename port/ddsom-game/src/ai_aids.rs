//! Aides d'IA des ennemis et des boss (suite de `ai`) : tirages d'attitude par tables, test de la
//! boîte devant un adversaire.
//!
//! Ces routines gardent le contrat des originaux : un résultat dans D0 (1 : oui, 0 : non) et les
//! drapeaux d'un `moveq`, que les handlers testent juste après l'appel (`beq`).

use cps2::Cps2Bus;
use m68k::decode::{Fetch, Size};
use m68k::{Cpu, SR_C, SR_Z};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8, A5};
use crate::ops::logique;
use crate::rng::Rng;
use crate::{call, regs, Native};
use crate::objects::pools;

/// Compteur de l'image courante, qui sert d'index dans les tables d'attitude.
const TICK: u32 = A5 + 0x7f92;
/// État du générateur pseudo-aléatoire.
const RNG_STATE: u32 = A5 + 0x1a;

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

/// Tirage du générateur pseudo-aléatoire du jeu (0x19a88).
fn random(b: &mut Cps2Bus) -> u16 {
    let mut rng = Rng(rd16(b, RNG_STATE));
    let (v, _) = rng.suivant();
    wr16(b, RNG_STATE, v);
    v
}

/// Mot de la table de pointeurs `table` désigné par `index`, étendu en signe comme le fait un index
/// `Dn.w` du 68000.
fn offset(index: u16) -> u32 {
    index as i16 as i32 as u32
}

fn pointer(c: &Cpu, b: &mut Cps2Bus, table: u32, number: u8) -> u32 {
    let _ = c;
    crate::mem::rd32(b, table + 4 * number as u32)
}

/// 0xa470 : tire l'attitude (octet de la table `+0x70`, indexée par le compteur d'images).
fn stance(c: &mut Cpu, b: &mut Cps2Bus) {
    let number = rd8(b, c.a[0] + f::STANCE_TABLE);
    c.a[4] = pointer(c, b, STANCE_POINTERS, number);
    let tick = rd16(b, TICK);
    let v = rd8(b, c.a[4].wrapping_add(offset(tick)));
    wr8(b, c.a[0] + f::STANCE_VALUE, v);
    c.d[0] = tick as u32;
    logique(c, v as u32, Size::Byte);
    rts(c, b);
}

/// Les quatre joueurs (A5 + 0x5de, pas 0x200).
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

/// Remplace l'octet bas / le mot bas d'un registre (les autres bits restent).
fn set_b(r: &mut u32, v: u8) {
    *r = *r & !0xff | v as u32;
}
fn set_w(r: &mut u32, v: u16) {
    *r = *r & !0xffff | v as u32;
}

/// Sortie « non » des tests : D0 = 0 et les drapeaux de `moveq`.
fn no(c: &mut Cpu, b: &mut Cps2Bus) {
    moveq(c, 0);
    rts(c, b);
}

/// 0x7f0c et 0x7ef4 : un adversaire (l'objet visé `+0x50`) est-il dans la boîte devant l'ennemi ?
/// D0 = décalage de la boîte, D1 et D2 = demi-largeur et demi-profondeur. Si oui, après un tirage de
/// probabilité, l'ennemi passe à l'état 6 avec une animation tirée au hasard. Les registres D1-D4, A1 et A4
/// sont laissés comme l'original (le code appelant les relit).
fn in_front(c: &mut Cpu, b: &mut Cps2Bus, strict: bool) {
    let me = c.a[0];
    let target = rd16(b, me + f::TARGET);
    if target == 0 {
        return no(c, b);
    }
    let other = object(target);
    c.a[1] = other;
    let state = rd8(b, other + f::SUB_STATE);
    if strict {
        if rd16(b, other + 0x0c) != 0 || (state != 12 && state != 14) {
            return no(c, b);
        }
    } else if state != 12 {
        return no(c, b);
    }
    let facing = rd8(b, me + f::FACING);
    set_b(&mut c.d[3], facing);
    if facing == rd8(b, other + f::FACING) {
        return no(c, b);
    }
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    set_w(&mut c.d[3], gap);
    let side = if (gap as i16) >= 0 { 0 } else { 32 };
    set_b(&mut c.d[4], side);
    if side != facing {
        return no(c, b);
    }
    let offset_x = if facing != 0 { (c.d[0] as u16).wrapping_neg() } else { c.d[0] as u16 };
    let other_x = rd16(b, other + f::X);
    set_w(&mut c.d[3], other_x);
    let half_x = c.d[1] as u16;
    let reach_x = offset_x.wrapping_add(rd16(b, me + f::X)).wrapping_sub(other_x).wrapping_add(half_x);
    set_w(&mut c.d[0], reach_x);
    set_w(&mut c.d[1], half_x.wrapping_mul(2));
    if reach_x > half_x.wrapping_mul(2) {
        return no(c, b);
    }
    let other_depth = rd16(b, other + f::DEPTH);
    set_w(&mut c.d[1], other_depth);
    let half_depth = c.d[2] as u16;
    let reach_depth = rd16(b, me + f::DEPTH).wrapping_sub(other_depth).wrapping_add(half_depth);
    set_w(&mut c.d[0], reach_depth);
    set_w(&mut c.d[2], half_depth.wrapping_mul(2));
    if reach_depth > half_depth.wrapping_mul(2) {
        return no(c, b);
    }
    let number = rd8(b, me + f::BOX_CHANCE);
    c.a[4] = pointer(c, b, BOX_CHANCE_POINTERS, number);
    let tick = rd16(b, TICK);
    c.d[2] = rd32(b, c.a[4].wrapping_add(offset(tick << 2)));
    let roll = random(b) as u32 & 31;
    c.d[0] = roll;
    if c.d[2] >> roll & 1 == 0 {
        return no(c, b);
    }
    let number = rd8(b, me + f::BOX_ANIMATION);
    // l'octet `+0x75` est déjà un multiple de 4 : pas de décalage
    c.a[4] = rd32(b, BOX_ANIMATION_POINTERS.wrapping_add(number as u32));
    c.a[4] = c.a[4].wrapping_add(offset((tick >> 1) << 5));
    let pick = random(b) as u32 & 31;
    wr8(b, me + f::STATE, 6);
    let v = rd8(b, c.a[4].wrapping_add(pick));
    wr8(b, me + f::SUB_STATE, v);
    wr8(b, me + f::SUB_STEP, 0);
    moveq(c, 1);
    rts(c, b);
}

/// 0x7f19c : l'objet A2 est-il dans la boîte devant l'objet A0 ? D0 = décalage, D1 et D2 = demi-largeur
/// et demi-profondeur. D0 : 0 non (ou en l'air), 1 oui et de face, 2 oui mais de dos.
fn box_around(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, other) = (c.a[0], c.a[2]);
    let found = box_reach(c, b, me, other);
    moveq(c, found);
    rts(c, b);
}

fn box_reach(c: &mut Cpu, b: &mut Cps2Bus, me: u32, other: u32) -> i32 {
    if rd16(b, other + 0x0c) != 0 {
        return 0;
    }
    let facing = rd8(b, me + f::FACING);
    let offset_x = if facing != 0 { (c.d[0] as u16).wrapping_neg() } else { c.d[0] as u16 };
    let other_x = rd16(b, other + f::X);
    set_w(&mut c.d[3], other_x);
    let half_x = c.d[1] as u16;
    let reach_x = offset_x.wrapping_add(rd16(b, me + f::X)).wrapping_sub(other_x).wrapping_add(half_x);
    set_w(&mut c.d[0], reach_x);
    set_w(&mut c.d[1], half_x.wrapping_mul(2));
    if reach_x > half_x.wrapping_mul(2) {
        return 0;
    }
    let other_depth = rd16(b, other + f::DEPTH);
    set_w(&mut c.d[1], other_depth);
    let half_depth = c.d[2] as u16;
    let reach_depth = rd16(b, me + f::DEPTH).wrapping_sub(other_depth).wrapping_add(half_depth);
    set_w(&mut c.d[0], reach_depth);
    set_w(&mut c.d[2], half_depth.wrapping_mul(2));
    if reach_depth > half_depth.wrapping_mul(2) {
        return 0;
    }
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, other + f::X));
    set_w(&mut c.d[1], gap);
    let side = if (gap as i16) < 0 { 32 } else { 0 };
    set_b(&mut c.d[2], side);
    if side == facing {
        1
    } else {
        2
    }
}

/// 0x67452 : le joueur le plus proche dans la boîte devant l'objet A0 (D0 = décalage, D1 = demi-largeur,
/// D2 = demi-profondeur) est mémorisé en `+0x50` (0 si aucun). Distance = écart en profondeur + écart en x.
/// Les registres D0-D6 et A1 sont laissés comme l'original.
fn nearest_player_in_box(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let (half_x, half_depth) = (c.d[1] as u16, c.d[2] as u16);
    wr16(b, me + f::TARGET, 0);
    // D4 : (2 x demi-largeur, 2 x demi-profondeur), D2 : (demi-largeur, demi-profondeur) ; D3 et D1 : distances
    c.d[4] = (half_x.wrapping_mul(2) as u32) << 16 | half_depth.wrapping_mul(2) as u32;
    c.d[2] = (half_x as u32) << 16 | half_depth as u32;
    c.d[1] = u32::MAX;
    c.d[3] = 0;
    let offset_x = if rd8(b, me + f::FACING) != 0 { (c.d[0] as u16).wrapping_neg() } else { c.d[0] as u16 };
    set_w(&mut c.d[0], offset_x);
    for k in 0..4 {
        let player = PLAYERS + k * PLAYER_STRIDE;
        c.a[1] = player;
        if rd8(b, player) == 0 {
            continue;
        }
        let depth = rd16(b, me + f::DEPTH).wrapping_sub(rd16(b, player + f::DEPTH)).wrapping_add(half_depth);
        set_w(&mut c.d[5], depth);
        if depth > half_depth.wrapping_mul(2) {
            continue;
        }
        set_w(&mut c.d[3], depth);
        let across = rd16(b, me + f::X).wrapping_add(offset_x).wrapping_sub(rd16(b, player + f::X)).wrapping_add(half_x);
        set_w(&mut c.d[5], across);
        if across > half_x.wrapping_mul(2) {
            continue;
        }
        let distance = (c.d[3] as u16).wrapping_add(across);
        set_w(&mut c.d[3], distance);
        if distance <= c.d[1] as u16 {
            set_w(&mut c.d[1], distance);
            wr16(b, me + f::TARGET, player as u16);
        }
    }
    c.a[1] = PLAYERS + 4 * PLAYER_STRIDE;
    set_w(&mut c.d[6], 0xffff);
    let found = rd16(b, me + f::TARGET);
    logique(c, found as u32, Size::Word);
    rts(c, b);
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

/// 0x11654 : poussée pour l'écart en x D0 entre le boss A0 et le joueur A1.
fn push_amount(c: &mut Cpu, b: &mut Cps2Bus) {
    let push = push_amount_for(b, c.a[0], c.a[1], c.d[0] as u16);
    c.d[0] = push;
    logique(c, push, Size::Long);
    rts(c, b);
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

/// 0x1161c : le boss A0 pousse le joueur A1. D0 = 1 si poussé, 0 sinon.
fn push_player(c: &mut Cpu, b: &mut Cps2Bus) {
    let pushed = push_player_once(b, c.a[0], c.a[1]);
    moveq(c, pushed as i32);
    rts(c, b);
}

/// 0x115e2 : le boss A0 pousse chaque joueur présent, sauf le voleur ou le nain dans leur état 26 (`+0x6a`),
/// un joueur en sortie (`+0x66`) ou protégé (`+0x188`).
fn push_players(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    for k in 0..4 {
        let player = PLAYERS + k * PLAYER_STRIDE;
        if rd8(b, player) == 0 {
            continue;
        }
        if rd8(b, player + 3) >> 1 == 5 && rd8(b, player + 0x6a) == 26 {
            continue;
        }
        if rd8(b, player + 0x66) == 0 && rd8(b, player + 0x188) == 0 {
            push_player_once(b, me, player);
        }
    }
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// Animations par table et tests d'orientation (0x103e0-0x105d0). Les fonctions « corps » ne font pas
// le RTS : `leaf!` l'ajoute pour la native.
// ---------------------------------------------------------------------------------------------

/// Démarrage d'une animation dans le jeu de données A4.
const ANIM_START: u32 = 0x1a3c;
/// Octet `+0x33` : variante d'animation (7 bits) ; `+0x3e` : copie dans `+0x44` après chaque pas.
const VARIANT: u32 = 0x33;
const ANIM_TIMER: u32 = 0x30;
const ANIM_MARK: u32 = 0x3e;
const ANIM_MARK_COPY: u32 = 0x44;
/// Pointeur (mot) vers le maître, dont la variante choisit l'animation.
const MASTER: u32 = 0xb2;

/// Premier jeu d'animation (entrée de 16 octets) de A4 dont l'octet `+3` vaut `key` sous le masque.
fn find_animation_set(c: &mut Cpu, b: &mut Cps2Bus, key: u8, mask: u8) {
    for _ in 0..0x10000 {
        if rd8(b, c.a[4] + 3) & mask == key & mask {
            break;
        }
        c.a[4] = c.a[4].wrapping_add(16);
    }
}

fn copy_mark(c: &mut Cpu, b: &mut Cps2Bus) {
    let v = rd8(b, c.a[0] + ANIM_MARK);
    wr8(b, c.a[0] + ANIM_MARK_COPY, v);
}

/// Adresse de l'objet désigné par un pointeur de 16 bits (étendu en signe).
fn object(pointer: u16) -> u32 {
    pointer as i16 as i32 as u32
}

/// 0x103e0 : démarre l'animation de la variante `+0x33` de l'objet, en gardant son compteur.
fn animation_by_variant(c: &mut Cpu, b: &mut Cps2Bus) {
    let timer = rd16(b, c.a[0] + ANIM_TIMER);
    let key = rd8(b, c.a[0] + VARIANT);
    find_animation_set(c, b, key, 0x7f);
    call(c, b, ANIM_START);
    wr16(b, c.a[0] + ANIM_TIMER, timer);
    copy_mark(c, b);
}

/// 0x10428 : démarre l'animation de la variante du maître (`+0xb2`).
fn animation_of_master(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[1] = object(rd16(b, c.a[0] + MASTER));
    let key = rd8(b, c.a[1] + VARIANT);
    find_animation_set(c, b, key, 0x7f);
    call(c, b, ANIM_START);
    copy_mark(c, b);
}

/// 0x10450 : démarre l'animation dont la marque (`+3`) est celle de `+0x3e` du maître.
fn animation_of_mark(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[1] = object(rd16(b, c.a[0] + MASTER));
    let key = rd8(b, c.a[1] + ANIM_MARK);
    find_animation_set(c, b, key, 0xff);
    call(c, b, ANIM_START);
}

/// 0x10474 : drapeaux de l'octet `A5+0x35` (joueurs en vie).
fn players_alive_flags(c: &mut Cpu, b: &mut Cps2Bus) {
    let v = rd8(b, A5 + 0x35);
    logique(c, v as u32, Size::Byte);
}

/// 0x1052e : l'objet et sa cible se font-ils face ? D0 = 1 / 0 (drapeaux de `moveq`) ; D0-D2 et A1 comme l'original.
fn facing_each_other(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let pointer = rd16(b, me + f::TARGET);
    if (pointer as i16) >= 0 {
        return moveq(c, 0);
    }
    c.d[2] = 0;
    let target = object(pointer);
    c.a[1] = target;
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, target + f::X)) & 0x8000;
    set_w(&mut c.d[0], gap);
    set_b(&mut c.d[1], rd8(b, me + f::FACING));
    let rotated = (c.d[1] as u16).rotate_right(6);
    set_w(&mut c.d[1], rotated & gap);
    let wanted = if rotated & gap & 0x8000 != 0 { 0 } else { 32 };
    c.d[2] = wanted;
    moveq(c, (wanted == rd8(b, target + f::FACING) as u32) as i32);
}

/// 0x104e0 : Z levé si la cible regarde dans le même sens que l'objet, qui la regarde (corps sans RTS).
fn same_side_flags(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let pointer = rd16(b, me + f::TARGET);
    if (pointer as i16) >= 0 {
        return moveq(c, 0);
    }
    let target = object(pointer);
    c.a[1] = target;
    let facing = rd8(b, me + f::FACING);
    set_b(&mut c.d[0], facing);
    if facing != rd8(b, target + f::FACING) {
        return moveq(c, 0);
    }
    c.d[0] = 32;
    let gap = rd16(b, me + f::X).wrapping_sub(rd16(b, target + f::X));
    set_w(&mut c.d[1], gap);
    if (gap as i16) < 0 {
        c.d[0] = 0;
    }
    crate::ops::cmp(c, facing as u32, c.d[0] & 0xff, Size::Byte);
}

/// 0x1047a : décision de se tourner vers la cible : 1 (face à face, tirage A), 2 (même côté, tirage B), 0 sinon.
fn turn_decision(c: &mut Cpu, b: &mut Cps2Bus) {
    facing_each_other(c, b);
    let result = if c.sr & SR_Z != 0 {
        same_side_flags(c, b);
        if c.sr & SR_Z != 0 {
            0
        } else {
            call(c, b, 0xa4c2);
            if c.sr & SR_Z != 0 {
                0
            } else {
                2
            }
        }
    } else {
        call(c, b, 0xa4b4);
        (c.sr & SR_Z == 0) as i32
    };
    moveq(c, result);
}

const fn aid(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost: |_, _| 0, run, touches_sound: false, outputs: regs::d(0) | regs::CCR, exact_cost: false }
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

/// 0xfc44 : un joueur est-il touché par la boîte d'attaque D0 de l'objet A0 ? D0 = 1 oui, 0 non.
fn attack_hits_a_player(c: &mut Cpu, b: &mut Cps2Bus) {
    let attack = ATTACK_BOXES.wrapping_add(offset((c.d[0] as u16) << 3));
    let me = c.a[0];
    c.a[2] = attack;
    let hit = (0..4).find(|&k| player_hit(b, me, attack, PLAYERS + k * PLAYER_STRIDE));
    // A1 : le joueur touché (ou la fin de la table) ; D6 : compteur de la boucle de l'original
    c.a[1] = PLAYERS + hit.map_or(4, |k| k) * PLAYER_STRIDE;
    set_w(&mut c.d[6], hit.map_or(0xffff, |k| 3 - k as u16));
    let hit = hit.is_some();
    moveq(c, hit as i32);
}

/// 0xfd64 : le joueur A1 est saisi par l'objet A0 (D0 = animation de prise).
fn seize_player(c: &mut Cpu, b: &mut Cps2Bus) {
    let (me, player) = (c.a[0], c.a[1]);
    wr32(b, player + 4, 0x0204_3200);
    wr8(b, player, 2);
    wr16(b, player + 0x186, me as u16);
    wr8(b, player + 0x188, 1);
    wr16(b, me + f::TARGET, player as u16);
    wr32(b, me + 4, 0x020c_0000);
    wr8(b, me + 6, c.d[0] as u8);
    logique(c, c.d[0] & 0xff, Size::Byte);
}

/// 0xfd8c : le joueur A1 est relâché (animation 0x34).
fn release_player(c: &mut Cpu, b: &mut Cps2Bus) {
    let player = c.a[1];
    c.d[0] = c.d[0] & !0xff | 0x34;
    wr32(b, player + 4, 0x0204_0000);
    wr8(b, player + 6, 0x34);
    wr8(b, player + 0x188, 0);
    logique(c, 0, Size::Byte);
}

// ---------------------------------------------------------------------------------------------
// Choix d'un point où marcher autour de la cible (0xe886, 0xeff2, 0xf084)
// ---------------------------------------------------------------------------------------------

/// Tables de points candidats par numéro (`+0x79`) : pointeur, puis nombre de points - 1 et paires
/// (décalage x, décalage profondeur) autour de la cible.
const WALK_POINTS: u32 = 0xe93e;
const MIRRORED_WALK_POINTS: u32 = 0xf06a;
/// Point choisi (`+0x7a`, `+0x7c`), numéro du dernier point retenu (`+0x99`).
const GOAL_X: u32 = 0x7a;
const GOAL_DEPTH: u32 = 0x7c;
const LAST_POINT: u32 = 0x99;
const POINT_SET: u32 = 0x79;
/// Tourne l'objet vers le point (D1, D2).
const TURN_TOWARD: u32 = 0xf0e8;

/// Écart « en ville » entre deux points : |dx| + |dy| (mots).
fn manhattan(b: &mut Cps2Bus, me: u32, x: u16, depth: u16) -> u16 {
    let dx = (x.wrapping_sub(rd16(b, me + f::X)) as i16).wrapping_abs() as u16;
    let dy = (depth.wrapping_sub(rd16(b, me + f::DEPTH)) as i16).wrapping_abs() as u16;
    dx.wrapping_add(dy)
}

/// Retient le point candidat qui maximise (`farthest`) ou minimise l'écart avec `me`, parmi ceux de la
/// table de `me`. `skip` : numéro de point à ignorer (sans lire sa paire). `mirror` : les décalages sont
/// retournés si la cible regarde à gauche. Rend D1/D2 = point retenu comme le fait l'original.
fn pick_point(c: &mut Cpu, b: &mut Cps2Bus, farthest: bool, skip: Option<u8>, mirror: bool) {
    let me = c.a[0];
    let target = object(rd16(b, me + f::TARGET));
    c.a[1] = target;
    let table = if mirror { MIRRORED_WALK_POINTS } else { WALK_POINTS };
    let set = rd8(b, me + POINT_SET) as u32;
    let mut at = rd32(b, table + 4 * set);
    c.a[2] = at;
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (rd16(b, target + f::X), rd16(b, target + f::DEPTH));
    wr16(b, me + GOAL_X, target_x);
    wr16(b, me + GOAL_DEPTH, target_depth);
    let mut best: u16 = if farthest { 0 } else { 0x7fff };
    for point in (0..=last).rev() {
        if skip == Some(point as u8) {
            continue;
        }
        let mut dx = rd16(b, at);
        let dy = rd16(b, at + 2);
        at += 4;
        if mirror && rd8(b, target + f::FACING) != 0 {
            dx = dx.wrapping_neg();
        }
        let (x, depth) = (dx.wrapping_add(target_x), dy.wrapping_add(target_depth));
        let distance = manhattan(b, me, x, depth);
        if if farthest { distance > best } else { distance <= best } {
            best = distance;
            wr16(b, me + GOAL_X, x);
            wr16(b, me + GOAL_DEPTH, depth);
            if skip.is_some() {
                wr8(b, me + LAST_POINT, point as u8);
            }
        }
    }
    c.a[2] = at;
    c.d[1] = rd16(b, me + GOAL_X) as u32;
    c.d[2] = rd16(b, me + GOAL_DEPTH) as u32;
}

/// 0xf084 : marche vers le point le plus éloigné autour de la cible.
fn walk_far_point(c: &mut Cpu, b: &mut Cps2Bus) {
    pick_point(c, b, true, None, false);
    call(c, b, TURN_TOWARD);
}

/// 0xeff2 : marche vers le point le plus proche autour de la cible (décalages retournés selon son sens).
fn walk_near_point(c: &mut Cpu, b: &mut Cps2Bus) {
    if rd16(b, c.a[0] + f::TARGET) == 0 {
        return;
    }
    pick_point(c, b, false, None, true);
    call(c, b, TURN_TOWARD);
}

/// 0xe886 : choisit le point de marche le plus éloigné autour de la cible, sans le dernier retenu ; si
/// l'objet est loin de la caméra, marche tout de suite vers un point lointain.
fn choose_walk_point(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if rd16(b, me + f::TARGET) == 0 {
        return;
    }
    let camera = if rd8(b, A5 + 0x36) != 0 { rd16(b, A5 + 0x556) } else { rd16(b, A5 + 0x506) };
    if rd16(b, me + f::X).wrapping_sub(camera) > 384 {
        return walk_far_point(c, b);
    }
    let last = rd8(b, me + LAST_POINT);
    pick_point(c, b, true, Some(last), false);
}

/// 0xe914 : si le point de marche est atteint (à 8 pixels près), en choisit un autre.
fn walk_point_reached(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let near = |goal: u16, here: u16| goal.wrapping_sub(here).wrapping_add(8) <= 16;
    if near(rd16(b, me + GOAL_X), rd16(b, me + f::X)) && near(rd16(b, me + GOAL_DEPTH), rd16(b, me + f::DEPTH)) {
        choose_walk_point(c, b);
    }
}

// ---------------------------------------------------------------------------------------------
// Divers : tests d'écran, cap aléatoire, prise (0x104a2-0x105d0, 0xfda2, 0xff54)
// ---------------------------------------------------------------------------------------------

/// Origine de l'écran en x (mot de A5), selon la caméra utilisée (`A5+0x36`).
fn screen_origin(b: &mut Cps2Bus) -> u16 {
    let offset = if rd8(b, A5 + 0x36) == 0 { 0x5a6 } else { 0x5ae };
    rd16(b, A5 + offset)
}

/// 0x104a2 : D0 = 1 si l'objet n'est pas entre les abscisses D0 et D1 (relatives à l'écran), 0 sinon.
fn outside_screen_range(c: &mut Cpu, b: &mut Cps2Bus) {
    let origin = screen_origin(b);
    let x = rd16(b, c.a[0] + f::X) as i16;
    let (low, high) = ((c.d[0] as u16).wrapping_add(origin) as i16, (c.d[1] as u16).wrapping_add(origin) as i16);
    let outside = low >= x || high <= x;
    moveq(c, outside as i32);
}

/// 0x10574 : le cap de l'objet (`+0x2d`, modulo 32) change d'au plus 1 au hasard.
fn drift_heading(c: &mut Cpu, b: &mut Cps2Bus) {
    const STEPS: [u8; 16] = [1, 1, 0xff, 0, 0xff, 1, 0xff, 0, 1, 0, 1, 0xff, 1, 0xff, 0, 0xff];
    let step = STEPS[random(b) as usize & 15];
    c.d[0] = c.d[0] & !0xff | step as u32;
    let heading = rd8(b, c.a[0] + 0x2d).wrapping_add(step) & 31;
    wr8(b, c.a[0] + 0x2d, heading);
    logique(c, heading as u32, Size::Byte);
}

/// 0x105b6 : essaie la palier suivant (`+0xaa` < `+0xab`) dont le seuil de la table A1 (ligne `+0xaa`, colonne
/// `+0xac`, mots) ne dépasse pas les PV (`+0x62`). D0 = 1 si le palier est passé.
fn next_threshold(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let level = rd8(b, me + 0xaa) as i8;
    let mut passed = false;
    if level < rd8(b, me + 0xab) as i8 {
        let column = rd8(b, me + 0xac) as i8 as i16;
        let index = (level as i16).wrapping_shl(7).wrapping_add(column);
        let threshold = rd16(b, c.a[1].wrapping_add(offset(index as u16))) as i16;
        if threshold >= rd16(b, me + 0x62) as i16 {
            wr8(b, me + 0xaa, level.wrapping_add(1) as u8);
            passed = true;
        }
    }
    moveq(c, passed as i32);
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

/// 0xfda2 : l'objet saisi (`+0x50`) est tenu par l'objet A0 selon l'étape (`+0x3e`) de l'animation de prise.
/// D0 = 0 sans objet saisi, 1 sans étape.
fn hold_seized(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    keep_inside_camera(b, me);
    let pointer = rd16(b, me + f::TARGET);
    if (pointer as i16) >= 0 {
        return moveq(c, 0);
    }
    let held = object(pointer);
    c.a[1] = held;
    let step = rd8(b, me + ANIM_MARK) as u32;
    if step == 0 {
        return moveq(c, 1);
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
    c.d[0] = if direction < 0 { 0xffff_ffff } else { 0xffff_0001 };
}

/// 0xff54 : compte à rebours (`+0x98`) puis, selon le nombre de joueurs et un tirage, état 0x020e0000 ou retour
/// à l'état 0x04040200 (objet actif).
fn countdown_then_state(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let left = rd8(b, me + 0x98).wrapping_sub(1);
    wr8(b, me + 0x98, left);
    let again = |b: &mut Cps2Bus| {
        wr8(b, me, 1);
        wr32(b, me + 4, 0x0404_0200);
    };
    if left as i8 > 0 {
        return again(b);
    }
    let row = ((rd8(b, A5 + 0x35) as u16) << 4).wrapping_add(((rd16(b, TICK) >> 2) << 1) as u16);
    let mask = b.fetch16(0xff9a_u32.wrapping_add(offset(row))) as u32;
    let roll = random(b) as u32 & 15;
    if mask >> roll & 1 == 0 {
        return again(b);
    }
    wr32(b, me + 4, 0x020e_0000);
}

// ---------------------------------------------------------------------------------------------
// Choix d'une cible selon un critère imposé (0x43bc)
// ---------------------------------------------------------------------------------------------

/// Critère de choix d'un joueur.
#[derive(Clone, Copy, PartialEq)]
enum Criterion {
    Nearest,
    LowestHp,
    Farthest,
}

/// Joueur ciblable au sens de 0x43ca : présent, d'état principal < 4, et visible sauf s'il porte
/// le drapeau `+0x182` (qui dispense du test de visibilité).
fn targetable(b: &mut Cps2Bus, player: u32) -> bool {
    rd8(b, player) != 0
        && (rd8(b, player + 0x182) != 0 || rd8(b, player + 1) & 2 == 0)
        && (rd8(b, player + 4) as i8) < 4
}

/// 0x43bc : choisit un joueur selon le critère D0 (0 le plus proche, 2 le moins de PV, 4 le plus
/// éloigné ; distance = |dx| + |dy|, à égalité le dernier l'emporte sauf pour le plus proche). A1 =
/// joueur (0 si aucun), copié en `+0x50` ; D5 = 2 si trouvé ; Z levé si aucun.
fn target_by_criterion(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let criterion = match c.d[0] as u16 {
        0 => Criterion::Nearest,
        2 => Criterion::LowestHp,
        _ => Criterion::Farthest,
    };
    let (x, depth) = (rd16(b, me + f::X), rd16(b, me + f::DEPTH));
    let mut best: u16 = match criterion {
        Criterion::Nearest => 0xffff,
        Criterion::LowestHp => 0x7fff,
        Criterion::Farthest => 0,
    };
    let mut found = 0;
    for k in 0..4 {
        let player = PLAYERS + k * PLAYER_STRIDE;
        if !targetable(b, player) {
            continue;
        }
        let (value, better) = match criterion {
            Criterion::LowestHp => {
                let hp = rd16(b, player + 0x62);
                (hp, hp as i16 <= best as i16)
            }
            _ => {
                let dx = (rd16(b, player + f::X).wrapping_sub(x) as i16).wrapping_abs() as u16;
                let dy = (rd16(b, player + f::DEPTH).wrapping_sub(depth) as i16).wrapping_abs() as u16;
                let distance = dx.wrapping_add(dy);
                (distance, if criterion == Criterion::Nearest { distance < best } else { distance >= best })
            }
        };
        if better {
            best = value;
            found = player;
        }
    }
    c.a[1] = found;
    wr16(b, me + f::TARGET, found as u16);
    c.d[5] = if found != 0 { 2 } else { 0 };
    logique(c, c.d[5], Size::Word);
}

// ---------------------------------------------------------------------------------------------
// Tirages par tables : attitude, riposte, esquive (0x788c, 0x79ce, 0x79f6, 0xb048, 0xb068, 0xf118,
// 0x10112). Une table de pointeurs (longs) est indexée par un octet de l'objet ; la ligne choisie est
// lue au hasard (octets) ou par le compteur d'images (masques de 32 bits, un bit tiré au hasard).
// ---------------------------------------------------------------------------------------------

/// Masque de 32 bits de la ligne `number` de la table `pointers`, choisi par le compteur d'images, et
/// tirage d'un de ses bits : vrai si le bit est levé. D0 = bit tiré, D2 = masque, A4 = ligne.
fn tick_mask_roll(c: &mut Cpu, b: &mut Cps2Bus, pointers: u32, number_at: u32) -> bool {
    let number = rd8(b, number_at);
    c.a[4] = rd32(b, pointers + 4 * number as u32);
    let tick = rd16(b, TICK);
    c.d[2] = rd32(b, c.a[4].wrapping_add(offset(tick << 2)));
    let roll = random(b) as u32 & 31;
    c.d[0] = roll;
    c.d[2] >> roll & 1 != 0
}

/// Octet tiré au hasard (32 choix) dans la ligne `number` de la table `pointers`. D0 = tirage, A4 = ligne.
fn random_entry(c: &mut Cpu, b: &mut Cps2Bus, pointers: u32, number_at: u32) -> u8 {
    let number = rd8(b, number_at);
    c.a[4] = rd32(b, pointers + 4 * number as u32);
    let roll = random(b) as u32 & 31;
    c.d[0] = roll;
    rd8(b, c.a[4] + roll)
}

/// 0x788c : `+0x6b` tiré dans la table de la ligne `+0x6a`.
fn draw_6b(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let v = random_entry(c, b, 0x78ae, me + 0x6a);
    wr8(b, me + 0x6b, v);
    logique(c, v as u32, Size::Byte);
}

/// Test d'écran de l'objet (marge D0) : retenue levée s'il est dans l'écran.
const ON_SCREEN: u32 = 0x1e92;

/// 0x79f6 : dans l'écran, `+0x6e` est tiré dans la table de la ligne `+0x6d`, sinon il vaut 1. D0 = 1.
fn draw_6e(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[0] = 0;
    call(c, b, ON_SCREEN);
    let v = if c.sr & SR_C != 0 { random_entry(c, b, 0x7a50, me + 0x6d) } else { 1 };
    wr8(b, me + 0x6e, v);
    moveq(c, 1);
}

/// 0x79ce : si le bit tiré dans le masque de la ligne `+0x6c` est levé, comme 0x79f6 ; sinon D0 = 0.
fn maybe_draw_6e(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if tick_mask_roll(c, b, 0x7a34, me + 0x6c) {
        draw_6e(c, b);
    } else {
        moveq(c, 0);
    }
}

/// 0xb048 et 0xb068 : la victime A1 pare-t-elle le coup de l'agresseur A0 (résolution des coups, `combat`) ?
/// Il faut qu'elle sache parer (`+0x67`), soit au sol (`+0xc` nul) et face à l'agresseur ; un bit est alors
/// tiré dans le masque de sa ligne `+0x68` (`+0x69` pour 0xb068). D0 = 1 si le coup est paré.
fn parries(c: &mut Cpu, b: &mut Cps2Bus, field: u32) {
    let (attacker, victim) = (c.a[0], c.a[1]);
    let facing = rd8(b, attacker + f::FACING);
    if rd8(b, victim + 0x67) == 0 || rd16(b, victim + 0x0c) != 0 || facing == rd8(b, victim + f::FACING) {
        return moveq(c, 0);
    }
    let parried = tick_mask_roll(c, b, 0xb0b0, victim + field);
    moveq(c, parried as i32);
}

fn parries_68(c: &mut Cpu, b: &mut Cps2Bus) {
    parries(c, b, 0x68)
}

fn parries_69(c: &mut Cpu, b: &mut Cps2Bus) {
    parries(c, b, 0x69)
}

/// 0xf118 : hors de l'écran, `+0x84` = 1 ; sinon `+0x84` = 0 et `+0x85` tiré dans la ligne `+0x8e`, bloc choisi
/// par le compteur d'images (32 octets par paire d'images).
fn draw_84(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[0] = 0;
    call(c, b, ON_SCREEN);
    if c.sr & SR_C == 0 {
        wr16(b, me + 0x84, 1);
        return;
    }
    let number = rd8(b, me + 0x8e) as u32;
    let block = (rd16(b, TICK) >> 1) << 5;
    c.a[4] = rd32(b, 0xf160 + 4 * number).wrapping_add(offset(block));
    let roll = random(b) as u32 & 31;
    c.d[0] = roll;
    let v = rd8(b, c.a[4] + roll);
    wr8(b, me + 0x85, v);
    wr8(b, me + 0x84, 0);
    logique(c, v as u32, Size::Word);
}

/// 0x10112 : `+0x68` = 8, ou, si `A5+0x5a` est posé, une valeur selon lui (plafonné à 4) et le nombre de
/// joueurs en vie.
fn set_68(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let level = rd8(b, A5 + 0x5a);
    c.d[0] = level as u32;
    let v = if level == 0 {
        8
    } else {
        c.d[1] = rd8(b, A5 + 0x35).wrapping_mul(4) as u32;
        let row = ((level.wrapping_sub(1) as i8).min(3) as u8).wrapping_add(c.d[1] as u8);
        c.d[0] = row as u32;
        crate::mem::prog8(b, 0x10144 + row as u32)
    };
    wr8(b, me + 0x68, v);
    logique(c, v as u32, Size::Byte);
}

// ---------------------------------------------------------------------------------------------
// Débris et butin (0x11a36, 0x1172e, 0xb648, 0xb780, 0xbad6, 0xe2f6)
// ---------------------------------------------------------------------------------------------


/// Alloue par `alloc` (0x2318, 0x23d8) : l'objet dans A1, ou `None` si le pool est plein.
fn alloc(c: &mut Cpu, b: &mut Cps2Bus, pool: &pools::Pool) -> Option<u32> {
    pool.alloc_68k(c, b).map(|o| o.0)
}

/// Mot suivant de la table A2 (espace de données).
fn next_word(c: &mut Cpu, b: &mut Cps2Bus) -> u16 {
    let v = rd16(b, c.a[2]);
    c.a[2] += 2;
    v
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

/// 0x1172e : D5 + 1-ème jeu de débris (table 0x1178a : nombre - 1, puis lignes de 4 mots : décalage x,
/// vitesse x, `+0x18`, `+0x58`), partant de la position de l'objet.
fn debris_set(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let index = (c.d[5] as u16).wrapping_mul(4);
    c.a[2] = crate::mem::prog32(b, 0x1178a_u32.wrapping_add(offset(index)));
    let count = next_word(c, b);
    let mirrored = rd8(b, me + f::FACING) != 0;
    for left in (0..=count).rev() {
        set_w(&mut c.d[5], left);
        let Some(fx) = alloc(c, b, &pools::EFFCT) else { continue };
        debris(b, fx, me);
        let z = rd16(b, me + 0x0c);
        wr16(b, fx + 0x0c, z);
        let depth = rd32(b, me + 0x10).wrapping_sub(1);
        wr32(b, fx + 0x10, depth);
        let (mut dx, mut vx) = (next_word(c, b), next_word(c, b));
        let v18 = next_word(c, b);
        wr16(b, fx + 0x18, v18);
        let v58 = next_word(c, b);
        wr16(b, fx + 0x58, v58);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
        }
        set_w(&mut c.d[1], dx);
        set_w(&mut c.d[2], vx);
        let x = rd16(b, fx + 0x08).wrapping_add(dx);
        wr16(b, fx + 0x08, x);
        wr16(b, fx + 0x14, vx);
    }
    set_w(&mut c.d[5], 0xffff);
}

/// 0x11a36 : cinq débris au sol (profondeur `A5+0x5aa`) ; table 0x11a9e, ou 0x11ada si D0.b est non nul
/// (lignes de 6 mots : décalage x, vitesse x, vitesse z, `+0x18`, décalage en profondeur, `+0x1c`).
/// Puis 0xede08.
fn ground_debris(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.a[2] = if c.d[0] as u8 == 0 { 0x11a9e } else { 0x11ada };
    let mirrored = rd8(b, me + f::FACING) != 0;
    for left in (0..=4u16).rev() {
        set_w(&mut c.d[5], left);
        let Some(fx) = alloc(c, b, &pools::EFFCT) else { continue };
        debris(b, fx, me);
        let ground = rd16(b, A5 + 0x5aa);
        wr16(b, fx + 0x10, ground);
        let (mut dx, mut vx, mut vz) = (next_word(c, b), next_word(c, b), next_word(c, b));
        let v18 = next_word(c, b);
        wr16(b, fx + 0x18, v18);
        let ddepth = next_word(c, b);
        let v1c = next_word(c, b);
        wr16(b, fx + 0x1c, v1c);
        if mirrored {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            vz = vz.wrapping_neg();
        }
        set_w(&mut c.d[1], dx);
        set_w(&mut c.d[2], vx);
        set_w(&mut c.d[3], vz);
        set_w(&mut c.d[4], ddepth);
        let x = rd16(b, fx + 0x08).wrapping_add(dx);
        wr16(b, fx + 0x08, x);
        let v14 = rd16(b, fx + 0x14).wrapping_add(vx);
        wr16(b, fx + 0x14, v14);
        wr16(b, fx + 0x16, vz);
        let depth = rd16(b, fx + 0x10).wrapping_add(ddepth);
        wr16(b, fx + 0x10, depth);
    }
    set_w(&mut c.d[5], 0xffff);
    call(c, b, 0xede08);
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

/// 0xb648 : une seule fois par objet (`+0x9d`), selon un bit tiré dans le masque `+0x9c` (table 0xb6fc), lâche
/// l'objet de genre 2 propre à son type (`+2`, table 0xb6e4 ; 0 : rien).
fn drop_once(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if rd8(b, me + 0x9d) != 0 {
        return;
    }
    let roll = random(b) as u32 & 31;
    c.d[0] = roll;
    c.a[1] = 0xb6fc;
    let mask = rd8(b, me + 0x9c) as u32;
    c.d[1] = rd32(b, 0xb6fc + 4 * mask);
    if c.d[1] >> roll & 1 == 0 {
        return;
    }
    let kind = rd8(b, me + 2) as u32;
    let variant = crate::mem::prog8(b, 0xb6e4 + kind);
    c.d[0] = variant as u32;
    if variant == 0 {
        return;
    }
    let Some(item) = alloc(c, b, &pools::ITEM) else { return };
    wr8(b, me + 0x9d, 1);
    tossed_item(b, me, item, 2, variant, (-0x40i16) as u16, false);
    let flip = random(b) & 1 != 0;
    c.d[0] = flip as u32;
    if flip {
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

/// 0xb780 : butin de l'objet (ligne `+0x80` de la table 0xb806, 16 tirages possibles ; négatif : rien).
fn drop_loot(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let row = rd8(b, me + 0x80) as u16;
    c.d[2] = row as u32;
    if row == 0 {
        return;
    }
    c.d[2] = (row << 4) as u32;
    for left in (0..loot_rolls(b)).rev() {
        c.d[5] = left as u32;
        let roll = (random(b) & 15).wrapping_add(row << 4);
        let variant = crate::mem::prog8(b, 0xb806_u32.wrapping_add(offset(roll)));
        set_w(&mut c.d[0], roll & 0xff00 | variant as u16);
        if (variant as i8) < 0 {
            return;
        }
        let Some(item) = alloc(c, b, &pools::ITEM) else { return };
        tossed_item(b, me, item, 0, variant, 0x40, left != 0);
    }
    set_w(&mut c.d[5], 0xffff);
}

/// 0xe2f6 : trésor de l'objet (ligne `+0x80` de la table 0xe372 ; chaque tirage désigne une liste d'objets
/// de genre 5 terminée par un octet négatif, numérotés à partir de 1 dans `+0x2f`).
fn drop_treasure(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let row = rd8(b, me + 0x80) as u16;
    c.d[2] = row as u32;
    if row == 0 {
        return;
    }
    c.d[2] = (row << 4) as u32;
    c.d[4] = 1;
    for left in (0..loot_rolls(b)).rev() {
        c.d[5] = left as u32;
        let roll = (random(b) & 15).wrapping_add(row << 4);
        set_w(&mut c.d[0], roll);
        let list = crate::mem::prog8(b, 0xe372_u32.wrapping_add(offset(roll)));
        c.d[1] = (list as u32) << 2 & 0xffff;
        if (list as i8) < 0 {
            return;
        }
        c.a[4] = rd32(b, 0xe642 + 4 * list as u32);
        loop {
            let variant = rd8(b, c.a[4]);
            c.a[4] += 1;
            set_b(&mut c.d[0], variant);
            if (variant as i8) < 0 {
                break;
            }
            let Some(item) = alloc(c, b, &pools::ITEM) else { return };
            wr8(b, item, 1);
            wr8(b, item + 2, 5);
            wr8(b, item + 3, variant);
            wr8(b, item + 0x2f, c.d[4] as u8);
            let rank = (c.d[4] as u8).wrapping_add(1);
            set_b(&mut c.d[4], rank);
            place_above(b, me, item);
        }
    }
    set_w(&mut c.d[5], 0xffff);
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

/// Parcourt les joueurs en jeu et en vie (`+0`, PV `+0x62`) comme les boucles de 0xbad6 (A1 sur le joueur, D6
/// décompte ; la classe dans D`class_reg`) ; `visit` rend vrai pour arrêter. Vrai si la boucle a été arrêtée.
fn scan_party(c: &mut Cpu, b: &mut Cps2Bus, class_reg: usize, mut visit: impl FnMut(&mut Cpu, u8) -> bool) -> bool {
    for k in 0..4u32 {
        let p = A5 + 0x5de + 0x200 * k;
        c.a[1] = p;
        c.d[6] = 3 - k;
        if rd8(b, p) == 0 || rd16(b, p + 0x62) == 0 {
            continue;
        }
        let class = rd8(b, p + 3) >> 1;
        c.d[class_reg] = class as u32;
        if visit(c, class) {
            return true;
        }
    }
    c.a[1] = A5 + 0x5de + 0x800;
    c.d[6] = 0xffff;
    false
}

/// Tire une entrée de `table` : D0 = `base` + (tirage & `mask`) × `scale` ; A1 = la table.
fn special_roll(c: &mut Cpu, b: &mut Cps2Bus, table: u32, base: u16, mask: u16, scale: u16) -> u32 {
    let roll = (random(b) & mask).wrapping_mul(scale).wrapping_add(base);
    c.d[0] = c.d[0] & 0xffff_0000 | roll as u32;
    c.a[1] = table;
    table.wrapping_add(offset(roll))
}

/// Début de la ligne de difficulté (`A5+0x49`) de `size` octets, gardé dans D3.
fn difficulty_row(c: &mut Cpu, b: &mut Cps2Bus, size: u16) -> u16 {
    let row = rd8(b, A5 + 0x49) as u16 * size;
    c.d[3] = row as u32;
    row
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

/// 0xbad6 : objet spécial lâché par l'objet A0 (ligne `+0x80` de la table 0xbf26, paires genre et variante, 16
/// tirages possibles ; genre négatif : rien), un ou deux tirages selon le nombre de joueurs. Une variante 0 à 3 de
/// genre 0 renvoie à des tables par difficulté (`A5+0x49`), certaines selon les classes des personnages en jeu
/// (clerc, elfe, magicien) ; certains objets sont réservés à un groupe d'un seul genre de lanceurs de sorts et
/// retirés sinon. L'objet ITEM est lancé en l'air comme le butin (0xb780). Registres de l'original.
fn drop_special(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    c.d[0] = 0;
    c.d[1] = 0;
    let row = rd8(b, me + 0x80) as u16;
    c.d[2] = row as u32;
    if row == 0 {
        return;
    }
    set_w(&mut c.d[2], row << 5);
    c.d[5] = (rd8(b, A5 + 0x35) > 3) as u32;
    let mut step = Draw::Row;
    loop {
        step = match step {
            Draw::Row => {
                let at = special_roll(c, b, special::ROWS, row << 5, 15, 2);
                let (kind, variant) = (rd8(b, at), rd8(b, at + 1));
                set_drawn(b, kind, variant);
                match (kind as i8, variant) {
                    (k, _) if k < 0 => return,
                    (0, 0) => Draw::Common,
                    (0, 1) => Draw::Pair,
                    (0, 2) => Draw::Kind2,
                    (0, _) => Draw::ByClass,
                    _ => Draw::Restrict,
                }
            }
            Draw::Common => {
                let row = difficulty_row(c, b, 64);
                let at = special_roll(c, b, special::COMMON, row, 31, 1);
                let caster = scan_party(c, b, 1, |_, class| matches!(class, CLERIC | ELF | MAGE));
                c.a[1] = special::COMMON;
                if caster {
                    let variant = rd8(b, at);
                    set_drawn(b, 7, variant);
                    Draw::Restrict
                } else {
                    let k = (c.d[0] as u16).wrapping_add(32);
                    set_w(&mut c.d[0], k);
                    redraw(b, at + 32, Draw::Pair, Draw::Restrict)
                }
            }
            Draw::Pair => {
                let row = difficulty_row(c, b, 64);
                let at = special_roll(c, b, special::PAIRS, row, 31, 2);
                let (kind, variant) = (rd8(b, at), rd8(b, at + 1));
                set_drawn(b, kind, variant);
                Draw::Restrict
            }
            Draw::Kind2 => {
                let row = difficulty_row(c, b, 16);
                let at = special_roll(c, b, special::KIND_2, row, 15, 1);
                let variant = rd8(b, at);
                set_drawn(b, 2, variant);
                Draw::Restrict
            }
            Draw::ByClass => {
                let row = difficulty_row(c, b, 64);
                let at = special_roll(c, b, special::BY_CLASS, row, 15, 1);
                c.d[3] = 0;
                scan_party(c, b, 1, |c, class| {
                    let bit = match class {
                        CLERIC => 1,
                        ELF => 2,
                        MAGE => 4,
                        _ => 0,
                    };
                    c.d[3] |= bit;
                    false
                });
                c.a[1] = special::BY_CLASS;
                let classes = c.d[3] as u8;
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
                    set_w(&mut c.d[1], shift);
                    let k = (c.d[0] as u16).wrapping_add(shift);
                    set_w(&mut c.d[0], k);
                    redraw(b, at + shift as u32, Draw::Common, Draw::Pair)
                }
            }
            Draw::Restrict => restrict(c, b),
            Draw::Spawn => {
                let (kind, variant) = drawn(b);
                let Some(item) = alloc(c, b, &pools::ITEM) else { return };
                tossed_item(b, me, item, kind, variant, 0x40, c.d[5] as u8 == 0);
                Draw::Next
            }
            Draw::Next => {
                let left = (c.d[5] as u16).wrapping_sub(1);
                set_w(&mut c.d[5], left);
                if left == 0xffff {
                    return;
                }
                Draw::Row
            }
        }
    }
}

/// 0xbcb4 : objets réservés. Genre 3 variante 5 : clercs et magiciens seulement (table 0xd776) ; genre 4 variante 15 :
/// magiciens seulement (0xda56) ; genre 4 variantes 8 et 9 : clercs seulement (0xdbc6) ; genre 4 variantes 10 et 11 :
/// selon les magiciens et les clercs en jeu (0xdd36). Un groupe qui ne convient pas garde l'objet tiré.
fn restrict(c: &mut Cpu, b: &mut Cps2Bus) -> Draw {
    let (kind, variant) = drawn(b);
    match (kind, variant) {
        (3, 5) => {
            c.d[1] = 0;
            let other = scan_party(c, b, 0, |c, class| match class {
                CLERIC => {
                    c.d[1] |= 1;
                    false
                }
                MAGE => {
                    c.d[1] |= 2;
                    false
                }
                _ => true,
            });
            if other || c.d[1] as u8 == 0 {
                return Draw::Spawn;
            }
            let shift = if c.d[1] as u8 == 1 { 0 } else { 16 };
            let row = difficulty_row(c, b, 32);
            let at = special_roll(c, b, special::CLERIC_OR_MAGE, row, 15, 1);
            let k = (c.d[0] as u16).wrapping_add(shift);
            set_w(&mut c.d[0], k);
            let variant = rd8(b, at + shift as u32);
            wr8(b, DRAWN_VARIANT, variant);
            match variant {
                0 => Draw::Next,
                1 => Draw::Common,
                2 => Draw::Pair,
                3 => Draw::ByClass,
                _ => Draw::Spawn,
            }
        }
        (4, 15) => only_class(c, b, MAGE, special::MAGES),
        (4, 8 | 9) => only_class(c, b, CLERIC, special::CLERICS),
        (4, 10 | 11) => {
            c.d[1] = 0;
            scan_party(c, b, 0, |c, class| {
                if class == MAGE {
                    c.d[1] |= 1;
                }
                if class == CLERIC {
                    c.d[1] |= 2;
                }
                false
            });
            let shift = match c.d[1] as u8 {
                0 => return Draw::Spawn,
                1 => {
                    c.d[0] = 0;
                    0
                }
                2 => 16,
                _ => 32,
            };
            set_w(&mut c.d[0], shift);
            let row = difficulty_row(c, b, 16) + shift;
            c.d[3] = row as u32;
            let at = special_roll(c, b, special::MAGE_OR_CLERIC, row, 15, 1);
            reroll_or_none(b, at)
        }
        _ => Draw::Spawn,
    }
}

/// Tous les joueurs en jeu sont de la classe `class` : nouvelle variante de genre 4 tirée dans `table`.
fn only_class(c: &mut Cpu, b: &mut Cps2Bus, class: u8, table: u32) -> Draw {
    if scan_party(c, b, 0, |_, k| k != class) {
        return Draw::Spawn;
    }
    let row = difficulty_row(c, b, 16);
    let at = special_roll(c, b, table, row, 15, 1);
    reroll_or_none(b, at)
}
fn reroll_or_none(b: &mut Cps2Bus, at: u32) -> Draw {
    let variant = rd8(b, at);
    set_drawn(b, 4, variant);
    if variant == 0 { Draw::Next } else { Draw::Spawn }
}

/// 0xe7fa : comme 0xe886, choisit le point de marche le plus proche autour de la cible et mémorise son
/// numéro (`+0x99`, -1 si aucun) ; loin de la caméra, marche vers un point lointain (0xf084).
fn choose_near_walk_point(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    if rd16(b, me + f::TARGET) == 0 {
        return;
    }
    let camera = if rd8(b, A5 + 0x36) != 0 { rd16(b, A5 + 0x556) } else { rd16(b, A5 + 0x506) };
    c.d[1] = c.d[1] & !0xffff | camera as u32;
    if rd16(b, me + f::X).wrapping_sub(camera) > 384 {
        return walk_far_point(c, b);
    }
    wr8(b, me + LAST_POINT, 0xff);
    let target = object(rd16(b, me + f::TARGET));
    c.a[1] = target;
    let set = rd8(b, me + POINT_SET) as u32;
    let mut at = rd32(b, WALK_POINTS + 4 * set);
    let last = rd16(b, at);
    at += 2;
    let (target_x, target_depth) = (rd16(b, target + f::X), rd16(b, target + f::DEPTH));
    wr16(b, me + GOAL_X, target_x);
    wr16(b, me + GOAL_DEPTH, target_depth);
    let mut best: u16 = 0x7fff;
    for point in (0..=last).rev() {
        let (x, depth) = (rd16(b, at).wrapping_add(target_x), rd16(b, at + 2).wrapping_add(target_depth));
        at += 4;
        let distance = manhattan(b, me, x, depth);
        if distance <= best {
            best = distance;
            wr16(b, me + GOAL_X, x);
            wr16(b, me + GOAL_DEPTH, depth);
            wr8(b, me + LAST_POINT, point as u8);
        }
        set_w(&mut c.d[0], x);
        set_w(&mut c.d[1], depth);
    }
    c.a[2] = at;
    set_w(&mut c.d[4], best);
    set_w(&mut c.d[6], 0xffff);
}

/// Routines d'atterrissage par type (longs du code) : ennemis (classe 2) aux indices 0..31, boss ensuite.
const LANDING_BY_KIND: u32 = 0x3e49c;

/// 0x3e480 : saute à la routine d'atterrissage propre au type (`+2`) de l'ennemi ou du boss A0, appelée par
/// les réactions communes aux coups (`families::reactions`). D0 = index × 4, A1 = routine, comme l'original.
fn landing_by_kind(c: &mut Cpu, b: &mut Cps2Bus) {
    let me = c.a[0];
    let base: u8 = if rd8(b, me + 0x28) == 2 { 0 } else { 32 };
    let index = base.wrapping_add(rd8(b, me + 2)) as u16 * 4;
    c.d[0] = index as u32;
    c.a[1] = crate::mem::prog32(b, LANDING_BY_KIND.wrapping_add(offset(index)));
    // les types sans routine propre pointent sur un simple RTS (0x3e2aa, 0x3bb9a)
    if c.a[1] == 0x3e2aa || c.a[1] == 0x3bb9a {
        return crate::mem::rts(c, b);
    }
    c.pc = c.a[1];
}

/// 0xfffc : `+0x98` (paramètre de comportement) lu dans la table 0x1000a selon le type `+2`.
fn behaviour_by_kind(c: &mut Cpu, b: &mut Cps2Bus) {
    let kind = rd8(b, c.a[0] + 2) as u32;
    c.d[0] = kind;
    let v = crate::mem::prog8(b, 0x1000a + kind);
    wr8(b, c.a[0] + 0x98, v);
    c.flags_logic(v as u32, Size::Byte);
}

/// 0x103ba : éclair d'écran (0x1ac68), secousse de 8 images (`A5+0x4be`) et effet 0xb141a, dont la
/// variante `+3` est remise à zéro ; rien de plus si l'effet n'a pas pu être créé (retenue).
fn flash_and_shake(c: &mut Cpu, b: &mut Cps2Bus) {
    call(c, b, 0x1ac68);
    wr16(b, A5 + 0x4be, 8);
    call(c, b, 0xb141a);
    if c.sr & SR_C != 0 {
        return;
    }
    wr8(b, c.a[1] + 3, 0);
    c.sr = c.sr & !0xf | SR_Z;
}

/// Native d'une fonction « corps » (sans RTS) : le RTS est ajouté.
macro_rules! leaf {
    ($entry:literal, $name:literal, $body:path) => {
        aid($entry, $name, |c, b| {
            $body(c, b);
            rts(c, b);
        })
    };
}

pub const NATIVES: &[Native] = &[
    Native { outputs: regs::d(0) | regs::CCR, ..leaf!(0xfffc, "ia_comportement_par_type", behaviour_by_kind) },
    Native { outputs: regs::a(1) | regs::CCR, ..leaf!(0x103ba, "ia_eclair_secousse", flash_and_shake) },
    Native { outputs: regs::a(0) | regs::d(1) | regs::d(2), ..leaf!(0xe886, "ia_point_de_marche", choose_walk_point) },
    Native { outputs: regs::a(0) | regs::d(1) | regs::d(2), ..leaf!(0xe914, "ia_point_atteint", walk_point_reached) },
    Native { outputs: regs::a(0) | regs::d(1) | regs::d(2), ..leaf!(0xeff2, "ia_point_proche", walk_near_point) },
    Native { outputs: regs::a(0) | regs::d(1) | regs::d(2), ..leaf!(0xf084, "ia_point_lointain", walk_far_point) },
    leaf!(0xfda2, "ia_tient_saisi", hold_seized),
    leaf!(0xff54, "ia_decompte_etat", countdown_then_state),
    leaf!(0x104a2, "ia_hors_ecran", outside_screen_range),
    leaf!(0x10574, "ia_cap_aleatoire", drift_heading),
    leaf!(0x105b6, "ia_palier_suivant", next_threshold),
    leaf!(0xfc44, "ia_attaque_touche", attack_hits_a_player),
    leaf!(0xfd64, "ia_saisit_joueur", seize_player),
    leaf!(0xfd8c, "ia_relache_joueur", release_player),
    leaf!(0x103e0, "ia_anim_variante", animation_by_variant),
    leaf!(0x10428, "ia_anim_maitre", animation_of_master),
    leaf!(0x10450, "ia_anim_marque", animation_of_mark),
    leaf!(0x10474, "ia_joueurs_en_vie", players_alive_flags),
    leaf!(0x1047a, "ia_decision_demi_tour", turn_decision),
    leaf!(0x104e0, "ia_meme_cote", same_side_flags),
    aid(0xa470, "ia_attitude", stance),
    aid(0x7f0c, "ia_devant", |c, b| in_front(c, b, true)),
    aid(0x7ef4, "ia_devant_etat_12", |c, b| in_front(c, b, false)),
    aid(0x7f19c, "ia_boite", box_around),
    aid(0x11654, "boss_poussee", push_amount),
    aid(0x1161c, "boss_pousse_joueur", push_player),
    Native { outputs: 0, ..aid(0x115e2, "boss_pousse_joueurs", push_players) },
    Native { outputs: regs::d(0) | regs::d(1) | regs::a(1) | regs::CCR, ..aid(0x67452, "ia_joueur_proche", nearest_player_in_box) },
    Native { outputs: regs::a(1) | regs::d(5) | regs::CCR, ..leaf!(0x43bc, "ia_cible_par_critere", target_by_criterion) },
    Native { outputs: regs::d(0) | regs::a(4) | regs::CCR, ..leaf!(0x788c, "ia_tirage_6b", draw_6b) },
    Native { outputs: regs::d(0) | regs::d(2) | regs::a(4) | regs::CCR, ..leaf!(0x79ce, "ia_tirage_6e_masque", maybe_draw_6e) },
    Native { outputs: regs::d(0) | regs::a(4) | regs::CCR, ..leaf!(0x79f6, "ia_tirage_6e", draw_6e) },
    Native { outputs: regs::d(0) | regs::d(2) | regs::a(4) | regs::CCR, ..leaf!(0xb048, "ia_parade", parries_68) },
    Native { outputs: regs::d(0) | regs::d(2) | regs::a(4) | regs::CCR, ..leaf!(0xb068, "ia_parade_b", parries_69) },
    Native { outputs: regs::d(0) | regs::a(4) | regs::CCR, ..leaf!(0xf118, "ia_tirage_84", draw_84) },
    Native { outputs: regs::d(0) | regs::d(1) | regs::CCR, ..leaf!(0x10112, "ia_valeur_68", set_68) },
    Native { outputs: regs::d(1) | regs::d(2) | regs::d(5) | regs::a(1) | regs::a(2), ..leaf!(0x1172e, "debris_jeu", debris_set) },
    Native { outputs: regs::ALL, ..leaf!(0x11a36, "debris_au_sol", ground_debris) },
    Native { outputs: regs::d(0) | regs::d(1) | regs::a(1), ..leaf!(0xb648, "butin_unique", drop_once) },
    Native { outputs: regs::d(0) | regs::d(2) | regs::d(5) | regs::a(1), ..leaf!(0xb780, "butin", drop_loot) },
    Native { outputs: regs::ALL & !regs::CCR, ..leaf!(0xbad6, "butin_special", drop_special) },
    Native { outputs: regs::d(0) | regs::d(1) | regs::d(2) | regs::d(4) | regs::d(5) | regs::a(1) | regs::a(4), ..leaf!(0xe2f6, "tresor", drop_treasure) },
    Native { outputs: regs::a(1) | regs::a(2), ..leaf!(0xe7fa, "ia_point_proche_numerote", choose_near_walk_point) },
    Native { outputs: regs::d(0) | regs::a(1), ..aid(0x3e480, "ia_atterrissage_par_type", landing_by_kind) },
];

