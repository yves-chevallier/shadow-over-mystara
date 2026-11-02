//! Scene elements, variants 8 to 23 (see the parent module).

use super::*;

/// Variant 8: bar that descends (Z + 8 per frame) to the bottom of the screen, then raises the byte
/// 0xff0eaa (A5-0x7156).
pub(super) const SCENE_DONE_FLAG: u32 = 0xff_0eaa;

pub(super) fn v8_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let v = x.a5_u16(ram::CAMERA_X);
    x.set16(0x8, v);
    let v = x.a5_u16(ram::CAMERA_Y);
    x.set16(0xc, v);
    x.add16(0xc, 0x20);
    x.set32(0x10, 0);
    x.add16(0x10, 0u16.wrapping_sub(0x20));
    start(x, 0x112);
}

pub(super) fn v8_step<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(0xc, 8);
    let below = x.u16(0xc).wrapping_add(x.u16(0x10)).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
    if below < 0x1c8 {
        return;
    }
    let y = x.a5_u16(ram::CAMERA_Y).wrapping_add(0x1c8);
    x.set16(0xc, y);
    crate::mem::wr8(x.b, SCENE_DONE_FLAG, 0xff);
    x.add8(0x4, 2);
    x.reinsert();
}

/// Variant 9: animated text on the secondary camera; signals its end to the master.
pub(super) fn v9_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let v = x.a5_u16(ram::CAMERA2_X);
    x.set16(0x8, v);
    let v = x.a5_u16(ram::CAMERA2_Y);
    x.set16(0xc, v);
    x.set32(0x10, 0);
    x.add16(0xc, 0xf0);
    start(x, 0x113);
    crate::sound::play(x.b, crate::sound::CMD_8E, x.o);
}

pub(super) fn v9_step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() || (signal(x) as i8) < 0 {
        return x.set8(0x4, 6);
    }
    if x.u16(0x30) != 0x7fff {
        return;
    }
    send_signal(x, 1);
    x.set16(0x30, 6);
}

/// Variant 10: plate that descends in two phases (Z 0x88 then 0xa0), waits for signal 2,
/// rises back to Z 0x80 and waits for the end.
pub(super) fn v10_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x8, 0x98_0000);
    x.set32(0xc, 0x70_0000);
    x.set32(0x10, 0);
    start(x, 0x114);
    crate::sound::play(x.b, crate::sound::CMD_3C, x.o);
}

pub(super) fn v10_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x5) {
        0 => {
            x.add32(0xc, 0x4000);
            if x.u16(0xc) < 0x88 {
                return;
            }
            x.set16(0xc, 0x88);
            send_signal(x, 1);
            x.add8(0x5, 2);
        }
        2 => {
            x.add32(0xc, 0x4000);
            if x.u16(0xc) < 0xa0 {
                return;
            }
            x.set16(0xc, 0xa0);
            x.add8(0x5, 2);
        }
        4 => {
            if signal(x) != 2 {
                return;
            }
            x.add8(0x5, 2);
            x.set8(0x42, 0x1e);
        }
        6 => {
            let n = x.u8(0x42).wrapping_sub(1);
            x.set8(0x42, n);
            if n != 0 {
                return;
            }
            x.add8(0x5, 2);
            crate::sound::play(x.b, crate::sound::CMD_40, x.o);
        }
        8 => {
            x.add32(0xc, 0u32.wrapping_sub(0x4000));
            if x.u16(0xc) as i16 > 0x80 {
                return;
            }
            x.set16(0xc, 0x80);
            send_signal(x, 1);
            x.add8(0x5, 2);
        }
        10 => {
            if (signal(x) as i8) < 0 {
                x.add8(0x4, 2);
            }
        }
        _ => {}
    }
}

/// Variant 11: fixed sprite on the main camera.
pub(super) fn v11_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x10, 0x8000);
    let y = x.a5_u16(ram::CAMERA_Y);
    x.set16(0xc, y);
    x.add16(0xc, 0xc0);
    x.set16(0x8, 0x30);
    start(x, 0x115);
    crate::sound::play(x.b, crate::sound::CMD_3E, x.o);
}

/// Variant 12: sprite that crosses the screen (1 pixel per frame to the right, or to the
/// left if `+0x2f`) and moves the camera back.
pub(super) fn v12_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x2c, 0);
    x.set32(0x8, 0xf0_0000);
    let y = x.a5_u16(ram::CAMERA2_Y);
    x.set16(0xc, y);
    x.set16(0x14, 0x100);
    x.add16(0xc, 0xc0);
    x.set32(0x10, 0);
    if x.u8(0x2f) != 0 {
        x.set8(0x2c, 0x20);
        let v = x.u16(0x14).wrapping_neg();
        x.set16(0x14, v);
        x.add32(0x8, 0u32.wrapping_sub(0x60_0000));
    }
    x.anim_start(ANIMS, 0x116);
    x.reinsert();
}

pub(super) fn v12_step<R: Bench>(x: &mut Cx<'_, R>) {
    let d = (((x.u16(0x14) as u32) << 16) as i32 >> 8) as u32;
    x.add32(0x8, d);
    let screen = x.u16(0x8).wrapping_sub(x.a5_u16(ram::CAMERA_X));
    if screen >= 0x180 {
        x.set8(0x4, 4);
    }
    if x.u8(0x2f) != 0 {
        return;
    }
    let cam = x.a5_u16(ram::CAMERA_X);
    if cam != 0 {
        x.set_a5_u16(ram::CAMERA_X, cam - 1);
    }
}

/// Variant 13: frame whose script is a direct entry (16 bytes per `+0x2f` starting from
/// 0xdea84), on a palette background 0x91c340 (8 identical colors).
pub(super) const V13_SCRIPTS: u32 = 0xdea84;

pub(super) fn v13_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let v = x.a5_u16(ram::CAMERA_X);
    x.set16(0x8, v);
    let v = x.a5_u16(ram::CAMERA_Y);
    x.set16(0xc, v);
    x.set32(0x10, 0);
    for i in 0..8 {
        wr32(x.b, 0x91c340 + 4 * i, 0xf111_f111);
    }
    x.add16(0x8, 0x10);
    x.add16(0xc, 0x50);
    let v = x.u8(0x2f) as u16;
    x.add16(0xc, v << 3);
    x.add16(0xc, v << 4);
    x.anim_start_entry(V13_SCRIPTS + 16 * v as u32);
    x.reinsert();
}

/// Variant 14: badge whose sprite and position depend on `+0x46` (number, or object
/// whose variant is taken if bit 15 is raised); 8-byte table at 0xdef1e.
pub(super) const V14_TABLE: u32 = 0xdef1e;

pub(super) fn v14_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let mut which = x.u16(0x46);
    if (which as i16) < 0 {
        let owner = x.linked(0x46);
        which = owner.u8(x.b, 0x3) as u16;
    }
    x.set32(0xc, 0);
    let row = V14_TABLE.wrapping_add((which << 3) as i16 as i32 as u32);
    let v = x.rom16(row);
    x.set16(0x8, v);
    let v = x.rom16(row + 2);
    x.set16(0x10, v);
    let v = x.rom8(row + 6);
    x.set8(0x2c, v);
    x.anim_start(ANIMS, (0x190 + which as u32) as u16);
    x.reinsert();
}

pub(super) fn v14_step<R: Bench>(x: &mut Cx<'_, R>) {
    if (signal(x) as i8) < 0 {
        x.add8(0x4, 2);
    }
}

/// Variant 15: colored pennant (palette 0x28f960, one 32-byte row per number drawn from
/// table 0xdefa4), position in table 0xdf01a.
pub(super) const V15_PALETTE_SELECT: u32 = 0xdefa4;
pub(super) const V15_PALETTES: u32 = 0x28f960;
pub(super) const V15_POSITIONS: u32 = 0xdf01a;

pub(super) fn v15_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x5, 0);
    x.set32(0x10, 0);
    let v = x.u8(0x2f) as u32;
    let line = x.rom8(V15_PALETTE_SELECT + v) as u32;
    copy_longs(x, V15_PALETTES + (line << 5), PALETTE_A, 8);
    or_longs(x, PALETTE_A, BRIGHT, 8);
    let px = x.rom16(V15_POSITIONS + 4 * v);
    x.set16(0x8, px);
    let py = x.rom16(V15_POSITIONS + 4 * v + 2);
    x.set16(0xc, py);
    start(x, 0x19c + v);
}

/// Variant 16: randomly flipped sprite, which lives as long as the master has not finished and its
/// animation is not complete.
pub(super) fn v16_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    random_flip(x);
    start(x, 0x1a2);
    crate::sound::play(x.b, crate::sound::CMD_3D, x.o);
}

pub(super) fn v16_step<R: Bench>(x: &mut Cx<'_, R>) {
    if signal(x) as i8 >= 0 && !x.anim_step() {
        return;
    }
    x.add8(0x4, 2);
}

/// Variant 17: bubble that comes out of the bottom of the screen (two positions according to `+0x2f`); at the end
/// of the animation, gives bit (`+0x2f` modulo 8) of `+0x30` back to the owner `+0x46`.
pub(super) fn v17_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set32(0x10, 0x4000);
    x.set16(0xc, 0x1c0);
    x.set32(0x8, 0);
    if x.u8(0x2f) == 0 {
        x.set16(0x8, 0x20);
        x.set32(0x10, 0x2000);
    }
    // `move.w #0x1a3,D0w ; add.b (0x2f,A0),D0b`: the addition does not overflow into the high byte
    let n = 0x100 | (0xa3 + x.u8(0x2f) as u32) & 0xff;
    start(x, n);
    crate::sound::play(x.b, crate::sound::CMD_3F, x.o);
}

pub(super) fn v17_step<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.anim_step() {
        return;
    }
    let owner = x.linked(0x46);
    let bit = x.u8(0x2f) & 7;
    let v = owner.u8(x.b, 0x30) & !(1 << bit);
    owner.set_u8(x.b, 0x30, v);
    x.add8(0x4, 2);
}

/// Variant 18: sprite placed on the secondary camera.
pub(super) fn v18_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let v = x.a5_u16(ram::CAMERA2_X);
    x.set16(0x8, v);
    let v = x.a5_u16(ram::CAMERA2_Y);
    x.set16(0xc, v);
    x.set32(0x10, 0);
    let dz = if x.u8(0x2f) != 0 { 0x50 } else { 0x100 };
    x.add16(0xc, dz);
    x.anim_start(ANIMS, 0xec);
    x.reinsert();
}

/// Variant 19: palette dot (0x28f340 + 0x60 + 32 per row, row from table
/// 0xdf182) placed on the camera; it disappears when the animation of its carrier (`+0x46`)
/// is finished.
pub(super) const V19_LINES: u32 = 0xdf182;
pub(super) const V19_PALETTES: u32 = 0x28f340 + 0x60;
pub(super) const V19_POSITIONS: u32 = 0xdf234;

pub(super) fn v19_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let v = x.u8(0x2f) as u32;
    let line = x.rom8(V19_LINES + 2 * v) as u32;
    let flip = x.rom8(V19_LINES + 2 * v + 1);
    x.set8(0x2c, flip);
    copy_longs(x, V19_PALETTES + (line << 5), PALETTE_B, 8);
    or_longs(x, PALETTE_B, BRIGHT, 8);
    let anim = if line == 3 { 0 } else { line };
    start(x, 0x1a8 + anim);
    v19_place(x);
}

/// Places the dot from the table, relative to the current camera.
pub(super) fn v19_place<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(0x2f) as u32;
    let px = x.rom16(V19_POSITIONS + 4 * v);
    x.set16(0x8, px);
    let py = x.rom16(V19_POSITIONS + 4 * v + 2);
    x.set16(0xc, py);
    x.set32(0x10, 0);
    let (cx, cy) = if x.a5_u8(ram::USE_CAMERA2) != 0 {
        (x.a5_u16(ram::CAMERA2_X), x.a5_u16(ram::CAMERA2_Y))
    } else {
        (x.a5_u16(ram::CAMERA_X), x.a5_u16(ram::CAMERA_Y))
    };
    x.add16(0x8, cx);
    x.add16(0xc, cy);
}

pub(super) fn v19_step<R: Bench>(x: &mut Cx<'_, R>) {
    v19_place(x);
    let owner = x.linked(0x46);
    if owner.u16(x.b, 0x32) != 0 {
        return;
    }
    x.add8(0x4, 2);
    kill(x);
}

/// Variants 20 and 22: debris falling from a randomly drawn point below the camera's bottom edge (8 abscissas
/// in table 0xdf2a8, 4 acceleration/animation pairs at 0xdf2ea).
pub(super) const V20_X: u32 = 0xdf2a8;
pub(super) const V20_FALL: u32 = 0xdf2ea;

pub(super) fn v20_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let v = x.a5_u16(ram::CAMERA_Y);
    x.set16(0xc, v);
    let v = x.a5_u16(ram::CAMERA_X);
    x.set16(0x8, v);
    x.add16(0xc, 0x110);
    x.set16(0x10, 0x10);
    let rng = x.random();
    let k = rng as u32 & 7;
    let dx = x.rom16(V20_X + 2 * k);
    x.add16(0x8, dx);
    x.set32(0x14, 0);
    x.set16(0x18, 0);
    let rng = x.random();
    let k = rng as u32 & 3;
    let az = x.rom16(V20_FALL + 4 * k);
    x.set16(0x1a, az);
    let anim = x.rom16(V20_FALL + 4 * k + 2) as u32;
    x.anim_start(ANIMS, anim as u16);
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
    let below = x.u16(0xc).wrapping_add(x.u16(0x10));
    if (below as i16) <= x.a5_u16(ram::CAMERA_Y) as i16 {
        kill(x);
    }
}

/// Variant 21: like 15, with the palette row designated by `+0x2f / 2` (table 0xdf332),
/// without brightening; positions in table 0xdf396.
pub(super) const V21_PALETTE_SELECT: u32 = 0xdf332;
pub(super) const V21_POSITIONS: u32 = 0xdf396;

pub(super) fn v21_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x5, 0);
    x.set32(0x10, 0);
    let v = x.u8(0x2f) as u32;
    let line = x.rom8(V21_PALETTE_SELECT + (v >> 1)) as u32;
    copy_longs(x, V15_PALETTES + (line << 5), PALETTE_A, 8);
    let px = x.rom16(V21_POSITIONS + 4 * v);
    x.set16(0x8, px);
    let py = x.rom16(V21_POSITIONS + 4 * v + 2);
    x.set16(0xc, py);
    start(x, 0x19c + (v >> 1));
}

/// Variant 23: spark stuck to the corner of the camera structure (A5+0x4fe, or 0x54e if
/// `A5+0x36`), whose color of palette 0x91c180 takes the brightness of color 0.
pub(super) const V23_PALETTE: u32 = 0x28f4c0;

pub(super) fn v23_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    copy_longs(x, V23_PALETTE, 0x91c180, 8);
    let top = rd32(x.b, 0x91c000) & BRIGHT;
    for i in 0..8 {
        let at = 0x91c180 + 4 * i;
        let v = rd32(x.b, at) & 0x0fff_0fff | top;
        wr32(x.b, at, v);
    }
    x.anim_start(ANIMS, 0x170);
    v23_follow(x);
    x.reinsert();
}

pub(super) fn v23_follow_and_insert<R: Bench>(x: &mut Cx<'_, R>) {
    v23_follow(x);
    x.reinsert();
}

pub(super) fn v23_follow<R: Bench>(x: &mut Cx<'_, R>) {
    let camera = if x.a5_u8(ram::USE_CAMERA2) != 0 { 0x54e } else { 0x4fe };
    x.set16(0x42, (0xff_8000u32 + camera) as u16);
    let cam = x.linked(0x42);
    let v = cam.u16(x.b, 0x8);
    x.set16(0x8, v);
    let v = cam.u16(x.b, 0xc);
    x.set16(0xc, v);
    x.add16(0xc, 0x10);
    x.set16(0x10, 0u16.wrapping_sub(0x10));
    x.set8(0x2c, 0);
    x.add16(0xc, 0x40);
    x.add16(0x8, 0x100);
}
