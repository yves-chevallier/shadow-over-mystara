use super::*;

/// State 4 ("dead"): the object is marked for deletion then released right away.
const DEAD: u32 = 0x0600_0000;
/// Palettes of the elements page (GFX RAM): 16 colors.
const PALETTE_A: u32 = 0x91c080;
const PALETTE_B: u32 = 0x91c3e0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x3) {
        0 => lifecycle(x, v0_init, v0_step),
        1 => lifecycle(x, v1_init, v1_step),
        2 => lifecycle(x, v2_init, v2_step),
        3 => match x.u8(0x4) {
            0 => v3_init(x),
            2 => v3_step(x),
            4 => {
                if signal(x) != 0 {
                    return kill(x);
                }
                x.reinsert();
            }
            _ => {}
        },
        4 => lifecycle(x, v4_init, v4_step),
        5 => lifecycle(x, v5_init, v5_step),
        6 => lifecycle(x, v6_init, v6_step),
        7 => lifecycle(x, v7_init, v7_step),
        8 => lifecycle(x, v8_init, v8_step),
        9 => lifecycle(x, v9_init, v9_step),
        10 => lifecycle(x, v10_init, v10_step),
        11 => lifecycle(x, v11_init, |_| {}),
        12 => lifecycle(x, v12_init, v12_step),
        13 => lifecycle(x, v13_init, |_| {}),
        14 => lifecycle(x, v14_init, v14_step),
        15 => lifecycle(x, v15_init, |_| {}),
        16 => lifecycle(x, v16_init, v16_step),
        17 => lifecycle(x, v17_init, v17_step),
        18 => lifecycle(x, v18_init, |_| {}),
        19 => lifecycle(x, v19_init, v19_step),
        20 => lifecycle(x, v20_init, v20_step),
        21 => lifecycle(x, v21_init, |_| {}),
        22 => lifecycle(x, v20_init, v20_fall),
        23 => lifecycle(x, v23_init, v23_follow_and_insert),
        _ => {}
    }
}

/// Common life cycle: 0 initialization, 2 alive, 4 dead, 6 release.
fn lifecycle<R: Bench>(x: &mut Cx<'_, R>, init: fn(&mut Cx<'_, R>), step: fn(&mut Cx<'_, R>)) {
    match x.u8(0x4) {
        0 => init(x),
        2 => step(x),
        4 => kill(x),
        6 => x.free(&pools::SOBJ),
        _ => {}
    }
}

fn kill<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(0x4, DEAD);
    x.free(&pools::SOBJ);
}

fn rom32<R: Bench>(x: &mut Cx<'_, R>, addr: u32) -> u32 {
    (x.rom16(addr) as u32) << 16 | x.rom16(addr + 2) as u32
}

/// The scene's master (`+0x4c`).
fn master<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    Obj(x.u32(0x4c) & 0xff_ffff)
}

/// Master's signal byte (`+8`).
fn signal<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let m = master(x);
    m.u8(x.b, 0x8)
}

fn send_signal<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    let m = master(x);
    m.set_u8(x.b, 0x8, v);
}

/// Starts script `n` and puts the object back in the actors' list.
fn start<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(ANIMS, n as u16);
    x.reinsert();
}

/// Creates a SOBJ object of type 6 (routine 0xddeaa), of variant `variant`; nothing on failure.
fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
    let child = x.alloc(&pools::SOBJ)?;
    child.set_u8(x.b, 0, 1);
    child.set_u8(x.b, 2, 6);
    child.set_u8(x.b, 3, variant);
    Some(child)
}

/// Scrolls the screen by one pixel every 8 frames (routine 0xdea4a) while keeping the object
/// still on screen.
fn scroll_creep<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(0x6f) & 7 != 0 {
        return;
    }
    let v = x.a5_u16(ram::CAMERA_X).wrapping_sub(1);
    x.set_a5_u16(ram::CAMERA_X, v);
    x.set_a5_u16(0x50e, v);
    let v = x.a5_u16(ram::CAMERA2_X).wrapping_add(1);
    x.set_a5_u16(ram::CAMERA2_X, v);
    x.set_a5_u16(0x55e, v);
    let (dx, dy) = camera_delta(x);
    x.add32(0x8, dx);
    x.add32(0x10, dy);
}

/// Variant 0: sprite tied to the master (animation set, flip indicator and number read from
/// a table with 8 bytes per value of `+0x2f`), random flip if the indicator is negative.
const V0_ENTRIES: u32 = 0xddf6c;

fn random_flip<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x2c, 0);
    let rng = x.random();
    if rng as u32 & 1 != 0 {
        x.set8(0x2c, 0x20);
    }
}

fn v0_init<R: Bench>(x: &mut Cx<'_, R>) {
    if (signal(x) as i8) < 0 {
        return x.free(&pools::SOBJ);
    }
    x.add8(0x4, 2);
    let e = V0_ENTRIES + 8 * x.u8(0x2f) as u32;
    let set = rom32(x, e);
    let flip = x.rom8(e + 4);
    x.set8(0x2c, flip);
    let n = x.rom16(e + 6) as u32;
    x.anim_start(set, n as u16);
    if (x.u8(0x2c) as i8) < 0 {
        random_flip(x);
    }
    x.reinsert();
}

fn v0_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if signal(x) as i8 >= 0 {
        return;
    }
    x.add8(0x4, 2);
}

/// Variant 1: light that slowly grows until the master's signal.
fn v1_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x8, 0x30_0000);
    x.set32(0xc, 0xd0_0000);
    x.set32(0x10, 0);
    start(x, 0xee);
    crate::sound::play(x.b, crate::sound::CMD_96, x.o);
}

fn v1_step<R: Bench>(x: &mut Cx<'_, R>) {
    if signal(x) != 0 {
        return kill(x);
    }
    x.add32(0x8, 0x8000);
    x.add32(0xc, 0x8000);
}

/// Variant 2: scrolling scenery, three phases (`+5`) paced by the master's signal.
fn v2_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set16(0x8, 0x100);
    x.set16(0xc, 0xc0);
    x.set32(0x10, 0);
    scroll_creep(x);
    start(x, 0xef);
}

fn v2_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            if signal(x) != 0 {
                x.add8(0x5, 2);
            }
        }
        2 => {
            scroll_creep(x);
            if signal(x) == 0 {
                x.add8(0x5, 2);
            }
        }
        4 => {
            if signal(x) != 0 {
                kill(x);
            }
        }
        _ => {}
    }
}

/// Variant 3: launches decorative SET objects from a byte list (0xde1e0, or
/// 0xde1e8 if `A5+0x38`), one every 30 frames, until the end of its script.
const V3_LISTS: u32 = 0xde1e0;

fn v3_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set16(0x8, 0x60);
    x.set16(0xc, 0xc0);
    x.set32(0x10, 0);
    x.set16(0x42, 0x1e);
    x.set32(0x44, V3_LISTS);
    if x.a5_u16(0x38) != 0 {
        x.add32(0x44, 8);
    }
    x.set16(0x48, 0);
    start(x, 0xf1);
    crate::sound::play(x.b, crate::sound::CMD_97, x.o);
}

fn v3_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x48) != 0 {
        let child = x.linked(0x48);
        if child.u8(x.b, 0x22) == 0 {
            return;
        }
        x.set16(0x48, 0);
    }
    let n = x.u16(0x42).wrapping_sub(1);
    x.set16(0x42, n);
    if n != 0 {
        return;
    }
    let mut list = x.u32(0x44);
    let kind = x.data8(list);
    list += 1;
    if kind != 0 {
        if let Some(set) = crate::families::creators::scene_set(x.b, kind) {
            x.set16(0x48, set.word());
        }
    }
    x.set32(0x44, list);
    x.anim_step();
    x.set16(0x42, 0x1e);
    if x.u16(0x32) == 0 {
        return;
    }
    x.set16(0x42, 0xa);
    x.add8(0x4, 2);
    send_signal(x, 1);
}

/// Variant 4: rises toward the top of the screen (Z - 1.25 per frame) and stops at Z = 0x20.
fn v4_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x10, 0x4000);
    x.set16(0x8, 5);
    x.set16(0xc, 0x98);
    start(x, 0xf5);
}

fn v4_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.add32(0xc, 0u32.wrapping_sub(0x14000));
    if x.u16(0xc) as i16 > 0x20 {
        return;
    }
    x.add8(0x4, 2);
    x.set8(0x5, 0);
}

/// Variant 5: leader that descends, triggers the scrolling then creates variants 0 (two at
/// most; X then Z offsets in table 0xde386).
const V5_OFFSETS: u32 = 0xde386;

fn v5_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x10, 0x8000);
    x.set16(0x8, 0x78);
    x.set16(0xc, 0x90);
    start(x, 0xf6);
}

fn v5_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            x.add32(0xc, 0x10000);
            if x.u16(0xc) < 0xd0 {
                return;
            }
            x.set16(0xc, 0xd0);
            x.add8(0x5, 2);
        }
        2 => {
            let y = x.a5_u32(ram::CAMERA2_Y).wrapping_add(0x10000);
            x.set_a5_u32(ram::CAMERA2_Y, y);
            x.set_a5_u16(0x562, (y >> 16) as u16);
            if ((y >> 16) as u16) < 0x28 {
                return;
            }
            x.add8(0x5, 2);
            send_signal(x, 1);
            x.set16(0x42, 1);
        }
        4 => {
            if signal(x) as i8 >= 0 {
                return v5_spawn(x);
            }
            x.add8(0x4, 2);
            x.set8(0x5, 0);
        }
        _ => {}
    }
}

fn v5_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(0x43).wrapping_sub(1);
    x.set8(0x43, n);
    if n != 0 || x.u8(0x42) == 2 {
        return;
    }
    x.set8(0x43, 0x30);
    let Some(child) = spawn(x, 0) else { return };
    let which = x.u8(0x42);
    child.set_u8(x.b, 0x2f, which);
    for off in [0x4c, 0x8, 0xc, 0x10] {
        let v = x.u32(off);
        child.set_u32(x.b, off, v);
    }
    let at = V5_OFFSETS + 4 * which as u32;
    let dx = x.rom16(at);
    let dz = x.rom16(at + 2);
    let v = child.u16(x.b, 0x8).wrapping_add(dx);
    child.set_u16(x.b, 0x8, v);
    let v = child.u16(x.b, 0xc).wrapping_add(dz);
    child.set_u16(x.b, 0xc, v);
    let v = child.u32(x.b, 0x10).wrapping_sub(1);
    child.set_u32(x.b, 0x10, v);
    x.add8(0x42, 1);
}

/// Variant 6: letter that falls from the top (Z 0x300) down to its stop height (table 0xde3ee:
/// X, stop), waits for signal 2 then rises by 0x290.
const V6_TABLE: u32 = 0xde3ee;

fn v6_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x10, 0xf000);
    x.set32(0xc, 0x300_0000);
    let v = x.u8(0x2f) as u32;
    let shift = (v << 8) & 0xffff;
    let y = x.u32(0x10).wrapping_sub(shift);
    x.set32(0x10, y);
    let px = x.rom16(V6_TABLE + 4 * v);
    x.set16(0x8, px);
    let stop = x.rom16(V6_TABLE + 4 * v + 2);
    x.set16(0x14, stop);
    x.anim_start(ANIMS, (v + 0xfe) as u16);
    x.reinsert();
}

fn v6_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            x.add16(0xc, 2);
            let stop = x.u16(0x14);
            if stop <= x.u16(0xc) {
                x.set16(0xc, stop);
                x.add8(0x5, 2);
                let m = master(x);
                let v = m.u8(x.b, 0xd).wrapping_sub(1);
                m.set_u8(x.b, 0xd, v);
            }
        }
        2 => {
            if signal(x) == 2 {
                x.add16(0xc, 0u16.wrapping_sub(0x290));
                x.add8(0x5, 2);
            }
        }
        4 => {
            if (signal(x) as i8) < 0 {
                x.add8(0x4, 2);
            }
        }
        _ => {}
    }
    x.reinsert();
}

/// Variant 7: palette fade (page B) then a 180-frame wait; also loads a transition
/// script (`A5+0x5c6`...). Script chosen by `+0x2f` and by `A5+0x72`.
const V7_SCRIPTS: u32 = 0xde4d2;
const V7_PALETTES: u32 = 0x292240;

fn v7_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set16(0x42, 6);
    let mut off = 4 * x.u8(0x2f) as u32;
    if x.a5_u16(0x72) != 2 {
        off += 8;
    }
    let script = rom32(x, V7_SCRIPTS + off);
    let w = x.data16(script);
    x.set_a5_u16(0x5ca, w);
    let w = x.data16(script + 2);
    x.set_a5_u16(0x5cc, w);
    x.set_a5_u32(0x5c6, script + 4);
    copy_longs(x, V7_PALETTES, PALETTE_B, 8);
    x.set_a5_u8(0x48e, 1);
    crate::palette::no_brightness(x.b);
}

fn v7_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        if rd8(x.b, PALETTE_B) & 0xf0 == 0xf0 {
            x.set16(0x42, 0xb4);
            x.add8(0x5, 2);
            return;
        }
        let n = x.u16(0x42).wrapping_sub(1);
        x.set16(0x42, n);
        if n != 0 {
            return;
        }
        x.set16(0x42, 6);
        for i in 0..8 {
            let v = rd32(x.b, PALETTE_B + 4 * i).wrapping_add(0x1000_1000);
            wr32(x.b, PALETTE_B + 4 * i, v);
        }
    } else {
        let n = x.u16(0x42).wrapping_sub(1);
        x.set16(0x42, n);
        if n != 0 {
            return;
        }
        x.add8(0x4, 2);
        let m = master(x);
        let v = m.u8(x.b, 0xc).wrapping_sub(1);
        m.set_u8(x.b, 0xc, v);
        x.set_a5_u32(0x5c6, 0);
    }
}

mod later;
use later::*;

