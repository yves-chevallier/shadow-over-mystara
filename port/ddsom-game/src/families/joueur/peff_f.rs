//! Effets des joueurs (PEFF), sixième série : types 15 (traînée de l'arme), 16 (bulle d'objet), 18
//! (pluie de pierres) et 19 (éclair). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;

const REINSERT: u32 = 0x1ee1a;
const FLASH_ON: u32 = 0x1ac68;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 15 (0x4c5be) : traînée de l'arme (armes 0x10 à 0x13, rang `+2f`). Elle suit le porteur à des
/// décalages lus dans une fiche de la ROM (`+3` : numéro de la pose) et se colore en cycle pour le rang 2.
mod trail {
    use super::*;

    const ANIMS: u32 = 0x1b6eda;
    const POSES: u32 = 0x4c6de;
    const COLORS: u32 = 0x4c66e;

    pub fn run(x: &mut Cx) {
        match x.state(0x4c5cc) {
            0 => {
                x.add8(0x4, 2);
                if x.u8(0x2f) == 2 {
                    x.set8(0x5e, 0);
                    x.set8(0x5d, 1);
                    x.set8(0x2a, 1);
                    x.set8(0x2b, 0x14);
                }
                let d3 = place(x);
                x.c.d[0] = d3 as u32;
                let rank = (x.u8(0x2f) as u32) << 3;
                x.c.d[1] = rank;
                let anim = (d3 as u32).wrapping_add(rank) & 0xffff;
                x.anim(ANIMS, anim);
                cycle(x);
                x.call(REINSERT);
            }
            2 => {
                if x.u8(0x5c) != 0 {
                    place(x);
                }
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
                cycle(x);
            }
            _ => x.free_peff(),
        }
    }

    /// `0x4c674` : se place sur le porteur ; renvoie le mot D3 de la fiche (numéro d'animation).
    fn place(x: &mut Cx) -> u16 {
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let player_row = (owner.u8(x.b, 0x3) as u32) << 2;
        let records = x.data32(POSES + player_row);
        x.c.a[2] = records;
        let row = (x.u8(0x3) as u32) << 3;
        x.c.d[0] = row;
        let dx = x.data16(records + row);
        let dz = x.data16(records + row + 2);
        let d3 = x.data16(records + row + 4);
        let d4 = x.data8(records + row + 6);
        let d5 = x.data8(records + row + 7);
        x.c.d[1] = dx as u32;
        x.c.d[2] = dz as u32;
        x.c.d[3] = d3 as u32;
        x.c.d[4] = d4 as u32;
        x.c.d[5] = d5 as u32;
        if d5 != 0 {
            let f = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, f);
        }
        let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
        x.set8(0x5c, d4);
        d3
    }

    /// `0x4c63c` : les traînées de rang 2 changent de couleur toutes les 4 images.
    fn cycle(x: &mut Cx) {
        if x.u8(0x2f) != 2 {
            return;
        }
        x.add8(0x5d, 0xff);
        if x.u8(0x5d) != 0 {
            return;
        }
        x.set8(0x5d, 4);
        x.add8(0x5e, 1);
        if x.u8(0x5e) > 5 {
            x.set8(0x5e, 0);
        }
        let k = x.u8(0x5e) as u32;
        x.c.d[0] = k;
        let c = x.rom8(COLORS + k);
        x.set8(0x2b, c);
    }
}

/// Type 16 (0x4c948) : bulle d'objet au-dessus d'un joueur (jeu 0x1b758c), qui y reste `+0x58` images.
mod bubble {
    use super::*;

    const ANIMS: u32 = 0x1b758c;
    const OFFSETS: u32 = 0x4c9e0;

    pub fn run(x: &mut Cx) {
        match x.state(0x4c956) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x58, 0x12c);
                let owner = x.link_a1(f::OWNER);
                let kind = owner.u8(x.b, 0x2) as u32;
                x.anim(ANIMS, kind);
            }
            2 => {
                x.add16(0x58, 0xffff);
                if x.u16(0x58) == 0 {
                    x.add8(0x4, 2);
                }
                let owner = x.link_a1(f::OWNER);
                x.copy_pos(owner);
                x.set16(0x12, 0);
                owner.set_u16(x.b, 0x12, 0x8000);
                let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                x.c.d[0] = row;
                let mut dx = x.rom16(OFFSETS + row);
                let mut dz = x.rom16(OFFSETS + row + 2);
                if x.a5_u8(0x59) != 0 {
                    dz = dz.wrapping_sub(0x30);
                }
                dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.call(ANIM_STEP);
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 18 (0x4ccca) : nuage de la pluie de pierres (magie 18 du clerc) : posé en haut à droite de l'écran
/// avec un tapis de pierres (PSHL 17), il monte puis redescend à la fin de la prière.
mod storm {
    use super::*;

    const ANIMS: u32 = 0x1ba964;
    const SPAWN_STONES: u32 = 0x53746;

    pub fn run(x: &mut Cx) {
        match x.state(0x4ccd8) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x58, 0x60);
                let (cx, cy) = if x.a5_u8(a5::USE_CAM2) != 0 {
                    (x.a5_u16(a5::CAM2_X), x.a5_u16(a5::CAM2_Y))
                } else {
                    (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y))
                };
                x.set16(0x8, cx.wrapping_add(0xc0));
                x.set16(0xc, cy.wrapping_add(0x100));
                x.anim(ANIMS, 0);
                x.call(SPAWN_STONES);
                x.call(REINSERT);
            }
            2 => {
                x.c.d[0] = x.c.d[0] & !0xff | x.u8(0x5) as u32;
                match x.u8(0x5) {
                    0 => {
                        x.add8(0x58, 0xff);
                        if x.u8(0x58) == 0 {
                            x.add8(0x5, 2);
                        }
                        let t = x.u8(0x58) & 1;
                        x.c.d[0] = x.c.d[0] & !0xff | t as u32;
                        if t != 0 {
                            x.add16(0xc, 0xffff);
                        }
                    }
                    2 => {
                        let owner = x.link_a1(f::OWNER);
                        if owner.u8(x.b, 0x7) == 6 {
                            x.add8(0x5, 2);
                            x.set8(0x58, 0x10);
                        }
                    }
                    _ => {
                        x.add16(0xc, 4);
                        x.add8(0x58, 0xff);
                        if x.u8(0x58) == 0 {
                            x.add8(0x4, 2);
                        }
                    }
                }
                x.call(ANIM_STEP);
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 19 (0x4ce0c) : éclair (magie 19 de l'elfe) : le corps (variante 0) et quatre étincelles de coin
/// (variante 1, rang `+2f`) qui clignotent, éclairent l'écran et se répètent `+0x5a` fois.
mod flash {
    use super::*;

    const ANIMS: u32 = 0x1baa5c;
    const CORNERS: u32 = 0x4ceb2;
    const FLIPS: u32 = 0x4ceae;
    const DELAYS: u32 = 0x4cec2;
    const SFX_BODY: u32 = 0xe981e;
    const SFX_CORNER: u32 = 0xe9888;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4ce1a) {
            0 if !child => {
                x.add8(0x4, 2);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
                x.call(SFX_BODY);
            }
            0 => init_corner(x),
            2 if !child => body(x),
            2 => corner(x),
            _ => x.free_peff(),
        }
    }

    fn init_corner(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set_bit(0x1, 1, true);
        let (cx, cy) = if x.a5_u8(a5::USE_CAM2) != 0 {
            (x.a5_u16(a5::CAM2_X), x.a5_u16(a5::CAM2_Y))
        } else {
            (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y))
        };
        x.set16(0x8, cx);
        x.set16(0xc, cy);
        let k = x.u8(0x2f) as u32;
        x.c.d[3] = k;
        let row = k << 2;
        x.c.d[0] = row;
        let dx = x.rom16(CORNERS + row);
        let dz = x.rom16(CORNERS + row + 2);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        let flip = x.rom8(FLIPS + k);
        x.set8(f::FLIP, flip);
        let delay = x.rom16(DELAYS + 2 * k);
        x.set16(0x58, delay);
        let anim = if k > 1 { 2 } else { 1 };
        x.anim(ANIMS, anim);
    }

    fn body(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if x.u8(0x5d) != owner.u8(x.b, 0x7) {
            x.add8(0x4, 2);
        }
        if x.u8(0x33) != 0 {
            x.set8(0x33, 0);
            x.set8(0x5c, 1);
            x.add16(0x10, 2);
            spawn_corners(x);
        }
        if x.u8(0x5c) != 0 {
            x.add16(0x8, 2);
            x.add16(0xc, 1);
            if x.u16(0x58) & 1 == 0 {
                x.add16(0x8, 0xfffc);
                x.add16(0xc, 0xfffe);
            }
        }
        x.add16(0x58, 1);
        let t = x.u16(0x58);
        x.c.d[0] = t as u32;
        if t > 0x140 {
            x.set_bit(0x1, 1, true);
            if t & 1 == 0 {
                x.set_bit(0x1, 1, false);
            }
        }
        x.call(ANIM_STEP);
    }

    /// `0x4cfbc` : quatre étincelles de coin, puis l'éclair d'écran.
    fn spawn_corners(x: &mut Cx) {
        for rank in (0..4u8).rev() {
            let Some(o) = x.alloc(&pools::PEFF) else { break };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 0x13);
            o.set_u8(x.b, 3, 1);
            o.set_u8(x.b, 0x2f, rank);
            o.set_u8(x.b, 0x5a, 2);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
        }
        x.call(FLASH_ON);
    }

    fn corner(x: &mut Cx) {
        if x.u16(0x58) != 0 {
            x.add16(0x58, 0xffff);
            if x.u16(0x58) != 0 {
                return;
            }
            x.set_bit(0x1, 1, false);
            x.set8(0x5b, 2);
            x.set_a5_u16(0x4ba, 0x100);
            x.call(SFX_CORNER);
        }
        x.call(ANIM_STEP);
        if x.c.cond(5) {
            x.add8(0x5a, 0xff);
            if x.u8(0x5a) == 0 {
                x.add8(0x4, 2);
                return;
            }
            x.set16(0x58, 0x18);
            x.set_bit(0x1, 1, true);
            let anim = if x.u8(0x2f) > 1 { 2 } else { 1 };
            x.anim(ANIMS, anim);
        } else {
            if x.u8(0x5b) != 0 {
                x.add8(0x5b, 0xff);
                if x.u8(0x5b) == 0 {
                    x.set_a5_u16(0x4ba, 0);
                }
            }
            x.call(REINSERT);
        }
    }
}

handlers! {
    0x4c5be "peff_15" 120 trail::run,
    0x4c948 "peff_16" 100 bubble::run,
    0x4ccca "peff_18" 100 storm::run,
    0x4ce0c "peff_19" 120 flash::run,
}
