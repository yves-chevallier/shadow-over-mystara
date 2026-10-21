//! Actions secondaires du jeu normal (`+4` = 2, `+5` = 0) : course, portage, accroupi, poussée, saut
//! d'attaque propre à chaque famille de personnages, et les actions lancées par les coups spéciaux
//! (`combos`).
//!
//! Comme les actions communes (`actions`), chacune est un petit automate dont `+7` est le pas. Les
//! routines d'origine encore appelées (sons, effets) passent par `Ctx::rom`.

use super::actions::to_stand;
use super::checks::{attack_button, item_curse, jump_button};
use super::states::State;
use super::walk::{clamp_x, crawl_speed, move_x, move_xy, move_xz, move_z, run_speed, walk_speed};
use super::{animation, ram, Ctx, Player};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::objects::{pools, Obj};

/// Routines d'origine encore appelées telles quelles.
mod rom {
    /// Effet de choc (poussière) : D0 variante, D1 réglages, D2 et D3 vitesses.
    pub const DUST: u32 = 0xb10c8;
    /// Son du dérapage de la course.
    pub const SKID_SOUND: u32 = 0xe78e8;
    /// Sons de l'attaque sautée (départ, atterrissage).
    pub const LEAP_SOUND: u32 = 0xe7622;
    pub const LEAP_LANDING: u32 = 0xe774c;
    /// Effet de joueur lancé par l'événement d'animation `+0x46` (D0 = numéro).
    pub const ANIM_EVENT: u32 = 0x4c58e;
    /// Son de la poussée.
    pub const PUSH_SOUND: u32 = 0xeae14;
    /// Numéro de terrain au point (D0, D1) -> D2.
    pub const TERRAIN_AT: u32 = 0x24b0;
}

/// Champs propres à ces actions.
mod field {
    /// Événement d'animation (octet posé par le script, consommé par l'action).
    pub const ANIM_EVENT: u32 = 0x46;
    /// Le joueur est en l'air pour une attaque sautée (h).
    pub const AIRBORNE: u32 = 0x67;
    /// Attaque en l'air déjà donnée.
    pub const AIR_ATTACK: u32 = 0x6b;
    /// Événement de geste du script d'animation.
    pub const GESTURE: u32 = 0x33;
    /// Sens mémorisé au début d'une action (orientation inverse de celle d'origine).
    pub const HELD_FACING: u32 = 0x1e8;
}

fn next_step(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP).wrapping_add(2);
    p.set_u8(g, Player::STEP, step);
}

/// Sens demandé par les directions : bit 0 (droite) -> 0x20.
fn facing_for(keys: u8) -> u8 {
    if keys & 1 != 0 {
        0x20
    } else {
        0
    }
}

/// Vitesse horizontale et accélération (mots), retournées (`neg.l` du couple) si le joueur regarde à gauche.
fn set_vx_pair(g: &mut Ctx, p: Player, vx: u16, ax: u16) {
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, ax);
    if p.faces_left(g) {
        let pair = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, pair);
    }
}

/// Une image sur quatre (selon le personnage), poussière sous les pieds : variante D0, réglages D1, vitesses D2/D3.
fn dust(g: &mut Ctx, p: Player, d0: u16, d1: u16, d2: u16, d3: u16) {
    let set = |r: &mut u32, v: u16| *r = *r & 0xffff_0000 | v as u32;
    set(&mut g.c.d[0], d0);
    set(&mut g.c.d[1], d1);
    set(&mut g.c.d[2], d2);
    set(&mut g.c.d[3], d3);
    g.rom(rom::DUST, p);
}

fn dust_turn(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::CHARACTER).wrapping_add(g.a5u8(ram::FRAME_COUNTER)) & 3 == 0
}

// ---- course (action 6, 0x3573e) ----------------------------------------------------------------

/// Durée de la course (images) et champ qui la décompte.
const RUN_TIME: u8 = 200;
const RUN_LEFT: u32 = Player::PARAM;

/// 0x3573e : course. Pas 0 : départ ; 2 : course (le compteur baisse de 2 par image) ; 4 : dérapage
/// d'arrêt ; 6 : dérapage de demi-tour.
pub fn run(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, RUN_LEFT, RUN_TIME);
            animation::start(g, p, 7);
            running(g, p);
        }
        2 => running(g, p),
        4 => skid_stop(g, p),
        6 => skid_turn(g, p),
        _ => {
            g.rom(0x3573e, p);
        }
    }
}

/// 0x35764.
fn running(g: &mut Ctx, p: Player) {
    if attack_button(g, p) || jump_button(g, p) || run_steer(g, p) {
        return;
    }
    let left = p.u8(g, RUN_LEFT).wrapping_sub(1);
    p.set_u8(g, RUN_LEFT, left);
    if left == 0 {
        to_stand(g, p);
        return;
    }
    run_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

/// Dérapage : pas `step`, vitesse -0x480 (accélération `ax`), animation `anim`.
fn start_skid(g: &mut Ctx, p: Player, step: u8, ax: u16, anim: u16) {
    p.set_u8(g, Player::STEP, step);
    set_vx_pair(g, p, (-0x480i16) as u16, ax);
    animation::start(g, p, anim);
}

/// 0x35814 : fin de course (compteur épuisé ou plus de direction) ou demi-tour demandé : dérapage. Vrai
/// si la course s'interrompt.
fn run_steer(g: &mut Ctx, p: Player) -> bool {
    let left = p.u8(g, RUN_LEFT).wrapping_sub(1);
    p.set_u8(g, RUN_LEFT, left);
    let keys = p.u8(g, Player::KEYS_HELD) & 0xf;
    if left == 0 || keys == 0 {
        p.set_u8(g, RUN_LEFT, 4);
        start_skid(g, p, 4, 40, 26);
    } else if keys & 3 == 0 || facing_for(keys) == p.u8(g, Player::FACING) {
        return false;
    } else {
        start_skid(g, p, 6, 56, 27);
    }
    g.rom(rom::SKID_SOUND, p);
    true
}

/// 0x358a4 : poussière du dérapage tant que la vitesse dépasse 1,5 pixel par image.
fn skid_dust(g: &mut Ctx, p: Player) {
    let speed = (p.u16(g, Obj::VX) as i16).wrapping_abs() as u16;
    if dust_turn(g, p) && speed > 0x180 {
        dust(g, p, 0, 0x101, (-16i16) as u16, 6);
    }
}

/// 0x35792 : dérapage d'arrêt ; un demi-tour reste possible tant que `+0x69` dure.
fn skid_stop(g: &mut Ctx, p: Player) {
    move_x(g, p);
    skid_dust(g, p);
    if animation::step(g, p) {
        p.set_u32(g, Obj::VX, 0);
        to_stand(g, p);
        return;
    }
    let left = p.u8(g, RUN_LEFT);
    if left == 0 {
        return;
    }
    p.set_u8(g, RUN_LEFT, left - 1);
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys != 0 && facing_for(keys) != p.u8(g, Player::FACING) {
        start_skid(g, p, 6, 56, 27);
    }
}

/// 0x357f6 : dérapage de demi-tour ; à la fin, le joueur s'est retourné.
fn skid_turn(g: &mut Ctx, p: Player) {
    move_x(g, p);
    skid_dust(g, p);
    if animation::step(g, p) {
        p.flip(g);
        to_stand(g, p);
    }
}

// ---- portage (action 0x14, 0x35c26) -------------------------------------------------------------

/// Objet porté (`+0x86`) : son état (`+5`, `+6`).
const HELD: u32 = Player::LINK;

/// 0x35c26 : prendre (`+0x8b` = 0) ou lâcher (2) l'objet porté.
pub fn carry(g: &mut Ctx, p: Player) {
    match p.u8(g, 0x8b) {
        0 => {}
        2 => {
            if animation::step(g, p) {
                to_stand(g, p);
            }
            return;
        }
        _ => {
            g.rom(0x35c26, p);
            return;
        }
    }
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::SLOW_8A, 1);
            let held = p.u32(g, HELD);
            let anim = if rd8(g.b, held + 0x6b) != 0 {
                next_step(g, p);
                45
            } else {
                43
            };
            animation::start(g, p, anim);
        }
        2 => {
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        4 => {
            if p.u8(g, field::GESTURE) != 0 {
                p.set_u8(g, field::GESTURE, 0);
                p.set_u8(g, Player::SLOW_8A, 0);
                let held = p.u32(g, HELD);
                wr8(g.b, held + 5, 8);
                wr16(g.b, held + 6, 0);
                put_down(g, p, held);
            }
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        _ => {
            g.rom(0x35c38, p);
        }
    }
}

/// 0x35ca8 : pose l'objet devant le joueur (décalages par classe, table 0x35ce2).
fn put_down(g: &mut Ctx, p: Player, held: u32) {
    let at = 0x35ce2 + 4 * p.class(g) as u32;
    let (mut dx, dz) = (g.prog16(at), g.prog16(at + 2));
    if p.faces_left(g) {
        dx = dx.wrapping_neg();
    }
    let (x, z, y) = (p.u16(g, Obj::X), p.u16(g, Obj::Z), p.u16(g, Obj::Y));
    wr16(g.b, held + Obj::X, x.wrapping_add(dx));
    wr16(g.b, held + Obj::Z, z.wrapping_add(dz));
    wr16(g.b, held + Obj::Y, y.wrapping_sub(8));
}

// ---- garde (actions 0x16 et 0x18, 0x35d52 et 0x35dca) ---------------------------------------------

/// Garde au bouclier tant que le bouton `held` (bit 7 : spécial, bit 4 : attaque) est maintenu : le
/// joueur recule lentement (directions opposées à son sens d'origine, ou verticales) sans se
/// retourner ; relâché, il reprend la pose (ou attaque, ou saute).
pub fn guard(g: &mut Ctx, p: Player, held: u8) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::SUBACTION, 0);
        let facing = p.u8(g, Player::FACING) ^ 0x20;
        p.set_u8(g, field::HELD_FACING, facing);
        animation::start(g, p, 25);
        return;
    }
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys & held == 0 {
        if !attack_button(g, p) && !jump_button(g, p) {
            to_stand(g, p);
        }
        return;
    }
    let dir = keys & 0xf;
    let moving = dir != 0 && (dir & 3 == 0 || facing_for(dir) == p.u8(g, field::HELD_FACING));
    if !moving {
        animation::start(g, p, 25);
        return;
    }
    crawl_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

// ---- action 0x2a (0x36c12) : saut sur place avec rebond (h) ---------------------------------------

/// 0x36c12 : décollage, retombée puis marche sur la hauteur ; la hauteur oscille de 2 pixels.
pub fn hop(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0);
            p.set_u32(g, Obj::VX, 0);
            p.set_u16(g, Obj::VZ, (-0x900i16) as u16);
            p.set_u16(g, Obj::AZ, 40);
            animation::start(g, p, 56);
        }
        2 => {
            move_z(g, p);
            if p.u16(g, Obj::Z) as i16 <= 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                p.set_u32(g, Obj::VZ, 0);
            }
            animation::step(g, p);
        }
        4 => {
            walk_speed(g, p);
            move_x(g, p);
            let y = p.u16(g, Obj::Y).wrapping_sub(4);
            p.set_u16(g, Obj::Y, y);
            animation::step(g, p);
            p.set_u8(g, 0x3b, 0);
            clamp_x(g.b, p);
        }
        _ => {
            g.rom(0x36c12, p);
            return;
        }
    }
    p.set_u8(g, 0x3b, 0);
    let z = p.u16(g, Obj::Z).wrapping_add(2);
    p.set_u16(g, Obj::Z, z);
    let phase = p.u8(g, Player::TIMER_6F).wrapping_add(1) & 1;
    p.set_u8(g, Player::TIMER_6F, phase);
    if phase == 0 {
        let z = p.u16(g, Obj::Z).wrapping_sub(4);
        p.set_u16(g, Obj::Z, z);
    }
}

// ---- poussée (action 0x2e, 0x36d68) ------------------------------------------------------------

/// 0x36d68 : pousse l'objet lié (`+0x86`) dans une des quatre directions (pas 2 à 8 : haut, bas,
/// gauche, droite) tant que la direction est maintenue.
pub fn push(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP);
    let key = match step {
        0 => {
            let first = p.u8(g, 0x16f);
            p.set_u8(g, Player::STEP, first);
            p.set_u8(g, Player::TIMER_6F, 32);
            animation::start(g, p, 58);
            p.set_u8(g, field::GESTURE, 0);
            return;
        }
        2 => 8,
        4 => 4,
        6 => 2,
        8 => 1,
        _ => {
            g.rom(0x36d68, p);
            return;
        }
    };
    if p.u8(g, Player::KEYS_HELD) & key == 0 {
        let held = p.u32(g, HELD);
        wr8(g.b, held + 0x66, 0);
        wr16(g.b, held + 0x6c, 0);
        to_stand(g, p);
        return;
    }
    let wait = p.u8(g, Player::TIMER_6F);
    if wait != 0 {
        p.set_u8(g, Player::TIMER_6F, wait - 1);
        if wait - 1 != 0 {
            return;
        }
        p.set_u8(g, field::GESTURE, 1);
    }
    animation::step(g, p);
    if step >= 6 {
        push_dust(g, p);
    }
    if p.u8(g, field::ANIM_EVENT) != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        g.rom(rom::PUSH_SOUND, p);
    }
}

/// 0x36eb0 : poussière du geste de poussée (réglages par classe, table 0x36eea).
fn push_dust(g: &mut Ctx, p: Player) {
    if !dust_turn(g, p) || p.u8(g, field::GESTURE) == 0 {
        return;
    }
    let at = 0x36eea + 8 * p.class(g) as u32;
    let (d0, d1, d2, d3) = (g.prog16(at), g.prog16(at + 2), g.prog16(at + 4), g.prog16(at + 6));
    g.c.d[4] = g.c.d[4] & 0xffff_0000 | 8 * p.class(g) as u32;
    dust(g, p, d0, d1, d2, d3);
}

// ---- action 0x32 (0x36f48) ---------------------------------------------------------------------

/// 0x36f48 : animation 55 pendant 24 images, puis fin de la marche lente.
pub fn spell_fizzle(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 24);
        animation::start(g, p, 55);
        return;
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
    } else {
        animation::step(g, p);
    }
}

// ---- action 0x12 des personnages : bonds ---------------------------------------------------------

/// 0x35a78 (combattant, clerc, elfe) : bond en avant, avec une attaque possible en l'air.
pub fn leap_attack(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, field::AIR_ATTACK, 0);
            p.set_u8(g, field::AIRBORNE, 0);
            p.set_u16(g, Obj::VX, 0x200);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x380);
            p.set_u16(g, Obj::AZ, (-56i16) as u16);
            if p.faces_left(g) {
                p.set_u16(g, Obj::VX, (-0x200i16) as u16);
            }
            animation::start(g, p, 31);
            g.rom(rom::LEAP_SOUND, p);
        }
        2 => {
            animation::step(g, p);
            move_xz(g, p);
            air_attack(g, p);
            if (p.u16(g, Obj::Z) as i16) < 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                animation::start(g, p, 14);
                g.rom(rom::LEAP_LANDING, p);
            }
        }
        4 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        _ => {
            g.rom(0x35a78, p);
        }
    }
    let event = p.u8(g, field::ANIM_EVENT);
    if event != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        g.c.d[0] = event as u32;
        g.rom(rom::ANIM_EVENT, p);
    }
}

/// 0x3769c : pendant le bond, le bouton 1 donne une attaque en l'air (animation par personnage, table
/// 0x376d6, ou 0x376e2 avec bas maintenu).
fn air_attack(g: &mut Ctx, p: Player) {
    if p.u8(g, field::AIR_ATTACK) != 0 || p.u8(g, Player::KEYS_NEW) & 0x10 == 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let table = if p.u8(g, Player::KEYS_HELD) & 4 != 0 { 0x376e2 } else { 0x376d6 };
    let anim = g.prog8(table + character);
    g.c.d[6] = anim as u32;
    animation::start_plain(g, p, anim as u16);
    item_curse(g, p);
}

/// 0x35b96 (magicien) : petit saut en arrière (le recul s'arrête devant un obstacle du décor).
pub fn hop_back(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            animation::start(g, p, 31);
        }
        2 => {
            if !animation::step(g, p) {
                return;
            }
            next_step(g, p);
            p.set_u16(g, Obj::VX, 0x300);
            p.set_u16(g, Obj::AX, (-32i16) as u16);
            let mut shift: u16 = 64;
            if p.faces_left(g) {
                shift = shift.wrapping_neg();
                let pair = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, pair);
            }
            let set = |r: &mut u32, v: u16| *r = *r & 0xffff_0000 | v as u32;
            let (x, y) = (p.u16(g, Obj::X).wrapping_add(shift), p.u16(g, Obj::Y));
            set(&mut g.c.d[0], x);
            set(&mut g.c.d[1], y);
            g.rom(rom::TERRAIN_AT, p);
            if g.c.d[2] & 63 == 0 {
                p.set_u16(g, Obj::X, x);
            }
            super::walk::clamp_to_screen(g.b, p);
            animation::start(g, p, 32);
        }
        4 => {
            move_x(g, p);
            let mut vx = p.u16(g, Obj::VX);
            if p.faces_left(g) {
                vx = vx.wrapping_neg();
            }
            if (vx as i16) < 0 {
                to_stand(g, p);
            } else {
                animation::step(g, p);
            }
        }
        _ => {
            g.rom(0x35b96, p);
        }
    }
}

/// Natives des actions 0x12 de personnages (table `characters::Slot::Action12`) : la fonction Rust reçoit
/// le joueur (A0) ; les appelants ne lisent aucun registre.
pub const LEAP: crate::Native = crate::Native {
    entry: 0x35a78,
    name: "leap_attack",
    cost: |_, _| 100,
    run: |c, b| {
        let p = Player(c.a[0]);
        leap_attack(&mut Ctx::new(c, b), p);
        crate::mem::rts(c, b)
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

pub const HOP_BACK: crate::Native = crate::Native {
    entry: 0x35b96,
    name: "magic_user_hop_back",
    cost: |_, _| 100,
    run: |c, b| {
        let p = Player(c.a[0]);
        hop_back(&mut Ctx::new(c, b), p);
        crate::mem::rts(c, b)
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};

// ---- parade (`+5` = 8, 0x39ff0) ----------------------------------------------------------------------

/// Riposte après une parade : sous-action d'attaque par classe (octets du code).
const RIPOSTE: u32 = 0x3a0a2;

/// 0x39ff0 : après une parade, le joueur glisse en arrière (vitesse 0x400 freinée de 0x40 par image) ; à
/// l'arrêt, il reprend le jeu à l'action `+0x6f`. Pendant la glissade, la direction de l'adversaire
/// maintenue enchaîne une riposte (sauf le magicien). Puis décor et limites de l'écran.
pub fn parry(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::ACTION) {
        0 => {
            p.set_u8(g, Player::ACTION, 2);
            p.set_u8(g, 0x3e, 0);
            set_vx_pair(g, p, 0x400, (-64i16) as u16);
            parry_slide(g, p);
        }
        2 => parry_slide(g, p),
        _ => {
            g.rom(0x39ff0, p);
            return;
        }
    }
    g.rom(0x33fe4, p);
    super::walk::clamp_to_screen(g.b, p);
}

fn parry_slide(g: &mut Ctx, p: Player) {
    move_x(g, p);
    let mut vx = p.u16(g, Obj::VX);
    if p.faces_left(g) {
        vx = vx.wrapping_neg();
    }
    if (vx as i16) < 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_state(g, 0x0200_0000);
        let resume = p.u8(g, Player::TIMER_6F);
        p.set_u8(g, Player::ACTION, resume);
        return;
    }
    let class = p.class(g);
    if class == 4 {
        return;
    }
    let toward = if p.faces_left(g) { 0 } else { 1 };
    if p.u8(g, Player::KEYS_HELD) >> toward & 1 != 0 {
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_state(g, 0x0200_0c00);
        p.set_u32(g, Obj::VX, 0);
        let kind = g.prog8(RIPOSTE + class as u32);
        p.set_u8(g, Player::SUBACTION, kind);
    }
}

// ---- actions des coups spéciaux (appelées par `combos`) --------------------------------------------

/// Attaque (action 0xc) de sous-action `kind` dans le sens mémorisé `facing_from` (ou le sens courant).
fn combo_attack(g: &mut Ctx, p: Player, kind: u8, facing_from: Option<u32>) -> bool {
    p.set_u8(g, Player::HITSTOP, 0);
    p.set_state(g, State::pack(2, 0, 0xc, 0).0);
    p.set_u8(g, Player::SUBACTION, kind);
    if let Some(at) = facing_from {
        let facing = p.u8(g, at);
        p.set_u8(g, Player::FACING, facing);
    }
    item_curse(g, p);
    true
}

/// Une action de coup spécial ; renvoie vrai si elle a changé l'état (Z à 0 dans l'original).
#[derive(Clone, Copy, Debug)]
pub enum Combo {
    /// 0x37386 : course dans le sens de la séquence (vrai seulement vers la gauche : Z accidentel).
    Dash,
    /// 0x373a8 : action 0x10, au sol et hors attaque.
    Action10,
    /// 0x373cc : action 0x12 (ou 0x1a avec la classe de vitesse 2).
    Action12,
    /// 0x3733c : attaque de sous-action 10.
    Attack10,
    /// 0x37366 : attaque de sous-action 12.
    Attack12,
    /// 0x373f4 : attaque de sous-action 22.
    Attack22,
    /// 0x3740e : attaque de sous-action 24 (sauf avec 8 PV ou moins, clerc excepté).
    Attack24,
    /// 0x3743e : attaque de sous-action 26 du nain, en pleine attaque.
    DwarfAttack26,
    /// 0x3746a : attaque de sous-action 30 (sort 36 ou 44 en ligne 2).
    Attack30,
    /// 0x374aa : le voleur saisit un ennemi de dos.
    Backstab,
}

pub fn combo(g: &mut Ctx, p: Player, which: Combo) -> bool {
    let slow = p.u8(g, Player::SLOW) != 0;
    match which {
        Combo::Attack10 => !slow && combo_attack(g, p, 10, Some(0x1b7)),
        Combo::Attack12 => combo_attack(g, p, 12, Some(0x1bd)),
        Combo::Attack22 => combo_attack(g, p, 22, None),
        Combo::Dash => {
            if slow || p.u8(g, Player::ACTION) == 0xc {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 6, 0).0);
            let facing = p.u8(g, 0x1a5);
            p.set_u8(g, Player::FACING, facing);
            facing != 0
        }
        Combo::Action10 => {
            if p.u16(g, Obj::Z) != 0 || p.u8(g, Player::ACTION) == 0xc {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0x10, 0).0);
            let facing = p.u8(g, 0x1ab);
            p.set_u8(g, Player::FACING, facing);
            true
        }
        Combo::Action12 => {
            if p.u8(g, Player::SPEED_CLASS) == 2 {
                p.set_state(g, State::pack(2, 0, 0x1a, 0).0);
                p.set_u8(g, 0x1e0, 0);
            } else {
                p.set_state(g, State::pack(2, 0, 0x12, 0).0);
            }
            true
        }
        Combo::Attack24 => {
            if p.class(g) != 1 && p.u16(g, Player::HP) <= 8 {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0xc, 0).0);
            p.set_u8(g, Player::SUBACTION, 24);
            item_curse(g, p);
            true
        }
        Combo::DwarfAttack26 => {
            if p.u8(g, Player::ACTION) != 0xc || p.u8(g, Player::SUBACTION) != 0 {
                return false;
            }
            combo_attack(g, p, 26, None)
        }
        Combo::Attack30 => {
            let spell = if p.class(g) == 2 { 36 } else { 44 };
            if p.u8(g, Player::SPECIAL_COUNT) != 2 || p.u8(g, 0x140) != spell {
                return false;
            }
            p.set_state(g, State::pack(2, 0, 0xc, 0).0);
            p.set_u8(g, Player::SUBACTION, 30);
            true
        }
        Combo::Backstab => backstab(g, p),
    }
}

/// Orientation d'un objet (0 ou 0x20).
const FACING: u32 = 0x2c;

/// Table (code, 0x37588) : non nul pour les types d'ennemis qui ne se laissent pas saisir.
const UNGRABBABLE: u32 = 0x37588;
/// Boîtes de corps (données) : un pointeur par jeu de boîtes, entrées de 8 octets.
const BODY_BOXES: u32 = 0x11bffa;

/// 0x374aa : un ennemi au sol, tourné dans le même sens que le joueur (donc de dos), saisissable et
/// assez proche est saisi ; le joueur passe à l'attaque de sous-action 26.
fn backstab(g: &mut Ctx, p: Player) -> bool {
    if g.a5u8(ram::FROZEN) != 0 {
        return false;
    }
    for enemy in pools::ENEMY.iter() {
        let e = enemy.0;
        let b = &mut *g.b;
        let ok = rd8(b, e) != 0
            && rd8(b, e + 1) & 1 != 0
            && rd8(b, e + 4) == 2
            && rd16(b, e + Obj::Z) == 0
            && rd16(b, e + 0x62) != 0
            && rd8(b, e + 0x86) == 0
            && rd8(b, e + 0x40) == 0
            && rd32(b, e + 0x94) & 1 << 30 == 0
            && rd8(b, p.at(Player::FACING)) == rd8(b, e + FACING);
        if !ok {
            continue;
        }
        let kind = rd8(b, e + 2) as u32;
        if crate::mem::prog8(b, UNGRABBABLE + kind) != 0 {
            continue;
        }
        let box_set = rd8(b, e + 0x2e) as u32;
        let set = rd32(b, BODY_BOXES + 4 * box_set);
        let body = set + 8 * rd8(b, e + 0x38) as u32;
        let facing = rd8(b, p.at(0x1cf));
        wr8(b, p.at(Player::FACING), facing);
        if !within_reach(b, p.0, e, body) {
            continue;
        }
        wr8(b, e, 2);
        wr32(b, e + 4, 0x0400_0000);
        wr8(b, e + 0x49, 100);
        let index = rd8(b, p.at(Player::INDEX));
        wr8(b, e + 0x48, index);
        wr16(b, e + 0x50, p.0 as u16);
        wr32(b, p.at(Player::MAIN), 0x0200_0c00);
        wr8(b, p.at(0x47), 88);
        wr8(b, p.at(Player::SUBACTION), 26);
        wr16(b, p.at(0x50), e as u16);
        return true;
    }
    false
}

/// 0x375a8 : l'ennemi `e` (boîte de corps `body`) est-il à portée de saisie du joueur `me` ? (en
/// profondeur ±(16 + demi-profondeur), en x devant le joueur ±(48 + l'octet `+1` de l'ennemi, comme l'original)).
fn within_reach(b: &mut cps2::Cps2Bus, me: u32, e: u32, body: u32) -> bool {
    let sbyte = |b: &mut cps2::Cps2Bus, a: u32| rd8(b, a) as i8 as i16 as u16;
    let near = |gap: u16, half: u16| gap.wrapping_add(half) <= half.wrapping_add(half);
    let gap = 4u16.wrapping_add(rd16(b, me + Obj::Y)).wrapping_sub(sbyte(b, body + 4).wrapping_add(rd16(b, e + Obj::Y)));
    if !near(gap, 16u8.wrapping_add(rd8(b, body + 5)) as u16) {
        return false;
    }
    let mut reach = (-48i16) as u16;
    if rd8(b, me + FACING) != 0 {
        reach = reach.wrapping_neg();
    }
    let mut offset = sbyte(b, body);
    if rd8(b, e + FACING) != 0 {
        offset = offset.wrapping_neg();
    }
    let gap = reach.wrapping_add(rd16(b, me + Obj::X)).wrapping_sub(offset.wrapping_add(rd16(b, e + Obj::X)));
    near(gap, 48u8.wrapping_add(rd8(b, e + 1)) as u16)
}

