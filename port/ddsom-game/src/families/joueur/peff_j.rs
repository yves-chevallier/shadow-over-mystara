//! Effets des joueurs (PEFF), dixième série : types 35 à 39, 46 et 47 (effets du bond, de l'échange, de
//! la glissade, du coup de nain, éclats des monstres touchés, pluie de l'écran de choix). Voir
//! `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;

const REINSERT: u32 = 0x1ee1a;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 35 (0x4fc1a) : effet du bond (kind 0x23, `player/spawn.rs::leap`) : posé haut devant le joueur
/// tant que celui-ci est dans l'action 0x16.
mod leap {
    use super::*;

    const ANIMS: u32 = 0x1bc60a;

    pub fn run(x: &mut Cx) {
        match x.state(0x4fc28) {
            0 => {
                x.add8(0x4, 2);
                let owner = follow_owner(x);
                x.set16(0x12, 0);
                x.set16(0x12, 0x8000);
                let (mut dx, dz) = (-0x62i16 as u16, 0x82u16);
                if owner.u8(x.b, 0x3) > 2 {
                    dx = -0x49i16 as u16;
                }
                if x.u8(0x3) != 0 {
                    dx = -0x62i16 as u16;
                }
                x.c.d[0] = dx as u32;
                x.c.d[1] = dz as u32;
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.call(REINSERT);
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x5) != 0 || owner.u16(x.b, 0x62) == 0 || owner.u8(x.b, 0x6a) != 0x16 {
                    return x.free_peff();
                }
                if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
                    x.copy_pos(owner);
                    x.set16(0x12, 0);
                    x.set16(0x12, 0x8000);
                    let dx = signed(-0x62i16 as u16, x.u8(f::FLIP) != 0);
                    x.c.d[0] = dx as u32;
                    x.c.d[1] = 0x82;
                    x.add16(0x8, dx);
                    x.add16(0xc, 0x82);
                    x.call(ANIM_STEP);
                    if x.c.cond(5) {
                        x.add8(0x4, 2);
                    }
                }
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 36 (0x4fd4a) : échange d'équipement (kind 0x24) : une étincelle posée devant le joueur.
mod swap {
    use super::*;

    const ANIMS: u32 = 0x1bc6b0;
    const OFFSETS_A: u32 = 0x4fdd4;
    const OFFSETS_B: u32 = 0x4fdec;
    const SFX: u32 = 0xe7c20;

    pub fn run(x: &mut Cx) {
        match x.state(0x4fd58) {
            0 => {
                x.add8(0x4, 2);
                let owner = follow_owner(x);
                x.add16(0x10, 0xffff);
                let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                x.c.d[0] = row;
                let table = if x.u8(0x3) != 0 { OFFSETS_B } else { OFFSETS_A };
                let dx = x.rom16(table + row);
                let dz = x.rom16(table + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                x.c.d[4] = owner.u8(x.b, 0x10d) as u32;
                x.call(SFX);
                x.call(REINSERT);
            }
            2 => {
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 37 (0x4fe38) : poussière de la glissade (kind 0x25) : suit le joueur tant qu'il glisse
/// (action 2, arme `+0x6a` inchangée).
mod dash {
    use super::*;

    const ANIMS: u32 = 0x1bc6f4;

    pub fn run(x: &mut Cx) {
        match x.state(0x4fe46) {
            0 => {
                x.add8(0x4, 2);
                pose(x);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x5) != 0
                    || owner.u8(x.b, 0x7) != 2
                    || owner.u16(x.b, 0x62) == 0
                    || owner.u8(x.b, 0x6a) != x.u8(0x5c)
                    || owner.u8(x.b, 0x10f) != 0
                {
                    return x.free_peff();
                }
                pose(x);
                if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
                    x.call(ANIM_STEP);
                }
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }

    fn pose(x: &mut Cx) {
        follow_owner(x);
        x.set16(0x12, 0);
        x.set16(0x12, 0x8000);
        let dx = signed(-0x2a_i16 as u16, x.u8(f::FLIP) != 0);
        x.c.d[0] = dx as u32;
        x.c.d[1] = 0xc;
        x.add16(0x8, dx);
        x.add16(0xc, 0xc);
    }
}

/// Type 38 (0x4ff1e) : éclat du coup (kind 0x26) : 0x18 images devant le joueur, tant que son action dure.
mod chip {
    use super::*;

    const ANIMS: u32 = 0x1bc77c;
    const OFFSETS: u32 = 0x4ff9a;

    pub fn run(x: &mut Cx) {
        match x.state(0x4ff2c) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x14, 0x18);
                let owner = follow_owner(x);
                x.set16(0x12, 0);
                owner.set_u16(x.b, 0x12, 0x8000);
                let row = ((x.u8(0x3) >> 1) as u32) << 2;
                x.c.d[0] = row;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x5) == 0 {
                    return x.free_peff();
                }
                x.add16(0x14, 0xffff);
                if x.u16(0x14) == 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 39 (0x4ffd0) : éclat d'un monstre touché (kind 0x27, créé par `0x5009e`) : une première copie
/// attend, la seconde (variante 1) reprend l'animation du monstre `+0x42` ; ils clignotent 0xe images.
mod chunk {
    use super::*;

    const ANIM_SETS: u32 = 0x396e8;
    const FRAMES: u32 = 0x11ae86;

    pub fn run(x: &mut Cx) {
        match x.state(0x4ffea) {
            0 => {
                x.c.d[0] = x.u8(0x3) as u32 * 2;
                if x.u8(0x3) == 0 {
                    x.add8(0x4, 2);
                    x.set16(0x30, 0x14);
                } else {
                    let host = x.link_a1(0x42);
                    x.add8(0x4, 2);
                    let class = host.u8(x.b, 0x10e) as u32;
                    let set = x.data32(ANIM_SETS + 4 * class);
                    x.c.a[4] = set;
                    let row = (host.u16(x.b, 0x42) as u32) << 3 & 0xffff;
                    x.c.d[1] = row;
                    let d0 = x.data16(FRAMES + row);
                    let d2 = x.data16(FRAMES + row + 2);
                    let d3 = x.data16(FRAMES + row + 4);
                    x.c.d[0] = x.c.d[0] & 0xffff_0000 | d0 as u32;
                    x.c.d[2] = d2 as u32;
                    x.c.d[3] = d3 as u32;
                    let dx = signed(d2, host.u8(x.b, f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, d3);
                    x.call(0x1a32);
                    x.set16(0x30, 0x14);
                }
            }
            2 => {
                x.c.d[0] = x.u8(0x5) as u32;
                if x.u8(0x5) == 0 {
                    x.add16(0x30, 0xffff);
                    if (x.u16(0x30) as i16) <= 0 {
                        x.add8(0x5, 2);
                        x.set16(0x30, 0xe);
                    }
                } else {
                    x.add16(0x30, 0xffff);
                    if (x.u16(0x30) as i16) <= 0 {
                        return x.free_peff();
                    }
                    let h = x.u8(0x1) ^ 2;
                    x.set8(0x1, h);
                }
            }
            _ => x.free_peff(),
        }
        if x.u8(0) != 0 {
            x.call(REINSERT);
        }
    }
}

/// Type 46 (0x51ad6) : pluie de pièces de l'écran de gain (kind 0x2e) posée sur la caméra, qui
/// clignote en retombant.
mod rain {
    use super::*;

    const ANIMS: u32 = 0x1be8b8;
    const OFFSETS: u32 = 0x51b44;
    const DELAYS: u32 = 0x51ba8;

    pub fn run(x: &mut Cx) {
        match x.state(0x51ae4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x5a, 0x70);
                let (cx, cy) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
                x.set16(0x8, cx);
                x.set16(0x10, cy);
                let row = (x.u8(0x3) as u32) << 3;
                x.c.d[0] = row;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                let delay = x.rom16(OFFSETS + row + 4);
                x.c.d[1] = dx as u32;
                x.set16(0xc, dz);
                x.set16(0x58, delay);
                x.add16(0x8, dx);
                x.add16(0x10, 0x100);
                x.anim(ANIMS, 1);
            }
            2 => {
                x.add16(0x5a, 0xffff);
                if x.u16(0x58) != 0 {
                    x.add16(0x58, 0xffff);
                    x.set_bit(0x1, 1, true);
                    return;
                }
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    let r = (x.rand_and(3) * 2) as u32;
                    let d = x.rom16(DELAYS + r);
                    x.set16(0x58, d);
                    x.anim(ANIMS, 1);
                    if (x.u16(0x5a) as i16) < 0 {
                        x.add8(0x4, 2);
                    }
                }
                let h = x.u8(0x1) ^ 2;
                x.set8(0x1, h);
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 47 (0x51bcc) : attente de 0x16 images avant de couper l'éclair de l'écran de victoire.
mod timer {
    use super::*;

    pub fn run(x: &mut Cx) {
        let state = x.state(0x51bda);
        if state == 0 {
            // l'initialisation enchaîne sur l'état 2
            x.add8(0x4, 2);
            x.set16(0x14, 0x15);
        }
        match state {
            0 | 2 => {
                x.add16(0x14, 0xffff);
                if (x.u16(0x14) as i16) >= 0 {
                    return;
                }
                if x.a5_u16(0x4ba) == 0x100 {
                    x.set_a5_u16(0x4ba, 0);
                }
                x.free_peff();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4fc1a "peff_35" leap::run,
    0x4fd4a "peff_36" swap::run,
    0x4fe38 "peff_37" dash::run,
    0x4ff1e "peff_38" chip::run,
    0x4ffd0 "peff_39" chunk::run,
    0x51ad6 "peff_46" rain::run,
    0x51bcc "peff_47" timer::run,
}
