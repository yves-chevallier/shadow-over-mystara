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

use std::sync::OnceLock;

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::objects::Obj;
use crate::{regs, Native};

#[path = "sound_table.rs"]
mod table;

/// Offsets relative to A5 (signed words/bytes extended: `A5 + n` modulo 2^32).
mod ram {
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
const MASK_HEAD: u16 = 0x3f0;
const ENTRY: u16 = 0x10;
/// Pan table (one byte per 8-pixel slice of screen), in ROM.
const TABLE_PAN: u32 = 0xee04e;

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
const DEBUG_FLAG: u32 = 0xc0;
const DEBUG_DISPLAY: u32 = 0x7fc6;
const HEX_BYTE: u32 = 0x29d78;
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

fn show_locks(c: &mut Cpu, b: &mut Cps2Bus) {
    let flag = rd16(b, DEBUG_FLAG);
    c.flags_logic(flag as u32, Size::Word);
    if flag == 0 {
        return;
    }
    let shown = rd16(b, a5(DEBUG_DISPLAY));
    c.flags_logic(shown as u32, Size::Word);
    if shown == 0 {
        return;
    }
    c.d[1] = 2;
    for (at, latch) in LATCH_DISPLAY {
        if let Some(at) = at {
            c.a[1] = at;
        }
        c.d[0] = c.d[0] & !0xff | rd8(b, latch) as u32;
        crate::call(c, b, HEX_BYTE);
    }
}

/// Address `A5 + off` (A5 is always `mem::A5`; negative offsets are written modulo 2^32).
const fn a5(off: u32) -> u32 {
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

/// Outcome of an enqueue, with the contents of the registers that the original leaves behind.
struct Queueing {
    /// Low word of D0 on return (new write head, or computed head if the queue is full).
    d0: u16,
    /// Cell written (A1 = end of the write), if the command was enqueued.
    written: Option<u32>,
}

/// Can the command be emitted? (sound allowed: 0xe0836)
fn sound_allowed(b: &mut Cps2Bus) -> bool {
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

/// Enqueues a command (0xe0848, without the permission test).
fn enqueue(b: &mut Cps2Bus, cmd: Command) -> Queueing {
    let read = rd16(b, a5(ram::READ_HEAD));
    let write = rd16(b, a5(ram::WRITE_HEAD));
    if queue_full(b) {
        return Queueing { d0: read.wrapping_sub(ENTRY) & MASK_HEAD, written: None };
    }
    let case = a5(ram::FILE).wrapping_add(write as i16 as u32);
    wr32(b, case, cmd.d1);
    wr32(b, case + 4, cmd.d2);
    wr32(b, case + 8, cmd.d3);
    let next = write.wrapping_add(ENTRY) & MASK_HEAD;
    wr16(b, a5(ram::WRITE_HEAD), next);
    Queueing { d0: next, written: Some(case + 8) }
}

/// Reproduces D0, A1 and the flags left by 0xe0848.
fn finish_enqueue(c: &mut Cpu, e: &Queueing) {
    c.d[0] = c.d[0] & 0xffff_0000 | e.d0 as u32;
    match e.written {
        Some(end) => {
            c.a[1] = end;
            c.flags_logic(e.d0 as u32, Size::Word);
        }
        None => c.sr = c.sr & 0xff00 | SR_C, // move #1,CCR: only the carry is raised
    }
}

fn register_command(c: &Cpu) -> Command {
    Command { d1: c.d[1], d2: c.d[2], d3: c.d[3] }
}

/// 0xe0836: enqueues the command if sound is allowed; returns `false` if nothing is enqueued (sound forbidden, queue
/// full).
pub fn emit(b: &mut Cps2Bus, cmd: Command) -> bool {
    sound_allowed(b) && enqueue(b, cmd).written.is_some()
}

/// 0xe0848: enqueues D1, D2, D3 (C raised if the queue is full).
fn enqueue_direct(c: &mut Cpu, b: &mut Cps2Bus) {
    let e = enqueue(b, register_command(c));
    finish_enqueue(c, &e);
    rts(c, b);
}

/// 0xe0836: enqueues D1, D2, D3 if sound is allowed.
fn enqueue_if_sound(c: &mut Cpu, b: &mut Cps2Bus) {
    if sound_allowed(b) {
        enqueue_direct(c, b);
    } else {
        c.sr = c.sr & 0xff00 | SR_C;
        rts(c, b);
    }
}

pub const ENQUEUE_IF_SOUND: Native = Native {
    entry: 0xe0836,
    name: "sound_enqueue",
    run: crate::Body::WithCpu(enqueue_if_sound),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

pub const ENQUEUE_DIRECT: Native = Native {
    entry: 0xe0848,
    name: "sound_enqueue_direct",
    run: crate::Body::WithCpu(enqueue_direct),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// The wrappers

/// "Program space" read: the command tables are stored in the code and read by
/// PC-relative accesses, which see the decrypted opcodes.
fn program_read(b: &mut Cps2Bus, address: u32) -> u16 {
    m68k::Bus::read_prog16(b, address & 0xff_ffff)
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

/// Description of an "sfx" entry point: the command is D1, its parameters D2 and D3.
#[derive(Clone, Copy, Debug)]
pub struct Wrapper {
    pub entry: u32,
    pub cmd: Cmd,
    pub d2: u32,
    pub d3: u32,
    pub opts: u8,
}

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

/// Plays sound `sound` (entry of an `sfx_*` wrapper or of a dispatcher, its identifier) for object `object` (A0 of
/// the original). The few sounds that also read D4 or A1 go through [`play_with`].
pub fn play(b: &mut Cps2Bus, sound: u32, object: Obj) {
    play_with(b, sound, Source { object: Some(object), ..Source::default() });
}

/// Plays a sound that reads nothing from its caller (no pan, player or variant: music, interface
/// noises), for callers without an object.
pub fn play_global(b: &mut Cps2Bus, sound: u32) {
    debug_assert!(!reads_object(sound), "sound {sound:x}: reads its object, use jouer");
    play_with(b, sound, Source::default());
}

/// Plays sound `sound` with everything it may read (see [`Source`]).
pub fn play_with(b: &mut Cps2Bus, sound: u32, source: Source) {
    if let Ok(i) = table::WRAPPERS.binary_search_by_key(&sound, |w| w.entry) {
        play_wrapper(b, &table::WRAPPERS[i], source);
    } else if let Some(&(_, kind)) = DISPATCHES.iter().find(|(e, _)| *e == sound) {
        dispatch(b, kind, source);
    } else if let Some(&(_, normal, alternate)) = BY_VERSION.iter().find(|(e, ..)| *e == sound) {
        let target = if version_alternative(b) { alternate } else { normal };
        play_with(b, target, source);
    } else if sound == NONE.entry {
        // objects without sound
    } else {
        panic!("sound {sound:x}: neither wrapper nor dispatcher")
    }
}

/// Does the sound read its object (A0)? The targets of the dispatchers by D4 or A1 are only known at run time:
/// presumed to read it.
fn reads_object(sound: u32) -> bool {
    if let Ok(i) = table::WRAPPERS.binary_search_by_key(&sound, |w| w.entry) {
        let w = &table::WRAPPERS[i];
        return w.opts & PAN != 0 || matches!(w.cmd, Cmd::ByObject(_) | Cmd::ByByte2(_));
    }
    if let Some(&(_, kind)) = DISPATCHES.iter().find(|(e, _)| *e == sound) {
        return !matches!(kind, Dispatch::Music { .. });
    }
    if let Some(&(_, normal, alternate)) = BY_VERSION.iter().find(|(e, ..)| *e == sound) {
        return reads_object(normal) || reads_object(alternate);
    }
    sound != NONE.entry
}

/// Pan (0-0x2f -> byte of the ROM table) of an object: screen position of the object relative to
/// the camera, clamped to 0..0x178, in 8-pixel slices.
fn pan(b: &mut Cps2Bus, object: u32) -> u8 {
    let x = rd16(b, object + 8);
    let camera_2 = rd8(b, a5(ram::CAMERA_CHOICE)) != 0;
    let camera = rd16(b, a5(if camera_2 { ram::CAMERA_2_X } else { ram::CAMERA_1_X }));
    let gap = x.wrapping_sub(camera);
    let gap = if (gap as i16) < 0 { 0 } else { gap };
    let gap = if gap < 0x180 { gap } else { 0x178 };
    rd8(b, TABLE_PAN + ((gap & 0x1ff) >> 3) as u32)
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

/// Native of a wrapper or a dispatcher (calls by address): the core, with what it reads from the registers;
/// all registers are returned intact, the flags are those of the last `move.l` of the private stack pointer.
fn native_sound_in(c: &mut Cpu, b: &mut Cps2Bus, sound: u32) {
    play_with(b, sound, Source { object: Some(Obj(c.a[0])), a1: Some(c.a[1]), d4: Some(c.d[4]) });
    let top = rd32(b, a5(ram::PRIVATE_STACK));
    c.flags_logic(top, Size::Long);
    rts(c, b);
}

/// The native routines of each described wrapper and of the dispatchers (built once, then kept).
pub fn native_wrappers() -> &'static [Native] {
    static LIST: OnceLock<Vec<Native>> = OnceLock::new();
    LIST.get_or_init(|| {
        let wrappers = table::WRAPPERS.iter().map(|w| (w.entry, "sound_wrapper"));
        let dispatches = DISPATCHES.iter().map(|&(entry, _)| (entry, "sound_dispatch"));
        let versions = BY_VERSION.iter().map(|&(entry, ..)| (entry, "sound_by_version"));
        wrappers
            .chain(dispatches)
            .chain(versions)
            .map(|(entry, name)| Native {
                entry,
                name,
                run: crate::Body::CpuIn(native_sound_in),
                touches_sound: false,
                outputs: 0, // all registers are returned intact: nothing to compare
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------------------------
// The dispatchers

/// Wrappers that do not compose their command themselves: they choose from a table, according to a
/// register or the object, another wrapper that they call (after their own save on the private
/// stack, D0-D3 zeroed), or else a music command.
#[derive(Clone, Copy, Debug)]
enum Dispatch {
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

const DISPATCHES: &[(u32, Dispatch)] = {
    use Dispatch::*;
    &[
        (0xe0c5c, Music { table: 0xe0ca6, strict: true }),
        (0xe0d8c, Music { table: 0xe0dd6, strict: false }),
        (0xe662a, ByVariantAndCategory(0xe666c)),
        (0xe6910, ByD4(0xe694a)),
        (0xe7622, ByVariant(0xe765c)),
        (0xe774c, ByVariant(0xe7786)),
        (0xe78e8, ByVariant(0xe7922)),
        (0xe7a12, ByVariant(0xe7a4c)),
        (0xe7c20, ByD4(0xe7c5a)),
        (0xe7e10, ByD4(0xe7e4a)),
        (0xe823a, ByVariantLong(0xe8274)),
        (0xec046, ByA1(0xec080)),
        (0xec200, ByA1(0xec23a)),
    ]
};

/// Long of the program space (tables read PC-relative).
fn program_read32(b: &mut Cps2Bus, address: u32) -> u32 {
    (program_read(b, address) as u32) << 16 | program_read(b, address + 2) as u32
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
            let a1 = rd32(b, list.wrapping_add(category));
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
    play_with(b, target, source);
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
        // Original dead code: debug screen, see `show_locks` (docs/code-mort.md, batch "CPU removal")
        unreachable!("display of the Z80 locks: the program word 0xc0 is zero in this version");
    }
    sent
}

/// The debug display of the locks is active (program word 0xc0 and `A5+0x7fc6` non-zero).
fn shown_locks(b: &mut Cps2Bus) -> bool {
    rd16(b, DEBUG_FLAG) != 0 && rd16(b, a5(DEBUG_DISPLAY)) != 0
}

/// `send` with the registers of the original (D0: read head, D0 high = -1 if silence; A4 after the entry
/// transmitted; flags of the lock display test).
fn send_to_z80(c: &mut Cpu, b: &mut Cps2Bus) {
    let silence = rd8(b, a5(ram::SILENCE)) != 0;
    if let Some(entry) = send(b) {
        c.a[4] = entry + 12;
    }
    let read = rd16(b, a5(ram::READ_HEAD));
    c.d[0] = if silence { 0xffff_0000 } else { 0 } | read as u32;
    show_locks(c, b);
    rts(c, b);
}

pub const SEND_TO_Z80: Native = Native {
    entry: 0x68c,
    name: "sound_send_z80",
    run: crate::Body::WithCpu(send_to_z80),
    touches_sound: true,
    outputs: 0, // VBLANK saved all the registers
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

fn service_enqueue(b: &mut Cps2Bus, s: Service) -> Queueing {
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
    let delay = rd16(b, line);
    wr16(b, a5(ram::DELAY), delay);
    let high = rd16(b, line + 2) as u32;
    Command { d1: high << 16 | command as u32, d2: 0, d3: 0 }
}

/// Native of a service command: all registers are returned (movem on the stack), only the flags of
/// 0xe0848 remain.
fn service_native_run(c: &mut Cpu, b: &mut Cps2Bus, s: Service) {
    let e = service_enqueue(b, s);
    let (d0, a1) = (c.d[0], c.a[1]);
    finish_enqueue(c, &e);
    c.d[0] = d0;
    c.a[1] = a1;
    rts(c, b);
}

/// Service command of entry `entry`, from D0 and D1 (for the natives).
fn service_in(entry: u32, c: &Cpu) -> Service {
    let (d0, d1) = (c.d[0] as u16, c.d[1] as u8);
    match entry {
        0xe0882 => Service::GeneralStop,
        0xe0a10 => Service::Ff05,
        0xe0a2e => Service::Ff06 { lock: d1, line: d0 },
        0xe0a40 => Service::Ff06 { lock: 0xff, line: d0 },
        0xe0a70 => Service::Ff07 { lock: d1, line: d0 },
        0xe0a82 => Service::Ff07 { lock: 0, line: d0 },
        0xe0ad8 => Service::Ff08(d0),
        0xe0af8 => Service::Ff09(d0),
        _ => unreachable!("{entry:x} is not a service command"),
    }
}

const fn service_native(entry: u32, name: &'static str, touches_sound: bool) -> Native {
    Native { entry, name, run: crate::Body::CpuIn(|c, b, entry| service_native_run(c, b, service_in(entry, c))), touches_sound, outputs: regs::CCR }
}

pub const SERVICE_FF00: Native = service_native(0xe0882, "sound_service_ff00", false);
pub const SERVICE_FF05: Native = service_native(0xe0a10, "sound_service_ff05", false);
pub const SERVICE_FF06_D1: Native = service_native(0xe0a2e, "sound_service_ff06", true);
pub const SERVICE_FF06: Native = service_native(0xe0a40, "sound_service_ff06", true);
pub const SERVICE_FF07_D1: Native = service_native(0xe0a70, "sound_service_ff07", true);
pub const SERVICE_FF07: Native = service_native(0xe0a82, "sound_service_ff07", true);
pub const SERVICE_FF08: Native = service_native(0xe0ad8, "sound_service_ff08", false);
pub const SERVICE_FF09: Native = service_native(0xe0af8, "sound_service_ff09", false);

/// Entries that test the alternate-palette version (`A5+0x70` non-zero) before calling the wrapper
/// `normal` or, in this version, the wrapper `alternate`: 0xe2240 / 0xe22e8 (character noise),
/// 0xe2430 (noise per object), 0xe27da (noise per kind), 0xe285c (sound 0xd5 or 0xd6, cleric's prayer), 0xe2a74 (sound
/// 0xf7 or 0xf6, magician's storm), 0xe296c (sound 0xea or 0xe6, elf's moon: hole found by the bot
/// `longue_d`).
const BY_VERSION: [(u32, u32, u32); 7] = [
    (0xe2240, 0xe2248, 0xe2390),
    (0xe22e8, 0xe22f0, 0xe2390),
    (0xe2430, 0xe2438, 0xe24d8),
    (0xe27da, 0xe27e2, 0xe28a4),
    (0xe285c, 0xe2864, 0xe28a4),
    (0xe2a74, 0xe2a7a, 0xe2a34),
    (0xe296c, 0xe2974, 0xe29f4),
];

/// Alternate-palette version (`A5+0x70` non-zero).
fn version_alternative(b: &mut Cps2Bus) -> bool {
    rd16(b, a5(0x70)) != 0
}

/// 0xee044: wrappers' private stack empty (pointer = its own address).
pub fn clear_private_stack(b: &mut Cps2Bus) {
    let p = a5(ram::PRIVATE_STACK);
    wr32(b, p, p);
}

/// `clear_private_stack`; A0 = the pointer, flags of its value.
fn init_private_stack(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_private_stack(b);
    let p = a5(ram::PRIVATE_STACK);
    c.a[0] = p;
    c.flags_logic(p, Size::Long);
    rts(c, b);
}

pub const INIT_PRIVATE_STACK: Native = Native {
    entry: 0xee044,
    name: "sound_init_stack",
    run: crate::Body::WithCpu(init_private_stack),
    touches_sound: false,
    outputs: regs::a(0) | regs::CCR,
};

/// 0xec238: empty sound (a plain `rts`), target of the dispatcher 0xec200 for objects without sound.
pub const NONE: Native = Native {
    entry: 0xec238,
    name: "sound_none",
    run: crate::Body::WithCpu(rts),
    touches_sound: false,
    outputs: regs::ALL,
};
