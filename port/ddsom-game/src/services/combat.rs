//! Collisions and hit resolution (subsystems X1 and X2 of the atlas).
//!
//! Each frame, `combat_update` (0x260ee) confronts the boxes of the visible objects: the per-category lists
//! filled by `sprites::check_visible` (FFFE72.. and arrays FFFE86..) are walked
//! in (attacker, victim) pairs; a pair whose boxes overlap triggers the application
//! of the hit (states, hit points, sparks, sounds). See `docs/combat.md` for the formats.
//!
//! The logic follows the original instruction by instruction (same RAM reads/writes,
//! same 8/16-bit arithmetic) but is expressed on typed views of the objects. The subroutines
//! outside the scope (effect allocation, sounds, gauge display, RNG...) are called directly.

use crate::world::objects::Enemy;
use crate::actors::player::Player;
use crate::world::World;

use crate::core::dispatch::Leftovers;
use crate::world::mem::{rd16, rd8, wr16, wr8, A5};
pub use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::actors::player::tables::{box_bytes, long};

/// Call counters of the rewritten routines (validation coverage).
pub(crate) mod cov {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;

    static ON: AtomicBool = AtomicBool::new(false);
    static HITS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

    pub fn hit(name: &'static str) {
        if ON.load(Ordering::Relaxed) {
            *HITS.lock().unwrap().entry(name).or_default() += 1;
        }
    }
    pub fn enable() {
        ON.store(true, Ordering::Relaxed);
    }
    pub fn report() -> Vec<(&'static str, u64)> {
        HITS.lock().unwrap().iter().map(|(k, v)| (*k, *v)).collect()
    }
}

/// Enables counting the calls of the internal routines (tool `combat_coverage`).
pub use cov::enable as enable_coverage;
/// Number of calls of each internal routine since activation.
pub use cov::report as coverage;

/// Debug trace (variable DDSOM_TRACE).
macro_rules! trace {
    ($($a:tt)*) => { if std::env::var("DDSOM_TRACE").is_ok() { eprintln!($($a)*); } };
}

pub(crate) mod contact;
pub(crate) mod damage;
pub(crate) mod shots;

use contact::*;
use damage::*;
use shots::*;

/// A5 variables (RAM FF8000) read by the collisions (the variables shared with the players are in
/// `player::ram`).
mod ram {
    use crate::world::mem::A5;
    use crate::world::ram::{Word, Byte};

    pub use crate::actors::player::ram::{
        FROZEN, INPUT_RAW as SCRIPTED_A, KILL_FLAG, PAUSE, PLAYERS_ALIVE, SCRIPTED_B, TICK, DSW,
    };
    /// Difficulty (> 5: strengthened enemy attacks).
    pub const DIFFICULTY: Byte = Byte::a5(0x48);
    /// Game damage setting (0, 2, 4, 6: enemy attack scale table).
    pub const DAMAGE_SETTING: Word = Word::a5(0x70);
    /// Current inputs (one byte per player) and those of the previous frame (`FF8000`, `FF8006`).
    pub const INPUTS: u32 = A5;
    pub const INPUTS_PREV: u32 = A5 + 6;
}

/// Loop counters that the original keeps in RAM (FF1A62..): their final value is visible.
mod scratch {
    pub const PLAYER_HITS_ENEMY: u32 = 0xff1a62;
    pub const SHOTS_OUTER: u32 = 0xff1a64;
    pub const SHOTS_INNER: u32 = 0xff1a66;
    pub const ENEMY_ATTACKS: u32 = 0xff1a68;
    pub const PUSH: u32 = 0xff1a6a;
    pub const CLASH: u32 = 0xff1a6c;
    pub const BOX_ATTACKS: u32 = 0xff1a6e;
    pub const ITEMS: u32 = 0xff1a70;
}

/// Fields of an object (player, enemy, shot, crate, item). The names marked (h) are
/// guesses drawn from usage.
mod f {
    /// Byte: 0 free; bit 0 active; 2 = dying / consumed (no longer participates).
    pub const ACTIVE: u32 = 0x00;
    pub const FLAGS: u32 = 0x01;
    /// Object type (shot: contact behaviour no.; player: player index).
    pub const KIND: u32 = 0x02;
    /// Variant; for a player, `VARIANT >> 1` = class (0 fighter .. 5 thief).
    pub const VARIANT: u32 = 0x03;
    /// Long: main state, substate, action and byte 7 (see `state`).
    pub const STATE: u32 = 0x04;
    pub const SUBSTATE: u32 = 0x05;
    pub const ACTION: u32 = 0x06;
    pub const X: u32 = 0x08;
    /// Height above the ground.
    pub const Z: u32 = 0x0c;
    /// Depth (sprite sort key).
    pub const Y: u32 = 0x10;
    pub const VX: u32 = 0x14;
    /// Pool class: 2 enemy, 4 boss, 6 player shot, 8 enemy shot...
    pub const CLASS: u32 = 0x28;
    /// Non-zero: the object faces left (the X offsets of the boxes are inverted).
    pub const FLIP: u32 = 0x2c;
    pub const FACING_AUX: u32 = 0x2d;
    /// Box set (index into the box pointer tables).
    pub const BOX_SET: u32 = 0x2e;
    pub const BODY_BOX: u32 = 0x38;
    pub const PUSH_BOX: u32 = 0x39;
    pub const ATTACK_BOX: u32 = 0x3a;
    pub const ATTACK_KIND: u32 = 0x3c;
    pub const ATTACK_ID: u32 = 0x3e;
    /// Non-zero: the object is inert for collisions.
    pub const BOXES_OFF: u32 = 0x40;
    pub const ATTACK_ID_ENEMY: u32 = 0x44;
    /// Attack power: A (against ordinary enemies), B (against bosses) (h).
    pub const POWER_A: u32 = 0x45;
    pub const POWER_B: u32 = 0x46;
    pub const HIT_BY_KIND: u32 = 0x48;
    pub const REACTION: u32 = 0x49;
    pub const REACTION_FLIP: u32 = 0x4a;
    pub const HIT_BY_VARIANT: u32 = 0x4b;
    pub const OWNER: u32 = 0x50;
    pub const HITSTOP: u32 = 0x53;
    pub const LINK: u32 = 0x54;
    pub const HITSTOP_VICTIM: u32 = 0x56;
    pub const HIT_FLAGS: u32 = 0x57;
    /// "Last hit received" bytes per player (enemy), effect bits (player).
    pub const HIT_MARK: u32 = 0x58;
    pub const COMBO_MARK: u32 = 0x5b;
    pub const HP_MAX: u32 = 0x60;
    pub const HP: u32 = 0x62;
    pub const HP_PREV: u32 = 0x64;
    pub const SUBACTION: u32 = 0x6a;
    pub const KEYS_HELD: u32 = 0x6c;
    pub const KEYS_NEW: u32 = 0x6e;
    pub const RECOIL: u32 = 0x70;
    pub const PREV_ACTION: u32 = 0x6f;
    pub const INVULN: u32 = 0x7e;
    pub const FLIES: u32 = 0x86;
    pub const STAGGER_DELAY: u32 = 0x87;
    pub const PREV_STATE: u32 = 0x8a;
    pub const DEATH_CREDIT: u32 = 0x93;
    pub const IMMUNITY: u32 = 0x94;
    pub const DEATH_VALUE: u32 = 0x9a;
    pub const HITS_TAKEN: u32 = 0x9c;
    pub const WEAPON_10D: u32 = 0x10d;
    pub const WEAPON_10E: u32 = 0x10e;
    pub const ARMOR: u32 = 0x160;
    pub const GEAR_163: u32 = 0x163;
    pub const GEAR_165: u32 = 0x165;
    pub const GEAR_166: u32 = 0x166;
    pub const GEAR_167: u32 = 0x167;
    pub const GEAR_184: u32 = 0x184;
    pub const KILLS_FLAG: u32 = 0x18a;
    pub const HITS_LANDED: u32 = 0x190;
    pub const DYING: u32 = 0x196;
    pub const DAMAGE_CAP: u32 = 0x16e;
    /// Crate: non-zero, it cannot be pushed (h).
    pub const CRATE_FIXED: u32 = 0x65;
    /// Crate: held by a player.
    pub const CRATE_HELD: u32 = 0x66;
    /// Crate: player that last touched, pushed or grabbed it (word).
    pub const CRATE_TOUCHER: u32 = 0x6c;
    /// Crate: pushed by a player.
    pub const CRATE_PUSHED: u32 = 0x6e;
    /// Crate: hit by a player or a shot (h).
    pub const CRATE_HIT: u32 = 0x71;
    /// Enemy: element it is weak to, and the weakness bonus (h).
    pub const WEAKNESS: u32 = 0x9e;
    pub const WEAK_ELEMENT: u32 = 0x9f;
}

/// Program ROM tables.
mod rom {
    /// Attack boxes of the player shots (8-byte entries).
    pub const SHOT_ATTACK: u32 = 0x121fc8;
    /// Attack boxes of the enemy shots.
    pub const ENEMY_SHOT_ATTACK: u32 = 0x1221e2;
    /// Pickup boxes of the items.
    pub const ITEM_PICKUP: u32 = 0x1233c8;
    /// Box pointer tables per set (index `BOX_SET`): body, push, attack.
    pub const BODY: u32 = 0x11bffa;
    pub const PUSH: u32 = 0x11c11a;
    pub const ATTACK: u32 = 0x11c23a;
}

// ---------------------------------------------------------------------------------------------
// Memory access

/// The memory and the state of the hit resolution that the original keeps in registers.
pub struct Combat<'a> {
    pub b: &'a mut World,
    /// Value of A2 in the original: box of the hit in progress, which certain subroutines
    /// (0x19dce, 0x388a, 0xf232e) overwrite without the original restoring it in `player_hits_enemies`.
    pub a2: u32,
    /// The leftovers D2-D5 that the resolution leaves to what follows (spark placement, D4 of the impact sound):
    /// see `dispatch::Leftovers`.
    pub leftovers: Leftovers,
}

impl crate::world::machine::Machine for Combat<'_> {
    fn bus(&mut self) -> &mut World {
        &mut *self.b
    }
}

impl Combat<'_> {
    /// bset on a byte.
    fn set_bit(&mut self, o: Obj, off: u32, bit: u32) {
        let v = o.u8(self.b, off) | 1 << bit;
        o.set_u8(self.b, off, v)
    }
    fn add8(&mut self, o: Obj, off: u32, d: u8) {
        let v = o.u8(self.b, off).wrapping_add(d);
        o.set_u8(self.b, off, v)
    }
    fn add16(&mut self, o: Obj, off: u32, d: u16) {
        let v = o.u16(self.b, off).wrapping_add(d);
        o.set_u16(self.b, off, v)
    }
    /// Bit 0 of the activity byte: the object takes part in collisions.
    fn is_active(&mut self, o: Obj) -> bool {
        o.u8(self.b, f::ACTIVE) & 1 != 0
    }
    /// Spark of variant `variant` from `a0` toward `a1`: 0xaa96c (`Spark::Linked`), 0xaaa04 / 0xaaa84
    /// (`Spark::Between`, the owner first or second). The placement leaves D2.w and D3.w (leftovers); the native
    /// restores A1 to A4, so A2 is unchanged.
    fn spark(&mut self, spark: Spark, a0: Obj, a1: Obj, variant: u8) {
        match spark {
            Spark::Linked => crate::services::effects_aids::spark_by(&mut self.leftovers, self.b, a0, a1.0, variant),
            Spark::Between { owner_first } => {
                crate::services::effects_aids::spark_between_by(&mut self.leftovers, self.b, a0, a1.0, variant, owner_first)
            }
        };
    }
}

/// The spark creators of the hit resolution.
#[derive(Clone, Copy)]
enum Spark {
    /// 0xaa96c: linked to its creator, placed between it and the other object.
    Linked,
    /// 0xaaa04 (`owner_first`) / 0xaaa84: between the creator and the object hit.
    Between { owner_first: bool },
}

fn set_low8(r: &mut u32, v: u8) {
    *r = (*r & !0xff) | v as u32;
}

/// Values of the long state (`+4`: main, substate, `+6`: action, `+7`).
mod state {
    /// Enemy: "hit", reaction to play (`REACTION` gives the animation).
    pub const HIT: u32 = 0x0400_0000;
    /// Enemy that parried the hit.
    pub const BLOCKED: u32 = 0x0402_0000;
    /// Player: busy acting (push, pickup...), action in byte 6.
    pub const ATTACKING: u32 = 0x0200_0c00;
    /// Player hit.
    pub const PLAYER_HURT: u32 = 0x0204_0000;
    /// Player: parry.
    pub const PARRY: u32 = 0x0208_0000;
    /// Player: lifts a crate.
    pub const GRABBED: u32 = 0x0200_1400;
    pub const GRABBING: u32 = 0x0200_2e00;
    pub const PICKING_UP: u32 = 0x0200_2200;
    pub const CRATE_PUSHED: u32 = 0x0200_2800;
}

// ---------------------------------------------------------------------------------------------
// Collision boxes

/// Box in ROM: 6 useful bytes (offset and half-size per axis), 8-byte entries.
///
/// | byte | contents |
/// |---|---|
/// | 0 / 1 | X offset (signed, inverted if the object faces left) / half-width |
/// | 2 / 3 | Z offset (height) / half-height |
/// | 4 / 5 | Y offset (depth) / half-depth |
#[derive(Clone, Copy)]
struct HitBox(u32);

impl HitBox {
    const X: u32 = 0;
    const HALF_X: u32 = 1;
    const Z: u32 = 2;
    const HALF_Z: u32 = 3;
    const Y: u32 = 4;
    const HALF_Y: u32 = 5;
}

/// Box `index` of the object's box set, in the pointer table `table`.
fn hitbox(w: &mut Combat, table: u32, o: Obj, index: u8) -> HitBox {
    let set = o.u8(w.b, f::BOX_SET) as u32;
    let base = long(w.b, table + 4 * set);
    HitBox(base + 8 * index as u32)
}

/// Object box for the index given by its field `field` (body, push or attack).
fn object_box(w: &mut Combat, table: u32, o: Obj, field: u32) -> HitBox {
    let idx = o.u8(w.b, field);
    hitbox(w, table, o, idx)
}

/// Result of comparing an axis: `delta` = gap between the centres (word, in 16-bit
/// arithmetic like the original), `half` = sum of the half-sizes.
#[derive(Clone, Copy)]
struct Probe {
    delta: u16,
    half: u16,
}

impl Probe {
    /// Overlap: |delta| <= half, computed like the original (`delta + half <= 2 * half`, unsigned).
    fn overlaps(self) -> bool {
        self.delta.wrapping_add(self.half) <= self.half.wrapping_add(self.half)
    }
    /// Penetration depth (`half - |delta|`) and sign of the gap (true: delta < 0).
    fn penetration(self) -> (u16, bool) {
        let neg = (self.delta as i16) < 0;
        let abs = if neg { self.delta.wrapping_neg() } else { self.delta };
        (self.half.wrapping_sub(abs), neg)
    }
}

/// Sum of the half-sizes: bytes added on 8 bits; `signed` then sign-extends
/// (collisions with crates), otherwise the extension is zero.
fn half_sum(a: u8, b: u8, signed: bool) -> u16 {
    let s = a.wrapping_add(b);
    if signed {
        s as i8 as i16 as u16
    } else {
        s as u16
    }
}

/// Two objects and the bytes of their boxes.
struct Pair {
    a: Obj,
    abox: [u8; 6],
    b: Obj,
    bbox: [u8; 6],
    signed: bool,
}

/// Variant of 0x27410 (enemy attacks): only the half-depth of the attack counts.
fn overlap3_attack(w: &mut Combat, victim: Obj, vbox: HitBox, attacker: Obj, abox: HitBox) -> bool {
    cov::hit("overlap3_attack");
    let pair = Pair::new(w, victim, vbox, attacker, abox, false);
    let (ay, ax, az) = (pair.probe_y(w), pair.probe_x(w), pair.probe_z(w));
    let half = pair.bbox[HitBox::HALF_Y as usize] as u16;
    let y = Probe { delta: ay.delta, half };
    y.overlaps() && ax.overlaps() && az.overlaps()
}

impl Pair {
    /// The pair, with the bytes of both boxes (`player::tables::box_bytes`).
    fn new(w: &mut Combat, a: Obj, abox: HitBox, b: Obj, bbox: HitBox, signed: bool) -> Pair {
        Pair { a, abox: box_bytes(w.b, abox.0), b, bbox: box_bytes(w.b, bbox.0), signed }
    }

    /// Gap of the centres on an axis: (box offset + position) of `a` minus that of `b`.
    /// `flip`: the offsets follow the object's orientation (X axis).
    fn probe(&self, w: &mut Combat, off: u32, half: u32, pos: u32, flip: bool) -> Probe {
        let mut ao = self.abox[off as usize] as i8 as i16 as u16;
        let mut bo = self.bbox[off as usize] as i8 as i16 as u16;
        if flip {
            if self.a.u8(w.b, f::FLIP) != 0 {
                ao = ao.wrapping_neg();
            }
            if self.b.u8(w.b, f::FLIP) != 0 {
                bo = bo.wrapping_neg();
            }
        }
        let a = ao.wrapping_add(self.a.u16(w.b, pos));
        let b = bo.wrapping_add(self.b.u16(w.b, pos));
        let half = half_sum(self.abox[half as usize], self.bbox[half as usize], self.signed);
        Probe { delta: a.wrapping_sub(b), half }
    }
    fn probe_y(&self, w: &mut Combat) -> Probe {
        self.probe(w, HitBox::Y, HitBox::HALF_Y, f::Y, false)
    }
    fn probe_x(&self, w: &mut Combat) -> Probe {
        self.probe(w, HitBox::X, HitBox::HALF_X, f::X, true)
    }
    fn probe_z(&self, w: &mut Combat) -> Probe {
        self.probe(w, HitBox::Z, HitBox::HALF_Z, f::Z, false)
    }
    /// Overlap on the three axes (Y, X then Z).
    fn overlaps(&self, w: &mut Combat) -> bool {
        self.probe_y(w).overlaps() && self.probe_x(w).overlaps() && self.probe_z(w).overlaps()
    }
}

fn overlap3(w: &mut Combat, a: Obj, abox: HitBox, b: Obj, bbox: HitBox) -> bool {
    Pair::new(w, a, abox, b, bbox, false).overlaps(w)
}

// ---------------------------------------------------------------------------------------------
// Lists of visible objects

/// Categories of the visible object list (see `sprites::check_visible`).
#[derive(Clone, Copy)]
enum List {
    PlayerShots = 0,
    EnemyShots = 1,
    Enemies = 2,
    Boxes = 3,
    Items = 4,
}

impl List {
    /// The category's collision queue (`objects::pools`), filled by `cull::update_visibility`.
    fn queue(self) -> &'static pools::Queue {
        match self {
            List::PlayerShots => &pools::QUEUE_PSHL,
            List::EnemyShots => &pools::QUEUE_ESHL,
            List::Enemies => &pools::QUEUE_ENEMY_BOSS,
            List::Boxes => &pools::QUEUE_BOX,
            List::Items => &pools::QUEUE_ITEM,
        }
    }
    fn len(self, w: &mut Combat) -> u16 {
        rd16(w.b, A5 + self.queue().count)
    }
    /// First pointer (word) of the array of the queue's objects.
    fn first(self) -> u32 {
        A5 + self.queue().array
    }
}

/// End of a loop turn: continue, or leave the loop (counter left as is).
#[derive(PartialEq)]
enum Flow {
    Next,
    Stop,
}

/// Walks a list like the original: loop counter kept in RAM (`counter`), pointer
/// advanced before the body.
fn scan(w: &mut Combat, list: List, counter: u32, mut body: impl FnMut(&mut Combat, Obj) -> Flow) {
    let n = list.len(w);
    wr16(w.b, counter, n);
    let mut p = list.first();
    loop {
        let o = Obj::from_word(rd16(w.b, p));
        p += 2;
        if body(w, o) == Flow::Stop {
            return;
        }
        let k = rd16(w.b, counter).wrapping_sub(1);
        wr16(w.b, counter, k);
        if k == 0 {
            return;
        }
    }
}

/// The 4 player records.
fn players() -> impl Iterator<Item = Obj> {
    crate::world::ram::PLAYERS.iter()
}

// ---------------------------------------------------------------------------------------------
// Root

/// 0x260ee: all the collisions of the frame.
pub fn combat_update(w: &mut Combat) {
    cov::hit("combat_update");
    if ram::PLAYERS_ALIVE.get(w.b) == 0 {
        return;
    }
    let scripted = ram::SCRIPTED_A.get(w.b) != 0 && ram::SCRIPTED_B.get(w.b) != 0;
    if ram::FROZEN.get(w.b) == 0 && !scripted {
        player_attacks_clash_enemies(w);
        player_attacks_clash_enemy_shots(w);
        player_hits_enemies(w);
        player_hits_boxes(w);
        player_shots_hit_enemies(w);
        if ram::PAUSE.get(w.b) != 0 {
            return;
        }
        enemies_hit_players(w);
        enemy_shots_hit_players(w);
        boxes_hit_enemies(w);
        boxes_hit_players(w);
        players_push_others(w);
    }
    players_pick_up_items(w);
}

// ---------------------------------------------------------------------------------------------
// Player hits enemies / crates

/// 0x26150: does a player's hit touch an enemy (attack box against body box)?
fn player_hits_enemies(w: &mut Combat) {
    cov::hit("player_hits_enemies");
    if List::Enemies.len(w) == 0 || ram::PAUSE.get(w.b) != 0 {
        return;
    }
    for p in players() {
        if !w.is_active(p) || p.u8(w.b, f::ATTACK_BOX) == 0 {
            continue;
        }
        // A2 stays the player's box from one enemy to the next, unless a hit overwrote it (w.a2)
        w.a2 = object_box(w, rom::ATTACK, p, f::ATTACK_BOX).0;
        scan(w, List::Enemies, scratch::PLAYER_HITS_ENEMY, |w, e| {
            if w.is_active(e) && e.u8(w.b, f::BODY_BOX) != 0 && e.u8(w.b, f::BOXES_OFF) == 0 && e.u16(w.b, f::HP) != 0 {
                let body = object_box(w, rom::BODY, e, f::BODY_BOX);
                let atk = HitBox(w.a2);
                if overlap3(w, p, atk, e, body) {
                    player_blow_on_enemy(w, p, e);
                }
            }
            Flow::Next
        });
    }
}

/// 0x261f0: does a player's hit touch a crate?
fn player_hits_boxes(w: &mut Combat) {
    cov::hit("player_hits_boxes");
    if List::Boxes.len(w) == 0 {
        return;
    }
    for p in players() {
        if p.u8(w.b, f::ACTIVE) == 0 || p.u8(w.b, f::ATTACK_BOX) == 0 {
            continue;
        }
        let atk = object_box(w, rom::ATTACK, p, f::ATTACK_BOX);
        scan(w, List::Boxes, scratch::PLAYER_HITS_ENEMY, |w, bx| {
            if w.is_active(bx) && bx.u8(w.b, f::BODY_BOX) != 0 && bx.u8(w.b, f::BOXES_OFF) == 0 {
                let body = object_box(w, rom::BODY, bx, f::BODY_BOX);
                if overlap3(w, p, atk, bx, body) {
                    player_blow_on_box(w, p, bx);
                }
            }
            Flow::Next
        });
    }
}

/// 0x2659e: a player hits a crate (the crate holds the hit and reacts).
fn player_blow_on_box(w: &mut Combat, p: Obj, bx: Obj) {
    cov::hit("player_blow_on_box");
    if p.u8(w.b, f::ACTION) == 6 || p.u8(w.b, f::ATTACK_BOX) == 4 {
        return;
    }
    bx.set_u16(w.b, f::KEYS_HELD, p.word());
    bx.set_u8(w.b, f::KEYS_NEW, 1);
    w.spark(Spark::Linked, p, bx, 0);
}

/// 0x2630e: a player `p` hits enemy `e`: stores the hit, computes the damage, starts the
/// victim's reaction and the spark.
fn player_blow_on_enemy(w: &mut Combat, p: Obj, e: Obj) {
    cov::hit("player_blow_on_enemy");
    trace!("hit {:06x} on {:06x}", p.0, e.0);
    if e.u8(w.b, f::HITSTOP) != 0 || p.u8(w.b, f::ATTACK_BOX) == 4 {
        return;
    }
    // the type 3 hit has no effect on the type 0x12 boss (h)
    if p.u8(w.b, f::ATTACK_KIND) == 3 && e.u8(w.b, f::CLASS) == 4 && e.u8(w.b, f::KIND) == 0x12 {
        return;
    }
    w.set_bit(p, f::HIT_MARK, 0);
    // the same hit (ATTACK_ID) only touches each victim once: the victim keeps the
    // last hit received from each player (byte `HIT_MARK + player no.`)
    let id = p.u8(w.b, f::ATTACK_ID);
    let slot = e.0 + f::HIT_MARK + p.u8(w.b, f::KIND) as u32;
    if rd8(w.b, slot) == id {
        return;
    }
    wr8(w.b, slot, id);
    p.set_u8(w.b, f::RECOIL, 0);
    if p.u8(w.b, f::ACTION) != 0xa {
        p.set_u8(w.b, f::RECOIL + 1, 0x18);
        p.set_u8(w.b, f::RECOIL, 1);
        let m = p.u8(w.b, f::SUBACTION);
        if m != 0 {
            p.set_u8(w.b, f::RECOIL, 2);
            if m != 2 {
                p.set_u8(w.b, f::RECOIL, 4);
                if m != 4 {
                    p.set_u8(w.b, f::RECOIL, 0);
                }
            }
        }
    }
    let hp = e.u16(w.b, f::HP);
    e.set_u16(w.b, f::HP_PREV, hp);
    let flip = p.u8(w.b, f::FLIP);
    e.set_u8(w.b, f::REACTION_FLIP, flip);
    let var = p.u8(w.b, f::VARIANT);
    e.set_u8(w.b, f::HIT_BY_VARIANT, var);
    let kind = p.u8(w.b, f::KIND);
    e.set_u8(w.b, f::HIT_BY_KIND, kind);
    e.set_u16(w.b, f::LINK, p.word());
    w.set_bit(e, f::HIT_FLAGS, 0);
    let st = e.u32(w.b, f::STATE);
    e.set_u32(w.b, f::PREV_STATE, st);
    e.set_u8(w.b, f::HITSTOP_VICTIM, 0xc);
    p.set_u8(w.b, f::HITSTOP, 0x14);
    p.set_u8(w.b, f::HIT_FLAGS, 0x14);
    let weapon = p.u8(w.b, f::WEAPON_10D);
    if weapon != 0x1d && weapon != 0x1e {
        e.set_u8(w.b, f::HITSTOP_VICTIM, 8);
        p.set_u8(w.b, f::HITSTOP, 8);
        p.set_u8(w.b, f::HIT_FLAGS, 8);
    }
    if p.u8(w.b, f::ATTACK_KIND) == 3 {
        p.set_u32(w.b, f::STATE, state::ATTACKING);
        p.set_u8(w.b, f::SUBACTION, 0x12);
        steal_loot(w, p, e);
        enemy_reacts_to_hit(w, p, e);
        return;
    }
    // hit parried / absorbed by the victim (0xb048: Z clear)
    // hit parried by the victim (0xb048)
    if crate::services::ai_aids::parries(w.b, p, e, crate::services::ai_aids::Parry::A) {
        e.set_u32(w.b, f::STATE, state::BLOCKED);
        p.set_u8(w.b, f::HITSTOP, 8);
        w.set_bit(p, f::HIT_MARK, 6);
        if p.u8(w.b, Player::BLOW_ID) == 0x60 {
            p.set_u32(w.b, f::STATE, state::ATTACKING);
            p.set_u8(w.b, f::SUBACTION, 0x12);
        }
        w.spark(Spark::Between { owner_first: true }, p, e, 4);
        return;
    }
    count_combo(w, p, e);
    let mut dmg = player_damage(w, p, e);
    if p.u8(w.b, f::WEAPON_10D) == 0xb && e.u8(w.b, Enemy::WEAK_TO_B) != 0 {
        dmg = dmg.wrapping_add(dmg >> 1);
    }
    if p.u8(w.b, f::WEAPON_10D) == 0xc && e.u8(w.b, Enemy::WEAK_TO_C) != 0 {
        dmg = 0x200;
    }
    if p.u8(w.b, f::GEAR_184) != 0 {
        dmg = dmg.wrapping_add(dmg >> 2);
    }
    if p.u8(w.b, f::GEAR_166) == 5 && p.u8(w.b, f::ATTACK_KIND) == 2 {
        dmg = dmg.wrapping_add(dmg >> 2);
    }
    if p.u8(w.b, f::GEAR_163) == 0 {
        dmg = dmg.wrapping_add(3);
    }
    if p.u8(w.b, f::GEAR_166) == 2 {
        dmg = dmg.wrapping_add(1);
    }
    // cap (low byte only) set by the player
    let cap = p.u8(w.b, f::DAMAGE_CAP);
    if (dmg as u8 as i8) > cap as i8 {
        dmg = (dmg & 0xff00) | cap as u16;
    }
    enemy_loses_hp(w, e, dmg);
    if e.u16(w.b, f::HP) == 0 && e.u8(w.b, f::DEATH_CREDIT) != 0 {
        let v = e.u8(w.b, f::DEATH_VALUE);
        ram::KILL_FLAG.set(w.b, v);
        p.set_u8(w.b, f::KILLS_FLAG, 1);
    }
    if p.u8(w.b, Player::MOVE_STATE) != 0 {
        let add = p.u8(w.b, Player::MOVE_KIND) as u16;
        w.add16(p, f::HITS_LANDED, add);
    }
    show_enemy_hp(w, p, e);
    enemy_reacts_to_hit(w, p, e);
}

/// HP -= damage, floor at 0 (16-bit subtraction, negative result brought back to 0).
fn enemy_loses_hp(w: &mut Combat, e: Obj, dmg: u16) {
    let hp = e.u16(w.b, f::HP).wrapping_sub(dmg);
    e.set_u16(w.b, f::HP, if (hp as i16) < 0 { 0 } else { hp });
}

/// 0x19dce: enqueues the display of the health gauge of the enemy hit (key: player, enemy type).
fn show_enemy_hp(w: &mut Combat, attacker: Obj, e: Obj) {
    cov::hit("show_enemy_hp");
    let key = ((attacker.u8(w.b, f::KIND) as u32) << 8) + e.u8(w.b, f::KIND) as u32;
    let (hp_max, hp, class) = (e.u16(w.b, f::HP_MAX), e.u16(w.b, f::HP), e.u8(w.b, f::CLASS));
    let bar = crate::world::events::Popup { header: key as u16, max: hp_max, value: hp, raw: class == 2 };
    // Original glitch: 0x19dce leaves A2 on the pop-up queue (unless they are inhibited), and A2 is the player's attack
    // box for the following enemies of `player_hits_enemies`
    if !crate::world::events::popups_inhibited(w.b) {
        w.a2 = crate::world::events::POPUP_INPUTS_ADDR;
    }
    crate::world::events::create_popup(w.b, bar);
}

/// End of 0x2630e (0x264e4): the victim goes into the "hit" reaction, the spark is created and the
/// impact sound played (0xe662a for simple hits, 0xe823a for special ones; tail jump).
fn enemy_reacts_to_hit(w: &mut Combat, p: Obj, e: Obj) {
    cov::hit("enemy_reacts_to_hit");
    e.set_u32(w.b, f::STATE, state::HIT);
    e.set_u8(w.b, f::FLAGS, 1);
    e.set_u8(w.b, f::REACTION, 0x4c);
    if e.u8(w.b, f::FLIES) == 0 {
        // 0xf232e
        w.a2 = crate::actors::player::hit_reactions::enemy_reacts_from_hit(w.b, crate::actors::player::Player(p.0), e.0);
    }
    w.spark(Spark::Linked, p, e, 0);
    let mut d4 = p.u8(w.b, f::WEAPON_10D);
    if p.u8(w.b, f::ATTACK_KIND) != 0 {
        d4 = p.u8(w.b, f::WEAPON_10E);
    }
    set_low8(&mut w.leftovers.d4, d4);
    let target = if p.u8(w.b, f::ATTACK_KIND) <= 1 { crate::hw::sound::HIT_BY_CATEGORY } else { crate::hw::sound::HIT_BY_VARIANT };
    let source = crate::hw::sound::Source { object: Some(p), a1: Some(e.0), d4: Some(w.leftovers.d4) };
    crate::hw::sound::play_with(w.b, target, source);
}

/// 0x26562: counts the chained special hits (byte `+0x9c` of the victim) outside class 4.
fn count_combo(w: &mut Combat, p: Obj, e: Obj) {
    cov::hit("count_combo");
    let class = p.u8(w.b, f::VARIANT) >> 1;
    if class == 4 {
        return;
    }
    let m = p.u8(w.b, f::SUBACTION);
    if (m == 0xc || m == 0x16 || m == 0x18) && m != p.u8(w.b, f::COMBO_MARK) {
        w.add8(e, f::HITS_TAKEN, 1);
        p.set_u8(w.b, f::COMBO_MARK, m);
    }
}

// ---------------------------------------------------------------------------------------------
// Native routines
