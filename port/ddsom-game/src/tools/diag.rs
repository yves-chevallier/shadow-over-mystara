//! Tools: the natives of `crate::diag` for the original code that calls them (arguments and results in the registers
//! of the original).

use m68k::Cpu;

use crate::diag::*;
use crate::regs;

/// Probe of a call by address (`crate::call`): with the label `call_<address in hex>` (or `call`) in `DDSOM_POISON`, the
/// registers and flags that the called routine changed (compared to `before`) are inverted on return, at site
/// `site`. If validation stays VALID, nothing that follows rereads what the routine leaves: the call can become that
/// of a core without registers.
pub(crate) fn probe_call(c: &mut Cpu, address: u32, before: &([u32; 8], [u32; 8], u16), site: &std::panic::Location) {
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

/// Registers that the adapter `label` writes like the original and which we want to prove dead
/// (`crate::regs` mask: `regs::a(1) | regs::d(6) | regs::CCR`...). With `DDSOM_POISON`, they are inverted.
#[track_caller]
pub(crate) fn dead_regs(c: &mut Cpu, label: &str, mask: u32) {
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

/// Probes: the X flag, apart from the other flags (`regs::CCR`: N, Z, V and C), to know which one is reread.
const X: u32 = 1 << 17;
