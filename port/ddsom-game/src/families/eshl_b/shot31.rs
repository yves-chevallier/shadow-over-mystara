use super::*;

const ANIMS: u32 = 0x16f504;
const SOUND_SPAWN: crate::sound::Sound = crate::sound::CMD_334_434;
const SOUND_HIT: crate::sound::Sound = crate::sound::CMD_138;
const SEARCH: u8 = 0;
const HUNT: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(0x5c) == 0 {
        return kill(x);
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => fly_away(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 7);
    x.set8(f::ATTACK_POWER, 5);
    let source = x.linked(0x5c);
    copy_position16(x, source);
    let flip = source.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    x.add16(0x10, 0xffff);
    x.set8(0x42, 0x30);
    let (dx, angle) = if flip != 0 { (0x4a, 8) } else { (-0x4ai16 as u16, 0x18) };
    x.set8(0x2d, angle);
    x.add16(0x8, dx);
    x.add16(0xc, 0x32);
    x.anim_start(ANIMS, 0);
    speed_table(x, 0x30);
}

fn fly_away<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    x.cull_offscreen();
}

/// Bounce sound when the animation raises its flag (`+0x33`).
fn animation_cue<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x33) != 0 {
        x.set8(0x33, 0);
        crate::sound::play(x.b, SOUND_SPAWN, x.o);
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let source = x.linked(0x5c);
    if (source.u16(x.b, 0x4) as i16) >= 0x406 {
        return fly_away(x);
    }
    match x.u8(0x5) {
        SEARCH => {
            animation_cue(x);
            x.add8(0x42, 0xff);
            if (x.u8(0x42) as i8) < 0 {
                x.add8(0x5, 2);
                x.set8(0x58, 0xfc);
                if source.u16(x.b, f::OWNER) != 0 {
                    let target = link_of(x, source, f::OWNER);
                    let d = x.u16(0x10).wrapping_sub(target.u16(x.b, 0x10));
                    if (d as i16) < 0 {
                        x.set8(0x58, 4);
                    }
                }
            }
            x.anim_step();
            x.move_by_table_xy();
        }
        HUNT => {
            animation_cue(x);
            x.add8(0x42, 0xff);
            if (x.u8(0x42) as i8) < 0 {
                x.set8(0x42, 0xa);
                let to = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
                let angle = crate::aids::direction_from(x.b, x.o, to).wrapping_add(x.u8(0x58)) & 0x1f;
                x.set8(0x2d, angle);
            }
            let touching = |x: &mut Cx<'_, R>, a: u16, b: u16| a.wrapping_sub(b).wrapping_add(8) <= 0x20 && x.a5_u8(GAME_PAUSED) == 0;
            let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
            let (mx, my) = (x.u16(0x8), x.u16(0x10));
            let hit = source.u8(x.b, 0x4) != 4 && touching(x, sx, mx) && touching(x, sy, my);
            if hit {
                crate::sound::play(x.b, SOUND_HIT, x.o);
                x.set8(0x4, st::FREE);
                source.set_u8(x.b, 0xb5, 0);
            } else {
                x.move_by_table_xy();
                x.anim_step();
            }
        }
        _ => {}
    }
    x.reinsert();
}
