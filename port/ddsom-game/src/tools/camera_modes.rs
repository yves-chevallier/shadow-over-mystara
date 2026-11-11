//! Tools: the natives of `crate::camera_modes` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::camera_modes::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::scroll::{CAMERA_1, CAMERA_2};

/// Native: D4 of the copy, A0 = the output camera.
fn camera1_update(c: &mut Cpu, b: &mut World) {
    if let Some((v, m)) = update_camera_1(b) {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = CAMERA_1.0 .0;
    rts(c, b);
}

/// Native: D4 of the copy, A0 = the output camera.
fn camera2_update(c: &mut Cpu, b: &mut World) {
    if let Some((v, m)) = update_camera_2(b) {
        c.d[4] = c.d[4] & !m | v;
    }
    c.a[0] = CAMERA_2.0 .0;
    rts(c, b);
}

const fn camera(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: regs::a(0) }
}

pub(crate) const CAMERA1_UPDATE: Native = camera(0x2ede0, "camera1_update", camera1_update);

pub(crate) const CAMERA2_UPDATE: Native = camera(0x32108, "camera2_update", camera2_update);
