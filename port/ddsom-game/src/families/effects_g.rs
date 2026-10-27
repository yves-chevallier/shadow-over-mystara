//! Famille EFFCT (suite) : types qu'aucun scénario n'exerce, réécrits pour que la table des types
//! `0x39ce0` n'ait plus d'entrée en code d'origine (validation par `examples/fuzz_effct.rs`).
//!
//! - 43 : pluie de débris (`0xb4bcc`) : en haut de l'écran (réglage 0, avec une poussière au départ) ou
//!   depuis le créateur (`+0x5c`), vitesses et animation tirées au hasard ; elle tombe, clignote et
//!   disparaît, ou disparaît avec le créateur ;
//! - 46 : pièces d'armure d'un ennemi (`0xb53f4`) : elles suivent le porteur et rejouent ses coups reçus,
//!   tombent quand sa vie passe sous leur seuil et se brisent en éclats (variante 1) qui rebondissent ;
//! - 48 : machine volante d'un ennemi (`0xb59a6`, `0xb59c8`) : canon qui tourne autour de son porteur et
//!   tire (1, avec son corps, variante 0), balayeur posé au bord de l'écran qui tire (2) ;
//! - 80 : feu d'artifice de la scène d'un personnage (`0x23606`) : le héros (0) tire des fusées pendant un
//!   fondu au noir, puis s'envole et termine la scène ; spectateurs et acolytes (1 à 6) attendent l'étape
//!   0x12 de la scène pour réagir, et s'en vont avec le fondu ;
//! - 63 : morceau projeté par un ennemi (ENEMY 22 et 23) : part en arc depuis son créateur (`+0x42`),
//!   retombe, fait voler des débris, attend, clignote puis disparaît ;
//! - 81 : éclat lancé par un personnage (`0xb96c0`) : part de la main du joueur (décalage par geste
//!   `+0x42`), glisse en arc puis clignote les dernières images.

use super::cx::{handlers, Cx};
use crate::mem::set_low_word;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

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
    x.free(&pools::EFFCT);
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
        x.anim_start(chunk::ANIMS_1, 0x14);
        return;
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
            x.move_xz();
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
                0 => {
                x.anim_start(chunk::ANIMS_0, 0x15);
            }
                1 => {
                x.anim_start(chunk::ANIMS_1, 0x15);
            }
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
            x.move_xz();
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
    x.anim_start(shard::ANIMS, anim as u16);
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
    x.random()
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
    x.anim_start(rain::ANIMS, (anim as u32) as u16);
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
            x.move_xz();
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
            x.move_xz();
            rain_fade(x);
        }
    }
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 46 : pièces d'armure d'un ennemi (créées par trois par `0xb53f4`)
// ---------------------------------------------------------------------------------------------

mod armour {
    /// Porteur (ENEMY) : la pièce suit sa position et rejoue son coup reçu (`+0x33`).
    pub const WEARER: u32 = 0x46;
    pub const PIECE: u32 = 0x2f;
    pub const LAST_HIT: u32 = 0x42;
    /// Vie du porteur (`+0x62`) et vie de départ (`+0x60`) : la pièce tombe quand la vie passe sous
    /// une fraction de la vie de départ (quart × (seuil + 1), seuil par pièce, table `THRESHOLDS`).
    pub const LIFE: u32 = 0x62;
    pub const FULL_LIFE: u32 = 0x60;
    pub const HIT: u32 = 0x33;
    /// Seuils (octets, espace programme) et jeux d'animations (longs) par pièce.
    pub const THRESHOLDS: u32 = 0xb550e;
    pub const ANIM_SETS: u32 = 0xb54dc;
    pub const WORN_ANIMS: u32 = 0x195384;
    /// Éclats de la pièce tombée (variante 1) : par pièce, nombre moins un puis sortes (octets, données).
    pub const SHARDS: u32 = 0xb5720;
    /// Éclat (variante 1) : par sorte, fiche de 16 octets (dx, dy, dz, vx, vy, vz et az) ; rebonds.
    pub const SHARD_LAUNCH: u32 = 0xb55dc;
    pub const SHARD_ANIMS: u32 = 0x17efe8;
    pub const BOUNCE_TIMER: u32 = 0x5c;
    pub const FALLING: u32 = 0x05;
    pub const ROLLING: u32 = 0x06;
}

/// Fin commune : retenue dans l'écran, ombre, remise en liste.
fn on_screen(x: &mut Cx) {
    x.call(rom::IN_ZONE);
    x.call(rom::SHADOW);
    x.reinsert();
}

fn armour(x: &mut Cx) {
    if x.suspended() {
        return;
    }
    if x.u8(f::VARIANT) != 0 {
        return armour_shard(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(armour::LAST_HIT, 0xff);
            x.anim_start(armour::WORN_ANIMS, 4);
        }
        2 => armour_worn(x),
        4 => armour_break(x),
        _ => free(x),
    }
}

/// La vie du porteur est-elle passée sous le seuil de la pièce ?
fn armour_falls(x: &mut Cx, wearer: Obj) -> bool {
    let quarter = wearer.u16(x.b, armour::FULL_LIFE) >> 2;
    let piece = x.u8(armour::PIECE) as u32;
    let steps = x.rom8(armour::THRESHOLDS + piece) as u16 + 1;
    let threshold = quarter.wrapping_mul(steps);
    threshold as i16 > wearer.u16(x.b, armour::LIFE) as i16
}

fn armour_worn(x: &mut Cx) {
    let link = x.u16(armour::WEARER);
    let wearer = Obj(link as i16 as i32 as u32 & 0xff_ffff);
    if (link as i16) >= 0 || wearer.u8(x.b, Obj::ACTIVE) == 0 || armour_falls(x, wearer) {
        return x.add8(f::STATE, 2);
    }
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = wearer.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = wearer.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    let hit = wearer.u8(x.b, armour::HIT) & 0x7f;
    if hit == 0 {
        set_hidden(x, true);
        return on_screen(x);
    }
    if hit == x.u8(armour::LAST_HIT) {
        return on_screen(x);
    }
    // nouveau coup : la pièce joue l'animation du coup
    x.set8(armour::LAST_HIT, hit);
    set_hidden(x, false);
    let piece = x.u8(armour::PIECE) as u32;
    let set = crate::mem::prog32(x.b, armour::ANIM_SETS + 4 * piece);
    x.anim_start(set, (hit as u32) as u16);
}

/// La pièce tombe : elle se brise en éclats (variante 1) posés sur elle.
fn armour_break(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let mut at = armour::SHARDS + 4 * x.u8(armour::PIECE) as u32;
    let count = x.data8(at);
    at += 1;
    x.c.a[4] = at;
    x.c.d[1] = count as u32;
    loop {
        let Some(o) = x.alloc(&pools::EFFCT) else { return };
        o.set_u8(x.b, Obj::ACTIVE, 1);
        o.set_u8(x.b, Obj::KIND, 46);
        o.set_u8(x.b, Obj::VARIANT, 1);
        let kind = x.data8(at);
        at += 1;
        x.c.a[4] = at;
        o.set_u8(x.b, armour::PIECE, kind);
        for off in [Obj::X, Obj::Y, Obj::Z] {
            let v = x.u16(off);
            o.set_u16(x.b, off, v);
        }
        let facing = x.u8(f::FACING);
        o.set_u8(x.b, f::FACING, facing);
        o.set_u16(x.b, armour::WEARER, x.o.0 as u16);
        let n = (x.c.d[1] as u16).wrapping_sub(1);
        set_low_word(&mut x.c.d[1], n);
        if n == 0xffff {
            return;
        }
    }
}

/// Allocation d'un effet : l'emplacement, ou `None` si le pool est plein.
fn armour_shard(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => armour_shard_init(x),
        2 => armour_shard_active(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

fn armour_shard_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(f::SUB_STATE, 0);
    let fiche = armour::SHARD_LAUNCH + ((x.u8(armour::PIECE) as u16) << 4) as u32;
    let mut dx = x.data16(fiche);
    let dy = x.data16(fiche + 2);
    x.add16(Obj::Y, dy);
    let dz = x.data16(fiche + 4);
    x.add16(Obj::Z, dz);
    let mut vx = x.data16(fiche + 6);
    x.set16(Obj::AX, 0);
    let vy = x.data16(fiche + 8);
    x.set16(Obj::VY, vy);
    x.set16(Obj::AY, 0);
    let vz = x.data32(fiche + 10);
    x.set32(Obj::VZ, vz);
    if x.u8(f::FACING) == 0 {
        dx = dx.wrapping_neg();
        vx = vx.wrapping_neg();
    }
    x.add16(Obj::X, dx);
    x.set16(Obj::VX, vx);
    x.anim_start(armour::SHARD_ANIMS, 9);
}

/// Nouveau rebond : vitesse et gravité verticales, délai tiré au hasard avant de s'arrêter.
fn armour_shard_bounce(x: &mut Cx) {
    x.set8(armour::ROLLING, 0);
    x.set16(Obj::VZ, 0x380);
    x.set16(Obj::AZ, 0xff80);
    let r = rng(x) & 0x1f;
    x.set8(armour::BOUNCE_TIMER, r as u8);
}

fn armour_shard_active(x: &mut Cx) {
    if x.a5_u8(0x3d) != 0 {
        return on_screen(x);
    }
    if x.u8(armour::FALLING) == 0 {
        x.move_xyz();
        if (x.u16(Obj::Z) as i16) < 0 {
            // premier contact avec le sol
            x.set16(Obj::Z, 0);
            x.add8(armour::FALLING, 2);
            x.set8(armour::ROLLING, 0);
            armour_shard_bounce(x);
            return on_screen(x);
        }
    }
    if x.u8(armour::ROLLING) == 0 {
        x.anim_step();
        x.move_z();
        if (x.u16(Obj::Z) as i16) >= 0 {
            x.anim_step();
            return on_screen(x);
        }
        x.set16(Obj::Z, 0);
        x.add8(armour::ROLLING, 2);
    }
    let t = x.u8(armour::BOUNCE_TIMER).wrapping_sub(1);
    x.set8(armour::BOUNCE_TIMER, t);
    if (t as i8) < 0 {
        // rebond suivant (sans retenue ni remise en liste à cette image)
        return armour_shard_bounce(x);
    }
    x.anim_step();
    on_screen(x);
}

// ---------------------------------------------------------------------------------------------
// Type 48 : machine volante d'un ennemi et ses canons (créés par `0xb59a6`, `0xb59c8`)
// ---------------------------------------------------------------------------------------------

mod flyer {
    /// Objet suivi (`+0x46`) ; la variante 0 suit l'objet suivi par son objet suivi.
    pub const LINK: u32 = 0x46;
    /// Phase du suivi (`+0xa4` du porteur) : 1 pour la variante 1, 3 pour la variante 2.
    pub const PHASE: u32 = 0xa4;
    pub const ARMED: u32 = 0x42;
    pub const COUNTDOWN: u32 = 0x43;
    /// Variante 1 : trajet circulaire, table de (dx, dy) en `+0x4c`, angle en `+0x5c`, position relative
    /// sauvée en `+0x18` / `+0x1a`.
    pub const PATH: u32 = 0x4c;
    pub const PATH_TABLE: u32 = 0x1ca4b6;
    pub const ANGLE: u32 = 0x5c;
    pub const REL_X: u32 = 0x18;
    pub const REL_Y: u32 = 0x1a;
    /// Variante 2 : un joueur sur la même profondeur (`+0x2f` = 1).
    pub const PLAYER_IN_LINE: u32 = 0x2f;
    pub const ANIMS: u32 = 0x17f002;
    /// Tir ennemi (ESHL 0x2c) lâché une image sur quatre.
    pub const SHOT: u8 = 0x2c;
    /// Retenue dans l'écran avec une marge D0.
    pub const KEEP_ON_SCREEN: u32 = 0xfbb0;
}

/// L'objet suivi, s'il existe et est actif (`0xb5bf0`).
fn flyer_link(x: &mut Cx) -> Option<Obj> {
    let p = x.u16(flyer::LINK);
    let o = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    ((p as i16) < 0 && o.u8(x.b, Obj::ACTIVE) != 0).then_some(o)
}

fn flyer(x: &mut Cx) {
    if x.suspended() {
        return;
    }
    let state = x.u8(f::STATE);
    match state {
        4 => return x.add8(f::STATE, 2),
        6 => return free(x),
        _ => {}
    }
    match (x.u8(f::VARIANT), state) {
        (0, 0) => {
            x.add8(f::STATE, 2);
            x.anim_start(flyer::ANIMS, 0xe);
        }
        (0, _) => flyer_body(x),
        (1, 0) => flyer_gun_init(x),
        (1, _) => flyer_gun(x),
        (_, 0) => {
            x.add8(f::STATE, 2);
            let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
            let facing = link.u8(x.b, f::FACING);
            x.set8(f::FACING, facing);
            x.set8(flyer::ARMED, 0);
            x.anim_start(flyer::ANIMS, 8);
        }
        _ => flyer_sweeper(x),
    }
}

/// Variante 0 : corps posé sur l'objet suivi par l'objet suivi, clignotant une image sur deux.
fn flyer_body(x: &mut Cx) {
    if x.a5_u8(0x3d) != 0 {
        return x.reinsert();
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    let p = link.u16(x.b, flyer::LINK);
    let host = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    if (p as i16) >= 0 || host.u8(x.b, Obj::ACTIVE) == 0 {
        return x.add8(f::STATE, 2);
    }
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = host.u16(x.b, off);
        x.set16(off, v);
    }
    let facing = host.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    x.add16(Obj::X, if facing != 0 { 0xfffa } else { 6 });
    x.add16(Obj::Y, 0xffff);
    x.add16(Obj::Z, 0x54);
    if x.a5_u16(0x6e) & 1 != 0 {
        x.set8(f::SUB_STATE, 0);
        set_hidden(x, true);
    } else {
        x.anim_step();
        set_hidden(x, false);
    }
    x.reinsert();
}

/// Tir ennemi (ESHL 0x2c) de variante `variant`, une image sur quatre (`0xb5d56`, `0xb5d82`).
fn flyer_shoot(x: &mut Cx, variant: u8) {
    if x.a5_u16(0x6e) & 3 != 0 {
        return;
    }
    let Some(o) = x.alloc(&pools::ESHL) else { return };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, flyer::SHOT);
    o.set_u8(x.b, Obj::VARIANT, variant);
    o.set_u8(x.b, flyer::PLAYER_IN_LINE, 0);
    o.set_u16(x.b, flyer::LINK, x.o.0 as u16);
}

fn flyer_gun_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    let facing = link.u8(x.b, f::FACING);
    x.set8(f::FACING, facing);
    // son corps (variante 0)
    if let Some(o) = x.alloc(&pools::EFFCT) {
        o.set_u8(x.b, Obj::ACTIVE, 1);
        o.set_u8(x.b, Obj::KIND, 48);
        o.set_u8(x.b, Obj::VARIANT, 0);
        o.set_u8(x.b, flyer::PLAYER_IN_LINE, 0);
        o.set_u16(x.b, flyer::LINK, x.o.0 as u16);
    }
    x.set8(flyer::ARMED, 0);
    x.set32(flyer::PATH, flyer::PATH_TABLE);
    let rel = if x.u8(f::FACING) != 0 { 0xff40 } else { 0xc0 };
    x.set16(flyer::REL_X, rel);
    x.set16(flyer::ANGLE, 0x1000);
    x.set16(flyer::REL_Y, 0);
}

/// Fin de la phase : désarmé, l'effet s'en va ; armé, il attend `COUNTDOWN` images.
fn flyer_wind_down(x: &mut Cx, keep_while_positive: bool) -> bool {
    if x.u8(flyer::ARMED) as i8 <= 0 {
        x.add8(f::STATE, 2);
        return false;
    }
    let t = x.u8(flyer::COUNTDOWN).wrapping_sub(1);
    x.set8(flyer::COUNTDOWN, t);
    if t as i8 > 0 {
        return keep_while_positive;
    }
    x.add8(f::STATE, 2);
    false
}

/// Variante 1 : canon qui tourne autour de l'objet suivi tant qu'il est dans la phase 1, et tire.
fn flyer_gun(x: &mut Cx) {
    if x.a5_u8(0x3d) != 0 {
        return;
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    if x.u8(f::SUB_STATE) != 0 {
        flyer_wind_down(x, true);
        return;
    }
    if link.u8(x.b, flyer::PHASE) != 1 {
        x.set8(flyer::COUNTDOWN, 0x20);
        return x.add8(f::SUB_STATE, 2);
    }
    // 0xb5c48 : un pas sur le cercle (table de 32 positions, angle en 1/256 dans l'octet haut)
    let angle = ((x.u8(flyer::ANGLE) as u16 & 0x1f) << 2) as u32;
    let path = x.u32(flyer::PATH);
    for (pos, rel, step) in [(Obj::X, flyer::REL_X, 0u32), (Obj::Y, flyer::REL_Y, 2)] {
        let r = x.u16(rel);
        x.set16(pos, r);
        let d = x.data16(path.wrapping_add(angle + step)) as i16 as i32 as u32;
        x.add32(pos, d << 8);
        let v = x.u16(pos);
        x.set16(rel, v);
        let base = link.u16(x.b, pos);
        x.add16(pos, base);
    }
    let turn = if x.u8(f::FACING) != 0 { 0xff80 } else { 0x80 };
    x.add16(flyer::ANGLE, turn);
    flyer_shoot(x, 0);
    set_low_word(&mut x.c.d[0], 0x10);
    x.call(flyer::KEEP_ON_SCREEN);
}

/// Variante 2 : balayeur posé au bord de l'écran, sur la profondeur de l'objet suivi (phase 3), qui tire.
fn flyer_sweeper(x: &mut Cx) {
    if x.a5_u8(0x3d) != 0 {
        return x.add8(f::STATE, 2);
    }
    let Some(link) = flyer_link(x) else { return x.add8(f::STATE, 2) };
    if x.u8(f::SUB_STATE) != 0 {
        if flyer_wind_down(x, true) {
            flyer_sweep(x, link);
        }
        return;
    }
    if link.u8(x.b, flyer::PHASE) != 3 {
        x.set8(flyer::COUNTDOWN, 0x20);
        return x.add8(f::SUB_STATE, 2);
    }
    flyer_sweep(x, link);
    flyer_shoot(x, 1);
}

/// `0xb5cce` : au bord de l'écran (côté selon l'orientation), à la profondeur de l'objet suivi ; note si
/// un joueur présent est sur la même ligne (à 8 pixels près).
fn flyer_sweep(x: &mut Cx, link: Obj) {
    x.set8(flyer::PLAYER_IN_LINE, 0);
    let y = link.u16(x.b, Obj::Y);
    x.set16(Obj::Y, y);
    let camera = if x.a5_u8(0x36) != 0 { 0x556 } else { 0x506 };
    let mut px = x.a5_u16(camera);
    if x.u8(f::FACING) != 0 {
        px = px.wrapping_add(0x1c0);
    }
    x.set16(Obj::X, px);
    x.set16(Obj::Z, 0x20);
    for k in 0..4 {
        let p = Obj(crate::mem::A5 + 0x5de + 0x200 * k);
        if p.u8(x.b, Obj::ACTIVE) == 0 {
            continue;
        }
        let d = p.u16(x.b, Obj::Y).wrapping_sub(y).wrapping_add(8);
        if d < 0x10 {
            x.set8(flyer::PLAYER_IN_LINE, 1);
        }
    }
    x.c.d[6] = x.c.d[6] & 0xffff_0000 | 0x7fff;
}

// ---------------------------------------------------------------------------------------------
// Type 80 : feu d'artifice de la scène d'un personnage (créé par `0x23606`, variantes 2 et 3)
// ---------------------------------------------------------------------------------------------

mod fireworks {
    /// Étape de la scène que les acteurs attendent pour partir (`A5+0x2e`), et retour au jeu (0xc).
    pub const SCENE_STEP: u32 = 0x2e;
    pub const GO: u8 = 0x12;
    pub const BACK: u8 = 0xc;
    /// Fondu en cours (`A5+0x48f`), ses réglages (`A5+0x48e`, `A5+0x490`) et ses palettes (`A5+0x492...`).
    pub const FADING: u32 = 0x48f;
    pub const FADE_SPEED: u32 = 0x48e;
    pub const FADE_MODE: u32 = 0x490;
    pub const FADE_SOURCES: u32 = 0x492;
    /// Tâches de fondu (programmes de `0x9b6`) : vers le noir, puis retour.
    pub const FADE_OUT: u32 = 0x1ccf8;
    pub const FADE_IN: u32 = 0x1ced4;
    /// Caméra du feu d'artifice (`A5+0x5a6`, `A5+0x5aa`).
    pub const CAMERA_X: u32 = 0x5a6;
    pub const CAMERA_Y: u32 = 0x5aa;
    pub const TIMER: u32 = 0x58;
    pub const DELAY: u32 = 0x5a;
    pub const HOST: u32 = 0x56;
    pub const DECOR: u32 = 0x42;
    /// Fusées (type 50) : variante, délai et décalage (x, y) de chaque tir (espace programme).
    pub const ROCKET_VARIANTS: u32 = 0xb9628;
    pub const ROCKET_DELAYS: u32 = 0xb9638;
    pub const ROCKET_PLACES: u32 = 0xb9658;
    pub const ANIMS_HERO: u32 = 0x1836ec;
    pub const ANIMS_FIRE: u32 = 0x1824d4;
    pub const ANIMS_LAST: u32 = 0x18373a;
    /// Adresse de retour du saut par la table des états (`jsr (table,PC)`), où l'objet est remis en liste.
    pub const AFTER_STATE: u32 = 0xb8f04;
    /// Reprise après l'attente de quatre images (`0xa40`) qui suit le dernier tir.
    pub const AFTER_WAIT: u32 = 0xb91d0;
    pub const WAIT: u32 = 0xa40;
    pub const NEW_TASK: u32 = 0x9b6;
    /// Décor de scène 0x55 (`0xc8312`).
    pub const SCENE_SET: u32 = 0xc8312;
}

/// Sons de la scène.
mod sfx {
    pub const LAST_ROCKET: u32 = 0xe49ae;
    pub const LIFT_OFF: u32 = 0xe580a;
    pub const JUMP: u32 = 0xe3da6;
    pub const CHEER: u32 = 0xe4e5a;
    pub const LANDING: u32 = 0xec7ae;
    /// Efface le début du scroll 1.
    pub const CLEAR_SCROLL1: u32 = 0x3525c;
}

fn fireworks(x: &mut Cx) {
    match x.u8(f::STATE) {
        0 => fireworks_init(x),
        2 => fireworks_active(x),
        _ => free(x),
    }
    fireworks_tail(x);
}

/// `sub.w #1,(off,A0)` : rend la valeur exacte (sans repli sur 16 bits) que testent les `bgt`/`ble` qui suivent.
fn countdown(x: &mut Cx, off: u32) -> i32 {
    let v = x.u16(off);
    x.set16(off, v.wrapping_sub(1));
    v as i16 as i32 - 1
}

/// Fin commune (`0xb8f04`) : l'objet encore actif est remis en liste.
fn fireworks_tail(x: &mut Cx) {
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

fn scene_go(x: &mut Cx) -> bool {
    x.a5_u8(fireworks::SCENE_STEP) == fireworks::GO
}

fn fading(x: &mut Cx) -> bool {
    x.a5_u8(fireworks::FADING) != 0
}

/// Pose X et Y (et Z) en coordonnées d'écran.
fn place(x: &mut Cx, px: u16, z: Option<u16>, py: u16) {
    x.set16(Obj::X, px);
    if let Some(z) = z {
        x.set16(Obj::Z, z);
    }
    x.set16(Obj::Y, py);
}

/// Un acteur attaché à l'objet `+0x56` : X - `dx`, Z + 0x60, juste devant lui.
fn follow_host(x: &mut Cx, dx: u16) {
    let host = x.linked(fireworks::HOST);
    let (hx, hz, hy) = (host.u16(x.b, Obj::X), host.u16(x.b, Obj::Z), host.u32(x.b, Obj::Y));
    x.set16(Obj::X, hx);
    x.set16(Obj::Z, hz);
    x.set32(Obj::Y, hy);
    x.add16(Obj::X, dx.wrapping_neg());
    x.add16(Obj::Z, 0x60);
    x.add16(Obj::Y + 2, 0xffff);
}

fn fireworks_init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    match x.u8(f::VARIANT) {
        0 => {
            x.set8(f::FACING, 0x20);
            place(x, 0x50, None, 0x160);
            x.anim_start(fireworks::ANIMS_HERO, 0);
        }
        1 => {
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x18);
            place(x, 0x110, None, 0x161);
            x.anim_start(0x17b14e, 0);
        }
        2 => {
            place(x, 0x178, None, 0x154);
            x.anim_start(0x177874, 0);
        }
        3 => {
            place(x, 0x158, Some(0x18), 0x168);
            x.set16(Obj::Y + 2, 0xffff);
            // ses deux acolytes (variantes 4 et 5)
            for (variant, dx) in [(4u8, 0x20u16), (5, 0x10)] {
                let Some(o) = x.alloc(&pools::EFFCT) else { break };
                o.set_u8(x.b, Obj::ACTIVE, 1);
                o.set_u8(x.b, Obj::KIND, 80);
                o.set_u8(x.b, Obj::VARIANT, variant);
                let (px, pz, py) = (x.u16(Obj::X), x.u16(Obj::Z), x.u32(Obj::Y));
                o.set_u16(x.b, Obj::X, px.wrapping_sub(dx));
                o.set_u16(x.b, Obj::Z, pz.wrapping_add(0x60));
                o.set_u32(x.b, Obj::Y, py);
                let frac = o.u16(x.b, Obj::Y + 2).wrapping_sub(1);
                o.set_u16(x.b, Obj::Y + 2, frac);
                o.set_u16(x.b, fireworks::HOST, x.o.0 as u16);
            }
            x.anim_start(fireworks::ANIMS_FIRE, 0);
        }
        4 => {
                x.anim_start(fireworks::ANIMS_FIRE, 2);
            }
        5 => {
                x.anim_start(fireworks::ANIMS_FIRE, 3);
            }
        _ => {
            load_colours(x, 0x253460, 0x91c2e0, 0x20);
            x.set8(0x2a, 1);
            x.set8(0x2b, 0x17);
            place(x, 0x120, Some(0xe0), 0x160);
            x.anim_start(fireworks::ANIMS_LAST, 3);
        }
    }
}

/// Copie `n` couleurs de la ROM (données) dans la palette, opaques (`| 0xf000`) ; D0 et D1 comme l'original.
fn load_colours(x: &mut Cx, from: u32, to: u32, n: u32) {
    let mut v = 0;
    for k in 0..n {
        v = x.data16(from + 2 * k) | 0xf000;
        crate::mem::wr16(x.b, to + 2 * k, v);
    }
    set_low_word(&mut x.c.d[0], v);
    set_low_word(&mut x.c.d[1], 0xffff);
}

/// `0xb95b8` : copie 0x400 octets de `from` vers `to` par blocs de huit longs (registres D1-D6, A3, A4
/// laissés comme l'original).
fn copy_block(x: &mut Cx, from: u32, to: u32) {
    let mut last = [0u32; 8];
    for k in 0..0x100 {
        let v = crate::mem::rd32(x.b, from + 4 * k);
        crate::mem::wr32(x.b, to + 4 * k, v);
        last[(k % 8) as usize] = v;
    }
    for (r, v) in [1usize, 2, 3, 4, 5, 6].into_iter().zip(&last[..6]) {
        x.c.d[r] = *v;
    }
    x.c.a[3] = last[6];
    x.c.a[4] = last[7];
    x.c.a[1] = from + 0x400;
    x.c.a[2] = to + 0x400;
    set_low_word(&mut x.c.d[0], 0xffff);
}

/// Lance la tâche de fondu `program` (`0x9b6`, D0.w = 0x10), A0 gardé.
fn fade_task(x: &mut Cx, program: u32) {
    x.c.a[0] = program;
    set_low_word(&mut x.c.d[0], 0x10);
    crate::call(x.c, x.b, fireworks::NEW_TASK);
    x.c.a[0] = x.o.0;
}

fn fireworks_active(x: &mut Cx) {
    let sub = x.u8(f::SUB_STATE);
    match x.u8(f::VARIANT) {
        0 => hero(x, sub),
        1 => match sub {
            0 if scene_go(x) => {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(0x17b162, 1);
            }
            2 if !fading(x) => {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(0x17b166, 7);
            }
            0 | 2 => {
                x.anim_step();
            }
            _ => {}
        },
        2 => match sub {
            0 if scene_go(x) => {
                x.add8(f::SUB_STATE, 2);
                x.set16(Obj::VX, 0xfe20);
                x.set16(Obj::AX, 0);
                x.set16(Obj::VZ, 0x340);
                x.set16(Obj::AZ, 0xfff0);
                x.call(sfx::JUMP);
                x.anim_start(0x17788c, 7);
            }
            0 => {
                x.anim_step();
            }
            _ => {
                x.anim_step();
                if x.u8(0x33) == 0 && !fading(x) {
                    return free(x);
                }
                x.move_xz();
            }
        },
        3 => match sub {
            0 => {
                if scene_go(x) {
                    x.add8(f::SUB_STATE, 2);
                    x.call(sfx::CHEER);
                }
                x.anim_step();
            }
            _ => {
                x.add16(Obj::X, 0xfffe);
                x.anim_step();
                if !fading(x) {
                    free(x);
                }
            }
        },
        4 => {
            if sub != 0 && !fading(x) {
                return free(x);
            }
            follow_host(x, 0x20);
            if sub != 0 {
                x.anim_step();
            } else if scene_go(x) {
                x.add8(f::SUB_STATE, 2);
                x.anim_step();
            }
        }
        5 => {
            follow_host(x, 0x10);
            if sub == 0 {
                if scene_go(x) {
                    x.add8(f::SUB_STATE, 2);
                }
            } else if !fading(x) {
                free(x);
            }
        }
        _ => last_actor(x, sub),
    }
}

/// Variante 6 : tombe sur la scène, joue son animation, attend le retour au jeu puis clignote et s'en va.
fn last_actor(x: &mut Cx, sub: u8) {
    match sub {
        0 => {
            let z = x.u16(Obj::Z) as i16 as i32 - 6;
            x.add16(Obj::Z, 0xfffa);
            if z <= 0 {
                x.add8(f::SUB_STATE, 2);
                x.call(sfx::LANDING);
                x.set16(Obj::Z, 0);
                x.anim_start(fireworks::ANIMS_LAST, 5);
                return;
            }
            if x.u8(0x33) == 0 {
                x.anim_step();
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x40);
            }
        }
        4 => {
            if x.a5_u8(fireworks::SCENE_STEP) == fireworks::BACK {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x40);
            }
        }
        _ => {
            if countdown(x, fireworks::TIMER) <= 0 {
                return free(x);
            }
            let v = x.u8(f::FLAGS) ^ 2;
            x.set8(f::FLAGS, v);
        }
    }
}

/// Variante 0 : le personnage qui tire le feu d'artifice (neuf étapes).
fn hero(x: &mut Cx, sub: u8) {
    match sub {
        0 => {
            if !scene_go(x) {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VX, 0x140);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x340);
            x.set16(Obj::AZ, 0xfff0);
            x.anim_start(fireworks::ANIMS_HERO, 4);
            // la scène passe au noir : palettes sauvées, fondu
            copy_block(x, 0x91c000, 0x910000);
            copy_block(x, 0x91c800, 0x910400);
            copy_block(x, 0x91cc00, 0x910800);
            x.set_a5_u16(fireworks::FADE_MODE, 3);
            x.set_a5_u8(fireworks::FADE_SPEED, 0xf);
            x.set_a5_u8(fireworks::FADING, 1);
            fade_task(x, fireworks::FADE_OUT);
        }
        2 => {
            x.move_xz();
            x.anim_step();
            if fading(x) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::X, 0xc0);
            x.set16(Obj::Z, 0);
            load_colours(x, 0x253260, 0x91c260, 0x10);
            x.set16(fireworks::TIMER, 0x10);
            x.set16(fireworks::DELAY, 0);
            x.anim_start(fireworks::ANIMS_HERO, 0);
        }
        4 => hero_rockets(x),
        6 => {
            if !fading(x) {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0x14);
                x.call(sfx::CLEAR_SCROLL1);
            }
            x.anim_step();
        }
        8 => {
            if countdown(x, fireworks::DELAY) <= 0 {
                // le décor 0x55, quitte à attendre une place
                let decor = loop {
                    x.c.d[0] = 0x55;
                    x.call(fireworks::SCENE_SET);
                    if x.c.sr & m68k::SR_C == 0 {
                        break x.c.a[1];
                    }
                };
                x.set16(fireworks::DECOR, decor as u16);
                x.add8(f::SUB_STATE, 2);
            }
            x.anim_step();
        }
        10 => {
            let decor = x.linked(fireworks::DECOR);
            if decor.u8(x.b, 0x22) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(fireworks::TIMER, 0xa);
            }
            x.anim_step();
        }
        12 => {
            if countdown(x, fireworks::TIMER) > 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VZ, 0x700);
            x.set16(Obj::AZ, 0);
            x.set32(Obj::VX, 0);
            x.c.d[0] = 0;
            x.call(sfx::LIFT_OFF);
            x.anim_start(fireworks::ANIMS_HERO, 6);
        }
        14 => hero_lift_off(x),
        _ => {}
    }
}

/// Étape 4 : une fusée à chaque délai écoulé, la dernière avec un son ; puis quatre images d'attente
/// (la tâche cède la main) et le fondu de retour.
fn hero_rockets(x: &mut Cx) {
    if countdown(x, fireworks::DELAY) > 0 {
        return;
    }
    let t = countdown(x, fireworks::TIMER);
    if t >= 0 {
        if t == 0 {
            x.call(sfx::LAST_ROCKET);
        }
        return rocket(x);
    }
    x.add8(f::SUB_STATE, 2);
    // l'attente cède la main : la tâche reprendra à `AFTER_WAIT` (native `fireworks_after_wait`), puis
    // à `AFTER_STATE`, comme l'original ; si elle revient sans céder, la suite est faite ici
    crate::taches::table::appeler(x.c, x.b, fireworks::AFTER_STATE);
    x.c.d[0] = 4;
    x.call_at(fireworks::WAIT, fireworks::AFTER_WAIT);
    fireworks_fade_back(x);
    crate::taches::table::depiler_retour(x.c, x.b);
}

/// Après l'attente : fondu de retour depuis les palettes sauvées.
fn fireworks_fade_back(x: &mut Cx) {
    x.set_a5_u16(fireworks::FADE_MODE, 1);
    x.set_a5_u8(fireworks::FADE_SPEED, 0xf);
    x.set_a5_u8(fireworks::FADING, 1);
    for (k, v) in [0x910000u32, 0x25c0c0, 0x910400, 0x910800].into_iter().enumerate() {
        x.set_a5_u32(fireworks::FADE_SOURCES + 4 * k as u32, v);
    }
    fade_task(x, fireworks::FADE_IN);
}

/// `0xb95e8` : une fusée (type 50) : variante, délai jusqu'à la suivante et place lus pour le tir `TIMER`.
fn rocket(x: &mut Cx) {
    let Some(o) = x.alloc(&pools::EFFCT) else { return };
    o.set_u8(x.b, Obj::ACTIVE, 1);
    o.set_u8(x.b, Obj::KIND, 0x32);
    let n = x.u16(fireworks::TIMER) as i16 as i32 as u32;
    let variant = x.rom8(fireworks::ROCKET_VARIANTS.wrapping_add(n));
    o.set_u8(x.b, Obj::VARIANT, variant);
    let delay = x.rom16(fireworks::ROCKET_DELAYS.wrapping_add(2 * n));
    x.set16(fireworks::DELAY, delay);
    let px = x.a5_u16(fireworks::CAMERA_X).wrapping_add(x.rom16(fireworks::ROCKET_PLACES.wrapping_add(4 * n)));
    let pz = x.a5_u16(fireworks::CAMERA_Y).wrapping_add(x.rom16(fireworks::ROCKET_PLACES.wrapping_add(4 * n + 2)));
    o.set_u16(x.b, Obj::X, px);
    o.set_u16(x.b, Obj::Z, pz);
}

/// Étape 14 : s'envole en laissant une traînée (type 50, variante 7) toutes les cinq images, jusqu'en haut
/// de l'écran : la scène se termine (`A5+0x2c` = 0x10000000).
fn hero_lift_off(x: &mut Cx) {
    if countdown(x, fireworks::TIMER) <= 0 {
        x.set16(fireworks::TIMER, 5);
        if let Some(o) = x.alloc(&pools::EFFCT) {
            o.set_u8(x.b, Obj::ACTIVE, 1);
            o.set_u8(x.b, Obj::KIND, 0x32);
            o.set_u8(x.b, Obj::VARIANT, 7);
            o.set_u16(x.b, fireworks::DECOR, x.o.0 as u16);
            let facing = x.u8(f::FACING);
            o.set_u8(x.b, f::FACING, facing);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                o.set_u16(x.b, off, v);
            }
            let y = o.u16(x.b, Obj::Y).wrapping_add(1);
            o.set_u16(x.b, Obj::Y, y);
            let frame = x.u32(0x34);
            o.set_u32(x.b, 0x34, frame);
            for off in [0x3cu32, 0x3f] {
                let v = x.u8(off);
                o.set_u8(x.b, off, v);
            }
            o.set_u16(x.b, 0x30, 0xe);
        }
    }
    x.move_xz();
    let top = x.a5_u16(fireworks::CAMERA_Y).wrapping_add(0x100);
    let height = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
    if height as i16 >= top as i16 {
        x.add8(f::SUB_STATE, 2);
        x.set_a5_u32(0x2c, 0x1000_0000);
        set_hidden(x, true);
    }
}

/// Reprise en `0xb91d0` (après l'attente de quatre images) : fondu de retour, puis retour en `AFTER_STATE`.
fn fireworks_after_wait(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let mut x = Cx::new(c, b);
    fireworks_fade_back(&mut x);
    x.ret();
}

/// `0xb8f04` : fin du gestionnaire après la reprise.
fn fireworks_after_state(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus) {
    let mut x = Cx::new(c, b);
    fireworks_tail(&mut x);
    x.ret();
}

/// Points de reprise du type 80 (la tâche y revient après avoir cédé la main dans `hero_rockets`).
pub const RESUMES: &[crate::Native] = &[
    super::cx::handler(fireworks::AFTER_WAIT, "effct_feu_artifice_reprise", fireworks_after_wait),
    super::cx::handler(fireworks::AFTER_STATE, "effct_feu_artifice_fin", fireworks_after_state),
];

handlers! {
    0xb8ef6 "effct_feu_artifice" fireworks,
    0xb59ea "effct_machine_volante" flyer,
    0xb5454 "effct_armure" armour,
    0xb4bec "effct_pluie_debris" debris_rain,
    0xb7866 "effct_morceau_ennemi" chunk,
    0xb96dc "effct_eclat_lance" shard,
}
