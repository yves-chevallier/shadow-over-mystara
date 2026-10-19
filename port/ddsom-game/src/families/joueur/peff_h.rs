//! Effets des joueurs (PEFF), huitième série : types 25 à 29 (averse, foudre du magicien, anneau, lune,
//! aura de prière). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;

const REINSERT: u32 = 0x1ee1a;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Le geste du joueur (`+7`) a-t-il changé depuis la création (octet `field`) ?
fn gesture_changed(x: &mut Cx, field: u32) -> bool {
    let owner = x.link_a1(f::OWNER);
    let gesture = owner.u8(x.b, 0x7);
    x.c.d[0] = x.c.d[0] & !0xff | gesture as u32;
    gesture != x.u8(field)
}

/// Type 25 (0x4dd02) : averse (magie 13, kind 0x19). Variante 0 : nuage posé devant le lanceur ;
/// variante 1 : gouttes (rang `+2f`) qui lancent la foudre (PSHL 19) à l'image du geste.
mod shower {
    use super::*;

    const ANIMS: u32 = 0x1bb4c0;
    const OFFSETS_A: u32 = 0x4dd84;
    const OFFSETS_B: u32 = 0x4de22;
    const OFFSETS_C: u32 = 0x4deac;
    const SPEEDS_A: u32 = 0x4df52;
    const SPEEDS_B: u32 = 0x4dfc4;
    const SPAWN_BOLT: u32 = 0x53e48;
    const SFX: u32 = 0xe8530;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4dd10) {
            0 if !child => {
                x.add8(0x4, 2);
                place(x, OFFSETS_A, true);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            0 => {
                x.add8(0x4, 2);
                if x.u8(0x15e) == 0x18 {
                    place(x, OFFSETS_B, true);
                } else {
                    place(x, OFFSETS_C, false);
                }
                x.anim(ANIMS, 1);
                x.call(REINSERT);
            }
            2 if !child => {
                if gesture_changed(x, 0x5d) {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, false);
                let v = x.u8(0x5c) ^ 1;
                x.set8(0x5c, v);
                if v != 0 {
                    x.set_bit(0x1, 1, true);
                }
                x.call(ANIM_STEP);
            }
            2 => bolt(x),
            _ => x.free_peff(),
        }
    }

    /// Place le nuage sur le lanceur : décalage lu dans une table, ligne de 8 octets (variante du
    /// lanceur, plus 4 pour le second exemplaire) ou de 4 octets.
    fn place(x: &mut Cx, table: u32, wide: bool) {
        let owner = follow_owner(x);
        let mut row = (owner.u8(x.b, 0x3) >> 1) as u32;
        if wide {
            row <<= 3;
            if x.u8(0x2f) != 0 {
                row += 4;
            }
        } else {
            row <<= 2;
        }
        x.c.d[0] = row;
        let dx = x.rom16(table + row);
        let dz = x.rom16(table + row + 2);
        x.c.d[1] = dx as u32;
        x.c.d[2] = dz as u32;
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
    }

    /// Variante 1, état 2 : une goutte fait tomber un éclair à la fin de son animation.
    fn bolt(x: &mut Cx) {
        let wide = x.u8(0x15e) == 0x18;
        x.call(ANIM_STEP);
        if x.c.cond(5) {
            x.add8(0x4, 2);
        }
        if x.u8(0x33) == 0 {
            return;
        }
        let owner = x.link_a1(f::OWNER);
        let rank = x.u8(0x2f);
        let mut row = (owner.u8(x.b, 0x3) >> 1) as u32;
        if wide {
            row <<= 3;
            if rank != 0 {
                row += 4;
            }
        } else {
            row <<= 2;
        }
        let speeds = if wide { SPEEDS_A } else { SPEEDS_B };
        let vx = x.rom16(speeds + row);
        let vz = x.rom16(speeds + row + 2);
        let saved = x.c.a[0];
        x.c.d[3] = rank as u32;
        x.c.d[2] = row;
        x.c.d[0] = vx as u32;
        x.c.d[1] = vz as u32;
        x.c.a[0] = ext(owner);
        x.c.a[5] = crate::mem::A5;
        crate::call(x.c, x.b, SPAWN_BOLT);
        crate::call(x.c, x.b, SFX);
        x.c.a[0] = saved;
        x.set8(0x33, 0);
    }
}

/// Type 26 (0x4e038) : foudre sur la scène (magie du magicien, kind 0x1a) : la colonne (variante 0) puis
/// les éclats (variante 1, rang `+2f`) qui clignotent avec le geste ; l'éclat de rang 0 balaye à son coup tous
/// les monstres (boss, puis ennemis) dans la zone.
mod strike {
    use super::*;

    const ANIMS: u32 = 0x1bb5ea;
    const FRAMES: u32 = 0x11bffa;
    const POWER_BOSS: u32 = 0x1043b4;
    const POWER_ENEMY: u32 = 0x104396;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e046) {
            0 if !child => {
                x.add8(0x4, 2);
                let owner = follow_owner(x);
                let _ = owner;
                let dx = signed(4, x.u8(f::FLIP) != 0);
                x.c.d[0] = dx as u32;
                x.c.d[1] = 0x7c;
                x.add16(0x8, dx);
                x.add16(0xc, 0x7c);
                x.set16(0x12, 0x4000);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                for off in [0x8, 0x10] {
                    let v = owner.u16(x.b, off);
                    x.set16(off, v);
                }
                if x.u8(0x2f) == 0 {
                    x.set16(0x12, 0xc000);
                }
                let n = x.u8(0x2f) as u32 + 1;
                x.anim(ANIMS, n);
                x.call(REINSERT);
            }
            2 if !child => {
                x.call(ANIM_STEP);
                if gesture_changed(x, 0x59) {
                    x.add8(0x4, 2);
                }
            }
            2 => {
                if x.u8(0x5) == 0 {
                    x.call(ANIM_STEP);
                    let owner = x.link_a1(f::OWNER);
                    let same = owner.u8(x.b, 0x7) == x.u8(0x59);
                    x.c.d[0] = x.c.d[0] & !0xff | owner.u8(x.b, 0x7) as u32;
                    if !same {
                        x.add8(0x5, 2);
                        x.set_bit(0x1, 1, false);
                        let n = x.u8(0x2f) as u32 + 3;
                        return x.anim(ANIMS, n);
                    }
                    x.set_bit(0x1, 1, true);
                    let timer = owner.u16(x.b, 0x168) & 1;
                    x.c.d[0] = timer as u32;
                    if timer != 0 {
                        x.set_bit(0x1, 1, false);
                    }
                    if x.u8(0x2f) == 0 && x.u8(0x33) != 0 {
                        x.set8(0x33, 0);
                        sweep(x);
                    }
                } else {
                    x.call(ANIM_STEP);
                    if x.c.cond(5) {
                        x.add8(0x4, 2);
                    }
                    x.set_bit(0x1, 1, true);
                    // l'original relit `(0x168,A1)` avec le A1 qu'il a sous la main (l'adresse du gestionnaire)
                    let a1 = x.c.a[1];
                    let timer = x.data16(a1.wrapping_add(0x168)) & 1;
                    x.c.d[0] = timer as u32;
                    if timer != 0 {
                        x.set_bit(0x1, 1, false);
                    }
                }
            }
            _ => x.free_peff(),
        }
    }

    /// `0x4e190` : blesse les monstres dans la zone de la foudre : les boss (3 emplacements), puis les
    /// ennemis (22).
    fn sweep(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        x.c.a[2] = ext(owner);
        for (first, count, power) in [(0x3a1eu32, 3u32, POWER_BOSS), (0x299e, 22, POWER_ENEMY)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                hit(x, owner, Obj(a1), power);
                a1 += 0xc0;
            }
        }
    }

    fn hit(x: &mut Cx, player: Obj, m: Obj, power: u32) {
        if m.u8(x.b, 0) == 0 || m.u16(x.b, 0x62) == 0 || m.u8(x.b, 0x38) == 0 || m.u8(x.b, 0x40) != 0 {
            return;
        }
        if m.u32(x.b, 0x94) >> 11 & 1 != 0 {
            return;
        }
        let set = m.u8(x.b, 0x2e) as u32;
        let table = crate::mem::rd32(x.b, FRAMES + (set << 2));
        let frame = table.wrapping_add((m.u8(x.b, 0x38) as u32) << 3);
        let b4 = x.data8(frame + 4) as i8 as i16 as u16;
        let b5 = x.data8(frame + 5);
        let theirs = m.u16(x.b, 0x10).wrapping_add(b4);
        let d0 = (-8i16 as u16).wrapping_add(x.u16(0x10)).wrapping_sub(theirs);
        let reach = 0x2cu8.wrapping_add(b5) as u16;
        if d0.wrapping_add(reach) > reach.wrapping_mul(2) {
            return;
        }
        let dx = signed(x.data8(frame) as i8 as i16 as u16, m.u8(x.b, f::FLIP) != 0);
        let d0 = x.u16(0x8).wrapping_sub(m.u16(x.b, 0x8).wrapping_add(dx));
        let reach = 0x90u8.wrapping_add(x.data8(frame + 1)) as u16;
        let d0 = d0.wrapping_add(reach);
        if d0 > reach.wrapping_mul(2) {
            return;
        }
        // touché
        let v = player.u16(x.b, 0x190).wrapping_add(2);
        player.set_u16(x.b, 0x190, v);
        let hits = m.u8(x.b, 0x9c).wrapping_add(1);
        m.set_u8(x.b, 0x9c, hits);
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u8(x.b, 0x1, 1);
        m.set_u8(x.b, 0x49, 0x84);
        let (class_id, variant_id) = (player.u8(x.b, 0x2), player.u8(x.b, 0x3));
        m.set_u8(x.b, 0x48, class_id);
        m.set_u8(x.b, 0x4b, variant_id);
        m.set_u16(x.b, 0x54, player.0 as u16);
        let class = player.u8(x.b, 0x10c) as u32;
        let mut d0 = x.data8(power + class) as u16;
        if player.u8(x.b, 0x10e) == 0x20 {
            d0 += d0 >> 2;
        }
        if player.u8(x.b, 0x10e) == 0x21 {
            d0 += d0 >> 1;
        }
        if player.u8(x.b, 0x166) == 4 {
            d0 += d0 >> 1;
        }
        let hp = m.u16(x.b, 0x62).wrapping_sub(d0);
        m.set_u16(x.b, 0x62, if (hp as i16) < 0 { 0 } else { hp });
        if m.u8(x.b, 0x28) != 4 && matches!(m.u8(x.b, 0x2), 0 | 0x13) {
            m.set_u16(x.b, 0x62, 0);
        }
        if m.u16(x.b, 0x62) == 0 && m.u8(x.b, 0x93) != 0 {
            player.set_u8(x.b, 0x18a, 1);
            let v = m.u8(x.b, 0x9a);
            x.set_a5_u8(0x4f, v);
        }
        x.c.d[0] = d0 as u32;
    }
}

/// Type 27 (0x4e316) : anneau de feu ou de glace (magie 16, kind 0x1b) : variante 0 posée devant le lanceur ;
/// variante 1 qui flamboie et fait trembler l'écran en fin de geste.
mod ring {
    use super::*;

    const ANIMS: u32 = 0x1bb80a;
    const OFFSETS: u32 = 0x4e412;
    const FLASH_A: u32 = 0x1ac72;
    const FLASH_B: u32 = 0x1ac9a;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e324) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                x.copy_pos(owner);
                let weapon = owner.u8(x.b, 0x15e);
                if weapon == 0x15 || weapon == 0x16 {
                    let (mut dx, mut dz) = (0x17u16, 0x26u16);
                    if child {
                        (dx, dz) = (4, 0x50);
                        if weapon == 0x15 && owner.u8(x.b, 0x1ee) != 0 {
                            (dx, dz) = (-0x37i16 as u16, 0x36);
                        }
                    }
                    x.c.d[0] = dx as u32;
                    x.c.d[1] = dz as u32;
                    let dx = signed(dx, x.u8(f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, dz);
                } else {
                    let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                    x.c.d[0] = row;
                    let dx = x.rom16(OFFSETS + row);
                    let dz = x.rom16(OFFSETS + row + 2);
                    let dx = signed(dx, x.u8(f::FLIP) != 0);
                    x.add16(0x8, dx);
                    x.add16(0xc, dz);
                }
                x.add16(0x10, 0xffff);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.call(REINSERT);
            }
            2 if !child => {
                x.set_bit(0x1, 1, true);
                let v = x.u8(0x5a) ^ 1;
                x.set8(0x5a, v);
                if v != 0 {
                    x.set_bit(0x1, 1, false);
                }
                if gesture_changed(x, 0x5b) {
                    x.add8(0x4, 2);
                }
                x.call(ANIM_STEP);
            }
            2 => {
                x.call(ANIM_STEP);
                if !x.c.cond(5) {
                    return;
                }
                x.add8(0x4, 2);
                x.set_a5_u16(0x4ba, 0x100);
                x.set8(0x5c, 8);
                x.set8(0x5d, 3);
                x.set8(0x5e, 1);
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x15e) == 0x16 {
                    x.call(FLASH_B);
                } else {
                    x.call(FLASH_A);
                }
            }
            4 if child => {
                x.add8(0x5c, 0xff);
                if x.u8(0x5c) != 0 {
                    return;
                }
                x.set8(0x5c, 4);
                x.set_a5_u16(0x4ba, 0);
                let v = x.u8(0x5e) ^ 1;
                x.set8(0x5e, v);
                if v != 0 {
                    x.set_a5_u16(0x4ba, 0x100);
                    x.set8(0x5c, 8);
                }
                x.add8(0x5d, 0xff);
                if x.u8(0x5d) == 0 {
                    x.add8(0x4, 2);
                    x.set_a5_u16(0x4ba, 0);
                }
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 28 (0x4e4fa) : lune (magie 12, kind 0x1c), sa lueur (variante 1) posée sur le lanceur.
mod moon {
    use super::*;

    const ANIMS: u32 = 0x1bb8f4;
    const OFFSETS: u32 = 0x4e578;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e508) {
            0 => {
                x.add8(0x4, 2);
                follow_owner(x);
                let row = (x.u8(0x3) as u32) << 3;
                x.c.d[0] = row;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                let dy = x.rom16(OFFSETS + row + 4);
                let dp = x.rom16(OFFSETS + row + 6);
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.add16(0x10, dy);
                x.add16(0x12, dp);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.call(REINSERT);
            }
            2 => {
                x.call(ANIM_STEP);
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x7) == 6 {
                    x.add8(0x4, 2);
                    return x.anim(ANIMS, if child { 5 } else { 4 });
                }
                if !child {
                    x.add16(0x58, 1);
                    if x.u8(0x33) != 0 {
                        x.set8(0x33, 0);
                        x.set_bit(0x1, 1, false);
                        x.set8(0x5c, 1);
                    }
                    if x.u8(0x5c) != 0 {
                        x.add16(0x8, 2);
                        x.add16(0xc, 1);
                        if x.u16(0x58) & 1 == 0 {
                            x.add16(0x8, 0xfffc);
                            x.add16(0xc, 0xfffe);
                        }
                    } else {
                        blink(x);
                    }
                } else {
                    x.set_bit(0x1, 1, true);
                    x.add16(0x58, 1);
                    let t = x.u16(0x58) & 1;
                    x.c.d[0] = t as u32;
                    if t == 0 {
                        x.set_bit(0x1, 1, false);
                    }
                }
            }
            4 => {
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, true);
                x.add16(0x58, 1);
                let t = x.u16(0x58) & 1;
                x.c.d[0] = t as u32;
                if t == 0 {
                    x.set_bit(0x1, 1, false);
                }
            }
            _ => x.free_peff(),
        }
    }

    fn blink(x: &mut Cx) {
        x.set_bit(0x1, 1, true);
        let t = x.u16(0x58) & 1;
        x.c.d[0] = t as u32;
        if t == 0 {
            x.set_bit(0x1, 1, false);
        }
    }
}

/// Type 29 (0x4e6ca) : aura de prière (kind 0x1d) : deux nimbes posés sur le lanceur.
mod aura {
    use super::*;

    const ANIMS: u32 = 0x1bbc9a;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4e6d8) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                let flip = owner.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                if child {
                    x.set8(f::FLIP, flip ^ 0x20);
                }
                x.copy_pos(owner);
                let (mut dx, mut dz) = (-0x1ai16 as u16, 0x45u16);
                if child {
                    (dx, dz) = (-0x17i16 as u16, 0x68);
                }
                x.c.d[0] = dx as u32;
                x.c.d[1] = dz as u32;
                let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.add16(0x10, 0xffff);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.call(REINSERT);
            }
            2 => {
                x.call(ANIM_STEP);
                if !child {
                    if gesture_changed(x, 0x59) {
                        x.add8(0x4, 2);
                    }
                } else if x.c.cond(5) {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4dd02 "peff_25" 150 shower::run,
    0x4e038 "peff_26" 150 strike::run,
    0x4e316 "peff_27" 120 ring::run,
    0x4e4fa "peff_28" 120 moon::run,
    0x4e6ca "peff_29" 100 aura::run,
}
