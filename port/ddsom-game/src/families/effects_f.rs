//! Famille EFFCT (suite) : types de scène réécrits après `effects_c.rs` (table des types `0x39ce0`).
//!
//! - 0 : décor qui glisse vers sa place à l'ouverture d'une scène, puis vide la scène (variante 3 :
//!   attend, fait défiler la caméra et s'efface) ;
//! - 35 : silhouette (variante 0) et reflet (variante 1) d'un porteur (`+0x46`) : la silhouette s'anime
//!   et transmet son image au porteur (`+0xa0`), que le reflet traduit en animation ;
//! - 53 : objet qui traverse l'écran en cloche, puis déclenche un décor, attend qu'il ait fini et crée
//!   le type 55 ;
//! - 54 : trois pièces d'une scène (la variante 0 ouvre le passage : fond, joueurs et caméra déplacés,
//!   tâche de fondu, puis un boss) ;
//! - 55 : machine qui, son délai écoulé, projette les joueurs, sauve la palette et lance la transition ;
//! - 68 : enseigne de départ, posée sur la caméra, qui libère les joueurs à la fin de son animation ;
//! - 77 : fondu sonore (deux volumes qui montent par paliers, puis arrêt) ;
//! - 89 : personnage qui entre par la gauche, attend un signal de la scène et ressort par la droite.

use super::cx::{handlers, Cx};
use crate::mem::{rd32, wr32, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;

/// Champs partagés.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const FLAGS: u32 = 0x01;
    pub const FACING: u32 = 0x2c;
}

/// Routines d'origine.
mod rom {
}

/// Caméra (x, profondeur) et caméra secondaire.
const CAMERA_X: u32 = 0x506;
const CAMERA_Y: u32 = 0x50a;
/// Étape de la scène, avancée par certains effets (`A5+0x4ff`).
const SCENE_STEP: u32 = 0x4ff;
/// Objet que la scène suit (`A5+0x46`, mot).
const FOCUS: u32 = 0x46;

/// Joueurs : quatre enregistrements de 0x200 octets.
const PLAYERS: u32 = A5 + 0x5de;

/// Joueurs présents et vivants.
fn living_players(x: &mut Cx) -> Vec<Obj> {
    (0..4)
        .map(|k| Obj(PLAYERS + 0x200 * k))
        .filter(|p| p.u8(x.b, 0) != 0 && p.u16(x.b, 0x62) != 0)
        .collect()
}

fn free(x: &mut Cx) {
    x.free(&pools::EFFCT);
}

/// Pose l'objet sur la caméra, décalé de (dx, dy), au sol.
fn at_camera(x: &mut Cx, dx: u16, dy: u16) {
    let (cx, cy) = (x.a5_u16(CAMERA_X), x.a5_u16(CAMERA_Y));
    x.set16(Obj::X, cx.wrapping_add(dx));
    x.set16(Obj::Y, cy.wrapping_add(dy));
}

/// Copie huit longs de couleurs de la ROM dans la palette, avec l'opacité forcée (`| 0xf000f000`).
fn load_palette(x: &mut Cx, from: u32, to: u32) {
    for k in 0..8 {
        let v = rd32(x.b, from + 4 * k);
        wr32(x.b, to + 4 * k, v | 0xf000_f000);
    }
}

/// Lance la tâche `program` (0x9b6) avec le paramètre D0.w = `param`.
fn new_task(x: &mut Cx, program: u32, param: u16) {
    crate::kernel::creer_tache(x.b, param as u32, program);
}

// ---------------------------------------------------------------------------------------------
// Type 0 : décor d'ouverture de scène
// ---------------------------------------------------------------------------------------------

mod opening {
    pub const ANIM_SET: u32 = 0x18c04c;
    /// Position de départ (x, profondeur) et abscisse d'arrivée (`+0x18`) par variante (mots,
    /// espace programme).
    pub const STARTS: u32 = 0xa8602;
    pub const TARGETS: u32 = 0xa8612;
    pub const TARGET: u32 = 0x18;
    /// Variante 3 : délai (`+0x1a`) avant de faire défiler la scène.
    pub const DELAY: u32 = 0x1a;
    /// Couleurs de la variante 3.
    pub const PALETTE: u32 = 0x28dca0;
    pub const PALETTE_RAM: u32 = 0x91c3a0;
    /// Pools vidés à l'arrivée : 32 objets à `A5+0x62de` (sauf ceux marqués `+0x58`), 18 à `A5+0x10de`.
    pub const POOL_A: u32 = 0x62de;
    pub const POOL_B: u32 = 0x10de;
    pub const SOUND_ARRIVAL: u32 = 0xeaa98;
    pub const SOUND_SCROLL: u32 = 0xedc66;
    /// La variante 3 attend que la caméra soit revenue à cette abscisse.
    pub const CAMERA_STOP: i16 = 0x5cc;
}

fn opening(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => opening_init(x),
        2 => match x.u8(f::SUB_STATE) {
            0 => opening_slide(x),
            4 => opening_wait(x),
            8 => {
                x.add16(Obj::X, 0xfffe);
                if x.a5_u16(CAMERA_X) as i16 > opening::CAMERA_STOP {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.call(opening::SOUND_SCROLL);
            }
            12 => {
                if !x.anim_step() {
                    return;
                }
                x.call(opening::SOUND_ARRIVAL);
                let step = x.a5_u8(SCENE_STEP).wrapping_add(4);
                x.set_a5_u8(SCENE_STEP, step);
                x.add8(f::STATE, 4);
            }
            _ => {}
        },
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::STATE, 2);
            let step = x.a5_u8(SCENE_STEP).wrapping_add(2);
            x.set_a5_u8(SCENE_STEP, step);
        }
        _ => free(x),
    }
}

fn opening_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.reinsert_foreground();
    let v = x.u8(f::VARIANT) as u32;
    let target = x.rom16(opening::TARGETS + 2 * v);
    x.set16(opening::TARGET, target);
    let (px, py) = (x.rom16(opening::STARTS + 4 * v), x.rom16(opening::STARTS + 4 * v + 2));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set16(Obj::Z, 0);
    if v == 3 {
        x.set8(0x2b, 0x1d);
        x.set8(0x2a, 1);
        load_palette(x, opening::PALETTE, opening::PALETTE_RAM);
        x.anim_start(opening::ANIM_SET, 0x20);
        return;
    }
    // son décor d'accompagnement (SOBJ 3), qui garde un lien long vers l'effet
    if let Some(o) = x.alloc(&pools::SOBJ) {
        o.set_u8(x.b, 0, 1);
        o.set_u8(x.b, 2, 3);
        o.set_u32(x.b, 0x14, x.o.0);
    }
    x.anim_start(opening::ANIM_SET, 0x1f);
}

/// Glisse vers la gauche de deux pixels par image jusqu'à l'abscisse d'arrivée.
fn opening_slide(x: &mut Cx) {
    x.add16(Obj::X, 0xfffe);
    let target = x.u16(opening::TARGET);
    if (target as i16) < (x.u16(Obj::X) as i16) {
        return;
    }
    x.set16(Obj::X, target);
    if x.u8(f::VARIANT) == 3 {
        x.add8(f::SUB_STATE, 4);
        x.set16(opening::DELAY, 0x40);
        return opening_wait(x);
    }
    x.add8(f::STATE, 2);
    clear_scene(x);
    x.call(opening::SOUND_ARRIVAL);
    crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::C, 0x20);
}

/// Met fin (état 6) aux objets des deux pools. Comme l'original, un objet du premier pool marqué
/// `+0x58` fait finir le parcours avec le corps de la seconde boucle, sur les emplacements restants du
/// premier pool : le second n'est alors pas parcouru.
fn clear_scene(x: &mut Cx) {
    let (mut slot, mut left) = (A5 + opening::POOL_A, 32);
    let mut spared = false;
    while left > 0 {
        let o = Obj(slot);
        if o.u8(x.b, 0) != 0 {
            if !spared && o.u8(x.b, 0x58) != 0 {
                spared = true;
            } else {
                o.set_u16(x.b, f::STATE, 0x600);
            }
        }
        slot += 0x60;
        left -= 1;
    }
    if spared {
        return;
    }
    for k in 0..18 {
        let o = Obj(A5 + opening::POOL_B + 0x60 * k);
        if o.u8(x.b, 0) != 0 {
            o.set_u16(x.b, f::STATE, 0x600);
        }
    }
}

/// Variante 3 : délai, puis la scène avance (`A5+0x4ff` + 4).
fn opening_wait(x: &mut Cx) {
    let n = x.u16(opening::DELAY).wrapping_sub(1);
    x.set16(opening::DELAY, n);
    if (n as i16) >= 0 {
        return;
    }
    x.add8(f::SUB_STATE, 4);
    let step = x.a5_u8(SCENE_STEP).wrapping_add(4);
    x.set_a5_u8(SCENE_STEP, step);
}

// ---------------------------------------------------------------------------------------------
// Type 35 : silhouette et reflet d'un porteur
// ---------------------------------------------------------------------------------------------

mod mirrored {
    pub const ANIM_SET: u32 = 0x174776;
    pub const BEARER: u32 = 0x46;
    /// Image courante, transmise par la silhouette au porteur (`+0xa0` du porteur ; négative : rien
    /// à montrer).
    pub const FRAME: u32 = 0x33;
    pub const BEARER_FRAME: u32 = 0xa0;
    /// Animation du reflet par image de la silhouette (octets, espace programme ; négatif : caché).
    pub const REFLECTIONS: u32 = 0xb300c;
}

fn mirrored(x: &mut Cx) {
    if x.suspended() {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set_bit(f::FLAGS, 1, true);
            x.anim_start(mirrored::ANIM_SET, 4);
        }
        2 => mirrored_follow(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

fn mirrored_follow(x: &mut Cx) {
    let pointer = x.u16(mirrored::BEARER);
    let bearer = x.linked(mirrored::BEARER);
    if (pointer as i16) >= 0 || bearer.u8(x.b, 0) == 0 {
        // porteur disparu
        return x.add8(f::STATE, 2);
    }
    let frame = bearer.u8(x.b, mirrored::BEARER_FRAME);
    if (frame as i8) < 0 {
        return mirrored_hide(x);
    }
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = bearer.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = bearer.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    if x.u8(f::SUB_STATE) == 0 {
        x.add8(f::SUB_STATE, 2);
        x.set_bit(f::FLAGS, 1, false);
        if x.u8(f::VARIANT) == 0 {
            x.anim_start(mirrored::ANIM_SET, 4);
        }
    }
    if x.u8(f::VARIANT) == 0 {
        // silhouette : un pixel devant et au-dessus du porteur
        x.add16(Obj::Y, 1);
        x.add16(Obj::Z, 0xffff);
        if x.a5_u8(0x3d) == 0 {
            // le porteur est dans A1, que l'original relit après le pas d'animation
            x.c.a[1] = bearer.0;
            x.anim_step();
            let frame = x.u8(mirrored::FRAME);
            Obj(x.c.a[1]).set_u8(x.b, mirrored::BEARER_FRAME, frame);
        }
    } else {
        // reflet : un pixel derrière et au-dessous
        x.add16(Obj::Y, 0xffff);
        x.add16(Obj::Z, 1);
        if frame != x.u8(mirrored::FRAME) {
            let anim = x.rom8(mirrored::REFLECTIONS + frame as u32);
            if (anim as i8) < 0 {
                return mirrored_hide(x);
            }
            x.anim_start(mirrored::ANIM_SET, (anim as u32) as u16);
            x.set_bit(f::FLAGS, 1, false);
        }
    }
    x.reinsert();
}

fn mirrored_hide(x: &mut Cx) {
    x.set8(f::SUB_STATE, 0);
    x.set_bit(f::FLAGS, 1, true);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 53 : objet qui traverse l'écran, puis déclenche la suite de la scène
// ---------------------------------------------------------------------------------------------

mod crossing {
    pub const ANIM_SET: u32 = 0x19685a;
    pub const TIMER: u32 = 0x42;
    /// Décor créé (pointeur) et son numéro de décor de scène.
    pub const SCENERY: u32 = 0x5c;
    pub const SCENERY_KIND: u32 = 0x83;
    pub const SOUND_LAND: u32 = 0xe3a2e;
    pub const SOUND_WAIT: u32 = 0xe3a98;
    pub const SOUND_DONE: u32 = 0xe3b02;
}

fn crossing(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set_a5_u16(FOCUS, x.o.0 as u16);
            at_camera(x, 0x180, 0x50);
            x.set16(Obj::Z, 0);
            x.set16(Obj::VX, 0xfc00);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x300);
            x.set16(Obj::AZ, 0xffc0);
            x.anim_start(crossing::ANIM_SET, 0);
            x.reinsert();
        }
        2 => crossing_steps(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

/// Les étapes (sous-état de 4 en 4) s'enchaînent dans la même image quand l'une se termine.
fn crossing_steps(x: &mut Cx) {
    let mut step = x.u8(f::SUB_STATE);
    loop {
        match step {
            // vol en cloche jusqu'au sol
            0 => {
                x.anim_step();
                if x.move_xz().pl() {
                    return;
                }
                next_step(x, 1, 0x18);
            }
            // glisse sur sa lancée
            4 => {
                x.anim_step();
                x.move_x();
                if !timer_out(x) {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.call(crossing::SOUND_LAND);
                x.anim_start(crossing::ANIM_SET, 3);
                x.reinsert();
            }
            8 => {
                if !x.anim_step() {
                    return;
                }
                next_step(x, 4, 0xc0);
            }
            12 => {
                x.anim_step();
                if !timer_out(x) {
                    return;
                }
                let Some(decor) = super::creators::scene_set(x.b, crossing::SCENERY_KIND as u8) else { return };
                x.set16(crossing::SCENERY, decor.0 as u16);
                x.add8(f::SUB_STATE, 4);
                x.set16(crossing::TIMER, 0);
                x.anim_start(crossing::ANIM_SET, 5);
            }
            // attend que le décor ait fini, en jouant un son toutes les 0x30 images
            16 => {
                x.anim_step();
                x.c.a[1] = x.linked(crossing::SCENERY).0;
                if timer_out(x) {
                    x.set16(crossing::TIMER, 0x30);
                    x.call(crossing::SOUND_WAIT);
                }
                if Obj(x.c.a[1]).u8(x.b, 0x22) == 0 {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.anim_start(crossing::ANIM_SET, 6);
            }
            20 => {
                if !x.anim_step() {
                    return;
                }
                x.call(crossing::SOUND_DONE);
                let o = x.o;
                let lanceur = super::effects_creators::launcher(x.b, o);
                if lanceur.is_none() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.anim_start(crossing::ANIM_SET, 7);
                return;
            }
            _ => return,
        }
        step += 4;
    }
}

/// Passe à l'étape suivante : délai, animation `anim`, remise en place dans la liste des acteurs.
fn next_step(x: &mut Cx, anim: u32, timer: u16) {
    x.add8(f::SUB_STATE, 4);
    x.set16(crossing::TIMER, timer);
    x.anim_start(crossing::ANIM_SET, anim as u16);
    x.reinsert();
}

/// `subq.w #1,(+0x42)` suivi de `bpl` : vrai quand le délai est épuisé (négatif).
fn timer_out(x: &mut Cx) -> bool {
    let n = x.u16(crossing::TIMER).wrapping_sub(1);
    x.set16(crossing::TIMER, n);
    (n as i16) < 0
}

// ---------------------------------------------------------------------------------------------
// Type 54 : pièces d'une scène qui ouvrent le passage vers un boss
// ---------------------------------------------------------------------------------------------

mod passage {
    pub const ANIM_SET: u32 = 0x196a28;
    /// Positions (x, profondeur) par variante (mots, espace programme).
    pub const PLACES: u32 = 0xb693c;
    /// Fondu de la scène (`A5+0x48e` : en cours tant que non nul) et son réglage.
    pub const FADE: u32 = 0x48e;
    pub const FADE_PROGRAM: u32 = 0x1ced4;
}

fn passage(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let (px, py) = (x.rom16(passage::PLACES + 4 * v), x.rom16(passage::PLACES + 4 * v + 2));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.set16(Obj::Z, 0);
            x.anim_start(passage::ANIM_SET, v as u16);
            x.reinsert();
        }
        2 => {
            // attend que l'objet suivi par la scène passe à l'état 4
            let focus = x.a5_u16(FOCUS);
            if focus == 0 {
                return;
            }
            let state = Obj(focus as i16 as i32 as u32 & 0xff_ffff).u8(x.b, f::STATE);
            if (state as i8) < 4 {
                x.anim_step();
                return;
            }
            if x.u8(f::VARIANT) != 0 {
                return free(x);
            }
            x.add8(f::STATE, 2);
            x.anim_start(passage::ANIM_SET, 3);
        }
        4 if x.u8(f::SUB_STATE) == 0 => passage_open(x),
        4 => passage_boss(x),
        _ => free(x),
    }
}

/// Ouvre le passage : la scène recule de 0x7c8 pixels (joueurs compris), puis un fondu est lancé.
fn passage_open(x: &mut Cx) {
    if x.a5_u8(passage::FADE) != 0 {
        return;
    }
    x.unlink();
    for p in living_players(x) {
        let (px, pz, py) = (p.u16(x.b, Obj::X), p.u16(x.b, Obj::Z), p.u16(x.b, Obj::Y));
        p.set_u16(x.b, Obj::X, px.wrapping_sub(0x7c8));
        p.set_u16(x.b, Obj::Z, pz.wrapping_sub(0x50));
        p.set_u16(x.b, Obj::Y, py.wrapping_add(0x50));
    }
    x.set_a5_u16(CAMERA_X, 0x4b0);
    x.set_a5_u16(0x52c, 0);
    x.set_a5_u16(0x490, 4);
    x.set_a5_u8(passage::FADE, 13);
    x.set_a5_u8(0x48f, 1);
    x.set_a5_u32(0x492, 0xff1190);
    x.set_a5_u32(0x49a, 0x266fe0);
    x.set_a5_u32(0x49e, 0x27cec0);
    new_task(x, passage::FADE_PROGRAM, 0x10);
    x.add8(f::SUB_STATE, 1);
}

/// Le fondu fini, crée le boss (type 7) devant la caméra et rend la main à la scène.
fn passage_boss(x: &mut Cx) {
    if x.a5_u8(passage::FADE) != 0 {
        return;
    }
    let Some(boss) = x.alloc(&pools::BOSS) else { return };
    boss.set_u8(x.b, 0, 1);
    boss.set_u8(x.b, 2, 7);
    let (cx, cy) = (x.a5_u16(CAMERA_X), x.a5_u16(CAMERA_Y));
    boss.set_u16(x.b, Obj::X, cx.wrapping_add(0xc0));
    boss.set_u16(x.b, Obj::Y, cy.wrapping_add(0x50));
    x.set_a5_u16(0x51e, 0);
    x.set_a5_u8(0x2e, 8);
    free(x);
}

// ---------------------------------------------------------------------------------------------
// Type 55 : machine qui projette les joueurs et lance la transition
// ---------------------------------------------------------------------------------------------

mod launcher {
    pub const ANIM_SET: u32 = 0x16ff94;
    pub const TIMER: u32 = 0x42;
    /// Objet prévenu au départ (passé à l'état 4).
    pub const PARTNER: u32 = 0x5c;
    pub const SOUND_START: u32 = 0xed474;
    pub const SOUND_SERVICE: u32 = 0xe0a10;
    pub const SOUND_LAUNCH: u32 = 0xedcd0;
    /// Sauvegarde de la palette (0x91c000, 33 × 32 octets) pour le fondu.
    pub const PALETTE_RAM: u32 = 0x91c000;
    pub const PALETTE_COPY: u32 = 0xff1190;
    pub const FADE_PROGRAM: u32 = 0x1ccf8;
}

fn launcher(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(launcher::TIMER, 0x40);
            x.call(launcher::SOUND_START);
            x.set16(Obj::X, 0xa00);
            x.set16(Obj::Y, 0x17c);
            x.anim_start(launcher::ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            let n = x.u16(launcher::TIMER).wrapping_sub(1);
            x.set16(launcher::TIMER, n);
            if (n as i16) >= 0 {
                return;
            }
            launch(x);
        }
        _ => free(x),
    }
}

fn launch(x: &mut Cx) {
    let partner = x.linked(launcher::PARTNER);
    partner.set_u16(x.b, f::STATE, 0x400);
    for k in 0..4 {
        let p = Obj(PLAYERS + 0x200 * k);
        if p.u8(x.b, 0) == 0 || p.u16(x.b, 0x62) == 0 {
            continue;
        }
        let px = p.u16(x.b, Obj::X).wrapping_add(0x310);
        p.set_u16(x.b, Obj::X, px);
        x.c.a[1] = p.0;
        x.call(launcher::SOUND_SERVICE);
        x.call(launcher::SOUND_LAUNCH);
        // animation 2 du joueur, puis le joueur est projeté en l'air
        let d6 = x.c.d[6];
        crate::objects::anim::start_by_character_68k(x.c, x.b, p, &crate::objects::anim::SETS_BY_VARIANT, 2);
        x.c.d[6] = d6;
        p.set_u16(x.b, Obj::VX, 0);
        p.set_u16(x.b, Obj::AX, 0);
        p.set_u16(x.b, Obj::VZ, 0x400);
        p.set_u16(x.b, Obj::AZ, 0xfff0);
        p.set_u16(x.b, Obj::Z, 0x50);
        let py = p.u16(x.b, Obj::Y).wrapping_sub(0x50);
        p.set_u16(x.b, Obj::Y, py);
        p.set_u32(x.b, f::STATE, 0x0008_040e);
    }
    for k in 0..33 * 8 {
        let v = rd32(x.b, launcher::PALETTE_RAM + 4 * k);
        wr32(x.b, launcher::PALETTE_COPY + 4 * k, v);
    }
    x.set_a5_u8(0x48f, 1);
    x.set_a5_u16(0x490, 1);
    x.set_a5_u8(0x48e, 13);
    new_task(x, launcher::FADE_PROGRAM, 0x10);
    x.set_a5_u16(CAMERA_X, 0xc78);
    // éclats (type 56) : dix-sept en principe (`dbf D6`), mais l'allocation se sert de D6 pour
    // parcourir le pool, si bien qu'en pratique la boucle remplit le pool
    // Glitch d'origine : la boucle compte dans D6 (`dbf`), que l'allocation réécrit (rang de l'emplacement trouvé
    // depuis la fin du pool) : elle s'arrête au pool plein, ou quand le dernier emplacement vient d'être pris.
    x.c.d[6] = 0x10;
    let o = x.o;
    let dernier = pools::EFFCT.slot(pools::EFFCT.slots - 1);
    while let Some(eclat) = super::effects_creators::regs_68k(x.c, x.b, o, super::effects_creators::CREATED, |c, b| super::effects_creators::camera_shard_par(c, b)) {
        let n = (x.c.d[6] as u16).wrapping_sub(1);
        x.c.d[6] = x.c.d[6] & 0xffff_0000 | n as u32;
        if eclat == dernier {
            break;
        }
    }
    free(x);
}

// ---------------------------------------------------------------------------------------------
// Type 68 : enseigne de départ
// ---------------------------------------------------------------------------------------------

mod start_sign {
    pub const ANIM_SET: u32 = 0x197ef0;
    pub const PALETTE: u32 = 0x28ff20;
    pub const PALETTE_RAM: u32 = 0x91c380;
    /// Demande de rechargement des couleurs.
    pub const RELOAD: u32 = 0x40;
}

fn start_sign_palette(x: &mut Cx) {
    load_palette(x, start_sign::PALETTE, start_sign::PALETTE_RAM);
    x.set8(0x2a, 1);
    x.set8(0x2b, 0x1c);
}

fn start_sign(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            start_sign_palette(x);
            at_camera_z(x);
            x.anim_start(start_sign::ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            at_camera_z(x);
            if x.u8(start_sign::RELOAD) != 0 {
                start_sign_palette(x);
                x.set8(start_sign::RELOAD, 0);
            }
            if x.anim_step() && x.a5_u16(0x52) == 0 {
                // fin de l'animation : les joueurs vivants et debout (étape ≤ 10) repartent
                x.set16(f::STATE, 0x602);
                x.set_a5_u8(0x59, 0);
                for p in living_players(x) {
                    if p.u8(x.b, 6) <= 10 {
                        p.set_u32(x.b, f::STATE, 0x0200_2c00);
                    }
                }
            }
            x.reinsert();
        }
        _ => {
            if x.u8(f::SUB_STATE) != 0 {
                return free(x);
            }
            x.set16(f::STATE, 0x200);
            start_sign_palette(x);
        }
    }
}

/// Enseigne posée à (0xb0, 0xd0) de la caméra, en hauteur (`+0xc`), profondeur nulle.
fn at_camera_z(x: &mut Cx) {
    let (cx, cy) = (x.a5_u16(CAMERA_X), x.a5_u16(CAMERA_Y));
    x.set16(Obj::X, cx);
    x.set16(Obj::Z, cy);
    x.set32(Obj::Y, 0);
    x.add16(Obj::X, 0xb0);
    x.add16(Obj::Z, 0xd0);
}

// ---------------------------------------------------------------------------------------------
// Type 77 : fondu sonore
// ---------------------------------------------------------------------------------------------

mod sound_fade {
    pub const STEPS: u32 = 0x30;
    pub const DELAY: u32 = 0x32;
    pub const LEVEL_A: u32 = 0x34;
    pub const LEVEL_B: u32 = 0x36;
    /// Commandes de service 0xff08 / 0xff09 (niveau en D0) et 0xff07 (arrêt, D0 = 0xc).
    pub const SET_A: u32 = 0xe0ad8;
    pub const SET_B: u32 = 0xe0af8;
    pub const STOP: u32 = 0xe0a82;
}

fn sound_fade_levels(x: &mut Cx) {
    for (level, command) in [(sound_fade::LEVEL_A, sound_fade::SET_A), (sound_fade::LEVEL_B, sound_fade::SET_B)] {
        let v = x.u16(level);
        x.c.d[0] = x.c.d[0] & 0xffff_0000 | v as u32;
        x.call(command);
    }
}

fn sound_fade(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(sound_fade::STEPS, 0x18);
            x.set16(sound_fade::DELAY, 5);
            x.set16(sound_fade::LEVEL_A, 0xff00);
            x.set16(sound_fade::LEVEL_B, 0xff00);
            sound_fade_levels(x);
        }
        2 if x.u8(f::SUB_STATE) == 0 => {
            let n = x.u16(sound_fade::DELAY).wrapping_sub(1);
            x.set16(sound_fade::DELAY, n);
            if (n as i16) > 0 {
                return;
            }
            x.add16(sound_fade::LEVEL_A, 2);
            x.add16(sound_fade::LEVEL_B, 4);
            sound_fade_levels(x);
            x.set16(sound_fade::DELAY, 5);
            let n = x.u16(sound_fade::STEPS).wrapping_sub(1);
            x.set16(sound_fade::STEPS, n);
            if (n as i16) > 0 {
                return;
            }
            x.c.d[0] = 0xc;
            x.call(sound_fade::STOP);
            free(x);
        }
        2 => {}
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 89 : personnage qui traverse la scène
// ---------------------------------------------------------------------------------------------

mod walker {
    pub const ANIM_SET: u32 = 0x19920e;
    pub const DELAY: u32 = 0x54;
    /// Signal de la scène (`A5-0x55c8`) qui le fait repartir.
    pub const SIGNAL: u32 = 0xff2a38;
}

fn walker(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(f::FACING, 0x20);
            x.set8(walker::DELAY, 0x38);
            at_camera(x, 0xffd0, 0x60);
            crate::terrain::push_68k(x.c, x.b, x.o);
            x.anim_start(walker::ANIM_SET, 0);
            x.reinsert();
        }
        2 => {
            match x.u8(f::SUB_STATE) {
                // entre en marchant, quatre pixels par image
                0 => {
                    let n = x.u8(walker::DELAY).wrapping_sub(1);
                    x.set8(walker::DELAY, n);
                    if n == 0 {
                        x.add8(f::SUB_STATE, 2);
                        x.anim_start(walker::ANIM_SET, 1);
                    } else {
                        x.add16(Obj::X, 4);
                        x.anim_step();
                    }
                }
                2 => {
                    x.anim_step();
                    if crate::mem::rd8(x.b, walker::SIGNAL) == 0x14 {
                        x.add8(f::SUB_STATE, 2);
                        x.anim_start(walker::ANIM_SET, 0);
                    }
                }
                // ressort par la droite, jusqu'à 0x1e0 pixels de la caméra
                _ => {
                    let limit = x.a5_u16(CAMERA_X).wrapping_add(0x1e0);
                    if limit > x.u16(Obj::X) {
                        x.add16(Obj::X, 4);
                    }
                    x.anim_step();
                }
            }
            crate::terrain::push_68k(x.c, x.b, x.o);
            x.reinsert();
        }
        _ => free(x),
    }
}

handlers! {
    0xa8594 "effct_ouverture" opening,
    0xb2f40 "effct_silhouette_reflet" mirrored,
    0xb66f8 "effct_traversee" crossing,
    0xb68f4 "effct_passage" passage,
    0xb6a74 "effct_lanceur" launcher,
    0xb7f28 "effct_enseigne_depart" start_sign,
    0xb896e "effct_fondu_sonore" sound_fade,
    0xba0b0 "effct_passant" walker,
}
