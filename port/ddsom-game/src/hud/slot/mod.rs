//! State machine of a player slot (see the `hud` module).

use crate::credits::CoinSlot;
use crate::screens::tables::Rom;
use cps2::Cps2Bus;

use super::{choice, draw, panel, player, select, var, Slot};
use crate::game::sbcd;
use crate::player::init::Join;
use crate::scene::Ctx;

mod play;
mod choose;
use play::*;
use choose::*;
pub use choose::resume_end_enter;

/// Original routines called by the interface (accounting, sounds, player start...).
mod rom {
    /// Player class according to their character (byte, code tables).
    use crate::screens::tables::Rom;
    pub const CLASS_OF_CHARACTER: Rom = Rom::at(0x158c0);
    pub const CLASS_OF_CHARACTER_ALT: Rom = Rom::at(0x15e18);
}

/// Pointers to the banner texts (pointer tables in the code).
mod banners {
    /// "PUSH START" outside continue (two languages, wide then narrow).
    use crate::screens::tables::Rom;
    pub const IDLE: Rom = Rom::at(0x156ca);
    /// Continue: "PUSH START".
    pub const IDLE_CONTINUE: Rom = Rom::at(0x156da);
    /// Pause banner (also when an item is present).
    pub const NOTICE: Rom = Rom::at(0x156ea);
    /// "INSERT COIN": a single credit or price of 1 coin, missing coins, more than one coin.
    pub const INSERT_ONE: Rom = Rom::at(0x15662);
    pub const INSERT: Rom = Rom::at(0x15672);
    pub const INSERT_COUNT: Rom = Rom::at(0x15682);
    pub const INSERT_COUNT_NARROW: Rom = Rom::at(0x156a6);
}

/// Initial countdown of the choice (BCD 19, sub-counter 0x32) and of the continue (BCD 19, 0x3c).
const CHOOSE_COUNTDOWN: u16 = 0x1932;
const RESUME_COUNTDOWN: u16 = 0x193c;
const RESUME_END_FRAMES: u16 = 0x78;

/// 0x1507a: forgets what is displayed in each panel (the player and the mode will be redrawn).
pub fn forget_displayed(b: &mut Cps2Bus) {
    for k in 0..4 {
        let s = Slot::new(k);
        s.set_p8(b, panel::PLAYER_DIRTY, 0xff);
        s.set_p8(b, panel::LAST_MODE, 0xff);
    }
}

/// Coin counter of a slot according to the coin slot mode: own, shared, or one for two
/// players.
fn coin_record(b: &mut Cps2Bus, s: Slot) -> CoinSlot {
    match var::COIN_MODE.get(b) {
        2 | 6 | 11 => s.coins,
        9 => CoinSlot::nth(if s.index(b) & 2 != 0 { 1 } else { 0 }),
        _ => CoinSlot::nth(0),
    }
}

/// 0x15216: one turn of a slot.
pub fn step(x: &mut Ctx, s: Slot) {
    let frames = s.p32(x.b, panel::FRAMES).wrapping_add(1);
    s.set_p32(x.b, panel::FRAMES, frames);
    let mode = var::DISPLAY.get(x.b);
    if mode != s.p8(x.b, panel::LAST_MODE) {
        s.set_p8(x.b, panel::LAST_MODE, mode);
        mode_change(x.b, s);
    }
    let s = Slot { coins: coin_record(x.b, s), ..s };
    match s.p8(x.b, panel::STATE) {
        0 => enter(x.b, s),
        2 => idle(x, s),
        4 => play(x, s),
        6 => choose(x, s),
        8 => resume(x, s),
        0xa => resume_end(x.b, s),
        0xc => resume_choose(x, s),
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Display mode change (0x178ee)
// ---------------------------------------------------------------------------------------------

/// Player palette in the waiting room page (indexed by their number and their character).
const LOBBY_PALETTES: Rom = Rom::at(0x179a2);

fn mode_change(b: &mut Cps2Bus, s: Slot) {
    draw::clear_banner(b, s);
    if s.g8(b, player::ACTIVE) == 0 {
        return;
    }
    match s.p8(b, panel::LAST_MODE) {
        2 => {
            s.set_p8(b, panel::HINT, 0);
            if s.g32(b, player::ITEMS) == !0 && s.g32(b, player::ITEMS + 4) == !0 {
                draw::clear_items_row(b, s);
                for (source, dest) in &draw::CHARACTER_PALETTES[..2] {
                    draw::blend_for(b, s, source.addr(), dest.0);
                }
                draw::portrait(b, s, true);
                let character = s.g8(b, player::CHARACTER);
                draw::character_name(b, s, character);
                let class = s.g8(b, player::CLASS);
                draw::face(b, s, class);
                let number = s.g8(b, player::NUMBER) as u32;
                let base = LOBBY_PALETTES.pc_word(b, 2 * number);
                draw::lobby_palette(b, s, base);
            } else {
                s.set_p8(b, panel::HINT, 1);
                notice_banner(b, s);
            }
        }
        3 => {
            s.set_p8(b, panel::FLAG_27, 0xff);
            draw::portrait(b, s, false);
        }
        4 => {
            if (s.g8(b, player::STATE) as i8) < 4 {
                draw::clear_frame(b, s);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// State 0: initialization
// ---------------------------------------------------------------------------------------------

fn enter(b: &mut Cps2Bus, s: Slot) {
    let state = s.p8(b, panel::STATE).wrapping_add(2);
    s.set_p8(b, panel::STATE, state);
    s.set_p8(b, panel::STATE + 1, 0);
    init_shown(b, s);
}

/// 0x152a8: the displayed values are those of the player; everything will be redrawn.
fn init_shown(b: &mut Cps2Bus, s: Slot) {
    let hp = s.g16(b, player::HP);
    s.set_p16(b, panel::SHOWN_HP, hp);
    let hp_max = s.g16(b, player::HP_MAX);
    s.set_p16(b, panel::SHOWN_HP_MAX, hp_max);
    let score = s.g32(b, player::SCORE);
    s.set_p32(b, panel::SHOWN_SCORE, score);
    let money = s.g32(b, player::MONEY);
    s.set_p32(b, panel::SHOWN_MONEY, money);
    let active = s.g8(b, player::ACTIVE);
    s.set_p8(b, panel::HP_DIRTY, active);
    if active != 0 {
        draw::character_palettes(b, s);
    }
    s.set_p8(b, panel::SCORE_DIRTY, 0xff);
    s.set_p8(b, panel::MONEY_DIRTY, 0xff);
    s.set_p8(b, panel::PLAYER_DIRTY, active);
    s.set_p8(b, panel::DISPLAYED_A, 0xff);
    s.set_p8(b, panel::DISPLAYED_B, 0xff);
    s.set_p8(b, panel::FLAG_2A, 0);
    s.set_p8(b, panel::DELAY_2E, 0);
    s.set_p8(b, 0x30, 0);
    s.set_p8(b, panel::FLAG_27, 0xff);
}

// ---------------------------------------------------------------------------------------------
// State 2: waiting for a player
// ---------------------------------------------------------------------------------------------

/// Price in credits of a game (0 in continue mode).
fn price(b: &mut Cps2Bus) -> u8 {
    if var::CONTINUE_MODE.get(b) != 0 {
        0
    } else if var::PRICE_TWO.get(b) != 0 {
        2
    } else {
        1
    }
}

fn start_pressed(b: &mut Cps2Bus, s: Slot) -> bool {
    let pressed = !var::START_BEFORE.get(b) & var::START_NOW.get(b);
    pressed >> (s.index(b) & 7) & 1 != 0
}

fn idle(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    // the player is already there (started another way): "in game" state
    if s.g8(b, player::ACTIVE) != 0 && (s.g8(b, player::STATE) as i8) < 4 {
        s.set_p32(b, panel::FRAMES, 0);
        s.set_p8(b, panel::STATE, 4);
        s.set_p8(b, panel::STEP, 1);
        return play(x, s);
    }
    let cost = price(b);
    if cost != 0 && s.coins.credits(b) < cost {
        return no_credit_banner(b, s);
    }
    if var::PAUSE.get(b) != 0 {
        return notice_banner(b, s);
    }
    if !start_pressed(b, s) || var::GAME_ON.get(b) == 0 {
        return idle_banner(b, s);
    }
    crate::eeprom::count_game(x.b);
    let b = &mut *x.b;
    let credits = s.coins.credits(b).wrapping_sub(cost);
    s.coins.set_credits(b, credits);
    s.set_p8(b, panel::STEP, 1);
    s.set_p8(b, panel::SUBSTEP, 0);
    if var::PHASE.get(b) == 4 && var::STEP.get(b) <= 2 {
        return join_now(b, s);
    }
    s.set_p8(b, panel::STATE, 6);
    s.set_p16(b, panel::COUNTDOWN, CHOOSE_COUNTDOWN);
    let choice = select::free_choice(b, 0, false);
    s.set_p8(b, panel::CHOICE, choice);
    if var::PAUSE.get(b) == 0 {
        draw::portrait(b, s, false);
        choice::show_character(b, s, choice);
        draw::countdown_digits(b, s);
    } else {
        draw::clear_banner(b, s);
        notice_banner(b, s);
    }
}

/// 0x156fa: the player enters the game directly, without choosing their character.
fn join_now(b: &mut Cps2Bus, s: Slot) {
    s.set_p32(b, panel::FRAMES, 0);
    s.set_p8(b, panel::STATE, 4);
    s.set_p8(b, panel::CHOICE, 0);
    s.set_g8(b, player::ACTIVE, 1);
    let number = s.index(b);
    s.set_g8(b, player::NUMBER, number);
    init_shown(b, s);
}

/// Banner of the "start" zone from a pointer table: `a2` is the table, the chosen pointer
/// is the one for the language (`+4`) and the narrow layout (`+8`).
fn banner_from(b: &mut Cps2Bus, s: Slot, table: Rom, attr: u16) {
    let mut table = table.addr();
    if var::SETTINGS.get(b) & 0x20 != 0 {
        table += 4;
    }
    let mut columns = 17;
    if draw::narrow(b) {
        table += 8;
        columns = 12;
    }
    draw::pointer_banner(b, s, table, columns, attr);
}

/// 0x155b2: pause (or warning) banner.
fn notice_banner(b: &mut Cps2Bus, s: Slot) {
    banner_from(b, s, banners::NOTICE, 0xe);
}

/// 0x1557a: "PUSH START" (or "INSERT COIN" while waiting for a credit).
fn idle_banner(b: &mut Cps2Bus, s: Slot) {
    if var::CONTINUE_MODE.get(b) != 0 {
        return banner_from(b, s, banners::IDLE_CONTINUE, 0xe);
    }
    banner_from(b, s, banners::IDLE, 0x1d);
}

/// 0x155ba: not enough credits: "INSERT COIN" with the number of missing coins.
fn no_credit_banner(b: &mut Cps2Bus, s: Slot) {
    let credits = s.coins.credits(b);
    let paid = s.coins.coins(b);
    let missing = if var::PRICE_TWO.get(b) != 0 {
        if credits == 0 {
            return banner_from(b, s, banners::INSERT, 0xe);
        }
        1
    } else {
        let price = var::PRICE.get(b);
        if price == 1 {
            return banner_from(b, s, banners::INSERT_ONE, 0xe);
        }
        if paid == 0 {
            return banner_from(b, s, banners::INSERT, 0xe);
        }
        price.wrapping_sub(paid)
    };
    // 0x155f0: text of the number of missing coins
    let mut table = banners::INSERT_COUNT.addr();
    let mut columns = 17;
    if draw::narrow(b) {
        table = banners::INSERT_COUNT_NARROW.addr();
        columns = 12;
    }
    if var::SETTINGS.get(b) & 0x20 == 0 {
        table += 4 * missing as u32;
    }
    draw::pointer_banner(b, s, table, columns, 0xe);
}

