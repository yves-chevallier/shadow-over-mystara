//! Tools: the natives of `crate::world::objects::anim` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::world::objects::anim::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::tools::objects::motion::{cmp, lsl_word};
use crate::tools::prog32;
use crate::world::objects::Obj;
use crate::hw::mem::{rd16, rd32, rd8, set_low_byte, set_low_word};

macro_rules! anim_native {
    ($const:ident, $entry:expr, $name:expr, $run:expr) => {
        pub(crate) const $const: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu(|c, b| {
                let obj = Obj(c.a[0]);
                $run(c, b, obj);
                rts(c, b);
            }),
            touches_sound: false,
            outputs: regs::ALL,
        };
    };
}

/// 0x1a32: starts script `D0` of the animation set A4 for the object. Leaves A4 after the entry.
fn start_regs(c: &mut Cpu, b: &mut World, obj: Obj) -> u32 {
    // add.w D0w,D0w ; move.w (A4,D0w),D0w
    let d0 = c.d[0] as u16;
    let doubled = c.addsub(false, d0 as u32, d0 as u32, false, Size::Word, false) as u16;
    let off = rd16(b, c.a[4].wrapping_add(doubled as i16 as i32 as u32));
    set_low_word(&mut c.d[0], off);
    c.flags_logic(off as u32, Size::Word);
    c.a[4] = c.a[4].wrapping_add(off as i16 as i32 as u32);
    let end = load_entry(b, obj, c.a[4], false);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    end
}

/// Animation step with full register semantics: A4 = next entry, D0.b = flags,
/// C = end of script.
fn step_regs(c: &mut Cpu, b: &mut World, obj: Obj, long: bool) -> Step {
    let timer = obj.u16(b, Obj::ANIM_TIMER);
    let left = c.addsub(true, 1, timer as u32, false, Size::Word, false) as u16;
    obj.set_u16(b, Obj::ANIM_TIMER, left);
    if left != 0 {
        return if timer == 0 { Step::Wrapped } else { Step::Running };
    }
    c.a[4] = obj.u32(b, Obj::ANIM_NEXT);
    let flags = obj.u8(b, Obj::ANIM_FLAGS);
    set_low_byte(&mut c.d[0], flags);
    c.flags_logic(flags as u32, Size::Byte);
    if flags != 0 {
        cmp(c, 0xfe, flags as u32, Size::Byte);
        if flags == 0xfe {
            obj.set_u16(b, Obj::ANIM_TIMER, 1);
            c.set_ccr(1);
            return Step::Ended;
        }
        c.a[4] = rd32(b, c.a[4]);
    }
    let end = load_entry(b, obj, c.a[4], long);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    Step::Loaded
}

/// Starts animation `D6` of the set specific to the object's character (long entries), registers included.
fn by_character_regs(c: &mut Cpu, b: &mut World, obj: Obj, sets: &CharacterSets) -> u32 {
    if sets.weapon_offset && obj.u8(b, ARMED) == 0 {
        let v = c.addsub(false, 0x40, c.d[6], false, Size::Word, false);
        set_low_word(&mut c.d[6], v as u16);
    }
    let variant = rd8(b, obj.at(Obj::VARIANT));
    let d5 = if sets.sign_extend {
        set_low_byte(&mut c.d[5], variant);
        (variant as i8 as i16) as u16
    } else {
        c.d[5] = variant as u32;
        variant as u16
    };
    let d5 = lsl_word(c, d5, 2);
    set_low_word(&mut c.d[5], d5);
    let set = prog32(b, sets.pointers.wrapping_add(d5 as i16 as i32 as u32));
    c.a[4] = set;
    let d6 = c.d[6] as u16;
    let doubled = c.addsub(false, d6 as u32, d6 as u32, false, Size::Word, false) as u16;
    let off = rd16(b, set.wrapping_add(doubled as i16 as i32 as u32));
    set_low_word(&mut c.d[6], off);
    c.a[4] = set.wrapping_add(off as i16 as i32 as u32);
    let end = load_entry(b, obj, c.a[4], true);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
    end
}

anim_native!(ANIM_START_INDEXED, 0x1a32, "anim_start_indexed", |c, b, o| {
    start_regs(c, b, o);
});

anim_native!(ANIM_START, 0x1a3c, "anim_start", |c: &mut Cpu, b: &mut World, o| {
    let end = start_entry(b, o, c.a[4]);
    c.a[4] = end;
    c.flags_logic(end, Size::Long);
});

anim_native!(ANIM_STEP, 0x1a56, "anim_step", |c, b, o| {
    step_regs(c, b, o, false);
});

anim_native!(ANIM_STEP_LONG, 0x2d58, "anim_step_long", |c, b, o| {
    step_regs(c, b, o, true);
});

anim_native!(ANIM_START_LONG_DOUBLE, 0x2c16, "anim_start_long_double", |c, b, o| {
    by_character_regs(c, b, o, &SETS_ARMED);
});

anim_native!(ANIM_START_LONG_BY_CHAR, 0x2c88, "anim_start_long_by_char", |c, b, o| {
    by_character_regs(c, b, o, &SETS_PLAIN);
});

anim_native!(ANIM_START_LONG_BY_VARIANT, 0x2cf0, "anim_start_long_by_variant", |c, b, o| {
    by_character_regs(c, b, o, &SETS_BY_VARIANT);
});
