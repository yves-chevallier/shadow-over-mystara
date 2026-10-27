//! Boss 11/12 : les douze anneaux. Chaque image, `update` replace les anneaux derrière la tête d'après les tables de la ROM
//! (lignes de 32 octets de 0x1069f8 choisies par la pose `+0xaa` ; trajectoires de 0x106cb8), puis recopie la position de
//! l'anneau suivi (rang `+0xa8`) dans la sauvegarde `+0xa2..+0xa6`.
//!
//! Un anneau est un objet ESHL de type 5 : `+0x2f` rang, `+0x42` et `+0x43` orientation et inclinaison (0 à 31), `+0x12` mot
//! d'état. Les recherches de voisin (0x8f482, 0x8f4a2) parcourent le pool sans autre contrôle que le rang.

use super::*;

const FIRST_SEGMENT: u32 = 0xffbc5e;
const LAST_SEGMENT: u32 = 0xffca3e;

fn flip8(x: &mut Cx) -> u32 {
    if x.u8(f::FLIP) != 0 { 0x18 } else { 8 }
}

/// 0x8f4c2 : l'anneau de rang `+0xa8` (A1), à condition qu'il y ait exactement douze anneaux et que le rang soit unique.
fn find_head(x: &mut Cx) -> bool {
    let want = x.u8(SEG);
    x.c.d[1] = 0;
    x.c.d[2] = 0;
    x.c.d[6] = 0x25;
    let mut a1 = A5 + 0x3c5e;
    loop {
        x.c.a[1] = a1;
        if rd8(x.b, a1) != 0 && rd8(x.b, a1 + 2) == 5 {
            let n = (x.c.d[1] as u16).wrapping_add(1);
            setw(x, 1, n);
            if rd8(x.b, a1 + 0x2f) == want {
                if x.c.d[2] != 0 {
                    return false;
                }
                x.c.d[2] = a1;
            }
        }
        a1 = a1.wrapping_add(0x60);
        x.c.a[1] = a1;
        let k = (x.c.d[6] as u16).wrapping_sub(1);
        setw(x, 6, k); // relu : compteur de boucle relu au tour suivant
        if k == 0xffff {
            break;
        }
    }
    if x.c.d[1] as u8 != 0xc || x.c.d[2] == 0 {
        return false;
    }
    x.c.a[1] = x.c.d[2];
    true
}

/// 0x8f482 : l'anneau de rang D0b avant `a2` dans le pool ; A3, faux s'il n'y en a pas.
fn find_prev(x: &mut Cx) -> bool {
    let want = x.c.d[0] as u8;
    let mut a3 = x.c.a[2];
    loop {
        a3 = a3.wrapping_sub(0x60);
        if (a3 as i32) < FIRST_SEGMENT as i32 {
            x.c.a[3] = a3;
            return false;
        }
        if rd8(x.b, a3 + 0x2f) == want {
            x.c.a[3] = a3;
            return true;
        }
    }
}

/// 0x8f4a2 : idem après `a2`.
fn find_next(x: &mut Cx) -> bool {
    let want = x.c.d[0] as u8;
    let mut a3 = x.c.a[2];
    loop {
        a3 = a3.wrapping_add(0x60);
        if (a3 as i32) > LAST_SEGMENT as i32 {
            x.c.a[3] = a3;
            return false;
        }
        if rd8(x.b, a3 + 0x2f) == want {
            x.c.a[3] = a3;
            return true;
        }
    }
}

/// 0x8ebde.
pub(super) fn update(x: &mut Cx) {
    if !find_head(x) {
        return;
    }
    follow(x);
    if !find_head(x) {
        return;
    }
    let a1 = x.c.a[1];
    let (a, b, c) = (rd16(x.b, a1 + 8), rd16(x.b, a1 + 0xc), rd16(x.b, a1 + 0x10));
    x.set16(SAVE_X, a);
    x.set16(SAVE_Z, b);
    x.set16(SAVE_Y, c);
}

fn put8(x: &mut Cx, a: u32, v: u8) {
    wr8(x.b, a, v);
}

/// 0x8ec02 : place les anneaux d'après la pose `+0xaa`.
fn follow(x: &mut Cx) {
    let a1 = x.c.a[1];
    let row = 0x1069f8u32.wrapping_add(((x.u8(FRAME) as u16) << 5) as u32);
    x.c.d[0] = x.u8(FRAME) as u32;
    let sh = (x.u8(FRAME) as u16) << 5;
    setw(x, 0, sh);
    let px = x.u16(0x8);
    wr16(x.b, a1 + 8, px);
    let mut z = x.u16(0xc);
    if (z as i16) < 0 {
        z = 0;
        x.c.d[0] = 0;
    } else {
        setw(x, 0, z);
    }
    wr16(x.b, a1 + 0xc, z);
    let py = x.u16(0x10);
    wr16(x.b, a1 + 0x10, py);
    let idx = x.u8(SEG);
    x.c.d[0] = idx as u32;
    let thr = x.data8(row + 2);
    let r = Fl::sub8(idx, thr);
    if r.eq() {
        // égal
    } else if r.gt() {
        if !behind(x, idx) {
            return;
        }
    } else {
        ahead(x, idx);
    }
    // 0x8ed58
    x.c.a[2] = a1;
    if idx == 0 {
        return place_head(x, a1, row);
    }
    if idx == thr {
        let sub = x.data8(row + 4) as u16;
        let t = x.rom16(0x8ed7a + 2 * sub as u32);
        let target = 0x8ed7au32.wrapping_add(t as i16 as i32 as u32);
        // si un anneau manque, la sous-routine rend la main (état de mort posé) et le placement continue
        if target == 0x8ed7e {
            rows_a(x, row);
        } else {
            rows_b(x, row);
        }
    }
    positions(x, a1, row)
}

/// Branche « rang inférieur au seuil » (0x8ec3e) : l'anneau tourne vers 8 (ou 0x18) et le rang suivi avance quand il y est.
fn ahead(x: &mut Cx, _idx: u8) {
    let a1 = x.c.a[1];
    let goal = flip8(x) as u16;
    let cur = rd8(x.b, a1 + 0x42) as u16;
    let diff = cur.wrapping_sub(goal);
    if diff != 0 {
        let mut d = diff & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d.wrapping_neg(), 2);
        if v == 0 {
            v = 1;
        }
        let n = rd8(x.b, a1 + 0x42).wrapping_add(v as u8) & 0x1f;
        put8(x, a1 + 0x42, n);
    }
    let t = rd8(x.b, a1 + 0x43);
    if t != 0 {
        let mut d = t as u16 & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d, 2);
        if v == 0 {
            v = 1;
        }
        let n = t.wrapping_sub(v as u8) & 0x1f;
        put8(x, a1 + 0x43, n);
    }
    let cur = rd8(x.b, a1 + 0x42);
    let goal = flip8(x);
    if cur == goal as u8 && rd8(x.b, a1 + 0x43) == 0 {
        x.add8(SEG, 1);
    }
}

/// Branche « rang supérieur au seuil » (0x8ecc6) : l'anneau précédent tourne vers 8 (ou 0x18). Faux si l'anneau manque.
fn behind(x: &mut Cx, idx: u8) -> bool {
    let a1 = x.c.a[1];
    x.c.d[1] = idx as u32;
    x.c.a[2] = a1;
    let prev = (x.c.d[0] as u16).wrapping_sub(1);
    setw(x, 0, prev);
    if !find_prev(x) {
        lost(x);
        return false;
    }
    let a3 = x.c.a[3];
    setw(x, 0, idx as u16);
    let goal = flip8(x) as u16;
    x.c.d[1] = rd8(x.b, a3 + 0x42) as u32;
    let diff = (rd8(x.b, a3 + 0x42) as u16).wrapping_sub(goal);
    setw(x, 1, diff);
    if diff != 0 {
        let mut d = diff & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d, 2);
        if v == 0 {
            v = 1;
        }
        setw(x, 1, v);
    }
    let d1 = x.c.d[1] as u8;
    let n = rd8(x.b, a3 + 0x42).wrapping_sub(d1) as u16 & 0x1f;
    put8(x, a3 + 0x42, n as u8);
    let t = rd8(x.b, a3 + 0x43);
    let mut d2 = t as u16;
    if t != 0 {
        d2 = d2.wrapping_sub(0x10);
        d2 = asr_w(d2, 2);
        if d2 == 0 {
            d2 = 1;
        }
    }
    let n3 = (rd8(x.b, a3 + 0x43).wrapping_add(d2 as u8) as u16) & 0x1f;
    put8(x, a3 + 0x43, n3 as u8);
    let cur = rd8(x.b, a3 + 0x42);
    let goal = flip8(x);
    if cur == goal as u8 && rd8(x.b, a3 + 0x43) == 0 {
        let v = x.u8(SEG).wrapping_sub(1);
        x.set8(SEG, v);
    }
    true
}

/// 0x8ed7e : oriente les anneaux précédents d'après une liste d'incréments (A4 = ligne + 8).
fn rows_a(x: &mut Cx, row: u32) -> bool {
    let a1 = x.c.a[1];
    let (mut d1, mut d2) = base_angles(x, row);
    let mut a4 = row + 8;
    x.c.a[2] = a1;
    let flip = x.u8(f::FLIP) != 0;
    let mut d0 = rd8(x.b, a1 + 0x2f).wrapping_sub(1) as u16;
    loop {
        x.c.d[0] = d0 as u32;
        if !find_prev(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        let mut inc = x.data8(a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d1 = (d1.wrapping_add(inc as u16)) & 0x1f;
        put8(x, a3 + 0x42, d1 as u8);
        let mut inc = x.data8(a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d2 = (d2.wrapping_add(inc as u16)) & 0x1f;
        put8(x, a3 + 0x43, d2 as u8);
        x.c.a[2] = a3; // relu : A2 relu par la suite (DDSOM_POISON : cov_tour4_c3, cov_tour8_c1)
        d0 = d0.wrapping_sub(1);
        if d0 == 0xffff {
            break;
        }
    }
    true
}

/// Angles de départ de la ligne (octets +6 et +7), retournés si le serpent regarde à gauche.
fn base_angles(x: &mut Cx, row: u32) -> (u16, u16) {
    let mut d1 = x.data8(row + 6) as u16;
    let mut d2 = x.data8(row + 7) as u16;
    if x.u8(f::FLIP) != 0 {
        d1 = 0x20u16.wrapping_sub(d1);
        d2 = 0x20u16.wrapping_sub(d2);
    }
    (d1 & 0x1f, d2 & 0x1f)
}

/// Écart signé ramené à -16..16 puis réduit (quart, négatif de l'écart) : le pas appliqué aux anneaux (0x8ee48...).
fn smooth(diff: u16) -> u16 {
    let mut d4 = diff;
    if (diff as i16) < 0 {
        d4 = d4.wrapping_neg();
    }
    if Fl::sub16(d4, 0x10).gt() {
        d4 = d4.wrapping_sub(0x10);
    }
    d4 = d4.wrapping_neg();
    if (diff as i16) >= 0 {
        // tst.w D3w ; bpl : D4 reste négatif
    } else {
        d4 = d4.wrapping_neg();
    }
    let mut s = asr_w(d4, 2);
    if s == 0 {
        s = 1;
    }
    s
}

/// 0x8edf2 : comme `rows_a`, mais chaque anneau rejoint l'orientation cible par un quart de l'écart.
fn rows_b(x: &mut Cx, row: u32) -> bool {
    let a1 = x.c.a[1];
    let (mut d1, mut d2) = base_angles(x, row);
    let mut a4 = row + 8;
    x.c.a[2] = a1;
    let flip = x.u8(f::FLIP) != 0;
    let mut d0 = rd8(x.b, a1 + 0x2f).wrapping_sub(1) as u16;
    loop {
        x.c.d[0] = d0 as u32;
        if !find_prev(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        let mut inc = x.data8(a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d1 = (d1.wrapping_add(inc as u16)) & 0x1f;
        let cur = rd8(x.b, a3 + 0x42) as u16;
        let diff = cur.wrapping_sub(d1);
        if diff != 0 {
            let s = smooth(diff);
            put8(x, a3 + 0x42, (s.wrapping_add(cur) & 0x1f) as u8);
        }
        let mut inc = x.data8(a4);
        a4 += 1;
        if flip {
            inc = 0x20u8.wrapping_sub(inc);
        }
        d2 = (d2.wrapping_add(inc as u16)) & 0x1f;
        let cur = rd8(x.b, a3 + 0x43) as u16;
        let diff = cur.wrapping_sub(d2);
        let step = if diff != 0 { smooth(diff) } else { cur_zero(cur) };
        put8(x, a3 + 0x43, (step.wrapping_add(cur) & 0x1f) as u8);
        x.c.a[2] = a3; // relu : A2 relu par la suite (DDSOM_POISON : cov_tour4_c3, cov_tour8_c1)
        d0 = d0.wrapping_sub(1);
        if d0 == 0xffff {
            break;
        }
    }
    true
}

/// Quand l'écart est nul, l'original ajoute D3b = 0 : l'orientation ne change pas.
fn cur_zero(_cur: u16) -> u16 {
    0
}

/// 0x8eeca et 0x8ef60 : positions des anneaux précédents d'après les trajectoires, puis de la tête.
fn positions(x: &mut Cx, a1: u32, row: u32) {
    x.c.a[2] = a1;
    let keep = row;
    let n = rd8(x.b, a1 + 0x2f) as i32 - 1;
    if n >= 0 {
        let mut d0 = n as u16;
        loop {
            x.c.d[0] = d0 as u32;
            if !find_prev(x) {
                return lost(x);
            }
            let a2 = x.c.a[2];
            let a3 = x.c.a[3];
            { let t = rd16(x.b, a2 + 8); wr16(x.b, a3 + 8, t); }
            { let t = rd16(x.b, a2 + 0xc); wr16(x.b, a3 + 0xc, t); }
            { let t = rd16(x.b, a2 + 0x10); wr16(x.b, a3 + 0x10, t); }
            let a4 = 0x106cb8u32.wrapping_add((d0 << 3) as u32);
            let mut a6 = rd32(x.b, a4);
            let ori = rd8(x.b, a3 + 0x42) as u16;
            a6 = a6.wrapping_add((ori.wrapping_add(0x10) & 0x1f) as u32 * 0x100);
            let tilt = (rd8(x.b, a3 + 0x43) as u32) << 3;
            let ax = asr_w(rd16(x.b, a6 + tilt), 8);
            { let t = rd16(x.b, a3 + 8).wrapping_add(ax); wr16(x.b, a3 + 8, t); }
            let az = asr_w(rd16(x.b, a6 + 2 + tilt), 8);
            { let t = rd16(x.b, a3 + 0xc).wrapping_add(az); wr16(x.b, a3 + 0xc, t); }
            let k = (rd16(x.b, a6 + 4 + tilt) & 0x1f) * 2;
            let a6b = rd32(x.b, a4 + 4);
            let ay = asr_w(rd16(x.b, a6b + k as u32), 8);
            { let t = rd16(x.b, a3 + 0x10).wrapping_add(ay); wr16(x.b, a3 + 0x10, t); }
            let s = rd8(x.b, a3 + 3) as u16 + 1;
            wr16(x.b, a3 + 0x12, s);
            x.c.a[2] = a3; // relu : A2 relu par la suite (DDSOM_POISON : cov_tour4_c3, cov_tour8_c1)
            d0 = d0.wrapping_sub(1);
            if d0 == 0xffff {
                break;
            }
        }
    }
    place_head(x, a1, keep)
}

/// 0x8ef60 : la tête se place contre l'anneau de rang 0 (`a2`) puis la pose de la ligne règle les anneaux suivants.
fn place_head(x: &mut Cx, a1: u32, row: u32) {
    let a2 = x.c.a[2];
    let mut d0 = 4u16;
    if x.u8(f::FLIP) == 0 {
        d0 = d0.wrapping_neg();
    }
    let px = d0.wrapping_add(rd16(x.b, a2 + 8));
    setw(x, 0, px);
    x.set16(0x8, px);
    let pz = 3u16.wrapping_add(rd16(x.b, a2 + 0xc));
    setw(x, 0, pz);
    x.set16(0xc, pz);
    let py = rd16(x.b, a2 + 0x10);
    x.set16(0x10, py);
    x.set16(0x10, py.wrapping_sub(1));
    x.set16(0xc, pz.wrapping_add(1));
    let pose = x.data8(row + 5) as u16;
    x.c.d[1] = pose as u32;
    let t = x.rom16(0x8efa2 + 2 * pose as u32);
    setw(x, 1, t);
    let target = 0x8efa2u32.wrapping_add(t as i16 as i32 as u32);
    // un anneau manquant interrompt la pose (état de mort posé) mais pas la suite
    match target {
        0x8efb0 => pose_0(x, a1),
        0x8eff6 => pose_1(x, a1, row),
        0x8f0da => pose_2(x, a1, row),
        0x8f1a4 => pose_3(x, a1, row),
        0x8f21a => pose_4(x, a1, row),
        0x8f294 => pose_5(x, a1, row),
        _ => pose_6(x, a1, row),
    };
    trail(x, a1);
}

/// Rang de l'anneau `a1` à la ligne `row` : sous le seuil (`+2`), la pose ne s'applique pas.
fn guarded(x: &mut Cx, a1: u32, row: u32) -> Option<u16> {
    let d0 = rd8(x.b, a1 + 0x2f);
    if Fl::sub8(d0, x.data8(row + 2)).lt() {
        return None;
    }
    Some(d0 as u16)
}

/// Fait suivre l'orientation `(+0x42, +0x43)` de proche en proche : l'anneau suivant reprend celle du précédent.
fn ripple(x: &mut Cx, mut d0: u16, start: u32) -> bool {
    let mut d2 = rd16(x.b, start + 0x42);
    let mut a2 = start;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 = d0.wrapping_add(1);
        setw(x, 0, d0);
        x.c.a[2] = a2;
        if !find_next(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        let d1 = rd16(x.b, a3 + 0x42);
        wr16(x.b, a3 + 0x42, d2);
        d2 = d1;
        a2 = a3;
    }
}

/// Pose 0 (0x8efb0) : tout le serpent regarde dans la direction de la tête.
fn pose_0(x: &mut Cx, a1: u32) -> bool {
    let v = flip8(x) as u8;
    put8(x, a1 + 0x42, v);
    put8(x, a1 + 0x43, 0);
    let mut a2 = a1;
    let mut d0 = rd8(x.b, a1 + 0x2f) as u16;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        x.c.d[0] = d0 as u32;
        x.c.a[2] = a2;
        if !find_next(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        { let t = rd8(x.b, a2 + 0x42); put8(x, a3 + 0x42, t); }
        { let t = rd8(x.b, a2 + 0x43); put8(x, a3 + 0x43, t); }
        a2 = a3;
    }
}

/// Pose 1 (0x8eff6) : ondulation : l'orientation rejoint 8/0x18, l'inclinaison oscille entre 4 et 0x1c.
fn pose_1(x: &mut Cx, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if d0 > 0xb {
        return true;
    }
    let goal = flip8(x) as u16;
    let cur = rd8(x.b, a1 + 0x42) as u16;
    let diff = goal.wrapping_sub(cur);
    if diff != 0 {
        let mut d = diff & 0x1f;
        if !Fl::sub16(d, 0x10).lt() {
            d = d.wrapping_sub(0x20);
        }
        let mut v = asr_w(d.wrapping_neg(), 2);
        if v == 0 {
            v = 1;
        }
        put8(x, a1 + 0x42, (v.wrapping_add(cur) & 0x1f) as u8);
        x.set8(0xac, 1);
    }
    let mut d1 = rd8(x.b, a1 + 0x43);
    let mut d2: u8 = 1;
    if !(Fl::sub8(d1, 4).le() || Fl::sub8(d1, 0x1c).gt()) {
        if !Fl::sub8(d1, 0x10).lt() {
            d2 = d2.wrapping_neg();
        }
        d1 = d1.wrapping_sub(d2) & 0x1f;
        put8(x, a1 + 0x43, d1);
        return true;
    }
    if x.a5_u16(0x6e) & 1 == 0 {
        return true;
    }
    let mut d2 = x.u8(0xac);
    let mut d3 = d2;
    if (d3 as i8) < 0 {
        d3 = d3.wrapping_neg();
    }
    if d3 != 1 {
        d2 = 1;
        x.set8(0xac, d2);
    }
    d1 = d1.wrapping_add(d2) & 0x1f;
    if !Fl::sub8(d1, 4).le() && !Fl::sub8(d1, 0x1c).gt() {
        d2 = d2.wrapping_neg();
        x.set8(0xac, d2);
    } else {
        put8(x, a1 + 0x43, d1);
    }
    ripple_words(x, d0, a1)
}

/// Fait suivre le mot `(+0x42)` de proche en proche à partir de l'anneau `start` (rang `d0`).
fn ripple_words(x: &mut Cx, d0: u16, start: u32) -> bool {
    ripple(x, d0, start)
}

/// Pose 2 (0x8f0da) : la tête regarde vers son point précédent ; les orientations se décalent d'un anneau à l'autre.
fn pose_2(x: &mut Cx, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let (d1, d2) = (x.u16(0x8), x.u16(0xc));
    let (d3, d4) = (x.u16(SAVE_X), x.u16(SAVE_Z));
    setw(x, 1, d1);
    setw(x, 2, d2);
    setw(x, 3, d3);
    setw(x, 4, d4);
    if d3 != d1 || d4 != d2 {
        x.c.a[1] = a1;
        x.call(0x1386);
        let angle = (x.c.d[0] as u8).wrapping_sub(rd8(x.b, a1 + 0x42));
        setb(x, 0, angle);
        if angle != 0 {
            let w = (x.c.d[0] as u16).wrapping_add(8) & 0x1f;
            setw(x, 0, w);
            let mut s: u8 = 1;
            if !Fl::sub16(w, 0x10).lt() {
                s = 0xff;
            }
            let n = rd8(x.b, a1 + 0x42).wrapping_add(s) & 0x1f;
            put8(x, a1 + 0x42, n);
        }
        put8(x, a1 + 0x42, 0);
    }
    let mut d0 = rd8(x.b, a1 + 0x2f) as u16;
    x.c.d[0] = d0 as u32;
    let mut a2 = a1;
    let mut d2 = rd8(x.b, a2 + 0x42) as u16;
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        x.c.d[0] = d0 as u32;
        x.c.a[2] = a2;
        if !find_next(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        let d1 = rd8(x.b, a3 + 0x42);
        put8(x, a3 + 0x42, d2 as u8);
        d2 = d1 as u16;
        a2 = a3;
    }
}

/// Pose 3 (0x8f1a4) : tous les anneaux rejoignent l'orientation de la tête (inversée pendant la réaction).
fn pose_3(x: &mut Cx, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let mut d5 = x.u8(f::FLIP);
    if x.u8(f::STATE) == 4 {
        d5 ^= 0x20;
    }
    let mut d0 = d0;
    let mut a2 = a1;
    loop {
        let goal = if d5 != 0 { 0x18u16 } else { 8 };
        let cur = rd8(x.b, a2 + 0x42) as u16;
        let diff = cur.wrapping_sub(goal);
        if diff != 0 {
            let mut d = diff & 0x1f;
            if !Fl::sub16(d, 0x10).lt() {
                d = d.wrapping_sub(0x20);
            }
            let mut v = asr_w(d.wrapping_neg(), 2);
            if v == 0 {
                v = 1;
            }
            put8(x, a2 + 0x42, (cur.wrapping_add(v) & 0x1f) as u8);
        }
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 = (d0 & 0xff00) | (d0 as u8).wrapping_add(1) as u16;
        x.c.d[0] = d0 as u32;
        x.c.a[2] = a2;
        if !find_next(x) {
            lost(x);
            return false;
        }
        a2 = x.c.a[3];
    }
}

/// Pose 4 (0x8f21a) : l'inclinaison de la tête suit la direction `+0x2d`, les orientations se décalent.
fn pose_4(x: &mut Cx, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let mut w = (x.u8(0x2d) as u16).wrapping_add(8) & 0x1f;
    setw(x, 0, w);
    let diff = (w as u8).wrapping_sub(rd8(x.b, a1 + 0x43));
    setb(x, 0, diff);
    if diff != 0 {
        w = x.c.d[0] as u16 & 0x1f;
        let mut s: u8 = 1;
        if !Fl::sub16(w, 0x10).lt() {
            s = 0xff;
        }
        let n = rd8(x.b, a1 + 0x43).wrapping_add(s) & 0x1f;
        put8(x, a1 + 0x43, n);
    }
    put8(x, a1 + 0x42, 8);
    let d0 = rd8(x.b, a1 + 0x2f) as u16;
    shift_words(x, d0, a1)
}

/// Décale les mots `(+0x42)` d'un anneau au suivant à partir de `start` (rang `d0`).
fn shift_words(x: &mut Cx, mut d0: u16, start: u32) -> bool {
    let mut a2 = start;
    let mut d2 = rd16(x.b, a2 + 0x42);
    loop {
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        x.c.d[0] = d0 as u32;
        x.c.a[2] = a2;
        if !find_next(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        let d1 = rd16(x.b, a3 + 0x42);
        wr16(x.b, a3 + 0x42, d2);
        d2 = d1;
        a2 = a3;
    }
}

/// Pose 5 (0x8f294) : la tête se tourne d'un cran à chaque image, sans aller plus loin.
fn pose_5(x: &mut Cx, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let goal = flip8(x) as u8;
    if rd8(x.b, a1 + 0x42) != goal {
        put8(x, a1 + 0x42, goal);
        x.set8(0xac, 1);
    }
    let step: u8 = if x.u8(f::FLIP) != 0 { 1 } else { 0xff };
    let n = rd8(x.b, a1 + 0x43).wrapping_add(step) & 0x1f;
    put8(x, a1 + 0x43, n);
    shift_words(x, d0, a1)
}

/// Pose 6 (0x8f30a).
fn pose_6(x: &mut Cx, a1: u32, row: u32) -> bool {
    let Some(d0) = guarded(x, a1, row) else { return true };
    if !Fl::sub16(d0, 0xb).lt() {
        return true;
    }
    let step: u8 = if x.u8(f::FLIP) != 0 { 1 } else { 0xff };
    let n = rd8(x.b, a1 + 0x42).wrapping_add(step) & 0x1f;
    put8(x, a1 + 0x42, n);
    put8(x, a1 + 0x43, 0);
    shift_words(x, d0, a1)
}

/// 0x8f36a : les anneaux suivants (rangs croissants) se placent derrière le précédent d'après les trajectoires.
fn trail(x: &mut Cx, a1: u32) -> bool {
    let mut a2 = a1;
    let v = x.u8(0xad) & !1;
    x.set8(0xad, v);
    let mut d0 = rd8(x.b, a1 + 0x2f) as u16;
    loop {
        x.c.a[2] = a2;
        if !Fl::sub16(d0, 0xb).lt() {
            return true;
        }
        d0 += 1;
        x.c.d[0] = d0 as u32;
        if !find_next(x) {
            lost(x);
            return false;
        }
        let a3 = x.c.a[3];
        let z = rd16(x.b, a2 + 0xc);
        wr16(x.b, a3 + 0xc, z);
        if (z as i16) < 0 {
            let v = x.u8(0xad) | 1;
            x.set8(0xad, v);
        }
        { let t = rd16(x.b, a2 + 8); wr16(x.b, a3 + 8, t); }
        { let t = rd16(x.b, a2 + 0x10); wr16(x.b, a3 + 0x10, t); }
        let a4 = 0x106cb8u32.wrapping_add(((d0 - 1) << 3) as u32);
        let (mut a6, mut d1);
        let mut d2;
        loop {
            a6 = rd32(x.b, a4).wrapping_add((rd8(x.b, a2 + 0x42) as u32) << 8);
            d1 = (rd8(x.b, a2 + 0x43) as u32) << 3;
            d2 = asr_w(rd16(x.b, a6 + 2 + d1), 8).wrapping_add(rd16(x.b, a3 + 0xc));
            if (d2 as i16) >= 0 || x.u8(0xad) & 1 != 0 {
                break;
            }
            let mut d5 = x.u8(f::FLIP);
            if x.u8(f::STATE) == 4 {
                d5 ^= 0x20;
            }
            let d3 = (rd8(x.b, a2 + 0x42) as u16) & 0x1f;
            let goal = if d5 != 0 { 0x18u16 } else { 8 };
            let diff = d3.wrapping_sub(goal);
            if diff != 0 {
                let mut s: u8 = 1;
                if !Fl::sub16(diff & 0x1f, 0x10).lt() {
                    s = 0xff;
                }
                let n = rd8(x.b, a2 + 0x42).wrapping_sub(s) & 0x1f;
                put8(x, a2 + 0x42, n);
                continue;
            }
            let d3 = (rd8(x.b, a2 + 0x43) as u16) & 0x1f;
            if d3 == 0 {
                break;
            }
            let mut s: u8 = 1;
            if !Fl::sub8(d3 as u8, 0x10).lt() {
                s = 0xff;
            }
            let n = rd8(x.b, a2 + 0x43).wrapping_sub(s) & 0x1f;
            put8(x, a2 + 0x43, n);
        }
        wr16(x.b, a3 + 0xc, d2);
        let ax = asr_w(rd16(x.b, a6 + d1), 8);
        { let t = rd16(x.b, a3 + 8).wrapping_add(ax); wr16(x.b, a3 + 8, t); }
        let k = (rd16(x.b, a6 + 4 + d1) & 0x1f) * 2;
        let a6b = rd32(x.b, a4 + 4);
        let ay = asr_w(rd16(x.b, a6b + k as u32), 8);
        { let t = rd16(x.b, a3 + 0x10).wrapping_add(ay); wr16(x.b, a3 + 0x10, t); }
        let s = rd8(x.b, a3 + 3) as u16 + 1;
        wr16(x.b, a3 + 0x12, s);
        a2 = a3;
    }
}
