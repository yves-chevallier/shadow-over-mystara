//! Tirs et attaques des ennemis (classe ESHL), types 40 à 63. Voir `docs/familles/eshl_c.md`.
//!
//! Même cadre que `eshl.rs` : un `mod` par type, `run` est appelé par le dispatch avec l'objet
//! courant ; `+4` est l'état principal, `+5` / `+6` / `+7` les sous-états (octets pairs).

use super::cx::{handlers, Cx};
use super::shell::*;
use crate::mem::wr32;
use crate::objects::Obj;

use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

/// Sous-routines communes de l'IA des ennemis.
mod rt {
}

/// Sons (zone 0xe0836-0xee080).
mod sfx {
    pub const ED186: u32 = 0xed186;
    pub const ED90A: u32 = 0xed90a;
    pub const ED32E: u32 = 0xed32e;
}

/// Mot (adresse RAM sur 16 bits) vu comme un objet : extension de signe, comme `movea.w`.
fn obj_of(word: u16) -> Obj {
    Obj(word as i16 as i32 as u32 & 0xff_ffff)
}

/// Démarre une animation par la routine de l'IA (`0x103d6`).
fn ai_anim(x: &mut Cx, set: u32, n: u32) {
    crate::families::enemy_lib::start(x, set, n);
}

/// Un pas d'animation par la routine de l'IA ; vrai à la fin du script.
fn ai_step(x: &mut Cx) -> bool {
    crate::families::enemy_lib::step(x)
}

/// Alloue un objet ESHL ; `None` si le pool est plein.
/// Tirage pseudo-aléatoire.
fn random(x: &mut Cx) -> u32 {
    let alea = x.random();
    x.c.d[0] & 0xffff_0000 | alea as u32
}

/// Les trois coordonnées 16.16 (X, Y, Z) de `from`, copiées en long.
fn copy_xyz32(x: &mut Cx, from: Obj) {
    for off in [0x8, 0x10, 0xc] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
}

/// Même chose en mots (parties entières seulement).
fn copy_xyz16(x: &mut Cx, from: Obj) {
    for off in [0x8, 0x10, 0xc] {
        let v = from.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Orientation recopiée d'un autre objet.
fn copy_flip(x: &mut Cx, from: Obj) -> u8 {
    let flip = from.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    flip
}

/// Opposé d'un mot.
fn neg(v: u16) -> u16 {
    v.wrapping_neg()
}

/// Octet vu comme signé.
fn s8(v: u8) -> i8 {
    v as i8
}

/// Mot vu comme signé.
fn s16(v: u16) -> i16 {
    v as i16
}

// ---------------------------------------------------------------------------------------------
// Types 40 et 41 : volée de projectiles lancés par un créateur (le créateur est en `+0x5c`)
// ---------------------------------------------------------------------------------------------

/// Tir 40 (0xa31e4) : projectile qui apparaît à la position de son créateur (`+0x5c` ; variantes 0
/// à 2 : décalage en X de 0 / -10 / +10 pixels), choisit une animation au hasard dans une table de
/// 16 octets et dure 90 images (`+0x42`), caché une image sur deux selon la parité de sa variante.
/// Il disparaît quand le propriétaire (`+0x50`) est dans l'état 0x406.
mod shot40 {
    use super::*;

    const ANIMS: u32 = 0x16ff94;
    /// Animation tirée au hasard (16 octets).
    const RANDOM_ANIMS: u32 = 0xa329e;
    /// Valeur de `+0x2b` (palette (h)) par `+0x47`.
    const PALETTES: u32 = 0xa31ba;
    const CREATOR: u32 = 0x5c;
    const LIFETIME: u32 = 0x42;
    const PALETTE_COUNTDOWN: u32 = 0x46;
    const PALETTE_INDEX: u32 = 0x47;
    /// Variable `A5+0x6e` : bit 0 = phase de clignotement globale.
    const BLINK_PHASE: u32 = 0x6e;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn spawn(x: &mut Cx) {
        if x.u16(CREATOR) == 0 {
            return;
        }
        x.add8(0x4, 2);
        crate::sound::jouer(x.b, sfx::ED186, x.o);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::ATTACK_ID, 0x1e);
        x.set16(LIFETIME, 0x5a);
        x.set8(PALETTE_COUNTDOWN, 4);
        let creator = x.linked(CREATOR);
        copy_xyz32(x, creator);
        copy_flip(x, creator);
        let owner = creator.u16(x.b, CREATOR);
        x.set16(f::OWNER, owner);
        match x.u8(0x3) {
            1 => {
                x.add16(0x8, neg(0xa));
                x.add16(0xc, 0xa);
            }
            2 => {
                x.add16(0x8, 0xa);
                x.add16(0xc, 0xa);
            }
            _ => {}
        }
        let r = random(x) & 0xf;
        let anim = x.rom8(RANDOM_ANIMS + r);
        x.anim_start(ANIMS, (anim as u32) as u16);
    }

    fn live(x: &mut Cx) {
        cycle_palette(x);
        if x.linked(f::OWNER).u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        if x.u8(0x5) == 0 {
            x.anim_step();
            let t = x.u16(LIFETIME).wrapping_sub(1);
            x.set16(LIFETIME, t);
            if s16(t) < 0 {
                x.add8(0x4, 2);
                crate::sound::jouer(x.b, sfx::ED90A, x.o);
            }
        }
        flicker(x);
        x.reinsert();
    }

    /// Tous les 5 pas, passe à la palette suivante de la table (8 entrées).
    fn cycle_palette(x: &mut Cx) {
        let n = x.u8(PALETTE_COUNTDOWN).wrapping_sub(1);
        x.set8(PALETTE_COUNTDOWN, n);
        if s8(n) >= 0 {
            return;
        }
        x.set8(PALETTE_COUNTDOWN, 4);
        x.set8(0x2a, 1);
        let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
        x.set8(PALETTE_INDEX, i);
        let p = x.rom8(PALETTES + i as u32);
        x.set8(0x2b, p);
    }

    /// Caché quand la parité de la variante diffère de la phase globale.
    fn flicker(x: &mut Cx) {
        let hidden = (x.u8(0x3) & 1) as u16 != x.a5_u16(BLINK_PHASE) & 1;
        x.set_bit(0x1, 1, hidden);
    }
}

/// Tir 41 (0xa336c) : rayon lancé vers la cible du propriétaire de son créateur (type 40, en
/// `+0x5c`). Direction et vitesse de Z lues selon la distance en X à la cible, recharge une palette
/// de 32 octets, puis, à l'arrivée au sol, crée trois tirs de type 40 (fonction de création
/// 0xa31c2) et finit son animation.
mod shot41 {
    use super::*;

    const ANIMS: u32 = 0x1700f8;
    const CREATOR: u32 = 0x5c;
    /// Vitesse de Z (mot) par tranche de 32 pixels de distance.
    const FALL_SPEEDS: u32 = 0xa3492;
    /// Palette copiée (ROM vers RAM palette) : 8 longs.
    const PALETTE_SRC: u32 = 0x25a6e0;
    const PALETTE_DST: u32 = 0x91c2e0;
    /// Champs de la cible (objet joueur) : position de visée.
    const TARGET_X: u32 = 0xb0;
    const TARGET_Y: u32 = 0xb2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn target(x: &mut Cx) -> Obj {
        let w = x.linked(CREATOR).u16(x.b, CREATOR);
        obj_of(w)
    }

    fn spawn(x: &mut Cx) {
        if x.u16(CREATOR) == 0 {
            return;
        }
        x.add8(0x4, 2);
        crate::sound::jouer(x.b, sfx::ED32E, x.o);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 5);
        x.set8(f::ATTACK_ID, 0x1f);
        let creator = x.linked(CREATOR);
        let owner = creator.u16(x.b, CREATOR);
        x.set16(f::OWNER, owner);
        copy_xyz32(x, creator);
        copy_flip(x, creator);
        x.add16(0x10, 0xffff);
        let t = target(x);
        let (tx, ty) = (t.u16(x.b, TARGET_X), t.u16(x.b, TARGET_Y));
        let dir = crate::aides::direction_from(x.b, x.o, (tx, ty));
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x41);
        let t = target(x);
        let mut dist = s16(x.u16(0x8).wrapping_sub(t.u16(x.b, TARGET_X)));
        if dist < 0 {
            dist = dist.wrapping_neg();
        }
        let band = (1..=11).find(|&i| dist <= 0x20 * i).map_or(0x18, |i| 2 * (i - 1) as u32);
        let speed = x.rom16(FALL_SPEEDS + band);
        x.set16(0x18, speed);
        copy_palette(x);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x17);
        x.anim_start(ANIMS, 0);
    }

    /// Recopie 8 longs de la ROM dans la RAM palette.
    fn copy_palette(x: &mut Cx) {
        for i in 0..8 {
            let v = x.data32(PALETTE_SRC + 4 * i);
            wr32(x.b, PALETTE_DST + 4 * i, v);
        }
    }

    fn live(x: &mut Cx) {
        if x.linked(f::OWNER).u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            0 => fall(x),
            2 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
        x.reinsert();
    }

    fn fall(x: &mut Cx) {
        x.move_xyz();
        if s16(x.u16(0xc)) > 0 {
            x.anim_step();
            return;
        }
        x.add8(0x5, 2);
        x.set16(0xc, 0);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x13);
        x.anim_start(ANIMS, 1);
        crate::families::eshl_creators::appeler(x, crate::families::eshl_creators::ring_28);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 42 et 43 : attaque à éclats lancés depuis un hôte (`+0x42`)
// ---------------------------------------------------------------------------------------------

/// Tir 42 (0xa3568) : attaque collée à son hôte (ennemi en `+0x42`), à 32 pixels devant lui et 64 en
/// hauteur. Elle suit l'animation de l'hôte (`+0x33`), puis, quand l'ennemi la déclenche (`+0x59`),
/// lance une rafale de 5 éclats (type 43) de 8 en 8 images dont l'angle (`+0x5e`) tourne de `+0x5f`
/// (modulo 32), attend 20 images puis finit son animation. Disparaît quand l'hôte passe à l'état 4
/// ou quand il est retombé (Z <= 0, dernier sous-état).
mod shot42 {
    use super::*;

    const ANIMS: u32 = 0x170222;
    /// Animation de l'attaque selon `+0x33` de l'hôte (table d'octets indexée par 7 bits).
    const ANIM_OF_HOST_FRAME: u32 = 0xa3600;
    const HOST: u32 = 0x42;
    const ARMED: u32 = 0x58;
    const TRIGGER: u32 = 0x59;
    const DONE: u32 = 0x5a;
    const SHARDS_LEFT: u32 = 0x46;
    const SHARD_DELAY: u32 = 0x48;
    const ANGLE: u32 = 0x5e;
    const ANGLE_STEP: u32 = 0x5f;
    /// Sous-états (`+5`).
    const IDLE: u8 = 0;
    const FIGHT: u8 = 2;
    const END: u8 = 4;
    /// Phases de la rafale (`+6`).
    const WIND_UP: u8 = 0;
    const WAIT_TRIGGER: u8 = 2;
    const BURST: u8 = 4;
    const PAUSE: u8 = 6;
    const RECOVER: u8 = 8;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        follow_host(x);
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                ai_anim(x, ANIMS, 1);
            }
            2 => live(x),
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    /// Se cale devant l'hôte (32 pixels côté regard, 64 en hauteur, un cran plus près).
    fn follow_host(x: &mut Cx) {
        let host = x.linked(HOST);
        copy_xyz16(x, host);
        x.add16(0x10, 1);
        let dx = if x.u8(f::FLIP) != 0 { neg(0x20) } else { 0x20 };
        x.add16(0x8, dx);
        x.add16(0xc, 0x40);
    }

    fn live(x: &mut Cx) {
        let host = x.linked(HOST);
        if host.u8(x.b, 4) == 4 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            IDLE => idle(x, host),
            FIGHT => fight(x),
            END => {
                if s16(host.u16(x.b, 0xc)) <= 0 {
                    x.free_eshl();
                }
            }
            _ => {}
        }
    }

    fn idle(x: &mut Cx, host: Obj) {
        if x.u8(ARMED) != 0 {
            if x.u8(0x3) != 2 {
                return x.free_eshl();
            }
            return x.add8(0x5, 2);
        }
        if x.u8(0x3) == 0 {
            return;
        }
        let frame = host.u8(x.b, 0x33) & 0x7f;
        let anim = x.rom8(ANIM_OF_HOST_FRAME + frame as u32);
        ai_anim(x, ANIMS, anim as u32);
    }

    fn fight(x: &mut Cx) {
        match x.u8(0x6) {
            WIND_UP => {
                if ai_step(x) {
                    x.add8(0x6, 2);
                }
            }
            WAIT_TRIGGER => {
                if x.u8(TRIGGER) == 0 {
                    return;
                }
                x.add8(0x6, 2);
                x.set16(SHARDS_LEFT, 5);
                x.set16(SHARD_DELAY, 1);
                // angle de départ et sens de rotation : selon le côté et la hauteur du propriétaire
                let (mut a, mut step, mut a2, mut step2) = (0x16u8, 1u8, 0x1au8, 0xffu8);
                if x.u8(f::FLIP) != 0 {
                    (a, step, a2, step2) = (0xa, 0xff, 6, 1);
                }
                let owner = x.linked(f::OWNER);
                if s16(owner.u16(x.b, 0x10)) > s16(x.u16(0x10)) {
                    (a, step) = (a2, step2);
                }
                x.set8(ANGLE, a);
                x.set8(ANGLE_STEP, step);
            }
            BURST => {
                let t = x.u16(SHARD_DELAY).wrapping_sub(1);
                x.set16(SHARD_DELAY, t);
                if s16(t) > 0 {
                    return;
                }
                spawn_shard(x);
                x.set16(SHARD_DELAY, 8);
                let n = x.u16(SHARDS_LEFT).wrapping_sub(1);
                x.set16(SHARDS_LEFT, n);
                if s16(n) > 0 {
                    return;
                }
                x.add8(0x6, 2);
                x.set16(SHARDS_LEFT, 0x14);
            }
            PAUSE => {
                let n = x.u16(SHARDS_LEFT).wrapping_sub(1);
                x.set16(SHARDS_LEFT, n);
                if s16(n) > 0 {
                    return;
                }
                x.add8(0x6, 2);
                ai_anim(x, ANIMS, 6);
            }
            RECOVER => {
                if ai_step(x) {
                    x.add8(0x5, 2);
                    x.set8(DONE, 1);
                }
            }
            _ => {}
        }
    }

    /// Crée un éclat (type 43) devant l'attaque, avec l'angle courant, puis tourne l'angle.
    fn spawn_shard(x: &mut Cx) {
        let Some(c) = x.alloc(&pools::ESHL) else { return };
        c.set_u8(x.b, 0, 1);
        c.set_u8(x.b, 2, 0x2b);
        let (px, pz) = (x.u16(0x8), x.u16(0xc));
        c.set_u16(x.b, 0x8, px);
        c.set_u16(x.b, 0xc, pz.wrapping_sub(0x40));
        let y = x.u32(0x10);
        c.set_u32(x.b, 0x10, y);
        let y_low = c.u16(x.b, 0x12).wrapping_add(1);
        c.set_u16(x.b, 0x12, y_low);
        let flip = x.u8(f::FLIP);
        c.set_u8(x.b, f::FLIP, flip);
        let dx = if flip != 0 { 0x2a } else { neg(0x2a) };
        let cx = c.u16(x.b, 0x8).wrapping_add(dx);
        c.set_u16(x.b, 0x8, cx);
        let angle = x.u8(ANGLE);
        c.set_u8(x.b, 0x2d, angle);
        let step = x.u8(ANGLE_STEP);
        x.set8(ANGLE, angle.wrapping_add(step) & 0x1f);
    }
}

/// Tir 43 (0xa3762) : éclat de l'attaque 42, qui part dans la direction `+0x2d` (table de vitesses
/// 0x40) en animation 0x10 + une valeur de la table 0xa37ba indexée par la direction / 2. Détruit au
/// contact (touché ou paré) ou en sortant de la fenêtre de l'écran.
mod shot43 {
    use super::*;

    const ANIMS: u32 = 0x170222;
    const ANIM_OF_DIRECTION: u32 = 0xa37ba;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.select_speed(&crate::objects::motion::SPEEDS_B, 0x40);
                let dir = x.u8(0x2d) as u32 >> 1;
                let extra = x.rom8(ANIM_OF_DIRECTION + dir);
                ai_anim(x, ANIMS, 0x10u32.wrapping_add(extra as u32) & 0xff);
            }
            2 => fly(x),
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    fn fly(x: &mut Cx) {
        if x.u8(0x5) != 0 {
            return;
        }
        if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
            return x.free_eshl();
        }
        x.move_xyz();
        if crate::ai_aids::outside_band(x.b, x.o, neg(0x50), 0x1d0) {
            x.free_eshl();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Types 44 et 45 : faisceau entre deux objets et débris
// ---------------------------------------------------------------------------------------------

/// Tir 44 (0xa380a) : faisceau tendu entre deux objets liés (variantes 0 et 1). `+0x46` est le
/// créateur (qui compte ses faisceaux dans son octet `+0x42`) et le créateur du créateur est
/// l'autre extrémité. Le faisceau se déplace de l'extrémité A2 vers l'extrémité A1 en 16 images
/// (position = A2 + t * (A1 - A2) / 16, `+0x14` = t) ; touché (bit 3 de `+0x57`) il joue son
/// animation de fin. La variante 1 démarre posée sur A2 (50 pixels plus haut), l'animation de départ
/// dépend de `+0x2f` de A1, et l'animation de fin garde le faisceau en vie jusqu'à son terme.
mod beam44 {
    use super::*;

    const ANIMS: u32 = 0x17f002;
    const CREATOR: u32 = 0x46;
    /// Compte de pas du faisceau.
    const STEP: u32 = 0x14;
    /// Compteur de faisceaux du créateur.
    const BEAM_COUNT: u32 = 0x42;
    /// Hauteur au-dessus de A2 de la position de visée en Z.
    const RAISE_Z: u16 = 0x50;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x3) {
            0 => variant0(x),
            1 => variant1(x),
            _ => {}
        }
    }

    /// Objet vivant désigné par un mot (adresse négative et octet d'activité non nul).
    fn live_link(x: &mut Cx, word: u16) -> Option<Obj> {
        if s16(word) >= 0 {
            return None;
        }
        let o = obj_of(word);
        (o.u8(x.b, 0) != 0).then_some(o)
    }

    /// Le faisceau est terminé : état suivant.
    fn end(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
    }

    /// Les deux extrémités (créateur et créateur du créateur).
    fn ends(x: &mut Cx) -> Option<(Obj, Obj)> {
        let w = x.u16(CREATOR);
        let a1 = live_link(x, w)?;
        let w = a1.u16(x.b, CREATOR);
        let a2 = live_link(x, w)?;
        Some((a1, a2))
    }

    /// Le créateur compte un faisceau de plus.
    fn count_beam(x: &mut Cx, a1: Obj) {
        let n = a1.u8(x.b, BEAM_COUNT).wrapping_add(1);
        a1.set_u8(x.b, BEAM_COUNT, n);
    }

    fn variant0(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                end(x);
                x.set8(STEP, 0);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x20);
                let w = x.u16(CREATOR);
                let Some(a1) = live_link(x, w) else { return end(x) };
                count_beam(x, a1);
                ai_anim(x, ANIMS, 0xd);
            }
            2 => beam(x, 0xf, false),
            4 => release(x),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn variant1(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                end(x);
                x.set8(STEP, 0);
                let w = x.u16(CREATOR);
                let Some(a1) = live_link(x, w) else { return end(x) };
                count_beam(x, a1);
                let w = a1.u16(x.b, CREATOR);
                let Some(a2) = live_link(x, w) else { return end(x) };
                let (px, py, pz) = (a2.u16(x.b, 0x8), a2.u16(x.b, 0x10), a2.u16(x.b, 0xc));
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.set16(0xc, pz.wrapping_add(RAISE_Z));
                copy_flip(x, a2);
                x.set8(0x2f, 0);
                let anim = if a1.u8(x.b, 0x2f) != 0 { 0x1a } else { 0x18 };
                ai_anim(x, ANIMS, anim);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x20);
            }
            2 => beam(x, 0x1b, true),
            4 => release(x),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// État 2. `hit_anim` : animation de fin lancée au contact ; `lingers` : variante 1, où la fin
    /// de l'animation de fin garde le faisceau (et où l'animation en cours le termine).
    fn beam(x: &mut Cx, hit_anim: u32, lingers: bool) {
        if x.a5_u8(GAME_PAUSED) != 0 {
            return x.reinsert();
        }
        if x.u8(0x5) != 0 {
            if ai_step(x) == lingers {
                return x.reinsert();
            }
            return end(x);
        }
        if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
            x.add8(0x5, 2);
            return ai_anim(x, ANIMS, hit_anim);
        }
        let Some((a1, a2)) = ends(x) else { return end(x) };
        interpolate(x, a1, a2);
        x.add8(STEP, 1);
        if s8(x.u8(STEP)) > 0xf {
            return end(x);
        }
        ai_step(x);
        x.reinsert();
    }

    /// Position = A2 + t * (A1 - A2) / 16 (arithmétique en mots), t = `+0x14` ; rien si t = 0.
    /// En Z, la différence est mesurée 0x50 plus haut.
    fn interpolate(x: &mut Cx, a1: Obj, a2: Obj) {
        let t = x.u8(STEP) as u16;
        if t == 0 {
            return;
        }
        for (off, raise) in [(0x8, 0), (0xc, RAISE_Z), (0x10, 0)] {
            let base = a2.u16(x.b, off);
            let d = s16(a1.u16(x.b, off).wrapping_sub(base).wrapping_sub(raise)) >> 4;
            let pos = (d as u16).wrapping_mul(t).wrapping_add(base).wrapping_add(raise);
            x.set16(off, pos);
        }
    }

    /// État 4 : rend un faisceau à son créateur.
    fn release(x: &mut Cx) {
        let w = x.u16(CREATOR);
        if let Some(a1) = live_link(x, w) {
            let n = a1.u8(x.b, BEAM_COUNT).wrapping_sub(1);
            a1.set_u8(x.b, BEAM_COUNT, n);
        }
        x.add8(0x4, 2);
    }
}

/// Tir 45 (0xa3b88) : débris projetés par un objet (`+0x46`), deux variantes. Variante 0 : jaillit de
/// l'objet avec une vitesse tirée au hasard dans une table de 8 lignes, rebondit une fois puis
/// disparaît à la fin de l'animation. Variante 1 : éclat posé sur l'objet (décalage selon `+0x2f`),
/// animé tant que l'objet est dans l'état `+0xa4 = 4`, puis tombe et disparaît au sol. Les débris
/// sont retenus dans l'écran (0xfbb0).
mod debris45 {
    use super::*;

    const ANIMS: u32 = 0x17f002;
    const SOURCE: u32 = 0x46;
    /// Lignes de 4 mots (VX, AX, VZ, VY) pour la variante 0 (espace de données).
    const BURST_SPEEDS: u32 = 0xa3e30;
    const BURST_ANIMS: u32 = 0xa3c3a;
    const HOP_VZ: u32 = 0xa3c94;
    /// Miroir de `+0x2f` pour l'objet retourné.
    const MIRROR: u32 = 0xa3d10;
    /// Décalages de la variante 1 par `+0x2f` : X, puis Z par lecture décalée de deux octets.
    const PLACE_OFFSETS: u32 = 0xa3dc2;
    /// Marge de la retenue à l'écran.
    const MARGIN: u32 = 0x10;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x3) {
            0 => variant0(x),
            1 => variant1(x),
            _ => {}
        }
    }

    /// L'objet disparaît : état 6.
    fn vanish(x: &mut Cx) {
        x.set8(0x4, 6);
        x.set8(0x5, 0);
    }

    /// Source (adresse négative), éventuellement vivante.
    fn source(x: &mut Cx, check_active: bool) -> Option<Obj> {
        let word = x.u16(SOURCE);
        if s16(word) >= 0 {
            return None;
        }
        let o = obj_of(word);
        if check_active && o.u8(x.b, 0) == 0 {
            return None;
        }
        Some(o)
    }

    fn variant0(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(src) = source(x, true) else { return vanish(x) };
                copy_xyz16(x, src);
                copy_flip(x, src);
                x.add16(0xc, 0x50);
                let r = random(x);
                let row = BURST_SPEEDS + (r & 0x38);
                let (mut vx, mut ax) = (x.data16(row), x.data16(row + 2));
                if x.u8(f::FLIP) == 0 {
                    (vx, ax) = (neg(vx), neg(ax));
                }
                x.set16(0x14, vx);
                x.set16(0x16, ax);
                let vz = x.data16(row + 4);
                x.set16(0x18, vz);
                x.set16(0x1a, 0);
                let vy = x.data16(row + 6);
                x.set16(0x1c, vy);
                x.set16(0x1e, 0);
                let anim = x.rom8(BURST_ANIMS + (r & 3));
                ai_anim(x, ANIMS, anim as u32);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0);
            }
            2 => {
                if ai_step(x) {
                    return vanish(x);
                }
                if x.u8(0x5) == 0 && x.u8(f::HIT_FLAGS) & 0x8 == 0 && x.u8(0x33) == 0 {
                    x.add8(0x5, 2);
                    x.set32(0x14, 0);
                    x.set32(0x1c, 0);
                    let r = random(x) & 6;
                    let vz = x.rom16(HOP_VZ + r);
                    x.set16(0x18, vz);
                    x.set16(0x1a, 0);
                    ai_anim(x, ANIMS, 0x11);
                }
                x.move_xyz();
                settle(x);
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Fin commune : retenu à l'écran, puis remis dans la liste triée.
    fn settle(x: &mut Cx) {
        crate::ai::keep_on_screen(x.b, x.o, MARGIN as u16);
        x.reinsert();
    }

    fn variant1(x: &mut Cx) {
        let Some(src) = source(x, false) else { return vanish(x) };
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let flip = copy_flip(x, src);
                if flip != 0 {
                    let k = x.u8(0x2f) as u32;
                    let mirrored = x.rom8(MIRROR + k);
                    x.set8(0x2f, mirrored);
                }
                let k = x.u8(0x2f);
                x.set8(0x2d, k.wrapping_mul(4));
                x.select_speed(&crate::objects::motion::SPEEDS_A, 0x1f);
                place(x, src);
                x.set8(f::ATTACK_ID, 0x14);
                x.anim_start(ANIMS, 0x1f);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        if src.u8(x.b, 0xa4) == 4 {
                            x.anim_step();
                        } else {
                            x.add8(0x5, 2);
                            x.set32(0x14, 0);
                            x.set32(0x1c, 0);
                            x.set16(0x18, neg(0x300));
                            x.set16(0x1a, 0);
                            x.anim_start(ANIMS, 0x21);
                        }
                    }
                    2 => {
                        x.anim_step();
                        x.move_by_table_xy();
                        x.move_z();
                        if s16(x.u16(0xc)) < 0 {
                            x.set16(0xc, 0);
                            x.add8(0x4, 2);
                        }
                    }
                    _ => {}
                }
                settle(x);
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Pose l'éclat sur la source : X et Y de la source, Z + 0x30, puis décalage selon `+0x2f`.
    fn place(x: &mut Cx, src: Obj) {
        let (sx, sy, sz) = (src.u16(x.b, 0x8), src.u16(x.b, 0x10), src.u16(x.b, 0xc));
        x.set16(0x8, sx);
        x.set16(0x10, sy);
        x.set16(0xc, sz.wrapping_add(0x30));
        let k = 2 * (x.u8(0x2f) & 7) as u32;
        let dx = x.rom16(PLACE_OFFSETS + k);
        // la ligne des Z est la même table lue avec un décalage de deux octets, indexée par dx
        let dz = x.rom16((PLACE_OFFSETS + 2).wrapping_add(dx as i16 as i32 as u32));
        x.add16(0x8, dx);
        x.add16(0xc, dz);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 46 à 50
// ---------------------------------------------------------------------------------------------

/// Tir 46 (0xa3e72) : objet de décor mobile (animation choisie par la variante) qui défile de 4
/// pixels par image vers la gauche et disparaît quand il dépasse le bord gauche de l'écran
/// (`A5+0x506` - 0x50).
mod scroller46 {
    use super::*;

    const ANIMS: u32 = 0x1703e6;
    /// Bord gauche de l'écran (variable `A5+0x506`).
    const SCREEN_LEFT: u32 = 0x506;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                let variant = x.u8(0x3) as u32;
                x.anim_start(ANIMS, variant as u16);
                x.reinsert();
            }
            2 => {
                x.add16(0x8, neg(4));
                let edge = x.a5_u16(SCREEN_LEFT).wrapping_sub(0x50);
                if s16(edge) > s16(x.u16(0x8)) {
                    x.free_eshl();
                }
            }
            _ => x.free_eshl(),
        }
    }
}

/// Tir 47 (0xa3ebe) : apparition invisible, décalée au hasard autour du bord de la caméra
/// (`A5+0x5a6` en X, `A5+0x5aa` en Y), qui attend un délai tiré d'une table (`+0x58`), devient
/// visible puis joue son animation jusqu'au bout.
mod spawn47 {
    use super::*;

    const ANIMS: u32 = 0x16fbf4;
    /// Tables de 16 mots de décalages : en X (dans la bande / hors bande), en Y (idem), délais.
    const X_INSIDE: u32 = 0xa3fd4;
    const X_OUTSIDE: u32 = 0xa3ff4;
    const Y_INSIDE: u32 = 0xa4014;
    const Y_OUTSIDE: u32 = 0xa4034;
    const DELAYS: u32 = 0xa4068;
    const CAMERA_X: u32 = 0x5a6;
    const CAMERA_Y: u32 = 0x5aa;
    const DELAY: u32 = 0x58;
    const WAIT: u8 = 0;
    const PLAY: u8 = 2;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x1e);
                x.set_bit(0x1, 1, true);
                place(x);
                let r = random(x);
                let delay = x.rom16(DELAYS + 2 * (r & 0xf));
                x.set16(DELAY, delay);
                ai_anim(x, ANIMS, 1);
            }
            2 => match x.u8(0x5) {
                WAIT => {
                    let t = x.u16(DELAY).wrapping_sub(1);
                    x.set16(DELAY, t);
                    if s16(t) <= 0 {
                        x.add8(0x5, 2);
                        x.set_bit(0x1, 1, false);
                    }
                }
                PLAY => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    /// Décale la position au hasard : ramène vers l'écran si elle en est trop loin.
    fn place(x: &mut Cx) {
        let idx = 2 * (random(x) & 0xf);
        let cam = x.a5_u16(CAMERA_X);
        let (lo, hi) = (cam.wrapping_add(0x20), cam.wrapping_add(0x160));
        let px = x.u16(0x8);
        let dx = if s16(lo) >= s16(px) {
            x.rom16(X_OUTSIDE + idx)
        } else if s16(hi) <= s16(px) {
            neg(x.rom16(X_OUTSIDE + idx))
        } else {
            x.rom16(X_INSIDE + idx)
        };
        x.add16(0x8, dx);
        let idx = 2 * (random(x) & 0xf);
        let cam = x.a5_u16(CAMERA_Y);
        let (lo, hi) = (cam.wrapping_add(0x38), cam.wrapping_add(0x68));
        let py = x.u16(0x10);
        let dy = if s16(lo) >= s16(py) {
            x.rom16(Y_OUTSIDE + idx)
        } else if s16(hi) <= s16(py) {
            neg(x.rom16(Y_OUTSIDE + idx))
        } else {
            x.rom16(Y_INSIDE + idx)
        };
        x.add16(0x10, dy);
    }
}

/// Tir 48 (0xa4088) : projectile lancé en cloche par un lanceur (`+0x5c`, dont il compte les tirs
/// retombés dans son octet `+0x1d`). Attend 16 images, monte (VZ = -0x100) jusqu'à Z <= 0x20, attend
/// 16 images, puis redescend à la hauteur de départ (`+0x4e`) ; il peut alors détruire l'objet
/// repéré en `+0x58`. Il libère ensuite son emplacement dans le lanceur (mot `+8+variante`).
mod lob48 {
    use super::*;

    const ANIMS: u32 = 0x17041e;
    const LAUNCHER: u32 = 0x5c;
    const WAIT: u32 = 0x4c;
    const HOME_Z: u32 = 0x4e;
    const TARGET: u32 = 0x58;
    /// Création d'un tir joueur de type 0x18 lié à cet objet ; Z levé si échec (l'original rend
    /// toujours Z levé).
    const SPAWN_PLAYER_SHOT: u32 = 0x54872;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(WAIT, 0x10);
                let z = x.u16(0xc);
                x.set16(HOME_Z, z);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x17);
                x.anim_start(ANIMS, 1);
                x.reinsert();
            }
            2 => {
                sub_state(x);
                if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
                    x.set8(f::HIT_FLAGS, 0);
                    let launcher = x.linked(LAUNCHER);
                    let n = launcher.u8(x.b, 0x1d).wrapping_add(1);
                    launcher.set_u8(x.b, 0x1d, n);
                }
            }
            4 => {
                x.add8(0x4, 2);
                let launcher = x.linked(LAUNCHER);
                let slot = (8u8.wrapping_add(x.u8(0x3))) as u32;
                launcher.set_u16(x.b, slot, 0);
            }
            _ => x.free_eshl(),
        }
    }

    fn sub_state(x: &mut Cx) {
                match x.u8(0x5) {
                    0 => {
                        if dec_wait(x) {
                            return;
                        }
                        x.add8(0x5, 2);
                        x.set16(0x18, neg(0x100));
                        x.set16(0x1a, neg(0x20));
                        x.call(SPAWN_PLAYER_SHOT);
                        if x.c.cond(6) {
                            let shot = x.c.a[1] as u16;
                            x.set16(TARGET, shot);
                        }
                    }
                    2 => {
                        x.move_z();
                        if s16(x.u16(0xc)) <= 0x20 {
                            x.add8(0x5, 2);
                            x.set16(WAIT, 0x10);
                        }
                    }
                    4 => {
                        if dec_wait(x) {
                            return;
                        }
                        x.add8(0x5, 2);
                        x.set16(0x18, 0x180);
                        x.set16(0x1a, 0);
                    }
                    6 => {
                        x.move_z();
                        if s16(x.u16(HOME_Z)) > s16(x.u16(0xc)) {
                            return;
                        }
                        x.add8(0x4, 2);
                        if x.u16(TARGET) != 0 {
                            let t = x.linked(TARGET);
                            t.set_u8(x.b, 0x4, 6);
                        }
                    }
                    _ => {}
                }
    }

    /// Décompte du délai `+0x4c` : vrai tant qu'il n'est pas écoulé.
    fn dec_wait(x: &mut Cx) -> bool {
        let t = x.u16(WAIT).wrapping_sub(1);
        x.set16(WAIT, t);
        t != 0
    }
}

/// Tir 49 (0xa418e) : coup porté à 0x6e pixels devant l'ennemi qui l'a créé (`+0x42`), à hauteur
/// 0x35. Il joue l'animation de départ tant que l'hôte n'est pas passé au sous-état 6 (`+7`) ; il
/// finit son animation ensuite, ou disparaît si l'hôte est étourdi (état 4) sans que l'animation
/// ait atteint son repère (`+0x33`).
mod strike49 {
    use super::*;

    const ANIMS: u32 = 0x170468;
    const HOST: u32 = 0x42;
    const SOUND: u32 = 0xed398;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_POWER, 0x14);
                let host = x.linked(HOST);
                let px = host.u16(x.b, 0x8);
                x.set16(0x8, px);
                x.set16(0xc, 0x35);
                let py = host.u16(x.b, 0x10);
                x.set16(0x10, py);
                x.add32(0x10, 2);
                let flip = copy_flip(x, host);
                x.add16(0x8, if flip != 0 { 0x6e } else { neg(0x6e) });
                crate::sound::jouer(x.b, SOUND, x.o);
                ai_anim(x, ANIMS, 0);
            }
            2 => match x.u8(0x5) {
                0 => {
                    let host = x.linked(HOST);
                    if host.u8(x.b, 7) == 6 {
                        return finish(x);
                    }
                    if host.u8(x.b, 4) == 4 {
                        if x.u8(0x33) != 0 {
                            finish(x);
                        } else {
                            x.free_eshl();
                        }
                    } else {
                        ai_step(x);
                    }
                }
                2 => {
                    if ai_step(x) {
                        x.free_eshl();
                    }
                }
                _ => {}
            },
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            x.reinsert();
        }
    }

    /// Passe à l'animation finale.
    fn finish(x: &mut Cx) {
        x.add8(0x5, 2);
        x.set8(0x6, 0);
        ai_anim(x, ANIMS, 2);
    }
}

/// Tir 50 (0xa4266) : frappes d'un ennemi (`+0x42`) en deux variantes (0 : balle qui part en cloche
/// de 0x70 pixels devant l'hôte ; 1 : balle lente à 0x76 pixels). L'animation avance par phases
/// (`+0x5a`, durée de phase `+0x5b` lue dans la table 0xa44aa) avec la valeur d'animation
/// `phase + 3`. Quand l'hôte est étourdi (état 4) ou que sa trajectoire s'achève, passe à
/// l'animation `phase + 6` (sous-état 4) puis disparaît. Retenu dans l'écran et testé par 0x33fe4
/// à chaque image.
mod bullet50 {
    use super::*;

    const ANIMS: u32 = 0x170468;
    const HOST: u32 = 0x42;
    const SOUND: u32 = 0xed398;
    /// Durée des phases d'animation (octets).
    const PHASE_LENGTHS: u32 = 0xa44aa;
    const PHASE: u32 = 0x5a;
    const PHASE_TIMER: u32 = 0x5b;
    const FALL_TIMER: u32 = 0x58;
    /// Sauvegarde de `+0x3b` (priorité du cadre) à travers l'animation.
    const SAVED_PRIORITY: u32 = 0x5e;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => match x.u8(0x3) {
                0 => launch(x, 0x12, 0x70, 0x1e, neg(0x380), true),
                _ => launch(x, 0x17, 0x76, 0x2b, 0x40, false),
            },
            2 => match x.u8(0x3) {
                0 => lob(x),
                _ => slow(x),
            },
            _ => x.free_eshl(),
        }
        if x.u8(0) != 0 {
            crate::ai::keep_in_zone(x.b, x.o);
            x.call(crate::terrain::SHADOW.entry);
            x.reinsert();
        }
    }

    /// Création : variante 0 (`lob = true`) ou 1.
    fn launch(x: &mut Cx, speed: u32, dx: u16, dz: u16, vz: u16, lob: bool) {
        x.add8(0x4, 2);
        if !lob {
            x.set16(FALL_TIMER, 0x28);
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0x14);
        x.select_speed(&crate::objects::motion::SPEEDS_B, speed as u16);
        let host = x.linked(HOST);
        copy_xyz16(x, host);
        let flip = x.u8(f::FLIP);
        x.add16(0x8, if flip != 0 { dx as u16 } else { neg(dx as u16) });
        x.add16(0xc, dz);
        x.add32(0x10, 2);
        x.set16(0x18, vz);
        crate::sound::jouer(x.b, SOUND, x.o);
        start_phase(x);
    }

    /// Avance d'une phase quand sa durée est écoulée (et qu'on n'est pas déjà en phase 2).
    fn phase_tick(x: &mut Cx) {
        if s8(x.u8(PHASE)) < 2 {
            let n = x.u8(PHASE_TIMER).wrapping_sub(1);
            x.set8(PHASE_TIMER, n);
            if s8(n) <= 0 {
                x.add8(PHASE, 1);
                start_phase(x);
                return;
            }
        }
        step_phase(x);
    }

    /// Démarre l'animation de la phase courante, puis un pas.
    fn start_phase(x: &mut Cx) {
        let phase = x.u8(PHASE);
        let len = x.rom8(PHASE_LENGTHS + phase as u32);
        x.set8(PHASE_TIMER, len);
        ai_anim(x, ANIMS, phase as u32 + 3);
        let p = x.u8(0x3b);
        x.set8(SAVED_PRIORITY, p);
        step_phase(x);
    }

    fn step_phase(x: &mut Cx) {
        ai_step(x);
        let p = x.u8(0x3b);
        x.set8(SAVED_PRIORITY, p);
    }

    fn restore_priority(x: &mut Cx) {
        let p = x.u8(SAVED_PRIORITY);
        x.set8(0x3b, p);
    }

    /// L'hôte est étourdi : passe à l'animation de fin (`phase + 6`), sous-état 4 pour la variante 0
    /// (état 2, sous-état 4), prochain sous-état pour la variante 1.
    fn host_stunned(x: &mut Cx) -> bool {
        let host = x.linked(HOST);
        host.u8(x.b, 4) == 4
    }

    fn end_animation(x: &mut Cx) {
        let phase = x.u8(PHASE) as u32;
        ai_anim(x, ANIMS, phase + 6);
    }

    fn lob(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                if host_stunned(x) {
                    x.set16(0x4, 0x204);
                    return end_animation(x);
                }
                x.move_xyz();
                phase_tick(x);
                if s16(x.u16(0xc)) > 0 {
                    return restore_priority(x);
                }
                x.add8(0x5, 2);
                x.select_speed(&crate::objects::motion::SPEEDS_B, 0x17);
                x.set16(FALL_TIMER, 0x1a);
                x.set16(0xc, 0);
                restore_priority(x);
            }
            2 => {
                if host_stunned(x) {
                    x.set16(0x4, 0x204);
                    return end_animation(x);
                }
                x.move_xy();
                let t = x.u16(FALL_TIMER).wrapping_sub(1);
                x.set16(FALL_TIMER, t);
                if s16(t) <= 0 {
                    x.set16(0x4, 0x204);
                    return end_animation(x);
                }
                phase_tick(x);
                restore_priority(x);
            }
            4 => {
                if ai_step(x) {
                    x.free_eshl();
                }
            }
            _ => {}
        }
    }

    fn slow(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                if host_stunned(x) {
                    return end_slow(x);
                }
                x.move_xyz();
                phase_tick(x);
                let t = x.u16(FALL_TIMER).wrapping_sub(1);
                x.set16(FALL_TIMER, t);
                if s16(t) <= 0 {
                    return end_slow(x);
                }
                restore_priority(x);
            }
            2 => {
                x.move_xy();
                if ai_step(x) {
                    x.free_eshl();
                }
            }
            _ => {}
        }
    }

    fn end_slow(x: &mut Cx) {
        x.add8(0x5, 2);
        end_animation(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 51 et 52
// ---------------------------------------------------------------------------------------------

/// Tir 51 (0xa4522) : projectile tombant du haut de l'écran, lancé par un lanceur (`+0x5c`, dont il
/// compte les tirs retombés dans son octet `+0x1d`). Attend `+0x42` images, apparaît 0x100 plus haut
/// que sa profondeur, attend 30 images puis tombe (VZ = -0x100, AZ = -0x40) jusqu'au sol, où il
/// déclenche deux sous-routines d'impact (0xab7d8 avec 3, 0x2860 avec 8) et disparaît.
mod drop51 {
    use super::*;

    const ANIMS: u32 = 0x16ea18;
    const LAUNCHER: u32 = 0x5c;
    const WAIT: u32 = 0x42;
    /// Haut de l'écran : `A5+0x50a` (joueur 1) ou `A5+0x55a` (joueur 2, si `A5+0x36`).
    const SCREEN_TOP: u32 = 0x50a;
    const SCREEN_TOP_P2: u32 = 0x55a;
    const IMPACT_FX: u32 = 0xab7d8;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                if wait(x) {
                    return;
                }
                x.add8(0x4, 2);
                x.set16(WAIT, 0x1e);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x17);
                let top = if x.a5_u8(0x36) != 0 { SCREEN_TOP_P2 } else { SCREEN_TOP };
                let top = x.a5_u16(top);
                let depth = x.u16(0x10);
                x.set16(0xc, top.wrapping_add(0x100).wrapping_sub(depth));
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                if x.u8(0x5) == 0 {
                    if wait(x) {
                        return;
                    }
                    x.add8(0x5, 2);
                    x.set16(0x18, neg(0x100));
                    x.set16(0x1a, neg(0x40));
                    return;
                }
                x.anim_step();
                x.move_z();
                if s16(x.u16(0xc)) >= 0 {
                    if x.u8(f::HIT_FLAGS) & 0x8 != 0 {
                        x.set8(f::HIT_FLAGS, 0);
                        let launcher = x.linked(LAUNCHER);
                        let n = launcher.u8(x.b, 0x1d).wrapping_add(1);
                        launcher.set_u8(x.b, 0x1d, n);
                    }
                    return;
                }
                x.add8(0x4, 2);
                x.c.d[0] = 3;
                x.call(IMPACT_FX);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
            }
            4 => {
                x.add8(0x4, 2);
                if x.u8(0x2f) == 0 {
                    let launcher = x.linked(LAUNCHER);
                    let slot = 8u8.wrapping_add(x.u8(0x3)) as u32;
                    launcher.set_u16(x.b, slot, 0);
                }
            }
            _ => x.free_eshl(),
        }
    }

    /// Décompte du délai `+0x42` : vrai tant qu'il n'est pas écoulé.
    fn wait(x: &mut Cx) -> bool {
        let t = x.u16(WAIT).wrapping_sub(1);
        x.set16(WAIT, t);
        t != 0
    }
}

/// Tir 52 (0xa46c6) : éléments de l'attaque finale d'un boss, attachés à lui (`+0x46`), quatre
/// variantes qui n'existent que tant que le boss est dans l'état `+0xa0` attendu :
///  0 (état 0x201) : boule posée sur le boss, qui lance trois sous-éléments (variantes 1 à 3 de
///    `+0x2f`) ; blesse les joueurs touchés ;
///  1 (état 0x214) : rayon collé à côté du boss ;
///  2 (état 0x215) : voile plein écran qui suit l'écran et blesse les joueurs hors des zones sûres
///    de l'étage 0x2e ou 0x69 ;
///  3 : variante de la précédente qui dure 256 images (`+0x42`).
/// Les états de verrouillage de l'écran sont `A5+0x43` (0x3f pris, 0xff libéré) et `A5+0x4ba`.
mod finale52 {
    use super::*;

    const ANIMS: u32 = 0x17e048;
    const PARENT: u32 = 0x46;
    const PARENT_STATE: u32 = 0xa0;
    const SUBTYPE: u32 = 0x2f;
    const TIMER: u32 = 0x42;
    /// Animation par sous-type de la variante 0 (octets).
    const ANIM_OF_SUBTYPE: u32 = 0xa473e;
    /// Sous-types des trois éléments lancés par la variante 0.
    const ORB_SUBTYPES: u32 = 0xa46c2;
    /// Écran : X (`A5+0x506`), Y (long `A5+0x50a`).
    const SCREEN_X: u32 = 0x506;
    const SCREEN_Y: u32 = 0x50a;
    const STAGE: u32 = 0x34;
    const SCREEN_LOCK: u32 = 0x43;
    const LOCK_TAKEN: u8 = 0x3f;
    const LOCK_FREE: u8 = 0xff;
    const FLAG_4BA: u32 = 0x4ba;
    /// Étage 0x2e : paires (début, longueur) des zones sûres en X, terminées par 0.
    const SAFE_RANGES: u32 = 0xa4bca;
    /// Étage 0x69 : rectangles sûrs (X, largeur, Y, hauteur), un par bit de `0xff2064`.
    const SAFE_RECTS: u32 = 0xa4bd4;
    const RECT_FLAGS: u32 = 0xff2064;
    /// Boîtes de l'attaque (lignes de 16 octets par `+0x3e`) et pointeurs vers les boîtes des joueurs.
    const ATTACK_BOXES: u32 = 0xa4cd2;
    const PLAYER_BOXES: u32 = 0xa4cba;
    /// Premier joueur (`A5+0x5de`) et pas.
    const PLAYERS: u32 = 0x5de;
    const PLAYER_SIZE: u32 = 0x200;
    const STAGE_REAL_FX_1: u32 = 0x10158;
    const STAGE_REAL_FX_2: u32 = 0x101da;
    const SCREEN_END_FX: u32 = 0xe55fe;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x3) {
            0 => variant0(x),
            1 => variant1(x),
            2 => variant2(x),
            3 => variant3(x),
            _ => {}
        }
    }

    fn vanish(x: &mut Cx) {
        x.set8(0x4, 6);
        x.set8(0x5, 0);
    }

    /// Boss vivant (adresse négative, octet d'activité non nul).
    fn parent(x: &mut Cx) -> Option<Obj> {
        let w = x.u16(PARENT);
        if s16(w) >= 0 {
            return None;
        }
        let o = obj_of(w);
        (o.u8(x.b, 0) != 0).then_some(o)
    }

    fn init_boxes(x: &mut Cx) {
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_POWER, 0);
    }

    /// Se cale sur le boss ; le sous-type décale en hauteur (0x1c moins le sous-type).
    fn attach(x: &mut Cx, boss: Obj) {
        copy_xyz16(x, boss);
        let sub = x.u8(SUBTYPE) as u16;
        if sub == 0 {
            x.add32(0x10, 1);
        }
        x.add16(0xc, 0x1cu16.wrapping_sub(sub));
        x.add16(0x10, sub);
    }

    /// Fin de l'image : retenu dans l'écran puis remis dans la liste triée.
    fn settle(x: &mut Cx) {
        crate::ai::keep_on_screen(x.b, x.o, 0x10);
        x.reinsert();
    }

    fn unlock_screen(x: &mut Cx) {
        x.set_a5_u8(SCREEN_LOCK, LOCK_FREE);
        x.set_a5_u16(FLAG_4BA, 0);
    }

    // ----- variante 0 -----

    fn variant0(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                attach(x, boss);
                let sub = x.u8(SUBTYPE) as u32;
                let anim = x.rom8(ANIM_OF_SUBTYPE + sub);
                x.anim_start(ANIMS, (anim as u32) as u16);
                init_boxes(x);
            }
            2 => {
                let Some(boss) = parent(x) else { return vanish(x) };
                if boss.u16(x.b, PARENT_STATE) != 0x201 {
                    return vanish(x);
                }
                x.set_bit(0x1, 1, false);
                match x.u8(SUBTYPE) {
                    0 => orb(x, boss),
                    1..=3 => follower(x, boss),
                    _ => {}
                }
                x.set_a5_u16(FLAG_4BA, 0);
                hit_players(x);
                x.reinsert();
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Sous-type 0 : suit l'animation de lancement, puis lance trois éléments et colle à côté du boss.
    fn orb(x: &mut Cx, boss: Obj) {
        let mut source = boss;
        if x.u8(0x5) == 0 {
            if !x.anim_step() {
                return attach(x, boss);
            }
            x.add8(0x5, 2);
            let last = spawn_followers(x);
            x.anim_start(ANIMS, 0x1b);
            // Glitch d'origine : la position est ensuite recopiée depuis A1, qui désigne alors le dernier élément créé
            // (et non le boss), ou, si le pool était plein, l'emplacement que l'allocation laisse dans A1.
            source = last;
        }
        x.anim_step();
        copy_xyz16(x, source);
        copy_flip(x, source);
        let dx = if x.u8(f::FLIP) != 0 { 8 } else { neg(8) };
        x.add16(0x8, dx);
        x.add16(0xc, 0xe);
        x.add16(0x10, 0xffff);
    }

    /// Crée trois éléments de sous-types 3, 2, 1 (table), liés au même boss. Rend l'A1 de l'original : le dernier
    /// élément créé, ou ce que l'allocation qui échoue laisse (`Pool::alloc_or_leftover`).
    fn spawn_followers(x: &mut Cx) -> Obj {
        let mut a1 = Obj(x.c.a[1]);
        for i in (0..3u32).rev() {
            let c = match x.alloc_or_leftover(&pools::ESHL) {
                Ok(c) => c,
                Err(a1) => return a1,
            };
            a1 = c;
            c.set_u8(x.b, 0, 1);
            c.set_u8(x.b, 2, 0x34);
            c.set_u8(x.b, 3, 0);
            let sub = x.rom8(ORB_SUBTYPES + i);
            c.set_u8(x.b, SUBTYPE, sub);
            let boss = x.u16(PARENT);
            c.set_u16(x.b, PARENT, boss);
        }
        a1
    }

    /// Sous-types 1 à 3 : invisibles tant que le boss n'a pas armé `+0x3e` (sauf le sous-type 2).
    fn follower(x: &mut Cx, boss: Obj) {
        attach(x, boss);
        x.set_bit(0x1, 1, true);
        let always = x.u8(SUBTYPE) & 7 == 2;
        if !always && boss.u8(x.b, 0x3e) == 0 {
            return;
        }
        x.set_bit(0x1, 1, false);
        x.anim_step();
    }

    // ----- variante 1 -----

    fn variant1(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                attach(x, boss);
                x.anim_start(ANIMS, 0x1c);
                init_boxes(x);
            }
            2 => {
                x.set_a5_u16(FLAG_4BA, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                x.set_bit(0x1, 1, false);
                if x.u8(0x5) == 0 {
                    if boss.u16(x.b, PARENT_STATE) == 0x214 {
                        x.anim_step();
                        beside(x, boss);
                        hit_players(x);
                        return settle(x);
                    }
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 0x1d);
                }
                if x.anim_step() {
                    return vanish(x);
                }
                beside(x, boss);
                hit_players(x);
                settle(x);
            }
            4 => x.add8(0x4, 2),
            6 => x.free_eshl(),
            _ => {}
        }
    }

    /// Collé au boss, 8 pixels devant lui et 0x28 plus haut ; le boss avance d'un cran en Y.
    fn beside(x: &mut Cx, boss: Obj) {
        let (px, pz) = (boss.u16(x.b, 0x8), boss.u16(x.b, 0xc));
        x.set16(0x8, px);
        x.set16(0xc, pz);
        let py = boss.u32(x.b, 0x10);
        x.set32(0x10, py);
        copy_flip(x, boss);
        let dx = if x.u8(f::FLIP) != 0 { neg(8) } else { 8 };
        x.add16(0x8, dx);
        x.add16(0xc, 0x28);
        let n = boss.u16(x.b, 0x12).wrapping_add(1);
        boss.set_u16(x.b, 0x12, n);
    }

    // ----- variantes 2 et 3 : voile plein écran -----

    /// Cale le voile sur l'écran (X, Y) à la hauteur 0x108 ; le boss avance d'un cran en Y s'il y en a un.
    fn cover_screen(x: &mut Cx, boss: Option<Obj>) {
        let sx = x.a5_u16(SCREEN_X);
        x.set16(0x8, sx);
        x.set16(0xc, 0);
        let sy = x.a5_u32(SCREEN_Y);
        x.set32(0x10, sy);
        x.add16(0xc, 0x108);
        if let Some(boss) = boss {
            let n = boss.u16(x.b, 0x12).wrapping_add(1);
            boss.set_u16(x.b, 0x12, n);
        }
    }

    fn lock_screen(x: &mut Cx) {
        x.set_a5_u8(SCREEN_LOCK, LOCK_TAKEN);
        init_boxes(x);
    }

    fn variant2(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                let Some(boss) = parent(x) else { return vanish(x) };
                attach(x, boss);
                x.anim_start(ANIMS, 0x1e);
                lock_screen(x);
                x.call(STAGE_REAL_FX_1);
            }
            2 => {
                let boss = parent(x).filter(|b| b.u16(x.b, PARENT_STATE) == 0x215);
                let Some(boss) = boss else {
                    x.add8(0x4, 2);
                    x.add8(0x4, 2);
                    return unlock_screen(x);
                };
                x.set_bit(0x1, 1, false);
                if x.u8(0x5) == 0 {
                    if x.anim_step() {
                        x.add8(0x5, 2);
                        x.anim_start(ANIMS, 0x1f);
                        return;
                    }
                    cover_screen(x, Some(boss));
                    let d = boss.u16(x.b, 0x10).wrapping_sub(x.u16(0x10));
                    x.add16(0x10, d);
                    x.add16(0x10, 0xffff);
                    x.add16(0xc, neg(d));
                    x.add16(0xc, 1);
                } else {
                    x.set_a5_u16(FLAG_4BA, 0);
                    x.anim_step();
                    fall_check(x);
                    // piège de l'original : la boucle des joueurs a laissé A1 après le quatrième joueur
                    // (0xff8dde) et 0xa49fc incrémente le mot +0x12 de cet « objet », pas celui du boss
                    cover_screen(x, Some(Obj(0xff_8000 + PLAYERS + 4 * PLAYER_SIZE)));
                }
                settle(x);
            }
            4 => {
                x.add8(0x4, 2);
                unlock_screen(x);
            }
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn variant3(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set8(0x5, 0);
                x.anim_start(ANIMS, 0x23);
                lock_screen(x);
                let fx = if x.a5_u8(STAGE) == 0x69 { STAGE_REAL_FX_2 } else { STAGE_REAL_FX_1 };
                x.call(fx);
            }
            2 => {
                if x.a5_u8(STAGE) == 0x2d && s16(x.a5_u16(SCREEN_X)) >= 0x340 {
                    crate::sound::jouer(x.b, SCREEN_END_FX, x.o);
                    return end3(x);
                }
                if x.u8(0x5) == 0 {
                    if x.anim_step() {
                        x.add8(0x5, 2);
                        x.set16(TIMER, 0x100);
                        x.anim_start(ANIMS, 0x1f);
                        return;
                    }
                } else {
                    x.set_a5_u16(FLAG_4BA, 0);
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) <= 0 {
                        return end3(x);
                    }
                    x.anim_step();
                    fall_check(x);
                }
                cover_screen(x, None);
                settle(x);
            }
            4 => {
                x.add8(0x4, 2);
                unlock_screen(x);
            }
            6 => x.free_eshl(),
            _ => {}
        }
    }

    fn end3(x: &mut Cx) {
        x.add8(0x4, 2);
        x.add8(0x4, 2);
        unlock_screen(x);
    }

    // ----- collisions avec les joueurs -----

    fn players() -> impl Iterator<Item = Obj> {
        (0..4).map(|i| Obj(0xff_8000 + PLAYERS + PLAYER_SIZE * i))
    }

    /// Joueur en jeu normal : actif, vivant (PV non nuls), état 2, non invulnérable.
    fn alive(x: &mut Cx, p: Obj) -> bool {
        p.u8(x.b, 0) != 0 && p.u8(x.b, 0x7e) == 0 && p.u8(x.b, 4) == 2 && p.u8(x.b, 0x10f) == 0 && p.u16(x.b, 0x62) != 0
    }

    /// Le voile blesse les joueurs touchés par l'attaque (boîtes `+0x3e`).
    fn hit_players(x: &mut Cx) {
        for p in players() {
            if alive(x, p) && overlap(x, p) {
                hurt(x, p);
            }
        }
    }

    /// Blesse les joueurs qui ne sont pas dans une zone sûre (étages 0x2e et 0x69).
    fn fall_check(x: &mut Cx) {
        for p in players() {
            if !alive(x, p) {
                continue;
            }
            let stage = x.a5_u8(STAGE);
            let hurts = if stage == 0x2e {
                !on_safe_range(x, p)
            } else if stage == 0x69 {
                let flags = x.data8(RECT_FLAGS);
                !(flags & 2 != 0 && in_rect(x, p, SAFE_RECTS) || flags & 1 != 0 && in_rect(x, p, SAFE_RECTS + 8))
            } else {
                true
            };
            if hurts {
                hurt(x, p);
            }
        }
    }

    /// Étage 0x2e : le joueur est au sol (Z < 0x100) et dans une des zones sûres.
    fn on_safe_range(x: &mut Cx, p: Obj) -> bool {
        if p.u8(x.b, 0xc) != 0 {
            return false;
        }
        let mut a2 = SAFE_RANGES;
        loop {
            let start = x.data16(a2);
            a2 += 2;
            if start == 0 {
                return false;
            }
            let d = p.u16(x.b, 0x8).wrapping_sub(start);
            if s16(d) < 0 {
                return false;
            }
            let len = x.data16(a2);
            a2 += 2;
            if s16(d) <= s16(len) {
                return true;
            }
        }
    }

    /// Étage 0x69 : le joueur est au sol et dans le rectangle (X, largeur, Y, hauteur).
    fn in_rect(x: &mut Cx, p: Obj, rect: u32) -> bool {
        if p.u8(x.b, 0xc) != 0 {
            return false;
        }
        let px = s16(p.u16(x.b, 0x8));
        let x0 = x.data16(rect);
        if s16(x0) >= px {
            return false;
        }
        if s16(x0.wrapping_add(x.data16(rect + 2))) <= px {
            return false;
        }
        let y1 = x.data16(rect + 4).wrapping_add(x.data16(rect + 6));
        s16(y1) > s16(p.u16(x.b, 0x10))
    }

    /// Recouvrement des boîtes de l'attaque (ligne `+0x3e` - 1) et du joueur (pointeur selon son
    /// numéro de personnage, ligne selon `+0x38`), axe par axe : X (miroir selon l'orientation),
    /// Z puis Y. Distance absolue des centres contre la somme des demi-tailles.
    fn overlap(x: &mut Cx, p: Obj) -> bool {
        if p.u8(x.b, 0x38) == 0 {
            return false;
        }
        let n = x.u8(0x3e);
        if n == 0 {
            return false;
        }
        let rows = ATTACK_BOXES + 16 * (n - 1) as u32;
        let character = p.u8(x.b, 3) as u32 & 0xe;
        let table = x.data32(PLAYER_BOXES + 2 * character);
        let boxes = table + 8 * p.u8(x.b, 0x38) as u32;
        for (k, pos) in [0x8u32, 0xc, 0x10].into_iter().enumerate() {
            let k = k as u32;
            let mut mine = x.data16(rows + 4 * k);
            let mut theirs = x.data8(boxes + 2 * k) as i8 as i16 as u16;
            if k == 0 {
                if x.u8(f::FLIP) != 0 {
                    mine = neg(mine);
                }
                if p.u8(x.b, f::FLIP) != 0 {
                    theirs = neg(theirs);
                }
            }
            let a = mine.wrapping_add(x.u16(pos));
            let b = theirs.wrapping_add(p.u16(x.b, pos));
            let mut d = a.wrapping_sub(b);
            if s16(d) < 0 {
                d = neg(d);
            }
            let reach = x.data16(rows + 4 * k + 2).wrapping_add(x.data8(boxes + 2 * k + 1) as u16);
            if s16(reach) < s16(d) {
                return false;
            }
        }
        true
    }

    /// Blessure : le joueur passe à l'état 0x02042000 ; PV -= PV max / 2 (sauf attaques 0x2c), un
    /// joueur à 0 PV est retiré du compte `A5+0x35`.
    fn hurt(x: &mut Cx, p: Obj) {
        p.set_u32(x.b, 0x4, 0x0204_2000);
        let mut hp = 0u16;
        if p.u8(x.b, 0x10d) == 0x2c || p.u8(x.b, 0x10e) == 0x2c {
            hp = p.u16(x.b, 0x62);
            let half = p.u16(x.b, 0x60) >> 1;
            let below = s16(hp) < s16(half);
            hp = hp.wrapping_sub(half);
            if below {
                hp = 0;
            }
        }
        p.set_u16(x.b, 0x62, hp);
        if hp == 0 {
            let n = x.a5_u8(0x35).wrapping_sub(1);
            x.set_a5_u8(0x35, n);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Types 53 à 55
// ---------------------------------------------------------------------------------------------

/// Caché une image sur deux selon la parité de la variante comparée à la phase globale `A5+0x6e`.
fn blink_by_variant(x: &mut Cx) {
    let hidden = (x.u8(0x3) & 1) as u16 != x.a5_u16(0x6e) & 1;
    x.set_bit(0x1, 1, hidden);
}

/// Haut de l'écran (`A5+0x50a`, ou `A5+0x55a` en mode deux joueurs `A5+0x36`).
fn screen_top(x: &mut Cx) -> u16 {
    let off = if x.a5_u8(0x36) != 0 { 0x55a } else { 0x50a };
    x.a5_u16(off)
}

/// Tir 53 (0xa4d82) : projectile visant le joueur ciblé par le créateur de son créateur. Il part de
/// la position de son créateur (`+0x5c`, lui-même lié à un ennemi dont `+0x50` désigne la cible),
/// dans la direction (0 à 8 environ) donnée par la routine 0x1376 ; animation et orientation lues dans
/// une table de 2 octets par direction. Détruit au contact (touché ou paré) ; retiré hors écran.
mod aimed53 {
    use super::*;

    const ANIMS: u32 = 0x175dc4;
    const LINK: u32 = 0x5c;
    /// Pour chaque direction : (animation, orientation).
    const DIRECTIONS: u32 = 0xa4e16;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => spawn(x),
            2 => {
                if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
                    return x.free_eshl();
                }
                x.move_by_table_xy();
                x.anim_step();
                x.cull_offscreen();
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn spawn(x: &mut Cx) {
        x.add8(0x4, 2);
        if x.u16(LINK) == 0 {
            return x.free_eshl();
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x23);
        let creator = x.linked(LINK);
        if creator.u16(x.b, LINK) == 0 {
            return x.free_eshl();
        }
        copy_xyz16(x, creator);
        let enemy = obj_of(creator.u16(x.b, LINK));
        let target = enemy.u16(x.b, f::OWNER);
        if target == 0 {
            return x.free_eshl();
        }
        let dir = crate::aides::direction_to(x.b, x.o, obj_of(target));
        x.set8(0x2d, dir);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 0x24);
        let row = DIRECTIONS + 2 * dir as u32;
        let flip = x.rom8(row + 1);
        x.set8(f::FLIP, flip);
        let anim = x.rom8(row);
        x.anim_start(ANIMS, (anim as u32) as u16);
        x.reinsert();
    }
}

/// Tir 54 (0xa4eaa) : gerbe de projectiles lancés par un objet (`+0x5c`, qui a un créateur), 17
/// exemplaires numérotés par la variante `+3` (créés par 0xa4e84). Sous-type `+0x2f` 0 : part de
/// l'objet avec les décalages et vitesses de la ligne `+3` de la table 0xa4fe6 (18 octets) ; autre
/// sous-type : ligne de 20 octets de la table 0xa51b4 (dont l'animation). Se déplace 60 (ou 30)
/// images, puis 10 images de plus, puis son animation s'achève ; clignote selon sa variante. Le
/// tir disparaît dès que son lanceur n'existe plus.
mod burst54 {
    use super::*;

    const ANIMS: u32 = 0x1706f6;
    const LINK: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    const SOUND: u32 = 0xe4560;
    /// Lignes de 9 mots : DX, DZ, DY, VX, AX, VZ, AZ, VY, AY.
    const ROWS: u32 = 0xa4fe6;
    /// Lignes de 10 mots : les mêmes, puis l'animation.
    const ROWS_ANIM: u32 = 0xa51b4;

    pub fn run(x: &mut Cx) {
        if x.u16(LINK) == 0 {
            return x.free_eshl();
        }
        match x.u8(0x4) {
            0 => {
                crate::sound::jouer(x.b, SOUND, x.o);
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x23);
                let with_anim = x.u8(0x2f) != 0;
                launch(x, with_anim);
            }
            2 => {
                match x.u8(0x5) {
                    0 => {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        if s16(t) < 0 {
                            x.add8(0x5, 2);
                            x.set16(TIMER, 0xa);
                        }
                        x.move_xyz();
                        x.anim_step();
                    }
                    2 => {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        if s16(t) < 0 {
                            x.add8(0x4, 2);
                        }
                        x.anim_step();
                    }
                    _ => {}
                }
                blink_by_variant(x);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    /// Se place sur le lanceur et prend les vitesses de la ligne de la variante.
    fn launch(x: &mut Cx, with_anim: bool) {
        let src = x.linked(LINK);
        for off in [0x8, 0x10, 0xc] {
            let v = src.u32(x.b, off);
            x.set32(off, v);
        }
        copy_flip(x, src);
        x.add16(0x10, 0xffff);
        x.set16(TIMER, if with_anim { 0x1e } else { 0x3c });
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let v = x.u8(0x3) as u32;
        let row = if with_anim { ROWS_ANIM + 20 * v } else { ROWS + 18 * v };
        let word = |x: &mut Cx, i: u32| x.rom16(row + 2 * i);
        let (mut dx, dz, dy, mut vx, mut ax) = (word(x, 0), word(x, 1), word(x, 2), word(x, 3), word(x, 4));
        for (i, off) in [0x18u32, 0x1a, 0x1c, 0x1e].into_iter().enumerate() {
            let w = word(x, 5 + i as u32);
            x.set16(off, w);
        }
        if flip != 0 {
            dx = neg(dx);
            vx = neg(vx);
            ax = neg(ax);
        }
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, dy);
        x.set16(0x14, vx);
        x.set16(0x16, ax);
        let anim = if with_anim { word(x, 9) as u32 } else { 0 };
        x.anim_start(ANIMS, anim as u16);
    }
}

/// Tir 55 (0xa5358) : projectile qui tombe du haut de l'écran, jeté d'un côté (variante 0 : à
/// gauche ; X et profondeur proviennent de `+0x58` / `+0x5a`, posés par le créateur). Il va en
/// diagonale (VX = ±0x800, VZ = -0xc00) jusqu'au sol (Z <= 0), puis reste 48 images en jouant sa
/// seconde animation, en clignotant pendant les 32 dernières. Immobile en pause (`A5+0x3d`).
mod fall55 {
    use super::*;

    const ANIMS: u32 = 0x170c3e;
    const SOUND: u32 = 0xeb0c0;
    const X_START: u32 = 0x58;
    const Y_START: u32 = 0x5a;
    const TIMER: u32 = 0x56;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                spawn(x);
                x.reinsert();
                crate::sound::jouer(x.b, SOUND, x.o);
            }
            2 => {
                if x.a5_u8(GAME_PAUSED) == 0 {
                    match x.u8(0x5) {
                        0 => fall(x),
                        2 => land(x),
                        _ => {}
                    }
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn spawn(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x2d);
        let (px, py) = (x.u16(X_START), x.u16(Y_START));
        x.set16(0x8, px);
        x.set16(0x10, py);
        let top = screen_top(x);
        x.set16(0xc, top.wrapping_add(0x100).wrapping_sub(py));
        x.set16(0x14, neg(0x800));
        x.set16(0x18, neg(0xc00));
        let mut dx = 0x80;
        if x.u8(0x3) == 0 {
            dx = neg(dx);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            let vx = neg(x.u16(0x14));
            x.set16(0x14, vx);
        }
        x.add16(0x8, dx);
        x.anim_start(ANIMS, 0);
    }

    fn fall(x: &mut Cx) {
        if x.move_xz().gt() {
            return;
        }
        x.add8(0x5, 2);
        x.set32(0xc, 0);
        x.set16(TIMER, 0x30);
        x.anim_start(ANIMS, 1);
    }

    fn land(x: &mut Cx) {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if t == 0 {
            x.add8(0x4, 2);
        }
        if s16(x.u16(TIMER)) <= 0x20 {
            x.set_bit(0x1, 1, true);
            let visible = x.u16(TIMER).wrapping_add(x.c.d[7] as u16) & 1 != 0;
            if visible {
                x.set_bit(0x1, 1, false);
            }
        }
        x.anim_step();
    }
}

// ---------------------------------------------------------------------------------------------
// Types 56 à 58
// ---------------------------------------------------------------------------------------------

/// Les quatre joueurs (enregistrements de 0x200 octets à partir de `A5+0x5de`).
fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(0xff_8000 + 0x5de + 0x200 * i))
}

/// Tir 56 (0xa5466) : objets à ramasser (12 variantes, deux par type d'objet : la variante paire
/// utilise la palette 0x14 et l'identifiant d'attaque 0x21, l'impaire 0x13 et 0x22). Posés à l'écran,
/// ils s'animent (clignotent selon `A5+0x6f` et `D7`, sauf pendant `A5+0x33`) ; un joueur au sol à
/// moins de 8 pixels en X et 6 en profondeur les ramasse : l'objet clignote 30 images (`+0x56`) puis
/// joue son animation de ramassage (types 0 à 5 : animation 3 et son 0xeb132 pour les variantes 0
/// et 1), ou, pour les variantes 6 et 7, attend un délai tiré d'une table avant de rejouer
/// l'animation 5 ; les variantes 8 et 9 dépendent des points de vie (`+0x62`) de leur créateur
/// (`+0x5c`). Disparaît quand il est à plus de 0x200 pixels à gauche de l'écran (sauf étage 0x19).
mod pickup56 {
    use super::*;

    const ANIMS: u32 = 0x170c3e;
    /// Animation de création par type d'objet (variante / 2).
    const INIT_ANIMS: [u32; 6] = [2, 4, 5, 5, 4, 7];
    /// Délais (16 mots) tirés au hasard pour les variantes 6 et 7.
    const DELAYS: u32 = 0xa55b2;
    const PICKUP_SOUND: u32 = 0xeb132;
    const COUNTDOWN: u32 = 0x56;
    /// Variable `A5+0x6f` : phase de clignotement ; `A5+0x33` : clignotement suspendu.
    const BLINK_PHASE: u32 = 0x6f;
    const BLINK_OFF: u32 = 0x33;
    const STAGE: u32 = 0x34;
    const STAGE_NO_CULL: u8 = 0x19;
    const LINK: u32 = 0x5c;
    const HIDDEN: u32 = 0x40;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                let v = x.u8(0x3);
                if v < 12 {
                    init(x, v);
                }
                x.reinsert();
            }
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx, v: u8) {
        let k = v as usize / 2;
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x14);
        x.set8(f::ATTACK_ID, 0x21);
        if v != 2 * k as u8 {
            x.set8(0x2b, 0x13);
            x.set8(f::ATTACK_ID, 0x22);
        }
        x.anim_start(ANIMS, (INIT_ANIMS[k]) as u16);
        if k == 3 {
            random_delay(x);
        }
    }

    /// Délai tiré dans la table 0xa55b2.
    fn random_delay(x: &mut Cx) {
        let r = random(x) & 0xf;
        let d = x.rom16(DELAYS + 2 * r);
        x.set16(COUNTDOWN, d);
    }

    fn live(x: &mut Cx) {
        if x.u8(HIDDEN) != 0 {
            return x.reinsert();
        }
        let v = x.u8(0x3);
        match v / 2 {
            0 => touch_then_pick(x, true),
            1 => flash_animate(x),
            2 => touch_then_pick(x, false),
            3 => wait_and_replay(x),
            4 => hp_dependent(x),
            5 => flash_animate(x),
            _ => {}
        }
        if v < 12 && x.a5_u8(STAGE) != STAGE_NO_CULL {
            let off = if x.a5_u8(0x36) != 0 { 0x556 } else { 0x506 };
            let edge = x.a5_u16(off).wrapping_sub(0x200);
            if !(s16(edge) < s16(x.u16(0x8))) {
                x.set8(0x4, 6);
            }
        }
        x.reinsert();
    }

    /// Rend l'objet visible puis le cache une image sur deux (sauf si `A5+0x33`).
    fn flash(x: &mut Cx) {
        x.set_bit(0x1, 1, false);
        if x.a5_u8(BLINK_OFF) == 0 && x.a5_u8(BLINK_PHASE).wrapping_add(x.c.d[7] as u8) & 1 != 0 {
            x.set_bit(0x1, 1, true);
        }
    }

    fn flash_animate(x: &mut Cx) {
        flash(x);
        x.anim_step();
    }

    /// Un joueur au sol, assez proche, ramasse l'objet : `+5` suivant, décompte de 30 images.
    fn pickup_touch(x: &mut Cx) {
        if x.u8(HIDDEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
            return;
        }
        for p in players() {
            if p.u8(x.b, 0) == 0 || p.u16(x.b, 0xc) != 0 {
                continue;
            }
            let dy = x.u16(0x10).wrapping_sub(p.u16(x.b, 0x10)).wrapping_add(6);
            if dy > 0xc {
                continue;
            }
            let dx = x.u16(0x8).wrapping_sub(p.u16(x.b, 0x8)).wrapping_add(8);
            if dx > 0x10 {
                continue;
            }
            x.add8(0x5, 2);
            x.set16(COUNTDOWN, 0x1e);
            return;
        }
    }

    /// Décompte du ramassage : clignote tous les 8 pas ; `true` quand il est écoulé.
    fn pick_countdown(x: &mut Cx) -> bool {
        let t = x.u16(COUNTDOWN).wrapping_sub(1);
        x.set16(COUNTDOWN, t);
        if t == 0 {
            x.add8(0x5, 2);
            x.set8(0x1, 1);
            return true;
        }
        if t & 7 == 0 {
            let flags = x.u8(0x1);
            x.set8(0x1, flags ^ 2);
        }
        false
    }

    /// Variantes 0, 1 (`with_effects`) et 4, 5 : attend le ramassage, clignote, joue l'animation de fin.
    fn touch_then_pick(x: &mut Cx, with_effects: bool) {
        match x.u8(0x5) {
            0 => pickup_touch(x),
            2 => {
                if pick_countdown(x) && with_effects {
                    x.anim_start(ANIMS, 3);
                    crate::sound::jouer(x.b, PICKUP_SOUND, x.o);
                }
            }
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                }
            }
            _ => {}
        }
    }

    /// Variantes 6, 7 : attend un délai (hors pause), puis rejoue l'animation 5 avec un nouveau délai.
    fn wait_and_replay(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            if x.u8(HIDDEN) != 0 || x.a5_u8(GAME_PAUSED) != 0 {
                return;
            }
            let t = x.u16(COUNTDOWN).wrapping_sub(1);
            x.set16(COUNTDOWN, t);
            if t == 0 {
                x.add8(0x5, 2);
            }
        } else if x.anim_step() {
            x.set8(0x5, 0);
            random_delay(x);
            x.anim_start(ANIMS, 5);
        }
    }

    /// Variantes 8, 9 : vivant tant que les PV du créateur sont non nuls ; à sa mort, animation 6,
    /// puis clignote 32 images avant de disparaître.
    fn hp_dependent(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                let maker = x.linked(LINK);
                if maker.u16(x.b, 0x62) != 0 {
                    flash_animate(x);
                } else {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 6);
                }
            }
            2 => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set16(COUNTDOWN, 0x20);
                }
                flash(x);
            }
            4 => {
                x.set_bit(0x1, 1, false);
                let t = x.u16(COUNTDOWN).wrapping_sub(1);
                x.set16(COUNTDOWN, t);
                if t == 0 {
                    x.add8(0x4, 2);
                }
                if x.u16(COUNTDOWN) & 1 != 0 {
                    x.set_bit(0x1, 1, true);
                }
            }
            _ => {}
        }
    }
}

/// Tir 57 (0xa593c) : projectile à tête chercheuse d'un créateur (`+0x5c`) : posé à 0x4a pixels
/// devant lui et 0x32 plus haut, il vise la cible du créateur (`+0x50` du créateur), se dirige vers
/// elle par la table de vitesses (direction recalculée toutes les 20 images) et explose en 16 éclats
/// (type 58) quand il est à moins de 24 pixels de la cible ou quand ses 360 images (`+0x46`) sont
/// écoulées. Si son créateur meurt ou perd sa cible, il clignote et disparaît.
mod seeker57 {
    use super::*;

    const ANIMS: u32 = 0x17100e;
    const LINK: u32 = 0x5c;
    const LIFE: u32 = 0x46;
    const RETARGET: u32 = 0x42;

    pub fn run(x: &mut Cx) {
        if x.u16(LINK) == 0 {
            return x.set32(0x4, 0x0600_0000);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => live(x),
            4 => fade(x),
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 7);
        x.set8(f::ATTACK_POWER, 5);
        let maker = x.linked(LINK);
        copy_xyz16(x, maker);
        let flip = copy_flip(x, maker);
        x.add16(0x10, 0xffff);
        x.set8(RETARGET, 0x14);
        x.set16(LIFE, 0x168);
        x.add16(0x8, if flip != 0 { 0x4a } else { neg(0x4a) });
        x.add16(0xc, 0x32);
        x.anim_start(ANIMS, 0);
        x.select_speed(&crate::objects::motion::SPEEDS_B, 7);
    }

    fn live(x: &mut Cx) {
        let maker = x.linked(LINK);
        if maker.u16(x.b, 0x4) == 0x406 || maker.u16(x.b, f::OWNER) == 0 {
            x.set32(0x4, 0x0400_0000);
            x.set8(RETARGET, 0x1e);
            return;
        }
        if x.u8(0x5) == 0 {
            track(x, maker);
        }
        x.reinsert();
    }

    fn track(x: &mut Cx, maker: Obj) {
        let left = x.u16(LIFE).wrapping_sub(1);
        x.set16(LIFE, left);
        if s16(left) < 0 {
            return explode(x);
        }
        let n = x.u8(RETARGET).wrapping_sub(1);
        x.set8(RETARGET, n);
        let mut aim = maker;
        if s8(n) < 0 {
            x.set8(RETARGET, 0x14);
            let w = x.linked(LINK).u16(x.b, f::OWNER);
            if w == 0 {
                return approach(x);
            }
            aim = obj_of(w);
            let dir = crate::aides::direction_to(x.b, x.o, aim);
            x.set8(0x2d, dir);
        }
        let dx = aim.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(8);
        if dx > 0x20 {
            return approach(x);
        }
        let dy = aim.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(8);
        if dy > 0x20 {
            return approach(x);
        }
        explode(x);
    }

    fn approach(x: &mut Cx) {
        x.move_by_table_xy();
        x.anim_step();
    }

    /// Crée 16 éclats (type 58, variantes 15 à 0) puis passe à l'état 6.
    fn explode(x: &mut Cx) {
        for n in (0..16u8).rev() {
            let Some(c) = x.alloc(&pools::ESHL) else { break };
            c.set_u8(x.b, 0, 1);
            c.set_u8(x.b, 2, 0x3a);
            c.set_u8(x.b, 3, n);
            c.set_u16(x.b, LINK, x.o.0 as u16);
        }
        x.add8(0x4, 4);
    }

    fn fade(x: &mut Cx) {
        let t = x.u16(RETARGET).wrapping_sub(1);
        x.set16(RETARGET, t);
        if s16(t) < 0 {
            return x.set8(0x4, 6);
        }
        let hidden = x.u16(RETARGET) & 3 != 0;
        x.set_bit(0x1, 1, hidden);
    }
}

/// Tir 58 (0xa5ad6) : éclat de l'explosion 57 (16 variantes issues de la table de 8 octets par
/// variante : durée `+0x42`, sous-type `+0x2f`, indice de vitesse `+0x2d`, animation, orientation,
/// table de vitesses). Part de la position de son créateur et se déplace par la table de vitesses
/// (XY ou XZ selon `+0x2f`) pendant sa durée, en clignotant selon sa variante.
mod shard58 {
    use super::*;

    const ANIMS: u32 = 0x1706f6;
    const LINK: u32 = 0x5c;
    const ROWS: u32 = 0xa5bd6;
    const TIMER: u32 = 0x42;

    pub fn run(x: &mut Cx) {
        if x.u16(LINK) == 0 {
            return x.set32(0x4, 0x0600_0000);
        }
        match x.u8(0x4) {
            0 => init(x),
            2 => {
                if x.u8(0x5) == 0 {
                    let t = x.u16(TIMER).wrapping_sub(1);
                    x.set16(TIMER, t);
                    if s16(t) < 0 {
                        x.add8(0x4, 2);
                    }
                    x.anim_step();
                    if x.u8(0x2f) == 0 {
                        x.move_by_table_xy();
                    } else {
                        x.move_by_table_xz();
                    }
                }
                blink_by_variant(x);
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn init(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x23);
        let maker = x.linked(LINK);
        copy_xyz32(x, maker);
        copy_flip(x, maker);
        x.add16(0x10, 0xffff);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let row = ROWS + 8 * x.u8(0x3) as u32;
        let timer = x.rom16(row);
        let subtype = x.rom8(row + 2);
        let speed_index = x.rom8(row + 3);
        let anim = x.rom8(row + 4);
        let new_flip = x.rom8(row + 5);
        let speeds = x.rom16(row + 6);
        x.set16(TIMER, timer);
        x.set8(0x2d, speed_index);
        x.set8(f::FLIP, new_flip);
        x.set8(0x2f, subtype);
        x.anim_start(ANIMS, (anim as u32) as u16);
        x.select_speed(&crate::objects::motion::SPEEDS_B, (speeds as u32) as u16);
    }
}

// ---------------------------------------------------------------------------------------------
// Types 59 à 62
// ---------------------------------------------------------------------------------------------

/// Tir 59 (0xa5cc2) : anneau d'effet d'un créateur (`+0x5c`), deux sous-types `+0x2f`. Sous-type 0 :
/// posé sur le créateur, avec le propriétaire du créateur (`+0x50`), joue l'animation 1, puis 2 (300
/// images), 3, et quand elle s'achève déclenche l'effet 0xe448c ; disparaît quand le propriétaire
/// est dans l'état 0x406. Sous-type 1 : posé près du créateur (décalage par variante), il joue son
/// animation et met l'octet `+0xae` du créateur à 1 à la fin. La palette change toutes les 3 images.
mod ring59 {
    use super::*;

    const ANIMS: u32 = 0x171026;
    const LINK: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    /// Décalages (X, Y) par variante pour le sous-type 1 (4 octets par ligne).
    const OFFSETS: u32 = 0xa5db2;
    /// Valeur de `+0x2b` (palette (h)) par `+0x47`.
    const PALETTES: u32 = 0xa5f30;
    const FX_START_0: u32 = 0xe4422;
    const FX_START_1: u32 = 0xe44f6;
    const FX_END: u32 = 0xe448c;
    const PALETTE_COUNTDOWN: u32 = 0x46;
    const PALETTE_INDEX: u32 = 0x47;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => start(x),
            2 => {
                match x.u8(0x2f) {
                    0 => ring(x),
                    1 => satellite(x),
                    _ => {}
                }
                x.reinsert();
            }
            _ => x.free_eshl(),
        }
    }

    fn start(x: &mut Cx) {
        if x.u16(LINK) == 0 {
            return;
        }
        x.add8(0x4, 2);
        match x.u8(0x2f) {
            0 => {
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x24);
                x.set8(PALETTE_COUNTDOWN, 2);
                crate::sound::jouer(x.b, FX_START_0, x.o);
                let maker = x.linked(LINK);
                copy_xyz32(x, maker);
                copy_flip(x, maker);
                x.add16(0x10, 1);
                x.set16(0xc, 0);
                let owner = maker.u16(x.b, LINK);
                x.set16(f::OWNER, owner);
                x.add16(0x10, neg(4));
                x.anim_start(ANIMS, 1);
            }
            1 => {
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x24);
                x.set8(PALETTE_COUNTDOWN, 2);
                crate::sound::jouer(x.b, FX_START_1, x.o);
                let maker = x.linked(LINK);
                let (mx, my) = (maker.u16(x.b, 0x8), maker.u16(x.b, 0x10));
                let z = maker.u32(x.b, 0xc);
                x.set32(0xc, z);
                copy_flip(x, maker);
                x.set16(0xc, 0);
                let row = OFFSETS + 4 * x.u8(0x3) as u32;
                let (dx, dy) = (x.rom16(row), x.rom16(row + 2));
                x.set16(0x8, dx.wrapping_add(mx));
                x.set16(0x10, dy.wrapping_add(my));
                x.anim_start(ANIMS, 0);
            }
            _ => {}
        }
    }

    /// Sous-type 0 : suit le propriétaire.
    fn ring(x: &mut Cx) {
        let owner = x.linked(f::OWNER);
        if owner.u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        match x.u8(0x5) {
            0 => {
                if x.anim_step() {
                    x.add8(0x5, 2);
                    x.set16(TIMER, 0x12c);
                    x.anim_start(ANIMS, 2);
                }
            }
            2 => {
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if s16(t) >= 0 {
                    x.anim_step();
                } else {
                    x.add8(0x5, 2);
                    x.anim_start(ANIMS, 3);
                }
            }
            4 => {
                if x.anim_step() {
                    x.add8(0x4, 2);
                    crate::sound::jouer(x.b, FX_END, x.o);
                }
            }
            _ => {}
        }
        cycle_palette(x);
    }

    /// Sous-type 1 : animation puis signal au créateur.
    fn satellite(x: &mut Cx) {
        let maker = x.linked(LINK);
        if maker.u16(x.b, 0x4) == 0x406 {
            return x.free_eshl();
        }
        if x.anim_step() {
            x.add8(0x4, 2);
            let maker = x.linked(LINK);
            maker.set_u8(x.b, 0xae, 1);
        }
        cycle_palette(x);
        blink_by_variant(x);
    }

    fn cycle_palette(x: &mut Cx) {
        let n = x.u8(PALETTE_COUNTDOWN).wrapping_sub(1);
        x.set8(PALETTE_COUNTDOWN, n);
        if s8(n) >= 0 {
            return;
        }
        x.set8(PALETTE_COUNTDOWN, 2);
        x.set8(0x2a, 1);
        let i = x.u8(PALETTE_INDEX).wrapping_add(1) & 7;
        x.set8(PALETTE_INDEX, i);
        let p = x.rom8(PALETTES + i as u32);
        x.set8(0x2b, p);
    }
}

/// Tir 60 (0xa5f5c) : plateforme mobile qui monte et descend avec l'écran (hauteur corrigée du
/// défilement vertical `A5+0x50a`/`0x55a`), 4 variantes. Cycle de `+5` : attendre (variante 0 :
/// `+0x42` images, variante 1 : un joueur monte dessus (0x7f19c), variante 3 : idem avec
/// `A5+0x51e` = 4, variante 2 : le créateur est dans l'état 0x204), monter (VZ = -0x100, AZ = -0x20,
/// en réalité descendre : Z diminue) jusqu'à Z <= 0x20 (0x38 si sous-type `+0x2f`), attendre 16
/// images puis remonter (VZ = 0x180) jusqu'à la hauteur de départ (`+0x4e`), et recommencer. La
/// couche de dessin `+0x3b` dépend de la hauteur. Crée un tir joueur de type 0x18 (0x54872).
mod platform60 {
    use super::*;

    const ANIMS: u32 = 0x1712d4;
    const LINK: u32 = 0x5c;
    const TIMER: u32 = 0x42;
    const RELOAD: u32 = 0x4c;
    const HOME_Z: u32 = 0x4e;
    /// Dernier défilement vertical (mot), et son nouveau relevé.
    const LAST_SCROLL: u32 = 0x16;
    const SCROLL: u32 = 0x14;
    const SPAWN_PLAYER_SHOT: u32 = 0x54872;
    const THUD: u32 = 0xeb04e;
    /// Positions de départ de la variante 2 par sous-type (4 octets par ligne).
    const START_POSITIONS: u32 = 0xa6046;
    /// Variable `A5+0x51e` (variante 3).
    const SIGNAL: u32 = 0x51e;
    /// Sous-états (`+5`).
    const WAIT: u8 = 0;
    const SINK: u8 = 2;
    const REST: u8 = 4;
    const RISE: u8 = 6;

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => start(x),
            2 => live(x),
            _ => x.free_eshl(),
        }
    }

    fn start(x: &mut Cx) {
        let top = screen_top(x);
        x.set16(LAST_SCROLL, top);
        match x.u8(0x3) {
            2 => {
                if x.u16(LINK) == 0 {
                    return;
                }
                x.add8(0x4, 2);
                let row = START_POSITIONS + 4 * x.u8(0x2f) as u32;
                let (px, py) = (x.rom16(row), x.rom16(row + 2));
                x.set16(0x8, px);
                x.set16(0x10, py);
                x.set16(0xc, 0x110);
                x.set16(HOME_Z, 0x110);
                x.set16(TIMER, 0x1e);
                x.set16(RELOAD, 0x1e);
            }
            0 | 1 | 3 => {
                x.add8(0x4, 2);
                x.set16(0xc, 0x110);
                x.set16(HOME_Z, 0x110);
                let t = x.u16(TIMER);
                x.set16(RELOAD, t);
            }
            _ => return,
        }
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x17);
        x.anim_start(ANIMS, 0);
        x.call(SPAWN_PLAYER_SHOT);
        x.reinsert();
    }

    fn live(x: &mut Cx) {
        // suit le défilement vertical de l'écran
        let top = screen_top(x);
        x.set16(SCROLL, top);
        let delta = top.wrapping_sub(x.u16(LAST_SCROLL));
        x.add16(0xc, delta);
        x.set16(LAST_SCROLL, top);
        match x.u8(0x3) {
            0 => cycle(x, Trigger::Timer),
            1 => cycle(x, Trigger::Player { signal: false }),
            3 => cycle(x, Trigger::Player { signal: true }),
            2 => {
                if x.u16(LINK) != 0 {
                    x.set8(0x1c, 0);
                    let maker = x.linked(LINK);
                    if maker.u16(x.b, 0x4) == 0x204 {
                        x.set8(0x1c, 1);
                    }
                    cycle(x, Trigger::Maker);
                }
            }
            _ => {}
        }
        let layer = if x.u8(0x5) == 0 {
            0
        } else {
            let z = s16(x.u16(0xc));
            if z >= 0xb0 {
                0x60
            } else if z >= 0x60 {
                0x5f
            } else {
                0xc
            }
        };
        x.set8(0x3b, layer);
    }

    /// Ce qui déclenche la descente.
    #[derive(Clone, Copy, PartialEq)]
    enum Trigger {
        Timer,
        Player { signal: bool },
        Maker,
    }

    fn cycle(x: &mut Cx, trigger: Trigger) {
        match x.u8(0x5) {
            WAIT => {
                let go = match trigger {
                    Trigger::Timer => {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        t == 0
                    }
                    Trigger::Player { signal } => {
                        let on = player_on(x);
                        if on && signal {
                            x.set_a5_u16(SIGNAL, 4);
                        }
                        on
                    }
                    Trigger::Maker => x.u8(0x1c) == 0 && {
                        let t = x.u16(TIMER).wrapping_sub(1);
                        x.set16(TIMER, t);
                        t == 0
                    },
                };
                if go {
                    x.add8(0x5, 2);
                    x.set16(0x18, neg(0x100));
                    x.set16(0x1a, neg(0x20));
                }
            }
            SINK => {
                x.move_z();
                let floor = if trigger == Trigger::Maker || x.u8(0x2f) == 0 { 0x20 } else { 0x38 };
                if s16(x.u16(0xc)) <= floor {
                    crate::sound::jouer(x.b, THUD, x.o);
                    x.add8(0x5, 2);
                    x.set16(TIMER, 0x10);
                }
            }
            REST => {
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if t == 0 {
                    x.add8(0x5, 2);
                    x.set16(0x18, 0x180);
                    x.set16(0x1a, 0);
                }
            }
            RISE => {
                x.move_z();
                if s16(x.u16(HOME_Z)) > s16(x.u16(0xc)) {
                    return;
                }
                let n = x.u8(0x5).wrapping_sub(6);
                x.set8(0x5, n);
                match trigger {
                    Trigger::Timer | Trigger::Maker => {
                        let reload = x.u16(RELOAD);
                        x.set16(TIMER, reload);
                    }
                    Trigger::Player { signal: true } => x.set_a5_u16(SIGNAL, 0),
                    Trigger::Player { signal: false } => {}
                }
            }
            _ => {}
        }
    }

    /// Un joueur (hors personnage 5) est-il sur la plateforme ? (collision 0x7f19c.)
    fn player_on(x: &mut Cx) -> bool {
        for p in players() {
            let character = p.u8(x.b, 3) >> 1;
            if character == 5 || p.u8(x.b, 0) != 1 {
                continue;
            }
            let boite = crate::ai_aids::Boite { decalage: 0, demi_largeur: 0x28, demi_profondeur: 8 };
            if crate::ai_aids::box_around(x.b, x.o, p, boite) != crate::ai_aids::DansBoite::Non {
                return true;
            }
        }
        false
    }
}

/// Tir 61 (0xa63d8) : marqueur collé à son créateur (`+0x5c`, de variante 0 à 2 qui choisit une des
/// trois tables de décalages ; 20 octets par variante du marqueur, 4 par sous-type `+0x2f`), qui
/// joue son animation et, à la fin, met l'octet `+0x1c` du créateur à 1.
mod marker61 {
    use super::*;

    const ANIMS: u32 = 0x171374;
    const LINK: u32 = 0x5c;
    /// Tables de décalages (X, Y) selon la variante du créateur.
    const OFFSET_TABLES: [u32; 3] = [0xa6538, 0xa65de, 0xa6670];

    pub fn run(x: &mut Cx) {
        if suspended(x) {
            return;
        }
        match x.u8(0x4) {
            0 => {
                if x.u16(LINK) == 0 {
                    return;
                }
                let maker = x.linked(LINK);
                let kind = maker.u8(x.b, 0x3) as usize;
                let Some(&table) = OFFSET_TABLES.get(kind) else { return };
                x.add8(0x4, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x27);
                place(x, table);
                x.anim_start(ANIMS, 0);
                x.reinsert();
            }
            2 => {
                if x.u16(LINK) == 0 {
                    return;
                }
                let maker = x.linked(LINK);
                if maker.u8(x.b, 0x3) > 2 || x.u8(0x5) != 0 {
                    return;
                }
                if x.anim_step() {
                    x.add8(0x4, 2);
                    let maker = x.linked(LINK);
                    maker.set_u8(x.b, 0x1c, 1);
                }
            }
            _ => x.free_eshl(),
        }
    }

    /// Se pose sur le créateur avec le décalage de la table (miroir selon l'orientation du créateur).
    fn place(x: &mut Cx, table: u32) {
        if x.u16(LINK) == 0 {
            return;
        }
        let maker = x.linked(LINK);
        copy_xyz16(x, maker);
        let v = x.u8(0x3) as u32;
        let row = table + 20 * v + 4 * x.u8(0x2f) as u32;
        let (mut dx, dy) = (x.rom16(row), x.rom16(row + 2));
        if maker.u8(x.b, f::FLIP) != 0 {
            dx = neg(dx);
        }
        x.add16(0x8, dx);
        x.add16(0x10, dy);
    }
}

/// Tir 62 (0xa66ea) : météore qui tombe sur le terrain près de la caméra. Position tirée au hasard
/// autour de l'écran (X dans 0x1ff + 0x40, Y dans 0x7f) à la hauteur 0x100, retardé de `+0x42` images
/// (table selon la variante), tombe (VZ = -0x200, AZ = -0x20) et, en touchant le sol (Z < 0),
/// déclenche les effets d'impact (0xb9c88, 0xab7d8 avec 3, 0x2860 avec 8, 0xeafdc).
mod meteor62 {
    use super::*;

    const ANIMS: u32 = 0x16ea18;
    /// Retard avant la chute (octets) par variante (32 entrées).
    const DELAYS: u32 = 0xa679e;
    const IMPACT_FX: [u32; 2] = [0xb9c88, 0xab7d8];
    const IMPACT_SOUND: u32 = 0xeafdc;
    const TIMER: u32 = 0x42;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => start(x),
            2 => live(x),
            4 => x.add8(0x4, 2),
            _ => x.free_eshl(),
        }
    }

    fn start(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x5, 0);
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x25);
        // caméra : bloc de A5+0x4fe (joueur 1) ou A5+0x54e (joueur 2)
        let camera = if x.a5_u8(0x36) != 0 { 0x54e } else { 0x4fe };
        x.set16(0xc, 0x100);
        let mut dx = random(x) as u16 & 0x1ff;
        if s16(dx) > 0x180 {
            dx = (dx >> 1) + 0x40;
        }
        let px = dx.wrapping_add(x.a5_u16(camera + 8));
        x.set16(0x8, px);
        let mut dy = random(x) as u16 & 0x7f;
        if s16(dy) > 0x50 {
            dy = (dy >> 1) + 0x10;
        }
        let py = dy.wrapping_add(0x10).wrapping_add(x.a5_u16(camera + 0xc));
        x.set16(0x10, py);
        x.set16(0x18, neg(0x200));
        x.set16(0x1a, neg(0x20));
        x.set32(0x14, 0);
        x.set32(0x1c, 0);
        let variant = x.u8(0x3) & 0x1f;
        let delay = x.rom8(DELAYS + variant as u32);
        x.set8(TIMER, delay);
        x.anim_start(ANIMS, 0);
    }

    fn live(x: &mut Cx) {
        if s8(x.u8(TIMER)) > 0 {
            let t = x.u8(TIMER) - 1;
            return x.set8(TIMER, t);
        }
        x.anim_step();
        x.move_z();
        if s16(x.u16(0xc)) < 0 {
            x.call(IMPACT_FX[0]);
            x.set16(0xc, 0);
            x.add8(0x4, 2);
            x.c.d[0] = 3;
            x.call(IMPACT_FX[1]);
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 8);
            crate::sound::jouer(x.b, IMPACT_SOUND, x.o);
        }
        crate::ai::keep_on_screen(x.b, x.o, 0x10);
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 63
// ---------------------------------------------------------------------------------------------

/// Tir 63 (0xa6864) : main d'un boss (`+0x5c`, dont l'octet `+0xac` donne la phase : < 1 repos,
/// 1 attaque, > 1 fin) qui suit le boss (2 pixels par image vers la position cible : 0x30 devant lui,
/// même profondeur) et joue les animations 3 (repos) et 5 (attaque). Au repos, `+0x42` compte les
/// images avant de tirer au sort (table 0xa6c54 selon le nombre de joueurs `A5+0x35` et l'état
/// `A5+0xff92`) si elle attaque quand la cible est de son côté ; une attaque lancée par le boss
/// (phase 1) crée un tir de type 53 à la fin de son animation. Sous-états : `+5` (0 repos,
/// 4 attaque) et `+6` (étapes de 4).
mod hand63 {
    use super::*;

    const ANIMS: u32 = 0x175da6;
    const LINK: u32 = 0x5c;
    const DELAY: u32 = 0x42;
    /// Table de pointeurs (par nombre de joueurs) vers 32 délais (mots).
    const DELAY_TABLES: u32 = 0xa6b0e;
    /// Table de pointeurs (par nombre de joueurs) vers des masques de 32 bits indexés par `FFFF92`.
    const GATE_TABLES: u32 = 0xa6c54;
    const PLAYER_COUNT: u32 = 0x35;
    const GATE_INDEX: u32 = 0x7f92;
    /// Phase du boss.
    const BOSS_PHASE: u32 = 0xac;
    /// Distance devant le boss et vitesse de rapprochement.
    const REACH: u16 = 0x30;
    const SPEED: u16 = 2;

    pub fn run(x: &mut Cx) {
        match x.u8(0x4) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0xc, 0x30);
                let boss = x.linked(LINK);
                let (bx, by) = (boss.u16(x.b, 0x8), boss.u16(x.b, 0x10));
                x.set16(0x8, bx);
                x.set16(0x10, by);
                resting_or_attacking(x);
            }
            2 => resting_or_attacking(x),
            4 => striking(x),
            _ => x.free_eshl(),
        }
    }

    /// Phase 1 / 2 (`+4 = 2`), puis remise dans la liste triée.
    fn resting_or_attacking(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            match x.u8(0x6) {
                0 => {
                    x.add8(0x6, 4);
                    pick_delay(x);
                    x.anim_start(ANIMS, 3);
                    rest(x);
                }
                4 => rest(x),
                8 => finish_attack(x),
                _ => {}
            }
        } else {
            match x.u8(0x6) {
                0 => {
                    x.add8(0x6, 4);
                    x.anim_start(ANIMS, 5);
                    attack(x);
                }
                4 => attack(x),
                8 => back_to_rest(x),
                _ => {}
            }
        }
        x.reinsert();
    }

    /// Délai avant le prochain tir au sort, selon le nombre de joueurs.
    fn pick_delay(x: &mut Cx) {
        let players = x.a5_u8(PLAYER_COUNT) as u32;
        let table = x.data32(DELAY_TABLES + 4 * players);
        let r = random(x) & 0x1f;
        let d = x.data16(table + 2 * r);
        x.set16(DELAY, d);
    }

    /// Tirage : le bit tiré dans le masque de la configuration courante est-il levé ?
    fn gate(x: &mut Cx) -> bool {
        let players = x.a5_u8(PLAYER_COUNT) as u32;
        let table = x.data32(GATE_TABLES + 4 * players);
        let index = x.a5_u16(GATE_INDEX).wrapping_shl(2) as u32;
        let mask = x.data32(table + index);
        let r = random(x) & 0x1f;
        mask >> r & 1 != 0
    }

    fn lost_boss(x: &mut Cx) {
        x.set16(0x4, 0x400);
    }

    /// Repos : décompte, tirage, puis attaque si la cible est du bon côté.
    fn rest(x: &mut Cx) {
        let boss = x.linked(LINK);
        if x.a5_u8(GAME_PAUSED) == 0 {
            let t = x.u16(DELAY).wrapping_sub(1);
            x.set16(DELAY, t);
            if s16(t) < 0 {
                pick_delay(x);
                if gate(x) && boss.u16(x.b, f::OWNER) != 0 {
                    let target = obj_of(boss.u16(x.b, f::OWNER));
                    let d = s16(boss.u16(x.b, 0x8).wrapping_sub(target.u16(x.b, 0x8)));
                    let facing = x.u8(f::FLIP) != 0;
                    if (d >= 0) != facing {
                        x.set16(0x6, 0x800);
                        x.anim_start(ANIMS, 5);
                        return;
                    }
                }
            }
        }
        if boss.u8(x.b, 0) == 0 {
            return lost_boss(x);
        }
        match s8(boss.u8(x.b, BOSS_PHASE)) {
            1 => {
                // le boss attaque : passe à la phase d'attaque
                x.add8(0x5, 4);
                x.set16(0x6, 0);
            }
            2.. => lost_boss(x),
            _ => {
                follow(x);
                x.anim_step();
            }
        }
    }

    /// Fin de la phase de repos par `+6 = 8` : attend la fin de l'animation puis crée un tir 53.
    fn finish_attack(x: &mut Cx) {
        let boss = x.linked(LINK);
        if boss.u16(x.b, f::OWNER) == 0 {
            return x.set16(0x6, 0);
        }
        if boss.u8(x.b, 0) == 0 {
            return lost_boss(x);
        }
        match s8(boss.u8(x.b, BOSS_PHASE)) {
            1 => {
                x.add8(0x5, 4);
                x.set16(0x6, 0);
                return;
            }
            2.. => return lost_boss(x),
            _ => {}
        }
        if x.anim_step() {
            if let Some(shot) = x.alloc(&pools::ESHL) {
                shot.set_u8(x.b, 0, 1);
                shot.set_u8(x.b, 2, 0x35);
                shot.set_u16(x.b, LINK, x.o.0 as u16);
            }
            x.set16(0x6, 0);
        }
    }

    /// Attaque en cours (le boss est en phase 1) : suit le boss, joue l'animation, puis termine.
    fn attack(x: &mut Cx) {
        follow(x);
        if !x.anim_step() {
            return;
        }
        x.set32(0x34, 0);
        x.add8(0x6, 4);
        back_to_rest(x);
    }

    /// Le boss a-t-il fini son attaque ? Repos si la phase est retombée sous 1.
    fn back_to_rest(x: &mut Cx) {
        let boss = x.linked(LINK);
        if boss.u8(x.b, 0) == 0 {
            return x.set8(0x4, 6);
        }
        match s8(boss.u8(x.b, BOSS_PHASE)) {
            1 => {}
            2.. => x.set8(0x4, 6),
            _ => {
                x.set8(0x5, 0);
                x.set16(0x6, 0);
            }
        }
    }

    /// État 4 : animation 5 jusqu'à la fin, puis disparition.
    fn striking(x: &mut Cx) {
        match x.u8(0x5) {
            0 => {
                x.add8(0x5, 4);
                x.anim_start(ANIMS, 5);
                strike_step(x);
            }
            4 => strike_step(x),
            _ => {}
        }
        x.reinsert();
    }

    fn strike_step(x: &mut Cx) {
        follow(x);
        if x.anim_step() {
            x.set16(0x4, 0x600);
        }
    }

    /// Rapproche la main de sa position cible (devant le boss, côté où il regarde), de 2 pixels par
    /// image en X et en Y. Boss disparu : état 4 (et retour à l'appelant qui continue).
    fn follow(x: &mut Cx) {
        let boss = x.linked(LINK);
        if boss.u8(x.b, 0) == 0 {
            return lost_boss(x);
        }
        let reach = if x.u8(f::FLIP) != 0 { REACH } else { neg(REACH) };
        let goal_x = reach.wrapping_add(boss.u16(x.b, 0x8));
        let goal_y = boss.u16(x.b, 0x10);
        let px = approach(goal_x, x.u16(0x8));
        x.set16(0x8, px);
        let py = approach(goal_y, x.u16(0x10));
        x.set16(0x10, py);
    }

    /// Un pas de 2 vers `goal` sans le dépasser.
    fn approach(goal: u16, pos: u16) -> u16 {
        if s16(goal) == s16(pos) {
            pos
        } else if s16(goal) < s16(pos) {
            let p = pos.wrapping_sub(SPEED);
            if s16(goal) <= s16(p) { p } else { goal }
        } else {
            let p = pos.wrapping_add(SPEED);
            if s16(goal) >= s16(p) { p } else { goal }
        }
    }
}

handlers! {
    0xa31e4 "eshl_40" shot40::run,
    0xa336c "eshl_41" shot41::run,
    0xa3568 "eshl_42" shot42::run,
    0xa3762 "eshl_43" shot43::run,
    0xa380a "eshl_44" beam44::run,
    0xa3b88 "eshl_45" debris45::run,
    0xa3e72 "eshl_46" scroller46::run,
    0xa3ebe "eshl_47" spawn47::run,
    0xa4088 "eshl_48" lob48::run,
    0xa418e "eshl_49" strike49::run,
    0xa4266 "eshl_50" bullet50::run,
    0xa4522 "eshl_51" drop51::run,
    0xa46c6 "eshl_52" finale52::run,
    0xa4d82 "eshl_53" aimed53::run,
    0xa4eaa "eshl_54" burst54::run,
    0xa5358 "eshl_55" fall55::run,
    0xa5466 "eshl_56" pickup56::run,
    0xa593c "eshl_57" seeker57::run,
    0xa5ad6 "eshl_58" shard58::run,
    0xa5cc2 "eshl_59" ring59::run,
    0xa5f5c "eshl_60" platform60::run,
    0xa63d8 "eshl_61" marker61::run,
    0xa66ea "eshl_62" meteor62::run,
    0xa6864 "eshl_63" hand63::run,
}
