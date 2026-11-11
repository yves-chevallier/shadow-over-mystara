//! Tools: the natives of `crate::services::scroll` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::services::scroll::*;
use crate::tools::frames::*;
use crate::Native;
use crate::world::objects::Obj;

/// `move_object` with the flags of the addition (`add.l`).
fn move_object_68k(c: &mut Cpu, b: &mut World, field: u32, d: i32) {
    let (d, v) = move_object(b, Obj(c.a[0]), field, d);
    crate::tools::ops::add(c, d, v, Size::Long);
}

/// Natives of the two parallaxes (object A0; flags of the last addition).
fn parallax_68k(c: &mut Cpu, b: &mut World, f: Parallax) {
    let (dx, dy) = parallax(b, f.0, f.1);
    move_object_68k(c, b, OBJECT_X, dx);
    move_object_68k(c, b, OBJECT_Y, dy);
}

fn creep_68k(c: &mut Cpu, b: &mut World, pixels: u16) {
    let old = crate::world::mem::rd16(b, c.a[0] + OBJECT_X);
    let x = creep(b, Obj(c.a[0]), pixels);
    // add.w (X), then move.w (N, Z, V, C)
    crate::tools::ops::add(c, pixels as u32, old as u32, Size::Word);
    crate::tools::ops::logic(c, x as u32, Size::Word);
}

const fn leaf(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: crate::regs::CCR }
}

pub(crate) const PARALLAX_QUARTER: Native = Native {
    ..leaf(0x33178, "scenery_parallax_quarter", |c, b| {
        parallax_68k(c, b, QUARTER);
        rts(c, b)
    })
};

pub(crate) const PARALLAX_THREE_QUARTERS: Native = Native {
    ..leaf(0x33196, "scenery_parallax_three_quarters", |c, b| {
        parallax_68k(c, b, THREE_QUARTERS);
        rts(c, b)
    })
};

pub(crate) const CREEP_6: Native = Native {
    ..leaf(0x331e8, "scenery_advance_6", |c, b| {
        creep_68k(c, b, 6);
        rts(c, b)
    })
};

pub(crate) const CREEP_3: Native = Native {
    ..leaf(0x3326c, "scenery_advance_3", |c, b| {
        creep_68k(c, b, 3);
        rts(c, b)
    })
};

/// 0x31616: moves the vertical position of camera A0 to keep the living players within
/// the window `[camera + 0x30, camera + 0x70]` (at most 6 pixels per frame). Transcribed
/// instruction by instruction: the routines that follow reread D2, D4-D6 and the flags.
fn follow_players_y(c: &mut Cpu, b: &mut World) {
    use crate::world::mem::{rd16, rd8, set_low_word, wr16, A5};
    use crate::tools::ops::cc;
    c.d[6] = 0; // maximum depth of the players
    c.d[5] = u32::MAX; // minimum
    for (k, &record) in PLAYER_RECORD.iter().enumerate() {
        let alive = rd8(b, A5 + record) != 0 && rd16(b, A5 + record + PLAYER_HP) != 0;
        if !alive {
            continue;
        }
        let y = rd16(b, A5 + record + PLAYER_Y);
        if k == 0 {
            set_low_word(&mut c.d[6], y);
            set_low_word(&mut c.d[5], y);
            continue;
        }
        if (c.d[6] as u16) < y {
            set_low_word(&mut c.d[6], y);
        }
        if (c.d[5] as u16) > y {
            set_low_word(&mut c.d[5], y);
        }
    }
    let cam_y = |c: &Cpu, b: &mut World| rd16(b, c.a[0] + Camera::Y) as u32;
    // sub.w src,Dn
    let sub_to = |c: &mut Cpu, n: usize, src: u32| {
        let dst = c.d[n];
        let r = crate::tools::ops::sub(c, src, dst, Size::Word);
        set_low_word(&mut c.d[n], r as u16);
    };
    { let v = c.d[6] as u16; set_low_word(&mut c.d[4], v); }
    let y = cam_y(c, b);
    sub_to(c, 6, y);
    sub_to(c, 6, FOLLOW_BELOW);
    // 0x316ca: D2 = D6; otherwise D2 = D5 is already set
    let step_down = if c.cond(cc::LE) {
        // the players are not too low: brings the camera back up if they are too high
        let y = cam_y(c, b);
        sub_to(c, 5, y);
        sub_to(c, 5, FOLLOW_ABOVE);
        if c.cond(cc::PL) {
            return rts(c, b);
        }
        let d5 = c.d[5];
        crate::tools::ops::cmp(c, 0xfffa, d5, Size::Word);
        if !c.cond(cc::GE) {
            c.d[5] = (-(FOLLOW_STEP as i32)) as u32;
        }
        let (d5, y) = (c.d[5] & 0xffff, cam_y(c, b));
        let y = crate::tools::ops::add(c, d5, y, Size::Word) as u16;
        wr16(b, c.a[0] + Camera::Y, y);
        { let v = c.d[4] as u16; set_low_word(&mut c.d[6], v); }
        let y = cam_y(c, b);
        sub_to(c, 6, y);
        sub_to(c, 6, FOLLOW_BELOW);
        if !c.cond(cc::GT) {
            return rts(c, b);
        }
        true
    } else {
        let y = cam_y(c, b);
        sub_to(c, 5, y);
        sub_to(c, 5, FOLLOW_ABOVE);
        if c.cond(cc::LE) {
            return rts(c, b);
        }
        { let v = c.d[5] as u16; set_low_word(&mut c.d[2], v); }
        let (d6, d5) = (c.d[6] & 0xffff, c.d[5]);
        crate::tools::ops::cmp(c, d6, d5, Size::Word);
        !c.cond(cc::LE)
    };
    if step_down {
        { let v = c.d[6] as u16; set_low_word(&mut c.d[2], v); }
    }
    let d2 = c.d[2];
    crate::tools::ops::cmp(c, FOLLOW_STEP, d2, Size::Word);
    if !c.cond(cc::LE) {
        c.d[2] = FOLLOW_STEP;
    }
    let (d2, y) = (c.d[2] & 0xffff, cam_y(c, b));
    let y = crate::tools::ops::add(c, d2, y, Size::Word) as u16;
    wr16(b, c.a[0] + Camera::Y, y);
    rts(c, b);
}

pub(crate) const FOLLOW_PLAYERS_Y: Native = Native {
    entry: 0x31616,
    name: "camera_follows_players_y",
    run: crate::Body::WithCpu(follow_players_y),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// `update_cameras` with the registers of the original: D4 of the scenery copies, A0 = camera 2.
pub(crate) fn update_cameras_68k(c: &mut Cpu, b: &mut World) {
    for (v, m) in update_cameras(b).into_iter().flatten() {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = CAMERA_2.0 .0;
}

/// Native of 0x33e20.
fn update_cameras_native(c: &mut Cpu, b: &mut World) {
    update_cameras_68k(c, b);
    rts(c, b);
}

/// Native of 0x33aa6: registers of the layer 1 clearing, D0 = 4 x scene, A1 = table of starts, A0 = camera
/// 2.
fn reset_cameras(c: &mut Cpu, b: &mut World) {
    cameras_start(b);
    crate::tools::video::leftovers_clear_layer1_start(c);
    c.d[0] = 4 * crate::world::ram::SCENE.get(b) as u32;
    c.a[1] = crate::world::tables::engine().get("scroll/starts").rom;
    update_cameras_68k(c, b);
    rts(c, b);
}

pub(crate) const RESET_CAMERAS: Native = Native {
    entry: 0x33aa6,
    name: "cameras_start",
    run: crate::Body::WithCpu(reset_cameras),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// Native of 0x33b24: D0 = scene, A1 = video control table, A4 = mode table, A0 = camera 2.
fn return_cameras(c: &mut Cpu, b: &mut World) {
    cameras_return(b);
    c.d[0] = crate::world::ram::SCENE.get(b) as u32;
    c.a[1] = crate::world::tables::engine().get("camera_output").rom;
    c.a[4] = crate::world::tables::engine().get("camera_frozen").rom;
    update_cameras_68k(c, b);
    rts(c, b);
}

pub(crate) const RETURN_CAMERAS: Native = Native {
    entry: 0x33b24,
    name: "cameras_return",
    run: crate::Body::WithCpu(return_cameras),
    touches_sound: false,
    // called by the doors (natives), which reread no register
    outputs: 0,
};

pub(crate) const UPDATE_CAMERAS: Native = Native {
    entry: 0x33e20,
    name: "cameras_update",
    run: crate::Body::WithCpu(update_cameras_native),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

/// Player records (work RAM, A5 + 0x200 k): "present" byte, HP (word) and
/// depth (word); the camera follows the vertical extent of the living players.
const PLAYER_RECORD: [u32; 4] = [0x5de, 0x7de, 0x9de, 0xbde];

const PLAYER_HP: u32 = 0x62;

const PLAYER_Y: u32 = 0x10;

/// Margins (pixels) between the camera and the highest / lowest-in-front player before moving it,
/// and maximum step per frame.
const FOLLOW_ABOVE: u32 = 0x30;

const FOLLOW_BELOW: u32 = 0x70;

const FOLLOW_STEP: u32 = 6;
