//! Effets des joueurs (PEFF), troisième série : types 2 à 5 (aura de maintien, coup, soin, lumière).
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;

/// Type 2 (0x4a880) : halo du joueur qui garde la pose 0x18 (`+0x6a` du joueur) tant que son
/// animation tourne (jeu 0x1b540c).
mod halo {
    use super::*;

    const OFFSETS: u32 = 0x4a93a;
    const ANIMS: u32 = 0x1b540c;

    pub fn run(x: &mut Cx) {
        match x.state(0x4a88e) {
            0 => {
                x.add8(0x4, 2);
                place(x);
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                let owner = x.link_a1(f::OWNER);
                if owner.u8(x.b, 0x5) != 0 || owner.u8(x.b, 0x6a) != 0x18 {
                    return x.free_peff();
                }
                if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
                    if x.anim_step() {
                        x.add8(0x4, 2);
                    }
                }
                place(x);
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn place(x: &mut Cx) {
        let owner = follow_owner(x);
        let row = (x.u8(0x33) as u32) << 2 & 0xffff;
        x.c.d[0] = row;
        let (dz_part, dz) = offset(x, OFFSETS, row);
        x.add16(0xc, dz_part);
        x.set16(0x12, 0);
        owner.set_u16(x.b, 0x12, 0x8000);
        if dz != 0 {
            owner.set_u16(x.b, 0x12, 0);
            x.set16(0x12, 0x8000);
        }
    }
}

/// Type 3 (0x4a9fe) : trace d'un coup, collée au joueur le temps de son geste (`+0x5c` = geste du
/// joueur, `+7`). La variante 0 prend l'animation `+0x2f`, la variante 1 l'animation 3.
mod slash {
    use super::*;

    const ANIMS: u32 = 0x1b5590;
    const OFFSETS_A: u32 = 0x4aa90;
    const OFFSETS_B: u32 = 0x4ab2a;

    pub fn run(x: &mut Cx) {
        match x.state(0x4aa0c) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        if x.u8(0x3) == 0 {
            let n = (x.u8(0x2f) as u32) as u16;
            x.anim_start(ANIMS, n);
            return;
        }
        follow(x, owner, OFFSETS_A);
        x.anim_start(ANIMS, 3);
    }

    /// Place la trace devant le joueur (décalage selon sa variante, inversé selon son regard).
    fn follow(x: &mut Cx, owner: Obj, table: u32) {
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.set16(0x12, 0);
        owner.set_u16(x.b, 0x12, 0x8000);
        let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
        x.c.d[0] = row;
        let (mut dx, dz) = offset(x, table, row);
        if flip == 0 {
            dx = dx.wrapping_neg();
        }
        x.c.d[1] = dx as u32;
        x.add16(0x8, dx);
        x.add16(0xc, dz);
    }

    fn active(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if owner.u8(x.b, 0x5) != 0 || x.u8(0x5c) != owner.u8(x.b, 0x7) {
            return x.free_peff();
        }
        x.c.d[0] = x.u8(0x5c) as u32;
        if x.u8(0x3) == 0 {
            follow(x, owner, OFFSETS_B);
        }
        if x.a5_u8(0x3d) == 0 && owner.u8(x.b, 0x53) == 0 {
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        x.reinsert();
    }
}

/// Type 4 (0x4abaa) : soin (variante 0 : étoiles autour du soigneur, 1 : étincelle qui monte, 2 :
/// potion). Voir `player/spawn.rs` (`cure`, `potion`).
mod cure {
    use super::*;

    const ANIMS: u32 = 0x1b56da;
    const OFFSETS_PRAYER: u32 = 0x4acb2;
    const OFFSETS_PLAYER: u32 = 0x4ad38;
    const OFFSETS_SPARK: u32 = 0x4ade0;
    const SPARK_VZ: u32 = 0x4adc8;
    const SPARK_TIMES: u32 = 0x4add8;
    const SFX_PRAYER: u32 = 0xe8f5c;
    const SFX_PRAYER_B: u32 = 0xe8fc6;

    pub fn run(x: &mut Cx) {
        match x.state(0x4abb8) {
            0 => match x.u8(0x3) {
                0 => init_prayer(x),
                1 => init_spark(x),
                _ => init_potion(x),
            },
            2 => match x.u8(0x3) {
                1 => spark(x),
                _ => hold(x),
            },
            _ => x.free_peff(),
        }
    }

    fn init_prayer(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = follow_owner(x);
        let (mut dx, mut dz) = (4i16, 0x65i16);
        x.c.d[0] = dx as u32 & 0xffff;
        x.c.d[1] = dz as u32;
        if owner.u8(x.b, 0x1ee) != 0 {
            (dx, dz) = (-0x36, 0x4e);
        }
        if x.u8(f::FLIP) != 0 {
            dx = -dx;
        }
        x.c.d[0] = dx as u16 as u32;
        x.c.d[1] = dz as u16 as u32;
        x.add16(0x8, dx as u16);
        x.add16(0xc, dz as u16);
        x.add16(0x10, 0xffff);
        x.set16(0x12, 0x8000);
        if x.u8(0x2f) == 0 {
            crate::sound::jouer(x.b, SFX_PRAYER, x.o);
        } else {
            crate::sound::jouer(x.b, SFX_PRAYER_B, x.o);
        }
        let n = (x.u8(0x2f) as u32) as u16;
        x.anim_start(ANIMS, n);
    }

    fn init_spark(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        x.copy_pos(owner);
        x.set16(0x12, 0);
        let row = (owner.u8(x.b, 0x5f) as u32) << 2 & 0xffff;
        x.c.d[0] = row;
        let (dx, dz) = offset(x, OFFSETS_SPARK, row);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        let r = (x.rand_and(7)) * 2;
        let vz = x.rom16(SPARK_VZ + r);
        x.set16(0x18, vz);
        x.set16(0x1a, 0);
        let r = x.rand_and(7);
        let t = x.rom8(SPARK_TIMES + r);
        x.set8(0x5c, t);
        let r = x.rand_and(1);
        x.set8(0x2f, r as u8);
        x.anim_start(ANIMS, (r + 2) as u16);
    }

    /// Potion : l'effet se pose sur le joueur qui boit (le soigneur lui-même, ou le premier joueur actif).
    fn init_potion(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let source = if owner.u8(x.b, 0x1ee) == 0 {
            let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
            x.c.d[0] = row;
            let (dx, dz) = offset(x, OFFSETS_PRAYER, row);
            Some((owner, dx, dz))
        } else {
            let mut found = None;
            let mut p = Obj(0xff85de);
            x.c.a[1] = 0xff85de;
            for _ in 0..4 {
                if p.u8(x.b, 0x78) != 0 {
                    let row = ((p.u8(x.b, 0x3) >> 1) as u32) << 2;
                    x.c.d[0] = row;
                    let (dx, dz) = offset(x, OFFSETS_PLAYER, row);
                    found = Some((p, dx, dz));
                    break;
                }
                p = Obj(p.0 + 0x200);
                x.c.a[1] = p.0;
            }
            found
        };
        if let Some((p, mut dx, dz)) = source {
            if p.u8(x.b, f::FLIP) != 0 {
                dx = dx.wrapping_neg();
            }
            x.c.d[1] = dx as u32;
            let flip = p.u8(x.b, f::FLIP);
            x.set8(f::FLIP, flip);
            x.copy_pos(p);
            x.add16(0x8, dx);
            x.add16(0xc, dz);
            x.add16(0x10, 0xffff);
            x.set16(0x12, 0x8000);
        }
        x.anim_start(ANIMS, 0);
        crate::sound::jouer(x.b, SFX_PRAYER, x.o);
    }

    /// Variantes 0 et 2 : l'étoile tourne une image puis disparaît ; tous les 4 temps de la durée de
    /// soin (`+0x168` du joueur) elle crée une étincelle.
    fn hold(x: &mut Cx) {
        x.anim_step();
        if x.a5_u8(0x3d) == 0 {
            x.add8(0x4, 2);
        }
        let owner = x.link_a1(f::OWNER);
        let timer = owner.u16(x.b, 0x168);
        x.set_d0w(timer);
        if timer > 0x1e && timer & 3 == 0 {
            spawn_spark(x);
        }
        x.reinsert();
    }

    /// `0x4ab82` : étincelle (variante 1) ; le compteur `+0x5f` tourne sur 8.
    fn spawn_spark(x: &mut Cx) {
        if let Some(o) = x.alloc(&pools::PEFF) {
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, 4);
            o.set_u8(x.b, 3, 1);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            let v = x.u8(0x5f).wrapping_add(1) & 7;
            x.set8(0x5f, v);
        }
    }

    fn spark(x: &mut Cx) {
        x.move_z();
        let fin = x.anim_step();
        if x.u8(0x2f) == 0 {
            if fin {
                x.add8(0x4, 2);
            }
        } else {
            x.add8(0x5c, 0xff);
            if x.u8(0x5c) == 0 {
                x.add8(0x4, 2);
            }
        }
        x.reinsert();
    }
}

/// Type 5 (0x4af14) : lumière sacrée (variante 0 : colonne qui clignote puis éclate, créant deux
/// halos de variante 1, avec secousses d'écran selon un script de la ROM ; variante 1 : halo posé
/// au sol, +2f = 0 devant, 1 derrière).
mod holy {
    use super::*;

    const ANIMS: u32 = 0x1b598c;
    const SHAKE_TABLE: u32 = 0x4b172;
    const SCRIPT_BY_PLAYER: u32 = 0x4b0b2;
    const SCRIPTS: u32 = 0x4b0da;
    const SFX_START: u32 = 0xe9f5e;
    const SFX_BURST_A: u32 = 0xe9fc8;
    const SFX_BURST_B: u32 = 0xe996c;
    const SFX_END: u32 = 0xea032;
    const BURST_SPAWN: u32 = 0x4aed0;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4af22) {
            0 if !child => init(x),
            0 => init_halo(x),
            2 if !child => active(x),
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
                x.reinsert();
            }
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x58, 0);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        let mut dx = -2i16;
        if x.u8(f::FLIP) != 0 {
            dx = 2;
        }
        x.c.d[0] = dx as u16 as u32;
        x.c.d[1] = 0x7a;
        x.add16(0x8, dx as u16);
        x.add16(0xc, 0x7a);
        x.add16(0x10, 0xffff);
        x.anim_start(ANIMS, 0);
        crate::sound::jouer(x.b, SFX_START, x.o);
    }

    fn init_halo(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        x.copy_pos(owner);
        let dy: u16 = if x.u8(0x2f) != 0 { 1 } else { 0xffff };
        x.c.d[2] = dy as u32;
        x.add16(0x10, dy);
        let anim = x.u8(0x2f) as u32 + 1;
        x.anim_start(ANIMS, anim as u16);
    }

    fn active(x: &mut Cx) {
        match x.u8(0x5) {
            0 => rise(x),
            2 => fall(x),
            _ => script(x),
        }
        x.reinsert();
    }

    /// Colonne qui clignote de plus en plus lentement, puis s'élève quand le joueur change de geste.
    fn rise(x: &mut Cx) {
        x.add16(0x58, 1);
        x.set_bit(0x1, 1, false);
        let t = x.u16(0x58);
        if t > 0x40 {
            x.anim_step();
            let owner = x.link_a1(f::OWNER);
            let action = owner.u8(x.b, 0x7);
            x.c.d[0] = x.c.d[0] & !0xff | action as u32;
            if action != x.u8(0x56) {
                x.add8(0x5, 2);
                x.set16(0x18, -0x400i16 as u16);
                x.set16(0x1a, -0x80i16 as u16);
            }
            return;
        }
        let mut mask = 1u16;
        if t <= 0x30 {
            mask = 3;
            if t <= 0x20 {
                mask = 7;
            }
        }
        x.c.d[1] = mask as u32;
        x.c.d[0] = (t & mask) as u32;
        if t & mask != 0 {
            x.set_bit(0x1, 1, true);
        }
    }

    /// Chute de la colonne jusqu'au sol, puis éclatement.
    fn fall(x: &mut Cx) {
        x.anim_step();
        x.move_z();
        if x.u16(0xc) > 8 {
            return;
        }
        x.add8(0x5, 2);
        x.set16(0x58, 0x50);
        x.set16(0xc, 8);
        x.anim_start(ANIMS, 0);
        x.call(BURST_SPAWN);
        crate::sound::jouer(x.b, SFX_BURST_A, x.o);
        crate::sound::jouer(x.b, SFX_BURST_B, x.o);
        let owner = x.link_a1(f::OWNER);
        let player = owner.u8(x.b, 0x10c) as u32;
        let row = x.rom8(SCRIPT_BY_PLAYER + player) as u32;
        x.c.d[0] = row;
        x.c.a[1] = SCRIPTS;
        let script = x.data32(SCRIPTS + row);
        x.set32(0x5c, script);
        x.set16(0x58, 8);
        x.set8(0x5b, 0);
    }

    /// Script de secousses d'écran : paires (commande, durée) lues à la suite dans la ROM.
    fn script(x: &mut Cx) {
        if x.u8(0x5b) == 0 {
            let mut at = x.u32(0x5c);
            let command = x.data8(at);
            let duration = x.data8(at + 1);
            at += 2;
            x.set8(0x5a, command);
            x.set8(0x5b, duration);
            x.set32(0x5c, at);
            x.c.a[1] = at;
            if command == 0xff {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                owner.set_u16(x.b, 0x168, 1);
                crate::sound::jouer(x.b, SFX_END, x.o);
            } else {
                x.c.d[0] = duration as u32;
                x.c.d[1] = command as u32;
                // la table désigne la secousse par sa routine (0x2824, 0x2860, 0x289c), qui laisse A1 = la caméra
                let mode = crate::camera_modes::shake::of_routine(x.rom32(SHAKE_TABLE + command as u32));
                x.c.a[1] = crate::camera_modes::start_shake(x.b, mode, duration as u16).0;
            }
        }
        x.add8(0x5b, 0xff);
        x.add16(0x58, 0xffff);
        x.set_bit(0x1, 1, true);
        let t = x.u16(0x58);
        x.c.d[0] = t as u32;
        if (t as i16) >= 0 && t & 1 != 0 {
            x.set_bit(0x1, 1, false);
        }
    }
}

handlers! {
    0x4a880 "peff_2" halo::run,
    0x4a9fe "peff_3" slash::run,
    0x4abaa "peff_4" cure::run,
    0x4af14 "peff_5" holy::run,
}
