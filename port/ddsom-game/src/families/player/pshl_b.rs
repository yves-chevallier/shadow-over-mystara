//! Player shots (PSHL), second series: types 7 (frozen ground) and 8 (the thief's shadows).
//! See `docs/families/joueur.md`.

use crate::families::player::records::Shot;
use crate::player::Player;
use crate::objects::Obj;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::player::tables::{player_data, players, Read};

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 7 (0x52b94): frozen ground (magic 15), two copies (variant 0: the ice image on the ground that changes
/// according to the caster's spell number `+0x1e9`; variant 1: its damage zone).
mod ground {
    use super::*;

    const ANIMS: u32 = 0x15b5fe;
    const SFX_RIGHT: crate::sound::Sound = crate::sound::CMD_164_B;
    const SFX_LEFT: crate::sound::Sound = crate::sound::CMD_164_C;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        let child = x.u8(Obj::VARIANT) != 0;
        match x.state(0x52ba2) {
            0 if !child => init_a(x),
            0 => init_b(x),
            2 if !child => {
                let owner = x.linked(f::OWNER);
                if owner.u16(x.b, Player::ACTION_TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                let spell = owner.u8(x.b, 0x1e9);
                if x.u8(0x5c) != spell {
                    x.set8(0x5c, spell);
                    pose(x);
                    let mut anim = x.u8(0x5c);
                    if owner.u8(x.b, Player::LEVEL) > 0x14 {
                        anim = anim.wrapping_add(5);
                    }
                    x.anim(ANIMS, anim as u32);
                }
                colors(x);
                x.anim_step();
                x.reinsert();
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u16(x.b, Player::ACTION_TIMER) == 0 {
                    x.add8(Obj::STATE, 2);
                }
                let spell = owner.u8(x.b, 0x1e9);
                x.set8(0x3a, spell.wrapping_add(0x18));
                if x.u8(0x5c) != spell {
                    x.set8(0x5c, spell);
                    pose(x);
                }
                x.set32(Obj::FRAME, 0);
                x.reinsert();
            }
            _ => x.free_pshl(),
        }
    }

    fn init_a<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(0x5d, 0);
        x.set8(0x5e, 1);
        x.set8(Shot::OWN_PALETTE, 1);
        x.set8(Shot::PALETTE, 0x14);
        let spell = owner.u8(x.b, 0x1e9);
        x.set8(0x5c, spell);
        pose(x);
        let anim = if owner.u8(x.b, Player::LEVEL) > 0x14 { 0xb } else { 0xa };
        x.anim(ANIMS, anim);
        if owner.u8(x.b, f::FLIP) != 0 {
            crate::sound::play(x.b, SFX_RIGHT, x.o);
        } else {
            crate::sound::play(x.b, SFX_LEFT, x.o);
        }
        colors(x);
    }

    fn init_b<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        x.set8(f::BOX_SET, 6);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let spell = owner.u8(x.b, 0x1e9);
        x.set8(0x5c, spell);
        let class = owner.u8(x.b, Player::LEVEL) as u32;
        let p1 = players("magic_damage/3").at8(x.b, class);
        let p2 = players("magic_damage/3").at8(x.b, 30 + class);
        x.set8(f::POWER, p1);
        x.set8(f::POWER2, p2);
        pose(x);
        x.anim(ANIMS, 0);
        x.set32(Obj::FRAME, 0);
    }

    /// `0x52d06`: places the object on the caster, in front or behind depending on the current magic.
    fn pose<R: Bench>(x: &mut Cx<'_, R>) {
        let owner = x.linked(f::OWNER);
        x.copy_pos(owner);
        x.set16(Shot::Y_FRACTION, 0);
        owner.set_u16(x.b, Player::Y_FRACTION, 0x8000);
        if owner.u8(x.b, Player::SPECIAL_KIND) == 0x1a {
            let half = owner.u8(x.b, Player::CHARACTER) >> 1;
            let extra = if half == 2 { 0 } else { 0x14 };
            let row = ((x.u8(0x5c) as u32) << 2) + extra;
            let dx = player_data().ground_pose_a.at16(x.b, row);
            let dz = player_data().ground_pose_a.at16(x.b, row + 2);
            *x.c.d2() = dz as u32;
            let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
        } else {
            let row = ((owner.u8(x.b, Player::CHARACTER) >> 1) as u32) << 2;
            let dx = player_data().ground_pose_b.at16(x.b, row);
            let dz = player_data().ground_pose_b.at16(x.b, row + 2);
            *x.c.d2() = dz as u32;
            let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
        }
    }

    /// `0x52df0`: changes colour every 4 frames (`+0x2b`).
    fn colors<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5e, 0xff);
        if x.u8(0x5e) != 0 {
            return;
        }
        x.set8(0x5e, 4);
        x.add8(0x5d, 1);
        if x.u8(0x5d) > 5 {
            x.set8(0x5d, 0);
        }
        let k = x.u8(0x5d) as u32;
        let c = player_data().ground_colors.at8(x.b, k);
        x.set8(Shot::PALETTE, c);
    }
}

/// Type 8 (0x52e42): shadows of the thief's kick. `+0x2f`: 0 = shadow that falls from above; 2 = shadow that detaches
/// from the thief; 4 = shadow that waits and strikes.
mod shadow {
    use super::*;

    const ANIMS: u32 = 0x15c4a8;
    const SFX_FALL: crate::sound::Sound = crate::sound::CMD_13A_23A;
    const SFX_STRIKE: crate::sound::Sound = crate::sound::CMD_13C_23C;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(0x52e50) {
            0 => {
                match x.u8(Shot::SETTING) {
                    0 => init_fall(x),
                    2 => init_throw(x),
                    _ => init_wait(x),
                }
            }
            2 => {
                match x.u8(Shot::SETTING) {
                    0 => fall(x),
                    2 => throw(x),
                    _ => strike(x),
                }
            }
            _ => x.free_pshl(),
        }
    }

    fn init_fall<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10] {
            let v = owner.u16(x.b, off);
            x.set16(off, v);
        }
        x.set16(Obj::Z, 0x40);
        let v = x.u8(Obj::VARIANT) as u32;
        let angle = player_data().shadow_angles.at8(x.b, v);
        x.set8(Obj::SPEED_INDEX, angle);
        let n = 0x17;
        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
        x.move_xy();
        let v = x.u8(Obj::VARIANT) as u32;
        let angle = player_data().shadow_angles.at8(x.b, v);
        x.set8(Obj::SPEED_INDEX, angle);
        let n = 0xd;
        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
        x.set16(Obj::AZ, -0x30i16 as u16);
        let v = x.u8(Obj::VARIANT) as u32;
        let pick = player_data().shadow_pick.at8(x.b, v);
        x.anim(ANIMS, pick as u32);
    }

    fn init_throw<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(Obj::STATE, 2);
        let owner = x.linked(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let dx = signed(0x2c, x.u8(f::FLIP) == 0);
        x.add16(Obj::X, dx);
        x.set16(Obj::Z, 0x60);
        x.add16(Obj::Y, 0xffff);
        x.set16(Obj::VZ, -0x400i16 as u16);
        x.set16(Obj::AZ, -0x40i16 as u16);
        x.set16(Obj::VX, 0x300);
        if x.u8(f::FLIP) == 0 {
            let v = x.u16(Obj::VX);
            x.set16(Obj::VX, v.wrapping_neg());
        }
        x.anim(ANIMS, 1);
    }

    fn init_wait<R: Bench>(x: &mut Cx<'_, R>) {
        x.add8(0x5c, 0xff);
        if x.u8(0x5c) != 0 {
            return;
        }
        x.add8(Obj::STATE, 2);
        x.set8(f::POWER, 0xc);
        x.set8(f::POWER2, 0xc);
        x.anim(ANIMS, 3);
        crate::sound::play(x.b, SFX_STRIKE, x.o);
    }

    fn fall<R: Bench>(x: &mut Cx<'_, R>) {
        if x.u8(Shot::SUB_STATE) == 0 {
            x.move_xyz();
            if (x.u16(Obj::Z) as i16) >= 0 {
                return x.reinsert();
            }
            x.add8(Shot::SUB_STATE, 2);
            x.set8(f::POWER, 0xc);
            x.set8(f::POWER2, 0xc);
            x.set32(Obj::Z, 0);
            x.anim(ANIMS, 3);
            if x.u8(Obj::VARIANT) == 0 {
                crate::sound::play(x.b, SFX_FALL, x.o);
            }
            x.set8(f::HIT_FLAGS, 0);
            x.set8(Obj::VARIANT, 0);
            x.reinsert();
        } else {
            let owner = x.linked(f::OWNER);
            if owner.u8(x.b, 0x1ec) == 0 && x.u8(f::HIT_FLAGS) & 2 != 0 {
                owner.set_u8(x.b, 0x1ec, 1);
                let hp = owner.u16(x.b, Player::HP).wrapping_sub(0xc);
                owner.set_u16(x.b, Player::HP, if (hp as i16) > 0 { hp } else { 1 });
            }
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
            x.reinsert();
        }
    }

    fn throw<R: Bench>(x: &mut Cx<'_, R>) {
        x.move_xz();
        if (x.u16(Obj::Z) as i16) >= 0 {
            return x.reinsert();
        }
        x.add8(Obj::STATE, 2);
        let mut offset = 0u16;
        let mut rank = 1u8;
        for _ in 0..3 {
            let Some(o) = x.alloc(&pools::PSHL) else { break };
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 8);
            o.set_u8(x.b, Obj::VARIANT, 1);
            o.set_u8(x.b, Shot::SETTING, 4);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            let px = x.u16(Obj::X);
            o.set_u16(x.b, Obj::X, px.wrapping_add(offset));
            let py = x.u16(Obj::Y);
            o.set_u16(x.b, Obj::Y, py);
            o.set_u8(x.b, 0x5c, rank);
            let step = signed(0x28, x.u8(f::FLIP) == 0);
            offset = offset.wrapping_add(step);
            rank = rank.wrapping_add(8);
            *x.c.d4() = offset as u32;
            *x.c.d3() = rank as u32;
        }
        x.reinsert();
    }

    fn strike<R: Bench>(x: &mut Cx<'_, R>) {
        if x.anim_step() {
            x.add8(Obj::STATE, 2);
        }
        x.reinsert();
    }
}

handlers! {
    0x52b94 "pshl_7" ground::run,
    0x52e42 "pshl_8" shadow::run,
}
