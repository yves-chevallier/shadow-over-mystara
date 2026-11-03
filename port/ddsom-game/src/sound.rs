//! Sound, 68000 side: queue of commands sent to the Z80 (format: `docs/sound-68000.md`).
//!
//! The game enqueues its commands in a circular queue in work RAM (64 entries of 16 bytes);
//! each frame, VBLANK transmits **one** of them to the Z80 through the shared RAM 0x618000. The roughly 520
//! `sfx_*` "wrappers" (0xe0be2-0xede08) only differ by their parameters: they are described
//! by the table `sound_table::WRAPPERS` and played by a single function, [`play`] (or [`play_with`]).
//!
//! An original wrapper saves all registers on a private stack (pointer in `PRIVATE_STACK`),
//! then composes three longs (D1 = command, D2 and D3 = parameters) and calls the enqueuer. All
//! registers are returned intact; only the memory changes: the queue, and the copy of the registers stays written
//! below the private stack pointer. This "ghost save" is reread by nobody and is no longer
//! reproduced (accepted difference, `play_wrapper`): a sound only reads from its caller the object (A0) and, for
//! a few, D4 or A1 ([`Source`]).

use cps2::Cps2Bus;

use crate::mem::{rd16, rd8, wr16, wr32, wr8};
use crate::objects::Obj;
use crate::Native;

#[path = "sound_table.rs"]
pub(crate) mod table;

/// Offsets relative to A5 (signed words/bytes extended: `A5 + n` modulo 2^32).
pub(crate) mod ram {
    /// Start of the queue (64 entries of 16 bytes).
    pub const FILE: u32 = 0xffff_a55c;
    /// Read head (next entry to transmit to the Z80), word.
    pub const READ_HEAD: u32 = 0xffff_a95c;
    /// Write head (next free cell), word.
    pub const WRITE_HEAD: u32 = 0xffff_a95e;
    /// 0 / -1 alternation of the two-variant wrappers (voice A / voice B), word.
    pub const ALTERNATION: u32 = 0xffff_a960;
    /// Pointer of the wrappers' private stack (long).
    pub const PRIVATE_STACK: u32 = 0xffff_aa1a;
    /// Non-zero byte: the "guarded" wrappers emit nothing.
    pub const MUTE_GUARD: u32 = 0xffff_aa1e;
    /// Sounds allowed if this word is non-zero...
    pub const SOUND_WORD: u32 = 0x7fcc;
    /// ... or if one of these bytes is (game in progress / attract mode with sound).
    pub const GAME_SOUND: u32 = 0x30;
    pub const ATTRACT_SOUND: u32 = 0x27;
    /// Last "guarded" command emitted (memo).
    pub const MEMO_COMMAND: u32 = 0x7fc4;
    /// Countdown decremented at each VBLANK (delay set by certain commands).
    pub const DELAY: u32 = 0x7fc2;
    /// Non-zero byte: sound output lock (copied to 0x619ffd).
    pub const SILENCE: u32 = 0x28;
    /// X position of camera 1 / 2 and camera choice (byte).
    pub const CAMERA_1_X: u32 = 0x5a6;
    pub const CAMERA_2_X: u32 = 0x5ae;
    pub const CAMERA_CHOICE: u32 = 0x36;
}

/// Size of the queue: 64 entries of 16 bytes.
pub(crate) const MASK_HEAD: u16 = 0x3f0;
pub(crate) const ENTRY: u16 = 0x10;

/// RAM shared with the Z80: bytes at odd addresses.
mod z80 {
    pub const BASE: u32 = 0x618000;
    /// Bytes of the command, in sending order: (byte of the queue entry, address).
    /// Byte 4 of the entry (high-order part of D2) is not transmitted.
    pub const FIELDS: [(usize, u32); 11] = [
        (0, BASE + 0x07),
        (1, BASE + 0x09),
        (2, BASE + 0x01),
        (3, BASE + 0x03),
        (5, BASE + 0x0d),
        (6, BASE + 0x0f),
        (7, BASE + 0x11),
        (8, BASE + 0x13),
        (9, BASE + 0x15),
        (10, BASE + 0x17),
        (11, BASE + 0x19),
    ];
    /// Handshake: the Z80 sets 0xff when it has read the command, the 68000 resets it to 0.
    pub const ACQUIT: u32 = BASE + 0x1f;
    /// Lock byte read by the Z80 (0xff if the sound output is locked).
    pub const LOCK: u32 = BASE + 0x1ffd;
}

/// Display of the Z80 locks (debug screen), at the tail of VBLANK: active only if the ROM word at 0xc0
/// (debug flag) and the word `A5+0x7fc6` are non-zero; each byte of the locks
/// 0x618001... is written in hexadecimal (0x29d78, two digits) at an address of the text layer.
const DEBUG_DISPLAY: u32 = 0x7fc6;
/// (address of the text layer if it changes, lock read), in display order.
const LATCH_DISPLAY: [(Option<u32>, u32); 11] = [
    (Some(0x9135cc), 0x618001),
    (None, 0x618003),
    (Some(0x9135d0), 0x618007),
    (None, 0x618009),
    (Some(0x9136d4), 0x61800d),
    (Some(0x9136d8), 0x61800f),
    (Some(0x9136dc), 0x618011),
    (Some(0x9135e0), 0x618013),
    (None, 0x618015),
    (Some(0x9135e4), 0x618017),
    (None, 0x618019),
];

/// Original dead code (debug screen, docs/code-mort.md): the locks of the Z80 in hexadecimal, attribute 2. The
/// cursor (A1 of the original) moves on after each byte, unless the table gives a new address.
fn show_locks(b: &mut Cps2Bus) {
    let mut at = 0;
    for (address, latch) in LATCH_DISPLAY {
        if let Some(address) = address {
            at = address;
        }
        let value = rd8(b, latch);
        at = hex_byte(b, at, value, 2);
    }
}

/// `0x29d78`: byte `value` as two hexadecimal digits (tile = the digit) of attribute `attribute`, from cell `at` of
/// the text layer, one column per digit; returns the cell that follows.
fn hex_byte(b: &mut Cps2Bus, mut at: u32, value: u8, attribute: u16) -> u32 {
    for digit in [value >> 4, value & 0xf] {
        crate::mem::wr16(b, at, digit as u16);
        crate::mem::wr16(b, at + 2, attribute);
        at += 0x80;
    }
    at
}

/// Address `A5 + off` (A5 is always `mem::A5`; negative offsets are written modulo 2^32).
pub(crate) const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

// ---------------------------------------------------------------------------------------------
// The command queue

/// A command: three longs, as the game composes them (D1, D2, D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub d1: u32,
    pub d2: u32,
    pub d3: u32,
}

/// Can the command be emitted? (sound allowed: 0xe0836)
pub(crate) fn sound_allowed(b: &mut Cps2Bus) -> bool {
    rd16(b, a5(ram::SOUND_WORD)) != 0
        || rd8(b, a5(ram::GAME_SOUND)) != 0
        || rd8(b, a5(ram::ATTRACT_SOUND)) != 0
}

/// The queue is full when the write would catch up with the read (one cell always stays free).
fn queue_full(b: &mut Cps2Bus) -> bool {
    let read = rd16(b, a5(ram::READ_HEAD));
    let write = rd16(b, a5(ram::WRITE_HEAD));
    read.wrapping_sub(ENTRY) & MASK_HEAD == write
}

/// Enqueues a command (0xe0848, without the permission test); returns false if the queue is full.
pub(crate) fn enqueue(b: &mut Cps2Bus, cmd: Command) -> bool {
    let write = rd16(b, a5(ram::WRITE_HEAD));
    if queue_full(b) {
        return false;
    }
    let case = a5(ram::FILE).wrapping_add(write as i16 as u32);
    wr32(b, case, cmd.d1);
    wr32(b, case + 4, cmd.d2);
    wr32(b, case + 8, cmd.d3);
    let next = write.wrapping_add(ENTRY) & MASK_HEAD;
    wr16(b, a5(ram::WRITE_HEAD), next);
    true
}

/// 0xe0836: enqueues the command if sound is allowed; returns `false` if nothing is enqueued (sound forbidden, queue
/// full).
pub fn emit(b: &mut Cps2Bus, cmd: Command) -> bool {
    sound_allowed(b) && enqueue(b, cmd)
}

// ---------------------------------------------------------------------------------------------
// The wrappers

/// Word of a command table: the tables are stored in the code and read PC-relative (`sound/*` of the engine data, by
/// address).
fn program_read(b: &mut Cps2Bus, address: u32) -> u16 {
    crate::tables::engine().pc_word(b, address & 0xff_ffff)
}

/// Origin of the command index in a word table in ROM.
#[derive(Clone, Copy, Debug)]
pub enum Cmd {
    /// Constant command.
    Fixed(u16),
    /// Two commands in alternation (table address): 0xedee4 returns 0 then 2 in turn.
    Alternate(u32),
    /// Index = 2 × byte 2 of the object + 4 × (byte 3 & 0xe) (0xedef2).
    ByObject(u32),
    /// Index = 2 × byte 2 of the object.
    ByByte2(u32),
    /// The last guarded command (`MEMO_COMMAND`), replayed.
    Memo,
    /// Command passed by the caller in D4.w (music).
    FromD4,
}

// Options of a wrapper.
/// Pan computed from the X position of object A0, added to byte 1 of D2.
pub const PAN: u8 = 1;
/// Emit nothing if `MUTE_GUARD` is non-zero.
pub const GUARD: u8 = 2;
/// Copies the command into `MEMO_COMMAND`.
pub const MEMO: u8 = 4;
/// Enqueues without testing the sound permission (0xe0848 instead of 0xe0836).
pub const DIRECT: u8 = 8;

/// A sound of the game, as the code designates it: the way its original wrapper composes the command. The entry of
/// that wrapper stays its identity (the ROM tables that designate sounds store it, and the tools find the natives by
/// it).
#[derive(Clone, Copy, Debug)]
pub struct Sound {
    pub entry: u32,
    kind: Kind,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Wrapper(Wrapper),
    /// Chooses another sound from a table (see [`Dispatch`]).
    Dispatch(Dispatch),
    /// Tests the alternate-palette version, then plays the sound at `normal` or at `alternate`.
    ByVersion { normal: u32, alternate: u32 },
    /// The sound of the objects without sound (a bare `rts`).
    Silent,
}

impl Sound {
    pub(crate) const fn wrapper(entry: u32, cmd: Cmd, d2: u32, d3: u32, opts: u8) -> Sound {
        Sound { entry, kind: Kind::Wrapper(Wrapper { cmd, d2, d3, opts }) }
    }

    const fn dispatch(entry: u32, d: Dispatch) -> Sound {
        Sound { entry, kind: Kind::Dispatch(d) }
    }

    const fn by_version(entry: u32, normal: u32, alternate: u32) -> Sound {
        Sound { entry, kind: Kind::ByVersion { normal, alternate } }
    }

    /// The sound whose original wrapper starts at `entry`, for the sounds that a ROM table (or a dispatcher)
    /// designates by address.
    pub fn at(entry: u32) -> Sound {
        if let Ok(i) = table::WRAPPERS.binary_search_by_key(&entry, |w| w.entry) {
            return table::WRAPPERS[i];
        }
        OTHERS.iter().find(|s| s.entry == entry).copied().unwrap_or_else(|| panic!("{entry:x}: no sound starts here"))
    }

    /// Name of the native of the tools that plays this sound for the original code.
    #[cfg(feature = "tools")]
    pub(crate) fn kind_name(self) -> &'static str {
        match self.kind {
            Kind::Wrapper(_) => "sound_wrapper",
            Kind::Dispatch(_) => "sound_dispatch",
            Kind::ByVersion { .. } => "sound_by_version",
            Kind::Silent => "sound_none",
        }
    }

    /// Does the sound read its object (A0)? The targets of the dispatchers by D4 or A1 are only known at run time:
    /// presumed to read it.
    fn reads_object(self) -> bool {
        match self.kind {
            Kind::Wrapper(w) => w.opts & PAN != 0 || matches!(w.cmd, Cmd::ByObject(_) | Cmd::ByByte2(_)),
            Kind::Dispatch(d) => !matches!(d, Dispatch::Music { .. }),
            Kind::ByVersion { normal, alternate } => Sound::at(normal).reads_object() || Sound::at(alternate).reads_object(),
            Kind::Silent => false,
        }
    }
}

/// How a wrapper composes its command: D1 is the command, D2 and D3 its parameters.
#[derive(Clone, Copy, Debug)]
struct Wrapper {
    cmd: Cmd,
    d2: u32,
    d3: u32,
    opts: u8,
}

pub use table::*;

/// What a sound reads from its caller: the registers the original consults, and nothing else (see the header,
/// "ghost save"). A sound that reads an absent value stops (panic): the caller must provide it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// A0: the object at the origin of the noise (pan, player number, character, variant).
    pub object: Option<Obj>,
    /// A1: the object designated by the `ByA1` dispatchers (0xec046, 0xec200).
    pub a1: Option<u32>,
    /// D4: music command (`FromD4`) or choice of the `ByD4` dispatchers.
    pub d4: Option<u32>,
}

impl Source {
    fn object(&self) -> u32 {
        self.object.expect("this sound reads its object (A0)").0
    }
    fn a1(&self) -> u32 {
        self.a1.expect("this sound reads A1")
    }
    fn d4(&self) -> u32 {
        self.d4.expect("this sound reads D4")
    }
}

/// Plays sound `sound` for object `object` (A0 of the original). The few sounds that also read D4 or A1 go through
/// [`play_with`].
pub fn play(b: &mut Cps2Bus, sound: Sound, object: Obj) {
    play_with(b, sound, Source { object: Some(object), ..Source::default() });
}

/// Plays a sound that reads nothing from its caller (no pan, player or variant: music, interface
/// noises), for callers without an object.
pub fn play_global(b: &mut Cps2Bus, sound: Sound) {
    debug_assert!(!sound.reads_object(), "sound {:x}: reads its object, use `play`", sound.entry);
    play_with(b, sound, Source::default());
}

/// Plays sound `sound` with everything it may read (see [`Source`]).
pub fn play_with(b: &mut Cps2Bus, sound: Sound, source: Source) {
    match sound.kind {
        Kind::Wrapper(w) => play_wrapper(b, &w, source),
        Kind::Dispatch(d) => dispatch(b, d, source),
        Kind::ByVersion { normal, alternate } => {
            let target = if version_alternative(b) { alternate } else { normal };
            play_with(b, Sound::at(target), source);
        }
        Kind::Silent => {}
    }
}

/// Pan (0-0x2f -> byte of the table `sound_pan`, one byte per 8-pixel slice of screen, 0xee04e) of an object: screen position of the object relative to
/// the camera, clamped to 0..0x178, in 8-pixel slices.
fn pan(b: &mut Cps2Bus, object: u32) -> u8 {
    let x = rd16(b, object + 8);
    let camera_2 = rd8(b, a5(ram::CAMERA_CHOICE)) != 0;
    let camera = rd16(b, a5(if camera_2 { ram::CAMERA_2_X } else { ram::CAMERA_1_X }));
    let gap = x.wrapping_sub(camera);
    let gap = if (gap as i16) < 0 { 0 } else { gap };
    let gap = if gap < 0x180 { gap } else { 0x178 };
    crate::tables::engine_table!("sound_pan").at8(b, ((gap & 0x1ff) >> 3) as u32)
}

/// Result of evaluating a wrapper (without effect on memory).
struct Plan {
    /// Command to enqueue, or `None` if the wrapper is guarded (nothing is emitted).
    command: Option<Command>,
    /// Is the sound allowed (otherwise the command is dropped by the enqueuer)?
    allowed: bool,
}

fn schedule(b: &mut Cps2Bus, w: &Wrapper, source: Source) -> Plan {
    // index of the command table, computed by the subroutines 0xedee4 / 0xedef2
    let index = match w.cmd {
        Cmd::Fixed(_) => 0,
        Cmd::Alternate(_) => {
            let voice_b = rd16(b, a5(ram::ALTERNATION)) != 0;
            if voice_b { 2 } else { 0 }
        }
        Cmd::ByObject(_) => {
            let byte3 = rd8(b, source.object() + 3) as u16;
            let byte2 = rd8(b, source.object() + 2) as u16;
            byte2 * 2 + ((byte3 & 0xe) << 2)
        }
        Cmd::ByByte2(_) => rd8(b, source.object() + 2) as u16 * 2,
        Cmd::Memo | Cmd::FromD4 => 0,
    };
    let d1 = match w.cmd {
        Cmd::Fixed(v) => v,
        Cmd::Alternate(t) | Cmd::ByObject(t) | Cmd::ByByte2(t) => program_read(b, t + index as i16 as u32),
        Cmd::Memo => rd16(b, a5(ram::MEMO_COMMAND)),
        Cmd::FromD4 => source.d4() as u16,
    };
    if w.opts & GUARD != 0 {
        let mute = rd8(b, a5(ram::MUTE_GUARD)) != 0;
        if mute {
            return Plan { command: None, allowed: false };
        }
    }
    let mut d2 = w.d2;
    if w.opts & PAN != 0 {
        d2 |= (pan(b, source.object()) as u32) << 8;
    }
    let direct = w.opts & DIRECT != 0;
    let allowed = direct || sound_allowed(b);
    Plan { command: Some(Command { d1: d1 as u32, d2, d3: w.d3 }), allowed }
}

/// A wrapper: composes its command and enqueues it.
//
// Accepted difference: the original starts by pushing D0-D7/A0-A4 on the private stack (pointer `PRIVATE_STACK`,
// FF2A1A) and pops them on exit; the copy stays in RAM below the pointer ("ghost save", FF297E-FF2A19 in
// all scenarios, three dispatch levels at most). Nothing rereads these words apart from the restorations of the
// wrappers themselves (read trace on the original machine, all scenarios: docs/son-68000.md), and
// `validate` ignores them: the game no longer writes them, and a sound no longer depends on the registers of its
// caller.
fn play_wrapper(b: &mut Cps2Bus, w: &Wrapper, source: Source) {
    let plan = schedule(b, w, source);
    if let Cmd::Alternate(_) = w.cmd {
        let v = rd16(b, a5(ram::ALTERNATION));
        wr16(b, a5(ram::ALTERNATION), !v);
    }
    if let Some(cmd) = plan.command {
        if w.opts & MEMO != 0 {
            wr16(b, a5(ram::MEMO_COMMAND), cmd.d1 as u16);
        }
        if plan.allowed {
            enqueue(b, cmd);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The dispatchers

/// Wrappers that do not compose their command themselves: they choose from a table, according to a
/// register or the object, another wrapper that they call (after their own save on the private
/// stack, D0-D3 zeroed), or else a music command.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Dispatch {
    /// `tst.b D4 ; bmi` then D4.w = 2 × D4.b, D0.w = table[D4], target = table + D0 (nothing if D4 < 0).
    ByD4(u32),
    /// D1.w = table[A0[3] & 0xe], target = table + D1.
    ByVariant(u32),
    /// A1 = long table[2 × (A0[3] & 0xe)], target = A1.
    ByVariantLong(u32),
    /// A1 = long table[2 × (A0[3] & 0xe)], then D1.b = A0[0x47], A1 = long (plain) at A1 + D1.w.
    ByVariantAndCategory(u32),
    /// D1 = 4 × A1[0x44], A1 = long table[D1], target = A1.
    ByA1(u32),
    /// Scene music: D1.w = table[2 × scene] (nothing if guarded, or if D1 <= 0 / D1 < 0 depending on
    /// `strict`), copied to the memo then enqueued if sound is allowed.
    Music { table: u32, strict: bool },
}

// The dispatchers.
/// Scene music (plays a positive command).
pub const SCENE_MUSIC: Sound = Sound::dispatch(0xe0c5c, Dispatch::Music { table: 0xe0ca6, strict: true });
/// Scene music, or its stop (plays a command zero or positive, always noted in the memo).
pub const SCENE_MUSIC_OR_STOP: Sound = Sound::dispatch(0xe0d8c, Dispatch::Music { table: 0xe0dd6, strict: false });
/// Noise of a hit, by the variant and the category of the object.
pub const HIT_BY_CATEGORY: Sound = Sound::dispatch(0xe662a, Dispatch::ByVariantAndCategory(0xe666c));
/// Chosen by D4 (no caller in the game).
pub const BY_D4: Sound = Sound::dispatch(0xe6910, Dispatch::ByD4(0xe694a));
/// Character voices and noises, by variant.
pub const JUMP_VOICE: Sound = Sound::dispatch(0xe7622, Dispatch::ByVariant(0xe765c));
pub const LANDING_VOICE: Sound = Sound::dispatch(0xe774c, Dispatch::ByVariant(0xe7786));
pub const SLIDE_VOICE: Sound = Sound::dispatch(0xe78e8, Dispatch::ByVariant(0xe7922));
pub const LANDING_EFFECT: Sound = Sound::dispatch(0xe7a12, Dispatch::ByVariant(0xe7a4c));
/// Voices chosen by D4.
pub const VOICE_BY_D4: Sound = Sound::dispatch(0xe7c20, Dispatch::ByD4(0xe7c5a));
pub const WEAPON_VOICE: Sound = Sound::dispatch(0xe7e10, Dispatch::ByD4(0xe7e4a));
/// Noise of a hit, by the variant of the object.
pub const HIT_BY_VARIANT: Sound = Sound::dispatch(0xe823a, Dispatch::ByVariantLong(0xe8274));
/// Noises of a contact, by the object in A1: an enemy (0xec046) or another object (0xec200) (h).
pub const CONTACT_ENEMY: Sound = Sound::dispatch(0xec046, Dispatch::ByA1(0xec080));
pub const CONTACT_OBJECT: Sound = Sound::dispatch(0xec200, Dispatch::ByA1(0xec23a));

// The sounds of the alternate-palette version (`A5+0x70` non-zero): `normal` or, in this version, `alternate`.
/// Character noises.
pub const GET_UP_B: Sound = Sound::by_version(0xe2240, 0xe2248, 0xe2390);
pub const GET_UP_A: Sound = Sound::by_version(0xe22e8, 0xe22f0, 0xe2390);
/// Noise per object.
pub const ITEM: Sound = Sound::by_version(0xe2430, 0xe2438, 0xe24d8);
/// Noise per kind.
pub const BY_KIND: Sound = Sound::by_version(0xe27da, 0xe27e2, 0xe28a4);
/// Sound 0xd5 or 0xd6, cleric's prayer.
pub const PRAYER: Sound = Sound::by_version(0xe285c, 0xe2864, 0xe28a4);
/// Sound 0xf7 or 0xf6, magician's storm.
pub const STORM: Sound = Sound::by_version(0xe2a74, 0xe2a7a, 0xe2a34);
/// Sound 0xea or 0xe6, elf's moon (hole found by the bot `longue_d`).
pub const MOON: Sound = Sound::by_version(0xe296c, 0xe2974, 0xe29f4);

/// 0xec238: the sound of the objects without sound (a bare `rts`).
pub const SILENT: Sound = Sound { entry: 0xec238, kind: Kind::Silent };

/// The sounds that are not wrappers: dispatchers, sounds by version and the silent sound.
pub const OTHERS: [Sound; 21] = [
    SCENE_MUSIC, SCENE_MUSIC_OR_STOP, HIT_BY_CATEGORY, BY_D4, JUMP_VOICE, LANDING_VOICE, SLIDE_VOICE, LANDING_EFFECT,
    VOICE_BY_D4, WEAPON_VOICE, HIT_BY_VARIANT, CONTACT_ENEMY, CONTACT_OBJECT, GET_UP_B, GET_UP_A, ITEM, BY_KIND,
    PRAYER, STORM, MOON, SILENT,
];

/// Long of a dispatcher table (read PC-relative).
fn program_read32(b: &mut Cps2Bus, address: u32) -> u32 {
    crate::tables::engine().pc_long(b, address & 0xff_ffff)
}

fn low_word(r: u32, v: u16) -> u32 {
    r & 0xffff_0000 | v as u32
}

/// A dispatcher: chooses the sound to play, which reads the modified source as in the original (doubled D4,
/// designated A1), or enqueues the scene music. The original saves and restores its registers (private stack, see
/// `play_wrapper`).
fn dispatch(b: &mut Cps2Bus, kind: Dispatch, source: Source) {
    let (target, source) = match kind {
        Dispatch::ByD4(table) => {
            let d4 = source.d4() as u8 as i8;
            if d4 < 0 {
                return;
            }
            let i = (d4 as i16 * 2) as u16;
            let d = program_read(b, table.wrapping_add(i as i16 as u32));
            (table.wrapping_add(d as i16 as u32), Source { d4: Some(low_word(source.d4(), i)), ..source })
        }
        Dispatch::ByVariant(table) => {
            let i = (rd8(b, source.object() + 3) & 0xe) as u32;
            let d = program_read(b, table + i);
            (table.wrapping_add(d as i16 as u32), source)
        }
        Dispatch::ByVariantLong(table) => {
            let i = 2 * (rd8(b, source.object() + 3) & 0xe) as u32;
            let a1 = program_read32(b, table + i);
            (a1, Source { a1: Some(a1), ..source })
        }
        Dispatch::ByVariantAndCategory(table) => {
            let i = 2 * (rd8(b, source.object() + 3) & 0xe) as u32;
            let list = program_read32(b, table + i);
            let category = rd8(b, source.object() + 0x47) as u32;
            let a1 = crate::tables::engine().long(b, list.wrapping_add(category));
            (a1, Source { a1: Some(a1), ..source })
        }
        Dispatch::ByA1(table) => {
            let i = 4 * rd8(b, source.a1() + 0x44) as u32;
            let a1 = program_read32(b, table + i);
            (a1, Source { a1: Some(a1), ..source })
        }
        Dispatch::Music { table, strict } => {
            if rd8(b, a5(ram::MUTE_GUARD)) == 0 {
                let scene = rd8(b, a5(0x34)) as i8 as i16;
                let i = (scene * 2) as u16;
                let d1 = program_read(b, table.wrapping_add(i as i16 as u32));
                if !strict {
                    wr16(b, a5(ram::MEMO_COMMAND), d1);
                }
                let played = if strict { d1 as i16 > 0 } else { d1 as i16 >= 0 };
                if played {
                    if strict {
                        wr16(b, a5(ram::MEMO_COMMAND), d1);
                    }
                    if sound_allowed(b) {
                        enqueue(b, Command { d1: d1 as u32, d2: 0, d3: 0 });
                    }
                }
            }
            return;
        }
    };
    play_with(b, Sound::at(target), source);
}

// ---------------------------------------------------------------------------------------------
// Transmission to the Z80 (VBLANK)

/// Transmits queue entry `entry` to the bytes of the shared RAM.
fn transmit(b: &mut Cps2Bus, entry: u32) {
    for (byte, address) in z80::FIELDS {
        let v = rd8(b, entry + byte as u32);
        wr8(b, address, v);
    }
}

/// 0x68c (VBLANK): one command of the queue goes to the Z80 if it has acknowledged the previous one. Returns the
/// queue entry transmitted.
pub fn send(b: &mut Cps2Bus) -> Option<u32> {
    let silence = rd8(b, a5(ram::SILENCE)) != 0;
    wr8(b, z80::LOCK, if silence { 0xff } else { 0 });
    let delay = rd16(b, a5(ram::DELAY));
    if delay != 0 {
        wr16(b, a5(ram::DELAY), delay - 1);
    }
    let read = rd16(b, a5(ram::READ_HEAD));
    let write = rd16(b, a5(ram::WRITE_HEAD));
    let mut sent = None;
    if read != write && rd8(b, z80::ACQUIT) == 0xff {
        let entry = a5(ram::FILE).wrapping_add(read as i16 as u32);
        transmit(b, entry);
        wr8(b, z80::ACQUIT, 0);
        wr16(b, a5(ram::READ_HEAD), read.wrapping_add(ENTRY) & MASK_HEAD);
        sent = Some(entry);
    }
    // the original ends with a JMP to the display of the locks
    if shown_locks(b) {
        // Original dead code: debug screen (docs/code-mort.md): the program word 0xc0 is zero in this version
        show_locks(b);
    }
    sent
}

/// The debug display of the locks is active (program word 0xc0, word 0 of `startup/header`, and `A5+0x7fc6`
/// non-zero).
fn shown_locks(b: &mut Cps2Bus) -> bool {
    crate::tables::engine_table!("startup/header").at16(b, 0) != 0 && rd16(b, a5(DEBUG_DISPLAY)) != 0
}

/// 0x68c (the registers it leaves, D0 = read head and A4 after the entry transmitted, are those that VBLANK saved).
fn send_to_z80(b: &mut Cps2Bus) {
    send(b);
}

pub const SEND_TO_Z80: Native = Native {
    entry: 0x68c,
    name: "sound_send_z80",
    run: crate::Body::Memory(send_to_z80),
    touches_sound: true,
    outputs: 0,
};

// ---------------------------------------------------------------------------------------------
// Service commands (0xe0882, 0xe0a10, 0xe0a2e-0xe0aae)

/// Byte of the shared RAM written by the delayed commands (read by the Z80).
const SERVICE_LOCK: u32 = z80::BASE + 0x05;
/// By index D0.w (in bytes): (delay `A5+0x7fc2`, high word of D1), plain words.
const DELAYS: u32 = 0xe0ab0;

/// Service command, enqueued without the sound permission test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// 0xe0882: command 0xff00 (general stop).
    GeneralStop,
    /// 0xe0a10: command 0xff05.
    Ff05,
    /// 0xe0a2e (lock = D1.b) / 0xe0a40 (lock 0xff): delayed command 0xff06. `line` (D0.w): offset in
    /// bytes of the row of the delay table.
    Ff06 { lock: u8, line: u16 },
    /// 0xe0a70 (lock = D1.b) / 0xe0a82 (lock 0): delayed command 0xff07 (see `Ff06`).
    Ff07 { lock: u8, line: u16 },
    /// 0xe0ad8 / 0xe0af8: commands 0xff08 and 0xff09, with parameter D0.w (high word of D1).
    Ff08(u16),
    Ff09(u16),
}

/// Enqueues a service command. A delayed command first writes the Z80 lock, then reads the delay
/// (`A5+0x7fc2`) and the high word of the command from the delay table.
pub fn service(b: &mut Cps2Bus, s: Service) {
    service_enqueue(b, s);
}

pub(crate) fn service_enqueue(b: &mut Cps2Bus, s: Service) -> bool {
    let simple = |d1: u32| Command { d1, d2: 0, d3: 0 };
    let cmd = match s {
        Service::GeneralStop => simple(0xff00),
        Service::Ff05 => simple(0xff05),
        Service::Ff06 { lock, line } => has_delay(b, 0xff06, lock, line),
        Service::Ff07 { lock, line } => has_delay(b, 0xff07, lock, line),
        Service::Ff08(p) => simple((p as u32) << 16 | 0xff08),
        Service::Ff09(p) => simple((p as u32) << 16 | 0xff09),
    };
    enqueue(b, cmd)
}

fn has_delay(b: &mut Cps2Bus, command: u16, lock: u8, line: u16) -> Command {
    wr8(b, SERVICE_LOCK, lock);
    let line = DELAYS.wrapping_add(line as i16 as u32);
    let delay = crate::tables::engine().word(b, line);
    wr16(b, a5(ram::DELAY), delay);
    let high = crate::tables::engine().word(b, line + 2) as u32;
    Command { d1: high << 16 | command as u32, d2: 0, d3: 0 }
}

/// Alternate-palette version (`A5+0x70` non-zero).
fn version_alternative(b: &mut Cps2Bus) -> bool {
    rd16(b, a5(0x70)) != 0
}

/// 0xee044: wrappers' private stack empty (pointer = its own address).
pub fn clear_private_stack(b: &mut Cps2Bus) {
    let p = a5(ram::PRIVATE_STACK);
    wr32(b, p, p);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every table that a sound reads is in `port/extract/sound_tables.txt`, the list from which the extraction takes
    /// them (`sound/*` of the engine data).
    #[test]
    fn tables_listed_for_the_extraction() {
        let listed: std::collections::BTreeSet<(String, u32)> = include_str!("../../extract/sound_tables.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                let (kind, at) = l.split_once(' ').unwrap();
                (kind.to_string(), u32::from_str_radix(at.trim_start_matches("0x"), 16).unwrap())
            })
            .collect();
        for s in table::WRAPPERS.iter().chain(&OTHERS) {
            let table = match s.kind {
                Kind::Wrapper(w) => match w.cmd {
                    Cmd::Alternate(t) => Some(("Alternate", t)),
                    Cmd::ByObject(t) => Some(("ByObject", t)),
                    Cmd::ByByte2(t) => Some(("ByByte2", t)),
                    _ => None,
                },
                Kind::Dispatch(d) => Some(match d {
                    Dispatch::ByD4(t) => ("ByD4", t),
                    Dispatch::ByVariant(t) => ("ByVariant", t),
                    Dispatch::ByVariantLong(t) => ("ByVariantLong", t),
                    Dispatch::ByVariantAndCategory(t) => ("ByVariantAndCategory", t),
                    Dispatch::ByA1(t) => ("ByA1", t),
                    Dispatch::Music { table, .. } => ("Music", table),
                }),
                _ => None,
            };
            if let Some((kind, at)) = table {
                assert!(listed.contains(&(kind.to_string(), at)), "{kind} {at:#x} (sound {:#x}) missing", s.entry);
            }
        }
    }
}
