//! Boss 14 / 19, groupe `+5` = 2 : le combat. Le dragon marche vers un joueur ; selon la distance, la direction
//! et l'état de ses PV il se retourne, souffle ou attaque (`attacks`).
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0, 8 | marche (tir 0x20, 0x21) : tire au sort, sonde les boîtes de portée |
//! | 2 | demi-tour (animation 1) |
//! | 4 | immobile un instant, puis demi-tour si besoin |
//! | 6 | approche lente |
//!
//! Les sondes de portée rendent vrai quand le dragon doit réagir ; plusieurs ont des particularités de l'original
//! (résultat toujours faux, retour avec l'état « attente » écrit) reproduites telles quelles.

use crate::families::cx::Banc;
use super::*;
use crate::objects::Courant;

/// Table des octets de seuils des ripostes spéciales (0x901e0), indexée par le quart de PV restant.
const SPECIAL_LEVELS: u32 = 0x901e0;
/// Choix d'attaque (0x91096, 32 octets) et d'attaque à courte portée (0x9110c, 32 octets par niveau de difficulté).
const PICK_A: u32 = 0x91096;
const PICK_B: u32 = 0x9110c;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => walk(x, 0x20),
        2 => turning(x),
        4 => hold(x),
        6 => close_in(x),
        8 => walk(x, 0x21),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        wait(x);
    }
}

// --- marche ---------------------------------------------------------------------------------------

fn walk(x: &mut Cx, shot: u8) {
    x.set8(SHOT, shot);
    x.set8(HOVER, 1);
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        speed(x, 0x11);
        lib::draw_stance(x);
        return;
    }
    lib::pick_target(x);
    lib::approach_target(x);
    if !sub8(x, WALK_DELAY, 1).gt() {
        lib::draw_stance(x);
        if !sub8(x, WALK_STEPS, 1).pl() {
            if lib::maybe_draw_6e(x) {
                return chase(x);
            }
            lib::draw_6b(x);
        }
    }
    if special_due(x) {
        return attack_4(x);
    }
    // probe_near (0x9132e) ne rend jamais vrai : la sonde est lancée pour rien
    lib::target_in_front(x, -0x20, 0x40, 0x10);
    if probe_front(x) || probe_back(x) {
        return pick_attack_a(x);
    }
    if probe_far(x) {
        return;
    }
    if facing_away(x) {
        return turn(x);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// 0x901c2 : une riposte spéciale est-elle due ? Les PV restants (en quarts de `+0x60`) la déclenchent.
fn special_due<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    let d1 = x.u16(0x60) as u32;
    let d3 = (x.u16(0x62) as u32) << 2;
    let (mut d0, mut d2) = (-1i32, 0u32);
    // l'original boucle sans fin si `+0x60` est nul
    for _ in 0..0x20000 {
        d2 = d2.wrapping_add(d1);
        d0 += 1;
        if (d2 as i32) >= (d3 as i32) {
            break;
        }
    }
    let level = x.rom8(SPECIAL_LEVELS + (d0 as u16 as u32));
    let left = x.u8(SPECIALS);
    if left == 0 || (level as i8) > (left as i8) {
        return false;
    }
    x.set8(SPECIALS, left - 1);
    true
}

/// 0x91024 : le dragon tourne-t-il le dos à sa cible ? (sans cible : attend, et rend vrai comme l'original).
fn facing_away<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::TARGET) as i16) >= 0 {
        wait(x);
        return true;
    }
    let target = x.linked(f::TARGET);
    let mut d = target.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_sub(0x20) as i16;
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    d < 0
}

/// 0x9130c : un joueur est tout près derrière (boîte 0x7f0c) : le dragon réagit (0xac3a).
fn probe_far(x: &mut Cx) -> bool {
    if !lib::guard_in_front_strict(x, -0x60, 0x60, 0x10) {
        return false;
    }
    lib::pick_byte(x);
    true
}

/// 0x91348, 0x91390 : joueur dans la boîte `(d0, d1, d2)` et tirage favorable (0xa4c2 si la cible regarde du même
/// côté que le dragon, 0xa4b4 sinon). Sans cible, le dragon attend et la sonde rend vrai comme l'original.
fn probe_target(x: &mut Cx, d0: i16, d1: u16, d2: u16, no_target: impl Fn(u16) -> bool) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    if no_target(x.u16(f::TARGET)) {
        wait(x);
        return true;
    }
    let target = x.linked(f::TARGET);
    let same_side = target.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    lib::decision(x, same_side)
}

fn probe_front(x: &mut Cx) -> bool {
    probe_target(x, -0x50, 0x80, 0x10, |t| (t as i16) >= 0)
}

fn probe_back(x: &mut Cx) -> bool {
    probe_target(x, -0xc0, 0x30, 0x10, |t| t == 0)
}

/// 0x91050 : choisit une attaque (`+6` = 0 à 10) dans la table.
fn pick_attack_a(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let which = x.data8(PICK_A + r);
    x.set8(f::SUB2, which);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

// --- +6 = 2 : demi-tour -----------------------------------------------------------------------------

fn turning(x: &mut Cx) {
    if x.u16(ANIM_SET) != 1 {
        return stand(x);
    }
    if step(x) {
        return stand(x);
    }
    lib::pick_target(x);
    lib::approach_target(x);
    x.move_by_table_xy();
}

// --- +6 = 4 : immobile -------------------------------------------------------------------------------

fn hold(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        return rest(x);
    }
    if sub8(x, APPROACH, 1).pl() {
        if facing_away(x) {
            return turn(x);
        }
        x.anim_step();
        return;
    }
    x.set32(f::STATE, STAND);
}

// --- +6 = 6 : approche ---------------------------------------------------------------------------------

fn close_in(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set16(TIMER, 0x28);
        x.set8(VULNERABLE, 1);
        return speed(x, 0x21);
    }
    if !sub16(x, TIMER, 1).gt() {
        return stand(x);
    }
    x.set8(SHOT, 0x20);
    lib::approach_target(x);
    if facing_away(x) {
        return turn(x);
    }
    x.anim_step();
    x.move_by_table_xy();
}

// --- choix d'une attaque après un tir d'essai (0x910b6) ---------------------------------------------------------

/// 0x910b6 : un joueur est à portée de souffle ? Le tirage (table par niveau) est fait, puis l'original écrase
/// le résultat par l'état « debout » : seule la consommation du tirage reste.
pub(super) fn pick_attack_b(x: &mut Cx) {
    if (x.u16(f::TARGET) as i16) >= 0 {
        return wait(x);
    }
    if !lib::target_in_box(x, -0x40, 0x40, 0x20) {
        return stand(x);
    }
    let level = (var::RANK.get(x.b) >> 1) as u32;
    let base = PICK_B + (level << 5);
    let r = random(x) & 0x1f;
    let which = x.data8(base + r);
    x.set8(f::SUB2, which);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
    x.set32(f::STATE, STAND);
}
