//! Shop: the second counter (PEFF 42, 0x50b9c) and the merchant's dialogue interpreter (0x514ac / 0x514cc) that PEFF
//! counter 43 (`peff.rs`) starts. See `docs/families/joueur.md`.
//!
//! The customer is object A2 read from the list of the four players (`0xff2aa4`); its positive `+0x58` means "it is
//! addressing the merchant" (reset to -1 as soon as it is served), its `+0x50` designates the player's record: BCD
//! purse `+0x112..+0x117` (long `+0x114` compared with the prices), six inventory slots `+0x140` (0x56: empty) and
//! their quantities `+0x148`, shop dialogue state `+0x167`.

use super::vars;
use crate::families::player::records::Shot;
use crate::player::Player;
use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::Current;
use crate::player::tables::{player_data, Aligned};

/// Merchant message to display (word, A5 - 0x554e) and the two shop sounds (D0 unused: the message is read from RAM).
const MESSAGE: u32 = A5 - 0x554e;
const SOUND_OK: crate::sound::Sound = crate::sound::CMD_135_235;
const SOUND_MESSAGE: crate::sound::Sound = crate::sound::CMD_136_236;
/// List of the four customers (address words, 0 = absent).
const CUSTOMERS: u32 = 0xff2aa4;

/// Fields of the customer and of its player.
mod c {
    pub const READY: u32 = 0x58;
    pub const PLAYER: u32 = 0x50;
}
mod p {
    pub const CHARACTER: u32 = 0x3;
    pub const STATE_10D: u32 = 0x10d;
    pub const GOLD: u32 = 0x114;
    pub const GOLD_END: u32 = 0x118;
    pub const SLOTS: u32 = 0x140;
    pub const COUNTS: u32 = 0x148;
    pub const SHOP: u32 = 0x167;
}
const SLOTS: u32 = 6;
const EMPTY: u8 = 0x56;

/// Customer's player (`movea.w (0x50,A2),A6`).
fn player_of<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) -> Obj {
    let w = customer.u16(x.b, c::PLAYER);
    Obj(w as i16 as i32 as u32 & 0xff_ffff)
}

/// First inventory slot that contains `item`.
fn find_slot<R: Bench>(x: &mut Cx<'_, R>, player: Obj, item: u8) -> Option<u32> {
    (0..SLOTS).find(|&k| player.u8(x.b, p::SLOTS + k) == item)
}

/// Sets the message and plays the message sound.
fn say<R: Bench>(x: &mut Cx<'_, R>, message: u16) {
    wr16(x.b, MESSAGE, message);
    crate::sound::play(x.b, SOUND_MESSAGE, x.o);
}

// ---------------------------------------------------------------------------------------------
// Type 42 (0x50b9c) : article en vitrine

/// Type 42: an article placed on the counter (variant `+3`: 0 to 11). It appears when its parent counter (`+0x5c`) is
/// in state 2 (and, for variants 0 and 7, when the shop offers it: bytes `A5-0x553f` / `A5-0x5540`), follows the
/// scroll, and watches for customers in its zone (`+0x14..+0x16` in X, `+0x54..+0x56` in depth): it displays its
/// message (`+0x5a`) and sells to the customer who addresses the merchant. BCD price `+0x4e..+0x53` (long `+0x50`),
/// article `+0x59`, slot of the purchase table `+0x58`, stock `+0x2f` (negative: sold out).
mod item {
    use super::*;

    const STATES: u32 = 0x50baa;
    const PARENT: u32 = 0x5c;
    const ZONE: u32 = 0x14;
    const DEPTH: u32 = 0x54;
    const SHOWN: u32 = 0x1a;
    const NEAR: u32 = 0x1b;
    const STOCK: u32 = 0x2f;
    const PRICE: u32 = 0x50;
    const PRICE_END: u32 = 0x54;
    const PURCHASE_INDEX: u32 = 0x58;
    const ITEM: u32 = 0x59;
    const OWN_MESSAGE: u32 = 0x5a;
    const ANIMS: crate::data::RomAddr = crate::data::RomAddr(0x1980e4);
    const ANIMS_ALT: crate::data::RomAddr = crate::data::RomAddr(0x1980fc);
    /// Variants 0 and 7: offered only if the shop has them (bytes in A5).
    const OFFER_0: u32 = A5 - 0x553f;
    const OFFER_7: u32 = A5 - 0x5540;
    /// Purchases made in the game (one byte per article).
    const PURCHASED: u32 = A5 - 0x5b88;
    const MAX_COUNT: i8 = 9;

    pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
        match x.state(STATES) {
            0 => init(x),
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn parent_ready<R: Bench>(x: &mut Cx<'_, R>) -> bool {
        let parent = x.linked(PARENT);
        parent.u8(x.b, Shot::STEP) == 2
    }

    fn follow_scroll<R: Bench>(x: &mut Cx<'_, R>) {
        let d = vars::CAM_X_LONG.get(x.b).wrapping_sub(vars::PREV_CAM_X.get(x.b));
        x.add32(Obj::X, d);
    }

    fn init<R: Bench>(x: &mut Cx<'_, R>) {
        if !parent_ready(x) {
            return x.free_peff();
        }
        let variant = x.u8(Obj::VARIANT);
        let offered = match variant {
            0 => rd8(x.b, OFFER_0) != 0,
            7 => rd8(x.b, OFFER_7) != 0,
            _ => true,
        };
        if !offered {
            return follow_scroll(x);
        }
        x.add8(Obj::STATE, 2);
        for (k, off) in [ZONE, ZONE + 2, DEPTH, DEPTH + 2].into_iter().enumerate() {
            let v = player_data().item_zone_sizes.word_at(x.b, 2 * k as u32);
            x.set16(off, v);
        }
        if x.u8(STOCK) != 0 {
            let v = x.u8(STOCK).wrapping_add(vars::PLAYERS_ALIVE.get(x.b));
            x.set8(STOCK, v);
        }
        let v = variant as u32;
        let price = player_data().item_prices.long_at(x.b, 4 * v);
        x.set32(PRICE, price);
        let message = player_data().item_messages.word_at(x.b, 2 * v);
        x.set16(OWN_MESSAGE, message);
        let purchase = player_data().item_purchases.word_at(x.b, 2 * v);
        x.set16(PURCHASE_INDEX, purchase);
        let anim = player_data().item_animations.word_at(x.b, 2 * v);
        let set = if vars::ALT_PALETTE.get(x.b) != 0 { ANIMS_ALT.0 } else { ANIMS.0 };
        x.anim_start(set, anim);
        x.reinsert();
    }

    fn active<R: Bench>(x: &mut Cx<'_, R>) {
        follow_scroll(x);
        if !parent_ready(x) {
            return x.free_peff();
        }
        x.set8(NEAR, 0);
        for k in 0..4 {
            let w = rd16(x.b, CUSTOMERS + 2 * k);
            if w == 0 {
                continue;
            }
            let customer = Obj(w as i16 as i32 as u32 & 0xff_ffff);
            if in_zone(x, customer) {
                x.set8(NEAR, 1);
                if x.u8(SHOWN) == 0 && x.u16(OWN_MESSAGE) != 0 {
                    let m = x.u16(OWN_MESSAGE);
                    wr16(x.b, MESSAGE, m);
                    x.set8(SHOWN, 1);
                }
                if (customer.u8(x.b, c::READY) as i8) > 0 {
                    sell(x, customer);
                }
            }
        }
        if x.u8(NEAR) == 0 {
            x.set8(SHOWN, 0);
        }
    }

    /// Is the customer in the sales zone?
    fn in_zone<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) -> bool {
        let dx = x.u16(Obj::X).wrapping_sub(customer.u16(x.b, Obj::X)).wrapping_add(x.u16(ZONE));
        if dx > x.u16(ZONE + 2) {
            return false;
        }
        let dz = x
            .u16(Obj::Z)
            .wrapping_add(x.u16(Obj::Y))
            .wrapping_sub(customer.u16(x.b, Obj::Z))
            .wrapping_sub(customer.u16(x.b, Obj::Y))
            .wrapping_add(x.u16(DEPTH));
        dz <= x.u16(DEPTH + 2)
    }

    /// The customer addresses the merchant (0x50d9c): depending on the variant, the article is refused (message 0x86:
    /// already equipped; 0x84: not for this character) or sold.
    fn sell<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) {
        customer.set_u8(x.b, c::READY, 0xff);
        let variant = x.u8(Obj::VARIANT);
        let player = player_of(x, customer);
        let state = player.u8(x.b, p::STATE_10D);
        let character = player.u8(x.b, p::CHARACTER);
        // D0 = 4 * variant, low byte replaced by the character, then D0.w >> 1: 4 * variant < 0x100 for the variants
        // that use it (0 to 11), the "class" is the character divided by two
        let class = character >> 1;
        // equipment (variants 0 and 5 to 11): refused if already worn or forbidden to the character
        let equipment = match variant {
            0 => Some((Some(8), class != 0)),
            5 => Some((Some(9), class == 1 || class == 4)),
            6 => Some((Some(0x1d), class == 4)),
            7 => Some((Some(0x1e), class == 4)),
            8..=11 => Some((None, class != 4)),
            _ => None,
        };
        match equipment {
            Some((worn, forbidden)) => {
                if worn == Some(state) {
                    return say(x, 0x86);
                }
                if forbidden {
                    return say(x, 0x84);
                }
                buy(x, player, false);
            }
            None => buy(x, player, true),
        }
    }

    /// Purchase (0x50f2a equipment, 0x50f94 stackable consumable): stock, purse, free slot, then payment.
    fn buy<R: Bench>(x: &mut Cx<'_, R>, player: Obj, stackable: bool) {
        if (x.u8(STOCK) as i8) < 0 {
            return crate::sound::play(x.b, SOUND_MESSAGE, x.o);
        }
        let gold = player.u32(x.b, p::GOLD);
        if (gold as i32) < (x.u32(PRICE) as i32) {
            return say(x, 0x82);
        }
        let item = x.u8(ITEM);
        let slot = match find_slot(x, player, item) {
            Some(_) if !stackable => return say(x, 0x86),
            Some(k) => k,
            None => match find_slot(x, player, EMPTY) {
                Some(k) => {
                    player.set_u8(x.b, p::COUNTS + k, 0);
                    k
                }
                None => return say(x, 0x87),
            },
        };
        let count = player.u8(x.b, p::COUNTS + slot);
        if (count as i8) >= MAX_COUNT {
            return say(x, 0x86);
        }
        player.set_u8(x.b, p::SLOTS + slot, item);
        player.set_u8(x.b, p::COUNTS + slot, count.wrapping_add(1));
        let index = x.u8(PURCHASE_INDEX) as u32;
        wr8(x.b, PURCHASED + index, 1);
        let stock = x.u8(STOCK).wrapping_sub(1);
        x.set8(STOCK, stock);
        if (stock as i8) < 0 {
            x.anim_step();
            x.anim_step();
        }
        pay(x, player);
        crate::sound::play(x.b, SOUND_OK, x.o);
    }

    /// The player's purse (six BCD bytes `+0x112..+0x117`) minus the price (`+0x4e..+0x53`), by `sbcd`.
    fn pay<R: Bench>(x: &mut Cx<'_, R>, player: Obj) {
        let mut borrow = false;
        for k in 1..=6 {
            let price = x.u8(PRICE_END - k);
            let gold = player.u8(x.b, p::GOLD_END - k);
            let (v, b) = crate::game::sbcd(gold, price, borrow);
            borrow = b;
            player.set_u8(x.b, p::GOLD_END - k, v);
        }
        // Original glitch: the exhausted `dbf` counter remains in D5 (`dispatch::Leftovers`)
        *x.c.d5() = *x.c.d5() & 0xffff_0000 | 0xffff;
    }
}

// ---------------------------------------------------------------------------------------------
// Merchant dialogue (0x514ac, 0x514cc)

/// Interpreter of a counter dialogue script (PEFF 43): `field` = `+0x1c` (zone A) or `+0x58` (zone B), script pointer,
/// which the interpreter advances and stores. A command = opcode (`+5`), parameter (`+6`), word D1 (message or relative
/// jump). The player's dialogue state (`+0x167`) is set by the shop menu.
mod talk {
    use super::*;

    /// Commands (opcode = offset in the jump table 0x514ec).
    const SAY: u8 = 0x00;
    const STOP: u8 = 0x04;
    const SAY_NEXT_UNLESS: u8 = 0x08;
    const OPEN_MENU: u8 = 0x0c;
    const JUMP: u8 = 0x10;
    const SELL_CONFIRMED: u8 = 0x14;
    const CANCELLED: u8 = 0x18;
    const LEARN: u8 = 0x1c;
    const JUMP_AND_STOP: u8 = 0x20;
    const GIVE_WEAPON: u8 = 0x24;
    const GIVE_ITEM_2B: u8 = 0x28;
    const FREE: u8 = 0x2c;

    const MENU_FLAG: u32 = A5 - 0x5b48;
    const SOLD_FLAG: u32 = A5 - 0x5b22;
    const LEARNED_FLAG: u32 = A5 - 0x5b6d;
    const ITEM_2B_FLAG: u32 = A5 - 0x5b60;
    const IDLE: u8 = 0xff;

    pub fn zone_a<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) {
        run(x, customer, 0x1c);
    }

    pub fn zone_b<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) {
        run(x, customer, 0x58);
    }

    /// Script reading: position (A3 of the original), counter field that stores it (A4), word of the current command
    /// (D1.w: message or relative jump).
    struct Script {
        pos: u32,
        field: u32,
        word: u16,
    }

    /// Next command: `+5`, `+6`, word; position advanced.
    fn fetch<R: Bench>(x: &mut Cx<'_, R>, s: &mut Script) -> u8 {
        let (op, param) = (rd8(x.b, s.pos), rd8(x.b, s.pos + 1));
        x.set8(Shot::SUB_STATE, op);
        x.set8(Shot::STEP, param);
        s.word = rd16(x.b, s.pos + 2);
        s.pos += 4;
        op
    }

    /// Relative jump of the word (`lea (0,A3,D1.w),A3`).
    fn skip(s: &mut Script) {
        s.pos = s.pos.wrapping_add(s.word as i16 as i32 as u32);
    }

    /// Stores the script position (`move.l A3,(A4)`).
    fn save<R: Bench>(x: &mut Cx<'_, R>, s: &Script) {
        let field = s.field;
        x.set32(field, s.pos);
    }

    /// The customer enters the menu (player state 2, substate 5): sound, menu open.
    fn open_menu<R: Bench>(x: &mut Cx<'_, R>, s: &mut Script, player: Obj) {
        crate::sound::play(x.b, SOUND_OK, x.o);
        player.set_u8(x.b, Player::SPEED_CLASS, 2);
        player.set_u8(x.b, Player::SHOP_VISIT, 5);
        player.set_u8(x.b, p::SHOP, IDLE);
        wr8(x.b, MENU_FLAG, 1);
        skip(s);
    }

    fn run<R: Bench>(x: &mut Cx<'_, R>, customer: Obj, field: u32) {
        customer.set_u8(x.b, c::READY, 0xff);
        let pos = x.u32(field);
        let s = &mut Script { pos, field, word: 0 };
        let mut op = fetch(x, s);
        loop {
            // the original indexes its table by D0.w, whose high byte comes from the caller: the counter leaves there a
            // depth offset at most equal to the extent of the zone, below 0x100 (0x51306, 0x51376), hence zero: only
            // the command byte counts
            match op {
                SAY => {
                    if rd16(x.b, MESSAGE) == 0 {
                        wr16(x.b, MESSAGE, s.word);
                        save(x, s);
                    }
                    return;
                }
                STOP => return,
                SAY_NEXT_UNLESS => {
                    if vars::SECOND_PASS.get(x.b) != 0 {
                        skip(s);
                    }
                    fetch(x, s);
                    op = SAY;
                    continue;
                }
                OPEN_MENU => {
                    let waiting = x.u8(Shot::STEP) != 0;
                    if waiting {
                        save(x, s);
                    }
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) != 0 {
                        if waiting {
                            return;
                        }
                    } else {
                        open_menu(x, s, player);
                    }
                }
                JUMP => skip(s),
                SELL_CONFIRMED => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 1 {
                        crate::sound::play(x.b, SOUND_OK, x.o);
                        player.set_u8(x.b, p::SHOP, 0xa);
                        wr8(x.b, SOLD_FLAG, 1);
                        skip(s);
                    }
                }
                CANCELLED => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 4 {
                        player.set_u8(x.b, p::SHOP, IDLE);
                        skip(s);
                    }
                }
                LEARN => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 3 {
                        let character = player.u8(x.b, p::CHARACTER);
                        // D0.w = character: the high byte is that of the depth offset the counter leaves there, zero
                        // (extents of 0x51306 / 0x51376 all below 0x100)
                        if character < 8 {
                            player.set_u8(x.b, p::SHOP, IDLE);
                            player.set_u8(x.b, Player::WORN_4, 4);
                            player.set_u8(x.b, Player::WEAPON_B, 0x2c);
                            wr8(x.b, LEARNED_FLAG, 1);
                            crate::sound::play(x.b, SOUND_OK, x.o);
                        } else {
                            player.set_u8(x.b, p::SHOP, 0xa);
                            wr8(x.b, SOLD_FLAG, 1);
                        }
                        wr16(x.b, MESSAGE, 0xeb);
                        save(x, s);
                        return;
                    }
                }
                JUMP_AND_STOP => {
                    skip(s);
                    save(x, s);
                    return;
                }
                GIVE_WEAPON => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 8 && give_weapon(x, s, player) {
                            return;
                        }
                }
                GIVE_ITEM_2B => {
                    let player = player_of(x, customer);
                    if player.u8(x.b, p::SHOP) == 5 && give_item_2b(x, s, player) {
                            return;
                        }
                }
                FREE => return x.free_peff(),
                _ => unreachable!("commande de dialogue {op:#x}"),
            }
            op = fetch(x, s);
        }
    }

    /// Weapon offered according to the class (0x5164c) in the first empty slot; with no room, message 0x87 and the
    /// reading continues. True when the dialogue stops.
    fn give_weapon<R: Bench>(x: &mut Cx<'_, R>, s: &Script, player: Obj) -> bool {
        let Some(k) = find_slot(x, player, EMPTY) else {
            wr16(x.b, MESSAGE, 0x87);
            crate::sound::play(x.b, SOUND_MESSAGE, x.o);
            return false;
        };
        player.set_u8(x.b, p::SHOP, IDLE);
        crate::sound::play(x.b, SOUND_OK, x.o);
        let character = player.u8(x.b, p::CHARACTER);
        // D0.w = character (high byte zero, see `LEARN`), divided by two
        let class = character >> 1;
        let (item, flag) = match class {
            4 => (0x4e, A5 - 0x5b76),
            1 => (0x51, A5 - 0x5b73),
            _ => (0x39, A5 - 0x5b84),
        };
        player.set_u8(x.b, p::SLOTS + k, item);
        player.set_u8(x.b, p::COUNTS + k, 0);
        wr8(x.b, flag, 1);
        wr16(x.b, MESSAGE, 0xea);
        save(x, s);
        true
    }

    /// Item 0x2b offered (0x51706): already owned, the dialogue stops on a sound; otherwise in the first empty slot
    /// (message 0xfa), and with no room, message 0x87 and the reading continues.
    fn give_item_2b<R: Bench>(x: &mut Cx<'_, R>, s: &Script, player: Obj) -> bool {
        const ITEM: u8 = 0x2b;
        if find_slot(x, player, ITEM).is_some() {
            save(x, s);
            player.set_u8(x.b, p::SHOP, IDLE);
            crate::sound::play(x.b, SOUND_OK, x.o);
            return true;
        }
        let Some(k) = find_slot(x, player, EMPTY) else {
            wr16(x.b, MESSAGE, 0x87);
            crate::sound::play(x.b, SOUND_MESSAGE, x.o);
            return false;
        };
        player.set_u8(x.b, p::SHOP, IDLE);
        crate::sound::play(x.b, SOUND_OK, x.o);
        player.set_u8(x.b, p::SLOTS + k, ITEM);
        player.set_u8(x.b, p::COUNTS + k, 1);
        wr8(x.b, ITEM_2B_FLAG, 1);
        wr16(x.b, MESSAGE, 0xfa);
        save(x, s);
        true
    }
}

handlers! {
    0x50b9c "peff_42" item::run,
}

/// Dialogue interpreters (0x514ac, 0x514cc): called by the counter (PEFF 43, `peff.rs`) with x.o = the counter,
/// `customer` = the customer (A2 of the original).
pub fn talk_a<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) {
    talk::zone_a(x, customer);
}
pub fn talk_b<R: Bench>(x: &mut Cx<'_, R>, customer: Obj) {
    talk::zone_b(x, customer);
}
