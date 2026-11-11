//! Fields of the effect objects (c), declared once ([`record!`]): the offsets (`Ballot::VOTERS`) and the typed
//! accessors (`ballot.voters(b)`). A meaning guessed from the code alone is marked (h). Names from docs/familles.md.

use crate::world::objects::Obj;
use crate::world::record::record;

/// Vote for the next path (type 23, variant 0): the ballot that counts the votes of the cursors.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ballot(pub Obj);

impl Ballot {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Ballot {
        /// Mode: the set of choices (`vote_choices`, `vote_cursor_places`).
        mode, set_mode, MODE: u8 @ 0x2f;
        /// Address of the text of the winning choice in the text layer.
        text_at, set_text_at, TEXT_AT: u32 @ 0x30;
        /// Number of characters of the winning choice, minus one.
        text_size, set_text_size, TEXT_SIZE: u16 @ 0x34;
        /// Blinks of the winning choice still to show.
        blinks, set_blinks, BLINKS: u8 @ 0x36;
        /// Frames before the next blink step.
        blink_timer, set_blink_timer, BLINK_TIMER: u8 @ 0x37;
        /// The text of the winning choice is shown.
        text_shown, set_text_shown, TEXT_SHOWN: u8 @ 0x38;
        /// Players who have not voted yet.
        voters, set_voters, VOTERS: u8 @ 0x42;
        /// Number of the last choice.
        last_choice, set_last_choice, LAST_CHOICE: u8 @ 0x43;
        /// Cursors still alive.
        cursors, set_cursors, CURSORS: u8 @ 0x44;
        /// The votes are counted.
        counted, set_counted, COUNTED: u8 @ 0x53;
        /// Time left to vote (word, set by the countdown; zero: time up).
        duration, set_duration, DURATION: u16 @ 0x54;
        /// Votes per choice (4 bytes), then the winning choice.
        votes, set_votes, VOTES: u32 @ 0x58;
    }
}

/// Barrier controlled by switches (type 88), seen by its switches.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BarrierView(pub Obj);

impl BarrierView {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl BarrierView {
        /// Non-zero: the barrier is open (h).
        open, set_open, OPEN: u8 @ 0x5a;
    }
}

/// Marker that blinks slowly (type 47).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Beacon(pub Obj);

impl Beacon {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Beacon {
        /// Frames before the next blink (word).
        blink_timer, set_blink_timer, BLINK_TIMER: u16 @ 0x5c;
    }
}

/// A player's body in a shop (the object designated by the buyers list `0xff2aa4`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Buyer(pub Obj);

impl Buyer {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Buyer {
        /// Player record (16-bit pointer).
        player, set_player, PLAYER: u16 @ 0x50;
        /// Purchase request: positive to buy, 0xff once handled (h).
        purchase, set_purchase, PURCHASE: u8 @ 0x58;
    }
}

/// Panels of an end-of-trial screen (types 37 and 38).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Card(pub Obj);

impl Card {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Card {
        /// Creator of the panel (16-bit pointer).
        creator, set_creator, CREATOR: u16 @ 0x42;
    }
}

/// Head of a chain (the object that carries the links of type 12).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChainHead(pub Obj);

impl ChainHead {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl ChainHead {
        /// Links still attached.
        links, set_links, LINKS: u8 @ 0x42;
        /// Carrier of the chain (16-bit pointer).
        carrier, set_carrier, CARRIER: u16 @ 0x54;
        /// Last direction of the head (most recent of the history).
        history, set_history, HISTORY: u8 @ 0x58;
    }
}

/// Object that follows a path (type 65).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Courier(pub Obj);

impl Courier {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Courier {
        /// Frames left for the current direction (byte), then delay before the spawn (word).
        timer, set_timer, TIMER: u8 @ 0x42;
        /// Link given to the object spawned (16-bit pointer).
        spawn_link, set_spawn_link, SPAWN_LINK: u16 @ 0x46;
        /// Address of the next pair of `courier_path`.
        path, set_path, PATH: u32 @ 0x5c;
    }
}

/// Platform that carries the players away (type 34).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ferry(pub Obj);

impl Ferry {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Ferry {
        /// Speed (long) (h).
        speed, set_speed, SPEED: u32 @ 0x58;
        /// Scenery whose stop ends the ride (16-bit pointer).
        scenery, set_scenery, SCENERY: u16 @ 0x5c;
    }
}

/// Floating object of an introduction scene (type 36).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Floater(pub Obj);

impl Floater {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Floater {
        /// Creator (16-bit pointer).
        creator, set_creator, CREATOR: u16 @ 0x5c;
    }
}

/// Four large characters in background 2 (type 66).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Letters(pub Obj);

impl Letters {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Letters {
        /// The blocks are cleared while the object is hidden.
        cleared, set_cleared, CLEARED: u8 @ 0x41;
        /// Frames before the next step of the script.
        timer, set_timer, TIMER: u16 @ 0x42;
        /// Address of the next step in `letters_script`.
        script, set_script, SCRIPT: u32 @ 0x5c;
    }
}

/// An object released by an effect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Linked(pub Obj);

impl Linked {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Linked {
        /// 0xff: released by its creator.
        released, set_released, RELEASED: u8 @ 0x56;
    }
}

/// Column that rises under an object (type 67).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pillar(pub Obj);

impl Pillar {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Pillar {
        /// Object it rises under (16-bit pointer).
        base, set_base, BASE: u16 @ 0x50;
        /// Delay before rising.
        delay, set_delay, DELAY: u8 @ 0x58;
    }
}

/// Gold donation cursor of the shop (type 20).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Purse(pub Obj);

impl Purse {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Purse {
        /// Length of the life gauge drawn.
        gauge_len, set_gauge_len, GAUGE_LEN: u16 @ 0x18;
    }
}

/// Creature that springs up (type 59).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Riser(pub Obj);

impl Riser {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Riser {
        /// Carrier of the creature (16-bit pointer).
        carrier, set_carrier, CARRIER: u16 @ 0x42;
    }
}

/// Scene timer (type 76).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SceneTimer(pub Obj);

impl SceneTimer {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl SceneTimer {
        /// Delay before the transition (word, in place of the animation timer).
        delay, set_delay, DELAY: u16 @ 0x30;
    }
}

/// Screen element fixed to the camera (types 3 and 91).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ScreenPiece(pub Obj);

impl ScreenPiece {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl ScreenPiece {
        /// Height on the screen, the shake of plane 1 subtracted.
        height, set_height, HEIGHT: u16 @ 0x50;
    }
}

/// Shop counter (type 22) and counter of a scene shop (type 70): sales zone around the object.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shop(pub Obj);

impl Shop {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Shop {
        /// Sales zone: x offset from the object.
        zone_dx, set_zone_dx, ZONE_DX: u16 @ 0x14;
        /// Sales zone: half-width.
        zone_half_width, set_zone_half_width, ZONE_HALF_WIDTH: u16 @ 0x16;
        /// Sales zone: depth offset from the object.
        zone_dy, set_zone_dy, ZONE_DY: u16 @ 0x54;
        /// Sales zone: half-depth.
        zone_half_depth, set_zone_half_depth, ZONE_HALF_DEPTH: u16 @ 0x56;
    }
}

/// Shop counter, seen as the BCD price paid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ShopPrice(pub Obj);

impl ShopPrice {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl ShopPrice {
        /// End of the six BCD digits of the price (bytes 0x4e-0x53).
        price_end, set_price_end, PRICE_END: u8 @ 0x54;
    }
}

/// Effects of a magician enemy (type 44).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spell(pub Obj);

impl Spell {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Spell {
        /// Step of the palette cycle (0 to 3).
        tint_step, set_tint_step, TINT_STEP: u8 @ 0x46;
    }
}

/// Barrier switch (type 87), seen by its barrier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwitchView(pub Obj);

impl SwitchView {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl SwitchView {
        /// Bit 0: touched.
        pressed, set_pressed, PRESSED: u8 @ 0x30;
    }
}

/// Object thrown at random (type 41).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Toss(pub Obj);

impl Toss {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Toss {
        /// Frames of the bounce (word).
        bounce_timer, set_bounce_timer, BOUNCE_TIMER: u16 @ 0x5c;
    }
}

/// The object that creates the vote (scenery) and waits for its result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VoteCreator(pub Obj);

impl VoteCreator {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl VoteCreator {
        /// Winning choice.
        result, set_result, RESULT: u8 @ 0x23;
        /// Set to 1 at the end of the vote.
        done, set_done, DONE: u16 @ 0x32;
    }
}

/// Vote, variant 1: the cursor of a player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VoteCursor(pub Obj);

impl VoteCursor {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl VoteCursor {
        /// Drawn with its own palette.
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        /// Palette (0x12 + player).
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Player (number of the character slot).
        player, set_player, PLAYER: u8 @ 0x2f;
        /// Choice under the cursor.
        choice, set_choice, CHOICE: u8 @ 0x42;
        /// Number of the last choice.
        last_choice, set_last_choice, LAST_CHOICE: u8 @ 0x43;
    }
}

/// Vote, variant 2: the countdown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VoteTimer(pub Obj);

impl VoteTimer {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl VoteTimer {
        /// Drawn with its own palette.
        own_palette, set_own_palette, OWN_PALETTE: u8 @ 0x2a;
        /// Palette (0xe).
        palette, set_palette, PALETTE: u8 @ 0x2b;
        /// Attribute byte of the current sprite frame (h).
        frame_attribute, set_frame_attribute, FRAME_ATTRIBUTE: u8 @ 0x3f;
        /// Seconds left (high byte of the word `0x143c` set at the start).
        seconds, set_seconds, SECONDS: u8 @ 0x54;
        /// Frames left in the current second.
        frames, set_frames, FRAMES: u8 @ 0x55;
    }
}

/// Flame wisp of the ending sequence (type 86).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Wisp(pub Obj);

impl Wisp {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Wisp {
        /// Creator (16-bit pointer).
        creator, set_creator, CREATOR: u16 @ 0x5c;
    }
}
