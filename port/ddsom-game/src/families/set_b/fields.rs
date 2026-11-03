//! Typed views of the SET B objects (types 16 to 32), declared once with `record!` (see `set::fields`). A guessed
//! meaning is marked (h).

use crate::families::set::fields::view;
use crate::record::record;

view! {
    /// A passage door (SET 7 `set::exit_gate`, SET 27 `door_a`, SET 29 `door_b`, SET 25 `exit_gate`): the players'
    /// counters and the scene state saved during a passage (camera, scroll, music).
    DoorObj;
}

record! {
    impl DoorObj {
        /// Saved `vars::SCROLL_STOP`.
        scroll_stop_saved, set_scroll_stop_saved, SCROLL_STOP_SAVED: u16 @ 0x14;
        /// Number of players entered.
        arrived, set_arrived, ARRIVED: u8 @ 0x16;
        /// Bits of the players entered.
        entered, set_entered, ENTERED: u8 @ 0x17;
        /// Saved `vars::SCROLL_MODE`.
        scroll_mode_saved, set_scroll_mode_saved, SCROLL_MODE_SAVED: u16 @ 0x18;
        /// Bits of the players already counted.
        counted, set_counted, COUNTED: u8 @ 0x1a;
        /// Non-zero: the depth is not checked to be in front of the door.
        ignore_depth, set_ignore_depth, IGNORE_DEPTH: u8 @ 0x1b;
        /// Saved `vars::MUSIC`, played again on the way back.
        music, set_music, MUSIC: u16 @ 0x1c;
        /// Saved `vars::CAMERA_MAX_X`.
        camera_max_x_saved, set_camera_max_x_saved, CAMERA_MAX_X_SAVED: u16 @ 0x22;
        /// Saved flag of the next scene.
        next_scene_flag_saved, set_next_scene_flag_saved, NEXT_SCENE_FLAG_SAVED: u8 @ 0x25;
        /// Saved `vars::TRANSITION`.
        transition_saved, set_transition_saved, TRANSITION_SAVED: u8 @ 0x26;
        /// Saved cameras: main X, Y, alternative X, Y (four longs).
        cameras_saved, set_cameras_saved, CAMERAS_SAVED: u32 @ 0x30;
        /// Frames spent in front of the door, one byte per player.
        near_counters, set_near_counters, NEAR_COUNTERS: u8 @ 0x42;
        /// Input bit that makes a player enter.
        key, set_key, KEY: u8 @ 0x46;
        /// Exit kind (0 annex zone, 1 return, 2 next scene, 3 end of game).
        mode, set_mode, MODE: u8 @ 0x48;
        /// Saved stage flag.
        stage_flag_saved, set_stage_flag_saved, STAGE_FLAG_SAVED: u8 @ 0x49;
        /// Saved scroll state.
        scroll_state_saved, set_scroll_state_saved, SCROLL_STATE_SAVED: u32 @ 0x4a;
        /// Scene loaded by the passage.
        next_scene, set_next_scene, NEXT_SCENE: u8 @ 0x4f;
    }
}

view! {
    /// The marker object that a passage door creates over an entering player (SET of variant 0x8b).
    PlayerMarker;
}

record! {
    impl PlayerMarker {
        /// Frames before it disappears (h).
        lifetime, set_lifetime, LIFETIME: u8 @ 0x42;
        /// Character of the player (palette of the marker).
        character, set_character, CHARACTER: u8 @ 0x43;
    }
}

view! {
    /// SET 31 (`zone_end`): end of a zone, waits for its event object before unlocking the camera.
    ZoneEndObj;
}

record! {
    impl ZoneEndObj {
        /// Event object created (low word of its address).
        event, set_event, EVENT: u16 @ 0x10;
        /// Frames before the next step.
        delay, set_delay, DELAY: u16 @ 0x14;
        /// The event has started (h).
        started, set_started, STARTED: u8 @ 0x22;
        /// The event is still running (h).
        running, set_running, RUNNING: u8 @ 0x23;
    }
}

view! {
    /// SET 25 (`exit_gate` of set_b): the exit door at the end of a level, fields beyond `DoorObj`.
    ExitGateBObj;
}

record! {
    impl ExitGateBObj {
        /// Number of players entered, added to the level count at the end.
        entered_count, set_entered_count, ENTERED_COUNT: u8 @ 0x24;
        /// Non-zero: the door does nothing.
        disabled, set_disabled, DISABLED: u8 @ 0x2f;
    }
}

view! {
    /// SET 16 (`bumper`): a trampoline.
    BumperObj;
}

record! {
    impl BumperObj {
        /// Frames before the trampoline lock is released.
        lock_delay, set_lock_delay, LOCK_DELAY: u16 @ 0x14;
        /// Box set of the contact zones.
        box_set, set_box_set, BOX_SET: u8 @ 0x2e;
        /// Player launched (h).
        player, set_player, PLAYER: u32 @ 0x34;
        /// Trampoline kind (launch speeds).
        kind, set_kind, KIND: u8 @ 0x42;
    }
}

view! {
    /// A player launched by a trampoline: speeds and accelerations written by `bumper`.
    LaunchedPlayer;
}

record! {
    impl LaunchedPlayer {
        /// Speed in X (8.8).
        vx, set_vx, VX: u16 @ 0x14;
        /// Acceleration in X.
        ax, set_ax, AX: u16 @ 0x16;
        /// Speed in Z.
        vz, set_vz, VZ: u16 @ 0x18;
        /// Acceleration in Z.
        az, set_az, AZ: u16 @ 0x1a;
        /// Speed in Y (depth).
        vy, set_vy, VY: u16 @ 0x1c;
        /// Acceleration in Y (drag).
        drag, set_drag, DRAG: u16 @ 0x1e;
    }
}

view! {
    /// SET 30 (`game_over`).
    GameOverObj;
}

record! {
    impl GameOverObj {
        /// Frames before the fade.
        wait, set_wait, WAIT: u16 @ 0x4c;
    }
}

view! {
    /// SET 26 (`idle_hunter`): pursuers sent after the motionless players.
    IdleHunterObj;
}

record! {
    impl IdleHunterObj {
        /// Last pursuer created for each player (low word of its address; four words).
        hunters, set_hunters, HUNTERS: u16 @ 0x08;
        /// Reference position of each player (four words).
        marks, set_marks, MARKS: u16 @ 0x10;
        /// Frames each player has stayed motionless (four bytes).
        idle, set_idle, IDLE: u8 @ 0x18;
        /// Frames of immobility that attract pursuers.
        idle_limit, set_idle_limit, IDLE_LIMIT: u8 @ 0x1c;
        /// Frames before the hunt starts.
        warmup, set_warmup, WARMUP: u16 @ 0x1e;
    }
}

view! {
    /// SET 28 (`level_clock`).
    LevelClockObj;
}

record! {
    impl LevelClockObj {
        /// Camera abscissa at the start.
        start_x, set_start_x, START_X: u16 @ 0x1e;
    }
}

view! {
    /// SET 23 (`rearguard`).
    RearguardObj;
}

record! {
    impl RearguardObj {
        /// Camera abscissa at the start.
        start_x, set_start_x, START_X: u16 @ 0x0a;
    }
}

view! {
    /// SET 18 (`scroll_to`): scrolls the alternative camera down to a target, then unlocks its owner.
    ScrollToObj;
}

record! {
    impl ScrollToObj {
        /// Objects of the controller still alive.
        live, set_live, LIVE: u16 @ 0x2a;
        /// The controller waits for an external unlock.
        wait_flag, set_wait_flag, WAIT_FLAG: u8 @ 0x2e;
        /// Target ordinate of the camera.
        target, set_target, TARGET: u16 @ 0x42;
        /// Scene controller to unlock (low word of its address).
        owner, set_owner, OWNER: u16 @ 0x46;
    }
}

view! {
    /// SET 32 (`test_selector`).
    TestSelectorObj;
}

record! {
    impl TestSelectorObj {
        /// Selected entry.
        choice, set_choice, CHOICE: u8 @ 0x2f;
    }
}

view! {
    /// SET 20 (`continue_countdown`): the countdown before the end of the game, minutes and seconds in decimal digits.
    ContinueObj;
}

record! {
    impl ContinueObj {
        /// Minutes left.
        minutes, set_minutes, MINUTES: u8 @ 0x14;
        /// Tens of seconds left.
        tens, set_tens, TENS: u8 @ 0x15;
        /// Units of seconds left.
        units, set_units, UNITS: u8 @ 0x16;
        /// Frames left in the current second.
        tick, set_tick, TICK: u8 @ 0x17;
        /// The announcement is shown.
        shown, set_shown, SHOWN: u8 @ 0x18;
        /// Frames before the inscription blinks.
        blink, set_blink, BLINK: u8 @ 0x19;
        /// Bit 0: phase of the blinking inscription.
        blink_phase, set_blink_phase, BLINK_PHASE: u8 @ 0x1a;
        /// Number of players alive known.
        players, set_players, PLAYERS: u8 @ 0x1b;
        /// Last value of the suspension counter `Obj::HIDDEN`.
        last_suspend, set_last_suspend, LAST_SUSPEND: u8 @ 0x1c;
        /// Orientation of the invitation (parameter by stage).
        prompt, set_prompt, PROMPT: u8 @ 0x2c;
        /// Minutes at the start (script parameter).
        start_minutes, set_start_minutes, START_MINUTES: u8 @ 0x42;
        /// Seconds at the start, in packed decimal (script parameter).
        start_seconds, set_start_seconds, START_SECONDS: u8 @ 0x43;
        /// Frames before the next announcement.
        delay, set_delay, DELAY: u16 @ 0x4c;
        /// Camera abscissa noted at the last announcement.
        camera_x, set_camera_x, CAMERA_X: u16 @ 0x4e;
    }
}

view! {
    /// A camera of the work RAM (`A5+0x4fe` main, `A5+0x54e` alternative), as `continue_countdown` reads it.
    CameraFields;
}

record! {
    impl CameraFields {
        /// Scroll mode of the camera (4: stopped) (h).
        mode, set_mode, MODE: u16 @ 0x20;
    }
}

view! {
    /// SET 21 (`life_bar`): the life gauge of a boss or of two linked objects, drawn in the tile map.
    LifeBarObj;
}

record! {
    impl LifeBarObj {
        /// Object whose life is shown (low word of its address).
        owner, set_owner, OWNER: u16 @ 0x20;
        /// Address in the tile map of the current end of the gauge.
        cursor, set_cursor, CURSOR: u32 @ 0x22;
        /// Points of the current piece.
        piece, set_piece, PIECE: u8 @ 0x26;
        /// Points of the last piece of the maximum.
        last_piece, set_last_piece, LAST_PIECE: u8 @ 0x27;
        /// Life drawn.
        shown, set_shown, SHOWN: u16 @ 0x2a;
        /// Maximum life, rounded down to whole pieces.
        max_full, set_max_full, MAX_FULL: u16 @ 0x2c;
        /// Maximum life (sum of the linked objects).
        max, set_max, MAX: u16 @ 0x30;
        /// Current life (sum of the linked objects).
        life, set_life, LIFE: u16 @ 0x32;
        /// Second object whose life is added (variant non-zero).
        second, set_second, SECOND: u16 @ 0x34;
    }
}

view! {
    /// An object whose life a `LifeBarObj` shows (a boss, or a player in the bonus duels): its hit points.
    LifeOwner;
}

record! {
    impl LifeOwner {
        /// Maximum and current hit points.
        hp_max, set_hp_max, HP_MAX: u16 @ 0x60;
        hp, set_hp, HP: u16 @ 0x62;
    }
}

view! {
    /// SET 24 (`dialog`): a dialog window. The fields from `+0x18` to `+0x34` come from its definition (`data/dialogs.json`).
    DialogObj;
}

record! {
    impl DialogObj {
        /// Address in the tile map of the frame being drawn.
        screen_at, set_screen_at, SCREEN_AT: u32 @ 0x14;
        /// Low word of `SCREEN_AT` (the frame moves down by adding to it).
        screen_at_low, set_screen_at_low, SCREEN_AT_LOW: u16 @ 0x16;
        /// Small frame (definition).
        small, set_small, SMALL: u8 @ 0x18;
        /// Number of lines (definition).
        lines, set_lines, LINES: u8 @ 0x19;
        /// The next definition follows (definition: `>= 0`).
        next, set_next, NEXT: u8 @ 0x1a;
        /// Frames before the frame grows by one more line.
        tick, set_tick, TICK: u8 @ 0x1b;
        /// Address of the next definition.
        next_def, set_next_def, NEXT_DEF: u32 @ 0x1c;
        /// Height of the frame (0xb small, 0xf otherwise; 0xff: no frame).
        height, set_height, HEIGHT: u8 @ 0x20;
        /// Lines still to draw.
        lines_left, set_lines_left, LINES_LEFT: u8 @ 0x21;
        /// The window is closing (h).
        closing, set_closing, CLOSING: u8 @ 0x22;
        /// Index of the text by player (kind 11).
        player_text, set_player_text, PLAYER_TEXT: u8 @ 0x23;
        /// Text, or list of definitions for the kinds that choose one (definition).
        text, set_text, TEXT: u32 @ 0x24;
        /// Kind of window (definition).
        kind, set_kind, KIND: u8 @ 0x2f;
        /// Number of tokens written in the buffer.
        tokens, set_tokens, TOKENS: u16 @ 0x30;
        /// Frames to wait once the text is shown (0xffff: until a key; definition).
        wait, set_wait, WAIT: u16 @ 0x32;
        /// Parameter of the kind (definition).
        setting, set_setting, SETTING: u16 @ 0x34;
        /// Frames per line of growth.
        speed, set_speed, SPEED: u8 @ 0x36;
        /// Player whose keys close the window (negative: any).
        listener, set_listener, LISTENER: u8 @ 0x37;
        /// Offset of the text engine cell used.
        slot_offset, set_slot_offset, SLOT_OFFSET: u8 @ 0x38;
        /// Text engine cell (low word of its address).
        slot, set_slot, SLOT: u16 @ 0x3a;
        /// Choice of the definition (kinds 5 and 19; script parameter).
        choice, set_choice, CHOICE: u8 @ 0x42;
        /// Player who speaks (variant 0x8b; script parameter).
        speaker, set_speaker, SPEAKER: u8 @ 0x43;
        /// Object that opened the window (low word of its address).
        owner, set_owner, OWNER: u16 @ 0x46;
    }
}

view! {
    /// An effect tied to a dialog window (the vote of SET 24, the EFFCT 23 of SET 3), which closes it.
    WindowEffect;
}

record! {
    impl WindowEffect {
        /// The window to close (low word of its address): `Obj::TIED`.
        window, set_window, WINDOW: u16 @ crate::objects::Obj::TIED;
    }
}
