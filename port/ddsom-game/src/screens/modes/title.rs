//! Title screens: "shuukyaku" (sub-mode 2, `0xfa296`) and second title (sub-mode 12, `0xfaef2`).
//!
//! Each screen is a sequence of steps (`FF0E72`, step of 2) run once per turn of the mode machine;
//! a step prepares system objects (SOBJ variants, SET), waits until they are done (fields of the
//! state block that the objects modify through the object's `+0x4c` pointer) and moves on to the next.
//! Waits within a step (object pool full, fade in progress) yield one frame to
//! the scheduler and resume at a Rust step (`steps!`).

use crate::hw::video::{palette, regs, SCRATCH, SCROLL2, SCROLL3, TEXT};
use crate::world::ram::Long;
use crate::world::screen_tables::{self as tables, Rom};
use super::{mode, Block, Flow, Sequence};
use crate::screens::scene::fading;
use crate::actors::effects_aids::scenery_6;
use crate::actors::families::creators::set_15_pure;
use crate::services::palette::{load_palette_set, load_palette_page};
use crate::services::video::{load_grid, clear_layer, Layer, Grid};
use crate::screens::scene::Ctx;
use crate::Native;
use crate::world::objects::{pools, Obj};
use crate::screens::obj;

/// Fields of the state block (offsets from `mode::BASE`).
mod f {
    pub const STEP: u32 = 2;
    /// Byte read by the title objects (animation variant to play).
    pub const CUE: u32 = 8;
    /// SOBJ object of the scene (16-bit address, sign-extended).
    pub const OBJECT: u32 = 0xa;
    /// Object-done flag (byte) or scroll speed (word, with `ACCEL`).
    pub const DONE: u32 = 0xc;
    pub const CUE_INDEX: u32 = 0xc;
    pub const SPEED: u32 = 0xc;
    pub const ACCEL: u32 = 0xe;
    /// Timer (byte or word depending on the step).
    pub const TIMER: u32 = 0x10;
}

/// Work RAM variables (offsets from A5).
mod var {
    use crate::world::ram::{Byte, Word};
    pub const VIDEO_FLAGS: u32 = 0x12;
    pub const LAYERS: u32 = 0x14;
    pub const REGION: Word = Word::a5(0x70);
    pub const SCREEN_TYPE: Byte = Byte::a5(0x36);
    pub const SCROLL_1_X: Word = Word::a5(0x4b6);
    pub const SCROLL_2_Y: Word = Word::a5(0x4ba);
    pub const CAMERA_X: Word = Word::a5(0x506);
    pub const CAMERA_Y: Word = Word::a5(0x50a);
    pub const CAMERA_2_X: Word = Word::a5(0x556);
    pub const CAMERA_2_Y: Word = Word::a5(0x55a);
    pub const CPS_COPY: Word = Word::a5(0x534);
    pub const GAUGE_SKIP: Word = Word::a5(0x7fb2);
}

/// Sounds and fade tasks of the titles.
mod ext {
    /// End of the title text: continues in the original (SOBJ object that moves on to the next step).
    pub const OBJECT_LAUNCH: crate::services::sound::Sound = crate::services::sound::CMD_1FE;
}

const LAYER_BITS: u16 = 0xe;
const FADE_PAGES_ALL: u8 = 0xf;
/// Palette of the title screens.
const TITLE_PALETTE: u16 = 0x53;
const GFX_PLANE_1: u32 = SCROLL2.0;
const GFX_PLANE_2: u32 = SCROLL2.0 + 0x3000;
const GFX_PLANE_1_START: u32 = TEXT.base;
const GFX_TABLE_B: u32 = SCROLL3.0 + 0x3800;
/// Rights notice, and the pointers to its second line per region.
const RIGHTS_TEXT: Rom = Rom::at(0xfacc4);
const RIGHTS_BY_REGION: Rom = Rom::at(0xfad92);

/// Screen of sub-mode `sub`, if it is rewritten here.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    let step = x.m8(f::STEP);
    Some(match sub {
        2 => title_step(x, step),
        0xc => second_title_step(x, step),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Aides communes

/// Frees the object pools and resets the sprites (start and end of screen).
fn reset_objects(x: &mut Ctx) {
    for pool in [&crate::world::objects::pools::SOBJ, &crate::world::objects::pools::SET, &crate::world::objects::pools::EFFCT] {
        pool.reset(x.b);
    }
    unlink_and_clear_lists(x);
}

/// Sorted lists undone (0x1f184) and sprite lists emptied (0x1de7c).
fn unlink_and_clear_lists(x: &mut Ctx) {
    crate::world::objects::lists::unlink_all(x.b);
    crate::services::sprites::clear_lists(x.b);
}

/// Creates the fade task `entry` (slot 0x10), A0 preserved.
fn start_fade_task(x: &mut Ctx, task: crate::core::tasks::Task) {
    crate::core::kernel::create_task(x.b, 0x10, task);
}

/// Common fade settings (delay, pages).
fn fade_setup(x: &mut Ctx, delay: u16, pages: u8) {
    fading::DELAY.set(x.b, delay);
    fading::PAGES.set(x.b, pages);
}

/// Loads the title palette on all pages (start of screen).
fn load_title_palette(x: &mut Ctx) {
    load_palette_page(x.b, 2, TITLE_PALETTE);
    load_palette_page(x.b, 3, TITLE_PALETTE);
}

/// Copies two plane 1 tile blocks then two plane 2 ones (title pictures).
fn load_title_tiles(x: &mut Ctx, a: u16, b: u16) {
    two_blocks(x, Grid::Layer2, a, GFX_PLANE_1);
    two_blocks(x, Grid::Layer3, b, GFX_TABLE_B);
}

/// Two tile blocks (`n`, `n + 1`) one after the other from `destination`; returns the end.
fn two_blocks(x: &mut Ctx, grid: Grid, n: u16, destination: u32) -> u32 {
    let end = load_grid(x.b, grid, n, destination);
    load_grid(x.b, grid, n + 1, end.destination).destination
}

// ---------------------------------------------------------------------------------------------
// Sous-mode 2 : titre

fn title_step(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0x00 => t00_reset(x),
        0x02 => t02_cue(x),
        0x04 => t04_fade_out(x),
        0x06 => t06_start(x),
        0x08 => t08_wait_flash(x),
        0x0a => t0a_wait(x),
        0x0c => t0c_fade_task(x),
        0x0e => t0e_wait_fade(x),
        0x10 => t10_start(x),
        0x12 => t12_wait(x),
        0x14 => t14_wait(x),
        0x16 => t16_slide(x),
        0x18 => t18_palette(x),
        0x1a => t1a_wait(x),
        0x1c => t1c_wait_logo(x),
        0x1e => t1e_end(x),
        // table 0xfa2a4 has sixteen entries, all rewritten (code-mort.md, batch "CPU removal")
        _ => unreachable!("title: step {step:#x} outside table 0xfa2a4"),
    }
}

/// Step 0: clears everything, loads the palette and pictures, displays the background.
fn t00_reset(x: &mut Ctx) -> Flow {
    reset_objects(x);
    crate::services::video::black_screen(x.b);
    x.set_v16(var::LAYERS, 0x12c0);
    x.wait(4, Step::T00Load)
}
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        T00Load = 0xfa2fe => |x| {
            var::CAMERA_Y.set(x.b, 0x300);
            for off in [var::CAMERA_X, var::SCROLL_1_X, var::SCROLL_2_Y, var::CAMERA_2_X, var::CAMERA_2_Y] {
                off.set(x.b, 0);
            }
            load_title_palette(x);
            load_palette_page(x.b, 1, 2);
            load_palette_set(x.b, 1);
            load_title_tiles(x, 138, 403);
            x.render();
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::T00Fade)
        },
        T00Fade = 0xfa384 => |x| {
            x.set_m8(f::TIMER, 30);
            x.set_m8(f::CUE_INDEX, 0);
            x.set_m8(0xd, 6);
            x.set_m8(f::CUE, 0);
            x.next_step();
            crate::services::sound::play_global(x.b, crate::services::sound::CMD_06);
            fade_setup(x, 3, FADE_PAGES_ALL);
            crate::core::tasks::fading::rise(x)
        },
        T06Object = 0xfa472 => t06_object,
        T06Retry = 0xfa486 => t06_object,
        T08Next = 0xfa4bc => |x| {
            x.next_step();
            x.set_v16(var::LAYERS, 0x12ce);
            var::CAMERA_Y.set(x.b, 0x300);
            x.set_m16(f::TIMER, 60);
            x.render();
            Flow::Done
        },
        T0aRetry = 0xfa4fc => |x| {
            // `bra 0xfa4e8`: retries without raising the flag again
            let Ok(set) = set_15_pure(x.b, 1) else {
                return x.wait(1, Step::T0aRetry);
            };
            set.set_u32(x.b, obj::SIGNAL, mode::BASE);
            x.next_step();
            Flow::Done
        },
        T10Object = 0xfa56e => t10_object,
        T10Retry = 0xfa582 => t10_object,
        T12Scenery = 0xfa5ba => |x| {
            x.set_m8(f::CUE, 2);
            x.render();
            x.set_v16(var::LAYERS, 0x12ce);
            x.set_m16(f::TIMER, 60);
            var::CAMERA_Y.set(x.b, 0x70);
            var::CAMERA_X.set(x.b, 0);
            var::SCROLL_1_X.set(x.b, 0);
            var::SCROLL_2_Y.set(x.b, 0);
            let mut tile = 480u16;
            let mut destination = GFX_PLANE_1;
            for _ in 0..4 {
                destination = two_blocks(x, Grid::Layer2, tile, destination) + 0x800;
                tile += 2;
            }
            x.render();
            x.set_v16(var::LAYERS, 0x09ce);
            x.wait(1, Step::T12Fade)
        },
        T12Fade = 0xfa620 => |x| {
            fade_setup(x, 3, FADE_PAGES_ALL);
            start_fade_task(x, crate::screens::scene::fading::RISE);
            x.next_step();
            x.set_m16(f::SPEED, 0xb00);
            x.set_m16(f::ACCEL, 0xffe0);
            x.set_m16(f::TIMER, 60);
            // Original glitch: the carry is not tested; a failure keeps the allocation's A1
            let set = set_15_pure(x.b, 0).unwrap_or_else(|a1| a1);
            x.set_m16(f::OBJECT, set.0 as u16);
            Flow::Done
        },
        T18Retry = 0xfa722 => t18_object,
        T18Loop = 0xfa746 => |x| {
            x.push_return(Step::T18After);
            t18_sub(x)
        },
        T18After = 0xfa74a => |x| {
            x.render();
            let link = x.m16(f::DONE);
            let o = link as i16 as i32 as u32 & 0xff_ffff;
            if Obj(o).u8(x.b, Obj::ACTIVE) != 0 {
                return x.wait(1, Step::T18Loop);
            }
            x.next_step();
            x.set_m16(f::TIMER, 60);
            Flow::Done
        },
        T18SubRetry = 0xfa77c => |x| {
            // `bra 0xfa762`
            t18_sub(x)
        },
        T1aRetry = 0xfa7fc => t1a_object,
        T1cText = 0xfa88e => |x| {
            x.render();
            crate::core::tasks::prompt::launch(x.b);
            x.set_m16(f::TIMER, 300);
            crate::services::text::print(x.b, RIGHTS_TEXT.addr());
            let region = var::REGION.get(x.b);
            let index = region.wrapping_add(region) as i16 as i32 as u32;
            let text = RIGHTS_BY_REGION.long(x.b, index);
            crate::services::text::print(x.b, text);
            Flow::Done
        },
        T1eWait = 0xfa90e => |x| leave_wait(x, Step::T1eWait, Step::T1eCleared, false),
        T1eCleared = 0xfa94e => |x| {
            crate::core::tasks::prompt::stop(x.b);
            clear_layer(x.b, Layer::One);
            clear_layer(x.b, Layer::Two);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::T1eDone)
        },
        T1eDone = 0xfa96e => |x| {
            x.set_m16(f::TIMER, 300);
            // `jmp 0x1a60a`
            load_palette_page(x.b, 1, 1);
            Flow::Done
        },
        S02Load = 0xfaf5c => |x| {
            var::SCREEN_TYPE.set(x.b, 1);
            load_title_palette(x);
            load_palette_set(x.b, 1);
            fade_setup(x, 3, 13);
            crate::services::palette::full_brightness(x.b);
            x.or16(var::VIDEO_FLAGS, 1);
            let flags = x.v16(var::VIDEO_FLAGS);
            regs::VIDEO_CONTROL.set(x.b, flags);
            regs::ROW_SCROLL_BASE.set(x.b, 0x9200);
            var::CPS_COPY.set(x.b, 0x9200);
            var::GAUGE_SKIP.set(x.b, 0);
            clear_sprite_area(x);
            two_blocks(x, Grid::Layer2, 454, GFX_PLANE_2);
            two_blocks(x, Grid::Layer3, 368, GFX_TABLE_B);
            s02_object(x)
        },
        S02Retry = 0xfaff8 => |x| {
            s02_object(x)
        },
        S02Show = 0xfb012 => |x| {
            x.or16(var::LAYERS, LAYER_BITS);
            x.next_step();
            var::SCROLL_2_Y.set(x.b, 0);
            let _ = set_15_pure(x.b, 2);
            x.wait(1, Step::S02Palette)
        },
        S02Palette = 0xfb030 => |x| {
            crate::services::palette::load_scene_page(x.b, 1);
            fading::PAGES.set(x.b, 2);
            crate::services::palette::full_brightness(x.b);
            Flow::Done
        },
        S04Retry = 0xfb064 => s04_object,
        S04Show = 0xfb088 => |x| {
            x.render();
            two_blocks(x, Grid::Blocks4x4, 7, GFX_PLANE_1_START);
            Flow::Done
        },
        S08Wait = 0xfb106 => |x| leave_wait(x, Step::S08Wait, Step::S08Cleared, true),
        S08Cleared = 0xfb152 => |x| {
            for layer in [Layer::One, Layer::Two, Layer::Three] {
                clear_layer(x.b, layer);
            }
            var::GAUGE_SKIP.set(x.b, 0);
            clear_sprite_area(x);
            load_palette_page(x.b, 1, 1);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::S08Done)
        },
        S08Done = 0xfb182 => |x| {
            x.set_m16(f::TIMER, 300);
            Flow::Done
        },
    }
}

/// Step 2: launches the title objects one after another, at intervals given by a table.
fn t02_cue(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m8(f::TIMER).wrapping_sub(1);
    x.set_m8(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let Ok(Obj(obj)) = scenery_6(x.b, 6) else {
        x.next_step();
        return Flow::Done;
    };
    let index = x.m8(f::CUE_INDEX);
    Obj(obj).set_u8(x.b, obj::INDEX, index);
    Obj(obj).set_u32(x.b, obj::SIGNAL, mode::BASE);
    x.set_m8(f::CUE_INDEX, index.wrapping_add(1));
    let delay = T02_DELAYS.pc_byte(x.b, index as u32);
    x.set_m8(f::TIMER, delay);
    if delay & 0x80 == 0 {
        return Flow::Done;
    }
    x.next_step();
    Flow::Done
}
/// Delays (frames) between appearances, in the original code; 0xff ends.
const T02_DELAYS: Rom = Rom::at(0xfa42e);

/// Step 4: waits for the objects to finish (`+0xd` zero), then fades to black.
fn t04_fade_out(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(0xd) != 0 {
        return Flow::Done;
    }
    x.next_step();
    fade_setup(x, 3, FADE_PAGES_ALL);
    crate::core::tasks::fading::descent(x)
}

/// Step 6: next background; creates the variant 7 object (waits if the pool is full).
fn t06_start(x: &mut Ctx) -> Flow {
    var::CAMERA_Y.set(x.b, 0);
    x.set_m8(f::CUE, 1);
    x.render();
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::T06Object)
}
fn t06_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 7) else {
        return x.wait(1, Step::T06Retry);
    };
    x.set_m8(f::DONE, 1);
    Obj(o).set_u32(x.b, obj::SIGNAL, mode::BASE);
    Obj(o).set_u8(x.b, obj::INDEX, 0);
    x.next_step();
    Flow::Done
}

/// Step 8: waits for the object to finish, white flash.
fn t08_wait_flash(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    fading::PAGES.set(x.b, FADE_PAGES_ALL);
    crate::services::palette::full_brightness(x.b);
    x.wait(4, Step::T08Next)
}

/// Step 10: after 60 frames, creates the variant 1 SET object (waits if the pool is full).
fn t0a_wait(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    t0a_object(x)
}
fn t0a_object(x: &mut Ctx) -> Flow {
    x.set_m8(f::DONE, 1);
    let Ok(Obj(set)) = set_15_pure(x.b, 1) else {
        return x.wait(1, Step::T0aRetry);
    };
    Obj(set).set_u32(x.b, obj::SIGNAL, mode::BASE);
    x.next_step();
    Flow::Done
}

/// Step 12: waits for the object, then starts the fade to black (task).
fn t0c_fade_task(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    x.next_step();
    fade_setup(x, 3, FADE_PAGES_ALL);
    start_fade_task(x, crate::screens::scene::fading::DESCENT);
    Flow::Done
}

/// Step 14: waits for the fade to finish (`A5+0x48f` zero).
fn t0e_wait_fade(x: &mut Ctx) -> Flow {
    x.render();
    if fading::STEP.get(x.b) != 0 {
        return Flow::Done;
    }
    x.next_step();
    Flow::Done
}

/// Step 16: second part of the title: camera at 512, variant 7 object (waits if full).
fn t10_start(x: &mut Ctx) -> Flow {
    var::CAMERA_X.set(x.b, 0x200);
    x.set_m8(f::CUE, 0xff);
    x.render();
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::T10Object)
}
fn t10_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 7) else {
        return x.wait(1, Step::T10Retry);
    };
    x.set_m8(f::DONE, 1);
    Obj(o).set_u32(x.b, obj::SIGNAL, mode::BASE);
    Obj(o).set_u8(x.b, obj::INDEX, 1);
    x.next_step();
    Flow::Done
}

/// Step 18: white flash, then loads the lower terrain (four strips of two tile blocks).
fn t12_wait(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    fading::PAGES.set(x.b, FADE_PAGES_ALL);
    crate::services::palette::full_brightness(x.b);
    x.wait(4, Step::T12Scenery)
}

/// Step 20: waits 60 frames, then starts animation 3.
fn t14_wait(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m8(f::TIMER).wrapping_sub(1);
    x.set_m8(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    x.set_m8(f::CUE, 3);
    x.next_step();
    Flow::Done
}

/// Step 22: the camera slides down (speed increasing up to 0x400) then stops at 0x300.
fn t16_slide(x: &mut Ctx) -> Flow {
    x.render();
    let mut speed = x.m16(f::SPEED);
    if speed != 0x400 {
        speed = speed.wrapping_add(x.m16(f::ACCEL));
        x.set_m16(f::SPEED, speed);
    }
    let delta = (((speed as u32) << 16) as i32) >> 8;
    for v in [var::CAMERA_Y, var::CAMERA_2_Y] {
        let v = Long(v.0);
        let moved = v.get(x.b).wrapping_add(delta as u32);
        v.set(x.b, moved);
    }
    if var::CAMERA_Y.get(x.b) < 0x300 {
        return Flow::Done;
    }
    var::CAMERA_Y.set(x.b, 0x300);
    x.next_step();
    x.set_m16(f::TIMER, 30);
    Flow::Done
}

/// Step 24: copies the logo palette (four 32-byte blocks), creates the logo object and
/// attend qu'il ait disparu.
fn t18_palette(x: &mut Ctx) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    // four 32-byte blocks from ROM to the palette table
    let mut src = LOGO_PALETTE.addr();
    for color in [0x170u32, 0x180, 0x1b0, 0x1f0] {
        let dst = palette(0, color);
        for i in 0..8 {
            let v = tables::long(x.b, src + 4 * i);
            dst.offset(4 * i).set_long(x.b, v);
        }
        src += 32;
    }
    fading::PAGES.set(x.b, 1);
    crate::services::palette::full_brightness(x.b);
    t18_object(x)
}
fn t18_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::T18Retry) };
    Obj(o).set_u8(x.b, Obj::ACTIVE, 1);
    Obj(o).set_u8(x.b, Obj::KIND, 4);
    Obj(o).set_u8(x.b, Obj::VARIANT, 0x34);
    x.set_m16(f::DONE, o as u16);
    x.set_m16(f::TIMER, 80);
    x.wait(1, Step::T18Loop)
}
/// Logo palette: four consecutive 32-byte blocks in ROM.
const LOGO_PALETTE: Rom = Rom::at(0x2538c0);
/// Subroutine `0xfa762` (called by `bsr`, returns at `Step::T18After`): after 80 frames, creates the
/// text object and runs the original's script.
fn t18_sub(x: &mut Ctx) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        crate::core::tasks::return_to_caller(x.b);
        return Flow::Yield;
    }
    t18_text_object(x)
}
fn t18_text_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { return x.wait(1, Step::T18SubRetry) };
    Obj(o).set_u8(x.b, Obj::ACTIVE, 1);
    Obj(o).set_u8(x.b, Obj::KIND, 0);
    Obj(o).set_u8(x.b, Obj::VARIANT, 0xf0);
    Obj(o).set_u8(x.b, obj::INDEX, 0);
    Obj(o).set_u32(x.b, Obj::X, 0);
    Obj(o).set_u16(x.b, Obj::Z, 0x3f0);
    Obj(o).set_u32(x.b, Obj::Y, 1);
    let target = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    Obj(target).set_u32(x.b, Obj::STATE, 0x0600_0000);
    var::SCROLL_2_Y.set(x.b, 0);
    x.set_v16(var::LAYERS, 0x12ce);
    crate::services::sound::play_global(x.b, ext::OBJECT_LAUNCH);
    Flow::Done
}

/// Step 26: after 60 frames, clears the planes and creates the logo object (variant 66).
fn t1a_wait(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    x.next_step();
    two_blocks(x, Grid::Blocks4x4, 7, GFX_PLANE_1_START);
    crate::services::sound::play_global(x.b, crate::services::sound::CMD_1FF);
    t1a_object(x)
}
fn t1a_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::T1aRetry) };
    Obj(o).set_u8(x.b, Obj::ACTIVE, 1);
    Obj(o).set_u8(x.b, Obj::KIND, 0);
    Obj(o).set_u8(x.b, Obj::VARIANT, 0x42);
    Obj(o).set_u8(x.b, obj::INDEX, 3);
    x.set_m16(f::OBJECT, o as u16);
    Flow::Done
}

/// Step 28: waits for the logo object to reach state 6, then creates the texts (3 objects) and displays
/// the rights notice according to the region.
fn t1c_wait_logo(x: &mut Ctx) -> Flow {
    x.render();
    let o = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    if Obj(o).u8(x.b, Obj::STATE) != 6 {
        return Flow::Done;
    }
    x.next_step();
    title_text(x)
}
/// `0xfa830`: three text objects around the camera, then the rights text.
pub fn title_text(x: &mut Ctx) -> Flow {
    // three variants (0xe6, 0xe5, 0xe4, low byte first) placed from the active camera and the table
    // of offsets 0xfa8bc
    const OFFSETS: Rom = Rom::at(0xfa8bc);
    let (camera_x, camera_y) = if var::SCREEN_TYPE.get(x.b) != 0 { (var::CAMERA_2_X, var::CAMERA_2_Y) } else { (var::CAMERA_X, var::CAMERA_Y) };
    let mut variants = 0x00e4_e5e6u32;
    for k in 0..3 {
        let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { break };
        Obj(o).set_u8(x.b, Obj::ACTIVE, 1);
        Obj(o).set_u8(x.b, Obj::KIND, 0);
        Obj(o).set_u8(x.b, Obj::VARIANT, variants as u8);
        variants >>= 8;
        let (px, py) = (camera_x.get(x.b), camera_y.get(x.b));
        let (dx, dy) = (OFFSETS.word(x.b, 4 * k), OFFSETS.word(x.b, 4 * k + 2));
        Obj(o).set_u16(x.b, Obj::X, px.wrapping_add(dx));
        Obj(o).set_u16(x.b, Obj::Z, py.wrapping_add(dy));
        Obj(o).set_u32(x.b, Obj::Y, 0);
    }
    x.wait(1, Step::T1cText)
}

/// Step 30: after 300 frames, fade to black (task), clears everything and moves to the next sub-mode.
fn t1e_end(x: &mut Ctx) -> Flow {
    x.render();
    leave_screen(x, Step::T1eWait)
}

/// End of screen common to both titles: 300-frame timer, next sub-mode, fade
/// to black by the task, then cleanup in three phases.
fn leave_screen(x: &mut Ctx, loop_resume: Step) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = mode::SUB.get(x.b).wrapping_add(2);
    mode::SUB.set(x.b, s);
    Obj(mode::BASE).set_u8(x.b, f::STEP, 0);
    fading::STEP.set(x.b, 1);
    fading::DELAY.set(x.b, 4);
    fading::PAGES.set(x.b, FADE_PAGES_ALL);
    start_fade_task(x, crate::screens::scene::fading::DESCENT);
    x.wait(1, loop_resume)
}

/// Waits for the fade to finish (`A5+0x48f`), then clear and yield of 16 frames.
fn leave_wait(x: &mut Ctx, loop_resume: Step, cleared: Step, second_title: bool) -> Flow {
    x.render();
    if fading::STEP.get(x.b) != 0 {
        return x.wait(1, loop_resume);
    }
    // the original frees the pools in a different order depending on the screen
    use crate::world::objects::pools::{EFFCT, SET, SOBJ};
    let pools = if second_title { [&SET, &EFFCT, &SOBJ] } else { [&SOBJ, &SET, &EFFCT] };
    for pool in pools {
        pool.reset(x.b);
    }
    unlink_and_clear_lists(x);
    crate::services::video::clear_objects(x.b);
    if second_title {
        var::SCREEN_TYPE.set(x.b, 0);
        x.and16(var::VIDEO_FLAGS, !1);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(16, cleared)
}

// ---------------------------------------------------------------------------------------------
// Sous-mode 12 : second titre

fn second_title_step(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0 => {
            crate::core::tasks::prompt::stop(x.b);
            x.next_step();
            reset_objects(x);
            Flow::Done
        }
        2 => s02_setup(x),
        4 => s04_wait_object(x),
        6 => s06_wait_logo(x),
        8 => {
            x.render();
            leave_screen(x, Step::S08Wait)
        }
        // table 0xfaf00 has five entries, all rewritten (code-mort.md, batch "CPU removal")
        _ => unreachable!("second title: step {step:#x} outside table 0xfaf00"),
    }
}

/// Step 2: title background, palette, two objects (SOBJ variant 9, then SET 2).
fn s02_setup(x: &mut Ctx) -> Flow {
    x.set_v16(var::LAYERS, 0x12c2);
    for off in [var::CAMERA_X, var::CAMERA_Y, var::CAMERA_2_X, var::CAMERA_2_Y] {
        Long(off.0).set(x.b, 0);
    }
    x.wait(4, Step::S02Load)
}
fn s02_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 9) else {
        return x.wait(1, Step::S02Retry);
    };
    x.set_m8(f::CUE, 0);
    Obj(o).set_u32(x.b, obj::SIGNAL, mode::BASE);
    x.set_m16(f::DONE, o as u16);
    x.render();
    x.wait(2, Step::S02Show)
}

/// Clears the 512-byte sprite area (`0xfb18a`).
fn clear_sprite_area(x: &mut Ctx) {
    for i in 0..128 {
        SCRATCH.offset(4 * i).set_long(x.b, 0);
    }
}

/// Step 4: waits for the object's signal (`+8`), creates the variant 68 SET object.
fn s04_wait_object(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::CUE) == 0 {
        return Flow::Done;
    }
    x.next_step();
    s04_object(x)
}
fn s04_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::S04Retry) };
    Obj(o).set_u8(x.b, Obj::ACTIVE, 1);
    Obj(o).set_u8(x.b, Obj::KIND, 0);
    Obj(o).set_u8(x.b, Obj::VARIANT, 0x44);
    Obj(o).set_u8(x.b, obj::INDEX, 3);
    x.set_m16(f::OBJECT, o as u16);
    x.wait(1, Step::S04Show)
}

/// Step 6: waits for the logo object to be at state 6, then rights text (like title 1).
fn s06_wait_logo(x: &mut Ctx) -> Flow {
    x.render();
    let o = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    if Obj(o).u8(x.b, Obj::STATE) != 6 {
        return Flow::Done;
    }
    x.next_step();
    title_text(x)
}


pub const NATIVES: &[Native] = &[];
