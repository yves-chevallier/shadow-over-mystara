use crate::player::Player;
use super::*;

// ---------------------------------------------------------------------------------------------
// Type 22 : comptoir de boutique
// ---------------------------------------------------------------------------------------------

pub(super) mod shop {
    pub const ANIM_SET: crate::data::RomAddr = crate::data::RomAddr(0x1c40c6);
    /// Purchase (`0xafad0`): per variant, a `bra.w` to the purchase kind (78 entries, `shop_purchases`).
    pub const PURCHASE_COUNT: u32 = 78;
    /// Purchase kinds: targets of the `bra.w`, as offsets from the start of the branches (`0xafade`).
    pub const EXIT: u32 = 0x138;
    pub const MEAL_BIG: u32 = 0x156;
    pub const MEAL: u32 = 0x15c;
    pub const NOT_CLASS_4: u32 = 0x1d6;
    pub const NOT_CLASSES_2_4_1: u32 = 0x1f2;
    pub const NOT_CLASSES_4_1: u32 = 0x20a;
    pub const NOT_CLASS_1: u32 = 0x222;
    /// Shop messages (`A5-0x554e`): insufficient gold, class refusal, full life,
    /// object at maximum, inventory full.
    pub const NO_GOLD: u16 = 0x82;
    pub const WRONG_CLASS: u16 = 0x84;
    pub const FULL_LIFE: u16 = 0x85;
    pub const TOO_MANY: u16 = 0x86;
    pub const NO_ROOM: u16 = 0x87;
    pub const SOUND_REFUSED: crate::sound::Sound = crate::sound::CMD_136_236;
    pub const SOUND_BOUGHT: crate::sound::Sound = crate::sound::CMD_135_235;
    /// Pointers (words) of the four possible buyers.
    pub const BUYERS: u32 = 0xff2aa4;
    /// Annonce en cours (`A5-0x554e`).
    pub const MESSAGE: u32 = 0xff2ab2;
    // champs
    pub const STOCK_LEFT: u32 = 0x18;
    pub const PRICE: u32 = 0x50;
    pub const ITEM: u32 = 0x58;
    pub const MESSAGE_ID: u32 = 0x5a;
    pub const ANNOUNCED: u32 = 0x5c;
    pub const OCCUPIED: u32 = 0x5d;
}

pub(super) fn shop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => shop_init(x),
        2 => shop_watch(x, shop_buy),
        _ => free(x),
    }
}

pub(super) fn shop_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    // scene 0x4c, second pass: items from the second half of the table
    let v = x.u8(f::VARIANT);
    if crate::ram::SCENE.get(x.b) == 0x4c && (v as i8) < 0x40 && vars::SECOND_PASS.get(x.b) != 0 && v != 0 {
        x.set8(f::VARIANT, if v == 0x14 { 0x0d } else { v }.wrapping_add(0x40));
    }
    let v = x.u8(f::VARIANT) as u32;
    let zone = 8 * v;
    for (k, off) in [0x14, 0x16, 0x54, 0x56].into_iter().enumerate() {
        let w = tables().shop_zones.at16(x.b, zone + 2 * k as u32);
        x.set16(off, w);
    }
    let level = crate::ram::PLAYERS_ALIVE.get(x.b) as u32;
    let stock = tables().shop_stock.at8(x.b, level);
    x.set8(shop::STOCK_LEFT, stock);
    let price = tables().shop_prices.at32(x.b, 4 * v);
    x.set32(shop::PRICE, price);
    let message = tables().shop_messages.at16(x.b, 2 * v);
    x.set16(shop::MESSAGE_ID, message);
    let item = tables().shop_items.at16(x.b, 2 * v);
    x.set16(shop::ITEM, item);
    let anim = tables().shop_animations.at16(x.b, 2 * v) as u32;
    x.anim_start(shop::ANIM_SET.0, anim as u16);
    x.reinsert();
}

/// Looks for a buyer in the zone; announces the item to the first one who enters. `buy` receives the buyer and its slot
/// in the buyers list (A1 of the original's loop).
pub(super) fn shop_watch<R: Bench>(x: &mut Cx<'_, R>, buy: fn(&mut Cx<'_, R>, Obj, u32)) {
    x.set8(shop::OCCUPIED, 0);
    for k in 0..4u32 {
        let slot = shop::BUYERS + 2 * k;
        let p = rd16(x.b, slot);
        if p != 0 {
            let buyer = Obj(p as i16 as i32 as u32 & 0xff_ffff);
            if shop_in_zone(x, buyer) {
                x.set8(shop::OCCUPIED, 1);
                let message = x.u16(shop::MESSAGE_ID);
                if x.u8(shop::ANNOUNCED) == 0 && message != 0 {
                    crate::mem::wr16(x.b, shop::MESSAGE, message);
                    x.set8(shop::ANNOUNCED, 1);
                }
                if (buyer.u8(x.b, Buyer::PURCHASE) as i8) > 0 {
                    buy(x, buyer, slot);
                }
            }
        }
    }
    if x.u8(shop::OCCUPIED) == 0 {
        x.set8(shop::ANNOUNCED, 0);
    }
}

/// Player of a buyer (`+0x50`).
pub(super) fn buyer_player<R: Bench>(x: &mut Cx<'_, R>, buyer: Obj) -> Obj {
    let p = buyer.u16(x.b, Buyer::PLAYER);
    Obj(p as i16 as i32 as u32 & 0xff_ffff)
}

/// Refusal: sound, then message.
pub(super) fn shop_refuse<R: Bench>(x: &mut Cx<'_, R>, message: u16) {
    crate::sound::play(x.b, shop::SOUND_REFUSED, x.o);
    crate::mem::wr16(x.b, shop::MESSAGE, message);
}

/// `0xafad0`: purchase by buyer `buyer` (A2), from slot `slot` of the buyers list, according to the purchase
/// kind of the variant.
pub(super) fn shop_buy<R: Bench>(x: &mut Cx<'_, R>, buyer: Obj, slot: u32) {
    let v = x.u8(f::VARIANT) as u32;
    if v >= shop::PURCHASE_COUNT {
        return;
    }
    // `bra.w` (opcode, displacement from the displacement word)
    let at = 4 * v + 2;
    let kind = at.wrapping_add(tables().shop_purchases.at16(x.b, at) as i16 as i32 as u32);
    match kind {
        shop::EXIT => {
            // the exit: the buyer leaves the list and goes to state 4, the palette comes back
            crate::mem::wr16(x.b, slot, 0);
            buyer.set_u16(x.b, Obj::STATE, 0x400);
            vars::FADE_PAGES.set(x.b, 0xf);
            crate::palette::full_brightness(x.b);
        }
        shop::MEAL_BIG => shop_meal(x, buyer, 0x30),
        shop::MEAL => shop_meal(x, buyer, 0x20),
        _ => {
            buyer.set_u8(x.b, Buyer::PURCHASE, 0xff);
            let class = buyer_player(x, buyer).u8(x.b, Obj::VARIANT) >> 1;
            let refused: &[(u8, u16)] = match kind {
                shop::NOT_CLASS_4 => &[(4, shop::WRONG_CLASS)],
                shop::NOT_CLASSES_2_4_1 => &[(2, shop::TOO_MANY), (4, shop::WRONG_CLASS), (1, shop::WRONG_CLASS)],
                shop::NOT_CLASSES_4_1 => &[(4, shop::WRONG_CLASS), (1, shop::WRONG_CLASS)],
                shop::NOT_CLASS_1 => &[(1, shop::WRONG_CLASS)],
                _ => &[],
            };
            if let Some(&(_, message)) = refused.iter().find(|&&(c, _)| c == class) {
                if message == shop::TOO_MANY {
                    crate::mem::wr16(x.b, shop::MESSAGE, message);
                    return crate::sound::play(x.b, shop::SOUND_REFUSED, x.o);
                }
                return shop_refuse(x, message);
            }
            shop_take(x, buyer);
        }
    }
}

/// Is the player's gold sufficient (or free item: `+0x167` = 9)? Otherwise refusal.
pub(super) fn shop_can_pay<R: Bench>(x: &mut Cx<'_, R>, player: Obj) -> bool {
    if player.u8(x.b, Player::FREE_ITEM) == 9 || player.u32(x.b, GOLD) as i32 >= x.u32(shop::PRICE) as i32 {
        return true;
    }
    shop_refuse(x, shop::NO_GOLD);
    false
}

/// `0xafd18`: the item joins the inventory (six slots `+0x140`, quantities `+0x148`): same
/// item, or empty slot (0x56); at most 9.
pub(super) fn shop_take<R: Bench>(x: &mut Cx<'_, R>, buyer: Obj) {
    buyer.set_u8(x.b, Buyer::PURCHASE, 0xff);
    let player = buyer_player(x, buyer);
    if !shop_can_pay(x, player) {
        return;
    }
    let item = x.u16(shop::ITEM) as u8;
    let slot = match (0..6).find(|&k| player.u8(x.b, Player::INVENTORY + k) == item) {
        Some(k) => k,
        None => match (0..6).find(|&k| player.u8(x.b, Player::INVENTORY + k) == 0x56) {
            Some(k) => {
                player.set_u8(x.b, Player::QUANTITIES + k, 0);
                k
            }
            None => {
                crate::mem::wr16(x.b, shop::MESSAGE, shop::NO_ROOM);
                return crate::sound::play(x.b, shop::SOUND_REFUSED, x.o);
            }
        },
    };
    if player.u8(x.b, Player::QUANTITIES + slot) as i8 >= 9 {
        crate::mem::wr16(x.b, shop::MESSAGE, shop::TOO_MANY);
        return crate::sound::play(x.b, shop::SOUND_REFUSED, x.o);
    }
    let id = x.u8(shop::ITEM + 1);
    player.set_u8(x.b, Player::INVENTORY + slot, id);
    let n = player.u8(x.b, Player::QUANTITIES + slot).wrapping_add(1);
    player.set_u8(x.b, Player::QUANTITIES + slot, n);
    let collected = x.u8(shop::ITEM) as u32;
    crate::mem::wr8(x.b, A5.wrapping_sub(0x5b88).wrapping_add(collected), 1);
    shop_pay(x, player);
}

/// `0xafdaa`: the price (six BCD bytes ending at `+0x53`) is subtracted from the player's gold (six bytes
/// ending at `+0x117`), except for a free item (`+0x167` = 9, reset to 0xff).
pub(super) fn shop_pay<R: Bench>(x: &mut Cx<'_, R>, player: Obj) {
    if player.u8(x.b, Player::FREE_ITEM) == 9 {
        player.set_u8(x.b, Player::FREE_ITEM, 0xff);
    } else {
        let mut borrow = false;
        for k in 1..=6 {
            let (v, b) = crate::game::sbcd(player.u8(x.b, Player::GOLD_END - k), x.u8(ShopPrice::PRICE_END - k), borrow);
            player.set_u8(x.b, Player::GOLD_END - k, v);
            borrow = b;
        }
    }
    crate::sound::play(x.b, shop::SOUND_BOUGHT, x.o);
}

/// `0xafc3c`: meal (healing `heal`): refused if life is full; the last of the stock moves the
/// stall back and clears its announcement.
pub(super) fn shop_meal<R: Bench>(x: &mut Cx<'_, R>, buyer: Obj, heal: u16) {
    buyer.set_u8(x.b, Buyer::PURCHASE, 0xff);
    if x.u8(shop::STOCK_LEFT) == 0 {
        return;
    }
    let player = buyer_player(x, buyer);
    let full = player.u16(x.b, Player::HP_MAX);
    if full as i16 <= player.u16(x.b, Player::HP) as i16 {
        crate::mem::wr16(x.b, shop::MESSAGE, shop::FULL_LIFE);
        return crate::sound::play(x.b, shop::SOUND_REFUSED, x.o);
    }
    if !shop_can_pay(x, player) {
        return;
    }
    let left = x.u8(shop::STOCK_LEFT).wrapping_sub(1);
    x.set8(shop::STOCK_LEFT, left);
    if left == 0 {
        x.add16(Obj::X, 0xfff0);
        x.add16(Obj::Z, 0xffe8);
        x.anim_step();
        x.set16(shop::MESSAGE_ID, 0);
    }
    let life = player.u16(x.b, Player::HP).wrapping_add(heal);
    player.set_u16(x.b, Player::HP, life);
    if (full as i16) < life as i16 {
        player.set_u16(x.b, Player::HP, full);
    }
    shop_pay(x, player);
}

pub(super) fn shop_in_zone<R: Bench>(x: &mut Cx<'_, R>, buyer: Obj) -> bool {
    let across = x.u16(Obj::X).wrapping_sub(buyer.u16(x.b, Obj::X)).wrapping_add(x.u16(Shop::ZONE_DX));
    if across > x.u16(Shop::ZONE_HALF_WIDTH) {
        return false;
    }
    let depth = x
        .u16(Obj::Z)
        .wrapping_add(x.u16(Obj::Y))
        .wrapping_sub(buyer.u16(x.b, Obj::Z))
        .wrapping_sub(buyer.u16(x.b, Obj::Y))
        .wrapping_add(x.u16(Shop::ZONE_DY));
    depth <= x.u16(Shop::ZONE_HALF_DEPTH)
}

// ---------------------------------------------------------------------------------------------
// Type 20 : curseur de don d'or (boutique)
// ---------------------------------------------------------------------------------------------

pub(super) mod purse {
    pub const ANIM_SET: crate::data::RomAddr = crate::data::RomAddr(0x1c40c6);
    /// Continuation of the original code when a pool is full: `tst.w (1,A5)` (`effects::address_error`).
    pub const POOL_FULL: crate::data::RomAddr = crate::data::RomAddr(0xaf2bc);
    /// Nombre de curseurs encore ouverts (`A5-0x5553`).
    pub const OPEN: u32 = 0xff2aad;
    pub const OWNER: u32 = 0x50;
    pub const GAUGE_LEN: u32 = 0x18;
    pub const DIGITS_AT: u32 = 0x52;
    pub const GAUGE_AT: u32 = 0x14;
    pub const GIFT: u32 = 0x1a;
    pub const LINKED: u32 = 0x5c;
    pub const OPENED: u32 = 0x58;
}

/// `0xc8312`: SET object of the scene; `None` if the pool is full.
pub(super) fn scene_set_of<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
    crate::families::creators::scene_set(x.b, variant)
}

/// The A1 that a failed allocation leaves (`Pool::alloc_or_leftover`): first slot if the counter is zero, end
/// of the pool otherwise.
pub(super) fn leftover<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool) -> Obj {
    if pool.free_count(x.b) == 0 { pool.slot(0) } else { Obj(pool.end()) }
}

/// Player's gold (`+0x114`, BCD on four bytes).
pub(super) const GOLD: u32 = 0x114;

pub(super) fn two_players<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    crate::ram::CABINET.get(x.b) == 1
}

pub(super) fn purse<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => purse_init(x),
        2 => purse_active(x),
        4 => purse_leave(x),
        _ => free(x),
    }
}

/// Allocation by an original routine: the slot (A1), or `None` (carry).
pub(super) fn purse_init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set16(Purse::GAUGE_LEN, 0);
    let (cx, cy) = (crate::ram::CAMERA_X.get(x.b), crate::ram::CAMERA_Y.get(x.b));
    x.set16(Obj::X, cx);
    x.set16(Obj::Z, cy);
    let mode = 4 * x.u8(Effect::SETTING) as u32;
    let offsets = if two_players(x) { &tables().purse_offsets_2p } else { &tables().purse_offsets };
    let (dx, dz) = (offsets.at16(x.b, mode), offsets.at16(x.b, mode + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    // 0x1a608 (palette page 1), still on the processor; D7 kept by the original
    crate::families::scenery::palette_page_1(x);
    x.set8(Effect::OWN_PALETTE, 1);
    let owner = x.linked(purse::OWNER);
    let player = owner.u8(x.b, Obj::KIND);
    let palette = 0x18u8.wrapping_add(player);
    x.set8(Effect::PALETTE, palette);
    crate::mem::wr32(x.b, 0x91c54e, 0xf062_f0f6);
    crate::mem::wr32(x.b, 0x91c552, 0xf800_ff33);
    // the original keeps D0 (the palette) across the allocation
    let Some(item) = x.alloc(&pools::ITEM) else { crate::families::effects::address_error(purse::POOL_FULL.0) };
    wr8(x.b, item.0, 1);
    wr8(x.b, item.0 + 2, 9);
    wr8(x.b, item.0 + 3, player);
    wr16(x.b, item.0 + 0x5c, x.o.0 as u16);
    // portrait: 8 longs of the class palette, with the brightness bit
    let dest = 0x91c000u32.wrapping_add(((palette as u16) << 5) as i16 as i32 as u32);
    let class = owner.u8(x.b, Obj::VARIANT) as u32;
    let src = tables().purse_portrait.at16(x.b, 2 * class) as i16 as i32 as u32;
    for k in 0..8 {
        let v = tables().purse_portraits.at32(x.b, src.wrapping_add(4 * k));
        let v = if k < 6 { v | 0xf000_f000 } else { v.wrapping_add(0xf000_f000) };
        wr32(x.b, dest + 4 * k, v);
    }
    let p4 = 4 * player as u32;
    let digits = if two_players(x) { &tables().purse_digits_2p } else { &tables().purse_digits };
    let v = digits.at32(x.b, p4);
    x.set32(purse::DIGITS_AT, v);
    let gauge = if two_players(x) { &tables().purse_gauge_2p } else { &tables().purse_gauge };
    let v = gauge.at32(x.b, p4);
    x.set32(purse::GAUGE_AT, v);
    let Some(peff) = x.alloc(&pools::PEFF) else { crate::families::effects::address_error(purse::POOL_FULL.0) };
    wr8(x.b, peff.0, 1);
    wr8(x.b, peff.0 + 2, 0x18);
    wr16(x.b, peff.0 + 0x5c, x.o.0 as u16);
    let w = x.u16(purse::OWNER);
    wr16(x.b, peff.0 + 0x50, w);
    let (pos, var) = if two_players(x) {
        (&tables().purse_icon_pos_2p, &tables().purse_icon_variant_2p)
    } else {
        (&tables().purse_icon_pos, &tables().purse_icon_variant)
    };
    let Some(icon) = x.alloc(&pools::SOBJ) else { crate::families::effects::address_error(purse::POOL_FULL.0) };
    wr8(x.b, icon.0, 1);
    wr8(x.b, icon.0 + 2, 0);
    let v = var.at8(x.b, player as u32);
    wr8(x.b, icon.0 + 3, v);
    let (px, py) = (pos.at16(x.b, p4).wrapping_add(cx), pos.at16(x.b, p4 + 2).wrapping_add(cy));
    wr16(x.b, icon.0 + Obj::X, px);
    wr16(x.b, icon.0 + Obj::Y, py);
    wr16(x.b, icon.0 + Obj::Z, 0);
    // six PEFF 0x11 (D5: the original's counter, kept by the allocation)
    for n in (0..=5u16).rev() {
        let Some(o) = x.alloc(&pools::PEFF) else { crate::families::effects::address_error(purse::POOL_FULL.0) };
        wr8(x.b, o.0, 1);
        wr8(x.b, o.0 + 2, 0x11);
        wr8(x.b, o.0 + 3, n as u8);
        wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
        wr16(x.b, o.0 + 0x50, w);
    }
    purse_anim(x, false);
}

/// 0xaf2c0 / 0xaf302: closed or open (`open`) cursor animation, according to the player's class.
pub(super) fn purse_anim<R: Bench>(x: &mut Cx<'_, R>, open: bool) {
    let owner = x.linked(purse::OWNER);
    x.set8(purse::OPENED, open as u8);
    let class = owner.u8(x.b, Obj::VARIANT);
    let base: u8 = match (class & 1 != 0, open) {
        (false, false) => 0x17,
        (true, false) => 0x2f,
        (false, true) => 0x18,
        (true, true) => 0x30,
    };
    x.anim_start(purse::ANIM_SET.0, ((class & !1).wrapping_add(base) as u32) as u16);
    x.reinsert();
}

/// Releases the linked object (`+0x5c`): its byte `+0x56` goes to 0xff.
pub(super) fn purse_release_linked<R: Bench>(x: &mut Cx<'_, R>) {
    let linked = x.linked(purse::LINKED);
    x.set16(purse::LINKED, 0);
    linked.set_u8(x.b, Linked::RELEASED, 0xff);
}

pub(super) fn purse_active<R: Bench>(x: &mut Cx<'_, R>) {
    let owner = x.linked(purse::OWNER);
    let player = owner.u8(x.b, Obj::KIND) as u32;
    let moves = A5 + player;
    let buttons = vars::INPUTS.0 + player;
    purse_give(x, owner, buttons);
    purse_draw_gauge(x, owner);
    purse_draw_gold(x, owner);
    if x.u16(purse::LINKED) != 0 {
        if rd8(x.b, moves) & 0x10 == 0 {
            purse_release_linked(x);
            return purse_anim(x, false);
        }
    } else {
        if x.u8(purse::OPENED) != 0 {
            if !x.anim_step() {
                return;
            }
            purse_anim(x, false);
        }
        if rd8(x.b, buttons) & 0x10 != 0 {
            purse_anim(x, true);
        }
    }
    purse_move(x, moves);
}

/// Gold donation: the cursor on another player, button 0x40 (10 coins) or 0x80 (100) pressed.
pub(super) fn purse_give<R: Bench>(x: &mut Cx<'_, R>, owner: Obj, buttons: u32) {
    for k in 0..4 {
        let p = rd16(x.b, shop::BUYERS + 2 * k);
        if p == 0 || p == x.o.0 as u16 {
            continue;
        }
        let other = Obj(p as i16 as i32 as u32 & 0xff_ffff);
        let pressed = rd8(x.b, buttons);
        let amount: i32 = if pressed & 0x40 != 0 {
            0x10
        } else if pressed & 0x80 != 0 {
            0x100
        } else {
            continue;
        };
        let dx = x.u16(Obj::X).wrapping_sub(other.u16(x.b, Obj::X)).wrapping_add(4);
        let dz = x.u16(Obj::Z).wrapping_sub(other.u16(x.b, Obj::Z)).wrapping_add(8);
        if dx > 8 || dz > 0x10 {
            continue;
        }
        let amount = amount.min(owner.u32(x.b, GOLD) as i32);
        x.set32(purse::GIFT, amount as u32);
        let receiver = Obj(other.u16(x.b, Buyer::PLAYER) as i16 as i32 as u32 & 0xff_ffff);
        let mut carry = false;
        for i in (1..4).rev() {
            let (v, c) = crate::ops::abcd(x.u8(purse::GIFT + i), receiver.u8(x.b, GOLD + i), carry);
            receiver.set_u8(x.b, GOLD + i, v);
            carry = c;
        }
        if receiver.u32(x.b, GOLD) as i32 > 0x99_9999 {
            receiver.set_u32(x.b, GOLD, 0x99_9999);
        }
        let mut borrow = false;
        for i in (1..4).rev() {
            let (v, c) = crate::game::sbcd(owner.u8(x.b, GOLD + i), x.u8(purse::GIFT + i), borrow);
            owner.set_u8(x.b, GOLD + i, v);
            borrow = c;
        }
    }
}

/// Player's life gauge (`+0x60` maximum, `+0x62` current): catches up with the current value by one notch
/// per frame, then draws the tiles (8 points each) in the text layer.
pub(super) fn purse_draw_gauge<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    let mut at = x.u32(purse::GAUGE_AT);
    let len = x.u16(purse::GAUGE_LEN);
    let (mut max, mut cur) = (owner.u16(x.b, Player::HP_MAX), owner.u16(x.b, Player::HP));
    if two_players(x) {
        max >>= 1;
        cur >>= 1;
    }
    if len == cur {
        return;
    }
    let len = if (len as i16) > (cur as i16) { len.wrapping_sub(1) } else { len.wrapping_add(1) };
    x.set16(purse::GAUGE_LEN, len);
    let mut left = len;
    loop {
        // subq.w #8 ; ble
        let last = (max as i16) <= 8;
        max = max.wrapping_sub(8);
        if last {
            break;
        }
        left = left.wrapping_sub(8);
        let tile = if (left as i16) < 0 {
            let t = left.wrapping_add(0x1318);
            left = 0;
            t
        } else {
            0x1318
        };
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0xa);
        at = at.wrapping_add(0x80);
    }
    // `max` is now between -7 and 0: the tiles of the gauge end are read backwards from the last one (0xaefee)
    let end = 0xe_u32.wrapping_add(max.wrapping_mul(2) as i16 as i32 as u32);
    let tile = tables().purse_gauge_end.at16(x.b, end).wrapping_add(left);
    wr16(x.b, at, tile);
    wr16(x.b, at + 2, 0xa);
}

/// Player's gold in six digits (leading zeros cleared), then three fixed tiles.
pub(super) fn purse_draw_gold<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    let mut gold = owner.u32(x.b, GOLD).rotate_left(8);
    let mut at = x.u32(purse::DIGITS_AT);
    let mut shown = false;
    for _ in 0..5 {
        gold = gold.rotate_left(4);
        let digit = gold as u16 & 0xf;
        let tile = if digit != 0 || shown {
            shown = true;
            digit + 0x310
        } else {
            0x10
        };
        wr16(x.b, at, tile);
        wr16(x.b, at + 2, 0);
        at = at.wrapping_add(0x80);
    }
    gold = gold.rotate_left(4);
    wr16(x.b, at, (gold as u16 & 0xf) + 0x310);
    wr16(x.b, at + 2, 0);
    wr32(x.b, at + 0x80, 0x10_0000);
    wr32(x.b, at + 0x100, 0x36c_0000);
    wr32(x.b, at + 0x180, 0x369_0000);
}

/// Movement by the player's directions (bit 5: fast), clamped to the screen.
pub(super) fn purse_move<R: Bench>(x: &mut Cx<'_, R>, moves: u32) {
    let m = rd8(x.b, moves);
    let step: u16 = if m & 0x20 != 0 { 5 } else { 2 };
    if m & 8 != 0 {
        x.add16(Obj::Z, step);
    }
    if m & 4 != 0 {
        x.add16(Obj::Z, step.wrapping_neg());
    }
    if m & 2 != 0 {
        x.add16(Obj::X, step.wrapping_neg());
    }
    if m & 1 != 0 {
        x.add16(Obj::X, step);
    }
    let clamp = |x: &mut Cx<'_, R>, off: u32, origin: u16, span: u16| {
        let v = x.u16(off) as i16;
        let (low, high) = (origin.wrapping_add(0x14), origin.wrapping_add(span));
        if low as i16 > v {
            x.set16(off, low);
        } else if (high as i16) < v {
            x.set16(off, high);
        }
    };
    let (cx, cy) = (crate::ram::CAMERA_X.get(x.b), crate::ram::CAMERA_Y.get(x.b));
    clamp(x, Obj::X, cx, 0x16c);
    clamp(x, Obj::Z, cy, 0x100);
}

/// Exit: the cursor closes then glides to the right off the screen.
pub(super) fn purse_leave<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB_STATE) == 0 {
        purse_anim(x, false);
        x.add8(f::SUB_STATE, 1);
        if x.u16(purse::LINKED) != 0 {
            purse_release_linked(x);
        }
    }
    x.add16(Obj::X, 4);
    let cx = crate::ram::CAMERA_X.get(x.b);
    if (x.u16(Obj::X).wrapping_sub(0x1a0) as i16) <= cx as i16 {
        return;
    }
    let n = rd8(x.b, purse::OPEN).wrapping_sub(1);
    wr8(x.b, purse::OPEN, if (n as i8) < 0 { 0 } else { n });
    free(x);
}

// ---------------------------------------------------------------------------------------------
// Type 70: counter of a scene shop (same sales zone as type 22)
// ---------------------------------------------------------------------------------------------

pub(super) mod counter {
    /// The counter that holds the sale in progress (`A5-0x5542`).
    pub const ACTIVE: u32 = 0xff2abe;
    /// Scene scenery 0x64 (the vendor) created at purchase, kept in `+0x1a`.
    pub const SELLER: u32 = 0x1a;
    pub const TIMER: u32 = 0x18;
    pub const STEP: u32 = 0x06;
    /// Sale message, sign tiles (20 longs in background 1), lists of objects to place (per
    /// variant: longs toward 8-byte sheets, negative byte at the end).
    pub const SALE_MESSAGE: u16 = 0x8e;
    pub const SIGN: u32 = 0x912ad4;
    pub const SIGN_TILES: [(u32, u32); 20] = [
        (0, 0x1b780005), (4, 0x1b880005), (0x80, 0x1b790005), (0x84, 0x1b890005),
        (0x100, 0x1b9a0005), (0x104, 0x1baa0005), (0x180, 0x1b9b0005), (0x184, 0x1bab0005),
        (0x200, 0x1bb00005), (0x204, 0x1bc00005), (0x280, 0x1bb10005), (0x284, 0x1bc10005),
        (0x300, 0x1bb80005), (0x304, 0x1bc80005), (0x380, 0x1bb90005), (0x384, 0x1bc90005),
        (0x400, 0x1b7c0005), (0x404, 0x1b8c0005), (0x480, 0x1b7d0005), (0x484, 0x1b8d0005),
    ];
}

/// `0xb83d8`: first buyer in the zone: the counter takes the sale (if there is no other) and
/// brings in the vendor (scenery 0x64); A1 and D6 kept like the original.
pub(super) fn counter_buy<R: Bench>(x: &mut Cx<'_, R>, _buyer: Obj, _slot: u32) {
    let active = rd16(x.b, counter::ACTIVE);
    if active != 0 && active != x.o.0 as u16 {
        return;
    }
    crate::mem::wr16(x.b, counter::ACTIVE, x.o.0 as u16);
    let Some(seller) = scene_set_of(x, 0x64) else {
        return crate::mem::wr16(x.b, counter::ACTIVE, 0);
    };
    x.set16(counter::SELLER, seller.0 as u16);
    x.add8(f::SUB_STATE, 2);
    x.set16(counter::TIMER, 0x18);
}

/// `0xb81d6`: the sale. After a delay, the sign is displayed, the message is announced and the items are
/// placed (PEFF 0x2a); then it waits for a buyer in front of the vendor, who speeds the vendor up; finally
/// the sale ends.
pub(super) fn counter_sale<R: Bench>(x: &mut Cx<'_, R>) {
    let step = x.u8(counter::STEP) as i8;
    if step > 2 {
        if countdown_word(x, counter::TIMER) {
            x.set8(f::SUB_STATE, 0);
            x.set8(counter::STEP, 0);
            crate::mem::wr16(x.b, counter::ACTIVE, 0);
        }
        return;
    }
    if step == 2 {
        for k in 0..4u32 {
            let p = rd16(x.b, shop::BUYERS + 2 * k);
            if p == 0 {
                continue;
            }
            let buyer = Obj(p as i16 as i32 as u32 & 0xff_ffff);
            if buyer.u8(x.b, Buyer::PURCHASE) as i8 <= 0 {
                continue;
            }
            let across = buyer.u16(x.b, Obj::X).wrapping_sub(crate::ram::CAMERA_X.get(x.b)).wrapping_sub(0xa8);
            if across > 0x50 {
                continue;
            }
            let depth = buyer.u16(x.b, Obj::Z).wrapping_add(buyer.u16(x.b, Obj::Y)).wrapping_sub(crate::ram::CAMERA_Y.get(x.b)).wrapping_sub(0x48);
            if depth <= 0x10 {
                let seller = x.linked(counter::SELLER);
                if seller.u16(x.b, VoteCreator::DONE) > 1 {
                    seller.set_u16(x.b, VoteCreator::DONE, 1);
                }
                x.add8(counter::STEP, 2);
                x.set16(counter::TIMER, 0x18);
                return;
            }
        }
        return;
    }
    if !countdown_word(x, counter::TIMER) {
        return;
    }
    x.add8(counter::STEP, 2);
    crate::mem::wr16(x.b, shop::MESSAGE, counter::SALE_MESSAGE);
    for (off, tile) in counter::SIGN_TILES {
        wr32(x.b, counter::SIGN + off, tile);
    }
    let v = x.u8(f::VARIANT) as u32;
    // the goods lists, designated by their address
    let goods = &tables().counter_goods_lists;
    let mut at = tables().counter_goods.at32(x.b, 4 * v).wrapping_sub(goods.rom);
    loop {
        let setting = goods.at8(x.b, at);
        at += 1;
        if (setting as i8) < 0 {
            return;
        }
        let Some(o) = x.alloc(&pools::PEFF) else { return };
        wr8(x.b, o.0, 1);
        wr8(x.b, o.0 + 2, 0x2a);
        wr16(x.b, o.0 + 0x5c, x.o.0 as u16);
        wr8(x.b, o.0 + 0x2f, setting);
        let variant = goods.at8(x.b, at);
        wr8(x.b, o.0 + 3, variant);
        let px = crate::ram::CAMERA_X.get(x.b).wrapping_add(goods.at16(x.b, at + 1));
        wr16(x.b, o.0 + Obj::X, px);
        let (z, y) = (goods.at16(x.b, at + 3), goods.at16(x.b, at + 5));
        wr16(x.b, o.0 + Obj::Z, z);
        wr16(x.b, o.0 + Obj::Y, y);
        at += 7;
    }
}

/// `sub.w #1,(off,A0) ; bpl`: true when the countdown goes below zero.
pub(super) fn countdown_word<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    let t = x.u16(off).wrapping_sub(1);
    x.set16(off, t);
    (t as i16) < 0
}

pub(super) fn counter<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        0 => {
            x.add8(f::STATE, 2);
            for (k, off) in [0x14, 0x16, 0x54, 0x56].into_iter().enumerate() {
                let w = tables().counter_zone.at16(x.b, 2 * k as u32);
                x.set16(off, w);
            }
            let v = x.u8(f::VARIANT) as u32;
            let message = tables().counter_messages.at16(x.b, 2 * v);
            x.set16(shop::MESSAGE_ID, message);
            let anim = tables().counter_animations.at16(x.b, 2 * v) as u32;
            x.anim_start(shop::ANIM_SET.0, anim as u16);
            x.reinsert();
        }
        2 if x.u8(f::SUB_STATE) != 0 => counter_sale(x),
        2 => shop_watch(x, counter_buy),
        _ => free(x),
    }
}

