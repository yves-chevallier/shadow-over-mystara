//! Famille ITEM (suite) : type 2 (`0xdb0c2`), pièce d'équipement à emplacement fixe.
//!
//! Une fiche par variante (table en clair `0xdb522`, 8 octets) donne l'emplacement du joueur où la pièce se
//! range (`+0x5b`, huit gestionnaires `0xdb392`), son identifiant, la variante de son scintillement
//! (`+0x54`), son compteur de coups (`+0x5c`), son indice dans le tableau des objets de collection
//! (`+0x5d`) et la variante de l'objet SET 0xa qui l'annonce (`+0x5e`). Ramassée, elle remplace la pièce
//! que le joueur portait à cet emplacement : l'ancienne est rejetée en arc, comme un ITEM 2 lancé par le
//! joueur (mode `+0x2f` = 1, délai `+0x59`) qui retombe et attend d'être repris. Déjà portée, elle ne
//! rapporte qu'un événement de score.

use crate::families::cx::Banc;
use super::cx::{handlers, Cx};
use crate::mem::A5;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;

mod f {
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    pub const KIND: u32 = 0x02;
    pub const VARIANT: u32 = 0x03;
    pub const STATE: u32 = 0x04;
    pub const SUB: u32 = 0x05;
    pub const SUB2: u32 = 0x06;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    pub const VZ: u32 = 0x18;
    pub const AZ: u32 = 0x1a;
    pub const SHADOWS: u32 = 0x2e;
    /// Non nul : pièce rejetée par un joueur.
    pub const THROWN: u32 = 0x2f;
    /// Images avant la disparition (mot).
    pub const LIFETIME: u32 = 0x42;
    /// Joueur qui ramasse ou qui a rejeté la pièce.
    pub const OWNER: u32 = 0x50;
    pub const SPARKLE: u32 = 0x54;
    /// Posé par le code des joueurs : la pièce est ramassée.
    pub const PICKED: u32 = 0x58;
    pub const DELAY: u32 = 0x59;
    /// Fiche : emplacement, coups, indice de collection, variante du SET 0xa.
    pub const SLOT: u32 = 0x5b;
    pub const HITS: u32 = 0x5c;
    pub const COLLECTION: u32 = 0x5d;
    pub const ANNOUNCE: u32 = 0x5e;
}

/// Champs du joueur.
mod p {
    pub const NUMBER: u32 = 0x02;
    pub const CHARACTER: u32 = 0x03;
    pub const Y: u32 = 0x10;
    /// Prêt à ramasser.
    pub const READY: u32 = 0x33;
    /// Second équipement en main.
    pub const SECOND_HAND: u32 = 0x10e;
}

mod rom {
    pub const STATES: u32 = 0xdb0d0;
    pub const SUB_STATES: u32 = 0xdb1fa;
    pub const SHEETS: u32 = 0xdb522;
    pub const SLOT_HANDLERS: u32 = 0xdb392;
    /// Par personnage : la pièce de l'emplacement 4 lui est interdite.
    pub const SLOT_4_FORBIDDEN: u32 = 0xdb482;
    pub const ANIMATIONS: u32 = 0x1bee82;
}

/// Tableau des objets de collection ramassés (octets).
const COLLECTED: u32 = A5 - 0x5b88;
const SHADOW_SET: u8 = 0x3c;
const LIFETIME: u16 = 0x4b0;

/// `jmp (table,PC,D0.w)` : D0 = indice, D1.w = mot de la table ; renvoie la cible.
fn jump<R: Banc>(x: &mut Cx<'_, R>, table: u32, i: u8) -> u32 {
    let w = x.rom16(table + i as u32);
    table.wrapping_add(w as i16 as i32 as u32)
}

fn run(x: &mut Cx) {
    let state = x.u8(f::STATE);
    let target = jump(x, rom::STATES, state);
    match state {
        0 => init(x),
        2 => active(x),
        4 | 6 => x.free(&pools::ITEM),
        _ => crate::families::effects::hors_table(target),
    }
}

/// Recopie la fiche de la variante (le scintillement sauf pour une pièce rejetée) ; A1 = la table, D0 =
/// le décalage de la ligne.
fn load_sheet<R: Banc>(x: &mut Cx<'_, R>, sparkle: bool) {
    let row = 8 * x.u8(f::VARIANT) as u32;
    let sheet = rom::SHEETS + row;
    let v = x.data8(sheet);
    x.set8(f::SLOT, v);
    if sparkle {
        let v = x.data8(sheet + 2);
        x.set8(f::SPARKLE, v);
    }
    for (i, off) in [(3, f::HITS), (4, f::COLLECTION), (5, f::ANNOUNCE)] {
        let v = x.data8(sheet + i);
        x.set8(off, v);
    }
}

fn start_animation(x: &mut Cx) {
    let variant = x.u8(f::VARIANT) as u32;
    x.anim(rom::ANIMATIONS, variant);
    x.reinsert();
}

/// État 0 : une pièce rejetée attend son délai ; une autre tombe (en l'air) ou attend au sol en scintillant.
fn init(x: &mut Cx) {
    if x.u8(f::THROWN) != 0 {
        let delay = x.u8(f::DELAY);
        if delay != 0 {
            return x.set8(f::DELAY, delay - 1);
        }
        x.add8(f::STATE, 2);
        x.set8(f::SHADOWS, SHADOW_SET);
        load_sheet(x, false);
    } else if x.u16(f::Z) != 0 {
        x.add8(f::STATE, 2);
        x.set8(f::SUB, 0);
        x.set16(f::SUB2, 0);
        x.set8(f::SHADOWS, SHADOW_SET);
        load_sheet(x, true);
    } else {
        x.add8(f::STATE, 2);
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
        x.set8(f::SHADOWS, SHADOW_SET);
        x.set16(f::LIFETIME, LIFETIME);
        load_sheet(x, true);
        super::effects_creators::twinkle(x.b, x.o);
    }
    start_animation(x);
}

fn active(x: &mut Cx) {
    let sub = x.u8(f::SUB);
    let target = jump(x, rom::SUB_STATES, sub);
    match sub {
        0 => fall(x),
        2 => {
            if x.u8(f::PICKED) != 0 {
                x.add8(f::SUB, 2);
            }
            age(x);
        }
        4 => {
            let player = x.linked(f::OWNER);
            if player.u8(x.b, p::READY) != 0 {
                x.add8(f::STATE, 2);
                equip(x);
            }
        }
        6 => thrown(x),
        _ => crate::families::effects::hors_table(target),
    }
}

/// Sous-état 0 : chute ; au sol, la pièce est ramenée dans la vue (sauf rejetée) et scintille.
fn fall(x: &mut Cx) {
    if !x.move_xyz().gt() {
        x.add8(f::SUB, 2);
        x.set16(f::LIFETIME, LIFETIME);
        x.set32(f::Z, 0);
        if x.u8(f::THROWN) == 0 {
            into_view(x);
            super::effects_creators::twinkle(x.b, x.o);
        }
    }
    crate::ai::keep_in_zone_d5(x.c, x.b, x.o);
    crate::terrain::push_68k(x.c, x.b, x.o);
    x.reinsert();
}

/// Ramène la pièce dans la vue : au plus haut à `caméra.y + 0x20`, entre `caméra.x + 0x18` et `+ 0x168`.
fn into_view<R: Banc>(x: &mut Cx<'_, R>) {
    let cam2 = crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0;
    let (cam_x, cam_y) = if cam2 { (crate::ram::CAMERA_ALT_X.get(x.b), crate::ram::CAMERA_ALT_Y.get(x.b)) } else { (crate::ram::CAMERA_X.get(x.b), crate::ram::CAMERA_Y.get(x.b)) };
    let top = cam_y.wrapping_add(0x20);
    if top >= x.u16(f::Y) {
        x.set16(f::Y, top);
    }
    let left = cam_x.wrapping_add(0x18);
    let px = x.u16(f::X);
    if left as i16 > px as i16 {
        x.set16(f::X, left);
    } else {
        let right = cam_x.wrapping_add(0x168);
        if right <= px {
            x.set16(f::X, right);
        }
    }
}

/// `0xdb320` : la durée de vie s'écoule ; la pièce clignote pendant ses 0x3c dernières images, puis
/// disparaît.
fn age(x: &mut Cx) {
    let left = x.u16(f::LIFETIME).wrapping_sub(1);
    x.set16(f::LIFETIME, left);
    if (left as i16) < 0 {
        x.set8(f::STATE, 4);
        x.set8(f::ACTIVE, 2);
    }
    if left <= 0x3c {
        let phase = left.wrapping_add(x.c.d[7] as u16) & 1;
        x.set_bit(f::FLAGS, 1, phase == 0);
    }
}

/// Sous-état 6 : la pièce rejetée attend son délai, puis part en arc depuis le joueur, vers le centre de
/// l'écran ; retombée, elle attend d'être ramassée.
fn thrown<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        x.move_xz();
        if x.u16(f::Z) as i16 >= 0 {
            return;
        }
        x.set8(f::SUB, 2);
        x.set8(f::SUB2, 0);
        x.set32(f::Z, 0);
        x.set8(f::PICKED, 0);
        return;
    }
    let delay = x.u8(f::DELAY).wrapping_sub(1);
    x.set8(f::DELAY, delay);
    if delay != 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    let player = x.linked(f::OWNER);
    let y = player.u16(x.b, p::Y).wrapping_sub(1);
    x.set16(f::Y, y);
    x.set16(f::Z, 0x10);
    x.set16(f::VZ, 0);
    x.set16(f::AZ, 0xffc0);
    x.set32(f::VX, 0x0200_0000);
    let cam2 = crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0;
    let cam_x = x.a5_u16(if cam2 { 0x556 } else { 0x506 });
    let middle = cam_x.wrapping_add(0xc0);
    if middle as i16 <= x.u16(f::X) as i16 {
        let v = x.u32(f::VX).wrapping_neg();
        x.set32(f::VX, v);
    }
    x.reinsert();
}

/// `0xdb350` : la pièce ramassée prend sa place sur le joueur (gestionnaire de son emplacement), est notée
/// dans le tableau des objets de collection, puis annoncée (SET 0xa).
fn equip(x: &mut Cx) {
    let player = x.linked(f::OWNER);
    let row = 8 * x.u8(f::VARIANT) as u32;
    let id = x.data8(rom::SHEETS + row + 1);
    let slot = x.u8(f::SLOT) as u32;
    let handler = x.rom32(rom::SLOT_HANDLERS + 4 * slot);
    // emplacement : (octet porté, compteur de coups, décalage du numéro de la pièce rejetée)
    match slot {
        0 => replace(x, player, id, 0x160, None, 0),
        1 => {}
        2 => replace(x, player, id, 0x162, Some(0x16b), 6),
        3 => replace(x, player, id, 0x163, Some(0x16c), 9),
        4 => slot_4(x, player, id),
        5 => replace(x, player, id, 0x165, Some(0x16d), 0x12),
        6 => replace(x, player, id, 0x166, None, 0x19),
        7 => replace(x, player, id, 0x167, None, 0x1f),
        _ => crate::families::effects::hors_table(handler),
    }
    let collection = x.u8(f::COLLECTION) as u32;
    crate::mem::wr8(x.b, COLLECTED + collection, 1);
    let announce = x.u8(f::ANNOUNCE);
    if announce != 0 {
        let o = x.o;
        super::creators::set_a_joueur(x.b, o, announce);
    }
}

/// Une pièce portée à `worn` (compteur de coups `hits`) remplacée par `id` : déjà portée, simple
/// événement ; sinon l'ancienne (numéro + `shift`) est rejetée. L'emplacement 2 ne rejette pas la pièce 1.
fn replace(x: &mut Cx, player: Obj, id: u8, worn: u32, hits: Option<u32>, shift: u8) {
    let old = player.u8(x.b, worn);
    if old == id {
        if let Some(h) = hits {
            let v = x.u8(f::HITS);
            player.set_u8(x.b, h, v);
        }
        return already_worn(x);
    }
    player.set_u8(x.b, worn, id);
    if let Some(h) = hits {
        let previous = player.u8(x.b, h);
        x.c.d[2] = x.c.d[2] & !0xff | previous as u32;
        let v = x.u8(f::HITS);
        player.set_u8(x.b, h, v);
    }
    if (old as i8) < 0 {
        return;
    }
    let dropped = old.wrapping_add(shift);
    if worn == 0x162 && dropped == 7 {
        return;
    }
    drop_old(x, player, dropped);
}

/// Emplacement 4 : interdit à certains personnages (la pièce elle-même est rejetée) ; sinon comme les
/// autres, et le second équipement en main suit la nouvelle pièce s'il en dépendait.
fn slot_4(x: &mut Cx, player: Obj, id: u8) {
    let character = player.u8(x.b, p::CHARACTER) as u32;
    x.c.d[2] = character;
    let forbidden = x.rom8(rom::SLOT_4_FORBIDDEN + character);
    if forbidden != 0 {
        let variant = x.u8(f::VARIANT);
        return drop_old(x, player, variant);
    }
    let old = player.u8(x.b, 0x164);
    if old == id {
        return already_worn(x);
    }
    player.set_u8(x.b, 0x164, id);
    if (old as i8) < 0 {
        return;
    }
    let hand = player.u8(x.b, p::SECOND_HAND).wrapping_sub(4);
    x.c.d[2] = x.c.d[2] & !0xff | hand as u32;
    if hand as i8 > 3 {
        let v = id.wrapping_add(0x28);
        player.set_u8(x.b, p::SECOND_HAND, v);
    }
    drop_old(x, player, old.wrapping_add(0xa));
}

/// `0xdb500` : la pièce était déjà portée : événement de score du joueur, puis son.
fn already_worn<R: Banc>(x: &mut Cx<'_, R>) {
    let player = x.linked(f::OWNER);
    let number = (player.u8(x.b, p::NUMBER) as u32) << 8;
    crate::evenements::enfiler_points(x.b, (0x11 + number) as u16);
    let o = x.o;
    super::effects_creators::pickup(x.b, o, 5);
}

/// `0xdb672` : crée l'ITEM 2 de variante `variant` rejeté par le joueur (A2), avec le compteur de coups D2.
fn drop_old(x: &mut Cx, player: Obj, variant: u8) {
    let Some(n) = x.alloc(&pools::ITEM) else { return };
    n.set_u8(x.b, f::ACTIVE, 1);
    n.set_u8(x.b, f::KIND, 2);
    n.set_u8(x.b, f::VARIANT, variant);
    n.set_u8(x.b, f::THROWN, 1);
    n.set_u16(x.b, f::OWNER, player.0 as u16);
    let slot = x.u8(f::SLOT);
    n.set_u8(x.b, f::SLOT, slot);
    n.set_u8(x.b, f::HITS, x.c.d[2] as u8);
    let px = x.u16(f::X);
    n.set_u16(x.b, f::X, px);
    let y = player.u16(x.b, p::Y).wrapping_sub(1);
    n.set_u16(x.b, f::Y, y);
    n.set_u16(x.b, f::Z, 0x10);
    n.set_u16(x.b, f::AZ, 0xffc0);
    n.set_u8(x.b, f::DELAY, 0xc);
}

handlers! {
    0xdb0c2 "item_piece" run,
}
