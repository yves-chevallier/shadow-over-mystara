use super::*;

// ---------------------------------------------------------------------------------------------
// Types 37 and 38: panels of an end-of-trial screen (title that descends; message that falls,
// blinks and changes with the state of its creator)
// ---------------------------------------------------------------------------------------------

pub(super) fn title_card<R: Bench>(x: &mut Cx<'_, R>) {
    let creator = x.linked(Card::CREATOR);
    let gone = |x: &mut Cx<'_, R>| {
        crate::sound::play(x.b, crate::sound::CMD_337, x.o);
        free(x);
    };
    if creator.u8(x.b, f::STATE) == 4 {
        return gone(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            crate::sound::play(x.b, crate::sound::CMD_333, x.o);
            x.anim_start(0x1942b2, 0);
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                x.add16(Obj::Z, 8u16.wrapping_neg());
                if (x.u16(Obj::Z) as i16) <= 0x55 {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, 0x55);
                }
                x.anim_step();
            } else if creator.u32(x.b, f::STATE) == 0x0204_1008 {
                gone(x);
            } else {
                x.anim_step();
            }
        }
        _ => gone(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

/// 0xb3556: displayed every other frame.
pub(super) fn blink_frames<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = vars::FRAME_LOW.get(x.b) & 1 == 0;
    set_hidden(x, hidden);
}

pub(super) fn result_card<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let (cx_, cy) = (vars::CAMERA1_X.get(x.b).wrapping_add(0xc0), vars::CAMERA1_Y.get(x.b).wrapping_add(0x68));
            x.set16(Obj::X, cx_);
            x.set16(Obj::Y, cy);
            x.set16(Obj::Z, 0x80);
            x.set16(Obj::VZ, 0xf000);
            crate::sound::play(x.b, crate::sound::CMD_333, x.o);
            x.anim_start(0x19431a, 0);
        }
        2 => {
            let creator = x.linked(Card::CREATOR);
            match x.u8(f::SUB_STATE) {
                0 => {
                    x.move_z();
                    if (x.u16(Obj::Z) as i16) <= 0 {
                        x.add8(f::SUB_STATE, 2);
                        x.set16(Obj::Z, 0);
                    }
                    x.anim_step();
                }
                2 => {
                    if creator.u32(x.b, f::STATE) == 0x0200_0200 {
                        x.add8(f::SUB_STATE, 2);
                        blink_frames(x);
                        crate::sound::play(x.b, crate::sound::CMD_332, x.o);
                        x.anim_start(0x19431a, 1);
                    } else {
                        x.anim_step();
                    }
                }
                4 => {
                    blink_frames(x);
                    if creator.u32(x.b, f::STATE) == 0x0200_0600 {
                        x.add8(f::SUB_STATE, 2);
                        crate::sound::play(x.b, crate::sound::CMD_337, x.o);
                        x.anim_start(0x19431a, 2);
                    } else {
                        x.anim_step();
                    }
                }
                6 => {
                    blink_frames(x);
                    if x.anim_step() {
                        free(x);
                    }
                }
                _ => {}
            }
        }
        _ => free(x),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

// ---------------------------------------------------------------------------------------------
// Type 3: screen element fixed to the camera (height following the shake `A5+0x4ba`)
// ---------------------------------------------------------------------------------------------

pub(super) fn screen_piece<R: Bench>(x: &mut Cx<'_, R>) {
    let follow = |x: &mut Cx<'_, R>| {
        let z = crate::ram::CAMERA_Y.get(x.b).wrapping_sub(crate::ram::SCROLL_Y_PLANE1.get(x.b)).wrapping_add(x.u16(ScreenPiece::HEIGHT));
        x.set16(Obj::Z, z);
    };
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let base: u8 = if v == 0 { 0xc } else { 0 };
            let places = &tables().screen_piece_places;
            let (px, height) = (places.at16(x.b, 4 * v), places.at16(x.b, 4 * v + 2));
            x.set16(Obj::X, px);
            x.set16(ScreenPiece::HEIGHT, height);
            follow(x);
            let anim = base.wrapping_add(x.u8(Effect::SETTING));
            x.anim_start(0x18c6ec, (anim as u32) as u16);
            x.reinsert_foreground();
        }
        2 => {
            follow(x);
            x.anim_step();
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 69: silhouette that paces at the top of the screen; type 64: thrown spark that
// falls back below the horizon
// ---------------------------------------------------------------------------------------------

pub(super) fn pacer<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set8(Effect::FACING, 0x20);
            x.set16(Obj::X, 0xc0);
            x.set16(Obj::Z, 0);
            x.set16(Obj::Y, 0x58);
            x.anim_start(0x1c03a6, 0xbc);
            x.reinsert_foreground();
        }
        2 => {
            x.anim_step();
            if x.u8(Effect::FACING) == 0 {
                x.add32(Obj::X, 0x1_0000u32.wrapping_neg());
                if (x.u16(Obj::X) as i16) <= 0xc0 {
                    x.set8(Effect::FACING, 0x20);
                }
            } else {
                x.add32(Obj::X, 0x1_0000);
                if (x.u16(Obj::X) as i16) >= 0x1c0 {
                    x.set8(Effect::FACING, 0);
                }
            }
        }
        _ => free(x),
    }
}

pub(super) fn spark_fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let r = random_nibble16(x) & 0x1f;
            x.add16(Obj::Z, r);
            let r = random_nibble16(x) & 0x3ff;
            x.set16(Obj::VX, r.wrapping_sub(0x200));
            x.set16(Obj::AX, 0);
            let r = random_nibble16(x) & 0x3ff;
            x.set16(Obj::VZ, r);
            x.set16(Obj::AZ, 0xffe0);
            x.anim_start(0x197a5c, 1);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            if x.move_xz().pl() {
                return;
            }
            if x.u8(f::VARIANT) == 0 {
                return free(x);
            }
            x.add8(f::STATE, 2);
            x.set16(Effect::TIMER, 0x40);
            let anim = (x.u8(Effect::SETTING) ^ 1) as u32 + 1;
            x.anim_start(0x197a5c, anim as u16);
            spark_fall_wait(x);
        }
        4 => spark_fall_wait(x),
        _ => free(x),
    }
}

pub(super) fn spark_fall_wait<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u16(Effect::TIMER).wrapping_sub(1);
    x.set16(Effect::TIMER, t);
    if (t as i16) < 0 {
        free(x);
    }
}

// ---------------------------------------------------------------------------------------------
// Type 66: animation of four large characters in background 2 (one step every 0x68 frames)
// ---------------------------------------------------------------------------------------------

/// Draws four blocks of 2 × 2 tiles (`tile`, `tile + 1` below, `tile + 0x10` on the right...) in
/// columns spaced 0x100 apart.
pub(super) fn draw_four_blocks<R: Bench>(x: &mut Cx<'_, R>, tiles: [u16; 4], attr: u16) {
    for (k, tile) in tiles.into_iter().enumerate() {
        let at = 0x912220 + 0x100 * k as u32;
        for (off, t) in [(0, tile), (0x80, tile.wrapping_add(1)), (4, tile.wrapping_add(0x10)), (0x84, tile.wrapping_add(0x11))] {
            wr16(x.b, at + off, t);
            wr16(x.b, at + off + 2, attr);
        }
    }
}

pub(super) fn big_letters<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::HIDDEN) != 0 {
        // hidden: clears the four blocks once and delays the next step
        if x.u8(Letters::CLEARED) != 0 {
            return;
        }
        x.add8(Letters::CLEARED, 1);
        x.add16(Letters::TIMER, 0x40);
        return draw_four_blocks(x, [0x2fc; 4], 0);
    }
    match x.u8(f::STATE) {
        0 | 2 => {
            if x.u8(f::STATE) == 0 {
                x.add8(f::STATE, 2);
                x.set16(Letters::TIMER, 0x67);
                x.set32(Letters::SCRIPT, tables().letters_script.rom);
            }
            let t = x.u16(Letters::TIMER).wrapping_sub(1);
            x.set16(Letters::TIMER, t);
            if (t as i16) >= 0 {
                return;
            }
            x.set16(Letters::TIMER, 0x67);
            let p = x.u32(Letters::SCRIPT);
            let first = held16(x.b, p);
            if (first as i16) < 0 {
                return free(x);
            }
            let (b, c, d, attr) = (held16(x.b, p + 2), held16(x.b, p + 4), held16(x.b, p + 6), held16(x.b, p + 8));
            x.set32(Letters::SCRIPT, p + 10);
            draw_four_blocks(x, [first, b, c, d], attr);
        }
        _ => free(x),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 91: another screen element fixed to the camera; type 47: marker that blinks slowly
// ---------------------------------------------------------------------------------------------

pub(super) fn screen_piece_b<R: Bench>(x: &mut Cx<'_, R>) {
    let follow = |x: &mut Cx<'_, R>| {
        let z = crate::ram::CAMERA_Y.get(x.b).wrapping_sub(crate::ram::SCROLL_Y_PLANE1.get(x.b)).wrapping_add(x.u16(ScreenPiece::HEIGHT));
        x.set16(Obj::Z, z);
    };
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let (px, height, anim) = if x.u8(Effect::SETTING) != 0 {
                (0xc0, 0x1a0, 0x170)
            } else {
                let v = x.u8(f::VARIANT) as u32;
                let places = &tables().screen_piece_b_places;
                (places.at16(x.b, 4 * v), places.at16(x.b, 4 * v + 2), 0x1b7)
            };
            x.set16(Obj::X, px);
            x.set16(ScreenPiece::HEIGHT, height);
            follow(x);
            x.anim_start(0x1c03a6, anim);
            x.reinsert();
        }
        2 => {
            follow(x);
            x.anim_step();
        }
        _ => free(x),
    }
}

pub(super) fn beacon<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::HIDDEN) != 0 {
        return free(x);
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            x.set16(Beacon::BLINK_TIMER, 0);
            // the table starts one sheet before variant 0: variant 0xff reads it
            let k = 8u32.wrapping_add(((x.u8(f::VARIANT) as i8 as i16 as u16) << 3) as i16 as i32 as u32);
            let sheets = &tables().beacon_sheets;
            for (i, field) in [Obj::X, Obj::Z, Obj::Y].into_iter().enumerate() {
                let d = sheets.at16(x.b, k.wrapping_add(2 * i as u32));
                x.add16(field, d);
            }
            let anim = sheets.at16(x.b, k.wrapping_add(6));
            let anim = anim & 0xff00 | (anim as u8).wrapping_add(x.u8(Effect::SETTING)) as u16;
            x.anim_start(0x19610c, (anim as u32) as u16);
            x.reinsert();
        }
        2 => {
            let t = x.u16(Beacon::BLINK_TIMER).wrapping_sub(1);
            x.set16(Beacon::BLINK_TIMER, t);
            if (t as i16) < 0 {
                x.set16(Beacon::BLINK_TIMER, 0x10);
                toggle_hidden(x);
            }
        }
        _ => free(x),
    }
}

