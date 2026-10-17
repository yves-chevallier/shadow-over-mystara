//! Objets spéciaux (classe SOBJ, pool de 16 objets de 0x50 octets, table 0x39fa8) : décors défilants
//! et éléments de scènes (écran-titre, fins, intermèdes), types 0 à 15 sauf le 4 (commun, voir
//! `set.rs`). Voir `docs/familles/sobj.md`.
//!
//! Les objets SOBJ figurent dans les listes triées : `+0x42` choisit la liste du premier plan
//! (non nul) ou celle des acteurs. Le créateur renseigne `+3` (variante) et souvent `+0x4c` /
//! `+0x2f` / `+0x14` (paramètres propres au type).

use super::cx::{handlers, sub, Cx};
use super::shell::{suspended, GAME_PAUSED};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::objects::Obj;

/// Variables de la RAM de travail (A5+) : caméra du jeu et caméra secondaire.
mod ram {
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    /// Position précédente de la caméra (pour en déduire le défilement de l'image).
    pub const PREV_CAMERA_X: u32 = 0x524;
    pub const PREV_CAMERA_Y: u32 = 0x528;
    pub const CAMERA2_X: u32 = 0x556;
    pub const CAMERA2_Y: u32 = 0x55a;
    pub const PREV_CAMERA2_X: u32 = 0x574;
    /// `A5+0x36` non nul : la caméra de référence est la secondaire.
    pub const USE_CAMERA2: u32 = 0x36;
}

/// Jeu d'animations commun des objets SOBJ.
const ANIMS: u32 = 0x1c03a6;

/// Fin de gestionnaire des objets à liste variable : liste du premier plan si `+0x42` est non nul,
/// liste des acteurs sinon.
fn reinsert_by_layer(x: &mut Cx) {
    if x.u8(0x42) != 0 {
        x.call(sub::REINSERT_FOREGROUND);
    } else {
        x.reinsert();
    }
}

/// Sort de l'écran de plus que `margin` pixels : état 4 (disparition). La marge est dans D2 ; le
/// test se fait en mots non signés (routine 0xddbfc de l'original).
fn cull_beyond(x: &mut Cx, margin: u16) {
    let mut camera = x.a5_u16(ram::CAMERA_X);
    if x.a5_u8(ram::USE_CAMERA2) != 0 {
        camera = x.a5_u16(ram::CAMERA2_X);
    }
    let screen_x = x.u16(0x8).wrapping_sub(camera).wrapping_add(margin);
    let limit = margin.wrapping_mul(2).wrapping_add(0x180);
    if screen_x > limit {
        x.set8(0x4, 4);
    }
}

/// Décalage de la caméra depuis la précédente image (X puis Y), en 16.16.
fn camera_delta(x: &mut Cx) -> (u32, u32) {
    (
        x.a5_u32(ram::CAMERA_X).wrapping_sub(x.a5_u32(ram::PREV_CAMERA_X)),
        x.a5_u32(ram::CAMERA_Y).wrapping_sub(x.a5_u32(ram::PREV_CAMERA_Y)),
    )
}

/// Bits de luminosité forcés au maximum des couleurs copiées dans les palettes de GFX RAM.
const BRIGHT: u32 = 0xf000_f000;

/// Copie `n` longs de la ROM (espace de données) vers la mémoire vidéo ou la RAM.
fn copy_longs(x: &mut Cx, from: u32, to: u32, n: u32) {
    for i in 0..n {
        let v = x.data32(from + 4 * i);
        wr32(x.b, to + 4 * i, v);
    }
}

/// Force des bits dans `n` longs consécutifs.
fn or_longs(x: &mut Cx, at: u32, mask: u32, n: u32) {
    for i in 0..n {
        let v = rd32(x.b, at + 4 * i) | mask;
        wr32(x.b, at + 4 * i, v);
    }
}

/// Opération arithmétique `asr.l` sur 32 bits.
fn asr(v: u32, n: u32) -> u32 {
    ((v as i32) >> n) as u32
}

// ---------------------------------------------------------------------------------------------
// Type 0 (0xdd61e) : élément de décor lié aux caméras
// ---------------------------------------------------------------------------------------------

/// Élément de décor dont la position suit le défilement des caméras à une fraction de leur
/// vitesse (parallaxe, nuages, avant-plan). La variante `+3` choisit, par deux tables d'octets de
/// la ROM, la routine d'initialisation et la routine de suivi par image.
mod scenery {
    use super::*;

    /// Variante -> routine d'initialisation (13 routines).
    const INIT_KIND: u32 = 0xdd6a4;
    /// Variante -> routine de suivi (23 routines).
    const STEP_KIND: u32 = 0xdd8ea;
    const WAIT: u8 = 0;
    const FOLLOW: u8 = 2;
    const GONE: u8 = 4;
    const FREE: u8 = 6;
    /// Fin du script d'animation (octet haut des drapeaux).
    const ANIM_END: u8 = 0xfe;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            WAIT => init(x),
            FOLLOW => follow(x),
            GONE => x.add8(0x4, 2),
            FREE => x.call(sub::FREE_SOBJ),
            _ => {}
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x14, 0);
        x.set8(0x42, 0);
        let variant = x.u8(0x3) as u32;
        x.anim_start(ANIMS, variant);
        match x.rom8(INIT_KIND + variant) {
            0 => {}
            1 => {
                let v = x.u16(0x8);
                x.set16(0xa, v);
                let v = x.u16(0x10);
                x.set16(0x12, v);
            }
            2 => {
                x.set8(0x42, 1);
                let v = x.a5_u32(ram::CAMERA_Y);
                x.set_a5_u32(0x50e, v);
            }
            3 => {
                x.set8(0x2d, 0);
                x.c.d[0] = 0xd;
                x.call(sub::SPEED_TABLE_B);
            }
            4 => {
                let v = x.a5_u16(ram::CAMERA2_X);
                x.set16(0xa, v);
                let v = x.a5_u16(ram::CAMERA2_Y);
                x.set16(0xe, v);
                x.add16(0x12, 0xd8);
                x.add16(0xe, 0x190);
            }
            5 => place_at(x, 0, 0x2c0),
            6 => x.set8(0x1d, 0xe),
            7 => x.set8(0x3f, 2),
            8 => place_at(x, 0, 0x2f0),
            9 => place_at(x, 0x100, 0x2f0),
            10 => place_at(x, 0x100, 0x2d0),
            11 => {
                x.set16(0x14, 0);
                x.set16(0x1c, 0x200);
            }
            _ => x.set8(0x42, 1),
        }
        reinsert_by_layer(x);
    }

    /// Positionne le décor (X, Y) et le remonte (Z = -0x100), au premier plan.
    fn place_at(x: &mut Cx, px: u16, py: u16) {
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(0xc, 0xff00);
        x.set8(0x42, 1);
    }

    fn follow(x: &mut Cx) {
        if x.u8(0x2f) == 0 {
            x.anim_step();
        }
        let variant = x.u8(0x3) as u32;
        let (dx, dy) = camera_delta(x);
        match x.rom8(STEP_KIND + variant) {
            0 => cull_beyond(x, 0x60),
            1 => {
                let d = x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X));
                let v = x.u16(0xa).wrapping_sub(d);
                x.set16(0x8, v);
                let d = x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
                let v = x.u16(0x12).wrapping_sub(d);
                x.set16(0x10, v);
                cull_beyond(x, 0x60);
            }
            2 => {
                let v = x.a5_u16(0x5e6);
                x.set16(0x8, v);
                let v = x.a5_u32(0x5ee).wrapping_add(1);
                x.set32(0x10, v);
                let n = x.u8(0x1d).wrapping_sub(1);
                x.set8(0x1d, n);
                if (n as i8) < 0 {
                    x.set8(0x1d, 0xe);
                    let v = x.u8(0xd) ^ 1;
                    x.set8(0xd, v);
                }
                cull_beyond(x, 0x60);
                reinsert_by_layer(x);
            }
            16 => {
                let v = x.a5_u16(0x5e6);
                x.set16(0x8, v);
                let v = x.a5_u32(0x5ee).wrapping_add(1);
                x.set32(0x10, v);
                cull_beyond(x, 0x60);
                reinsert_by_layer(x);
            }
            3 => {
                x.add32(0x8, dx);
                x.add32(0x10, dy);
                reinsert_by_layer(x);
            }
            7 => {
                let d = x.a5_u32(ram::PREV_CAMERA2_X).wrapping_sub(x.a5_u32(ram::CAMERA2_X));
                x.add32(0x8, 0u32.wrapping_sub(d));
                cull_beyond(x, 0x60);
                reinsert_by_layer(x);
            }
            4 => {
                let d2 = dy.wrapping_sub(asr(dy, 1));
                x.add32(0x8, 0u32.wrapping_sub(dx));
                x.add32(0xc, 0u32.wrapping_sub(d2));
                reinsert_by_layer(x);
            }
            5 => {
                let d0 = dx.wrapping_sub(asr(dx, 3));
                let d1 = asr(dy, 3).wrapping_sub(dy);
                x.add32(0x8, d0);
                x.add32(0xc, 0u32.wrapping_sub(d1));
                reinsert_by_layer(x);
            }
            10 => {
                x.add32(0x8, asr(dx, 2));
                x.add32(0xc, asr(dy, 2));
                reinsert_by_layer(x);
            }
            11 => {
                let d = x.a5_u32(ram::CAMERA2_X).wrapping_sub(x.a5_u32(ram::PREV_CAMERA2_X));
                x.add32(0x8, 0u32.wrapping_sub(d));
                reinsert_by_layer(x);
            }
            6 => reinsert_by_layer(x),
            8 => wait_script_end(x, 2),
            9 => {
                x.call(sub::MOVE_XY);
                if x.u16(0x10) > 0x240 {
                    x.add8(0x4, 2);
                }
            }
            12..=15 => wait_script_end(x, 4),
            17 => {
                let mut d0 = asr(dx, 1);
                let mut d2 = asr(dy, 1);
                if x.u8(0x3) == 0xa {
                    d0 = d0.wrapping_add(asr(d0, 2));
                    d2 = d2.wrapping_add(asr(d2, 2));
                }
                x.add32(0x8, d0);
                x.add32(0xc, d2);
            }
            18 => {
                x.call(sub::MOVE_Y);
                if x.u8(0x1) & 1 == 0 {
                    x.add8(0x4, 2);
                }
            }
            20 => {
                let lim = x.a5_u16(ram::CAMERA_X).wrapping_sub(0x40) as i16;
                if lim > x.u16(0x8) as i16 {
                    x.add8(0x4, 2);
                }
            }
            21 => {
                if x.a5_u8(0x42) == 0 {
                    x.anim_start(ANIMS, variant);
                } else {
                    wait_script_end(x, 2)
                }
            }
            22 => {
                if x.a5_u8(0x42) != 0 {
                    x.call(sub::MOVE_XY);
                    if x.u16(0x10) > 0x240 {
                        x.add8(0x4, 2);
                    }
                }
            }
            _ => {}
        }
    }

    /// Passe à l'état suivant (+`step`) quand l'animation est terminée.
    fn wait_script_end(x: &mut Cx, step: u8) {
        if x.u8(0x32) == ANIM_END {
            x.add8(0x4, step);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1 (0xddc28) : décor fixe à deux comportements
// ---------------------------------------------------------------------------------------------

/// Décor animé (jeu d'animations 0x1c40c6) qui reste en place, se déplace avec la caméra à demi
/// vitesse ou suit la caméra secondaire selon la variante.
mod fixture {
    use super::*;

    const ANIMS: u32 = 0x1c40c6;
    /// Variante -> forme du comportement à la création (0 : aucun, 1 : mémorise X/Y).
    const INIT_KIND: u32 = 0xddc6e;
    /// Variante -> forme du comportement par image (0 : sortie d'écran, 1 : défilement, 2 : suivi).
    const STEP_KIND: u32 = 0xddd16;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let variant = x.u8(0x3) as u32;
                x.anim_start(ANIMS, variant);
                x.reinsert();
                if x.rom8(INIT_KIND + variant) == 1 {
                    let v = x.u16(0x8);
                    x.set16(0xa, v);
                    let v = x.u16(0x10);
                    x.set16(0x12, v);
                }
            }
            2 => {
                if x.u8(0x2f) == 0 {
                    x.anim_step();
                }
                let variant = x.u8(0x3) as u32;
                match x.rom8(STEP_KIND + variant) {
                    0 => cull_beyond(x, 0x60),
                    1 => {
                        let (dx, dy) = camera_delta(x);
                        x.add32(0x8, asr(dx, 1));
                        x.add32(0xc, asr(dy, 1));
                    }
                    _ => {
                        let d = x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X));
                        let v = x.u16(0xa).wrapping_sub(d);
                        x.set16(0x8, v);
                        let d = x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
                        let v = x.u16(0x12).wrapping_sub(d);
                        x.set16(0x10, v);
                        cull_beyond(x, 0x60);
                    }
                }
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 2 (0xdddb2) : décor clignotant
// ---------------------------------------------------------------------------------------------

/// Décor animé qui disparaît 16 fois de suite à chaque fin de script (`+0x14` compte les
/// clignotements) ; sa marge d'écran est le D2 laissé par l'appelant.
mod flasher {
    use super::*;

    const BLINKS: u8 = 0x10;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                restart(x);
            }
            2 => {
                if x.anim_step() {
                    x.set_bit(0x1, 1, true);
                    let n = x.u8(0x14).wrapping_sub(1);
                    x.set8(0x14, n);
                    if (n as i8) < 0 {
                        x.set_bit(0x1, 1, false);
                        restart(x);
                    }
                }
            }
            _ => return x.call(sub::FREE_SOBJ),
        }
        let margin = x.c.d[2] as u16;
        cull_beyond(x, margin);
        x.reinsert();
    }

    fn restart(x: &mut Cx) {
        x.set8(0x14, BLINKS);
        let variant = x.u8(0x3) as u32;
        x.anim_start(ANIMS, variant);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 3 (0xdde18) : étiquette collée à un objet
// ---------------------------------------------------------------------------------------------

/// Étiquette qui suit un objet (adresse longue dans `+0x14`) avec un décalage constant (X + 0x50,
/// Y - 0x58) tant qu'il n'est pas dans l'état 6.
mod tag {
    use super::*;

    const ANIM: u32 = 0x43;
    const OFFSET_X: u16 = 0x50;
    const OFFSET_Y: u16 = 0x58;
    const HOST: u32 = 0x14;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x18, OFFSET_X);
                x.set16(0x1a, OFFSET_Y);
                x.anim_start(ANIMS, ANIM);
                x.reinsert();
            }
            2 => {
                let host = Obj(x.u32(HOST) & 0xff_ffff);
                if host.u8(x.b, 0x4) == 6 {
                    x.add8(0x4, 2);
                    return;
                }
                let px = host.u16(x.b, 0x8).wrapping_add(x.u16(0x18));
                let py = host.u16(x.b, 0x10).wrapping_sub(x.u16(0x1a));
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.anim_step();
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5 (0xdde8c) : objet fantôme
// ---------------------------------------------------------------------------------------------

/// Objet sans comportement : il se libère dès sa première image.
mod ghost {
    use super::*;

    pub fn run(x: &mut Cx) {
        if x.u8(0x4) == 0 {
            x.add8(0x4, 2);
        }
        x.call(sub::FREE_SOBJ);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 6 (0xddec4) : éléments des scènes scriptées
// ---------------------------------------------------------------------------------------------

/// Éléments des scènes (présentation, intermèdes, fin) : 24 variantes (`+3`), chacune une petite
/// machine à états. Les variantes sont reliées à un objet « maître » (`+0x4c`, souvent un autre
/// objet de ce type) dont l'octet `+8` sert de boîte aux lettres : 1 « top », 2 « étape suivante »,
/// négatif « fin ». Beaucoup chargent aussi des palettes depuis la ROM (0x28f340...) dans la
/// palette de la page 0x91c080-0x91c3ff et lancent des sons (0xea2e8...).
mod scene {
    use super::*;

    /// État 4 (« mort ») : l'objet est marqué à supprimer puis libéré tout de suite.
    const DEAD: u32 = 0x0600_0000;
    /// Palettes de la page des éléments (GFX RAM) : 16 couleurs.
    const PALETTE_A: u32 = 0x91c080;
    const PALETTE_B: u32 = 0x91c3e0;
    /// Générateur de nombres aléatoires.
    const RANDOM: u32 = 0x19a88;
    /// Création d'un objet SET dont le type est dans D0.
    const SPAWN_SET: u32 = 0xc8312;

    pub fn run(x: &mut Cx) {
        match x.u8(0x3) {
            0 => lifecycle(x, v0_init, v0_step),
            1 => lifecycle(x, v1_init, v1_step),
            2 => lifecycle(x, v2_init, v2_step),
            3 => match x.u8(0x4) {
                0 => v3_init(x),
                2 => v3_step(x),
                4 => {
                    if signal(x) != 0 {
                        return kill(x);
                    }
                    x.reinsert();
                }
                _ => {}
            },
            4 => lifecycle(x, v4_init, v4_step),
            5 => lifecycle(x, v5_init, v5_step),
            6 => lifecycle(x, v6_init, v6_step),
            7 => lifecycle(x, v7_init, v7_step),
            8 => lifecycle(x, v8_init, v8_step),
            9 => lifecycle(x, v9_init, v9_step),
            10 => lifecycle(x, v10_init, v10_step),
            11 => lifecycle(x, v11_init, |_| {}),
            12 => lifecycle(x, v12_init, v12_step),
            13 => lifecycle(x, v13_init, |_| {}),
            14 => lifecycle(x, v14_init, v14_step),
            15 => lifecycle(x, v15_init, |_| {}),
            16 => lifecycle(x, v16_init, v16_step),
            17 => lifecycle(x, v17_init, v17_step),
            18 => lifecycle(x, v18_init, |_| {}),
            19 => lifecycle(x, v19_init, v19_step),
            20 => lifecycle(x, v20_init, v20_step),
            21 => lifecycle(x, v21_init, |_| {}),
            22 => lifecycle(x, v20_init, v20_fall),
            23 => lifecycle(x, v23_init, v23_follow_and_insert),
            _ => {}
        }
    }

    /// Cycle de vie commun : 0 initialisation, 2 vie, 4 mort, 6 libération.
    fn lifecycle(x: &mut Cx, init: fn(&mut Cx), step: fn(&mut Cx)) {
        match x.u8(0x4) {
            0 => init(x),
            2 => step(x),
            4 => kill(x),
            6 => x.call(sub::FREE_SOBJ),
            _ => {}
        }
    }

    fn kill(x: &mut Cx) {
        x.set32(0x4, DEAD);
        x.call(sub::FREE_SOBJ);
    }

    fn rom32(x: &mut Cx, addr: u32) -> u32 {
        (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
    }

    /// Le maître de la scène (`+0x4c`).
    fn master(x: &mut Cx) -> Obj {
        Obj(x.u32(0x4c) & 0xff_ffff)
    }

    /// Octet de signal du maître (`+8`).
    fn signal(x: &mut Cx) -> u8 {
        let m = master(x);
        m.u8(x.b, 0x8)
    }

    fn send_signal(x: &mut Cx, v: u8) {
        let m = master(x);
        m.set_u8(x.b, 0x8, v);
    }

    /// Démarre le script `n` et remet l'objet en place dans la liste des acteurs.
    fn start(x: &mut Cx, n: u32) {
        x.anim_start(ANIMS, n);
        x.reinsert();
    }

    /// Crée un objet SOBJ de type 6 (routine 0xddeaa), de variante `variant` ; rien en cas d'échec.
    fn spawn(x: &mut Cx, variant: u8) -> Option<Obj> {
        x.call(sub::ALLOC_SOBJ);
        if x.c.sr & m68k::SR_C != 0 {
            return None;
        }
        let child = Obj(x.c.a[1]);
        child.set_u8(x.b, 0, 1);
        child.set_u8(x.b, 2, 6);
        child.set_u8(x.b, 3, variant);
        Some(child)
    }

    /// Fait défiler l'écran d'un pixel toutes les 8 images (routine 0xdea4a) en gardant l'objet
    /// immobile à l'écran.
    fn scroll_creep(x: &mut Cx) {
        if x.a5_u8(0x6f) & 7 != 0 {
            return;
        }
        let v = x.a5_u16(ram::CAMERA_X).wrapping_sub(1);
        x.set_a5_u16(ram::CAMERA_X, v);
        x.set_a5_u16(0x50e, v);
        let v = x.a5_u16(ram::CAMERA2_X).wrapping_add(1);
        x.set_a5_u16(ram::CAMERA2_X, v);
        x.set_a5_u16(0x55e, v);
        let (dx, dy) = camera_delta(x);
        x.add32(0x8, dx);
        x.add32(0x10, dy);
    }

    /// Variante 0 : sprite lié au maître (jeu d'animations, indicateur de flip et numéro lus dans
    /// une table à 8 octets par valeur de `+0x2f`), flip aléatoire si l'indicateur est négatif.
    const V0_ENTRIES: u32 = 0xddf6c;

    fn random_flip(x: &mut Cx) {
        x.set8(0x2c, 0);
        x.call(RANDOM);
        if x.c.d[0] & 1 != 0 {
            x.set8(0x2c, 0x20);
        }
    }

    fn v0_init(x: &mut Cx) {
        if (signal(x) as i8) < 0 {
            return x.call(sub::FREE_SOBJ);
        }
        x.add8(0x4, 2);
        let e = V0_ENTRIES + 8 * x.u8(0x2f) as u32;
        let set = rom32(x, e);
        let flip = x.rom8(e + 4);
        x.set8(0x2c, flip);
        let n = x.rom16(e + 6) as u32;
        x.anim_start(set, n);
        if (x.u8(0x2c) as i8) < 0 {
            random_flip(x);
        }
        x.reinsert();
    }

    fn v0_step(x: &mut Cx) {
        x.anim_step();
        if signal(x) as i8 >= 0 {
            return;
        }
        x.add8(0x4, 2);
    }

    /// Variante 1 : lumière qui grandit lentement jusqu'au signal du maître.
    fn v1_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x8, 0x30_0000);
        x.set32(0xc, 0xd0_0000);
        x.set32(0x10, 0);
        start(x, 0xee);
        x.call(0xea662);
    }

    fn v1_step(x: &mut Cx) {
        if signal(x) != 0 {
            return kill(x);
        }
        x.add32(0x8, 0x8000);
        x.add32(0xc, 0x8000);
    }

    /// Variante 2 : décor qui défile, trois temps (`+5`) rythmés par le signal du maître.
    fn v2_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x8, 0x100);
        x.set16(0xc, 0xc0);
        x.set32(0x10, 0);
        scroll_creep(x);
        start(x, 0xef);
    }

    fn v2_step(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                if signal(x) != 0 {
                    x.add8(0x5, 2);
                }
            }
            2 => {
                scroll_creep(x);
                if signal(x) == 0 {
                    x.add8(0x5, 2);
                }
            }
            4 => {
                if signal(x) != 0 {
                    kill(x);
                }
            }
            _ => {}
        }
    }

    /// Variante 3 : lance des objets SET décoratifs d'après une liste d'octets (0xde1e0, ou
    /// 0xde1e8 si `A5+0x38`), un toutes les 30 images, jusqu'à la fin de son script.
    const V3_LISTS: u32 = 0xde1e0;

    fn v3_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x8, 0x60);
        x.set16(0xc, 0xc0);
        x.set32(0x10, 0);
        x.set16(0x42, 0x1e);
        x.set32(0x44, V3_LISTS);
        if x.a5_u16(0x38) != 0 {
            x.add32(0x44, 8);
        }
        x.set16(0x48, 0);
        start(x, 0xf1);
        x.call(0xea6a2);
    }

    fn v3_step(x: &mut Cx) {
        if x.u16(0x48) != 0 {
            let child = x.linked(0x48);
            if child.u8(x.b, 0x22) == 0 {
                return;
            }
            x.set16(0x48, 0);
        }
        let n = x.u16(0x42).wrapping_sub(1);
        x.set16(0x42, n);
        if n != 0 {
            return;
        }
        let mut list = x.u32(0x44);
        let kind = x.data8(list);
        list += 1;
        if kind != 0 {
            x.c.d[0] = kind as u32;
            x.call(SPAWN_SET);
            if x.c.sr & m68k::SR_C == 0 {
                let a1 = x.c.a[1] as u16;
                x.set16(0x48, a1);
            }
        }
        x.set32(0x44, list);
        x.anim_step();
        x.set16(0x42, 0x1e);
        if x.u16(0x32) == 0 {
            return;
        }
        x.set16(0x42, 0xa);
        x.add8(0x4, 2);
        send_signal(x, 1);
    }

    /// Variante 4 : monte vers le haut de l'écran (Z - 1,25 par image) et s'arrête à Z = 0x20.
    fn v4_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x4000);
        x.set16(0x8, 5);
        x.set16(0xc, 0x98);
        start(x, 0xf5);
    }

    fn v4_step(x: &mut Cx) {
        x.add32(0xc, 0u32.wrapping_sub(0x14000));
        if x.u16(0xc) as i16 > 0x20 {
            return;
        }
        x.add8(0x4, 2);
        x.set8(0x5, 0);
    }

    /// Variante 5 : meneur qui descend, déclenche le défilement puis crée des variantes 0 (deux au
    /// plus ; décalages X puis Z en table 0xde386).
    const V5_OFFSETS: u32 = 0xde386;

    fn v5_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x8000);
        x.set16(0x8, 0x78);
        x.set16(0xc, 0x90);
        start(x, 0xf6);
    }

    fn v5_step(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add32(0xc, 0x10000);
                if x.u16(0xc) < 0xd0 {
                    return;
                }
                x.set16(0xc, 0xd0);
                x.add8(0x5, 2);
            }
            2 => {
                let y = x.a5_u32(ram::CAMERA2_Y).wrapping_add(0x10000);
                x.set_a5_u32(ram::CAMERA2_Y, y);
                x.set_a5_u16(0x562, (y >> 16) as u16);
                if ((y >> 16) as u16) < 0x28 {
                    return;
                }
                x.add8(0x5, 2);
                send_signal(x, 1);
                x.set16(0x42, 1);
            }
            4 => {
                if signal(x) as i8 >= 0 {
                    return v5_spawn(x);
                }
                x.add8(0x4, 2);
                x.set8(0x5, 0);
            }
            _ => {}
        }
    }

    fn v5_spawn(x: &mut Cx) {
        let n = x.u8(0x43).wrapping_sub(1);
        x.set8(0x43, n);
        if n != 0 || x.u8(0x42) == 2 {
            return;
        }
        x.set8(0x43, 0x30);
        let Some(child) = spawn(x, 0) else { return };
        let which = x.u8(0x42);
        child.set_u8(x.b, 0x2f, which);
        for off in [0x4c, 0x8, 0xc, 0x10] {
            let v = x.u32(off);
            child.set_u32(x.b, off, v);
        }
        let at = V5_OFFSETS + 4 * which as u32;
        let dx = x.rom16(at);
        let dz = x.rom16(at + 2);
        let v = child.u16(x.b, 0x8).wrapping_add(dx);
        child.set_u16(x.b, 0x8, v);
        let v = child.u16(x.b, 0xc).wrapping_add(dz);
        child.set_u16(x.b, 0xc, v);
        let v = child.u32(x.b, 0x10).wrapping_sub(1);
        child.set_u32(x.b, 0x10, v);
        x.add8(0x42, 1);
    }

    /// Variante 6 : lettre qui tombe du haut (Z 0x300) jusqu'à sa hauteur d'arrêt (table 0xde3ee :
    /// X, arrêt), attend le signal 2 puis remonte de 0x290.
    const V6_TABLE: u32 = 0xde3ee;

    fn v6_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x10, 0xf000);
        x.set32(0xc, 0x300_0000);
        let v = x.u8(0x2f) as u32;
        let shift = (v << 8) & 0xffff;
        let y = x.u32(0x10).wrapping_sub(shift);
        x.set32(0x10, y);
        let px = x.rom16(V6_TABLE + 4 * v);
        x.set16(0x8, px);
        let stop = x.rom16(V6_TABLE + 4 * v + 2);
        x.set16(0x14, stop);
        x.anim_start(ANIMS, v + 0xfe);
        x.reinsert();
    }

    fn v6_step(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add16(0xc, 2);
                let stop = x.u16(0x14);
                if stop <= x.u16(0xc) {
                    x.set16(0xc, stop);
                    x.add8(0x5, 2);
                    let m = master(x);
                    let v = m.u8(x.b, 0xd).wrapping_sub(1);
                    m.set_u8(x.b, 0xd, v);
                }
            }
            2 => {
                if signal(x) == 2 {
                    x.add16(0xc, 0u16.wrapping_sub(0x290));
                    x.add8(0x5, 2);
                }
            }
            4 => {
                if (signal(x) as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
        x.reinsert();
    }

    /// Variante 7 : fondu de palette (page B) puis attente de 180 images ; charge aussi un script
    /// de transition (`A5+0x5c6`...). Script choisi par `+0x2f` et par `A5+0x72`.
    const V7_SCRIPTS: u32 = 0xde4d2;
    const V7_PALETTES: u32 = 0x292240;

    fn v7_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x42, 6);
        let mut off = 4 * x.u8(0x2f) as u32;
        if x.a5_u16(0x72) != 2 {
            off += 8;
        }
        let script = rom32(x, V7_SCRIPTS + off);
        let w = x.data16(script);
        x.set_a5_u16(0x5ca, w);
        let w = x.data16(script + 2);
        x.set_a5_u16(0x5cc, w);
        x.set_a5_u32(0x5c6, script + 4);
        copy_longs(x, V7_PALETTES, PALETTE_B, 8);
        x.set_a5_u8(0x48e, 1);
        x.call(0x168c);
    }

    fn v7_step(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            if rd8(x.b, PALETTE_B) & 0xf0 == 0xf0 {
                x.set16(0x42, 0xb4);
                x.add8(0x5, 2);
                return;
            }
            let n = x.u16(0x42).wrapping_sub(1);
            x.set16(0x42, n);
            if n != 0 {
                return;
            }
            x.set16(0x42, 6);
            for i in 0..8 {
                let v = rd32(x.b, PALETTE_B + 4 * i).wrapping_add(0x1000_1000);
                wr32(x.b, PALETTE_B + 4 * i, v);
            }
        } else {
            let n = x.u16(0x42).wrapping_sub(1);
            x.set16(0x42, n);
            if n != 0 {
                return;
            }
            x.add8(0x4, 2);
            let m = master(x);
            let v = m.u8(x.b, 0xc).wrapping_sub(1);
            m.set_u8(x.b, 0xc, v);
            x.set_a5_u32(0x5c6, 0);
        }
    }

    /// Variante 8 : barre qui descend (Z + 8 par image) jusqu'en bas de l'écran, puis lève l'octet
    /// 0xff0eaa (A5-0x7156).
    const SCENE_DONE_FLAG: u32 = 0xff_0eaa;

    fn v8_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, v);
        x.add16(0xc, 0x20);
        x.set32(0x10, 0);
        x.add16(0x10, 0u16.wrapping_sub(0x20));
        start(x, 0x112);
    }

    fn v8_step(x: &mut Cx) {
        x.add16(0xc, 8);
        let below = x.u16(0xc).wrapping_add(x.u16(0x10)).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
        if below < 0x1c8 {
            return;
        }
        let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0x1c8);
        x.set16(0xc, y);
        crate::mem::wr8(x.b, SCENE_DONE_FLAG, 0xff);
        x.add8(0x4, 2);
        x.reinsert();
    }

    /// Variante 9 : texte animé sur la caméra secondaire ; signale sa fin au maître.
    fn v9_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA2_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA2_Y);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        x.add16(0xc, 0xf0);
        start(x, 0x113);
        x.call(0xea2e8);
    }

    fn v9_step(x: &mut Cx) {
        if x.anim_step() || (signal(x) as i8) < 0 {
            return x.set8(0x4, 6);
        }
        if x.u16(0x30) != 0x7fff {
            return;
        }
        send_signal(x, 1);
        x.set16(0x30, 6);
    }

    /// Variante 10 : plaque qui descend en deux temps (Z 0x88 puis 0xa0), attend le signal 2,
    /// remonte à Z 0x80 et attend la fin.
    fn v10_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x8, 0x98_0000);
        x.set32(0xc, 0x70_0000);
        x.set32(0x10, 0);
        start(x, 0x114);
        x.call(0xea77e);
    }

    fn v10_step(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add32(0xc, 0x4000);
                if x.u16(0xc) < 0x88 {
                    return;
                }
                x.set16(0xc, 0x88);
                send_signal(x, 1);
                x.add8(0x5, 2);
            }
            2 => {
                x.add32(0xc, 0x4000);
                if x.u16(0xc) < 0xa0 {
                    return;
                }
                x.set16(0xc, 0xa0);
                x.add8(0x5, 2);
            }
            4 => {
                if signal(x) != 2 {
                    return;
                }
                x.add8(0x5, 2);
                x.set8(0x42, 0x1e);
            }
            6 => {
                let n = x.u8(0x42).wrapping_sub(1);
                x.set8(0x42, n);
                if n != 0 {
                    return;
                }
                x.add8(0x5, 2);
                x.call(0xea87e);
            }
            8 => {
                x.add32(0xc, 0u32.wrapping_sub(0x4000));
                if x.u16(0xc) as i16 > 0x80 {
                    return;
                }
                x.set16(0xc, 0x80);
                send_signal(x, 1);
                x.add8(0x5, 2);
            }
            10 => {
                if (signal(x) as i8) < 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Variante 11 : sprite fixe sur la caméra principale.
    fn v11_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x8000);
        let y = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, y);
        x.add16(0xc, 0xc0);
        x.set16(0x8, 0x30);
        start(x, 0x115);
        x.call(0xea7fe);
    }

    /// Variante 12 : sprite qui traverse l'écran (1 pixel par image vers la droite, ou vers la
    /// gauche si `+0x2f`) et fait reculer la caméra.
    fn v12_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x2c, 0);
        x.set32(0x8, 0xf0_0000);
        let y = x.a5_u16(ram::CAMERA2_Y);
        x.set16(0xc, y);
        x.set16(0x14, 0x100);
        x.add16(0xc, 0xc0);
        x.set32(0x10, 0);
        if x.u8(0x2f) != 0 {
            x.set8(0x2c, 0x20);
            let v = x.u16(0x14).wrapping_neg();
            x.set16(0x14, v);
            x.add32(0x8, 0u32.wrapping_sub(0x60_0000));
        }
        x.anim_start(ANIMS, 0x116);
        x.reinsert();
    }

    fn v12_step(x: &mut Cx) {
        let d = (((x.u16(0x14) as u32) << 16) as i32 >> 8) as u32;
        x.add32(0x8, d);
        let screen = x.u16(0x8).wrapping_sub(x.a5_u16(ram::CAMERA_X));
        if screen >= 0x180 {
            x.set8(0x4, 4);
        }
        if x.u8(0x2f) != 0 {
            return;
        }
        let cam = x.a5_u16(ram::CAMERA_X);
        if cam != 0 {
            x.set_a5_u16(ram::CAMERA_X, cam - 1);
        }
    }

    /// Variante 13 : cadre dont le script est une entrée directe (16 octets par `+0x2f` à partir de
    /// 0xdea84), sur un fond de palette 0x91c340 (8 couleurs identiques).
    const V13_SCRIPTS: u32 = 0xdea84;

    fn v13_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        for i in 0..8 {
            wr32(x.b, 0x91c340 + 4 * i, 0xf111_f111);
        }
        x.add16(0x8, 0x10);
        x.add16(0xc, 0x50);
        let v = x.u8(0x2f) as u16;
        x.add16(0xc, v << 3);
        x.add16(0xc, v << 4);
        x.c.a[4] = V13_SCRIPTS + 16 * v as u32;
        x.call(0x1a3c);
        x.reinsert();
    }

    /// Variante 14 : insigne dont le sprite et la position dépendent de `+0x46` (numéro, ou objet
    /// dont on prend la variante si le bit 15 est levé) ; table à 8 octets en 0xdef1e.
    const V14_TABLE: u32 = 0xdef1e;

    fn v14_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let mut which = x.u16(0x46);
        if (which as i16) < 0 {
            let owner = x.linked(0x46);
            which = owner.u8(x.b, 0x3) as u16;
        }
        x.set32(0xc, 0);
        let row = V14_TABLE.wrapping_add((which << 3) as i16 as i32 as u32);
        let v = x.rom16(row);
        x.set16(0x8, v);
        let v = x.rom16(row + 2);
        x.set16(0x10, v);
        let v = x.rom8(row + 6);
        x.set8(0x2c, v);
        x.anim_start(ANIMS, 0x190 + which as u32);
        x.reinsert();
    }

    fn v14_step(x: &mut Cx) {
        if (signal(x) as i8) < 0 {
            x.add8(0x4, 2);
        }
    }

    /// Variante 15 : fanion coloré (palette 0x28f960, une ligne de 32 octets par numéro tiré de
    /// la table 0xdefa4), position en table 0xdf01a.
    const V15_PALETTE_SELECT: u32 = 0xdefa4;
    const V15_PALETTES: u32 = 0x28f960;
    const V15_POSITIONS: u32 = 0xdf01a;

    fn v15_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set32(0x10, 0);
        let v = x.u8(0x2f) as u32;
        let line = x.rom8(V15_PALETTE_SELECT + v) as u32;
        copy_longs(x, V15_PALETTES + (line << 5), PALETTE_A, 8);
        or_longs(x, PALETTE_A, BRIGHT, 8);
        let px = x.rom16(V15_POSITIONS + 4 * v);
        x.set16(0x8, px);
        let py = x.rom16(V15_POSITIONS + 4 * v + 2);
        x.set16(0xc, py);
        start(x, 0x19c + v);
    }

    /// Variante 16 : sprite flippé au hasard, qui vit tant que le maître n'a pas fini et que son
    /// animation n'est pas terminée.
    fn v16_init(x: &mut Cx) {
        x.add8(0x4, 2);
        random_flip(x);
        start(x, 0x1a2);
        x.call(0xea7be);
    }

    fn v16_step(x: &mut Cx) {
        if signal(x) as i8 >= 0 && !x.anim_step() {
            return;
        }
        x.add8(0x4, 2);
    }

    /// Variante 17 : bulle qui sort du bas de l'écran (deux positions selon `+0x2f`) ; en fin
    /// d'animation, rend le bit (`+0x2f` modulo 8) de `+0x30` au propriétaire `+0x46`.
    fn v17_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set32(0x10, 0x4000);
        x.set16(0xc, 0x1c0);
        x.set32(0x8, 0);
        if x.u8(0x2f) == 0 {
            x.set16(0x8, 0x20);
            x.set32(0x10, 0x2000);
        }
        // `move.w #0x1a3,D0w ; add.b (0x2f,A0),D0b` : l'addition ne déborde pas sur l'octet haut
        let n = 0x100 | (0xa3 + x.u8(0x2f) as u32) & 0xff;
        start(x, n);
        x.call(0xea83e);
    }

    fn v17_step(x: &mut Cx) {
        if !x.anim_step() {
            return;
        }
        let owner = x.linked(0x46);
        let bit = x.u8(0x2f) & 7;
        let v = owner.u8(x.b, 0x30) & !(1 << bit);
        owner.set_u8(x.b, 0x30, v);
        x.add8(0x4, 2);
    }

    /// Variante 18 : sprite posé sur la caméra secondaire.
    fn v18_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA2_X);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA2_Y);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        let dz = if x.u8(0x2f) != 0 { 0x50 } else { 0x100 };
        x.add16(0xc, dz);
        x.anim_start(ANIMS, 0xec);
        x.reinsert();
    }

    /// Variante 19 : pastille de palette (0x28f340 + 0x60 + 32 par ligne, ligne de la table
    /// 0xdf182) posée sur la caméra ; elle disparaît quand l'animation de son porteur (`+0x46`)
    /// est finie.
    const V19_LINES: u32 = 0xdf182;
    const V19_PALETTES: u32 = 0x28f340 + 0x60;
    const V19_POSITIONS: u32 = 0xdf234;

    fn v19_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let v = x.u8(0x2f) as u32;
        let line = x.rom8(V19_LINES + 2 * v) as u32;
        let flip = x.rom8(V19_LINES + 2 * v + 1);
        x.set8(0x2c, flip);
        copy_longs(x, V19_PALETTES + (line << 5), PALETTE_B, 8);
        or_longs(x, PALETTE_B, BRIGHT, 8);
        let anim = if line == 3 { 0 } else { line };
        start(x, 0x1a8 + anim);
        v19_place(x);
    }

    /// Place la pastille d'après la table, relativement à la caméra courante.
    fn v19_place(x: &mut Cx) {
        let v = x.u8(0x2f) as u32;
        let px = x.rom16(V19_POSITIONS + 4 * v);
        x.set16(0x8, px);
        let py = x.rom16(V19_POSITIONS + 4 * v + 2);
        x.set16(0xc, py);
        x.set32(0x10, 0);
        let (cx, cy) = if x.a5_u8(ram::USE_CAMERA2) != 0 {
            (x.a5_u16(ram::CAMERA2_X), x.a5_u16(ram::CAMERA2_Y))
        } else {
            (x.a5_u16(ram::CAMERA_X), x.a5_u16(ram::CAMERA_Y))
        };
        x.add16(0x8, cx);
        x.add16(0xc, cy);
    }

    fn v19_step(x: &mut Cx) {
        v19_place(x);
        let owner = x.linked(0x46);
        if owner.u16(x.b, 0x32) != 0 {
            return;
        }
        x.add8(0x4, 2);
        kill(x);
    }

    /// Variantes 20 et 22 : débris qui tombent d'un point tiré au hasard sous le bord bas de la
    /// caméra (8 abscisses en table 0xdf2a8, 4 couples accélération/animation en 0xdf2ea).
    const V20_X: u32 = 0xdf2a8;
    const V20_FALL: u32 = 0xdf2ea;

    fn v20_init(x: &mut Cx) {
        x.add8(0x4, 2);
        let v = x.a5_u16(ram::CAMERA_Y);
        x.set16(0xc, v);
        let v = x.a5_u16(ram::CAMERA_X);
        x.set16(0x8, v);
        x.add16(0xc, 0x110);
        x.set16(0x10, 0x10);
        x.call(RANDOM);
        let k = x.c.d[0] & 7;
        let dx = x.rom16(V20_X + 2 * k);
        x.add16(0x8, dx);
        x.set32(0x14, 0);
        x.set16(0x18, 0);
        x.call(RANDOM);
        let k = x.c.d[0] & 3;
        let az = x.rom16(V20_FALL + 4 * k);
        x.set16(0x1a, az);
        let anim = x.rom16(V20_FALL + 4 * k + 2) as u32;
        x.anim_start(ANIMS, anim);
        x.reinsert();
    }

    fn v20_step(x: &mut Cx) {
        if (signal(x) as i8) < 0 {
            return kill(x);
        }
        v20_fall(x);
    }

    /// Chute : sort (état mort) quand le bas du sprite repasse au-dessus du bord haut de l'écran.
    fn v20_fall(x: &mut Cx) {
        x.call(sub::MOVE_XZ);
        let below = x.u16(0xc).wrapping_add(x.u16(0x10));
        if (below as i16) <= x.a5_u16(ram::CAMERA_Y) as i16 {
            kill(x);
        }
    }

    /// Variante 21 : comme 15, avec la ligne de palette désignée par `+0x2f / 2` (table 0xdf332),
    /// sans éclaircissement ; positions en table 0xdf396.
    const V21_PALETTE_SELECT: u32 = 0xdf332;
    const V21_POSITIONS: u32 = 0xdf396;

    fn v21_init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set32(0x10, 0);
        let v = x.u8(0x2f) as u32;
        let line = x.rom8(V21_PALETTE_SELECT + (v >> 1)) as u32;
        copy_longs(x, V15_PALETTES + (line << 5), PALETTE_A, 8);
        let px = x.rom16(V21_POSITIONS + 4 * v);
        x.set16(0x8, px);
        let py = x.rom16(V21_POSITIONS + 4 * v + 2);
        x.set16(0xc, py);
        start(x, 0x19c + (v >> 1));
    }

    /// Variante 23 : étincelle collée au coin de la structure de caméra (A5+0x4fe, ou 0x54e si
    /// `A5+0x36`), dont la couleur de la palette 0x91c180 reprend la luminosité de la couleur 0.
    const V23_PALETTE: u32 = 0x28f4c0;

    fn v23_init(x: &mut Cx) {
        x.add8(0x4, 2);
        copy_longs(x, V23_PALETTE, 0x91c180, 8);
        let top = rd32(x.b, 0x91c000) & BRIGHT;
        for i in 0..8 {
            let at = 0x91c180 + 4 * i;
            let v = rd32(x.b, at) & 0x0fff_0fff | top;
            wr32(x.b, at, v);
        }
        x.anim_start(ANIMS, 0x170);
        v23_follow(x);
        x.reinsert();
    }

    fn v23_follow_and_insert(x: &mut Cx) {
        v23_follow(x);
        x.reinsert();
    }

    fn v23_follow(x: &mut Cx) {
        let camera = if x.a5_u8(ram::USE_CAMERA2) != 0 { 0x54e } else { 0x4fe };
        x.set16(0x42, (0xff_8000u32 + camera) as u16);
        let cam = x.linked(0x42);
        let v = cam.u16(x.b, 0x8);
        x.set16(0x8, v);
        let v = cam.u16(x.b, 0xc);
        x.set16(0xc, v);
        x.add16(0xc, 0x10);
        x.set16(0x10, 0u16.wrapping_sub(0x10));
        x.set8(0x2c, 0);
        x.add16(0xc, 0x40);
        x.add16(0x8, 0x100);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 7, 9, 10 : aides à l'écran (bulle de conseil, cadre clignotant, flèche)
// ---------------------------------------------------------------------------------------------

/// Drapeau de l'état 6 posé à la sortie des objets d'aide : état 6, sous-état 0.
const LEAVE_6_0: u16 = 0x0600;
/// État 6, sous-état 2 : l'objet se libérera à la prochaine image (voir `hint`).
const LEAVE_6_2: u16 = 0x0602;
/// Octet de la RAM (0xff0ec6, A5-0x713a) : l'aide est inhibée tant qu'il est non nul.
const HINT_INHIBIT: u32 = 0xff_0ec6;

/// Lit les quatre joueurs enregistrés en 0xff2aa4 (A5-0x555c) : mots, 0 = absent.
const PLAYERS: u32 = 0xff_2aa4;

/// Bulle de conseil (type 7) : tableau de 0x28ff20 pour la palette 0x91c380, animations 0x118 à
/// 0x136 (une par conseil) lues dans `+0x14`, durée de chaque conseil dans `+0x16`. Elle est liée à
/// un objet (`+0x46`) dont `+0x42` bit 1 demande la fin et bit 0 signale que le conseil a été lu.
mod hint {
    use super::*;

    const PALETTE: u32 = 0x28ff20;
    const PALETTE_DST: u32 = 0x91c380;
    const FIRST_ADVICE: u32 = 0x118;
    const LAST_ADVICE: u16 = 0x136;
    const ADVICE_FRAMES: u8 = 0x50;
    const LEAVE_SOUND: u32 = 0xea49a;
    const SPAWN_ARROW: u32 = 0xdf896;

    pub fn run(x: &mut Cx) {
        let owner = x.linked(0x46);
        if owner.u8(x.b, 0x43) == 0 && rd8(x.b, HINT_INHIBIT) != 0 {
            x.set32(0x4, 0);
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => step(x),
            4 | 6 => leave(x),
            _ => {}
        }
    }

    fn start(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        x.add8(0x4, 2);
        load_palette(x);
        x.set8(0x16, ADVICE_FRAMES);
        x.set16(0x14, FIRST_ADVICE as u16);
        x.anim_start(ANIMS, FIRST_ADVICE);
    }

    /// Palette de la bulle, luminosité maximale ; l'objet devient visible (`+0x2a`, `+0x2b`).
    fn load_palette(x: &mut Cx) {
        copy_longs(x, PALETTE, PALETTE_DST, 8);
        or_longs(x, PALETTE_DST, BRIGHT, 8);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x1c);
    }

    fn step(x: &mut Cx) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        if x.u8(0x40) != 0 {
            load_palette(x);
        }
        x.set8(0x40, 0);
        if x.u16(0x46) == 0 {
            return x.set16(0x4, LEAVE_6_2);
        }
        let owner = x.linked(0x46);
        if owner.u8(x.b, 0x42) & 2 != 0 {
            x.call(LEAVE_SOUND);
            x.call(SPAWN_ARROW);
            let owner = x.linked(0x46);
            let v = owner.u8(x.b, 0x42) & !2;
            owner.set_u8(x.b, 0x42, v);
            return x.set16(0x4, LEAVE_6_2);
        }
        match x.u8(0x5) {
            0 => countdown(x),
            2 => next_advice(x),
            _ => {}
        }
        x.reinsert();
    }

    /// Sous-état 0 : le conseil reste affiché `+0x16` images, puis passe au suivant (sous-état 2)
    /// ou, après le dernier, rend la main à l'objet lié.
    fn countdown(x: &mut Cx) {
        if x.u16(0x46) == 0 {
            return x.set16(0x4, LEAVE_6_2);
        }
        let n = x.u8(0x16).wrapping_sub(1);
        x.set8(0x16, n);
        if (n as i8) < 0 {
            if x.u16(0x14) != LAST_ADVICE {
                x.add8(0x5, 2);
            } else {
                // le dernier conseil reste tant que la partie est dans l'état 0xc ou 6
                let mode = x.a5_u8(0x34);
                if mode == 0xc || mode == 6 {
                    return x.set8(0x16, ADVICE_FRAMES);
                }
                x.set16(0x4, LEAVE_6_2);
                let owner = x.linked(0x46);
                let v = owner.u8(x.b, 0x42) | 1;
                owner.set_u8(x.b, 0x42, v);
                return;
            }
        }
        follow(x);
    }

    fn next_advice(x: &mut Cx) {
        x.add8(0x5, 0u8.wrapping_sub(2));
        x.set8(0x16, ADVICE_FRAMES);
        let n = x.u16(0x14).wrapping_add(1);
        x.set16(0x14, n);
        x.anim_start(ANIMS, n as u32);
        follow(x);
    }

    /// La bulle suit la caméra (X + 0xb0, Z + 0xcf) et avance son animation.
    fn follow(x: &mut Cx) {
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0xb0);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(0xcf);
        x.set16(0xc, v);
        x.set32(0x10, 0);
        x.anim_step();
    }

    fn leave(x: &mut Cx) {
        if x.u8(0x5) != 0 {
            return x.call(sub::FREE_SOBJ);
        }
        x.set16(0x4, 0x200);
        load_palette(x);
    }
}

/// Repère attaché (type 8) : un petit marqueur (animation 0x13b ou 0x13c selon `+3`) collé à
/// l'objet `+0x14`, un peu au-dessus (0x20 ou 0x10) ; il disparaît quand il sort à gauche de l'écran.
mod marker {
    use super::*;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let anim = if x.u8(0x3) == 0 { 0x13b } else { 0x13c };
                x.anim_start(ANIMS, anim);
                track(x)
            }
            2 => track(x),
            _ => x.call(sub::FREE_SOBJ),
        }
    }

    fn track(x: &mut Cx) {
        let target = x.linked(0x14);
        for off in [0x8, 0xc, 0x10] {
            let v = target.u16(x.b, off);
            x.set16(off, v);
        }
        let lift = if x.u8(0x3) == 0 { 0x20 } else { 0x10 };
        x.add16(0xc, 0u16.wrapping_sub(lift));
        x.add16(0x10, 1);
        let limit = x.a5_u16(ram::CAMERA_X).wrapping_sub(0x40) as i16;
        if limit > x.u16(0x8) as i16 {
            return x.call(sub::FREE_SOBJ);
        }
        x.anim_step();
        x.reinsert();
    }
}

/// Cadre clignotant (type 9) : 8 clignotements de 20 images, placé d'après une table (par `+3`)
/// relative à la caméra, ou (si `+0x2f`) sur le premier joueur à une profondeur donnée.
mod frame {
    use super::*;

    const ANIM_BY_VARIANT: u32 = 0xdf80a;
    const OFFSETS: u32 = 0xdf87a;
    const BLINK_FRAMES: u16 = 0x14;
    const BLINKS: u8 = 8;
    /// Mot de la RAM (A5+0x46) : joueur suivi.
    const TRACKED: u32 = 0x46;

    pub fn run(x: &mut Cx) {
        if x.a5_u8(0x42) != 0 {
            x.set32(0x4, 0x0600_0000);
        }
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x14, BLINK_FRAMES);
                x.set8(0x16, BLINKS);
                place(x);
                let variant = x.u8(0x3) as u32;
                let anim = x.rom16(ANIM_BY_VARIANT + 2 * variant) as u32;
                x.anim_start(ANIMS, anim);
                x.reinsert();
            }
            2 => {
                place(x);
                let n = x.u16(0x14).wrapping_sub(1);
                x.set16(0x14, n);
                if (n as i16) >= 0 {
                    return;
                }
                let v = x.u8(0x1) ^ 2;
                x.set8(0x1, v);
                x.set16(0x14, BLINK_FRAMES);
                let n = x.u8(0x16).wrapping_sub(1);
                x.set8(0x16, n);
                if (n as i8) < 0 {
                    x.set8(0x4, 4);
                }
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }

    /// Position : avec `+0x2f`, ne reste que si le joueur suivi est dans la zone de profondeur
    /// 0x170..0x260 ; sinon, caméra + décalage de la table.
    fn place(x: &mut Cx) {
        if x.u8(0x2f) != 0 {
            let who = x.a5_u16(TRACKED);
            let ok = who != 0 && {
                let depth = Obj(who as i16 as i32 as u32 & 0xff_ffff).u16(x.b, 0x10) as i16;
                depth < 0x260 && depth >= 0x170
            };
            if !ok {
                return x.set16(0x4, LEAVE_6_0);
            }
        }
        let row = OFFSETS + 4 * x.u8(0x3) as u32;
        let dx = x.rom16(row);
        let dz = x.rom16(row + 2);
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(dx);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(dz);
        x.set16(0xc, v);
        x.set32(0x10, 0);
    }
}

/// Flèche de conseil (type 10) : posée sur la caméra, clignote 10 fois (20 images) ; son animation
/// et les événements donnés (4 appels à 0x19ae6, avec un code par joueur) viennent du numéro de
/// conseil de l'objet lié (`+0x14` de la bulle, 0x118...). Naît à la fin d'une bulle (type 7).
mod arrow {
    use super::*;

    const ANIM_BY_ADVICE: u32 = 0xdf9ae;
    const EVENT_BY_ADVICE: u32 = 0xdfa0e;
    const SCORE_EVENT: u32 = 0x19ae6;
    const LAST_ADVICE: u16 = 0x136;
    const BLINK_FRAMES: u16 = 0x14;
    const BLINKS: u8 = 0xa;

    pub fn run(x: &mut Cx) {
        if x.a5_u8(0x42) != 0 {
            x.set32(0x4, 0x0600_0000);
        }
        if suspended(x) {
            // une flèche cachée ou suspendue disparaît
            return x.set32(0x4, 0x0600_0000);
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => step(x),
            _ => x.call(sub::FREE_SOBJ),
        }
    }

    fn advice_of_owner(x: &mut Cx) -> u16 {
        let owner = x.linked(0x46);
        owner.u16(x.b, 0x14)
    }

    fn start(x: &mut Cx) {
        if x.u16(0x46) == 0 {
            return;
        }
        let advice = advice_of_owner(x);
        if advice == LAST_ADVICE {
            return x.set8(0x4, 4);
        }
        x.add8(0x4, 2);
        x.set16(0x16, BLINK_FRAMES);
        x.set8(0x18, BLINKS);
        place(x, 0xa7f);
        announce(x);
        pick_anim(x);
        x.reinsert();
    }

    fn place(x: &mut Cx, dz: u16) {
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0x58);
        x.set16(0x8, v);
        let v = x.a5_u16(ram::CAMERA_Y).wrapping_add(dz);
        x.set16(0xc, v);
        x.set32(0x10, 0);
    }

    /// Quatre événements consécutifs (codes +0x100 par joueur).
    fn announce(x: &mut Cx) {
        let advice = advice_of_owner(x);
        let i = advice.wrapping_sub(0x118).wrapping_mul(2);
        let code = x.rom16(EVENT_BY_ADVICE.wrapping_add(i as i16 as i32 as u32));
        x.c.d[0] = (x.c.d[0] & 0xffff_0000) | code as u32;
        for _ in 0..4 {
            x.call(SCORE_EVENT);
            let next = (x.c.d[0] as u16).wrapping_add(0x100);
            x.c.d[0] = (x.c.d[0] & 0xffff_0000) | next as u32;
        }
    }

    fn pick_anim(x: &mut Cx) {
        if x.u16(0x46) == 0 {
            return;
        }
        let advice = advice_of_owner(x);
        let i = advice.wrapping_sub(0x118).wrapping_mul(2);
        let anim = x.rom16(ANIM_BY_ADVICE.wrapping_add(i as i16 as i32 as u32)) as u32;
        x.anim_start(ANIMS, anim);
    }

    fn step(x: &mut Cx) {
        let n = x.u16(0x16).wrapping_sub(1);
        x.set16(0x16, n);
        if (n as i16) < 0 {
            let v = x.u8(0x1) ^ 2;
            x.set8(0x1, v);
            x.set16(0x16, BLINK_FRAMES);
            let n = x.u8(0x18).wrapping_sub(1);
            x.set8(0x18, n);
            if (n as i8) < 0 {
                return x.set8(0x4, 4);
            }
        }
        place(x, 0xaf);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 11, 12 : éléments de l'interface collés à la caméra
// ---------------------------------------------------------------------------------------------

/// Adresse (mot) de la structure de caméra dans la RAM : A5+0x4fe, ou A5+0x54e si `A5+0x36`.
fn camera_record(x: &mut Cx) -> u16 {
    let record = if x.a5_u8(ram::USE_CAMERA2) != 0 { 0x54e } else { 0x4fe };
    (0xff_8000u32 + record) as u16
}

/// Place l'objet à la position de la structure de caméra `+0x42`, décalée de (`+0x14`, `+0x18`).
fn stick_to_camera(x: &mut Cx) {
    let camera = x.linked(0x42);
    let v = camera.u16(x.b, 0x8).wrapping_add(x.u16(0x14));
    x.set16(0x8, v);
    let v = camera.u16(x.b, 0xc).wrapping_add(x.u16(0x18));
    x.set16(0xc, v);
}

/// Marque de l'interface (type 11) : deux variantes. La variante 0 (la bulle) entre par le bas
/// (Z 0x100 -> 0x80, 3 par image), crée une variante 1 (le doigt, avec son paramètre `+0x2c`),
/// attend qu'elle ait fini puis ressort (Z -> 0x110). La variante 1 se colle à la bulle et suit le
/// compteur `+0x33` du propriétaire `+0x46` (clignote, sons, prévient `+0x44` quand il a épuisé
/// ses essais).
mod badge {
    use super::*;

    const KILL: u32 = 0x0600_0000;
    const BADGE_VARIANT: u8 = 1;
    const SOUND: u32 = 0xea428;

    pub fn run(x: &mut Cx) {
        if x.u8(0x3) == 0 {
            parent(x)
        } else {
            child(x)
        }
    }

    /// Crée un objet SOBJ de type 11 de variante `variant` et de paramètre `param` (`+0x2c`).
    fn spawn(x: &mut Cx, variant: u8, param: u8) -> Option<Obj> {
        x.call(sub::ALLOC_SOBJ);
        if x.c.sr & m68k::SR_C != 0 {
            return None;
        }
        let o = Obj(x.c.a[1]);
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, 2, 0xb);
        o.set_u8(x.b, 3, variant);
        o.set_u8(x.b, 0x2c, param);
        Some(o)
    }

    fn parent(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x18, 0x100);
                x.set16(0x14, 0x158);
                if x.u8(0x2c) != 0 {
                    x.set16(0x14, 0x28);
                }
                x.set32(0x10, 0xffff_ffff);
                let camera = camera_record(x);
                x.set16(0x42, camera);
                stick_to_camera(x);
                x.anim_start(ANIMS, 0x15c);
                x.reinsert();
            }
            2 => match x.u8(0x5) {
                0 => {
                    let z = x.u16(0x18).wrapping_sub(3);
                    x.set16(0x18, z);
                    if z as i16 > 0x80 {
                        return stick_to_camera(x);
                    }
                    x.set16(0x18, 0x80);
                    let param = x.u8(0x2c);
                    match spawn(x, BADGE_VARIANT, param) {
                        Some(child) => {
                            let me = x.o.0 as u16;
                            child.set_u16(x.b, 0x46, me);
                            x.set8(0x48, 0);
                        }
                        None => x.set8(0x48, 0x78),
                    }
                    x.add8(0x5, 2);
                    stick_to_camera(x);
                    x.set8(0x44, 0);
                    x.anim_start(ANIMS, 0x15d);
                }
                2 => {
                    stick_to_camera(x);
                    let wait = x.u8(0x48);
                    if wait != 0 {
                        let n = wait - 1;
                        x.set8(0x48, n);
                        if n == 0 {
                            return back_to_idle(x);
                        }
                    } else if x.u8(0x44) != 0 {
                        return back_to_idle(x);
                    }
                    x.anim_step();
                }
                4 => {
                    let z = x.u16(0x18).wrapping_add(3);
                    x.set16(0x18, z);
                    if (z as i16) < 0x110 {
                        return stick_to_camera(x);
                    }
                    x.add8(0x4, 2);
                }
                _ => {}
            },
            4 => {
                x.set32(0x4, KILL);
                x.call(sub::FREE_SOBJ);
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }

    fn back_to_idle(x: &mut Cx) {
        x.add8(0x5, 2);
        x.anim_start(ANIMS, 0x15c);
    }

    fn child(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.linked(0x46);
                let counter = owner.u8(x.b, 0x33);
                x.set8(0x49, counter);
                x.set_bit(0x1, 1, true);
                x.set8(0x48, 3);
                x.set8(0x44, 0x14);
                x.set8(0x45, 1);
                x.set16(0x18, 0x78);
                x.set16(0x14, 0x152);
                if x.u8(0x2c) != 0 {
                    x.set8(0x2c, 0);
                    x.set16(0x14, 0x2e);
                }
                x.set32(0x10, 0xffff_fffe);
                let camera = camera_record(x);
                x.set16(0x42, camera);
                stick_to_camera(x);
                x.anim_start(ANIMS, 0x15b);
                x.reinsert();
            }
            2 => {
                stick_to_camera(x);
                let owner = x.linked(0x46);
                let counter = owner.u8(x.b, 0x33);
                if counter == x.u8(0x49) {
                    return;
                }
                x.set8(0x49, counter);
                if counter != 0 {
                    x.set_bit(0x1, 1, false);
                    return x.call(SOUND);
                }
                x.set_bit(0x1, 1, true);
                let n = x.u8(0x48).wrapping_sub(1);
                x.set8(0x48, n);
                if n != 0 {
                    return;
                }
                owner.set_u8(x.b, 0x44, 1);
                x.set32(0x4, KILL);
                x.call(sub::FREE_SOBJ);
            }
            4 => {
                x.set32(0x4, KILL);
                x.call(sub::FREE_SOBJ);
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }
}

/// Pastille d'interface (type 12) : collée à la caméra (Z + 0x30), animation 0x1ab + variante.
mod hud_dot {
    use super::*;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x18, 0x30);
                x.set32(0x10, 0);
                let camera = camera_record(x);
                x.set16(0x42, camera);
                stick_to_camera(x);
                // `move.w #0x1ab,D0w ; add.b (3,A0),D0b` : l'addition ne déborde pas sur l'octet haut
                let n = 0x100 | (0xab + x.u8(0x3) as u32) & 0xff;
                x.anim_start(ANIMS, n);
                x.reinsert();
            }
            2 => stick_to_camera(x),
            4 => {
                x.set32(0x4, 0x0600_0000);
                x.call(sub::FREE_SOBJ);
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }
}

/// Panneau défilant (type 13) : naît à droite de la caméra (X + 0x20, Z 0x1c8, profondeur -0x100),
/// avance avec elle jusqu'à la fin de son animation (script 0x3b) puis ordonne aux quatre joueurs
/// enregistrés (0xff2aa4...) de passer à l'état 4 et se libère.
mod panel {
    use super::*;

    const PALETTE: u32 = 0x28ff20;
    const PALETTE_DST: u32 = 0x91c3c0;
    const SLIDE_ANIMS: u32 = 0x1c40c6;

    pub fn run(x: &mut Cx) {
        if x.u8(0x4) == 0 {
            x.add8(0x4, 2);
            copy_longs(x, PALETTE, PALETTE_DST, 8);
            or_longs(x, PALETTE_DST, BRIGHT, 8);
            x.set8(0x2b, 0x1e);
            x.set8(0x2a, 1);
            let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0x20);
            x.set16(0x8, v);
            x.set16(0xc, 0x1c8);
            x.set16(0x10, 0xff00);
            x.anim_start(SLIDE_ANIMS, 0x3b);
            x.reinsert();
        }
        let d = x.a5_u16(ram::CAMERA_X).wrapping_sub(x.a5_u16(ram::PREV_CAMERA_X));
        x.add16(0x8, d);
        if !x.anim_step() {
            return;
        }
        for i in 0..4 {
            let at = PLAYERS + 2 * i;
            let who = rd16(x.b, at);
            if who != 0 {
                wr16(x.b, who as i16 as i32 as u32 + 4, 0x400);
                wr16(x.b, at, 0);
            }
        }
        x.call(sub::FREE_SOBJ);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 14 : titre de niveau
// ---------------------------------------------------------------------------------------------

/// Titre de niveau (type 14) : bandeau animé (jeu 0x1c03a6, numéro tiré de la table 0xdff0c selon
/// le niveau `A5+0x34`) collé à la caméra, dont la palette (0x91c1e0) s'éclaircit par pas de 4
/// images en 16 étapes lues dans 0x291f00 ; il dure 0xb4 images. Les niveaux 7 et 0x2b ont leur
/// propre déroulement.
mod stage_title {
    use super::*;

    const ANIM_BY_STAGE: u32 = 0xdff0c;
    const FADE: u32 = 0x291f00;
    const FADE_OUT_FINAL: u32 = 0x292120;
    const PALETTE_DST: u32 = 0x91c1e0;
    const DURATION: u16 = 0xb4;
    const STEP_FRAMES: u16 = 4;
    const STEPS: u16 = 0x10;
    const STAGE_7: u8 = 7;
    const STAGE_2B: u8 = 0x2b;
    /// Scripts d'animation directs du niveau 0x2b (normal, avec `A5+0x70`).
    const SCRIPT_2B: u32 = 0xe0152;
    const SCRIPT_2B_ALT: u32 = 0xe01c6;
    /// `A5+0x70` : jeu à deux joueurs (h), décale le titre.
    const ALT: u32 = 0x70;

    pub fn run(x: &mut Cx) {
        if x.a5_u8(0x48f) != 0 {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => step(x),
            4 => {
                if x.a5_u8(0x34) == STAGE_2B {
                    x.set_a5_u32(0x5c6, 0);
                }
                x.call(sub::FREE_SOBJ)
            }
            _ => x.call(sub::FREE_SOBJ),
        }
    }

    fn fade_to(x: &mut Cx, step: u16) {
        let from = FADE + ((step as u32) << 5 & 0xffff);
        copy_longs(x, from, PALETTE_DST, 8);
    }

    fn start(x: &mut Cx) {
        if x.a5_u8(0x30) == 0 {
            return x.call(sub::FREE_SOBJ);
        }
        let stage = x.a5_u8(0x34);
        if stage == STAGE_2B {
            return start_2b(x);
        }
        if x.a5_u8(0x4e) != 0 && stage == 0x1c {
            return x.call(sub::FREE_SOBJ);
        }
        x.add8(0x4, 2);
        x.set16(0x14, DURATION);
        x.set16(0x18, STEP_FRAMES);
        x.set16(0x1c, STEPS);
        fade_to(x, STEPS);
        let mut anim = x.rom16(ANIM_BY_STAGE + 2 * stage as u32);
        if x.a5_u16(ALT) != 0 {
            anim = anim.wrapping_add(0x10d);
        }
        x.anim_start(ANIMS, anim as u32);
        place(x);
    }

    fn start_2b(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set16(0x14, DURATION);
        x.set16(0x18, STEP_FRAMES);
        x.set16(0x1c, STEPS);
        fade_to(x, STEPS);
        x.c.a[4] = if x.a5_u16(ALT) != 0 { SCRIPT_2B_ALT } else { SCRIPT_2B };
        x.call(0x1a3c);
        place(x);
    }

    /// Titre à (X caméra + 0x44, Z caméra + 0xa8) ; un peu plus à gauche à deux joueurs.
    fn place(x: &mut Cx) {
        let camera = Obj(0xff_0000 | camera_record(x) as u32);
        let v = camera.u16(x.b, 0x8);
        x.set16(0x8, v);
        let v = camera.u16(x.b, 0xc);
        x.set16(0xc, v);
        x.set16(0x10, 0);
        x.add16(0x8, 0x44);
        if x.a5_u16(ALT) != 0 {
            x.add16(0x8, 0u16.wrapping_sub(0xc));
        }
        x.add16(0xc, 0xa8);
    }

    fn step(x: &mut Cx) {
        let stage = x.a5_u8(0x34);
        if stage == STAGE_7 && x.u8(0x5) == 0 {
            return step_stage_7(x);
        }
        if stage == STAGE_2B {
            return step_stage_2b(x);
        }
        place(x);
        x.anim_step();
        if tick_fade(x) {
            return finish_tick(x);
        }
        finish_tick(x)
    }

    /// Un pas du fondu : toutes les 4 images, palette suivante ; à l'étape 0 le compteur reste
    /// bloqué à 0xffff.
    fn tick_fade(x: &mut Cx) -> bool {
        let n = x.u16(0x18).wrapping_sub(1);
        x.set16(0x18, n);
        if n != 0 {
            return false;
        }
        let s = x.u16(0x1c).wrapping_sub(1);
        x.set16(0x1c, s);
        if s != 0 {
            x.set16(0x18, STEP_FRAMES);
        } else {
            x.set16(0x18, 0xffff);
        }
        let s = x.u16(0x1c);
        fade_to(x, s);
        true
    }

    /// Décompte de la durée du titre ; à zéro il passe à l'état 4. Remise en place dans la liste.
    fn finish_tick(x: &mut Cx) {
        let n = x.u16(0x14).wrapping_sub(1);
        x.set16(0x14, n);
        if n == 0 {
            x.add8(0x4, 2);
        }
        x.reinsert();
    }

    /// Niveau 0x2b : comme le fondu normal mais sans avancer l'animation.
    fn step_stage_2b(x: &mut Cx) {
        tick_fade(x);
        place(x);
        finish_tick(x);
    }

    /// Niveau 7, sous-état 0 : le fondu dure jusqu'à l'étape négative, où la palette finale
    /// (0x292120) est posée et le sous-état passe à 2.
    fn step_stage_7(x: &mut Cx) {
        place(x);
        let n = x.u16(0x18).wrapping_sub(1);
        x.set16(0x18, n);
        if n == 0 {
            let s = x.u16(0x1c).wrapping_sub(1);
            x.set16(0x1c, s);
            if (s as i16) < 0 {
                x.set16(0x18, 0xffff);
                copy_longs(x, FADE_OUT_FINAL, PALETTE_DST, 8);
                x.add8(0x5, 2);
                return x.reinsert();
            }
            x.set16(0x18, STEP_FRAMES);
            fade_to(x, s);
        }
        finish_tick(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 15 : cible à frapper (h)
// ---------------------------------------------------------------------------------------------

/// Cible (type 15, variantes 0x25 et autres, rectangle `+0x16..+0x1c` autour de l'objet) : quand
/// un joueur la touche d'une attaque (`+0x58` du joueur positif), elle encaisse ; au troisième
/// coup (`+0x14`) elle lève un drapeau de la RAM (0xff2ac0 pour la variante 0x25, 0xff2ac1
/// sinon), joue un son et passe à l'état 4. `+0x15` mémorise qu'un joueur est dedans, `+0x1e`
/// le code (mot) mémorisé en 0xff2ab2 au premier contact.
mod target {
    use super::*;

    const RECTS: u32 = 0xe02c6;
    const HITS_TO_BREAK: u8 = 3;
    const SOUND: u32 = 0xea57e;
    const SHOOTER_CODE: u32 = 0xff_2ab2;
    const FLAG_A: u32 = 0xff_2ac0;
    const FLAG_B: u32 = 0xff_2ac1;
    const VARIANT_A: u8 = 0x25;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x14, 0);
                let row = if x.u8(0x3) == VARIANT_A { 0 } else { 1 };
                let rect = RECTS + 8 * row;
                for i in 0..4 {
                    let v = x.data16(rect + 2 * i);
                    x.set16(0x16 + 2 * i, v);
                }
                let v = x.u16(0x8);
                x.set16(0xa, v);
                let v = x.u16(0x10);
                x.set16(0x12, v);
                let variant = x.u8(0x3) as u32;
                x.anim_start(0x1c40c6, variant);
                x.reinsert();
            }
            2 => {
                follow_camera2(x);
                let y = x.u16(0x12).wrapping_sub(cam2_dy(x));
                x.set16(0x10, y);
                x.set8(0x15, 0);
                for i in 0..4 {
                    let who = rd16(x.b, PLAYERS + 2 * i);
                    if who != 0 {
                        probe(x, Obj(who as i16 as i32 as u32 & 0xff_ffff));
                    }
                }
                follow_camera2(x);
            }
            4 => follow_camera2(x),
            _ => x.call(sub::FREE_SOBJ),
        }
    }

    fn cam2_dx(x: &mut Cx) -> u16 {
        x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X))
    }

    fn cam2_dy(x: &mut Cx) -> u16 {
        x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y))
    }

    fn follow_camera2(x: &mut Cx) {
        let v = x.u16(0xa).wrapping_sub(cam2_dx(x));
        x.set16(0x8, v);
    }

    /// Le joueur `p` est-il dans le rectangle ? Si oui, `+0x15` = 1 ; une attaque en cours
    /// (`+0x58` > 0) compte un coup.
    fn probe(x: &mut Cx, p: Obj) {
        let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(x.u16(0x16));
        if dx > x.u16(0x18) {
            return;
        }
        let dy = x
            .u16(0xc)
            .wrapping_add(x.u16(0x10))
            .wrapping_sub(p.u16(x.b, 0xc))
            .wrapping_sub(p.u16(x.b, 0x10))
            .wrapping_add(x.u16(0x1a));
        if dy > x.u16(0x1c) {
            return;
        }
        x.set8(0x15, 1);
        if x.u8(0x7) == 0 {
            let code = x.u16(0x1e);
            if code != 0 {
                wr16(x.b, SHOOTER_CODE, code);
                x.set8(0x7, 1);
            }
        }
        if (p.u8(x.b, 0x58) as i8) > 0 {
            hit(x, p);
        }
    }

    fn hit(x: &mut Cx, p: Obj) {
        p.set_u8(x.b, 0x58, 0xff);
        let n = x.u8(0x14) + 1;
        x.set8(0x14, n);
        if n < HITS_TO_BREAK {
            return;
        }
        x.call(SOUND);
        let flag = if x.u8(0x3) == VARIANT_A { FLAG_A } else { FLAG_B };
        wr8(x.b, flag, 1);
        x.add8(0x4, 2);
    }
}

handlers! {
    0xdd61e "sobj_00" 600 scenery::run,
    0xddc28 "sobj_01" 400 fixture::run,
    0xdddb2 "sobj_02" 400 flasher::run,
    0xdde18 "sobj_03" 300 tag::run,
    0xdde8c "sobj_05" 100 ghost::run,
    0xddec4 "sobj_06" 400 scene::run,
    0xdf4d4 "sobj_07" 400 hint::run,
    0xdf6b0 "sobj_08" 300 marker::run,
    0xdf774 "sobj_09" 300 frame::run,
    0xdf8ae "sobj_10" 300 arrow::run,
    0xdfa68 "sobj_11" 400 badge::run,
    0xdfca0 "sobj_12" 200 hud_dot::run,
    0xdfd44 "sobj_13" 400 panel::run,
    0xdfe3c "sobj_14" 500 stage_title::run,
    0xe0258 "sobj_15" 800 target::run,
}
