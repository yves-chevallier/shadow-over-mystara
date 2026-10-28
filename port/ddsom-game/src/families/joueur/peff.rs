//! Effets des joueurs (classe PEFF, pool de 48 objets de 0x60 octets, table 0x39992), première série.
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::Banc;
use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use super::peff_boutique;
use crate::mem::rd16;
use crate::objects::Obj;
use crate::objects::Courant;

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
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init<R: Banc>(x: &mut Cx<'_, R>) {
        x.add8(0x4, 2);
        let variant = x.u8(0x3) as u32;
        let column = x.rom16(COLUMNS + 2 * variant);
        let cam_x = x.a5_u16(a5::CAM2_X);
        x.set16(0x8, column.wrapping_add(cam_x));
        let cam_y = x.a5_u16(a5::CAM2_Y);
        x.set16(0xc, cam_y);
        x.set32(0x10, 0);
        let which = x.u8(0x2f) as u32;
        let byte = x.data8(ANIM_BYTES + which);
        x.set8(0x42, byte);
        let n = (which * 2 + byte as u32) & 0xffff;
        let set = if x.a5_u16(a5::ALT_PALETTE) != 0 { ANIMS_ALT } else { ANIMS };
        x.anim_start(set, n as u16);
        x.reinsert();
    }

    fn active(x: &mut Cx) {
        let d = x.u16(0xc).wrapping_sub(x.a5_u16(a5::CAM2_Y));
        if d >= 0x110 {
            x.free_peff();
        }
    }
}

/// Type 43 (0x51076) : comptoir de boutique (scènes 0x45 et suivantes). Deux zones, posées d'après la scène,
/// guettent les quatre joueurs ; un joueur dans une zone avec `+0x58` positif lance le script de dialogue de
/// la zone (pointeurs `+0x1c` et `+0x58`, interprète 0x514ac / 0x514cc : `peff_boutique.rs`). L'objet suit
/// le défilement de l'écran à demi-vitesse.
mod shop {
    use super::*;

    /// Par scène (depuis 0x45) : (décalage, largeur) en X et en profondeur des deux zones (mots, en clair).
    const ZONES_A: u32 = 0x51306;
    const ZONES_B: u32 = 0x51376;
    /// Par scène : script de dialogue de chaque zone (longs, ROM programme).
    const SCRIPTS_A: u32 = 0x51106;
    const SCRIPTS_B: u32 = 0x5113e;
    const ANIMS: u32 = 0x1c03a6;
    const FIRST_SCENE: u16 = 0x45;
    /// Quatre joueurs enregistrés (mots, 0 = absent).
    const PLAYERS: u32 = 0xff2aa4;
    /// Position du défilement et sa valeur à l'image précédente (longs, A5).
    const SCROLL: [u32; 2] = [0x506, 0x50a];
    const PREVIOUS_SCROLL: [u32; 2] = [0x524, 0x528];
    /// Zone A : `+0x14` décalage X, `+0x16` largeur, `+0x18` décalage en profondeur, `+0x1a` étendue.
    const ZONE_A: u32 = 0x14;
    const ZONE_B: u32 = 0x50;
    /// Octet du joueur : positif quand il peut parler au marchand.
    const PLAYER_READY: u32 = 0x58;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        let scene = x.a5_u8(0x34);
        let row = (scene.wrapping_sub(FIRST_SCENE as u8) as u32) << 3;
        for (table, field) in [(ZONES_A, ZONE_A), (ZONES_B, ZONE_B)] {
            for k in (0..8).step_by(2) {
                let v = x.data16(table + row + k);
                x.set16(field + k, v);
            }
        }
        let index = (scene as u16).wrapping_sub(FIRST_SCENE).wrapping_mul(4) as i16 as u32;
        let a = x.rom32(SCRIPTS_A.wrapping_add(index));
        x.set32(0x1c, a);
        let b = x.rom32(SCRIPTS_B.wrapping_add(index));
        x.set32(0x58, b);
        let variant = x.u8(0x3) as u32;
        x.anim(ANIMS, variant);
        x.reinsert();
    }

    /// Deux tours des quatre joueurs (D6 compteur, A1 pointeur dans la liste, comme l'original : l'interprète du
    /// dialogue, appelé avec A2 = le joueur prêt dans la zone, sauve tous les registres dans ses appels de son), puis
    /// suivi du défilement et pas d'animation.
    fn active(x: &mut Cx) {
        let zones: [(u32, fn(&mut Cx)); 2] = [(ZONE_A, peff_boutique::talk_a), (ZONE_B, peff_boutique::talk_b)];
        for (zone, talk) in zones {
            x.c.d[6] = 3;
            x.c.a[1] = PLAYERS;
            loop {
                if let Some(player) = in_zone(x, zone) {
                    if (player.u8(x.b, PLAYER_READY) as i8) > 0 {
                        talk(x);
                    }
                }
                x.c.a[1] = x.c.a[1].wrapping_add(2);
                let n = (x.c.d[6] as u16).wrapping_sub(1);
                x.c.d[6] = x.c.d[6] & 0xffff_0000 | n as u32;
                if n == 0xffff {
                    break;
                }
            }
        }
        let [dx, dz] = [0, 1].map(|k| {
            (x.a5_u32(SCROLL[k]).wrapping_sub(x.a5_u32(PREVIOUS_SCROLL[k])) as i32 >> 1) as u32
        });
        x.add32(0x8, dx);
        x.add32(0xc, dz);
        x.anim_step();
    }

    /// Le joueur de la case A1, s'il est dans la zone (`D0` et `A2` laissés comme l'original).
    fn in_zone(x: &mut Cx, zone: u32) -> Option<Obj> {
        let w = rd16(x.b, x.c.a[1]);
        x.set_d0w(w);
        if w == 0 {
            return None;
        }
        x.c.a[2] = w as i16 as i32 as u32;
        let p = Obj(x.c.a[2] & 0xff_ffff);
        let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(x.u16(zone));
        x.set_d0w(dx);
        if dx > x.u16(zone + 2) {
            return None;
        }
        let dz = x.u16(0xc).wrapping_add(x.u16(0x10)).wrapping_sub(p.u16(x.b, 0xc)).wrapping_sub(p.u16(x.b, 0x10));
        let dz = dz.wrapping_add(x.u16(zone + 4));
        x.set_d0w(dz);
        (dz <= x.u16(zone + 6)).then_some(p)
    }
}

handlers! {
    0x51076 "peff_43" shop::run,
    0x517dc "peff_44" badge::run,
}
