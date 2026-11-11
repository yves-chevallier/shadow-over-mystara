use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Host, view};

const ANIMS: RomAddr = RomAddr(0x17b9c2);
const HOST: u32 = 0x46;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if (x.u16(HOST) as i16) >= 0 {
        return vanish(x);
    }
    let host = x.linked(HOST);
    if host.u8(x.b, Obj::ACTIVE) == 0 {
        return vanish(x);
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::SUB_STATE, 0);
            x.set8(Shot5::RING_X, 0);
            x.set8(Shot5::RING_Y, 0);
            x.set8(Obj::SPEED_INDEX, 0);
            x.anim_start(ANIMS.0, 0);
            let kind = host.u8(x.b, Obj::KIND);
            x.set8(Shot::ATTACK_ID, if kind == 0xc { 0x29 } else { 0x2a });
            x.set8(Base::BOX_SET, 7);
            boxes(x, host);
        }
        2 => {
            if vars::GAME_PAUSED.get(x.b) == 0 {
                x.set8(Shot5::CLEARED, 0);
                x.set8(Obj::COUNTER_53, 0);
                select_frame(x);
                boxes(x, host);
            }
            let flags = host.u8(x.b, Obj::FLAGS);
            x.set8(Obj::FLAGS, flags);
            x.reinsert();
        }
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn vanish<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Obj::STATE, st::FREE);
    x.set8(Base::SUB_STATE, 0);
}

/// Collision boxes according to the enemy's attack state: disarmed (0) for some states.
fn boxes<R: Bench>(x: &mut Cx<'_, R>, host: Obj) {
    let row = x.u8(Base::SETTING) as u32;
    let index = box_index(x, row, 3);
    x.set8(Base::GROUND_MARK, index);
    let (kind, detail) = (host.u16(x.b, Host::ATTACK), host.u8(x.b, Host::ATTACK_DETAIL));
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
        let v = if armed { box_index(x, row, i) } else { 0 };
        x.set8(Base::BODY_BOX + i, v);
    }
}

/// The tables of the shot (`sprites/tables.json`, `shot5`).
fn shot5() -> &'static ddsom_data::tables::Shot5 {
    &crate::world::data::data().sprite_tables.shot5
}

/// Byte `i` of the boxes of `+0x2f` = `row` (`shot5.boxes`, program space): index of the body, of the push, of the
/// attack, then an index stored in `+0x3b`.
fn box_index<R: Bench>(x: &mut Cx<'_, R>, row: u32, i: u32) -> u8 {
    let t = shot5();
    match t.boxes.get(row as usize) {
        Some(b) => b[i as usize],
        None => {
            let a = t.boxes_rom + 4 * row + i;
            crate::world::data::Missing { table: "shot5 boxes", key: t.boxes_rom }.byte_in(x.b, a, true)
        }
    }
}

/// Frame `n` of the frame list of `+0x2f` = `set`, in the even (`odd` false) or odd lists: the address of the frame
/// (`+0x34`). The lists are designated by offsets (words, relative to their table, program space).
fn list_frame<R: Bench>(x: &mut Cx<'_, R>, set: u32, odd: bool, n: u32) -> u32 {
    let t = shot5();
    let (base, offsets) = if odd { (t.odd_rom, &t.odd) } else { (t.even_rom, &t.even) };
    let offset = match offsets.get(set as usize) {
        Some(&o) => o as u16,
        None => crate::world::data::Missing { table: "shot5 lists", key: base }.word_in(x.b, base + 2 * set, true),
    };
    let list = base.wrapping_add(offset as i16 as i32 as u32);
    let d = crate::world::data::data();
    let frame = d.sprite_tables.frame_lists.iter().find(|l| l.rom == list).and_then(|l| l.frames.get(n as usize));
    match frame {
        Some(&f) => d.frames.number(f).rom,
        None => crate::world::data::Missing { table: "shot5 frame list", key: list }.read32(x.b, list + 4 * n),
    }
}

/// Entry `i` of the choice of the odd frame (`shot5.mirrored`, data space): (frame, flip).
fn mirrored<R: Bench>(x: &mut Cx<'_, R>, i: u32) -> [u8; 2] {
    let t = shot5();
    match t.mirrored.get(i as usize) {
        Some(&m) => m,
        None => {
            let a = t.mirrored_rom + 2 * i;
            let m = crate::world::data::Missing { table: "shot5 mirrored", key: t.mirrored_rom };
            [m.read8(x.b, a), m.read8(x.b, a + 1)]
        }
    }
}

/// Chooses the sprite frame (`+0x34`); `+0x2d` is a counter 1..=0x11.
fn select_frame<R: Bench>(x: &mut Cx<'_, R>) {
    let mut n = x.u8(Obj::SPEED_INDEX) as u16;
    if n >= 0x11 {
        n = 0;
    }
    n += 1;
    x.set8(Obj::SPEED_INDEX, n as u8);
    let host = x.linked(HOST);
    let set = x.u8(Base::SETTING) as u32;
    if host.u8(x.b, Host::FROZEN) != 0 || n & 1 == 0 {
        let frame = list_frame(x, set, false, (n / 2) as u32);
        x.set32(Obj::FRAME, frame);
    } else if (x.u16(HOST) as i16) < 0 {
        let row = (x.u8(Shot5::RING_X) as u32 >> 1 << 4) + (x.u8(Shot5::RING_Y) as u32 >> 1);
        let [_, flip] = mirrored(x, row);
        x.set8(Base::FLIP, flip);
        // the original clears D0 before this read: the frame always comes from the first row
        let [slot, _] = mirrored(x, 0);
        let frame = list_frame(x, set, true, slot as u32);
        x.set32(Obj::FRAME, frame);
    }
}

view! {
    /// Fields of shot 5.
    Shot5 {
        /// Selects the odd frame, first index (h).
        ring_x, set_ring_x, RING_X: u8 @ 0x42;
        /// Selects the odd frame, second index (h).
        ring_y, set_ring_y, RING_Y: u8 @ 0x43;
        /// Cleared each frame (h).
        cleared, set_cleared, CLEARED: u8 @ 0x56;
    }
}
