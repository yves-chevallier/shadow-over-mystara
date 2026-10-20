//! Tirs des joueurs (classe PSHL, pool de 18 objets de 0x60 octets, table 0x39908), première série :
//! types 0 à 5. Voir `docs/familles/joueur.md`.
//!
//! Les tirs naissent d'un créateur (`player/spawn.rs`) qui fixe leur genre, leur propriétaire (`+0x50`),
//! leurs boîtes (`+0x2e`), la puissance (`+0x45`, `+0x46`) et la vitesse ; le gestionnaire les fait vivre.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, wr16};

const REINSERT: u32 = 0x1ee1a;
const TERRAIN_OF_OBJECT: u32 = 0x24a8;
const SFX_HIT_WALL: u32 = 0xe7376;
const SFX_LAND: u32 = 0xe73e8;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Le tir est arrêté par le terrain : terrains 5 à 8 sans le bit 6 de l'attribut (D2).
fn stopped_by_terrain(x: &mut Cx) -> bool {
    x.call(TERRAIN_OF_OBJECT);
    let terrain = x.c.d[2];
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | ((terrain & 0x3f).wrapping_sub(5) & 0xffff);
    (terrain & 0x3f).wrapping_sub(5) as u16 <= 3 && terrain & 0x40 == 0
}

/// Clignotement final : l'objet est caché une image sur deux (parité du compteur `+0x50`).
fn blink_by_timer(x: &mut Cx) {
    x.set_bit(0x1, 1, true);
    let t = x.u8(0x50);
    x.c.d[0] = x.c.d[0] & !0xff | (t & 1) as u32;
    if t & 1 != 0 {
        x.set_bit(0x1, 1, false);
    }
}

/// Type 0 (0x51c64) : marqueur de coup suivant son porteur (`+0x5c`).
mod marker {
    use super::*;

    const ANIMS: u32 = 0x15ad38;

    pub fn run(x: &mut Cx) {
        match x.state(0x51c72) {
            0 => {
                x.add8(0x4, 2);
                let carrier = x.link_a1(0x5c);
                x.set8(f::BOX_SET, 6);
                let kind = carrier.u8(x.b, 0x2) as u32;
                x.c.d[0] = kind;
                x.copy_pos(carrier);
                x.anim(ANIMS, kind);
                x.set32(0x34, 0);
                x.call(REINSERT);
            }
            2 => {
                let carrier = x.link_a1(0x5c);
                for off in [0x8, 0x10] {
                    let v = carrier.u16(x.b, off);
                    x.set16(off, v);
                }
                let id = carrier.u8(x.b, 0x49);
                x.c.d[0] = x.c.d[0] & !0xff | id as u32;
                if id != x.u8(0x42) {
                    return x.free_pshl();
                }
                let z = carrier.u16(x.b, 0xc);
                x.set16(0xc, z);
                if x.u8(0x5) == 0 {
                    if (z as i16) > 0 {
                        x.add8(0x5, 2);
                    }
                    x.call(REINSERT);
                } else if z == 0 {
                    x.free_pshl();
                } else {
                    x.call(REINSERT);
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Types 1 et 9 (0x51da6) : tir d'objet lancé (kind 1 : jeu 0x15ae8a, kind 9 : jeu 0x15c552) qui vole
/// en ligne droite, rebondit sur l'impact, retombe puis clignote.
mod thrown {
    use super::*;

    const OFFSETS: u32 = 0x51e16;
    const ANIMS_A: u32 = 0x15ae8a;
    const ANIMS_B: u32 = 0x15c552;
    const SFX_START: u32 = 0xeb84a;

    pub fn run(x: &mut Cx) {
        match x.state(0x51db4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.link_a1(f::OWNER);
                for off in [0x8, 0xc] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                let y = owner.u32(x.b, 0x10).wrapping_sub(1);
                x.set32(0x10, y);
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
                pose(x);
                x.call(SFX_START);
                x.call(REINSERT);
            }
            2 => {
                if x.u8(f::HIT_FLAGS) & 2 != 0 {
                    x.call(SFX_HIT_WALL);
                    return x.free_pshl();
                }
                if !stopped_by_terrain(x) {
                    x.call(MOVE_XZ);
                    return x.call(CULL);
                }
                x.add8(0x4, 2);
                x.set8(0x0, 2);
                x.set8(0x50, 2);
                x.set16(0x18, 0x300);
                x.set16(0x1a, -0x30i16 as u16);
                x.set16(0x14, 0x100);
                x.c.d[0] = 1;
                if x.u8(0x2d) == 2 {
                    x.set16(0x14, 0x100u16.wrapping_neg());
                    x.c.d[0] = 0xff;
                }
                pose(x);
                x.call(SFX_LAND);
            }
            4 => {
                if x.u8(0x5) == 0 {
                    x.add8(0x50, 0xff);
                    if x.u8(0x50) == 0 {
                        x.set8(0x50, 2);
                        let step: u8 = if (x.u16(0x14) as i16) < 0 { 0xff } else { 1 };
                        x.c.d[0] = x.c.d[0] & !0xff | step as u32;
                        let angle = x.u8(0x2d).wrapping_add(step);
                        x.set8(0x2d, angle);
                        x.c.d[0] = (angle & 7) as u32;
                        x.anim(ANIMS_A, (angle & 7) as u32);
                    }
                    x.call(MOVE_XZ);
                    if (x.u16(0xc) as i16) >= 0 {
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set8(0x50, 0x3c);
                    x.set8(0x2d, 2);
                    if (x.u16(0x14) as i16) < 0 {
                        x.set8(0x2d, 6);
                    }
                    pose(x);
                } else {
                    x.add8(0x50, 0xff);
                    if x.u8(0x50) == 0 {
                        x.add8(0x4, 2);
                    }
                    blink_by_timer(x);
                }
            }
            _ => x.free_pshl(),
        }
    }

    /// Animation d'après l'angle `+0x2d` (`0x51f48`).
    fn pose(x: &mut Cx) {
        let angle = (x.u8(0x2d) & 7) as u32;
        x.c.d[0] = angle;
        let set = if x.u8(0x2) == 1 { ANIMS_A } else { ANIMS_B };
        x.c.a[4] = set;
        x.call(0x1a32);
    }
}

/// Types 2, 10 et 11 (0x5204e) : tir lancé en ligne droite (kinds 2, 0xa, 0xb : jeux 0x15af1c,
/// 0x15c5e4, 0x15c62a), qui s'écrase au sol, s'élève un peu puis clignote.
mod flung {
    use super::*;

    const OFFSETS: u32 = 0x520f6;
    const SFX_START: u32 = 0xeb7d8;

    fn set(x: &mut Cx) -> u32 {
        match x.u8(0x2) {
            2 => 0x15af1c,
            0xa => 0x15c5e4,
            _ => 0x15c62a,
        }
    }

    pub fn run(x: &mut Cx) {
        match x.state(0x5205c) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.link_a1(f::OWNER);
                for off in [0x8, 0xc] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                let y = owner.u32(x.b, 0x10).wrapping_sub(1);
                x.set32(0x10, y);
                let mut row = (owner.u8(x.b, 0x3) >> 1) as u32;
                x.c.d[0] = row;
                if row == 2 && x.u16(0xc) != 0 {
                    row += 4;
                }
                let row = row << 2;
                x.c.d[0] = row;
                x.c.a[4] = OFFSETS;
                let dx = x.data16(OFFSETS + row);
                let dz = x.data16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) == 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                let a = set(x);
                x.anim(a, 0);
                x.call(SFX_START);
                x.call(REINSERT);
            }
            2 => {
                if x.u8(f::HIT_FLAGS) & 2 != 0 {
                    x.call(SFX_HIT_WALL);
                    return x.free_pshl();
                }
                if !stopped_by_terrain(x) {
                    x.call(ANIM_STEP);
                    x.call(MOVE_X);
                    return x.call(CULL);
                }
                x.add8(0x4, 2);
                x.set8(0x0, 2);
                x.set8(0x50, 4);
                let a = set(x);
                x.anim(a, 1);
                x.call(SFX_LAND);
            }
            4 => {
                x.c.d[0] = x.u8(0x5) as u32;
                match x.u8(0x5) {
                    0 => {
                        x.call(ANIM_STEP);
                        x.add8(0x50, 0xff);
                        if x.u8(0x50) == 0 {
                            x.add8(0x5, 2);
                            x.set16(0x18, 0);
                            x.set16(0x1a, -0x20i16 as u16);
                        }
                    }
                    2 => {
                        x.call(MOVE_Z);
                        if (x.u16(0xc) as i16) < 0 {
                            x.add8(0x5, 2);
                            x.set8(0x50, 0x3c);
                        }
                    }
                    _ => {
                        x.add8(0x50, 0xff);
                        if x.u8(0x50) == 0 {
                            x.add8(0x4, 2);
                        }
                        blink_by_timer(x);
                    }
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 3 (0x52234) : tir à vitesse longue (le long `+0x14` = 0x1800000), qui retombe, puis s'arrête et
/// se consume (30 + 80 images).
mod arc {
    use super::*;

    const OFFSETS: u32 = 0x522c0;
    const ANIMS: u32 = 0x15af62;
    const SFX_BURST: u32 = 0xeb3d6;
    const SFX_MID: u32 = 0xeb280;

    pub fn run(x: &mut Cx) {
        match x.state(0x52242) {
            0 => {
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
                    x.c.d[1] = dx as u32;
                }
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
                active(x);
            }
            2 => active(x),
            _ => x.free_pshl(),
        }
    }

    fn active(x: &mut Cx) {
        x.c.d[0] = x.c.d[0] & !0xff | x.u8(0x5) as u32;
        match x.u8(0x5) {
            0 => {
                x.call(MOVE_XZ);
                if (x.u16(0xc) as i16) >= 0 {
                    return;
                }
                x.add8(0x5, 2);
                x.set32(0xc, 0);
                x.set16(0x5c, 0x50);
                x.anim(ANIMS, 1);
                x.call(SFX_BURST);
            }
            2 => {
                if x.u16(0x5c) == 0x48 {
                    x.call(SFX_MID);
                }
                x.add16(0x5c, 0xffff);
                if x.u16(0x5c) == 0 {
                    x.add8(0x5, 2);
                    x.set16(0x5c, 0x50);
                    x.anim(ANIMS, 2);
                }
                x.call(ANIM_STEP);
            }
            _ => {
                x.call(ANIM_STEP);
                x.add16(0x5c, 0xffff);
                if x.u16(0x5c) == 0 {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, true);
                let t = x.u16(0x5c);
                x.c.d[0] = (t & 1) as u32;
                if t & 1 != 0 {
                    x.set_bit(0x1, 1, false);
                }
            }
        }
    }
}

/// Type 4 (0x523c4) : projectile qui rebondit sur le terrain (hache, `+0x5c` rebonds) puis se
/// consume en clignotant (`+0x5d` : 0x20 images).
mod bouncer {
    use super::*;

    const OFFSETS: u32 = 0x52464;
    const ANIMS: u32 = 0x15b0a2;
    const SFX_START: u32 = 0xeb8bc;

    pub fn run(x: &mut Cx) {
        match x.state(0x523d2) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x57, 0);
                x.set8(0x5e, 0);
                x.set8(0x5c, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                x.add16(0x10, 0xffff);
                x.set32(0x14, 0x200_0000);
                x.set16(0x18, 0x800);
                x.set16(0x1a, -0x60i16 as u16);
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
                x.call(REINSERT);
                active(x);
            }
            2 => active(x),
            _ => x.free_pshl(),
        }
    }

    fn active(x: &mut Cx) {
        if x.u8(0x5) != 0 {
            return burn(x);
        }
        x.set8(0x0, 1);
        if x.u8(0x5e) != 0 {
            x.add8(0x5e, 0xff);
            x.set8(0x0, 2);
        }
        x.call(ANIM_STEP);
        x.call(MOVE_XZ);
        let hit = x.u8(0x57) & 2 != 0;
        let mut bounce = hit;
        if !hit {
            x.call(TERRAIN_OF_OBJECT);
            let terrain = x.c.d[2];
            let ground = (terrain & 0x3f).wrapping_sub(5) as u16 <= 3;
            if !ground || terrain & 0x40 != 0 {
                if (x.u16(0xc) as i16) > 0 {
                    return finish(x);
                }
                x.set16(0xc, 0);
                bounce = true;
            } else {
                // contre un mur : demi-tour
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                let vx = x.u16(0x14);
                x.set16(0x14, vx.wrapping_neg());
            }
        }
        let _ = bounce;
        x.set16(0x18, 0x600);
        x.set16(0x1a, -0x40i16 as u16);
        x.set8(0x0, 2);
        x.set8(0x5e, 8);
        x.set8(0x57, 0);
        x.add8(0x5c, 0xff);
        if x.u8(0x5c) == 0 {
            x.set8(0x5, 2);
            x.set8(0x0, 2);
            x.set8(0x5d, 0x20);
        }
        finish(x);
    }

    fn finish(x: &mut Cx) {
        if x.u8(0x33) != 0 {
            x.set8(0x33, 0);
            x.call(SFX_START);
        }
    }

    /// Fin de vie : clignote 0x20 images, retombe puis disparaît.
    fn burn(x: &mut Cx) {
        x.add8(0x5d, 0xff);
        if x.u8(0x5d) == 0 {
            return x.free_pshl();
        }
        x.set_bit(0x1, 1, true);
        let t = x.u8(0x5d);
        x.c.d[0] = x.c.d[0] & !0xff | (t & 1) as u32;
        if t & 1 != 0 {
            x.set_bit(0x1, 1, false);
        }
        if (x.u16(0xc) as i16) > 0 {
            return x.call(MOVE_XZ);
        }
        x.anim(ANIMS, 1);
    }
}

/// Type 5 (0x5265a) : explosion en gerbe (variante 0 : projectile qui part en arc et éclate ; variante 1 :
/// flamme posée par l'éclatement, décalée selon son rang `+0x2f`).
mod burst {
    use super::*;

    const ANIMS: u32 = 0x15b13c;
    const SHOT_COUNTS: u32 = 0x5263c;
    const OFFSET_TABLES: u32 = 0x52732;
    const SFX_RIGHT: u32 = 0xe89a4;
    const SFX_LEFT: u32 = 0xe8a2e;
    const SFX_FLAME: u32 = 0xe8ab8;
    const FLASH_ON: u32 = 0x1ac68;
    const FLASH_PULSE: u32 = 0x1ac86;
    const SHAKE: u32 = 0x2824;
    /// Compteur des flammes en vie (`A5-0x5fbc`, adresse absolue).
    const FLAMES: u32 = 0xff2044;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x52668) {
            0 if !child => {
                x.add8(0x4, 2);
                x.set16(0x14, 0x500);
                x.set16(0x16, 0x40);
                x.set16(0x18, -0x500i16 as u16);
                x.set16(0x1a, 0);
                if x.u8(f::FLIP) == 0 {
                    let v = x.u32(0x14).wrapping_neg();
                    x.set32(0x14, v);
                }
                x.anim(ANIMS, 0);
                if (x.u16(0x14) as i16) < 0 {
                    x.call(SFX_LEFT);
                } else {
                    x.call(SFX_RIGHT);
                }
            }
            0 => flame_init(x),
            2 if !child => {
                x.call(MOVE_XZ);
                x.call(ANIM_STEP);
                if (x.u16(0xc) as i16) < 0 {
                    x.add8(0x4, 2);
                    spawn_flames(x);
                }
                x.call(CULL);
                x.call(REINSERT);
            }
            2 => {
                if x.u8(0x5e) != 0 {
                    x.add8(0x5e, 0xff);
                    if x.u8(0x5e) == 0 {
                        x.set_a5_u16(0x4ba, 0);
                    } else if x.u8(0x5e) == 2 {
                        x.call(FLASH_PULSE);
                    }
                }
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_pshl(),
        }
    }

    fn flame_init(x: &mut Cx) {
        x.add8(0x5c, 0xff);
        if x.u8(0x5c) != 0 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 6);
        let count = rd16(x.b, FLAMES) as u32;
        x.c.d[0] = count << 2 & 0xffff;
        let table = x.data32(OFFSET_TABLES + (count << 2 & 0xffff));
        x.c.a[1] = table;
        let k = (x.u8(0x2f) as u32) << 2 & 0xffff;
        x.c.d[0] = k;
        let dx = x.data16(table + k);
        let dy = x.data16(table + k + 2);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.c.d[1] = dx as u32;
        x.c.d[2] = dy as u32;
        x.add16(0x8, dx);
        x.add16(0x10, dy);
        x.set8(0x5e, 4);
        x.set_a5_u16(0x4ba, 0x100);
        x.call(FLASH_ON);
        x.c.d[0] = 0x20;
        x.call(SHAKE);
        x.anim(ANIMS, 1);
        x.call(SFX_FLAME);
        x.call(REINSERT);
    }

    /// `0x525ce` : crée les flammes (variante 1), une de plus tous les 4 pas de retard.
    fn spawn_flames(x: &mut Cx) {
        wr16(x.b, FLAMES, 0);
        let owner = x.link_a1(f::OWNER);
        let player = owner.u8(x.b, 0x10c) as u32;
        let n = x.rom8(SHOT_COUNTS + player).wrapping_sub(1);
        let (mut rank, mut delay) = (0u8, 1u8);
        let mut remaining = n as u16;
        x.c.d[5] = n as u32;
        loop {
            let Some(o) = x.alloc(ALLOC_PSHL) else { break };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 5);
            o.set_u8(x.b, 3, 1);
            let flip = x.u8(f::FLIP);
            o.set_u8(x.b, f::FLIP, flip);
            let owner_word = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner_word);
            let (p1, p2) = (x.u8(0x45), x.u8(0x46));
            o.set_u8(x.b, 0x45, p1);
            o.set_u8(x.b, 0x46, p2);
            let (px, py) = (x.u16(0x8), x.u16(0x10));
            o.set_u16(x.b, 0x8, px);
            o.set_u16(x.b, 0x10, py);
            o.set_u32(x.b, 0xc, 0);
            o.set_u8(x.b, 0x2f, rank);
            o.set_u8(x.b, 0x5c, delay);
            rank = rank.wrapping_add(1);
            delay = delay.wrapping_add(4);
            let count = rd16(x.b, FLAMES).wrapping_add(1);
            wr16(x.b, FLAMES, count);
            let done = remaining == 0;
            remaining = remaining.wrapping_sub(1);
            if done {
                break;
            }
        }
        x.c.d[3] = rank as u32;
        x.c.d[4] = delay as u32;
    }
}

handlers! {
    0x51c64 "pshl_0" 100 marker::run,
    0x51da6 "pshl_1" 120 thrown::run,
    0x5204e "pshl_2" 120 flung::run,
    0x52234 "pshl_3" 120 arc::run,
    0x523c4 "pshl_4" 120 bouncer::run,
    0x5265a "pshl_5" 120 burst::run,
}
