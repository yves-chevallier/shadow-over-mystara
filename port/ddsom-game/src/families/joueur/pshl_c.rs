//! Tirs des joueurs (PSHL), troisième série : types 12 à 17 (objets lancés des magiciens et du
//! clerc : jarre, flèches, gerbes de flamme). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, wr16};
use crate::objects::pools;
use crate::objects::Courant;

const SHAKE: u32 = 0x2824;
const FLASH_ON: u32 = 0x1ac68;
const FLASH_PULSE: u32 = 0x1ac86;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Compteurs d'objets en vie (`A5-0x5fba`, `-0x5fb8`, `-0x5fb6`, `-0x5fb4`) : adresses absolues.
const COUNTER_14: u32 = 0xff2046;
const COUNTER_15: u32 = 0xff2048;
const COUNTER_16: u32 = 0xff204a;

/// Décrémente un compteur de la RAM (`subq.w #1`).
fn decrement(x: &mut Cx, address: u32) {
    let v = rd16(x.b, address).wrapping_sub(1);
    wr16(x.b, address, v);
}

/// Type 12 (0x530bc) : jet de flammes. Variante 0 : projectile à vitesse longue (comme le type 3) qui
/// retombe et crée trois gerbes (variante 1) ; variante 1 : gerbe qui attend `+0x5c` images puis brûle.
mod jet {
    use super::*;

    const OFFSETS: u32 = 0x53150;
    const ANIMS: u32 = 0x15c698;
    const SFX: u32 = 0xeb364;
    const KIND: u8 = 0xc;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x530ca) {
            0 if !child => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                x.add16(0x10, 0xffff);
                x.set32(0x14, 0x180_0000);
                x.set16(0x18, 0x300);
                x.set16(0x1a, -0x38i16 as u16);
                let row = (owner.u8(x.b, 0x3) as u32) << 2 & 0xffff;
                x.c.d[0] = row;
                x.c.a[4] = OFFSETS;
                let dx = x.data16(OFFSETS + row);
                let dz = x.data16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let mut dx = dx;
                if x.u8(f::FLIP) == 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(0x14);
                    x.set16(0x14, vx.wrapping_neg());
                }
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                x.reinsert();
                active(x);
            }
            0 => {
                x.add16(0x5c, 0xffff);
                if x.u16(0x5c) == 0 {
                    x.add8(0x4, 2);
                    x.anim(ANIMS, 1);
                    x.reinsert();
                    x.call(SFX);
                }
            }
            2 => active(x),
            _ => x.free_pshl(),
        }
    }

    fn active(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
            return;
        }
        x.move_xz();
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
        let (mut offset, mut delay) = (0u16, 1u16);
        for _ in 0..3 {
            let Some(o) = x.alloc(&pools::PSHL) else { return };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, KIND);
            o.set_u8(x.b, 3, 1);
            let (px, py) = (x.u16(0x8), x.u16(0x10));
            o.set_u16(x.b, 0x8, px.wrapping_add(offset));
            o.set_u16(x.b, 0x10, py);
            o.set_u8(x.b, f::POWER, 0x10);
            o.set_u8(x.b, f::POWER2, 0x10);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            o.set_u16(x.b, 0x5c, delay);
            offset = offset.wrapping_add(signed(0x28, x.u8(f::FLIP) == 0));
            delay += 8;
            x.c.d[4] = offset as u32;
            x.c.d[3] = delay as u32;
        }
    }
}

/// Type 13 (0x53290) : objet lancé consommant un objet d'inventaire (jeu 0x15b0a2) ; il retombe avec
/// la vitesse que lui a donnée son créateur.
mod item {
    use super::*;

    const OFFSETS: u32 = 0x53314;
    const ANIMS: u32 = 0x15b0a2;

    pub fn run(x: &mut Cx) {
        match x.state(0x5329e) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                let page = (owner.u8(x.b, 0x15d) as u32) << 4 & 0xffff;
                x.c.d[0] = page;
                let base = ext(owner).wrapping_add(0x120);
                x.c.a[4] = base;
                let count = owner.0 + 0x120 + page + 8;
                let left = x.data8(count).wrapping_sub(1);
                crate::mem::wr8(x.b, count, left);
                if left == 0 {
                    crate::mem::wr8(x.b, owner.0 + 0x120 + page, 0x26);
                }
                x.copy_pos(owner);
                x.add16(0x10, 0xffff);
                let row = (owner.u8(x.b, 0x3) as u32) << 2 & 0xffff;
                x.c.d[0] = row;
                x.c.a[4] = OFFSETS;
                let dx = x.data16(OFFSETS + row);
                let dz = x.data16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(0xc) as i16) < 0 {
                    x.add8(0x4, 2);
                    x.set32(0xc, 0);
                }
                x.reinsert();
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 14 (0x53412) : flamme de jet (compteur `A5-0x5fba`) qui attend `+0x5c` images puis brûle.
mod flame {
    use super::*;

    const ANIMS: u32 = 0x15c7a6;
    const SFX: u32 = 0xe83e2;

    pub fn run(x: &mut Cx) {
        match x.state(0x53420) {
            0 => {
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 6);
                x.set8(f::POWER2, 6);
                x.anim(ANIMS, 0);
                x.reinsert();
                x.call(SFX);
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => {
                decrement(x, COUNTER_14);
                x.free_pshl();
            }
        }
    }
}

/// Type 15 (0x534d6) : projectile de lanceur (compteur `A5-0x5fb8`), qui avance de 4 pixels par image
/// pendant `+0x5c` images.
mod volley {
    use super::*;

    const OFFSETS: u32 = 0x53580;
    const LIFETIMES: u32 = 0x535a4;
    const ANIMS: u32 = 0x15c85a;

    pub fn run(x: &mut Cx) {
        match x.state(0x534e4) {
            0 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x5) != 0 {
                    return die(x);
                }
                x.add8(0x4, 2);
                x.set8(0x2a, 1);
                x.set8(0x2b, 0x14);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 6);
                x.set8(f::POWER2, 6);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                let mut v = owner.u8(x.b, 0x6a) as u32;
                x.c.d[0] = v;
                if owner.u16(x.b, 0xc) != 0 {
                    v = 0x10;
                }
                let row = v * 2;
                x.c.d[0] = row;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.add16(0x10, 0xffff);
                let class = owner.u8(x.b, 0x10c) as u32;
                x.c.d[0] = class;
                let life = x.rom8(LIFETIMES + class);
                x.set8(0x5c, life);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                let dx = signed(4, x.u8(f::FLIP) == 0);
                x.set_d0w(dx);
                x.add16(0x8, dx);
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) == 0 {
                    x.add8(0x4, 2);
                }
                x.anim_step();
                x.cull_offscreen();
            }
            _ => die(x),
        }
    }

    fn die(x: &mut Cx) {
        decrement(x, COUNTER_15);
        x.free_pshl();
    }
}

/// Type 16 (0x5369a) : étoile de lanceur (compteur `A5-0x5fb6`), animation à couleurs tournantes.
mod star {
    use super::*;

    const COLORS: u32 = 0x53734;
    const ANIMS: u32 = 0x15c8a2;
    const SFX: u32 = 0xe8454;

    pub fn run(x: &mut Cx) {
        match x.state(0x536a8) {
            0 => {
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set8(0x5d, 0);
                x.set8(0x2b, 0x12);
                x.set8(0x2a, 1);
                x.set8(0x5c, 0x10);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 6);
                x.set8(f::POWER2, 6);
                x.anim(ANIMS, 0);
                x.reinsert();
                if x.u8(0x3) == 0 {
                    x.call(SFX);
                }
            }
            2 => {
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) == 0 {
                    x.add8(0x4, 2);
                }
                let frame = x.a5_u8(a5::FRAME) & 3;
                x.c.d[0] = x.c.d[0] & !0xff | frame as u32;
                if frame == 0 {
                    let n = x.u8(0x5d).wrapping_add(1) & 7;
                    x.set8(0x5d, n);
                    x.c.d[0] = n as u32;
                    let c = x.rom8(COLORS + n as u32);
                    x.set8(0x2b, c);
                }
                x.anim_step();
            }
            _ => {
                decrement(x, COUNTER_16);
                x.free_pshl();
            }
        }
    }
}

/// Type 17 (0x537fa) : pierre qui part en arc, s'écrase au sol avec secousse d'écran et éclair.
mod stone {
    use super::*;

    const ANIMS: u32 = 0x15c90a;
    const SFX: u32 = 0xe8ab8;

    pub fn run(x: &mut Cx) {
        match x.state(0x53808) {
            0 => {
                x.add16(0x5c, 0xffff);
                if x.u16(0x5c) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                x.set8(f::POWER, 0xa0);
                x.set8(f::POWER2, 0xa0);
                x.set16(0x14, -0x400i16 as u16);
                x.set16(0x18, -0x800i16 as u16);
                let mut dx = 0x60u16;
                if x.u8(f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(0x14);
                    x.set16(0x14, vx.wrapping_neg());
                }
                x.set_d0w(dx);
                x.add16(0x8, dx);
                x.anim(ANIMS, 0);
                x.reinsert();
            }
            2 => match x.u8(0x5) {
                0 => {
                    x.move_xz();
                    if (x.u16(0xc) as i16) >= 0 {
                        x.anim_step();
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set32(0xc, 0);
                    x.set16(0x5c, 4);
                    x.set_d0w(8);
                    x.call(SHAKE);
                    x.anim(ANIMS, 1);
                    x.call(SFX);
                }
                2 => {
                    x.add16(0x5c, 0xffff);
                    if x.u16(0x5c) == 0 {
                        x.add8(0x5, 2);
                        x.set16(0x5c, 4);
                        x.set_a5_u16(0x4ba, 0x100);
                        x.call(FLASH_ON);
                    }
                    x.anim_step();
                }
                4 => {
                    x.add16(0x5c, 0xffff);
                    if x.u16(0x5c) == 0 {
                        x.add8(0x5, 2);
                        x.set16(0x5c, 2);
                        x.call(FLASH_PULSE);
                    }
                    x.anim_step();
                }
                _ => {
                    if x.u16(0x5c) != 0 {
                        x.add16(0x5c, 0xffff);
                        if x.u16(0x5c) != 0 {
                            return;
                        }
                        x.set_a5_u16(0x4ba, 0);
                    }
                    if x.anim_step() {
                        x.add8(0x4, 2);
                    }
                }
            },
            _ => x.free_pshl(),
        }
    }
}

handlers! {
    0x530bc "pshl_12" jet::run,
    0x53290 "pshl_13" item::run,
    0x53412 "pshl_14" flame::run,
    0x534d6 "pshl_15" volley::run,
    0x5369a "pshl_16" star::run,
    0x537fa "pshl_17" stone::run,
}
