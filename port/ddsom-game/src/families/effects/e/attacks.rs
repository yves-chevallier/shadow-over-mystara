use super::*;

// ---------------------------------------------------------------------------------------------
// Type 11: effects attached to an enemy's attacks (ten variants)
// ---------------------------------------------------------------------------------------------

pub(super) mod attack_fx {
    /// Variant 0: spots around the attacker (offset word per attacker type, then eight (dx, dz)).
    pub const FLASH_PLACES: u32 = 0xabfbe;
    pub const SOUND_FLASH: crate::sound::Sound = crate::sound::CMD_18F;
    /// Variant 2: offsets (dx, dz) and animation per attacker type (program space).
    pub const MARK_PLACES: u32 = 0xac6ea;
    pub const MARK_ANIMS: u32 = 0xac6ca;
    /// Attacker's action (`+0x49`), noted by variant 5 in `+0x42`.
    pub const ACTION: u32 = 0x49;
    pub const NOTED_ACTION: u32 = 0x42;
    /// Variants 8 and 9: spray sheet per attacker type (byte, program space: 0 = none),
    /// sheets (data: offsets, then count minus one and (vx, vz, setting, padding) per spark),
    /// spark animation per setting.
    pub const SHOWER_SHEETS: u32 = 0xad1b4;
    pub const SHOWER_DATA: u32 = 0xad232;
    pub const SHOWER_ANIMS: u32 = 0xad354;
    pub const ANIM_A: u32 = 0x18f412;
    pub const ANIM_B: u32 = 0x15cdb6;
    pub const OWNER: u32 = 0x5c;
    pub const SOURCE: u32 = 0x54;
}

/// 0xabec8: creates an EFFCT 11 of variant `variant` and mode `mode`, attached to the current object.
pub(super) fn attack_fx_spawn<R: Bench>(x: &mut Cx<'_, R>, variant: u8, mode: u8) -> Option<Obj> {
    let o = x.alloc(&pools::EFFCT)?;
    wr8(x.b, o.0, 1);
    wr8(x.b, o.0 + 2, 11);
    wr16(x.b, o.0 + attack_fx::OWNER, x.o.0 as u16);
    wr8(x.b, o.0 + 3, variant);
    wr8(x.b, o.0 + 0x2f, mode);
    Some(o)
}

/// Index of the attacker in the tables: its type, + 0x20 if its class (`+0x28`) equals `class`
/// (`when_equal`) or does not.
pub(super) fn attacker_index<R: Bench>(x: &mut Cx<'_, R>, a: Obj, class: u8, when_equal: bool) -> u32 {
    let shifted = (a.u8(x.b, 0x28) == class) == when_equal;
    let base: u8 = if shifted { 0x20 } else { 0 };
    base.wrapping_add(a.u8(x.b, 2)) as u32
}

pub(super) fn attack_fx<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::VARIANT) {
        0 => attack_fx_flash(x),
        1 => attack_fx_burst(x),
        2 => attack_fx_mark(x),
        3 => attack_fx_glint(x),
        4 => attack_fx_rise(x),
        5 => attack_fx_trail(x),
        6 => attack_fx_parts(x),
        7 => attack_fx_sparks(x),
        8 | 9 => attack_fx_shower(x),
        _ => {}
    }
}

/// End of animation: removal (state 6), otherwise re-listing.
pub(super) fn attack_fx_anim_or_leave<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        return x.set32(f::STATE, 0x0600_0000);
    }
    x.reinsert();
}

/// Variant 0: spark placed at random around the attacker (eight spots per attacker type, flipped every other
/// frame), random animation, sound; disappears at the end of the animation.
pub(super) fn attack_fx_flash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            x.add32(Obj::Y, 0xffff_ffff);
            let k = attacker_index(x, owner, 2, false);
            let offset = x.data16(attack_fx::FLASH_PLACES + 2 * k) as i16 as i32 as u32;
            let places = attack_fx::FLASH_PLACES.wrapping_add(offset);
            let rng = x.random();
            let pick = 4 * (rng as u32 & 7);
            let (mut dx, dz) = (x.data16(places + pick), x.data16(places + pick + 2));
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            if x.a5_u8(0x6f) & 1 != 0 {
                x.set8(0x2c, facing ^ 0x20);
            }
            let rng = x.random();
            let anim = rng as u32 & 3;
            x.anim_start(attack_fx::ANIM_A, anim as u16);
            crate::sound::play(x.b, attack_fx::SOUND_FLASH, x.o);
        }
        2 => attack_fx_anim_or_leave(x),
        _ => free(x),
    }
}

/// Variant 2: mark placed on the attacker (offset and animation per type); at the end of its animation,
/// it advances the attacker (`+6` += 2) and disappears.
pub(super) fn attack_fx_mark<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            x.add32(Obj::Y, 0xffff_ffff);
            let kind = owner.u8(x.b, Obj::KIND) as u32;
            let mut dx = x.rom16(attack_fx::MARK_PLACES + 4 * kind);
            let dz = x.rom16(attack_fx::MARK_PLACES + 4 * kind + 2);
            x.add16(Obj::Z, dz);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            let anim = x.rom8(attack_fx::MARK_ANIMS + kind);
            x.anim_start(attack_fx::ANIM_A, (anim as u32) as u16);
            x.reinsert();
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let step = owner.u8(x.b, 6).wrapping_add(2);
            owner.set_u8(x.b, 6, step);
            x.set32(f::STATE, 0x0600_0000);
        }
        _ => free(x),
    }
}

/// Variant 5: like the spark (1), but it follows the attacker only as long as the attacker keeps the action noted at
/// birth (`+0x49` → `+0x42`) and has not passed step 4.
pub(super) fn attack_fx_trail<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, attack_fx::ACTION);
            x.set8(attack_fx::NOTED_ACTION, action);
            burst_init(x);
        }
        2 => {
            let owner = x.linked(attack_fx::OWNER);
            if owner.u8(x.b, attack_fx::ACTION) != x.u8(attack_fx::NOTED_ACTION) || owner.u8(x.b, 6) as i8 >= 4 {
                return free(x);
            }
            for off in [Obj::X, Obj::Z] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            x.anim_step();
            x.reinsert();
        }
        _ => free(x),
    }
}

/// Variants 8 and 9: spray of sparks from the attacker. The first (setting 0) creates the sparks of a sheet
/// specific to the attacker's type (speeds, setting) then disappears; each spark follows the attacker as long as
/// it is in the expected action, then detaches, falls back blinking and disappears on the ground.
pub(super) fn attack_fx_shower<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x2f) == 0 {
        return attack_fx_shower_spawn(x);
    }
    match x.u8(f::STATE) {
        0 => {
            let mode = x.u8(0x2f).wrapping_sub(1) as u32;
            let anim = x.rom8(attack_fx::SHOWER_ANIMS + mode);
            if x.u8(f::VARIANT) != 8 {
                x.set16(0x2a, 0x110);
            }
            x.add8(f::STATE, 2);
            x.anim_start(attack_fx::ANIM_A, (anim as u32) as u16);
            attack_fx_shard(x);
        }
        2 => attack_fx_shard(x),
        _ => free(x),
    }
}

pub(super) fn attack_fx_shower_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    let k = attacker_index(x, owner, 4, true);
    let sheet = x.rom8(attack_fx::SHOWER_SHEETS + k) as u16;
    if sheet != 0 {
        let offset = x.data16(attack_fx::SHOWER_DATA + sheet.wrapping_sub(2) as i16 as i32 as u32) as i16 as i32 as u32;
        let mut at = attack_fx::SHOWER_DATA.wrapping_add(offset);
        let mut n = x.data16(at);
        at += 2;
        loop {
            let variant = x.u8(f::VARIANT);
            let me = x.o;
            let created = crate::families::effects_creators::attack(x.b, me, variant, 0);
            let Some(o) = created else { break };
            let (vx, vz) = (x.data16(at), x.data16(at + 2));
            o.set_u16(x.b, Obj::VX, vx);
            o.set_u16(x.b, Obj::VZ, vz);
            let mode = x.data8(at + 4);
            o.set_u8(x.b, 0x2f, mode);
            at += 6;
            let link = x.u16(attack_fx::OWNER);
            o.set_u16(x.b, attack_fx::OWNER, link);
            n = n.wrapping_sub(1);
            if n == 0xffff {
                break;
            }
        }
    }
    free(x);
}

/// A spark of the spray (`0xad384`).
pub(super) fn attack_fx_shard<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    if x.u8(f::SUB_STATE) == 0 {
        let first = x.u8(f::VARIANT) == 8;
        let (a, b) = if first { (0x70, 0x90) } else { (0x94, 0x94) };
        let action = owner.u8(x.b, attack_fx::ACTION);
        let follows = if action == a || action == b || action != 0xa8 {
            owner.u8(x.b, f::STATE) == 4 && (owner.u8(x.b, 6) as i8) < 6
        } else {
            // action 0xa8: the spark follows the attacker during its steps 4 to 6, otherwise disappears
            if owner.u8(x.b, 6).wrapping_sub(4) > 2 {
                return free(x);
            }
            true
        };
        if follows {
            let (px, pz, py) = (owner.u16(x.b, Obj::X), owner.u16(x.b, Obj::Z), owner.u32(x.b, Obj::Y));
            x.set16(Obj::X, px);
            x.set16(Obj::Z, pz);
            x.set32(Obj::Y, py.wrapping_sub(1));
            let vz = x.u16(Obj::VZ);
            x.add16(Obj::Z, vz);
            let mut vx = x.u16(Obj::VX);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                vx = vx.wrapping_neg();
            }
            x.add16(Obj::X, vx);
            return x.reinsert();
        }
        // detached: it falls back, pushed outward
        x.add8(f::SUB_STATE, 2);
        x.set16(Obj::VZ, 0x200);
        x.set16(Obj::AZ, 0xffc0);
        x.set16(Obj::AX, 0);
        let mut vx: u16 = 0x180;
        if (x.u16(Obj::VX) as i16) < 0 {
            vx = vx.wrapping_neg();
        }
        if x.u8(0x2c) != 0 {
            vx = vx.wrapping_neg();
        }
        x.set16(Obj::VX, vx);
        if x.counter() & 1 != 0 {
            let v = x.u8(0x01) ^ 2;
            x.set8(0x01, v);
        }
    }
    let v = x.u8(0x01) ^ 2;
    x.set8(0x01, v);
    x.move_xz();
    if x.u16(Obj::Z) as i16 <= 0 {
        free(x);
    }
}

/// Variant 1: spark thrown from the attacker, which follows it then disappears.
pub(super) fn attack_fx_burst<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => burst_init(x),
        2 => {
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let (vx, vz) = (x.u16(Obj::VX), x.u16(Obj::VZ));
            x.add16(Obj::X, vx);
            x.add16(Obj::Z, vz);
            // the pause (`A5+0x3d`) keeps it alive until the end of the animation, otherwise it disappears
            if x.a5_u8(0x3d) == 0 || x.anim_step() {
                x.set32(f::STATE, 0x0600_0000);
            }
        }
        _ => free(x),
    }
}

/// Birth of the spark (variants 1 and 5): placed on the attacker, speed and animation according to its type.
pub(super) fn burst_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let owner = x.linked(attack_fx::OWNER);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let facing = owner.u8(x.b, 0x2c);
    x.set8(0x2c, facing);
    let (anim, table, k) = match x.u8(0x2f) {
        0 => {
            x.add32(Obj::Y, 0xffff_ffff);
            let k = attacker_index(x, owner, 4, true);
            (x.rom8(0xac38c + k) as u32, 0xac3cc, k)
        }
        1 => {
            x.add32(Obj::Y, 1);
            let base: u8 = if x.u8(0x28) == 4 { 0x20 } else { 0 };
            (4, 0xac526, base.wrapping_add(owner.u8(x.b, 2)) as u32)
        }
        _ => return,
    };
    let (mut vx, vz) = (x.data16(table + 4 * k), x.data16(table + 4 * k + 2));
    if x.u8(0x2c) != 0 {
        vx = vx.wrapping_neg();
    }
    x.set16(Obj::VX, vx);
    x.set16(Obj::VZ, vz);
    x.add16(Obj::X, vx);
    x.add16(Obj::Z, vz);
    x.anim_start(attack_fx::ANIM_A, anim as u16);
    x.reinsert();
}

/// Variant 3: reflection placed on the attacker as long as it is in action 0x68 or 0x6c.
pub(super) fn attack_fx_glint<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let owner = x.linked(attack_fx::OWNER);
            for off in [Obj::X, Obj::Z] {
                let v = owner.u16(x.b, off);
                x.set16(off, v);
            }
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            let k = 4 * attacker_index(x, owner, 2, false) + 2 * x.u8(0x2f) as u32;
            let mut dx = x.rom8(0xac80a + k) as i8 as u16;
            let dz = x.rom8(0xac80b + k) as i8 as u16;
            x.add16(Obj::Z, dz);
            if x.u8(0x2c) != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.anim_start(attack_fx::ANIM_A, 0xb);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            let owner = x.linked(attack_fx::OWNER);
            if owner.u16(x.b, f::STATE) == 0x400 && matches!(owner.u8(x.b, 0x49), 0x68 | 0x6c) {
                return x.reinsert();
            }
            x.add8(f::STATE, 2);
        }
        _ => free(x),
    }
}

/// Variant 4: column that rises from the bottom of the screen to a height depending on the attacker.
pub(super) fn attack_fx_rise<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            for off in [Obj::X, Obj::Y] {
                let v = owner.u32(x.b, off);
                x.set32(off, v);
            }
            let d: u32 = if x.u8(0x2f) & 1 != 0 { 1 } else { 0xffff_ffff };
            x.add32(Obj::Y, d);
            let top = if crate::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { crate::ram::CAMERA_ALT_Y.get(x.b) } else { crate::ram::CAMERA_Y.get(x.b) };
            let z = top.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, z);
            let mode = x.u8(0x2f) as u32;
            let anim = x.rom8(0xac9f0 + mode) as u32;
            x.anim_start(attack_fx::ANIM_A, anim as u16);
            x.reinsert();
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                x.add16(Obj::Z, 4u16.wrapping_neg());
                let k = (x.u8(0x2f) & !1) as u32 + ((((owner.u8(x.b, 2) & 3) >> 1) as u32) << 3);
                let limit = x.rom16(0xaca5e + k);
                if (limit as i16) >= (x.u16(Obj::Z) as i16) {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, limit);
                    if x.u8(0x2f) == 0 {
                        owner.set_u8(x.b, 6, 6);
                    }
                }
            }
            if owner.u16(x.b, f::STATE) == 0x400 && owner.u8(x.b, 0x49) == 0x9c {
                x.anim_step();
            } else {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Variant 6: pieces of a broken object; mode 0 creates modes 2 and 3 (and becomes mode 1).
pub(super) fn attack_fx_parts<R: Bench>(x: &mut Cx<'_, R>) {
    let mode = x.u8(0x2f);
    if mode == 0 {
        let source = x.linked(attack_fx::SOURCE);
        let k = attacker_index(x, source, 4, true);
        let src = x.u16(attack_fx::SOURCE);
        if x.rom8(0xacafe + k) != 0 {
            if let Some(o) = attack_fx_spawn(x, 6, 2) {
                wr16(x.b, o.0 + attack_fx::SOURCE, src);
            }
        }
        if let Some(o) = attack_fx_spawn(x, 6, 3) {
            wr16(x.b, o.0 + attack_fx::SOURCE, src);
        }
        return x.set8(0x2f, 1);
    }
    if mode > 3 {
        return;
    }
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            let source = x.linked(attack_fx::SOURCE);
            let k = attacker_index(x, source, 4, true);
            match mode {
                1 | 2 => {
                    let table = if mode == 1 { 0xacc06 } else { 0xacd8a };
                    let mut dx = x.rom16(table + 4 * k);
                    if dx == 0 && mode == 2 {
                        return x.set32(f::STATE, 0x0600_0000);
                    }
                    if dx != 0 {
                        let dz = x.rom16(table + 4 * k + 2);
                        let (sx, sz) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Z));
                        x.set16(Obj::X, sx);
                        x.set16(Obj::Z, sz);
                        if source.u8(x.b, 0x2c) != 0 {
                            dx = dx.wrapping_neg();
                        }
                        x.add16(Obj::X, dx);
                        x.add16(Obj::Z, dz);
                    }
                    let d: u32 = if mode == 1 { 0xffff_ffff } else { 1 };
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(d);
                    x.set32(Obj::Y, depth);
                    x.anim_start(attack_fx::ANIM_B, (4 + mode as u32) as u16);
                }
                _ => {
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(2);
                    x.set32(Obj::Y, depth);
                    let mut dx = x.data16(0xacf02 + 4 * k);
                    let dz = x.data16(0xacf02 + 4 * k + 2);
                    let anchor = if dz != 0 { source } else { x.linked(attack_fx::OWNER) };
                    for off in [Obj::X, Obj::Z] {
                        let v = anchor.u32(x.b, off);
                        x.set32(off, v);
                    }
                    let facing = anchor.u8(x.b, 0x2c);
                    x.set8(0x2c, facing);
                    if facing != 0 {
                        dx = dx.wrapping_neg();
                    }
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, dz);
                    x.anim_start(attack_fx::ANIM_B, 7);
                }
            }
            x.reinsert();
        }
        2 => {
            if x.anim_step() {
                x.add8(f::STATE, 2);
            }
        }
        _ => free(x),
    }
}

/// Variant 7: mode 0 creates a spray of sparkles (modes 1 and 2, list depending on the attacker) then
/// disappears; a sparkle follows the attacker as long as it strikes.
pub(super) fn attack_fx_sparks<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x2f) {
        0 => {
            if x.u8(f::STATE) == 0 {
                return x.add8(f::STATE, 2);
            }
            let owner = x.linked(attack_fx::OWNER);
            let k = attacker_index(x, owner, 4, true);
            let list = x.rom8(0xad034 + k);
            if list != 0 {
                let entry = 0xad0a8u32.wrapping_add((list as u16).wrapping_sub(2) as i16 as i32 as u32);
                let mut p = 0xad0a8u32.wrapping_add(x.data16(entry) as i16 as i32 as u32);
                let count = x.data16(p);
                p += 2;
                for _ in 0..=count {
                    // temporary mode: D1.b of the original (word of the mode table), replaced immediately
                    let Some(o) = attack_fx_spawn(x, 7, 6) else { break };
                    let (vx, vz, m) = (x.data16(p), x.data16(p + 2), x.data8(p + 4));
                    wr16(x.b, o.0 + Obj::VX, vx);
                    wr16(x.b, o.0 + Obj::VZ, vz);
                    wr8(x.b, o.0 + 0x2f, m);
                    p += 6;
                    let owner = x.u16(attack_fx::OWNER);
                    wr16(x.b, o.0 + attack_fx::OWNER, owner);
                }
            }
            free(x);
        }
        m @ (1 | 2) => {
            if x.u8(f::STATE) == 0 {
                x.add8(f::STATE, 2);
                x.anim_start(attack_fx::ANIM_A, (0x15 + m as u32) as u16);
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, 0x49);
            let striking = matches!(action, 0x60 | 0x88)
                || (action == 0xa8 && owner.u8(x.b, 6).wrapping_sub(8) <= 2);
            if !striking || owner.u8(x.b, f::STATE) != 4 || (owner.u16(x.b, Obj::Z) as i16) <= 0 {
                return free(x);
            }
            for off in [Obj::X, Obj::Z] {
                let v = owner.u16(x.b, off);
                x.set16(off, v);
            }
            let depth = owner.u32(x.b, Obj::Y).wrapping_sub(1);
            x.set32(Obj::Y, depth);
            let vz = x.u16(Obj::VZ);
            x.add16(Obj::Z, vz);
            let mut vx = x.u16(Obj::VX);
            let facing = owner.u8(x.b, 0x2c);
            x.set8(0x2c, facing);
            if facing != 0 {
                vx = vx.wrapping_neg();
            }
            x.add16(Obj::X, vx);
            x.anim_step();
            x.reinsert();
        }
        _ => {}
    }
}

