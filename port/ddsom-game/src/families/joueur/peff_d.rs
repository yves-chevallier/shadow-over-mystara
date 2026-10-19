//! Effets des joueurs (PEFF), quatrième série : types 6 à 10 (rayons : tir de magie, impact, souffle).
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;

const REINSERT: u32 = 0x1ee1a;

/// Signe : `neg.w` quand le regard de l'objet vaut `flip_means_neg` (0 : gauche, 1 : droite).
fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Le geste du joueur (`+7`) a changé depuis la création : la durée de vie de l'effet est finie.
fn gesture_changed(x: &mut Cx, field: u32) -> bool {
    let owner = x.link_a1(f::OWNER);
    let gesture = owner.u8(x.b, 0x7);
    x.c.d[0] = x.c.d[0] & !0xff | gesture as u32;
    gesture != x.u8(field)
}

/// Fait clignoter l'objet : caché une image sur deux (`+0x5c` bascule). `hidden_when_zero` choisit la
/// phase cachée.
fn flicker(x: &mut Cx, hidden_when_zero: bool) {
    x.set_bit(0x1, 1, true);
    let v = x.u8(0x5c) ^ 1;
    x.set8(0x5c, v);
    if (v == 0) != hidden_when_zero {
        x.set_bit(0x1, 1, false);
    }
}

/// Type 6 (0x4b1b6) : faisceau lancé (magie 17). Variante 0 : rayon posé devant le lanceur tant que
/// son geste est le 4 ; variantes 1 et 2 : extrémités (selon l'octet `+0x15e` lu *au-delà* de l'objet,
/// comme l'original).
mod beam {
    use super::*;

    const ANIMS: u32 = 0x1b5ab8;
    const OFFSETS: u32 = 0x4b2dc;
    const SFX: u32 = 0xe8d4a;
    const SPAWN_TRAIL: u32 = 0x54700;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4b1c4) {
            0 if !child => init_root(x),
            0 => init_tip(x),
            2 if !child => {
                x.call(ANIM_STEP);
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x7) == 4 {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            2 => {
                if x.u8(0x33) != 0 {
                    x.set8(0x33, 0);
                    x.call(SPAWN_TRAIL);
                }
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }

    fn init_root(x: &mut Cx) {
        x.add8(0x4, 2);
        follow_owner(x);
        let dx = signed(-2i16 as u16, x.u8(f::FLIP) != 0);
        x.c.d[0] = dx as u32;
        x.c.d[1] = 0x58;
        x.add16(0x8, dx);
        x.add16(0xc, 0x58);
        x.add16(0x10, 0xffff);
        x.anim(ANIMS, 0);
    }

    fn init_tip(x: &mut Cx) {
        x.add8(0x4, 2);
        if x.u8(0x15e) == 0x17 {
            let owner = follow_owner(x);
            let _ = owner;
            let dx = signed(-0x30i16 as u16, x.u8(f::FLIP) != 0);
            x.c.d[0] = dx as u32;
            x.c.d[1] = 0x3a;
            x.c.d[2] = 0xffff_0000;
            x.add16(0x8, dx);
            x.add16(0xc, 0x3a);
            x.add32(0x10, 0xffff_0000);
        } else {
            let owner = follow_owner(x);
            let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
            x.c.d[0] = row;
            let (mut dx, dz) = offset(x, OFFSETS, row);
            dx = signed(dx, x.u8(f::FLIP) != 0);
            x.c.d[1] = dx as u32;
            x.c.d[3] = 0xffff_0000;
            x.add16(0x8, dx);
            x.add16(0xc, dz);
            x.add32(0x10, 0xffff_0000);
        }
        x.anim(ANIMS, 1);
        x.call(SFX);
    }
}

/// Type 7 (0x4b3b4) : étincelle de fin de rayon, deux de variante 0 (`+0x2f` = 0, 1) et une de
/// variante 1 qui attend la fin de son animation.
mod spark {
    use super::*;

    const ANIMS: u32 = 0x1b5bc2;
    const OFFSETS_A: u32 = 0x4b43c;
    const OFFSETS_B: u32 = 0x4b4ae;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4b3c2) {
            0 if !child => init_a(x),
            0 => init_b(x),
            2 if !child => {
                x.call(ANIM_STEP);
                flicker(x, false);
                if gesture_changed(x, 0x5d) {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            2 => {
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }

    fn init_a(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = follow_owner(x);
        let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
        x.c.d[0] = row;
        let (dx, dz) = offset(x, OFFSETS_A, row);
        let depth: u32 = if x.u8(0x2f) == 0 { 0xfffe_8000 } else { 0xffff_0000 };
        x.c.d[3] = depth;
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.c.d[1] = dx as u32;
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add32(0x10, depth);
        let n = x.u8(0x2f) as u32;
        x.anim(ANIMS, n);
    }

    fn init_b(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
        x.c.d[0] = row;
        let (dx, dz) = offset(x, OFFSETS_B, row);
        let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
        x.c.d[1] = dx as u32;
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 1);
        x.anim(ANIMS, 2);
    }
}

/// Type 8 (0x4b530) : lueur de 8 images à la bouche du lanceur.
mod glow {
    use super::*;

    pub fn run(x: &mut Cx) {
        match x.state(0x4b53e) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5c, 8);
                follow_owner(x);
                let dx = signed(-0x16i16 as u16, x.u8(f::FLIP) != 0);
                x.c.d[0] = dx as u32;
                x.c.d[1] = 0x66;
                x.add16(0x8, dx);
                x.add16(0xc, 0x66);
                x.add16(0x10, 1);
                x.anim(0x1b5d8e, 0);
            }
            2 => {
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) == 0 {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 9 (0x4b61a) : rayon (variante 0, deux exemplaires `+0x2f` = 0, 1 : le cœur du rayon) et son
/// générateur (variante 1) qui lance des tirs (PSHL 6) selon le joueur.
mod ray {
    use super::*;

    const ANIMS: u32 = 0x1b5db2;
    const SHOT_COUNTS: u32 = 0x4b75a;
    const ALLOC_PSHL: u32 = 0x225e;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4b628) {
            0 if !child => init_core(x),
            0 => init_generator(x),
            2 if !child => {
                x.call(ANIM_STEP);
                flicker(x, true);
                if gesture_changed(x, 0x5f) {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            2 => {
                x.c.d[0] = x.u8(0x5) as u32;
                match x.u8(0x5) {
                    0 => fire(x),
                    _ => {
                        if x.u8(0x5d) == 0 {
                            x.add8(0x4, 2);
                            let owner = x.link_a1(f::OWNER);
                            x.c.a[6] = owner.0 | 0xffff_0000;
                            owner.set_u16(x.b, 0x168, 1);
                        }
                    }
                }
            }
            _ => x.free_peff(),
        }
    }

    fn init_core(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = follow_owner(x);
        let (mut dx, mut dz) = (-4i16 as u16, 0x34u16);
        if owner.u8(x.b, 0x15e) != 0x27 {
            dx = -0x18i16 as u16;
            dz = 0x45;
        }
        x.c.d[0] = dx as u32;
        x.c.d[1] = dz as u32;
        let depth: u32 = if x.u8(0x2f) == 0 { 0xfffe_8000 } else { 0xffff_0000 };
        x.c.d[2] = depth;
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add32(0x10, depth);
        let n = x.u8(0x2f) as u32;
        x.anim(ANIMS, n);
    }

    fn init_generator(x: &mut Cx) {
        x.add8(0x4, 2);
        follow_owner(x);
    }

    /// Lance les tirs du rayon : un par valeur de la table du joueur (`+0x10c`).
    fn fire(x: &mut Cx) {
        x.add8(0x5, 2);
        let owner = x.link_a1(f::OWNER);
        x.c.a[6] = owner.0 | 0xffff_0000;
        x.set8(0x5d, 0);
        x.set8(0x5e, 0);
        let player = owner.u8(x.b, 0x10c);
        let mut remaining = x.rom8(SHOT_COUNTS + player as u32);
        x.c.d[4] = remaining as u32;
        let mut created = 0u8;
        loop {
            let Some(shot) = x.alloc(ALLOC_PSHL) else { break };
            shot.set_u8(x.b, 0, 1);
            shot.set_u8(x.b, 2, 6);
            shot.set_u16(x.b, f::OWNER, owner.0 as u16);
            shot.set_u16(x.b, 0x5c, x.o.0 as u16);
            shot.set_u8(x.b, 3, remaining);
            created += 1;
            if remaining == 0 {
                break;
            }
            remaining -= 1;
        }
        x.c.d[5] = created as u32;
        x.set8(0x5d, created);
    }
}

/// Type 10 (0x4b7c8) : impact du rayon (variante 0, deux exemplaires) et poussière finale (variante 1).
mod impact {
    use super::*;

    const ANIMS: u32 = 0x1b5ecc;
    const OFFSETS: u32 = 0x4b83e;
    const BURST: u32 = 0x52578;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4b7d6) {
            0 => init(x),
            2 if !child => {
                x.call(ANIM_STEP);
                flicker(x, true);
                if gesture_changed(x, 0x5d) {
                    x.add8(0x4, 2);
                }
                x.call(REINSERT);
            }
            2 => {
                x.call(ANIM_STEP);
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x33) != 0 {
                    x.add8(0x4, 2);
                    owner.set_u8(x.b, 0x33, 0);
                    x.call(BURST);
                }
                x.call(REINSERT);
            }
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = follow_owner(x);
        let mut row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 3;
        if x.u8(0x3) != 0 {
            row += 4;
        }
        x.c.d[0] = row;
        let (dx, dz) = offset(x, OFFSETS, row);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.c.d[1] = dx as u32;
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
        let n = x.u8(0x3) as u32;
        x.anim(ANIMS, n);
    }
}

handlers! {
    0x4b1b6 "peff_6" 120 beam::run,
    0x4b3b4 "peff_7" 120 spark::run,
    0x4b530 "peff_8" 120 glow::run,
    0x4b61a "peff_9" 120 ray::run,
    0x4b7c8 "peff_10" 120 impact::run,
}
