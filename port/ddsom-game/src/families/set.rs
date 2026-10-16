//! Objets de décor (classe SET, pool de 32 objets de 0x50 octets, table 0x39eac) : types 0, 1 et
//! 3 à 14. Les objets SET ne figurent pas dans les listes triées (voir `docs/objets.md`).

use super::cx::{handlers, sub, Cx};
use crate::objects::Obj;
use m68k::SR_C;

/// Type 8 (0xcc25a), partagé par les tables SET (8), SOBJ (4) et EFFCT (18) : générateur du
/// contrôleur de scène. À la première image il attend ; à la suivante il crée un objet SET de
/// type 2 (le contrôleur, de variante `+0x2f`) puis passe à l'état 6. Dans les états 4 et 6 il se
/// libère dans le pool de sa classe (EFFCT, SET ou SOBJ selon `+0x28`).
mod scene_spawner {
    use super::*;

    /// Table (espace programme) des routines de libération, indexée par `(classe - 0xe) * 2` octets.
    const FREE_BY_CLASS: u32 = 0xcc2b8;
    const WAIT: u8 = 0;
    const SPAWN: u8 = 2;
    const SCENE_CONTROLLER: u8 = 2;
    /// Paramètre du créateur : variante du contrôleur à créer.
    const VARIANT_PARAM: u32 = 0x2f;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            WAIT => x.add8(0x4, 2),
            SPAWN => {
                x.call(sub::ALLOC_SET);
                if x.c.sr & SR_C != 0 {
                    return;
                }
                let child = Obj(x.c.a[1]);
                child.set_u8(x.b, 0, 1);
                child.set_u8(x.b, 2, SCENE_CONTROLLER);
                let variant = x.u8(VARIANT_PARAM);
                child.set_u8(x.b, 3, variant);
                x.set8(0x4, 6);
            }
            _ => {
                let class = x.u8(0x28) as u32;
                let offset = (class.wrapping_sub(0xe) as u16).wrapping_mul(2) as i16;
                let at = FREE_BY_CLASS.wrapping_add(offset as i32 as u32);
                let free = (x.rom16(at) as u32) << 16 | x.rom16(at + 2) as u32;
                x.call(free);
            }
        }
    }
}

handlers! {
    0xcc25a "set_commun_cc25a" 120 scene_spawner::run,
}
