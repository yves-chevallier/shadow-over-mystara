use super::*;

// ---------------------------------------------------------------------------------------------
// Type 63: chunk thrown by an enemy
// ---------------------------------------------------------------------------------------------

pub(super) mod chunk {
    /// Creator (ENEMY 22 or 23).
    pub const CREATOR: u32 = 0x42;
    pub const TIMER: u32 = 0x58;
    /// Animation sets of variants 0 and 1 (variant 2 reuses that of 1).
    pub const ANIMS_0: crate::world::data::RomAddr = crate::world::data::RomAddr(0x16abaa);
    pub const ANIMS_1: crate::world::data::RomAddr = crate::world::data::RomAddr(0x16b762);
    /// Kind of debris (`0x1172e`) at impact.
    pub const DEBRIS_KIND: u32 = 0xc;
}

/// Start (variants 0 and 1): placed on the creator, 0x37 higher, thrown forward.
pub(super) struct Throw {
    vz: u16,
    az: u16,
    dx: u16,
    vx: u16,
    anims: u32,
}

pub(super) const THROWS: [Throw; 2] = [
    Throw { vz: 0x4c0, az: 0xffd0, dx: 0xffa0, vx: 0x200, anims: chunk::ANIMS_0.0 },
    Throw { vz: 0x500, az: 0xffd8, dx: 0xffc8, vx: 0x280, anims: chunk::ANIMS_1.0 },
];

pub(super) fn chunk<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => chunk_init(x),
        2 => chunk_active(x),
        _ => free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.keep_in_zone();
        x.push_terrain();
        x.reinsert();
    }
}

pub(super) fn chunk_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let variant = x.u8(f::VARIANT);
    let Some(throw) = THROWS.get(variant as usize) else {
        // variant 2: no throw, only the animation
        x.anim_start(chunk::ANIMS_1.0, 0x14);
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

pub(super) fn chunk_active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            x.move_xz();
            if x.u16(Obj::Z) as i16 > 0 {
                x.anim_step();
                return;
            }
            // on the ground: sound, debris, then the impact frame
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::Z, 0);
            x.set16(chunk::TIMER, 0x5a);
            crate::hw::sound::play(x.b, rom::SOUND_LANDING, x.o);
            crate::services::ai_aids::debris_set(x.b, x.o, chunk::DEBRIS_KIND as u16);
            // Original glitch: the exhausted `dbf` counter stays in D5 (`dispatch::Leftovers`)
            *x.c.d5() = 0xffff;
            match x.u8(f::VARIANT) {
                0 => {
                x.anim_start(chunk::ANIMS_0.0, 0x15);
            }
                1 => {
                x.anim_start(chunk::ANIMS_1.0, 0x15);
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
            // blinks until the end of the delay
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
// Type 81: spark thrown by a character
// ---------------------------------------------------------------------------------------------

pub(super) mod shard {
    pub const OWNER: u32 = 0x50;
    pub const TIMER: u32 = 0x58;
    pub const GESTURE: u32 = 0x42;
    pub const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x19844e);
}

pub(super) fn shard<R: Bench>(x: &mut Cx<'_, R>) {
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

pub(super) fn shard_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(shard::TIMER, 0x20);
    x.set16(Obj::VX, 0xff80);
    x.set16(Obj::VZ, 0x300);
    x.set16(Obj::AZ, 0xffc8);
    let owner = x.linked(shard::OWNER);
    let gesture = (owner.u16(x.b, shard::GESTURE) << 3) as i16 as i32 as u32;
    let hand = &tables().hand_offsets;
    let mut dx = hand.at16(x.b, gesture.wrapping_add(2));
    let dz = hand.at16(x.b, gesture.wrapping_add(4));
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
    x.anim_start(shard::ANIMS.0, anim as u16);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// Type 43: rain of debris
// ---------------------------------------------------------------------------------------------

pub(super) mod rain {
    /// Followed object: the effect disappears when it is released or when it goes to state 4 or more.
    pub const LINK: u32 = 0x5c;
    pub const SETTING: u32 = 0x2f;
    pub const TIMER: u32 = 0x42;
    /// Reference depth, taken again every other frame.
    pub const SAVED_Y: u32 = 0x1c;
    pub const SPEED_INDEX: u32 = 0x2d;
    pub const ANIMS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x194c74);
}

pub(super) fn rng<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.random()
}

pub(super) fn debris_rain<R: Bench>(x: &mut Cx<'_, R>) {
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

pub(super) fn rain_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    if x.u8(rain::SETTING) == 0 {
        // rain: at the top right of the screen, a dust cloud at the start
        let px = x.u16(Obj::X).wrapping_add(0x1a0).wrapping_add(crate::world::ram::CAMERA_X.get(x.b));
        let py = x.u16(Obj::Y).wrapping_add(0xa0).wrapping_add(crate::world::ram::CAMERA_Y.get(x.b));
        x.set16(Obj::X, px);
        x.set16(Obj::Z, py);
        let y = x.u16(Obj::Y);
        x.set16(rain::SAVED_Y, y);
        x.set16(Obj::AX, 0xffe8);
        let o = x.o;
        crate::services::effects_aids::shock(x.b, o, 5, 0x201, 0, 0);
        x.set16(rain::TIMER, 0x26);
        let variant = x.u8(f::VARIANT) as u32;
        let speed = tables().rain_speed_by_variant.at8(x.b, variant);
        x.set8(rain::SPEED_INDEX, speed);
        let t = tables();
        rain_launch(x, &t.rain_camera_vx, &t.rain_camera_vz, &t.rain_camera_anims, 0x1f);
    } else {
        // spark leaving the creator
        x.add8(f::SUB_STATE, 4);
        let from = x.linked(rain::LINK);
        for off in [Obj::X, Obj::Z, Obj::Y] {
            let v = from.u16(x.b, off);
            x.set16(off, v);
        }
        x.set16(Obj::AZ, 0xffd8);
        x.set16(rain::TIMER, 0x14);
        let pick = (rng(x) & 0xf) as u32;
        let speed = tables().rain_shard_speed.at8(x.b, pick);
        x.set8(rain::SPEED_INDEX, speed);
        let t = tables();
        rain_launch(x, &t.rain_shard_vx, &t.rain_shard_vz, &t.rain_shard_anims, 0xf);
    }
}

/// X and Z speeds and animation drawn from the tables (words, words, bytes; `anim_mask` bounds the last draw).
pub(super) fn rain_launch<R: Bench>(x: &mut Cx<'_, R>, vx_table: &Table, vz_table: &Table, anims: &Table, anim_mask: u16) {
    let pick = 2 * (rng(x) & 0xf) as u32;
    let vx = vx_table.at16(x.b, pick);
    x.set16(Obj::VX, vx);
    let pick = 2 * (rng(x) & 0xf) as u32;
    let vz = vz_table.at16(x.b, pick);
    x.set16(Obj::VZ, vz);
    let pick = (rng(x) & anim_mask) as u32;
    let anim = anims.at8(x.b, pick);
    x.anim_start(rain::ANIMS.0, (anim as u32) as u16);
}

/// Countdown of `TIMER`; true when it drops below zero.
pub(super) fn rain_countdown<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let t = x.u16(rain::TIMER).wrapping_sub(1);
    x.set16(rain::TIMER, t);
    (t as i16) < 0
}

/// End of life: removal once the delay has elapsed, blinking three frames out of four before.
pub(super) fn rain_fade<R: Bench>(x: &mut Cx<'_, R>) {
    if rain_countdown(x) {
        return x.set8(f::STATE, 6);
    }
    let t = x.u16(rain::TIMER);
    set_hidden(x, t & 3 != 0);
}

pub(super) fn rain_active<R: Bench>(x: &mut Cx<'_, R>) {
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
            // every other frame, the rain takes its reference depth again (the draw is lost)
            rng(x);
            if vars::FRAME_LOW.get(x.b) & 1 == 0 {
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

