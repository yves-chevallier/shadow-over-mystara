//! Boss 3, état actif hors attaques : arrivée (`+5` = 0), combat (2), ronde (6), envol (8), attente (10), joueur
//! emporté (12), riposte (14).

use super::*;

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
            set_dw(x, 0, z);
            x.set16(0x10, z);
            x.set16(0xc, (-0x80i16) as u16);
            stop(x);
            x.set16(0x18, 0x380);
            body_anim(x, 0xc);
        }
        2 => {
            if x.a5_u8(var::PAUSE) != 0 {
                return hidden(x);
            }
            let cam = camera(x);
            let px = cam.u16(x.b, 0x8).wrapping_add(0x100);
            set_dw(x, 0, px);
            x.set16(0x8, px);
            step(x);
            x.call(call::MOVE_Z);
            if (x.u16(0xc) as i16) < 0x80 {
                return hidden(x);
            }
            next6(x);
            let z = cam.u16(x.b, 0xc).wrapping_add(0x40);
            set_dw(x, 0, z);
            x.set16(0x10, z);
            x.set16(0xc, 0xc0);
            stop(x);
            x.set16(0x18, (-0x600i16) as u16);
            x.set16(0x1a, (-0x60i16) as u16);
            x.set8(TIMER, 0x20);
            body_anim(x, 0xd);
            x.call(0x1f0be);
            end_frame(x, 0x50);
        }
        4 => {
            x.set_bit(0x1, 1, true);
            if sub8(x, TIMER, 1).pl() {
                return end_frame(x, 0x50);
            }
            next6(x);
            x.call(0xe1370);
            x.set_bit(0x1, 1, false);
            descend(x);
        }
        6 => descend(x),
        _ => {
            if x.a5_u8(var::PAUSE) == 0 && step(x) {
                return enter_fight(x);
            }
            end_frame(x, 0x50);
        }
    }
}

/// 0x7f67a : encore hors de l'écran : retenue, remise dans la liste du premier plan.
fn hidden(x: &mut Cx) {
    set_dw(x, 0, 0x50);
    x.call(call::ZONE);
    x.call(0x1ef6c);
}

/// 0x7f6a8 : descente jusqu'au sol, puis cri.
fn descend(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) == 0 {
        step(x);
        x.call(call::MOVE_Z);
        if (x.u16(0xc) as i16) <= 0 {
            x.set16(0xc, 0);
            next6(x);
            stop(x);
            x.call(0xb3df6);
            x.c.d[0] = 0x30;
            x.call(call::SOUND_EVENT);
            body_anim(x, 0xe);
        }
    }
    end_frame(x, 0x50);
}

/// 0x7f712 : immunités du type, puis le combat commence dans la même image (approche).
fn enter_fight(x: &mut Cx) {
    x.call(rom::IMMUNITIES);
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
    if x.a5_u8(var::PLAYERS_ALIVE) != 0 {
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
    step(x);
}

/// 0x7f7ca : demi-tour (en vol : `+0xa6` images au plus), puis approche.
fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        next7(x);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(FLYING, 0);
        x.set8(VULNERABLE, 1);
        x.set8(ANGLE, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(TIMER, 0x20);
            x.set8(ANGLE, 8);
        }
        return body_anim(x, 2);
    }
    if !step(x) && sub8(x, TIMER, 1).gt() {
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
    x.call(call::RETARGET);
    let r = rand_and(x, 0xf);
    set_dw(x, 1, 0xffc0);
    let ok = if 0xffc0u32 >> r & 1 != 0 { pick_target(x) } else {
        x.call(call::PICK_OTHER);
        !zero(x)
    };
    if !ok {
        idle_alone(x);
        return None;
    }
    if sub8(x, WALK_STEPS, 1).ne() {
        return Some(true);
    }
    x.call(call::STEPS_DONE);
    x.call(call::STEPS_CHECK);
    if zero(x) {
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
    x.call(call::ASK_TARGET);
    match retarget(x, 0) {
        None | Some(false) => return,
        Some(true) => {}
    }
    x.set8(0xaf, 2);
    if target_in_box(x, -0x20, 0x20, 0x10) {
        x.call(call::ATTACK_LOCK);
        return choose_attack(x, false);
    }
    if lunge_box(x, -0x50, 0x20, false) || lunge_box(x, -0xc0, 0x30, true) {
        return;
    }
    if target_in_box_far(x, -0x60, 0x60, 0x10) {
        return x.call(call::ATTACK_LOCK);
    }
    step(x);
    x.call(call::MOVE_TABLE_XY);
    if target_behind(x) {
        x.set32(SAVED_STATE, 0x0202_0000);
        x.set32(f::STATE, 0x0202_0200);
    }
}

/// 0x7f984 / 0x7f9fe : cible dans la boîte (et du bon côté : 0xa4c2 si elle regarde comme le boss, 0xa4b4 sinon) :
/// ronde (6 chances sur 16) ou attaque. Vrai si l'étape a changé.
fn lunge_box(x: &mut Cx, d0: i16, d1: u16, far: bool) -> bool {
    if !target_in_box(x, d0, d1, 0x10) {
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
    let check = if side == x.u8(f::FLIP) { call::TARGET_SAME_SIDE } else { call::TARGET_OTHER_SIDE };
    x.call(check);
    if zero(x) {
        x.c.d[0] = 0;
        return false;
    }
    let r = rand_and(x, 0xf);
    set_dw(x, 1, 0xffc0);
    if 0xffc0u32 >> r & 1 != 0 {
        x.set32(f::STATE, 0x0202_0800);
        return true;
    }
    x.set32(f::STATE, 0x0204_0000);
    x.call(call::ATTACK_LOCK);
    choose_attack(x, far);
    x.c.d[0] = 1;
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
    x.call(call::ASK_TARGET);
    match retarget(x, 0x0c00) {
        None | Some(false) => return,
        Some(true) => {}
    }
    step(x);
    x.c.d[0] = 1;
    let h = x.u16(0xc) as i16;
    if h != 0x30 {
        let d = if h < 0x30 { 1 } else { 0xffff };
        set_dw(x, 0, d);
        x.add16(0xc, d);
    }
    if target_in_box(x, -0x18, 0x30, 0x18) || target_in_box(x, -0xc0, 0x30, 0x10) || target_in_box(x, -0x50, 0x20, 0x10) {
        x.call(call::ATTACK_LOCK);
        return choose_attack(x, false);
    }
    x.call(call::MOVE_TABLE_XY);
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
    step(x);
}

/// 0x7fbca : monte jusqu'à l'altitude 0x30.
fn climb(x: &mut Cx) {
    x.set8(HOVER, 0);
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(FLYING, 1);
        x.set8(INVULNERABLE, 1);
        stop(x);
        x.set16(0x18, 0x400);
        return body_anim(x, 1);
    }
    step(x);
    x.call(call::MOVE_Z);
    if (x.u16(0xc) as i16) < 0x30 {
        return;
    }
    x.set8(INVULNERABLE, 1);
    x.set16(0xc, 0x30);
    set_steps(x, 0x0c00);
}

/// 0x7fc34 : descend jusqu'au sol, puis vol stationnaire.
fn land(x: &mut Cx) {
    x.set8(HOVER, 0);
    if x.u8(0x7) == 0 {
        next7(x);
        stop(x);
        x.set16(0x18, (-0x400i16) as u16);
    }
    step(x);
    x.call(call::MOVE_Z);
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
    x.call(call::ASK_TARGET);
    if target_behind(x) {
        x.set32(SAVED_STATE, 0x0202_0400);
        x.set32(f::STATE, 0x0202_0200);
    }
    x.call(call::MOVE_TABLE_XY);
    step(x);
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
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        idle_alone(x);
    }
}

/// 0x81810 : point de passage opposé au quart de l'écran où se trouve le boss ; Z = sens nul.
fn pick_round_point(x: &mut Cx) -> bool {
    let cam = camera(x);
    let dx = x.u16(0x8).wrapping_sub(cam.u16(x.b, 0x8)) as i16;
    let dz = x.u16(0x10).wrapping_sub(cam.u16(x.b, 0xc)) as i16;
    set_dw(x, 0, dx as u16);
    set_dw(x, 1, dz as u16);
    let quarter: u32 = match (dx > 0xc0, dz > 0x80) {
        (false, false) => 0,
        (false, true) => 6,
        (true, false) => 0xc,
        (true, true) => 0x12,
    };
    x.c.d[6] = quarter;
    let gx = x.rom16(ROUND_POINTS + quarter).wrapping_add(cam.u16(x.b, 0x8));
    set_dw(x, 0, gx);
    x.set16(GOAL_X, gx);
    let gz = x.rom16(ROUND_POINTS + 2 + quarter).wrapping_add(cam.u16(x.b, 0xc));
    set_dw(x, 0, gz);
    x.set16(GOAL_Z, gz);
    let angle = x.rom8(ROUND_POINTS + 4 + quarter);
    x.set8(ANGLE, angle);
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
        x.call(call::MOVE_TABLE_XY);
        step(x);
        return;
    }
    let arrived = target_in_box(x, 0x80, 0x100, 0x10) || {
        let dx = x.u16(GOAL_X).wrapping_sub(x.u16(0x8)).wrapping_add(8);
        set_dw(x, 0, dx);
        dx <= 0x10 && {
            let dz = x.u16(GOAL_Z).wrapping_sub(x.u16(0x10)).wrapping_add(8);
            set_dw(x, 0, dz);
            dz <= 0x10
        }
    };
    if arrived {
        return x.set32(f::STATE, 0x0202_0400);
    }
    if sub16(x, TIMER, 1).mi() {
        let turn = if x.u8(GOAL_TURN) != 0 { 0xff } else { 1 };
        let a = x.u8(ANGLE).wrapping_add(turn) & 0x1f;
        x.set8(ANGLE, a);
        x.set16(TIMER, 6);
    }
    x.call(call::MOVE_TABLE_XY);
    step(x);
}

/// 0x807aa : saut d'envol (vers l'avant), retombée, puis approche.
fn take_off(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    match x.u8(0x7) {
        0 => {
            next7(x);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(ANGLE, angle);
            stop(x);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x60i16) as u16);
            speed(x, 0x15);
            body_anim(x, 0xa);
        }
        2 => {
            step(x);
            if x.u8(KEY) == 0 {
                x.call(call::MOVE_TABLE_XY);
                x.call(call::MOVE_Z);
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
                step(x);
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
    if x.a5_u8(var::PLAYERS_ALIVE) != 0 && pick_target(x) {
        let s = if x.u8(FLYING) != 0 { 0x0202_0a00 } else { 0x0202_0400 };
        x.c.d[0] = s;
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
    step(x);
}

// --- groupe 12 : un joueur emporté --------------------------------------------------------------

/// 0x80948 : le joueur saisi (0xfd64) est emporté en l'air puis lâché de haut (0xfd8c).
pub(super) fn carrying(x: &mut Cx) {
    let held = |x: &mut Cx| {
        let t = x.u16(f::TARGET);
        set_dw(x, 0, t);
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
            set_dw(x, 0, v);
            x.set16(0x14, v);
            x.set16(0x16, 0);
            x.set32(0x18, 0);
            let t = x.u16(f::TARGET);
            set_dw(x, 0, t);
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
            x.call(call::HOLD);
        }
        2 => {
            step(x);
            x.call(call::MOVE_XZ);
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
                set_dw(x, 0, v);
                x.set16(0x14, v);
                x.set16(0x18, 0x200);
                x.set16(0x1a, 0x20);
                attack_anim(x, 0x13);
            }
            x.call(call::HOLD);
        }
        _ => {
            step(x);
            x.call(call::MOVE_XZ);
            if (x.u16(0xc) as i16) <= 0x70 {
                return x.call(call::HOLD);
            }
            let p = load_target(x);
            p.set_u8(x.b, 0x6f, 0x10);
            x.call(call::RELEASE);
            x.set32(f::STATE, 0x0202_0c00);
        }
    }
}

// --- groupe 14 : riposte ----------------------------------------------------------------------------

/// 0x80a5c : riposte tirée selon la difficulté (0xffda) : attaque 16 (ruée en piqué) ou 20 (rase-mottes).
pub(super) fn counter(x: &mut Cx) {
    x.set8(0x86, 0);
    x.call(0xffda);
    let s = if zero(x) { 0x0204_1000 } else { 0x0204_1400 };
    x.set8(0, 1);
    x.set8(INVULNERABLE, 6);
    x.set32(f::STATE, s);
}
