use super::*;

// ---------------------------------------------------------------------------------------------
// Type 11: effects attached to an enemy's attacks (ten variants)
// ---------------------------------------------------------------------------------------------

pub(super) mod attack_fx {
    pub const SOUND_FLASH: crate::hw::sound::Sound = crate::hw::sound::CMD_18F;
    /// Attacker's action (`+0x49`), noted by variant 5 in `+0x42`.
    pub const ACTION: u32 = 0x49;
    pub const NOTED_ACTION: u32 = 0x42;
    pub const ANIM_A: crate::world::data::RomAddr = crate::world::data::RomAddr(0x18f412);
    pub const ANIM_B: crate::world::data::RomAddr = crate::world::data::RomAddr(0x15cdb6);
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
    let shifted = (a.u8(x.b, Obj::CLASS) == class) == when_equal;
    let base: u8 = if shifted { 0x20 } else { 0 };
    base.wrapping_add(a.u8(x.b, Obj::KIND)) as u32
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
            let offset = tables().attack_fx_flash_places.at16(x.b, 2 * k) as i16 as i32 as u32;
            // the spots follow their offsets in the same table
            let places = &tables().attack_fx_flash_places;
            let rng = x.random();
            let pick = offset.wrapping_add(4 * (rng as u32 & 7));
            let (mut dx, dz) = (places.at16(x.b, pick), places.at16(x.b, pick + 2));
            let facing = owner.u8(x.b, Effect::FACING);
            x.set8(Effect::FACING, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.add16(Obj::Z, dz);
            if vars::FRAME_BYTE.get(x.b) & 1 != 0 {
                x.set8(Effect::FACING, facing ^ 0x20);
            }
            let rng = x.random();
            let anim = rng as u32 & 3;
            x.anim_start(attack_fx::ANIM_A.0, anim as u16);
            crate::hw::sound::play(x.b, attack_fx::SOUND_FLASH, x.o);
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
            let mut dx = tables().attack_fx_mark_places.at16(x.b, 4 * kind);
            let dz = tables().attack_fx_mark_places.at16(x.b, 4 * kind + 2);
            x.add16(Obj::Z, dz);
            let facing = owner.u8(x.b, Effect::FACING);
            x.set8(Effect::FACING, facing);
            if facing != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            let anim = tables().attack_fx_mark_anims.at8(x.b, kind);
            x.anim_start(attack_fx::ANIM_A.0, (anim as u32) as u16);
            x.reinsert();
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let step = owner.u8(x.b, Carrier::STEP).wrapping_add(2);
            owner.set_u8(x.b, Carrier::STEP, step);
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
            if owner.u8(x.b, attack_fx::ACTION) != x.u8(attack_fx::NOTED_ACTION) || owner.u8(x.b, Carrier::STEP) as i8 >= 4 {
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
    if x.u8(Effect::SETTING) == 0 {
        return attack_fx_shower_spawn(x);
    }
    match x.u8(f::STATE) {
        0 => {
            let mode = x.u8(Effect::SETTING).wrapping_sub(1) as u32;
            let anim = tables().attack_fx_shower_anims.at8(x.b, mode);
            if x.u8(f::VARIANT) != 8 {
                x.set16(Effect::OWN_PALETTE, 0x110);
            }
            x.add8(f::STATE, 2);
            x.anim_start(attack_fx::ANIM_A.0, (anim as u32) as u16);
            attack_fx_shard(x);
        }
        2 => attack_fx_shard(x),
        _ => free(x),
    }
}

pub(super) fn attack_fx_shower_spawn<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(attack_fx::OWNER);
    let k = attacker_index(x, owner, 4, true);
    let sheet = tables().attack_fx_shower_sheets.at8(x.b, k) as u16;
    if sheet != 0 {
        // the sheets follow their offsets in the same table
        let data = &tables().attack_fx_shower_data;
        let mut at = data.at16(x.b, sheet.wrapping_sub(2) as i16 as i32 as u32) as i16 as i32 as u32;
        let mut n = data.at16(x.b, at);
        at += 2;
        loop {
            let variant = x.u8(f::VARIANT);
            let me = x.o;
            let created = crate::actors::families::effects_creators::attack(x.b, me, variant, 0);
            let Some(o) = created else { break };
            let (vx, vz) = (data.at16(x.b, at), data.at16(x.b, at + 2));
            o.set_u16(x.b, Obj::VX, vx);
            o.set_u16(x.b, Obj::VZ, vz);
            let mode = data.at8(x.b, at + 4);
            o.set_u8(x.b, Effect::SETTING, mode);
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
            owner.u8(x.b, f::STATE) == 4 && (owner.u8(x.b, Carrier::STEP) as i8) < 6
        } else {
            // action 0xa8: the spark follows the attacker during its steps 4 to 6, otherwise disappears
            if owner.u8(x.b, Carrier::STEP).wrapping_sub(4) > 2 {
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
            let facing = owner.u8(x.b, Effect::FACING);
            x.set8(Effect::FACING, facing);
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
        if x.u8(Effect::FACING) != 0 {
            vx = vx.wrapping_neg();
        }
        x.set16(Obj::VX, vx);
        if x.counter() & 1 != 0 {
            let v = x.u8(Obj::FLAGS) ^ 2;
            x.set8(Obj::FLAGS, v);
        }
    }
    let v = x.u8(Obj::FLAGS) ^ 2;
    x.set8(Obj::FLAGS, v);
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
            if vars::PAUSE.get(x.b) == 0 || x.anim_step() {
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
    let facing = owner.u8(x.b, Effect::FACING);
    x.set8(Effect::FACING, facing);
    let (anim, table, k) = match x.u8(Effect::SETTING) {
        0 => {
            x.add32(Obj::Y, 0xffff_ffff);
            let k = attacker_index(x, owner, 4, true);
            (tables().burst_anims.at8(x.b, k) as u32, &tables().burst_speeds, k)
        }
        1 => {
            x.add32(Obj::Y, 1);
            let base: u8 = if x.u8(Obj::CLASS) == 4 { 0x20 } else { 0 };
            (4, &tables().burst_speeds_by_type, base.wrapping_add(owner.u8(x.b, Obj::KIND)) as u32)
        }
        _ => return,
    };
    let (mut vx, vz) = (table.at16(x.b, 4 * k), table.at16(x.b, 4 * k + 2));
    if x.u8(Effect::FACING) != 0 {
        vx = vx.wrapping_neg();
    }
    x.set16(Obj::VX, vx);
    x.set16(Obj::VZ, vz);
    x.add16(Obj::X, vx);
    x.add16(Obj::Z, vz);
    x.anim_start(attack_fx::ANIM_A.0, anim as u16);
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
            let facing = owner.u8(x.b, Effect::FACING);
            x.set8(Effect::FACING, facing);
            let k = 4 * attacker_index(x, owner, 2, false) + 2 * x.u8(Effect::SETTING) as u32;
            let mut dx = tables().glint_offsets.at8(x.b, k) as i8 as u16;
            let dz = tables().glint_offsets.at8(x.b, k + 1) as i8 as u16;
            x.add16(Obj::Z, dz);
            if x.u8(Effect::FACING) != 0 {
                dx = dx.wrapping_neg();
            }
            x.add16(Obj::X, dx);
            x.anim_start(attack_fx::ANIM_A.0, 0xb);
            x.reinsert();
        }
        2 => {
            x.anim_step();
            let owner = x.linked(attack_fx::OWNER);
            if owner.u16(x.b, f::STATE) == 0x400 && matches!(owner.u8(x.b, Carrier::ACTION), 0x68 | 0x6c) {
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
            let d: u32 = if x.u8(Effect::SETTING) & 1 != 0 { 1 } else { 0xffff_ffff };
            x.add32(Obj::Y, d);
            let top = if crate::world::ram::CAMERA_ALT_ACTIVE.get(x.b) != 0 { crate::world::ram::CAMERA_ALT_Y.get(x.b) } else { crate::world::ram::CAMERA_Y.get(x.b) };
            let z = top.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, z);
            let mode = x.u8(Effect::SETTING) as u32;
            let anim = tables().rise_anims.at8(x.b, mode) as u32;
            x.anim_start(attack_fx::ANIM_A.0, anim as u16);
            x.reinsert();
        }
        2 => {
            if x.u8(f::SUB_STATE) == 0 {
                x.add16(Obj::Z, 4u16.wrapping_neg());
                let k = (x.u8(Effect::SETTING) & !1) as u32 + ((((owner.u8(x.b, Obj::KIND) & 3) >> 1) as u32) << 3);
                let limit = tables().rise_limits.at16(x.b, k);
                if (limit as i16) >= (x.u16(Obj::Z) as i16) {
                    x.add8(f::SUB_STATE, 2);
                    x.set16(Obj::Z, limit);
                    if x.u8(Effect::SETTING) == 0 {
                        owner.set_u8(x.b, Carrier::STEP, 6);
                    }
                }
            }
            if owner.u16(x.b, f::STATE) == 0x400 && owner.u8(x.b, Carrier::ACTION) == 0x9c {
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
    let mode = x.u8(Effect::SETTING);
    if mode == 0 {
        let source = x.linked(attack_fx::SOURCE);
        let k = attacker_index(x, source, 4, true);
        let src = x.u16(attack_fx::SOURCE);
        if tables().parts_second.at8(x.b, k) != 0 {
            if let Some(o) = attack_fx_spawn(x, 6, 2) {
                wr16(x.b, o.0 + attack_fx::SOURCE, src);
            }
        }
        if let Some(o) = attack_fx_spawn(x, 6, 3) {
            wr16(x.b, o.0 + attack_fx::SOURCE, src);
        }
        return x.set8(Effect::SETTING, 1);
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
                    let table = if mode == 1 { &tables().parts_offsets_1 } else { &tables().parts_offsets_2 };
                    let mut dx = table.at16(x.b, 4 * k);
                    if dx == 0 && mode == 2 {
                        return x.set32(f::STATE, 0x0600_0000);
                    }
                    if dx != 0 {
                        let dz = table.at16(x.b, 4 * k + 2);
                        let (sx, sz) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Z));
                        x.set16(Obj::X, sx);
                        x.set16(Obj::Z, sz);
                        if source.u8(x.b, Effect::FACING) != 0 {
                            dx = dx.wrapping_neg();
                        }
                        x.add16(Obj::X, dx);
                        x.add16(Obj::Z, dz);
                    }
                    let d: u32 = if mode == 1 { 0xffff_ffff } else { 1 };
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(d);
                    x.set32(Obj::Y, depth);
                    x.anim_start(attack_fx::ANIM_B.0, (4 + mode as u32) as u16);
                }
                _ => {
                    let depth = source.u32(x.b, Obj::Y).wrapping_add(2);
                    x.set32(Obj::Y, depth);
                    let mut dx = tables().parts_offsets_3.at16(x.b, 4 * k);
                    let dz = tables().parts_offsets_3.at16(x.b, 4 * k + 2);
                    let anchor = if dz != 0 { source } else { x.linked(attack_fx::OWNER) };
                    for off in [Obj::X, Obj::Z] {
                        let v = anchor.u32(x.b, off);
                        x.set32(off, v);
                    }
                    let facing = anchor.u8(x.b, Effect::FACING);
                    x.set8(Effect::FACING, facing);
                    if facing != 0 {
                        dx = dx.wrapping_neg();
                    }
                    x.add16(Obj::X, dx);
                    x.add16(Obj::Z, dz);
                    x.anim_start(attack_fx::ANIM_B.0, 7);
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
    match x.u8(Effect::SETTING) {
        0 => {
            if x.u8(f::STATE) == 0 {
                return x.add8(f::STATE, 2);
            }
            let owner = x.linked(attack_fx::OWNER);
            let k = attacker_index(x, owner, 4, true);
            let list = tables().sparks_lists.at8(x.b, k);
            if list != 0 {
                // the lists follow their offsets in the same table
                let lists = &tables().sparks_data;
                let entry = (list as u16).wrapping_sub(2) as i16 as i32 as u32;
                let mut p = lists.at16(x.b, entry) as i16 as i32 as u32;
                let count = lists.at16(x.b, p);
                p += 2;
                for _ in 0..=count {
                    // temporary mode: D1.b of the original (word of the mode table), replaced immediately
                    let Some(o) = attack_fx_spawn(x, 7, 6) else { break };
                    let (vx, vz, m) = (lists.at16(x.b, p), lists.at16(x.b, p + 2), lists.at8(x.b, p + 4));
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
                x.anim_start(attack_fx::ANIM_A.0, (0x15 + m as u32) as u16);
                return;
            }
            let owner = x.linked(attack_fx::OWNER);
            let action = owner.u8(x.b, Carrier::ACTION);
            let striking = matches!(action, 0x60 | 0x88)
                || (action == 0xa8 && owner.u8(x.b, Carrier::STEP).wrapping_sub(8) <= 2);
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
            let facing = owner.u8(x.b, Effect::FACING);
            x.set8(Effect::FACING, facing);
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

