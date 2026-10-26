//! Famille EFFCT (suite) : types qu'aucun scénario n'exerce, réécrits pour que la table des types
//! `0x39ce0` n'ait plus d'entrée en code d'origine (validation par `examples/fuzz_effct.rs`).
//!
//! - 43 : pluie de débris (`0xb4bcc`) : en haut de l'écran (réglage 0, avec une poussière au départ) ou
//!   depuis le créateur (`+0x5c`), vitesses et animation tirées au hasard ; elle tombe, clignote et
//!   disparaît, ou disparaît avec le créateur ;
//! - 46 : pièces d'armure d'un ennemi (`0xb53f4`) : elles suivent le porteur et rejouent ses coups reçus,
//!   tombent quand sa vie passe sous leur seuil et se brisent en éclats (variante 1) qui rebondissent ;
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
    pub const ALLOC_EFFECT: u32 = 0x2318;
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
    x.call(super::cx::sub::PAUSE_HOOK);
    if x.c.sr & m68k::SR_Z == 0 {
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
    x.anim_start(set, hit as u32);
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
        let Some(o) = alloc(x) else { return };
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
fn alloc(x: &mut Cx) -> Option<Obj> {
    x.call(rom::ALLOC_EFFECT);
    (x.c.sr & m68k::SR_C == 0).then(|| Obj(x.c.a[1]))
}

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
        x.call(super::cx::sub::MOVE_XYZ);
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
        x.call(super::cx::sub::MOVE_Z);
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

handlers! {
    0xb5454 "effct_armure" 200 armour,
    0xb4bec "effct_pluie_debris" 200 debris_rain,
    0xb7866 "effct_morceau_ennemi" 200 chunk,
    0xb96dc "effct_eclat_lance" 150 shard,
}
