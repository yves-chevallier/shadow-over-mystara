//! The exit sequences of the scenes, one array of states per sequence (index = sequence number / 2; see
//! `docs/scenes.md` for the scenes).

use super::*;

// ---------------------------------------------------------------------------------------------
// The sequences (index = sequence number / 2; see `docs/scenes.md` for the scenes)

/// Final state without clearing the flags, 8-frame yield, only the final command afterwards.
pub(super) const L_SILENT: Leave = Leave { clear_flags: false, frames: 8, tail: Tail::Command, ..LEAVE };

pub(super) static SEQ_0: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq0_start),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
pub(super) static SEQ_1: Sequence = Sequence {
    update: Reduced,
    last: 24,
    states: &[
        St::Custom(walk_out_now),
        St::Custom(seq1_clear),
        Idle,
        BONUS,
        Idle,
        BLACKOUT,
        St::Leave(Leave { clear_word: Some(scratch(0xa53a)), ..LEAVE }),
    ],
};
pub(super) static SEQ_2: Sequence = Sequence {
    update: Full,
    last: 40,
    states: &[
        St::Custom(seq2_start),
        St::Custom(seq2_wait_exit),
        St::Custom(seq2_message_done),
        St::Custom(seq2_boss_state),
        St::Custom(seq2_countdown),
        St::TimerWait,
        TALLY,
        BONUS,
        CHARACTERS,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
/// Common model (sequences 3, 8, 11, 12): frame, exit, shared states, departure.
pub(super) static SEQ_3: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(LEAVE)] };
pub(super) static SEQ_4: Sequence = Sequence {
    update: Full,
    last: 40,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(144) },
        St::Custom(seq4_probe),
        St::Custom(seq4_effect),
        St::Custom(message_then_next),
        TALLY,
        BONUS,
        CHARACTERS,
        SHOP,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
pub(super) static SEQ_5: Sequence = Sequence {
    update: Update::World,
    last: 8,
    states: &[
        St::Clear { timer: Some(32) },
        St::Custom(walk_with_effect),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1UnlessLate, ..LEAVE }),
    ],
};
/// Sequence 6 (scene 7): like sequence 0, with sound command 0xe0a82(4).
pub(super) static SEQ_6: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq6_start),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
pub(super) static SEQ_7: Sequence = Sequence {
    update: Update::World,
    last: 8,
    states: &[
        St::Clear { timer: Some(32) },
        St::Custom(walk_with_effect),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
pub(super) static SEQ_9: Sequence = Sequence {
    update: Full,
    last: 24,
    states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, St::Leave(Leave { frames: 2, tail: Tail::Twice, ..LEAVE })],
};
pub(super) static SEQ_10: Sequence = Sequence {
    update: NoCollisions,
    last: 36,
    states: &[
        St::Custom(seq10_clear),
        St::Custom(seq10_walk),
        St::PlayersThen { state: 10, timer: Some(112) },
        Idle,
        St::TimerWait,
        TALLY,
        BONUS,
        CHARACTERS,
        BLACKOUT,
        St::Leave(LEAVE),
    ],
};
pub(super) static SEQ_13: Sequence = Sequence {
    update: Full,
    last: 32,
    states: &[
        CLEAR,
        WALK,
        TALLY,
        St::FadeScript { timer: 60 },
        Idle,
        St::PlayersThen { state: 16, timer: None },
        BONUS,
        CHARACTERS,
        leave_to(106),
    ],
};
pub(super) static SEQ_14: Sequence = Sequence {
    update: Full,
    last: 44,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(96) },
        TALLY,
        St::FadeScript { timer: 60 },
        St::Custom(seq14_wait_players),
        St::Custom(seq14_message_done),
        St::Custom(seq14_countdown),
        St::Custom(seq14_message_done_wait),
        St::TimerWait,
        BONUS,
        CHARACTERS,
        leave_to(27),
    ],
};
pub(super) static SEQ_15: Sequence = Sequence {
    update: Full,
    last: 36,
    states: &[
        St::Custom(seq15_clear),
        WALK,
        TALLY,
        St::Custom(seq15_fade),
        Idle,
        BLACKOUT,
        BONUS,
        CHARACTERS,
        SHOP,
        St::Leave(LEAVE),
    ],
};
pub(super) static SEQ_16: Sequence = Sequence {
    update: NoCollisions,
    last: 24,
    states: &[St::Custom(seq16_start), BLACKOUT, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(LEAVE)],
};
pub(super) static SEQ_17: Sequence = Sequence {
    update: Full,
    last: 28,
    states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(Leave { tail: Tail::Sound, ..LEAVE })],
};
pub(super) static SEQ_18: Sequence =
    Sequence { update: Full, last: 24, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, BLACKOUT, St::Leave(LEAVE)] };
pub(super) static SEQ_19: Sequence = Sequence {
    update: NoCollisions,
    last: 24,
    states: &[St::Clear { timer: Some(32) }, WALK, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(Leave { tail: Tail::Sound, ..LEAVE })],
};
/// Sequence 20 (scenes 35 and 111): players leave after 48 more frames, fade, scene 33.
pub(super) static SEQ_20: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq20_walk),
        St::Custom(fade_out_world),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Fixed(33), ..LEAVE }),
    ],
};
pub(super) static SEQ_21: Sequence = Sequence {
    update: NoCollisions,
    last: 12,
    states: &[
        St::Custom(seq21_start),
        St::Custom(seq21_route),
        St::Leave(Leave { wait_fade: WaitFade::Nothing, next_scene: NextScene::FromBlock, ..LEAVE }),
    ],
};
pub(super) static SEQ_22: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(L_SILENT)] };
pub(super) static SEQ_23: Sequence = Sequence {
    update: NoCollisions,
    last: 8,
    states: &[
        St::Clear { timer: Some(128) },
        St::Custom(seq23_walk),
        St::Leave(Leave { wait_fade: WaitFade::InputsWorld, next_scene: NextScene::Fixed(44), ..LEAVE }),
    ],
};
pub(super) static SEQ_25: Sequence = Sequence {
    update: Full,
    last: 36,
    states: &[CLEAR, WALK, TALLY, St::Custom(seq25_route), St::TimerWait, BONUS, CHARACTERS, SHOP, BLACKOUT, St::Leave(L_SILENT)],
};
pub(super) static SEQ_26: Sequence =
    Sequence { update: Full, last: 28, states: &[CLEAR, WALK, TALLY, BLACKOUT, BONUS, CHARACTERS, SHOP, leave_to(38)] };
/// Sequence 27 (scene 54), when `A5+0x66` is zero: tally, bonus, scene 55.
pub(super) static SEQ_27: Sequence =
    Sequence { update: Full, last: 16, states: &[St::Custom(seq27_clear), WALK, TALLY, BONUS, leave_to(55)] };
pub(super) static SEQ_28: Sequence =
    Sequence { update: Full, last: 24, states: &[St::Custom(seq28_clear), WALK, TALLY, BONUS, CHARACTERS, SHOP, leave_to(59)] };
/// Sequence 29 (scene 59, and 54 when `A5+0x66` is set): exit and fade, next scene.
pub(super) static SEQ_29: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq29_start),
        St::Custom(seq29_walk),
        St::Leave(Leave { wait_fade: WaitFade::InputsWorld, next_scene: NextScene::Plus1, ..LEAVE }),
    ],
};
pub(super) static SEQ_30: Sequence = Sequence {
    update: Full,
    last: 52,
    states: &[
        CLEAR,
        St::WalkWait { timer: Some(96) },
        TALLY,
        St::FadeScript { timer: 60 },
        St::PlayersThen { state: 12, timer: Some(64) },
        St::Custom(seq30_message_79),
        St::Custom(seq30_message_done),
        St::Custom(seq30_effect),
        St::Custom(seq30_players_wait),
        St::TimerWait,
        BONUS,
        CHARACTERS,
        SHOP,
        leave_to(65),
    ],
};
/// Sequence 31 (scene 65): fade after 192 frames, next scene without black palettes or sound.
pub(super) static SEQ_31: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq31_start),
        St::Custom(fade_out_world),
        St::Leave(Leave {
            wait_fade: WaitFade::World,
            black: false,
            clear_flags: false,
            next_scene: NextScene::Plus1,
            tail: Tail::Jump,
            ..LEAVE
        }),
    ],
};
pub(super) static SEQ_32: Sequence = Sequence {
    update: Full,
    last: 24,
    states: &[St::Custom(seq32_clear), WALK, TALLY, BONUS, St::Custom(seq32_route), CHARACTERS, leave_to(105)],
};
/// Sequence 33 (scene 67): players leave and fade, scene 38.
pub(super) static SEQ_33: Sequence = Sequence {
    update: Nothing,
    last: 0xff,
    states: &[
        St::Custom(seq33_start),
        St::Custom(seq33_walk),
        St::Leave(Leave { wait_fade: WaitFade::World, next_scene: NextScene::Fixed(38), tail: Tail::Sound, ..LEAVE }),
    ],
};
pub(super) static SEQ_34: Sequence = Sequence {
    update: NoCollisions,
    last: 28,
    states: &[
        St::Clear { timer: Some(4) },
        WALK,
        SHOP,
        BONUS,
        CHARACTERS,
        SHOP,
        BLACKOUT,
        St::Leave(Leave { tail: Tail::Sound, ..LEAVE }),
    ],
};
pub(super) static SEQ_35: Sequence = Sequence {
    update: Full,
    last: 20,
    states: &[
        St::Custom(seq35_clear),
        St::Custom(seq35_walk),
        BLACKOUT,
        TALLY,
        St::Custom(seq35_fade),
        St::Custom(seq35_message),
        St::Custom(seq35_boss),
        St::Leave(Leave { phase: PHASE_ENDING, extra_sound: Some(crate::services::sound::CMD_9B), ..LEAVE }),
    ],
};

/// Rewritten sequence of index `index`; `None`: ceded to the original.
pub(super) fn sequence(index: u8) -> Option<&'static Sequence> {
    Some(match index {
        0 => &SEQ_0,
        1 => &SEQ_1,
        2 => &SEQ_2,
        3 | 8 | 11 | 12 => &SEQ_3,
        4 => &SEQ_4,
        5 => &SEQ_5,
        6 => &SEQ_6,
        7 => &SEQ_7,
        9 => &SEQ_9,
        10 => &SEQ_10,
        13 => &SEQ_13,
        14 => &SEQ_14,
        15 => &SEQ_15,
        16 => &SEQ_16,
        17 => &SEQ_17,
        18 => &SEQ_18,
        19 => &SEQ_19,
        20 => &SEQ_20,
        21 => &SEQ_21,
        22 | 24 => &SEQ_22,
        23 => &SEQ_23,
        25 => &SEQ_25,
        26 => &SEQ_26,
        28 => &SEQ_28,
        29 => &SEQ_29,
        30 => &SEQ_30,
        31 => &SEQ_31,
        32 => &SEQ_32,
        33 => &SEQ_33,
        34 => &SEQ_34,
        35 => &SEQ_35,
        _ => return None,
    })
}
