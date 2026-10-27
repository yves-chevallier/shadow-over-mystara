//! Gestionnaires des types d'ennemis de la classe ENEMY (C02). Voir `docs/familles/enemy.md`.
//!
//! Même cadre que les familles ESHL / SET / SOBJ (`cx.rs`) : un type est un `mod` avec une fonction
//! `run`, des constantes nommées pour ses tables et ses jeux d'animation, et un commentaire de rôle.
//! Les noms de rôle sont des hypothèses (h) tirées du comportement et des noms de debug du jeu.

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::mem::{wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;

// ---------------------------------------------------------------------------------------------
// Type 20 (0x74fc0) : « ManScorpion » (h), ennemi composite greffé sur un ennemi de type 19
// ---------------------------------------------------------------------------------------------

/// Créé par un ennemi de type 19 (parent en `+0x50`), il engendre ses halos (ESHL type 11 en
/// `+0xa4/+0xa6`), une ombre (EFFCT 0x20 en `+0xb4`), deux flammes (ITEM type 6 en `+0xa8/+0xaa`) et,
/// pour la variante 1, deux bras (ENEMY type 19, `+0xac/+0xae`, `+0xb0` = bras présents).
/// Variantes 0 et 1 : vol vers le bord gauche en jetant des étincelles ; variante 2 : vol vers le
/// bord droit ; quand les bras sont tombés le parent est averti. États : 0 création, 2 vol,
/// 4 touché (recul de 15 images puis mort en plusieurs temps), 6 libération des parties.
mod man_scorpion {
    use super::*;

    const ANIMS: u32 = 0x169fb0;
    const PART_TYPE: u8 = 0xb;
    const ARM_TYPE: u8 = 0x13;
    const SHADOW_TYPE: u8 = 0x20;
    const FLAME_TYPE: u8 = 6;
    /// État long d'un bras lâché (état 2, sous-état 6, sous-sous-état 0xc).
    const ARM_RELEASED: u32 = 0x0206_0c00;
    /// Touché, mort en cours (état 4, sous-état 2).
    const DYING: u32 = 0x0402_0600;

    /// Champs propres à ce type.
    mod p {
        pub const LIMBS: u32 = 0xa4;
        pub const FLAMES: u32 = 0xa8;
        pub const ARMS: u32 = 0xac;
        pub const SHADOW: u32 = 0xb4;
        pub const ARMS_MASK: u32 = 0xb0;
        pub const COUNTER: u32 = 0xb2;
        pub const DELAY: u32 = 0xa2;
        pub const HIT_FREEZE: u32 = 0x56;
    }

    pub fn run(x: &mut Cx) {
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit(x),
            st::DEAD => die(x),
            _ => {}
        }
    }

    // --- création ------------------------------------------------------------------------------

    fn init(x: &mut Cx) {
        x.set32(f::IMMUNITY, 0xffff_ffff);
        x.set8(0x7e, 0xb);
        let r = random(x);
        x.set8(p::COUNTER, r as u8);
        // au moins deux joueurs vivants, et pas plus d'un ennemi composite déjà en scène
        let w = |x: &mut Cx, off: u32| x.a5_u16(off) as i16;
        if w(x, 0x7e64) < 2 || w(x, 0x7e6e) < 2 {
            return;
        }
        if !(w(x, 0x7e62) < 2 || w(x, 0x7e68) >= 1) {
            return;
        }
        match x.u8(0x3) {
            0 => spawn(x, 0, false),
            1 => spawn(x, 0, true),
            2 => spawn(x, 2, false),
            _ => {}
        }
        x.set16(0x16, 0);
        x.c.d[0] = 0;
        x.set8(f::FLIP, 0);
        x.set8(f::BOX_SET, 0x3b);
        x.set16(0xc, 4);
        lib::set_immunities(x);
        x.c.d[0] = 0;
        x.anim_start(ANIMS, 0);
        lib::initial_hp(x);
        lib::sound(x, 0xea8be);
    }

    /// Crée les parties ; `flame_kind` est la variante des flammes ; la variante 1 crée aussi les bras.
    fn spawn(x: &mut Cx, flame_kind: u8, with_arms: bool) {
        spawn_limbs(x);
        spawn_shadow(x);
        spawn_flames(x, flame_kind);
        if with_arms {
            let mut at = x.o.0 + p::ARMS;
            for d5 in [1u8, 0] {
                x.c.d[5] = d5 as u32;
                let o = alloc(x, &pools::ENEMY);
                let active = x.u8(0);
                o.set_u8(x.b, 0, active);
                o.set_u8(x.b, 0xbc, d5);
                o.set_u8(x.b, 3, 0);
                o.set_u8(x.b, 0x2f, 1);
                o.set_u8(x.b, 2, ARM_TYPE);
                o.set_u16(x.b, f::CREATOR, x.o.0 as u16);
                wr16(x.b, at, o.0 as u16);
                at += 2;
            }
            x.set8(p::ARMS_MASK, 3);
            x.add8(f::STATE, 2);
            return;
        }
        let parent = x.linked(f::OWNER);
        let v = parent.u8(x.b, 0x30).wrapping_add(2);
        parent.set_u8(x.b, 0x30, v);
        x.add8(f::STATE, 2);
        x.set16(0x14, 0xfb00);
    }

    /// Deux halos (ESHL type 11, variantes 1 puis 0) rangés en `+0xa4`, `+0xa6`.
    fn spawn_limbs(x: &mut Cx) {
        let mut at = x.o.0 + p::LIMBS;
        for d5 in [1u8, 0] {
            x.c.d[5] = d5 as u32;
            let o = alloc(x, &pools::ESHL);
            let active = x.u8(0);
            o.set_u8(x.b, 0, active);
            o.set_u8(x.b, 3, d5);
            o.set_u8(x.b, 2, PART_TYPE);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            wr16(x.b, at, o.0 as u16);
            at += 2;
        }
    }

    fn spawn_shadow(x: &mut Cx) {
        let o = alloc(x, &pools::EFFCT);
        let active = x.u8(0);
        o.set_u8(x.b, 0, active);
        o.set_u8(x.b, 2, SHADOW_TYPE);
        o.set_u16(x.b, f::OWNER, x.o.0 as u16);
        x.set16(p::SHADOW, o.0 as u16);
    }

    /// Deux flammes (ITEM type 6, orientation 0x20 puis 0) rangées en `+0xa8`, `+0xaa`.
    fn spawn_flames(x: &mut Cx, kind: u8) {
        let mut at = x.o.0 + p::FLAMES;
        for d5 in [1u8, 0] {
            x.c.d[5] = d5 as u32;
            x.c.d[4] = kind as u32;
            let o = alloc(x, &pools::ITEM);
            let active = x.u8(0);
            o.set_u8(x.b, 0, active);
            o.set_u8(x.b, f::FLIP, d5 << 5);
            o.set_u8(x.b, 2, FLAME_TYPE);
            o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            o.set_u8(x.b, 3, kind);
            let parent = x.u16(f::OWNER);
            o.set_u16(x.b, f::HOST, parent);
            wr16(x.b, at, o.0 as u16);
            at += 2;
        }
    }

    // --- état 2 : vol --------------------------------------------------------------------------

    fn active(x: &mut Cx) {
        check_arms(x);
        if !paused(x) {
            match (x.u8(0x3), x.u8(f::SUB)) {
                (0, 0) | (1, _) => {
                    if x.u8(0x3) == 0 && x.u8(p::ARMS_MASK) == 0 {
                        stop_flight(x);
                    } else {
                        fly_left(x);
                    }
                }
                (2, 0) => {
                    if x.u8(p::ARMS_MASK) == 0 {
                        stop_flight(x);
                    } else {
                        fly_right(x);
                    }
                }
                (0, 2) | (2, 2) => take_from_parent(x),
                _ => {}
            }
        }
        x.reinsert();
    }

    /// Plus de bras : sous-état 2, puis tentative de passage à l'état « mort en cours ».
    fn stop_flight(x: &mut Cx) {
        x.add8(f::SUB, 2);
        detach_from_host(x);
        take_from_parent(x);
    }

    /// Le coup de grâce n'est possible que tant que le parent a des coups en réserve (`+0x32`).
    fn take_from_parent(x: &mut Cx) {
        let parent = x.linked(f::OWNER);
        let left = parent.u8(x.b, 0x32);
        if left == 0 {
            x.anim_step();
            return;
        }
        parent.set_u8(x.b, 0x32, left - 1);
        x.set32(f::STATE, DYING);
    }

    /// Étincelles : tous les 8 pas, tous les 16 s'il n'y a qu'un bras.
    fn sparkles(x: &mut Cx) {
        let mask = x.u8(p::ARMS_MASK);
        let one_arm = (mask & 1 != 0) != (mask & 2 != 0);
        let step = if one_arm { 0xf } else { 7 };
        let n = x.u8(p::COUNTER).wrapping_add(1) & step;
        x.set8(p::COUNTER, n);
        if n == 0 {
            lib::shock(x, 0, 0x201, 0x20, 0x10);
        }
    }

    /// Vol vers le bord gauche : mort (état 6) une fois sorti de l'écran.
    fn fly_left(x: &mut Cx) {
        sparkles(x);
        x.move_x();
        let edge = x.a5_u16(0x506).wrapping_sub(0x50) as i16;
        if edge > x.u16(0x8) as i16 {
            x.set32(f::STATE, st::DEAD_LONG);
        }
        x.anim_step();
    }

    /// Vol vers le bord droit ; à l'arrivée, lâche les bras et rend la main au parent.
    fn fly_right(x: &mut Cx) {
        sparkles(x);
        x.move_x();
        let dx = x.u16(0x8).wrapping_sub(x.a5_u16(0x506));
        if dx < 0x100 {
            let mask = x.u8(p::ARMS_MASK);
            for (bit, off) in [(1u8, p::ARMS), (2, p::ARMS + 2)] {
                if mask & bit != 0 {
                    let arm = x.linked(off);
                    arm.set_u32(x.b, f::STATE, ARM_RELEASED);
                    arm.set_u8(x.b, 0, 2);
                }
            }
            x.set8(p::ARMS_MASK, 0);
            let parent = x.linked(f::OWNER);
            parent.set_u8(x.b, 0x24, 0);
            x.set32(f::STATE, 0x0202_0000);
        }
        x.anim_step();
    }

    /// Un bras dont l'état est >= 4 est détaché ; sans plus de bras, le parent est libéré de son
    /// attente (`+0x24`). Laisse D0 = 0 comme l'original.
    fn check_arms(x: &mut Cx) {
        let mut left = x.u8(p::ARMS_MASK);
        if left != 0 {
            if left & 1 != 0 {
                let arm = x.linked(p::ARMS);
                if (arm.u8(x.b, f::STATE) as i8) >= 4 {
                    x.set16(p::ARMS, 0);
                    x.set_bit(p::ARMS_MASK, 0, false);
                    left &= !1;
                    x.set16(0x14, 0xfe80);
                }
            }
            if x.u8(p::ARMS_MASK) & 2 != 0 {
                let arm = x.linked(p::ARMS + 2);
                if (arm.u8(x.b, f::STATE) as i8) < 4 {
                    return x.c.d[0] = 0;
                }
                x.set16(p::ARMS + 2, 0);
                x.set_bit(p::ARMS_MASK, 1, false);
                left &= !2;
                x.set16(0x14, 0xfe80);
            }
            if left == 0 {
                let parent = x.linked(f::OWNER);
                parent.set_u8(x.b, 0x24, 0);
            }
        }
        x.c.d[0] = 0;
    }

    // --- état 4 : touché -----------------------------------------------------------------------

    fn hit(x: &mut Cx) {
        check_arms(x);
        let freeze = x.u8(f::FREEZE) as i8;
        if freeze > 0 {
            return x.set8(f::FREEZE, (freeze - 1) as u8);
        }
        match x.u8(f::SUB) {
            0 => {
                match x.u8(f::SUB2) {
                    0 => {
                        x.add8(f::SUB2, 2);
                        if (x.u16(f::HP) as i16) > 0 {
                            react(x);
                        } else {
                            x.set8(f::SUB, 2);
                            x.set8(f::SUB2, 0);
                            x.set8(0, 2);
                            dying(x);
                        }
                    }
                    2 => recoil(x),
                    _ => {}
                }
                x.reinsert();
            }
            2 => dying(x),
            _ => {}
        }
    }

    /// Copie le gel (`+0x58` long, `+0x53`) sur une partie.
    fn share_freeze(x: &mut Cx, o: Obj) {
        let (l, g) = (x.u32(0x58), x.u8(f::FREEZE));
        o.set_u32(x.b, 0x58, l);
        o.set_u8(x.b, f::FREEZE, g);
    }

    /// Début d'un coup : gèle les parties, démarre le recul (15 images) et l'effet `0xab7d8`.
    fn react(x: &mut Cx) {
        let freeze = x.u8(p::HIT_FREEZE);
        x.set8(f::FREEZE, freeze);
        x.set8(p::DELAY, 0xf);
        let (a1, a2) = (x.linked(p::LIMBS), x.linked(p::LIMBS + 2));
        share_freeze(x, a2);
        share_freeze(x, a1);
        a1.set_u32(x.b, f::STATE, 0x0400_0000);
        a2.set_u32(x.b, f::STATE, 0x0400_0000);
        let mask = x.u8(p::ARMS_MASK);
        for (bit, off) in [(1u8, p::ARMS), (2, p::ARMS + 2)] {
            if mask & bit != 0 {
                let arm = x.linked(off);
                arm.set_u8(x.b, f::FREEZE, freeze);
            }
        }
        lib::debris_spray(x, 0);
    }

    /// Recul de 15 images (2 pixels, sens alterné), puis retour à l'état 2.
    fn recoil(x: &mut Cx) {
        let left = x.u8(p::DELAY);
        if left != 0 {
            let left = left - 1;
            x.set8(p::DELAY, left);
            let mut d = 2i16;
            if left & 1 != 0 {
                d = -d;
            }
            if x.u8(f::FLIP) != 0 {
                d = -d;
            }
            x.add16(0x8, d as u16);
            return;
        }
        x.set8(0, 1);
        x.set32(0x58, 0);
        x.set32(f::STATE, 0x0200_0000);
        for off in [p::LIMBS, p::LIMBS + 2] {
            let o = x.linked(off);
            o.set_u32(x.b, 0x58, 0);
            o.set_u32(x.b, f::STATE, 0x0200_0000);
        }
    }

    /// Sous-état 2 : mort en plusieurs temps (`+6` : 0 lance la mort d'une variante 0, 6 celle
    /// d'une autre variante ; 2 et 8 attendent ; 4 et 10 finissent).
    fn dying(x: &mut Cx) {
        match x.u8(f::SUB2) {
            0 => death_start(x),
            2 | 8 => wait(x),
            4 => death_end(x),
            6 => death_start_b(x),
            10 => death_end_b(x),
            _ => {}
        }
    }

    fn death_start(x: &mut Cx) {
        x.add8(f::SUB2, 2);
        react(x);
        if x.u8(0x3) == 0 {
            let parent = x.linked(f::OWNER);
            let v = parent.u8(x.b, 0x21).wrapping_add(1);
            parent.set_u8(x.b, 0x21, v);
            parent.set_u8(x.b, 0x24, 0);
        }
        lib::sound(x, 0xeb4ba);
        for off in [p::LIMBS, p::LIMBS + 2, p::SHADOW] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, st::DEAD_LONG);
        }
        for off in [p::FLAMES, p::FLAMES + 2] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, 0x0202_0000);
        }
        for off in [p::ARMS, p::ARMS + 2] {
            if x.u16(off) != 0 {
                let o = x.linked(off);
                o.set_u8(x.b, 0x90, 1);
            }
        }
        detach_from_host(x);
        x.set8(p::DELAY, 0x5a);
        x.anim_start(ANIMS, 2);
        x.reinsert();
    }

    fn wait(x: &mut Cx) {
        let n = x.u8(p::DELAY).wrapping_sub(1);
        x.set8(p::DELAY, n);
        if n == 0 {
            x.add8(f::SUB2, 2);
            x.set8(p::DELAY, 0x28);
        }
        x.reinsert();
    }

    fn death_end(x: &mut Cx) {
        let n = x.u8(p::DELAY).wrapping_sub(1);
        x.set8(p::DELAY, n);
        if n != 0 {
            return blink(x);
        }
        if x.u8(0x3) == 0 {
            let parent = x.linked(f::OWNER);
            parent.set_u8(x.b, 0x26, 0xff);
        }
        x.set32(f::STATE, st::DEAD_LONG);
        lib::object_event(x);
        free(x);
    }

    fn death_start_b(x: &mut Cx) {
        x.add8(f::SUB2, 2);
        lib::sound(x, 0xeb4ba);
        let parent = x.linked(f::OWNER);
        let v = parent.u8(x.b, 0x21).wrapping_add(1);
        parent.set_u8(x.b, 0x21, v);
        parent.set_u8(x.b, 0x24, 0);
        for off in [p::LIMBS, p::LIMBS + 2, p::SHADOW] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, st::DEAD_LONG);
        }
        for off in [p::FLAMES, p::FLAMES + 2] {
            let o = x.linked(off);
            let s = o.u8(x.b, f::SUB).wrapping_add(2);
            o.set_u8(x.b, f::SUB, s);
        }
        x.set8(p::DELAY, 0x5a);
        x.anim_start(ANIMS, 2);
        x.reinsert();
    }

    fn death_end_b(x: &mut Cx) {
        let n = x.u8(p::DELAY).wrapping_sub(1);
        x.set8(p::DELAY, n);
        if n != 0 {
            return blink(x);
        }
        let parent = x.linked(f::OWNER);
        parent.set_u8(x.b, 0x26, 0xff);
        x.set32(f::STATE, st::DEAD_LONG);
        free(x);
    }

    /// Clignote pendant l'attente (caché quand `(+0xa2) & 3 != 0`), puis remet l'objet en liste.
    fn blink(x: &mut Cx) {
        let hidden = x.u8(p::DELAY) & 3 != 0;
        x.set_bit(0x1, 1, hidden);
        x.reinsert();
    }

    // --- état 6 : libération -------------------------------------------------------------------

    /// Libère les parties ; la variante 0 rend aussi ses compteurs au parent et à l'hôte.
    fn die(x: &mut Cx) {
        let first = x.u8(0x3) == 0;
        let mut host = None;
        if first {
            let h = x.linked(f::HOST);
            add16(x, h, 0x2a, 0xffff);
            host = Some(h);
            let parent = x.linked(f::OWNER);
            add8(x, parent, 0x20, 0xff);
            parent.set_u8(x.b, 0x24, 0);
            add8(x, parent, 0x30, 0xfe);
            let flag = parent.u8(x.b, 0x26);
            let v = x.u8(0xb1) ^ flag;
            x.set8(0xb1, v);
            if v != 0 {
                parent.set_u8(x.b, 0x26, 0xff);
            }
        }
        for off in [p::LIMBS, p::LIMBS + 2, p::SHADOW, p::FLAMES, p::FLAMES + 2] {
            let o = x.linked(off);
            o.set_u32(x.b, f::STATE, st::DEAD_LONG);
        }
        let mask = x.u8(p::ARMS_MASK);
        for (bit, off) in [(1u8, p::ARMS), (2, p::ARMS + 2)] {
            if mask & bit != 0 {
                let o = x.linked(off);
                o.set_u32(x.b, f::STATE, st::DEAD_LONG);
                if let Some(h) = host {
                    add16(x, h, 0x2a, 0xffff);
                }
            }
        }
        free(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 28 (0x79300) : « Nagpa » (h), personnage de scène qui apparaît quand un joueur approche
// ---------------------------------------------------------------------------------------------

/// Caché au départ (bit 1 de `+1`), il se montre quand un joueur vivant passe à moins de 0x30 pixels
/// (0x50 pour la variante 1), joue son animation d'apparition, puis redevient caché quand plus
/// personne n'est à portée. Le bit de la variante dans l'octet `0xff2064` (`A5-0x5f9c`, bit 1 variante 0, bit 0 variante 1) indique
/// qu'il est visible. Sous-états (`+5`) : 0 attente, 2 apparition, 4 visible, 6 disparition.
mod nagpa {
    use super::*;

    const ANIMS: u32 = 0x16c7fc;
    /// Octet de visibilité : bit 1 pour la variante 0, bit 0 pour la variante 1.
    const VISIBLE_FLAGS: u32 = 0xff8000 - 0x5f9c;

    pub fn run(x: &mut Cx) {
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT | st::DEAD => free(x),
            _ => {}
        }
    }

    fn init(x: &mut Cx) {
        let variant = x.u8(0x3);
        if variant > 1 {
            return;
        }
        x.add8(f::STATE, 2);
        x.set_bit(0x1, 1, true);
        x.set8(f::BOX_SET, 0x47);
        lib::set_immunities(x);
        x.anim_start(ANIMS, (variant as u32) as u16);
    }

    fn active(x: &mut Cx) {
        if paused(x) {
            return;
        }
        match x.u8(f::SUB) {
            0 => wait_for_player(x),
            2 => appear(x),
            4 => stay(x),
            6 => disappear(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Un joueur vivant est-il à portée ? (D0 = 1 si oui)
    fn player_near(x: &mut Cx) -> bool {
        let range: u16 = if x.u8(0x3) == 0 { 0x30 } else { 0x50 };
        for i in 0..4u32 {
            let player = A5 + 0x5de + 0x200 * i;
            x.c.a[2] = player;
            if x.data8(player) == 1 {
                if lib::in_box(x, Obj(player), range as i16, 0x14, 0xffe8).found() {
                    x.c.d[0] = 1;
                    return true;
                }
            }
        }
        x.c.d[0] = 0;
        false
    }

    fn mark_visible(x: &mut Cx, on: bool) {
        let bit = x.u8(0x3) != 0;
        let v = x.data8(VISIBLE_FLAGS);
        let mask = if bit { 1 } else { 2 };
        wr8(x.b, VISIBLE_FLAGS, if on { v | mask } else { v & !mask });
    }

    fn wait_for_player(x: &mut Cx) {
        if !player_near(x) {
            return;
        }
        lib::sound(x, 0xe5632);
        mark_visible(x, true);
        x.add8(f::SUB, 2);
        x.set_bit(0x1, 1, false);
        x.set8(0x86, 1);
    }

    fn appear(x: &mut Cx) {
        if x.anim_step() {
            x.add8(f::SUB, 2);
        }
    }

    fn stay(x: &mut Cx) {
        if player_near(x) {
            return;
        }
        x.add8(f::SUB, 2);
        lib::sound(x, 0xe5632);
        mark_visible(x, false);
        let n = 2 + (x.u8(0x3) != 0) as u32;
        x.anim_start(ANIMS, n as u16);
    }

    fn disappear(x: &mut Cx) {
        if !x.anim_step() {
            return;
        }
        x.set8(0x86, 0);
        x.add8(f::SUB, 0xfa);
        x.set_bit(0x1, 1, true);
        let n = (x.u8(0x3) != 0) as u32;
        x.anim_start(ANIMS, n as u16);
    }
}

handlers! {
    0x74fc0 "enemy_20" man_scorpion::run,
    0x79300 "enemy_28" nagpa::run,
}
