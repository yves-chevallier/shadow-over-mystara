use crate::world::objects::player::Player;
use super::*;

// ---------------------------------------------------------------------------------------------
// Type 87: barrier switch
// ---------------------------------------------------------------------------------------------

pub(super) mod switch {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bebc2);
    /// The controlled barrier (16-bit pointer); its non-zero byte `+0x5a`: already open.
    pub const BARRIER: u32 = 0x42;
    /// "Touched" flag read by the barrier.
    pub const PRESSED: u32 = 0x30;
    /// BOX pool: 21 slots of 0x80 bytes.
    pub const BOXES: u32 = 0xffeede;
}

pub(super) fn barrier_switch<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::HIDDEN) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let (px, py) = (tables().switch_positions.at16(x.b, 4 * v), tables().switch_positions.at16(x.b, 4 * v + 2));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.anim_start(switch::ANIM_SET.0, 8);
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                let barrier = x.linked(switch::BARRIER);
                if barrier.u8(x.b, BarrierView::OPEN) != 0 {
                    x.add8(f::SUB_STATE, 2);
                } else {
                    x.set8(switch::PRESSED, 0);
                    switch_watch(x);
                }
            }
        }
        4 => {}
        _ => free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert_foreground();
    }
}

/// A player on the ground, then a barrel (BOX 3 variant 1) on the ground, on the switch.
pub(super) fn switch_watch<R: Bench>(x: &mut Cx<'_, R>) {
    let hit_box = (
        tables().switch_hit_box.at16(x.b, 0),
        tables().switch_hit_box.at16(x.b, 2),
        tables().switch_hit_box.at16(x.b, 4),
        tables().switch_hit_box.at16(x.b, 6),
    );
    for p in players() {
        if p.is_active(x.b) && (p.u16(x.b, Obj::Z) as i16) <= 0 && in_box(x, p, hit_box) {
            x.set8(switch::PRESSED, 1);
            break;
        }
    }
    for k in 0..21 {
        let o = Obj(switch::BOXES + 0x80 * k);
        if rd8(x.b, o.0) != 0
            && o.u8(x.b, f::HIDDEN) == 0
            && (o.u16(x.b, Obj::Z) as i16) <= 0
            && o.u8(x.b, Obj::KIND) == 3
            && o.u8(x.b, Obj::VARIANT) == 1
            && in_box(x, o, hit_box)
        {
            return x.set8(switch::PRESSED, 1);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 88: barrier controlled by switches (type 87)
// ---------------------------------------------------------------------------------------------

pub(super) mod barrier {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c03a6);
    pub const OPEN_SOUND: crate::services::sound::Sound = crate::services::sound::CMD_34E_44E;
    /// Associated SET scenery (16-bit pointer).
    pub const SET: u32 = 0x46;
    /// Switches (up to four 16-bit pointers) and their number.
    pub const CHILDREN: u32 = 0x50;
    pub const COUNT: u32 = 0x58;
    pub const OPEN: u32 = 0x5a;
}

pub(super) fn barrier<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::HIDDEN) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => barrier_init(x),
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                if barrier_all_pressed(x) {
                    x.add8(f::SUB_STATE, 2);
                    x.set8(barrier::OPEN, 1);
                    crate::services::sound::play(x.b, barrier::OPEN_SOUND, x.o);
                    x.set16(Obj::VZ, 0x180);
                    x.set16(Obj::VX, 0x200);
                }
            } else {
                // rises while shaking, then disappears
                x.move_xz();
                let vx = x.u16(Obj::VX).wrapping_neg();
                x.set16(Obj::VX, vx);
                if (x.u16(Obj::Z) as i16) >= 0x80 {
                    let set = x.linked(barrier::SET);
                    set.set_u8(x.b, Effect::SETTING, 0);
                    free(x);
                }
            }
        }
        4 => {}
        _ => free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

pub(super) fn barrier_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let v = x.u8(f::VARIANT) as u32;
    // the scenery: the original retries until it finds a slot
    let set = loop {
        if let Some(o) = x.alloc(&pools::SET) {
            break o;
        }
    };
    wr8(x.b, set.0, 1);
    wr8(x.b, set.0 + 2, 7);
    wr8(x.b, set.0 + 0x2f, 1);
    let sv = tables().barrier_set_variants.at8(x.b, v);
    wr8(x.b, set.0 + 3, sv);
    x.set16(barrier::SET, set.0 as u16);
    // the switches
    let switches = &tables().barrier_switches;
    let list = 4 * v;
    let mut slot = barrier::CHILDREN;
    for k in 0..4 {
        if k > 0 && (switches.at8(x.b, list + k) as i8) < 0 {
            break;
        }
        let variant = switches.at8(x.b, list + k);
        if let Some(o) = x.alloc(&pools::EFFCT) {
            wr8(x.b, o.0, 1);
            wr8(x.b, o.0 + 2, 87);
            wr8(x.b, o.0 + 3, variant);
            wr16(x.b, o.0 + 0x42, x.o.0 as u16);
            x.set16(slot, o.0 as u16);
            slot += 2;
            x.add16(barrier::COUNT, 1);
        }
    }
    let anim = if matches!(v, 1 | 4) { 0x19 } else { 0x18 };
    x.anim_start(barrier::ANIM_SET.0, anim);
}

/// 0xba094: all the switches are pressed (bit 0 of each one's `+0x30`).
pub(super) fn barrier_all_pressed<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u16(barrier::COUNT);
    if n == 0 {
        return false;
    }
    let mut all = 1u8;
    for k in 0..n as u32 {
        let child = x.linked(barrier::CHILDREN + 2 * k);
        all &= child.u8(x.b, SwitchView::PRESSED);
    }
    all != 0
}

// ---------------------------------------------------------------------------------------------
// Type 67: column that rises under an object until the end of a scene count, then releases
// the players
// ---------------------------------------------------------------------------------------------

pub(super) fn pillar<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let base = x.linked(Pillar::BASE);
            let (bx, by) = (base.u16(x.b, Obj::X), base.u16(x.b, Obj::Y));
            x.set16(Obj::X, bx);
            x.set16(Obj::Y, by);
            base.set_u16(x.b, Carrier::Y_LOW, 0x4000);
            x.set16(Effect::Y_LOW, if v & 2 != 0 { 0x8000 } else { 0 });
            let top = if crate::world::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { crate::world::ram::CAMERA_ALT_Y.get(x.b) } else { crate::world::ram::CAMERA_Y.get(x.b) };
            x.set16(Obj::Z, top.wrapping_add(0x100).wrapping_sub(by));
            let delay = tables().pillar_delays.at8(x.b, v as u32);
            x.set8(Pillar::DELAY, delay);
            x.anim_start(0x1b67f2, (5u8.wrapping_add(v) as u32) as u16);
        }
        2 => {
            let delay = x.u8(Pillar::DELAY);
            if delay != 0 {
                x.set8(Pillar::DELAY, delay - 1);
            } else {
                let z = x.u16(Obj::Z);
                let rising = if v & 1 != 0 { z > 0x60 } else { (z as i16) >= 0 };
                if rising {
                    x.set16(Obj::Z, z.wrapping_sub(3));
                }
            }
            if rd16(x.b, 0xff2a3a) <= 0x40 {
                x.add8(f::STATE, 2);
                for k in 0..4 {
                    let p = crate::world::ram::PLAYERS.nth(k);
                    if p.is_active(x.b) && p.u16(x.b, Player::HP) != 0 {
                        p.set_u8(x.b, Player::ACTION, 0xe);
                    }
                }
            }
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 34: platform that advances and carries the players away, until its scenery stops it
// ---------------------------------------------------------------------------------------------

pub(super) fn ferry<R: Bench>(x: &mut Cx<'_, R>) {
    if x.suspended() {
        return x.set8(f::STATE, 4);
    }
    if vars::PAUSE.get(x.b) != 0 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(Ferry::SPEED, 0x1_0000);
            x.set16(Obj::Z, 0);
            vars::FROZEN.set(x.b, 0);
            x.anim_start(0x1c03a6, 0);
            x.reinsert();
        }
        2 if x.u8(f::SUB_STATE) == 0 => {
            let v = x.u32(Ferry::SPEED);
            x.add32(Obj::X, v);
            if crate::world::ram::PHASE.get(x.b) == 0xc {
                x.add8(f::SUB_STATE, 2);
                // Original glitch: the allocation failure is not tested, the link takes the A1 left behind.
                let scenery = scene_set_of(x, 0x63).unwrap_or_else(|| leftover(x, &pools::SET));
                x.set16(Ferry::SCENERY, scenery.0 as u16);
            }
        }
        2 => {
            let stop = x.linked(Ferry::SCENERY);
            if stop.u8(x.b, Carrier::HALTED) != 0 {
                return vars::FROZEN.set(x.b, 1);
            }
            let dx = x.u32(Ferry::SPEED);
            x.add32(Obj::X, dx);
            let (fx, fy) = (x.u16(Obj::X), x.u16(Obj::Y));
            for k in 0..4 {
                let p = crate::world::ram::PLAYERS.nth(k);
                if !p.is_active(x.b) {
                    continue;
                }
                p.set_u16(x.b, Obj::Y, fy.wrapping_sub(0x12));
                let px = p.u32(x.b, Obj::X).wrapping_add(dx);
                p.set_u32(x.b, Obj::X, px);
                let px = p.u16(x.b, Obj::X) as i16;
                let (right, left) = (fx.wrapping_add(0x48), fx.wrapping_sub(0x48));
                if (right as i16) < px {
                    p.set_u16(x.b, Obj::X, right);
                } else if (left as i16) > px {
                    p.set_u16(x.b, Obj::X, left);
                }
            }
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 10: slabs of a diagonal row, which sink when a player on the ground walks on them
// ---------------------------------------------------------------------------------------------

pub(super) mod tiles {
    /// Neighbors (16-bit pointers): previous `+0x30`, next `+0x32`; pressed `+0x34`.
    pub const PREVIOUS: u32 = 0x30;
    pub const NEXT: u32 = 0x32;
    pub const DOWN: u32 = 0x34;
}

/// Next slab (type 10, variant + 1), up to the ninth.
pub(super) fn floor_tile<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => {
            x.set8(tiles::DOWN, 0);
            x.set16(Obj::Y, 0x70);
            let px = tables().tiles_positions.at16(x.b, 2 * v as u32);
            x.set16(Obj::X, px);
            if (v as i8) < 9 {
                let Some(o) = x.alloc(&pools::EFFCT) else { return };
                x.set16(tiles::NEXT, o.0 as u16);
                let a = rd8(x.b, o.0).wrapping_add(1);
                wr8(x.b, o.0, a);
                wr8(x.b, o.0 + 2, 10);
                wr8(x.b, o.0 + 3, v.wrapping_add(1));
                wr16(x.b, o.0 + tiles::PREVIOUS, x.o.0 as u16);
            }
            x.add8(f::STATE, 2);
        }
        2 => floor_tile_watch(x, v),
        _ => free(x),
    }
}

pub(super) fn floor_tile_watch<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    // an already sunken neighbor blocks the slab
    if v != 0 {
        let prev = x.linked(tiles::PREVIOUS);
        if prev.u8(x.b, tiles::DOWN) != 0 {
            return;
        }
    }
    if v != 9 {
        let next = x.linked(tiles::NEXT);
        if next.u8(x.b, tiles::DOWN) != 0 {
            return;
        }
    }
    let tx = x.u16(Obj::X);
    let mut stepped = false;
    for k in 0..4 {
        let p = crate::world::ram::PLAYERS.nth(k);
        if !p.is_active(x.b) || p.u16(x.b, Obj::Z) != 0 {
            continue;
        }
        // the row is diagonal: the x offset follows the depth
        let d = p.u16(x.b, Obj::Y).wrapping_sub(0x1a8);
        if d > 0x27 {
            continue;
        }
        let px = p.u16(x.b, Obj::X) as i16;
        let left = tx.wrapping_sub(d).wrapping_sub(8);
        if (left as i16) > px || (left.wrapping_add(0x10) as i16) <= px {
            continue;
        }
        stepped = true;
        break;
    }
    let down = x.u8(tiles::DOWN) != 0;
    if stepped == down {
        return;
    }
    let Some(o) = x.alloc(&pools::SET) else { return };
    let a = rd8(x.b, o.0).wrapping_add(1);
    wr8(x.b, o.0, a);
    wr8(x.b, o.0 + 2, 6);
    let table = if stepped { &tables().tiles_pressed } else { &tables().tiles_released };
    let variant = table.at8(x.b, v as u32);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, 0);
    if stepped {
        x.add8(tiles::DOWN, 1);
    } else {
        x.set8(tiles::DOWN, 0);
    }
}

