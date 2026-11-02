use super::*;

const ANIMS: u32 = 0x16e920;
/// Offsets (X, Z, Y) of the 16 effects (8 bytes per row).
const OFFSETS: u32 = 0xa1438;
/// Number of enemies by (A5+0x7f92 / 2) * 4 + A5+0x35.
const GROUP_SIZES: u32 = 0xa15ae;
/// Relative positions (X, Y) of the enemies: 2 pairs per row, drawn at random.
const GROUP_OFFSETS: u32 = 0xa15ee;
const GROUP_SIZE_SELECTOR: u32 = 0x7f92;
const SPAWN_ENEMIES_AT: u16 = 0x20;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    let source = x.linked(0x5c);
    if x.u8(0x3) == 0 {
        x.set8(f::BOX_SET, 7);
        x.set8(f::ATTACK_ID, 0x2f);
        x.set16(0x58, 0x50);
        x.set16(0x5a, 1);
        x.set16(0x56, 0);
        copy_position16(x, source);
        x.anim_start(ANIMS, 2);
        x.reinsert();
        x.set32(0x34, 0);
        active(x);
    } else {
        copy_position16(x, source);
        let row = OFFSETS + ((x.u16(0x56) << 3) as u32);
        for (off, column) in [(0x8, 0), (0xc, 2), (0x10, 4)] {
            let d = x.rom16(row + column);
            x.add16(off, d);
        }
        x.anim_start(ANIMS, 1);
        x.reinsert();
    }
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x3) != 0 {
        if x.anim_step() {
            x.add8(0x4, 2);
        }
        return;
    }
    x.add16(0x58, 0xffff);
    if x.u16(0x58) == 0 {
        x.add8(0x4, 2);
    }
    if x.u16(0x58) == SPAWN_ENEMIES_AT {
        spawn_enemies(x);
    }
    x.add16(0x5a, 0xffff);
    if x.u16(0x5a) == 0 {
        x.set16(0x5a, 2);
        spawn_effect(x);
    }
}

/// Launches the next effect (variant 1) and advances `+0x56` modulo 16.
fn spawn_effect<R: Bench>(x: &mut Cx<'_, R>) {
    if let Some(e) = x.alloc(&pools::ESHL) {
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 0x18);
        e.set_u8(x.b, 3, 1);
        let (n, source) = (x.u16(0x56), x.u16(0x5c));
        e.set_u16(x.b, 0x56, n);
        e.set_u16(x.b, 0x5c, source);
        x.set16(0x56, n.wrapping_add(1) & 0xf);
    }
}

/// Spawns the group of enemies around the creator.
fn spawn_enemies<R: Bench>(x: &mut Cx<'_, R>) {
    let mut d0 = x.a5_u8(0x35) as u16;
    if d0 != 0 {
        d0 = d0.wrapping_sub(1) & 3;
    }
    d0 = d0.wrapping_add((x.a5_u16(GROUP_SIZE_SELECTOR) >> 1) << 2);
    let mut count = x.rom8(GROUP_SIZES.wrapping_add(d0 as i16 as i32 as u32)) as u16;
    count = count.wrapping_sub(1);
    let mut label: u16 = 1;
    let rng = x.random();
    let mut table = GROUP_OFFSETS + ((rng as u32 & 7) << 3);
    loop {
        let Some(e) = x.alloc(&pools::ENEMY) else { return };
        e.set_u8(x.b, 0, 1);
        e.set_u8(x.b, 2, 1);
        e.set_u8(x.b, 3, 4);
        let source = x.linked(0x5c);
        let (sx, sy) = (source.u16(x.b, 0x8), source.u16(x.b, 0x10));
        let (dx, dy) = (x.data16(table), x.data16(table + 2));
        table += 4;
        e.set_u16(x.b, 0x8, sx.wrapping_add(dx));
        e.set_u16(x.b, 0x10, sy.wrapping_add(dy));
        e.set_u16(x.b, 0x42, label);
        label = label.wrapping_add(0x40);
        count = count.wrapping_sub(1);
        if count == 0xffff {
            return;
        }
    }
}
