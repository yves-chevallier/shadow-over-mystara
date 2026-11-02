//! Typed views of the SET C objects (types 2, 3 and 15), declared once with `record!` (see `set::fields`). SET 15
//! picks one of 25 small automata by its variant: each automaton gives the free fields its own meaning, hence one
//! view per variant (or group of variants). A guessed meaning is marked (h).

use crate::families::set::fields::view;
use crate::record::record;

view! {
    /// SET 15 (`finale`): the fields that every variant uses the same way.
    ScreenObj;
}

record! {
    impl ScreenObj {
        /// The conductor object that drives the screen (address).
        conductor, set_conductor, CONDUCTOR: u32 @ 0x4c;
    }
}

view! {
    /// SET 15, variants 0 to 6 (`finale`): shakes, quakes, sliding and scrolling backgrounds.
    ScrollScreenObj;
}

record! {
    impl ScrollScreenObj {
        /// Frames of the current step (title blink, quake).
        timer, set_timer, TIMER: u8 @ 0x14;
        /// Offset added to the abscissa of the background.
        offset_x, set_offset_x, OFFSET_X: u16 @ 0x16;
        /// Step of the shake table (0 to 3).
        step, set_step, STEP: u8 @ 0x18;
        /// Mask of the background abscissa (wrapping).
        mask_x, set_mask_x, MASK_X: u16 @ 0x1c;
        /// Mask of the background ordinate (wrapping).
        mask_y, set_mask_y, MASK_Y: u16 @ 0x1e;
        /// Position of the sliding backdrop (16.16).
        slide, set_slide, SLIDE: u32 @ 0x34;
        /// Frames before the next flash or quake.
        delay, set_delay, DELAY: u8 @ 0x42;
        /// Object that created it (low word of its address).
        parent, set_parent, PARENT: u16 @ 0x42;
        /// Bit 0: phase of the title blink.
        flash, set_flash, FLASH: u8 @ 0x43;
    }
}

view! {
    /// SET 15, variants 0 to 6: the speed words of the scrolling (8.8), over the timer bytes of `ScrollScreenObj`.
    ScrollSpeedObj;
}

record! {
    impl ScrollSpeedObj {
        /// Horizontal speed (8.8).
        speed_x, set_speed_x, SPEED_X: u16 @ 0x14;
        /// Vertical speed (8.8).
        speed_y, set_speed_y, SPEED_Y: u16 @ 0x18;
    }
}

view! {
    /// The conductor of the camera script of SET 15 variant 3: the row of its script.
    BossCameraFields;
}

record! {
    impl BossCameraFields {
        /// Row of the camera table (h).
        camera_row, set_camera_row, CAMERA_ROW: u8 @ 0x3f;
    }
}

view! {
    /// SET 15, variants 8, 9, 11 and 23 (`later`): hit points refill, player effects and handover.
    RefillObj;
}

record! {
    impl RefillObj {
        /// Step of the four-component shake table.
        step, set_step, STEP: u8 @ 0x18;
        /// Phase of the handover (0 to 2).
        phase, set_phase, PHASE: u8 @ 0x30;
        /// Mark given to the next effect (h).
        mark, set_mark, MARK: u8 @ 0x31;
        /// Frames before the next step; for the refill, the bonus flag.
        delay, set_delay, DELAY: u16 @ 0x42;
        /// Player or character of the effect (h).
        target, set_target, TARGET: u16 @ 0x50;
        /// Screen object that created the effect (low word of its address).
        creator, set_creator, CREATOR: u16 @ 0x5c;
    }
}

view! {
    /// SET 15, variants 18 and 19 (`rewards`): end-of-level bonuses, one player after the other.
    RewardObj;
}

record! {
    impl RewardObj {
        /// Bits of the bonuses already given.
        taken, set_taken, TAKEN: u32 @ 0x20;
        /// Candidate examined.
        candidate, set_candidate, CANDIDATE: u8 @ 0x24;
        /// Players still to serve.
        left, set_left, LEFT: u8 @ 0x25;
        /// Player served (low word of its record address).
        player, set_player, PLAYER: u16 @ 0x26;
        /// Frames before the next pulse of the player's palette.
        pulse_tick, set_pulse_tick, PULSE_TICK: u8 @ 0x2a;
        /// Pulses left.
        pulses, set_pulses, PULSES: u8 @ 0x2b;
        /// Reward object shown (low word of its address).
        reward, set_reward, REWARD: u16 @ 0x46;
        /// Owner of the reward object.
        owner, set_owner, OWNER: u16 @ 0x46;
        /// Value shown by the countdown digits (variant 19).
        value, set_value, VALUE: u32 @ 0x4c;
    }
}

view! {
    /// SET 15 variant 20 (`credits_scroll`): the scrolling text.
    CreditsObj;
}

record! {
    impl CreditsObj {
        /// Vertical scroll (16.16).
        scroll, set_scroll, SCROLL: u32 @ 0x20;
        /// Next byte of the text.
        text, set_text, TEXT: u32 @ 0x24;
        /// Column written in the current line.
        column, set_column, COLUMN: u8 @ 0x29;
        /// Phase of the line writing (0 or 8).
        phase, set_phase, PHASE: u8 @ 0x2a;
        /// Offset of the next cell in the tile map.
        dest, set_dest, DEST: u16 @ 0x2c;
        /// Attribute of the next cells.
        attr, set_attr, ATTR: u8 @ 0x2e;
    }
}

view! {
    /// SET 15 variant 21 (`flicker`): a twinkling palette color.
    FlickerObj;
}

record! {
    impl FlickerObj {
        /// The color is lit.
        lit, set_lit, LIT: u8 @ 0x20;
        /// Color saved before lighting.
        color, set_color, COLOR: u16 @ 0x22;
    }
}

view! {
    /// SET 15 variant 22 (`name_entry` of the screens): the players present, one after the other.
    NameShowObj;
}

record! {
    impl NameShowObj {
        /// Player examined (low word of its record address).
        player, set_player, PLAYER: u16 @ 0x20;
        /// Players left to examine.
        left, set_left, LEFT: u8 @ 0x22;
        /// Name entry object created (low word of its address).
        cursor, set_cursor, CURSOR: u16 @ 0x24;
        /// Frames before the next player.
        delay, set_delay, DELAY: u16 @ 0x26;
    }
}

view! {
    /// SET 15 variant 24 (`results_panel`).
    ResultsObj;
}

record! {
    impl ResultsObj {
        /// Fade countdown (h).
        fade, set_fade, FADE: u8 @ 0x20;
        /// Palette copied (eight longs).
        palette, set_palette, PALETTE: u32 @ 0x30;
    }
}

view! {
    /// SET 15 variant 10 (`shaking_intro`) and 15 (`quake_light`).
    IntroObj;
}

record! {
    impl IntroObj {
        /// Horizontal speed of the background (8.8).
        speed, set_speed, SPEED: u16 @ 0x14;
        /// Acceleration of the background.
        accel, set_accel, ACCEL: u16 @ 0x16;
        /// Step of the shake table.
        step, set_step, STEP: u8 @ 0x18;
    }
}

view! {
    /// SET 15 variants 13 (`fireworks`) and 14 (`lightning`).
    FireworksObj;
}

record! {
    impl FireworksObj {
        /// Bits of the lightning bolts already drawn (bit 0: started).
        used, set_used, USED: u8 @ 0x30;
        /// The fireworks have started.
        started, set_started, STARTED: u8 @ 0x42;
    }
}

view! {
    /// SET 15 variants 16 (`ranking_start`) and 17 (`palette_fade`).
    RankingObj;
}

record! {
    impl RankingObj {
        /// Bits of the players already shown; for the fade, frames before the next step.
        taken, set_taken, TAKEN: u16 @ 0x20;
        /// Steps left in the fade.
        steps, set_steps, STEPS: u8 @ 0x21;
        /// Order of the next effect.
        order, set_order, ORDER: u8 @ 0x22;
        /// Effects shown.
        shown, set_shown, SHOWN: u8 @ 0x23;
        /// Frames before the next effect.
        delay, set_delay, DELAY: u16 @ 0x24;
        /// Frames before the end of the fade.
        wait, set_wait, WAIT: u16 @ 0x42;
        /// Object notified at the end (low word of its address).
        target, set_target, TARGET: u16 @ 0x46;
    }
}

view! {
    /// SET 15 variant 7 (`status_window`): the level-up window of a player.
    StatusObj;
}

record! {
    impl StatusObj {
        /// Player shown (low word of its record address).
        player, set_player, PLAYER: u16 @ 0x34;
        /// Levels gained over the minimum of its range.
        level_step, set_level_step, LEVEL_STEP: u8 @ 0x36;
        /// Half of the class table (h).
        half, set_half, HALF: u8 @ 0x37;
    }
}

view! {
    /// SET 2 (`scene`): the scene controller, fields beyond the constants of its module.
    SceneObj;
    /// The conductor of a SET 15 screen (a boss or a SET object), as the screens read it.
    Conductor;
}

record! {
    impl SceneObj {
        /// Frames before the curtain moves on.
        curtain_delay, set_curtain_delay, CURTAIN_DELAY: u16 @ 0x14;
    }
}

record! {
    impl Conductor {
        /// Signal byte to the screens (1, 2: steps reached; negative: finished) (h).
        signal, set_signal, SIGNAL: u8 @ 0x08;
    }
}

view! {
    /// An effect created by a SET 15 screen (handover, ranking), numbered by the screen.
    ScreenEffect;
}

record! {
    impl ScreenEffect {
        /// Order or mark given by the screen.
        order, set_order, ORDER: u8 @ 0x42;
    }
}
