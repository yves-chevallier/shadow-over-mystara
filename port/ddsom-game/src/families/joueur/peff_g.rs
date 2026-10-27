//! Effets des joueurs (PEFF), septième série : types 21 à 23 (étincelle du coup, fumée de la fin de
//! prière, lueur de la prière). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;
use crate::objects::Courant;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 21 (0x4d2fe) : étincelle du coup (variantes 0 et 1), posée devant le joueur ; la variante 0 vit
/// tant que son geste (`+0x5c`) dure et clignote.
mod hit_spark {
    use super::*;

    const ANIMS: u32 = 0x1bb146;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4d30c) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                let gesture = owner.u8(x.b, 0x7);
                x.set8(0x5c, gesture);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                x.add16(0x10, 0xffff);
                let (mut dx, mut dz) = (0x10u16, 0x22u16);
                if child {
                    (dx, dz) = (-0x18i16 as u16, 0x36);
                }
                x.c.d[0] = dx as u32;
                x.c.d[1] = dz as u32;
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 if !child => {
                x.anim_step();
                x.set_bit(0x1, 1, true);
                let v = x.u8(0x5d) ^ 1;
                x.set8(0x5d, v);
                if v == 0 {
                    x.set_bit(0x1, 1, false);
                }
                let owner = x.link_a1(f::OWNER);
                let gesture = owner.u8(x.b, 0x7);
                x.c.d[0] = x.c.d[0] & !0xff | gesture as u32;
                if gesture != x.u8(0x5c) {
                    x.add8(0x4, 2);
                }
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 22 (0x4d3e2) : la prière du clerc (kind 0x16). Variante 0 : nimbe qui dure `+0x168 - 0x1e` images ;
/// variante 1 : rayons (rang `+2f` 0 = compte à rebours, 1 = pluie d'étincelles) ; variante 2 : lueur finale.
mod prayer {
    use super::*;

    const ANIMS: u32 = 0x1bb240;
    const DURATIONS: u32 = 0x4d45e;
    const ANGLES: u32 = 0x4d5e6;
    const KIND: u8 = 0x16;

    pub fn run(x: &mut Cx) {
        let variant = x.u8(0x3);
        match x.state(0x4d3f0) {
            0 => match variant {
                0 => init_halo(x),
                1 => init_rays(x),
                _ => init_glow(x),
            },
            2 => match variant {
                0 => {
                    x.anim_step();
                    x.add16(0x5c, 0xffff);
                    if x.u16(0x5c) == 0 {
                        x.add8(0x4, 2);
                    }
                }
                1 => rays(x),
                _ => {
                    if x.anim_step() {
                        x.add8(0x4, 2);
                    }
                }
            },
            _ => x.free_peff(),
        }
    }

    fn init_halo(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(0x8, 0);
        x.add16(0xc, 0x37);
        x.add16(0x10, 0xffff);
        if owner.u8(x.b, 0x15e) == 0x24 {
            x.add16(0x10, 2);
        }
        let class = owner.u8(x.b, 0x10c) as u32;
        x.c.d[0] = class * 2;
        let life = x.rom16(DURATIONS + class * 2);
        x.set16(0x5c, life);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    fn init_rays(x: &mut Cx) {
        if x.u8(0x2f) == 0 {
            x.add8(0x4, 2);
            let owner = x.link_a1(f::OWNER);
            let life = owner.u16(x.b, 0x168).wrapping_sub(0x1e);
            x.set16(0x5c, life);
            return;
        }
        x.add8(0x4, 2);
        x.set16(0x5c, 0x28);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(0xc, 0x20);
        x.set_d0w(0x43);
        let n = x.c.d[0] as u16;
        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
        x.move_by_table_xy();
        x.set_d0w(0x21);
        let n = x.c.d[0] as u16;
        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
        x.anim(ANIMS, 1);
        x.reinsert();
    }

    fn init_glow(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(0x8, 0);
        x.add16(0xc, 0x37);
        x.add16(0x10, 0xffff);
        x.anim(ANIMS, 3);
        x.reinsert();
    }

    fn rays(x: &mut Cx) {
        if x.u8(0x2f) == 0 {
            x.add16(0x5c, 0xffff);
            if x.u16(0x5c) == 0 {
                x.add8(0x4, 2);
                return;
            }
            if x.u16(0x5c) & 7 == 0 {
                for rank in (0..5u32).rev() {
                    let Some(o) = x.alloc(&pools::PEFF) else { break };
                    o.set_u8(x.b, 0, 1);
                    o.set_u8(x.b, 2, KIND);
                    o.set_u8(x.b, 3, 1);
                    o.set_u8(x.b, 0x2f, 1);
                    let owner = x.u16(f::OWNER);
                    o.set_u16(x.b, f::OWNER, owner);
                    let angle = x.rom8(ANGLES + rank);
                    o.set_u8(x.b, 0x2d, angle);
                }
            }
            return;
        }
        x.set_bit(0x1, 1, true);
        let d = x.u16(0x5c).wrapping_add(x.c.d[7] as u16);
        x.c.d[0] = (d & 1) as u32;
        if d & 1 != 0 {
            x.set_bit(0x1, 1, false);
        }
        if x.u8(0x5) == 0 {
            x.add16(0x5c, 0xffff);
            if x.u16(0x5c) == 0 {
                x.add8(0x5, 2);
                x.anim(ANIMS, 2);
            }
            x.anim_step();
            x.move_by_table_xy();
            x.reinsert();
        } else {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            x.move_by_table_xy();
            x.reinsert();
        }
    }
}

/// Type 23 (0x4d682) : lueur de la prière (kind 0x17), collée au lanceur ; la variante 1 est plus haute.
mod glow {
    use super::*;

    const ANIMS: u32 = 0x1bb402;

    pub fn run(x: &mut Cx) {
        match x.state(0x4d690) {
            0 => {
                x.add8(0x4, 2);
                follow_owner(x);
                let (mut dx, mut dz) = (0x13u16, 0x2du16);
                if x.u8(0x3) != 0 {
                    (dx, dz) = (-0x69i16 as u16, 0x3a);
                }
                x.c.d[0] = dx as u32;
                x.c.d[1] = dz as u32;
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.add16(0x10, 0xffff);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                let gesture = owner.u8(x.b, 0x7);
                x.c.d[0] = x.c.d[0] & !0xff | gesture as u32;
                if gesture != x.u8(0x5d) {
                    x.add8(0x4, 2);
                }
                x.anim_step();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4d2fe "peff_21" hit_spark::run,
    0x4d3e2 "peff_22" prayer::run,
    0x4d682 "peff_23" glow::run,
}
