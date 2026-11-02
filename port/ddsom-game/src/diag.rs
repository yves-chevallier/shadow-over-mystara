//! Refactoring checks (`docs/guide-refactorisation.md`, « Registres morts »).
//!
//! A routine converted to an ordinary Rust function stops writing the registers that the original left (A1 of an
//! allocation, D0/D1/A4 of a release, flags...). Before dropping them, one must know that nobody
//! rereads them: the Rust caller, but also what follows (next handler of the dispatch loop, task that inherits the
//! registers, code that tests a flag). The `_68k` adapter that still writes them calls
//! [`dead_regs`] right after: with `DDSOM_POISON=label,...`, these registers are then **inverted** (value
//! complemented, flags inverted), which guarantees a wrong value. If validation stays VALID, no reader
//! exists in the scenarios: the adapter can be replaced by the pure function. Otherwise the difference points to the
//! reader (frame, address); the write is kept at that exact place, commented out.
//!
//! Without the variable, `dead_regs` costs only one atomic read.

use std::sync::OnceLock;

use m68k::Cpu;

use crate::regs;

/// Poisoned labels (`DDSOM_POISON`, comma-separated; `all` takes them all).
fn labels() -> &'static [String] {
    static E: OnceLock<Vec<String>> = OnceLock::new();
    E.get_or_init(|| {
        std::env::var("DDSOM_POISON").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default()
    })
}

/// The label is poisoned.
pub fn poison_active(label: &str) -> bool {
    let e = labels();
    !e.is_empty() && e.iter().any(|x| x == label || x == "all")
}

/// Registers targeted at site `site` (`file:line:column`, or step name) by one of the labels `names`, or
/// `None` if no entry targets the site. An entry `label@pattern` only applies to the sites that contain the
/// pattern (`motion@families/sobj`); `label.d4` (`.a1`, `.ccr`) only targets one register; an entry preceded by
/// `!` excludes these sites or this register (`motion,!motion@sobj.rs:120`, `exit,!exit.d4@ba310`), to isolate by
/// bisection the reader of a register. An entry `label@pattern#a1+d0` targets this site except A1 and D0 (reader
/// already known and written in the clear, to look for the others); preceded by `=`, it spares these registers without
/// targeting the site.
fn selection(names: &[&str], site: &str) -> Option<u32> {
    const ALL: u32 = 0x7fff | regs::CCR | X;
    let (mut vises, mut spared, mut active) = (0, 0, false);
    for x in labels() {
        let (excluded, x) = match x.strip_prefix('!') {
            Some(x) => (true, x),
            None => (false, x.as_str()),
        };
        let (guard_only, x) = match x.strip_prefix('=') {
            Some(x) => (true, x),
            None => (false, x),
        };
        let (x, guard) = match x.split_once('#') {
            Some((x, g)) => (x, mask_of(g)),
            None => (x, 0),
        };
        let (name, pattern) = match x.split_once('@') {
            Some((n, m)) => (n, Some(m)),
            None => (x, None),
        };
        let (name, reg_names) = match name.split_once('.') {
            Some((n, r)) => (n, mask_of(r)),
            None => (name, ALL),
        };
        if !(names.contains(&name) || name == "all") || !pattern.map_or(true, |m| site.contains(m)) {
            continue;
        }
        if excluded {
            if reg_names == ALL {
                return None;
            }
            spared |= reg_names;
        } else {
            if !guard_only {
                active = true;
                vises |= reg_names;
            }
            spared |= guard;
        }
    }
    active.then_some(vises & !spared)
}

/// [`selection`] for a single label.
fn guards(label: &str, site: &str) -> Option<u32> {
    selection(&[label], site)
}

/// `crate::regs` mask of a `d0+a1+ccr` list.
fn mask_of(list: &str) -> u32 {
    list
        .split('+')
        .map(|r| match (r.get(..1), r.get(1..).and_then(|n| n.parse::<u32>().ok())) {
            (Some("d"), Some(n)) if n < 8 => regs::d(n),
            (Some("a"), Some(n)) if n < 8 => regs::a(n),
            _ if r == "ccr" => regs::CCR | X,
            _ if r == "nzvc" => regs::CCR,
            _ if r == "x" => X,
            _ => panic!("DDSOM_POISON: register {r:?}"),
        })
        .fold(0, |m, r| m | r)
}

/// Probe of a call by address (`crate::call`): with the label `call_<address in hex>` (or `call`) in `DDSOM_POISON`, the
/// registers and flags that the called routine changed (compared to `before`) are inverted on return, at site
/// `site`. If validation stays VALID, nothing that follows rereads what the routine leaves: the call can become that
/// of a core without registers.
pub fn probe_call(c: &mut Cpu, address: u32, before: &([u32; 8], [u32; 8], u16), site: &std::panic::Location) {
    if labels().is_empty() {
        return;
    }
    let label = format!("call_{address:x}");
    let site = site.to_string();
    // `call` alone targets all calls by address (`call@families/`: those of the families)
    let Some(chosen) = selection(&[&label, "call"], &site) else {
        return;
    };
    invert_changes(c, before, !chosen, &label, &site);
}

/// Probe of the exit of an object handler (dispatch loop, `dispatch.rs`): with the label `exit` in
/// `DDSOM_POISON`, the registers and flags that the handler changed (except A0 and D7, which the loop rereads) are
/// inverted on its return. The site is `<looped> <handler>` in hexadecimal (`39f82 a17d0`): `exit@a17d0` targets
/// a handler, `exit@39e86` a class. If validation stays VALID, no following handler nor anything after it
/// rereads what the handler leaves: it can become a Rust function without output registers.
pub fn probe_exit(c: &mut Cpu, before: &([u32; 8], [u32; 8], u16), looped: u32, handler: u32) {
    if labels().is_empty() {
        return;
    }
    let site = format!("{looped:x} {handler:x}");
    let Some(chosen) = selection(&["exit"], &site) else {
        return;
    };
    invert_changes(c, before, !chosen | regs::a(0) | regs::d(7), "exit", &site);
}

/// Inverts the registers and flags changed since `before`, except those of `guard`.
fn invert_changes(c: &mut Cpu, before: &([u32; 8], [u32; 8], u16), guard: u32, label: &str, site: &str) {
    let mut mask = 0;
    for i in 0..8 {
        if c.d[i] != before.0[i] {
            mask |= regs::d(i as u32);
        }
        if i < 7 && c.a[i] != before.1[i] {
            mask |= regs::a(i as u32);
        }
    }
    if (c.sr ^ before.2) & 0x0f != 0 {
        mask |= regs::CCR;
    }
    if (c.sr ^ before.2) & 0x10 != 0 {
        mask |= X;
    }
    mask &= !guard;
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {label} {site} mask={mask:05x}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    invert(d, &mut a[..7], sr, mask);
}

/// Registers of a task that a step can inherit: D0-D7, A0-A4, A6 and the flags (A5 is reset by each step,
/// A7 is the stack).
pub const TASK_REGS: u32 = 0xff | 0x1f00 | 0x4000 | regs::CCR | X;

/// Probes: the X flag, apart from the other flags (`regs::CCR`: N, Z, V and C), to know which one is reread.
const X: u32 = 1 << 17;

/// Inverts N, Z, V, C (`regs::CCR`) and X (`X`) according to the mask.
fn invert_flags(sr: &mut u16, mask: u32) {
    if mask & regs::CCR != 0 {
        *sr ^= 0x0f;
    }
    if mask & X != 0 {
        *sr ^= 0x10;
    }
}

/// Inverts the registers of the mask (`crate::regs`).
fn invert(d: &mut [u32; 8], a: &mut [u32], sr: &mut u16, mask: u32) {
    for i in 0..8 {
        if mask & regs::d(i) != 0 {
            d[i as usize] = !d[i as usize];
        }
        if i < 7 && mask & regs::a(i) != 0 {
            a[i as usize] = !a[i as usize];
        }
    }
    invert_flags(sr, mask);
}

/// Probe of the return of a step (`tasks::execute`, step that returns `Flow::Done`): with the label `ret`
/// (`ret@<pattern>`, pattern searched in the step name), all the registers of the task (D0-D7, A0-A4, A6,
/// flags) are inverted before the rest of the current call. VALID: nothing that follows rereads what the step (or what
/// preceded the call) leaves in the registers.
pub fn probe_return(c: &mut Cpu, step: &dyn std::fmt::Debug) {
    if labels().is_empty() {
        return;
    }
    let site = format!("{step:?}");
    let Some(chosen) = guards("return", &site) else {
        return;
    };
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON return {site}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    invert(d, &mut a[..7], sr, TASK_REGS & chosen);
}

/// Registers that the adapter `label` writes like the original and which we want to prove dead
/// (`crate::regs` mask: `regs::a(1) | regs::d(6) | regs::CCR`...). With `DDSOM_POISON`, they are inverted.
#[track_caller]
pub fn dead_regs(c: &mut Cpu, label: &str, mask: u32) {
    if labels().is_empty() {
        return;
    }
    // the site is that of the adapter's caller (`#[track_caller]` on the adapters and the methods of
    // `objects::Current`)
    let site = std::panic::Location::caller().to_string();
    let Some(chosen) = guards(label, &site) else {
        return;
    };
    // `regs::CCR` of an adapter: all the flags, X included
    let mask = if mask & regs::CCR != 0 { mask | X } else { mask } & chosen;
    // DDSOM_POISON_TRACE=1: each poisoning, with its call site and A1 (diagnosing a difference)
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {label} {site} a1={:06x}", c.a[1]);
    }
    for i in 0..8 {
        if mask & regs::d(i) != 0 {
            c.d[i as usize] = !c.d[i as usize];
        }
        if i < 7 && mask & regs::a(i) != 0 {
            c.a[i as usize] = !c.a[i as usize];
        }
    }
    invert_flags(&mut c.sr, mask);
}

/// Probe of the registers that a task finds when the scheduler launches it (`kernel::launch`): with the label
/// `label` (`resume`: registers returned by the context of a task that yielded; `new`: registers inherited by
/// a new task) in `DDSOM_POISON`, the chosen registers (all, or `resume.d3`...), except those of `guard`, are
/// inverted. The site is the step launched (`Dispatch(3)`...). If validation stays VALID, the task does not reread
/// these registers: the context does not need to keep them.
pub fn probe_launch(c: &mut Cpu, label: &str, step: &dyn std::fmt::Debug, guard: u32) {
    if labels().is_empty() {
        return;
    }
    // without spaces or commas: the site stays a single word of a `DDSOM_POISON` list (`tools/bisect_poison.py`)
    let site: String = format!("{step:?}").chars().filter(|c| *c != ' ').map(|c| if c == ',' { ';' } else { c }).collect();
    let Some(chosen) = guards(label, &site) else {
        return;
    };
    let mask = TASK_REGS & chosen & !guard;
    if std::env::var_os("DDSOM_POISON_TRACE").is_some() {
        eprintln!("POISON {label} {site} mask={mask:05x}");
    }
    let (d, a, sr) = (&mut c.d, &mut c.a, &mut c.sr);
    invert(d, &mut a[..7], sr, mask);
}
