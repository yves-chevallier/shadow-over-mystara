//! Boss de type 22 (0x9aeaa) : la créature que le boss de type 2 libère à sa mort (h), à qui il passe le relais
//! (`+0x42` = son hôte). Voir `docs/familles/boss.md`.
//!
//! Même gabarit que le type 16 (`boss_16`) : groupes `+5` pairs, étapes `+6` et `+7`, états posés d'un coup en
//! longs, deux jeux d'animations (corps 0x18492a, attaques 0x184964). Sa seconde forme (`+0xb5` levé) a d'autres
//! animations et d'autres attaques. Les réactions aux coups sont les routines communes (`reactions.rs`).
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `moves` | arrivée : tombe du haut de l'écran |
//! | 2 | `moves` | combat : attente (`+6` = 0 et 8), demi-tour (2), bond vers la cible (6), charge (4) |
//! | 4 | `attacks` | attaques : six coups (`+6` = 0 à 22) |
//! | 6 | `moves` | course vers un point de l'écran (`+6` = 0 et 2), saut (4 et 6) |
//! | 8 | `moves` | idem après une attaque |
//! | 10 | `moves` | pause |
//! | 12 | `moves` | un joueur saisi (`+0x50`) : morsures |
//! | 14 | `moves` | transformation en seconde forme |
//! | état 4 | `hit` | réactions aux coups, recul, à terre, mort |

use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod hit;
mod moves;

/// Champs propres à ce type.
pub(self) mod g {
    pub const KEY: u32 = 0x33;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Animation (numéro) en cours et dernière lancée.
    pub const ANIM_ID: u32 = 0xa2;
    pub const ANIM_LAST: u32 = 0xa3;
    /// Minuteries (mots, ou octets sur la même adresse).
    pub const TIMER: u32 = 0xa0;
    pub const TIMER2: u32 = 0xaa;
    pub const SPOT_TIMER: u32 = 0xbc;
    /// État à reprendre après un demi-tour : long en `+0xa6` (états) et octet en `+0xa5` (étape `+6`).
    pub const SAVED_STEP: u32 = 0xa5;
    pub const SAVED_STATE: u32 = 0xa6;
    /// Point visé (mots), sens de rotation de la trajectoire.
    pub const SPOT_X: u32 = 0xb0;
    pub const SPOT_Z: u32 = 0xb2;
    pub const SPIN: u32 = 0xb4;
    /// Seconde forme.
    pub const SECOND: u32 = 0xb5;
    pub const SECOND_FX: u32 = 0xbf;
    pub const EFFECT_COUNT: u32 = 0xb7;
    pub const ARM_TIMER: u32 = 0xbb;
    pub const HIT_TICK: u32 = 0xba;
    pub const BITES: u32 = 0xbe;
    pub const RAGE: u32 = 0xae;
    /// Délai (mot) avant qu'un joueur trop proche déclenche la riposte, et son compte à rebours.
    pub const GUARD: u32 = 0x46;
    pub const GUARD_TIMER: u32 = 0x5c;
    /// Hôte (boss de type 2).
    pub const HOST: u32 = 0x42;
}
use g::*;

pub(self) const SET_A: u32 = 0x18492a;
pub(self) const SET_B: u32 = 0x184964;

/// États posés d'un coup (`+4..+7`).
pub(self) mod state {
    pub const STAND: u32 = 0x0202_0000;
    pub const TURN: u32 = 0x0202_0200;
    pub const LEAP: u32 = 0x0202_0600;
    pub const IDLE: u32 = 0x020a_0000;
}

pub(self) mod call {
    pub const FREE_BOSS: u32 = 0x2234;
    pub const SPEED: u32 = 0x1b64;
    pub const MOVE_TABLE_XY: u32 = 0x1cb4;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const MOVE_YZ: u32 = 0x1dd4;
    pub const ASK_TARGET: u32 = 0xe76c;
    pub const PICK_FAR: u32 = 0x44ee;
    pub const FACE_TARGET: u32 = 0xa470;
    pub const AIM: u32 = 0x788c;
    pub const AIM_DONE: u32 = 0x79ce;
    pub const REROLL: u32 = 0xac3a;
    pub const EDGE_PAIR: u32 = 0xa4b4;
    pub const EDGE_SAME: u32 = 0xa4c2;
    pub const GRAB_FIND: u32 = 0xfc44;
    pub const GRAB: u32 = 0xfd64;
    pub const RELEASE: u32 = 0xfd8c;
    pub const HOLD: u32 = 0xfda2;
    pub const IN_ARENA: u32 = 0xfbb0;
    pub const BLOOD: u32 = 0xedd96;
    pub const SPLASH: u32 = 0xaaa40;
    pub const DUST: u32 = 0xb10c8;
    pub const MESSAGE: u32 = 0x289c;
    pub const NEAR_PLAYER: u32 = 0x7f19c;
    pub const FAR_BOX: u32 = 0x7f0c;
}

// --- animations --------------------------------------------------------------------------------

/// Numéro d'animation `id` (+0xa2) ; lance l'animation `n` du jeu `set` si ce n'est pas la dernière lancée (+0xa3).
pub(self) fn pose(x: &mut Cx, set: u32, id: u8, n: u32) {
    x.set8(ANIM_ID, id);
    if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
        let id = x.u8(ANIM_ID);
        x.set8(ANIM_LAST, id);
        x.anim_start(set, n);
    }
}

/// Idem sans test : l'animation est toujours relancée, le numéro mémorisé.
pub(self) fn pose_always(x: &mut Cx, set: u32, id: u8, n: u32) {
    x.set8(ANIM_ID, id);
    x.set8(ANIM_LAST, id);
    x.anim_start(set, n);
}

/// Réglages sonores `+0x44` et `+0x45` posés avec l'animation.
pub(self) fn sounds(x: &mut Cx, a: u8, b: u8) {
    x.set8(0x44, a);
    x.set8(0x45, b);
}

/// Animation du corps ou des attaques selon qu'il est en seconde forme.
pub(self) fn second_form(x: &mut Cx) -> bool {
    x.u8(SECOND) != 0
}

pub(self) fn next7(x: &mut Cx) {
    x.add8(0x7, 2);
}
pub(self) fn next6(x: &mut Cx) {
    x.add8(f::SUB2, 2);
}

/// Une poussière / explosion `D0 .. D3` (0xb10c8) tous les huit appels (`+0xb7`).
pub(self) fn rubble(x: &mut Cx, d0: u32, d1: u32, d2: u32, d3: u32) {
    x.add8(EFFECT_COUNT, 1);
    let n = x.u8(EFFECT_COUNT) & 7;
    x.set8(EFFECT_COUNT, n);
    if n != 0 {
        return;
    }
    x.c.d[0] = d0;
    x.c.d[1] = d1;
    x.c.d[2] = d2;
    x.c.d[3] = d3;
    x.call(call::DUST);
}

/// 0x9d8e0 : la cible est-elle du mauvais côté (le boss ne la regarde pas) ?
pub(self) fn needs_turn(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let ahead = (t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16) > 0;
    let wanted = if ahead { 0x20 } else { 0 };
    wanted != x.u8(f::FLIP)
}

/// 0x9d928 : la cible est-elle loin à la fois en profondeur et en X ?
pub(self) fn target_far(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let dz = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
    if dz <= 0x40 {
        return false;
    }
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
    dx > 0x180
}

/// 0x9b30a et 0x9b384 : une cible dans la boîte, avec des coups possibles (0xa4b4 / 0xa4c2) ; le boss fonce ou attaque.
fn hunt_box(x: &mut Cx, d0: i16, d1: u16, d2: u16, table: u32) -> bool {
    if !target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let same = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    x.c.a[1] = t.0;
    x.call(if same { call::EDGE_SAME } else { call::EDGE_PAIR });
    if zero(x) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    x.call(call::REROLL);
    attack_from_table(x, table);
    true
}

pub(self) fn hunt_close(x: &mut Cx) -> bool {
    hunt_box(x, -0x40, 0x40, 8, 0x9d280)
}
pub(self) fn hunt_far(x: &mut Cx) -> bool {
    hunt_box(x, -0xd0, 0x50, 0x50, 0x9d480)
}

/// 0x9d24e et 0x9d254 : attaque tirée dans la ligne de la difficulté de `table` (32 octets par couple de niveaux).
pub(self) fn attack_from_table(x: &mut Cx, table: u32) {
    let row = ((x.a5_u16(var::DIFFICULTY) >> 1) as u32) << 5;
    let r = rand_and(x, 0x1f);
    let step = x.data8(table + row + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x9d680 : fin d'une attaque : debout, ou une nouvelle attaque (table 0x9d6e0) si un joueur est tout près.
pub(self) fn next_attack(x: &mut Cx) {
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            break;
        }
        x.c.a[2] = p.0;
        set_dw(x, 0, (-0x38i16) as u16);
        set_dw(x, 1, 0x38);
        set_dw(x, 2, 0x18);
        x.call(call::NEAR_PLAYER);
        if !zero(x) {
            let row = ((x.a5_u16(var::DIFFICULTY) >> 1) as u32) << 5;
            let r = rand_and(x, 0x1f);
            let step = x.data8(0x9d6e0 + row + r);
            x.set8(f::SUB2, step);
            x.set16(f::STATE, 0x0208);
            x.set8(0x7, 0);
            return;
        }
    }
    x.set32(f::STATE, state::STAND);
}

/// 0x9d9d0 : un joueur tout près (boîte ±0x28 en X, ±8 en profondeur) à la hauteur du sol ?
pub(self) fn player_within_reach(x: &mut Cx) -> bool {
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            continue;
        }
        if reach(x, p, -0x28, 0x28, 8) {
            return true;
        }
    }
    false
}

/// 0x9da00 : `p` est-il dans la boîte (décalage `d0`, demi-largeur `d1`, demi-profondeur `d2`) du boss, au sol ?
fn reach(x: &mut Cx, p: crate::objects::Obj, d0: i16, d1: u16, d2: u16) -> bool {
    if p.u16(x.b, 0xc) != 0 {
        return false;
    }
    let mut off = d0 as u16;
    if x.u8(f::FLIP) != 0 {
        off = off.wrapping_neg();
    }
    let dx = off.wrapping_add(x.u16(0x8)).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(d1);
    if dx > d1.wrapping_mul(2) {
        return false;
    }
    let dz = x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)).wrapping_add(d2);
    dz <= d2.wrapping_mul(2)
}

/// 0x9da5c : choisit le point d'arrivée de la course (`+0xb0`, `+0xb2`), l'angle et le sens de rotation, d'après la
/// position du boss dans l'écran (table 0x9dabe : X, Z, angle, sens).
pub(self) fn choose_spot(x: &mut Cx) {
    let dx = x.u16(0x8).wrapping_sub(x.a5_u16(var::SCROLL_X));
    let dz = x.u16(0x10).wrapping_sub(x.a5_u16(var::SCROLL_Y));
    let row = if (dx as i16) > 0xc0 {
        if (dz as i16) > 0x80 { 0x12 } else { 0xc }
    } else if (dz as i16) > 0x80 {
        6
    } else {
        0
    };
    let sx = x.rom16(0x9dabe + row).wrapping_add(x.a5_u16(var::SCROLL_X));
    x.set16(SPOT_X, sx);
    let sz = x.rom16(0x9dac0 + row).wrapping_add(x.a5_u16(var::SCROLL_Y));
    x.set16(SPOT_Z, sz);
    let angle = x.rom8(0x9dac2 + row);
    x.set8(0x2d, angle);
    let spin = x.rom8(0x9dac3 + row);
    x.set8(SPIN, spin);
}

/// 0x9db72 : cinq poussières en ligne sous le boss qui retombe.
pub(self) fn dust_line(x: &mut Cx) {
    for dz in [-0x30i16, -0x18, 0, 0x18, 0x30] {
        x.c.d[0] = 1;
        set_dw(x, 1, 0);
        set_dw(x, 2, dz as u16);
        set_dw(x, 3, 4);
        x.call(call::DUST);
    }
}

/// Cinq appels de 0x9eca4 (éclats), D5 = 4 à 0 comme la boucle `dbf` de l'original.
pub(self) fn scatter(x: &mut Cx) {
    for i in (0..5u32).rev() {
        x.c.d[5] = i;
        x.call(0x9eca4);
    }
    x.c.d[5] |= 0xffff;
}

/// 0x9db28 : quinze couleurs de palette écrites en 0x91c340 ; le boss prend le décalage de palette 0x1a.
fn set_palette(x: &mut Cx) {
    const COLORS: [u16; 15] = [
        0xf333, 0xfdef, 0xfaed, 0xf9b9, 0xf575, 0xf244, 0xffff, 0xfddd, 0xfa9a, 0xf656, 0xf5cb, 0xf397, 0xf075, 0xf053, 0xf031,
    ];
    for (i, c) in COLORS.iter().enumerate() {
        crate::mem::wr16(x.b, 0x91c340 + 2 * i as u32, *c);
    }
    x.set8(0x2b, 0x1a);
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.call(rom::TICK_COUNTERS);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => {
            x.set_a5_u8(0x50, 0);
            x.tail(call::FREE_BOSS);
        }
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    set_palette(x);
    let r = random(x);
    x.set8(EFFECT_COUNT, r as u8);
    x.set8(f::BOX_SET, 0x22);
    x.call(0xd5ef8);
    x.call(0x453c);
    for (off, v) in [(0x70, 0), (0x7e, 0x23), (0x7f, 0), (0x8e, 3), (0x78, 0xff), (0x68, 8), (0x69, 8), (0x6a, 3), (0x6c, 1), (0x6d, 8)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && x.a5_u8(var::PLAYERS_ALIVE) != 1 { 7 } else { 6 };
    x.set8(0x72, speed);
    x.set8(0x73, speed);
    for (off, v) in [(0x74, 5), (0x75, 0x3c), (0x76, 6), (0x81, 0x12), (0x80, 0x17)] {
        x.set8(off, v);
    }
    x.set16(GUARD, 0xb4);
    x.set16(GUARD_TIMER, 0xf0);
    x.set_a5_u8(0x5a, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    pick_target(x);
    x.call(call::FACE_TARGET);
    x.call(call::AIM);
    x.set8(RAGE, 0xff);
    x.set8(SECOND_FX, 0);
    x.set16(SPOT_TIMER, 0x10);
    x.set8(0x93, 0);
    x.c.d[0] = 0;
    x.c.a[4] = SET_A;
    x.tail(0x1a32);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x9afbc : un groupe par image (hors gel et pause) ; un compte à rebours (`+0x5c`, reprise à 0xf0) déclenche, quand
/// le boss est au sol et que `+0x46` désigne un objet, l'état 0x0204_1600 (colère).
fn active(x: &mut Cx) {
    if x.u16(f::TARGET) == 0 {
        pick_target(x);
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.a5_u8(var::PAUSE) == 0 {
        group(x);
        if tst8(x, ARM_TIMER).gt() {
            sub8(x, ARM_TIMER, 1);
            x.set8(0x38, 0);
        }
    }
    let expired = sub16(x, GUARD_TIMER, 1).le();
    let fire = !expired && (x.u16(GUARD) as i16) < 0 && x.u16(0xc) == 0;
    if fire {
        x.set8(0x86, 0);
        x.set16(0x84, 0);
        x.set8(0x49, 0);
        x.set8(0, 1);
        x.set8(ARM_TIMER, 0x50);
        x.set32(f::STATE, 0x0204_1600);
    }
    if expired || fire {
        x.set16(GUARD, 0xb4);
        x.set16(GUARD_TIMER, 0xf0);
    }
    x.c.d[0] = 0x30;
    x.call(call::IN_ARENA);
    x.call(SHADOW);
    x.reinsert();
}

fn group(x: &mut Cx) {
    match x.u8(f::SUB) {
        0 => moves::arrive(x),
        2 => moves::fight(x),
        4 => attacks::run(x),
        6 => moves::run_to_spot(x),
        8 => moves::run_to_spot_after_attack(x),
        10 => moves::paused(x),
        12 => moves::holding(x),
        14 => moves::transform(x),
        _ => {}
    }
}

handlers! {
    0x9aeaa "boss_22" 1200 run,
}
