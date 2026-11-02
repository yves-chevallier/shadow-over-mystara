use super::*;

// ---------------------------------------------------------------------------------------------
// Scenery effects (type 42) linked by `+0x46`

/// Scenery effect (type 42) of variant `variant` and setting `setting`, linked by `+0x46`.
pub(crate) fn scenery_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8, setting: Option<u8>) -> Option<Obj> {
    let o = spawn(r, b, kind::SCENERY)?;
    o.set_u8(b, Obj::VARIANT, variant);
    if let Some(s) = setting {
        o.set_u8(b, SETTING, s);
    }
    link(r, b, o, link::SCENERY, creator.0);
    Some(o)
}

/// `dbf Dn` loop from `last` down to 0: `body(r, b, n)` at each turn; the counter (low word of `Dn`) goes to the
/// processor after each turn. Stops at the first `None` (pool full); returns the last object created.
pub(super) fn repeats<R: Regs>(
    r: &mut R,
    b: &mut Cps2Bus,
    dn: usize,
    last: u16,
    mut body: impl FnMut(&mut R, &mut Cps2Bus, u16) -> Option<Obj>,
) -> Option<Obj> {
    let mut n = last;
    loop {
        let o = body(r, b, n)?;
        n = n.wrapping_sub(1);
        r.d_word(dn, n);
        if n == 0xffff {
            return Some(o);
        }
    }
}

/// Several scenery effects: D1 counts from `last` down to 0, setting = D1 or zero.
pub(crate) fn scenery_burst_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8, last: u16, setting_counter: bool) -> Option<Obj> {
    r.d(1, last as u32);
    repeats(r, b, 1, last, |r, b, n| scenery_via(r, b, creator, variant, Some(if setting_counter { n as u8 } else { 0 })))
}

/// 0xb3df6: two brief bursts (variant 0, settings 0 and 1) placed on the creator (x and depth).
pub(crate) fn scenery_0_pair_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let mut last = None;
    for setting in 0..2u8 {
        let o = spawn(r, b, kind::SCENERY)?;
        o.set_u8(b, Obj::VARIANT, 0);
        o.set_u8(b, SETTING, setting);
        let (x, y) = (creator.u16(b, Obj::X), creator.u16(b, Obj::Y));
        o.set_u16(b, Obj::X, x);
        o.set_u16(b, Obj::Y, y);
        link(r, b, o, link::SCENERY, creator.0);
        last = Some(o);
    }
    last
}

/// 0xb3f6a: one frame out of four (`A5+0x6e`), puff (variant 4) of randomly drawn setting; `+0x58` = bit 2 of the draw.
pub(crate) fn scenery_4_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let frame = var::IMAGES_WORD_LOW.get(b);
    r.d_word(0, frame);
    r.d_word(1, frame);
    if and_d0(r, frame, 3) != 0 {
        return None;
    }
    let o = spawn(r, b, kind::SCENERY)?;
    o.set_u8(b, Obj::VARIANT, 4);
    let rng = crate::rng::random(b);
    let v = and_d0(r, rng, 7);
    o.set_u8(b, SETTING, v as u8);
    let high = (v & 4) >> 2;
    r.d_word(0, high);
    o.set_u8(b, Effect::BLINK_PHASE, high as u8);
    link(r, b, o, link::SCENERY, creator.0);
    Some(o)
}

/// 0xb3fd4 (variant 6, one time out of sixteen) and 0xb4088 (variant 10, zero setting, one time out of four).
pub(crate) fn scenery_by_chance_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, mask: u16, variant: u8, setting: Option<u8>) -> Option<Obj> {
    let rng = crate::rng::random(b);
    if and_d0(r, rng, mask) != 0 {
        return None;
    }
    scenery_via(r, b, creator, variant, setting)
}

// ---------------------------------------------------------------------------------------------
// Spells of a wizard enemy (type 44) linked by `+0x5c`

/// 0xb4f44: a spell effect, setting `setting` (variant left as is).
pub(crate) fn spell_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::SPELL)?;
    o.set_u8(b, SETTING, setting);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb4ef4 / 0xb4f60 / 0xb4f1c: six spell effects (variants 5 down to 0) of setting `setting`.
pub(crate) fn spell_ring_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    r.d(1, 5);
    repeats(r, b, 1, 5, |r, b, n| {
        let o = spawn(r, b, kind::SPELL)?;
        o.set_u8(b, Obj::VARIANT, n as u8);
        o.set_u8(b, SETTING, setting);
        link(r, b, o, link::CREATOR, creator.0);
        Some(o)
    })
}

/// 0xb3e4e, 0xb3ec6: cries of a boss (scene effect 42, variant `variant`): a randomly drawn sheet (`rng & 0x38`, bytes
/// in `table`) gives the number of effects minus one, then the setting `+0x2f` of each; linked to the creator by
/// `+0x46`. A2 follows the reading of the sheet.
pub(crate) fn shouts_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8, table: &Table) -> Option<Obj> {
    let rng = crate::rng::random(b);
    let mut at = and_d0(r, rng, 0x38) as u32;
    let n = table.at8(b, at);
    r.d(1, n as u32);
    at += 1;
    repeats(r, b, 1, n as u16, |r, b, _| {
        r.a(2, table.rom + at);
        let o = spawn(r, b, kind::SCENERY)?;
        o.set_u8(b, Obj::VARIANT, variant);
        let setting = table.at8(b, at);
        at += 1;
        r.a(2, table.rom + at);
        o.set_u8(b, SETTING, setting);
        link(r, b, o, link::SCENERY, creator.0);
        Some(o)
    })
}

/// 0xb4bcc: rain of debris (type 43) of variant `variant`, setting `setting`, linked by `+0x5c`.
pub(crate) fn debris_rain_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8, setting: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::DEBRIS_RAIN)?;
    o.set_u8(b, Obj::VARIANT, variant);
    o.set_u8(b, SETTING, setting);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xb53f4: three effects 46 (settings 2, 1, 0) linked by `+0x46`.
pub(crate) fn type_46_trio(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let mut last = None;
    for setting in [2u8, 1, 0] {
        let o = spawn(r, b, kind::TYPE_46)?;
        o.set_u8(b, Obj::VARIANT, 0);
        o.set_u8(b, SETTING, setting);
        link(r, b, o, link::SCENERY, creator.0);
        last = Some(o);
    }
    last
}

/// 0xb59a6, 0xb59c8: effect 48 of variant `variant`, linked by `+0x46`.
pub(crate) fn type_48(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::TYPE_48)?;
    o.set_u8(b, Obj::VARIANT, variant);
    o.set_u8(b, SETTING, 0);
    link(r, b, o, link::SCENERY, creator.0);
    Some(o)
}

/// 0xb96c0: thrown burst (type 81) of variant `variant`, linked to the player by `+0x50`.
pub(crate) fn thrown_shard(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, variant: u8) -> Option<Obj> {
    let o = spawn(r, b, kind::THROWN_SHARD)?;
    put8(r, b, o, Obj::VARIANT, variant);
    link(r, b, o, link::OWNER, creator.0);
    Some(o)
}

/// 0xb230e: block of 2 × 2 tiles written at `at` (tile `tile`, attribute `attr`; next row 0x80 further); returns the
/// address after the two rows (A1) and the tile + 0x11 (D0).
pub(crate) fn tile_block(b: &mut Cps2Bus, mut at: u32, mut tile: u16, attr: u16) -> (u32, u16) {
    for _ in 0..2 {
        wr16(b, at, tile);
        wr16(b, at + 2, attr);
        wr16(b, at + 4, tile.wrapping_add(0x10));
        wr16(b, at + 6, attr);
        at = at.wrapping_add(0x80);
        tile = tile.wrapping_add(1);
    }
    (at, tile.wrapping_add(0xf))
}

// ---------------------------------------------------------------------------------------------
// Scene creators

/// Fish (type 30) placed on the alternate camera.
pub(super) fn fish_at_camera(b: &mut Cps2Bus, o: Obj) {
    let (x, y) = (crate::ram::CAMERA_ALT_X.get(b), crate::ram::CAMERA_ALT_Y.get(b));
    o.set_u16(b, Obj::X, x);
    o.set_u16(b, Obj::Y, y);
}

/// 0xb1764: three fish (variant 0) that swim across, 0x20 pixels left of the alternate camera, at a height of 0x40;
/// depth and speed (`+0x58`) read at 0xb17ae (D4: rank in the table, D5: counter).
pub(crate) fn fish_school(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    r.d(4, 0);
    r.d(5, 2);
    let mut k: u16 = 0;
    repeats(r, b, 5, 2, |r, b, _| {
        let o = spawn(r, b, kind::FISH)?;
        o.set_u8(b, TURNED, 0x20);
        fish_at_camera(b, o);
        o.set_u16(b, Obj::Z, 0xffc0);
        let x = o.u16(b, Obj::X).wrapping_sub(0x20);
        o.set_u16(b, Obj::X, x);
        let dy = tables().fish_school.at16(b, k as u32);
        r.d_word(0, dy);
        let y = o.u16(b, Obj::Y).wrapping_add(dy);
        o.set_u16(b, Obj::Y, y);
        let speed = tables().fish_school.at16(b, k as u32 + 2);
        put16(r, b, o, 0x58, speed);
        k += 4;
        r.d_word(4, k);
        Some(o)
    })
}

/// 0xb17ba: four fish (variant 1) around the alternate camera (offsets at 0xb1808), every other one flipped.
pub(crate) fn fish_swimmers(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    r.d(4, 0);
    r.d(5, 3);
    let mut k: u16 = 0;
    repeats(r, b, 5, 3, |r, b, n| {
        let o = spawn(r, b, kind::FISH)?;
        o.set_u8(b, Obj::VARIANT, 1);
        fish_at_camera(b, o);
        let swimmers = &tables().fish_swimmers;
        let (dx, dy) = (swimmers.at16(b, k as u32), swimmers.at16(b, k as u32 + 2));
        r.d_word(0, dx);
        r.d_word(1, dy);
        let x = o.u16(b, Obj::X).wrapping_add(dx);
        o.set_u16(b, Obj::X, x);
        let y = r.addition(dy as u32, o.u16(b, Obj::Y) as u32, Size::Word);
        o.set_u16(b, Obj::Y, y as u16);
        let odd = n as u8 & 1;
        r.d_byte(2, odd);
        r.flags(odd as u32, Size::Byte);
        if odd != 0 {
            let f = o.u8(b, TURNED) ^ 0x20;
            put8(r, b, o, TURNED, f);
        }
        k += 4;
        r.d_word(4, k);
        Some(o)
    })
}

/// 0xb7dec: four columns (type 67, variants 3 down to 0) under each present and living player (A2: the player, D6:
/// player counter, D5: column counter).
pub fn pillars(b: &mut Cps2Bus) -> Option<Obj> {
    pillars_via(&mut Pure, b)
}
pub(crate) fn pillars_via(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    r.d(6, 3);
    let mut last = None;
    for k in 0..4u16 {
        let p = crate::ram::PLAYERS.nth(k as u32);
        r.a(2, p.0);
        if p.u8(b, Obj::ACTIVE) != 0 && p.u16(b, PlayerView::LIFE) != 0 {
            r.d(5, 3);
            last = Some(repeats(r, b, 5, 3, |r, b, n| {
                let o = spawn(r, b, kind::PILLAR)?;
                o.set_u8(b, Obj::VARIANT, n as u8);
                link(r, b, o, link::OWNER, p.0);
                Some(o)
            })?);
        } else {
            r.flags(0, Size::Byte);
        }
        r.a(2, p.0 + crate::ram::PLAYERS.step);
        r.d_word(6, 2u16.wrapping_sub(k));
    }
    last
}

/// 0xabb70: trails (type 9) of the scene entrance. The two places of each present player are marked in the table
/// `A5-0x5f9a` (6 words), then one trail is created per remaining free place, `5 - A5+0x35` times.
pub(crate) fn trails(r: &mut impl Regs, b: &mut Cps2Bus) -> Option<Obj> {
    const SEATS: u32 = A5 - 0x5f9a;
    for k in 0..3 {
        wr32(b, SEATS + 4 * k, 0);
    }
    r.a(2, SEATS + 8);
    r.d(6, 3);
    for k in 0..4u16 {
        let p = crate::ram::PLAYERS.nth(k as u32).0;
        if rd8(b, p) & 1 != 0 {
            let seat = (rd8(b, p + 3) & 0xe) as u32;
            r.d(0, seat);
            r.a(2, SEATS);
            wr16(b, SEATS + seat, 0x101);
        }
        r.a(1, p + crate::ram::PLAYERS.step);
        r.d_word(6, 2u16.wrapping_sub(k));
    }
    r.d(0, 0);
    let living = var::PLAYERS_ALIVE.get(b);
    let mut place: u8 = 0;
    let mut rank = living;
    let mut seat = SEATS;
    r.d(5, 5u8.wrapping_sub(living) as u32);
    r.d_byte(4, living);
    r.a(4, SEATS);
    let n = 5u8.wrapping_sub(living) as u16;
    repeats(r, b, 5, n, |r, b, _| {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let o = r.alloc_without_test(b, &pools::EFFCT);
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, kind::TRAIL);
        o.set_u8(b, TURNED, 0x20);
        o.set_u8(b, Effect::OWN_PALETTE, 2);
        o.set_u8(b, Effect::RANK, rank);
        rank = rank.wrapping_add(1);
        r.d_byte(4, rank);
        while rd8(b, seat) != 0 {
            place = place.wrapping_add(2);
            seat += 2;
        }
        r.d_byte(0, place);
        r.a(4, seat);
        o.set_u8(b, Obj::VARIANT, place);
        wr16(b, seat, 0x101);
        r.flags(0x101, Size::Word);
        Some(o)
    })
}

/// 0xa8af8 (and 0xa8ae8, which takes the list in D0): creates the scenery list (SET) number `list` (pointer table
/// 0xa8b26): entries of 4 bytes (type, setting `+0x2f`, word whose low byte is the variant), terminated by a negative
/// type; stops at the first full pool. A4 follows the reading; D0.w = the word read.
pub fn set_list(b: &mut Cps2Bus, list: u8) -> Option<Obj> {
    set_list_via(&mut Pure, b, list)
}
pub(crate) fn set_list_via(r: &mut impl Regs, b: &mut Cps2Bus, list: u8) -> Option<Obj> {
    let k = (list as u16) << 2;
    r.d_word(1, k);
    // the lists are designated by their address
    let lists = &tables().set_lists;
    let mut at = tables().set_list_addresses.at32(b, k as u32);
    r.a(4, at);
    let mut last = None;
    loop {
        let kind = lists.at8(b, at.wrapping_sub(lists.rom));
        at += 1;
        r.a(4, at);
        r.d_byte(0, kind);
        r.flags(kind as u32, Size::Byte);
        if (kind as i8) < 0 {
            return last;
        }
        let o = r.allocated(b, &pools::SET)?;
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, kind);
        let setting = lists.at8(b, at.wrapping_sub(lists.rom));
        o.set_u8(b, SETTING, setting);
        let word = lists.at16(b, at.wrapping_sub(lists.rom) + 1);
        at += 3;
        r.a(4, at);
        r.d_word(0, word);
        put8(r, b, o, Obj::VARIANT, word as u8);
        last = Some(o);
    }
}

// ---------------------------------------------------------------------------------------------
// Enemy shots (ESHL) created by the family

/// 0xa836c: hit (ESHL 0x48, variant 0) of setting `setting`, linked by `+0x5c`.
pub(crate) fn eshl_slash_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj, setting: u8) -> Option<Obj> {
    let o = spawn_in(r, b, &pools::ESHL, kind::ESHL_SLASH)?;
    o.set_u8(b, Obj::VARIANT, 0);
    o.set_u8(b, SETTING, setting);
    link(r, b, o, link::CREATOR, creator.0);
    Some(o)
}

/// 0xa817c: rising column (ESHL 0x47) placed on the creator, 0x80 higher, right in front; D0 = 1, or 0 if the pool is
/// full.
pub(crate) fn eshl_rise_via(r: &mut impl Regs, b: &mut Cps2Bus, creator: Obj) -> Option<Obj> {
    let Some(o) = spawn_in(r, b, &pools::ESHL, kind::ESHL_RISE) else {
        r.d(0, 0);
        r.flags(0, Size::Long);
        return None;
    };
    o.set_u8(b, Obj::VARIANT, 0);
    o.set_u8(b, SETTING, 0);
    o.set_u16(b, link::SCENERY, creator.word());
    let facing = creator.u8(b, TURNED);
    o.set_u8(b, TURNED, facing);
    copy_position(b, o, creator);
    // `addi.w`, `subi.w`: their X remains after `moveq`
    let z = r.addition(0x80, o.u16(b, Obj::Z) as u32, Size::Word);
    o.set_u16(b, Obj::Z, z as u16);
    let y = r.subtract(1, o.u16(b, Obj::Y) as u32, Size::Word);
    o.set_u16(b, Obj::Y, y as u16);
    r.d(0, 1);
    r.flags(1, Size::Long);
    Some(o)
}

