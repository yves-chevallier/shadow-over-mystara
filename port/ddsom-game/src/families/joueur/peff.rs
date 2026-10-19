//! Effets des joueurs (classe PEFF, pool de 48 objets de 0x60 octets, table 0x39992), première série.
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;

/// Type 44 (0x517dc) : pastille posée sur la caméra secondaire à un décalage de colonne (`+3`), animée
/// par le jeu `0x1bcbbe` (ou `0x1bcd5a` en palette alternative) ; elle sort quand elle quitte l'écran par le bas.
mod badge {
    use super::*;

    /// Décalages de colonne par variante (mots, ROM programme).
    const COLUMNS: u32 = 0x5184e;
    /// Octets d'animation par `+0x2f` (RAM).
    const ANIM_BYTES: u32 = 0xff2478;
    const ANIMS: u32 = 0x1bcbbe;
    const ANIMS_ALT: u32 = 0x1bcd5a;

    pub fn run(x: &mut Cx) {
        match x.state(0x517ea) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let variant = x.u8(0x3) as u32;
        let column = x.rom16(COLUMNS + 2 * variant);
        let cam_x = x.a5_u16(a5::CAM2_X);
        x.c.d[0] = x.u8(0x3) as u32 * 2;
        x.c.d[1] = column.wrapping_add(cam_x) as u32;
        x.set16(0x8, x.c.d[1] as u16);
        let cam_y = x.a5_u16(a5::CAM2_Y);
        x.set16(0xc, cam_y);
        x.set32(0x10, 0);
        x.c.a[4] = ANIM_BYTES;
        let which = x.u8(0x2f) as u32;
        let byte = x.data8(ANIM_BYTES + which);
        x.c.d[0] = which;
        x.c.d[1] = byte as u32;
        x.set8(0x42, byte);
        let n = (which * 2 + byte as u32) & 0xffff;
        x.c.d[0] = n;
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ANIMS_ALT } else { ANIMS };
        x.c.a[4] = set;
        x.call(0x1a32);
        x.reinsert();
    }

    fn active(x: &mut Cx) {
        let d = x.u16(0xc).wrapping_sub(x.a5_u16(a5::CAM2_Y));
        x.set_d0w(d);
        if d >= 0x110 {
            x.free_peff();
        }
    }
}

handlers! {
    0x517dc "peff_44" 80 badge::run,
}
