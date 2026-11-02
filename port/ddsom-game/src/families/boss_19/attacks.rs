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

/// Players table (FF85DE, 0x200 bytes each).
const PLAYERS: u32 = 0xff85de;
/// Choice of the breath target (bytes, per draw) then routine (words relative to 0x90494).
const AIM_PICK: u32 = 0x9049a;
const AIM_ROUTINES: u32 = 0x90494;
/// Offsets (longs) of the player slots walked in a randomly drawn order.
const PLAYER_ORDERS: u32 = 0x90556;

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
    let z = x.u16(0xc).wrapping_neg();
    lib::shock(x, 0, kind, 0, z);
}

/// Word read at A0 − 0x7f92: the original wanted the frame counter (`A5 + 0x6e`) but wrote a displacement
/// relative to A0; we read the same address.
fn tick_word<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let addr = x.o.0.wrapping_sub(0x7f92) & 0xff_ffff;
    x.data16(addr)
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
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.set16(0xc, 0);
    x.set32(0x18, 0);
    let v = x.u16(0x14) as i16;
    x.set16(0x14, if grow { v.wrapping_shl(1) } else { v >> 1 } as u16);
    x.set16(0x16, 0);
}

fn jump_back_and_forth<R: Bench>(x: &mut Cx<'_, R>, vx: i16, vz: i16) {
    let (a, b) = signed_by_facing(x, vx, vz);
    x.set16(0x14, a);
    x.set16(0x16, b);
}

// --- +6 = 0: backward leap -------------------------------------------------------------------

fn back_hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x47);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            jump_back_and_forth(x, 0x100, 0x40);
            x.set16(0x18, (-0x100i16) as u16);
            x.set16(0x1a, 0);
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
            x.add8(0x7, 2);
            x.set32(0x14, 0);
            x.set16(0x18, 0x100);
            x.set16(0x1a, 0);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            set_anim(x, 2, 5);
        }
        4 => {
            if tick_word(x) & 3 == 0 {
                dust(x, 0x101);
            }
            x.move_xz();
            if (x.u16(0xc) as i16) > 0x28 {
                x.set32(0x18, 0);
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
    x.set8(HOVER, 1);
    x.set8(HURT_BOX, 1);
    x.set8(FRAME_KIND, 0x48);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(MARK, 0);
            let (a, _) = signed_by_facing(x, 0x200, 0);
            x.set16(0x14, a);
            x.set16(0x16, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
        }
        2 => {
            x.move_x();
            if x.u8(MARK) != 0 {
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
    let boxes = if x.u8(TYPE) == TYPE_GREEN { 2 } else { 4 };
    x.set8(HURT_BOX, boxes);
    x.set8(HOVER, 1);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(MARK, 0);
            x.set8(TIMER, 0x10);
            speed(x, 0x11);
            breath_step(x);
        }
        2 => breath_step(x),
        _ => {}
    }
}

fn breath_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(MARK) != 0 {
        return fight::pick_attack_b(x);
    }
    aim_choice(x);
    x.anim_step();
}

/// Is player `a2` usable (0x9059e): slot occupied and `+0x182` zero.
fn usable<R: Bench>(x: &mut Cx<'_, R>, a2: Obj) -> bool {
    a2.u8(x.b, 0) != 0 && a2.u8(x.b, 0x182) == 0
}

/// 0x90458: keeps the current target as long as the timer (byte `+0xa6`) runs, otherwise draws another.
fn aim_choice<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return;
    }
    if sub8(x, TIMER, 1).gt() {
        // 0x90596: the target (+0x50) if usable
        let t = x.u16(f::TARGET);
        if (t as i16) < 0 {
            let target = x.linked(f::TARGET);
            if usable(x, target) {
                return aim(x, target);
            }
        }
    }
    x.set8(TIMER, 0x10);
    let r = random(x) & 0xf;
    let pick = x.rom8(AIM_PICK + r) as u32;
    let routine = (AIM_ROUTINES as i32 + x.rom16(AIM_ROUTINES + pick) as i16 as i32) as u32;
    let found = match routine {
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
        if ok && (player.u8(x.b, 4) as i8) < 4 {
            let score = if by_own {
                x.u16(0x60) as i16
            } else {
                let dx = (player.u16(x.b, 0x8) as i16).wrapping_abs_keep();
                let dy = (player.u16(x.b, 0x10) as i16).wrapping_abs_keep();
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
    let mut order = PLAYER_ORDERS + (random(x) & 0xc);
    let mut counter: u16;
    loop {
        let offset = x.data32(order);
        order += 4;
        let player = Obj(PLAYERS.wrapping_add(offset));
        let ok = usable(x, player);
        counter = ok as u16;
        if ok && (player.u8(x.b, 4) as i8) < 4 {
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
    let px = p.u16(x.b, 0x8).wrapping_add(d1 as u16);
    let py = p.u16(x.b, 0x10);
    lib::turn_toward(x, px, py);
    x.move_by_table_xy();
}

// --- +6 = 6: backward somersault ---------------------------------------------------------------------

fn backflip<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x49);
    x.set8(HURT_BOX, 3);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHIELD, 8);
            x.set8(0, 1);
            let (a, b) = signed_by_facing(x, -0x400, 0x80);
            x.set16(0x14, a);
            x.set16(0x16, b);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x40i16) as u16);
            x.set32(0x1c, 0);
            x.set16(TIMER, 0x40);
            set_anim(x, 2, 7);
        }
        2 => {
            x.move_xz();
            bounce(x, false);
            step(x);
            let t = x.u16(TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(TIMER, t);
            dust(x, 0x100);
        }
        _ => {}
    }
}

// --- +6 = 8: forward jump then leap -----------------------------------------------------------------------

fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x49);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHIELD, 8);
            x.set8(0, 1);
            let (a, _) = signed_by_facing(x, -0x100, 0);
            x.set16(0x14, a);
            x.set16(0x16, 0);
            x.set16(0x18, 0x200);
            x.set16(0x1a, (-0x20i16) as u16);
            x.set32(0x1c, 0);
            rest(x);
        }
        2 => {
            step(x);
            x.set8(SHIELD, 8);
            x.move_xz();
            if (x.u16(0x18) as i16) > 0 {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x10);
        }
        4 => {
            step(x);
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            x.set16(TIMER, 0x20);
            let (a, b) = signed_by_facing(x, 0x200, 0x40);
            x.set16(0x14, a);
            x.set16(0x16, b);
            x.set16(0x18, (-0x300i16) as u16);
            x.set16(0x1a, 0);
            set_anim(x, 2, 7);
        }
        6 => {
            x.set8(HURT_BOX, 3);
            x.set8(SHIELD, 8);
            x.move_xz();
            bounce(x, false);
            step(x);
            let t = x.u16(TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(TIMER, t);
            if t & 3 == 0 {
                dust(x, 0x100);
            }
        }
        _ => {}
    }
}

// --- +6 = 10: big jump ------------------------------------------------------------------------------------

fn lift<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(0x18, 0x800);
    x.set16(0x1a, (-0x40i16) as u16);
}

fn big_jump<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(FRAME_KIND, 0x49);
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(SHIELD, 8);
            x.set8(0, 1);
            lift(x);
            x.set32(0x1c, 0);
            x.set32(0x14, 0);
            rest(x);
        }
        2 => {
            x.set8(SHIELD, 8);
            x.move_z();
            if (x.u16(0xc) as i16) >= 0x100 {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x20);
                return;
            }
            if (x.u16(0x18) as i16) < 0 {
                return lift(x);
            }
            x.anim_step();
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(0x7, 2);
            // lands on the other side of the screen
            let mut d0 = var::SCROLL_X.get(x.b);
            if x.u8(0xa) == 0 {
                d0 = var::CAMERA_ALT_X.get(x.b);
            }
            let mut d1 = d0.wrapping_sub(0x40).wrapping_add(0xc0);
            let (mut side, mut vx) = (0x20u8, 0x600u16);
            d1 = d1.wrapping_sub(x.u16(0x8));
            if (d1 as i16) >= 0 {
                side = 0;
                d0 = d0.wrapping_add(0x1c0);
                vx = vx.wrapping_neg();
            }
            x.set16(0x8, d0);
            x.set16(0xc, 0xc0);
            x.set8(f::FLIP, side);
            x.set16(0x14, vx);
            x.set16(0x16, 0);
            x.set16(0x18, (-0x500i16) as u16);
            x.set16(0x1a, 0);
            x.set32(0x1c, 0);
            x.set16(TIMER, 0x20);
            set_anim(x, 2, 7);
        }
        6 => {
            x.set8(HURT_BOX, 3);
            x.move_xz();
            bounce(x, true);
            step(x);
            let t = x.u16(TIMER).wrapping_sub(1);
            if t & 0x8000 != 0 {
                return fight::pick_attack_b(x);
            }
            x.set16(TIMER, t);
            if (x.u16(0xc) as i16) >= 0x30 || t & 3 != 0 {
                return;
            }
            dust(x, 0x100);
        }
        _ => {}
    }
}
