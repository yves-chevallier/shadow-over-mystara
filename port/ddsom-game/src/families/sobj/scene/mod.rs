use super::*;
use crate::families::shell::vars;
use crate::ram;
use crate::families::shell::records::{Base, SetView, view};
use crate::families::shell::tables::{held16, held8, tables, Read};
use crate::data::RomAddr;

/// State 4 ("dead"): the object is marked for deletion then released right away.
const DEAD: u32 = 0x0600_0000;
/// Palettes of the elements page (GFX RAM): 16 colors.
const PALETTE_A: u32 = 0x91c080;
const PALETTE_B: u32 = 0x91c3e0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::VARIANT) {
        0 => lifecycle(x, v0_init, v0_step),
        1 => lifecycle(x, v1_init, v1_step),
        2 => lifecycle(x, v2_init, v2_step),
        3 => match x.u8(Obj::STATE) {
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
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => step(x),
        4 => kill(x),
        6 => x.free(&pools::SOBJ),
        _ => {}
    }
}

fn kill<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(Obj::STATE, DEAD);
    x.free(&pools::SOBJ);
}

/// The scene's master (`+0x4c`).
fn master<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    Obj(x.u32(Element::MASTER) & 0xff_ffff)
}

/// Master's signal byte (`+8`).
fn signal<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let m = master(x);
    m.u8(x.b, Obj::X)
}

fn send_signal<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    let m = master(x);
    m.set_u8(x.b, Obj::X, v);
}

/// Starts script `n` and puts the object back in the actors' list.
fn start<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(ANIMS.0, n as u16);
    x.reinsert();
}

/// Creates a SOBJ object of type 6 (routine 0xddeaa), of variant `variant`; nothing on failure.
fn spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
    let child = x.alloc(&pools::SOBJ)?;
    child.set_u8(x.b, Obj::ACTIVE, 1);
    child.set_u8(x.b, Obj::KIND, 6);
    child.set_u8(x.b, Obj::VARIANT, variant);
    Some(child)
}

/// Scrolls the screen by one pixel every 8 frames (routine 0xdea4a) while keeping the object
/// still on screen.
fn scroll_creep<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::FRAME_BYTE.get(x.b) & 7 != 0 {
        return;
    }
    let v = ram::CAMERA_X.get(x.b).wrapping_sub(1);
    ram::CAMERA_X.set(x.b, v);
    vars::CAMERA_TARGET_X_WORD.set(x.b, v);
    let v = ram::CAMERA_ALT_X.get(x.b).wrapping_add(1);
    ram::CAMERA_ALT_X.set(x.b, v);
    vars::CAMERA_ALT_TARGET_X.set(x.b, v);
    let (dx, dy) = camera_delta(x);
    x.add32(Obj::X, dx);
    x.add32(Obj::Y, dy);
}


fn random_flip<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Base::FLIP, 0);
    let rng = x.random();
    if rng as u32 & 1 != 0 {
        x.set8(Base::FLIP, 0x20);
    }
}

/// Variant 0: sprite tied to the master (animation set, flip indicator and number read from
/// a table with 8 bytes per value of `+0x2f`), random flip if the indicator is negative.
fn v0_init<R: Bench>(x: &mut Cx<'_, R>) {
    if (signal(x) as i8) < 0 {
        return x.free(&pools::SOBJ);
    }
    x.add8(Obj::STATE, 2);
    let e = 8 * x.u8(Base::SETTING) as u32;
    let set = tables().sobj_scene_v0_entries.at32(x.b, e);
    let flip = tables().sobj_scene_v0_entries.at8(x.b, e + 4);
    x.set8(Base::FLIP, flip);
    let n = tables().sobj_scene_v0_entries.at16(x.b, e + 6) as u32;
    x.anim_start(set, n as u16);
    if (x.u8(Base::FLIP) as i8) < 0 {
        random_flip(x);
    }
    x.reinsert();
}

fn v0_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if signal(x) as i8 >= 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
}

/// Variant 1: light that slowly grows until the master's signal.
fn v1_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::X, 0x30_0000);
    x.set32(Obj::Z, 0xd0_0000);
    x.set32(Obj::Y, 0);
    start(x, 0xee);
    crate::sound::play(x.b, crate::sound::CMD_96, x.o);
}

fn v1_step<R: Bench>(x: &mut Cx<'_, R>) {
    if signal(x) != 0 {
        return kill(x);
    }
    x.add32(Obj::X, 0x8000);
    x.add32(Obj::Z, 0x8000);
}

/// Variant 2: scrolling scenery, three phases (`+5`) paced by the master's signal.
fn v2_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set16(Obj::X, 0x100);
    x.set16(Obj::Z, 0xc0);
    x.set32(Obj::Y, 0);
    scroll_creep(x);
    start(x, 0xef);
}

fn v2_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            if signal(x) != 0 {
                x.add8(Base::SUB_STATE, 2);
            }
        }
        2 => {
            scroll_creep(x);
            if signal(x) == 0 {
                x.add8(Base::SUB_STATE, 2);
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
fn v3_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set16(Obj::X, 0x60);
    x.set16(Obj::Z, 0xc0);
    x.set32(Obj::Y, 0);
    x.set16(Element::TIMER, 0x1e);
    // the list is designated by its address (`sobj_scene_v3_lists`)
    x.set32(Element::LIST, tables().sobj_scene_v3_lists.rom);
    if vars::ALT_LISTS.get(x.b) != 0 {
        x.add32(Element::LIST, 8);
    }
    x.set16(Element::CHILD, 0);
    start(x, 0xf1);
    crate::sound::play(x.b, crate::sound::CMD_97, x.o);
}

fn v3_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Element::CHILD) != 0 {
        let child = x.linked(Element::CHILD);
        if child.u8(x.b, SetView::FINISHED) == 0 {
            return;
        }
        x.set16(Element::CHILD, 0);
    }
    let n = x.u16(Element::TIMER).wrapping_sub(1);
    x.set16(Element::TIMER, n);
    if n != 0 {
        return;
    }
    let mut list = x.u32(Element::LIST);
    let kind = held8(x.b, list);
    list += 1;
    if kind != 0 {
        if let Some(set) = crate::families::creators::scene_set(x.b, kind) {
            x.set16(Element::CHILD, set.word());
        }
    }
    x.set32(Element::LIST, list);
    x.anim_step();
    x.set16(Element::TIMER, 0x1e);
    if x.u16(Obj::ANIM_FLAGS) == 0 {
        return;
    }
    x.set16(Element::TIMER, 0xa);
    x.add8(Obj::STATE, 2);
    send_signal(x, 1);
}

/// Variant 4: rises toward the top of the screen (Z - 1.25 per frame) and stops at Z = 0x20.
fn v4_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::Y, 0x4000);
    x.set16(Obj::X, 5);
    x.set16(Obj::Z, 0x98);
    start(x, 0xf5);
}

fn v4_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.add32(Obj::Z, 0u32.wrapping_sub(0x14000));
    if x.u16(Obj::Z) as i16 > 0x20 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
}


/// Variant 5: leader that descends, triggers the scrolling then creates variants 0 (two at
/// most; X then Z offsets in table 0xde386).
fn v5_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::Y, 0x8000);
    x.set16(Obj::X, 0x78);
    x.set16(Obj::Z, 0x90);
    start(x, 0xf6);
}

fn v5_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            x.add32(Obj::Z, 0x10000);
            if x.u16(Obj::Z) < 0xd0 {
                return;
            }
            x.set16(Obj::Z, 0xd0);
            x.add8(Base::SUB_STATE, 2);
        }
        2 => {
            let y = vars::CAMERA_ALT_Y_LONG.get(x.b).wrapping_add(0x10000);
            vars::CAMERA_ALT_Y_LONG.set(x.b, y);
            vars::CAMERA_ALT_TARGET_Y.set(x.b, (y >> 16) as u16);
            if ((y >> 16) as u16) < 0x28 {
                return;
            }
            x.add8(Base::SUB_STATE, 2);
            send_signal(x, 1);
            x.set16(Element::TIMER, 1);
        }
        4 => {
            if signal(x) as i8 >= 0 {
                return v5_spawn(x);
            }
            x.add8(Obj::STATE, 2);
            x.set8(Base::SUB_STATE, 0);
        }
        _ => {}
    }
}

fn v5_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(Element::DELAY).wrapping_sub(1);
    x.set8(Element::DELAY, n);
    if n != 0 || x.u8(Element::WHICH) == 2 {
        return;
    }
    x.set8(Element::DELAY, 0x30);
    let Some(child) = spawn(x, 0) else { return };
    let which = x.u8(Element::WHICH);
    child.set_u8(x.b, Base::SETTING, which);
    for off in [0x4c, 0x8, 0xc, 0x10] {
        let v = x.u32(off);
        child.set_u32(x.b, off, v);
    }
    let at = 4 * which as u32;
    let dx = tables().sobj_scene_v5_offsets.at16(x.b, at);
    let dz = tables().sobj_scene_v5_offsets.at16(x.b, at + 2);
    let v = child.u16(x.b, Obj::X).wrapping_add(dx);
    child.set_u16(x.b, Obj::X, v);
    let v = child.u16(x.b, Obj::Z).wrapping_add(dz);
    child.set_u16(x.b, Obj::Z, v);
    let v = child.u32(x.b, Obj::Y).wrapping_sub(1);
    child.set_u32(x.b, Obj::Y, v);
    x.add8(Element::WHICH, 1);
}


/// Variant 6: letter that falls from the top (Z 0x300) down to its stop height (table 0xde3ee:
/// X, stop), waits for signal 2 then rises by 0x290.
fn v6_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::Y, 0xf000);
    x.set32(Obj::Z, 0x300_0000);
    let v = x.u8(Base::SETTING) as u32;
    let shift = (v << 8) & 0xffff;
    let y = x.u32(Obj::Y).wrapping_sub(shift);
    x.set32(Obj::Y, y);
    let px = tables().sobj_scene_v6_table.at16(x.b, 4 * v);
    x.set16(Obj::X, px);
    let stop = tables().sobj_scene_v6_table.at16(x.b, 4 * v + 2);
    x.set16(Element::STOP, stop);
    x.anim_start(ANIMS.0, (v + 0xfe) as u16);
    x.reinsert();
}

fn v6_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            x.add16(Obj::Z, 2);
            let stop = x.u16(Element::STOP);
            if stop <= x.u16(Obj::Z) {
                x.set16(Obj::Z, stop);
                x.add8(Base::SUB_STATE, 2);
                let m = master(x);
                let v = m.u8(x.b, Element::PENDING).wrapping_sub(1);
                m.set_u8(x.b, Element::PENDING, v);
            }
        }
        2 => {
            if signal(x) == 2 {
                x.add16(Obj::Z, 0u16.wrapping_sub(0x290));
                x.add8(Base::SUB_STATE, 2);
            }
        }
        4 => {
            if (signal(x) as i8) < 0 {
                x.add8(Obj::STATE, 2);
            }
        }
        _ => {}
    }
    x.reinsert();
}

/// Variant 7: palette fade (page B) then a 180-frame wait; also loads a transition
/// script (`A5+0x5c6`...). Script chosen by `+0x2f` and by `A5+0x72`.
const V7_PALETTES: RomAddr = RomAddr(0x292240);

fn v7_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set16(Element::TIMER, 6);
    let mut off = 4 * x.u8(Base::SETTING) as u32;
    if vars::CABINET_PLAYERS.get(x.b) != 2 {
        off += 8;
    }
    let script = tables().sobj_scene_v7_scripts.at32(x.b, off);
    let w = held16(x.b, script);
    vars::SCENE_SCRIPT_A.set(x.b, w);
    let w = held16(x.b, script + 2);
    vars::SCENE_SCRIPT_B.set(x.b, w);
    vars::SCENE_SCRIPT.set(x.b, script + 4);
    copy_longs(x, V7_PALETTES, PALETTE_B, 8);
    vars::FADE_PAGES.set(x.b, 1);
    crate::palette::no_brightness(x.b);
}

fn v7_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SUB_STATE) == 0 {
        if rd8(x.b, PALETTE_B) & 0xf0 == 0xf0 {
            x.set16(Element::TIMER, 0xb4);
            x.add8(Base::SUB_STATE, 2);
            return;
        }
        let n = x.u16(Element::TIMER).wrapping_sub(1);
        x.set16(Element::TIMER, n);
        if n != 0 {
            return;
        }
        x.set16(Element::TIMER, 6);
        for i in 0..8 {
            let v = rd32(x.b, PALETTE_B + 4 * i).wrapping_add(0x1000_1000);
            wr32(x.b, PALETTE_B + 4 * i, v);
        }
    } else {
        let n = x.u16(Element::TIMER).wrapping_sub(1);
        x.set16(Element::TIMER, n);
        if n != 0 {
            return;
        }
        x.add8(Obj::STATE, 2);
        let m = master(x);
        let v = m.u8(x.b, Obj::Z).wrapping_sub(1);
        m.set_u8(x.b, Obj::Z, v);
        vars::SCENE_SCRIPT.set(x.b, 0);
    }
}

pub mod later;
use later::*;

view! {
    /// Fields of the scene elements (type 6).
    Element {
        /// Variant 6: height where the letter stops.
        stop, set_stop, STOP: u16 @ 0x14;
        /// Letters still falling, counted down by them (h).
        pending, set_pending, PENDING: u8 @ 0xd;
        /// Countdown of the current step.
        timer, set_timer, TIMER: u16 @ 0x42;
        /// Variant 5: number of elements created.
        which, set_which, WHICH: u8 @ 0x42;
        /// Variant 5: delay before the next element (h).
        delay, set_delay, DELAY: u8 @ 0x43;
        /// Variant 3: address of the next byte of its list.
        list, set_list, LIST: u32 @ 0x44;
        /// Child SET object (16-bit pointer).
        child, set_child, CHILD: u16 @ 0x48;
        /// Master object (address).
        master, set_master, MASTER: u32 @ 0x4c;
    }
}
