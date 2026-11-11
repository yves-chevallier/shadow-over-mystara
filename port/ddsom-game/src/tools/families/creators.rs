//! Tools: the natives of `crate::families::creators` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::decode::Size;
use m68k::Cpu;

use crate::families::creators::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::mem::{prog8, rd8, wr8, A5};
use crate::tools::prog32;
use crate::objects::Obj;
use crate::objects::pools;

/// Allocates in a pool; `None` (carry set) if the pool is full.
fn alloc(c: &mut Cpu, b: &mut World, pool: &pools::Pool) -> Option<Obj> {
    pool.alloc_68k(c, b)
}

/// Flags of a `move` of `v` (size `size`): N and Z according to the value, V and C cleared.
fn moved(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// 0x9ef20: ESHL shot 9 linked to enemy A0, one frame out of four (counter `A5+0x6e`).
fn enemy_shot_9(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    enemy_shot_9_via(c, b, me);
    rts(c, b);
}

fn set_13(c: &mut Cpu, b: &mut World) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 13);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

fn scene_set_native(c: &mut Cpu, b: &mut World) {
    let v = c.d[0] as u8;
    scene_set_via(c, b, v);
    rts(c, b);
}

/// 0xd981a: end of scene specific to the variant (`+3`, index table 0xd9836, routines 0xd982e: in practice always
/// 0xd99f8, a plain return).
fn scene_exit(c: &mut Cpu, b: &mut World) {
    let scene = Obj(c.a[0]);
    let variant = scene.u8(b, Obj::VARIANT) as u32;
    let i = 4 * prog8(b, 0xd9836 + variant) as u32;
    (c.d[0], c.a[1]) = (i, prog32(b, 0xd982e + i));
    run_scene_exit(b, scene);
    rts(c, b);
}

fn set_a_creator(c: &mut Cpu, b: &mut World) {
    let (me, v) = (Obj(c.a[0]), c.d[0] as u8);
    set_a_via(c, b, v, |b| me.u8(b, Obj::VARIANT));
    rts(c, b);
}

fn set_a_player_native(c: &mut Cpu, b: &mut World) {
    let (me, v) = (Obj(c.a[0]), c.d[0] as u8);
    set_a_player_via(c, b, me, v);
    rts(c, b);
}

fn scene_item(c: &mut Cpu, b: &mut World) {
    let level = rd8(b, A5 - 0x5ac6);
    if (level as i8) < 0 {
        moved(c, level as u32, Size::Byte);
        return rts(c, b);
    }
    if let Some(item) = alloc(c, b, &pools::ITEM) {
        item.set_u8(b, 0x0, 1);
        item.set_u8(b, 0x2, 6);
        item.set_u8(b, 0x3, c.d[0] as u8);
        item.set_u8(b, 0x2f, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

/// 0xd9778: music specific to the scene variant (`+3`, index table 0xd9798, routines 0xd978c: nothing (0xd99f8) or the
/// music of scene 0xe0c5c). D0 = offset in the table, A1 = the routine.
fn scene_music(c: &mut Cpu, b: &mut World) {
    let scene = Obj(c.a[0]);
    let variant = scene.u8(b, Obj::VARIANT) as u32;
    let i = 4 * prog8(b, 0xd9798 + variant) as u32;
    (c.d[0], c.a[1]) = (i, prog32(b, 0xd978c + i));
    play_scene_music(b, scene);
    rts(c, b);
}

/// 0xd99f8: empty routine of the scene tables.
fn nothing(c: &mut Cpu, b: &mut World) {
    rts(c, b);
}

/// 0xce00a: SET object 15 of variant D0.
fn set_15(c: &mut Cpu, b: &mut World) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 0xf);
        let v = c.d[0] as u8;
        set.set_u8(b, 0x3, v);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xdfe26: SOBJ object 14.
fn sobj_14(c: &mut Cpu, b: &mut World) {
    if let Some(o) = alloc(c, b, &pools::SOBJ) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0xe);
        moved(c, 0xe, Size::Byte);
    }
    rts(c, b);
}

/// 0xdfc78: SOBJ object 12 of variant D0, zero speed; flags of D0.b.
fn sobj_12(c: &mut Cpu, b: &mut World) {
    if let Some(o) = alloc(c, b, &pools::SOBJ) {
        o.set_u8(b, 0x0, 1);
        o.set_u8(b, 0x2, 0xc);
        let v = c.d[0] as u8;
        o.set_u8(b, 0x3, v);
        o.set_u16(b, 0x14, 0);
        moved(c, v as u32, Size::Byte);
    }
    rts(c, b);
}

/// 0xc9760: SET object 5 (variant 1), `+0x24` zero.
fn set_5(c: &mut Cpu, b: &mut World) {
    if let Some(set) = alloc(c, b, &pools::SET) {
        set.set_u8(b, 0x0, 1);
        set.set_u8(b, 0x2, 5);
        set.set_u8(b, 0x3, 1);
        set.set_u8(b, 0x24, 0);
        moved(c, 0, Size::Byte);
    }
    rts(c, b);
}

/// 0xd6182: 44 cells of a tile column (A1, step 0x80) filled with 0x00200000 (empty tile).
fn blank_column(c: &mut Cpu, b: &mut World) {
    for _ in 0..0x2c {
        crate::mem::wr32(b, c.a[1], 0x20_0000);
        c.a[1] = c.a[1].wrapping_add(0x80);
    }
    c.d[0] = 0xffff;
    c.d[4] = 0x20_0000;
    rts(c, b);
}

/// 0xceb46: bonus sheet in A4 (guard byte, then 9 bytes): if the guard is positive, each characteristic `+0x151..` of
/// player A0 takes the maximum; A4 passes the sheet.
fn bonus_sheet(c: &mut Cpu, b: &mut World) {
    let guard = rd8(b, c.a[4]);
    c.a[4] = c.a[4].wrapping_add(1);
    if (guard as i8) < 0 {
        c.a[4] = c.a[4].wrapping_add(9);
        return rts(c, b);
    }
    let mut at = c.a[0] + 0x151;
    for _ in 0..9 {
        let v = rd8(b, c.a[4]);
        c.a[4] = c.a[4].wrapping_add(1);
        c.d[1] = c.d[1] & !0xff | v as u32;
        if v as i8 > rd8(b, at) as i8 {
            wr8(b, at, v);
        }
        at += 1;
    }
    c.a[1] = at;
    c.d[0] = 0xffff;
    rts(c, b);
}

/// 0x4b8d0: effect 0xb (variant 0).
fn peff_b(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    peff_via(c, b, me, 0xb, Some(0));
    rts(c, b);
}

/// 0x4b518: effect 8.
fn peff_8(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    peff_via(c, b, me, 8, None);
    rts(c, b);
}

/// 0x51bb6: effect 0x2f (no link).
fn peff_2f(c: &mut Cpu, b: &mut World) {
    effect_2f_via(c, b);
    rts(c, b);
}

/// 0x4cfbc: four 0x13 effects (variant 1, `+0x2f` = 3 down to 0, `+0x5a` = 2), then the screen flash (0x1ac68).
fn peff_13_x4(c: &mut Cpu, b: &mut World) {
    let me = c.a[0];
    c.d[5] = 3;
    while let Some(fx) = alloc(c, b, &pools::PEFF) {
        fx.set_u8(b, 0x0, 1);
        fx.set_u8(b, 0x2, 0x13);
        fx.set_u8(b, 0x3, 1);
        fx.set_u8(b, 0x2f, c.d[5] as u8);
        fx.set_u8(b, 0x5a, 2);
        fx.set_u16(b, 0x50, me as u16);
        let n = (c.d[5] as u16).wrapping_sub(1);
        c.d[5] = c.d[5] & !0xffff | n as u32;
        if n == 0xffff {
            break;
        }
    }
    crate::palette::set_flash_color(b, crate::palette::FLASH_COLOR_RED);
    rts(c, b);
}

fn quake_native(c: &mut Cpu, b: &mut World) {
    let (me, pair) = (Obj(c.a[0]), c.d[0] as u8 == 0);
    quake_via(c, b, me, pair);
    rts(c, b);
}

/// 0x63ae0: if more than two enemy slots remain (`A5+0x7e62`), two enemies of type 7 (variants 0 and 1) linked to the
/// creator through `+0x46`; D0 = 1, otherwise 0 (the allocation is not checked).
fn enemy_7_pair(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    crate::families::small::enemy_pair_kind_via(c, b, me, 7);
    rts(c, b);
}

/// Enemy of type `kind` (variant 5) placed at (D0, D1, D2) = (x, z, y), linked to the creator by `link`; flags of D0.b
/// (`or.b D0b,D0b`).
fn enemy_at(c: &mut Cpu, b: &mut World, kind: u8, link: u32, extra: &[(u32, u8)]) {
    let (me, pos) = (Obj(c.a[0]), (c.d[0] as u16, c.d[1] as u16, c.d[2] as u16));
    let d0 = c.d[0];
    if enemy_at_via(c, b, me, kind, link, extra, pos).is_some() {
        moved(c, d0 & 0xff, Size::Byte);
    }
    rts(c, b);
}

/// 0x5f764: enemy of type 5 at (D0, D1, D2), linked by `+0xa4`.
fn enemy_5_at(c: &mut Cpu, b: &mut World) {
    enemy_at(c, b, 5, 0xa4, &[]);
}

/// 0x57c8c: enemy of type 1 at (D0, D1, D2), linked by `+0xaa` (`+0xac` = 1, `+0x2f` = 0).
fn enemy_1_at(c: &mut Cpu, b: &mut World) {
    enemy_at(c, b, 1, 0xaa, &[(0xac, 1), (0x2f, 0)]);
}

/// 0x97218: boss: `+0x67` = 1, `+0x79` = 0x31, state 0x02020400.
fn boss_state_0202(c: &mut Cpu, b: &mut World) {
    let me = Obj(c.a[0]);
    me.set_u8(b, 0x67, 1);
    me.set_u8(b, 0x79, 0x31);
    me.set_u32(b, 0x4, 0x0202_0400);
    moved(c, 0x0202_0400, Size::Long);
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

pub const NATIVES: &[Native] = &[
    native(0x51c06, "create_shot_weapon", |c, b| { let me = Obj(c.a[0]); weapon_shot(c, b, me); rts(c, b) }, regs::d(0) | regs::d(1) | CREATED),
    native(0x4b7a0, "create_lightning", |c, b| { let (me, v) = (Obj(c.a[0]), c.d[0] as u8); bolt_via(c, b, me, v); rts(c, b) }, CREATED),
    native(0x4dc9a, "create_sparks", |c, b| { let me = Obj(c.a[0]); sparks_via(c, b, me); rts(c, b) }, CREATED),
    native(0x9ef20, "create_shot_9", enemy_shot_9, regs::d(0) | CREATED),
    native(0xcdedc, "create_set_13", set_13, CREATED),
    native(0xc8312, "create_set_scene", scene_set_native, regs::d(0) | CREATED),
    native(0xd981a, "scene_end_variant", scene_exit, regs::d(0) | regs::a(1)),
    native(0xcc352, "create_set_a", set_a_creator, CREATED),
    native(0xdc5fe, "create_item_scene", scene_item, CREATED),
    native(0xd9778, "music_variant", scene_music, regs::d(0) | regs::a(1)),
    native(0xd99f8, "scene_nothing", nothing, regs::ALL),
    native(0xce00a, "create_set_15", set_15, CREATED),
    native(0xcc0ca, "scene_repart", |c, b| {
        let o = Obj(c.a[0]);
        crate::families::scenery::restart_scene(&mut crate::families::cx::Cx { c: &mut *c, b: &mut *b, o });
        rts(c, b)
    }, regs::a(0) | regs::d(7)),
    native(0x4b8d0, "create_effect_b", peff_b, CREATED),
    native(0x4b518, "create_effect_8", peff_8, CREATED),
    native(0x51bb6, "create_effect_2f", peff_2f, CREATED),
    native(0x4cfbc, "create_effects_13", peff_13_x4, CREATED | regs::d(5)),
    native(0x4b340, "create_effects_7", quake_native, CREATED),
    native(0x63ae0, "create_enemies_7", enemy_7_pair, CREATED | regs::d(0)),
    native(0x5f764, "create_enemy_5", enemy_5_at, CREATED),
    native(0x57c8c, "create_enemy_1", enemy_1_at, CREATED),
    native(0x97218, "boss_state_0202", boss_state_0202, regs::CCR),
    native(0xc9760, "create_set_5", set_5, CREATED),
    native(0xd6182, "empty_column", blank_column, regs::a(1) | regs::d(0) | regs::d(4)),
    native(0xceb46, "sheet_bonus", bonus_sheet, regs::a(1) | regs::a(4) | regs::d(0) | regs::d(1)),
    native(0xdfe26, "create_sobj_14", sobj_14, CREATED),
    native(0xdfc78, "create_sobj_12", sobj_12, CREATED),
    native(0xdf688, "create_sobj_8", |c, b| { let me = Obj(c.a[0]); sobj_8_via(c, b, me); rts(c, b) }, CREATED),
    native(0xcc38e, "create_set_a_player", set_a_player_native, CREATED | regs::a(4)),
];

const CREATED: u32 = regs::a(1) | regs::CCR;
