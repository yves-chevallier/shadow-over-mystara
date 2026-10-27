//! Type 4, arrivée (`+5 = 0`), bête debout (`+5 = 2`) et repos (`+5 = 10`).

use super::*;

/// Tables de la ROM : attaques à choisir par niveau de difficulté (`A5+0x7f92`), 32 octets par niveau.
const CLOSE_ATTACKS: u32 = 0x5eec6;
const FAR_ATTACKS: u32 = 0x5f0c6;
const NEXT_MOVES: u32 = 0x5f3d8;
/// Tirage de la chance d'attaquer, un long par niveau de difficulté.
const ATTACK_CHANCE: u32 = 0x5f2ea;

// --- sous-état 0 : arrivée ----------------------------------------------------------------------

pub(super) fn arrival(x: &mut Cx) {
    if x.u8(f::SUB2) != 0 {
        return;
    }
    let left = x.u8(DELAY).wrapping_sub(1);
    x.set8(DELAY, left);
    if left == 0 && attack_chance(x) {
        return set_state_long(x, ACTIVE_STANCE);
    }
    match x.u8(0x7) {
        0 => walk_begin(x),
        2 => {
            if !pick_target(x) {
                return;
            }
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(0x7, 2);
                x.set8(VULNERABLE, 1);
                x.set8(WANT_ANIM, 0);
                x.set8(CUR_ANIM, 0);
                return body(x, 1);
            }
            if lib::target_in_box(x, 0, 0x80, 0x20) {
                set_state_long(x, ACTIVE_STANCE);
                lib::draw_6b(x);
                return lib::draw_stance(x);
            }
            step(x);
            move_xy(x);
        }
        4 => turn_end(x),
        _ => {}
    }
}

/// 0x5daa6 : marche vers l'avant (animation 2, vitesse 0xb) pendant `0x24` images.
pub(super) fn walk_begin(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set8(TIMER, 0x24);
    x.set8(VULNERABLE, 1);
    set_angle(x, 8, 0x18);
    pose(x, 2, SET_BODY, 2);
    speed(x, 0xb);
}

/// 0x5db66 : fin d'un demi-tour de l'arrivée.
pub(super) fn turn_end(x: &mut Cx) {
    if x.anim_step() {
        x.set8(0x7, 0);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(VULNERABLE, 1);
        x.set8(TIMER, 0);
        body(x, 3);
    }
}

/// 0x5f2c6 : la bête a-t-elle envie d'attaquer ? (un bit tiré dans un masque par niveau de difficulté).
fn attack_chance(x: &mut Cx) -> bool {
    let level = (x.a5_u16(0x7f92).wrapping_mul(4)) as u32;
    let mask = rom32(x, ATTACK_CHANCE + level);
    let bit = random(x) & 0x1f;
    mask >> bit & 1 != 0
}

// --- sous-état 2 : debout -----------------------------------------------------------------------

pub(super) fn run(x: &mut Cx) {
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    if x.a5_u8(0x35) == 0 {
        return set_state_long(x, ACTIVE_IDLE);
    }
    match x.u8(f::SUB2) {
        0 | 8 => pursue(x),
        2 => turn(x),
        4 => wait(x),
        6 => approach(x),
        _ => {}
    }
}

/// +6 = 0 et 8 (0x5dc2c) : marche vers la cible, observe, puis attaque ou change de côté.
fn pursue(x: &mut Cx) {
    if target_far(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        pose(x, 2, SET_BODY, 2);
        speed(x, 0xb);
        return lib::draw_stance(x);
    }
    let t = x.u8(AI_TURN).wrapping_sub(1);
    x.set8(AI_TURN, t);
    if (t as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let shot = if x.u8(f::SUB2) == 0 { 0 } else { 1 };
        x.set8(AI_SHOT, shot);
        lib::approach_target(x);
        let n = x.u8(AI_STEP).wrapping_sub(1);
        x.set8(AI_STEP, n);
        if n == 0 {
            lib::draw_6b(x);
            return pursue_tail(x);
        }
    }
    x.set8(REPEAT_LEFT, 1);
    engage(x);
}

/// 0x5dcc8 : un test de la cible peut figer la bête un instant ; sinon elle engage.
pub(super) fn pursue_tail(x: &mut Cx) {
    if lib::maybe_draw_6e(x) {
        return x.set16(f::SUB2, 0x0400);
    }
    x.set8(REPEAT_LEFT, 1);
    engage(x);
}

/// 0x5dce0 : la cible est-elle à portée ? Frappe, saisit, ou continue d'avancer.
pub(super) fn engage(x: &mut Cx) {
    if lib::target_in_box(x, -0x28, 0x28, 0x10) {
        x.set16(f::STATE, 0x0204);
        return choose_attack(x, CLOSE_ATTACKS);
    }
    if reach(x, -0x38, 0x38, 0x10, 0xf, CLOSE_ATTACKS) || reach(x, -0xb0, 0x40, 0x10, 0x2, FAR_ATTACKS) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 0x20) {
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    let Some(dx) = target_dx(x) else { return };
    // l'écart (bête - cible) est négatif quand la cible est à droite : la bête doit regarder à droite
    let want = if dx > 0 { 0x20 } else { 0 };
    if want != x.u8(f::FLIP) {
        x.set32(g::RESUME_STATE, ACTIVE_STANCE);
        x.set8(RESUME_STEP, 0);
        set_state_long(x, ACTIVE_TURN);
    }
}

/// Un joueur est juste devant (0x5dd7a ou 0x5ddf4) : au hasard une attaque de la table `attacks`
/// ou un coup court (état `0x02020800`). `mask` : bits du tirage qui choisissent le coup court
/// (`0xf` : jusqu'à 10 sur 16, `0x2` : une fois sur deux).
fn reach(x: &mut Cx, d0: i16, d1: u16, d2: u16, mask: u16, attacks: u32) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    let Some(t) = target(x) else { return false };
    let same_side = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    x.call(if same_side { 0xa4c2 } else { 0xa4b4 });
    if x.c.cond(7) {
        return false;
    }
    let r = random(x) as u16;
    let short = if mask == 0xf { (0xffc0u16 >> (r & mask)) & 1 != 0 } else { r & mask != 0 };
    if short {
        set_state_long(x, 0x0202_0800);
        return true;
    }
    set_state_long(x, 0x0204_0000);
    lib::pick_byte(x);
    choose_attack(x, attacks);
    true
}

/// 0x5ee94 et 0x5ee9a : tire l'attaque (`+6`) dans la table du niveau de difficulté.
pub(super) fn choose_attack(x: &mut Cx, table: u32) {
    let level = (x.a5_u16(0x7f92) >> 1 << 5) as u32;
    let r = random(x) & 0x1f;
    let pick = x.data8(table + level + r);
    x.set8(f::SUB2, pick);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x5ee68 : la cible est loin en abscisse.
fn target_far(x: &mut Cx) -> bool {
    let Some(t) = target(x) else { return false };
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xa0);
    dx > 0x140
}

/// +6 = 2 (0x5dbd4) : demi-tour (animation 1 puis 3) puis reprise de l'état mémorisé.
fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        x.set8(WANT_ANIM, 0);
        x.set8(CUR_ANIM, 0);
        return body(x, 1);
    }
    if x.anim_step() {
        let resume = x.u32(RESUME_STATE);
        x.set32(f::STATE, resume);
        let step = x.u8(RESUME_STEP);
        x.set8(f::SUB2, step);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        body(x, 3);
    }
}

/// +6 = 4 (0x5de68) : attente debout avant de passer à l'attaque.
fn wait(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        return ready(x);
    }
    let n = x.u8(AI_REPEAT).wrapping_sub(1);
    x.set8(AI_REPEAT, n);
    if (n as i8) < 0 {
        return set_state_long(x, ACTIVE_STANCE);
    }
    x.set32(RESUME_STATE, ACTIVE_STANCE);
    x.set8(RESUME_STEP, 4);
    turn_if_behind(x);
    step(x);
}

/// +7 = 0 des attaques (0x5de70) : la bête se prépare (animation de repos).
pub(super) fn ready(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    pose(x, 6, SET_BODY, 0);
}

/// +6 = 6 (0x5decc) : s'approche de la cible en la regardant.
fn approach(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x28);
            lib::draw_stance(x);
            x.set8(VULNERABLE, 1);
            pose(x, 4, SET_BODY, 5);
            speed(x, 0x1a);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) < 0 {
                return set_state_long(x, ACTIVE_STANCE);
            }
            let n = x.u8(AI_TURN).wrapping_sub(1);
            x.set8(AI_TURN, n);
            if (n as i8) < 0 {
                lib::draw_stance(x);
                pick_target(x);
                x.set8(AI_SHOT, 0);
                lib::approach_target(x);
            }
            x.set32(RESUME_STATE, ACTIVE_STANCE);
            let sub2 = x.u8(f::SUB2);
            x.set8(RESUME_STEP, sub2);
            turn_if_behind(x);
            step(x);
            move_xy(x);
        }
        _ => {}
    }
}

/// 0x5f36a : fin d'un coup ; la bête enchaîne une charge si un joueur est proche, sinon se remet debout.
pub(super) fn next_move(x: &mut Cx) {
    let Some(_) = target(x) else { return set_state_long(x, ACTIVE_STANCE) };
    if !lib::target_in_box(x, -0x38, 0x38, 0x20) && !lib::target_in_box(x, -0xb0, 0x40, 0x20) {
        return set_state_long(x, ACTIVE_STANCE);
    }
    let level = (x.a5_u16(0x7f92) >> 1 << 5) as u32;
    let r = random(x) & 0x1f;
    let pick = x.data8(NEXT_MOVES + level + r);
    x.set8(f::SUB2, pick);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
}

// --- sous-état 10 : repos -----------------------------------------------------------------------

/// Repos : attend qu'un joueur soit en vie (`+6` = 0 seulement).
pub(super) fn rest(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        match x.u8(0x7) {
            0 => walk_begin(x),
            2 => {
                let t = x.u8(TIMER).wrapping_sub(1);
                x.set8(TIMER, t);
                if t != 0 {
                    step(x);
                    move_xy(x);
                } else {
                    x.add8(0x7, 2);
                    x.set8(VULNERABLE, 1);
                    x.set8(WANT_ANIM, 0);
                    x.set8(CUR_ANIM, 0);
                    body(x, 1);
                }
            }
            4 => turn_end(x),
            _ => {}
        }
    }
    if x.a5_u8(0x35) != 0 {
        set_state_long(x, ACTIVE_STANCE);
    }
}
