//! Boss 3, état actif hors attaques : arrivée (`+5` = 0), combat (2), ronde (6), envol (8), attente (10), joueur
//! emporté (12), riposte (14).

use super::*;
use crate::objects::Courant;

// --- groupe 0 : arrivée ------------------------------------------------------------------------

/// 0x7f5b8 : la variante 0 descend du haut de l'écran (en cinématique) ; les autres combattent tout de suite.
pub(super) fn intro(x: &mut Cx) {
    if x.u8(0x3) != 0 {
        return enter_fight(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            let cam = camera(x);
            let z = cam.u16(x.b, 0xc).wrapping_add(0x80);
            x.set16(0x10, z);
            x.set16(0xc, (-0x80i16) as u16);
            stop(x);
            x.set16(0x18, 0x380);
            body_anim(x, 0xc);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 {
                return hidden(x);
            }
            let cam = camera(x);
            let px = cam.u16(x.b, 0x8).wrapping_add(0x100);
            x.set16(0x8, px);
            lib::step(x);
            x.move_z();
            if (x.u16(0xc) as i16) < 0x80 {
                return hidden(x);
            }
            next6(x);
            let z = cam.u16(x.b, 0xc).wrapping_add(0x40);
            x.set16(0x10, z);
            x.set16(0xc, 0xc0);
            stop(x);
            x.set16(0x18, (-0x600i16) as u16);
            x.set16(0x1a, (-0x60i16) as u16);
            x.set8(TIMER, 0x20);
            body_anim(x, 0xd);
            x.unlink();
            end_frame(x, 0x50);
        }
        4 => {
            x.set_bit(0x1, 1, true);
            if sub8(x, TIMER, 1).pl() {
                return end_frame(x, 0x50);
            }
            next6(x);
            lib::sound(x, 0xe1370);
            x.set_bit(0x1, 1, false);
            descend(x);
        }
        6 => descend(x),
        _ => {
            if var::PAUSE.get(x.b) == 0 && lib::step(x) {
                return enter_fight(x);
            }
            end_frame(x, 0x50);
        }
    }
}

/// 0x7f67a : encore hors de l'écran : retenue, remise dans la liste du premier plan.
fn hidden(x: &mut Cx) {
    lib::keep_on_screen(x, 0x50);
    x.reinsert_foreground();
}

/// 0x7f6a8 : descente jusqu'au sol, puis cri.
fn descend(x: &mut Cx) {
    if var::PAUSE.get(x.b) == 0 {
        lib::step(x);
        x.move_z();
        if (x.u16(0xc) as i16) <= 0 {
            x.set16(0xc, 0);
            next6(x);
            stop(x);
            crate::families::effects_creators::scenery_0(x.b, x.o);
            lib::shake_54(x, 0x30);
            body_anim(x, 0xe);
        }
    }
    end_frame(x, 0x50);
}

/// 0x7f712 : immunités du type, puis le combat commence dans la même image (approche).
fn enter_fight(x: &mut Cx) {
    lib::set_immunities(x);
    x.set32(f::STATE, 0x0202_0400);
    run(x);
}

// --- groupe 2 : combat -------------------------------------------------------------------------

/// 0x7f720 : une étape du combat ; sans joueur en vie, attente (ou montée si le palier de PV change).
pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => hover(x),
        2 => turn(x),
        4 | 8 => approach(x),
        6 => sweep(x),
        10 => climb(x),
        12 => hold_high(x),
        14 => land(x),
        _ => patrol(x),
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        return;
    }
    idle_alone(x);
    if wound_changed(x) {
        set_steps(x, 0x0c00);
    }
}

/// 0x7f75e : vol stationnaire au sol ; en l'air ou blessé, il remonte.
fn hover(x: &mut Cx) {
    if (x.u16(0xc) as i16) > 0 || wound_changed(x) {
        x.set8(INVULNERABLE, 1);
        return set_steps(x, 0x0a00);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(FLYING, 0);
        x.set8(VULNERABLE, 1);
        return body_anim(x, 0);
    }
    if !sub8(x, IDLE_COUNT, 1).gt() {
        return x.set32(f::STATE, 0x0202_0400);
    }
    if target_behind(x) {
        x.set32(SAVED_STATE, 0x0202_0400);
        x.set32(f::STATE, 0x0202_0200);
    }
    lib::step(x);
}

/// 0x7f7ca : demi-tour (en vol : `+0xa6` images au plus), puis approche.
fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        next7(x);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(FLYING, 0);
        x.set8(VULNERABLE, 1);
        x.set8(Obj::SPEED_INDEX, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(TIMER, 0x20);
            x.set8(Obj::SPEED_INDEX, 8);
        }
        return body_anim(x, 2);
    }
    if !lib::step(x) && sub8(x, TIMER, 1).gt() {
        return;
    }
    set_steps(x, 0x0400);
    body_anim(x, 0);
}

/// Toutes les `+0x71` images, nouvelle cible (0xa470, puis 0x424e ou 0x44ee au hasard) ; au bout de `+0x6b`
/// choix, le compte des pas (0x788c, 0x79ce) peut renvoyer au vol stationnaire. Faux s'il n'y a plus de cible
/// (attente), ou si l'étape a changé.
fn retarget(x: &mut Cx, hover_steps: u16) -> Option<bool> {
    if sub8(x, WALK_DELAY, 1).pl() {
        return Some(true);
    }
    lib::draw_stance(x);
    let r = rand_and(x, 0xf);
    let ok = if 0xffc0u32 >> r & 1 != 0 { pick_target(x) } else {
        lib::random_player(x)
    };
    if !ok {
        idle_alone(x);
        return None;
    }
    if sub8(x, WALK_STEPS, 1).ne() {
        return Some(true);
    }
    lib::draw_6b(x);
    if !lib::maybe_draw_6e(x) {
        return Some(true);
    }
    set_steps(x, hover_steps);
    Some(false)
}

/// 0x7f842 : approche de la cible au sol ; selon les boîtes devant lui, il attaque.
fn approach(x: &mut Cx) {
    if wound_changed(x) {
        return set_steps(x, 0x0a00);
    }
    if !pick_target(x) {
        return idle_alone(x);
    }
    if target_far(x) {
        return x.set32(f::STATE, 0x0202_0600);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(FLYING, 0);
        x.set8(VULNERABLE, 1);
        body_anim(x, 3);
        return speed(x, 0x15);
    }
    let aim = if x.u8(f::SUB2) == 0 { 0x2f } else { 0x30 };
    x.set8(AIM, aim);
    lib::approach_target(x);
    match retarget(x, 0) {
        None | Some(false) => return,
        Some(true) => {}
    }
    x.set8(0xaf, 2);
    if lib::target_in_box(x, -0x20, 0x20, 0x10) {
        lib::pick_byte(x);
        return choose_attack(x, false);
    }
    if lunge_box(x, -0x50, 0x20, false) || lunge_box(x, -0xc0, 0x30, true) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x60, 0x60, 0x10) {
        return lib::pick_byte(x);
    }
    lib::step(x);
    x.move_by_table_xy();
    if target_behind(x) {
        x.set32(SAVED_STATE, 0x0202_0000);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x7f984 / 0x7f9fe : cible dans la boîte (et du bon côté : 0xa4c2 si elle regarde comme le boss, 0xa4b4 sinon) :
/// ronde (6 chances sur 16) ou attaque. Vrai si l'étape a changé.
fn lunge_box(x: &mut Cx, d0: i16, d1: u16, far: bool) -> bool {
    if !lib::target_in_box(x, d0, d1, 0x10) {
        x.c.d[0] = 0;
        return false;
    }
    if x.u16(f::TARGET) == 0 {
        idle_alone(x);
        return true;
    }
    let t = load_target(x);
    let side = t.u8(x.b, f::FLIP);
    x.c.d[0] = x.c.d[0] & !0xff | side as u32;
    let same_side = side == x.u8(f::FLIP);
    if !lib::decision(x, same_side) {
        x.c.d[0] = 0;
        return false;
    }
    let r = rand_and(x, 0xf);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    lib::pick_byte(x);
    choose_attack(x, far);
    true
}

/// 0x7fa78 : en vol bas (`+0xa2`), approche à hauteur 0x30 ; toute cible dans une boîte déclenche une attaque.
fn patrol(x: &mut Cx) {
    x.set8(HOVER, 0);
    if wound_changed(x) {
        return set_steps(x, 0x0e00);
    }
    if !pick_target(x) {
        return idle_alone(x);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(FLYING, 1);
        x.set8(VULNERABLE, 0);
        speed(x, 0x17);
        return body_anim(x, 1);
    }
    x.set8(AIM, 0x23);
    lib::approach_target(x);
    match retarget(x, 0x0c00) {
        None | Some(false) => return,
        Some(true) => {}
    }
    lib::step(x);
    let h = x.u16(0xc) as i16;
    if h != 0x30 {
        let d = if h < 0x30 { 1 } else { 0xffff };
        x.add16(0xc, d);
    }
    if lib::target_in_box(x, -0x18, 0x30, 0x18) || lib::target_in_box(x, -0xc0, 0x30, 0x10) || lib::target_in_box(x, -0x50, 0x20, 0x10) {
        lib::pick_byte(x);
        return choose_attack(x, false);
    }
    x.move_by_table_xy();
    if target_behind(x) {
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
    }
}

/// 0x7fb94 : tient l'altitude `+0x6e` images, puis se pose.
fn hold_high(x: &mut Cx) {
    x.set8(HOVER, 0);
    if wound_changed(x) {
        return set_steps(x, 0x0e00);
    }
    if !sub8(x, IDLE_COUNT, 1).gt() {
        return set_steps(x, 0x1000);
    }
    if target_behind(x) {
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
    }
    lib::step(x);
}

/// 0x7fbca : monte jusqu'à l'altitude 0x30.
fn climb<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(HOVER, 0);
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(FLYING, 1);
        x.set8(INVULNERABLE, 1);
        stop(x);
        x.set16(0x18, 0x400);
        return body_anim(x, 1);
    }
    lib::step(x);
    x.move_z();
    if (x.u16(0xc) as i16) < 0x30 {
        return;
    }
    x.set8(INVULNERABLE, 1);
    x.set16(0xc, 0x30);
    set_steps(x, 0x0c00);
}

/// 0x7fc34 : descend jusqu'au sol, puis vol stationnaire.
fn land<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(HOVER, 0);
    if x.u8(0x7) == 0 {
        next7(x);
        stop(x);
        x.set16(0x18, (-0x400i16) as u16);
    }
    lib::step(x);
    x.move_z();
    if (x.u16(0xc) as i16) > 0 {
        return;
    }
    x.set16(0xc, 0);
    set_steps(x, 0);
}

/// 0x7fc6e : vol en ligne droite (`+0xa6` images) vers sa cible, puis approche.
fn sweep(x: &mut Cx) {
    x.set8(HOVER, 0);
    if x.u8(0x7) == 0 {
        next7(x);
        x.set16(TIMER, 0x1e);
        x.set8(FLYING, 0);
        x.set8(VULNERABLE, 1);
        speed(x, 0x1f);
        return body_anim(x, 1);
    }
    if sub16(x, TIMER, 1).mi() {
        return x.set32(f::STATE, 0x0202_0400);
    }
    x.set8(AIM, 0x2f);
    lib::approach_target(x);
    if target_behind(x) {
        x.set32(SAVED_STATE, 0x0202_0400);
        x.set32(f::STATE, 0x0202_0200);
    }
    x.move_by_table_xy();
    lib::step(x);
}

// --- groupe 6 : ronde, groupe 8 : envol -------------------------------------------------------------

/// Points de passage de la ronde, par quart de l'écran (mots X, Z, octets angle et sens ; relatifs au PC).
const ROUND_POINTS: u32 = 0x81876;

/// 0x806b4 : ronde (`+6` = 0, 2), saut d'envol (4) ; sans joueur en vie, attente.
pub(super) fn round(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 | 2 => circling(x),
        4 => take_off(x),
        _ => {
            x.set8(INVULNERABLE, 1);
            x.set32(f::STATE, 0x0202_0a00);
            circling(x)
        }
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        idle_alone(x);
    }
}

/// 0x81810 : point de passage opposé au quart de l'écran où se trouve le boss ; Z = sens nul.
fn pick_round_point<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> bool {
    let cam = camera(x);
    let dx = x.u16(0x8).wrapping_sub(cam.u16(x.b, 0x8)) as i16;
    let dz = x.u16(0x10).wrapping_sub(cam.u16(x.b, 0xc)) as i16;
    let quarter: u32 = match (dx > 0xc0, dz > 0x80) {
        (false, false) => 0,
        (false, true) => 6,
        (true, false) => 0xc,
        (true, true) => 0x12,
    };
    let gx = x.rom16(ROUND_POINTS + quarter).wrapping_add(cam.u16(x.b, 0x8));
    x.set16(GOAL_X, gx);
    let gz = x.rom16(ROUND_POINTS + 2 + quarter).wrapping_add(cam.u16(x.b, 0xc));
    x.set16(GOAL_Z, gz);
    let angle = x.rom8(ROUND_POINTS + 4 + quarter);
    x.set8(Obj::SPEED_INDEX, angle);
    let turn = x.rom8(ROUND_POINTS + 5 + quarter);
    x.set8(GOAL_TURN, turn);
    turn != 0
}

/// 0x806ea : vole vers le point de passage en tournant ; à l'arrivée (ou une cible loin devant) : approche.
fn circling(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        next7(x);
        x.set16(TIMER, 6);
        if pick_round_point(x) {
            body_anim(x, 3);
        }
        speed(x, 0x15);
        x.move_by_table_xy();
        lib::step(x);
        return;
    }
    let arrived = lib::target_in_box(x, 0x80, 0x100, 0x10) || {
        let dx = x.u16(GOAL_X).wrapping_sub(x.u16(0x8)).wrapping_add(8);
        dx <= 0x10 && {
            let dz = x.u16(GOAL_Z).wrapping_sub(x.u16(0x10)).wrapping_add(8);
            dz <= 0x10
        }
    };
    if arrived {
        return x.set32(f::STATE, 0x0202_0400);
    }
    if sub16(x, TIMER, 1).mi() {
        let turn = if x.u8(GOAL_TURN) != 0 { 0xff } else { 1 };
        let a = x.u8(Obj::SPEED_INDEX).wrapping_add(turn) & 0x1f;
        x.set8(Obj::SPEED_INDEX, a);
        x.set16(TIMER, 6);
    }
    x.move_by_table_xy();
    lib::step(x);
}

/// 0x807aa : saut d'envol (vers l'avant), retombée, puis approche.
fn take_off<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(INVULNERABLE, 1);
    match x.u8(0x7) {
        0 => {
            next7(x);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(Obj::SPEED_INDEX, angle);
            stop(x);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x60i16) as u16);
            speed(x, 0x15);
            body_anim(x, 0xa);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                x.move_by_table_xy();
                x.move_z();
            }
            if (x.u16(0xc) as i16) >= 0 {
                return;
            }
            x.set16(0xc, 0);
            x.set32(0x18, 0);
            next7(x);
            body_anim(x, 0xb);
        }
        _ => x.set32(f::STATE, 0x0202_0400),
    }
}

/// 0x80844 : envol : debout un moment (`+0xa6`), puis approche ; ou saut (`+6` = 2), ou ronde (4).
pub(super) fn rise(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => match x.u8(0x7) {
            0 => {
                next7(x);
                x.set16(TIMER, 0x14);
                x.set8(VULNERABLE, 1);
                body_anim(x, 0);
            }
            2 => {
                if !sub16(x, TIMER, 1).gt() {
                    return x.set32(f::STATE, 0x0202_0000);
                }
                if target_behind(x) {
                    x.set32(SAVED_STATE, 0x0208_0004);
                    return x.set32(f::STATE, 0x0202_0200);
                }
                lib::step(x);
            }
            _ => {
                x.add8(0x7, 0xfe);
                x.set8(VULNERABLE, 1);
                body_anim(x, 0);
            }
        },
        2 => take_off(x),
        4 => circling(x),
        _ => x.set32(f::STATE, 0x0202_0a00),
    }
}

// --- groupe 10 : attente --------------------------------------------------------------------------

/// 0x808d4 : un joueur revient : vol bas ou approche ; sinon le boss reste en pose (au sol ou en l'air).
pub(super) fn wait_for_players(x: &mut Cx) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 && pick_target(x) {
        let s = if x.u8(FLYING) != 0 { 0x0202_0a00 } else { 0x0202_0400 };
        return x.set32(f::STATE, s);
    }
    let anim = if x.u8(FLYING) == 0 && (x.u16(0xc) as i16) <= 0 {
        0
    } else {
        x.set8(HOVER, 0);
        1
    };
    if x.u8(0x7) == 0 {
        next7(x);
        return body_anim(x, anim);
    }
    lib::step(x);
}

// --- groupe 12 : un joueur emporté --------------------------------------------------------------

/// 0x80948 : le joueur saisi (0xfd64) est emporté en l'air puis lâché de haut (0xfd8c).
pub(super) fn carrying(x: &mut Cx) {
    let held = |x: &mut Cx| {
        let t = x.u16(f::TARGET);
        if (t as i16) >= 0 {
            return None;
        }
        let p = load_target(x);
        (p.u8(x.b, 0) != 0 && p.u8(x.b, 0x188) != 0).then_some(p)
    };
    if held(x).is_none() {
        return x.set32(f::STATE, 0x0202_0c00);
    }
    match x.u8(0x7) {
        0 => {
            next7(x);
            let v = ((x.u16(0x14) as i16) >> 2) as u16;
            x.set16(0x14, v);
            x.set16(0x16, 0);
            x.set32(0x18, 0);
            let t = x.u16(f::TARGET);
            if (t as i16) >= 0 {
                return x.set32(f::STATE, 0x0202_0c00);
            }
            let p = load_target(x);
            let (px, pz, ph) = (p.u16(x.b, 0x8), p.u16(x.b, 0x10), p.u16(x.b, 0xc));
            x.set16(0x8, px);
            x.set16(0x10, pz);
            x.set16(0xc, ph.wrapping_add(0x30));
            x.set8(TIMER, 0x10);
            attack_anim(x, 0x12);
            lib::hold_seized(x);
        }
        2 => {
            lib::step(x);
            x.move_xz();
            if (x.u16(0xc) as i16) <= 0x30 {
                x.set16(0xc, 0x30);
            }
            if !sub8(x, TIMER, 1).gt() {
                next7(x);
                stop(x);
                let mut v = 0x200u16;
                if x.u8(f::FLIP) == 0 {
                    v = v.wrapping_neg();
                }
                x.set16(0x14, v);
                x.set16(0x18, 0x200);
                x.set16(0x1a, 0x20);
                attack_anim(x, 0x13);
            }
            lib::hold_seized(x);
        }
        _ => {
            lib::step(x);
            x.move_xz();
            if (x.u16(0xc) as i16) <= 0x70 {
                return lib::hold_seized(x);
            }
            let p = load_target(x);
            p.set_u8(x.b, 0x6f, 0x10);
            lib::release_player(x, p);
            x.set32(f::STATE, 0x0202_0c00);
        }
    }
}

// --- groupe 14 : riposte ----------------------------------------------------------------------------

/// 0x80a5c : riposte tirée selon la difficulté (0xffda) : attaque 16 (ruée en piqué) ou 20 (rase-mottes).
pub(super) fn counter(x: &mut Cx) {
    x.set8(0x86, 0);
    let s = if !lib::roll_ffda(x) { 0x0204_1000 } else { 0x0204_1400 };
    x.set8(0, 1);
    x.set8(INVULNERABLE, 6);
    x.set32(f::STATE, s);
}
