//! Effets des joueurs (PEFF), quinzième série : type 24 (le tas d'or de l'interface qui reprend les objets
//! du joueur contre des points). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use crate::mem::{rd32, wr32, wr8};
use super::pshell::*;
use crate::objects::Obj;

const REINSERT: u32 = 0x1ee1a;
const ANIMS: u32 = 0x1c40c6;
/// Position de l'icône par joueur (X, Y) : deux versions selon `A5+0x20` (1 : écran à un joueur).
const PLAYER_SPOTS_A: u32 = 0x4d792;
const PLAYER_SPOTS_B: u32 = 0x4d782;
/// Tables par personnage (pointeurs) donnant, pour chaque équipement, 0 (rien), 1 (vendu `+0x148` fois),
/// ≥ 2 (vendu une fois) ou négatif (échangé contre rien).
const SELL_TABLES: u32 = 0x4d884;
/// Prix en points (BCD sur 3 octets, 4 octets par équipement).
const PRICES: u32 = 0x4db30;

/// Addition décimale d'un octet (ABCD) avec retenue entrante : résultat et retenue sortante.
fn bcd_add(a: u8, b: u8, carry: bool) -> (u8, bool) {
    let mut r = a as u16 + b as u16 + carry as u16;
    if (a & 0xf) + (b & 0xf) + carry as u8 > 9 {
        r += 6;
    }
    let out = r > 0x99;
    if out {
        r += 0x60;
    }
    (r as u8, out)
}

/// Le tas : posé dans l'angle de l'écran du joueur, il s'anime quand un joueur qui porte un équipement
/// (rang `+3` de la rangée) s'en approche et qu'il appuie sur le bouton (`+0x17`) ; l'équipement est repris
/// et son prix ajouté au score (plafonné à 999999).
mod coins {
    use super::*;

    pub fn run(x: &mut Cx) {
        match x.state(0x4d726) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x16, 0);
                let table = if x.a5_u8(0x20) == 1 { PLAYER_SPOTS_A } else { PLAYER_SPOTS_B };
                let (cx, cy) = (x.a5_u16(a5::CAM_X), x.a5_u16(a5::CAM_Y));
                let owner = x.link_a1(f::OWNER);
                let row = (owner.u8(x.b, 0x2) as u32) << 2;
                x.c.d[0] = row;
                let dx = x.data16(table + row);
                let dy = x.data16(table + row + 2);
                x.set16(0x8, cx.wrapping_add(dx));
                x.set16(0x10, cy.wrapping_add(dy));
                x.anim(ANIMS, 0x2b);
                x.call(REINSERT);
            }
            2 => {
                check(x);
                let delta = x.a5_u32(a5::CAM_X).wrapping_sub(x.a5_u32(0x524));
                x.c.d[0] = delta;
                x.add32(0x8, delta);
            }
            _ => x.call(0x2218),
        }
    }

    /// `0x4d7ac`
    fn check(x: &mut Cx) {
        let linked = x.link_a1(0x5c);
        let flags = x.u8(0x17);
        let had = flags & 1 != 0;
        x.set8(0x17, flags & !1);
        if had {
            x.set16(0x1a, 0);
        }
        let d0 = linked.u16(x.b, 0x5c);
        if d0 == 0 && x.u16(0x1a) != 0 {
            x.set8(0x17, 1);
        } else {
            x.set16(0x1a, d0);
        }
        let dx = linked.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
        if dx > 0x18 {
            return away(x);
        }
        let dy = x.u16(0x10).wrapping_sub(linked.u16(x.b, 0x10)).wrapping_sub(linked.u16(x.b, 0xc));
        if dy > 0x20 {
            return away(x);
        }
        if x.u8(0x16) == 0 {
            x.set8(0x16, 1);
            return x.anim(ANIMS, 0x2c);
        }
        if x.u8(0x17) == 0 {
            return;
        }
        sell(x);
    }

    fn away(x: &mut Cx) {
        if x.u8(0x16) == 0 {
            return;
        }
        x.set8(0x16, 0);
        x.anim(ANIMS, 0x2b);
    }

    /// `0x4d81c` : le joueur du lien (`+0x1a`) rend l'équipement de son emplacement `+3`.
    fn sell(x: &mut Cx) {
        let who = Obj(x.u16(0x1a) as i16 as i32 as u32 & 0xff_ffff);
        x.c.a[6] = x.u16(0x1a) as i16 as i32 as u32;
        let player_word = who.u16(x.b, f::OWNER);
        let player = Obj(player_word as i16 as i32 as u32 & 0xff_ffff);
        x.c.a[1] = player_word as i16 as i32 as u32;
        let rank = who.u8(x.b, 0x3) as u32;
        let slot = player.0 + rank;
        x.c.a[2] = (player_word as i16 as i32 as u32).wrapping_add(rank);
        let character = (player.u8(x.b, 0x3) & 0xe) as u32;
        let table = x.rom32(SELL_TABLES + character * 2);
        x.c.a[3] = table;
        let id = crate::mem::rd8(x.b, slot + 0x140) as u32;
        let code = x.data8(table + id);
        x.c.d[0] = code as u32;
        if code == 0 {
            return;
        }
        let repeats: u16;
        if code == 1 {
            repeats = (crate::mem::rd8(x.b, slot + 0x148) as u16).wrapping_sub(1);
        } else if (code as i8) > 1 {
            repeats = 0;
        } else {
            wr8(x.b, slot + 0x140, 0x56);
            wr8(x.b, slot + 0x148, 0);
            return;
        }
        let price = PRICES + (id << 2);
        let mut left = repeats;
        loop {
            // 3 octets décimaux de `price+1..price+3` ajoutés à `(0x115..0x117, joueur)`
            let mut carry = false;
            for k in (1..=3u32).rev() {
                let add = x.data8(price + k);
                let sum = crate::mem::rd8(x.b, player.0 + 0x114 + k);
                let (r, c) = bcd_add(sum, add, carry);
                wr8(x.b, player.0 + 0x114 + k, r);
                carry = c;
            }
            if (rd32(x.b, player.0 + 0x114) as i32) > 0x99_9999 {
                wr32(x.b, player.0 + 0x114, 0x99_9999);
            }
            let again = left != 0;
            left = left.wrapping_sub(1);
            if !again {
                break;
            }
        }
        wr8(x.b, slot + 0x140, 0x56);
        wr8(x.b, slot + 0x148, 0);
    }
}

handlers! {
    0x4d718 "peff_24" 150 coins::run,
}
