//! Tirs des joueurs (PSHL), deuxième série : types 7 (sol gelé) et 8 (ombres du voleur).
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;
use crate::objects::Courant;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 7 (0x52b94) : sol gelé (magie 15), deux exemplaires (variante 0 : l'image de glace au sol qui
/// change selon le numéro de sort `+0x1e9` du lanceur ; variante 1 : sa zone de dégâts).
mod ground {
    use super::*;

    const ANIMS: u32 = 0x15b5fe;
    const COLORS: u32 = 0x52e1a;
    const POSE_A: u32 = 0x52d6a;
    const POSE_B: u32 = 0x52dd8;
    const POWER: u32 = 0x10422e;
    const POWER2: u32 = 0x10424c;
    const SFX_RIGHT: u32 = 0xe86e0;
    const SFX_LEFT: u32 = 0xe876a;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x52ba2) {
            0 if !child => init_a(x),
            0 => init_b(x),
            2 if !child => {
                let owner = x.link_a1(f::OWNER);
                if owner.u16(x.b, 0x168) == 0 {
                    x.add8(0x4, 2);
                }
                let spell = owner.u8(x.b, 0x1e9);
                if x.u8(0x5c) != spell {
                    x.set8(0x5c, spell);
                    pose(x);
                    let mut anim = x.u8(0x5c);
                    if owner.u8(x.b, 0x10c) > 0x14 {
                        anim = anim.wrapping_add(5);
                    }
                    x.anim(ANIMS, anim as u32);
                }
                colors(x);
                x.anim_step();
                x.reinsert();
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u16(x.b, 0x168) == 0 {
                    x.add8(0x4, 2);
                }
                let spell = owner.u8(x.b, 0x1e9);
                x.set8(0x3a, spell.wrapping_add(0x18));
                if x.u8(0x5c) != spell {
                    x.set8(0x5c, spell);
                    pose(x);
                }
                x.set32(0x34, 0);
                x.reinsert();
            }
            _ => x.free_pshl(),
        }
    }

    fn init_a(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(0x5d, 0);
        x.set8(0x5e, 1);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x14);
        let spell = owner.u8(x.b, 0x1e9);
        x.set8(0x5c, spell);
        pose(x);
        let anim = if owner.u8(x.b, 0x10c) > 0x14 { 0xb } else { 0xa };
        x.anim(ANIMS, anim);
        if owner.u8(x.b, f::FLIP) != 0 {
            crate::sound::jouer(x.b, SFX_RIGHT, x.o);
        } else {
            crate::sound::jouer(x.b, SFX_LEFT, x.o);
        }
        colors(x);
    }

    fn init_b(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 6);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let spell = owner.u8(x.b, 0x1e9);
        x.set8(0x5c, spell);
        let class = owner.u8(x.b, 0x10c) as u32;
        x.c.d[0] = class;
        let p1 = x.data8(POWER + class);
        let p2 = x.data8(POWER2 + class);
        x.set8(f::POWER, p1);
        x.set8(f::POWER2, p2);
        pose(x);
        x.anim(ANIMS, 0);
        x.set32(0x34, 0);
    }

    /// `0x52d06` : place l'objet sur le lanceur, devant ou derrière selon la magie en cours.
    fn pose(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        x.copy_pos(owner);
        x.set16(0x12, 0);
        owner.set_u16(x.b, 0x12, 0x8000);
        if owner.u8(x.b, 0x15e) == 0x1a {
            let half = owner.u8(x.b, 0x3) >> 1;
            let extra = if half == 2 { 0 } else { 0x14 };
            let row = ((x.u8(0x5c) as u32) << 2) + extra;
            x.c.d[0] = row;
            let dx = x.rom16(POSE_A + row);
            let dz = x.rom16(POSE_A + row + 2);
            x.c.d[1] = dx as u32;
            x.c.d[2] = dz as u32;
            let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
            x.add16(0x8, dx);
            x.add16(0xc, dz);
        } else {
            let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
            x.c.d[0] = row;
            let dx = x.rom16(POSE_B + row);
            let dz = x.rom16(POSE_B + row + 2);
            x.c.d[1] = dx as u32;
            x.c.d[2] = dz as u32;
            let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
            x.add16(0x8, dx);
            x.add16(0xc, dz);
        }
    }

    /// `0x52df0` : change de couleur toutes les 4 images (`+0x2b`).
    fn colors(x: &mut Cx) {
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
        x.c.d[0] = k;
        let c = x.rom8(COLORS + k);
        x.set8(0x2b, c);
    }
}

/// Type 8 (0x52e42) : ombres du coup de pied du voleur. `+0x2f` : 0 = ombre qui tombe d'en haut ;
/// 2 = ombre qui se détache du voleur ; 4 = ombre qui attend et frappe.
mod shadow {
    use super::*;

    const ANIMS: u32 = 0x15c4a8;
    const ANGLES: u32 = 0x52eda;
    const PICK: u32 = 0x52ed4;
    const SFX_FALL: u32 = 0xeb2f2;
    const SFX_STRIKE: u32 = 0xeb364;

    pub fn run(x: &mut Cx) {
        match x.state(0x52e50) {
            0 => {
                x.c.d[0] = x.u8(0x2f) as u32;
                match x.u8(0x2f) {
                    0 => init_fall(x),
                    2 => init_throw(x),
                    _ => init_wait(x),
                }
            }
            2 => {
                x.c.d[0] = x.u8(0x2f) as u32;
                match x.u8(0x2f) {
                    0 => fall(x),
                    2 => throw(x),
                    _ => strike(x),
                }
            }
            _ => x.free_pshl(),
        }
    }

    fn init_fall(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        for off in [0x8, 0x10] {
            let v = owner.u16(x.b, off);
            x.set16(off, v);
        }
        x.set16(0xc, 0x40);
        let v = x.u8(0x3) as u32;
        x.c.d[0] = v;
        let angle = x.rom8(ANGLES + v);
        x.set8(0x2d, angle);
        x.set_d0w(0x17);
        let n = x.c.d[0] as u16;
        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
        x.move_xy();
        let v = x.u8(0x3) as u32;
        x.c.d[0] = v;
        let angle = x.rom8(ANGLES + v);
        x.set8(0x2d, angle);
        x.set_d0w(0xd);
        let n = x.c.d[0] as u16;
        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
        x.set16(0x1a, -0x30i16 as u16);
        let v = x.u8(0x3) as u32;
        let pick = x.rom8(PICK + v);
        x.anim(ANIMS, pick as u32);
    }

    fn init_throw(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let dx = signed(0x2c, x.u8(f::FLIP) == 0);
        x.c.d[0] = dx as u32;
        x.add16(0x8, dx);
        x.set16(0xc, 0x60);
        x.add16(0x10, 0xffff);
        x.set16(0x18, -0x400i16 as u16);
        x.set16(0x1a, -0x40i16 as u16);
        x.set16(0x14, 0x300);
        if x.u8(f::FLIP) == 0 {
            let v = x.u16(0x14);
            x.set16(0x14, v.wrapping_neg());
        }
        x.anim(ANIMS, 1);
    }

    fn init_wait(x: &mut Cx) {
        x.add8(0x5c, 0xff);
        if x.u8(0x5c) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(f::POWER, 0xc);
        x.set8(f::POWER2, 0xc);
        x.anim(ANIMS, 3);
        crate::sound::jouer(x.b, SFX_STRIKE, x.o);
    }

    fn fall(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            x.move_xyz();
            if (x.u16(0xc) as i16) >= 0 {
                return x.reinsert();
            }
            x.add8(0x5, 2);
            x.set8(f::POWER, 0xc);
            x.set8(f::POWER2, 0xc);
            x.set32(0xc, 0);
            x.anim(ANIMS, 3);
            if x.u8(0x3) == 0 {
                crate::sound::jouer(x.b, SFX_FALL, x.o);
            }
            x.set8(f::HIT_FLAGS, 0);
            x.set8(0x3, 0);
            x.reinsert();
        } else {
            let owner = x.link_a1(f::OWNER);
            if owner.u8(x.b, 0x1ec) == 0 && x.u8(f::HIT_FLAGS) & 2 != 0 {
                owner.set_u8(x.b, 0x1ec, 1);
                let hp = owner.u16(x.b, 0x62).wrapping_sub(0xc);
                owner.set_u16(x.b, 0x62, if (hp as i16) > 0 { hp } else { 1 });
            }
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            x.reinsert();
        }
    }

    fn throw(x: &mut Cx) {
        x.move_xz();
        if (x.u16(0xc) as i16) >= 0 {
            return x.reinsert();
        }
        x.add8(0x4, 2);
        let mut offset = 0u16;
        let mut rank = 1u8;
        for _ in 0..3 {
            let Some(o) = x.alloc(&pools::PSHL) else { break };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 8);
            o.set_u8(x.b, 3, 1);
            o.set_u8(x.b, 0x2f, 4);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            let px = x.u16(0x8);
            o.set_u16(x.b, 0x8, px.wrapping_add(offset));
            let py = x.u16(0x10);
            o.set_u16(x.b, 0x10, py);
            o.set_u8(x.b, 0x5c, rank);
            let step = signed(0x28, x.u8(f::FLIP) == 0);
            offset = offset.wrapping_add(step);
            rank = rank.wrapping_add(8);
            x.c.d[4] = offset as u32;
            x.c.d[3] = rank as u32;
        }
        x.reinsert();
    }

    fn strike(x: &mut Cx) {
        if x.anim_step() {
            x.add8(0x4, 2);
        }
        x.reinsert();
    }
}

handlers! {
    0x52b94 "pshl_7" ground::run,
    0x52e42 "pshl_8" shadow::run,
}
