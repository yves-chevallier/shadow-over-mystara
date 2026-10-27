//! Effets des joueurs (PEFF), treizième série : type 34 (le familier de la prière du clerc,
//! `player/spawn.rs::swarm`). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd16, rd32, wr16};
use crate::objects::Obj;
use crate::objects::Courant;

const ANGLE_TO_TARGET: u32 = 0x1376;
const IN_ZONE: u32 = 0xfbae;
const SHADOW: u32 = 0x33fe4;
const BLOOD: u32 = 0xb141a;
const SFX_BITE: u32 = 0xe8ba2;
const ANIMS: u32 = 0x1bc346;
const SPEEDS: u32 = 0x1c7836;
/// Table des cadres de boîtes de collision (un pointeur par jeu de boîtes).
const FRAMES: u32 = 0x11bffa;
/// Boîtes du familier : pour s'engager, pour mordre.
const BOX_ATTACK: u32 = 0x4fbcc;
const BOX_BITE: u32 = 0x4fbd4;
/// Décalage (X, Z) par étape de l'animation du lanceur.
const OFFSETS: u32 = 0x4f3ea;
/// Compteur de familiers en vie (`A5-0x5fc0`, adresse absolue).
const COUNTER: u32 = 0xff2040;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Le familier du clerc : sort de terre (sous-état 0), cherche l'ennemi le plus proche (`+0x5c`) et s'en
/// approche (2), erre faute de cible (4), le mord jusqu'à `+0x5a` fois (6), puis retombe (état 4). `+0x42`
/// est sa durée de vie restante ; le long `+4..+7` est recopié en `+0x52` à chaque image.
mod pet {
    use super::*;

    pub fn run(x: &mut Cx) {
        if x.a5_u8(0x34) != x.u8(0x5b) {
            return gone(x);
        }
        match x.state(0x4f2c8) {
            0 => init(x),
            2 => active(x),
            4 => back(x),
            _ => gone(x),
        }
        let states = x.u32(0x4);
        x.set32(0x52, states);
    }

    fn gone(x: &mut Cx) {
        let n = rd16(x.b, COUNTER).wrapping_add(1);
        wr16(x.b, COUNTER, n);
        x.free_peff();
    }

    /// `move.l #0x4000000,(4,A0)` : état 4, sous-états nuls.
    fn die(x: &mut Cx) {
        x.set32(0x4, 0x0400_0000);
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5e, 0);
        let owner = x.link_a1(f::OWNER);
        for off in [0x8, 0xc, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let y = x.u32(0x10).wrapping_sub(1);
        x.set32(0x10, y);
        x.set8(0x47, 0xff);
        x.set16(0x18, 0x400);
        x.set16(0x1a, -0x40i16 as u16);
        let (mut dx, mut vx, mut angle) = (-0x1ci16 as u16, -0x200i16 as u16, 0x18u8);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        if flip != 0 {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            angle = 8;
        }
        x.add16(0x8, dx);
        x.set16(0x14, vx);
        x.set8(0x2d, angle);
        x.set16(0x16, 0);
        x.set32(0x1c, 0);
        let rank = x.u8(0x3);
        x.set8(0x1c, rank);
        x.set8(0x46, 0xau8.wrapping_sub(rank));
        x.set32(0x4c, SPEEDS);
        pose(x);
        let class = owner.u8(x.b, 0x10c);
        let life = if class <= 0xe { 0x2d0 } else { 0x348 };
        x.set16(0x42, life);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    /// `0x4f388` : suit l'étape d'animation du lanceur (`+0x33`) : place et image.
    fn pose(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        let step = owner.u8(x.b, 0x33);
        x.c.d[0] = step as u32;
        if step == x.u8(0x47) {
            return;
        }
        let (px, pz) = (owner.u16(x.b, 0x8), owner.u16(x.b, 0xc));
        x.set16(0x8, px);
        x.set16(0xc, pz);
        x.set8(0x47, step);
        let row = (step as u32) << 2;
        let dx = x.rom16(OFFSETS + row);
        let dz = x.rom16(OFFSETS + row + 2);
        x.add16(0xc, dz);
        let dx = signed(dx, x.u8(f::FLIP) != 0);
        x.add16(0x8, dx);
        let mut anim = step;
        if step == 4 {
            x.add8(0x6, 2);
            x.add32(0x10, 2);
            anim = step.wrapping_add(x.u8(0x3));
        }
        x.anim(ANIMS, anim as u32);
    }

    fn active(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if x.a5_u8(a5::PAUSED) != 0 || owner.u8(x.b, 0x4) as i8 >= 4 || x.a5_u8(0x35) == 0 {
            return die(x);
        }
        // la fin (zone, ombre, réinsertion) suit toujours le sous-état, même quand il a basculé à l'état 4
        let tail = match x.u8(0x5) {
            0 => emerge(x),
            2 => approach(x),
            4 => chase(x),
            _ => attack(x),
        };
        if tail {
            x.call(IN_ZONE);
            x.call(SHADOW);
            x.reinsert();
        }
    }

    // --- sous-état 0 : sortie de terre ---------------------------------------------------------------

    fn emerge(x: &mut Cx) -> bool {
        match x.u8(0x6) {
            0 => pose(x),
            2 => {
                x.move_xyz();
                if (x.u16(0x18) as i16) <= 0 {
                    x.add8(0x6, 2);
                    x.anim(ANIMS, 7);
                }
            }
            _ => {
                x.move_xz();
                if (x.u16(0xc) as i16) <= 0 {
                    x.set32(0xc, 0);
                    x.add8(0x5, 2);
                    x.set8(0x6, 0);
                }
            }
        }
        x.anim_step();
        true
    }

    // --- sous-état 2 : se dirige vers sa cible ------------------------------------------------------------

    fn approach(x: &mut Cx) -> bool {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            leave(x);
            return true;
        }
        if x.u8(0x6) == 0 {
            x.anim(ANIMS, 9);
            find_target(x);
            if x.u16(0x5c) != 0 {
                x.add8(0x6, 2);
            } else {
                x.set8(0x5, 4);
                x.set8(0x6, 0);
            }
        } else {
            x.add8(0x46, 0xff);
            let mut lost = false;
            if x.u8(0x46) == 0 {
                x.set8(0x46, 2);
                let target = x.link_a1(0x5c);
                if target.u16(x.b, 0x62) == 0 {
                    find_target(x);
                    lost = x.u16(0x5c) == 0;
                    if lost {
                        x.set8(0x5, 4);
                        x.set8(0x6, 0);
                    }
                }
                if !lost {
                    turn(x);
                }
            }
            if !lost {
                x.move_by_table_xy();
                attack_test(x);
            }
        }
        x.anim_step();
        true
    }

    // --- sous-état 4 : errance sans cible ----------------------------------------------------------------

    fn chase(x: &mut Cx) -> bool {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            leave(x);
            return true;
        }
        x.anim_step();
        x.add8(0x46, 0xff);
        if x.u8(0x46) == 0 {
            x.set8(0x46, 2);
            find_target(x);
            if x.u16(0x5c) != 0 {
                x.add8(0x5, 0xfe);
                x.set8(0x6, 2);
                return true;
            }
            x.link_a1(f::OWNER);
            turn(x);
        }
        x.move_by_table_xy();
        true
    }

    /// Fin de vie : état 4, sous-états nuls, puis la retombée (`bra 0x4f7d2`, qui revient au sous-état appelant).
    fn leave(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set16(0x6, 0);
        back(x);
    }

    // --- sous-état 6 : bond et morsure ----------------------------------------------------------------

    fn attack(x: &mut Cx) -> bool {
        if x.u8(0x6) == 0 {
            x.add8(0x6, 2);
            x.set8(0x7, 0);
            x.set16(0x18, 0x400);
            x.set16(0x1a, -0x40i16 as u16);
            x.link_a1(0x5c);
            x.call(ANGLE_TO_TARGET);
            let angle = x.c.d[0] as u8;
            x.set8(0x2d, angle);
            flip_by_angle(x);
            x.anim(ANIMS, 0xa);
            return true;
        }
        let target = x.link_a1(0x5c);
        if target.u16(x.b, 0x4) == 0x406 || x.a5_u8(a5::PAUSED) != 0 {
            die(x);
            return true;
        }
        match x.u8(0x7) {
            0 => leap(x),
            2 => land(x),
            _ => bite(x, target),
        }
    }

    fn leap(x: &mut Cx) -> bool {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            leave(x);
            return true;
        }
        if find_hit(x) {
            x.anim(ANIMS, 0xc);
            return true;
        }
        x.anim_step();
        x.move_by_table_xy();
        x.move_z();
        if (x.u16(0x18) as i16) < 0 {
            x.add8(0x7, 2);
            x.anim(ANIMS, 0xb);
        }
        true
    }

    fn land(x: &mut Cx) -> bool {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            leave(x);
            return true;
        }
        if find_hit(x) {
            x.anim(ANIMS, 0xd);
            return true;
        }
        x.anim_step();
        x.move_by_table_xy();
        x.move_z();
        if (x.u16(0xc) as i16) <= 0 {
            x.set32(0xc, 0);
            x.set8(0x5, 2);
            x.set16(0x6, 0);
        }
        true
    }

    /// Le familier colle à sa cible et la mord toutes les 30 images (`+0x58`).
    fn bite(x: &mut Cx, target: Obj) -> bool {
        if (target.u16(x.b, 0x62) as i16) <= 0 {
            die(x);
            return true;
        }
        for off in [0x8, 0x10] {
            let v = target.u32(x.b, off);
            x.set32(off, v);
        }
        let y = x.u32(0x10).wrapping_sub(1);
        x.set32(0x10, y);
        let z = target.u32(x.b, 0xc);
        x.set32(0xc, z);
        let vz = x.u16(0x18);
        x.add16(0xc, vz);
        let mut d0 = x.u16(0x14);
        let t_flip = target.u8(x.b, f::FLIP);
        x.c.d[1] = t_flip as u32;
        if t_flip != x.u8(0x47) {
            let own = x.u8(0x48) ^ 0x20;
            x.set8(f::FLIP, own);
            d0 = d0.wrapping_neg();
        }
        x.add16(0x8, d0);
        x.anim_step();
        if target.u16(x.b, 0x4) == 0x200
            || target.u8(x.b, 0x95) & 2 != 0
            || target.u8(x.b, 0x49) == 0x84
            || target.u8(x.b, 0x49) == 0x64
        {
            return true;
        }
        x.add8(0x58, 0xff);
        if x.u8(0x58) != 0 {
            return true;
        }
        x.set8(0x58, 0x1e);
        let left = x.u8(0x5a).wrapping_sub(1);
        x.set8(0x5a, left);
        if (left as i8) < 0 {
            die(x);
            x.c.d[2] = 0x10;
            x.c.d[3] = 0x10;
            x.call(BLOOD);
            return true;
        }
        if left == 0 {
            x.set8(0x58, 5);
        }
        x.call(SFX_BITE);
        let damage = x.u8(0x49) as u16;
        let hp = target.u16(x.b, 0x62).wrapping_sub(damage);
        target.set_u16(x.b, 0x62, hp);
        if (hp as i16) <= 0 {
            target.set_u16(x.b, 0x62, 0);
            target.set_u8(x.b, 0x49, 0);
            target.set_u32(x.b, 0x4, 0x0400_0000);
            let flip = target.u8(x.b, f::FLIP) ^ 0x20;
            target.set_u8(x.b, 0x4a, flip);
            let owner = x.u16(f::OWNER);
            target.set_u16(x.b, 0x54, owner);
            if target.u8(x.b, 0x28) == 4 && target.u8(x.b, 0x93) != 0 {
                let player = x.link_a1(f::OWNER);
                player.set_u8(x.b, 0x18a, 1);
                let v = target.u8(x.b, 0x9a);
                x.set_a5_u8(0x4f, v);
            }
            die(x);
        }
        x.c.d[2] = 0x10;
        x.c.d[3] = 0x10;
        x.call(BLOOD);
        true
    }

    // --- état 4 : retombée -----------------------------------------------------------------------------------

    fn back(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                if x.u8(0x5e) != 0 {
                    let target = x.link_a1(0x5c);
                    let n = target.u8(x.b, 0x9b).wrapping_sub(1);
                    target.set_u8(x.b, 0x9b, n);
                }
                x.add8(0x5, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, -0x40i16 as u16);
                x.set16(0x16, 0);
                let dx = signed(0x200, x.u8(f::FLIP) != 0);
                x.set16(0x14, dx);
                x.anim(ANIMS, 0xe);
            }
            2 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(0x18) as i16) <= 0 && (x.u16(0xc) as i16) <= 0 {
                    x.set8(0x46, 0x28);
                    x.add8(0x5, 2);
                }
            }
            _ => {
                x.add8(0x46, 0xff);
                if x.u8(0x46) == 0 {
                    x.add8(0x4, 2);
                }
                let v = x.u8(0x1) ^ 2;
                x.set8(0x1, v);
            }
        }
        x.reinsert();
    }

    // --- recherches de cibles ------------------------------------------------------------------------------------

    /// `0x4f874` : l'ennemi (puis le boss) le plus proche, mordu moins de 3 fois ; `+0x5c` reçoit son adresse.
    /// Comme l'original, laisse dans A1 le dernier objet examiné.
    fn find_target(x: &mut Cx) {
        let mut best = u32::MAX;
        let mut found = 0u16;
        for (first, free, total) in [(0x299eu32, 0x7e62u32, 0x16u16), (0x3a1e, 0x7e6c, 3)] {
            let mut left = total.wrapping_sub(x.a5_u16(free)) as u8;
            if left == 0 {
                continue;
            }
            let mut a1 = 0xff8000 + first;
            loop {
                let o = Obj(a1);
                if o.u8(x.b, 0) != 0 {
                    x.c.a[1] = a1;
                    if o.u16(x.b, 0x62) != 0
                        && o.u8(x.b, 0x95) & 2 == 0
                        && o.u8(x.b, 0x9b) < 3
                        && o.u8(x.b, 0x1) & 2 == 0
                    {
                        let mut d = 0u32;
                        for off in [0x8u32, 0xc, 0x10] {
                            d = d.wrapping_add(o.u32(x.b, off).abs_diff(x.u32(off)));
                        }
                        if d < best {
                            best = d;
                            found = o.0 as u16;
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
        x.c.d[1] = best;
        x.c.a[2] = found as i16 as i32 as u32;
        x.set16(0x5c, found);
    }

    /// `0x4f920` : tourne d'un cran vers l'objet que désigne A1.
    fn turn(x: &mut Cx) {
        x.call(ANGLE_TO_TARGET);
        let diff = (x.c.d[0] as u8).wrapping_sub(x.u8(0x2d)) & 0x1f;
        let step: u8 = if diff > 0x10 { 0xff } else { 1 };
        let angle = x.u8(0x2d).wrapping_add(step) & 0x1f;
        x.set8(0x2d, angle);
        flip_by_angle(x);
    }

    fn flip_by_angle(x: &mut Cx) {
        x.set8(f::FLIP, 0);
        if (x.u8(0x2d) as i8) <= 0x10 {
            x.set8(f::FLIP, 0x20);
        }
    }

    /// Recouvrement de la boîte du monstre `frame` et de la boîte `mine` du familier : profondeur (`0x4fb44`),
    /// largeur (`0x4fb6c`) et, pour la morsure, hauteur (`0x4fba4`).
    fn overlap(x: &mut Cx, m: Obj, frame: u32, mine: u32, with_z: bool) -> bool {
        let (e, p) = (frame, mine);
        let d = (x.data8(e + 4) as i8 as i16 as u16).wrapping_add(m.u16(x.b, 0x10));
        let q = (x.data8(p + 4) as i8 as i16 as u16).wrapping_add(x.u16(0x10));
        let reach = x.data8(e + 5).wrapping_add(x.data8(p + 5)) as u16;
        if d.wrapping_sub(q).wrapping_add(reach) > reach.wrapping_mul(2) {
            return false;
        }
        let a = signed(x.data8(e) as i8 as i16 as u16, m.u8(x.b, f::FLIP) != 0);
        let c = signed(x.data8(p) as i8 as i16 as u16, x.u8(f::FLIP) != 0);
        let d = a.wrapping_sub(c).wrapping_add(m.u16(x.b, 0x8)).wrapping_sub(x.u16(0x8));
        let reach = x.data8(e + 1).wrapping_add(x.data8(p + 1)) as u16;
        if d.wrapping_add(reach) > reach.wrapping_mul(2) {
            return false;
        }
        if !with_z {
            return true;
        }
        let d = (x.data8(e + 2) as i8 as i16 as u16).wrapping_add(m.u16(x.b, 0xc));
        let q = (x.data8(p + 2) as i8 as i16 as u16).wrapping_add(x.u16(0xc));
        let reach = x.data8(e + 3).wrapping_add(x.data8(p + 3)) as u16;
        d.wrapping_sub(q).wrapping_add(reach) <= reach.wrapping_mul(2)
    }

    /// Premier monstre (ENEMY puis BOSS) dont une boîte recouvre celle du familier (`0x4f95a` pour
    /// s'engager, `0x4f9be` pour la morsure, qui exclut en plus les monstres déjà mordus 3 fois ou frappés).
    fn search_hit(x: &mut Cx, bite: bool) -> Option<Obj> {
        for (first, count) in [(0x299eu32, 22u32), (0x3a1e, 3)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                let m = Obj(a1);
                a1 += 0xc0;
                if m.u8(x.b, 0) == 0 {
                    continue;
                }
                let set = m.u8(x.b, 0x38) as u32;
                if set == 0 {
                    continue;
                }
                if bite && (m.u8(x.b, 0x95) & 2 != 0 || m.u8(x.b, 0x9b) >= 3 || m.u8(x.b, 0x49) == 0x84) {
                    continue;
                }
                let boxes = (m.u8(x.b, 0x2e) as u32) << 2;
                let table = rd32(x.b, FRAMES + boxes);
                let frame = table.wrapping_add(set << 3);
                let mine = if bite { BOX_BITE } else { BOX_ATTACK };
                x.c.a[1] = m.0;
                if overlap(x, m, frame, mine, bite) {
                    return Some(m);
                }
            }
        }
        None
    }

    /// `0x4f95a` : engage l'attaque (sous-état 6) quand un monstre touche le familier.
    fn attack_test(x: &mut Cx) {
        if let Some(m) = search_hit(x, false) {
            x.c.a[1] = m.0;
            x.set16(0x5c, m.0 as u16);
            x.set16(0x4, 0x206);
            x.set16(0x6, 0);
        }
    }

    /// `0x4f9be` : un monstre est à portée de morsure : la cible est fixée et la morsure s'engage.
    fn find_hit(x: &mut Cx) -> bool {
        let Some(m) = search_hit(x, true) else {
            x.c.d[0] = 0;
            return false;
        };
        x.c.a[1] = m.0;
        let dz = x.u16(0xc).wrapping_sub(m.u16(x.b, 0xc));
        x.set16(0x18, dz);
        let dx = x.u16(0x8).wrapping_sub(m.u16(x.b, 0x8));
        x.set16(0x14, dx);
        let y = m.u32(x.b, 0x10).wrapping_sub(1);
        x.set32(0x10, y);
        x.set16(0x5c, m.0 as u16);
        x.set8(0x5e, 1);
        let flip = m.u8(x.b, f::FLIP);
        x.set8(0x47, flip);
        let own = x.u8(f::FLIP);
        x.set8(0x48, own);
        x.set32(0x4, 0x0206_0204);
        x.set8(0x58, 5);
        let hits = m.u8(x.b, 0x9b).wrapping_add(1);
        m.set_u8(x.b, 0x9b, hits);
        let owner = x.link_a1(f::OWNER);
        let class = owner.u8(x.b, 0x10c);
        let (d1, d2) = if class <= 0xe {
            (6, 4)
        } else if m.u8(x.b, 0x28) == 2 {
            if class <= 0x13 { (0xa, 5) } else { (0xc, 6) }
        } else if class <= 0x13 {
            (7, 2)
        } else {
            (8, 2)
        };
        x.c.d[1] = d1 as u32;
        x.c.d[2] = d2 as u32;
        x.set8(0x5a, d1);
        x.set8(0x49, d2);
        true
    }
}

handlers! {
    0x4f2a6 "peff_34" pet::run,
}
