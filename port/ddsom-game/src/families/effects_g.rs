//! Famille EFFCT (suite) : types qu'aucun scénario n'exerce, réécrits pour que la table des types
//! `0x39ce0` n'ait plus d'entrée en code d'origine (validation par `examples/fuzz_effct.rs`).
//!
//! - 43 : pluie de débris (`0xb4bcc`) : en haut de l'écran (réglage 0, avec une poussière au départ) ou
//!   depuis le créateur (`+0x5c`), vitesses et animation tirées au hasard ; elle tombe, clignote et
//!   disparaît, ou disparaît avec le créateur ;
//! - 63 : morceau projeté par un ennemi (ENEMY 22 et 23) : part en arc depuis son créateur (`+0x42`),
//!   retombe, fait voler des débris, attend, clignote puis disparaît ;
//! - 81 : éclat lancé par un personnage (`0xb96c0`) : part de la main du joueur (décalage par geste
//!   `+0x42`), glisse en arc puis clignote les dernières images.

use super::cx::{handlers, Cx};
use crate::mem::set_low_word;
use crate::objects::Obj;

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
    pub const FREE_EFFECT: u32 = 0x2218;
    /// Retenue dans l'écran (marge 0x30).
    pub const IN_ZONE: u32 = 0xfbae;
    /// Ombre au sol.
    pub const SHADOW: u32 = 0x33fe4;
    /// Débris projetés (D5 = sorte).
    pub const DEBRIS: u32 = 0x1172e;
    /// Son d'impact au sol.
    pub const SOUND_LANDING: u32 = 0xece9e;
}

fn free(x: &mut Cx) {
    x.call(rom::FREE_EFFECT);
}

/// Bit 1 de `FLAGS` : objet masqué (clignotement).
fn set_hidden(x: &mut Cx, on: bool) {
    x.set_bit(f::FLAGS, 1, on);
}

// ---------------------------------------------------------------------------------------------
// Type 63 : morceau projeté par un ennemi
// ---------------------------------------------------------------------------------------------

mod chunk {
    /// Créateur (ENEMY 22 ou 23).
    pub const CREATOR: u32 = 0x42;
    pub const TIMER: u32 = 0x58;
    /// Jeux d'animations des variantes 0 et 1 (la variante 2 reprend celui de la 1).
    pub const ANIMS_0: u32 = 0x16abaa;
    pub const ANIMS_1: u32 = 0x16b762;
    /// Sorte de débris (`0x1172e`) à l'impact.
    pub const DEBRIS_KIND: u32 = 0xc;
}

/// Départ (variantes 0 et 1) : posé sur le créateur, 0x37 plus haut, lancé vers l'avant.
struct Throw {
    vz: u16,
    az: u16,
    dx: u16,
    vx: u16,
    anims: u32,
}

const THROWS: [Throw; 2] = [
    Throw { vz: 0x4c0, az: 0xffd0, dx: 0xffa0, vx: 0x200, anims: chunk::ANIMS_0 },
    Throw { vz: 0x500, az: 0xffd8, dx: 0xffc8, vx: 0x280, anims: chunk::ANIMS_1 },
];

fn chunk(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => chunk_init(x),
        2 => chunk_active(x),
        _ => free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.call(rom::IN_ZONE);
        x.call(rom::SHADOW);
        x.reinsert();
    }
}

fn chunk_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let variant = x.u8(f::VARIANT);
    let Some(throw) = THROWS.get(variant as usize) else {
        // variante 2 : pas de lancer, seulement l'animation
        return x.anim_start(chunk::ANIMS_1, 0x14);
    };
    let creator = x.linked(chunk::CREATOR);
    let (px, py, pz) = (creator.u32(x.b, Obj::X), creator.u32(x.b, Obj::Y), creator.u32(x.b, Obj::Z));
    x.set32(Obj::X, px);
    x.set32(Obj::Y, py.wrapping_sub(1));
    x.set32(Obj::Z, pz);
    x.add16(Obj::Z, 0x37);
    let facing = creator.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    x.set16(Obj::VZ, throw.vz);
    x.set16(Obj::AZ, throw.az);
    let (mut dx, mut vx) = (throw.dx, throw.vx);
    if facing != 0 {
        dx = dx.wrapping_neg();
        vx = vx.wrapping_neg();
    }
    x.add16(Obj::X, dx);
    x.set16(Obj::VX, vx);
    x.anim_start(throw.anims, 0x14);
}

fn chunk_active(x: &mut Cx) {
    match x.u8(f::SUB_STATE) {
        0 => {
            x.call(super::cx::sub::MOVE_XZ);
            if x.u16(Obj::Z) as i16 > 0 {
                x.anim_step();
                return;
            }
            // au sol : son, débris, puis l'image d'impact
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::Z, 0);
            x.set16(chunk::TIMER, 0x5a);
            x.call(rom::SOUND_LANDING);
            x.c.d[5] = chunk::DEBRIS_KIND;
            x.call(rom::DEBRIS);
            match x.u8(f::VARIANT) {
                0 => x.anim_start(chunk::ANIMS_0, 0x15),
                1 => x.anim_start(chunk::ANIMS_1, 0x15),
                _ => {}
            }
        }
        2 => {
            let t = x.u16(chunk::TIMER).wrapping_sub(1);
            x.set16(chunk::TIMER, t);
            if t as i16 <= 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(chunk::TIMER, 0x3c);
            }
            x.anim_step();
        }
        _ => {
            // clignote jusqu'à la fin du délai
            let v = x.u8(f::FLAGS) ^ 2;
            x.set8(f::FLAGS, v);
            let t = x.u16(chunk::TIMER).wrapping_sub(1);
            x.set16(chunk::TIMER, t);
            if t as i16 > 0 {
                x.anim_step();
            } else {
                free(x);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 81 : éclat lancé par un personnage
// ---------------------------------------------------------------------------------------------

mod shard {
    pub const OWNER: u32 = 0x50;
    pub const TIMER: u32 = 0x58;
    /// Décalages (x, profondeur) de la main, par geste du joueur (`+0x42`) : fiches de 8 octets.
    pub const HAND: u32 = 0x11ae86;
    pub const GESTURE: u32 = 0x42;
    pub const ANIMS: u32 = 0x19844e;
}

fn shard(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => shard_init(x),
        2 => {
            x.call(super::cx::sub::MOVE_XZ);
            let t = x.u8(shard::TIMER).wrapping_sub(1);
            x.set8(shard::TIMER, t);
            if t == 0 {
                x.add8(f::STATE, 2);
            }
            set_hidden(x, t & 1 != 0);
            x.anim_step();
        }
        _ => free(x),
    }
}

fn shard_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(shard::TIMER, 0x20);
    x.set16(Obj::VX, 0xff80);
    x.set16(Obj::VZ, 0x300);
    x.set16(Obj::AZ, 0xffc8);
    let owner = x.linked(shard::OWNER);
    let gesture = (owner.u16(x.b, shard::GESTURE) << 3) as i16 as i32 as u32;
    let mut dx = x.data16(shard::HAND.wrapping_add(gesture).wrapping_add(2));
    let dz = x.data16(shard::HAND.wrapping_add(gesture).wrapping_add(4));
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u16(x.b, off);
        x.set16(off, v);
    }
    x.add16(Obj::Y, 0xffff);
    let facing = owner.u8(x.b, f::FACING) != 0;
    if facing {
        dx = dx.wrapping_neg();
        let vx = x.u16(Obj::VX).wrapping_neg();
        x.set16(Obj::VX, vx);
    }
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    let anim = x.u8(f::VARIANT) as u32 + if facing { 0 } else { 1 };
    x.anim_start(shard::ANIMS, anim);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 43 : pluie de débris
// ---------------------------------------------------------------------------------------------

mod rain {
    /// Objet suivi : l'effet disparaît quand il est libéré ou quand il passe à l'état 4 ou plus.
    pub const LINK: u32 = 0x5c;
    pub const SETTING: u32 = 0x2f;
    pub const TIMER: u32 = 0x42;
    /// Profondeur de référence, reprise une image sur deux.
    pub const SAVED_Y: u32 = 0x1c;
    pub const SPEED_INDEX: u32 = 0x2d;
    pub const ANIMS: u32 = 0x194c74;
    /// Tables (espace programme) : vitesse d'animation par variante, puis tirages (vitesses X et Z,
    /// animation) pour la pluie autour de la caméra et pour les éclats du créateur.
    pub const SPEED_BY_VARIANT: u32 = 0xb4d7a;
    pub const CAMERA_VX: u32 = 0xb4e04;
    pub const CAMERA_VZ: u32 = 0xb4e24;
    pub const CAMERA_ANIMS: u32 = 0xb4de4;
    pub const SHARD_SPEED: u32 = 0xb4e94;
    pub const SHARD_VX: u32 = 0xb4ea4;
    pub const SHARD_VZ: u32 = 0xb4ec4;
    pub const SHARD_ANIMS: u32 = 0xb4ee4;
    /// Poussière (type 25) posée au départ de la pluie : `0xb10c8`, sorte 5.
    pub const DUST: u32 = 0xb10c8;
}

fn rng(x: &mut Cx) -> u16 {
    x.call(super::rom::RNG);
    x.c.d[0] as u16
}

fn debris_rain(x: &mut Cx) {
    if x.u16(rain::LINK) == 0 {
        // plus d'objet suivi : retrait
        return x.set32(f::STATE, 0x0600_0000);
    }
    match x.u8(f::STATE) {
        0 => rain_init(x),
        2 => rain_active(x),
        _ => free(x),
    }
}

fn rain_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    if x.u8(rain::SETTING) == 0 {
        // pluie : en haut à droite de l'écran, une poussière au départ
        let px = x.u16(Obj::X).wrapping_add(0x1a0).wrapping_add(x.a5_u16(0x506));
        let py = x.u16(Obj::Y).wrapping_add(0xa0).wrapping_add(x.a5_u16(0x50a));
        x.set16(Obj::X, px);
        x.set16(Obj::Z, py);
        let y = x.u16(Obj::Y);
        x.set16(rain::SAVED_Y, y);
        x.set16(Obj::AX, 0xffe8);
        x.c.d[0] = 5;
        for (r, v) in [(1, 0x201), (2, 0), (3, 0)] {
            set_low_word(&mut x.c.d[r], v);
        }
        x.call(rain::DUST);
        x.set16(rain::TIMER, 0x26);
        let variant = x.u8(f::VARIANT) as u32;
        let speed = x.rom8(rain::SPEED_BY_VARIANT + variant);
        x.set8(rain::SPEED_INDEX, speed);
        rain_launch(x, rain::CAMERA_VX, rain::CAMERA_VZ, rain::CAMERA_ANIMS, 0x1f);
    } else {
        // éclat qui part du créateur
        x.add8(f::SUB_STATE, 4);
        let from = x.linked(rain::LINK);
        for off in [Obj::X, Obj::Z, Obj::Y] {
            let v = from.u16(x.b, off);
            x.set16(off, v);
        }
        x.set16(Obj::AZ, 0xffd8);
        x.set16(rain::TIMER, 0x14);
        let pick = (rng(x) & 0xf) as u32;
        let speed = x.rom8(rain::SHARD_SPEED + pick);
        x.set8(rain::SPEED_INDEX, speed);
        rain_launch(x, rain::SHARD_VX, rain::SHARD_VZ, rain::SHARD_ANIMS, 0xf);
    }
}

/// Vitesses X et Z et animation tirées dans les tables (mots, mots, octets ; `anim_mask` borne le dernier tirage).
fn rain_launch(x: &mut Cx, vx_table: u32, vz_table: u32, anims: u32, anim_mask: u16) {
    let pick = 2 * (rng(x) & 0xf) as u32;
    let vx = x.rom16(vx_table + pick);
    x.set16(Obj::VX, vx);
    let pick = 2 * (rng(x) & 0xf) as u32;
    let vz = x.rom16(vz_table + pick);
    x.set16(Obj::VZ, vz);
    let pick = (rng(x) & anim_mask) as u32;
    let anim = x.rom8(anims + pick);
    x.anim_start(rain::ANIMS, anim as u32);
}

/// Décompte de `TIMER` ; vrai quand il passe sous zéro.
fn rain_countdown(x: &mut Cx) -> bool {
    let t = x.u16(rain::TIMER).wrapping_sub(1);
    x.set16(rain::TIMER, t);
    (t as i16) < 0
}

/// Fin de vie : retrait une fois le délai écoulé, clignotement trois images sur quatre avant.
fn rain_fade(x: &mut Cx) {
    if rain_countdown(x) {
        return x.set8(f::STATE, 6);
    }
    let t = x.u16(rain::TIMER);
    set_hidden(x, t & 3 != 0);
}

fn rain_active(x: &mut Cx) {
    let link = x.linked(rain::LINK);
    if link.u8(x.b, f::STATE) as i8 >= 4 {
        return free(x);
    }
    match x.u8(f::SUB_STATE) {
        0 => {
            x.call(super::cx::sub::MOVE_XZ);
            if rain_countdown(x) {
                x.add8(f::SUB_STATE, 2);
                x.set16(rain::TIMER, 0x1c);
            }
            // une image sur deux, la pluie reprend sa profondeur de référence (le tirage est perdu)
            rng(x);
            if x.a5_u16(0x6e) & 1 == 0 {
                let y = x.u16(rain::SAVED_Y);
                x.set16(Obj::Y, y);
            }
            x.anim_step();
        }
        2 => rain_fade(x),
        _ => {
            x.call(super::cx::sub::MOVE_XZ);
            rain_fade(x);
        }
    }
    x.reinsert();
}

handlers! {
    0xb4bec "effct_pluie_debris" 200 debris_rain,
    0xb7866 "effct_morceau_ennemi" 200 chunk,
    0xb96dc "effct_eclat_lance" 150 shard,
}
