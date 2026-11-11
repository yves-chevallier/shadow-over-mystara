//! EFFCT family, type 31 (`0xb1a98`): end-of-scene and countdown screens, ten variants.
//!
//! - 0: coin thrown by a chest (flies, falls back, waits for the end of the sequence);
//! - 2: reward icon that, at the end of the sequence, becomes the picked-up object (`0xe24d8`);
//! - 3: displayed rank; 4: plate; 5: result row of a player, drawn in background 2
//!   when it reaches the screen; 6: three-digit counters; 8: row per player;
//! - 7: character of the end screen that walks to its place, jumps, turns around;
//! - 1: trophy of the end board, which waits for the signal of its controller, animates after a randomly drawn delay
//!   then counts itself down with the controller; 9: sign placed on the alternate camera.

use crate::world::objects::player::Player;
use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use crate::hw::mem::{rd8, wr16, wr32};
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;
use crate::actors::families::effects::tables::{tables, Read, Table};
use crate::actors::families::effects::records::*;
use crate::actors::families::effects::vars;

mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const STEP: u32 = 0x06;
    pub const SUB_STEP: u32 = 0x07;
    pub const VARIANT: u32 = 0x03;
    pub const HIDDEN: u32 = 0x40;
    pub const FACING: u32 = 0x2c;
    pub const MODE: u32 = 0x2f;
    /// Displayed value (variants 3, 5, 9) or rank of the character (variant 7).
    pub const VALUE: u32 = 0x42;
    pub const SHOWN: u32 = 0x43;
    /// Sequence controller (long): its byte `+8` gives the step (2: end; negative: suspended).
    pub const SEQUENCE: u32 = 0x4c;
}

const ANIM_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c03a6);

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::EFFCT);
}

fn random<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    x.random() as u32
}

/// State 4 (`0xb2916`): goes to 6 and releases; state 6: releases.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STATE) == 4 {
        x.add8(f::STATE, 2);
    }
    free(x)
}

fn sequence_step<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let seq = x.u32(f::SEQUENCE);
    rd8(x.b, seq + 8)
}

fn countdown_screen<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(f::VARIANT);
    match (v, x.u8(f::STATE)) {
        (_, 4 | 6) => finish(x),
        (0, 0) => coin_init(x),
        (0, 2) => coin_active(x),
        (1, 0) => trophy_init(x),
        (1, 2) => trophy_active(x),
        (2, 0) => reward_init(x),
        (2, 2) => reward_active(x),
        (3, 0) => rank_init(x),
        (3, 2) => rank_active(x),
        (4, 0) => plate_init(x),
        (5, 0) => line_init(x),
        (5, 2) => line_active(x),
        (6, 0) => counters_init(x),
        (6, 2) => counters_active(x),
        (7, 0) => walker_init(x),
        (7, 2) => walker_active(x),
        (8, 0) => player_line_init(x),
        (9, 0) => sign_init(x),
        (9, 2) => {
            x.anim_step();
        }
        (8, 2) => {
            if rd8(x.b, 0xff2a39) as i8 >= 10 {
                finish_from_active(x);
            }
        }
        _ => {}
    }
}

/// End from the active state (`0xb2916`: `addq.b #2` then release).
fn finish_from_active<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    free(x)
}

// ---------------------------------------------------------------------------------------------
// variant 0: thrown coin
// ---------------------------------------------------------------------------------------------


fn coin_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let source = x.linked(Coin::SOURCE);
    let (sx, sz) = (source.u16(x.b, Obj::X), source.u16(x.b, Obj::Z));
    x.set16(Obj::X, sx);
    x.set16(Obj::Z, sz);
    x.set32(Obj::Y, 0x1_0000);
    let r = random(x) as u16 & 0x7f;
    let dx = if r & 1 != 0 { r.wrapping_neg() } else { r };
    x.add16(Obj::X, dx);
    let k = (random(x) & 7) << 3;
    x.set16(Obj::AX, 0);
    for (off, field) in [(0, Obj::VX), (2, Obj::VZ), (4, Obj::AZ)] {
        let v = tables().coin_speeds.at16(x.b, k + off);
        x.set16(field, v);
    }
    let r = random(x);
    x.set8(f::FACING, if r & 1 != 0 { 0x20 } else { 0 });
    let r = random(x);
    let anim = (tables().coin_anims.at8(x.b, r & 0xf) as u16).wrapping_add(0x104);
    x.anim_start(ANIM_SET.0, (r & 0xffff_0000 | anim as u32) as u16);
    x.set8(f::STEP, 0);
    x.reinsert();
}

fn coin_active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB_STATE) == 2 {
        if sequence_step(x) == 2 {
            x.add8(f::SUB_STATE, 2);
            x.set8(f::HIDDEN, 0);
        }
        return;
    }
    if (sequence_step(x) as i8) < 0 {
        return x.set8(f::HIDDEN, 1);
    }
    x.anim_step();
    x.move_xz();
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.set32(Obj::Y, 0);
    }
    if x.u16(Obj::Z) >= crate::world::ram::CAMERA_Y.get(x.b) {
        return x.reinsert();
    }
    // exit through the bottom of the screen
    x.add8(f::STATE, 2);
    x.set8(f::HIDDEN, 1);
}

// ---------------------------------------------------------------------------------------------
// variant 1: trophy of the end board
// ---------------------------------------------------------------------------------------------

mod trophy {
    /// Board controller (16-bit pointer): `+0x30` = phase (1: animate, 2: end), `+0x31` = trophies that have not
    /// animated yet.
    pub const CONTROLLER: u32 = 0x5c;
    pub const PHASE: u32 = 0x30;
    pub const PENDING: u32 = 0x31;
    /// Object whose variant chooses the animation (negative pointer), otherwise `+0x50` is the rank itself.
    pub const SOURCE: u32 = 0x50;
    pub const DELAY: u32 = 0x42;
    /// Scenery created when only five trophies remain, and which must have finished (`+0x22`).
    pub const SCENERY: u32 = 0x58;
    pub const SCENERY_KIND: u32 = 0x1d;
}

fn trophy_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let source = x.u16(trophy::SOURCE);
    let rank = if (source as i16) < 0 { x.linked(trophy::SOURCE).u8(x.b, f::VARIANT) as u16 } else { source };
    let mut place = (x.u8(f::MODE) as u32) << 3;
    let controller = x.linked(trophy::CONTROLLER);
    if controller.u8(x.b, f::VARIANT) == 0xb {
        place += 0x30;
    }
    let places = &tables().trophy_places;
    let (px, py, facing) = (places.at16(x.b, place), places.at16(x.b, place + 2), places.at8(x.b, place + 4));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set8(f::FACING, facing);
    x.set32(Obj::Z, 0);
    let mut anim = tables().reward_anims.at16(x.b, 2 * rank as u32);
    if x.u8(f::MODE) & 1 != 0 {
        anim = anim & 0xff00 | (anim as u8).wrapping_add(1) as u16;
    }
    x.anim_start(REWARD_SET.0, (anim as u32) as u16);
    x.reinsert();
}

fn trophy_active<R: Bench>(x: &mut Cx<'_, R>) {
    let controller = x.linked(trophy::CONTROLLER);
    match x.u8(f::SUB_STATE) {
        0 => {
            if controller.u8(x.b, trophy::PHASE) != 1 {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            let r = random(x) & 7;
            let delay = tables().trophy_delays.at8(x.b, r);
            x.set8(trophy::DELAY, delay);
        }
        2 => {
            let n = x.u8(trophy::DELAY).wrapping_sub(1);
            x.set8(trophy::DELAY, n);
            if n == 0 {
                x.add8(f::SUB_STATE, 2);
            }
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            let pending = controller.u8(x.b, trophy::PENDING).wrapping_sub(1);
            controller.set_u8(x.b, trophy::PENDING, pending);
            if pending != 5 {
                return x.add8(f::SUB_STATE, 2);
            }
            if let Some(scenery) = crate::actors::families::creators::scene_set(x.b, trophy::SCENERY_KIND as u8) {
                x.set16(trophy::SCENERY, scenery.0 as u16);
            }
        }
        6 => {
            if x.linked(trophy::SCENERY).u8(x.b, Carrier::HALTED) == 0 {
                return;
            }
            x.add8(f::SUB_STATE, 2);
            let pending = controller.u8(x.b, trophy::PENDING).wrapping_sub(1);
            controller.set_u8(x.b, trophy::PENDING, pending);
        }
        _ => {
            if controller.u8(x.b, trophy::PHASE) == 2 {
                finish_from_active(x);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// variant 9: sign placed on the alternate camera
// ---------------------------------------------------------------------------------------------


fn sign_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let (cy, cx) = (crate::world::ram::CAMERA_ALT_Y.get(x.b), crate::world::ram::CAMERA_ALT_X.get(x.b));
    x.set16(Obj::Z, cy);
    x.set16(Obj::X, cx);
    x.set32(Obj::Y, 0xffff_ffff);
    x.add16(Obj::X, 0x110);
    let k = 2 * x.u8(f::VALUE) as u32;
    let lift = tables().sign_heights.at16(x.b, k);
    x.add16(Obj::Z, lift);
    x.anim_start(ANIM_SET.0, 0x1b7);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// variant 2: reward icon
// ---------------------------------------------------------------------------------------------

const REWARD_SET: crate::world::data::RomAddr = crate::world::data::RomAddr(0x190ba8);

/// 0xb24b6: base animation of the mode + `n`.
fn reward_anim<R: Bench>(x: &mut Cx<'_, R>, n: u16) {
    let mode = x.u8(f::MODE) as u32;
    let base = tables().reward_anims.at16(x.b, 2 * mode);
    x.anim_start(REWARD_SET.0, (base.wrapping_add(n) as u32) as u16);
}

fn reward_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    reward_anim(x, 2);
    x.reinsert();
}

fn reward_active<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if x.u8(f::SUB_STATE) != 0 || sequence_step(x) != 2 {
        return;
    }
    reward_anim(x, 0xe);
    // becomes for an instant the object (type `+0x42`, variant `+0x2f`) for the duration of the pickup effect
    let variant = x.u8(f::VARIANT);
    let (mode, kind) = (x.u8(f::MODE), x.u8(f::VALUE));
    x.set8(f::VARIANT, mode);
    x.set8(Obj::KIND, kind);
    // the variant is kept in D0.b during the sound, then put back
    crate::services::sound::play(x.b, crate::services::sound::CMD_A9_BY_CHARACTER, x.o);
    x.set8(f::VARIANT, variant);
    x.set8(Obj::KIND, 31);
    x.add8(f::SUB_STATE, 2);
}

// ---------------------------------------------------------------------------------------------
// variants 3, 4, 6, 8: rank, plate, counters, player rows
// ---------------------------------------------------------------------------------------------

fn at_camera<R: Bench>(x: &mut Cx<'_, R>, cx: u32, cy: u32) {
    let (px, pz) = (x.a5_u16(cx), x.a5_u16(cy));
    x.set16(Obj::X, px);
    x.set16(Obj::Z, pz);
    x.set32(Obj::Y, 0);
}

fn rank_anim<R: Bench>(x: &mut Cx<'_, R>, value: u8) {
    x.anim_start(ANIM_SET.0, (0xe3u16.wrapping_sub(value as u16) as u32) as u16);
    x.reinsert();
}

fn rank_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    at_camera(x, 0x506, 0x50a);
    x.add16(Obj::X, 0x13c);
    x.add16(Obj::Z, 0xcc);
    let value = x.u8(f::VALUE);
    x.set8(f::SHOWN, value);
    rank_anim(x, value);
}

fn rank_active<R: Bench>(x: &mut Cx<'_, R>) {
    let value = x.u8(f::VALUE);
    if value != x.u8(f::SHOWN) {
        x.set8(f::SHOWN, value);
        rank_anim(x, value);
    }
}

fn plate_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let cam = crate::world::ram::CAMERA_ALT_X.get(x.b);
    x.set16(Obj::X, cam);
    x.set32(Obj::Y, 0);
    x.set16(Obj::Z, 0x460);
    x.add16(Obj::X, 0x80);
    let anim = if vars::DAMAGE_SETTING.get(x.b) == 0 { 0x168 } else { 0x169 };
    x.anim_start(ANIM_SET.0, anim);
    x.reinsert();
}

/// Scene counters (two bytes at `A5-0x5b21`) and their places in background 2.
const COUNTERS: u32 = 0xff24df;
const COUNTER_TILES: [u32; 2] = [0x910f6c, 0x910f74];

/// 0xb2348: binary value (< 1000) in BCD on three digits.
fn to_bcd(value: u16) -> u16 {
    let (mut rest, mut bcd) = (value, 0u16);
    loop {
        rest = rest.wrapping_sub(100);
        if (rest as i16) < 0 {
            break;
        }
        bcd += 0x100;
    }
    rest = rest.wrapping_add(100);
    loop {
        rest = rest.wrapping_sub(10);
        if (rest as i16) < 0 {
            break;
        }
        bcd += 0x10;
    }
    rest.wrapping_add(10).wrapping_add(bcd)
}

/// 0xb230e: block of 2 × 2 tiles (`tile`, `tile + 0x10`; next row `tile + 1`...), then the following row. Returns the
/// next tile (`tile + 2`) and the new address.
fn draw_block<R: Bench>(x: &mut Cx<'_, R>, at: &mut u32, tile: u16, attr: u16) -> u16 {
    let mut t = tile;
    for _ in 0..2 {
        wr16(x.b, *at, t);
        wr16(x.b, *at + 2, attr);
        wr16(x.b, *at + 4, t.wrapping_add(0x10));
        wr16(x.b, *at + 6, attr);
        *at += 0x80;
        t = t.wrapping_add(1);
    }
    t
}

/// 0xb22d0: three-digit BCD number in big digits (leading zeros erased).
fn draw_big_number<R: Bench>(x: &mut Cx<'_, R>, mut at: u32, bcd: u16) {
    let mut shown = false;
    for i in 0..4 {
        let digit = bcd.rotate_left(4 * (i + 1)) & 0xf;
        if i == 3 {
            shown = true;
        }
        if digit != 0 {
            shown = true;
        } else if !shown {
            at += 0x100;
            continue;
        }
        let tile = tables().digit_tiles.at16(x.b, 2 * digit as u32);
        draw_block(x, &mut at, tile, 3);
    }
}

fn counters_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let cam = crate::world::ram::CAMERA_ALT_X.get(x.b);
    x.set16(Obj::X, cam);
    x.set32(Obj::Y, 0);
    x.set16(Obj::Z, 0x448);
    x.add16(Obj::X, 0x50);
    let at = if vars::DAMAGE_SETTING.get(x.b) == 0 { 0xb2af0 } else { 0xb2b6a };
    x.anim_start_entry(at);
    x.reinsert();
    for (k, at) in COUNTER_TILES.into_iter().enumerate() {
        let value = rd8(x.b, COUNTERS + k as u32) as u16;
        draw_big_number(x, at, to_bcd(value));
    }
}

fn counters_active<R: Bench>(x: &mut Cx<'_, R>) {
    let gap = x.u16(Obj::Z).wrapping_sub(crate::world::ram::CAMERA_ALT_Y.get(x.b));
    if (gap as i16) < 0x100 {
        return;
    }
    for at in COUNTER_TILES {
        for row in 0..3 {
            wr32(x.b, at + 0x80 * row, 0x20_0000);
            wr32(x.b, at + 0x80 * row + 4, 0x20_0000);
        }
    }
}

fn player_line_init<R: Bench>(x: &mut Cx<'_, R>) {
    let mode = x.u8(f::MODE) as u32;
    let player = Obj(0xff85de + 0x200 * mode);
    let present = player.is_active(x.b)
        && (player.u8(x.b, f::STATE) as i8) < 4
        && player.u32(x.b, Player::NAME_HEAD) == 0xffff_ffff
        && player.u32(x.b, Player::NAME_TAIL) == 0xffff_ffff;
    if !present {
        return finish_from_active(x);
    }
    x.add8(f::STATE, 2);
    at_camera(x, 0x506, 0x50a);
    x.add16(Obj::X, 0x10);
    x.add16(Obj::Z, 0xb8);
    x.add16(Obj::Z, ((mode as u16) << 5).wrapping_neg());
    x.anim_start(ANIM_SET.0, (0x173 + mode) as u16);
    x.reinsert();
}

// ---------------------------------------------------------------------------------------------
// variant 5: result row of a player
// ---------------------------------------------------------------------------------------------

mod line {
    pub const LAYER: u32 = 0x912000;
    pub const PALETTES: u32 = 0x91c600;
}

fn line_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let (cam_x, cam_y) = (crate::world::ram::CAMERA_ALT_X.get(x.b), crate::world::ram::CAMERA_ALT_Y.get(x.b));
    x.set16(Obj::Z, cam_y);
    x.set16(Obj::X, cam_x);
    x.set32(Obj::Y, 0);
    x.add16(Obj::X, 0x20);
    let rank = x.u8(f::VALUE) as u32;
    let dz = tables().line_offsets.at16(x.b, 2 * rank);
    x.add16(Obj::Z, dz);
    let mode = x.u8(f::MODE) ^ 3;
    x.set8(f::MODE, mode);
    x.anim_start(ANIM_SET.0, 0x16a);
    x.reinsert();
}

/// The (rising) row reaches the top of the camera: it is drawn once in background 2.
fn line_active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB_STATE) != 0 || (x.u16(Obj::Z) as i16) < (crate::world::ram::CAMERA_ALT_Y.get(x.b) as i16) {
        return;
    }
    x.add8(f::SUB_STATE, 2);
    let rank = x.u8(f::VALUE) as u32;
    let place = tables().line_places.at16(x.b, 2 * rank) as i16 as i32 as u32;
    let base = line::LAYER.wrapping_add(place);
    let record = x.u32(f::SEQUENCE);
    let mut d = LineDraw { record, attr: 0, shown: false };
    d.portrait(x, base + 0x180);
    d.number(x, base);
    d.level(x, base + 0x8c);
    d.name(x, base + 0x400);
    d.score(x, base + 0xa88);
    d.title(x, base + 0x408);
}

/// Drawing of a row: the player's sheet (`+0x4c`) and the current attribute (D1 of the original), which passes from one
/// element to the next.
struct LineDraw {
    record: u32,
    attr: u16,
    shown: bool,
}

impl LineDraw {
    fn class<R: Bench>(&self, x: &mut Cx<'_, R>) -> u8 {
        rd8(x.b, self.record + 0xc)
    }

    /// 0xb222e: tile pair `tile`, `tile + 0x10`, then next row.
    fn pair<R: Bench>(&self, x: &mut Cx<'_, R>, at: &mut u32, tile: u16) {
        wr16(x.b, *at, tile);
        wr16(x.b, *at + 2, self.attr);
        wr16(x.b, *at + 4, tile.wrapping_add(0x10));
        wr16(x.b, *at + 6, self.attr);
        *at += 0x80;
    }

    /// 0xb2212: small digit (leading zeros erased as long as `shown` is false).
    fn digit<R: Bench>(&mut self, x: &mut Cx<'_, R>, at: &mut u32, digit: u16) {
        let digit = digit & 0xf;
        if digit == 0 && !self.shown {
            *at += 0x80;
            return;
        }
        self.shown = true;
        let tile = tables().line_small_digits.at16(x.b, 2 * digit as u32);
        self.pair(x, at, tile);
    }

    /// 0xb20d2: tile string (one per byte) terminated by 0x40, one column per character.
    fn text<R: Bench>(&self, x: &mut Cx<'_, R>, at: &mut u32, table: &Table, mut s: u32) {
        loop {
            let c = table.at8(x.b, s);
            s += 1;
            if c == 0x40 {
                return;
            }
            wr16(x.b, *at, c as u16);
            wr16(x.b, *at + 2, self.attr);
            *at += 0x80;
        }
    }

    /// 0xb1ff8: portrait (palette copied, then 4 columns of 2 tiles).
    fn portrait<R: Bench>(&mut self, x: &mut Cx<'_, R>, mut at: u32) {
        let class = self.class(x) as u32;
        let mode = x.u8(f::MODE) as u32;
        let src = class << 5;
        let dst = line::PALETTES + (mode << 5);
        for k in 0..8 {
            let v = crate::services::palette::library_excerpt!("line_portraits", 0x25c4c0, 0x25c6c0, 4).at32(x.b, src + 4 * k) | 0xf000_f000;
            wr32(x.b, dst + 4 * k, v);
        }
        let mut tile = tables().line_portrait_tiles.at16(x.b, 2 * class);
        self.attr = 0x10u8.wrapping_add(mode as u8) as u16;
        for _ in 0..4 {
            self.pair(x, &mut at, tile);
            tile = tile.wrapping_add(1);
        }
    }

    /// 0xb21d8: player number (small digit) and a fixed tile.
    fn number<R: Bench>(&mut self, x: &mut Cx<'_, R>, mut at: u32) {
        let mode = x.u8(f::MODE);
        self.attr = tables().line_number_attrs.at8(x.b, mode as u32) as u16;
        self.shown = true;
        self.digit(x, &mut at, mode.wrapping_add(1) as u16);
        self.pair(x, &mut at, 0x211);
    }

    /// 0xb2078: player level (two digits) after two fixed tiles.
    fn level<R: Bench>(&mut self, x: &mut Cx<'_, R>, mut at: u32) {
        for tile in [0x4c, 0x5f] {
            wr16(x.b, at, tile);
            wr16(x.b, at + 2, self.attr);
            at += 0x80;
        }
        let bcd = to_bcd(rd8(x.b, self.record + 0xd) as u16);
        for digit in [(bcd >> 4) & 0xf, bcd & 0xf] {
            wr16(x.b, at, digit);
            wr16(x.b, at + 2, self.attr);
            at += 0x80;
        }
    }

    /// 0xb20b8: character name.
    fn name<R: Bench>(&mut self, x: &mut Cx<'_, R>, mut at: u32) {
        let mode = x.u8(f::MODE) as u32;
        self.attr = tables().line_name_attrs.at8(x.b, mode) as u16;
        let pair = (self.class(x) & !1) as u32;
        let s = tables().line_names.at16(x.b, 2 * pair) as u32;
        let s = s << 16 | tables().line_names.at16(x.b, 2 * pair + 2) as u32;
        // the names are strings of `line_name_texts`, designated by their address
        let texts = &tables().line_name_texts;
        self.text(x, &mut at, texts, s.wrapping_sub(texts.rom));
    }

    /// 0xb219e: three fixed tiles, the score (eight BCD digits), then the text that follows.
    fn score<R: Bench>(&mut self, x: &mut Cx<'_, R>, mut at: u32) {
        for tile in [0x1f6, 0x219, 0x211] {
            self.pair(x, &mut at, tile);
        }
        at += 0x100;
        self.shown = false;
        let score = crate::hw::mem::rd32(x.b, self.record + 8);
        for i in 0..8 {
            self.digit(x, &mut at, (score >> (28 - 4 * i)) as u16);
        }
        at += 4;
        self.text(x, &mut at, &tables().line_score_suffix, 0);
    }

    /// 0xb212c: title in big characters; 0xff: default title of the character copied into the sheet; 0xf9 to 0xfe:
    /// special title of six consecutive characters.
    fn title<R: Bench>(&mut self, x: &mut Cx<'_, R>, mut at: u32) {
        let mut p = self.record;
        if rd8(x.b, p) == 0xff {
            let mut k = (self.class(x) as u32) << 3;
            if vars::DAMAGE_SETTING.get(x.b) != 0 {
                k += 0x60;
            }
            let (a, b) = (tables().line_default_titles.at32(x.b, k), tables().line_default_titles.at32(x.b, k + 4));
            wr32(x.b, p, a);
            wr32(x.b, p + 4, b);
        }
        let first = rd8(x.b, p);
        if first >= 0xf9 {
            let k = ((first as i8 as i16).wrapping_neg() - 2) as i32 * 2;
            let mut tile = tables().line_special_titles.at16(x.b, k as u32);
            for _ in 0..6 {
                tile = draw_block(x, &mut at, tile, self.attr);
            }
            return;
        }
        loop {
            let c = rd8(x.b, p);
            p += 1;
            if c == 0xff {
                return;
            }
            let tile = tables().line_glyphs.at16(x.b, 2 * c as u32);
            draw_block(x, &mut at, tile, self.attr);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// variant 7: character of the end screen
// ---------------------------------------------------------------------------------------------

mod walker {
    pub const TARGET_X: u32 = 0x50;
    pub const TARGET_Y: u32 = 0x52;
    /// Object that the character notifies when leaving (16-bit pointer), whose `+0x23` counts the characters still on
    /// their way.
    pub const WAITER: u32 = 0x46;
    pub const WALK_SPEEDS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c75b6);
    pub const RUN_SPEEDS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1c7eb6);
}

fn walker_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    let places = &tables().walker_places;
    let place = 12 * x.u8(f::VALUE) as u32;
    for (k, off) in [Obj::X, Obj::Z, Obj::Y, walker::TARGET_X, walker::TARGET_Y].into_iter().enumerate() {
        let v = places.at16(x.b, place + 2 * k as u32);
        x.set16(off, v);
    }
    let sub = places.at8(x.b, place + 10);
    x.set8(f::SUB_STATE, sub);
    let (cam_x, cam_y) = (crate::world::ram::CAMERA_ALT_X.get(x.b), crate::world::ram::CAMERA_ALT_Y.get(x.b));
    x.add16(Obj::X, cam_x);
    x.add16(walker::TARGET_X, cam_x);
    x.add16(Obj::Y, cam_y);
    x.add16(walker::TARGET_Y, cam_y);
    let left = (x.u16(Obj::X) as i16) < (x.u16(walker::TARGET_X) as i16);
    x.set8(f::FACING, if left { 0x20 } else { 0 });
}

fn walker_active<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB_STATE) {
        0 => walker_wait(x),
        2 => walker_walk(x),
        4 => walker_jump(x),
        6 => walker_turn(x),
        8 => walker_run(x),
        _ => {}
    }
    x.reinsert();
}

fn walker_facing_target<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    if (x.u16(Obj::X) as i16) > (x.u16(walker::TARGET_X) as i16) {
        0
    } else {
        0x20
    }
}

/// 0xb27aa: about-turn, then return to the current substate.
fn walker_start_turn<R: Bench>(x: &mut Cx<'_, R>) {
    let sub = x.u8(f::SUB_STATE);
    x.set8(f::SUB_STEP, sub);
    x.set8(f::STEP, 0);
    x.set8(f::SUB_STATE, 6);
    walker_turn(x);
}

fn walker_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STEP) == 0 {
        if x.u8(f::FACING) != 0 {
            return walker_start_turn(x);
        }
        x.add8(f::STEP, 2);
        reward_anim(x, 2);
        let waiter = x.linked(walker::WAITER);
        let n = waiter.u8(x.b, Carrier::WAITING).wrapping_sub(1);
        waiter.set_u8(x.b, Carrier::WAITING, n);
    }
    x.anim_step();
}

/// One step toward the target (direction 0x137e, current speed table).
fn walker_move<R: Bench>(x: &mut Cx<'_, R>) {
    let (tx, ty) = (x.u16(walker::TARGET_X), x.u16(walker::TARGET_Y));
    let direction = crate::actors::aids::direction_from(x.b, x.o, (tx, ty));
    x.set8(Obj::SPEED_INDEX, direction);
    x.move_by_table_xy();
}

/// The target is reached within 8 on the axis `off`.
fn walker_near<R: Bench>(x: &mut Cx<'_, R>, off: u32, target: u32) -> bool {
    x.u16(off).wrapping_sub(x.u16(target)).wrapping_add(8) <= 0x10
}

fn walker_walk<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STEP) == 0 {
        if walker_facing_target(x) != x.u8(f::FACING) {
            return walker_start_turn(x);
        }
        x.add8(f::STEP, 2);
        reward_anim(x, 4);
        x.set32(Obj::SPEED_TABLE, walker::WALK_SPEEDS.0);
    }
    walker_move(x);
    x.anim_step();
    if walker_near(x, Obj::X, walker::TARGET_X) && walker_near(x, Obj::Y, walker::TARGET_Y) {
        x.set8(f::SUB_STATE, 0);
        x.set16(f::STEP, 0);
        reward_anim(x, 2);
    }
}

fn walker_jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 2);
            x.set8(f::SUB_STEP, 0);
            reward_anim(x, 5);
            x.set16(Obj::VX, 0xfc00);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, 0xffc0);
        }
        2 => match x.u8(f::SUB_STEP) {
            0 => {
                if x.anim_step() {
                    reward_anim(x, 6);
                    x.add8(f::SUB_STEP, 2);
                }
            }
            2 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(Obj::VZ) as i16) <= 0 {
                    reward_anim(x, 7);
                    x.add8(f::SUB_STEP, 2);
                }
            }
            4 => {
                x.anim_step();
                x.move_xz();
                if (x.u16(Obj::Z) as i16) <= 0 {
                    x.add8(f::STEP, 2);
                    x.set8(f::SUB_STEP, 0);
                    x.set32(Obj::Z, 0);
                    reward_anim(x, 8);
                }
            }
            _ => {}
        },
        4 => {
            if x.anim_step() {
                x.set8(f::SUB_STATE, 2);
                x.set16(f::STEP, 0);
            }
        }
        _ => {}
    }
}

fn walker_turn<R: Bench>(x: &mut Cx<'_, R>) {
    let late = (x.u8(f::MODE) as i8) >= 0xa;
    if x.u8(f::STEP) == 0 {
        x.add8(f::STEP, 2);
        if !late {
            let facing = x.u8(f::FACING) ^ 0x20;
            x.set8(f::FACING, facing);
        }
        reward_anim(x, 3);
    }
    if !x.anim_step() {
        return;
    }
    let back = x.u8(f::SUB_STEP);
    x.set8(f::SUB_STATE, back);
    x.set16(f::STEP, 0);
    if late {
        let facing = x.u8(f::FACING) ^ 0x20;
        x.set8(f::FACING, facing);
    }
    reward_anim(x, 2);
}

fn walker_run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STEP) {
        0 => {
            x.add8(f::STEP, 2);
            x.set8(f::SUB_STEP, 0);
            x.set16(Obj::AX, 0);
            let vx: u16 = if x.u8(f::FACING) == 0 { 0x380u16.wrapping_neg() } else { 0x380 };
            x.set16(Obj::VX, vx);
            x.set32(Obj::SPEED_TABLE, walker::RUN_SPEEDS.0);
            reward_anim(x, 9);
        }
        2 => {
            x.anim_step();
            walker_move(x);
            if !walker_near(x, Obj::X, walker::TARGET_X) {
                return;
            }
            x.add8(f::STEP, 2);
            if x.u8(f::FACING) != 0 {
                x.add8(f::STEP, 2);
                x.set16(Obj::AX, 0xffe0);
                reward_anim(x, 0xc);
            } else {
                x.set16(Obj::AX, 0x20);
                reward_anim(x, 0xa);
            }
        }
        step @ (4 | 6) => {
            if x.u8(f::SUB_STEP) == 0 {
                x.anim_step();
                x.move_x();
                if x.u8(Obj::VX) != 0 {
                    return;
                }
                x.add8(f::SUB_STEP, 2);
                reward_anim(x, if step == 4 { 0xb } else { 0xd });
            } else if x.u8(f::SUB_STEP) == 2 {
                if !x.anim_step() {
                    return;
                }
                x.set8(f::SUB_STATE, 0);
                x.set16(f::STEP, 0);
                if step == 6 {
                    let facing = x.u8(f::FACING) ^ 0x20;
                    x.set8(f::FACING, facing);
                    reward_anim(x, 2);
                }
            }
        }
        _ => {}
    }
}

handlers! {
    EFFCT_COUNTDOWN 0xb1a98 "effct_countdown" countdown_screen,
}
