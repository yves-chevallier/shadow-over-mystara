//! Effets des joueurs (PEFF), deuxième série : types 11, 20, 33. Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::pools;

const ANIM_START: u32 = 0x1a32;
const REINSERT: u32 = 0x1ee1a;
const RNG: u32 = 0x19a88;

/// Tirage aléatoire du jeu (`0x19a88`) : D0 en sortie.
fn rand(x: &mut Cx) -> u32 {
    x.call(RNG);
    x.c.d[0]
}

/// Type 11 (0x4b93c) : averse de grêlons. Le parent (variante 0) compte 0x70 images en créant, toutes les
/// 8 images, deux grêlons (variante 1, sens 0 et 1) ; chaque grêlon tourne autour de sa position
/// de départ en suivant une table de vitesses (`+0x4c`, choisie par `+0x58`) avant de retomber.
mod hail {
    use super::*;

    const SPEED_TABLES: u32 = 0x4baf6;
    const ANIMS: u32 = 0x1b609a;
    const KIND: u8 = 0xb;
    /// Durée d'attente avant la chute : `+0x58` (octet), tour de la table : `+0x59`.
    const TIMER: u32 = 0x58;
    const PHASE: u32 = 0x59;
    const PARENT: u32 = 0x5c;
    const ANGLE: u32 = 0x2d;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4b94a) {
            0 if !child => {
                x.add8(0x4, 2);
                x.set8(TIMER, 0x70);
                x.set8(PHASE, 1);
                parent_tick(x);
            }
            0 => child_init(x),
            2 if !child => parent_tick(x),
            2 => child_active(x),
            4 if !child => x.add8(0x4, 2),
            4 => child_fall(x),
            _ => x.free_peff(),
        }
    }

    /// Une image du parent : au bout de 0x70 images il cesse de créer des grêlons (`+0x5a`).
    fn parent_tick(x: &mut Cx) {
        x.add8(TIMER, 0xff);
        if x.u8(TIMER) == 0 {
            x.add8(0x4, 2);
            x.set8(0x5a, 1);
        }
        x.add8(PHASE, 0xff);
        let phase = x.u8(PHASE) & 7;
        x.set8(PHASE, phase);
        if phase == 0 {
            spawn_pair(x);
        }
    }

    /// Deux grêlons liés au parent (`+0x5c`) et au joueur.
    fn spawn_pair(x: &mut Cx) {
        for side in 0..2 {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, KIND);
            o.set_u8(x.b, 3, 1);
            o.set_u8(x.b, 0x2f, side);
            o.set_u16(x.b, PARENT, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
        }
    }

    fn child_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        for off in [0x8, 0x10] {
            let v = owner.u16(x.b, off);
            x.set16(off, v);
        }
        let mut dx = 0x10u16;
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_neg();
        }
        x.set_d0w(dx);
        x.add16(0x8, dx);
        x.set16(0xc, 0x10);
        x.set8(TIMER, 0);
        x.set8(ANGLE, 0x10);
        select_table(x);
        place(x);
        let r = x.rand_and(3);
        x.set8(PHASE, r as u8);
        x.c.a[4] = ANIMS;
        x.call(ANIM_START);
    }

    /// `0x4bae6` : table de vitesses d'après `+0x58`.
    fn select_table(x: &mut Cx) {
        let index = x.u8(TIMER) as u32;
        x.c.d[0] = index;
        let table = x.rom32(SPEED_TABLES + index);
        x.c.a[4] = table;
        x.set32(0x4c, table);
    }

    /// `0x4bb1a` : vitesses de la rangée courante, puis position autour du joueur et déplacement.
    fn place(x: &mut Cx) {
        let row = (x.u8(ANGLE) as u32) << 2 & 0xffff;
        x.c.d[0] = row;
        let table = x.u32(0x4c);
        x.c.a[4] = table;
        let vx = x.data16(table + row);
        let vy = x.data16(table + row + 2);
        x.set16(0x14, vx);
        x.set16(0x1c, vy);
        x.set16(0x16, 0);
        x.set16(0x1e, 0);
        let owner = x.link_a1(f::OWNER);
        for off in [0x8, 0x10] {
            let v = owner.u16(x.b, off);
            x.set16(off, v);
        }
        let mut dx = 0x10u16;
        if x.u8(f::FLIP) == 0 {
            dx = dx.wrapping_neg();
        }
        x.set_d0w(dx);
        x.add16(0x8, dx);
        x.add32(0xc, 0x10000);
        x.call(MOVE_XY);
    }

    fn child_active(x: &mut Cx) {
        let parent = x.link_a1(PARENT);
        if parent.u8(x.b, 0x5a) == 0 {
            x.call(ANIM_STEP);
            let step: u8 = if x.u8(0x2f) != 0 { 0xfe } else { 2 };
            x.c.d[0] = x.c.d[0] & !0xff | step as u32;
            let angle = x.u8(ANGLE).wrapping_add(step) & 0x1f;
            x.set8(ANGLE, angle);
            place(x);
            if x.u16(0xe) == 0 && x.u16(0xc) & 0xf == 0 {
                x.add8(TIMER, 4);
                select_table(x);
            }
            if x.u32(0xc) == 0x300000 {
                let r = x.rand_and(3) + 4;
                x.set8(PHASE, r as u8);
                x.set_d0w(r as u16);
                x.c.a[4] = ANIMS;
                x.call(ANIM_START);
            }
            if x.u16(0xc) <= 0x58 {
                return x.call(REINSERT);
            }
        }
        // fin de la chute : le grêlon éclate
        x.add8(0x4, 2);
        x.set8(TIMER, 0x20);
        select_table(x);
        x.c.d[0] = 9;
        x.c.a[4] = ANIMS;
        x.call(ANIM_START);
        x.call(REINSERT);
    }

    fn child_fall(x: &mut Cx) {
        x.call(ANIM_STEP);
        if x.c.cond(5) {
            x.add8(0x4, 2);
        }
        let row = (x.u8(ANGLE) as u32) << 2 & 0xffff;
        x.c.d[0] = row;
        let table = x.u32(0x4c);
        x.c.a[4] = table;
        let vx = x.data16(table + row);
        let vy = x.data16(table + row + 2);
        x.set16(0x14, vx);
        x.set16(0x1c, vy);
        x.set16(0x16, 0);
        x.set16(0x1e, 0);
        x.add32(0xc, 0x8000);
        x.call(MOVE_XY);
        x.call(REINSERT);
    }
}

/// Type 33 (0x4f14c) : brindille projetée depuis un bord de l'écran (variante `+3` : bit 0 = côté), qui
/// vole `+0x58` images puis, si le jeu n'est pas en pause, crée une trace (type 33 de variante + 1).
mod spark {
    use super::*;

    const THROWS: u32 = 0x4f1f6;
    const ANIMS: u32 = 0x1bc298;

    pub fn run(x: &mut Cx) {
        match x.state(0x4f15a) {
            0 => init(x),
            2 => flight(x),
            4 => {
                x.call(ANIM_STEP);
                x.call(MOVE_XZ);
                if x.u8(0x1) & 1 == 0 {
                    x.free_peff();
                }
            }
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x58, 1);
        let cam = if x.a5_u8(a5::USE_CAM2) != 0 { 0x54e } else { 0x4fe };
        x.c.a[6] = 0xff8000 + cam;
        let right = x.u8(0x3) & 1 != 0;
        let shift = if right { 0 } else { 0x180 };
        x.c.d[0] = shift;
        let cam_x = x.a5_u16(cam + 8);
        let cam_y = x.a5_u16(cam + 0xc);
        x.set16(0x8, cam_x);
        x.set16(0x10, cam_y);
        x.add16(0x8, shift as u16);
        x.set16(0xc, 0xc0);
        let r = x.rand_and(0x7f);
        x.add16(0xc, r as u16);
        let row = x.rand_and(0x18);
        x.c.d[0] = row;
        let (vx, vz, az) = (x.rom16(THROWS + row), x.rom16(THROWS + row + 2), x.rom16(THROWS + row + 4));
        x.set16(0x14, vx);
        x.set16(0x18, vz);
        x.set16(0x1a, az);
        if !right {
            x.set16(0x14, vx.wrapping_neg());
        }
        x.rand_and(1);
        x.c.a[4] = ANIMS;
        x.call(ANIM_START);
        x.call(REINSERT);
    }

    fn flight(x: &mut Cx) {
        x.call(ANIM_STEP);
        x.call(MOVE_XZ);
        let t = (x.u16(0x58) as i16).wrapping_sub(1);
        x.set16(0x58, t as u16);
        if t >= 0 {
            return;
        }
        if x.a5_u8(0x3d) != 0 {
            if let Some(o) = x.alloc(&pools::PEFF) {
                o.set_u8(x.b, 0, 1);
                o.set_u8(x.b, 2, 0x21);
                let v = x.u8(0x3).wrapping_add(1);
                o.set_u8(x.b, 3, v);
            }
        }
        x.add8(0x4, 2);
    }
}

/// Type 20 (0x4d08c) : la sorcière. Le parent (variante 0) naît dans un SET, suit le tourbillon (état 2) et
/// crée à chaque image un éclat (variante 1) ; l'éclat vole en arc depuis le bord de l'écran.
mod witch {
    use super::*;

    const ANIMS: u32 = 0x1bb058;
    const SPREAD: u32 = 0x4d1bc;
    const KIND: u8 = 0x14;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4d09a) {
            0 if !child => parent_init(x),
            0 => child_init(x),
            2 if !child => parent_active(x),
            2 => child_active(x),
            _ => x.free_peff(),
        }
    }

    fn parent_init(x: &mut Cx) {
        let Some(set) = x.alloc(&pools::SET) else { return };
        set.set_u8(x.b, 0, 1);
        set.set_u8(x.b, 2, 0);
        set.set_u8(x.b, 3, 0x4e);
        set.set_u8(x.b, 0x2f, 2);
        x.add8(0x4, 2);
        x.set8(0x5c, 0);
        x.set8(0x5e, 0);
        x.set8(0x5b, 0x1e);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.copy_pos(owner);
        x.add16(0xc, 0x60);
        x.add16(0x10, 1);
        x.c.d[0] = 0;
        x.c.a[4] = ANIMS;
        x.call(ANIM_START);
        x.call(0x1ac7c);
        x.call(REINSERT);
    }

    fn child_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(0x14, -0x400i16 as u16);
        x.set16(0x18, -0x700i16 as u16);
        let cam = if x.a5_u8(a5::USE_CAM2) != 0 { (a5::CAM2_X, a5::CAM2_Y) } else { (a5::CAM_X, a5::CAM_Y) };
        let (cx, cy) = (x.a5_u16(cam.0), x.a5_u16(cam.1));
        x.set16(0x8, cx);
        x.set16(0x10, cy);
        x.add16(0x10, 0x18);
        x.set16(0xc, 0xe8);
        let r = x.rand_and(0x1f);
        x.add16(0xc, r as u16);
        let mut d2 = (x.u8(0x5e) as u32 & 0xf) * 2;
        if owner.u8(x.b, f::FLIP) != 0 {
            let vx = x.u16(0x14);
            x.set16(0x14, vx.wrapping_neg());
            d2 += 0x20;
        }
        let base = x.rom16(SPREAD + d2);
        x.c.d[2] = base as u32;
        let jitter = x.rand_and(3);
        let dx = jitter as u16 + base;
        x.add16(0x8, dx);
        let anim = x.rand_and(1) + 1;
        x.set_d0w(anim as u16);
        x.c.a[4] = ANIMS;
        x.call(ANIM_START);
        x.call(REINSERT);
    }

    fn child_active(x: &mut Cx) {
        x.call(MOVE_XZ);
        if (x.u16(0xc) as i16) >= 0 {
            return;
        }
        x.add8(0x4, 2);
    }

    /// Crée un éclat (variante 1) qui garde le rang `+0x5e` de ce parent ; renvoie vrai si le pool est plein.
    fn spawn_spark(x: &mut Cx) -> bool {
        match x.alloc(&pools::PEFF) {
            None => {
                x.c.d[0] = 1;
                true
            }
            Some(o) => {
                o.set_u8(x.b, 0, 1);
                o.set_u8(x.b, 2, KIND);
                o.set_u8(x.b, 3, 1);
                let rank = x.u8(0x5e);
                o.set_u8(x.b, 0x5e, rank);
                let owner = x.u16(f::OWNER);
                o.set_u16(x.b, f::OWNER, owner);
                x.c.d[0] = 0;
                false
            }
        }
    }

    fn parent_active(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if owner.u8(x.b, 0x7) == 0xe {
            x.add8(0x4, 2);
            x.set_a5_u16(0x4ba, 0);
        } else {
            if x.u8(0x33) != 0 {
                x.set8(0x33, 0);
                x.set8(0x5d, 1);
            }
            if x.u8(0x5d) != 0 {
                x.add16(0x8, 2);
                x.add16(0xc, 1);
                if x.u16(0x58) & 1 == 0 {
                    x.add16(0x8, 0xfffc);
                    x.add16(0xc, 0xfffe);
                }
            }
            x.add16(0x58, 1);
            let t = x.u16(0x58);
            x.c.d[0] = t as u32;
            if t > 0x140 {
                x.set_bit(0x1, 1, true);
                if t & 1 == 0 {
                    x.set_bit(0x1, 1, false);
                }
            }
            x.add8(0x5b, 0xff);
            if x.u8(0x5b) == 0 {
                x.set8(0x5b, 2);
                x.set_a5_u16(0x4ba, 0x100);
                let v = x.u8(0x5c) ^ 1;
                x.set8(0x5c, v);
                if v == 0 {
                    x.set8(0x5b, 0x60);
                    x.set_a5_u16(0x4ba, 0);
                }
            }
        }
        if x.u8(0x5d) != 0 && !spawn_spark(x) {
            x.add8(0x5e, 1);
        }
        x.call(ANIM_STEP);
    }
}

handlers! {
    0x4b93c "peff_11" hail::run,
    0x4d08c "peff_20" witch::run,
    0x4f14c "peff_33" spark::run,
}
