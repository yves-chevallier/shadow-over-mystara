//! Tirs des joueurs (PSHL), quatrième série : types 19 à 24 (foudre en chaîne, projectiles de l'elfe et du
//! magicien : flèche, couteau, boule de feu). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, wr16};
use crate::objects::Obj;

const REINSERT: u32 = 0x1ee1a;
const TERRAIN_OF_OBJECT: u32 = 0x24a8;
const SHAKE: u32 = 0x2824;
const FLASH_ON: u32 = 0x1ac68;
const FLASH_PULSE: u32 = 0x1ac86;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

fn stopped_by_terrain(x: &mut Cx) -> bool {
    x.call(TERRAIN_OF_OBJECT);
    let terrain = x.c.d[2];
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | ((terrain & 0x3f).wrapping_sub(5) & 0xffff);
    (terrain & 0x3f).wrapping_sub(5) as u16 <= 3 && terrain & 0x40 == 0
}

/// Type 19 (0x53e94) : foudre en chaîne. Un maillon (variante 0) crée quatre étincelles d'écran
/// (EFFCT 12), les relie en liste par `+0x5c`, puis poursuit la cible vivante la plus proche
/// (ENEMY ou BOSS, `+0x5c`) en tournant de 1/32 de tour par image. Le compteur `A5-0x5fb4` limite le
/// nombre de maillons.
mod chain {
    use super::*;

    const LINKS: u32 = 0xff204c;
    const SPEEDS: u32 = 0x1c89b6;
    const ANIMS: u32 = 0x15cdb6;
    const ALLOC_EFFCT: u32 = 0x2318;
    const ANGLE_TO_TARGET: u32 = 0x1376;
    const MOVE_TABLE_XY: u32 = 0x1cb4;
    const ACTIVE_CHECK: u32 = 0x1f0be;
    const SPAWN_EFFECT: u32 = 0xabec8;
    const SFX_END: u32 = 0xe859a;

    pub fn run(x: &mut Cx) {
        match x.state(0x53ea2) {
            0 => init(x),
            2 => active(x),
            4 => dying(x),
            _ => release(x),
        }
    }

    fn release(x: &mut Cx) {
        let n = rd16(x.b, LINKS).wrapping_add(1);
        wr16(x.b, LINKS, n);
        x.free_pshl();
    }

    fn init(x: &mut Cx) {
        if x.a5_u16(0x7e68) < 4 {
            return release(x);
        }
        x.add8(0x4, 2);
        x.set16(0x42, 0x12c);
        x.set8(f::BOX_SET, 6);
        let owner = x.link_a1(f::OWNER);
        for off in [0x8, 0xc, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let dz = x.u16(0x18);
        x.c.d[0] = dz as u32;
        x.add16(0xc, dz);
        let mut dx = x.u16(0x14);
        x.c.d[0] = dx as u32;
        let y = x.u32(0x10).wrapping_sub(1);
        x.set32(0x10, y);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.set8(0x2d, 0x18);
        if x.u8(f::FLIP) != 0 {
            x.set8(0x2d, 8);
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx);
        x.set8(0x47, 2);
        x.set32(0x4c, SPEEDS);
        x.c.a[4] = x.o.0 + 0x58 + 4;
        x.set32(0x58, 0xffff_ffff);
        // quatre étincelles, triées par adresse puis chaînées
        let mut sparks = [0u16; 4];
        for s in sparks.iter_mut() {
            x.call(ALLOC_EFFCT);
            let spark = Obj(x.c.a[1]);
            *s = spark.0 as u16;
            spark.set_u8(x.b, 0, 1);
            spark.set_u8(x.b, 2, 0xc);
        }
        sparks.sort();
        for (i, s) in sparks.iter().enumerate() {
            x.set16(0x30 + 2 * i as u32, *s);
        }
        let mut previous = x.o.0 as u16;
        for (rank, s) in sparks.iter().enumerate() {
            let spark = Obj(*s as i16 as i32 as u32 & 0xff_ffff);
            spark.set_u8(x.b, 3, 0);
            spark.set_u8(x.b, 0x2f, rank as u8);
            spark.set_u16(x.b, 0x5c, previous);
            spark.set_u16(x.b, 0x46, x.o.0 as u16);
            previous = *s;
        }
        find_target(x, 0);
        x.anim(ANIMS, 0);
        x.call(REINSERT);
    }

    fn active(x: &mut Cx) {
        x.call(ANIM_STEP);
        'follow: {
            if x.u16(0x42) != 0 {
                x.add16(0x42, 0xffff);
                if x.u16(0x42) == 0 {
                    x.set16(0x5c, 0);
                }
                x.add8(0x47, 0xff);
                if x.u8(0x47) != 0 {
                    break 'follow;
                }
                x.set8(0x47, 2);
                if x.u16(0x5c) != 0 {
                    let target = x.link_a1(0x5c);
                    if target.u16(x.b, 0x62) != 0 {
                        if target.u8(x.b, 0x86) == 0 {
                            steer(x);
                        } else {
                            // cible en transit : on en cherche une autre
                            x.c.d[6] = x.c.a[1] & 0xffff;
                            x.set16(0x52, x.c.a[1] as u16);
                            find_target(x, target.0 as u16);
                            let now = x.link_a1(0x5c);
                            if now.0 as u16 == x.u16(0x52) {
                                steer(x);
                            }
                        }
                        break 'follow;
                    }
                    x.set16(0x5c, 0);
                }
            }
            // plus de cible (ou plus de vie) : retrait si l'objet sort de l'écran
            if !off_screen(x) {
                if x.u16(0x42) != 0 {
                    find_target(x, 0);
                }
            } else {
                x.set8(0x5, 0);
                x.add8(0x4, 2);
                if x.u16(0x5c) != 0 {
                    let target = x.link_a1(0x5c);
                    let n = target.u8(x.b, 0x92).wrapping_sub(1);
                    target.set_u8(x.b, 0x92, n);
                }
                return;
            }
        }
        // histoire des angles, élévation vers la cible, déplacement
        let (a, b, c) = (x.u8(0x58), x.u8(0x59), x.u8(0x5a));
        x.set8(0x59, a);
        x.set8(0x5a, b);
        x.set8(0x5b, c);
        rise(x);
        let angle = x.u8(0x2d);
        x.set8(0x58, angle);
        x.set8(0x2d, angle & 0x1f);
        x.call(MOVE_TABLE_XY);
        x.set8(f::FLIP, 0);
        if (x.u8(0x2d) as i8) <= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
        x.call(REINSERT);
    }

    /// Tourne d'un cran vers la cible (`0x53fe0`).
    fn steer(x: &mut Cx) {
        x.call(ANGLE_TO_TARGET);
        let delta = x.c.d[0] as u8;
        let mut step: u8 = 1;
        x.c.d[1] = 1;
        let diff = delta.wrapping_sub(x.u8(0x2d)) & 0x1f;
        x.c.d[0] = x.c.d[0] & !0xff | diff as u32;
        if diff > 0x10 {
            step = 0xff;
            x.c.d[1] = 0xffff_ffff;
        }
        let a = x.u8(0x2d).wrapping_add(step) & 0x1f;
        x.set8(0x2d, a);
    }

    /// Monte ou descend d'un cran de 3 pixels vers la hauteur de la cible (`0x54090`).
    fn rise(x: &mut Cx) {
        if x.u16(0x5c) == 0 {
            return;
        }
        let target = x.link_a1(0x5c);
        let d0 = target.u16(x.b, 0xc).wrapping_add(0x20).wrapping_sub(x.u16(0xc)).wrapping_add(8);
        x.c.d[0] = d0 as u32;
        if d0 < 0x10 {
            return;
        }
        if (d0 as i16) < 0 {
            let z = x.u32(0xc).wrapping_sub(0x30000);
            x.set32(0xc, z);
            x.set_bit(0x2d, 6, true);
        } else {
            let z = x.u32(0xc).wrapping_add(0x30000);
            x.set32(0xc, z);
            x.set_bit(0x2d, 5, true);
        }
    }

    /// `0x54152` : vrai quand le maillon est hors de la zone visible (caméra +/- marges).
    fn off_screen(x: &mut Cx) -> bool {
        let cam = if x.a5_u8(a5::USE_CAM2) != 0 { 0x54e } else { 0x4fe };
        let (cx, cy) = (x.a5_u16(cam + 8), x.a5_u16(cam + 0xc));
        let d0 = x.u16(0x8).wrapping_sub(cx).wrapping_add(0x40);
        if d0 > 0x200 {
            return true;
        }
        let d0 = x.u16(0xc).wrapping_add(x.u16(0x10)).wrapping_sub(cy).wrapping_add(0x10);
        d0 > 0x120
    }

    /// `0x54190` / `0x54192` : choisit la cible. `held` est la cible prioritaire déjà tenue.
    fn find_target(x: &mut Cx, held: u16) {
        let mut best_priority = held;
        let mut nearest = 0u16;
        let mut best_distance = u32::MAX;
        for (pool_first, pool_free, total) in [(0x299e, 0x7e62, 0x16u16), (0x3a1e, 0x7e6c, 3u16)] {
            let active = total.wrapping_sub(x.a5_u16(pool_free)) as u8;
            if active == 0 {
                continue;
            }
            let mut left = active;
            let mut a1 = 0xff8000 + pool_first;
            loop {
                let o = Obj(a1);
                if o.u8(x.b, 0) != 0 {
                    let candidate = o.u8(x.b, 0x97) & 2 == 0
                        && o.u16(x.b, 0x62) != 0
                        && o.u8(x.b, 0x1) & 2 == 0
                        && o.u8(x.b, 0x86) == 0;
                    if candidate {
                        let taken = o.u8(x.b, 0x92);
                        if taken != 0 {
                            let keep = best_priority != 0 && {
                                let b = Obj(0xff0000 | best_priority as u32);
                                b.u8(x.b, 0x92) < taken
                            };
                            if !keep {
                                best_priority = o.0 as u16;
                            }
                        } else {
                            let mut d = 0u32;
                            for off in [0x8, 0xc, 0x10] {
                                let diff = o.u32(x.b, off).wrapping_sub(x.u32(off));
                                d = d.wrapping_add(if (o.u32(x.b, off) < x.u32(off)) { diff.wrapping_neg() } else { diff });
                            }
                            if d < best_distance {
                                best_distance = d;
                                nearest = o.0 as u16;
                            }
                        }
                    }
                    left -= 1;
                    if left == 0 {
                        break;
                    }
                }
                a1 += 0xc0;
            }
        }
        x.c.d[1] = best_distance;
        x.c.d[5] = nearest as u32;
        x.c.d[6] = best_priority as u32;
        let chosen = if nearest != 0 { nearest } else { best_priority };
        x.set16(0x5c, chosen);
        if chosen != 0 {
            let target = Obj(chosen as i16 as i32 as u32 & 0xff_ffff);
            let n = target.u8(x.b, 0x92).wrapping_add(1);
            target.set_u8(x.b, 0x92, n);
        }
    }

    /// État 4 : le maillon s'éteint (avec un éclat de fin à l'endroit où il est mort).
    fn dying(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            x.add8(0x5, 2);
            x.set8(0x0, 2);
            x.set8(0x42, 4);
            x.call(ACTIVE_CHECK);
            if x.u16(0x54) != 0 {
                x.set32(0x58, 0x8080_8080);
                x.c.d[0] = 6;
                x.c.d[1] = 0;
                x.call(SPAWN_EFFECT);
                if !x.c.cond(5) {
                    let spark = Obj(x.c.a[1]);
                    let v = x.u16(0x54);
                    spark.set_u16(x.b, 0x54, v);
                    let (px, pz) = (x.u32(0x8), x.u32(0xc));
                    spark.set_u32(x.b, 0x8, px);
                    spark.set_u32(x.b, 0xc, pz);
                    let flip = x.u8(f::FLIP);
                    spark.set_u8(x.b, f::FLIP, flip);
                }
                x.call(SFX_END);
            } else {
                x.set32(0x58, 0x8080_8080);
            }
        } else if x.u8(0x42) == 0 {
            x.add8(0x4, 2);
            release(x);
        }
    }
}

/// Type 20 (0x542ee) : mur de feu ou de glace (une colonne par exemplaire, selon le rang `+3`) qui avance
/// vers le fond de l'écran par pas de 16 pixels puis s'éteint.
mod wall {
    use super::*;

    const ANIMS: u32 = 0x15d00c;
    const POWER_A: u32 = 0x1041b6;
    const POWER_B: u32 = 0x1041d4;
    const PICK: u32 = 0x5438e;

    pub fn run(x: &mut Cx) {
        match x.state(0x542fc) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.link_a1(f::OWNER);
                x.copy_pos(owner);
                let mut dx = -0x80i16 as u16;
                if owner.u8(x.b, f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                }
                x.c.d[0] = dx as u32;
                x.c.d[1] = 0x38;
                x.add16(0x8, dx);
                x.add16(0xc, 0x38);
                let rank = x.u8(0x3) as u32;
                x.set16(0x12, rank as u16);
                let delay = (rank << 2) as u8;
                x.set8(0x5a, delay);
                x.set8(0x5c, delay);
                x.set8(0x5b, 0x40u8.wrapping_sub(delay));
                let class = owner.u8(x.b, 0x10c) as u32;
                x.c.d[0] = class;
                let p1 = x.data8(POWER_A + class);
                let p2 = x.data8(POWER_B + class);
                x.set8(f::POWER, p1);
                x.set8(f::POWER2, p2);
                let anim = x.rom8(PICK + class);
                x.anim(ANIMS, anim as u32);
                x.call(REINSERT);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        if x.u8(0x5a) == 0 {
                            x.add8(0x5, 2);
                        } else {
                            x.add8(0x5a, 0xff);
                            let dx = signed(0x10, x.u8(f::FLIP) == 0);
                            x.set_d0w(dx);
                            x.add16(0x8, dx);
                        }
                    }
                    2 => {
                        x.add8(0x5b, 0xff);
                        if x.u8(0x5b) == 0 {
                            x.add8(0x5, 2);
                            if x.u8(0x3) == 0 {
                                x.add8(0x4, 2);
                            }
                        }
                    }
                    _ => {
                        if x.u8(0x5c) == 0 {
                            x.add8(0x4, 2);
                        } else {
                            x.add8(0x5c, 0xff);
                            let dx = signed(0x10, x.u8(f::FLIP) != 0);
                            x.set_d0w(dx);
                            x.add16(0x8, dx);
                        }
                    }
                }
                x.call(ANIM_STEP);
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 21 (0x54450) : couteau lancé (voleur) à vitesse longue 0x8000000.
mod knife {
    use super::*;

    const ANIMS: u32 = 0x15d0b0;
    const SFX_HIT: u32 = 0xeba12;

    pub fn run(x: &mut Cx) {
        match x.state(0x5445e) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                for off in [0x8, 0xc] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                let y = owner.u32(x.b, 0x10);
                x.set32(0x10, y);
                x.add16(0x10, 0xffff);
                x.set32(0x14, 0x800_0000);
                let (mut dx, mut dz) = (0x48u16, 0x2cu16);
                if owner.u16(x.b, 0xc) != 0 {
                    (dx, dz) = (0x35, 0x38);
                }
                if x.u8(f::FLIP) == 0 {
                    dx = dx.wrapping_neg();
                    let v = x.u32(0x14).wrapping_neg();
                    x.set32(0x14, v);
                }
                x.c.d[0] = dx as u32;
                x.c.d[1] = dz as u32;
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 1);
                x.call(REINSERT);
            }
            2 => {
                if x.u8(f::HIT_FLAGS) & 2 != 0 || stopped_by_terrain(x) {
                    x.add8(0x4, 2);
                    return x.call(SFX_HIT);
                }
                x.call(ANIM_STEP);
                x.call(MOVE_XY);
                x.call(CULL);
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 22 (0x54560) : boule de feu (magie 14). La variante 0 vole en arc, la variante 1 est l'explosion
/// qu'elle crée au sol (avec secousse et éclair d'écran).
mod fireball {
    use super::*;

    const OFFSETS: u32 = 0x5460e;
    const ANIMS: u32 = 0x15d15e;
    const POWER_A: u32 = 0x10426a;
    const POWER_B: u32 = 0x104288;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x5456e) {
            0 if !child => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                x.copy_pos(owner);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                x.c.d[0] = row;
                let mut dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                x.set16(0x14, -0x500i16 as u16);
                x.set16(0x16, -0x40i16 as u16);
                x.set16(0x18, -0x500i16 as u16);
                x.set16(0x1a, 0);
                if x.u8(f::FLIP) != 0 {
                    let v = x.u32(0x14).wrapping_neg();
                    x.set32(0x14, v);
                    dx = dx.wrapping_neg();
                }
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                let class = owner.u8(x.b, 0x10c) as u32;
                x.c.d[0] = class;
                let p1 = x.data8(POWER_A + class);
                let p2 = x.data8(POWER_B + class);
                x.set8(f::POWER, p1);
                x.set8(f::POWER2, p2);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            0 => {
                x.add8(0x4, 2);
                let parent = x.link_a1(0x5c);
                x.copy_pos(parent);
                let flip = parent.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                let (p1, p2) = (parent.u8(x.b, f::POWER), parent.u8(x.b, f::POWER2));
                x.set8(f::POWER, p1);
                x.set8(f::POWER2, p2);
                x.set8(0x56, 4);
                x.set_a5_u16(0x4ba, 0x100);
                x.call(FLASH_ON);
                x.set_d0w(0x20);
                x.call(SHAKE);
                x.anim(ANIMS, 2);
                x.call(REINSERT);
            }
            2 if !child => {
                if x.u8(0x5) == 0 {
                    x.call(ANIM_STEP);
                    if x.c.cond(5) {
                        x.add8(0x5, 2);
                        x.anim(ANIMS, 1);
                    }
                } else {
                    x.call(MOVE_XZ);
                    x.call(ANIM_STEP);
                    if (x.u16(0xc) as i16) < 0 {
                        x.add8(0x4, 2);
                        if let Some(o) = x.alloc(ALLOC_PSHL) {
                            o.set_u8(x.b, 0, 1);
                            o.set_u8(x.b, 2, 0x16);
                            o.set_u8(x.b, 3, 1);
                            o.set_u16(x.b, 0x5c, x.o.0 as u16);
                            let owner = x.u16(f::OWNER);
                            o.set_u16(x.b, f::OWNER, owner);
                        }
                    }
                    x.call(CULL);
                }
            }
            2 => {
                if x.u8(0x56) != 0 {
                    x.add8(0x56, 0xff);
                    if x.u8(0x56) == 0 {
                        x.set_a5_u16(0x4ba, 0);
                    } else if x.u8(0x56) == 2 {
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
}

/// Type 23 (0x5471a) : éclair de l'elfe, posé devant le lanceur (le geste 0x17 le place plus haut).
mod bolt {
    use super::*;

    const OFFSETS: u32 = 0x547ec;
    const ANIMS: u32 = 0x15d3fa;

    pub fn run(x: &mut Cx) {
        match x.state(0x54728) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::POWER, 0);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                if owner.u8(x.b, 0x15e) == 0x17 {
                    let dx = signed(-0x3c_i16 as u16, x.u8(f::FLIP) != 0);
                    x.c.d[0] = dx as u32;
                    x.c.d[1] = 0x39;
                    x.c.d[2] = 0xfffe_8000;
                    x.add16(0x8, dx);
                    x.add16(0xc, 0x39);
                    x.add32(0x10, 0xfffe_8000);
                } else {
                    let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                    x.c.d[0] = row;
                    let dx = x.rom16(OFFSETS + row);
                    let dz = x.rom16(OFFSETS + row + 2);
                    x.c.d[1] = dx as u32;
                    x.c.d[2] = dz as u32;
                    let dx = signed(dx, x.u8(f::FLIP) != 0);
                    x.c.d[3] = 0xfffe_8000;
                    x.add16(0x8, dx);
                    x.add16(0xc, dz);
                    x.add32(0x10, 0xfffe_8000);
                }
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            2 => {
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_pshl(),
        }
    }
}

/// Type 24 (0x548a0) : étincelle de foudre collée à son créateur (`+0x5c`).
mod spark {
    use super::*;

    const ANIMS: u32 = 0x15ad38;

    pub fn run(x: &mut Cx) {
        match x.state(0x548ae) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 6);
                let parent = x.link_a1(0x5c);
                x.copy_pos(parent);
                let n = 0x1du32.wrapping_add(x.u8(0x3) as u32) & 0xff;
                x.anim(ANIMS, n);
                x.set32(0x34, 0);
                x.call(REINSERT);
            }
            2 => {
                let parent = x.link_a1(0x5c);
                x.copy_pos(parent);
                x.call(REINSERT);
            }
            _ => x.free_pshl(),
        }
    }
}

handlers! {
    0x53e94 "pshl_19" 200 chain::run,
    0x542ee "pshl_20" 120 wall::run,
    0x54450 "pshl_21" 120 knife::run,
    0x54560 "pshl_22" 120 fireball::run,
    0x5471a "pshl_23" 120 bolt::run,
    0x548a0 "pshl_24" 100 spark::run,
}
