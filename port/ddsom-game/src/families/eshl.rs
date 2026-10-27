//! Tirs et attaques des ennemis (classe ESHL, pool de 38 objets de 0x60 octets, table 0x39b96).
//!
//! Un objet ESHL est créé par un ennemi, un boss ou un effet : `+0x50` désigne son propriétaire
//! (adresse RAM sur 16 bits), `+0x03` est la variante choisie par le créateur, `+0x2c` l'orientation
//! héritée du propriétaire. Les boîtes de collision (jeu `+0x2e`, puissance `+0x44/+0x45`) sont lues
//! par `combat`. Chaque type est une machine à états : `+4` état principal, `+5` / `+6` sous-états
//! (octets pairs, indices d'une table de sauts de l'original).

use super::cx::{handlers, Cx};
use super::shell::*;
use crate::objects::Obj;

/// Sons (wrappers de la zone 0xe0836-0xee080).
mod sfx {
    pub const E7376: u32 = 0xe7376;
    pub const E73E8: u32 = 0xe73e8;
    pub const EBE86: u32 = 0xebe86;
    pub const E46DE: u32 = 0xe46de;
    pub const E47BC: u32 = 0xe47bc;
    pub const EBF6A: u32 = 0xebf6a;
    pub const ECE2C: u32 = 0xece2c;
    pub const EBCBE: u32 = 0xebcbe;
    pub const EBD30: u32 = 0xebd30;
}

use crate::objects::pools;
use crate::objects::Courant;

// ---------------------------------------------------------------------------------------------
// Type 0 (0x9dbfc) : projectile horizontal, deux variantes (+3)
// ---------------------------------------------------------------------------------------------

/// Projectile qui part du côté du propriétaire à 5 pixels par image. Il s'arrête quand il touche
/// quelque chose (drapeaux `+0x57`) ou certains terrains, puis retombe en clignotant avant de
/// disparaître.
mod shot0 {
    use super::*;

    pub const ANIMS: u32 = 0x160350;
    /// Sous-états de la chute (`+6`).
    const FALL_START: u8 = 0;
    const FALL_DROP: u8 = 2;
    const FALL_BLINK: u8 = 4;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if !spawn(x) {
                    return;
                }
                fly(x);
            }
            2 => fly(x),
            4 => fall(x),
            _ => x.free_eshl(),
        }
    }

    /// Création : place le projectile devant le propriétaire. Faux si celui-ci a disparu.
    fn spawn(x: &mut Cx) -> bool {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        if x.u16(f::OWNER) == 0 {
            return false;
        }
        let owner = x.linked(f::OWNER);
        place_in_front(x, owner, 0x40, 0x38);
        x.set32(0x18, 0);
        x.set8(0x2d, 0x18);
        x.set16(0x14, -0x500i16 as u16);
        if x.u8(f::FLIP) != 0 {
            x.set8(0x2d, 8);
            x.set16(0x14, 0x500);
        }
        let (anim, id, power) = if x.u8(0x3) == 0 { (2, 0x18, 3) } else { (0xf, 0x19, 5) };
        x.set8(f::ATTACK_POWER, power);
        x.set8(f::ATTACK_ID, id);
        x.anim_start(ANIMS, anim);
        crate::sound::jouer(x.b, sfx::EBE86, x.o);
        true
    }

    /// Vol : détruit au contact, s'arrête sur les terrains 5 à 8 sans mur (bit 6), sinon avance.
    fn fly(x: &mut Cx) {
        let hit = x.u8(f::HIT_FLAGS);
        if hit & 0x08 != 0 {
            crate::sound::jouer(x.b, sfx::E7376, x.o);
            x.free_eshl();
            return;
        }
        if hit & 0x20 != 0 {
            x.add8(0x4, 2);
            return fall(x);
        }
        // registres de 0x24a8 laissés comme l'original (non prouvés morts : voir `pshl::stopped_by_terrain`)
        let t = crate::video::terrain_of(x.b, x.o);
        crate::video::terrain_68k(x.c, &t);
        let terrain = t.numero as u32;
        let ground = (terrain & 0x3f).wrapping_sub(5) as u16 <= 3;
        if ground && terrain & 0x40 == 0 {
            return fall(x);
        }
        x.move_xz();
        x.anim_step();
        x.cull_offscreen();
        x.reinsert();
    }

    /// Chute après l'impact : tombe, puis clignote 20 images avant de disparaître.
    fn fall(x: &mut Cx) {
        match x.u8(0x6) {
            FALL_START => {
                x.add8(0x6, 2);
                crate::sound::jouer(x.b, sfx::E73E8, x.o);
                x.set16(0x18, -0xc0i16 as u16);
                x.set16(0x1a, -0x10i16 as u16);
                x.set16(0x14, 0);
                let anim = if x.u8(0x3) != 0 { 0x10 } else { 3 };
                x.anim_start(ANIMS, anim);
            }
            FALL_DROP => {
                x.move_xz();
                x.anim_step();
                if (x.u16(0xc) as i16) < 0 {
                    x.set_bit(0x1, 1, true);
                    x.set16(0xc, 0);
                    x.add8(0x6, 2);
                    x.set8(f::TIMER, 0x14);
                }
            }
            FALL_BLINK => {
                let t = x.u8(f::TIMER).wrapping_sub(1);
                x.set8(f::TIMER, t);
                if (t as i8) < 0 {
                    x.set8(0x4, st::FREE);
                }
                blink(x);
                x.anim_step();
            }
            _ => {}
        }
    }
}


/// Tir 1 (0x9de10) : boule lancée en cloche. Deux modes selon `+0x2f` : 0, le propriétaire la
/// lance devant lui ; sinon elle jaillit avec une vitesse tirée d'une table à 8 lignes (variante
/// `+3`). Elle retombe, rebondit au sol (Z < 0 : Z remis à 0), attend 80 images dans une
/// animation puis clignote 80 images avant de disparaître.
mod shot1 {
    use super::*;

    const ANIMS: u32 = 0x16c946;
    /// Table de vitesses (16 octets par variante : VX AX VZ AZ VY AY).
    const SPEEDS: u32 = 0x9dfe2;
    const LOB: u8 = 0;
    const LANDED: u8 = 2;
    const BLINK: u8 = 4;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 8);
                x.set8(f::ATTACK_POWER, 5);
                x.add8(0x4, 2);
                if x.u8(0x2f) != 0 {
                    burst(x);
                } else {
                    lob(x);
                }
                x.anim_start(ANIMS, 0);
                x.reinsert();
                body(x);
            }
            2 => body(x),
            _ => x.free_eshl(),
        }
    }

    /// Lancé par le propriétaire : devant lui, vitesse horizontale 2,5 pixels par image.
    fn lob(x: &mut Cx) {
        let owner = x.linked(f::OWNER);
        place_in_front(x, owner, 0x40, 0x20);
        let vx = if x.u8(f::FLIP) != 0 { 0x280 } else { -0x280i16 as u16 };
        x.set16(0x14, vx);
        x.set16(0x16, 0);
        x.set16(0x18, 0x300);
        x.set16(0x1a, -0x38i16 as u16);
        x.set16(0x1c, 0);
        x.set16(0x1e, 0);
    }

    /// Jaillissement : position du propriétaire, vitesses de la ligne `+3` de la table.
    fn burst(x: &mut Cx) {
        let owner = x.linked(f::OWNER);
        copy_position(x, owner);
        x.add16(0x10, 0xffff);
        x.add16(0xc, 0x32);
        let row = SPEEDS + 16 * x.u8(0x3) as u32;
        for i in 0..6 {
            let v = x.rom16(row + 2 * i);
            x.set16(0x14 + 2 * i, v);
        }
    }

    fn body(x: &mut Cx) {
        match x.u8(0x5) {
            LOB => {
                if !x.move_xyz().gt() {
                    x.add8(0x5, 2);
                    x.set16(f::TIMER, 0x50);
                    x.set32(0xc, 0);
                    crate::sound::jouer(x.b, sfx::EBCBE, x.o);
                    x.anim_start(ANIMS, 1);
                }
            }
            LANDED => {
                if x.u8(0x33) != 0 {
                    x.set8(0x33, 0);
                    crate::sound::jouer(x.b, sfx::EBD30, x.o);
                }
                let t = x.u16(f::TIMER).wrapping_sub(1);
                x.set16(f::TIMER, t);
                if t != 0 {
                    x.anim_step();
                } else {
                    x.add8(0x5, 2);
                    x.set16(f::TIMER, 0x50);
                    x.anim_start(ANIMS, 2);
                }
            }
            BLINK => {
                x.anim_step();
                let t = x.u16(f::TIMER).wrapping_sub(1);
                x.set16(f::TIMER, t);
                if t == 0 {
                    x.add8(0x4, 2);
                }
                blink(x);
            }
            _ => {}
        }
        finish(x);
    }

    /// Fin du gestionnaire quand l'état est inchangé : liste triée puis retrait hors écran.
    fn finish(x: &mut Cx) {
        x.reinsert();
        x.cull_offscreen();
    }
}


/// Tir 2 (0x9e0aa) : projectile horizontal sans animation propre (le sprite est celui de la
/// variante du propriétaire). Détruit au contact (son), ou sur un terrain bloquant.
mod shot2 {
    use super::*;

    const ANIMS: u32 = 0x16caa6;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 3);
                if x.u16(f::OWNER) == 0 {
                    x.set32(0x4, 0x0600_0000);
                    return;
                }
                let owner = x.linked(f::OWNER);
                place_in_front(x, owner, 0x40, 0x20);
                let (anim, id) = if owner.u8(x.b, 0x2) != 0 { (1, 0xa) } else { (0, 9) };
                x.set8(f::ATTACK_ID, id);
                x.anim_start(ANIMS, anim);
                crate::sound::jouer(x.b, sfx::EBF6A, x.o);
                fly(x);
            }
            2 => fly(x),
            _ => x.free_eshl(),
        }
    }

    fn fly(x: &mut Cx) {
        if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
            crate::sound::jouer(x.b, sfx::E7376, x.o);
            return x.free_eshl();
        }
        if blocked_by_terrain(x) {
            return x.free_eshl();
        }
        x.move_xz();
        x.cull_offscreen();
        x.reinsert();
    }
}

/// Tir 3 (0x9e1b6) : projectile attaché à un propriétaire (le créateur est vivant tant que son
/// état `+4` est inférieur à 4). Quatre variantes lancées en éventail : décalage (dx, dz) et
/// vitesses (vx, vy) lus dans une table de 8 octets par variante. Disparaît avec la fin de son
/// animation ou quand le propriétaire n'est plus en jeu normal.
mod shot3 {
    use super::*;

    const ANIMS: u32 = 0x16cb10;
    const FAN: u32 = 0x9e280;

    pub fn run(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        if owner.u8(x.b, 0x4) as i8 >= 4 {
            return x.free_eshl();
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0);
                x.set8(f::ATTACK_ID, 0x1b);
                place(x, owner);
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                x.move_xy();
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_eshl(),
        }
    }

    fn place(x: &mut Cx, owner: Obj) {
        copy_position(x, owner);
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let row = FAN + 8 * x.u8(0x3) as u32;
        let (dx, dz) = (x.rom16(row), x.rom16(row + 2));
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        let (vx, vy) = (x.rom16(row + 4), x.rom16(row + 6));
        x.set16(0x14, vx);
        x.set16(0x1c, vy);
        for off in [0x16, 0x1a, 0x18, 0x1e] {
            x.set16(off, 0);
        }
    }
}

/// Tir 4 (0x9e2dc) : bombe lancée par un ennemi (sans propriétaire : position et sens fixés par le
/// créateur). Elle retombe selon une table de vitesses, rebondit au sol une fois avec les
/// vitesses mises de côté en `+0x42` / `+0x50`, puis explose en fin d'animation.
mod shot4 {
    use super::*;

    const ANIMS: u32 = 0x16cb74;
    const FALLING: u8 = 0;
    const BOUNCING: u8 = 2;
    const EXPLODING: u8 = 4;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                match x.u8(0x5) {
                    FALLING => fall(x),
                    BOUNCING => bounce(x),
                    EXPLODING => explode(x),
                    _ => {}
                }
                let (px, py) = (x.u16(0x8), x.u16(0x10));
                // seuls les registres de 0x24b0 restent (D0-D4, A1), pour le gestionnaire suivant
                let t = crate::video::terrain(x.b, px, py);
                crate::video::terrain_68k(x.c, &t);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        for off in [0x18, 0x1c, 0x1e, 0x16] {
            x.set16(off, 0);
        }
        x.set16(0x1a, -0x38i16 as u16);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 3);
        x.set8(f::ATTACK_ID, 9);
        x.set8(0x2d, 8);
        let (mut dx, mut bounce_vx, mut bounce_ax) = (0x38i16, -0x250i16, -0x500i16);
        if x.u8(f::FLIP) == 0 {
            x.set8(0x2d, 0x18);
            (dx, bounce_vx, bounce_ax) = (-dx, -bounce_vx, -bounce_ax);
        }
        x.add16(0x8, dx as u16);
        x.set16(0x42, bounce_vx as u16);
        x.set16(f::OWNER, bounce_ax as u16);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x36);
        x.anim_start(ANIMS, 0);
    }

    fn fall(x: &mut Cx) {
        x.move_xz();
        x.anim_step();
        x.cull_offscreen();
        if (x.u16(0xc) as i16) < 0 {
            crate::sound::jouer(x.b, sfx::ECE2C, x.o);
            x.set16(0xc, 0);
            x.add8(0x5, 2);
            let (vx, ax) = (x.u16(f::OWNER), x.u16(0x42));
            x.set16(0x14, vx);
            x.set16(0x16, ax);
            x.set8(f::ATTACK_POWER, 6);
            x.set8(f::ATTACK_ID, 8);
            x.set8(f::TIMER, 0x20);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x17);
        }
    }

    fn bounce(x: &mut Cx) {
        x.move_by_table_xy();
        x.anim_step();
        x.cull_offscreen();
        let t = x.u8(f::TIMER).wrapping_sub(1);
        x.set8(f::TIMER, t);
        if (t as i8) < 0 {
            x.add8(0x5, 2);
            crate::sound::jouer(x.b, sfx::EBD30, x.o);
            x.anim_start(ANIMS, 1);
        }
    }

    fn explode(x: &mut Cx) {
        if x.anim_step() {
            x.set16(0x4, 0x400);
        }
    }
}

/// Tir 6 (0x9ebca) : effet de contact collé devant le propriétaire (2 variantes de puissance 6 et
/// 7). Il reste tant que son animation dure et que le propriétaire est en jeu normal.
mod shot6 {
    use super::*;

    const ANIMS: u32 = 0x16ccea;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return;
                }
                if x.u16(f::OWNER) == 0 {
                    return x.free_eshl();
                }
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                x.set8(f::BOX_SET, 7);
                let (id, power) = if x.u8(0x3) == 0 { (0xc, 6) } else { (0xd, 7) };
                x.set8(f::ATTACK_POWER, power);
                x.set8(f::ATTACK_ID, id);
                place_in_front(x, owner, 0x40, 0x1b);
                let variant = x.u8(0x3) as u32;
                x.anim_start(ANIMS, variant as u16);
                x.reinsert();
            }
            2 => {
                if suspended(x) {
                    return;
                }
                if x.u16(f::OWNER) == 0 || x.linked(f::OWNER).u8(x.b, 0x4) as i8 >= 4 {
                    return x.free_eshl();
                }
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return;
                }
                if x.anim_step() {
                    x.set8(0x4, st::FREE);
                }
            }
            _ => x.free_eshl(),
        }
    }
}

/// Tir 7 (0x9ecc0) : étincelle posée sur le propriétaire (décalage lu dans une table à 4 octets
/// par variante) ; disparaît en fin d'animation.
mod shot7 {
    use super::*;

    const ANIMS: u32 = 0x16d1d6;
    const OFFSETS: u32 = 0x9ed64;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if x.u16(f::OWNER) == 0 {
                    return;
                }
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 6);
                x.set8(f::ATTACK_ID, 0x1c);
                let owner = x.linked(f::OWNER);
                copy_position(x, owner);
                x.add16(0x10, 0xffff);
                let row = OFFSETS + 4 * x.u8(0x3) as u32;
                let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
                x.add16(0x8, dx);
                x.add16(0x10, dy);
                x.anim_start(ANIMS, 0);
                x.reinsert();
                wait_end(x);
            }
            2 => wait_end(x),
            _ => x.free_eshl(),
        }
    }

    fn wait_end(x: &mut Cx) {
        if x.anim_step() {
            x.add8(0x4, 2);
        }
    }
}

/// Tir 8 (0x9ee00) : coup de pointe horizontal devant le propriétaire. Avance (X, Y, Z) jusqu'à
/// ce que l'animation lève `+0x33`, puis termine l'animation.
mod shot8 {
    use super::*;

    const ANIMS: u32 = 0x16ccea;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if x.u16(f::OWNER) == 0 {
                    return;
                }
                x.add8(0x4, 2);
                let owner = x.linked(f::OWNER);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 6);
                x.set8(f::ATTACK_ID, 0xe);
                place_in_front(x, owner, 0x46, 0x12);
                x.anim_start(ANIMS, 2);
                body(x);
            }
            2 => body(x),
            _ => x.free_eshl(),
        }
    }

    const THRUST: u8 = 0;
    const RECOVER: u8 = 2;

    fn body(x: &mut Cx) {
        match x.u8(0x5) {
            THRUST => {
                x.move_xyz();
                x.cull_offscreen();
                if x.u8(0x33) != 0 {
                    x.add8(0x5, 2);
                }
                x.anim_step();
            }
            RECOVER => {
                if x.anim_step() {
                    x.set8(0x4, st::FREE);
                }
            }
            _ => {}
        }
        crate::ai::keep_in_zone(x.b, x.o);
        x.call(crate::terrain::SHADOW.entry);
        x.reinsert();
    }
}

/// Tir 9 (0x9ef46) : attaque à trois formes selon la variante `+3` (boule qui retombe, souffle
/// collé à un ennemi, étincelle posée sur un ennemi).
mod shot9 {
    use super::*;

    const ANIMS: u32 = 0x16d39a;

    pub fn run(x: &mut Cx) {
        match x.u8(0x3) {
            0 => bolt(x),
            1 => breath(x),
            _ => sparkle(x),
        }
    }

    fn die(x: &mut Cx) {
        x.set8(0x4, st::FREE);
    }

    // ----- variante 0 : boule qui retombe -----

    /// Se cale sur le propriétaire (devant lui, légèrement plus profond). Sans propriétaire,
    /// l'objet est marqué à détruire (et l'appelant poursuit quand même son initialisation).
    fn attach(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return die(x);
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10, 0xc] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x8, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
        x.add32(0x10, 0x1200);
        x.add16(0xc, -0x1fi16 as u16);
    }

    fn bolt(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                attach(x);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x20);
                x.set16(0x18, -0x400i16 as u16);
                x.set16(0x1a, 0);
                // particularité de l'original : lecture à un décalage fixe de l'objet (A0 - 0x7f92)
                let phase = x.data16(x.o.0.wrapping_sub(0x7f92)) as u32;
                let mut index = (phase >> 2 & 3) + 6;
                if x.u8(f::FLIP) == 0 {
                    index += 0x10;
                }
                x.set8(0x2d, index as u8);
                x.set32(0x14, 0);
                x.set32(0x1c, 0);
                x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1d);
                x.anim_start(ANIMS, 0);
            }
            2 => {
                if !suspended(x) {
                    match x.u8(0x5) {
                        0 => {
                            x.move_by_table_xy();
                            x.move_z();
                            if (x.u16(0xc) as i16) > 0 {
                                x.anim_step();
                            } else {
                                x.add8(0x5, 2);
                                x.set16(0xc, 0);
                                // l'original lit ici le mot bas du long +0x14 (l'accélération)
                                let (vx, vy) = (x.u16(0x16) as i16 >> 1, x.u16(0x1c) as i16 >> 1);
                                x.set16(0x14, vx as u16);
                                x.set16(0x1c, vy as u16);
                                x.set16(0x18, 0);
                                x.set16(0x1a, 0x10);
                                x.anim_start(ANIMS, 1);
                            }
                        }
                        2 => {
                            x.move_xyz();
                            if x.anim_step() {
                                die(x);
                            }
                        }
                        _ => {}
                    }
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    // ----- variante 1 : souffle collé à un ennemi de type 7 -----

    const FIRE_FRAMES: u32 = 0x16d676;
    const END_FRAMES: u32 = 0x16d684;
    /// Hauteurs limites (mots) de la montée du souffle, par valeur de `+0x3e`.
    const RISE_LIMITS: u32 = 0x9f14c;

    /// Le créateur doit être un ennemi (classe 2) de type 7, variante 0, dans l'état 0x208, lui-même
    /// lié à un joueur encore en jeu ; sinon le souffle disparaît.
    fn breath(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(0x2f, 0);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x20);
                x.anim_start(ANIMS, 2);
            }
            2 => {
                if !suspended(x) {
                    if (x.u16(0x46) as i16) >= 0 {
                        return die(x);
                    }
                    let enemy = x.linked(0x46);
                    let (class, kind, variant, state) =
                        (enemy.u8(x.b, 0x28), enemy.u8(x.b, 0x2), enemy.u8(x.b, 0x3), enemy.u16(x.b, 0x4));
                    if class != 2 || kind != 7 || variant != 0 || state != 0x208 {
                        return die(x);
                    }
                    let target = enemy.u16(x.b, 0x46);
                    if (target as i16) >= 0 || Obj(target as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x4) as i8 >= 4 {
                        return die(x);
                    }
                    follow(x, enemy);
                    breath_body(x, enemy);
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn breath_body(x: &mut Cx, enemy: Obj) {
        const RISING: u8 = 0;
        const ENDING: u8 = 2;
        match x.u8(0x5) {
            RISING => {
                if enemy.u8(x.b, 0x6) as i8 > 4 {
                    x.add8(0x5, 2);
                    x.set8(0x6, 0);
                    x.anim_start(ANIMS, 4);
                    return end_frames(x);
                }
                match x.u8(0x6) {
                    0 => {
                        x.anim_step();
                        let z = x.u16(0xc) as i16;
                        let k = x.u8(0x3e) as i8;
                        if z >= 0xa0 || k < 0 || x.rom_i16(RISE_LIMITS + 2 * k as u32) < z {
                            return;
                        }
                        x.add8(0x6, 2);
                        x.anim_start(ANIMS, 3);
                        grow(x);
                    }
                    _ => grow(x),
                }
            }
            ENDING => end_frames(x),
            _ => {}
        }
    }

    fn grow(x: &mut Cx) {
        x.anim_step();
        pick_frame(x, FIRE_FRAMES);
    }

    fn end_frames(x: &mut Cx) {
        if x.anim_step() {
            return die(x);
        }
        pick_frame(x, END_FRAMES);
    }

    /// Colle le souffle à son créateur : position, orientation, un cran plus profond.
    fn follow(x: &mut Cx, enemy: Obj) {
        for off in [0x8, 0xc, 0x10] {
            let v = enemy.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = enemy.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        x.add16(0x12, 1);
        x.add16(0x8, if flip != 0 { 0x3e } else { -0x3ei16 as u16 });
        x.add16(0xc, -0x1fi16 as u16);
    }

    /// Cadre de sprite du souffle : jeu choisi selon la hauteur, cadre selon `+0x3e`.
    fn pick_frame(x: &mut Cx, set: u32) {
        let n = ((x.u16(0xc) >> 4).min(0xa) as i16 - 4).max(0);
        x.set8(0x2f, n as u8);
        let frames = set.wrapping_add(x.data16(set + 2 * n as u32) as i16 as i32 as u32);
        let entry = frames + 8 * x.u8(0x3e) as u32;
        let (a, b) = (x.data32(entry), x.data32(entry + 4));
        x.set32(0x34, a);
        x.set32(0x38, b);
    }

    // ----- variante 2 : étincelle posée sur un ennemi -----

    fn sparkle(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x20);
                x.anim_start(ANIMS, 5);
            }
            2 => {
                if suspended(x) {
                    x.set_bit(0x1, 1, false);
                    return x.reinsert();
                }
                if (x.u16(0x46) as i16) >= 0 {
                    return die(x);
                }
                let host = x.linked(0x46);
                if host.u8(x.b, 0) == 0 {
                    return die(x);
                }
                let hx = host.u16(x.b, 0x8);
                x.set16(0x8, hx);
                let hy = host.u16(x.b, 0x10).wrapping_sub(8);
                x.set16(0x10, hy);
                x.set16(0xc, 0);
                let flip = host.u8(x.b, f::FLIP);
                x.set8(f::FLIP, flip);
                let growth = host.u8(x.b, 0x2f) as u16;
                if growth == 0 {
                    x.set_bit(0x1, 1, true);
                    return x.reinsert();
                }
                let reach = (growth << 4) + 0x74 + (host.u16(x.b, 0xc) & 0xf);
                x.add16(0x8, if flip != 0 { reach } else { reach.wrapping_neg() });
                x.anim_step();
                x.set_bit(0x1, 1, false);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }
}

/// Tir 10 (0x9f376) : projectile de bord (puissance 0x10), posé selon une ligne de table choisie
/// par le registre D5 du dispatch (particularité de l'original). Attend un état du propriétaire,
/// puis laisse 240 images avant de disparaître.
mod shot10 {
    use super::*;

    const ANIMS: u32 = 0x16da84;
    const PLACEMENT: u32 = 0x9f3c8;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::ATTACK_POWER, 0x10);
                x.set8(f::ATTACK_ID, 1);
                x.set8(f::BOX_SET, 0x40);
                x.add32(0x10, u32::MAX);
                let row = PLACEMENT.wrapping_add(x.c.d[5] as u16 as i16 as i32 as u32);
                let (z, dx) = (x.rom16(row), x.rom16(row + 2));
                x.set16(0xc, z);
                let flip = x.u8(f::FLIP) != 0;
                x.add16(0x8, if flip { dx } else { dx.wrapping_neg() });
                x.anim_start(ANIMS, 0);
            }
            2 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x4) as i8 >= 4 {
                    return x.free_eshl();
                }
                match x.u8(0x5) {
                    2 => {
                        x.add8(0x5, 2);
                        x.set16(0x52, 0xf0);
                        x.anim_start(ANIMS, 1);
                    }
                    0 | 1 => {
                        x.anim_step();
                        x.reinsert();
                    }
                    _ => {
                        let t = x.u16(0x52).wrapping_sub(1);
                        x.set16(0x52, t);
                        if (t as i16) < 0 {
                            x.add8(0x4, 2);
                        }
                        x.anim_step();
                        x.reinsert();
                    }
                }
            }
            _ => x.free_eshl(),
        }
    }
}

/// Tir 11 (0x9f426) : halo collé à un ennemi (déplacement lu dans une table à 4 octets par
/// variante). Son animation avance selon les bits 0 et 1 de `+0xb0` du propriétaire ; il peut
/// aussi mettre le propriétaire dans l'état 4 (étourdi) après avoir été gelé (`+0x53`).
mod shot11 {
    use super::*;

    const ANIMS: u32 = 0x169fb0;
    const OFFSETS: u32 = 0x9f524;
    /// Variable `A5+0x6f` : bit 0 = cadence d'animation forcée.
    const FORCE_ANIM: u32 = 0x6f;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                attach(x);
                x.set32(0xc, 0);
                x.set8(f::BOX_SET, 0x3b);
                x.anim_start(ANIMS, 1);
            }
            2 => {
                if frozen(x) {
                    return x.reinsert();
                }
                if x.a5_u8(GAME_PAUSED) == 0 {
                    let owner = attach(x);
                    let flags = owner.u8(x.b, 0xb0);
                    if flags != 0 {
                        let differ = (flags & 1 != 0) != (flags & 2 != 0);
                        if !differ || x.a5_u8(FORCE_ANIM) & 1 != 0 {
                            x.anim_step();
                        }
                    }
                }
                x.reinsert();
            }
            4 => {
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, 0x4) == 4 {
                    if !frozen(x) {
                        attach(x);
                    }
                } else {
                    let v = x.u16(0x54);
                    owner.set_u16(x.b, 0x54, v);
                    owner.set_u32(x.b, 0x4, 0x0400_0000);
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Compte à rebours `+0x53` positif : décompté, et le halo reste figé.
    fn frozen(x: &mut Cx) -> bool {
        let n = x.u8(0x53);
        if (n as i8) > 0 {
            x.set8(0x53, n - 1);
            return true;
        }
        false
    }

    fn attach(x: &mut Cx) -> Obj {
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let row = OFFSETS + (4 * x.u8(0x3) as u32 & 0xff);
        let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
        x.add16(0x8, dx);
        x.add16(0x10, dy);
        owner
    }
}

/// Tir 12 (0x9f5a8) : marque posée sur le point d'attache `+0x5a` (décalage en X et Z lu dans
/// une table, selon la variante) ; l'animation vient d'une table d'octets en ROM.
mod shot12 {
    use super::*;

    const ANIMS: u32 = 0x16dda2;
    const OFFSETS: u32 = 0x9f654;
    const ANIM_OF_VARIANT: u32 = 0x9f5e0;
    /// Point d'attache (adresse RAM sur 16 bits).
    const ANCHOR: u32 = 0x5a;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                attach(x);
                let variant = x.u8(0x3) as u32;
                let anim = x.rom8(ANIM_OF_VARIANT + variant);
                x.anim_start(ANIMS, (anim as u32) as u16);
                animate(x);
            }
            2 => animate(x),
            _ => x.free_eshl(),
        }
    }

    fn animate(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            x.anim_step();
        }
        x.reinsert();
    }

    fn attach(x: &mut Cx) {
        let anchor = x.linked(ANCHOR);
        for off in [0x8, 0xc, 0x10] {
            let v = anchor.u16(x.b, off);
            x.set16(off, v);
        }
        x.add32(0x10, u32::MAX);
        let row = OFFSETS + (4 * x.u8(0x3) as u32 & 0xff);
        let (dx, dz) = (x.rom16(row), x.rom16(row + 2));
        let flip = anchor.u8(x.b, f::FLIP) != 0;
        x.add16(0x8, if flip { dx } else { dx.wrapping_neg() });
        x.add16(0xc, dz);
    }
}

/// Tir 13 (0x9f67c) : effet collé au propriétaire (position, orientation et décalage par
/// variante lus dans une table de 8 octets), dont l'animation (0xc ou 0xe) suit l'octet `+0xa1`
/// du propriétaire. En pause, il suit seulement la position.
mod shot13 {
    use super::*;

    const ANIMS: u32 = 0x17a276;
    const OFFSETS: u32 = 0x9f776;
    const CURRENT_ANIM: u32 = 0x42;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    attach(x);
                    return;
                }
                x.add8(0x4, 2);
                attach(x);
                x.set32(0xc, 0);
                x.set8(f::BOX_SET, 0x29);
                x.anim_start(ANIMS, 0xc);
            }
            2 => {
                let owner = attach(x);
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return;
                }
                let mode = owner.u8(x.b, 0xa1);
                let anim = if mode != 0 { 0xe } else { 0xc };
                if anim != x.u8(CURRENT_ANIM) {
                    x.set8(CURRENT_ANIM, anim);
                    x.anim_start(ANIMS, (anim as u32) as u16);
                }
                if mode != 2 {
                    x.anim_step();
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Colle l'effet au propriétaire. Sans propriétaire, l'original ne fait rien et A1 garde la
    /// valeur laissée par le dispatch (adresse du gestionnaire).
    fn attach(x: &mut Cx) -> Obj {
        if x.u16(f::OWNER) == 0 {
            return Obj(x.c.a[1]);
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10, 0xc] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let row = OFFSETS + (8 * x.u8(0x3) as u32 & 0xff);
        let (dx, dy, dfrac, dz) = (x.rom16(row), x.rom16(row + 2), x.rom16(row + 4), x.rom16(row + 6));
        x.add16(0x8, if flip != 0 { dx.wrapping_neg() } else { dx });
        x.add16(0x10, dy);
        x.add32(0x10, dfrac as i16 as i32 as u32);
        x.add16(0xc, dz);
        owner
    }
}

/// Tir 14 (0x9f786) : partie d'attaque d'un ennemi composite (variante `+3` de 0 à 7, 9 types de
/// formes) attachée à son propriétaire (`+0x50`). Elle lit les drapeaux d'attaque du propriétaire
/// (`+0xa8` : attaque lancée, `+0xa9` : attaque en préparation, un bit par variante) et le pousse
/// dans l'état correspondant, joue l'animation de la phase et crée des tirs de type 15.
mod shot14 {
    use super::*;

    const ANIMS: u32 = 0x17a276;
    const ANIMS_CHARGE: u32 = 0x17a29e;
    const ANIMS_END: u32 = 0x17a2b4;
    const ANIM_OF_VARIANT: u32 = 0x9f7dc;
    const OFFSETS: u32 = 0x9fd50;
    /// Table d'octets (espace de données) des animations du tir de bord.
    const BREATH_ANIMS: u32 = 0x9fe00;
    /// Drapeaux d'attaque du propriétaire.
    const LAUNCHED: u32 = 0xa8;
    const PREPARING: u32 = 0xa9;

    /// Phases (`+4`).
    const START: u8 = 0;
    const ACTIVE: u8 = 2;
    const ENDING: u8 = 4;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            START => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return attach(x);
                }
                if (x.a5_u16(0x7e64) as i16) < 1 {
                    return;
                }
                x.add8(0x4, 2);
                attach(x);
                x.set32(0xc, 0);
                x.set8(f::BOX_SET, 7);
                let variant = x.u8(0x3) as u32;
                let anim = x.rom8(ANIM_OF_VARIANT + variant);
                x.anim_start(ANIMS, (anim as u32) as u16);
            }
            ACTIVE => {
                if x.a5_u8(GAME_PAUSED) != 0 {
                    return attach(x);
                }
                let owner = x.linked(f::OWNER);
                let stalled = owner.u16(x.b, 0x4) != 0x402 && (owner.u8(x.b, 0x53) as i8) > 0;
                if !stalled {
                    match x.u8(0x5) {
                        0 => enter(x, owner),
                        2 => attack(x),
                        _ => {}
                    }
                }
                x.reinsert();
            }
            ENDING => {
                match x.u8(0x5) {
                    0 => finish(x),
                    2 => attach(x),
                    _ => {}
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Colle la partie au propriétaire : position, orientation et décalage selon la variante.
    fn attach(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            // l'original se rappelle lui-même indéfiniment ; ce cas n'existe pas en jeu
            return;
        }
        let owner = x.linked(f::OWNER);
        for off in [0x8, 0x10, 0xc] {
            let v = owner.u32(x.b, off);
            x.set32(off, v);
        }
        let flip = owner.u8(x.b, f::FLIP);
        x.set8(f::FLIP, flip);
        let row = OFFSETS + (8 * x.u8(0x3) as u32 & 0xff);
        let (mut dx, mut dy, mut dfrac, dz) = (x.rom16(row), x.rom16(row + 2), x.rom16(row + 4), x.rom16(row + 6));
        if flip != 0 {
            dx = dx.wrapping_neg();
            if x.u8(0x3) == 2 {
                dx = dx.wrapping_sub(0x24);
                dy = dy.wrapping_sub(0x20);
                dfrac = 0x9000;
            }
        }
        x.add16(0x8, dx);
        x.add16(0x10, dy);
        x.add32(0x10, dfrac as i16 as i32 as u32);
        x.add16(0xc, dz);
    }

    /// Premier sous-état de la phase active : selon la variante et les drapeaux d'attaque du
    /// propriétaire, fixe les états (`+4..+7`) de la partie, puis anime.
    fn enter(x: &mut Cx, owner: Obj) {
        attach(x);
        let v = x.u8(0x3);
        let flags = |x: &mut Cx, off: u32| owner.u8(x.b, off) >> (v & 7) & 1 != 0;
        let (launched, preparing) = (flags(x, LAUNCHED), flags(x, PREPARING));
        let owner_state = owner.u32(x.b, 0x4);
        let remap = |x: &mut Cx, rules: &[(u32, u32)]| {
            for &(from, to) in rules {
                if owner_state == from {
                    x.set32(0x4, to);
                }
            }
        };
        match v {
            0 => {
                if launched {
                    x.set32(0x4, 0x0400_0a00);
                } else if preparing {
                    remap(
                        x,
                        &[
                            (0x0204_0402, 0x0202_0000),
                            (0x0204_0602, 0x0202_0200),
                            (0x0204_0a06, 0x0202_0000),
                            (0x0204_1002, 0x0202_0600),
                            (0x0204_1204, 0x0202_0600),
                            (0x0204_1606, 0x0202_0600),
                        ],
                    );
                }
            }
            2 => {
                if launched {
                    x.set32(0x4, 0x0400_0400);
                }
            }
            3 | 4 => {
                if launched {
                    x.set32(0x4, if v == 3 { 0x0400_0800 } else { 0x0400_0600 });
                } else if preparing {
                    remap(
                        x,
                        &[(0x0204_0804, 0x0202_0400), (0x0204_0a02, 0x0202_0400), (0x0204_1404, 0x0202_0400), (0x0204_1602, 0x0202_0400)],
                    );
                }
            }
            6 => {
                if launched {
                    x.set32(0x4, 0x0400_0000);
                }
            }
            7 => {
                if launched {
                    x.set32(0x4, 0x0400_0200);
                }
            }
            _ => {}
        }
        if v == 2 {
            let anim = if owner.u8(x.b, 0xa1) == 2 { 4 } else { 3 };
            if anim != x.u8(0x5c) {
                x.set8(0x5c, anim);
                x.anim_start(ANIMS, (anim as u32) as u16);
            }
        }
        x.anim_step();
        x.reinsert();
    }

    /// Crée un tir de type 15 lié à cette partie ; l'original n'examine pas l'échec d'allocation.
    fn spawn_child(x: &mut Cx, variant: u8) -> Obj {
        // Glitch d'origine : l'échec de l'allocation n'est pas testé (voir `Pool::alloc_unchecked`).
        let child = x.alloc_unchecked(&pools::ESHL);
        let active = x.u8(0);
        child.set_u8(x.b, 0, active);
        child.set_u8(x.b, 2, 0xf);
        child.set_u8(x.b, 3, variant);
        child.set_u16(x.b, f::OWNER, x.o.0 as u16);
        child
    }

    /// Sous-état 2 de la phase active : cinq enchaînements d'animations selon `+6` / `+7`.
    fn attack(x: &mut Cx) {
        match x.u8(0x6) {
            0 => match x.u8(0x7) {
                0 => begin_charge(x, 3),
                2 => {
                    attach(x);
                    if x.anim_step() {
                        x.add8(0x7, 2);
                        let child = spawn_child(x, 1);
                        x.set16(0x44, child.0 as u16);
                    }
                }
                4 => wait_child(x),
                _ => close(x, 7),
            },
            2 => match x.u8(0x7) {
                0 => begin_charge(x, 3),
                2 => {
                    attach(x);
                    if x.anim_step() {
                        x.add8(0x7, 2);
                        let owner = x.linked(f::OWNER);
                        owner.set_u8(x.b, 0xbd, 0);
                        let child = spawn_child(x, 1);
                        x.set16(0x44, child.0 as u16);
                    }
                }
                4 => {
                    let child = x.u16(0x44);
                    if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x5d) == 1 {
                        x.add8(0x7, 2);
                        let owner = x.linked(f::OWNER);
                        owner.set_u8(x.b, 0xbd, 0);
                        x.anim_start(ANIMS_CHARGE, 4);
                    } else {
                        attach(x);
                        x.anim_step();
                    }
                }
                _ => close(x, 7),
            },
            4 => match x.u8(0x7) {
                0 => {
                    x.add8(0x7, 2);
                    x.set8(f::ATTACK_POWER, 8);
                    x.set8(f::ATTACK_ID, 0xf);
                    crate::sound::jouer(x.b, sfx::E46DE, x.o);
                    breath_anim(x);
                }
                _ => {
                    if x.u8(0x33) != 0 {
                        x.set8(0x33, 0);
                        let owner = x.linked(f::OWNER);
                        owner.set_u8(x.b, 0xbd, 0);
                    }
                    attach(x);
                    if x.anim_step() {
                        x.set32(0x4, 0x0200_0000);
                        let owner = x.linked(f::OWNER);
                        let v = owner.u8(x.b, PREPARING);
                        owner.set_u8(x.b, PREPARING, v & !0x18);
                        let anim = if x.u8(0x3) == 3 { 8 } else { 9 };
                        x.anim_start(ANIMS, anim);
                    }
                }
            },
            _ => match x.u8(0x7) {
                0 => {
                    x.add8(0x7, 2);
                    x.anim_start(ANIMS_CHARGE, 2);
                }
                _ => {
                    if x.u8(0x33) != 0 {
                        crate::sound::jouer(x.b, sfx::E47BC, x.o);
                        x.add8(0x7, 2);
                        let child = spawn_child(x, 0);
                        x.set16(0x42, child.0 as u16);
                    }
                    attach(x);
                    x.anim_step();
                }
            },
        }
    }

    fn begin_charge(x: &mut Cx, anim: u32) {
        x.add8(0x7, 2);
        x.anim_start(ANIMS_CHARGE, anim as u16);
    }

    fn wait_child(x: &mut Cx) {
        let child = x.u16(0x44);
        if child == 0 || Obj(child as i16 as i32 as u32 & 0xff_ffff).u8(x.b, 0x5d) == 1 {
            x.add8(0x7, 2);
            x.anim_start(ANIMS_CHARGE, 4);
        } else {
            attach(x);
            x.anim_step();
        }
    }

    /// Fin d'une charge : rend la main au propriétaire (état 2) et lance l'animation de clôture.
    fn close(x: &mut Cx, anim: u32) {
        attach(x);
        if x.anim_step() {
            x.set32(0x4, 0x0200_0000);
            let owner = x.linked(f::OWNER);
            let v = owner.u8(x.b, PREPARING);
            owner.set_u8(x.b, PREPARING, v & !1);
            owner.set_u8(x.b, 0xbd, 0);
            x.anim_start(ANIMS, anim as u16);
        }
    }

    /// Animation du souffle selon la distance entre le propriétaire et sa cible.
    fn breath_anim(x: &mut Cx) {
        if x.u16(f::OWNER) == 0 {
            return;
        }
        let owner = x.linked(f::OWNER);
        let target = owner.u16(x.b, f::OWNER);
        let mut index = 3u32;
        if target != 0 {
            let target = Obj(target as i16 as i32 as u32 & 0xff_ffff);
            let (a, b) = if owner.u8(x.b, f::FLIP) == 0 {
                (target.u16(x.b, 0x8), owner.u16(x.b, 0x8))
            } else {
                (owner.u16(x.b, 0x8), target.u16(x.b, 0x8))
            };
            index = if (b.wrapping_sub(a) as i16) > 0xfc { 3 } else { 2 };
        }
        index *= 2;
        let first = x.data8(BREATH_ANIMS + index);
        let second = x.data8(BREATH_ANIMS + index + 1);
        let anim = if x.u8(0x3) == 3 { first } else { second };
        x.anim_start(ANIMS_CHARGE, (anim as u32) as u16);
    }

    /// Phase 4, sous-état 0 : relâche le drapeau de préparation du propriétaire et lance
    /// l'animation de fin choisie par `+6`.
    fn finish(x: &mut Cx) {
        x.add8(0x5, 2);
        let v = x.u8(0x3);
        let owner = x.linked(f::OWNER);
        let flags = owner.u8(x.b, PREPARING);
        owner.set_u8(x.b, PREPARING, flags & !(1 << (v & 7)));
        let anim = match x.u8(0x6) {
            0 => 5,
            2 => 6,
            4 => 7,
            6 => 4,
            8 => 3,
            _ => 2,
        };
        x.anim_start(ANIMS_END, anim);
        attach(x);
    }
}

/// Tir 5 (0x9e458) : zone de contact d'un grand ennemi (lien `+0x46` vers cet ennemi). Elle
/// recopie les drapeaux de visibilité de l'ennemi, choisit à chaque image son cadre de sprite
/// (deux tables, l'une pour les images paires, l'autre, avec retournement, pour les impaires quand
/// `+0xb2` de l'ennemi est nul) et ses boîtes de collision (corps, poussée, attaque) selon l'état
/// d'attaque `+0xa0/+0xa1` de l'ennemi. Elle disparaît avec lui.
mod shot5 {
    use super::*;

    const ANIMS: u32 = 0x17b9c2;
    /// Tables de décalages (mots, espace programme) vers des tableaux de cadres (longs).
    const FRAMES_EVEN: u32 = 0x9e638;
    const FRAMES_ODD: u32 = 0x9e6a4;
    /// Choix du cadre impair : octets (cadre, retournement) indexés par `+0x42`, `+0x43`.
    const MIRRORED: u32 = 0x9e6bc;
    /// Boîtes par valeur de `+0x2f` : 4 octets (indice du corps, de la poussée, de l'attaque, puis
    /// un indice mémorisé en `+0x3b`).
    const BOXES: u32 = 0x9e55c;
    const HOST: u32 = 0x46;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        if (x.u16(HOST) as i16) >= 0 {
            return vanish(x);
        }
        let host = x.linked(HOST);
        if host.u8(x.b, 0) == 0 {
            return vanish(x);
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.set8(0x42, 0);
                x.set8(0x43, 0);
                x.set8(0x2d, 0);
                x.anim_start(ANIMS, 0);
                let kind = host.u8(x.b, 0x2);
                x.set8(f::ATTACK_ID, if kind == 0xc { 0x29 } else { 0x2a });
                x.set8(f::BOX_SET, 7);
                boxes(x, host);
            }
            2 => {
                if x.a5_u8(GAME_PAUSED) == 0 {
                    x.set8(0x56, 0);
                    x.set8(0x53, 0);
                    select_frame(x);
                    boxes(x, host);
                }
                let flags = host.u8(x.b, 0x1);
                x.set8(0x1, flags);
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn vanish(x: &mut Cx) {
        x.set8(0x4, st::FREE);
        x.set8(0x5, 0);
    }

    /// Boîtes de collision selon l'état d'attaque de l'ennemi : désarmées (0) pour certains états.
    fn boxes(x: &mut Cx, host: Obj) {
        let row = 4 * x.u8(0x2f) as u32;
        let index = x.rom8(BOXES + 3 + row);
        x.set8(0x3b, index);
        let (kind, detail) = (host.u16(x.b, 0xa0), host.u8(x.b, 0xa1));
        let special = matches!(kind, 0x200 | 0x224 | 0x225 | 0x226) || kind >> 8 == 4;
        let armed = if kind >> 8 == 0 {
            match detail {
                5 | 6 => true,
                7 => !special,
                _ => false,
            }
        } else {
            !special
        };
        for i in 0..3 {
            let v = if armed { x.rom8(BOXES + i + row) } else { 0 };
            x.set8(0x38 + i, v);
        }
    }

    /// Tableau de cadres numéro `set` d'une table de décalages (mots, relatifs à la table).
    fn table(x: &mut Cx, base: u32, set: u32) -> u32 {
        base.wrapping_add(x.rom16(base + 2 * set) as i16 as i32 as u32)
    }

    /// Choisit le cadre du sprite (`+0x34`) ; `+0x2d` est un compteur 1..=0x11.
    fn select_frame(x: &mut Cx) {
        let mut n = x.u8(0x2d) as u16;
        if n >= 0x11 {
            n = 0;
        }
        n += 1;
        x.set8(0x2d, n as u8);
        let host = x.linked(HOST);
        let set = x.u8(0x2f) as u32;
        if host.u8(x.b, 0xb2) != 0 || n & 1 == 0 {
            let frames = table(x, FRAMES_EVEN, set);
            let frame = x.data32(frames + 2 * (n & !1) as u32);
            x.set32(0x34, frame);
        } else if (x.u16(HOST) as i16) < 0 {
            let row = (x.u8(0x42) as u32 >> 1 << 5) + 2 * (x.u8(0x43) as u32 >> 1);
            let flip = x.data8(MIRRORED + row + 1);
            x.set8(f::FLIP, flip);
            // l'original efface D0 avant cette lecture : le cadre vient toujours de la première ligne
            let slot = 4 * x.data8(MIRRORED) as u32;
            let frames = table(x, FRAMES_ODD, set);
            let frame = x.data32(frames + slot);
            x.set32(0x34, frame);
        }
    }
}

handlers! {
    0x9dbfc "eshl_00" shot0::run,
    0x9de10 "eshl_01" shot1::run,
    0x9e0aa "eshl_02" shot2::run,
    0x9e1b6 "eshl_03" shot3::run,
    0x9e2dc "eshl_04" shot4::run,
    0x9ebca "eshl_06" shot6::run,
    0x9ecc0 "eshl_07" shot7::run,
    0x9ee00 "eshl_08" shot8::run,
    0x9ef46 "eshl_09" shot9::run,
    0x9f376 "eshl_10" shot10::run,
    0x9f426 "eshl_11" shot11::run,
    0x9f5a8 "eshl_12" shot12::run,
    0x9f67c "eshl_13" shot13::run,
    0x9f786 "eshl_14" shot14::run,
    0x9e458 "eshl_05" shot5::run,
}
