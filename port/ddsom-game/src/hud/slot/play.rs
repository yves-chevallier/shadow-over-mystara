//! State 4 of a player slot: the player is in the game; the panel gauges, digits and names are kept up to date.

use super::*;

// ---------------------------------------------------------------------------------------------
// State 4: the player is in the game
// ---------------------------------------------------------------------------------------------

pub(super) fn play(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    let mode = v8(b, var::DISPLAY);
    if mode == 0 {
        return;
    }
    if s.g16(b, player::STATE) == 0x402 {
        return join_continue(x, s);
    }
    match mode {
        2 => rank_mode(b, s),
        3 => lock_mode(b, s),
        4 => {}
        _ => update_panel(b, s),
    }
}

/// Mode 1: updates the elements that changed, then the HP gauge.
pub(super) fn update_panel(b: &mut Cps2Bus, s: Slot) {
    update_hp(b, s);
    update_selection(b, s);
    money_changed(b, s);
    score_changed(b, s);
    update_names(b, s);
    redraw_player(b, s);
    money_redraw(b, s);
    score_redraw(b, s);
    letters(b, s);
    draw::hp_bar(b, s);
}

/// 0x16838: the displayed HP catch up with those of the player (the decrease is gradual, one point per frame).
pub(super) fn update_hp(b: &mut Cps2Bus, s: Slot) {
    let hp = s.g16(b, player::HP);
    let shown = s.p16(b, panel::SHOWN_HP);
    if hp != shown {
        s.set_p8(b, panel::HP_DIRTY, 0xff);
        if (hp as i16) < (shown as i16) {
            s.set_p16(b, panel::SHOWN_HP, shown.wrapping_sub(1));
        } else {
            s.set_p16(b, panel::SHOWN_HP, hp);
        }
    }
    let max = s.g16(b, player::HP_MAX);
    if max != s.p16(b, panel::SHOWN_HP_MAX) {
        s.set_p8(b, panel::HP_DIRTY, 1);
        s.set_p16(b, panel::SHOWN_HP_MAX, max);
    }
}

/// Byte of the class table (`+0x120` of the player, 16-byte row per family `+0x15d`).
pub(super) fn selection_class(b: &mut Cps2Bus, s: Slot) -> u8 {
    let kind = s.g8(b, 0x15d) as u32;
    s.g8(b, 0x120 + (kind << 4))
}

/// 0x15f9e: must the player's selection image be redrawn?
pub(super) fn update_selection(b: &mut Cps2Bus, s: Slot) {
    if s.g8(b, player::ACTIVE) == 0 {
        return;
    }
    let held = s.g8(b, 0x15c);
    if held != s.p8(b, panel::DISPLAYED_B) {
        s.set_p8(b, panel::DISPLAYED_B, held);
        s.set_p8(b, panel::DISPLAYED_A, 0xff);
        return;
    }
    if held == 0 {
        let settings = v8(b, var::SETTINGS) & 0x20;
        if settings != s.p8(b, 0x2f) {
            s.set_p8(b, 0x2f, settings);
            s.set_p8(b, panel::DISPLAYED_A, 0xfe);
        }
        return;
    }
    let class = selection_class(b, s);
    if class != s.p8(b, panel::DISPLAYED_A) {
        s.set_p8(b, panel::DISPLAYED_A, 0xff);
    }
}

/// 0x15ffc: redraws the selection letters and name when they have changed.
pub(super) fn letters(b: &mut Cps2Bus, s: Slot) {
    if s.g8(b, player::ACTIVE) == 0 {
        return;
    }
    let displayed = s.p8(b, panel::DISPLAYED_A);
    if (displayed as i8) >= 0 || (displayed != 0xff && s.p8(b, panel::FLAG_2A) != 0) {
        return;
    }
    let class = selection_class(b, s);
    choice::selection_letters(b, s, class);
    s.set_p8(b, panel::DISPLAYED_A, class);
    choice::selection_name(b, s, class);
}

/// 0x15e1e: has the score changed? (in narrow columns, after a delay)
pub(super) fn score_changed(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::SCORE_DIRTY, 0);
    if draw::narrow(b) {
        let delay = s.p8(b, panel::DELAY_2E);
        if delay != 0 {
            s.set_p8(b, panel::DELAY_2E, delay - 1);
            if delay == 1 {
                s.set_p8(b, panel::SCORE_DIRTY, 0xff);
                return;
            }
        }
    }
    if s.g32(b, player::SCORE) != s.p32(b, panel::SHOWN_SCORE) {
        s.set_p8(b, panel::SCORE_DIRTY, 0xff);
    }
}

/// 0x15e6e: has the money changed?
pub(super) fn money_changed(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::MONEY_DIRTY, 0);
    if s.g32(b, player::MONEY) != s.p32(b, panel::SHOWN_MONEY) {
        s.set_p8(b, panel::MONEY_DIRTY, 0xff);
    }
}

/// 0x15e50 / 0x15e58: redraws the score (followed by a closing cell in narrow columns).
pub(super) fn score_redraw(b: &mut Cps2Bus, s: Slot) {
    if s.p8(b, panel::SCORE_DIRTY) != 0 {
        draw_score(b, s);
    }
}

pub(super) fn draw_score(b: &mut Cps2Bus, s: Slot) {
    let end = draw::score(b, s);
    if draw::narrow(b) {
        draw::closing_cell(b, s, end, 0x2608);
    }
}

/// 0x15e86: redraws the money if it has changed.
pub(super) fn money_redraw(b: &mut Cps2Bus, s: Slot) {
    if s.g32(b, player::MONEY) == s.p32(b, panel::SHOWN_MONEY) || s.p8(b, panel::MONEY_DIRTY) == 0 {
        return;
    }
    if draw::narrow(b) && s.p8(b, panel::DELAY_2E) == 0 {
        s.set_p8(b, panel::DELAY_2E, 0x78);
    }
    let end = draw::money(b, s);
    if draw::narrow(b) {
        draw::closing_cell(b, s, end, 0x95e);
    }
}

/// 0x15ec8: the player name (multi-player layouts), or the blinking of the lone player.
pub(super) fn update_names(b: &mut Cps2Bus, s: Slot) {
    if v8(b, var::GAME_ON) == 0 {
        return;
    }
    if v8(b, var::LAYOUT) != 0 {
        let (first, second) = (s.g32(b, 0x160), s.g32(b, 0x164));
        if first != s.p32(b, panel::SHOWN_NAME) || second != s.p32(b, panel::SHOWN_NAME + 4) {
            choice::names(b, s);
        }
        return;
    }
    let mut busy = s.p16(b, panel::HP_BAR) != 0
        || s.p8(b, panel::DISPLAYED_A) == 0xff
        || s.p8(b, panel::SCORE_DIRTY) != 0
        || s.p8(b, panel::MONEY_DIRTY) != 0;
    if !busy {
        let timer = s.p16(b, panel::COUNTDOWN);
        if timer != 0 {
            s.set_p16(b, panel::COUNTDOWN, timer - 1);
            busy = timer == 1;
        }
    }
    if busy {
        if s.p8(b, panel::FLAG_2A) == 0 {
            return;
        }
        s.set_p8(b, panel::FLAG_2A, 0);
        return refresh_all(b, s);
    }
    if s.g16(b, player::HP) == 0 || !start_pressed(b, s) {
        return;
    }
    let flag = s.p8(b, panel::FLAG_2A) ^ 1;
    s.set_p8(b, panel::FLAG_2A, flag);
    if flag == 0 {
        return refresh_all(b, s);
    }
    s.set_p16(b, panel::COUNTDOWN, 0x78);
    choice::names_alt(b, s);
}

/// 0x15f44: everything will be redrawn on the next turn.
pub(super) fn refresh_all(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::DISPLAYED_A, 0xff);
    s.set_p8(b, panel::PLAYER_DIRTY, 0xff);
    s.set_p8(b, panel::HP_DIRTY, 1);
}

/// 0x16878: the player has changed: portrait, name, gauge, items, names and values are redrawn.
pub(super) fn redraw_player(b: &mut Cps2Bus, s: Slot) {
    if s.p8(b, panel::PLAYER_DIRTY) == 0 || v8(b, var::DISPLAY) == 0 {
        return;
    }
    s.set_p8(b, panel::PLAYER_DIRTY, 0);
    if v8(b, var::GAME_ON) == 0 || rd8(b, 0xff_1a14) == 0 {
        draw::character_palettes(b, s);
    }
    redraw_all(b, s);
}

/// 0x168a2: redraws all the zones of the player.
pub(super) fn redraw_all(b: &mut Cps2Bus, s: Slot) {
    draw::portrait(b, s, false);
    let character = s.g8(b, player::CHARACTER);
    draw::character_name(b, s, character);
    draw::class_icon(b, s);
    let class = s.g8(b, player::CLASS);
    draw::face(b, s, class);
    draw::items(b, s);
    choice::names(b, s);
    s.set_p8(b, panel::DISPLAYED_A, 0xff);
    s.set_p8(b, panel::DISPLAYED_B, 0xff);
    s.set_p8(b, panel::FLAG_2A, 0);
    s.set_p8(b, panel::DELAY_2E, 0);
    s.set_p8(b, 0x30, 0);
    s.set_p8(b, panel::HP_DIRTY, 0xff);
    let money = s.g32(b, player::MONEY);
    s.set_p32(b, panel::SHOWN_MONEY, money);
    if !draw::narrow(b) {
        draw::money(b, s);
    }
    draw_score(b, s);
}

/// Mode 2 (ranking): the player's items, or the banner if the player has special conditions.
pub(super) fn rank_mode(b: &mut Cps2Bus, s: Slot) {
    if s.p8(b, panel::HINT) != 0 {
        return notice_banner(b, s);
    }
    let (first, second) = (s.g32(b, player::ITEMS), s.g32(b, player::ITEMS + 4));
    if first == s.p32(b, panel::SHOWN_ITEMS) && second == s.p32(b, panel::SHOWN_ITEMS + 4) {
        return;
    }
    s.set_p32(b, panel::SHOWN_ITEMS, first);
    s.set_p32(b, panel::SHOWN_ITEMS + 4, second);
    choice::rank_items(b, s);
}

/// Mode 3 (locked choice of the players): the character chosen by each player is displayed, without duplicates.
pub(super) fn lock_mode(b: &mut Cps2Bus, s: Slot) {
    if s.p8(b, panel::FLAG_27) == 0xfe {
        return;
    }
    if s.p8(b, panel::FLAG_27) == 0xff {
        draw::portrait(b, s, false);
    }
    let character = s.g8(b, player::CHARACTER);
    if character & 0x80 != 0 {
        s.set_g8(b, player::CHARACTER, character & 0x7f);
        let class = prog8(b, 0x16b3c + ((character & 0x7f) >> 1) as u32);
        s.set_g8(b, player::CLASS, class);
        draw::character_palettes(b, s);
        redraw_all(b, s);
        s.set_p8(b, panel::FLAG_27, 0xfe);
        let locked = s.g8(b, player::CHARACTER) | 0x80;
        s.set_g8(b, player::CHARACTER, locked);
        return;
    }
    let mut choice = character;
    for k in 0..4 {
        let other = Slot::new(k);
        if other.g8(b, player::ACTIVE) != 0 && other.g8(b, player::CHARACTER) & 0x80 != 0
            && choice == other.g8(b, player::CHARACTER) & 0x7f {
                choice += 1;
            }
    }
    if choice == s.p8(b, panel::FLAG_27) {
        return;
    }
    s.set_p8(b, panel::FLAG_27, choice);
    s.set_p8(b, panel::CHOICE, choice);
    choice::show_character(b, s, choice);
}
