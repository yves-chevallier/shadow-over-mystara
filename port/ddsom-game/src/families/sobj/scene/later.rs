//! Scene elements, variants 8 to 23 (see the parent module).

use super::*;
use crate::data::RomAddr;
use crate::ram;
use crate::families::shell::records::{Base, view};
use crate::families::shell::tables::{tables, Read};

/// Variant 8: bar that descends (Z + 8 per frame) to the bottom of the screen, then raises the byte
/// 0xff0eaa (A5-0x7156).
pub(super) const SCENE_DONE_FLAG: u32 = 0xff_0eaa;

pub(super) fn v8_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let v = ram::CAMERA_X.get(x.b);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_Y.get(x.b);
    x.set16(Obj::Z, v);
    x.add16(Obj::Z, 0x20);
    x.set32(Obj::Y, 0);
    x.add16(Obj::Y, 0u16.wrapping_sub(0x20));
    start(x, 0x112);
}

pub(super) fn v8_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(Obj::Z, 8);
    let below = x.u16(Obj::Z).wrapping_add(x.u16(Obj::Y)).wrapping_sub(ram::CAMERA_Y.get(x.b));
    if below < 0x1c8 {
        return;
    }
    let y = ram::CAMERA_Y.get(x.b).wrapping_add(0x1c8);
    x.set16(Obj::Z, y);
    crate::mem::wr8(x.b, SCENE_DONE_FLAG, 0xff);
    x.add8(Obj::STATE, 2);
    x.reinsert();
}

/// Variant 9: animated text on the secondary camera; signals its end to the master.
pub(super) fn v9_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let v = ram::CAMERA_ALT_X.get(x.b);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_ALT_Y.get(x.b);
    x.set16(Obj::Z, v);
    x.set32(Obj::Y, 0);
    x.add16(Obj::Z, 0xf0);
    start(x, 0x113);
    crate::sound::play(x.b, crate::sound::CMD_8E, x.o);
}

pub(super) fn v9_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() || (signal(x) as i8) < 0 {
        return x.set8(Obj::STATE, 6);
    }
    if x.u16(Obj::ANIM_TIMER) != 0x7fff {
        return;
    }
    send_signal(x, 1);
    x.set16(Obj::ANIM_TIMER, 6);
}

/// Variant 10: plate that descends in two phases (Z 0x88 then 0xa0), waits for signal 2,
/// rises back to Z 0x80 and waits for the end.
pub(super) fn v10_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::X, 0x98_0000);
    x.set32(Obj::Z, 0x70_0000);
    x.set32(Obj::Y, 0);
    start(x, 0x114);
    crate::sound::play(x.b, crate::sound::CMD_3C, x.o);
}

pub(super) fn v10_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::SUB_STATE) {
        0 => {
            x.add32(Obj::Z, 0x4000);
            if x.u16(Obj::Z) < 0x88 {
                return;
            }
            x.set16(Obj::Z, 0x88);
            send_signal(x, 1);
            x.add8(Base::SUB_STATE, 2);
        }
        2 => {
            x.add32(Obj::Z, 0x4000);
            if x.u16(Obj::Z) < 0xa0 {
                return;
            }
            x.set16(Obj::Z, 0xa0);
            x.add8(Base::SUB_STATE, 2);
        }
        4 => {
            if signal(x) != 2 {
                return;
            }
            x.add8(Base::SUB_STATE, 2);
            x.set8(Late::TIMER, 0x1e);
        }
        6 => {
            let n = x.u8(Late::TIMER).wrapping_sub(1);
            x.set8(Late::TIMER, n);
            if n != 0 {
                return;
            }
            x.add8(Base::SUB_STATE, 2);
            crate::sound::play(x.b, crate::sound::CMD_40, x.o);
        }
        8 => {
            x.add32(Obj::Z, 0u32.wrapping_sub(0x4000));
            if x.u16(Obj::Z) as i16 > 0x80 {
                return;
            }
            x.set16(Obj::Z, 0x80);
            send_signal(x, 1);
            x.add8(Base::SUB_STATE, 2);
        }
        10 => {
            if (signal(x) as i8) < 0 {
                x.add8(Obj::STATE, 2);
            }
        }
        _ => {}
    }
}

/// Variant 11: fixed sprite on the main camera.
pub(super) fn v11_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::Y, 0x8000);
    let y = ram::CAMERA_Y.get(x.b);
    x.set16(Obj::Z, y);
    x.add16(Obj::Z, 0xc0);
    x.set16(Obj::X, 0x30);
    start(x, 0x115);
    crate::sound::play(x.b, crate::sound::CMD_3E, x.o);
}

/// Variant 12: sprite that crosses the screen (1 pixel per frame to the right, or to the
/// left if `+0x2f`) and moves the camera back.
pub(super) fn v12_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::FLIP, 0);
    x.set32(Obj::X, 0xf0_0000);
    let y = ram::CAMERA_ALT_Y.get(x.b);
    x.set16(Obj::Z, y);
    x.set16(Obj::VX, 0x100);
    x.add16(Obj::Z, 0xc0);
    x.set32(Obj::Y, 0);
    if x.u8(Base::SETTING) != 0 {
        x.set8(Base::FLIP, 0x20);
        let v = x.u16(Obj::VX).wrapping_neg();
        x.set16(Obj::VX, v);
        x.add32(Obj::X, 0u32.wrapping_sub(0x60_0000));
    }
    x.anim_start(ANIMS.0, 0x116);
    x.reinsert();
}

pub(super) fn v12_step<R: Bench>(x: &mut Cx<'_, R>) {
    let d = (((x.u16(Obj::VX) as u32) << 16) as i32 >> 8) as u32;
    x.add32(Obj::X, d);
    let screen = x.u16(Obj::X).wrapping_sub(ram::CAMERA_X.get(x.b));
    if screen >= 0x180 {
        x.set8(Obj::STATE, 4);
    }
    if x.u8(Base::SETTING) != 0 {
        return;
    }
    let cam = ram::CAMERA_X.get(x.b);
    if cam != 0 {
        ram::CAMERA_X.set(x.b, cam - 1);
    }
}

/// Variant 13: frame whose script is a direct entry (16 bytes per `+0x2f` starting from
/// 0xdea84), on a palette background 0x91c340 (8 identical colors).
pub(super) const V13_SCRIPTS: RomAddr = RomAddr(0xdea84);

pub(super) fn v13_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let v = ram::CAMERA_X.get(x.b);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_Y.get(x.b);
    x.set16(Obj::Z, v);
    x.set32(Obj::Y, 0);
    for i in 0..8 {
        wr32(x.b, 0x91c340 + 4 * i, 0xf111_f111);
    }
    x.add16(Obj::X, 0x10);
    x.add16(Obj::Z, 0x50);
    let v = x.u8(Base::SETTING) as u16;
    x.add16(Obj::Z, v << 3);
    x.add16(Obj::Z, v << 4);
    x.anim_start_entry(V13_SCRIPTS.offset(16 * v as i32).0);
    x.reinsert();
}


/// Variant 14: badge whose sprite and position depend on `+0x46` (number, or object
/// whose variant is taken if bit 15 is raised); 8-byte table at 0xdef1e.
pub(super) fn v14_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let mut which = x.u16(Late::WHICH);
    if (which as i16) < 0 {
        let owner = x.linked(Late::WHICH);
        which = owner.u8(x.b, Obj::VARIANT) as u16;
    }
    x.set32(Obj::Z, 0);
    let row = (which << 3) as i16 as i32 as u32;
    let v = tables().sobj_scene_v14_table.at16(x.b, row);
    x.set16(Obj::X, v);
    let v = tables().sobj_scene_v14_table.at16(x.b, row + 2);
    x.set16(Obj::Y, v);
    let v = tables().sobj_scene_v14_table.at8(x.b, row + 6);
    x.set8(Base::FLIP, v);
    x.anim_start(ANIMS.0, (0x190 + which as u32) as u16);
    x.reinsert();
}

pub(super) fn v14_step<R: Bench>(x: &mut Cx<'_, R>) {
    if (signal(x) as i8) < 0 {
        x.add8(Obj::STATE, 2);
    }
}

pub(super) const V15_PALETTES: RomAddr = RomAddr(0x28f960);

/// Variant 15: colored pennant (palette 0x28f960, one 32-byte row per number drawn from
/// table 0xdefa4), position in table 0xdf01a.
pub(super) fn v15_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
    x.set32(Obj::Y, 0);
    let v = x.u8(Base::SETTING) as u32;
    let line = tables().sobj_scene_v15_palette_select.at8(x.b, v) as u32;
    copy_longs(x, V15_PALETTES.offset((line << 5) as i32), PALETTE_A, 8);
    or_longs(x, PALETTE_A, BRIGHT, 8);
    let px = tables().sobj_scene_v15_positions.at16(x.b, 4 * v);
    x.set16(Obj::X, px);
    let py = tables().sobj_scene_v15_positions.at16(x.b, 4 * v + 2);
    x.set16(Obj::Z, py);
    start(x, 0x19c + v);
}

/// Variant 16: randomly flipped sprite, which lives as long as the master has not finished and its
/// animation is not complete.
pub(super) fn v16_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    random_flip(x);
    start(x, 0x1a2);
    crate::sound::play(x.b, crate::sound::CMD_3D, x.o);
}

pub(super) fn v16_step<R: Bench>(x: &mut Cx<'_, R>) {
    if signal(x) as i8 >= 0 && !x.anim_step() {
        return;
    }
    x.add8(Obj::STATE, 2);
}

/// Variant 17: bubble that comes out of the bottom of the screen (two positions according to `+0x2f`); at the end
/// of the animation, gives bit (`+0x2f` modulo 8) of `+0x30` back to the owner `+0x46`.
pub(super) fn v17_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set32(Obj::Y, 0x4000);
    x.set16(Obj::Z, 0x1c0);
    x.set32(Obj::X, 0);
    if x.u8(Base::SETTING) == 0 {
        x.set16(Obj::X, 0x20);
        x.set32(Obj::Y, 0x2000);
    }
    // `move.w #0x1a3,D0w ; add.b (0x2f,A0),D0b`: the addition does not overflow into the high byte
    let n = 0x100 | (0xa3 + x.u8(Base::SETTING) as u32) & 0xff;
    start(x, n);
    crate::sound::play(x.b, crate::sound::CMD_3F, x.o);
}

pub(super) fn v17_step<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.anim_step() {
        return;
    }
    let owner = x.linked(Late::WHICH);
    let bit = x.u8(Base::SETTING) & 7;
    let v = owner.u8(x.b, Late::OWNER_BITS) & !(1 << bit);
    owner.set_u8(x.b, Late::OWNER_BITS, v);
    x.add8(Obj::STATE, 2);
}

/// Variant 18: sprite placed on the secondary camera.
pub(super) fn v18_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let v = ram::CAMERA_ALT_X.get(x.b);
    x.set16(Obj::X, v);
    let v = ram::CAMERA_ALT_Y.get(x.b);
    x.set16(Obj::Z, v);
    x.set32(Obj::Y, 0);
    let dz = if x.u8(Base::SETTING) != 0 { 0x50 } else { 0x100 };
    x.add16(Obj::Z, dz);
    x.anim_start(ANIMS.0, 0xec);
    x.reinsert();
}

pub(super) const V19_PALETTES: RomAddr = RomAddr(0x28f340 + 0x60);

/// Variant 19: palette dot (0x28f340 + 0x60 + 32 per row, row from table
/// 0xdf182) placed on the camera; it disappears when the animation of its carrier (`+0x46`)
/// is finished.
pub(super) fn v19_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let v = x.u8(Base::SETTING) as u32;
    let line = tables().sobj_scene_v19_lines.at8(x.b, 2 * v) as u32;
    let flip = tables().sobj_scene_v19_lines.at8(x.b, 2 * v + 1);
    x.set8(Base::FLIP, flip);
    copy_longs(x, V19_PALETTES.offset((line << 5) as i32), PALETTE_B, 8);
    or_longs(x, PALETTE_B, BRIGHT, 8);
    let anim = if line == 3 { 0 } else { line };
    start(x, 0x1a8 + anim);
    v19_place(x);
}

/// Places the dot from the table, relative to the current camera.
pub(super) fn v19_place<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(Base::SETTING) as u32;
    let px = tables().sobj_scene_v19_positions.at16(x.b, 4 * v);
    x.set16(Obj::X, px);
    let py = tables().sobj_scene_v19_positions.at16(x.b, 4 * v + 2);
    x.set16(Obj::Z, py);
    x.set32(Obj::Y, 0);
    let (cx, cy) = if ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 {
        (ram::CAMERA_ALT_X.get(x.b), ram::CAMERA_ALT_Y.get(x.b))
    } else {
        (ram::CAMERA_X.get(x.b), ram::CAMERA_Y.get(x.b))
    };
    x.add16(Obj::X, cx);
    x.add16(Obj::Z, cy);
}

pub(super) fn v19_step<R: Bench>(x: &mut Cx<'_, R>) {
    v19_place(x);
    let owner = x.linked(Late::WHICH);
    if owner.u16(x.b, Obj::ANIM_FLAGS) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    kill(x);
}


/// Variants 20 and 22: debris falling from a randomly drawn point below the camera's bottom edge (8 abscissas
/// in table 0xdf2a8, 4 acceleration/animation pairs at 0xdf2ea).
pub(super) fn v20_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    let v = ram::CAMERA_Y.get(x.b);
    x.set16(Obj::Z, v);
    let v = ram::CAMERA_X.get(x.b);
    x.set16(Obj::X, v);
    x.add16(Obj::Z, 0x110);
    x.set16(Obj::Y, 0x10);
    let rng = x.random();
    let k = rng as u32 & 7;
    let dx = tables().sobj_scene_v20_x.at16(x.b, 2 * k);
    x.add16(Obj::X, dx);
    x.set32(Obj::VX, 0);
    x.set16(Obj::VZ, 0);
    let rng = x.random();
    let k = rng as u32 & 3;
    let az = tables().sobj_scene_v20_fall.at16(x.b, 4 * k);
    x.set16(Obj::AZ, az);
    let anim = tables().sobj_scene_v20_fall.at16(x.b, 4 * k + 2) as u32;
    x.anim_start(ANIMS.0, anim as u16);
    x.reinsert();
}

pub(super) fn v20_step<R: Bench>(x: &mut Cx<'_, R>) {
    if (signal(x) as i8) < 0 {
        return kill(x);
    }
    v20_fall(x);
}

/// Fall: leaves (dead state) when the bottom of the sprite goes back above the top edge of the screen.
pub(super) fn v20_fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    let below = x.u16(Obj::Z).wrapping_add(x.u16(Obj::Y));
    if (below as i16) <= ram::CAMERA_Y.get(x.b) as i16 {
        kill(x);
    }
}


/// Variant 21: like 15, with the palette row designated by `+0x2f / 2` (table 0xdf332),
/// without brightening; positions in table 0xdf396.
pub(super) fn v21_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::SUB_STATE, 0);
    x.set32(Obj::Y, 0);
    let v = x.u8(Base::SETTING) as u32;
    let line = tables().sobj_scene_v21_palette_select.at8(x.b, v >> 1) as u32;
    copy_longs(x, V15_PALETTES.offset((line << 5) as i32), PALETTE_A, 8);
    let px = tables().sobj_scene_v21_positions.at16(x.b, 4 * v);
    x.set16(Obj::X, px);
    let py = tables().sobj_scene_v21_positions.at16(x.b, 4 * v + 2);
    x.set16(Obj::Z, py);
    start(x, 0x19c + (v >> 1));
}

/// Variant 23: spark stuck to the corner of the camera structure (A5+0x4fe, or 0x54e if
/// `A5+0x36`), whose color of palette 0x91c180 takes the brightness of color 0.
pub(super) const V23_PALETTE: RomAddr = RomAddr(0x28f4c0);

pub(super) fn v23_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    copy_longs(x, V23_PALETTE, 0x91c180, 8);
    let top = rd32(x.b, 0x91c000) & BRIGHT;
    for i in 0..8 {
        let at = 0x91c180 + 4 * i;
        let v = rd32(x.b, at) & 0x0fff_0fff | top;
        wr32(x.b, at, v);
    }
    x.anim_start(ANIMS.0, 0x170);
    v23_follow(x);
    x.reinsert();
}

pub(super) fn v23_follow_and_insert<R: Bench>(x: &mut Cx<'_, R>) {
    v23_follow(x);
    x.reinsert();
}

pub(super) fn v23_follow<R: Bench>(x: &mut Cx<'_, R>) {
    let camera = if ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { 0x54e } else { 0x4fe };
    x.set16(Late::CAMERA, (0xff_8000u32 + camera) as u16);
    let cam = x.linked(Late::CAMERA);
    let v = cam.u16(x.b, Obj::X);
    x.set16(Obj::X, v);
    let v = cam.u16(x.b, Obj::Z);
    x.set16(Obj::Z, v);
    x.add16(Obj::Z, 0x10);
    x.set16(Obj::Y, 0u16.wrapping_sub(0x10));
    x.set8(Base::FLIP, 0);
    x.add16(Obj::Z, 0x40);
    x.add16(Obj::X, 0x100);
}

view! {
    /// Fields of the scene elements of variants 8 to 23.
    Late {
        /// Bits of the carrier cleared by the dot (h).
        owner_bits, set_owner_bits, OWNER_BITS: u8 @ 0x30;
        /// Camera it sticks to (16-bit pointer).
        camera, set_camera, CAMERA: u16 @ 0x42;
        /// Countdown (h).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Number of the badge or object whose variant is taken (bit 15).
        which, set_which, WHICH: u16 @ 0x46;
    }
}
