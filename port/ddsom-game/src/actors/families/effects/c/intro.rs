use super::*;

// ---------------------------------------------------------------------------------------------
// Type 36: floating object of an introduction scene (three variants)
// ---------------------------------------------------------------------------------------------

pub(super) mod floater {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x194062);
    /// Step of the scene sequence (`A5-0x55c8`).
    pub const STEP: u32 = 0xff2a38;
    pub const TIMER: u32 = 0x54;
    /// Undulation: period (frames per step, word) and index into the speed table (`+0x2d`).
    pub const WAVE_TICK: u32 = 0x56;
    /// Countdown of the screen shake (`A5+0x4ba`).
    pub const SHAKE: u32 = 0x58;
}

pub(super) fn floater<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let anim = match x.u8(f::VARIANT) {
                0 => 0,
                1 => 2,
                _ => {
                    x.set16(Obj::X, 0x120);
                    x.set16(Obj::Y, 0x198);
                    x.set16(Obj::Z, 8);
                    1
                }
            };
            x.anim_start(floater::ANIM_SET.0, anim);
            x.reinsert();
        }
        2 => match x.u8(f::VARIANT) {
            0 => floater_main(x),
            1 => {
                // reflection stuck under its creator
                if vars::FROZEN.get(x.b) != 0 {
                    x.add8(f::STATE, 2);
                }
                let creator = x.linked(Floater::CREATOR);
                let z = creator.u16(x.b, Obj::Z).wrapping_add(0x3e);
                x.set16(Obj::Z, z);
                x.anim_step();
            }
            _ => floater_rise(x),
        },
        _ => free(x),
    }
}

/// Vertical undulation: the Z speed follows the current speed table, one step every four frames.
pub(super) fn floater_wave<R: Bench>(x: &mut Cx<'_, R>) {
    let k = x.u8(Obj::SPEED_INDEX) as u32;
    let table = x.u32(Obj::SPEED_TABLE);
    let v = speed_word(x.b, table, 4 * k + 2);
    x.set16(Obj::VZ, v);
    let dz = ((v as u32) << 16) as i32 >> 8;
    x.add32(Obj::Z, dz as u32);
    let t = x.u16(floater::WAVE_TICK).wrapping_sub(1) & 3;
    x.set16(floater::WAVE_TICK, t);
    if t == 0 {
        x.set16(floater::WAVE_TICK, 4);
        let k = (x.u8(Obj::SPEED_INDEX) + 1) & 0x1f;
        x.set8(Obj::SPEED_INDEX, k);
    }
    x.anim_step();
}

pub(super) fn floater_start_wave<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(Obj::VZ, 0x100_0000);
    x.set16(floater::WAVE_TICK, 4);
    x.set8(Obj::SPEED_INDEX, 0);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, 1);
}

pub(super) fn floater_main<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            if vars::SUB_STEP.get(x.b) == 6 {
                x.add8(f::SUB_STATE, 2);
                x.set16(floater::TIMER, 0xc);
                x.anim_start(floater::ANIM_SET.0, 1);
            } else {
                x.anim_step();
            }
        }
        2 => {
            x.add16(Obj::Z, 1);
            let t = x.u16(floater::TIMER).wrapping_sub(1);
            x.set16(floater::TIMER, t);
            if t != 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            floater_start_wave(x);
            // 0xb337e: the reflection (type 36 variant 1)
            if let Some(o) = x.alloc(&pools::EFFCT) {
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 36);
                wr8(x.b, o.0 + 3, 1);
                for field in [Obj::X, Obj::Z, Obj::Y] {
                    let v = x.u16(field);
                    wr16(x.b, o.0 + field, v);
                }
                let z = rd16(x.b, o.0 + Obj::Z).wrapping_add(0x3e);
                wr16(x.b, o.0 + Obj::Z, z);
                x.set16(Effect::Y_LOW, 0x8000);
                wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
            }
        }
        4 => {
            if rd8(x.b, floater::STEP) == 0x18 {
                x.add8(f::SUB_STATE, 2);
                x.set16(floater::TIMER, 0x80);
                x.anim_start(floater::ANIM_SET.0, 3);
            } else {
                floater_wave(x);
            }
        }
        6 => {
            let t = x.u16(floater::TIMER).wrapping_sub(1);
            x.set16(floater::TIMER, t);
            if t == 0 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(floater::ANIM_SET.0, 1);
                return;
            }
            if x.u8(floater::SHAKE) != 0 {
                let s = x.u8(floater::SHAKE) - 1;
                x.set8(floater::SHAKE, s);
                if s == 0 {
                    crate::world::ram::SCROLL_Y_PLANE1.set(x.b, 0);
                }
            }
            if x.u8(Effect::ANIM_EVENT) != 0 {
                // hit: screen shake
                x.set8(Effect::ANIM_EVENT, 0);
                crate::world::placement::load_effect_palette(x.b, crate::world::placement::effect_palette::LIGHT);
                crate::actors::families::effects_creators::pillars(x.b);
                crate::hw::sound::play(x.b, crate::hw::sound::CMD_357, x.o);
                crate::world::ram::SCROLL_Y_PLANE1.set(x.b, 0x100);
                x.set8(floater::SHAKE, 8);
                crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
            }
            x.anim_step();
        }
        8 => {
            if rd8(x.b, floater::STEP) == 0x20 {
                x.add8(f::SUB_STATE, 2);
                x.set16(floater::TIMER, 0xf0);
            }
            floater_wave(x);
        }
        10 => fade_out(x, floater::TIMER),
        _ => {}
    }
}

/// Disappearance: blinks faster and faster during the countdown `timer`, then goes to
/// release.
pub(super) fn fade_out<R: Bench>(x: &mut Cx<'_, R>, timer: u32) {
    let t = x.u16(timer).wrapping_sub(1);
    x.set16(timer, t);
    if t == 0 {
        return x.add8(f::STATE, 2);
    }
    let mask = if t as i16 > 0x80 {
        7
    } else if t as i16 > 0x30 {
        3
    } else {
        1
    };
    set_hidden(x, t & mask == 0);
}

pub(super) fn floater_rise<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            x.add8(f::SUB_STATE, 2);
            floater_start_wave(x);
            x.anim_step();
        }
        2 => {
            if vars::SUB_STEP.get(x.b) == 0xc {
                x.add8(f::SUB_STATE, 2);
            }
            floater_wave(x);
        }
        4 => {
            let hidden = vars::FRAME_BYTE.get(x.b) & 1 != 0;
            set_hidden(x, hidden);
            x.add16(Obj::Z, 4);
            if x.u16(Obj::Z) > 0x70 {
                x.add8(f::STATE, 2);
            }
            x.anim_step();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 58: machine of the introduction scene (variant 0) and its reflection (variant 1)
// ---------------------------------------------------------------------------------------------

pub(super) mod machine {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x196baa);
    pub const TIMER: u32 = 0x54;
}

pub(super) fn machine<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let anim = if x.u8(f::VARIANT) != 0 { 3 } else { 0 };
            x.anim_start(machine::ANIM_SET.0, anim);
            x.reinsert();
        }
        2 if x.u8(f::VARIANT) != 0 => {
            if vars::FROZEN.get(x.b) != 0 {
                x.add8(f::STATE, 2);
            }
            x.anim_step();
        }
        2 => machine_active(x),
        _ => free(x),
    }
}

pub(super) fn machine_active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            if vars::SUB_STEP.get(x.b) == 6 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(machine::ANIM_SET.0, 2);
            } else {
                x.anim_step();
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(machine::TIMER, 0x40);
                x.anim_start(machine::ANIM_SET.0, 0);
                // a BOX 0x12 object; the original does not test the allocation failure
                // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
                let Obj(o) = x.alloc_unchecked(&pools::BOSS);
                wr8(x.b, o, 1);
                wr8(x.b, o + 2, 0x12);
            }
        }
        4 => {
            x.anim_step();
            let t = x.u16(machine::TIMER).wrapping_sub(1);
            x.set16(machine::TIMER, t);
            if t == 0 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(machine::ANIM_SET.0, 1);
            }
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(machine::ANIM_SET.0, 0);
                // 0xb6eb0: the reflection (type 58 variant 1)
                if let Some(o) = x.alloc(&pools::EFFCT) {
                    wr8(x.b, o.0, 1);
                    wr8(x.b, o.0 + 2, 58);
                    wr8(x.b, o.0 + 3, 1);
                    for field in [Obj::X, Obj::Z, Obj::Y] {
                        let v = x.u16(field);
                        wr16(x.b, o.0 + field, v);
                    }
                    let px = rd16(x.b, o.0 + Obj::X).wrapping_sub(6);
                    wr16(x.b, o.0 + Obj::X, px);
                    let pz = rd16(x.b, o.0 + Obj::Z).wrapping_add(0x1c);
                    wr16(x.b, o.0 + Obj::Z, pz);
                    x.set16(Effect::Y_LOW, 0x8000);
                    wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
                }
            } else if x.u8(Effect::ANIM_EVENT) != 0 {
                x.set8(Effect::ANIM_EVENT, 0);
                x.set16(Obj::X, 0x130);
                x.set16(Obj::Y, 0x180);
            }
        }
        8 => {
            if rd8(x.b, floater::STEP) == 0x20 {
                x.add8(f::SUB_STATE, 2);
                x.set16(machine::TIMER, 0xf0);
            }
            x.anim_step();
        }
        10 => fade_out(x, machine::TIMER),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 30: fish of the introduction scene (variant 0: school that crosses; others: fish
// that swims, jumps when the screen shakes)
// ---------------------------------------------------------------------------------------------

pub(super) mod fish {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x190a20);
    pub const TIMER: u32 = 0x58;
}

/// Randomly drawn swimming speed (0x100 or 0x180), to the left if `left`.
pub(super) fn fish_speed<R: Bench>(x: &mut Cx<'_, R>, left: bool) {
    let r = random_nibble16(x);
    let v: u16 = if r & 1 != 0 { 0x180 } else { 0x100 };
    x.set16(Obj::VX, if left { v.wrapping_neg() } else { v });
}

pub(super) fn fish<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 if v == 0 => {
            let t = x.u16(fish::TIMER).wrapping_sub(1);
            x.set16(fish::TIMER, t);
            if t != 0 {
                return;
            }
            x.add8(f::STATE, 2);
            x.set16(fish::TIMER, 4);
            x.set8(Obj::SPEED_INDEX, 0);
            x.select_speed(&crate::world::objects::motion::SPEEDS_B, 1);
            x.anim_start(fish::ANIM_SET.0, 6);
            x.reinsert();
        }
        0 => {
            x.add8(f::STATE, 2);
            x.set16(fish::TIMER, 0xa0);
            let left = x.u8(Effect::FACING) == 0;
            fish_speed(x, left);
            x.anim_start(fish::ANIM_SET.0, 1);
            x.reinsert();
        }
        2 if v == 0 => {
            x.add16(Obj::X, 2);
            // undulation in depth, like type 36 but without a noted speed
            let k = x.u8(Obj::SPEED_INDEX) as u32;
            let table = x.u32(Obj::SPEED_TABLE);
            let w = speed_word(x.b, table, 4 * k + 2);
            x.add32(Obj::Y, (((w as u32) << 16) as i32 >> 8) as u32);
            let t = x.u16(fish::TIMER).wrapping_sub(1) & 3;
            x.set16(fish::TIMER, t);
            if t == 0 {
                x.set16(fish::TIMER, 4);
                let k = (x.u8(Obj::SPEED_INDEX) + 1) & 0x1f;
                x.set8(Obj::SPEED_INDEX, k);
            }
            if (crate::world::ram::CAMERA_X.get(x.b).wrapping_add(0x1c0) as i16) <= (x.u16(Obj::X) as i16) {
                x.add8(f::STATE, 2);
            }
            x.anim_step();
        }
        2 => fish_swim(x),
        4 => x.add8(f::STATE, 2),
        _ => free(x),
    }
}

pub(super) fn fish_swim<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            let t = x.u16(fish::TIMER).wrapping_sub(1);
            x.set16(fish::TIMER, t);
            if t == 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(fish::TIMER, 0x60);
                x.anim_start(fish::ANIM_SET.0, 0);
                return;
            }
            x.move_x();
            // U-turn at the edge of the alternate camera
            let cam = crate::world::ram::CAMERA_ALT_X.get(x.b);
            let px = x.u16(Obj::X);
            let turn = if (x.u16(Obj::VX) as i16) < 0 {
                (cam.wrapping_add(0x40).wrapping_sub(px) as i16) >= 0
            } else {
                (cam.wrapping_add(0x140).wrapping_sub(px) as i16) < 0
            };
            if turn {
                let r = random_nibble16(x);
                let v: u16 = if r & 1 != 0 { 0x180 } else { 0x100 };
                let facing = x.u8(Effect::FACING) ^ 0x20;
                x.set8(Effect::FACING, facing);
                x.set16(Obj::VX, if facing == 0 { v.wrapping_neg() } else { v });
            }
            x.anim_step();
        }
        2 => {
            if vars::SUB_STEP.get(x.b) == 6 {
                x.add8(f::SUB_STATE, 2);
                x.set8(Effect::FACING, 0x20);
                x.anim_start(fish::ANIM_SET.0, 2);
            }
        }
        4 => {
            if vars::SUB_STEP.get(x.b) == 8 {
                x.add8(f::SUB_STATE, 2);
                x.anim_start(fish::ANIM_SET.0, 0);
            }
        }
        6 => {
            if crate::world::ram::SCROLL_Y_PLANE1.get(x.b) == 0 {
                return;
            }
            // the screen shakes: jump
            x.add8(f::SUB_STATE, 2);
            if random_nibble16(x) & 1 != 0 {
                let facing = x.u8(Effect::FACING) ^ 0x20;
                x.set8(Effect::FACING, facing);
            }
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, 0xffc8);
            let vx: u16 = if x.u8(Effect::FACING) != 0 { 0x200u16.wrapping_neg() } else { 0x200 };
            x.set16(Obj::VX, vx);
            x.anim_start(fish::ANIM_SET.0, 4);
        }
        8 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) < 0 && (x.u16(Obj::Z) as i16) <= -0x10 {
                x.add8(f::SUB_STATE, 2);
                x.set32(Obj::Z, 0);
                x.set16(fish::TIMER, 0x60);
                x.anim_start(fish::ANIM_SET.0, 5);
            }
        }
        10 => {
            let t = x.u16(fish::TIMER).wrapping_sub(1);
            x.set16(fish::TIMER, t);
            if t == 0 {
                x.add8(f::STATE, 2);
            }
            let mask = if t > 0x30 { 3 } else { 1 };
            set_hidden(x, t & mask == 0);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 61: creature of the introduction scene, which comes out of the water, swims undulating then leaves
// ---------------------------------------------------------------------------------------------

pub(super) mod swimmer {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x197768);
    pub const TIMER: u32 = 0x42;
}

pub(super) fn swimmer<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let side = random_nibble16(x) as u8 & 1;
            x.set8(f::VARIANT, side);
            let wave = random_nibble16(x) as u8 & 1;
            x.set8(Effect::SETTING, wave);
            // 0xb7792: its wake (type 62)
            if let Some(o) = x.alloc(&pools::EFFCT) {
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 62);
                wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
            }
            // two SET 0x17 scenery objects (variants 0 and 1)
            for (variant, flip) in [(0u8, 1u8), (1, 0)] {
                let Some(o) = x.alloc(&pools::SET) else { crate::actors::families::effects::address_error(0xb778c) };
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 0x17);
                wr8(x.b, o.0 + 3, variant);
                wr8(x.b, o.0 + 0x2f, side ^ flip);
            }
            swimmer_active(x);
        }
        2 => swimmer_active(x),
        _ => return free(x),
    }
    x.reinsert();
}

pub(super) fn swimmer_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(swimmer::ANIM_SET.0, n as u16);
    x.anim_step();
}

/// Steps by 4; a completed step chains the next in the same frame.
pub(super) fn swimmer_active<R: Bench>(x: &mut Cx<'_, R>) {
    let mut right = x.u8(f::VARIANT) != 0;
    let mut sub = x.u8(f::SUB_STATE);
    loop {
        match sub {
            0 => {
                x.add8(f::SUB_STATE, 4);
                x.set16(Obj::X, 399);
                x.set16(Obj::Y, 312);
                x.set8(Effect::OWN_PALETTE, 1);
                x.set8(Effect::PALETTE, 0x1b);
                for k in 0..8 {
                    let v = crate::services::palette::library_excerpt!("swimmer_palette", 0x253440, 0x253460, 4).nth32(x.b, k);
                    let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
                    wr32(x.b, 0x91c360 + 4 * k, v);
                }
                swimmer_anim(x, 4);
            }
            4 => {
                if (crate::world::ram::CAMERA_X.get(x.b) as i16) < 0x10 || !x.anim_step() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                swimmer_anim(x, 2);
            }
            8 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                x.set16(swimmer::TIMER, 0x42);
                swimmer_anim(x, 3);
            }
            12 => {
                // undulation: rise (variant 0) or descent, direction inverted by `+0x2f`
                let (base, wave): (u32, u32) = if right { (0x2000, 0x4000u32.wrapping_neg()) } else { (0x2000u32.wrapping_neg(), 0x4000) };
                x.add32(Obj::Y, base);
                if x.u8(Effect::SETTING) != 0 {
                    x.add32(Obj::Y, wave);
                }
                x.anim_step();
                let t = x.u16(swimmer::TIMER).wrapping_sub(1);
                x.set16(swimmer::TIMER, t);
                if (t as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                swimmer_anim(x, 1);
                if !right {
                    return;
                }
            }
            16 => {
                if !x.anim_step() {
                    return;
                }
                x.add8(f::SUB_STATE, 4);
                let facing = x.u8(Effect::FACING) ^ 0x20;
                x.set8(Effect::FACING, facing);
                swimmer_anim(x, 0);
            }
            20 => {
                x.add16(Obj::X, 4);
                x.anim_step();
                // 0x3403c (collision with the terrain, second form), still on the processor
                let o = x.o;
                let [d2, d3, d4, d5] = [*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()];
                let mut leftovers = [d2, d3, d4, d5];
                let [r2, r3, r4, r5] = &mut leftovers;
                crate::world::terrain::push_second_leftovers(x.b, o, [r2, r3, r4, r5]);
                (*x.c.d2(), *x.c.d3(), *x.c.d4(), *x.c.d5()) = (leftovers[0], leftovers[1], leftovers[2], leftovers[3]);
                let gap = x.u16(Obj::X).wrapping_sub(crate::world::ram::CAMERA_X.get(x.b));
                if (gap as i16) > 0x1a0 {
                    free(x);
                }
                return;
            }
            _ => return,
        }
        // completed step 8 always chains the code of step 12 of variant 0
        right &= sub != 8;
        sub += 4;
    }
}

// ---------------------------------------------------------------------------------------------
// Type 59: creature of a scene that springs up, rises with the camera, throws its projectiles then
// leaps off the screen (sixteen steps)
// ---------------------------------------------------------------------------------------------

pub(super) mod riser {
    pub const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x197208);
    pub const TIMER: u32 = 0x56;
}

pub(super) fn riser_anim<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.add8(f::SUB_STATE, 2);
    x.anim_start(riser::ANIM_SET.0, n as u16);
}

/// Rises 3 points per frame and drags the camera along (`A5+0x50a`, 16.16).
pub(super) fn riser_climb<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(Obj::Z, 3u16.wrapping_neg());
    let cam = vars::CAMERA_Y_LONG.get(x.b).wrapping_sub(0x2_1000);
    vars::CAMERA_Y_LONG.set(x.b, cam);
}

/// A projectile (type 50, variant `variant`) leaves the creator when it is hit (`+0x33`).
pub(super) fn riser_shoot<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    if x.u8(Effect::ANIM_EVENT) == 0 {
        return;
    }
    x.add8(f::SUB_STATE, 2);
    x.set8(Effect::ANIM_EVENT, 0);
    let Some(o) = x.alloc(&pools::EFFCT) else { return };
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 50);
    wr8(x.b, o.0 + 3, variant);
    if variant == 2 {
        wr16(x.b, o.0 + 0x42, x.o.0 as u16);
        let facing = x.u8(Effect::FACING);
        wr8(x.b, o.0 + 0x2c, facing);
        crate::hw::sound::play(x.b, crate::hw::sound::CMD_368, x.o);
    } else {
        for field in [Obj::X, Obj::Y, Obj::Z] {
            let v = x.u16(field);
            wr16(x.b, o.0 + field, v);
        }
        let dx: u16 = if x.u8(Effect::FACING) != 0 { 0xfff0 } else { 0x10 };
        let px = rd16(x.b, o.0 + Obj::X).wrapping_add(dx);
        wr16(x.b, o.0 + Obj::X, px);
    }
}

pub(super) fn riser<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.add16(Obj::Z, 0x32);
            crate::services::palette::set_flash_color(x.b, crate::services::palette::FLASH_COLOR_WHITE_VALUE);
            vars::SHAKE.set(x.b, 0xa);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_5F, x.o);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_173, x.o);
            x.anim_start(riser::ANIM_SET.0, 9);
        }
        2 => riser_active(x),
        _ => return free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

pub(super) fn riser_active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => {
            if x.anim_step() {
                x.set16(riser::TIMER, 0x28);
                riser_anim(x, 0);
            }
        }
        2 => {
            x.anim_step();
            if !countdown(x, riser::TIMER) {
                riser_anim(x, 1);
            } else if x.u16(riser::TIMER) == 0x27 {
                x.add16(Obj::Z, 0x32u16.wrapping_neg());
            }
        }
        4 => {
            riser_climb(x);
            if (x.u16(Obj::Z) as i16) <= 0x68 {
                riser_anim(x, 2);
            } else {
                x.anim_step();
            }
        }
        6 | 8 => {
            riser_climb(x);
            x.anim_step();
            let variant = if x.u8(f::SUB_STATE) == 6 { 2 } else { 6 };
            riser_shoot(x, variant);
        }
        10 => {
            x.add16(Obj::Z, 3u16.wrapping_neg());
            let cam = crate::world::ram::CAMERA_Y.get(x.b).wrapping_sub(2);
            crate::world::ram::CAMERA_Y.set(x.b, cam);
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.set16(Obj::Z, 0);
                riser_anim(x, 3);
            } else {
                x.anim_step();
            }
        }
        12 => {
            let carrier = x.linked(Riser::CARRIER);
            if carrier.u32(x.b, f::STATE) == 0x0200_042a {
                riser_anim(x, 4);
            } else {
                x.anim_step();
            }
        }
        14 => {
            x.anim_step();
            if x.u8(Effect::ANIM_EVENT) != 0 {
                x.add8(f::SUB_STATE, 2);
                x.set16(riser::TIMER, 0x14);
            }
        }
        16 => {
            if countdown(x, riser::TIMER) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(riser::TIMER, 0x50);
            // the thrown chunk (type 39 variant 9): the original retries until it finds a slot
            let o = loop {
                if let Some(o) = x.alloc(&pools::EFFCT) {
                    break o;
                }
            };
            wr8(x.b, o.0, 1);
            wr8(x.b, o.0 + 2, 39);
            wr8(x.b, o.0 + 3, 9);
            wr16(x.b, o.0 + 0x42, x.o.0 as u16);
            let facing = x.u8(Effect::FACING);
            wr8(x.b, o.0 + 0x2c, facing);
            let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
            wr16(x.b, o.0 + Obj::X, px);
            wr16(x.b, o.0 + Obj::Y, py);
            wr16(x.b, o.0 + Obj::Z, 0x66);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_365, x.o);
        }
        18 => {
            if countdown(x, riser::TIMER) {
                return;
            }
            x.set16(riser::TIMER, 0x3c);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_366_B, x.o);
            riser_anim(x, 5);
        }
        20 => {
            if !countdown(x, riser::TIMER) {
                x.add8(f::SUB_STATE, 2);
                x.set16(riser::TIMER, 0x5a);
            }
            x.anim_step();
        }
        22 => {
            if countdown(x, riser::TIMER) {
                x.anim_step();
                return;
            }
            x.add8(f::SUB_STATE, 2);
            x.set16(Obj::VZ, 0x380);
            x.set16(Obj::AZ, 0xffd0);
            x.set16(Obj::VX, 0x280);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VY, 0xfea0);
            x.set16(Obj::AY, 0);
            x.move_xyz();
            x.anim_start(riser::ANIM_SET.0, 6);
        }
        24 => {
            x.move_xyz();
            if (x.u16(Obj::VZ) as i16) > 0 {
                x.anim_step();
            } else {
                riser_anim(x, 7);
            }
        }
        26 => {
            x.move_xyz();
            if (x.u16(Obj::Z) as i16) > 0 {
                x.anim_step();
            } else {
                x.set16(Obj::Z, 0);
                riser_anim(x, 8);
            }
        }
        28 => {
            if x.anim_step() {
                riser_anim(x, 0xa);
            }
        }
        _ => {
            x.anim_step();
        }
    }
}

