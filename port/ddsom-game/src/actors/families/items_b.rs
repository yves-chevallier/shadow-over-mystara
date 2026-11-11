//! ITEM family (continued): type 2 (`0xdb0c2`), equipment piece with a fixed slot.
//!
//! One sheet per variant (table in the clear `0xdb522`, 8 bytes) gives the player slot where the piece is stored
//! (`+0x5b`, eight handlers `0xdb392`), its identifier, the variant of its sparkle (`+0x54`), its hit counter
//! (`+0x5c`), its index in the collectible objects table (`+0x5d`) and the variant of the SET 0xa object that announces
//! it (`+0x5e`). When picked up, it replaces the piece the player was wearing in that slot: the old one is thrown off
//! in an arc, like an ITEM 2 thrown by the player (mode `+0x2f` = 1, delay `+0x59`) that falls back and waits to be
//! picked up again. If already worn, it yields only a score event.

use crate::actors::player::Player;
use crate::actors::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::tables::{tables, Read};
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;

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
    /// Non-zero: piece thrown off by a player.
    pub const THROWN: u32 = 0x2f;
    /// Frames before disappearance (word).
    pub const LIFETIME: u32 = 0x42;
    /// Player who picks up or threw off the piece.
    pub const OWNER: u32 = 0x50;
    pub const SPARKLE: u32 = 0x54;
    /// Set by the player code: the piece is picked up.
    pub const PICKED: u32 = 0x58;
    pub const DELAY: u32 = 0x59;
    /// Sheet: slot, hits, collection index, SET 0xa variant.
    pub const SLOT: u32 = 0x5b;
    pub const HITS: u32 = 0x5c;
    pub const COLLECTION: u32 = 0x5d;
    pub const ANNOUNCE: u32 = 0x5e;
}

/// Player fields.
mod p {
    pub const NUMBER: u32 = 0x02;
    pub const CHARACTER: u32 = 0x03;
    pub const Y: u32 = 0x10;
    /// Ready to pick up.
    pub const READY: u32 = 0x33;
    /// Second equipment in hand.
    pub const SECOND_HAND: u32 = 0x10e;
}

mod rom {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bee82);
}

/// Table of collected objects picked up (bytes).
const SHADOW_SET: u8 = 0x3c;
const LIFETIME: u16 = 0x4b0;

fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let state = x.u8(f::STATE);
    match state {
        0 => init(x),
        2 => active(x),
        4 | 6 => x.free(&pools::ITEM),
        _ => crate::actors::families::effects::bad_index(state),
    }
}

/// Copies the variant's sheet (the sparkle except for a thrown-off piece); A1 = the table, D0 = the row offset.
fn load_sheet<R: Bench>(x: &mut Cx<'_, R>, sparkle: bool) {
    let row = 8 * x.u8(f::VARIANT) as u32;
    let sheet = row;
    let v = tables().equipment_b_sheets.at8(x.b, sheet);
    x.set8(f::SLOT, v);
    if sparkle {
        let v = tables().equipment_b_sheets.at8(x.b, sheet + 2);
        x.set8(f::SPARKLE, v);
    }
    for (i, off) in [(3, f::HITS), (4, f::COLLECTION), (5, f::ANNOUNCE)] {
        let v = tables().equipment_b_sheets.at8(x.b, sheet + i);
        x.set8(off, v);
    }
}

fn start_animation<R: Bench>(x: &mut Cx<'_, R>) {
    let variant = x.u8(f::VARIANT) as u32;
    x.anim(rom::ANIMATIONS.0, variant);
    x.reinsert();
}

/// State 0: a thrown-off piece waits for its delay; another one falls (in the air) or waits on the ground, sparkling.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
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

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let sub = x.u8(f::SUB);
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
        _ => crate::actors::families::effects::bad_index(sub),
    }
}

/// Substate 0: fall; on the ground, the piece is brought back into view (unless thrown off) and sparkles.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.move_xyz().gt() {
        x.add8(f::SUB, 2);
        x.set16(f::LIFETIME, LIFETIME);
        x.set32(f::Z, 0);
        if x.u8(f::THROWN) == 0 {
            into_view(x);
            super::effects_creators::twinkle(x.b, x.o);
        }
    }
    x.keep_in_zone();
    x.push_terrain();
    x.reinsert();
}

/// Brings the piece back into view: at most as high as `camera.y + 0x20`, between `camera.x + 0x18` and `+ 0x168`.
fn into_view<R: Bench>(x: &mut Cx<'_, R>) {
    let cam2 = crate::world::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0;
    let (cam_x, cam_y) = if cam2 { (crate::world::ram::CAMERA_ALT_X.get(x.b), crate::world::ram::CAMERA_ALT_Y.get(x.b)) } else { (crate::world::ram::CAMERA_X.get(x.b), crate::world::ram::CAMERA_Y.get(x.b)) };
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

/// `0xdb320`: the lifetime runs out; the piece blinks during its last 0x3c frames, then disappears.
fn age<R: Bench>(x: &mut Cx<'_, R>) {
    let left = x.u16(f::LIFETIME).wrapping_sub(1);
    x.set16(f::LIFETIME, left);
    if (left as i16) < 0 {
        x.set8(f::STATE, 4);
        x.set8(f::ACTIVE, 2);
    }
    if left <= 0x3c {
        let phase = left.wrapping_add(x.counter()) & 1;
        x.set_bit(f::FLAGS, 1, phase == 0);
    }
}

/// Substate 6: the thrown-off piece waits for its delay, then leaves in an arc from the player, toward the center of
/// the screen; once fallen, it waits to be picked up.
fn thrown<R: Bench>(x: &mut Cx<'_, R>) {
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
    let cam_x = crate::actors::families::shell::camera(x.b).0.get(x.b);
    let middle = cam_x.wrapping_add(0xc0);
    if middle as i16 <= x.u16(f::X) as i16 {
        let v = x.u32(f::VX).wrapping_neg();
        x.set32(f::VX, v);
    }
    x.reinsert();
}

/// `0xdb350`: the picked-up piece takes its place on the player (handler of its slot), is recorded in the collectible
/// objects table, then announced (SET 0xa).
fn equip<R: Bench>(x: &mut Cx<'_, R>) {
    let player = x.linked(f::OWNER);
    let row = 8 * x.u8(f::VARIANT) as u32;
    let id = tables().equipment_b_sheets.at8(x.b, row + 1);
    let slot = x.u8(f::SLOT) as u32;
    // slot (handlers of the table 0xdb392): (worn byte, hit counter, offset of the number of the thrown-off piece)
    match slot {
        0 => replace(x, player, id, 0x160, None, 0),
        1 => {}
        2 => replace(x, player, id, 0x162, Some(0x16b), 6),
        3 => replace(x, player, id, 0x163, Some(0x16c), 9),
        4 => slot_4(x, player, id),
        5 => replace(x, player, id, 0x165, Some(0x16d), 0x12),
        6 => replace(x, player, id, 0x166, None, 0x19),
        7 => replace(x, player, id, 0x167, None, 0x1f),
        _ => crate::actors::families::effects::bad_index(slot),
    }
    let collection = x.u8(f::COLLECTION) as u32;
    crate::world::mem::wr8(x.b, crate::actors::families::shell::vars::PICKED.0 + collection, 1);
    let announce = x.u8(f::ANNOUNCE);
    if announce != 0 {
        let o = x.o;
        super::creators::set_a_player(x.b, o, announce);
    }
}

/// A piece worn at `worn` (hit counter `hits`) replaced by `id`: already worn, simple event; otherwise the old one
/// (number + `shift`) is thrown off. Slot 2 does not throw off piece 1.
fn replace<R: Bench>(x: &mut Cx<'_, R>, player: Obj, id: u8, worn: u32, hits: Option<u32>, shift: u8) {
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
        // Original glitch: the previous hit count stays in D2.b (`dispatch::Leftovers`)
        *x.c.d2() = *x.c.d2() & !0xff | previous as u32;
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

/// Slot 4: forbidden to some characters (the piece itself is thrown off); otherwise like the others, and the second
/// equipment in hand follows the new piece if it depended on it.
fn slot_4<R: Bench>(x: &mut Cx<'_, R>, player: Obj, id: u8) {
    let character = player.u8(x.b, p::CHARACTER) as u32;
    // Original glitch: the character stays in D2 (`dispatch::Leftovers`)
    *x.c.d2() = character;
    let forbidden = tables().equipment_b_slot_4_forbidden.at8(x.b, character);
    if forbidden != 0 {
        let variant = x.u8(f::VARIANT);
        return drop_old(x, player, variant);
    }
    let old = player.u8(x.b, Player::WORN_4);
    if old == id {
        return already_worn(x);
    }
    player.set_u8(x.b, Player::WORN_4, id);
    if (old as i8) < 0 {
        return;
    }
    let hand = player.u8(x.b, p::SECOND_HAND).wrapping_sub(4);
    *x.c.d2() = *x.c.d2() & !0xff | hand as u32;
    if hand as i8 > 3 {
        let v = id.wrapping_add(0x28);
        player.set_u8(x.b, p::SECOND_HAND, v);
    }
    drop_old(x, player, old.wrapping_add(0xa));
}

/// `0xdb500`: the piece was already worn: score event of the player, then sound.
fn already_worn<R: Bench>(x: &mut Cx<'_, R>) {
    let player = x.linked(f::OWNER);
    let number = (player.u8(x.b, p::NUMBER) as u32) << 8;
    crate::world::events::enqueue_points(x.b, (0x11 + number) as u16);
    let o = x.o;
    super::effects_creators::pickup(x.b, o, 5);
}

/// `0xdb672`: creates the ITEM 2 of variant `variant` thrown off by the player (A2), with hit counter D2.
fn drop_old<R: Bench>(x: &mut Cx<'_, R>, player: Obj, variant: u8) {
    let Some(n) = x.alloc(&pools::ITEM) else { return };
    n.set_u8(x.b, f::ACTIVE, 1);
    n.set_u8(x.b, f::KIND, 2);
    n.set_u8(x.b, f::VARIANT, variant);
    n.set_u8(x.b, f::THROWN, 1);
    n.set_u16(x.b, f::OWNER, player.0 as u16);
    let slot = x.u8(f::SLOT);
    n.set_u8(x.b, f::SLOT, slot);
    // the hit count is D2.b, whatever the caller left there (`dispatch::Leftovers`)
    let hits = *x.c.d2() as u8;
    n.set_u8(x.b, f::HITS, hits);
    let px = x.u16(f::X);
    n.set_u16(x.b, f::X, px);
    let y = player.u16(x.b, p::Y).wrapping_sub(1);
    n.set_u16(x.b, f::Y, y);
    n.set_u16(x.b, f::Z, 0x10);
    n.set_u16(x.b, f::AZ, 0xffc0);
    n.set_u8(x.b, f::DELAY, 0xc);
}

handlers! {
    ITEM_PIECE 0xdb0c2 "item_piece" run,
}
