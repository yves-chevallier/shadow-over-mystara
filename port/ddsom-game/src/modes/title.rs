//! Title screens: "shuukyaku" (sub-mode 2, `0xfa296`) and second title (sub-mode 12, `0xfaef2`).
//!
//! Each screen is a sequence of steps (`FF0E72`, step of 2) run once per turn of the mode machine;
//! a step prepares system objects (SOBJ variants, SET), waits until they are done (fields of the
//! state block that the objects modify through the object's `+0x4c` pointer) and moves on to the next.
//! Waits within a step (object pool full, fade in progress) yield one frame to
//! the scheduler and resume at a Rust step (`steps!`).

use super::{mode, Block, Flow, Sequence};
use crate::scene::fading;
use crate::effects_aids::scenery_6;
use crate::families::creators::set_15_pure;
use crate::palette::{load_palette_set, load_palette_page};
use crate::video::{load_grid, clear_layer, Layer, Grid};
use crate::mem::{rd16, rd8, rd32, wr16, wr32, wr8};
use crate::scene::Ctx;
use crate::Native;
use crate::objects::{pools, Obj};

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
    pub const VIDEO_FLAGS: u32 = 0x12;
    pub const LAYERS: u32 = 0x14;
    pub const REGION: u32 = 0x70;
    pub const SCREEN_TYPE: u32 = 0x36;
    pub const SCROLL_1_X: u32 = 0x4b6;
    pub const SCROLL_2_Y: u32 = 0x4ba;
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA_2_X: u32 = 0x556;
    pub const CAMERA_2_Y: u32 = 0x55a;
    pub const CPS_COPY: u32 = 0x534;
    pub const GAUGE_SKIP: u32 = 0x7fb2;
}

/// Sounds and fade tasks of the titles.
mod ext {
    pub const SOUND_E0EBC: u32 = 0xe0ebc;
    pub const SOUND_E185C: u32 = 0xe185c;
    /// End of the title text: continues in the original (SOBJ object that moves on to the next step).
    pub const OBJECT_LAUNCH: u32 = 0xea288;
}

const LAYER_BITS: u16 = 0xe;
const FADE_PAGES_ALL: u8 = 0xf;
/// Palette of the title screens.
const TITLE_PALETTE: u16 = 0x53;
const GFX_PLANE_1: u32 = 0x914000;
const GFX_PLANE_2: u32 = 0x917000;
const GFX_PLANE_1_START: u32 = 0x912000;
const GFX_TABLE_B: u32 = 0x91b800;
const CPS_REG_VIDEO: u32 = 0x804122;
const CPS_REG_SCROLL: u32 = 0x804108;
const SPRITE_CLEAR_AREA: u32 = 0x920000;

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
    for pool in [&crate::objects::pools::SOBJ, &crate::objects::pools::SET, &crate::objects::pools::EFFCT] {
        pool.reset(x.b);
    }
    unlink_and_clear_lists(x);
}

/// Sorted lists undone (0x1f184) and sprite lists emptied (0x1de7c).
fn unlink_and_clear_lists(x: &mut Ctx) {
    crate::objects::lists::unlink_all(x.b);
    crate::sprites::clear_lists(x.b);
}

/// Creates the fade task `entry` (slot 0x10), A0 preserved.
fn start_fade_task(x: &mut Ctx, entry: u32) {
    crate::kernel::create_task(x.b, 0x10, entry);
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
    crate::video::black_screen(x.b);
    x.set_v16(var::LAYERS, 0x12c0);
    x.wait(4, Step::T00Load)
}
const T00_LOAD: u32 = 0xfa2fe;
crate::steps! {
    /// Steps (resume points) of the state machine.
    pub enum Step {
        T00Load = T00_LOAD => |x| {
            x.set_v16(var::CAMERA_Y, 0x300);
            for off in [var::CAMERA_X, var::SCROLL_1_X, var::SCROLL_2_Y, var::CAMERA_2_X, var::CAMERA_2_Y] {
                x.set_v16(off, 0);
            }
            load_title_palette(x);
            load_palette_page(x.b, 1, 2);
            load_palette_set(x.b, 1);
            load_title_tiles(x, 138, 403);
            x.render();
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::T00Fade)
        },
        T00Fade = T00_FADE => |x| {
            x.set_m8(f::TIMER, 30);
            x.set_m8(f::CUE_INDEX, 0);
            x.set_m8(0xd, 6);
            x.set_m8(f::CUE, 0);
            x.next_step();
            crate::sound::play_global(x.b, ext::SOUND_E0EBC);
            fade_setup(x, 3, FADE_PAGES_ALL);
            crate::tasks::fading::rise(x)
        },
        T06Object = T06_OBJECT => t06_object,
        T06Retry = T06_RETRY => t06_object,
        T08Next = T08_NEXT => |x| {
            x.next_step();
            x.set_v16(var::LAYERS, 0x12ce);
            x.set_v16(var::CAMERA_Y, 0x300);
            x.set_m16(f::TIMER, 60);
            x.render();
            Flow::Done
        },
        T0aRetry = T0A_RETRY => |x| {
            // `bra 0xfa4e8`: retries without raising the flag again
            let Ok(set) = set_15_pure(x.b, 1) else {
                return x.wait(1, Step::T0aRetry);
            };
            wr32(x.b, set.0 + 0x4c, mode::BASE);
            x.next_step();
            Flow::Done
        },
        T10Object = T10_OBJECT => t10_object,
        T10Retry = T10_RETRY => t10_object,
        T12Scenery = T12_SCENERY => |x| {
            x.set_m8(f::CUE, 2);
            x.render();
            x.set_v16(var::LAYERS, 0x12ce);
            x.set_m16(f::TIMER, 60);
            x.set_v16(var::CAMERA_Y, 0x70);
            x.set_v16(var::CAMERA_X, 0);
            x.set_v16(var::SCROLL_1_X, 0);
            x.set_v16(var::SCROLL_2_Y, 0);
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
        T12Fade = T12_FADE => |x| {
            fade_setup(x, 3, FADE_PAGES_ALL);
            start_fade_task(x, crate::scene::fading::RISE);
            x.next_step();
            x.set_m16(f::SPEED, 0xb00);
            x.set_m16(f::ACCEL, 0xffe0);
            x.set_m16(f::TIMER, 60);
            // Original glitch: the carry is not tested; a failure keeps the allocation's A1
            let set = set_15_pure(x.b, 0).unwrap_or_else(|a1| a1);
            x.set_m16(f::OBJECT, set.0 as u16);
            Flow::Done
        },
        T18Retry = T18_RETRY => t18_object,
        T18Loop = T18_LOOP => |x| {
            x.push_return(Step::T18After);
            t18_sub(x)
        },
        T18After = T18_AFTER => |x| {
            x.render();
            let link = x.m16(f::DONE);
            let o = link as i16 as i32 as u32 & 0xff_ffff;
            if rd8(x.b, o) != 0 {
                return x.wait(1, Step::T18Loop);
            }
            x.next_step();
            x.set_m16(f::TIMER, 60);
            Flow::Done
        },
        T18SubRetry = T18_SUB_RETRY => |x| {
            // `bra 0xfa762`
            t18_sub(x)
        },
        T1aRetry = T1A_RETRY => t1a_object,
        T1cText = T1C_TEXT => |x| {
            x.render();
            crate::tasks::prompt::launch(x.b);
            x.set_m16(f::TIMER, 300);
            crate::text::print(x.b, 0xfacc4);
            let region = x.v16(var::REGION);
            let index = region.wrapping_add(region) as i16 as i32 as u32;
            let text = rd32(x.b, 0xfad92u32.wrapping_add(index));
            crate::text::print(x.b, text);
            Flow::Done
        },
        T1eWait = T1E_FADED => |x| leave_wait(x, Step::T1eWait, Step::T1eCleared, false),
        T1eCleared = T1E_CLEARED => |x| {
            crate::tasks::prompt::stop(x.b);
            clear_layer(x.b, Layer::One);
            clear_layer(x.b, Layer::Two);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::T1eDone)
        },
        T1eDone = T1E_DONE => |x| {
            x.set_m16(f::TIMER, 300);
            // `jmp 0x1a60a`
            load_palette_page(x.b, 1, 1);
            Flow::Done
        },
        S02Load = S02_LOAD => |x| {
            x.set_v8(var::SCREEN_TYPE, 1);
            load_title_palette(x);
            load_palette_set(x.b, 1);
            fade_setup(x, 3, 13);
            crate::palette::full_brightness(x.b);
            x.or16(var::VIDEO_FLAGS, 1);
            let flags = x.v16(var::VIDEO_FLAGS);
            wr16(x.b, CPS_REG_VIDEO, flags);
            wr16(x.b, CPS_REG_SCROLL, 0x9200);
            x.set_v16(var::CPS_COPY, 0x9200);
            x.set_v16(var::GAUGE_SKIP, 0);
            clear_sprite_area(x);
            two_blocks(x, Grid::Layer2, 454, GFX_PLANE_2);
            two_blocks(x, Grid::Layer3, 368, GFX_TABLE_B);
            s02_object(x)
        },
        S02Retry = S02_RETRY => |x| {
            s02_object(x)
        },
        S02Show = S02_SHOW => |x| {
            x.or16(var::LAYERS, LAYER_BITS);
            x.next_step();
            x.set_v16(var::SCROLL_2_Y, 0);
            let _ = set_15_pure(x.b, 2);
            x.wait(1, Step::S02Palette)
        },
        S02Palette = S02_PALETTE => |x| {
            crate::palette::load_scene_page(x.b, 1);
            fading::PAGES.set(x.b, 2);
            crate::palette::full_brightness(x.b);
            Flow::Done
        },
        S04Retry = S04_RETRY => s04_object,
        S04Show = S04_SHOW => |x| {
            x.render();
            two_blocks(x, Grid::Blocks4x4, 7, GFX_PLANE_1_START);
            Flow::Done
        },
        S08Wait = S08_WAIT => |x| leave_wait(x, Step::S08Wait, Step::S08Cleared, true),
        S08Cleared = S08_CLEARED => |x| {
            for layer in [Layer::One, Layer::Two, Layer::Three] {
                clear_layer(x.b, layer);
            }
            x.set_v16(var::GAUGE_SKIP, 0);
            clear_sprite_area(x);
            load_palette_page(x.b, 1, 1);
            x.or16(var::LAYERS, LAYER_BITS);
            x.wait(1, Step::S08Done)
        },
        S08Done = S08_DONE => |x| {
            x.set_m16(f::TIMER, 300);
            Flow::Done
        },
    }
}
const T00_FADE: u32 = 0xfa384;

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
    wr8(x.b, obj + 0x2f, index);
    wr32(x.b, obj + 0x4c, mode::BASE);
    x.set_m8(f::CUE_INDEX, index.wrapping_add(1));
    let delay = x.prog8(T02_DELAYS + index as u32);
    x.set_m8(f::TIMER, delay);
    if delay & 0x80 == 0 {
        return Flow::Done;
    }
    x.next_step();
    Flow::Done
}
/// Delays (frames) between appearances, in the original code; 0xff ends.
const T02_DELAYS: u32 = 0xfa42e;

/// Step 4: waits for the objects to finish (`+0xd` zero), then fades to black.
fn t04_fade_out(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(0xd) != 0 {
        return Flow::Done;
    }
    x.next_step();
    fade_setup(x, 3, FADE_PAGES_ALL);
    crate::tasks::fading::descent(x)
}

/// Step 6: next background; creates the variant 7 object (waits if the pool is full).
fn t06_start(x: &mut Ctx) -> Flow {
    x.set_v16(var::CAMERA_Y, 0);
    x.set_m8(f::CUE, 1);
    x.render();
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::T06Object)
}
const T06_OBJECT: u32 = 0xfa472;
fn t06_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 7) else {
        return x.wait(1, Step::T06Retry);
    };
    x.set_m8(f::DONE, 1);
    wr32(x.b, o + 0x4c, mode::BASE);
    wr8(x.b, o + 0x2f, 0);
    x.next_step();
    Flow::Done
}
const T06_RETRY: u32 = 0xfa486;

/// Step 8: waits for the object to finish, white flash.
fn t08_wait_flash(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    fading::PAGES.set(x.b, FADE_PAGES_ALL);
    crate::palette::full_brightness(x.b);
    x.wait(4, Step::T08Next)
}
const T08_NEXT: u32 = 0xfa4bc;

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
    wr32(x.b, set + 0x4c, mode::BASE);
    x.next_step();
    Flow::Done
}
const T0A_RETRY: u32 = 0xfa4fc;

/// Step 12: waits for the object, then starts the fade to black (task).
fn t0c_fade_task(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    x.next_step();
    fade_setup(x, 3, FADE_PAGES_ALL);
    start_fade_task(x, crate::scene::fading::DESCENT);
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
    x.set_v16(var::CAMERA_X, 0x200);
    x.set_m8(f::CUE, 0xff);
    x.render();
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, Step::T10Object)
}
const T10_OBJECT: u32 = 0xfa56e;
fn t10_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 7) else {
        return x.wait(1, Step::T10Retry);
    };
    x.set_m8(f::DONE, 1);
    wr32(x.b, o + 0x4c, mode::BASE);
    wr8(x.b, o + 0x2f, 1);
    x.next_step();
    Flow::Done
}
const T10_RETRY: u32 = 0xfa582;

/// Step 18: white flash, then loads the lower terrain (four strips of two tile blocks).
fn t12_wait(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    fading::PAGES.set(x.b, FADE_PAGES_ALL);
    crate::palette::full_brightness(x.b);
    x.wait(4, Step::T12Scenery)
}
const T12_SCENERY: u32 = 0xfa5ba;
const T12_FADE: u32 = 0xfa620;

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
    for off in [var::CAMERA_Y, var::CAMERA_2_Y] {
        let v = rd32(x.b, crate::mem::A5 + off).wrapping_add(delta as u32);
        wr32(x.b, crate::mem::A5 + off, v);
    }
    if x.v16(var::CAMERA_Y) < 0x300 {
        return Flow::Done;
    }
    x.set_v16(var::CAMERA_Y, 0x300);
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
    let mut src = LOGO_PALETTE;
    for dst in [0x91c2e0u32, 0x91c300, 0x91c360, 0x91c3e0] {
        for i in 0..8 {
            let v = rd32(x.b, src + 4 * i);
            wr32(x.b, dst + 4 * i, v);
        }
        src += 32;
    }
    fading::PAGES.set(x.b, 1);
    crate::palette::full_brightness(x.b);
    t18_object(x)
}
fn t18_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::T18Retry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 4);
    wr8(x.b, o + 3, 0x34);
    x.set_m16(f::DONE, o as u16);
    x.set_m16(f::TIMER, 80);
    x.wait(1, Step::T18Loop)
}
/// Logo palette: four consecutive 32-byte blocks in ROM.
const LOGO_PALETTE: u32 = 0x2538c0;
const T18_RETRY: u32 = 0xfa722;
const T18_LOOP: u32 = 0xfa746;
const T18_AFTER: u32 = 0xfa74a;
/// Subroutine `0xfa762` (called by `bsr`, returns at `T18_AFTER`): after 80 frames, creates the
/// text object and runs the original's script.
fn t18_sub(x: &mut Ctx) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        crate::tasks::return_to_caller(x.b);
        return Flow::Yield;
    }
    t18_text_object(x)
}
fn t18_text_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { return x.wait(1, Step::T18SubRetry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 0xf0);
    wr8(x.b, o + 0x2f, 0);
    wr32(x.b, o + 8, 0);
    wr16(x.b, o + 0xc, 0x3f0);
    wr32(x.b, o + 0x10, 1);
    let target = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    wr32(x.b, target + 4, 0x0600_0000);
    x.set_v16(var::SCROLL_2_Y, 0);
    x.set_v16(var::LAYERS, 0x12ce);
    crate::sound::play_global(x.b, ext::OBJECT_LAUNCH);
    Flow::Done
}
const T18_SUB_RETRY: u32 = 0xfa77c;

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
    crate::sound::play_global(x.b, ext::SOUND_E185C);
    t1a_object(x)
}
fn t1a_object(x: &mut Ctx) -> Flow {
    let Some(Obj(o)) = pools::SET.alloc(x.b) else { return x.wait(1, Step::T1aRetry) };
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 0x42);
    wr8(x.b, o + 0x2f, 3);
    x.set_m16(f::OBJECT, o as u16);
    Flow::Done
}
const T1A_RETRY: u32 = 0xfa7fc;

/// Step 28: waits for the logo object to reach state 6, then creates the texts (3 objects) and displays
/// the rights notice according to the region.
fn t1c_wait_logo(x: &mut Ctx) -> Flow {
    x.render();
    let o = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.next_step();
    title_text(x)
}
/// `0xfa830`: three text objects around the camera, then the rights text.
pub fn title_text(x: &mut Ctx) -> Flow {
    // three variants (0xe6, 0xe5, 0xe4, low byte first) placed from the active camera and the table
    // of offsets 0xfa8bc
    const OFFSETS: u32 = 0xfa8bc;
    let camera = crate::mem::A5 + if x.v8(var::SCREEN_TYPE) != 0 { 0x54e } else { 0x4fe };
    let mut variants = 0x00e4_e5e6u32;
    for k in 0..3 {
        let Some(Obj(o)) = pools::SOBJ.alloc(x.b) else { break };
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0);
        wr8(x.b, o + 3, variants as u8);
        variants >>= 8;
        let (px, py) = (rd16(x.b, camera + 8), rd16(x.b, camera + 12));
        let (dx, dy) = (rd16(x.b, OFFSETS + 4 * k), rd16(x.b, OFFSETS + 4 * k + 2));
        wr16(x.b, o + 8, px.wrapping_add(dx));
        wr16(x.b, o + 12, py.wrapping_add(dy));
        wr32(x.b, o + 0x10, 0);
    }
    x.wait(1, Step::T1cText)
}
const T1C_TEXT: u32 = 0xfa88e;

/// Step 30: after 300 frames, fade to black (task), clears everything and moves to the next sub-mode.
fn t1e_end(x: &mut Ctx) -> Flow {
    x.render();
    leave_screen(x, Step::T1eWait)
}
const T1E_FADED: u32 = 0xfa90e;

/// End of screen common to both titles: 300-frame timer, next sub-mode, fade
/// to black by the task, then cleanup in three phases.
fn leave_screen(x: &mut Ctx, loop_resume: Step) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::BASE + f::STEP, 0);
    fading::STEP.set(x.b, 1);
    fading::DELAY.set(x.b, 4);
    fading::PAGES.set(x.b, FADE_PAGES_ALL);
    start_fade_task(x, crate::scene::fading::DESCENT);
    x.wait(1, loop_resume)
}

/// Waits for the fade to finish (`A5+0x48f`), then clear and yield of 16 frames.
fn leave_wait(x: &mut Ctx, loop_resume: Step, cleared: Step, second_title: bool) -> Flow {
    x.render();
    if fading::STEP.get(x.b) != 0 {
        return x.wait(1, loop_resume);
    }
    // the original frees the pools in a different order depending on the screen
    use crate::objects::pools::{EFFCT, SET, SOBJ};
    let pools = if second_title { [&SET, &EFFCT, &SOBJ] } else { [&SOBJ, &SET, &EFFCT] };
    for pool in pools {
        pool.reset(x.b);
    }
    unlink_and_clear_lists(x);
    crate::video::clear_objects(x.b);
    if second_title {
        x.set_v8(var::SCREEN_TYPE, 0);
        x.and16(var::VIDEO_FLAGS, !1);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(16, cleared)
}
const T1E_CLEARED: u32 = 0xfa94e;
const T1E_DONE: u32 = 0xfa96e;

// ---------------------------------------------------------------------------------------------
// Sous-mode 12 : second titre

fn second_title_step(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0 => {
            crate::tasks::prompt::stop(x.b);
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
        x.set_v32(off, 0);
    }
    x.wait(4, Step::S02Load)
}
const S02_LOAD: u32 = 0xfaf5c;
fn s02_object(x: &mut Ctx) -> Flow {
    let Ok(Obj(o)) = scenery_6(x.b, 9) else {
        return x.wait(1, Step::S02Retry);
    };
    x.set_m8(f::CUE, 0);
    wr32(x.b, o + 0x4c, mode::BASE);
    x.set_m16(f::DONE, o as u16);
    x.render();
    x.wait(2, Step::S02Show)
}
const S02_RETRY: u32 = 0xfaff8;
const S02_SHOW: u32 = 0xfb012;
const S02_PALETTE: u32 = 0xfb030;

/// Clears the 512-byte sprite area (`0xfb18a`).
fn clear_sprite_area(x: &mut Ctx) {
    for i in 0..128 {
        wr32(x.b, SPRITE_CLEAR_AREA + 4 * i, 0);
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
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 0x44);
    wr8(x.b, o + 0x2f, 3);
    x.set_m16(f::OBJECT, o as u16);
    x.wait(1, Step::S04Show)
}
const S04_RETRY: u32 = 0xfb064;
const S04_SHOW: u32 = 0xfb088;

/// Step 6: waits for the logo object to be at state 6, then rights text (like title 1).
fn s06_wait_logo(x: &mut Ctx) -> Flow {
    x.render();
    let o = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.next_step();
    title_text(x)
}

const S08_WAIT: u32 = 0xfb106;
const S08_CLEARED: u32 = 0xfb152;
const S08_DONE: u32 = 0xfb182;

pub const NATIVES: &[Native] = &[];
