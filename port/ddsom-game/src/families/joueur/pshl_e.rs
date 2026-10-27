//! Tirs des joueurs (PSHL), cinquième série : types 6 (tourbillon du rayon) et 18 (gerbe du clerc).
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;

const REINSERT: u32 = 0x1ee1a;
const SPEED_SELECT: u32 = 0x1b64;
const FLASH_ON: u32 = 0x1ac68;
const FLASH_OFF: u32 = 0x1ac90;
const MOVE_TABLE_XY: u32 = 0x1cb4;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 6 (0x527ec) : tir du rayon (PEFF 9) qui décrit un cercle autour du porteur (`+0x5c`), le
/// rang de l'arc étant la variante `+3`.
mod orbit {
    use super::*;

    const OFFSETS: u32 = 0x52802;
    const DELAYS: u32 = 0x528c8;
    const POWER_A: u32 = 0x528d0;
    const POWER_B: u32 = 0x528e8;
    const ANIMS: u32 = 0x15b346;
    const SPEEDS: u32 = 0x1c89b6;
    const SFX: u32 = 0xe64ce;
    const TURN_STEPS: u32 = 0x52aae;
    const TARGET_ANGLES: u32 = 0x52a76;
    const TURN_TABLE: u32 = 0x52b32;

    pub fn run(x: &mut Cx) {
        match x.state(0x527fa) {
            0 => init(x),
            2 => {
                x.c.d[0] = x.c.d[0] & !0xff | x.u8(0x5) as u32;
                match x.u8(0x5) {
                    0 => wait(x),
                    2 => {
                        x.add8(0x5, 2);
                        x.call(ANIM_STEP);
                    }
                    _ => fly(x),
                }
                x.call(REINSERT);
            }
            _ => {
                let parent = x.link_a1(0x5c);
                let n = parent.u8(x.b, 0x5d).wrapping_sub(1);
                parent.set_u8(x.b, 0x5d, n);
                x.free_pshl();
            }
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let parent = x.link_a1(0x5c);
        x.copy_pos(parent);
        let flip = parent.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let v = x.u8(0x3) as u32;
        x.c.d[0] = v;
        let delay = x.rom8(DELAYS + v);
        x.set8(0x53, delay);
        x.set8(f::BOX_SET, 8);
        let row = v << 3;
        x.c.d[0] = row;
        let dx = x.rom16(OFFSETS + row);
        let dz = x.rom16(OFFSETS + row + 2);
        let dy = x.rom16(OFFSETS + row + 4);
        x.add16(0xc, dz);
        x.add16(0x10, dy);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.c.d[1] = dx as u32;
        x.add16(0x8, dx);
        let owner = x.link_a1(f::OWNER);
        let class = owner.u8(x.b, 0x10c) as u32;
        x.c.d[0] = class;
        let p1 = x.rom8(POWER_A + class);
        let p2 = x.rom8(POWER_B + class);
        x.set8(f::POWER, p1);
        x.set8(f::POWER2, p2);
        x.set_bit(0x1, 1, true);
        x.anim(ANIMS, 0);
        x.call(SFX);
    }

    /// Sous-état 0 : attend le retard `+0x53`, puis passe à l'animation d'ouverture.
    fn wait(x: &mut Cx) {
        if x.u8(0x6) == 0 {
            let delay = x.u8(0x53);
            if delay != 0 {
                x.add8(0x53, 0xff);
                if x.u8(0x53) != 0 {
                    return;
                }
            }
            x.set_bit(0x1, 1, false);
            x.add8(0x6, 2);
        }
        x.call(ANIM_STEP);
        if x.c.cond(5) {
            x.add8(0x5, 2);
            x.set8(0x6, 0);
            x.set16(0x2a, 0x100);
            x.anim(ANIMS, 1);
        }
    }

    /// Sous-état 4 : vol en cercle.
    fn fly(x: &mut Cx) {
        match x.u8(0x6) {
            0 => {
                x.add8(0x6, 2);
                x.set8(0x7, 0);
                x.set32(0x4c, SPEEDS);
                x.set8(0x2d, 0x18);
                x.set8(0x5a, 0);
                x.set8(0x2f, 0);
                if x.u8(f::FLIP) != 0 {
                    x.set8(0x2d, 8);
                    x.set8(0x5a, 8);
                    x.add8(0x2f, 2);
                }
                x.set8(0x5b, 0);
                let owner = x.link_a1(f::OWNER);
                let cam_y = if x.a5_u8(a5::USE_CAM2) != 0 { x.a5_u16(a5::CAM2_Y) } else { x.a5_u16(a5::CAM_Y) };
                let d0 = owner.u16(x.b, 0x10).wrapping_sub(cam_y);
                x.c.d[0] = d0 as u32;
                if d0 < 0x60 {
                    x.set8(0x5b, 0x10);
                    x.add8(0x2f, 1);
                }
                x.set16(0x58, 0x202);
                x.call(ANIM_STEP);
            }
            _ => match x.u8(0x7) {
                0 => {
                    let cam_x = if x.a5_u8(a5::USE_CAM2) != 0 { x.a5_u16(0x556) } else { x.a5_u16(a5::CAM_X) };
                    let d1 = x.u16(0x8).wrapping_sub(cam_x) as i16;
                    x.c.d[0] = cam_x as u32;
                    x.c.d[1] = d1 as u16 as u32;
                    if x.u8(0x2d) == 8 {
                        if d1 > 0x130 {
                            x.add8(0x7, 2);
                        }
                    } else if d1 < 0x50 {
                        x.add8(0x7, 2);
                    }
                    x.call(MOVE_TABLE_XY);
                    x.call(ANIM_STEP);
                }
                2 => {
                    turn(x);
                    let k = (x.u8(0x2f) as u32) << 1;
                    x.c.d[0] = k;
                    let want_a = x.rom8(TARGET_ANGLES + k);
                    let want_b = x.rom8(TARGET_ANGLES + k + 1);
                    if want_a == x.u8(0x5b) && want_b == x.u8(0x2d) {
                        x.add8(0x7, 2);
                    }
                    x.call(ANIM_STEP);
                }
                _ => {
                    x.call(MOVE_TABLE_XY);
                    x.call(ANIM_STEP);
                    x.call(CULL);
                }
            },
        }
    }

    /// `0x52a9e` : avance d'un pas de rotation tous les `+0x58` images.
    fn turn(x: &mut Cx) {
        x.call(MOVE_TABLE_XY);
        x.add8(0x59, 0xff);
        if x.u8(0x59) != 0 {
            return;
        }
        let mut d3 = ((x.u8(0x5a) & 8) >> 3) as u32;
        let d4 = (x.u8(0x5b) >> 3) as u32;
        d3 += d4;
        let step = x.rom8(TURN_STEPS + d3);
        let timer = x.u8(0x58);
        x.set8(0x59, timer);
        let angle = x.u8(0x2d).wrapping_add(step) & 0x1f;
        x.set8(0x2d, angle);
        let low = angle & 0xf;
        x.c.d[0] = low as u32;
        let row = x.u8(0x5a).wrapping_add(x.u8(0x5b)) as u32;
        let at = TURN_TABLE + row;
        x.c.a[3] = at;
        let expected = x.data8(at);
        x.c.d[1] = expected as u32;
        if low != expected {
            return;
        }
        let d5 = x.u8(0x5a) & 6;
        x.c.d[5] = d5 as u32;
        if d5 == 6 && low == x.data8(at) {
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            let v = x.u8(0x5b) ^ 0x10;
            x.set8(0x5b, v);
        }
        let next = x.u8(0x5a).wrapping_add(2) & 0xf;
        x.set8(0x5a, next);
        let anim = x.data8(at + 1);
        x.c.d[0] = anim as u32;
        x.c.a[4] = ANIMS;
        x.call(0x1a32);
    }
}

/// Type 18 (0x53a08) : gerbe de flammes du clerc (cinq variantes). 0 : cercle de départ ; 1 : boule en vol
/// avec créations aléatoires ; 2 : étincelle ; 3 : braise ; 4 : anneau qui tourne autour du lanceur.
mod fountain {
    use super::*;

    const ANIMS: u32 = 0x15cb54;
    const TURN: u32 = 0x53c7c;

    pub fn run(x: &mut Cx) {
        match x.state(0x53a16) {
            0 => {
                x.c.d[0] = x.u8(0x3) as u32 * 2;
                match x.u8(0x3) {
                    0 => init_ring(x),
                    1 => init_ball(x),
                    2 => init_spark(x),
                    3 => init_ember(x),
                    _ => init_halo(x),
                }
            }
            2 => {
                x.c.d[0] = x.u8(0x3) as u32 * 2;
                match x.u8(0x3) {
                    0 => ring(x),
                    1 => ball(x),
                    2 => spark(x),
                    3 => ember(x),
                    _ => halo(x),
                }
            }
            _ => x.free_pshl(),
        }
    }

    fn init_ring(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x5a, 0x10);
        let owner = x.link_a1(f::OWNER);
        x.copy_pos(owner);
        x.set_d0w(0x45);
        x.call(SPEED_SELECT);
        x.call(MOVE_XY);
        x.anim(ANIMS, 0);
        x.call(REINSERT);
    }

    fn init_ball(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x5a, 1);
        x.set8(0x5c, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.set16(0x14, 0xe80);
        x.set16(0x16, -0x40i16 as u16);
        let mut dx = 0x46u16;
        if x.u8(f::FLIP) == 0 {
            let v = x.u32(0x14).wrapping_neg();
            x.set32(0x14, v);
            dx = dx.wrapping_neg();
        }
        x.set_d0w(dx);
        x.add16(0x8, dx);
        x.add16(0x10, 0xffff);
        x.anim(ANIMS, 3);
        x.set16(0x5e, 6);
        x.set_a5_u16(0x4ba, 0x100);
        x.call(FLASH_ON);
        x.call(REINSERT);
    }

    fn init_spark(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(0x10, 1);
        x.anim(ANIMS, 4);
        x.call(REINSERT);
    }

    fn init_ember(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x5a, 0x90);
        let r = x.rand_and(7);
        x.add16(0x5a, r as u16);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(0x10, 1);
        x.anim(ANIMS, 5);
        x.call(REINSERT);
    }

    fn init_halo(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x5a, 1);
        x.set8(0x5c, 4);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let mut dx = 0x30u16;
        x.set8(0x2d, 8);
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_neg();
            x.set8(0x2d, 0x18);
        }
        x.set_d0w(dx);
        x.add16(0x8, dx);
        x.set_d0w(0x4a);
        x.call(SPEED_SELECT);
        x.call(MOVE_XY);
        x.anim(ANIMS, 3);
        x.set16(0x5e, 6);
        x.set_a5_u16(0x4ba, 0x100);
        x.call(FLASH_ON);
        x.call(REINSERT);
    }

    // --- états actifs ----------------------------------------------------------------------------

    fn ring(x: &mut Cx) {
        x.c.d[0] = x.c.d[0] & !0xff | x.u8(0x5) as u32;
        if x.u8(0x5) == 0 {
            x.add16(0x5a, 0xffff);
            if x.u16(0x5a) == 0 {
                x.set16(0x5a, 0x10);
                x.add8(0x2f, 1);
                let step = x.u8(0x2f);
                let mut play = step == 1;
                if !play {
                    x.set16(0x5a, 0x20);
                    play = step == 2;
                }
                if play {
                    x.anim(ANIMS, step as u32);
                    return;
                }
                x.add8(0x5, 2);
                x.set16(0x5a, 0x14);
                let mut i = x.u8(0x5d) as u32;
                x.c.d[0] = i;
                if x.u8(f::FLIP) != 0 {
                    i += 6;
                }
                x.c.d[0] = i;
                let v = x.rom8(TURN + i);
                x.set8(0x5d, v);
            }
            x.call(ANIM_STEP);
        } else {
            x.add16(0x5a, 0xffff);
            if x.u16(0x5a) == 0 {
                x.add8(0x4, 2);
            }
            let want: u8 = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.c.d[0] = x.c.d[0] & !0xff | want as u32;
            if want != x.u8(0x2d) {
                let step = x.u8(0x5d);
                let angle = x.u8(0x2d).wrapping_add(step) & 0x1f;
                x.set8(0x2d, angle);
                let owner = x.link_a1(f::OWNER);
                x.copy_pos(owner);
                x.set_d0w(0x45);
                x.call(SPEED_SELECT);
                x.call(MOVE_XY);
            }
            x.call(ANIM_STEP);
        }
    }

    fn ball(x: &mut Cx) {
        if x.u16(0x5e) != 0 {
            x.add16(0x5e, 0xffff);
            if x.u16(0x5e) == 0 {
                x.set_a5_u16(0x4ba, 0);
            } else if x.u16(0x5e) == 2 {
                x.call(FLASH_OFF);
            }
        }
        x.call(MOVE_X);
        x.add16(0x5a, 0xffff);
        let t = x.u16(0x5a) & 3;
        x.set16(0x5a, t);
        if t == 0 {
            spawn(x, 2, None);
        }
        x.add8(0x5c, 0xff);
        let t = x.u8(0x5c) & 3;
        x.set8(0x5c, t);
        if t == 0 {
            let r = x.rand_and(0xf);
            if 0x3f >> r & 1 != 0 {
                spawn(x, 3, None);
            }
        }
        x.call(CULL);
        if x.u8(0x4) == 6 {
            x.set_a5_u16(0x4ba, 0);
        }
        x.call(ANIM_STEP);
    }

    /// Crée une braise ou une étincelle liée à la boule (`0x53992`, `0x539b2`).
    fn spawn(x: &mut Cx, variant: u8, _power: Option<(u8, u8)>) {
        if let Some(o) = x.alloc(&pools::PSHL) {
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 0x12);
            o.set_u8(x.b, 3, variant);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
        }
    }

    fn spark(x: &mut Cx) {
        x.call(ANIM_STEP);
        if x.c.cond(5) {
            x.add8(0x4, 2);
        }
    }

    fn ember(x: &mut Cx) {
        x.add16(0x5a, 0xffff);
        if x.u16(0x5a) == 0 {
            x.add8(0x4, 2);
        }
        x.call(ANIM_STEP);
    }

    fn halo(x: &mut Cx) {
        x.c.d[0] = x.c.d[0] & !0xff | x.u8(0x5) as u32;
        if x.u8(0x5) == 0 {
            if x.u16(0x5e) != 0 {
                x.add16(0x5e, 0xffff);
                if x.u16(0x5e) == 0 {
                    x.add8(0x5, 2);
                    x.set16(0x5a, 1);
                    x.set8(0x5c, 4);
                    x.set_a5_u16(0x4ba, 0);
                } else if x.u16(0x5e) == 2 {
                    x.call(FLASH_OFF);
                }
            }
            x.call(ANIM_STEP);
        } else {
            x.add16(0x5a, 0xffff);
            if x.u16(0x5a) == 0 {
                x.set16(0x5a, 1);
                let (mut step, mut target): (u8, u8) = (1, 0x18);
                if x.u8(f::FLIP) != 0 {
                    step = 0xff;
                    target = 8;
                }
                x.c.d[0] = x.c.d[0] & !0xff | step as u32;
                x.c.d[1] = x.c.d[1] & !0xff | target as u32;
                let angle = x.u8(0x2d).wrapping_add(step) & 0x1f;
                x.set8(0x2d, angle);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                let dx = signed(0x30, x.u8(f::FLIP) == 0);
                x.set_d0w(dx);
                x.add16(0x8, dx);
                x.set_d0w(0x4a);
                x.call(SPEED_SELECT);
                x.call(MOVE_XY);
                let want: u8 = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
                x.c.d[0] = x.c.d[0] & !0xff | want as u32;
                if want == x.u8(0x2d) {
                    x.add8(0x4, 2);
                }
            }
            x.add8(0x5c, 0xff);
            let t = x.u8(0x5c) & 3;
            x.set8(0x5c, t);
            if t == 0 {
                spawn(x, 2, None);
            }
            x.call(ANIM_STEP);
        }
    }
}

handlers! {
    0x527ec "pshl_6" orbit::run,
    0x53a08 "pshl_18" fountain::run,
}
