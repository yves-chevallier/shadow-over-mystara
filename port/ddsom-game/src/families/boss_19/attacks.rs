//! Boss 14 / 19, group `+5` = 4: the attacks (`+6` = 0 to 10), each in steps `+7`.
//!
//! | `+6` | Attack |
//! |---|---|
//! | 0 | backward leap then landing, about-face |
//! | 2 | charge forward (as long as the mark `+0xa3` is not raised) |
//! | 4 | breath: follows a player (`aim`) while the animation plays |
//! | 6 | backward somersault |
//! | 8 | forward jump then leap |
//! | 10 | big jump: rises, passes to the other side of the screen, lands |
//!
//! They end with `fight::pick_attack_b`. Several leak a puff of dust (effect 0xb10c8).

use crate::families::cx::Bench;
use super::fight;
use super::*;
use crate::objects::Obj;
use crate::objects::Current;

/// Players table (0x200 bytes each).
const PLAYERS: u32 = crate::ram::PLAYERS.base;
/// Offsets (longs) of the player slots walked in a randomly drawn order.
const PLAYER_ORDERS: &str = "boss_19/player_orders";

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => back_hop(x),
        2 => charge(x),
        4 => breath(x),
        6 => backflip(x),
        8 => hop(x),
        10 => big_jump(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        wait(x);
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Dust under the dragon (0xb10c8): `kind` 0x100 or 0x101.
fn dust<R: Bench>(x: &mut Cx<'_, R>, kind: u16) {
    let z = x.u16(Obj::Z).wrapping_neg();
    lib::shock(x, 0, kind, 0, z);
}

/// Word read at A0 − 0x7f92: the original wanted the frame counter (`A5 + 0x6e`) but wrote a displacement
/// relative to A0; we read the same address.
fn tick_word<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let addr = x.o.0.wrapping_sub(0x7f92) & 0xff_ffff;
    crate::mem::rd16(x.b, addr)
}

/// Speeds `(vx, vz)` by direction: `(a, b)` if the dragon looks right (`+0x2c` zero inverted).
fn signed_by_facing<R: Bench>(x: &mut Cx<'_, R>, a: i16, b: i16) -> (u16, u16) {
    if x.u8(f::FLIP) != 0 {
        (a as u16, b as u16)
    } else {
        (a.wrapping_neg() as u16, b.wrapping_neg() as u16)
    }
}

/// Touches the ground: the altitude is brought back to 0, the vertical speed cancelled and the horizontal speed halved
/// (`grow`: doubled in the big jump).
fn bounce<R: Bench>(x: &mut Cx<'_, R>, grow: bool) {
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.set16(Obj::Z, 0);
    x.set32(Enemy::VZ_LONG, 0);
    let v = x.u16(Enemy::VX) as i16;
    x.set16(Enemy::VX, if grow { v.wrapping_shl(1) } else { v >> 1 } as u16);
    x.set16(Enemy::AX, 0);
}

fn jump_back_and_forth<R: Bench>(x: &mut Cx<'_, R>, vx: i16, vz: i16) {
    let (a, b) = signed_by_facing(x, vx, vz);
    x.set16(Enemy::VX, a);
    x.set16(Enemy::AX, b);
}

// --- +6 = 0: backward leap -------------------------------------------------------------------

fn back_hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x47);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            jump_back_and_forth(x, 0x100, 0x40);
            x.set16(Enemy::VZ, (-0x100i16) as u16);
            x.set16(Enemy::GRAVITY, 0);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            set_anim(x, 2, 4);
        }
        2 => {
            x.move_xz();
            bounce(x, false);
            if !step(x) {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x100);
            x.set16(Enemy::GRAVITY, 0);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            set_anim(x, 2, 5);
        }
        4 => {
            if tick_word(x) & 3 == 0 {
                dust(x, 0x101);
            }
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0x28 {
                x.set32(Enemy::VZ_LONG, 0);
            }
            if step(x) {
                fight::pick_attack_b(x);
            }
        }
        _ => {}
    }
}

/// Animation byte (`+0x44`) read by the scripts.
const FRAME_KIND: u32 = 0x44;

// --- +6 = 2 : charge ----------------------------------------------------------------------------

fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss19::HOVER, 1);
    x.set8(Boss19::HURT_BOX, 1);
    x.set8(FRAME_KIND, 0x48);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss19::MARK, 0);
            let (a, _) = signed_by_facing(x, 0x200, 0);
            x.set16(Enemy::VX, a);
            x.set16(Enemy::AX, 0);
            x.set32(Enemy::VZ_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
        }
        2 => {
            x.move_x();
            if x.u8(Boss19::MARK) != 0 {
                return fight::pick_attack_b(x);
            }
            x.anim_step();
            if tick_word(x) & 3 == 0 {
                dust(x, 0x101);
            }
        }
        _ => {}
    }
}

// --- +6 = 4 : souffle ---------------------------------------------------------------------------

fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    let boxes = if x.u8(Enemy::KIND) == TYPE_GREEN { 2 } else { 4 };
    x.set8(Boss19::HURT_BOX, boxes);
    x.set8(Boss19::HOVER, 1);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss19::MARK, 0);
            x.set8(Boss19::TIMER, 0x10);
            speed(x, 0x11);
            breath_step(x);
        }
        2 => breath_step(x),
        _ => {}
    }
}

fn breath_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss19::MARK) != 0 {
        return fight::pick_attack_b(x);
    }
    aim_choice(x);
    x.anim_step();
}

/// Is player `a2` usable (0x9059e): slot occupied and `+0x182` zero.
fn usable<R: Bench>(x: &mut Cx<'_, R>, a2: Obj) -> bool {
    a2.u8(x.b, Obj::ACTIVE) != 0 && a2.u8(x.b, Player::AURA) == 0
}

/// 0x90458: keeps the current target as long as the timer (byte `+0xa6`) runs, otherwise draws another.
fn aim_choice<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return;
    }
    if sub8(x, Boss19::TIMER, 1).gt() {
        // 0x90596: the target (+0x50) if usable
        let t = x.u16(f::TARGET);
        if (t as i16) < 0 {
            let target = x.linked(f::TARGET);
            if usable(x, target) {
                return aim(x, target);
            }
        }
    }
    x.set8(Boss19::TIMER, 0x10);
    let r = (x.random() as u32) & 0xf;
    // choice of the breath target, then routine
    let found = match decide(x, "boss_19/aim_picks", r, "boss_19/aim_jumps") {
        0x904aa => nearest(x, false),
        0x904ee => nearest(x, true),
        0x90524 => in_random_order(x),
        t => unreachable!("boss 19: target choice {t:x}"),
    };
    if let Some(p) = found {
        aim(x, p);
    }
}

/// 0x904aa and 0x904ee: the player with the lowest score. The score is `|x| + |y|` (0x904aa) or the dragon's `+0x60`
/// (0x904ee); the best starts at -1, so it is almost never reached. The original destroys its loop counter
/// (`D0`) through the usual call: the loop picks up the result of this test.
fn nearest<R: Bench>(x: &mut Cx<'_, R>, by_own: bool) -> Option<Obj> {
    let mut best: i16 = -1;
    let mut chosen = None;
    let mut counter: u16;
    let mut a2 = PLAYERS;
    loop {
        let player = Obj(a2);
        let ok = usable(x, player);
        counter = ok as u16;
        if ok && (player.u8(x.b, Obj::STATE) as i8) < 4 {
            let score = if by_own {
                x.u16(Enemy::HP_MAX) as i16
            } else {
                let dx = (player.u16(x.b, Obj::X) as i16).wrapping_abs_keep();
                let dy = (player.u16(x.b, Obj::Y) as i16).wrapping_abs_keep();
                dx.wrapping_add(dy)
            };
            if score < best {
                best = score;
                chosen = Some(player);
            }
        }
        a2 += 0x200;
        counter = counter.wrapping_sub(1);
        if counter == 0xffff {
            break;
        }
    }
    if best >= 0 { chosen } else { None }
}

trait AbsKeep {
    fn wrapping_abs_keep(self) -> i16;
}

impl AbsKeep for i16 {
    /// `neg.w` if negative (−0x8000 stays negative).
    fn wrapping_abs_keep(self) -> i16 {
        if self < 0 { self.wrapping_neg() } else { self }
    }
}

/// 0x90524: walks through the players in a randomly drawn order (rotation table) until a usable player is found.
/// Same destroyed loop counter as `nearest`.
fn in_random_order<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    let mut order = (x.random() as u32) & 0xc;
    let mut counter: u16;
    loop {
        let offset = long(x, PLAYER_ORDERS, order);
        order += 4;
        let player = Obj(PLAYERS.wrapping_add(offset));
        let ok = usable(x, player);
        counter = ok as u16;
        if ok && (player.u8(x.b, Obj::STATE) as i8) < 4 {
            return Some(player);
        }
        counter = counter.wrapping_sub(1);
        if counter == 0xffff {
            return None;
        }
    }
}

/// 0x90576: walks toward player `p`, aiming 0xc0 pixels in front of them.
fn aim<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    let mut d1: i16 = 0xc0;
    if x.u8(f::FLIP) != 0 {
        d1 = d1.wrapping_neg();
    }
    let px = p.u16(x.b, Obj::X).wrapping_add(d1 as u16);
    let py = p.u16(x.b, Obj::Y);
    lib::turn_toward(x, px, py);
    x.move_by_table_xy();
}

// --- +6 = 6: backward somersault ---------------------------------------------------------------------

fn backflip<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x49);
    x.set8(Boss19::HURT_BOX, 3);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss19::SHIELD, 8);
            x.set8(Enemy::ACTIVE, 1);
            let (a, b) = signed_by_facing(x, -0x400, 0x80);
            x.set16(Enemy::VX, a);
            x.set16(Enemy::AX, b);
            x.set16(Enemy::VZ, 0x300);
            x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
            x.set32(Enemy::VY_LONG, 0);
            x.set16(Boss19::TIMER, 0x40);
            set_anim(x, 2, 7);
        }
        2 => {
            x.move_xz();
            bounce(x, false);
            step(x);
            let t = x.u16(Boss19::TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(Boss19::TIMER, t);
            dust(x, 0x100);
        }
        _ => {}
    }
}

// --- +6 = 8: forward jump then leap -----------------------------------------------------------------------

fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x49);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss19::SHIELD, 8);
            x.set8(Enemy::ACTIVE, 1);
            let (a, _) = signed_by_facing(x, -0x100, 0);
            x.set16(Enemy::VX, a);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x200);
            x.set16(Enemy::GRAVITY, (-0x20i16) as u16);
            x.set32(Enemy::VY_LONG, 0);
            rest(x);
        }
        2 => {
            step(x);
            x.set8(Boss19::SHIELD, 8);
            x.move_xz();
            if (x.u16(Enemy::VZ) as i16) > 0 {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss19::TIMER, 0x10);
        }
        4 => {
            step(x);
            if sub16(x, Boss19::TIMER, 1).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss19::TIMER, 0x20);
            let (a, b) = signed_by_facing(x, 0x200, 0x40);
            x.set16(Enemy::VX, a);
            x.set16(Enemy::AX, b);
            x.set16(Enemy::VZ, (-0x300i16) as u16);
            x.set16(Enemy::GRAVITY, 0);
            set_anim(x, 2, 7);
        }
        6 => {
            x.set8(Boss19::HURT_BOX, 3);
            x.set8(Boss19::SHIELD, 8);
            x.move_xz();
            bounce(x, false);
            step(x);
            let t = x.u16(Boss19::TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(Boss19::TIMER, t);
            if t & 3 == 0 {
                dust(x, 0x100);
            }
        }
        _ => {}
    }
}

// --- +6 = 10: big jump ------------------------------------------------------------------------------------

fn lift<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(Enemy::VZ, 0x800);
    x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
}

fn big_jump<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x49);
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss19::SHIELD, 8);
            x.set8(Enemy::ACTIVE, 1);
            lift(x);
            x.set32(Enemy::VY_LONG, 0);
            x.set32(Enemy::VX_LONG, 0);
            rest(x);
        }
        2 => {
            x.set8(Boss19::SHIELD, 8);
            x.move_z();
            if (x.u16(Obj::Z) as i16) >= 0x100 {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss19::TIMER, 0x20);
                return;
            }
            if (x.u16(Enemy::VZ) as i16) < 0 {
                return lift(x);
            }
            x.anim_step();
        }
        4 => {
            if sub16(x, Boss19::TIMER, 1).gt() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            // lands on the other side of the screen
            let mut d0 = var::SCROLL_X.get(x.b);
            // the high byte of the saved X, as the original reads it (`tst.b (0xa,A0)`)
            if x.u8(Enemy::X_SAVED) == 0 {
                d0 = var::CAMERA_ALT_X.get(x.b);
            }
            let mut d1 = d0.wrapping_sub(0x40).wrapping_add(0xc0);
            let (mut side, mut vx) = (0x20u8, 0x600u16);
            d1 = d1.wrapping_sub(x.u16(Obj::X));
            if (d1 as i16) >= 0 {
                side = 0;
                d0 = d0.wrapping_add(0x1c0);
                vx = vx.wrapping_neg();
            }
            x.set16(Obj::X, d0);
            x.set16(Obj::Z, 0xc0);
            x.set8(f::FLIP, side);
            x.set16(Enemy::VX, vx);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, (-0x500i16) as u16);
            x.set16(Enemy::GRAVITY, 0);
            x.set32(Enemy::VY_LONG, 0);
            x.set16(Boss19::TIMER, 0x20);
            set_anim(x, 2, 7);
        }
        6 => {
            x.set8(Boss19::HURT_BOX, 3);
            x.move_xz();
            bounce(x, true);
            step(x);
            let t = x.u16(Boss19::TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(Boss19::TIMER, t);
            if (x.u16(Obj::Z) as i16) >= 0x30 || t & 3 != 0 {
                return;
            }
            dust(x, 0x100);
        }
        _ => {}
    }
}
