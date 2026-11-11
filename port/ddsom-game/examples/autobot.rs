//! Bot recorder: plays the game in a closed loop (reads RAM: players, enemies, bosses, items)
//! and writes a pure input scenario (keys + RAM writes `poke`) that `validate` replays.
//!
//!   autobot <roms> <eeprom> <base.txt> <frames> <out.txt> [--players N] [--god HP]
//!           [--plan scene:duration,scene:duration,...] [--weaken N] [--seed S]
//!
//! * `base.txt`: fixed inputs (coins, Start, character choice), produced by
//!   `tools/make_bot.py --base-only`.
//! * The bot walks to the nearest enemy (ENEMY FFA99E, BOSS FFBA1E), lines up in depth, hits,
//!   jumps, casts a spell / uses an item now and then; with no enemy, it picks up items (ITEM) and boxes
//!   (BOX), then walks right while tapping buttons (menus, shops, route choices).
//! * `--god HP`: resets the HP (player `+0x62`) to HP when they fall below half.
//! * `--weaken N`: N game frames after entering a scene, boss HP is set to 1
//!   (the bot finishes them with one hit: boss death sequences, natural scene end).
//! * `--plan`: scene itinerary. The first scene is written to FF8034 during character
//!   selection; then, after `duration` game frames in the scene (or 400 frames after a
//!   natural progression), the bot jumps to the next one (FF8034 <- scene, phase FF802C <- 8: the game's scene entry).
//!   `scene:duration:f` instead forces the scene exit (phase FF802C <- 0x10, like `tools/gen_scene_end.py`);
//!   `scene:duration:t` forces it at the fade state of sequence 18, which plays the scene's transition script (like
//!   the `trans_*` scenarios); the bot then waits for the next scene chosen by the game (route votes), plus 400 frames.
//! * `--spawn family:type:variant:frame`: at the given frame, creates by hand an `enemy` or `boss` object of the given
//!   type and variant, 0x60 pixels right of player 1 (like `creatures spawn`), for types the itineraries never
//!   show; the RAM writes are part of the produced scenario.
//! * `--door frame:x:y:key[:duration]`: at the given frame, every player is put on the ground at (x, y) and holds
//!   the key (bit of `+0x6c`: 0 right, 1 left, 2 down, 3 up) for `duration` frames (60 by default), then the bot resumes:
//!   to go through passage doors (SET 27 and 29) and zone exits (SET 7), see `tools/gen_long.py`.
//! * `--wander`: in shops (scenes 0x45 to 0x52) and during a route vote (EFFCT 23), each player holds a random
//!   direction for 20 to 90 frames and sometimes taps a button (shop cursors: articles, gold gifts, exit; vote
//!   cursors: varied choices, ties); at each vote, one player in four abstains (the countdown runs out).
//!   See `tools/gen_long.py` (`long_votes_*`, `long_boutiques_*`).
//! * `--frail N`: every N frames, living players 3 and 4 (state < 4) are set to 1 HP: they fall at the next hit
//!   (continues during boss fights, F3 interface: frame cleared while a boss is present).
//! * `--patience N`: an item or box chased for more than N frames is ignored for 2,000 frames
//!   (otherwise the bot stays stuck in front of an unreachable item and the scene never advances).
//! * `--cast N`, `--charges V`: frequent spells and items (see `Magic`).
//! * `--offset N`: the game starts N frames later (attract first): the plan's scene is written at
//!   frame 2400 + N and the bot only plays after 2600 + N (base produced with `bot_lib.build(offset=N)`).
//!
//! It runs like `validate` (68000 overclocked x16, machine B): the scenario is therefore reproduced exactly.
use std::path::Path;

const OVERCLOCK: f64 = 16.0;
const P0: u32 = 0xff85de;

struct Bot {
    m: cps2::Machine,
    f: usize,
    held: std::collections::HashMap<String, bool>,
    out: Vec<(usize, String)>,
    /// `--patience N`: item chased by each player and for how many frames; items ignored until a given frame
    patience: usize,
    chase: [(u32, usize); 4],
    ignore: std::collections::HashMap<u32, usize>,
    /// `--spells a,b,..` `--spell-period N`: identifiers put in turn in the first equipment row, cast in front of an enemy
    spells: Vec<u32>,
    spell_period: usize,
    spell_next: usize,
    /// frame of the last identifier put, per player (one per period, when the player is idle)
    spell_at: [usize; 4],
    rod_at: [usize; 4],
    /// `--rods N`: every N frames, the magic-user changes rod (`WEAPON_B` 0x24..0x27, charge full or empty)
    rods: usize,
    /// `--door`: (frame, x, y, key, duration)
    doors: Vec<(usize, u32, u32, u32, usize)>,
    /// `--wander`: (start and end of the gesture, keys, abstention in the current vote) per player
    wander: bool,
    wander_keys: [(usize, usize, [bool; 8], bool); 4],
    voting: bool,
    frail: usize,
}

impl Bot {
    fn rd8(&self, a: u32) -> u32 {
        let o = (a - 0xff0000) as usize;
        let w = self.m.bus.ram[o / 2] as u32;
        if o.is_multiple_of(2) { w >> 8 } else { w & 0xff }
    }
    fn rd16(&self, a: u32) -> u32 {
        self.rd8(a) << 8 | self.rd8(a + 1)
    }
    /// Input held or released from the current frame on (effective for frame `f`).
    fn set(&mut self, name: String, down: bool) {
        let cur = self.held.get(&name).copied().unwrap_or(false);
        if cur != down {
            ddsom_game::core::scenario::set_input(&mut self.m, &name, down);
            self.out.push((self.f - 1, format!("{name} {}", down as u8)));
            self.held.insert(name, down);
        }
    }
    fn poke(&mut self, addr: u32, val: u32, size: u8) {
        ddsom_game::core::scenario::poke(&mut self.m, addr, val, size);
        self.out.push((self.f - 1, format!("poke{} {addr:x} {val}", 8 * size as u32)));
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let base_path = &a[3];
    let frames: usize = a[4].parse().unwrap();
    let out_path = &a[5];
    let (mut players, mut god, mut plan, mut seed, mut weaken) = (1usize, 0u32, Vec::<(u32, usize, u8)>::new(), 1u64, 0usize);
    let mut offset = 0usize;
    let mut magic = Magic::default();
    let mut patience = 0usize;
    let mut spawn: Option<(bool, u32, u32, usize)> = None;
    let (mut spells, mut spell_period, mut rods) = (Vec::<u32>::new(), 0usize, 0usize);
    let mut doors = Vec::<(usize, u32, u32, u32, usize)>::new();
    let mut wander = false;
    let mut frail = 0usize;
    let mut i = 6;
    while i < a.len() {
        match a[i].as_str() {
            "--players" => players = a[i + 1].parse().unwrap(),
            "--god" => god = u32::from_str_radix(a[i + 1].trim_start_matches("0x"), 16).unwrap(),
            "--weaken" => weaken = a[i + 1].parse().unwrap(),
            "--seed" => seed = a[i + 1].parse().unwrap(),
            "--offset" => offset = a[i + 1].parse().unwrap(),
            "--patience" => patience = a[i + 1].parse().unwrap(),
            "--cast" => magic.period = a[i + 1].parse().unwrap(),
            "--charges" => magic.charges = a[i + 1].parse().unwrap(),
            "--spells" => {
                spells = a[i + 1].split(',').map(|t| u32::from_str_radix(t.trim_start_matches("0x"), 16).unwrap()).collect()
            }
            "--spell-period" => spell_period = a[i + 1].parse().unwrap(),
            "--door" => {
                let t: Vec<usize> = a[i + 1].split(':').map(|v| usize::from_str_radix(v.trim_start_matches("0x"), if v.starts_with("0x") { 16 } else { 10 }).unwrap()).collect();
                doors.push((t[0], t[1] as u32, t[2] as u32, t[3] as u32, t.get(4).copied().unwrap_or(60)));
            }
            "--rods" => rods = a[i + 1].parse().unwrap(),
            "--frail" => frail = a[i + 1].parse().unwrap(),
            "--wander" => {
                wander = true;
                i -= 1;
            }
            "--spawn" => {
                let t: Vec<&str> = a[i + 1].split(':').collect();
                spawn = Some((t[0] == "boss", t[1].parse().unwrap(), t[2].parse().unwrap(), t[3].parse().unwrap()));
            }
            "--plan" => {
                plan = a[i + 1]
                    .split(',')
                    .map(|t| {
                        let v: Vec<&str> = t.split(':').collect();
                        (v[0].parse().unwrap(), v[1].parse().unwrap(), v.get(2).map_or(0, |m| m.as_bytes()[0]))
                    })
                    .collect()
            }
            _ => panic!("unknown option {}", a[i]),
        }
        i += 2;
    }
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(OVERCLOCK);
    ddsom_game::install_reference(&mut m);
    m.idle = Some(ddsom_game::idle);
    let base = ddsom_game::core::scenario::Scenario::load(Path::new(base_path)).unwrap();
    let mut b = Bot { m, f: 0, held: Default::default(), out: Vec::new(), patience, chase: [(0, 0); 4], ignore: Default::default(), spells, spell_period, spell_next: 0, spell_at: [usize::MAX; 4], rod_at: [usize::MAX; 4], rods, doors, wander, wander_keys: [(0, 0, [false; 8], false); 4], voting: false, frail };
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut rnd = move |n: u32| {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 33) % n as u64) as u32
    };
    let (mut k, mut since, mut trans): (usize, usize, usize) = (0, 0, 0);
    let mut forced = false;
    let mut last_scene = 0;
    let mut visited = std::collections::BTreeSet::new();
    for f in 0..frames {
        b.f = f;
        base.apply(f, &mut b.m);
        let (scene, mut phase) = (b.rd8(0xff8034), b.rd8(0xff802c));
        if scene != last_scene {
            eprintln!("frame {f}: scene {scene:#x}, phase {phase:#x}");
            last_scene = scene;
            since = 0;
            visited.insert(scene);
        }
        if f == 2400 + offset && !plan.is_empty() {
            b.poke(0xff8034, plan[0].0, 1);
        }
        if let Some((boss, kind, variant, at)) = spawn {
            if f == at {
                let (pool, n) = if boss { (0xffba1eu32, 3) } else { (0xffa99e, 22) };
                if let Some(slot) = (0..n).map(|i| pool + 0xc0 * i).find(|&o| b.rd8(o) == 0) {
                    let (px, py) = (b.rd16(P0 + 8), b.rd16(P0 + 0x10));
                    b.poke(slot + 0x28, if boss { 4 } else { 2 }, 1);
                    b.poke(slot + 2, kind, 1);
                    b.poke(slot + 3, variant, 1);
                    b.poke(slot + 8, (px + 0x60) & 0xffff, 2);
                    b.poke(slot + 0x10, py, 2);
                    if !boss {
                        let free = b.rd16(0xff8000 + 0x7e62);
                        b.poke(0xff8000 + 0x7e62, free.wrapping_sub(1) & 0xffff, 2);
                    }
                    b.poke(slot, 1, 1);
                }
            }
        }
        let active = b.rd8(0xff8030) != 0;
        let select_end = phase == 0x04 && b.rd8(0xff802d) > 0x0c;
        if f > 2600 + offset && select_end {
            // end of the selection (countdown): no key
            for pl in 0..players {
                apply_keys(&mut b, pl, [false; 8]);
            }
        } else if f > 2600 + offset && (!active || phase == 0x04) && phase != 0x08 {
            // out of the game (ranking name, screens) or character selection (after a continue / a
            // new game): Button 1 taps and cursor moves
            for pl in 0..players {
                let g = f + 17 * pl;
                let keys = [
                    g % 150 >= 20 && g % 150 < 24 && (g / 150 + pl) % 2 == 0,
                    g % 150 >= 20 && g % 150 < 24 && (g / 150 + pl) % 2 == 1,
                    g % 230 >= 30 && g % 230 < 34,
                    g % 230 >= 60 && g % 230 < 64,
                    g % 50 < 4,
                    false, false, false,
                ];
                apply_keys(&mut b, pl, keys);
            }
        } else if f > 2600 + offset && active && b.rd8(0xff8031) != 0 {
            // --- scene itinerary; `since` = frames spent in the current scene, transitions excluded (phases 8 and 0x10)
            if phase != 0x08 && phase != 0x10 {
                since += 1;
            }
            if weaken != 0 && since >= weaken && since % 30 == 0 {
                // weakens the bosses: the next hit finishes them
                for i in 0..3 {
                    let o = 0xffba1e + 0xc0 * i;
                    if b.rd8(o) != 0 && b.rd16(o + 0x62) > 1 {
                        b.poke(o + 0x62, 1, 2);
                    }
                }
            }
            // transition sequences (phases 8 and 0x10) that last too long: go to the next scene
            // (a route vote in progress, with --wander, does not count: it lasts up to 20 s after its dialogues)
            trans = if (phase == 0x08 || phase == 0x10) && !(b.wander && vote_open(&b)) { trans + 1 } else { 0 };
            // (forced exit: dialogues and vote may last much longer)
            let trans_max = if forced { 6000 } else { 1500 };
            if !plan.is_empty() && (trans > trans_max || (phase != 0x08 && phase != 0x10)) {
                // a living boss extends the stay (up to 5 times the planned duration)
                let boss = (0..3).any(|i| b.rd8(0xffba1e + 0xc0 * i) != 0);
                let limit = if boss { 5 * plan[k].1 } else { plan[k].1 };
                let timeout = trans > trans_max || if scene == plan[k].0 && !forced { since >= limit } else { since >= 400 };
                if plan[k].2 != 0 && !forced && scene == plan[k].0 && since >= limit {
                    // forced scene exit (gen_scene_end.py): the scene's exit sequence, or its transition script
                    forced = true;
                    eprintln!("frame {f}: forced exit of scene {}", plan[k].0);
                    b.poke(0xff802c, 0x10, 1);
                    b.poke(0xff802d, 0, 1);
                    b.poke(0xff802e, 0, 2);
                    if plan[k].2 == b't' {
                        b.poke(0xff2a36, 0x0224_1400, 4);
                        for r in 0..4 {
                            b.poke(0xff2ae0 + 4 * r, 0, 4);
                        }
                    }
                    phase = 0x10;
                } else if timeout && k + 1 < plan.len() {
                    k += 1;
                    since = 0;
                    trans = 0;
                    forced = false;
                    eprintln!("frame {f}: jump to scene {}", plan[k].0);
                    b.poke(0xff8034, plan[k].0, 1);
                    b.poke(0xff802c, 0x08, 1);
                    b.poke(0xff802d, 0, 1);
                    b.poke(0xff802e, 0, 1);
                    phase = 0x08;
                }
            }
            for pl in 0..players {
                if phase == 0x08 {
                    // scene entry: no key (an input during this phase used to make the two machines
                    // of validate diverge, see docs/couverture.md)
                    apply_keys(&mut b, pl, [false; 8]);
                } else if let Some(&(at, x, y, key, dur)) = b.doors.iter().find(|d| f >= d.0 && f < d.0 + d.4) {
                    // door: living players are put in front of it and hold the key
                    let base = P0 + 0x200 * pl as u32;
                    if f == at && b.rd8(base) != 0 {
                        b.poke(base + 8, x, 2);
                        b.poke(base + 0x10, y, 2);
                        b.poke(base + 0xc, 0, 2);
                    }
                    let _ = dur;
                    let mut keys = [false; 8];
                    keys[[0, 1, 3, 2][key as usize]] = f > at + 2;
                    apply_keys(&mut b, pl, keys);
                } else if b.wander && (matches!(scene, 0x45..=0x52) || vote_open(&b)) {
                    wander_step(&mut b, pl, &mut rnd);
                } else {
                    policy(&mut b, pl, &mut rnd, god, magic);
                }
            }
        }
        b.m.run_frame();
    }
    // scenario = base + recording (stable sort by frame)
    let mut lines: Vec<(usize, String)> = std::fs::read_to_string(base_path)
        .unwrap()
        .lines()
        .filter_map(|l| Some((l.split(' ').next()?.parse().ok()?, l.to_string())))
        .collect();
    lines.extend(b.out.iter().map(|(f, s)| (*f, format!("{f} {s}"))));
    lines.sort_by_key(|l| l.0);
    let mut text: String = lines.iter().map(|l| l.1.clone() + "\n").collect();
    text += &format!("{} quit\n", frames + 1);
    std::fs::write(out_path, text).unwrap();
    eprintln!("visited scenes: {:x?}", visited);
}

/// Spells and items: `--cast N` selects the next row (Button 4) then uses it (Button 3) every N frames
/// in front of an enemy; `--charges V` resets to V the charges (reserve `+0x150`) of the occupied equipment rows.
#[derive(Clone, Copy, Default)]
struct Magic {
    period: usize,
    charges: u32,
}

fn policy(b: &mut Bot, pl: usize, rnd: &mut impl FnMut(u32) -> u32, god: u32, magic: Magic) {
    let base = P0 + 0x200 * pl as u32;
    if b.rd8(base) == 0 {
        apply_keys(b, pl, [false; 8]);
        return;
    }
    let f = b.f + 7 * pl;
    if magic.charges != 0 && f.is_multiple_of(50) {
        // three 16-byte rows at +0x120: identifier (0x56: empty), byte +8 = reserve index
        for line in 0..3 {
            let row = base + 0x120 + 16 * line;
            let slot = base + 0x150 + b.rd8(row + 8);
            if b.rd8(row) != 0x56 && b.rd8(row + 8) < 9 && b.rd8(slot) < magic.charges {
                b.poke(slot, magic.charges, 1);
            }
        }
    }
    let quiet = b.rd16(base + 4) == 0x200 && !matches!(b.rd8(base + 6), 0xc | 0xe | 0x14 | 0x22) && b.rd8(base + 0x15c) == 0;
    if b.spell_period != 0 && !b.spells.is_empty() && f % b.spell_period < 100 && quiet
        && b.spell_at[pl] / b.spell_period != f / b.spell_period
    {
        b.spell_at[pl] = f;
        // puts the next identifier at the head of the first row, charged, and selects it (`0x15d` = 0)
        let id = b.spells[b.spell_next % b.spells.len()];
        b.spell_next += 1;
        b.poke(base + 0x120, id, 1);
        b.poke(base + 0x128, 0, 1);
        b.poke(base + 0x150, 5, 1);
        b.poke(base + 0x15d, 0, 1);
    }
    if b.rods != 0 && f % b.rods < 100 && b.rd8(base + 3) >> 1 == 4 && quiet && b.rod_at[pl] / b.rods != f / b.rods {
        // a rod (0x24 to 0x27), in turn full (1), with six charges (the last one warns the interface) and empty
        b.rod_at[pl] = f;
        let n = (f / b.rods) as u32;
        b.poke(base + 0x10e, 0x24 + n % 4, 1);
        for flag in 0x18b..0x192 {
            b.poke(base + flag, [1, 6, 0][(n as usize / 4) % 3], 1);
        }
    }
    if b.frail != 0 && pl >= 2 && b.f.is_multiple_of(b.frail) && b.rd8(base + 4) < 4 && b.rd16(base + 0x62) > 1 {
        b.poke(base + 0x62, 1, 2);
    }
    let hp = b.rd16(base + 0x62);
    if god != 0 && hp < god - 8 && hp > 0 && b.rd8(base + 4) < 4 {
        b.poke(base + 0x62, god, 2);
    }
    let (px, py) = (b.rd16(base + 8) as i32, b.rd16(base + 0x10) as i32);
    // target: living enemy / boss, otherwise items and boxes
    let mut best: Option<(i32, i32, i32, bool, u32)> = None; // (distance, x, y, hostile, object)
    let now = b.f;
    let scan = |b: &Bot, start: u32, n: u32, size: u32, hostile: bool, best: &mut Option<(i32, i32, i32, bool, u32)>| {
        for i in 0..n {
            let o = start + i * size;
            if b.rd8(o) == 0 || (hostile && b.rd16(o + 0x62) == 0) || b.ignore.get(&o).is_some_and(|&u| u > now) {
                continue;
            }
            let (x, y) = (b.rd16(o + 8) as i32, b.rd16(o + 0x10) as i32);
            let d = (x - px).abs() + 2 * (y - py).abs();
            if best.is_none_or(|t| (hostile, -d) > (t.3, -t.0)) {
                *best = Some((d, x, y, hostile, o));
            }
        }
    };
    scan(b, 0xffba1e, 3, 0xc0, true, &mut best);
    scan(b, 0xffa99e, 22, 0xc0, true, &mut best);
    if best.is_none() {
        scan(b, 0xffe2de, 32, 0x60, false, &mut best);
        scan(b, 0xffeede, 21, 0x80, false, &mut best);
    }
    // item or box chased too long (out of reach: behind a wall, scenery): ignored for 2,000 frames
    if b.patience != 0 {
        let o = best.filter(|t| !t.3).map_or(0, |t| t.4);
        if b.chase[pl].0 != o {
            b.chase[pl] = (o, 0);
        }
        b.chase[pl].1 += 1;
        if o != 0 && b.chase[pl].1 > b.patience {
            b.ignore.insert(o, now + 2000);
            b.chase[pl] = (0, 0);
            best = None;
        }
    }
    if pl == 0 && b.f.is_multiple_of(200) && std::env::var("BOTDEBUG").is_ok() {
        eprintln!("f {} pos {px},{py} hp {hp} target {best:?}", b.f);
    }
    let (mut right, mut left, mut up, mut down, mut b1, mut b2, mut b3, mut b4) = (false, false, false, false, false, false, false, false);
    match best {
        Some((_, tx, ty, hostile, _)) => {
            let (dx, dy) = (tx - px, ty - py);
            let range = if hostile { 40 } else { 6 };
            if dx > range { right = true } else if dx < -range { left = true }
            if dy.abs() > 6 { if dy > 0 { up = true } else { down = true } }
            if hostile && dx.abs() <= range + 16 && dy.abs() <= 12 {
                b1 = (f / 4).is_multiple_of(2);
                // turns towards the target one frame in 16
                if f % 16 < 2 { if dx > 0 { right = true } else { left = true } }
                if f % 500 == 120 { b2 = true } // jump
                if f % 331 < 4 { b3 = true } // spell / item
                if magic.period != 0 && f % magic.period < 4 { b4 = true }
                if magic.period != 0 && f % magic.period >= 12 && f % magic.period < 16 { b3 = true }
            }
        }
        None => {
            right = true;
            if f % 90 < 4 { b1 = true }
            if f % 150 < 3 { if (f / 150).is_multiple_of(2) { up = true } else { down = true } }
            if f % 700 < 4 { b2 = true }
        }
    }
    if b.spell_period != 0 && matches!(f.wrapping_sub(b.spell_at[pl]), 2..=5 | 42..=45 | 82..=85) { b4 = true } // special attack of --spells
    // items: select (Button 4) then use (Button 3)
    if f % 1100 < 4 { b4 = true }
    if f % 1100 >= 20 && f % 1100 < 24 { b3 = true }
    // variety: a few random actions
    if rnd(400) == 0 { b2 = true }
    apply_keys(b, pl, [right, left, up, down, b1, b2, b3, b4]);
}

/// Is a route vote open? (a living EFFCT of type 23; FFD49E: 38 slots of 0x60 bytes)
fn vote_open(b: &Bot) -> bool {
    (0..38).any(|i| {
        let o = 0xffd49e + 0x60 * i;
        b.rd8(o) != 0 && b.rd8(o + 2) == 23
    })
}

/// `--wander`: random gestures (direction held until the end of the gesture, button tapped during its first four frames).
fn wander_step(b: &mut Bot, pl: usize, rnd: &mut impl FnMut(u32) -> u32) {
    let vote = vote_open(b);
    if vote && !b.voting {
        // new vote: abstentions drawn for every player
        for k in 0..4 {
            b.wander_keys[k].3 = rnd(4) == 0;
        }
    }
    if pl == 0 {
        b.voting = vote;
    }
    let (_, until, _, abstain) = b.wander_keys[pl];
    if b.f >= until {
        let mut keys = [false; 8];
        match rnd(9) {
            0 => keys[0] = true,
            1 => keys[1] = true,
            2 => keys[2] = true,
            3 => keys[3] = true,
            4 => (keys[0], keys[2]) = (true, true),
            5 => (keys[1], keys[3]) = (true, true),
            _ => {}
        }
        if !(vote && abstain) {
            match rnd(16) {
                0..=5 => keys[4] = true,
                6 | 7 => keys[5] = true,
                8 | 9 => keys[6] = true,
                10 => keys[7] = true,
                _ => {}
            }
        }
        b.wander_keys[pl] = (b.f, b.f + 20 + rnd(70) as usize, keys, abstain);
    }
    let (start, _, mut keys, _) = b.wander_keys[pl];
    if b.f >= start + 4 {
        keys[4..].fill(false);
    }
    apply_keys(b, pl, keys);
}

fn apply_keys(b: &mut Bot, pl: usize, keys: [bool; 8]) {
    let names = ["Right", "Left", "Up", "Down", "Button 1", "Button 2", "Button 3", "Button 4"];
    for (n, v) in names.iter().zip(keys) {
        b.set(format!("P{} {n}", pl + 1), v);
    }
}
