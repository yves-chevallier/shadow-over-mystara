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
mod two;

/// Champs propres à ce type.
pub(self) mod g {
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
use crate::objects::pools;
use crate::objects::Courant;

/// Jeux d'animations d'un boss.
#[derive(Clone, Copy)]
pub(self) enum Set {
    A,
    B,
}

/// Ce qui distingue le boss de type 2 de celui de type 22 : le code est celui d'une même famille, copié avec d'autres
/// jeux d'animations et d'autres tables de la ROM.
pub(self) struct Kit {
    set_a: u32,
    set_b: u32,
    /// Tables d'attaque (32 octets par couple de niveaux) : portée courte, portée longue, fin d'attaque.
    close: u32,
    far: u32,
    next: u32,
    /// Points d'arrivée de la course (lignes de 6 octets : X, Z, angle, sens) et chances de riposte (lignes de
    /// `1 << chances_shift` octets).
    spot: u32,
    chances: u32,
    chances_shift: u32,
    /// Table de tirage de l'élan, table des réactions aux coups.
    dash: u32,
    reactions: u32,
}

const TWO_KIT: Kit = Kit {
    set_a: 0x1733ac,
    set_b: 0x1733e6,
    close: 0x7ea1e,
    far: 0x7ec1e,
    next: 0x7ee7c,
    spot: 0x7f25a,
    chances: 0x7e7a2,
    chances_shift: 2,
    dash: 0x7cd3e,
    reactions: 0x7e5e2,
};

const PART_KIT: Kit = Kit {
    set_a: 0x18492a,
    set_b: 0x184964,
    close: 0x9d280,
    far: 0x9d480,
    next: 0x9d6e0,
    spot: 0x9dabe,
    chances: 0x9d012,
    chances_shift: 4,
    dash: 0x9b5a0,
    reactions: 0x9ce66,
};

/// Le boss est-il le type 2 (et non le type 22) ?
pub(self) fn two(x: &mut Cx) -> bool {
    x.u8(2) == 2
}

pub(self) fn kit(x: &mut Cx) -> &'static Kit {
    if two(x) { &TWO_KIT } else { &PART_KIT }
}

/// Lance l'animation `n` du jeu `set` (0x1a32).
pub(self) fn start(x: &mut Cx, set: Set, n: u32) {
    let k = kit(x);
    x.anim_start(if matches!(set, Set::A) { k.set_a } else { k.set_b }, n as u16);
}

/// États posés d'un coup (`+4..+7`).
pub(self) mod state {
    pub const STAND: u32 = 0x0202_0000;
    pub const TURN: u32 = 0x0202_0200;
    pub const LEAP: u32 = 0x0202_0600;
    pub const IDLE: u32 = 0x020a_0000;
}

pub(self) mod call {
    pub const SPLASH: u32 = 0xaaa40;
}

// --- animations --------------------------------------------------------------------------------

/// Numéro d'animation `id` (+0xa2) ; lance l'animation `n` du jeu `set` si ce n'est pas la dernière lancée (+0xa3).
pub(self) fn pose(x: &mut Cx, set: Set, id: u8, n: u32) {
    x.set8(ANIM_ID, id);
    if x.u8(ANIM_ID) != x.u8(ANIM_LAST) {
        let id = x.u8(ANIM_ID);
        x.set8(ANIM_LAST, id);
        start(x, set, n);
    }
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
    lib::shock(x, d0, d1 as u16, d2 as u16, d3 as u16);
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
    if !lib::target_in_box(x, d0, d1, d2) || x.u16(f::TARGET) == 0 {
        return false;
    }
    let t = x.linked(f::TARGET);
    let same = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    if !lib::decision(x, same) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    lib::pick_byte(x);
    attack_from_table(x, table);
    true
}

pub(self) fn hunt_close(x: &mut Cx) -> bool {
    let table = kit(x).close;
    hunt_box(x, -0x40, 0x40, 8, table)
}
pub(self) fn hunt_far(x: &mut Cx) -> bool {
    let table = kit(x).far;
    hunt_box(x, -0xd0, 0x50, 0x50, table)
}

/// 0x9d24e et 0x9d254 : attaque tirée dans la ligne de la difficulté de `table` (32 octets par couple de niveaux).
pub(self) fn attack_from_table(x: &mut Cx, table: u32) {
    let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
    let r = rand_and(x, 0x1f);
    let step = x.data8(table + row + r);
    x.set8(f::SUB2, step);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x9d680 (0x7ee1e pour le type 2) : fin d'une attaque : debout, ou une nouvelle attaque si un joueur est tout près.
pub(self) fn next_attack(x: &mut Cx) {
    for k in 0..4u32 {
        let p = crate::objects::Obj(0xff85de + 0x200 * k);
        if p.u8(x.b, 0) == 0 {
            // le type 22 s'arrête au premier joueur absent, le type 2 passe au suivant
            if two(x) {
                continue;
            }
            break;
        }
        if reach(x, p, -0x38, 0x38, 0x18) {
            let row = ((var::RANK.get(x.b) >> 1) as u32) << 5;
            let r = rand_and(x, 0x1f);
            let table = kit(x).next;
            let step = x.data8(table + row + r);
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
pub(self) fn reach(x: &mut Cx, p: crate::objects::Obj, d0: i16, d1: u16, d2: u16) -> bool {
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
    let dx = x.u16(0x8).wrapping_sub(var::SCROLL_X.get(x.b));
    let dz = x.u16(0x10).wrapping_sub(var::SCROLL_Y.get(x.b));
    let row = if (dx as i16) > 0xc0 {
        if (dz as i16) > 0x80 { 0x12 } else { 0xc }
    } else if (dz as i16) > 0x80 {
        6
    } else {
        0
    };
    let base = kit(x).spot;
    let sx = x.rom16(base + row).wrapping_add(var::SCROLL_X.get(x.b));
    x.set16(SPOT_X, sx);
    let sz = x.rom16(base + 2 + row).wrapping_add(var::SCROLL_Y.get(x.b));
    x.set16(SPOT_Z, sz);
    let angle = x.rom8(base + 4 + row);
    x.set8(0x2d, angle);
    let spin = x.rom8(base + 5 + row);
    x.set8(SPIN, spin);
}

/// 0x9db72 : cinq poussières en ligne sous le boss qui retombe.
pub(self) fn dust_line(x: &mut Cx) {
    for dz in [-0x30i16, -0x18, 0, 0x18, 0x30] {
        lib::shock(x, 1, 0, dz as u16, 4);
    }
}

/// 0x7f328 et 0x7f3a0 : poussières en ligne, vers la droite (le boss regarde à gauche) ou vers la gauche.
pub(self) fn dust_trail(x: &mut Cx, right: bool) {
    const RIGHT: [i16; 6] = [-0x10, 8, 0x20, 0x38, 0x50, 0x68];
    const LEFT: [i16; 6] = [-0x68, -0x58, -0x38, -0x20, -0x8, 0x10];
    for dz in if right { RIGHT } else { LEFT } {
        lib::shock(x, 1, 0, dz as u16, 4);
    }
}

/// Cinq appels de 0x9eca4 (éclats), D5 = 4 à 0 comme la boucle `dbf` de l'original.
pub(self) fn scatter(x: &mut Cx) {
    for i in (0..5u32).rev() {
        x.c.d[5] = i;
        eshl_debris(x, i as u8);
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
    x.tick_counters();
    if two(x) {
        two::spawn_scene_controller(x);
    }
    match x.u8(f::STATE) {
        st::INIT if two(x) => two::init(x),
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => {
            if !two(x) {
                x.set_a5_u8(0x50, 0);
            }
            x.free(&pools::BOSS);
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
    lib::create_life_bar(x);
    lib::initial_hp_boss(x);
    for (off, v) in [(0x70, 0), (0x7e, 0x23), (0x7f, 0), (0x8e, 3), (0x78, 0xff), (0x68, 8), (0x69, 8), (0x6a, 3), (0x6c, 1), (0x6d, 8)] {
        x.set8(off, v);
    }
    let speed = if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 { 7 } else { 6 };
    x.set8(0x72, speed);
    x.set8(0x73, speed);
    for (off, v) in [(0x74, 5), (0x75, 0x3c), (0x76, 6), (0x81, 0x12), (0x80, 0x17)] {
        x.set8(off, v);
    }
    x.set16(GUARD, 0xb4);
    x.set16(GUARD_TIMER, 0xf0);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(RAGE, 0xff);
    x.set8(SECOND_FX, 0);
    x.set16(SPOT_TIMER, 0x10);
    x.set8(0x93, 0);
    // `jmp 0x1a32` (registres de l'original laissés au gestionnaire suivant)
    let set = kit(x).set_a;
    x.c.a[0] = x.o.0;
    crate::objects::anim::start_68k(x.c, x.b, x.o, set, 0);
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
    } else if var::PAUSE.get(x.b) == 0 {
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
        let fuse = if two(x) { 0x46 } else { 0x50 };
        x.set8(ARM_TIMER, fuse);
        x.set32(f::STATE, 0x0204_1600);
    }
    if expired || fire {
        x.set16(GUARD, 0xb4);
        x.set16(GUARD_TIMER, 0xf0);
    }
    lib::keep_on_screen(x, 0x30);
    lib::collide_terrain(x);
    x.reinsert();
}

fn group(x: &mut Cx) {
    match x.u8(f::SUB) {
        0 if two(x) => two::arrive(x),
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
    0x9aeaa "boss_22" run,
    0x7c5a8 "boss_2" run,
}
