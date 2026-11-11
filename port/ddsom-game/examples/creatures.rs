//! Images of the game's creatures (enemies, bosses, characters), taken from the state of a game in progress.
//!
//!   creatures scan <roms> <eeprom> <scenario> <frames> <cache> [label]
//!   creatures spawn <roms> <eeprom> <cache> <scene> <enemy|boss> <type> <variant>
//!   creatures build <cache> <images_folder> <index.md>
//!
//! `scan` replays an input scenario on the machine (native code), and at each frame examines the objects
//! of the ENEMY and BOSS pools and player 1. For each object whose sprite frame (`+0x34`) changes, it
//! rebuilds the Object RAM entries of that object alone (same decoding as `sprites.rs`: pieces,
//! mirror, forced palette, attached objects), renders them with the machine's current palette on a
//! transparent background and stores the cropped sample in `<cache>/<family>_<type>_<variant>/`.
//! `build` picks the base pose of each type (first frame in active state, "walk" sub-state),
//! writes the PNGs (1:1 and x2), the summary sheet and the Markdown index.
//!
//! The images derive from the ROMs: they stay local (`assets/` folder, ignored by git).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ddsom_game::mem::{rd16, rd32, rd8};

const A5: u32 = 0xff8000;
const ENEMY: (u32, u32, u32) = (A5 + 0x299e, 0xc0, 22);
const BOSS: (u32, u32, u32) = (A5 + 0x3a1e, 0xc0, 3);
const PLAYER: u32 = A5 + 0x5de;
const STAGE: u32 = A5 + 0x34;

/// Sampling window (frames) and maximum number of samples of a key per window.
const WINDOW: usize = 3000;
const PER_WINDOW: u32 = 30;

const W: usize = 384;
const H: usize = 224;
/// Anchor point (object position) on the rendering screen.
const ANCHOR_X: u16 = 192;
const ANCHOR_H: u16 = 0x38;

// ---------------------------------------------------------------------------------------------
// Cropped RGBA image

#[derive(Clone)]
struct Sprite {
    w: usize,
    h: usize,
    /// Position of the top-left corner relative to the anchor point.
    ox: i32,
    oy: i32,
    rgba: Vec<u8>,
}

impl Sprite {
    fn write(&self, path: &Path, scale: usize) {
        let (w, h) = (self.w * scale, self.h * scale);
        let mut out = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let s = ((y / scale) * self.w + x / scale) * 4;
                out[(y * w + x) * 4..][..4].copy_from_slice(&self.rgba[s..s + 4]);
            }
        }
        save_png(path, w, h, &out);
    }

    fn opaque(&self) -> usize {
        self.rgba.as_chunks::<4>().0.iter().filter(|p| p[3] != 0).count()
    }

    /// Palette missing (not loaded yet): pure blue dominates the opaque pixels.
    fn garbage(&self) -> bool {
        let blue = self.rgba.as_chunks::<4>().0.iter().filter(|p| p[3] != 0 && p[0] < 40 && p[1] < 40 && p[2] > 200).count();
        blue * 10 > self.opaque() * 3
    }

    fn read(path: &Path, ox: i32, oy: i32) -> Sprite {
        let d = png::Decoder::new(fs::File::open(path).expect("sprite"));
        let mut r = d.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).unwrap();
        Sprite { w: info.width as usize, h: info.height as usize, ox, oy, rgba: buf }
    }
}

fn save_png(path: &Path, w: usize, h: usize, rgba: &[u8]) {
    let mut e = png::Encoder::new(fs::File::create(path).expect("png"), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(rgba).unwrap();
}

// ---------------------------------------------------------------------------------------------
// Decoding of the frames (like sprites.rs)

struct Frame {
    count: u16,
    second: u16,
    bank: u8,
    per_piece: bool,
    offsets: u32,
    pieces: u32,
}

fn frame_at(b: &mut ddsom_game::world::World, f: u32) -> Frame {
    Frame { count: rd16(b, f), second: rd16(b, f + 2), bank: rd8(b, f + 4), per_piece: rd8(b, f + 5) != 0, offsets: rd32(b, f + 6), pieces: f + 10 }
}

fn with_palette(attr: u16, pal: Option<u8>) -> u16 {
    match pal {
        Some(p) => (attr & 0xff00) | ((attr as u8 & 0xe0) | p) as u16,
        None => attr,
    }
}

/// Object RAM entries of a frame placed at (x, y); without mirror (ROM orientation).
fn emit(b: &mut ddsom_game::world::World, f: u32, x: u16, y: u16, palette: Option<u8>, out: &mut Vec<[u16; 4]>) {
    let fr = frame_at(b, f);
    let n = if fr.per_piece { fr.second } else { fr.count };
    let common = with_palette(fr.second, palette);
    let bits = ((fr.bank as u32) << 16 | 1).rotate_right(3) & !0xe000_0000 | 0x2000_0000;
    let (mut off, mut piece) = (fr.offsets, fr.pieces);
    for _ in 0..n {
        let (dx, dy) = (rd16(b, off), rd16(b, off + 2));
        off += 4;
        let code = rd16(b, piece);
        let attr = if fr.per_piece { with_palette(rd16(b, piece + 2), palette) } else { common };
        let pos = ((x.wrapping_add(dx).wrapping_add(0x40) as u32) << 16 | y.wrapping_add(dy) as u32) & 0x3ff_03ff | bits;
        out.push([(pos >> 16) as u16, pos as u16, code, attr]);
        piece += if fr.per_piece { 4 } else { 2 };
    }
}

fn plausible(b: &mut ddsom_game::world::World, f: u32) -> bool {
    (0x1000..0x400000).contains(&f) && f.is_multiple_of(2) && {
        let n = rd16(b, f);
        (1..=0x80).contains(&n) && rd32(b, f + 6) < 0x400000
    }
}

fn palette_of(b: &mut ddsom_game::world::World, o: u32) -> Option<u8> {
    (rd8(b, o + 0x2a) & 1 != 0).then(|| rd8(b, o + 0x2b))
}

/// Entries of object `o` and of its attached objects (behind, itself, in front).
fn object_entries(b: &mut ddsom_game::world::World, o: u32) -> Option<Vec<[u16; 4]>> {
    let f = rd32(b, o + 0x34);
    if !plausible(b, f) {
        return None;
    }
    let y0 = 0x100u16.wrapping_sub(ANCHOR_H);
    let (mut behind, mut front, mut own) = (vec![], vec![], vec![]);
    for field in [0x4au32, 0x48] {
        let w = rd16(b, o + field);
        if (w as i16) >= 0 {
            continue;
        }
        let a = w as i16 as i32 as u32 & 0xff_ffff;
        let cf = rd32(b, a + 0x34);
        if rd8(b, a) == 0 || !plausible(b, cf) {
            continue;
        }
        let cx = ANCHOR_X.wrapping_add(rd16(b, a + 8));
        let cy = (!ANCHOR_H.wrapping_add(rd16(b, a + 0xc))).wrapping_add(0x101);
        let pal = palette_of(b, a);
        let dest = if (rd16(b, a + 0x10) as i16) < 0 { &mut behind } else { &mut front };
        emit(b, cf, cx, cy, pal, dest);
    }
    let pal = palette_of(b, o);
    emit(b, f, ANCHOR_X, y0, pal, &mut own);
    behind.extend(own);
    behind.extend(front);
    Some(behind)
}

// ---------------------------------------------------------------------------------------------
// Rendering on a transparent background

/// Renders the entries with the machine's palette, twice on two different backgrounds: a pixel is
/// opaque if it is identical in both renderings.
fn render(m: &ddsom_game::GameMachine, entries: &[[u16; 4]]) -> Option<Sprite> {
    let mut obj = vec![0u16; 0x1000];
    for (i, e) in entries.iter().take(0x3fc).enumerate() {
        obj[4 * i..4 * i + 4].copy_from_slice(e);
    }
    let end = 4 * entries.len().min(0x3fc);
    obj[end..end + 4].copy_from_slice(&[0x8000, 0x8000, 0, 0]);
    let mut output = m.bus.hw.output;
    output[2] = 0; // no layer priority
    let (cps_a, mut cps_b) = ([0u16; 32], [0u16; 32]);
    cps_b[0x30 / 2] = 0x3f; // the six palette pages
    let shot = |bg: u16| {
        let mut pal = m.bus.hw.palette.clone();
        pal[0xbff] = bg;
        let v = cps2_gfx::VideoState { gfxram: &[], cps_a: &cps_a, cps_b: &cps_b, objram: &obj, output: &output, palette_ram: Some(&pal) };
        cps2_gfx::render(&v, &m.gfx)
    };
    let (a, b) = (shot(0xff0f), shot(0xf0f0));
    let opaque: Vec<bool> = a.iter().zip(&b).map(|(p, q)| p == q).collect();
    let (mut x0, mut y0, mut x1, mut y1) = (W, H, 0, 0);
    for (i, &o) in opaque.iter().enumerate() {
        if o {
            let (x, y) = (i % W, i / W);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    if x1 < x0 {
        return None;
    }
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let i = (y0 + y) * W + x0 + x;
            if opaque[i] {
                rgba[(y * w + x) * 4..][..3].copy_from_slice(&a[i]);
                rgba[(y * w + x) * 4 + 3] = 255;
            }
        }
    }
    // the anchor point is at (ANCHOR_X, line of the object of height ANCHOR_H)
    let row0 = 0x100 - ANCHOR_H as i32 - m.bus.hw.output[3] as i32;
    Some(Sprite { w, h, ox: x0 as i32 - ANCHOR_X as i32, oy: y0 as i32 - row0, rgba })
}

// ---------------------------------------------------------------------------------------------
// scan

fn pools() -> Vec<(&'static str, u32)> {
    let mut v = vec![];
    for (name, (base, size, n)) in [("enemy", ENEMY), ("boss", BOSS)] {
        for k in 0..n {
            v.push((name, base + size * k));
        }
    }
    v.push(("player", PLAYER));
    v
}

/// Sampler: at each frame examines the observed objects and stores the new frames in the cache.
struct Observer {
    cache: PathBuf,
    label: String,
    objects: Vec<(&'static str, u32)>,
    /// Last frame seen per slot.
    last: Vec<u32>,
    /// Number of samples per key and per window of `WINDOW` frames (the palettes of some bosses are
    /// loaded only after they appear: late samples are needed too).
    count: BTreeMap<(String, usize), u32>,
    /// Scenes where each key was seen (number of frames).
    scenes: BTreeMap<(String, u8), u32>,
    seq: u32,
}

impl Observer {
    fn new(cache: &Path, label: &str) -> Self {
        let objects = pools();
        Observer { cache: cache.into(), label: label.into(), last: vec![0; objects.len()], objects, count: BTreeMap::new(), scenes: BTreeMap::new(), seq: 0 }
    }

    fn frame(&mut self, m: &mut ddsom_game::GameMachine, f: usize) {
        let scene = rd8(&mut m.bus, STAGE);
        for i in 0..self.objects.len() {
            let (family, o) = self.objects[i];
            let b = &mut m.bus;
            let (active, kind, variant) = (rd8(b, o), rd8(b, o + 2), rd8(b, o + 3));
            if active == 0 || rd8(b, o + 0x40) != 0 || rd8(b, o + 1) & 2 != 0 {
                self.last[i] = 0;
                continue;
            }
            // player 1: byte +3 is the character number (one key per character)
            let key = if family == "player" { format!("player_{variant:02}_00") } else { format!("{family}_{kind:02}_{variant:02}") };
            if family != "player" {
                *self.scenes.entry((key.clone(), scene)).or_default() += 1;
            }
            let frame = rd32(b, o + 0x34);
            if frame == self.last[i] {
                continue;
            }
            self.last[i] = frame;
            let window = (key.clone(), f / WINDOW);
            if *self.count.entry(window.clone()).or_default() >= PER_WINDOW {
                continue;
            }
            let Some(entries) = object_entries(b, o) else { continue };
            let Some(sp) = render(m, &entries) else { continue };
            *self.count.get_mut(&window).unwrap() += 1;
            let st = [rd8(&mut m.bus, o + 4), rd8(&mut m.bus, o + 5), rd8(&mut m.bus, o + 6)];
            let dir = self.cache.join(&key);
            fs::create_dir_all(&dir).unwrap();
            let name = format!("{}_{:06}", self.label, self.seq);
            sp.write(&dir.join(format!("{name}.png")), 1);
            // the slot links the successive samples of the same object (animation)
            let meta = format!("{} {} {} {} {:02x} {:02x} {:02x} {} {:06x}\n", sp.ox, sp.oy, scene, i, st[0], st[1], st[2], f, frame);
            fs::write(dir.join(format!("{name}.txt")), meta).unwrap();
            self.seq += 1;
        }
    }

    /// Writes the scenes seen: `scenes_<label>.txt` (games played) or `forced_<label>.txt` (objects created by hand).
    fn finish(self, prefix: &str) {
        let mut s = String::new();
        for ((key, scene), n) in &self.scenes {
            s += &format!("{key} {scene} {n}\n");
        }
        fs::write(self.cache.join(format!("{prefix}_{}.txt", self.label)), s).unwrap();
    }
}

fn load_machine(roms: &str, eeprom: &str) -> ddsom_game::GameMachine {
    let ee = fs::read(eeprom).ok();
    let m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(m)
}

/// Replays a scenario and samples the creatures after frame 600.
fn scan(a: &[String]) {
    let [roms, eeprom, scenario, frames, cache] = &a[..5] else { usage() };
    let label = a.get(5).cloned().unwrap_or_else(|| Path::new(scenario).file_stem().unwrap().to_string_lossy().into());
    let mut m = load_machine(roms, eeprom);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let mut obs = Observer::new(Path::new(cache), &label);
    for f in 0..frames.parse::<usize>().expect("frames") {
        script.apply(f, &mut m);
        m.run_frame();
        if f >= 600 {
            obs.frame(&mut m, f);
        }
    }
    obs.finish("scenes");
}

/// Enters scene `scene` (one character, invincible player), spawns by hand an object of family
/// `enemy` or `boss` (type, variant) next to the player and samples it: for the types that
/// the runs never meet. The palette is that of the chosen scene.
///   creatures spawn <roms> <eeprom> <cache> <scene> <enemy|boss> <type> <variant>
fn spawn(a: &[String]) {
    let [roms, eeprom, cache, scene, family, kind, variant] = &a[..7] else { usage() };
    let (scene, kind, variant): (u32, u8, u8) = (scene.parse().unwrap(), kind.parse().unwrap(), variant.parse().unwrap());
    let (pool, size, n) = if family == "boss" { BOSS } else { ENEMY };
    let mut m = load_machine(roms, eeprom);
    let mut obs = Observer::new(Path::new(cache), &format!("forced{scene}-{family}{kind}-{variant}"));
    let mut spawned = false;
    for f in 0..3600usize {
        // coin, Start, first character, jump into the scene (like `boss_scan`)
        for (at, input, down) in [(910, "Coin 1", true), (916, "Coin 1", false), (1000, "1 Player Start", true), (1006, "1 Player Start", false), (1440, "P1 Button 1", true), (1446, "P1 Button 1", false)] {
            if f == at + 1 {
                ddsom_game::scenario::set_input(&mut m, input, down);
            }
        }
        if f == 2400 {
            ddsom_game::scenario::poke(&mut m, 0xff8034, scene, 1);
        }
        for k in 0..2 {
            ddsom_game::scenario::poke(&mut m, PLAYER + 0x200 * k + 0x62, 0x70, 2);
        }
        m.run_frame();
        if f == 3000 && !spawned {
            let b = &mut m.bus;
            let slot = (0..n).map(|k| pool + size * k).find(|&o| rd8(b, o) == 0).expect("pool plein");
            let (px, py) = (rd32(b, PLAYER + 8), rd32(b, PLAYER + 0x10));
            for off in 0..size {
                ddsom_game::mem::wr8(b, slot + off, 0);
            }
            ddsom_game::mem::wr8(b, slot + 0x28, if family == "boss" { 4 } else { 2 });
            ddsom_game::mem::wr8(b, slot + 2, kind);
            ddsom_game::mem::wr8(b, slot + 3, variant);
            ddsom_game::mem::wr32(b, slot + 8, px.wrapping_add(0x60 << 16));
            ddsom_game::mem::wr32(b, slot + 0x10, py);
            if family == "enemy" {
                let free = rd16(b, A5 + 0x7e62);
                ddsom_game::mem::wr16(b, A5 + 0x7e62, free.wrapping_sub(1));
            }
            ddsom_game::mem::wr8(b, slot, 1);
            spawned = true;
        }
        if f > 3000 {
            obs.frame(&mut m, f);
        }
    }
    obs.finish("forced");
}

// ---------------------------------------------------------------------------------------------
// build

const ENEMY_NAMES: [&str; 29] = [
    "Kobold", "Skeleton", "Troglodyte", "Bow Gnoll", "Owlbear", "Ghoul", "Hellhound", "Wyvern", "Gargoyle", "Giant Beetle",
    "Giant Scorpion", "Mud Golem", "Shadow Elf", "Axe Gnoll", "Troll", "Goblin", "Riyakaa", "(inutilise)", "Ogre",
    "Displacer Beast", "Man Scorpion", "Harpy", "Chimera", "Spectre", "Manticore", "Dark Warrior 1", "Dark Warrior 2",
    "War Machine", "Nagpa",
];
const BOSS_NAMES: [(u8, &str); 16] = [
    (4, "Manticore (type 4)"), (7, "(sans nom)"), (20, "Dragon rouge (type 20)"), (21, "Compagnon de Tel'Arin"), (5, "Ezerhorden"), (6, "Manticore"), (8, "Dark Warrior 2"), (10, "Nagpa"), (11, "Salamandre de glace"),
    (12, "Salamandre de feu"), (13, "Dragon noir en vol"), (14, "Dragon vert"), (15, "Dragon rouge"), (17, "Tel'Arin"),
    (18, "Lich"), (19, "Dragon noir"),
];
const PLAYER_NAMES: [&str; 6] = ["Fighter", "Cleric", "Elf", "Dwarf", "Magic-User", "Thief"];

fn name_of(family: &str, t: u8) -> String {
    match family {
        "enemy" => ENEMY_NAMES.get(t as usize).unwrap_or(&"?").to_string(),
        "boss" => BOSS_NAMES.iter().find(|n| n.0 == t).map_or("?", |n| n.1).to_string(),
        _ => PLAYER_NAMES.get(t as usize / 2).unwrap_or(&"?").to_string(),
    }
}

/// Remarks on what the image shows when it differs from the type's name (noted from the sheets).
fn remark(family: &str, t: u8, variant: u8) -> &'static str {
    match (family, t, variant) {
        ("enemy", 19, _) => "guerrier à bouclier et bombe (gobelin ou gnoll), pas un Displacer Beast : nom du type hypothétique",
        ("enemy", 20, 1) => "épave de la scène 1 (décor), pas un Man Scorpion",
        ("enemy", 22..=24, _) => "morceau recopié sur un maître (tête ou queue d'un boss), pas une créature entière",
        ("enemy", 25, _) => "patte du dragon rouge (scène 46), pas un Dark Warrior",
        ("enemy", 7, _) => "tentacule : partie attachée à un hôte",
        ("enemy", 12, 4 | 6) => "faisceau blanc d'apparition, avant la forme visible",
        ("enemy", 18, _) => "montre un manticore ailé, pas un Ogre",
        ("boss", 4, _) => "manticore",
        ("boss", 5, _) => "volutes d'apparition (l'image montre peu de choses avant la matérialisation)",
        ("boss", 7, _) => "silhouette en transparence tramée",
        ("boss", 15 | 20, _) => "tête du dragon rouge",
        _ => "",
    }
}

struct Sample {
    sprite: Sprite,
    slot: u32,
    states: [u8; 3],
    frame: u32,
    file: String,
}

fn load_samples(dir: &Path) -> Vec<Sample> {
    let mut names: Vec<_> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    names.sort();
    names
        .into_iter()
        .map(|p| {
            let t = fs::read_to_string(&p).unwrap();
            let v: Vec<&str> = t.split_whitespace().collect();
            let hex = |s: &str| u8::from_str_radix(s, 16).unwrap();
            Sample {
                sprite: Sprite::read(&p.with_extension("png"), v[0].parse().unwrap(), v[1].parse().unwrap()),
                slot: v[3].parse().unwrap(),
                states: [hex(v[4]), hex(v[5]), hex(v[6])],
                frame: u32::from_str_radix(v[8], 16).unwrap(),
                file: p.file_stem().unwrap().to_string_lossy().into(),
            }
        })
        .collect()
}

/// Base pose: among the samples in active state (+4 = 2) and walking sub-state (+5 = 0) with a loaded
/// palette, the one of median size (discards progressive appearances and attack gestures).
fn base_index(s: &[Sample]) -> usize {
    let usable = |x: &&(usize, &Sample)| !x.1.sprite.garbage();
    let all: Vec<(usize, &Sample)> = s.iter().enumerate().collect();
    let mut pool: Vec<&(usize, &Sample)> = all.iter().filter(|x| x.1.states[0] == 2 && x.1.states[1] == 0).filter(usable).collect();
    if pool.is_empty() {
        pool = all.iter().filter(usable).collect();
    }
    if pool.is_empty() {
        return 0;
    }
    pool.sort_by_key(|x| x.1.sprite.opaque());
    pool[pool.len() / 2].0
}

/// Sequence of at most `n` distinct frames of the same slot and the same run, starting from `from`.
fn animation(s: &[Sample], from: usize, n: usize) -> Vec<&Sample> {
    let run = s[from].file.rsplit_once('_').unwrap().0;
    let mut v: Vec<&Sample> = vec![];
    for x in &s[from..] {
        if x.file.rsplit_once('_').unwrap().0 != run || x.slot != s[from].slot {
            break;
        }
        if v.iter().all(|y| y.frame != x.frame) {
            v.push(x);
        }
        if v.len() == n {
            break;
        }
    }
    v
}

/// Sheet: sprites aligned on their anchor, laid out in a grid, with a caption below (drawn by hand).
struct Sheet {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Sheet {
    fn new(w: usize, h: usize) -> Self {
        let mut px = vec![0u8; w * h * 4];
        for p in px.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[40, 40, 52, 255]);
        }
        Sheet { w, h, px }
    }
    fn blit(&mut self, s: &Sprite, x: i32, y: i32) {
        for sy in 0..s.h {
            for sx in 0..s.w {
                let (dx, dy) = (x + sx as i32, y + sy as i32);
                let a = s.rgba[(sy * s.w + sx) * 4 + 3];
                if a != 0 && dx >= 0 && dy >= 0 && (dx as usize) < self.w && (dy as usize) < self.h {
                    let d = (dy as usize * self.w + dx as usize) * 4;
                    self.px[d..d + 4].copy_from_slice(&s.rgba[(sy * s.w + sx) * 4..][..4]);
                }
            }
        }
    }
    fn text(&mut self, x: i32, y: i32, t: &str) {
        let mut cx = x;
        for c in t.chars() {
            if let Some(g) = glyph(c) {
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..5 {
                        if bits >> (4 - col) & 1 != 0 {
                            let (px, py) = (cx + col, y + row as i32);
                            if px >= 0 && py >= 0 && (px as usize) < self.w && (py as usize) < self.h {
                                let d = (py as usize * self.w + px as usize) * 4;
                                self.px[d..d + 4].copy_from_slice(&[235, 235, 235, 255]);
                            }
                        }
                    }
                }
            }
            cx += 6;
        }
    }
}

/// Minimal 5x7 font (capitals, digits, a few signs) for the sheet captions.
fn glyph(c: char) -> Option<[u8; 7]> {
    const FONT: &[(char, [u8; 7])] = &[
        ('A', [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]), ('B', [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e]),
        ('C', [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e]), ('D', [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e]),
        ('E', [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f]), ('F', [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10]),
        ('G', [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f]), ('H', [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
        ('I', [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e]), ('J', [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c]),
        ('K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]), ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f]),
        ('M', [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11]), ('N', [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
        ('O', [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]), ('P', [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10]),
        ('Q', [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d]), ('R', [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11]),
        ('S', [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e]), ('T', [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
        ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]), ('V', [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04]),
        ('W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a]), ('X', [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11]),
        ('Y', [0x11, 0x11, 0x11, 0x0a, 0x04, 0x04, 0x04]), ('Z', [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f]),
        ('0', [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e]), ('1', [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e]),
        ('2', [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f]), ('3', [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e]),
        ('4', [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02]), ('5', [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e]),
        ('6', [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e]), ('7', [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
        ('8', [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e]), ('9', [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c]),
        ('(', [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02]), (')', [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08]),
        ('-', [0, 0, 0, 0x1f, 0, 0, 0]), ('\'', [0x04, 0x04, 0x08, 0, 0, 0, 0]), ('.', [0, 0, 0, 0, 0, 0x0c, 0x0c]),
    ];
    let c = c.to_ascii_uppercase();
    FONT.iter().find(|f| f.0 == c).map(|f| f.1)
}

/// A creature: an object type and a variant, with its base pose and a few animation frames.
struct Entry {
    family: String,
    t: u8,
    variant: u8,
    name: String,
    file: String,
    scenes: Vec<u8>,
    /// Never met in the runs: seen only during a `creatures spawn` trial in the given scene.
    forced: bool,
    sprite: Sprite,
    frames: Vec<Sprite>,
}

/// Scenes where each key (`family_type_variant`) was seen, from the `<prefix>_*.txt` of `scan` (`scenes`)
/// or `spawn` (`forced`).
fn load_scenes(cache: &Path, prefix: &str) -> BTreeMap<String, BTreeSet<u8>> {
    let mut scenes: BTreeMap<String, BTreeSet<u8>> = BTreeMap::new();
    for e in fs::read_dir(cache).unwrap().flatten() {
        if e.file_name().to_string_lossy().starts_with(prefix) {
            for l in fs::read_to_string(e.path()).unwrap().lines() {
                let v: Vec<&str> = l.split(' ').collect();
                scenes.entry(v[0].into()).or_default().insert(v[1].parse().unwrap());
            }
        }
    }
    scenes
}

fn load_entries(cache: &Path, images: &Path) -> Vec<Entry> {
    let (scenes, forced) = (load_scenes(cache, "scenes_"), load_scenes(cache, "forced_"));
    let mut entries = vec![];
    for e in fs::read_dir(cache).unwrap().flatten() {
        let key = e.file_name().to_string_lossy().to_string();
        let p: Vec<&str> = key.split('_').collect();
        // odd character numbers are transient states of the player
        if !e.path().is_dir() || p.len() != 3 || (p[0] == "player" && p[1].parse::<u8>().unwrap() % 2 == 1) {
            continue;
        }
        let (family, t, variant): (String, u8, u8) = (p[0].into(), p[1].parse().unwrap(), p[2].parse().unwrap());
        let samples = load_samples(&e.path());
        if samples.is_empty() {
            continue;
        }
        let bi = base_index(&samples);
        let frames: Vec<Sprite> = animation(&samples, bi, 8).iter().map(|s| s.sprite.clone()).collect();
        entries.push(Entry {
            name: name_of(&family, t),
            forced: family != "player" && !scenes.contains_key(&key),
            scenes: scenes.get(&key).or(forced.get(&key)).map(|s| s.iter().copied().collect()).unwrap_or_default(),
            file: key,
            sprite: samples[bi].sprite.clone(),
            frames,
            family,
            t,
            variant,
        });
    }
    let order = |f: &str| match f {
        "player" => 0,
        "enemy" => 1,
        _ => 2,
    };
    entries.sort_by_key(|e| (order(&e.family), e.t, e.variant));
    for e in &entries {
        e.sprite.write(&images.join(format!("{}.png", e.file)), 1);
        e.sprite.write(&images.join(format!("{}_x2.png", e.file)), 2);
        if e.frames.len() > 1 {
            let s = strip(&e.frames, 2);
            save_png(&images.join(format!("{}_anim.png", e.file)), s.w, s.h, &s.px);
        }
    }
    entries
}

/// Summary sheet: the creatures of the list in rows, caption (type.variant name) below.
fn contact_sheet(es: &[&Entry]) -> Sheet {
    const WIDTH: usize = 1500;
    let mut placed = vec![];
    let (mut x, mut y, mut row_h) = (0usize, 0usize, 0usize);
    for e in es {
        let label = format!("{}.{} {}", e.t, e.variant, e.name);
        let w = (e.sprite.w + 12).max(6 * label.len() + 8);
        if x + w > WIDTH {
            (x, y, row_h) = (0, y + row_h, 0);
        }
        placed.push((e, label, x, y, w));
        x += w;
        row_h = row_h.max(e.sprite.h + 28);
    }
    let mut sh = Sheet::new(WIDTH, y + row_h);
    let mut rows: BTreeMap<usize, usize> = BTreeMap::new();
    for (e, _, _, y, _) in &placed {
        let r = rows.entry(*y).or_default();
        *r = (*r).max(e.sprite.h + 28);
    }
    for (e, label, x, y, w) in placed {
        let rh = rows[&y];
        sh.blit(&e.sprite, (x + (w - e.sprite.w) / 2) as i32, (y + rh - 22 - e.sprite.h) as i32);
        sh.text((x + (w - 6 * label.len()) / 2) as i32, (y + rh - 14) as i32, &label);
    }
    sh
}

fn build(a: &[String]) {
    let [cache, images, index] = &a[..3] else { usage() };
    let (cache, images) = (Path::new(cache), Path::new(images));
    fs::create_dir_all(images).unwrap();
    let entries = load_entries(cache, images);
    let sections = [("player", "personnages", "Personnages jouables", "joueur"), ("enemy", "ennemis", "Ennemis (pool ENEMY, 29 types)", "ENEMY"), ("boss", "boss", "Boss (pool BOSS)", "BOSS")];
    let mut md = String::from(INTRO);
    for (family, slug, title, fam) in sections {
        let es: Vec<&Entry> = entries.iter().filter(|e| e.family == family).collect();
        let sheet = contact_sheet(&es);
        save_png(&images.join(format!("planche_{slug}.png")), sheet.w, sheet.h, &sheet.px);
        md += &format!("## {title}\n\n![planche des {slug}](../assets/creatures/planche_{slug}.png)\n\n");
        md += "| Type | Variante | Nom | Famille | Scènes (`FF8034`) | Images | Remarque |\n|---|---|---|---|---|---|---|\n";
        for e in es {
            let list = e.scenes.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ");
            let sc = match (e.forced, list.is_empty()) {
                (true, _) => format!("hors parcours ; essai forcé en scène {list}"),
                (false, true) => "–".to_string(),
                _ => list,
            };
            let f = &e.file;
            let anim = if e.frames.len() > 1 { format!(" · [anim](../assets/creatures/{f}_anim.png)") } else { String::new() };
            md += &format!("| {} | {} | {} | {fam} | {sc} | [1:1](../assets/creatures/{f}.png) · [x2](../assets/creatures/{f}_x2.png){anim} | {} |\n", e.t, e.variant, e.name, remark(&e.family, e.t, e.variant));
        }
        md += "\n";
    }
    let missing: Vec<String> = (0..29u8).filter(|t| *t != 17 && !entries.iter().any(|e| e.family == "enemy" && e.t == *t)).map(|t| format!("{t} {}", ENEMY_NAMES[t as usize])).collect();
    if !missing.is_empty() {
        md += &format!("Types d'ennemis sans image : {} (jamais rencontrés dans les parcours ; l'objet créé à la main par `spawn` ne montre rien).\n", missing.join(", "));
    }
    fs::write(index, md).unwrap();
    println!("{} creatures", entries.len());
}

const INTRO: &str = "# Les créatures du jeu

Image de chaque créature (types d'ennemis du pool ENEMY, boss du pool BOSS, six personnages jouables) dans sa pose
de base (premier cadre de l'animation de marche ou d'attente), avec sa palette, sur fond transparent, à l'échelle
1:1 (`x2` : agrandie deux fois) avec une bande de cadres d'animation (`anim`). Un même type d'objet a souvent
plusieurs **variantes** (octet `+3`) : autre couleur, autre créature, autre rôle ; chacune a son image.
Les noms des types sont ceux de `docs/familles/enemy.md` et `boss.md` (hypothèses : une variante peut montrer
autre chose que le nom du type).

Les images sont tirées des ROMs : **elles ne sont pas versionnées**. Le dossier `assets/creatures/` est ignoré
par git (`assets/` dans `.gitignore`) ; les liens ci-dessous ne fonctionnent qu'après génération.

## Régénérer les images

```sh
DDSOM_ROMS=rom tools/creatures.sh      # scan des scénarios de couverture puis assemblage -> assets/creatures/
tools/creatures.sh build               # seulement l'assemblage (images, planches, ce fichier)
```

Le script compile `port/ddsom-game/examples/creatures.rs`, lance `creatures scan` sur les scénarios
`scenarios/cov/cov_tour*.txt` et `cov_natural_c0.txt` (4 en parallèle, 30 000 images chacun), puis `creatures build`
qui réécrit les PNG, les planches `planche_*.png` et ce fichier. `CREATURES_EXTRA=\"nom ...\"` ajoute des scénarios
de `scenarios/cov/`.

## Méthode

À chaque image de la partie rejouée (code natif), `scan` lit les objets des pools ENEMY et BOSS et le joueur 1.
Pour un objet dont le cadre de sprite (`+0x34`, voir `docs/objets.md`) change, il reconstruit les entrées d'Object
RAM de cet objet seul, comme `sprites.rs` : morceaux de 16x16 décalés, attributs par morceau, palette imposée
(`+0x2a`/`+0x2b`), objets rattachés (`+0x48`/`+0x4a`) devant ou derrière. Il les rend avec `cps2-gfx` et la
**palette courante de la machine** ; la transparence vient de deux rendus sur deux fonds différents (un pixel est
opaque s'il est identique dans les deux). L'orientation est celle de la ROM (sans miroir) ; les ombres sont omises.
`build` choisit la pose de base (premier échantillon en état actif `+4 = 2`, sous-état de marche `+5 = 0`), et prend
la suite de cadres distincts du même objet pour la bande d'animation.

## Limites

* Seules les créatures rencontrées dans les scénarios sont présentes ; une créature n'apparaît que dans les scènes
  atteintes par ces parcours (colonne 'scenes').
* Un objet constitué de plusieurs objets (corps et membres séparés, boss à parties) n'est rendu que par son objet
  principal et ses objets rattachés.
* La palette est celle de la scène : une créature peut changer de teinte d'une scène à l'autre ; les images montrent
  la première rencontrée.

";

/// Strip of frames aligned on their anchor.
fn strip(frames: &[Sprite], gap: usize) -> Sheet {
    let x0 = frames.iter().map(|f| f.ox).min().unwrap();
    let x1 = frames.iter().map(|f| f.ox + f.w as i32).max().unwrap();
    let y0 = frames.iter().map(|f| f.oy).min().unwrap();
    let y1 = frames.iter().map(|f| f.oy + f.h as i32).max().unwrap();
    let (cw, ch) = ((x1 - x0) as usize + gap * 2, (y1 - y0) as usize + gap * 2);
    let mut s = Sheet::new(cw * frames.len(), ch);
    for (i, f) in frames.iter().enumerate() {
        s.blit(f, (i * cw + gap) as i32 + f.ox - x0, gap as i32 + f.oy - y0);
    }
    s
}

fn usage() -> ! {
    eprintln!("usage: creatures scan <roms> <eeprom> <scenario> <frames> <cache> [label]\n        creatures spawn <roms> <eeprom> <cache> <scene> <enemy|boss> <type> <variant>\n        creatures build <cache> <images_folder> <index.md>");
    std::process::exit(2);
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("scan") if a.len() >= 6 => scan(&a[1..]),
        Some("spawn") if a.len() == 8 => spawn(&a[1..]),
        Some("build") if a.len() == 4 => build(&a[1..]),
        _ => usage(),
    }
}
