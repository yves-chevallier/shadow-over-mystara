//! Effets des joueurs (PEFF), quatorzième série : type 41 (le gardien invoqué par la magie du mur, créé
//! par `player/spawn.rs::wall_target`). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd32, rd8, wr32, wr8};
use crate::objects::Obj;
use crate::objects::Courant;

const IN_ZONE: u32 = 0xfbae;
const SPAWN_EFFECT: u32 = 0xb10c8;
const ANIMS: u32 = 0x1bc9b6;
const SPEEDS: u32 = 0x1c7836;
const FRAMES: u32 = 0x11bffa;
/// Palette copiée dans la RAM graphique à la création (8 longs, lus en clair).
const PALETTE_SOURCE: u32 = 0x5036a;
const PALETTE_WINDOW: u32 = 0x91c180;
const OFFSETS: u32 = 0x5043a;
/// Boîtes : pour s'engager, pour blesser.
const BOX_ENGAGE: u32 = 0x50b8a;
const BOX_STRIKE: u32 = 0x50b92;
/// Compteur de gardiens en vie (`A5-0x5fbe`, adresse absolue).
const COUNTER: u32 = 0xff2042;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Le gardien : sort de terre (sous-état 0), cherche l'ennemi le plus proche (2), erre faute de cible (4),
/// bondit sur lui (6), puis disparaît en clignotant (état 4). Durée de vie `+0x42` (0x4b0 images).
mod guardian {
    use super::*;

    pub fn run(x: &mut Cx) {
        if x.a5_u8(0x34) != x.u8(0x5b) {
            return gone(x);
        }
        match x.state(0x50362) {
            0 => init(x),
            2 => active(x),
            4 => leaving(x),
            _ => gone(x),
        }
    }

    fn gone(x: &mut Cx) {
        let n = rd8(x.b, COUNTER).wrapping_sub(1);
        wr8(x.b, COUNTER, n);
        x.free_peff();
    }

    fn die(x: &mut Cx) {
        x.set32(0x4, 0x0400_0000);
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        for i in 0..8u32 {
            let v = rd32(x.b, PALETTE_SOURCE + 4 * i);
            wr32(x.b, PALETTE_WINDOW + 4 * i, v);
        }
        let owner = x.link_a1(f::OWNER);
        for off in [0x8, 0xc, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let y = x.u32(0x10).wrapping_sub(1);
        x.set32(0x10, y);
        x.set8(0x4a, 0);
        x.set8(0x46, 0xa);
        x.set8(0x48, 0x3c);
        x.set32(0x4c, SPEEDS);
        x.set16(0x18, 0x400);
        x.set16(0x1a, -0x40i16 as u16);
        x.set16(0x16, 0);
        x.set32(0x1c, 0);
        let (mut vx, mut angle) = (-0x200i16 as u16, 0x18u8);
        let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
        x.c.d[0] = row;
        let mut dx = x.rom16(OFFSETS + row);
        let dz = x.rom16(OFFSETS + row + 2);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        if flip != 0 {
            dx = dx.wrapping_neg();
            vx = vx.wrapping_neg();
            angle = 8;
        }
        x.add16(0xc, dz);
        x.add16(0x8, dx);
        x.set16(0x14, vx);
        x.set8(0x2d, angle);
        x.set16(0x42, 0x4b0);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    fn active(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if owner.u8(x.b, 0x4) as i8 >= 4 || x.a5_u8(a5::PAUSED) != 0 || x.a5_u8(0x35) == 0 {
            return die(x);
        }
        // le saut vers l'état 4 revient ici (le sous-état a été appelé par `jsr`) : la fin s'exécute dans tous les cas
        match x.u8(0x5) {
            0 => start(x),
            2 => seek(x),
            4 => wander(x),
            _ => lunge(x),
        }
        x.call(IN_ZONE);
        x.call(crate::terrain::SHADOW.entry);
        x.reinsert();
    }

    /// Fin de vie : état 4, sous-états nuls, puis la sortie (`bra 0x50794`).
    fn leave(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set16(0x6, 0);
        leaving(x);
    }

    // --- sous-état 0 : sortie de terre -------------------------------------------------------------------

    fn start(x: &mut Cx) {
        match x.u8(0x6) {
            0 => {
                x.move_xyz();
                if (x.u16(0x18) as i16) <= 0 {
                    x.add8(0x6, 2);
                    x.set8(0x4b, 0x1e);
                }
            }
            2 => {
                x.add8(0x4b, 0xff);
                if x.u8(0x4b) == 0 {
                    x.add8(0x6, 2);
                    x.set8(0x2a, 1);
                    x.set8(0x2b, 0xc);
                    x.anim(ANIMS, 6);
                } else if x.u8(0x4b) & 1 != 0 {
                    dust(x);
                }
            }
            4 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(0xc) as i16) <= 0 {
                    x.add8(0x6, 2);
                    x.set32(0xc, 0);
                    x.anim(ANIMS, 5);
                }
            }
            _ => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set8(0x6, 0);
                    x.anim(ANIMS, 1);
                }
            }
        }
    }

    /// Poussière de l'arrivée (`0xb10c8`) : effet EFFCT 7 à une hauteur légèrement plus haute.
    fn dust(x: &mut Cx) {
        x.c.d[0] = 7;
        x.c.d[1] = 0x200;
        x.c.d[2] = 0;
        x.c.d[3] = 0xffff;
        x.call(SPAWN_EFFECT);
        if x.c.cond(5) {
            return;
        }
        let o = Obj(x.c.a[1]);
        o.set_u8(x.b, 0x43, 1);
        let z = o.u16(x.b, 0xc).wrapping_add(0x18);
        o.set_u16(x.b, 0xc, z);
    }

    // --- sous-état 2 : se dirige vers sa cible ----------------------------------------------------------------

    fn seek(x: &mut Cx) {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            return leave(x);
        }
        if x.u8(0x4a) != 0 {
            return flip_turn(x);
        }
        if x.u8(0x6) == 0 {
            x.anim(ANIMS, 3);
            find_target(x);
            if x.u16(0x5c) != 0 {
                x.add8(0x6, 2);
            } else {
                x.set8(0x5, 4);
                x.set8(0x6, 0);
            }
            return;
        }
        x.anim_step();
        x.add8(0x46, 0xff);
        if x.u8(0x46) == 0 {
            x.set8(0x46, 2);
            let target = x.link_a1(0x5c);
            let mut lost = false;
            if target.u16(x.b, 0x62) == 0 {
                find_target(x);
                lost = x.u16(0x5c) == 0;
                if lost {
                    x.set8(0x5, 4);
                    x.set8(0x6, 0);
                }
            }
            if lost {
                return;
            }
            turn(x);
        }
        if (x.u8(0x33) as i8) >= 0 {
            x.move_by_table_xy();
        }
        engage_test(x);
    }

    /// Demi-tour en cours : l'animation 2 puis retour à l'image 3 avec le regard inversé.
    fn flip_turn(x: &mut Cx) {
        if !x.anim_step() {
            return;
        }
        let f = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, f);
        x.set8(0x4a, 0);
        x.anim(ANIMS, 3);
    }

    // --- sous-état 4 : errance sans cible ---------------------------------------------------------------------

    fn wander(x: &mut Cx) {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            return leave(x);
        }
        if x.u8(0x4a) != 0 {
            return flip_turn(x);
        }
        x.anim_step();
        x.add8(0x46, 0xff);
        if x.u8(0x46) == 0 {
            x.set8(0x46, 2);
            find_target(x);
            if x.u16(0x5c) != 0 {
                x.add8(0x5, 0xfe);
                x.set8(0x6, 2);
                return;
            }
            x.link_a1(f::OWNER);
            turn(x);
        }
        if (x.u8(0x33) as i8) >= 0 {
            x.move_by_table_xy();
        }
    }

    // --- sous-état 6 : bond sur la cible -----------------------------------------------------------------------

    fn lunge(x: &mut Cx) {
        x.add16(0x42, 0xffff);
        if x.u16(0x42) == 0 {
            return leave(x);
        }
        match x.u8(0x7) {
            0 => {
                x.add8(0x7, 2);
                x.anim(ANIMS, 4);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(0x7, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, -0x40i16 as u16);
                x.set16(0x14, 0x300);
                x.set16(0x16, 0);
                x.set32(0x1c, 0);
                if x.u8(f::FLIP) == 0 {
                    let vx = x.u16(0x14);
                    x.set16(0x14, vx.wrapping_neg());
                }
            }
            4 => {
                strike(x);
                x.move_xz();
                if (x.u16(0xc) as i16) > 0 {
                    return;
                }
                x.set32(0xc, 0);
                let mut vy = -0x40i16 as u16;
                if x.u8(f::FLIP) == 0 {
                    vy = vy.wrapping_neg();
                }
                x.set16(0x16, vy);
                x.add8(0x7, 2);
                x.anim(ANIMS, 5);
            }
            _ => {
                x.add8(0x4b, 0xff);
                let t = x.u8(0x4b) & 3;
                x.set8(0x4b, t);
                if t == 0 {
                    x.c.d[0] = 1;
                    x.c.d[1] = 1;
                    x.c.d[2] = 0xfff0;
                    x.c.d[3] = 0xffff;
                    x.call(SPAWN_EFFECT);
                }
                x.move_x();
                if x.u8(0x14) == 0 {
                    x.set32(0x4, 0x0202_0000);
                }
            }
        }
    }

    // --- état 4 : sortie ------------------------------------------------------------------------------------

    fn leaving(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 2);
                x.set8(0x4b, 0x1e);
                fade_dust(x);
            }
            2 => fade_dust(x),
            4 => {
                x.add8(0x5, 2);
                x.set16(0x18, 0x400);
                x.set16(0x1a, -0x40i16 as u16);
                x.set32(0x14, 0);
            }
            6 => {
                x.move_xz();
                if (x.u16(0xc) as i16) <= 0 {
                    x.set32(0xc, 0);
                    x.add8(0x5, 2);
                    x.set8(0x46, 0x28);
                }
            }
            _ => {
                let v = x.u8(0x1) ^ 2;
                x.set8(0x1, v);
                x.add8(0x46, 0xff);
                if x.u8(0x46) == 0 {
                    x.add8(0x4, 2);
                }
            }
        }
        x.reinsert();
    }

    /// `0x507bc` : poussière de la disparition pendant 0x1e images, puis le gardien s'enfonce.
    fn fade_dust(x: &mut Cx) {
        x.add8(0x4b, 0xff);
        if x.u8(0x4b) == 0 {
            x.add8(0x5, 2);
            x.set8(0x2a, 0);
            x.anim(ANIMS, 0);
        } else if x.u8(0x4b) & 1 != 0 {
            dust(x);
        }
    }

    // --- recherches et tests ---------------------------------------------------------------------------------------

    /// `0x5085a` : l'ennemi (puis le boss) le plus proche ; `+0x5c` reçoit son adresse.
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
                    if o.u16(x.b, 0x62) != 0 && o.u8(x.b, 0x95) & 2 == 0 && o.u8(x.b, 0x1) & 2 == 0 {
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

    /// `0x508fc` : tourne d'un cran vers l'objet que désigne A1 ; si le regard doit changer, lance le demi-tour.
    fn turn(x: &mut Cx) {
        x.call(crate::aides::DIRECTION_TO_OBJECT.entry);
        let diff = (x.c.d[0] as u8).wrapping_sub(x.u8(0x2d)) & 0x1f;
        let step: u8 = if diff > 0x10 { 0xff } else { 1 };
        let angle = x.u8(0x2d).wrapping_add(step) & 0x1f;
        x.set8(0x2d, angle);
        let want: u8 = if (angle as i8) > 0x10 { 0 } else { 0x20 };
        x.c.d[0] = want as u32;
        if want != x.u8(f::FLIP) {
            x.set8(0x4a, 1);
            x.anim(ANIMS, 2);
        }
    }

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

    fn frame_of(x: &mut Cx, m: Obj) -> u32 {
        let boxes = (m.u8(x.b, 0x2e) as u32) << 2;
        let table = rd32(x.b, FRAMES + boxes);
        table.wrapping_add((m.u8(x.b, 0x38) as u32) << 3)
    }

    /// `0x5094a` : le gardien s'engage quand un monstre touche sa boîte (profondeur et largeur).
    fn engage_test(x: &mut Cx) {
        for (first, count) in [(0x299eu32, 22u32), (0x3a1e, 3)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                let m = Obj(a1);
                a1 += 0xc0;
                if m.u8(x.b, 0) == 0 || m.u8(x.b, 0x38) == 0 {
                    continue;
                }
                let frame = frame_of(x, m);
                x.c.a[1] = m.0;
                if overlap(x, m, frame, BOX_ENGAGE, false) {
                    x.set16(0x5c, m.0 as u16);
                    x.set16(0x4, 0x206);
                    x.set16(0x6, 0);
                    return;
                }
            }
        }
    }

    /// `0x509ae` : pendant le bond, blesse tous les monstres que touche la boîte du gardien.
    fn strike(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        x.c.a[2] = ext(owner);
        for (first, count) in [(0x299eu32, 22u32), (0x3a1e, 3)] {
            let mut a1 = 0xff8000 + first;
            for _ in 0..count {
                let m = Obj(a1);
                a1 += 0xc0;
                if m.u8(x.b, 0) != 1 || m.u32(x.b, 0x94) >> 17 & 1 != 0 || m.u8(x.b, 0x38) == 0 || m.u8(x.b, 0x53) != 0 {
                    continue;
                }
                let frame = frame_of(x, m);
                x.c.a[1] = m.0;
                if overlap(x, m, frame, BOX_STRIKE, true) {
                    hurt(x, owner, m);
                }
            }
        }
        x.c.d[0] = 0;
    }

    fn hurt(x: &mut Cx, owner: Obj, m: Obj) {
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u8(x.b, 0x49, 8);
        m.set_u8(x.b, 0, 2);
        let flip = x.u8(f::FLIP);
        m.set_u8(x.b, 0x4a, flip);
        m.set_u8(x.b, 0x56, 8);
        let player = x.u16(f::OWNER);
        m.set_u16(x.b, 0x54, player);
        let mut damage = owner.u8(x.b, 0x109) as u16;
        if damage == 0 {
            damage = owner.u8(x.b, 0x10a) as u16;
        }
        x.c.d[0] = damage as u32;
        let hp = m.u16(x.b, 0x62);
        if hp >= damage {
            m.set_u16(x.b, 0x62, hp - damage);
            return;
        }
        m.set_u16(x.b, 0x62, hp.wrapping_sub(damage));
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u16(x.b, 0x62, 0);
        if m.u8(x.b, 0x28) == 4 && m.u8(x.b, 0x93) != 0 {
            owner.set_u8(x.b, 0x18a, 1);
            let v = m.u8(x.b, 0x9a);
            x.set_a5_u8(0x4f, v);
        }
    }
}

handlers! {
    0x50348 "peff_41" guardian::run,
}
