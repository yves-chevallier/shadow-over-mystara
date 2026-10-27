//! Effets des joueurs (PEFF), neuvième série : types 30 (tempête), 31 (tourbillon) et 32 (cible de la
//! foudre). Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Courant;

const SPEEDS: u32 = 0x1c89b6;

fn signed(v: u16, negate: bool) -> u16 {
    if negate { v.wrapping_neg() } else { v }
}

/// Type 30 (0x4e79c) : la tempête (magie 18, kind 0x1e). La variante 0 est le nuage (état `+5` : 0 gronde,
/// 4 attend la fin, 8 secoue, 0xc se dissipe) qui crée seize éclairs (variante 1, rang `+2f`) puis
/// quatre étincelles (variante 2) ; les éclairs rejoignent un point de la cible de façon aléatoire.
mod storm {
    use super::*;

    const ANIMS: u32 = 0x1bbe14;
    const OFFSETS_A: u32 = 0x4e8ee;
    const OFFSETS_B: u32 = 0x4eae6;
    const SFX_RUMBLE: u32 = 0xe996c;
    const SFX_BOLT: u32 = 0xe99c8;
    const SFX_BEGIN: u32 = 0xe9a32;
    const SFX_END: u32 = 0xea032;
    const KIND: u8 = 0x1e;

    pub fn run(x: &mut Cx) {
        let variant = x.u8(0x3);
        match x.state(0x4e7aa) {
            0 => match variant {
                0 => init_cloud(x),
                1 => init_bolt(x),
                _ => init_spark(x),
            },
            2 => match variant {
                0 => cloud(x),
                1 => bolt(x),
                _ => spark(x),
            },
            _ => x.free_peff(),
        }
    }

    fn init_cloud(x: &mut Cx) {
        x.add8(0x4, 2);
        follow_owner(x);
        x.add16(0xc, 0x20);
        x.add16(0x10, 2);
        x.set16(0x58, 0x20);
        x.anim(ANIMS, 0);
        x.reinsert();
    }

    fn init_bolt(x: &mut Cx) {
        x.add8(0x4, 2);
        let parent = x.link_a1(0x5c);
        let (px, py) = (parent.u16(x.b, 0x8), parent.u16(x.b, 0x10));
        x.set16(0x54, px);
        x.set16(0x56, py.wrapping_add(1));
        x.set16(0x8, px);
        x.set16(0x10, py);
        let a = x.rand_and(0x1ff).wrapping_sub(0xff) as u16;
        x.add16(0x8, a);
        let mut d2 = a;
        let b = x.rand_and(0x3f).wrapping_sub(0x1f) as u16;
        x.add16(0x10, b);
        let b = b.wrapping_add(0x1f);
        d2 = d2.wrapping_add(0xff).wrapping_add(b);
        let vz = ((d2 as i16) >> 4) as u16;
        x.c.d[0] = b as u32;
        x.c.d[2] = d2 as u32;
        x.set16(0x1a, vz);
        x.set16(0x18, 0);
        let anim = x.rand_and(1) + 1;
        x.anim(ANIMS, anim);
    }

    fn init_spark(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        let px = owner.u16(x.b, 0x8);
        x.set16(0x8, px);
        x.set16(0xc, 0);
        let py = owner.u16(x.b, 0x10);
        x.set16(0x10, py);
        let k = (x.u8(0x2f) as u32) << 3;
        x.c.d[0] = k;
        let (dx, dz, dy) = (x.rom16(OFFSETS_A + k), x.rom16(OFFSETS_A + k + 2), x.rom16(OFFSETS_A + k + 4));
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, dy);
        x.set32(0x4c, SPEEDS);
        x.set16(0x58, 2);
        let angle = x.u8(0x2f).wrapping_shl(3) & 0x1f;
        x.set8(0x2d, angle);
        let n = x.u8(0x2f) as u32;
        x.anim(ANIMS, n);
        x.reinsert();
    }

    // --- états actifs -------------------------------------------------------------------------------

    fn cloud(x: &mut Cx) {
        x.c.d[0] = x.u8(0x5) as u32;
        match x.u8(0x5) {
            0 => {
                x.add16(0x58, 0xffff);
                if (x.u16(0x58) as i16) < 0 {
                    x.add8(0x5, 4);
                    return spawn_bolts(x);
                }
                let hidden = x.u8(0x1) & 2 == 0;
                x.set_bit(0x1, 1, hidden);
                let owner = x.link_a1(f::OWNER);
                let d0 = owner.u16(x.b, 0x168);
                x.set_d0w(d0);
                x.call(crate::camera_modes::SHAKE_A.entry);
                x.call(SFX_RUMBLE);
            }
            4 => {
                if x.a5_u8(0x3d) == 0 {
                    return end(x);
                }
                let owner = x.link_a1(f::OWNER);
                let d0 = owner.u16(x.b, 0x168);
                x.set_d0w(d0);
                x.call(crate::camera_modes::SHAKE_B.entry);
                if x.u8(0x5a) != 0 {
                    return;
                }
                x.set16(0x58, 0x20);
                x.add8(0x5, 4);
                x.call(SFX_BEGIN);
            }
            8 => {
                toggle(x, 0x9, 1 << 1);
                toggle(x, 0xd, 1);
                x.add16(0x58, 0xffff);
                if (x.u16(0x58) as i16) < 0 {
                    x.add8(0x5, 4);
                    let owner = x.link_a1(f::OWNER);
                    let d0 = owner.u16(x.b, 0x168);
                    x.set_d0w(d0);
                    x.call(crate::camera_modes::SHAKE_C.entry);
                    spawn_sparks(x);
                    shake_tail(x);
                }
            }
            _ => shake_tail(x),
        }
    }

    fn toggle(x: &mut Cx, off: u32, mask: u8) {
        let v = x.u8(off) ^ mask;
        x.set8(off, v);
    }

    /// Sous-état 0xc : la tempête continue de trembler tant que le jeu est figé.
    fn shake_tail(x: &mut Cx) {
        x.link_a1(f::OWNER);
        toggle(x, 0x9, 1);
        toggle(x, 0xd, 1);
        if x.a5_u8(0x3d) == 0 {
            return end(x);
        }
        x.anim_step();
    }

    fn end(x: &mut Cx) {
        x.add8(0x4, 2);
        x.anim_step();
        x.call(SFX_END);
    }

    /// `0x4ec02` : seize éclairs.
    fn spawn_bolts(x: &mut Cx) {
        for rank in (0..16u8).rev() {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, KIND);
            o.set_u8(x.b, 3, 1);
            o.set_u8(x.b, 0x2f, rank);
            o.set_u16(x.b, 0x5c, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            x.add8(0x5a, 1);
        }
    }

    /// `0x4ec36` : quatre étincelles.
    fn spawn_sparks(x: &mut Cx) {
        for rank in (0..4u8).rev() {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, KIND);
            o.set_u8(x.b, 3, 2);
            o.set_u8(x.b, 0x2f, rank);
            o.set_u16(x.b, 0x5c, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
        }
    }

    fn move_towards(x: &mut Cx, at: u32, target: u16, step: u32) {
        let here = x.u16(at);
        if target == here {
            return;
        }
        if (target as i16) > (here as i16) {
            let v = x.u32(at).wrapping_add(step);
            x.set32(at, v);
            if (target as i16) <= (x.u16(at) as i16) {
                x.set16(at, target);
            }
        } else {
            let v = x.u32(at).wrapping_sub(step);
            x.set32(at, v);
            if (target as i16) >= (x.u16(at) as i16) {
                x.set16(at, target);
            }
        }
    }

    fn bolt(x: &mut Cx) {
        x.reinsert();
        if x.u8(0x5) != 0 {
            let parent = x.link_a1(0x5c);
            let k = (x.u8(0x2f) as u32) << 2;
            x.c.d[0] = k;
            let (dx, dz) = (x.rom16(OFFSETS_B + k), x.rom16(OFFSETS_B + k + 2));
            x.set16(0x8, dx);
            x.set16(0xc, dz);
            let (px, pz) = (parent.u16(x.b, 0x8), parent.u16(x.b, 0xc));
            let py = parent.u16(x.b, 0x10);
            x.set16(0x10, py);
            x.add16(0x10, 0xffff);
            x.add16(0x8, px);
            x.add16(0xc, pz);
            if parent.u8(x.b, 0x5) == 0xc {
                x.add8(0x4, 2);
            }
            return;
        }
        x.anim_step();
        let vz = x.u16(0x18).wrapping_add(x.u16(0x1a));
        x.set16(0x18, vz);
        let d0 = (((vz as u32) << 16) as i32 >> 8) as u32;
        let z = x.u32(0xc).wrapping_add(d0);
        x.set32(0xc, z);
        let tx = x.u16(0x54);
        let ty = x.u16(0x56);
        move_towards(x, 0x8, tx, 0x60000);
        move_towards(x, 0x10, ty, 0x8000);
        if x.u16(0x8) != tx || x.u16(0x10) != ty {
            return;
        }
        x.add8(0x5, 2);
        let parent = x.link_a1(0x5c);
        let n = parent.u8(x.b, 0x5a).wrapping_sub(1);
        parent.set_u8(x.b, 0x5a, n);
        x.set8(0x2f, n);
        x.call(SFX_BOLT);
        let k = (n as u32) << 2;
        x.c.d[0] = k;
        let (dx, dz) = (x.rom16(OFFSETS_B + k), x.rom16(OFFSETS_B + k + 2));
        x.set16(0x8, dx);
        x.set16(0xc, dz);
        let parent = x.link_a1(0x5c);
        let (px, pz) = (parent.u16(x.b, 0x8), parent.u16(x.b, 0xc));
        let py = parent.u16(x.b, 0x10);
        x.set16(0x10, py);
        x.add16(0x10, 0xffff);
        x.add16(0x8, px);
        x.add16(0xc, pz);
    }

    fn spark(x: &mut Cx) {
        if x.a5_u8(0x3d) == 0 {
            x.add8(0x4, 2);
            return;
        }
        x.add16(0x58, 0xffff);
        if (x.u16(0x58) as i16) < 0 {
            let a = x.u8(0x2d).wrapping_add(1) & 0x1f;
            x.set8(0x2d, a);
            x.set16(0x58, 2);
        }
        x.reinsert();
        x.move_by_table_xy();
        let k = x.u8(0x2f);
        if k & 1 == 0 {
            let vx = x.u16(0x14);
            x.set16(0x18, if k & 2 == 0 { vx } else { ((vx as i16) >> 1) as u16 });
        } else {
            let vx = x.u16(0x14).wrapping_neg();
            x.set16(0x18, if k & 2 == 0 { vx } else { ((vx as i16) >> 1) as u16 });
        }
        x.move_z();
    }
}

/// Type 31 (0x4ec82) : tourbillon (magie 19, kind 0x1f) : variante 0 = l'œil qui aspire (`+5` : 0 grandit,
/// puis il balaie la zone), variante 1 = un satellite (rang `+0x5a`, angle `+0x2d`) qui est tiré en spirale.
mod whirl {
    use super::*;

    const ANIMS: u32 = 0x1bbfda;
    const OFFSETS: u32 = 0x4ed06;
    const FLIPS: u32 = 0x4eda6;
    const SPAWN_TABLE: u32 = 0x4eebe;
    const POWER: u32 = 0x4efea;
    const SFX_START: u32 = 0xe9d3e;
    const SFX_LOOP: u32 = 0xe9da8;
    const KIND: u8 = 0x1f;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x4ec90) {
            0 if !child => init_eye(x),
            0 => init_satellite(x),
            2 if !child => eye(x),
            2 => {
                x.move_xz();
                x.add8(0x58, 0xff);
                if x.u8(0x58) == 0 {
                    x.add8(0x4, 2);
                }
            }
            _ => x.free_peff(),
        }
    }

    fn init_eye(x: &mut Cx) {
        x.add8(0x4, 2);
        x.set8(0x59, 0);
        let owner = follow_owner(x);
        x.add16(0x10, 0xffff);
        let (mut dx, mut dz) = (-0x37i16 as u16, 0x4au16);
        if owner.u8(x.b, 0x15e) != 0x25 {
            let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
            x.c.d[0] = row;
            dx = x.rom16(OFFSETS + row);
            dz = x.rom16(OFFSETS + row + 2);
        }
        x.c.d[1] = dx as u32;
        x.c.d[2] = dz as u32;
        let dx = signed(dx, owner.u8(x.b, f::FLIP) != 0);
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        x.add16(0x10, 0xffff);
        x.call(SFX_START);
    }

    fn init_satellite(x: &mut Cx) {
        x.add8(0x4, 2);
        let owner = x.link_a1(f::OWNER);
        x.copy_pos(owner);
        x.set8(0x58, 8);
        let k = x.u8(0x2d) as u32;
        x.c.d[0] = k;
        let flip = x.rom8(FLIPS + k);
        x.set8(f::FLIP, flip);
        let n = x.u8(0x5a) as i32;
        let (mut d4, mut d5) = (0u16, 0u16);
        let mut left = n;
        loop {
            x.set_d0w(0x21);
            let n = x.c.d[0] as u16;
            x.select_speed(&crate::objects::motion::SPEEDS_A, n);
            d4 = d4.wrapping_add(x.u16(0x14));
            d5 = d5.wrapping_add(x.u16(0x1c));
            left -= 1;
            if left < 0 {
                break;
            }
        }
        x.c.d[4] = d4 as u32;
        x.c.d[5] = d5 as u32;
        x.add16(0x8, ((d4 as i16) >> 8) as u16);
        x.add16(0xc, ((d5 as i16) >> 8) as u16);
        x.set_d0w(0x19);
        let n = x.c.d[0] as u16;
        x.select_speed(&crate::objects::motion::SPEEDS_A, n);
        let vy = x.u16(0x1c);
        x.set16(0x18, vy);
        let vx = x.u16(0x14).wrapping_neg();
        x.set16(0x14, vx);
        let vz = x.u16(0x18).wrapping_neg();
        x.set16(0x18, vz);
        let angle = x.u8(0x2d) as u32;
        x.anim(ANIMS, angle);
        x.reinsert();
    }

    fn eye(x: &mut Cx) {
        if x.u8(0x5) == 0 {
            let owner = x.link_a1(f::OWNER);
            if owner.u16(x.b, 0x168) <= 0x10 {
                x.add8(0x5, 2);
                x.set8(0x5b, 4);
                x.set8(0x5c, 3);
                x.set8(0x5d, 1);
                x.set_a5_u16(0x4ba, 0x100);
                x.call(crate::palette::FLASH_COLOR_WHITE.entry);
                x.call(SFX_LOOP);
                x.anim(ANIMS, 0x20);
                return x.reinsert();
            }
            // le tourbillon se forme : un satellite par image
            let k = x.u8(0x59) as u32;
            x.add8(0x59, 1);
            let n = x.u8(0x59) & 0x1f;
            x.set8(0x59, n);
            x.c.d[0] = k * 2;
            let angle = x.rom8(SPAWN_TABLE + 2 * k);
            let count = x.rom8(SPAWN_TABLE + 2 * k + 1);
            if let Some(o) = x.alloc(&pools::PEFF) {
                o.set_u8(x.b, 0, 1);
                o.set_u8(x.b, 2, KIND);
                o.set_u8(x.b, 3, 1);
                o.set_u8(x.b, 0x5a, count);
                o.set_u8(x.b, 0x2d, angle);
                o.set_u16(x.b, f::OWNER, x.o.0 as u16);
            }
            return;
        }
        if x.u8(0x5c) != 0 {
            x.add8(0x5b, 0xff);
            if x.u8(0x5b) != 0 {
                return;
            }
            x.set8(0x5b, 4);
            x.set_a5_u16(0x4ba, 0);
            let v = x.u8(0x5d) ^ 1;
            x.set8(0x5d, v);
            if v != 0 {
                x.set_a5_u16(0x4ba, 0x100);
            }
            x.add8(0x5c, 0xff);
            return;
        }
        if !x.anim_step() {
            return;
        }
        x.add8(0x4, 2);
        x.set_a5_u16(0x4ba, 0);
        sweep(x);
    }

    /// `0x4eefe` : attire (et blesse) tous les monstres (boss, puis ennemis) dans la zone devant le lanceur.
    fn sweep(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        for (first, count) in [(0x3a1eu32, 3u32), (0x299e, 22)] {
            let mut a2 = 0xff8000 + first;
            for _ in 0..count {
                hit(x, owner, Obj(a2));
                a2 += 0xc0;
            }
        }
    }

    fn hit(x: &mut Cx, player: Obj, m: Obj) {
        if m.u8(x.b, 0) == 0 || m.u8(x.b, 1) == 0 || m.u16(x.b, 0x62) == 0 || m.u8(x.b, 0x40) != 0 {
            return;
        }
        if m.u32(x.b, 0x94) >> 16 & 1 != 0 {
            return;
        }
        if m.u16(x.b, 0x4) == 0x400 && matches!(m.u8(x.b, 0x49), 0x68 | 0x6c) {
            return;
        }
        let px = player.u16(x.b, 0x8);
        let mx = m.u16(x.b, 0x8);
        if player.u8(x.b, f::FLIP) != 0 {
            let d0 = px.wrapping_sub(0x10);
            if d0 > mx || d0.wrapping_add(0xe0) <= mx {
                return;
            }
        } else {
            let d0 = px.wrapping_add(0x10);
            if d0 <= mx || d0.wrapping_sub(0xe0) > mx {
                return;
            }
        }
        m.set_u32(x.b, 0x4, 0x0400_0000);
        m.set_u8(x.b, 0x1, 1);
        m.set_u8(x.b, 0x49, 0x68);
        let (class_id, variant_id) = (player.u8(x.b, 0x2), player.u8(x.b, 0x3));
        m.set_u8(x.b, 0x48, class_id);
        m.set_u8(x.b, 0x4b, variant_id);
        m.set_u16(x.b, 0x54, player.0 as u16);
        m.set_u8(x.b, 0x87, 0x78);
        if player.u8(x.b, 0x15e) == 0x25 {
            let class = player.u8(x.b, 0x10c) as u32;
            let v = x.rom8(POWER + class);
            m.set_u8(x.b, 0x87, v);
        }
    }
}

/// Type 32 (0x4f026) : cible de la foudre (kind 0x20) : un point lancé du lanceur qui retombe sur le sol et
/// reste dans l'écran avant de disparaître après `+0x58` images.
mod target {
    use super::*;

    const OFFSETS: u32 = 0x4f0b0;
    const ANIMS: u32 = 0x1bc23e;

    pub fn run(x: &mut Cx) {
        match x.state(0x4f034) {
            0 => {
                x.add8(0x4, 2);
                x.set16(0x14, -0x180i16 as u16);
                x.set16(0x18, 0x300);
                x.set16(0x1a, -0x48i16 as u16);
                let owner = follow_owner(x);
                x.add16(0x10, 0xffff);
                let row = ((owner.u8(x.b, 0x3) >> 1) as u32) << 2;
                x.c.d[0] = row;
                let dx = x.rom16(OFFSETS + row);
                let dz = x.rom16(OFFSETS + row + 2);
                x.c.d[1] = dx as u32;
                x.c.d[2] = dz as u32;
                let mut dx = dx;
                if owner.u8(x.b, f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                    let vx = x.u16(0x14);
                    x.set16(0x14, vx.wrapping_neg());
                }
                x.add16(0x8, dx);
                x.add16(0xc, dz);
                let n = x.u8(0x3) as u32;
                x.anim(ANIMS, n);
                x.reinsert();
            }
            2 => {
                if x.u8(0x5) == 0 {
                    x.move_xz();
                    if (x.u16(0xc) as i16) <= 0 {
                        x.add8(0x5, 2);
                        x.set16(0x58, 0x78);
                        x.set32(0xc, 0);
                        let cam_x = if x.a5_u8(a5::USE_CAM2) != 0 { x.a5_u16(a5::CAM2_X) } else { x.a5_u16(a5::CAM_X) };
                        let low = cam_x.wrapping_add(0x20);
                        if (low as i16) > (x.u16(0x8) as i16) {
                            x.set16(0x8, low);
                        } else {
                            let high = cam_x.wrapping_add(0x160);
                            if high <= x.u16(0x8) {
                                x.set16(0x8, high);
                            }
                        }
                    }
                } else {
                    x.add16(0x58, 0xffff);
                    if x.u16(0x58) == 0 {
                        x.add8(0x4, 2);
                    }
                }
                x.anim_step();
            }
            _ => x.free_peff(),
        }
    }
}

handlers! {
    0x4e79c "peff_30" storm::run,
    0x4ec82 "peff_31" whirl::run,
    0x4f026 "peff_32" target::run,
}
