//! Effets des joueurs (PEFF), cinquième série : types 12 (le gel), 13 (bénédiction) et 14 (renvoi).
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Caméra de référence : position (X, Y) de la caméra principale, ou de la secondaire si `A5+0x36`.
fn camera<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> (u16, u16) {
    if x.a5_u8(a5::USE_CAM2) != 0 {
        (x.a5_u16(a5::CAM2_X), x.a5_u16(a5::CAM2_Y))
    } else {
        (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y))
    }
}

/// Type 12 (0x4bc66) : le gel (magie 15 et sorts de gel), six variantes qui ont chacune leurs états.
/// 0 : éclat de glace qui rebondit sur les bords de l'écran ; 1 : son halo ; 2 : brume ; 3 : flocon ;
/// 4 : éclat tombant (`+2f` pair) ; 5 : couronne autour du lanceur. Tant que le jeu est figé
/// (`A5+0x3d` non nul) les effets durent ; il passe à l'état 4 dès que le gel cesse.
mod ice {
    use super::*;

    const ANIMS: u32 = 0x1b6350;
    const OFFSETS: u32 = 0x4bd0e;
    const DELAYS: u32 = 0x4bd98;
    const SFX: u32 = 0xe87f4;
    const SCATTER_X: u32 = 0x4c016;
    const SCATTER_Y: u32 = 0x4c036;
    const SPREAD: u32 = 0x4c0ae;
    const CROWN_A: u32 = 0x4c140;
    const CROWN_B: u32 = 0x4c158;
    /// Octets d'angle de rebond (haut, bas, gauche, droite...).
    const BOUNCE_LEFT: u32 = 0x4be70;
    const BOUNCE_RIGHT: u32 = 0x4be78;
    const BOUNCE_TOP: u32 = 0x4be80;
    const BOUNCE_BOTTOM: u32 = 0x4be88;

    pub fn run(x: &mut Cx) {
        let variant = x.u8(0x3);
        x.c.d[0] = variant as u32 * 2;
        match variant {
            0 => shard(x),
            1 => halo(x),
            2 => mist(x),
            3 => flake(x),
            4 => falling(x),
            _ => crown(x),
        }
    }

    // --- variante 0 : éclat qui rebondit ---------------------------------------------------------

    fn shard(x: &mut Cx) {
        match x.state(0x4bc90) {
            0 => {
                x.add16(0x54, 0xffff);
                if x.u16(0x54) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                let (cx, cy) = camera(x);
                x.set16(0x8, cx);
                x.set16(0x10, cy);
                x.set16(0xc, 0);
                let k = (x.u8(0x2f) as u32) << 2 & 0xffff;
                x.c.d[0] = k;
                let (dx, dy) = offset(x, OFFSETS, k);
                x.add16(0x8, dx);
                x.add16(0x10, dy);
                let r = x.rand_and_b(0x1f);
                x.set8(0x2d, r);
                x.set8(0x54, r.wrapping_add(1));
                x.set_d0w(0x2b);
                let n = x.c.d[0] as u16;
                x.select_speed(&crate::objects::motion::SPEEDS_B, n);
                x.anim(ANIMS, 0);
                // un halo (variante 1) suit l'éclat
                if let Some(o) = x.alloc(&pools::PEFF) {
                    o.set_u8(x.b, 0, 1);
                    o.set_u8(x.b, 2, 0xc);
                    o.set_u8(x.b, 3, 1);
                    o.set_u16(x.b, 0x5c, x.o.0 as u16);
                    let owner = x.u16(f::OWNER);
                    o.set_u16(x.b, f::OWNER, owner);
                }
            }
            2 => {
                x.add8(0x54, 0xff);
                if x.u8(0x54) == 0 {
                    let r = x.rand_and_b(0x1f);
                    x.set8(0x2d, r);
                    let low = x.c.d[0] as u8 & 0xf;
                    x.c.d[0] = x.c.d[0] & !0xff | low as u32;
                    let index = x.c.d[0] as u16 as i16 as i32 as u32;
                    let delay = x.rom8(DELAYS.wrapping_add(index));
                    x.set8(0x54, delay);
                }
                bounce(x);
                x.anim_step();
                x.move_by_table_xy();
                x.c.d[0] = x.u16(0x8) as u32;
                x.c.d[1] = x.u16(0x10) as u32;
                x.call(crate::terrain::SHADOW.entry);
                if !x.c.cond(7) {
                    let a = x.u8(0x2d).wrapping_add(0x10) & 0x1f;
                    x.c.d[0] = x.c.d[0] & !0xff | a as u32;
                    x.set8(0x2d, a);
                    x.move_by_table_xy();
                }
                if x.a5_u8(0x3d) == 0 {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, true);
                let t = x.u8(0x54);
                x.c.d[0] = x.c.d[0] & !0xff | (t & 2) as u32;
                if t & 2 != 0 {
                    x.set_bit(0x1, 1, false);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    /// Rebond sur les bords de l'écran (`0x4bda8`) : le tirage `andi.w #7,D2w` porte sur D2, laissé
    /// par l'appelant.
    fn bounce(x: &mut Cx) {
        // Glitch d'origine : le tirage n'est pas utilisé, le rebond est choisi par le D2 hérité.
        x.random();
        let d2 = x.c.d[2] & 7;
        x.c.d[2] = x.c.d[2] & 0xffff_0000 | d2;
        let d0 = x.u16(0x8);
        let (cx, cy) = camera(x);
        let d1 = cx.wrapping_add(8);
        x.c.d[0] = d0 as u32;
        x.c.d[1] = d1 as u32;
        if d1 > d0 {
            let d1 = d1.wrapping_add(1);
            x.set16(0x8, d1);
            let a = x.data8(BOUNCE_LEFT + d2);
            x.set8(0x2d, a);
            x.set8(0x54, 0x18);
            return;
        }
        let d1 = cx.wrapping_add(0x178);
        if d1 <= d0 {
            let d1 = d1.wrapping_sub(1);
            x.set16(0x8, d1);
            let a = x.data8(BOUNCE_RIGHT + d2);
            x.set8(0x2d, a);
            x.set8(0x54, 0x18);
            return;
        }
        let d0 = x.u16(0x10);
        let d1 = cy.wrapping_add(0x28);
        if d1 > d0 {
            let d1 = d1.wrapping_add(1);
            x.set16(0x10, d1);
            let a = x.data8(BOUNCE_TOP + d2);
            x.set8(0x2d, a);
            x.set8(0x54, 0x18);
        }
        let d1 = cy.wrapping_add(0xc0);
        if d1 <= d0 {
            let d1 = d1.wrapping_sub(1);
            x.set16(0x10, d1);
            let a = x.data8(BOUNCE_BOTTOM + d2);
            x.set8(0x2d, a);
            x.set8(0x54, 0x18);
        }
    }

    // --- variante 1 : halo de l'éclat --------------------------------------------------------------

    fn halo(x: &mut Cx) {
        match x.state(0x4be9e) {
            0 => {
                x.add8(0x4, 2);
                let parent = x.link_a1(0x5c);
                x.copy_pos(parent);
                x.add16(0x10, 0xffff);
                x.anim(ANIMS, 1);
            }
            2 => {
                x.anim_step();
                let parent = x.link_a1(0x5c);
                let flags = parent.u8(x.b, 0x1);
                x.set8(0x1, flags);
                x.copy_pos(parent);
                x.add16(0x10, 0xffff);
                if x.a5_u8(0x3d) == 0 {
                    x.add8(0x4, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    // --- variante 2 : brume ------------------------------------------------------------------------

    fn mist(x: &mut Cx) {
        match x.state(0x4bf16) {
            0 => {
                x.add8(0x4, 2);
                crate::sound::jouer(x.b, SFX, x.o);
                x.set16(0x54, 0x168);
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x15e) != 0x1a {
                    x.set16(0x54, 0x78);
                }
                mist_tick(x);
            }
            2 => mist_tick(x),
            _ => x.free_peff(),
        }
    }

    fn mist_tick(x: &mut Cx) {
        x.add16(0x54, 0xffff);
        if x.u16(0x54) == 0 {
            x.add8(0x4, 2);
        }
        let t = x.u16(0x54);
        let d0 = x.u8(0x55);
        let mut mask = 1u8;
        if t <= 0x78 {
            mask = 3;
            if t <= 0x3c {
                mask = 7;
            }
        }
        x.c.d[1] = x.c.d[1] & !0xff | mask as u32;
        x.c.d[0] = x.c.d[0] & !0xff | (d0 & mask) as u32;
        if d0 & mask == 0 {
            // flocon (variante 3), sans lien
            if let Some(o) = x.alloc(&pools::PEFF) {
                o.set_u8(x.b, 0, 1);
                o.set_u8(x.b, 2, 0xc);
                o.set_u8(x.b, 3, 3);
            }
        }
        x.c.d[0] = (t & 0x3f) as u32;
        if t & 0x3f == 0 {
            crate::sound::jouer(x.b, SFX, x.o);
        }
    }

    // --- variante 3 : flocon -----------------------------------------------------------------------

    fn flake(x: &mut Cx) {
        match x.state(0x4bf92) {
            0 => {
                x.add8(0x4, 2);
                let (cx, cy) = camera(x);
                x.set16(0x8, cx);
                x.set16(0x10, cy);
                let a = x.rand_and(0xf);
                x.c.d[1] = a;
                let b = x.rand_and(0xf);
                let (a2, b2) = (b * 2, a * 2);
                x.c.d[0] = a2;
                x.c.d[1] = b2;
                let dx = x.rom16(SCATTER_X + a2);
                let dy = x.rom16(SCATTER_Y + b2);
                x.set_d0w(dx);
                x.c.d[1] = x.c.d[1] & 0xffff_0000 | dy as u32;
                x.add16(0x8, dx);
                x.add16(0x10, dy);
                x.c.d[0] = x.u16(0x8) as u32;
                x.c.d[1] = x.u16(0x10) as u32;
                // registres de 0x24b0 laissés : D2 (le terrain) est relu par le rebond d'un gestionnaire suivant
                let (px, py) = (x.u16(0x8), x.u16(0x10));
                let t = crate::video::terrain(x.b, px, py);
                crate::video::terrain_68k(x.c, &t);
                if t.numero != 0 {
                    x.set8(0x4, 4);
                }
                let n = x.rand_and(1) + 2;
                x.anim(ANIMS, n);
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    // --- variante 4 : éclat tombant ----------------------------------------------------------------

    fn falling(x: &mut Cx) {
        match x.state(0x4c076) {
            0 => {
                x.add16(0x54, 0xffff);
                if x.u16(0x54) != 0 {
                    return;
                }
                x.add8(0x4, 2);
                let k = x.u8(0x2f) as u32;
                x.c.d[0] = k;
                let dx = x.rom16(SPREAD + k);
                x.set_d0w(dx);
                let dx = signed(dx, x.u8(f::FLIP) == 0);
                x.add16(0x8, dx);
                x.anim(ANIMS, 4);
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    // --- variante 5 : couronne autour du lanceur ----------------------------------------------------

    fn crown(x: &mut Cx) {
        match x.state(0x4c0da) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                x.copy_pos(owner);
                x.add16(0x10, 0xffff);
                let k = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                x.c.d[0] = k;
                let table = if owner.u8(x.b, 0x15e) == 0x1a { CROWN_A } else { CROWN_B };
                let (dx, dz) = offset(x, table, k);
                let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, 5);
            }
            2 => {
                x.anim_step();
                let frozen = x.a5_u8(0x3d) != 0;
                if frozen {
                    let owner = x.link_a1(f::OWNER);
                    let gesture = owner.u8(x.b, 0x7);
                    x.c.d[0] = x.c.d[0] & !0xff | gesture as u32;
                    if gesture == x.u8(0x56) {
                        return x.reinsert();
                    }
                }
                x.add8(0x4, 2);
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 13 (0x4c1b6) : bénédiction (variantes 0 à 2) : posée devant le lanceur, elle clignote tant que
/// son effet (`+0x168` du lanceur) dure.
mod blessing {
    use super::*;

    const OFFSETS: u32 = 0x4c226;
    const ANIMS: u32 = 0x1b670a;

    pub fn run(x: &mut Cx) {
        match x.state(0x4c1c4) {
            0 => {
                x.add8(0x4, 2);
                follow_owner(x);
                x.add16(0x10, 0xffff);
                let v = x.u8(0x3);
                let k = (v as u32) << 2;
                x.c.d[0] = k;
                let (dx, dz) = offset(x, OFFSETS, k);
                let dx = signed(dx, x.u8(f::FLIP) != 0);
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                x.anim(ANIMS, (v & 1) as u32);
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u16(x.b, 0x168) == 0 {
                    x.add8(0x4, 2);
                }
                x.set_bit(0x1, 1, false);
                let blink = x.a5_u8(a5::FRAME) & 1;
                x.c.d[0] = x.c.d[0] & !0xff | blink as u32;
                if blink != 0 {
                    x.set_bit(0x1, 1, true);
                }
                x.anim_step();
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }
}

/// Type 14 (0x4c2b4) : renvoi. Variante 0 : rayon posé sur le lanceur ; variantes 1 : quatre faisceaux qui
/// s'abattent sur le monstre visé (`+0x5c`) et le blessent en touchant le sol.
mod banish {
    use super::*;

    const ANIMS: u32 = 0x1b67f2;
    const ALT_BANK: u32 = 0x4c3a4;
    const FRAMES_A: u32 = 0x4c3a8;
    const FRAMES_B: u32 = 0x4c3d6;
    const SFX: u32 = 0xe9e9e;
    const POWER_TABLE: u32 = 0x1042e2;
    const POWER_TABLE_BOSS: u32 = 0x104300;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4c2c2) {
            0 if !child => {
                x.add8(0x4, 2);
                follow_owner(x);
                let dx = signed(-7i16 as u16, x.u8(f::FLIP) != 0);
                x.c.d[0] = dx as u32;
                x.c.d[1] = 0x5e;
                x.add16(0x8, dx);
                x.add16(0xc, 0x5e);
                x.add16(0x10, 0xffff);
                x.anim(ANIMS, 0);
            }
            0 => init_beam(x),
            2 if !child => {
                x.anim_step();
                x.set_bit(0x1, 1, true);
                let blink = x.a5_u8(a5::FRAME) & 1;
                x.c.d[0] = x.c.d[0] & !0xff | blink as u32;
                if blink != 0 {
                    x.set_bit(0x1, 1, false);
                }
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x7) == 4 {
                    x.add8(0x4, 2);
                }
                x.reinsert();
            }
            2 => fall(x),
            _ => x.free_peff(),
        }
    }

    fn init_beam(x: &mut Cx) {
        x.add8(0x4, 2);
        let target = Obj(x.u16(0x5c) as i16 as i32 as u32 & 0xff_ffff);
        x.c.a[2] = x.u16(0x5c) as i16 as i32 as u32;
        let (tx, ty) = (target.u16(x.b, 0x8), target.u16(x.b, 0x10));
        x.set16(0x8, tx);
        x.set16(0x10, ty);
        target.set_u16(x.b, 0x12, 0x4000);
        let k = x.u8(0x2f);
        let priority = if k & 2 != 0 { 0x8000 } else { 0 };
        x.c.d[1] = k as u32;
        x.set16(0x12, priority);
        let (_, cy) = camera(x);
        let z = cy.wrapping_add(0x100).wrapping_sub(x.u16(0x10));
        x.set16(0xc, z);
        x.c.d[0] = k as u32;
        let hold = x.rom8(ALT_BANK + k as u32);
        x.set8(0x58, hold);
        let kind = target.u8(x.b, 0x2) as u32;
        let row = kind * 2;
        x.c.d[1] = row;
        let mut frame = x.rom16(FRAMES_A + row);
        if target.u8(x.b, 0x28) != 4 {
            frame = x.rom16(FRAMES_B + row);
        }
        let anim = frame & 0xff00 | (frame as u8).wrapping_add(k) as u16;
        let n = (x.c.d[0] & 0xffff_0000 | anim as u32) as u16;
        x.anim_start(ANIMS, n);
        if k == 0 {
            crate::sound::jouer(x.b, SFX, x.o);
        }
    }

    fn fall(x: &mut Cx) {
        let k = x.u8(0x2f);
        if x.u8(0x58) != 0 {
            x.add8(0x58, 0xff);
        } else if k & 1 != 0 {
            if x.u16(0xc) > 0x60 {
                x.add16(0xc, 0xfffd);
            }
        } else if (x.u16(0xc) as i16) >= 0 {
            x.add16(0xc, 0xfffd);
        }
        if x.a5_u8(0x3d) == 0 {
            x.add8(0x4, 2);
        }
        strike(x);
        x.anim_step();
        x.reinsert();
    }

    /// Le faisceau 0 touche le sol : blesse le monstre visé (`0x4c4a0`).
    fn strike(x: &mut Cx) {
        if x.u8(0x2f) != 0 || (x.u16(0xc) as i16) >= 0 {
            return;
        }
        let player = x.link_a1(f::OWNER);
        let v = player.u16(x.b, 0x190).wrapping_add(2);
        player.set_u16(x.b, 0x190, v);
        let monster = Obj(x.u16(0x5c) as i16 as i32 as u32 & 0xff_ffff);
        x.c.a[2] = x.u16(0x5c) as i16 as i32 as u32;
        let hits = monster.u8(x.b, 0x9c).wrapping_add(1);
        monster.set_u8(x.b, 0x9c, hits);
        if monster.u8(x.b, 0x49) == 0x5c {
            return;
        }
        monster.set_u32(x.b, 0x4, 0x0400_0000);
        monster.set_u8(x.b, 0x1, 1);
        monster.set_u8(x.b, 0x49, 0x5c);
        let (class_id, variant_id) = (player.u8(x.b, 0x2), player.u8(x.b, 0x3));
        monster.set_u8(x.b, 0x48, class_id);
        monster.set_u8(x.b, 0x4b, variant_id);
        monster.set_u16(x.b, 0x54, player.0 as u16);
        monster.set_u8(x.b, 0x4a, 0);
        if x.rand_and_b(1) != 0 {
            monster.set_u8(x.b, 0x4a, 0x20);
        }
        let class = player.u8(x.b, 0x10c) as u32;
        let table = if monster.u8(x.b, 0x28) == 4 { POWER_TABLE_BOSS } else { POWER_TABLE };
        let mut damage = x.data8(table + class) as u16;
        if player.u8(x.b, 0x166) == 4 {
            damage += damage >> 2;
        }
        x.c.d[0] = damage as u32;
        let hp = monster.u16(x.b, 0x62).wrapping_sub(damage);
        monster.set_u16(x.b, 0x62, if (hp as i16) < 0 { 0 } else { hp });
        if monster.u8(x.b, 0x28) != 4 && matches!(monster.u8(x.b, 0x2), 0 | 0x13) {
            monster.set_u16(x.b, 0x62, 0);
        }
        if monster.u16(x.b, 0x62) == 0 && monster.u8(x.b, 0x93) != 0 {
            player.set_u8(x.b, 0x18a, 1);
            let v = monster.u8(x.b, 0x9a);
            x.set_a5_u8(0x4f, v);
        }
        let mut d0 = (player.u8(x.b, 0x2) as u16) << 8;
        d0 = d0 & 0xff00 | (d0 as u8).wrapping_add(monster.u8(x.b, 0x2)) as u16;
        let (max, valeur, classe) = (monster.u16(x.b, 0x60), monster.u16(x.b, 0x62), monster.u8(x.b, 0x28));
        crate::evenements::creer_popup(x.b, crate::evenements::Popup { entete: d0, max, valeur, brut: classe == 2 });
    }
}

handlers! {
    0x4bc66 "peff_12" ice::run,
    0x4c1b6 "peff_13" blessing::run,
    0x4c2b4 "peff_14" banish::run,
}
