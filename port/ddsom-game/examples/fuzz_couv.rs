//! Coverage-guided differential fuzz: validates against the original the paths of native routines that the
//! scenarios do not exercise (docs/couverture-fonctions.md, "fuzz validation").
//!
//! The scenario plays the game with the original code alone (`validate`'s reference machine). At calls to a
//! targeted native routine, the real state is kept as a base; copies of it are perturbed and then run by the
//! original code (interpreted, coverage collected) and by the native, and compared (memory, contract registers, PC).
//!
//! The perturbations target what the original reads: each base is first run by the original with a bus that
//! logs RAM reads; the bytes and words read are then rewritten (small, extreme, neighboring values,
//! flipped bits). Reads made just before a conditional jump of which one side was never taken by the
//! scenarios (bitmap `FUZZ_UNION`, merged output of `cov`) are "hot": they are drawn more often, with
//! the immediate value compared and its neighbors. A copy that reaches a new instruction enters the corpus and
//! will be perturbed again (evolutionary fuzz).
//!
//!   fuzz_couv <roms> <eeprom> <scenario> <frames> <targets> <coverage.bin>
//!
//! Targets (comma-separated): zone code from docs/atlas.md (`C04`), hex entry address (`0x8ec02`) or
//! native name prefix (`name:boss_`). `-name:prefix` excludes. The output is a bitmap in the format of `cov`
//! (original instructions executed during the trials), and `<exit>.identiques` those of the trials without mismatch.
//!
//! Environment: FUZZ_UNION (scenario bitmap), FUZZ_ATLAS (docs/atlas.md), FUZZ_PERIODE (16: one call out of
//! n), FUZZ_POINTS (400 calls kept at most per native), FUZZ_ESSAIS (24 per call kept), FUZZ_GRAINE, FUZZ_PAS (at most 300,000 instructions for the original),
//! FUZZ_EXEMPLES (3 per native), FUZZ_TRACE=1 (trace of the original for the first mismatch).

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use m68k::decode::Fetch;
use m68k::{Cpu, Ea, Op};

use ddsom_game::mem::rd32;

/// Original's bus during a trial: logs work RAM reads (address, size, PC).
struct Journal<'a> {
    b: &'a mut Cps2Bus,
    pc: u32,
    reads: Option<&'a mut Vec<(u32, u8, u32)>>,
}

impl Journal<'_> {
    fn note(&mut self, a: u32, size: u8) {
        if let Some(l) = self.reads.as_deref_mut() {
            let a = a & 0xff_ffff;
            // neither kernel RAM (tasks, stacks), nor the private stack of the sound wrappers and its pointer, nor the
            // sorted lists (heads, object links): states that no code produces
            if a >= 0xff_1000 && !(0xff_2900..0xff_2a20).contains(&a) && !list_link(a) && !existence_or_type(a) && l.len() < 200_000 {
                // long read: two consecutive words read by the same instruction
                if let Some(d) = l.last_mut().filter(|d| size == 2 && d.1 == 2 && d.2 == self.pc && d.0 + 2 == a) {
                    d.1 = 4;
                    return;
                }
                l.push((a, size, self.pc));
            }
        }
    }
}

impl Fetch for Journal<'_> {
    fn fetch16(&mut self, a: u32) -> u16 {
        self.b.fetch16(a)
    }
}

impl m68k::Bus for Journal<'_> {
    fn read8(&mut self, a: u32) -> u8 {
        self.note(a, 1);
        self.b.read8(a)
    }
    fn read16(&mut self, a: u32) -> u16 {
        self.note(a, 2);
        self.b.read16(a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        original_write(self.pc, a, 1, v as u32);
        self.b.write8(a, v)
    }
    fn write16(&mut self, a: u32, v: u16) {
        original_write(self.pc, a, 2, v as u32);
        self.b.write16(a, v)
    }
    fn read_prog16(&mut self, a: u32) -> u16 {
        self.b.read_prog16(a)
    }
    fn reset_devices(&mut self) {
        self.b.reset_devices()
    }
    fn irq_ack(&mut self, l: u8) {
        m68k::Bus::irq_ack(self.b, l)
    }
}

thread_local! {
    /// Address whose writes by the original are reported (`FUZZ_SURVEILLE` diagnostic), 0: none.
    static WATCHED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static WATCHED_END: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// PC of the original whose registers are shown (`FUZZ_PC=address,...`) during the watch.
    static WATCHED_PCS: Vec<u32> = std::env::var("FUZZ_PC")
        .map(|v| v.split(',').filter_map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).ok()).collect())
        .unwrap_or_default();
}

fn original_write(pc: u32, a: u32, size: u32, v: u32) {
    let s = WATCHED.with(|x| x.get());
    let f = WATCHED_END.with(|x| x.get()).max(s + 1);
    let a = a & 0xff_ffff;
    if s != 0 && a < f && a + size > s {
        eprintln!("ORIGINAL {pc:06x} : {a:06x} = {v:x}");
    }
}

/// A perturbation: write `value` (1, 2 or 4 bytes) at `address`.
#[derive(Clone, Copy, Debug)]
struct Mutation {
    address: u32,
    size: u8,
    value: u32,
    /// value already seen in this field (same field of another pool object, another call) or compared by the code
    plausible: bool,
}

/// Key of a field: (pool, object type, offset in the slot) for objects, (player, offset) for
/// players, otherwise the address.
fn key(b: &Cps2Bus, a: u32) -> u32 {
    use ddsom_game::objects::pools::POOLS;
    for (i, p) in POOLS.iter().enumerate() {
        let start = ddsom_game::mem::A5 + p.first;
        if (start..start + p.stride * p.slots).contains(&a) {
            let slot = start + (a - start) / p.stride * p.stride;
            let type_ = read(b, slot + 2, 1);
            return 0x0100_0000 | (i as u32) << 20 | type_ << 12 | (a - slot);
        }
    }
    if (0xff_85de..0xff_85de + 0x800).contains(&a) {
        return 0x0200_0000 | ((a - 0xff_85de) % 0x200);
    }
    a
}

/// Existence (`+0`) and type (`+2`) bytes of the pool objects: an object is neither born nor changes type under
/// a handler (its other fields would not have the state of this type, and the dispatch loop would leave
/// registers of another handler).
fn existence_or_type(a: u32) -> bool {
    use ddsom_game::objects::pools::POOLS;
    POOLS.iter().any(|p| {
        let start = ddsom_game::mem::A5 + p.first;
        (start..start + p.stride * p.slots).contains(&a) && matches!((a - start) % p.stride, 0 | 2)
    })
}

/// Heads and tails of the sorted lists (A5+0x5be-0x5c5) and `+0x24`/`+0x26` links of the listed objects and players.
fn list_link(a: u32) -> bool {
    use ddsom_game::objects::pools::POOLS;
    if (0xff_85be..0xff_85c6).contains(&a) {
        return true;
    }
    let link = |start: u32, step: u32, n: u32| (start..start + step * n).contains(&a) && (0x24..0x28).contains(&((a - start) % step));
    link(0xff_85de, 0x200, 4) || POOLS.iter().any(|p| p.listed && link(ddsom_game::mem::A5 + p.first, p.stride, p.slots))
}

/// Values seen for each field (key, size) in the base states.
type Domains = HashMap<(u32, u8), Vec<u32>>;

fn read(b: &Cps2Bus, a: u32, size: u8) -> u32 {
    let word = |a: u32| b.ram[(a as usize & 0xffff) >> 1] as u32;
    match (size, a & 1) {
        (4, _) => word(a) << 16 | word(a + 2),
        (2, _) => word(a),
        (_, 0) => word(a) >> 8,
        _ => word(a) & 0xff,
    }
}

/// Hot read: made just before a conditional jump taken on one side only, with the values that could
/// flip the jump.
#[derive(Clone, Debug)]
struct HotSpot {
    address: u32,
    size: u8,
    values: Vec<u32>,
}

struct Base {
    c: Cpu,
    b: Cps2Bus,
    // state synthesized from the call of another entry (substitution)
    subst: bool,
    reads: Vec<(u32, u8)>,
    hot_spots: Vec<HotSpot>,
}

#[derive(Default)]
struct Summary {
    calls: u64,
    tests: u64,
    gaps: u64,
    panics: u64,
    abandons: u64,
    // discarded trials: the original jumped out of the code (jump table read out of its bounds)
    outside_code: u64,
    // trials where the original goes through an instruction the scenarios do not execute, and mismatches among them
    new_count: u64,
    new_gaps: u64,
    // mismatches whose perturbations are all plausible
    plausible_gaps: u64,
    new_total: u64,
    examples: Vec<String>,
    // mismatch signatures already seen (first address in mismatch): number of occurrences
    signatures: BTreeMap<String, u64>,
}

struct Target {
    n: &'static ddsom_game::Native,
    bases: Vec<Base>,
    // (base, perturbations) that reached a new instruction
    corpus: Vec<(usize, Vec<Mutation>)>,
    summary: Summary,
    // calls kept for the fuzz
    points: u64,
    // atlas zone of the entry
    zone: String,
    // handler of an object class: (pool, type)
    handler: Option<(usize, u8)>,
    // synthesized calls (entry never called, tried from the state of a neighbor)
    substitutions: u64,
    // bases that do not go through the targeted zones
    unfocused: u64,
}

struct State {
    targets: HashMap<u32, Target>,
    union: Vec<u8>,
    seen: Vec<u8>,
    // instructions executed by trials without mismatch (the original and the native are identical)
    identical_seen: Vec<u8>,
    // conditional jumps taken on one side only by the scenarios (address -> (target, fall-through))
    boundary: HashMap<u32, (u32, u32)>,
    rng: Rng,
    domains: Domains,
    // try the never-called entries from the state of a neighbor (`FUZZ_SUBSTITUTS`)
    substitutes: bool,
    // targeted zones (`FUZZ_FOCUS=F3,C1`): only the bases that go through them are fuzzed
    focus: Vec<(u32, u32)>,
    period: u64,
    points_max: u64,
    tests: u32,
    max_step: u32,
    examples: usize,
    trace: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn bit(bm: &[u8], a: u32) -> bool {
    let w = (a as usize & 0xfffff) >> 1;
    bm.get(w >> 3).is_some_and(|x| x >> (w & 7) & 1 != 0)
}

fn set_bit(bm: &mut [u8], a: u32) -> bool {
    let w = (a as usize & 0xfffff) >> 1;
    let is_new = bm[w >> 3] >> (w & 7) & 1 == 0;
    bm[w >> 3] |= 1 << (w & 7);
    is_new
}

struct Rng(u64);

impl Rng {
    fn rng(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn over(&mut self, n: u64) -> u64 {
        self.rng() % n.max(1)
    }
}

impl State {
    fn rng(&mut self) -> u64 {
        self.rng.rng()
    }
    fn over(&mut self, n: u64) -> u64 {
        self.rng.over(n)
    }
}

/// Result of a run of the original.
struct Original {
    c: Cpu,
    b: Cps2Bus,
    pcs: Vec<u32>,
    // stayed in the code (instruction starts, jump tables read within their bounds)
    valid: bool,
}

/// Runs the original code (interpreted) until the routine returns; returns the final state and the PCs executed.
fn run_original(c0: &Cpu, b0: &Cps2Bus, max_step: u32, mut reads: Option<&mut Vec<(u32, u8, u32)>>) -> Option<Original> {
    let (mut c, mut b) = (c0.clone(), b0.clone());
    let ret = rd32(&mut b, c.a[7]);
    let sp_after = c.a[7].wrapping_add(4);
    let mut pcs = Vec::new();
    let mut valid = true;
    let r = catch_unwind(AssertUnwindSafe(|| {
        for _ in 0..max_step {
            if c.pc == ret && c.a[7] == sp_after {
                return true;
            }
            pcs.push(c.pc);
            if WATCHED.with(|x| x.get()) != 0 && WATCHED_PCS.with(|p| p.contains(&c.pc)) {
                eprintln!("ORIGINAL en {:06x} : D={:08x?} A={:08x?}", c.pc, c.d, c.a);
            }
            if valid && !in_tables(&c, &mut b) {
                valid = false;
            }
            let mut j = Journal { b: &mut b, pc: c.pc, reads: reads.as_deref_mut() };
            c.step(&mut j);
        }
        false
    }));
    match r {
        Ok(true) => Some(Original { c, b, pcs, valid }),
        _ => None,
    }
}

/// Instruction starts of listing.txt (`FUZZ_LISTING`); empty: no check.
static INSTRUCTIONS: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

fn instruction(a: u32) -> bool {
    INSTRUCTIONS.get().is_none_or(|bm| bm.is_empty() || a >= 0x10_0000 || bit(bm, a)) || execute(a)
}

/// Jump targets of the listing (`LAB_xxxxxxxx:`, `FUN_`): code, never a table.
static LABELS: std::sync::OnceLock<HashSet<u32>> = std::sync::OnceLock::new();

fn label(a: u32) -> bool {
    LABELS.get().is_some_and(|e| e.contains(&a))
}

/// Instructions executed by the scenarios (`FUZZ_UNION`).
static UNION: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

fn execute(a: u32) -> bool {
    a < 0x10_0000 && a & 1 == 0 && UNION.get().is_some_and(|u| bit(u, a))
}

thread_local! {
    // length of the jump tables (table address -> number of entries)
    static TABLES: RefCell<HashMap<(u32, bool), u32>> = RefCell::new(HashMap::new());
}

/// Number of entries of a jump table: 16-bit offsets from the table (it ends where the first following
/// target begins), or long addresses; each target must be an instruction start.
fn table_length(b: &mut Cps2Bus, base: u32, long: bool) -> u32 {
    TABLES.with(|t| {
        *t.borrow_mut().entry((base, long)).or_insert_with(|| {
            let mut end = u32::MAX;
            for i in 0..512u32 {
                // the table ends where code executed by the scenarios begins (the listing sometimes disassembles the
                // tables as instructions)
                let a = base + if long { 4 } else { 2 } * i;
                if i > 0 && (execute(a) || label(a)) {
                    return i;
                }
                let target = if long {
                    (b.fetch16(base + 4 * i) as u32) << 16 | b.fetch16(base + 4 * i + 2) as u32
                } else {
                    base.wrapping_add(b.fetch16(base + 2 * i) as i16 as i32 as u32)
                };
                if target >= 0x10_0000 || target & 1 != 0 || !instruction(target) {
                    return i;
                }
                if !long && target > base {
                    end = end.min(target);
                }
                if !long && base + 2 * (i + 1) >= end {
                    return i + 1;
                }
            }
            512
        })
    })
}

/// False if the instruction at PC reads a jump table (`move.w (d8,PC,Xn)` or `move.l` followed by a `jmp`/`jsr`)
/// beyond its entries: the perturbed state is impossible.
fn in_tables(c: &Cpu, b: &mut Cps2Bus) -> bool {
    if c.pc & 1 != 0 || !instruction(c.pc) {
        return false;
    }
    let ins = m68k::decode(b, c.pc);
    let index = |ext: u16| {
        let r = ((ext >> 12) & 7) as usize;
        let v = if ext & 0x8000 != 0 { c.a[r] } else { c.d[r] };
        if ext & 0x800 != 0 { v } else { v as u16 as i16 as i32 as u32 }
    };
    let (size, base, ext) = match ins.op {
        Op::Move(t, Ea::PcIdx(base, ext), _) | Op::Movea(t, Ea::PcIdx(base, ext), _) => (t, base, ext),
        // `jmp`/`jsr (d8,PC,Xn)` in a list of `bra`: the target must be one of them
        Op::Jmp(Ea::PcIdx(base, ext)) | Op::Jsr(Ea::PcIdx(base, ext)) => {
            let mut a = base;
            let mut starts = Vec::new();
            loop {
                let i = m68k::decode(b, a);
                if !matches!(i.op, Op::Bra(_)) || starts.len() > 256 {
                    break;
                }
                starts.push(a);
                a += i.len;
            }
            return starts.is_empty() || starts.contains(&base.wrapping_add(index(ext)));
        }
        _ => return true,
    };
    let next = m68k::decode(b, c.pc + ins.len);
    if !matches!(next.op, Op::Jmp(_) | Op::Jsr(_)) {
        return true;
    }
    let index = index(ext);
    let step = if size == m68k::Size::Long { 4 } else { 2 };
    index % step == 0 && index / step < table_length(b, base, step == 4)
}

/// Values that a conditional jump compares with the memory read by instruction `ins` (immediate value and neighbors,
/// tested bit), failing that the usual values.
fn hot_values(op: &Op) -> Vec<u32> {
    match *op {
        Op::Cmp(_, Ea::Imm(v), _) => vec![v, v.wrapping_add(1), v.wrapping_sub(1), 0],
        Op::Bit(_, m68k::decode::ShiftCount::Imm(n), _) => {
            let m = 1u32 << (n & 7);
            vec![m, 0, 0xff, m | 1]
        }
        _ => vec![0, 1, 0xff, 0x80, 2, 0xffff_ffff],
    }
}

/// The reads of a base (address, size) and its hot reads.
type BaseReads = (Vec<(u32, u8)>, Vec<HotSpot>);

/// Runs the original on the base to log its reads and spot the hot reads.
fn analyse_base(e: &State, c: &Cpu, b: &Cps2Bus) -> Option<BaseReads> {
    let mut reads = Vec::new();
    let o = run_original(c, b, e.max_step, Some(&mut reads))?;
    // targeted zones (`FUZZ_FOCUS`): the base must go through them
    if !e.focus.is_empty() && !o.pcs.iter().any(|&p| e.focus.iter().any(|(d, f)| (*d..*f).contains(&p))) {
        return Some((Vec::new(), Vec::new()));
    }
    let mut bb = b.clone();
    // reads per instruction
    let mut by_pc: HashMap<u32, Vec<(u32, u8)>> = HashMap::new();
    let mut order = Vec::new();
    let mut views = HashSet::new();
    for &(a, t, pc) in &reads {
        by_pc.entry(pc).or_default().push((a, t));
        if views.insert((a, t)) {
            order.push((a, t));
        }
    }
    // hot: reads of the two instructions preceding a jump of the frontier
    let mut hot_spots = Vec::new();
    let mut views_ch = HashSet::new();
    for w in o.pcs.windows(3) {
        let jump = w[2];
        if !e.boundary.contains_key(&jump) {
            continue;
        }
        for &pc in &w[..2] {
            if let Some(l) = by_pc.get(&pc) {
                let ins = m68k::decode(&mut bb, pc);
                for &(a, t) in l {
                    if views_ch.insert((a, t, pc)) {
                        hot_spots.push(HotSpot { address: a, size: t, values: hot_values(&ins.op) });
                    }
                }
            }
        }
    }
    // jump-table dispatches: the reads of the three previous instructions receive the valid indices
    // (raw, or divided by 2 or 4: the index is often doubled before the access)
    for w in o.pcs.windows(4) {
        let Some(indices) = table_indices(&mut bb, w[3]).filter(|v| !v.is_empty()) else { continue };
        let mut values: Vec<u32> = Vec::new();
        for i in indices {
            for d in [1, 2, 4] {
                if i % d == 0 && !values.contains(&(i / d)) {
                    values.push(i / d);
                }
            }
        }
        for &pc in &w[..3] {
            if let Some(l) = by_pc.get(&pc) {
                for &(a, t) in l {
                    if t <= 2 && views_ch.insert((a, t, pc)) {
                        hot_spots.push(HotSpot { address: a, size: t, values: values.clone() });
                    }
                }
            }
        }
    }
    Some((order, hot_spots))
}

/// Valid indices of the jump-table dispatch at `pc` (`move.w (d8,PC,Xn)` followed by a `jmp`/`jsr`, or
/// `jmp (d8,PC,Xn)` in a list of `bra`).
fn table_indices(b: &mut Cps2Bus, pc: u32) -> Option<Vec<u32>> {
    let ins = m68k::decode(b, pc);
    match ins.op {
        Op::Move(t, Ea::PcIdx(base, ext), _) | Op::Movea(t, Ea::PcIdx(base, ext), _) => {
            let next = m68k::decode(b, pc + ins.len);
            if !matches!(next.op, Op::Jmp(_) | Op::Jsr(_)) {
                return None;
            }
            let step = if t == m68k::Size::Long { 4 } else { 2 };
            let d8 = ext as i8 as i32 as u32;
            let _ = d8;
            Some((0..table_length(b, base, step == 4).min(64)).map(|i| i * step).collect())
        }
        Op::Jmp(Ea::PcIdx(base, _)) | Op::Jsr(Ea::PcIdx(base, _)) => {
            let mut a = base;
            let mut v = Vec::new();
            while v.len() < 64 {
                let i = m68k::decode(b, a);
                if !matches!(i.op, Op::Bra(_)) {
                    break;
                }
                v.push(a - base);
                a += i.len;
            }
            (!v.is_empty()).then_some(v)
        }
        _ => None,
    }
}

fn mutate(e: &mut Rng, domains: &Domains, base: &Base, muts: &mut Vec<Mutation>) {
    let n = 1 + e.over(4);
    for _ in 0..n {
        let hot = !base.hot_spots.is_empty() && e.over(2) == 0;
        let (address, size, values) = if hot {
            let h = &base.hot_spots[e.over(base.hot_spots.len() as u64) as usize];
            (h.address, h.size, Some(h.values.clone()))
        } else if !base.reads.is_empty() {
            let (a, t) = base.reads[e.over(base.reads.len() as u64) as usize];
            (a, t, None)
        } else {
            return;
        };
        // size: sometimes the other byte of the word, sometimes a single byte of a word
        let (address, size) = match (size, e.over(8)) {
            (2, 0) => (address + 1, 1),
            (2, 1) => (address, 1),
            (4, 0) => (address + 2, 2),
            _ => (address, size),
        };
        // the type of the current object (byte +2) does not change under its handler
        let current_type = (base.c.a[0] & 0xff_ffff) + 2;
        if (address..address + size as u32).contains(&current_type) {
            continue;
        }
        let current = read(&base.b, address, size);
        let mask = match size {
            1 => 0xff,
            2 => 0xffff,
            _ => 0xffff_ffff,
        };
        let domain = domains.get(&(key(&base.b, address), size)).filter(|d| d.len() > 1);
        let draw = e.over(10);
        let value = match (&values, domain) {
            (Some(v), _) if (5..8).contains(&draw) => v[e.over(v.len() as u64) as usize],
            (_, Some(d)) if draw < 8 => d[e.over(d.len() as u64) as usize],
            _ => match e.over(12) {
                0 => 0,
                1 => 1,
                2 => 2 * e.over(8) as u32,
                3 => e.over(16) as u32,
                4 => 0xffff_ffff,
                5 => 0x80,
                6 => current ^ (1 << e.over(8 * size as u64)),
                7 => current.wrapping_add(1),
                8 => current.wrapping_sub(1),
                9 => 0x8000,
                10 if !base.reads.is_empty() => {
                    // value of another read
                    let (a2, _) = base.reads[e.over(base.reads.len() as u64) as usize];
                    read(&base.b, a2 & !1, 2)
                }
                _ => e.rng() as u32,
            },
        } & mask;
        let plausible = values.as_ref().is_some_and(|v| v.contains(&value))
            || domains.get(&(key(&base.b, address), size)).is_some_and(|d| d.contains(&value));
        muts.push(Mutation { address, size, value, plausible });
    }
}

fn apply(b: &mut Cps2Bus, muts: &[Mutation]) {
    for m in muts {
        match m.size {
            1 => ddsom_game::mem::wr8(b, m.address, m.value as u8),
            2 => ddsom_game::mem::wr16(b, m.address & !1, m.value as u16),
            _ => ddsom_game::mem::wr32(b, m.address & !1, m.value),
        }
    }
}

fn describe(muts: &[Mutation]) -> String {
    muts.iter()
        .map(|m| {
            let h = if m.plausible { "" } else { "!" };
            match m.size {
                1 => format!("{:06x}={:02x}{h}", m.address, m.value),
                2 => format!("{:06x}.w={:04x}{h}", m.address, m.value),
                _ => format!("{:06x}.l={:08x}{h}", m.address, m.value),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Table of the native's Rust tasks, rebuilt from the FF0064 table and the original's stack heap (the
/// reference machine has no Rust table): states, counts, stacks, entries, free stacks.
fn table_from_original(b: &mut Cps2Bus) {
    use ddsom_game::mem::{rd32, rd8};
    use ddsom_game::tasks::table::{self, Slot, State, Resume};
    use ddsom_game::tools::kernel::task_at;
    let mut t = table::Table::default();
    for (k, e) in t.slots.iter_mut().enumerate() {
        let r = table::record(k);
        let Some(state) = State::from_byte(rd8(b, r)) else { continue };
        let entry = rd32(b, r + 4);
        *e = Slot {
            task: task_at(entry),
            state,
            count: rd8(b, r + 1),
            stack: rd32(b, r + 0xc),
            resume: task_at(entry).map_or(Resume::Address(entry), Resume::Entry),
            ..Slot::default()
        };
    }
    let top = match rd32(b, 0xff_005a) {
        0 => 0xff_01a6,
        s => s,
    };
    t.stacks = (top..0xff_01a6).step_by(4).rev().map(|a| rd32(b, a)).collect();
    *table::table(b) = t;
}

/// Runs the native on state `c0`/`b0` and compares it with the original's result `o`: returns the panic message or
/// the mismatches (empty: identical).
fn compare(n: &ddsom_game::Native, c0: &Cpu, b0: &Cps2Bus, o: &Original) -> Result<Vec<String>, String> {
    let (mut c1, mut b1) = (c0.clone(), b0.clone());
    b1.irq_horizon = u64::MAX;
    table_from_original(&mut b1);
    let r = catch_unwind(AssertUnwindSafe(|| n.execute(&mut c1, &mut b1)));
    if let Err(p) = r {
        return Err(p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default());
    }
    let mut diffs = Vec::new();
    let (c2, b2) = (&o.c, &o.b);
    for i in 0..8 {
        if n.outputs & ddsom_game::regs::d(i as u32) != 0 && c1.d[i] != c2.d[i] {
            diffs.push(format!("d{i}={:08x} instead of {:08x}", c1.d[i], c2.d[i]));
        }
        if (n.outputs & ddsom_game::regs::a(i as u32) != 0 || i == 7) && c1.a[i] != c2.a[i] {
            diffs.push(format!("a{i}={:08x} instead of {:08x}", c1.a[i], c2.a[i]));
        }
    }
    if n.outputs & ddsom_game::regs::CCR != 0 && c1.sr & 0x1f != c2.sr & 0x1f {
        diffs.push(format!("ccr={:02x} instead of {:02x}", c1.sr & 0x1f, c2.sr & 0x1f));
    }
    if c1.pc != c2.pc {
        diffs.push(format!("pc={:06x} instead of {:06x}", c1.pc, c2.pc));
    }
    // original kernel RAM: the native keeps the task table in Rust; compared separately
    if let Some(t) = ddsom_game::tasks::table::read(&b1) {
        diffs.extend(ddsom_game::tasks::table::gaps(t, &b2.ram, false));
    }
    for i in 0..0x800usize {
        if ddsom_game::tasks::table::kernel_ram(2 * i as u32) {
            b1.ram[i] = b2.ram[i];
        }
    }
    let sp = c2.a[7] & 0xff_ffff;
    ddsom_game::mem::diff_bus(&b1, b2, sp.saturating_sub(0x400)..sp, &mut diffs);
    Ok(diffs)
}

/// Signature of a mismatch: first thing that differs (or the panic).
fn signature(r: &Result<Vec<String>, String>) -> Option<String> {
    match r {
        Err(msg) => Some(format!("PANIQUE {}", msg.chars().take(100).collect::<String>())),
        Ok(d) if d.is_empty() => None,
        Ok(d) => Some(d[0].split('=').next().unwrap_or("").to_string()),
    }
}

/// The original stayed only on instruction starts of the listing (otherwise the perturbed state made it jump into
/// data, through a jump table read out of its bounds: impossible state, trial discarded).
fn valid_path(_: &State, o: &Original) -> bool {
    o.valid
}

/// A trial: the perturbed base, run by the original then by the native.
fn test(e: &mut State, entry: u32, ib: usize, muts: Vec<Mutation>) {
    let max_step = e.max_step;
    let state_of = |e: &State, muts: &[Mutation]| {
        let base = &e.targets[&entry].bases[ib];
        let mut b = base.b.clone();
        apply(&mut b, muts);
        (base.c.clone(), b)
    };
    let (c0, b0) = state_of(e, &muts);
    let o = match run_original(&c0, &b0, max_step, None) {
        Some(o) if valid_path(e, &o) => o,
        Some(_) => {
            e.targets.get_mut(&entry).unwrap().summary.outside_code += 1;
            return;
        }
        None => {
            e.targets.get_mut(&entry).unwrap().summary.abandons += 1;
            return;
        }
    };
    let mut new_total = 0;
    for &pc in &o.pcs {
        if set_bit(&mut e.seen, pc) && !bit(&e.union, pc) {
            new_total += 1;
        }
    }
    let is_new = o.pcs.iter().any(|&p| !bit(&e.union, p));
    let n = e.targets[&entry].n;
    let r = compare(n, &c0, &b0, &o);
    if matches!(&r, Ok(d) if d.is_empty()) {
        for &pc in &o.pcs {
            set_bit(&mut e.identical_seen, pc);
        }
    }
    let target = e.targets.get_mut(&entry).unwrap();
    target.summary.tests += 1;
    target.summary.new_count += is_new as u64;
    target.summary.new_total += new_total;
    if new_total > 0 && !muts.is_empty() {
        if target.corpus.len() < 64 {
            target.corpus.push((ib, muts.clone()));
        } else {
            let k = (o.pcs.len() + muts.len()) % 64;
            target.corpus[k] = (ib, muts.clone());
        }
    }
    let Some(sig) = signature(&r) else { return };
    if r.is_err() {
        target.summary.panics += 1;
    } else {
        target.summary.gaps += 1;
    }
    if is_new {
        target.summary.new_gaps += 1;
    }
    let outside = muts.iter().any(|m| !m.plausible);
    // mismatch limited to the output registers (no effect as long as no caller rereads them)
    let regs = matches!(&r, Ok(d) if d.iter().all(|x| x.starts_with('d') || x.starts_with('a') && !x.starts_with("au") || x.starts_with("ccr")));
    target.summary.plausible_gaps += (!outside && !regs && !target.bases[ib].subst) as u64;
    // entry never called, tried from the state of a neighbor: its input registers are those of another call
    let subst = target.bases[ib].subst;
    let sig = format!("{}{}{}{}{}{sig}", if subst { "SUBST " } else { "" }, if regs { "REG " } else { "" }, if muts.is_empty() { "BASE " } else { "" }, if outside { "HORS " } else { "" }, if is_new { "NEUF " } else { "" });
    let count = target.summary.signatures.entry(sig.clone()).or_insert(0);
    *count += 1;
    if *count != 1 || target.summary.examples.len() >= e.examples {
        return;
    }
    // reduction: remove the perturbations the mismatch does not depend on
    let sig0 = signature(&r);
    let mut min = muts.clone();
    let mut k = 0;
    while k < min.len() {
        let mut test = min.clone();
        test.remove(k);
        let (c, b) = state_of(e, &test);
        let guard = match run_original(&c, &b, max_step, None) {
            Some(o2) if valid_path(e, &o2) => signature(&compare(n, &c, &b, &o2)) == sig0,
            _ => false,
        };
        if guard {
            min = test;
        } else {
            k += 1;
        }
    }
    let (c0, b0) = state_of(e, &min);
    let o = run_original(&c0, &b0, max_step, None).unwrap_or(o);
    let r = compare(n, &c0, &b0, &o);
    let text = format!(
        "{} ; perturbations [{}] ; A0={:06x} A6={:06x} D0={:08x} D1={:08x} ; new path {}",
        match &r {
            Err(m) => format!("PANIQUE {m}"),
            Ok(d) => d.join(", "),
        },
        describe(&min),
        c0.a[0],
        c0.a[6],
        c0.d[0],
        c0.d[1],
        o.pcs.iter().filter(|&&p| !bit(&e.union, p)).take(16).map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" ")
    );
    #[cfg(feature = "watch_writes")]
    if let Ok(a) = std::env::var("FUZZ_SURVEILLE") {
        // Rust call stack of the native's writes to this address
        let (a, end) = a.split_once('-').unwrap_or((&a, "0"));
        let a = u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap();
        let end = u32::from_str_radix(end.trim_start_matches("0x"), 16).unwrap();
        ddsom_game::mem::watched::ADDRESS.store(a, std::sync::atomic::Ordering::Relaxed);
        ddsom_game::mem::watched::END.store(end, std::sync::atomic::Ordering::Relaxed);
        WATCHED_END.with(|x| x.set(end));
        eprintln!("SURVEILLE {a:06x} for {} {:06x} ({})", n.name, n.entry, describe(&min));
        WATCHED.with(|x| x.set(a));
        let _ = run_original(&c0, &b0, max_step, None);
        WATCHED.with(|x| x.set(0));
        let _ = compare(n, &c0, &b0, &o);
        ddsom_game::mem::watched::ADDRESS.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    let text = if e.trace {
        let mut bb = b0.clone();
        let object: String = (0..0xc0).map(|i| format!("{:02x}", ddsom_game::mem::rd8(&mut bb, c0.a[0] + i))).collect();
        format!("{text}\n      A0 {object}\n      TRACE {}", o.pcs.iter().take(4000).map(|p| format!("{p:x}")).collect::<Vec<_>>().join(" "))
    } else {
        text
    };
    e.targets.get_mut(&entry).unwrap().summary.examples.push(format!("[{sig}] {text}"));
}

fn fuzzer(entry: u32, c: &Cpu, b: &Cps2Bus) {
    STATE.with(|x| {
        let mut x = x.borrow_mut();
        let e = x.as_mut().unwrap();
        let period = e.period;
        let k = {
            let target = e.targets.get_mut(&entry).unwrap();
            target.summary.calls += 1;
            target.summary.calls
        };
        // the first calls, then one out of `period`, at most `points` times
        if k > 4 && k % period != 1 || e.targets[&entry].points >= e.points_max {
            return;
        }
        e.targets.get_mut(&entry).unwrap().points += 1;
        fuzz_from(e, entry, c, b, false);
        if !e.substitutes {
            return;
        }
        // an entry of the same zone (or a handler of the same pool) never called, from this state
        let here = &e.targets[&entry];
        let (zone, pool) = (here.zone.clone(), here.handler.map(|g| g.0));
        let mut candidates: Vec<u32> = e
            .targets
            .iter()
            .filter(|(_, y)| y.summary.calls == 0 && y.substitutions < e.points_max)
            .filter(|(_, y)| match (pool, y.handler) {
                (Some(p), Some((q, _))) => p == q,
                (None, None) => y.zone == zone,
                _ => false,
            })
            .map(|(&a, _)| a)
            .collect();
        if candidates.is_empty() {
            return;
        }
        candidates.sort();
        let y = candidates[e.over(candidates.len() as u64) as usize];
        let mut c2 = c.clone();
        let mut b2 = b.clone();
        c2.pc = y;
        if e.targets[&y].handler.is_none() && key(&b2, c2.a[0] & 0xff_ffff) < 0x0100_0000 {
            // A0 is not an object: a player (the sounds read the position of object A0)
            c2.a[0] = 0xff_85de + 0x200 * e.over(4) as u32;
        }
        if let Some((_, t)) = e.targets[&y].handler {
            // object A0 becomes of the type of `y`, often in its initial state
            let obj = c2.a[0] & 0xff_ffff;
            ddsom_game::mem::wr8(&mut b2, obj + 2, t);
            ddsom_game::mem::wr8(&mut b2, obj + 3, 0);
            if e.over(2) == 0 {
                ddsom_game::mem::wr32(&mut b2, obj + 4, 0);
            }
        }
        e.targets.get_mut(&y).unwrap().substitutions += 1;
        fuzz_from(e, y, &c2, &b2, true);
    });
}

/// Fuzz of `entry` from state `c`, `b` (PC on the entry): new base, then the trials.
fn fuzz_from(e: &mut State, entry: u32, c: &Cpu, b: &Cps2Bus, subst: bool) {
    // new base (at most 3 per native; then, replacement of a random base along with its corpus)
    let analysis = analyse_base(e, c, b);
    let Some((reads, hot_spots)) = analysis else {
        e.targets.get_mut(&entry).unwrap().summary.abandons += 1;
        return;
    };
    if !e.focus.is_empty() && reads.is_empty() {
        // the base does not go through the targeted zones: after 16, the native is no longer tried
        let target = e.targets.get_mut(&entry).unwrap();
        target.unfocused += 1;
        if target.unfocused >= 16 && target.bases.is_empty() {
            target.points = u64::MAX;
            target.substitutions = u64::MAX;
        }
        return;
    }
    for &(a, t) in &reads {
        let v = read(b, a, t);
        let d = e.domains.entry((key(b, a), t)).or_default();
        if d.len() < 48 && !d.contains(&v) {
            d.push(v);
        }
    }
    let base = Base { c: c.clone(), b: b.clone(), subst, reads, hot_spots };
    // the base itself (without perturbation): coverage and check
    let nb = e.targets[&entry].bases.len();
    let ib = if nb < 3 {
        e.targets.get_mut(&entry).unwrap().bases.push(base);
        nb
    } else {
        let ib = e.over(3) as usize;
        let target = e.targets.get_mut(&entry).unwrap();
        target.bases[ib] = base;
        target.corpus.retain(|(i, _)| *i != ib);
        ib
    };
    test(e, entry, ib, Vec::new());
    for _ in 0..e.tests {
        let (ib, mut muts) = {
            let nb = e.targets[&entry].bases.len() as u64;
            let nc = e.targets[&entry].corpus.len() as u64;
            if nc > 0 && !e.rng().is_multiple_of(3) {
                let k = (e.rng() % nc) as usize;
                let (ib, m) = &e.targets[&entry].corpus[k];
                (*ib, m.clone())
            } else {
                ((e.rng() % nb) as usize, Vec::new())
            }
        };
        let State { targets, rng, domains, .. } = &mut *e;
        mutate(rng, domains, &targets[&entry].bases[ib], &mut muts);
        test(e, entry, ib, muts);
    }
}

/// Jump tables of the ten object classes (0x398e2-0x39ff0, W0): handler -> (pool, type).
fn handlers(b: &mut Cps2Bus) -> HashMap<u32, (usize, u8)> {
    use ddsom_game::objects::pools::POOLS;
    const TABLES: [(u32, u32); 10] = [
        (0x10de, 0x39908),
        (0x179e, 0x39992),
        (0x299e, 0x39a78),
        (0x3a1e, 0x39b12),
        (0x3c5e, 0x39b96),
        (0x549e, 0x39ce0),
        (0x6ede, 0x39e76),
        (0x4a9e, 0x39eac),
        (0x62de, 0x39f5a),
        (0x795e, 0x39fa8),
    ];
    let mut g = HashMap::new();
    for (first, table) in TABLES {
        let pool = POOLS.iter().position(|p| p.first == first).expect("pool");
        for t in 0..128u32 {
            let a = table + 4 * t;
            if t > 0 && instruction(a) && INSTRUCTIONS.get().is_some_and(|i| !i.is_empty()) {
                break;
            }
            let entry = (b.fetch16(a) as u32) << 16 | b.fetch16(a + 2) as u32;
            if entry >= 0x10_0000 || entry & 1 != 0 || !instruction(entry) {
                break;
            }
            g.entry(entry).or_insert((pool, t as u8));
        }
    }
    g
}

fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    let target = STATE.with(|x| x.borrow().as_ref().unwrap().targets.contains_key(&pc));
    if target {
        fuzzer(pc, c, b);
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

/// Zones of docs/atlas.md: (start, end, code).
fn zones(atlas: &str) -> Vec<(u32, u32, String)> {
    let mut v = Vec::new();
    for l in std::fs::read_to_string(atlas).expect("atlas").lines() {
        let fields: Vec<&str> = l.split('|').map(str::trim).collect();
        if fields.len() > 3 && fields[1].starts_with("0x") && fields[1].len() == 17 && fields[1].as_bytes()[8] == b'-' {
            let d = u32::from_str_radix(&fields[1][2..8], 16).unwrap();
            let f = u32::from_str_radix(&fields[1][11..17], 16).unwrap();
            v.push((d, f, fields[2].to_string()));
        }
    }
    v
}

/// Conditional jumps of the program of which only one side was executed by the scenarios.
fn boundary(b: &mut Cps2Bus, union: &[u8]) -> HashMap<u32, (u32, u32)> {
    let mut f = HashMap::new();
    for w in 0..0x80000u32 {
        let pc = 2 * w;
        if !bit(union, pc) {
            continue;
        }
        let ins = m68k::decode(b, pc);
        let target = match ins.op {
            Op::Bcc(_, t) | Op::Dbcc(_, _, t) => t,
            _ => continue,
        };
        let next = pc + ins.len;
        if bit(union, target) != bit(union, next) {
            f.insert(pc, (target, next));
        }
    }
    f
}

/// Instruction starts of listing.txt (lines `address  mnemonic`); labels (`LAB_`, `; ---- FUN_`).
fn instructions(listing: &str) -> Vec<u8> {
    let mut bm = vec![0u8; 0x10000];
    let mut labels = HashSet::new();
    for l in std::fs::read_to_string(listing).expect("listing").lines() {
        for prefix in ["LAB_", "; ---- FUN_"] {
            if let Some(h) = l.strip_prefix(prefix) {
                if let Ok(a) = u32::from_str_radix(h.trim_end_matches(':').trim(), 16) {
                    labels.insert(a);
                }
            }
        }
        if l.len() > 8 && l.as_bytes()[6] == b' ' && l.as_bytes()[7] == b' ' {
            if let Ok(a) = u32::from_str_radix(&l[..6], 16) {
                if a < 0x10_0000 {
                    set_bit(&mut bm, a);
                }
            }
        }
    }
    let _ = LABELS.set(labels);
    bm
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 7 {
        eprintln!("usage: fuzz_couv <roms> <eeprom> <scenario> <frames> <targets> <coverage.bin>");
        std::process::exit(2);
    }
    let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(std::env::var("DDSOM_OC").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0));
    ddsom_game::install_reference(&mut m);
    m.executor = Some(run);
    let atlas = std::env::var("FUZZ_ATLAS").unwrap_or_else(|_| "docs/atlas.md".into());
    let zs = zones(&atlas);
    let zone_of = |x: u32| zs.iter().find(|(d, f, _)| (*d..*f).contains(&x)).map(|z| z.2.as_str()).unwrap_or("?");
    INSTRUCTIONS.set(std::env::var("FUZZ_LISTING").ok().map(|f| instructions(&f)).unwrap_or_default()).unwrap();
    let handlers = handlers(&mut m.bus.clone());
    let mut targets = HashMap::new();
    let specs: Vec<&str> = a[5].split(',').collect();
    let native_tests = ddsom_game::tools::hud::TEST_NATIVES.iter().chain(ddsom_game::tools::credits::TEST_NATIVES).chain(ddsom_game::tools::credits_task::TEST_NATIVES);
    for n in ddsom_game::all_natives().chain(native_tests) {
        let wanted = |s: &str| {
            // `nom:` is the former spelling, still accepted by the existing scripts
            if let Some(p) = s.strip_prefix("name:").or_else(|| s.strip_prefix("nom:")) {
                n.name.starts_with(p)
            } else if let Some(h) = s.strip_prefix("0x") {
                u32::from_str_radix(h, 16).is_ok_and(|x| x == n.entry)
            } else {
                s == "*" || s == "all" || zone_of(n.entry) == s
            }
        };
        let excluded = specs.iter().filter_map(|s| s.strip_prefix('-')).any(wanted);
        if !excluded && specs.iter().filter(|s| !s.starts_with('-')).any(|s| wanted(s)) && !ddsom_game::tools::kernel::NATIVES.iter().any(|k| k.entry == n.entry) {
            targets.insert(n.entry, Target { n, bases: Vec::new(), corpus: Vec::new(), summary: Summary::default(), points: 0, zone: zone_of(n.entry).to_string(), handler: handlers.get(&n.entry).copied(), substitutions: 0, unfocused: 0 });
        }
    }
    eprintln!("{} targeted natives", targets.len());
    let union = std::env::var("FUZZ_UNION").ok().map(|f| std::fs::read(f).expect("union")).unwrap_or_else(|| vec![0; 0x10000]);
    let front = boundary(&mut m.bus.clone(), &union);
    UNION.set(union.clone()).unwrap();
    let seed = env("FUZZ_GRAINE", 1);
    STATE.with(|x| {
        *x.borrow_mut() = Some(State {
            targets,
            union,
            seen: vec![0; 0x10000],
            identical_seen: vec![0; 0x10000],
            boundary: front,
            domains: HashMap::new(),
            substitutes: std::env::var_os("FUZZ_SUBSTITUTS").is_some(),
            focus: std::env::var("FUZZ_FOCUS")
                .map(|v| zs.iter().filter(|z| v.split(',').any(|f| f == z.2)).map(|z| (z.0, z.1)).collect())
                .unwrap_or_default(),
            rng: Rng(0x9e37_79b9_7f4a_7c15 ^ seed.wrapping_mul(0x1000_0000_01b3)),
            period: env("FUZZ_PERIODE", 16),
            points_max: env("FUZZ_POINTS", 400),
            tests: env("FUZZ_ESSAIS", 24) as u32,
            max_step: env("FUZZ_PAS", 300_000) as u32,
            examples: env("FUZZ_EXEMPLES", 3) as usize,
            trace: std::env::var_os("FUZZ_TRACE").is_some(),
        })
    });
    // trial panics are counted, not displayed (FUZZ_PANIQUES=1: displayed)
    if std::env::var_os("FUZZ_PANIQUES").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).expect("scenario");
    for f in 0..a[4].parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let e = STATE.with(|x| x.borrow_mut().take().unwrap());
    std::fs::write(&a[6], &e.seen).expect("output");
    std::fs::write(format!("{}.identiques", a[6]), &e.identical_seen).expect("output");
    let mut total = 0;
    let mut sort: Vec<&Target> = e.targets.values().filter(|c| c.summary.tests > 0 || c.summary.gaps + c.summary.panics > 0).collect();
    sort.sort_by_key(|c| c.n.entry);
    for c in sort {
        let b = &c.summary;
        println!(
            "{:06x} {:32} {:>6} calls {:>6} trials ({:>5} new) {:>5} gaps ({:>5} new, {:>5} plausible) {:>4} panics {:>4} abandoned {:>4} outside code {:>5} fresh{}",
            c.n.entry, c.n.name, b.calls, b.tests, b.new_count, b.gaps, b.new_gaps, b.plausible_gaps, b.panics, b.abandons, b.outside_code, b.new_total, if c.substitutions > 0 && c.substitutions < u64::MAX { format!(" ({} substitutions)", c.substitutions) } else { String::new() }
        );
        total += b.gaps + b.panics;
        for (s, k) in &b.signatures {
            println!("    [{k}] {s}");
        }
        for x in &b.examples {
            println!("    {x}");
        }
    }
    let trials: u64 = e.targets.values().map(|c| c.summary.tests).sum();
    if trials == 0 {
        println!("FUZZ FAILED: no trial compared with the original");
        std::process::exit(1);
    }
    println!("{}", if total == 0 { "FUZZ OK" } else { "FUZZ MISMATCHES" });
}
