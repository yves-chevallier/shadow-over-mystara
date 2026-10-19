//! Boss 11/12, états actifs hors attaques : arrivée (groupe 0), attente (6), poursuite (2, 8, 10, 12) et les tests qui
//! déclenchent une attaque ou une manœuvre.

use super::*;

// --- groupe 0 : arrivée --------------------------------------------------------------------------------------

/// 0x8d7b8 : le serpent surgit à droite de l'écran, retombe, rebondit, se met en marche.
pub(super) fn intro(x: &mut Cx) {
    match dispatch0(x, 0x8d7c6, f::SUB2) {
        0x8d7ce => {
            x.add8(f::SUB2, 2);
            x.set8(NO_ZONE, 0xff);
            let px = x.a5_u16(var::SCROLL_X).wrapping_add(0x190);
            setw(x, 0, px);
            x.set16(0x8, px);
            let py = x.a5_u16(var::SCROLL_Y).wrapping_add(0x40);
            setw(x, 0, py);
            x.set16(0x10, py);
            x.set16(0xc, 0);
            x.set16(0x14, (-0x200i16) as u16);
            x.set16(0x16, 0);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x20i16) as u16);
            x.set32(0x1c, 0);
            body(x, 6);
            sync(x);
            fall(x)
        }
        0x8d818 => fall(x),
        0x8d858 => {
            x.set8(NO_ZONE, 0xff);
            x.call(call::MOVE_X);
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                body(x, 0);
            }
            sync(x);
        }
        _ => {
            if Fl::sub8(x.a5_u8(0x2c), 0xc).lt() {
                x.call(call::STEP);
                return sync(x);
            }
            x.set8(0, 1);
            x.add8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            x.call(rom::IMMUNITIES);
            x.set16(ANIM_KIND, 0xffff);
            body(x, 0)
        }
    }
}

/// 0x8d818 : chute jusqu'au sol.
fn fall(x: &mut Cx) {
    x.set8(NO_ZONE, 0xff);
    x.call(call::MOVE_XZ);
    if !tst16(x, 0xc).mi() {
        return;
    }
    x.set16(0xc, 0);
    x.add8(f::SUB2, 2);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
    let v = asr_w(x.u16(0x14), 2);
    setw(x, 0, v);
    x.set16(0x14, v);
    body(x, 7);
    x.c.d[0] = 8;
    x.call(call::SOUND_B);
    x.call(0xecc0a);
}

// --- groupe 6 : sans joueur ----------------------------------------------------------------------------------

/// 0x8d8bc : attend ; un joueur revient (vie > 0) et le compteur `+0xb6` repart.
pub(super) fn lone(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) != 0 {
        x.c.d[0] = 2;
        x.call(0x43bc);
        if !zero(x) {
            if tst8(x, 0xb6).gt() {
                sub8(x, 0xb6, 1);
            } else {
                x.set8(0xb6, 0);
                return to_chase(x);
            }
        }
    }
    if x.u8(SUB3) == 0 {
        x.add8(SUB3, 2);
        return body(x, 0);
    }
    x.call(call::STEP);
    sync(x);
}

// --- groupes 2, 8, 10, 12 : poursuite --------------------------------------------------------------------------

/// 0x8eb52 : délai de marche selon la difficulté.
fn walk_delay(x: &mut Cx) {
    let level = x.a5_u16(var::DIFFICULTY);
    setw(x, 0, level);
    let v = x.rom8(0x8eb5e + level as u32);
    x.set8(0x71, v);
}

/// 0x8eb7e : nombre de pas tiré.
fn walk_steps(x: &mut Cx) {
    x.call(0x19a88);
    let i = x.c.d[0] as u16 & 0x1f;
    setw(x, 0, i);
    let v = x.rom8(0x8eb90 + i as u32);
    x.set8(0x6b, v);
}

/// 0x8d902.
pub(super) fn chase(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        return to_wait(x);
    }
    x.c.d[0] = 2;
    x.call(0x43bc);
    if zero(x) {
        return to_wait(x);
    }
    if tst16(x, 0xc).gt() {
        return to_attack_2(x);
    }
    let mut aim = true;
    if x.u8(SUB3) == 0 {
        x.add8(SUB3, 2);
        walk_steps(x);
        walk_delay(x);
    } else if sub8(x, 0x71, 1).gt() {
        aim = false;
    } else {
        walk_delay(x);
        if sub8(x, 0x6b, 1).gt() {
        } else {
            walk_steps(x);
        }
    }
    if aim {
        x.call(call::ASK_TARGET);
        speed(x, 0x17);
    }
    x.call(call::MOVE_TABLE_XY);
    pose(x);
    if turn_needed(x) {
        return to_turn(x);
    }
    x.c.d[0] = 0;
    if hit_near(x) {
        return pick(x, 0x8db68);
    }
    if far_roll(x) {
        return pick(x, 0x8db88);
    }
    if near_roll(x) {
        return pick(x, 0x8db78);
    }
    if off_screen(x) {
        return to_attack_2(x);
    }
}

/// 0x8d998 : animation de marche selon l'orientation.
fn pose(x: &mut Cx) {
    let w = x.u16(ANIM_KIND);
    if w == 1 {
        if x.anim_step() {
            body(x, 0);
        }
        return sync(x);
    }
    if w == 0 || w == 8 || w == 9 {
        let mut d1 = x.u8(0x2d) as u16;
        x.c.d[1] = d1 as u32;
        if x.u8(f::FLIP) != 0 {
            d1 = d1.wrapping_add(0x10);
        }
        d1 &= 0x1f;
        setw(x, 1, d1);
        let n = if Fl::sub16(d1, 0x10).gt() { 8 } else { 9 };
        x.c.d[0] = n;
        if n as u8 != x.u8(ANIM_N) {
            return body(x, n);
        }
        x.call(call::STEP);
        return sync(x);
    }
    body(x, 0);
    sync(x)
}

/// 0x8da02 : le serpent est hors de l'écran.
fn off_screen(x: &mut Cx) -> bool {
    let mut d0 = x.a5_u16(var::SCROLL_X);
    if x.a5_u8(var::CAMERA_SELECT) != 0 {
        d0 = x.a5_u16(0x556);
    }
    setw(x, 0, d0);
    let here = x.u16(0x8);
    if Fl::sub16(d0, here).ge() {
        x.c.d[0] = 1;
        return true;
    }
    d0 = d0.wrapping_add(0x1c0);
    setw(x, 0, d0);
    if Fl::sub16(d0, here).ge() {
        x.c.d[0] = 0;
        return false;
    }
    x.c.d[0] = 1;
    true
}

/// 0x8da28 : la cible est derrière le serpent.
fn turn_needed(x: &mut Cx) -> bool {
    let w = x.u16(f::TARGET);
    setw(x, 0, w);
    if (w as i16) >= 0 {
        x.c.d[0] = 0;
        return false;
    }
    x.c.a[1] = w as i16 as i32 as u32;
    if rd8(x.b, x.c.a[1]) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let mut d = rd16(x.b, x.c.a[1] + 8).wrapping_sub(x.u16(0x8));
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    setw(x, 0, d);
    if Fl::sub16(d, 0xffd0).gt() {
        x.c.d[0] = 0;
        return false;
    }
    x.c.d[0] = 1;
    true
}

/// 0x8dafe : un joueur tout près devant.
fn hit_near(x: &mut Cx) -> bool {
    setw(x, 0, 0xfff0);
    setw(x, 1, 0x20);
    setw(x, 2, 0x10);
    x.call(0xb5e0);
    let ok = !zero(x);
    x.c.d[0] = ok as u32;
    ok
}

/// 0x8dae2 : rang de vie (0 à 3 environ) : nombre de fois que les PV maximaux tiennent dans quatre fois les PV.
pub(super) fn life_rank(x: &mut Cx) {
    x.c.d[1] = x.u16(0x60) as u32;
    x.c.d[2] = 0;
    x.c.d[3] = x.u16(f::HP) as u32;
    x.c.d[3] <<= 2;
    x.c.d[0] = 0xffff_ffff;
    for _ in 0..0x20000 {
        x.c.d[2] = x.c.d[2].wrapping_add(x.c.d[1]);
        let w = (x.c.d[0] as u16).wrapping_add(1);
        setw(x, 0, w);
        if !Fl::sub32(x.c.d[2], x.c.d[3]).lt() {
            return;
        }
    }
}

/// 0x8da56 : tirage selon le rang de vie (table 0x8da88).
fn near_roll(x: &mut Cx) -> bool {
    life_rank(x);
    let i = (x.c.d[0] as u16 & 3) * 4;
    setw(x, 0, i);
    let mask = rom32(x, 0x8da88 + i as u32);
    x.c.d[1] = mask;
    x.call(0x19a88);
    let r = x.c.d[0] as u16 & 1;
    setw(x, 0, r);
    if r == 0 {
        x.c.d[0] = 0;
        return false;
    }
    x.call(0x19a88);
    let b = x.c.d[0] as u16 & 0x1f;
    setw(x, 0, b);
    if mask >> b & 1 != 0 {
        x.c.d[0] = 1;
        return true;
    }
    x.c.d[0] = 0;
    false
}

/// 0x8da98 : tirage selon le rang de vie (table 0x8dad2), puis un joueur dans la boîte.
fn far_roll(x: &mut Cx) -> bool {
    life_rank(x);
    let i = (x.c.d[0] as u16 & 3) * 4;
    setw(x, 0, i);
    let mask = rom32(x, 0x8dad2 + i as u32);
    x.c.d[1] = mask;
    x.call(0x19a88);
    let b = x.c.d[0] as u16 & 0x1f;
    setw(x, 0, b);
    if mask >> b & 1 != 0 {
        setw(x, 0, 0xffe0);
        setw(x, 1, 0xa0);
        setw(x, 2, 0x80);
        x.call(0xb5e0);
        if !zero(x) {
            x.c.d[0] = 1;
            return true;
        }
    }
    x.c.d[0] = 0;
    false
}

/// 0x8db2a : tire une attaque dans la table `table` (16 octets), ou renonce.
fn pick(x: &mut Cx, table: u32) {
    x.c.a[4] = table;
    if !roll(x) {
        x.call(0x19a88);
        let i = x.c.d[0] as u16 & 0xf;
        setw(x, 0, i);
        let v = x.data8(table + i as u32);
        setb(x, 1, v);
        if (v as i8) >= 0 {
            x.set32(f::STATE, 0x0204_0000);
            x.set8(f::SUB2, v);
            return;
        }
    }
    life_rank(x);
    let i = x.c.d[0] as u16 & 3;
    setw(x, 0, i);
    let v = x.rom8(0x8db64 + i as u32);
    setb(x, 0, v);
    if v == 0 {
        return;
    }
    x.set8(0xb6, v);
    to_wait(x)
}

/// 0x8db98 : tirage « renoncer » selon la difficulté et le nombre de joueurs ; vrai si le bit tombe.
fn roll(x: &mut Cx) -> bool {
    x.call(0x19a88);
    let r = x.c.d[0] as u16 & 7;
    setw(x, 0, r);
    let n = x.a5_u8(var::PLAYERS_ALIVE).wrapping_sub(1);
    setb(x, 1, n);
    let d1 = (x.c.d[1] as u16 & 2).wrapping_mul(4);
    setw(x, 1, d1);
    let level = (x.a5_u16(var::DIFFICULTY) >> 8) as u8;
    setb(x, 2, level);
    let d2 = (x.c.d[2] as u16) >> 2;
    setw(x, 2, d2);
    let d1 = d1.wrapping_add(d2);
    setw(x, 1, d1);
    let v = x.rom8(0x8dbc0 + d1 as u32);
    setb(x, 1, v);
    v >> (r & 7) & 1 != 0
}
