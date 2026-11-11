use super::*;
use crate::core::tasks::fading::Fade;

// ---------------------------------------------------------------------------------------------
// Types 82 and 83: spark placed on an object; rain of sparks
// ---------------------------------------------------------------------------------------------

pub(super) mod shards {
    pub const ANIM_SPARK: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1b670a);
    pub const ANIM_RAIN: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bb058);
    pub const OWNER: u32 = 0x5c;
    pub const BLINK: u32 = 0x50;
    pub const TIMER: u32 = 0x58;
    pub const COUNT: u32 = 0x5e;
}

/// Type 82: spark placed next to its creator, which blinks until the end of its action.
pub(super) fn spark<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(shards::BLINK, 0x30);
            let owner = x.linked(shards::OWNER);
            let facing = owner.u8(x.b, Effect::FACING);
            x.set8(Effect::FACING, facing);
            copy_position(x, owner);
            let (dx, dz) = if x.u8(f::VARIANT) == 0 {
                (0x10, 5)
            } else {
                let k = ((owner.u8(x.b, f::VARIANT) as u32) >> 1) << 2;
                (tables().shards_spark_offsets.at16(x.b, k), tables().shards_spark_offsets.at16(x.b, k + 2))
            };
            let dx = if x.u8(Effect::FACING) != 0 { dx.wrapping_neg() } else { dx };
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            x.add16(Obj::Y, 0xffff);
            x.anim_start(shards::ANIM_SPARK.0, 0);
            crate::world::placement::load_effect_palette(x.b, crate::world::placement::effect_palette::LIGHT);
        }
        2 => {
            x.anim_step();
            let owner = x.linked(shards::OWNER);
            let done = if x.u8(f::VARIANT) == 0 {
                if owner.u8(x.b, Carrier::SPARK_FLAG) != 0 {
                    true
                } else {
                    let n = x.u8(shards::BLINK).wrapping_sub(1);
                    x.set8(shards::BLINK, n);
                    n == 0
                }
            } else {
                owner.u8(x.b, Carrier::STEP) != 0x26
            };
            if done {
                x.add8(f::STATE, 2);
            }
            let odd = x.u8(shards::BLINK) & 1 != 0;
            set_hidden(x, odd);
        }
        _ => return free(x),
    }
    x.reinsert();
}

/// Type 83: variant 0 creates for 0xe6 frames one drop (variant 1) per frame, which falls in an
/// arc from the top of the screen.
pub(super) fn shard_rain<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 if v == 0 => {
            x.add8(f::STATE, 2);
            x.set16(shards::TIMER, 0xe6);
            crate::services::palette::set_flash_color(x.b, crate::actors::families::small::LIGHTNING_BLUE);
        }
        0 => {
            // Original glitch: A1 is the handler's address (0xb997c), left by the dispatch loop: the byte "`+0x2c`"
            // read is a constant byte of its code (0xb99a8, `effects/rain_handler_facing` of the engine data)
            let handler_facing = crate::world::tables::engine_table!("effects/rain_handler_facing").at8(x.b, 0);
            x.add8(f::STATE, 2);
            x.set16(Obj::VX, 0xfc00);
            x.set16(Obj::VZ, 0xf900);
            let (cx_, cy) = (crate::world::ram::CAMERA_X.get(x.b), crate::world::ram::CAMERA_Y.get(x.b));
            x.set16(Obj::X, cx_);
            x.set16(Obj::Y, cy.wrapping_add(0x18));
            x.set16(Obj::Z, 0xe8);
            let r = random_nibble16(x) & 0x1f;
            x.add16(Obj::Z, r);
            let mut k = (x.u8(shards::COUNT) & 0xf) as u32 * 2;
            if handler_facing != 0 {
                let vx = x.u16(Obj::VX).wrapping_neg();
                x.set16(Obj::VX, vx);
                k += 0x20;
            }
            let base = tables().shards_rain_offsets.at16(x.b, k);
            let r = random_nibble16(x) & 3;
            x.add16(Obj::X, base.wrapping_add(r));
            let rng = x.random();
            x.anim_start(shards::ANIM_RAIN.0, (rng & 1) + 1);
            x.reinsert();
        }
        2 if v == 0 => {
            let t = x.u16(shards::TIMER).wrapping_sub(1);
            x.set16(shards::TIMER, t);
            if t == 0 {
                x.add8(f::STATE, 2);
            }
            // 0xb9956: the next drop
            if let Some(o) = x.alloc(&pools::EFFCT) {
                wr8(x.b, o.0, 1);
                wr8(x.b, o.0 + 2, 83);
                wr8(x.b, o.0 + 3, 1);
                let n = x.u8(shards::COUNT);
                wr8(x.b, o.0 + 0x5e, n);
                x.add8(shards::COUNT, 1);
            }
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) < 0 {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Generator draw, low word.
pub(super) fn random_nibble16<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    x.random()
}

// ---------------------------------------------------------------------------------------------
// Types 85 and 86: objects of the ending sequence (piece that glides; flame wisp that moves away)
// ---------------------------------------------------------------------------------------------

/// Jeu d'animations de l'interface.
pub(super) const ANIM_INTERFACE: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c03a6);

pub(super) fn ending_piece<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(Obj::X, 0xd0);
            x.set16(Obj::Z, 0xffe6);
            x.set16(Obj::Y, 0x190);
            x.anim_start(ANIM_INTERFACE.0, 0x17);
            x.reinsert();
        }
        2 => match x.u8(f::SUB_STATE) {
            0 => {
                if vars::FROZEN.get(x.b) != 0 && rd8(x.b, floater::STEP) == 0xc {
                    x.add8(f::SUB_STATE, 2);
                    crate::hw::sound::play(x.b, crate::hw::sound::CMD_34C_44C_B, x.o);
                }
            }
            2 => {
                x.add16(Obj::X, 2u16.wrapping_neg());
                if (x.u16(Obj::X) as i16) <= 0x80 {
                    x.add8(f::SUB_STATE, 2);
                    wr8(x.b, floater::STEP, 0x14);
                    crate::services::camera_modes::start_shake(x.b, crate::services::camera_modes::shake::A, 0x10);
                    return;
                }
                // shakes in place, every other frame
                x.add16(Obj::X, 2);
                x.add16(Obj::Y, 2);
                if vars::FRAME_BYTE.get(x.b) & 1 != 0 {
                    x.add16(Obj::X, 4u16.wrapping_neg());
                    x.add16(Obj::Y, 4u16.wrapping_neg());
                }
            }
            _ => {}
        },
        _ => free(x),
    }
}

pub(super) fn ending_wisp<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            if x.u16(Wisp::CREATOR) == 0 {
                return;
            }
            let owner = x.linked(Wisp::CREATOR);
            let (mut px, mut py) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Y));
            if x.u8(f::VARIANT) == 0 {
                let k = (owner.u8(x.b, f::VARIANT) as u32) << 2;
                px = px.wrapping_add(tables().ending_piece_offsets.at16(x.b, k));
                py = py.wrapping_add(tables().ending_piece_offsets.at16(x.b, k + 2));
            }
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.set16(Obj::VX, 0xff20);
            x.anim_start(0x1991ca, 0);
            x.reinsert_foreground();
        }
        2 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
            let odd = vars::FRAME_LOW.get(x.b) & 1 != 0;
            set_hidden(x, odd);
            x.move_x();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 76: scene timer (plays a sound, then launches transition 0x15 or 0x14 after a delay)
// ---------------------------------------------------------------------------------------------

pub(super) fn scene_timer<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let (delay, sound) = if v == 0 { (0x168, crate::hw::sound::CMD_9D) } else { (0x10e, crate::hw::sound::CMD_8F) };
            x.set16(SceneTimer::DELAY, delay);
            crate::hw::sound::play(x.b, sound, x.o);
        }
        2 => {
            if countdown(x, 0x30) {
                return;
            }
            let music = if v == 0 { 0x15 } else { 0x14 };
            // Original glitch: D4 = the music stays for the following handlers (`dispatch::Leftovers`)
            *x.c.d4() = music;
            crate::hw::sound::play_with(x.b, crate::hw::sound::FROM_D4, crate::hw::sound::Source { object: Some(x.o), d4: Some(music), ..Default::default() });
            free(x);
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 84: trigger of an end-of-scene sequence (fade, camera, script tasks); type 90:
// dust drifting at the top of the screen
// ---------------------------------------------------------------------------------------------

/// Launches script task `entry` (0x9b6, `trap #0`), A0 kept.
pub(super) fn start_script<R: Bench>(x: &mut Cx<'_, R>, fade: Fade) {
    crate::core::kernel::create_task(x.b, 0x10, crate::core::tasks::Task::Fading(fade));
}

pub(super) fn finale_trigger<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            finale_step(x);
        }
        2 => finale_step(x),
        4 => {
            x.set8(Effect::SETTING, 1);
            x.set16(f::STATE, 0x600);
        }
        _ => free(x),
    }
}

pub(super) fn finale_step<R: Bench>(x: &mut Cx<'_, R>) {
    let mut sub = x.u8(f::SUB_STATE);
    if sub == 0 {
        x.add8(f::SUB_STATE, 4);
        x.set16(Effect::TIMER, 0xc0);
        sub = 4;
    }
    match sub {
        4 => {
            let t = x.u16(Effect::TIMER).wrapping_sub(1);
            x.set16(Effect::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.add8(f::SUB_STATE, 4);
            crate::hw::sound::play(x.b, crate::hw::sound::CMD_8A, x.o);
            vars::FADE_DELAY.set(x.b, 1);
            vars::FADE_PAGES.set(x.b, 8);
            vars::BLOCK_SOURCE_2_WORD.set(x.b, 0xf20);
            start_script(x, Fade::ToColours);
        }
        8 => {
            if vars::FADE_STEP.get(x.b) != 0 {
                return;
            }
            x.add8(f::SUB_STATE, 4);
            crate::world::ram::CAMERA_ALT_X.set(x.b, 0x440);
            crate::world::ram::CAMERA_ALT_Y.set(x.b, 0x38b);
            vars::FADE_DELAY.set(x.b, 4);
            vars::FADE_PAGES.set(x.b, 8);
            vars::FADE_STEP.set(x.b, 1);
            crate::actors::families::scenery::restore_palette_page(x);
            vars::BLOCK_SOURCE_2.set(x.b, 0xff1190);
            start_script(x, Fade::ToPalettes);
            finale_end(x);
        }
        12 => finale_end(x),
        _ => {}
    }
}

/// End: once the fade is finished, creates the BOX 0x14 object and goes on hold (state 6).
pub(super) fn finale_end<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::FADE_STEP.get(x.b) != 0 {
        return;
    }
    let Some(Obj(o)) = x.alloc(&pools::BOSS) else { return };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0x14);
    x.set8(Effect::SETTING, 1);
    x.set16(f::STATE, 0x600);
}

pub(super) fn drift_dust<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let mut px = (random_nibble16(x) & 0xff).wrapping_add(crate::world::ram::CAMERA_X.get(x.b));
            if x.u8(Effect::FACING) != 0 {
                px = px.wrapping_add(0x80);
            }
            x.set16(Obj::X, px);
            let py = (random_nibble16(x) & 0x3f).wrapping_add(crate::world::ram::CAMERA_Y.get(x.b)).wrapping_add(0x20);
            x.set16(Obj::Y, py);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, 0x10);
            x.set16(Obj::VX, 0);
            let ax: u16 = if x.u8(Effect::FACING) != 0 { 0xffc0 } else { 0x40 };
            x.set16(Obj::AX, ax);
            x.anim_start(0x1901ba, 0);
            x.reinsert();
        }
        2 => {
            x.move_xz();
            if x.anim_step() {
                free(x);
            }
        }
        _ => free(x),
    }
}

