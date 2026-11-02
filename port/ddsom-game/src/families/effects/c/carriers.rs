use super::*;

// ---------------------------------------------------------------------------------------------
// Type 50: effects attached to an object (16 variants)
// ---------------------------------------------------------------------------------------------

pub(super) mod attached {
    pub const ANIM_SET: u32 = 0x19626c;
    pub const ANIM_SET_B: u32 = 0x162ebc;
    /// Duration (variant 5) and random offsets, words indexed by 4 bits of the generator (space
    /// programme).
    pub const DELAYS: u32 = 0xb6160;
    pub const JITTER: u32 = 0xb6180;
    /// L'objet porteur (pointeur de 16 bits).
    pub const CARRIER: u32 = 0x42;
    pub const TIMER: u32 = 0x58;
    pub const BLINK: u32 = 0x30;
}

/// `subq.w #1,(off,A0)` followed by `bgt`: true if the counter stays positive.
pub(super) fn countdown<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    let old = x.u16(off);
    x.set16(off, old.wrapping_sub(1));
    (old as i16) > 1
}

/// `bchg #1,(1,A0)` : bascule l'affichage.
pub(super) fn toggle_hidden<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(0x01) ^ 2;
    x.set8(0x01, v);
}

pub(super) fn set_hidden<R: Bench>(x: &mut Cx<'_, R>, on: bool) {
    x.set_bit(0x01, 1, on);
}

/// Draw from the game's generator (0x19a88), 4 bits.
pub(super) fn random_nibble<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let rng = x.random();
    rng as u32 & 0xf
}

pub(super) fn copy_position<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for field in [Obj::X, Obj::Z, Obj::Y] {
        let v = from.u16(x.b, field);
        x.set16(field, v);
    }
}

pub(super) fn attached<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => attached_init(x),
        2 => attached_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

pub(super) fn attached_init<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    if v != 4 {
        x.add8(f::STATE, 2);
    }
    let carrier = x.linked(attached::CARRIER);
    match v {
        0..=3 => {
            copy_position(x, carrier);
            if v == 3 {
                x.add16(Obj::Y, 4u16.wrapping_neg());
            }
            x.anim_start(attached::ANIM_SET, (v as u32) as u16);
        }
        4 => {
            // reflection: copies the carrier's sprite frame
            copy_position(x, carrier);
            x.add16(Obj::Y, 1);
            let frame = carrier.u32(x.b, 0x34);
            x.set32(0x34, frame);
            for off in [0x3c, 0x3f] {
                let b = carrier.u8(x.b, off);
                x.set8(off, b);
            }
            x.set16(attached::BLINK, 0x14);
            x.add8(f::STATE, 2);
        }
        5 => {
            set_hidden(x, true);
            let k = random_nibble(x);
            let delay = x.rom16(attached::DELAYS + 2 * k);
            x.set16(attached::TIMER, delay);
            let k = random_nibble(x);
            let dx = x.rom16(attached::JITTER + 2 * k);
            x.add16(Obj::X, dx);
            if (carrier.u8(x.b, f::VARIANT) as i8) < 5 {
                let k = random_nibble(x);
                let dy = x.rom16(attached::JITTER + 2 * k);
                x.add16(Obj::Y, dy);
            }
            x.anim_start(attached::ANIM_SET, 4);
            let carrier = x.linked(attached::CARRIER);
            if matches!(carrier.u8(x.b, f::VARIANT), 4 | 9) {
                crate::sound::play(x.b, crate::sound::CMD_367_467, x.o);
            }
        }
        6 => {
                x.anim_start(attached::ANIM_SET, 5);
            }
        7 => {}
        8..=11 => {
            const SOUNDS: [crate::sound::Sound; 4] = [crate::sound::CMD_102_202, crate::sound::CMD_10C_20C, crate::sound::CMD_10F_20F, crate::sound::CMD_109_209];
            crate::sound::play(x.b, SOUNDS[v as usize - 8], x.o);
            x.anim_start(attached::ANIM_SET, (v as u32 - 3) as u16);
        }
        12 | 13 => {
            set_hidden(x, true);
            x.anim_start(attached::ANIM_SET, (v as u32 - 3) as u16);
        }
        14 => {
                x.anim_start(attached::ANIM_SET_B, 0x16);
            }
        15 => {
            x.set16(attached::TIMER, 0x28);
            x.anim_start(attached::ANIM_SET, 0xb);
        }
        _ => {}
    }
}

pub(super) fn attached_active<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    let sub = x.u8(f::SUB_STATE);
    match v {
        0..=3 => {
            let carrier = x.linked(attached::CARRIER);
            if carrier.u8(x.b, f::STATE) == 4 {
                return free(x);
            }
            copy_position(x, carrier);
            if v == 3 {
                x.add16(Obj::Y, 4u16.wrapping_neg());
            }
            if x.anim_step() {
                free(x);
            }
        }
        4 if sub == 0 => {
            if !countdown(x, attached::BLINK) {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::BLINK, 0xe);
            }
        }
        4 | 7 => {
            if countdown(x, attached::BLINK) {
                toggle_hidden(x);
            } else {
                free(x);
            }
        }
        5 if sub == 0 => {
            if countdown(x, attached::TIMER) {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            set_hidden(x, false);
            crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 0xc);
            x.anim_step();
        }
        5 | 6 | 8..=11 => {
            if x.anim_step() {
                free(x);
            }
        }
        12 | 13 => {
            if sub == 0 {
                if countdown(x, attached::TIMER) {
                    return;
                }
                x.add8(f::SUB_STATE, 2);
                set_hidden(x, false);
                x.move_xyz();
                x.anim_step();
            } else {
                x.move_xyz();
                if x.anim_step() {
                    free(x);
                } else {
                    toggle_hidden(x);
                }
            }
        }
        14 => attached_shake(x, sub),
        15 => {
            if !countdown(x, attached::TIMER) {
                return free(x);
            }
            toggle_hidden(x);
            x.anim_step();
        }
        _ => {}
    }
}

/// Variant 14: waits for the carrier to leave the active state, shakes, then fades by blinking.
pub(super) fn attached_shake<R: Bench>(x: &mut Cx<'_, R>, sub: u8) {
    match sub {
        0 => {
            let carrier = x.linked(attached::CARRIER);
            if carrier.u16(x.b, f::STATE) != 0x200 {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::TIMER, 10);
            }
        }
        2 => {
            if !countdown(x, attached::TIMER) {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::TIMER, 0x1e);
                x.set16(Obj::VX, 2);
            }
        }
        4 | 6 => {
            let vx = x.u16(Obj::VX).wrapping_neg();
            x.set16(Obj::VX, vx);
            x.add16(Obj::X, vx);
            if sub == 4 {
                if !countdown(x, attached::TIMER) {
                    x.add8(f::SUB_STATE, 2);
                    let carrier = x.linked(attached::CARRIER);
                    let facing = carrier.u8(x.b, 0x2c);
                    x.set8(0x2c, facing);
                    x.anim_start(attached::ANIM_SET_B, 0x17);
                }
            } else if x.anim_step() {
                x.add8(f::SUB_STATE, 2);
                x.set16(attached::TIMER, 0x3c);
                crate::ai_aids::debris_set(x.b, x.o, 0xb);
                // Original glitch: the exhausted `dbf` counter stays in D5 (`dispatch::Leftovers`)
                *x.c.d5() = 0xffff;
            }
        }
        8 => {
            if !countdown(x, attached::TIMER) {
                return free(x);
            }
            if (x.u16(attached::TIMER) as i16) <= 0x1e {
                toggle_hidden(x);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 49: accessory of a carrier, which falls when the carrier goes into a given state
// ---------------------------------------------------------------------------------------------

pub(super) mod prop {
    pub const ANIM_SET: u32 = 0x1961be;
    pub const CARRIER: u32 = 0x42;
}

/// 0xb5f66 / 0xb5f8e: placed at `dx` from the carrier, 0x45 higher, right in front of it.
pub(super) fn prop_place<R: Bench>(x: &mut Cx<'_, R>, dx: u16) {
    let carrier = x.linked(prop::CARRIER);
    let (cx_, cz) = (carrier.u16(x.b, Obj::X), carrier.u16(x.b, Obj::Z));
    x.set16(Obj::X, cx_.wrapping_add(dx));
    x.set16(Obj::Z, cz.wrapping_sub(0x45));
    let depth = carrier.u32(x.b, Obj::Y).wrapping_add(1);
    x.set32(Obj::Y, depth);
}

/// States (`+4..+7`) of the carrier.
pub(super) fn carrier_state<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let carrier = x.linked(prop::CARRIER);
    carrier.u32(x.b, f::STATE)
}

/// Fall: depth and x in 16.16, down to 0xf0 below the top of the screen.
pub(super) fn prop_fall<R: Bench>(x: &mut Cx<'_, R>, dz: u32, dx: u32, alt_camera: bool) {
    x.add32(Obj::Z, dz);
    x.add32(Obj::X, dx.wrapping_neg());
    let top = if alt_camera && crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { x.a5_u16(0x5b2) } else { x.a5_u16(0x5aa) };
    if (top.wrapping_add(0xf0) as i16) <= (x.u16(Obj::Z) as i16) {
        return free(x);
    }
    x.anim_step();
}

pub(super) fn prop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            prop_place(x, 0xffd0);
            let anim = (x.u8(f::VARIANT) != 0) as u32;
            x.anim_start(prop::ANIM_SET, anim as u16);
        }
        2 => prop_active(x),
        _ => free(x),
    }
    if x.u8(0) != 0 {
        if x.u8(f::VARIANT) != 0 {
            x.reinsert();
        } else {
            x.reinsert_foreground();
        }
    }
}

pub(super) fn prop_active<R: Bench>(x: &mut Cx<'_, R>) {
    let sub = x.u8(f::SUB_STATE);
    match (x.u8(f::VARIANT), sub) {
        (0, 0) => {
            if carrier_state(x) == 0x0200_000c {
                return free(x);
            }
            prop_place(x, 0xffd0);
            x.anim_step();
        }
        (1, 0) => {
            if matches!(carrier_state(x), 0x0200_0608 | 0x0200_0808) {
                x.add8(f::SUB_STATE, 2);
            } else {
                prop_place(x, 0x30);
            }
            x.anim_step();
        }
        (1, 2) => {
            if matches!(carrier_state(x), 0x0200_060c | 0x0200_080c) {
                x.add8(f::SUB_STATE, 2);
                crate::sound::play(x.b, crate::sound::CMD_311_411, x.o);
            }
            x.anim_step();
        }
        (1, 4) => prop_fall(x, 0x1_8000, 0x8000, true),
        (2, 0) => {
            if carrier_state(x) == 0x0200_0008 {
                x.add8(f::SUB_STATE, 2);
            } else {
                prop_place(x, 0x30);
            }
            x.anim_step();
        }
        (2, 2) => {
            if carrier_state(x) == 0x0200_000c {
                x.add8(f::SUB_STATE, 2);
                crate::sound::play(x.b, crate::sound::CMD_311_411, x.o);
            }
            x.anim_step();
        }
        (2, 4) => prop_fall(x, 0x8000, 0x1_8000, false),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Type 16: double of a carrier object, which glides to its place and follows the carrier's steps
// ---------------------------------------------------------------------------------------------

pub(super) mod double {
    /// Per variant: arrival x (words), initial direction and speed table (bytes), space
    /// programme.
    pub const TARGETS: u32 = 0xae76e;
    pub const DIRECTIONS: u32 = 0xae756;
    pub const SPEEDS: u32 = 0xae75a;
    pub const CARRIER: u32 = 0x42;
    pub const TARGET_X: u32 = 0x58;
}

/// 0xae748 + 0x1b64: speed table of the variant.
pub(super) fn double_speed_table<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT) as u32;
    let n = (x.rom8(double::SPEEDS + v) as u32) as u16;
    x.select_speed(&crate::objects::motion::SPEEDS_B, n);
}

/// Copies the carrier's sprite frame (and its height if `with_z`).
pub(super) fn double_copy_frame<R: Bench>(x: &mut Cx<'_, R>, with_flip: bool, with_z: bool) {
    let carrier = x.linked(double::CARRIER);
    let frame = carrier.u32(x.b, 0x34);
    x.set32(0x34, frame);
    if with_flip {
        let b = carrier.u8(x.b, 0x3b);
        x.set8(0x3b, b);
    }
    if with_z {
        let z = carrier.u16(x.b, Obj::Z);
        x.set16(Obj::Z, z);
    }
}

/// Movement in x and y; true when the arrival x is reached (in the walking direction).
pub(super) fn double_slide<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.move_xy();
    let (mut a, mut b) = (x.u16(Obj::X) as i16, x.u16(double::TARGET_X) as i16);
    if (x.u8(Obj::SPEED_INDEX) as i8) >= 0x10 {
        std::mem::swap(&mut a, &mut b);
    }
    b <= a
}

/// 0xae700: creates the next object (type 39), retrying until it finds a slot.
pub(super) fn double_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    let o = loop {
        if let Some(o) = x.alloc(&pools::EFFCT) {
            break o;
        }
    };
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 39);
    let v = x.u8(f::VARIANT);
    wr8(x.b, o.0 + 3, v);
    let carrier = x.u16(double::CARRIER);
    wr16(x.b, o.0 + 0x42, carrier);
    let facing = x.u8(0x2c);
    wr8(x.b, o.0 + 0x2c, facing);
    let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
    wr16(x.b, o.0 + Obj::X, px);
    wr16(x.b, o.0 + Obj::Y, py);
    wr16(x.b, o.0 + Obj::Z, 0x66);
}

pub(super) fn double<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let target = x.rom16(double::TARGETS + 2 * v);
            x.set16(double::TARGET_X, target);
            let dir = x.rom8(double::DIRECTIONS + v);
            x.set8(Obj::SPEED_INDEX, dir);
            double_speed_table(x);
            let carrier = x.linked(double::CARRIER);
            copy_position(x, carrier);
            let frame = carrier.u32(x.b, 0x34);
            x.set32(0x34, frame);
            x.set8(0x3b, 0);
            for off in [0x3c, 0x3f] {
                let b = carrier.u8(x.b, off);
                x.set8(off, b);
            }
        }
        2 => double_active(x),
        _ => return free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

pub(super) fn double_active<R: Bench>(x: &mut Cx<'_, R>) {
    let sub = x.u8(f::SUB_STATE);
    // steps 2 to 8: waits for the carrier to reach the expected state
    const AWAITED: [(u8, u32); 4] = [(2, 0x0200_0412), (4, 0x0200_0420), (6, 0x0200_0424), (8, 0x0200_0428)];
    match sub {
        0 => {
            toggle_hidden(x);
            if !double_slide(x) {
                return double_copy_frame(x, false, false);
            }
            x.add8(f::SUB_STATE, 2);
            set_hidden(x, false);
            let target = x.u16(double::TARGET_X);
            x.set16(Obj::X, target);
            double_copy_frame(x, true, false);
        }
        10 => {
            toggle_hidden(x);
            let carrier = x.linked(double::CARRIER);
            let (cx_, cy) = (carrier.u16(x.b, Obj::X), carrier.u16(x.b, Obj::Y));
            let dir = crate::aids::direction_from(x.b, x.o, (cx_, cy));
            x.set8(Obj::SPEED_INDEX, dir);
            double_speed_table(x);
            if double_slide(x) {
                return free(x);
            }
            double_copy_frame(x, true, true);
        }
        _ => {
            let Some(&(_, awaited)) = AWAITED.iter().find(|(s, _)| *s == sub) else { return };
            let carrier = x.linked(double::CARRIER);
            if carrier.u32(x.b, f::STATE) == awaited {
                x.add8(f::SUB_STATE, 2);
                match sub {
                    2 | 4 => double_spawn(x),
                    8 => {
                        let cx_ = carrier.u16(x.b, Obj::X);
                        x.set16(double::TARGET_X, cx_);
                    }
                    _ => {}
                }
            }
            double_copy_frame(x, true, sub == 8);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Type 60: second double of a carrier (same principle as type 16, seven steps)
// ---------------------------------------------------------------------------------------------

pub(super) mod double2 {
    pub const TARGETS: u32 = 0xb7516;
    pub const DIRECTIONS: u32 = 0xb74fe;
    pub const SPEEDS: u32 = 0xb7502;
}

/// 0xb74ea: initial direction of the variant, and its speed table in D0 (D1 = variant).
/// Direction of the variant; returns the speed table (D0 of the original).
pub(super) fn double2_direction<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    let v = x.u8(f::VARIANT) as u32;
    let dir = x.rom8(double2::DIRECTIONS + v);
    x.set8(Obj::SPEED_INDEX, dir);
    x.rom8(double2::SPEEDS + v) as u16
}

pub(super) fn double2<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let v = x.u8(f::VARIANT) as u32;
            let target = x.rom16(double2::TARGETS + 2 * v);
            x.set16(double::TARGET_X, target);
            let n = double2_direction(x);
            x.select_speed(&crate::objects::motion::SPEEDS_B, n);
            let carrier = x.linked(double::CARRIER);
            copy_position(x, carrier);
            set_hidden(x, true);
            let frame = carrier.u32(x.b, 0x34);
            x.set32(0x34, frame);
            x.set8(0x3b, 0);
            for off in [0x3c, 0x3f] {
                let b = carrier.u8(x.b, off);
                x.set8(off, b);
            }
        }
        2 => double2_active(x),
        _ => return free(x),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

pub(super) fn double2_active<R: Bench>(x: &mut Cx<'_, R>) {
    let carrier = x.linked(double::CARRIER);
    let sub = x.u8(f::SUB_STATE);
    let awaited = |sub: u8| match sub {
        0 => 0x202,
        4 => 0x208,
        6 => 0x20a,
        8 => 0x212,
        _ => 0x216,
    };
    match sub {
        0 => {
            if carrier.u16(x.b, f::STATE) == 0x202 {
                x.add8(f::SUB_STATE, 2);
                let z = carrier.u16(x.b, Obj::Z);
                x.set16(Obj::Z, z);
                set_hidden(x, false);
                x.set8(0x3b, 0);
                for off in [0x3c, 0x3f] {
                    let b = carrier.u8(x.b, off);
                    x.set8(off, b);
                }
            }
        }
        2 | 12 => {
            toggle_hidden(x);
            if !double_slide(x) {
                return double_copy_frame(x, sub == 12, true);
            }
            if sub == 12 {
                return free(x);
            }
            x.add8(f::SUB_STATE, 2);
            set_hidden(x, false);
            let target = x.u16(double::TARGET_X);
            x.set16(Obj::X, target);
            double_copy_frame(x, true, true);
        }
        4 | 6 | 8 | 10 => {
            if carrier.u16(x.b, f::STATE) == awaited(sub) {
                x.add8(f::SUB_STATE, 2);
                match sub {
                    4 => {
                        if let Some(o) = x.alloc(&pools::EFFCT) {
                            wr8(x.b, o.0, 1);
                            wr8(x.b, o.0 + 2, 50);
                            wr8(x.b, o.0 + 3, 2);
                            wr16(x.b, o.0 + 0x42, x.o.0 as u16);
                            let facing = x.u8(0x2c);
                            wr8(x.b, o.0 + 0x2c, facing);
                        }
                    }
                    6 => {
                        if let Some(o) = x.alloc(&pools::EFFCT) {
                            wr8(x.b, o.0, 1);
                            wr8(x.b, o.0 + 2, 50);
                            wr8(x.b, o.0 + 3, 6);
                            for field in [Obj::X, Obj::Y, Obj::Z] {
                                let v = x.u16(field);
                                wr16(x.b, o.0 + field, v);
                            }
                            let dx: u16 = if x.u8(0x2c) != 0 { 0x10 } else { 0xfff0 };
                            let px = rd16(x.b, o.0 + Obj::X).wrapping_add(dx);
                            wr16(x.b, o.0 + Obj::X, px);
                        }
                    }
                    8 => {
                        // the thrown chunk (type 39, variant + 5): the original retries until it finds a slot
                        let o = loop {
                            if let Some(o) = x.alloc(&pools::EFFCT) {
                                break o;
                            }
                        };
                        wr8(x.b, o.0, 1);
                        wr8(x.b, o.0 + 2, 39);
                        let v = x.u8(f::VARIANT).wrapping_add(5);
                        wr8(x.b, o.0 + 3, v);
                        let c = x.u16(double::CARRIER);
                        wr16(x.b, o.0 + 0x42, c);
                        let facing = x.u8(0x2c);
                        wr8(x.b, o.0 + 0x2c, facing);
                        let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                        wr16(x.b, o.0 + Obj::X, px);
                        wr16(x.b, o.0 + Obj::Y, py);
                        wr16(x.b, o.0 + Obj::Z, 0x66);
                    }
                    _ => {
                        // return: U-turn toward the carrier
                        let cx_ = carrier.u16(x.b, Obj::X);
                        x.set16(double::TARGET_X, cx_);
                        let n = double2_direction(x);
                        let dir = (x.u8(Obj::SPEED_INDEX).wrapping_add(0x10)) & 0x1f;
                        x.set8(Obj::SPEED_INDEX, dir);
                        x.select_speed(&crate::objects::motion::SPEEDS_B, n);
                    }
                }
            }
            double_copy_frame(x, true, true);
        }
        _ => {}
    }
}

