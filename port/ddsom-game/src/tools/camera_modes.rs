//! Tools: the natives of `crate::camera_modes` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::camera_modes::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::motion::{self, LastAdd};
use crate::objects::pools::{self, Pool};
use crate::objects::Obj;
use crate::player::{Player, PLAYER_BASE, PLAYER_COUNT, PLAYER_STRIDE};
use crate::scroll::{cam, CAMERA_1, CAMERA_2};
use crate::video::Grid;

/// Native: D4 of the copy, A0 = the output camera.
fn camera1_update(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some((v, m)) = update_camera_1(b) {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = A5 + CAMERA_1;
    rts(c, b);
}

/// Native: D4 of the copy, A0 = the output camera.
fn camera2_update(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some((v, m)) = update_camera_2(b) {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = A5 + CAMERA_2;
    rts(c, b);
}

const fn camera(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: regs::a(0) }
}

pub const CAMERA1_UPDATE: Native = camera(0x2ede0, "camera1_update", camera1_update);

pub const CAMERA2_UPDATE: Native = camera(0x32108, "camera2_update", camera2_update);
