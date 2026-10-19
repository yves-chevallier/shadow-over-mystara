//! Phase 24 (`0x21c2a`) : séquences scriptées entre deux phases de jeu (h).
//!
//! La phase est déclenchée par le contrôleur de scène (SET 2, action de fin 4 : les joueurs sortent) ou
//! par un objet de scène. Elle contient deux séquences, choisies par l'étape (`A5+0x2d`, pas de 2) :
//!
//! * étape 0 : un boss (type 9) entre, un dialogue (message 4) s'ouvre, la caméra arrive en 800, le
//!   boss s'avance, un deuxième dialogue (message 6) conclut et le jeu reprend (phase 12) ;
//! * étape 2 : transition imposée (`A5+0x51e` = 76 → 92), la caméra avance jusqu'à 2408, puis le jeu reprend.
//!
//! Chaque image appelle ensuite la copie des entrées, la mise à jour du monde et les sprites.

use cps2::Cps2Bus;
use m68k::Cpu;

use super::{scratch, sub, var, Ctx};
use crate::mem::{rts, wr32};
use crate::Native;

/// Variables temporaires des deux séquences.
const TIMER: u32 = scratch(0x9a2a);
const MESSAGE: u32 = scratch(0x9a2c);
const BOSS: u32 = scratch(0x9a30);

/// Durée d'attente selon le nombre de joueurs (mots de l'espace programme, dans le code).
const WAITS: u32 = 0x21c94;
/// Compteur d'état du boss d'entrée (octet) : 12 = a fini d'entrer.
const BOSS_STATE: u32 = 0x4ff;
/// Abscisse de caméra (mot) à atteindre / vitesse du défilement forcé.
const CAMERA_X: u32 = 0x506;
const ALT_CAMERA_X: u32 = 0x556;
const TRANSITION: u32 = 0x51e;
/// Horloge de partie BCD au retour au jeu : 4:59:59... (minutes, secondes, images).
const GAME_CLOCK: u32 = 0x38;
const CLOCK_START: u32 = 0x0459_5900;
/// Phase 12, étape et sous-étape à zéro.
const GAME_PHASE: u32 = 0x0c00_0000;

const STEP_BOSS_INTRO: u8 = 0;
const STEP_SCROLL: u8 = 2;

pub fn phase_24(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut x = Ctx::new(c, b);
    match x.v8(var::STEP) {
        STEP_BOSS_INTRO => boss_intro(&mut x),
        STEP_SCROLL => scroll_out(&mut x),
        _ => {}
    }
    x.call(sub::INPUTS);
    x.world();
}

/// Rend la main au jeu : phase 12, scène libre, horloge réarmée.
fn resume_game(x: &mut Ctx) {
    x.set_v32(var::PHASE, GAME_PHASE);
    x.set_v8(var::HOLD, 0);
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.set_v32(GAME_CLOCK, CLOCK_START);
}

fn boss_intro(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.lock();
            x.set_v8(var::INPUT_LOCK, 0);
            let players = x.v8(var::PLAYERS) as u32;
            let wait = x.prog16(WAITS + 2 * players);
            x.wr16(TIMER, wait);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v8(BOSS_STATE, 8);
                let boss = x.spawn(sub::ALLOC_BOSS, 9);
                x.store_obj(BOSS, boss);
            }
        }
        4 => {
            if x.v8(BOSS_STATE) == 12 {
                x.next_sub();
                x.message(4, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let boss = x.load_obj(BOSS);
                boss.set_u8(x.b, 6, 2);
            }
        }
        8 => {
            if x.v16(ALT_CAMERA_X) == 800 {
                x.next_sub();
                x.set_v8(var::INPUT_LOCK, 0xff);
                x.wr16(TIMER, 80);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v8(var::HOLD, 0);
                let boss = x.load_obj(BOSS);
                let state = boss.u8(x.b, 6).wrapping_add(2);
                boss.set_u8(x.b, 6, state);
            } else if x.rd16(TIMER) == 40 {
                x.c.d[0] = 40;
                x.call(0x2860);
            }
        }
        12 => {
            let boss = x.load_obj(BOSS);
            if boss.u8(x.b, 6) == 0x16 {
                x.next_sub();
                x.set_v8(var::HOLD, 1);
                x.wr16(TIMER, 40);
            }
        }
        14 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(6, MESSAGE);
            }
        }
        16 => {
            if x.message_done(MESSAGE) {
                resume_game(x);
                let boss = x.load_obj(BOSS);
                wr32(x.b, boss.0 + 4, 0x0202_0000);
            }
        }
        _ => {}
    }
}

fn scroll_out(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.lock();
            x.set_v8(var::INPUT_LOCK, 0xff);
            x.wr16(TIMER, 20);
        }
        2 => {
            if x.rd16(TIMER) != 0 {
                if x.tick(TIMER) {
                    x.set_v16(TRANSITION, 76);
                    x.call(0xb68b0);
                }
            } else if x.v16(TRANSITION) == 92 {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        4 => {
            if x.rd16(TIMER) != 0 {
                x.tick(TIMER);
            } else if x.v16(CAMERA_X) <= 2408 {
                let camera = x.v16(CAMERA_X).wrapping_add(2);
                x.set_v16(CAMERA_X, camera);
            } else {
                x.next_sub();
                x.set_v16(CAMERA_X, 2408);
                x.call(0xb66e4);
            }
        }
        6 => {} // 0xb66e4 passe lui-même à la sous-étape 8
        8 => {
            x.set_v32(var::PHASE, GAME_PHASE);
            x.set_v8(var::HOLD, 0);
            x.set_v8(var::PLAYERS_FROZEN, 0);
            x.set_v8(var::INPUT_LOCK, 0xff);
            x.set_v32(GAME_CLOCK, CLOCK_START);
        }
        _ => {}
    }
}

pub const PHASE_24: Native = Native {
    entry: 0x21c2a,
    name: "scene_phase_24",
    cost: |_, _| 100,
    run: |c, b| {
        phase_24(c, b);
        rts(c, b);
    },
    touches_sound: false,
    outputs: 0,
    exact_cost: false,
};
