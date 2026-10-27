//! Type 19, la bête debout (sous-état `+5 = 2`, 0x72b5c) : poursuite d'un joueur, demi-tours et choix
//! d'une attaque. Les étapes sont `+6` : 0 et 8 poursuite, 2 demi-tour, 4 attente, 6 approche.

use super::*;
use crate::objects::Courant;

/// Choix d'une attaque dans une table de la ROM : `+4 = 0x0204` (attaque) et `+6` tiré dans la
/// table du niveau de difficulté (`A5+0x7f92`).
const PICK_TABLE: u32 = 0x7462e;
/// Attaques de contact (0x74c3e) : tirage par niveau de difficulté.
const CLOSE_TABLE: u32 = 0x74cac;

pub(super) fn run(x: &mut Cx) {
    if matches!(x.u8(Obj::VARIANT), 0xc | 0xd) || grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    // une pièce dont le parent est touché l'est aussi
    if (x.u8(PART) as i8) >= 0 {
        if let Some(p) = parent(x) {
            if state_of(x, p) == 0x0402_0000 {
                return set_state_long(x, KNOCKED);
            }
        }
    }
    match x.u8(f::SUB2) {
        0 | 8 => pursue(x),
        2 => turn(x),
        4 => wait(x),
        6 => approach(x),
        _ => {}
    }
    // plus personne à combattre : la bête se repose
    if x.u16(0xc) == 0 && var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, ACTIVE_IDLE);
    }
}

/// +6 = 0 (0x72be0) : marche vers la cible, observe, puis attaque ou change de côté.
fn pursue(x: &mut Cx) {
    if target_far(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        x.set8(WANT_ANIM, 0xc);
        if x.u8(WANT_ANIM) != x.u8(CUR_ANIM) {
            let want = x.u8(WANT_ANIM);
            x.set8(CUR_ANIM, want);
            play(x, 8);
        }
        speed(x, 0x11);
        lib::draw_stance(x);
        return;
    }
    let t = x.u8(AI_TURN).wrapping_sub(1);
    x.set8(AI_TURN, t);
    if (t as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let shot = if x.u8(f::SUB2) == 0 { 0xe } else { 0xf };
        x.set8(AI_SHOT, shot);
        lib::approach_target(x);
        let n = x.u8(AI_STEP).wrapping_sub(1);
        x.set8(AI_STEP, n);
        if n == 0 {
            lib::draw_6b(x);
            return pursue_tail(x);
        }
    }
    x.set8(REPEAT_LEFT, 2);
    engage(x);
}

/// 0x72c7e : la bête se fige un instant si un test de la cible l'y invite, sinon elle engage.
pub(super) fn pursue_tail(x: &mut Cx) {
    if lib::maybe_draw_6e(x) {
        return x.set16(f::SUB2, 0x0400);
    }
    x.set8(REPEAT_LEFT, 2);
    engage(x);
}

/// 0x72c96 : la cible est-elle à portée ? Frappe, saisit, ou continue d'avancer.
pub(super) fn engage(x: &mut Cx) {
    if lib::target_in_box(x, -0x30, 0x18, 4) {
        set_state_long(x, 0x0204_0200);
        return lib::pick_byte(x);
    }
    if reach_front(x) || reach_back(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x30, 0x30, 0x20) {
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    face_resume(x);
}

/// Si la cible est derrière la bête, elle fait demi-tour puis reprend (0x72ce2 et suivantes).
fn face_resume(x: &mut Cx) {
    let Some(t) = target(x) else { return };
    let dx = x.u16(0x8).wrapping_sub(t.u16(x.b, 0x8)) as i16;
    let want = if dx >= 0 { 0 } else { 0x20 };
    if want != x.u8(f::FLIP) {
        x.set32(RESUME_STATE, ACTIVE_STANCE);
        x.set8(RESUME_SUB2, 0);
        set_state_long(x, 0x0202_0200);
    }
}

/// Un joueur est juste devant (0x72d2a ou 0x72da4) : attaque courte ou longue. `far` : portée de
/// la seconde boîte.
fn reach_gen(x: &mut Cx, d0: i16, close: bool) -> bool {
    if !lib::target_in_box(x, d0, 0x20, 2) {
        return false;
    }
    let Some(t) = target(x) else { return false };
    let same_side = t.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    if !lib::decision(x, same_side) {
        return false;
    }
    let r = random(x) as u16;
    let mask = if close { 0xf } else { 0x2 };
    let take_a = if close { (0xffc0u16 >> (r & mask)) & 1 != 0 } else { r & mask != 0 };
    if take_a {
        set_state_long(x, 0x0202_0800);
        return true;
    }
    set_state_long(x, 0x0204_0000);
    lib::pick_byte(x);
    choose_attack_a(x, close);
    true
}

/// 0x72d2a : un joueur à bout de bras.
fn reach_front(x: &mut Cx) -> bool {
    reach_gen(x, -0x40, true)
}

/// 0x72da4 : un joueur plus loin derrière.
fn reach_back(x: &mut Cx) -> bool {
    reach_gen(x, -0xb0, false)
}

/// +6 = 2 (0x72e18) : demi-tour ; fini, la bête reprend l'état mémorisé.
fn turn(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 1);
        x.set8(WANT_ANIM, 5);
        let want = x.u8(WANT_ANIM);
        x.set8(CUR_ANIM, want);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return play(x, 5);
    }
    if x.anim_step() {
        let resume = x.u32(RESUME_STATE);
        x.set32(f::STATE, resume);
        let sub2 = x.u8(RESUME_SUB2);
        x.set8(f::SUB2, sub2);
        play(x, 0);
    }
}

/// +6 = 4 (0x72e78) : attente debout avant de passer à l'attaque.
fn wait(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        return super::attacks::ready(x);
    }
    let n = x.u8(AI_REPEAT).wrapping_sub(1);
    x.set8(AI_REPEAT, n);
    if (n as i8) < 0 {
        return set_state_long(x, ACTIVE_STANCE);
    }
    if player_ahead(x) {
        x.set32(RESUME_STATE, ACTIVE_STANCE);
        x.set8(RESUME_SUB2, 4);
        set_state_long(x, 0x0202_0200);
    }
    step(x);
}

/// +6 = 6 (0x72eec) : s'approche de la cible en la regardant.
fn approach(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x28);
            lib::draw_stance(x);
            approach_walk(x);
        }
        // reprise du pas de marche, sans refaire le décompte
        4 => {
            x.add8(0x7, 0xfe);
            approach_walk(x);
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
                x.set8(AI_SHOT, 0xe);
                lib::approach_target(x);
            }
            if player_ahead(x) {
                x.set32(RESUME_STATE, 0x0202_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(RESUME_SUB2, sub2);
                set_state_long(x, 0x0202_0200);
            }
            step(x);
            move_xy(x);
        }
        _ => {}
    }
}

fn approach_walk(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 0x16);
    if x.u8(WANT_ANIM) != x.u8(CUR_ANIM) {
        let want = x.u8(WANT_ANIM);
        x.set8(CUR_ANIM, want);
        play(x, 0xd);
    }
    speed(x, 0x17);
}

// --- choix des attaques ---------------------------------------------------------------------------

/// 0x745a4 (`close`) / 0x745ac : tire une attaque dans la table du profil et du niveau de difficulté.
fn choose_attack_a(x: &mut Cx, close: bool) {
    let base = if close { 0 } else { 0x14 };
    let profile = x.u8(PROFILE).wrapping_shl(2) as u32;
    let table = x.data32(PICK_TABLE + profile);
    let level = (var::RANK.get(x.b) >> 1 << 5) as u32;
    let r = random(x) & 0x1f;
    let pick = x.data8(table.wrapping_add(level).wrapping_add(r));
    x.set8(f::SUB2, pick.wrapping_add(base));
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
    if x.in_arena(0) {
        return;
    }
    // hors de l'écran : pas de bond
    if matches!(x.u8(f::SUB2), 0x16 | 0x18 | 0x1a | 0x1c | 0x1e) {
        x.set8(f::SUB2, 0x14);
    }
}

/// 0x74c3e : fin d'une attaque, la bête enchaîne sur une charge ou se remet debout.
pub(super) fn next_move(x: &mut Cx) {
    let Some(_) = target(x) else { return set_state_long(x, ACTIVE_STANCE) };
    if !lib::target_in_box(x, -0x30, 0x30, 8) && !lib::target_in_box(x, -0xd0, 0x70, 8) {
        return set_state_long(x, ACTIVE_STANCE);
    }
    let level = (var::RANK.get(x.b) >> 1 << 5) as u32;
    let r = random(x) & 0x1f;
    let pick = x.data8(CLOSE_TABLE + level + r);
    x.set8(f::SUB2, pick);
    x.set16(f::STATE, 0x0208);
    x.set8(0x7, 0);
}
