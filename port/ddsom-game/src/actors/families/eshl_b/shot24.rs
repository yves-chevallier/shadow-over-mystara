use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::world::ram;
use crate::actors::families::shell::records::{Spawner, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16e920);
const SPAWN_ENEMIES_AT: u16 = 0x20;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let source = x.linked(Shot24::SOURCE);
    if x.u8(Obj::VARIANT) == 0 {
        x.set8(Base::BOX_SET, 7);
        x.set8(Shot::ATTACK_ID, 0x2f);
        x.set16(Shot24::TIMER, 0x50);
        x.set16(Shot24::LINK, 1);
        x.set16(Shot24::COUNT, 0);
        copy_position16(x, source);
        x.anim_start(ANIMS.0, 2);
        x.reinsert();
        x.set32(Obj::FRAME, 0);
        active(x);
    } else {
        copy_position16(x, source);
        let row = (x.u16(Shot24::COUNT) << 3) as u32;
        for (off, column) in [(0x8, 0), (0xc, 2), (0x10, 4)] {
            let d = tables().shot24_offsets.at16(x.b, row + column);
            x.add16(off, d);
        }
        x.anim_start(ANIMS.0, 1);
        x.reinsert();
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) != 0 {
        if x.anim_step() {
            x.add8(Obj::STATE, 2);
        }
        return;
    }
    x.add16(Shot24::TIMER, 0xffff);
    if x.u16(Shot24::TIMER) == 0 {
        x.add8(Obj::STATE, 2);
    }
    if x.u16(Shot24::TIMER) == SPAWN_ENEMIES_AT {
        spawn_enemies(x);
    }
    x.add16(Shot24::LINK, 0xffff);
    if x.u16(Shot24::LINK) == 0 {
        x.set16(Shot24::LINK, 2);
        spawn_effect(x);
    }
}

/// Launches the next effect (variant 1) and advances `+0x56` modulo 16.
fn spawn_effect<R: Bench>(x: &mut Cx<'_, R>) {
    if let Some(e) = x.alloc(&pools::ESHL) {
        e.set_u8(x.b, Obj::ACTIVE, 1);
        e.set_u8(x.b, Obj::KIND, 0x18);
        e.set_u8(x.b, Obj::VARIANT, 1);
        let (n, source) = (x.u16(Shot24::COUNT), x.u16(Shot24::SOURCE));
        e.set_u16(x.b, Spawner::COUNT, n);
        e.set_u16(x.b, Spawner::SOURCE, source);
        x.set16(Shot24::COUNT, n.wrapping_add(1) & 0xf);
    }
}

/// Spawns the group of enemies around the creator.
fn spawn_enemies<R: Bench>(x: &mut Cx<'_, R>) {
    let mut d0 = ram::PLAYERS_ALIVE.get(x.b) as u16;
    if d0 != 0 {
        d0 = d0.wrapping_sub(1) & 3;
    }
    d0 = d0.wrapping_add((vars::RANK.get(x.b) >> 1) << 2);
    let mut count = tables().shot24_group_sizes.at8(x.b, d0 as i16 as i32 as u32) as u16;
    count = count.wrapping_sub(1);
    let mut label: u16 = 1;
    let rng = x.random();
    let mut table = (rng as u32 & 7) << 3;
    loop {
        let Some(e) = x.alloc(&pools::ENEMY) else { return };
        e.set_u8(x.b, Obj::ACTIVE, 1);
        e.set_u8(x.b, Obj::KIND, 1);
        e.set_u8(x.b, Obj::VARIANT, 4);
        let source = x.linked(Shot24::SOURCE);
        let (sx, sy) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Y));
        let (dx, dy) = (tables().shot24_group_offsets.at16(x.b, table), tables().shot24_group_offsets.at16(x.b, table + 2));
        table += 4;
        e.set_u16(x.b, Obj::X, sx.wrapping_add(dx));
        e.set_u16(x.b, Obj::Y, sy.wrapping_add(dy));
        e.set_u16(x.b, Spawner::LABEL, label);
        label = label.wrapping_add(0x40);
        count = count.wrapping_sub(1);
        if count == 0xffff {
            return;
        }
    }
}

view! {
    /// Fields of shot 24 (a spawner, see `Spawner`).
    Shot24 {
        /// Number of created shots, row of the offsets (h).
        count, set_count, COUNT: u16 @ 0x56;
        /// Countdown of the current step (h).
        timer, set_timer, TIMER: u16 @ 0x58;
        /// Linked object: shadow or second timer (h).
        link, set_link, LINK: u16 @ 0x5a;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
        /// Row of the height tables (h).
        row, set_row, ROW: u8 @ 0x60;
    }
}
