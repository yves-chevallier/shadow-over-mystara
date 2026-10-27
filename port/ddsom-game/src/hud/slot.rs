//! Machine à états d'un emplacement de joueur (voir le module `hud`).

use cps2::Cps2Bus;

use super::{choice, draw, panel, player, select, v8, var, Slot};
use crate::game::sbcd;
use crate::mem::{prog16, prog8, rd32, rd8, wr32, wr8, A5};
use crate::scene::Ctx;

/// Routines d'origine appelées par l'interface (comptabilité, sons, démarrage de joueur...).
mod rom {
    /// Comptabilité : une partie démarre.
    pub const COUNT_GAME: u32 = 0x2e4ec;
    /// Statistiques : le joueur a choisi le personnage D0 ; temps de jeu D0 (long) d'un joueur qui revient.
    pub const COUNT_CHARACTER: u32 = 0x2e630;
    pub const COUNT_TIME: u32 = 0x2e696;
    /// Initialisations du joueur k (créent le joueur dans la partie), pour le choix du personnage et pour
    /// la reprise ; leur adresse est dans une table du code.
    pub const START_PLAYER: u32 = 0x158b8;
    pub const START_PLAYER_RESUME: u32 = 0x15de0;
    pub const START_PLAYER_ALT: u32 = 0x15e10;
    /// Classe du joueur selon son personnage (octet, tables du code).
    pub const CLASS_OF_CHARACTER: u32 = 0x158c0;
    pub const CLASS_OF_CHARACTER_ALT: u32 = 0x15e18;
}

/// Pointeurs vers les textes des bannières (tables de pointeurs dans le code).
mod banners {
    /// « PUSH START » hors reprise (deux langues, large puis étroit).
    pub const IDLE: u32 = 0x156ca;
    /// Reprise : « PUSH START ».
    pub const IDLE_CONTINUE: u32 = 0x156da;
    /// Bannière de pause (aussi quand un objet est présent).
    pub const NOTICE: u32 = 0x156ea;
    /// « INSERT COIN » : un seul crédit ou prix de 1 pièce, pièces manquantes, plus d'une pièce.
    pub const INSERT_ONE: u32 = 0x15662;
    pub const INSERT: u32 = 0x15672;
    pub const INSERT_COUNT: u32 = 0x15682;
    pub const INSERT_COUNT_NARROW: u32 = 0x156a6;
}

/// Compte à rebours initial du choix (BCD 19, sous-compteur 0x32) et de la reprise (BCD 19, 0x3c).
const CHOOSE_COUNTDOWN: u16 = 0x1932;
const RESUME_COUNTDOWN: u16 = 0x193c;
const RESUME_END_FRAMES: u16 = 0x78;

/// 0x1507a : oublie ce qui est affiché dans chaque panneau (le joueur et le mode seront redessinés).
pub fn forget_displayed(b: &mut Cps2Bus) {
    for k in 0..4 {
        let s = Slot::new(k);
        s.set_p8(b, panel::PLAYER_DIRTY, 0xff);
        s.set_p8(b, panel::LAST_MODE, 0xff);
    }
}

/// Compteur de pièces d'un emplacement selon le mode des monnayeurs : propre, partagé, ou un pour deux
/// joueurs.
fn coin_record(b: &mut Cps2Bus, s: Slot) -> u32 {
    match v8(b, var::COIN_MODE) {
        2 | 6 | 11 => s.coins,
        9 => A5 + var::COINS + if s.index(b) & 2 != 0 { 0x10 } else { 0 },
        _ => A5 + var::COINS,
    }
}

/// 0x15216 : un tour d'un emplacement.
pub fn step(x: &mut Ctx, s: Slot) {
    let frames = s.p32(x.b, panel::FRAMES).wrapping_add(1);
    s.set_p32(x.b, panel::FRAMES, frames);
    let mode = v8(x.b, var::DISPLAY);
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
// Changement de mode d'affichage (0x178ee)
// ---------------------------------------------------------------------------------------------

/// Palette du joueur dans la page de la salle d'attente (indexée par son numéro et son personnage).
const LOBBY_PALETTES: u32 = 0x179a2;

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
                draw::blend_for(b, s, 0x25_c640, 0x91_c500);
                draw::blend_for(b, s, 0x25_c4c0, 0x91_c480);
                draw::portrait(b, s, true);
                let character = s.g8(b, player::CHARACTER);
                draw::character_name(b, s, character);
                let class = s.g8(b, player::CLASS);
                draw::face(b, s, class);
                let number = s.g8(b, player::NUMBER) as u32;
                let base = prog16(b, LOBBY_PALETTES + 2 * number);
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
// État 0 : initialisation
// ---------------------------------------------------------------------------------------------

fn enter(b: &mut Cps2Bus, s: Slot) {
    let state = s.p8(b, panel::STATE).wrapping_add(2);
    s.set_p8(b, panel::STATE, state);
    s.set_p8(b, panel::STATE + 1, 0);
    init_shown(b, s);
}

/// 0x152a8 : les valeurs affichées sont celles du joueur ; tout sera redessiné.
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
// État 2 : attente d'un joueur
// ---------------------------------------------------------------------------------------------

/// Prix en crédits d'une partie (0 en mode reprise).
fn price(b: &mut Cps2Bus) -> u8 {
    if v8(b, var::CONTINUE_MODE) != 0 {
        0
    } else if v8(b, var::PRICE_TWO) != 0 {
        2
    } else {
        1
    }
}

fn start_pressed(b: &mut Cps2Bus, s: Slot) -> bool {
    let pressed = !v8(b, var::START_BEFORE) & v8(b, var::START_NOW);
    pressed >> (s.index(b) & 7) & 1 != 0
}

fn idle(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    // le joueur est déjà là (démarré autrement) : état « en jeu »
    if s.g8(b, player::ACTIVE) != 0 && (s.g8(b, player::STATE) as i8) < 4 {
        s.set_p32(b, panel::FRAMES, 0);
        s.set_p8(b, panel::STATE, 4);
        s.set_p8(b, panel::STEP, 1);
        return play(x, s);
    }
    let cost = price(b);
    if cost != 0 && rd8(b, s.coins + 2) < cost {
        return no_credit_banner(b, s);
    }
    if v8(b, var::PAUSE) != 0 {
        return notice_banner(b, s);
    }
    if !start_pressed(b, s) || v8(b, var::GAME_ON) == 0 {
        return idle_banner(b, s);
    }
    x.call(rom::COUNT_GAME);
    let b = &mut *x.b;
    let credits = rd8(b, s.coins + 2).wrapping_sub(cost);
    wr8(b, s.coins + 2, credits);
    s.set_p8(b, panel::STEP, 1);
    s.set_p8(b, panel::SUBSTEP, 0);
    if v8(b, var::PHASE) == 4 && v8(b, var::STEP) <= 2 {
        return join_now(b, s);
    }
    s.set_p8(b, panel::STATE, 6);
    s.set_p16(b, panel::COUNTDOWN, CHOOSE_COUNTDOWN);
    let choice = select::free_choice(b, 0, false);
    s.set_p8(b, panel::CHOICE, choice);
    if v8(b, var::PAUSE) == 0 {
        draw::portrait(b, s, false);
        choice::show_character(b, s, choice);
        draw::countdown_digits(b, s);
    } else {
        draw::clear_banner(b, s);
        notice_banner(b, s);
    }
}

/// 0x156fa : le joueur entre directement dans la partie, sans choisir son personnage.
fn join_now(b: &mut Cps2Bus, s: Slot) {
    s.set_p32(b, panel::FRAMES, 0);
    s.set_p8(b, panel::STATE, 4);
    s.set_p8(b, panel::CHOICE, 0);
    s.set_g8(b, player::ACTIVE, 1);
    let number = s.index(b);
    s.set_g8(b, player::NUMBER, number);
    init_shown(b, s);
}

/// Bannière de la zone « départ » à partir d'une table de pointeurs : `a2` est la table, le pointeur choisi
/// est celui de la langue (`+4`) et de la disposition étroite (`+8`).
fn banner_from(b: &mut Cps2Bus, s: Slot, mut table: u32, attr: u16) {
    if v8(b, var::SETTINGS) & 0x20 != 0 {
        table += 4;
    }
    let mut columns = 17;
    if draw::narrow(b) {
        table += 8;
        columns = 12;
    }
    draw::pointer_banner(b, s, table, columns, attr);
}

/// 0x155b2 : bannière de pause (ou d'avertissement).
fn notice_banner(b: &mut Cps2Bus, s: Slot) {
    banner_from(b, s, banners::NOTICE, 0xe);
}

/// 0x1557a : « PUSH START » (ou « INSERT COIN » en attente de crédit).
fn idle_banner(b: &mut Cps2Bus, s: Slot) {
    if v8(b, var::CONTINUE_MODE) != 0 {
        return banner_from(b, s, banners::IDLE_CONTINUE, 0xe);
    }
    banner_from(b, s, banners::IDLE, 0x1d);
}

/// 0x155ba : pas assez de crédits : « INSERT COIN » avec le nombre de pièces manquantes.
fn no_credit_banner(b: &mut Cps2Bus, s: Slot) {
    let credits = rd8(b, s.coins + 2);
    let paid = rd8(b, s.coins + 1);
    let missing = if v8(b, var::PRICE_TWO) != 0 {
        if credits == 0 {
            return banner_from(b, s, banners::INSERT, 0xe);
        }
        1
    } else {
        let price = v8(b, var::PRICE);
        if price == 1 {
            return banner_from(b, s, banners::INSERT_ONE, 0xe);
        }
        if paid == 0 {
            return banner_from(b, s, banners::INSERT, 0xe);
        }
        price.wrapping_sub(paid)
    };
    // 0x155f0 : texte du nombre de pièces manquantes
    let mut table = banners::INSERT_COUNT;
    let mut columns = 17;
    if draw::narrow(b) {
        table = banners::INSERT_COUNT_NARROW;
        columns = 12;
    }
    if v8(b, var::SETTINGS) & 0x20 == 0 {
        table += 4 * missing as u32;
    }
    draw::pointer_banner(b, s, table, columns, 0xe);
}

// ---------------------------------------------------------------------------------------------
// État 4 : le joueur est en jeu
// ---------------------------------------------------------------------------------------------

fn play(x: &mut Ctx, s: Slot) {
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

/// Mode 1 : met à jour les éléments qui ont changé, puis la jauge de PV.
fn update_panel(b: &mut Cps2Bus, s: Slot) {
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

/// 0x16838 : les PV affichés rattrapent ceux du joueur (la baisse est progressive, un point par image).
fn update_hp(b: &mut Cps2Bus, s: Slot) {
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

/// Octet de la table de classe (`+0x120` du joueur, ligne de 16 octets par famille `+0x15d`).
fn selection_class(b: &mut Cps2Bus, s: Slot) -> u8 {
    let kind = s.g8(b, 0x15d) as u32;
    s.g8(b, 0x120 + (kind << 4))
}

/// 0x15f9e : l'image de sélection du joueur doit-elle être redessinée ?
fn update_selection(b: &mut Cps2Bus, s: Slot) {
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

/// 0x15ffc : redessine les lettres et le nom de sélection quand ils ont changé.
fn letters(b: &mut Cps2Bus, s: Slot) {
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

/// 0x15e1e : le score a-t-il changé ? (en colonnes étroites, après un délai)
fn score_changed(b: &mut Cps2Bus, s: Slot) {
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

/// 0x15e6e : l'argent a-t-il changé ?
fn money_changed(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::MONEY_DIRTY, 0);
    if s.g32(b, player::MONEY) != s.p32(b, panel::SHOWN_MONEY) {
        s.set_p8(b, panel::MONEY_DIRTY, 0xff);
    }
}

/// 0x15e50 / 0x15e58 : redessine le score (suivi d'une case de fermeture en colonnes étroites).
fn score_redraw(b: &mut Cps2Bus, s: Slot) {
    if s.p8(b, panel::SCORE_DIRTY) != 0 {
        draw_score(b, s);
    }
}

fn draw_score(b: &mut Cps2Bus, s: Slot) {
    let end = draw::score(b, s);
    if draw::narrow(b) {
        draw::closing_cell(b, s, end, 0x2608);
    }
}

/// 0x15e86 : redessine l'argent s'il a changé.
fn money_redraw(b: &mut Cps2Bus, s: Slot) {
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

/// 0x15ec8 : le nom du joueur (dispositions à plusieurs joueurs), ou le clignotement du joueur seul.
fn update_names(b: &mut Cps2Bus, s: Slot) {
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

/// 0x15f44 : tout sera redessiné au prochain tour.
fn refresh_all(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::DISPLAYED_A, 0xff);
    s.set_p8(b, panel::PLAYER_DIRTY, 0xff);
    s.set_p8(b, panel::HP_DIRTY, 1);
}

/// 0x16878 : le joueur a changé : portrait, nom, jauge, objets, noms et valeurs sont redessinés.
fn redraw_player(b: &mut Cps2Bus, s: Slot) {
    if s.p8(b, panel::PLAYER_DIRTY) == 0 || v8(b, var::DISPLAY) == 0 {
        return;
    }
    s.set_p8(b, panel::PLAYER_DIRTY, 0);
    if v8(b, var::GAME_ON) == 0 || rd8(b, 0xff_1a14) == 0 {
        draw::character_palettes(b, s);
    }
    redraw_all(b, s);
}

/// 0x168a2 : redessine toutes les zones du joueur.
fn redraw_all(b: &mut Cps2Bus, s: Slot) {
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

/// Mode 2 (classement) : les objets du joueur, ou la bannière si le joueur a des conditions particulières.
fn rank_mode(b: &mut Cps2Bus, s: Slot) {
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

/// Mode 3 (choix verrouillé des joueurs) : le personnage choisi par chaque joueur s'affiche, sans doublon.
fn lock_mode(b: &mut Cps2Bus, s: Slot) {
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
        if other.g8(b, player::ACTIVE) != 0 && other.g8(b, player::CHARACTER) & 0x80 != 0 {
            if choice == other.g8(b, player::CHARACTER) & 0x7f {
                choice += 1;
            }
        }
    }
    if choice == s.p8(b, panel::FLAG_27) {
        return;
    }
    s.set_p8(b, panel::FLAG_27, choice);
    s.set_p8(b, panel::CHOICE, choice);
    choice::show_character(b, s, choice);
}

// ---------------------------------------------------------------------------------------------
// État 6 : choix du personnage
// ---------------------------------------------------------------------------------------------

/// Un bouton (partie haute des fronts montants) vient-il d'être pressé ?
fn button_pressed(b: &mut Cps2Bus, s: Slot) -> bool {
    let index = s.index(b);
    select::inputs(b, index).1 & 0xf0 != 0
}

/// Le compte à rebours BCD du panneau perd une unité et repart pour `frames` images ; vrai (sans rien
/// changer) s'il passe sous zéro.
fn tick_countdown(b: &mut Cps2Bus, s: Slot, frames: u8) -> bool {
    let (value, _) = sbcd(s.p8(b, panel::COUNTDOWN), 1, false);
    if value & 0x80 != 0 {
        return true;
    }
    s.set_p8(b, panel::COUNTDOWN, value);
    s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
    false
}

/// Le bouton de départ du joueur est-il tout juste pressé ?
fn start_edge(b: &mut Cps2Bus, s: Slot) -> bool {
    start_pressed(b, s)
}

/// 0x157a8 : redessine l'écran de choix (après une pause).
fn redraw_choice(b: &mut Cps2Bus, s: Slot) {
    draw::portrait(b, s, false);
    let choice = s.p8(b, panel::CHOICE);
    choice::show_character(b, s, choice);
    draw::countdown_digits(b, s);
    s.set_p8(b, panel::PLAYER_DIRTY, 0);
}

/// 0x15788 : le joueur choisit son personnage (croix : suivant ou précédent, temps limité).
fn choose(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    let pause = v8(b, var::PAUSE);
    let seen = rd8(b, var::SEEN_PAUSE);
    if pause != seen {
        if !pause & seen != 0 {
            redraw_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if s.p8(b, panel::PLAYER_DIRTY) != 0 {
        if v8(b, var::PAUSE) == 0 {
            redraw_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if v8(b, var::PAUSE) != 0 {
        return notice_banner(b, s);
    }
    let current = s.p8(b, panel::CHOICE);
    let choice = select::free_choice(b, current, false);
    let choice = select::navigate(b, s, choice, &|b, c, back| select::free_choice(b, c, back));
    if choice != s.p8(b, panel::CHOICE) {
        s.set_p8(b, panel::CHOICE, choice);
        choice::show_character(b, s, choice);
        draw::countdown_digits(b, s);
    }
    let mut done = false;
    let frames = s.p8(b, panel::COUNTDOWN_FRAMES).wrapping_sub(1);
    s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
    if frames == 0 {
        if tick_countdown(b, s, 0x32) {
            done = true;
        } else {
            draw::countdown_digits(b, s);
        }
    }
    if !done && !button_pressed(b, s) {
        if !start_edge(b, s) {
            return;
        }
        let variant = s.p8(b, panel::CHOICE) ^ 1;
        s.set_p8(b, panel::CHOICE, variant);
    }
    // le choix est fait (bouton, ou fin du temps)
    let current = s.p8(b, panel::CHOICE);
    let chosen = select::free_choice(b, current, false);
    s.set_p8(b, panel::CHOICE, chosen);
    s.set_g8(b, player::CHARACTER, chosen);
    x.c.d[0] = chosen as u32;
    x.call(rom::COUNT_CHARACTER);
    let b = &mut *x.b;
    s.set_p8(b, panel::LAST_MODE, 0x80);
    s.set_p8(b, panel::STATE, 0);
    let class = prog8(b, rom::CLASS_OF_CHARACTER + (chosen >> 1) as u32);
    s.set_g8(b, 0x15d, class);
    start_player(x, s, rom::START_PLAYER);
}

/// 0x1588c : appelle l'initialisation du joueur de ce panneau (adresse lue dans une table du code), puis
/// règle le personnage 5.
fn start_player(x: &mut Ctx, s: Slot, table: u32) {
    let index = s.index(x.b) as u32;
    let routine = prog16(x.b, table + 2 * index) as i16 as i32 as u32;
    x.call(routine);
    let b = &mut *x.b;
    if (s.g8(b, player::CHARACTER) & 0x7f) >> 1 == 5 {
        s.set_g8(b, 0x162, 1);
        s.set_g8(b, 0x16b, 5);
        wr8(b, 0xff_2478 + 0x3f, 1);
    }
}

// ---------------------------------------------------------------------------------------------
// États 8, 0xa, 0xc : reprise
// ---------------------------------------------------------------------------------------------

/// 0x15a26 : le joueur est tombé (état 0x402) : début de la reprise.
fn join_continue(x: &mut Ctx, s: Slot) {
    x.c.d[0] = s.p32(x.b, panel::FRAMES);
    x.call(rom::COUNT_TIME);
    let b = &mut *x.b;
    s.set_p8(b, panel::SUBSTEP, 1);
    let character = s.g8(b, player::CHARACTER);
    s.set_p8(b, panel::CHOSEN, character);
    let class = s.g8(b, player::CLASS);
    s.set_p8(b, panel::CLASS, class);
    let flags = s.g8(b, player::FLAGS) | 2;
    s.set_g8(b, player::FLAGS, flags);
    s.set_p8(b, panel::CLAIM, 1);
    if v8(b, 0x35) == 0 && others_free(b) {
        let flags = s.g8(b, player::FLAGS) & !2;
        s.set_g8(b, player::FLAGS, flags);
        s.set_p8(b, panel::CLAIM, 0);
    }
    // un joueur arrive : le rang baisse (`objects::rank`)
    crate::objects::rank::lower_68k(x.c, x.b);
    let b = &mut *x.b;
    s.set_p8(b, panel::STEP, 0);
    s.set_p8(b, panel::FLAG_27, 0);
    s.set_p8(b, panel::STATE, 8);
    if v8(b, var::CONTINUE_ALLOWED) == 0 {
        return resume_declined(b, s);
    }
    draw::clear_frame(b, s);
    draw::start_banner(b, s, draw::START_BANNER_A);
    restart_countdown(b, s);
    resume(x, s);
}

/// Aucun panneau n'est en reprise (somme des sous-étapes nulle) : le joueur est le seul à attendre.
fn others_free(b: &mut Cps2Bus) -> bool {
    (0..4).fold(1u8, |free, k| free.wrapping_sub(Slot::new(k).p8(b, panel::SUBSTEP))) == 0
}

/// Pièces comptées pour la reprise de ce joueur : les pièces partielles du prix, ou les crédits.
fn coins_inserted(b: &mut Cps2Bus, s: Slot) -> u8 {
    if v8(b, var::PRICE_TWO) == 0 && v8(b, var::PRICE) != 1 {
        rd8(b, s.coins + 1)
    } else {
        rd8(b, s.coins + 2)
    }
}

/// 0x15a9c : le compte à rebours repart de 19 secondes, avec les pièces actuelles en mémoire.
fn restart_countdown(b: &mut Cps2Bus, s: Slot) {
    let coins = coins_inserted(b, s);
    s.set_p8(b, panel::SHOWN_COINS, coins);
    s.set_p16(b, panel::COUNTDOWN, RESUME_COUNTDOWN);
    draw::continue_digits(b, s);
}

/// 0x15ac4 : compte à rebours de la reprise : il repart quand une pièce est insérée, se précipite quand un
/// bouton est pressé dans les dernières secondes, et la reprise a lieu sur la touche de départ.
fn resume(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    if s.p8(b, panel::FLAG_27) != 0 {
        return resume_end_enter(b, s);
    }
    if s.p8(b, panel::CLAIM) == 0 {
        let flags = s.g8(b, player::FLAGS) & !2;
        s.set_g8(b, player::FLAGS, flags);
        s.set_p8(b, panel::CLAIM, 1);
        s.set_g8(b, player::FLAGS, flags | 2);
        if v8(b, 0x35) == 0 && others_free(b) {
            s.set_g8(b, player::FLAGS, flags);
            s.set_p8(b, panel::CLAIM, 0);
        }
    }
    if s.p8(b, panel::PLAYER_DIRTY) != 0 {
        s.set_p8(b, panel::PLAYER_DIRTY, 0);
        draw::start_banner(b, s, draw::START_BANNER_A);
        draw::continue_digits(b, s);
    }
    let hurry = s.p8(b, panel::COUNTDOWN) <= 0x16 && button_pressed(b, s);
    let tick = hurry || {
        let frames = s.p8(b, panel::COUNTDOWN_FRAMES).wrapping_sub(1);
        s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
        frames == 0
    };
    if tick {
        if tick_countdown(b, s, 0x3c) {
            return resume_declined(b, s);
        }
        draw::continue_digits(b, s);
    }
    let coins = coins_inserted(b, s);
    let shown = s.p8(b, panel::SHOWN_COINS);
    if coins as i8 > shown as i8 {
        restart_countdown(b, s);
        return resume(x, s);
    }
    if coins != shown {
        s.set_p8(b, panel::SHOWN_COINS, coins);
    }
    if v8(b, var::CONTINUE_MODE) == 0 && rd8(b, s.coins + 2) == 0 {
        return;
    }
    if !start_pressed(b, s) {
        return;
    }
    x.call(rom::COUNT_GAME);
    let b = &mut *x.b;
    let credits = rd8(b, s.coins + 2).wrapping_sub(1);
    wr8(b, s.coins + 2, credits);
    s.set_p8(b, panel::SUBSTEP, 0);
    s.set_p8(b, panel::STEP, 1);
    s.set_p8(b, panel::STATE, 0xc);
    s.set_p16(b, panel::COUNTDOWN, RESUME_COUNTDOWN);
    let character = s.g8(b, player::CHARACTER);
    s.set_p8(b, panel::CHOICE, character);
    if v8(b, var::PAUSE) == 0 {
        draw::portrait(b, s, false);
        choice::show_character(b, s, character);
        let class = s.g8(b, player::CLASS);
        draw::face(b, s, class);
        draw::countdown_digits(b, s);
    } else {
        draw::clear_banner(b, s);
        notice_banner(b, s);
    }
}

/// 0x15c0c : la reprise est refusée ou le temps est écoulé : la partie du joueur est terminée et son argent
/// s'ajoute à son score (plafonné).
fn resume_declined(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::SUBSTEP, 0);
    s.set_p8(b, panel::FLAG_27, 1);
    s.set_p16(b, panel::COUNTDOWN, 0);
    wr32(b, s.player + player::STATE, 0x0600_0000);
    let money = rd32(b, s.player + player::MONEY) & !0xf;
    wr32(b, s.player + player::MONEY, money);
    // score += argent, en décimal codé (octet par octet, de la fin)
    let mut carry = false;
    let mut sum = [0u8; 4];
    for k in (0..4u32).rev() {
        let add = rd8(b, s.player + player::MONEY + k);
        let base = rd8(b, s.player + player::SCORE + k);
        let (value, out) = crate::ops::abcd(add, base, carry);
        sum[k as usize] = value;
        carry = out;
    }
    let total = u32::from_be_bytes(sum);
    wr32(b, s.player + player::SCORE, if (total as i32) <= 0x999_9999 { total } else { 0x999_9999 });
}

/// 0x15c50 : fin de la reprise : la bannière de fin s'affiche, puis l'état 0xa attend la fin du message.
pub fn resume_end_enter(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::FLAG_27, 0);
    draw::start_banner(b, s, draw::START_BANNER_B);
    s.set_p8(b, panel::STATE, 0xa);
    s.set_p16(b, panel::COUNTDOWN, RESUME_END_FRAMES);
    resume_end(b, s);
}

/// 0x15c64 : état 0xa, le message de fin reste affiché 120 images, puis le panneau est libéré.
fn resume_end(b: &mut Cps2Bus, s: Slot) {
    s.set_p8(b, panel::FLAG_27, 0);
    let left = s.p16(b, panel::COUNTDOWN).wrapping_sub(1);
    s.set_p16(b, panel::COUNTDOWN, left);
    if left != 0 {
        return;
    }
    s.set_p8(b, panel::STATE, 0);
    s.set_p8(b, panel::STEP, 0);
    s.set_p8(b, panel::SUBSTEP, 0);
    s.set_p8(b, 0x30, 0);
    s.set_p8(b, panel::LAST_MODE, 0xff);
    s.set_p8(b, panel::PLAYER_DIRTY, 0xff);
}

/// 0x15cae : redessine l'écran de choix de la reprise (après une pause).
fn redraw_resume_choice(b: &mut Cps2Bus, s: Slot) {
    draw::portrait(b, s, false);
    let choice = s.p8(b, panel::CHOICE);
    choice::show_character(b, s, choice);
    if choice >> 1 == s.p8(b, panel::CHOSEN) >> 1 {
        let class = s.g8(b, player::CLASS);
        draw::face(b, s, class);
    }
    draw::countdown_digits(b, s);
    s.set_p8(b, panel::PLAYER_DIRTY, 0);
}

/// 0x15c8e : état 0xc, le joueur qui reprend choisit son personnage ; il peut reprendre celui de sa partie
/// (même personnage : il garde son équipement) ou un autre.
fn resume_choose(x: &mut Ctx, s: Slot) {
    let b = &mut *x.b;
    let pause = v8(b, var::PAUSE);
    let seen = rd8(b, var::SEEN_PAUSE);
    if pause != seen {
        if !pause & seen != 0 {
            redraw_resume_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if s.p8(b, panel::PLAYER_DIRTY) != 0 {
        if v8(b, var::PAUSE) == 0 {
            redraw_resume_choice(b, s);
        } else {
            draw::clear_banner(b, s);
            return notice_banner(b, s);
        }
    }
    if v8(b, var::PAUSE) != 0 {
        return notice_banner(b, s);
    }
    let own = s.p8(b, panel::CHOSEN) >> 1;
    let other = |b: &mut Cps2Bus, c: u8, back: bool| select::other_choice(b, s, c, back);
    let current = s.p8(b, panel::CHOICE);
    let choice = other(b, current, false);
    let choice = select::navigate(b, s, choice, &other);
    if choice != s.p8(b, panel::CHOICE) {
        s.set_p8(b, panel::CHOICE, choice);
        choice::show_character(b, s, choice);
        if s.p8(b, panel::CHOICE) >> 1 == own {
            let class = s.g8(b, player::CLASS);
            draw::face(b, s, class);
        }
        draw::countdown_digits(b, s);
    }
    let mut done = false;
    let frames = s.p8(b, panel::COUNTDOWN_FRAMES).wrapping_sub(1);
    s.set_p8(b, panel::COUNTDOWN_FRAMES, frames);
    if frames == 0 {
        if tick_countdown(b, s, 0x32) {
            done = true;
        } else {
            draw::countdown_digits(b, s);
        }
    }
    if !done && !button_pressed(b, s) {
        if !start_edge(b, s) {
            return;
        }
        let variant = s.p8(b, panel::CHOICE) ^ 1;
        s.set_p8(b, panel::CHOICE, variant);
    }
    // le choix est fait : le dernier chiffre du score avance (jusqu'à 9)
    let last = s.g8(b, 0x113);
    if last & 0xf != 9 {
        s.set_g8(b, 0x113, last.wrapping_add(1));
    }
    let current = s.p8(b, panel::CHOICE);
    let chosen = select::resume_choice(b, s, current, false);
    s.set_p8(b, panel::CHOICE, chosen);
    s.set_g8(b, player::CHARACTER, chosen);
    x.c.d[0] = chosen as u32;
    x.call(rom::COUNT_CHARACTER);
    let b = &mut *x.b;
    let counter = rd8(b, A5 + 0x5a).wrapping_add(1);
    wr8(b, A5 + 0x5a, counter);
    s.set_p8(b, panel::LAST_MODE, 0x80);
    s.set_p8(b, panel::STATE, 0);
    if chosen >> 1 == s.p8(b, panel::CHOSEN) >> 1 {
        s.set_p32(b, panel::FRAMES, 0);
        start_player(x, s, rom::START_PLAYER_RESUME);
    } else {
        let class = prog8(b, rom::CLASS_OF_CHARACTER_ALT + (chosen >> 1) as u32);
        s.set_g8(b, 0x15d, class);
        start_player(x, s, rom::START_PLAYER_ALT);
    }
}
