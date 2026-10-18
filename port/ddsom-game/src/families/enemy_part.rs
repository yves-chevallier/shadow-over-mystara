//! Parties d'ennemis composites (ENEMY type 7). Voir `docs/familles/enemy.md`.
//!
//! Une partie est un ennemi qui n'a pas de logique de combat propre : il se colle à son hôte (mot
//! `+0x46`), recopie sa position avec un décalage propre à sa variante, et joue les animations que
//! l'état de l'hôte (`+0xa2`) lui commande. Il prévient l'hôte de la fin de ses gestes (`+0xa3` de
//! l'hôte).

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

// ---------------------------------------------------------------------------------------------
// Type 7 (0x63b2a) : « Wyvern » (h), tentacule d'un Displacer Beast
// ---------------------------------------------------------------------------------------------

/// Tentacule de l'hôte (un ennemi de type 19, ou 14 pour un autre jeu d'animations) ; variante 0 =
/// tentacule qui attaque, variante 1 = tentacule d'accompagnement. Sous-états (`+5`) choisis d'après
/// l'octet `+0xa2` de l'hôte par une table de la ROM (0x63fa8) : 0 attente, 2 début d'attaque,
/// 4 coup, 6 retour, 8 coup spécial, 10 recul. Tant que l'hôte n'est plus une adresse de RAM valide ou
/// est libéré, la partie passe à l'état 6 (libération).
mod tentacle {
    use super::*;

    /// Hôte (mot `+0x46`) : variables lues.
    const HOST_STATE: u32 = 0xa2;
    const HOST_DONE: u32 = 0xa3;
    const HOST_LAST_HIT: u32 = 0x3e;
    /// Décalage (X, Y, fraction de Y, Z) par variante, 8 octets par ligne.
    const OFFSETS: u32 = 0x63f34;
    /// Sous-état d'après l'état de l'hôte (octets en espace de données).
    const SUB_FOR_HOST: u32 = 0x63fa8;
    /// Durées d'attente de l'animation de repos (mots, indexées par un tirage pair).
    const IDLE_DELAYS: u32 = 0x63c7e;
    /// Jeux d'animations par type d'hôte : (autre hôte, Displacer Beast).
    const BODY: (u32, u32) = (0x17dbb0, 0x1824d4);
    const ALT: (u32, u32) = (0x17dbd2, 0x1824f6);
    const MID: (u32, u32) = (0x17dbba, 0x1824de);
    const DISPLACER_BEAST: u8 = 0x13;
    const TROLL: u8 = 0xe;
    /// Sons et effets.
    const SFX_STRIKE: u32 = 0xe4e5a;
    const SFX_TROLL_STRIKE: u32 = 0xe50fe;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit(x),
            st::DEAD => free(x),
            _ => {}
        }
    }

    /// Hôte vivant, ou `None` : alors la partie passe à l'état 6 (`0x63e96`).
    fn host(x: &mut Cx) -> Option<Obj> {
        let h = host_ptr(x)?;
        if h.u8(x.b, 0) == 0 {
            x.set8(f::STATE, st::DEAD);
            return None;
        }
        Some(h)
    }

    /// Hôte désigné par `+0x46`, sans vérifier qu'il est actif (`A1` en garde l'adresse étendue).
    fn host_ptr(x: &mut Cx) -> Option<Obj> {
        let w = x.u16(f::HOST);
        if (w as i16) >= 0 {
            x.set8(f::STATE, st::DEAD);
            return None;
        }
        x.c.a[1] = w as i16 as i32 as u32;
        Some(Obj(x.c.a[1] & 0xff_ffff))
    }

    /// L'hôte de la dernière lecture de `A1` (registre conservé par les animations).
    fn a1(x: &mut Cx) -> Obj {
        Obj(x.c.a[1] & 0xff_ffff)
    }

    // --- état 0 : création ---------------------------------------------------------------------

    fn init(x: &mut Cx) {
        x.add8(f::STATE, 2);
        let Some(h) = host(x) else { return };
        if h.u8(x.b, 2) == TROLL {
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x1c);
        }
        x.set8(f::BOX_SET, 0x2e);
        x.set8(f::SUB, 0);
        x.set16(f::SUB2, 0);
        follow(x);
        x.call(rom::IMMUNITIES);
        let n = if x.u8(0x3) != 0 { 3 } else { 2 };
        start(x, BODY, n);
    }

    /// Se colle à l'hôte : position, orientation et drapeaux copiés, plus le décalage de la variante
    /// (X inversé quand l'hôte regarde à gauche).
    fn follow(x: &mut Cx) {
        let Some(h) = host_ptr(x) else { return };
        for off in [0x8, 0x10, 0xc] {
            let v = h.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = h.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let flags = h.u8(x.b, 1);
        x.set8(1, flags);
        let row = (x.u8(0x3) as u32) << 3;
        let mut dx = x.rom16(OFFSETS + row) as i16;
        let dy = x.rom16(OFFSETS + row + 2);
        let dy_frac = x.rom16(OFFSETS + row + 4) as i16;
        let dz = x.rom16(OFFSETS + row + 6);
        if flip != 0 {
            dx = dx.wrapping_neg();
        }
        x.add16(0x8, dx as u16);
        x.add16(0x10, dy);
        x.add32(0x10, dy_frac as i32 as u32);
        x.add16(0xc, dz);
    }

    /// Démarre l'animation `n` du jeu `sets` (celui du Displacer Beast si l'hôte en est un).
    fn start(x: &mut Cx, sets: (u32, u32), n: u32) {
        x.c.d[0] = n;
        let mut set = sets.0;
        let w = x.u16(f::HOST);
        if (w as i16) < 0 {
            x.c.a[1] = w as i16 as i32 as u32;
            if a1(x).u8(x.b, 2) == DISPLACER_BEAST {
                set = sets.1;
            }
        }
        x.c.a[4] = set;
        x.call(0x1a32);
    }

    // --- état 2 : actif ------------------------------------------------------------------------

    fn active(x: &mut Cx) {
        follow(x);
        if paused(x) {
            if x.u8(0x3) != 0 {
                return finish(x);
            }
            let l = x.u32(f::STATE) & !0xff;
            if l == 0x0204_0400 || l == 0x0208_0400 {
                x.set16(0xa4, 1);
            }
        }
        let Some(h) = host(x) else { return };
        let host_state = h.u8(x.b, HOST_STATE);
        if host_state != x.u8(HOST_STATE) {
            x.set8(HOST_STATE, host_state);
            if x.u8(0x3) == 0 {
                h.set_u8(x.b, HOST_DONE, 0);
            }
            let sub = x.data8(SUB_FOR_HOST + host_state as u32);
            x.set8(f::SUB, sub);
            x.set16(f::SUB2, 0);
        }
        match x.u8(f::SUB) {
            0 => idle(x),
            2 => step2(x),
            4 => step4(x),
            6 => step6(x),
            8 => step8(x),
            10 => step10(x),
            _ => {}
        }
        finish(x);
    }

    /// Fin commune : cachée quand le dernier coup de l'hôte est négatif, puis remise en liste.
    fn finish(x: &mut Cx) {
        let Some(h) = host(x) else { return };
        if (h.u8(x.b, HOST_LAST_HIT) as i8) < 0 {
            x.set_bit(0x1, 1, true);
        }
        x.reinsert();
    }

    /// Démarre l'animation de repos et tire la durée de l'attente (`+0x30`).
    fn idle_anim(x: &mut Cx) {
        start(x, BODY, 2);
        let r = random(x) & 0xe;
        let delay = x.rom16(IDLE_DELAYS + r);
        x.set16(0x30, delay);
    }

    /// Sous-état simple de la variante 1 : démarre l'animation 3 une fois.
    fn variant1_once(x: &mut Cx) {
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            start(x, BODY, 3);
        }
    }

    fn idle(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            return variant1_once(x);
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            idle_anim(x);
        }
        if x.anim_step() {
            return idle_anim(x);
        }
        if x.u8(0x33) == 0 {
            return;
        }
        x.set8(0x33, 0);
        x.c.a[1] = x.u16(f::HOST) as i16 as i32 as u32;
        if x.u8(2) == TROLL {
            x.call(SFX_TROLL_STRIKE);
        } else {
            x.call(SFX_STRIKE);
        }
    }

    fn step2(x: &mut Cx) {
        if x.u8(0x3) == 0 {
            if x.u8(f::SUB2) == 0 {
                x.add8(f::SUB2, 2);
                idle_anim(x);
            }
            return;
        }
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(0x44, 0x48);
                start(x, MID, 0);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                a1(x).set_u8(x.b, HOST_DONE, 1);
            }
            _ => {}
        }
    }

    fn step4(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            return variant1_once(x);
        }
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                start(x, MID, 1);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set16(0xa4, 0x168);
                start(x, MID, 2);
                if x.a5_u8(0x30) != 0 {
                    x.call(0xe525a);
                }
            }
            4 => {
                let n = (x.u16(0xa4) as i16).wrapping_sub(1);
                x.set16(0xa4, n as u16);
                if n <= 0 {
                    x.add8(f::SUB2, 2);
                    start(x, MID, 3);
                } else {
                    x.call(0x9ef20);
                }
            }
            6 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                a1(x).set_u8(x.b, HOST_DONE, 1);
            }
            8 => a1(x).set_u8(x.b, HOST_DONE, 1),
            _ => {}
        }
    }

    fn step6(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            return variant1_once(x);
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            start(x, MID, 8);
        }
    }

    fn step8(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            return variant1_once(x);
        }
        match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                start(x, MID, 9);
            }
            2 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set16(0xa4, 0x168);
                start(x, MID, 10);
                x.call(0x9eede);
                x.call(0xe4fb6);
                x.call(SFX_STRIKE);
            }
            4 => {
                let n = (x.u16(0xa4) as i16).wrapping_sub(1);
                x.set16(0xa4, n as u16);
                if n <= 0 {
                    x.add8(f::SUB2, 2);
                    start(x, MID, 11);
                }
            }
            6 => {
                if x.anim_step() {
                    x.add8(f::SUB2, 2);
                }
            }
            8 => a1(x).set_u8(x.b, HOST_DONE, 1),
            _ => {}
        }
    }

    fn step10(x: &mut Cx) {
        if x.u8(0x3) != 0 {
            return variant1_once(x);
        }
        if x.u8(f::SUB2) == 0 {
            x.add8(f::SUB2, 2);
            start(x, ALT, 0);
        } else {
            x.call(0x1a56);
        }
    }

    // --- état 4 : touché -----------------------------------------------------------------------

    /// L'hôte a été touché : la partie se synchronise sur ses PV (`+0x60/+0x62/+0x64` à 0x7fff).
    fn hit(x: &mut Cx) {
        follow(x);
        if x.u16(0x60) != x.u16(f::HP) {
            for off in [0x60, f::HP, 0x64] {
                x.set16(off, 0x7fff);
            }
        }
        x.set32(f::STATE, 0x0200_0000);
        finish(x);
    }
}

handlers! {
    0x63b2a "enemy_7" 600 tentacle::run,
}
