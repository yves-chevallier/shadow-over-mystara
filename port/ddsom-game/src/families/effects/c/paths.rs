use super::*;

// ---------------------------------------------------------------------------------------------
// Type 12: link of a chain, which replays the previous link's directions with a delay
// ---------------------------------------------------------------------------------------------

pub(super) mod link {
    pub const ANIM_SET: u32 = 0x15cdb6;
    /// Delay (index into the previous one's history) per rank `+0x2f` (bytes, program space).
    pub const DELAYS: u32 = 0xad536;
    pub const PREVIOUS: u32 = 0x5c;
    /// The head of the chain: its byte `+0x58` is 0x80 (end) or 0xfe (break), `+0x42` counts the
    /// maillons.
    pub const HEAD: u32 = 0x46;
    /// History of the directions (4 bytes, most recent first).
    pub const HISTORY: u32 = 0x58;
    /// Break: 16-byte sheets per type of the head's carrier (data).
    pub const BREAKS: u32 = 0xad5e2;
}

pub(super) fn chain_link<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let prev = x.linked(link::PREVIOUS);
            for off in [Obj::X, Obj::Z, Obj::Y, Obj::SPEED_TABLE] {
                let v = prev.u32(x.b, off);
                x.set32(off, v);
            }
            let facing = prev.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            x.set32(link::HISTORY, 0xffff_ffff);
            let anim = 1u8.wrapping_add(x.u8(0x2f));
            x.anim_start(link::ANIM_SET, (anim as u32) as u16);
            x.reinsert();
        }
        2 if x.u8(f::SUB_STATE) != 0 => {
            if x.anim_step() {
                free(x);
            }
        }
        2 => chain_link_follow(x),
        4 => {
            // Original glitch: A1 is the handler's address (0xad476) left by the dispatch (write into
            // ROM, no effect)
            let a1 = 0xad476;
            let n = rd8(x.b, a1 + 0x42).wrapping_sub(1);
            wr8(x.b, a1 + 0x42, n);
            free(x);
        }
        _ => free(x),
    }
}

/// `0xad53a`: the chain breaks. Each link (rank `+0x2f` decremented) plays its break animation;
/// the last one disappears. A sheet per type of the head's carrier (`+0x54` of the head; 0x20 for a carrier
/// that is not an ENEMY) says whether the link disappears (negative byte), stays in place (positive) or lands
/// on the carrier (zero). The original always re-reads the sheet's first entry (its index is reset to zero).
pub(super) fn chain_break<R: Bench>(x: &mut Cx<'_, R>, head: Obj) {
    x.add8(f::SUB_STATE, 2);
    let n = head.u8(x.b, 0x42).wrapping_sub(1);
    head.set_u8(x.b, 0x42, n);
    let rank = x.u8(0x2f).wrapping_sub(1);
    x.set8(0x2f, rank);
    if (rank as i8) < 0 {
        x.add8(f::STATE, 2);
        return free(x);
    }
    let p = head.u16(x.b, 0x54);
    let carrier = Obj(p as i16 as i32 as u32 & 0xff_ffff);
    let row = if carrier.u8(x.b, Obj::CLASS) == 2 { carrier.u8(x.b, Obj::KIND) as u16 } else { 0x20 };
    let sheet = link::BREAKS.wrapping_add((row << 4) as i16 as i32 as u32);
    let what = x.data8(sheet) as i8;
    if what < 0 {
        return free(x);
    }
    if what == 0 {
        let (cx_, cz, cy) = (carrier.u16(x.b, Obj::X), carrier.u16(x.b, Obj::Z), carrier.u32(x.b, Obj::Y));
        x.set16(Obj::X, cx_);
        x.set16(Obj::Z, cz);
        x.set32(Obj::Y, cy.wrapping_sub(1));
        let mut d = x.data16(sheet + 4 + 2);
        x.add16(Obj::Z, d);
        let facing = carrier.u8(x.b, 0x2c);
        x.set8(0x2c, facing);
        if facing != 0 {
            d = d.wrapping_neg();
        }
        x.add16(Obj::X, d);
        x.set8(0x2c, facing ^ 0x20);
    }
    let anim = 5u8.wrapping_add(x.u8(0x2f));
    x.anim_start(link::ANIM_SET, (anim as u32) as u16);
}

pub(super) fn chain_link_follow<R: Bench>(x: &mut Cx<'_, R>) {
    let head = x.linked(link::HEAD);
    match head.u8(x.b, 0x58) {
        0x80 => {
            let n = head.u8(x.b, 0x42).wrapping_sub(1);
            head.set_u8(x.b, 0x42, n);
            return free(x);
        }
        0xfe => return chain_break(x, head),
        _ => {}
    }
    for k in (0..3).rev() {
        let b = x.u8(link::HISTORY + k);
        x.set8(link::HISTORY + k + 1, b);
    }
    let prev = x.linked(link::PREVIOUS);
    let rank = x.u8(0x2f) as u32;
    let delay = x.rom8(link::DELAYS + rank) as u32;
    let dir = prev.u8(x.b, link::HISTORY + delay);
    x.set8(Obj::SPEED_INDEX, dir);
    x.set8(link::HISTORY, dir);
    if (dir as i8) >= 0 {
        // bits 5 and 6: jump one level up or down
        if dir & 0x20 != 0 {
            x.add32(Obj::Z, 0x3_0000);
            x.set8(Obj::SPEED_INDEX, dir & 0x1f);
        } else if dir & 0x40 != 0 {
            x.add32(Obj::Z, 0x3_0000u32.wrapping_neg());
            x.set8(Obj::SPEED_INDEX, dir & 0x1f);
        }
        x.move_by_table_xy();
    }
    x.anim_step();
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 65: object that follows a path (directions and durations read from a list), then spawns
// another object described by a sheet
// ---------------------------------------------------------------------------------------------

pub(super) mod courier {
    pub const ANIM_SET: u32 = 0x197afe;
    /// Path: pairs (direction, duration) ended by a negative direction (data).
    pub const PATH: u32 = 0xb7b1e;
    /// 8-byte sheets per variant: animation, type, variant, mode, x, z (data).
    pub const CARDS: u32 = 0xb7bec;
}

pub(super) fn courier<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set32(0x5c, courier::PATH);
            x.set8(0x42, 1);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x15);
            let card = courier::CARDS + 8 * x.u8(f::VARIANT) as u32;
            let anim = x.data8(card) as u32;
            x.anim_start(courier::ANIM_SET, anim as u16);
            x.reinsert_foreground();
        }
        2 => {
            x.move_by_table_xy();
            x.anim_step();
            let t = x.u8(0x42).wrapping_sub(1);
            x.set8(0x42, t);
            if t != 0 {
                return;
            }
            let p = x.u32(0x5c);
            let dir = x.data8(p);
            x.set8(Obj::SPEED_INDEX, dir);
            if (dir as i8) < 0 {
                x.add8(f::STATE, 2);
                x.set16(0x42, 0x20);
                return x.unlink();
            }
            let len = x.data8(p + 1);
            x.set8(0x42, len);
            x.set32(0x5c, p + 2);
        }
        4 => {
            if x.u8(f::SUB_STATE) == 0 {
                let t = x.u16(0x42).wrapping_sub(1);
                x.set16(0x42, t);
                if (t as i16) < 0 {
                    x.add8(f::SUB_STATE, 1);
                }
                return;
            }
            let Some(Obj(o)) = x.alloc(&pools::ENEMY) else { return };
            x.add8(f::STATE, 2);
            let card = courier::CARDS + 8 * x.u8(f::VARIANT) as u32;
            let link = x.u16(0x46);
            wr16(x.b, o + 0x46, link);
            wr8(x.b, o, 1);
            for (k, off) in [(1, 2), (2, 3), (3, 0x2f)] {
                let v = x.data8(card + k);
                wr8(x.b, o + off, v);
            }
            wr8(x.b, o + 0x2c, 0x20);
            let cam = crate::ram::CAMERA_X.get(x.b);
            let (px, pz) = (x.data16(card + 4), x.data16(card + 6));
            wr16(x.b, o + Obj::X, px.wrapping_add(cam));
            wr16(x.b, o + Obj::Z, pz);
            let r = random_nibble16(x) & 0x1f;
            wr16(x.b, o + Obj::Y, r + 0x50);
        }
        _ => free(x),
    }
}

