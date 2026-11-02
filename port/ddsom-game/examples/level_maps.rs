//! Level maps: complete image of the scenery (scroll 2 and scroll 3) of each scene, without sprites or
//! interface, rebuilt from the ROM's scenery maps. See `docs/niveaux.md`.
//!
//!   level_maps cartes  <roms> <eeprom> <folder> [scene...]   one image per scene (1 to 82 by default)
//!   level_maps planche <folder> <niveaux.md>                 overview sheet `sheet.png` + Markdown index
//!   level_maps tables  <roms>                                dimensions of the strip tables of the 115 scenes
//!
//! Method. The game's cameras (`scroll.rs`) copy into their layers columns of cells read from a
//! ROM map: 256x256-pixel *strips* (2x2 block indices, block banks) designated, for
//! each scene, by a table of words (`0x2ff94 + 4 s` for scroll 2, 16-pixel cells; `0x32d7c + 4 s` for
//! scroll 3, 32-pixel cells). The tool reads these same tables (see `Layer::grid`), recomposes all the
//! cells of the map, draws them with the scene's palette taken from the machine (real game run up to
//! the scene's game phase, `FF8034` forced as in `docs/coverage.md`) and superimposes the two layers
//! in the order set by CPS-B. No sprite, no interface, no scroll 1.

use std::path::Path;

const MAP1: u32 = 0x1f9ba0;
const BANKS1: u32 = 0x320f6;
const TABLES1: u32 = 0x2ff94;
const MAP2: u32 = 0x2459c0;
const BANKS2: u32 = 0x33a9c;
const TABLES2: u32 = 0x32d7c;
/// Base of the scenery tiles in the graphics region (bank 1 of the CPS2 mapper).
const TILES: usize = 0x20000 * 64;
const SCENES: std::ops::RangeInclusive<u32> = 1..=82;
/// Screen width: step of the superposition of the parallax layers.
const SCREEN: usize = 384;
const BACKGROUND: [u8; 3] = [24, 16, 40];

// ---------------------------------------------------------------------------------------------
// Cartes en ROM
// ---------------------------------------------------------------------------------------------

struct Rom<'a> {
    data: &'a [u16],
    prog: &'a [u16],
    gfx: &'a [u8],
}

impl Rom<'_> {
    fn rd16(&self, a: u32) -> u16 {
        self.data.get((a >> 1) as usize).copied().unwrap_or(0)
    }
    fn rd32(&self, a: u32) -> u32 {
        (self.rd16(a) as u32) << 16 | self.rd16(a.wrapping_add(2)) as u32
    }
    /// Read in program space (code tables).
    fn prog32(&self, a: u32) -> u32 {
        (self.prog[(a >> 1) as usize] as u32) << 16 | self.prog[(a >> 1) as usize + 1] as u32
    }
}

/// A scenery layer: geometry of its ROM map.
#[derive(Clone, Copy)]
struct Layer {
    map: u32,
    banks: u32,
    tables: u32,
    /// Address of the line-step word of the first strip table.
    start: u32,
    /// Size of a cell in pixels (16 or 32) and number of cells per strip side.
    case: usize,
    cases: usize,
    /// Tile offset added to the map code, colour base of the hardware.
    offset: u16,
    colour: u16,
    /// Fill strip number (never displayed: the camera does not go that far), left empty.
    empty: u16,
}

const SCROLL2: Layer = Layer { map: MAP1, banks: BANKS1, tables: TABLES1, start: 0x318a2, case: 16, cases: 16, offset: 0x1800, colour: 0x40, empty: 0xfc };
const SCROLL3: Layer = Layer { map: MAP2, banks: BANKS2, tables: TABLES2, start: 0x33564, case: 32, cases: 8, offset: 0x1f00, colour: 0x60, empty: 0xfd };

impl Layer {
    /// Cell (tile code, attribute) at address `at` of the virtual map (`map_cell` of `scroll.rs`).
    fn case_a(&self, rom: &Rom, at: u32) -> (u16, u16) {
        let off = at.wrapping_sub(self.map);
        let c = off >> 2;
        let (bank, index, sub) = if self.case == 16 {
            let bank = rom.rd32(self.banks + (off >> 16 & 0xc));
            let index = rom.rd16(self.map + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32;
            (bank, index, (off as u16 >> 3 & 8) as u32 + (off & 4))
        } else {
            let bank = rom.rd32(self.banks + ((off >> 16 & 0xf) << 2));
            let index = rom.rd16(self.map + (c & 6) + (c >> 1 & !7)) as u32;
            (bank, index, (off as u16 >> 2 & 8) as u32 + (off & 4))
        };
        let v = rom.rd32(bank.wrapping_add(index << 4).wrapping_add(sub));
        ((v >> 16) as u16 + self.offset, v as u16)
    }

    /// Grid of the scene's strip numbers, from top to bottom (empty if the tables are inconsistent).
    ///
    /// The tables of all the scenes follow one another in ROM: a line-step word `W` (the camera's,
    /// `+0x40`), `K` lines of `W` words above the line of ordinate 0 (the one designated by the scene's
    /// pointer: increasing ordinates are at decreasing addresses), then a few lines of negative
    /// ordinate (the camera goes down a little below 0). A table's pointer therefore fixes where the next one begins:
    /// the reading starts from the first table and searches, after each pointer, for the number of negative lines
    /// that leads to a consistent header and following pointer. In scroll 2 the last word of each
    /// line is a sentinel (0xfe).
    fn grid(&self, rom: &Rom, scene: u32) -> Vec<Vec<u16>> {
        let mut pointers: Vec<u32> = (0..115).map(|s| rom.prog32(self.tables + 4 * s)).collect();
        pointers.sort();
        pointers.dedup();
        let target = rom.prog32(self.tables + 4 * scene);
        let valid = |head: u32, p: u32| {
            let w = rom.rd16(head) as u32;
            (1..=64).contains(&w) && p >= head + 2 && (p - head - 2) % (2 * w) == 0
        };
        let mut head = self.start;
        for (i, &p) in pointers.iter().enumerate() {
            if !valid(head, p) {
                return Vec::new();
            }
            let w = rom.rd16(head) as u32;
            let after = (0..=8u32).find(|t| pointers.get(i + 1).map_or(*t == 0, |&q| valid(p + 2 * w * (1 + t), q)));
            let Some(after) = after else { return Vec::new() };
            if p == target {
                let sentinelle = (self.case == 16) as u32;
                return (0..=(p - head - 2) / (2 * w) + after)
                    .map(|i| (0..w - sentinelle).map(|x| rom.rd16(head + 2 + 2 * w * i + 2 * x)).collect())
                    .collect();
            }
            head = p + 2 * w * (1 + after);
        }
        Vec::new()
    }

    /// Scenery image: the grid's strips side by side, each cell drawn with palette `pal`.
    fn image(&self, rom: &Rom, pal: &[[u8; 3]], grid: &[Vec<u16>]) -> Image {
        let band = self.case * self.cases;
        let mut img = Image::new(grid[0].len() * band, grid.len() * band);
        let column = if self.case == 16 { 0x40 } else { 0x20 };
        for (ky, line) in grid.iter().enumerate() {
            for (kx, &strip) in line.iter().enumerate() {
                if strip == self.empty {
                    continue;
                }
                let base = self.map + (strip as u32) * (self.cases * self.cases * 4) as u32;
                for cx in 0..self.cases {
                    for cy in 0..self.cases {
                        let (code, attr) = self.case_a(rom, base + cx as u32 * column + cy as u32 * 4);
                        self.tile(rom, pal, &mut img, kx * band + cx * self.case, ky * band + cy * self.case, code, attr);
                    }
                }
            }
        }
        img
    }

    /// The scene uses the default table (that of scene 1): its layer is not taken from a map.
    fn shared(&self, rom: &Rom, scene: u32) -> bool {
        rom.prog32(self.tables + 4 * scene) == rom.prog32(self.tables + 4)
    }


    /// Draws a tile (`cps2-gfx` decoding: one plane per byte, mirrors at bits 5 and 6 of the attribute).
    fn tile(&self, rom: &Rom, pal: &[[u8; 3]], img: &mut Image, x0: usize, y0: usize, code: u16, attr: u16) {
        let t = self.case;
        for py in 0..t {
            for px in 0..t {
                let sx = if attr & 0x20 != 0 { t - 1 - px } else { px };
                let sy = if attr & 0x40 != 0 { t - 1 - py } else { py };
                let line = if t == 16 {
                    TILES + code as usize * 128 + sy * 8 + (sx / 8) * 4
                } else {
                    TILES + (code as usize & 0x3fff) * 512 + sy * 16 + (sx / 8) * 4
                };
                if line + 4 > rom.gfx.len() {
                    continue;
                }
                let bit = 7 - sx % 8;
                let p = (rom.gfx[line + 3] >> bit & 1) << 3 | (rom.gfx[line + 2] >> bit & 1) << 2 | (rom.gfx[line + 1] >> bit & 1) << 1 | (rom.gfx[line] >> bit & 1);
                if p != 15 {
                    img.set(x0 + px, y0 + py, pal[((attr & 0x1f) + self.colour) as usize * 16 + p as usize]);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------------------------

/// RGBA image (alpha 0: nothing drawn).
struct Image {
    w: usize,
    h: usize,
    px: Vec<[u8; 4]>,
}

impl Image {
    fn new(w: usize, h: usize) -> Self {
        Image { w, h, px: vec![[0; 4]; w * h] }
    }

    fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        self.px[y * self.w + x] = [c[0], c[1], c[2], 255];
    }

    /// Removes the entirely empty borders.
    fn reframe(self) -> Image {
        let (mut x0, mut y0, mut x1, mut y1) = (self.w, self.h, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.px[y * self.w + x][3] != 0 {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                }
            }
        }
        if x1 == 0 {
            return self;
        }
        let mut img = Image::new(x1 - x0, y1 - y0);
        for y in y0..y1 {
            img.px[(y - y0) * img.w..][..img.w].copy_from_slice(&self.px[y * self.w + x0..][..img.w]);
        }
        img
    }

    fn png(&self, path: &Path) {
        let mut buf = Vec::with_capacity(self.w * self.h * 3);
        for p in &self.px {
            buf.extend_from_slice(if p[3] == 0 { &BACKGROUND } else { &p[..3] });
        }
        write_png(path, self.w, self.h, &buf);
    }
}


/// Superimposes the layers (from back to front), each with the ordinate `u` of its bottom line (the
/// scenery ordinates increase upwards; the gap between layers comes from the cameras at the start of the scene).
/// A layer narrower than the frame is a parallax one: the screen that sees column `x` of the frame sees
/// its columns from `x * (w - 384) / (frame - 384)`; the image is therefore assembled in slices of one
/// screen, each as the player sees it.
fn compose(layers: &[(&Image, i64)]) -> Image {
    let w = layers.iter().map(|c| c.0.w).max().unwrap();
    let low = layers.iter().map(|c| c.1).min().unwrap();
    let high = layers.iter().map(|c| c.1 + c.0.h as i64).max().unwrap();
    let h = (high - low) as usize;
    let mut img = Image::new(w, h);
    let mut x0 = 0;
    while x0 < w {
        let camera = x0.min(w.saturating_sub(SCREEN));
        for &(c, u) in layers {
            let offset = if c.w >= w || w <= SCREEN { camera } else { camera * c.w.saturating_sub(SCREEN) / (w - SCREEN) };
            let y0 = (high - u - c.h as i64) as usize;
            for x in x0..(x0 + SCREEN).min(w) {
                let sx = x - camera + offset;
                if sx >= c.w {
                    continue;
                }
                for sy in 0..c.h {
                    let p = c.px[sy * c.w + sx];
                    if p[3] != 0 {
                        img.px[(y0 + sy) * w + x] = p;
                    }
                }
            }
        }
        x0 += SCREEN;
    }
    img.reframe()
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) {
    let mut e = png::Encoder::new(std::fs::File::create(path).unwrap(), w as u32, h as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_compression(png::Compression::Fast);
    e.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn read_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let mut r = png::Decoder::new(std::fs::File::open(path).unwrap()).read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).unwrap();
    (info.width as usize, info.height as usize, buf)
}

// ---------------------------------------------------------------------------------------------
// Machine: reaching a scene
// ---------------------------------------------------------------------------------------------

fn rw(m: &cps2::Machine, a: u32) -> u16 {
    m.bus.ram[((a - 0xff0000) / 2) as usize]
}

fn phase(m: &cps2::Machine) -> u32 {
    (rw(m, 0xff802c) >> 8) as u32
}

fn scene(m: &cps2::Machine) -> u32 {
    (rw(m, 0xff8034) >> 8) as u32
}

fn poke(m: &mut cps2::Machine, a: u32, v: u32, size: u8) {
    ddsom_game::scenario::poke(m, a, v, size);
}

/// Starts the game: coin, Start, chosen character, `FF8034` forced during the selection (scene 1).
fn start_up(roms: &Path, eeprom: &Path) -> cps2::Machine {
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(roms, ee.as_deref()).unwrap();
    ddsom_game::install(&mut m);
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    let inputs = [(922, "Coin 1", true), (928, "Coin 1", false), (1002, "1 Player Start", true), (1008, "1 Player Start", false), (1442, "P1 Button 1", true), (1448, "P1 Button 1", false)];
    for f in 0..2400 {
        for &(i, name, low) in &inputs {
            if i == f {
                ddsom_game::scenario::set_input(&mut m, name, low);
            }
        }
        if f == 2399 {
            poke(&mut m, 0xff8034, 1, 1);
        }
        m.run_frame();
    }
    m
}

/// What the machine gives of a scene in play: palette and layer order.
struct Reading {
    pal: Vec<[u8; 3]>,
    /// Scenery layers (2, 3) from back to front.
    order: Vec<u8>,
    /// Ordinate gap between cameras 1 (scroll 2) and 2 (scroll 3) at the start of the scene.
    gap_y: i64,
    in_game: bool,
}

/// Jumps to scene `n` (game in progress, invincible player), waits for the game phase and reads the palette.
fn enter(m: &mut cps2::Machine, n: u32) -> Reading {
    for (a, v) in [(0xff8034, n), (0xff802c, 8), (0xff802d, 0), (0xff802e, 0)] {
        poke(m, a, v, 1);
    }
    let (mut stable, mut in_game) = (0, false);
    for _ in 0..4000 {
        m.run_frame();
        poke(m, 0xff85de + 0x62, 0x70, 2);
        if phase(m) == 12 && scene(m) == n {
            stable += 1;
            if stable >= 90 {
                in_game = true;
                break;
            }
        }
    }
    let b = &m.bus;
    let v = cps2_gfx::VideoState { gfxram: &b.gfxram, cps_a: &b.cps_a, cps_b: &b.cps_b, objram: &m.obj_buffer, output: &m.pri_ctrl_buffer, palette_ram: Some(&b.palette) };
    let command = b.cps_b[0x26 / 2];
    // active layer: CPS-B bit (0x04 / 0x08) and CPS-A video command bit (4 / 8), as in `cps2-gfx`
    let active = |l: u8| command & (1 << l) != 0 && b.cps_a[0x22 / 2] & (1 << l) != 0;
    let order = (0..4).map(|i| (command >> (6 + 2 * i) & 3) as u8).filter(|&l| (l == 2 || l == 3) && active(l)).collect();
    // target positions of the cameras (records `A5 + 0x4fe` and `A5 + 0x54e`, field `+0x0c`)
    let (y1, y2) = (rw(m, 0xff84fe + 0xc) as i16 as i64, rw(m, 0xff854e + 0xc) as i16 as i64);
    Reading { pal: cps2_gfx::render::build_palette(&v), order, gap_y: y1 - y2, in_game }
}

/// Scene that naturally follows: phase 16 (scene exit) is forced and the next scene is read.
fn next_scene(m: &mut cps2::Machine) -> Option<u32> {
    let n = scene(m);
    for (a, v) in [(0xff802c, 16), (0xff802d, 0), (0xff802e, 0)] {
        poke(m, a, v, 1);
    }
    for _ in 0..3000 {
        m.run_frame();
        poke(m, 0xff85de + 0x62, 0x70, 2);
        if scene(m) != n && phase(m) == 8 {
            return Some(scene(m));
        }
    }
    None
}

fn cards(roms: &Path, eeprom: &Path, exit: &Path, scenes: &[u32]) {
    let mut m = start_up(roms, eeprom);
    let (gfx, data, prog) = (m.gfx.clone(), m.bus.rom.clone(), m.bus.rom_dec.clone());
    let rom = Rom { data: &data, prog: &prog, gfx: &gfx };
    for d in ["couches", "infos"] {
        std::fs::create_dir_all(exit.join(d)).unwrap();
    }
    let routes = std::env::var("DDSOM_ROUTES").is_ok();
    for &n in scenes {
        let r = enter(&mut m, n);
        if std::env::var("DDSOM_REF").is_ok() {
            write_png(&exit.join(format!("ref_{n:02}.png")), 384, 224, &m.render().concat());
        }
        let (g2, g3) = (SCROLL2.grid(&rom, n), SCROLL3.grid(&rom, n));
        if g2.is_empty() || g3.is_empty() {
            eprintln!("scene {n}: unreadable strip tables");
            continue;
        }
        let (s2, s3) = (SCROLL2.image(&rom, &r.pal, &g2), SCROLL3.image(&rom, &r.pal, &g3));
        // The scenes whose scroll 3 table is the default one (scene 1) have no map for this
        // layer: its memory is a leftover of a previous scene, masked by scroll 2 (checked by capture).
        let without_s3 = n != 1 && SCROLL3.shared(&rom, n);
        let order: Vec<u8> = r.order.iter().copied().filter(|&l| l == 2 || !without_s3).collect();
        if order.is_empty() {
            eprintln!("scene {n}: no active scenery layer");
            continue;
        }
        let layers: Vec<(&Image, i64)> = order.iter().map(|&l| if l == 2 { (&s2, -r.gap_y) } else { (&s3, 0) }).collect();
        let img = compose(&layers);
        img.png(&exit.join(format!("{n:02}.png")));
        s2.png(&exit.join("couches").join(format!("{n:02}_scroll2.png")));
        if !without_s3 {
            s3.png(&exit.join("couches").join(format!("{n:02}_scroll3.png")));
        }
        // without DDSOM_ROUTES, the already-read next scene is kept
        let old = std::fs::read_to_string(exit.join("infos").join(format!("{n:02}.tsv"))).ok().and_then(|t| t.trim().split('\t').nth(6).and_then(|s| s.parse().ok()));
        let next = if routes { next_scene(&mut m) } else { old };
        let order: Vec<String> = order.iter().map(|l| l.to_string()).collect();
        let info = format!("{n}\t{}\t{}\t{}x{}\t{}x{}\t{}\t{}\t{}\n", img.w, img.h, s2.w, s2.h, s3.w, s3.h, order.join(">"), next.map_or(String::from("-"), |s| s.to_string()), r.in_game as u8);
        std::fs::write(exit.join("infos").join(format!("{n:02}.tsv")), info).unwrap();
        println!("scene {n}: {}x{} (scroll 2 {}x{}, scroll 3 {}x{}, order {}) next {next:?}{}", img.w, img.h, s2.w, s2.h, s3.w, s3.h, order.join(">"), if r.in_game { "" } else { " WARNING: game phase not reached, doubtful palette" });
    }
}

// ---------------------------------------------------------------------------------------------
// Overview sheet and index
// ---------------------------------------------------------------------------------------------

/// 3x5 digits for the sheet's numbers.
const DIGITS: [[u8; 5]; 10] = [[7, 5, 5, 5, 7], [2, 6, 2, 2, 7], [7, 1, 7, 4, 7], [7, 1, 7, 1, 7], [5, 5, 7, 1, 1], [7, 4, 7, 1, 7], [7, 4, 7, 5, 7], [7, 1, 1, 1, 1], [7, 5, 7, 5, 7], [7, 5, 7, 1, 7]];

fn label(rgb: &mut [u8], width: usize, x0: usize, y0: usize, n: u32) {
    let text = n.to_string();
    let k = 4;
    // dark background under the number
    for y in y0 - 2..y0 + 5 * k + 2 {
        for x in x0 - 2..x0 + text.len() * 4 * k + 2 {
            rgb[(y * width + x) * 3..][..3].copy_from_slice(&[0, 0, 0]);
        }
    }
    for (i, c) in text.bytes().enumerate() {
        for (ry, line) in DIGITS[(c - b'0') as usize].iter().enumerate() {
            for rx in 0..3 {
                if line >> (2 - rx) & 1 != 0 {
                    for (dy, dx) in (0..k * k).map(|v| (v / k, v % k)) {
                        let (x, y) = (x0 + (i * 4 + rx) * k + dx, y0 + ry * k + dy);
                        rgb[(y * width + x) * 3..][..3].copy_from_slice(&[255, 255, 255]);
                    }
                }
            }
        }
    }
}

fn sheet(dir: &Path, doc: &Path) {
    const CELL: (usize, usize) = (320, 190);
    const COLUMNS: usize = 5;
    let views: Vec<u32> = SCENES.filter(|n| dir.join(format!("{n:02}.png")).exists()).collect();
    let lines = views.len().div_ceil(COLUMNS);
    let (lw, lh) = (COLUMNS * (CELL.0 + 8) + 8, lines_height(lines, CELL.1));
    let mut rgb = vec![0u8; lw * lh * 3];
    for p in rgb.chunks_mut(3) {
        p.copy_from_slice(&[12, 10, 20]);
    }
    for (i, &n) in views.iter().enumerate() {
        let (w, h, src) = read_png(&dir.join(format!("{n:02}.png")));
        // averaging reduction, at the scale that makes the image fit in the cell
        let scale = (w as f64 / CELL.0 as f64).max(h as f64 / CELL.1 as f64).max(1.0);
        let (tw, th) = (((w as f64 / scale) as usize).max(1), ((h as f64 / scale) as usize).max(1));
        let (ox, oy) = (8 + (i % COLUMNS) * (CELL.0 + 8), 8 + (i / COLUMNS) * (CELL.1 + 8));
        for ty in 0..th {
            for tx in 0..tw {
                let (x0, x1) = ((tx as f64 * scale) as usize, (((tx + 1) as f64 * scale) as usize).clamp(1, w).max((tx as f64 * scale) as usize + 1).min(w));
                let (y0, y1) = ((ty as f64 * scale) as usize, (((ty + 1) as f64 * scale) as usize).max((ty as f64 * scale) as usize + 1).min(h));
                let mut acc = [0u32; 3];
                for y in y0..y1 {
                    for x in x0..x1 {
                        for c in 0..3 {
                            acc[c] += src[(y * w + x) * 3 + c] as u32;
                        }
                    }
                }
                let nb = ((y1 - y0) * (x1 - x0)).max(1) as u32;
                for c in 0..3 {
                    rgb[((oy + ty) * lw + ox + tx) * 3 + c] = (acc[c] / nb) as u8;
                }
            }
        }
        label(&mut rgb, lw, ox + 4, oy + 4, n);
    }
    write_png(&dir.join("planche.png"), lw, lh, &rgb);
    index(dir, doc, &views);
    println!("planche : {} vignettes, {lw}x{lh}", views.len());
}

fn lines_height(lines: usize, cell: usize) -> usize {
    lines * (cell + 8) + 8
}

/// Boss connus (`docs/coverage.md`, `docs/atlas.md`).
fn boss(n: u32) -> &'static str {
    match n {
        24 => "Manticore",
        32 => "Black Dragon",
        40 => "Salamandre (givre)",
        42 => "Salamandre (flamme)",
        54 => "Tel'Arin",
        _ => "",
    }
}

fn index(dir: &Path, doc: &Path, views: &[u32]) {
    let mut md = String::new();
    md += "# Niveaux : images complètes des décors\n\n";
    md += "Une image par scène jouable (1 à 68) et par boutique (69 à 82) : tout le décor de la scène (scroll 2 et\n";
    md += "scroll 3 superposés dans l'ordre du matériel), **sans sprites ni interface**. `planche.png` en donne un\n";
    md += "aperçu (vignette et numéro de chaque scène).\n\n";
    md += "Les images sont tirées des ROMs (œuvre protégée) : elles ne sont **jamais versionnées**. Elles sont écrites\n";
    md += "dans `assets/niveaux/` (dossier ignoré par git) ; les liens ci-dessous ne fonctionnent qu'après les avoir\n";
    md += "générées.\n\n";
    md += "## Régénérer\n\n```sh\n";
    md += "# depuis port/ ; les ROMs sont lues dans DDSOM_ROMS (dossier des fichiers dd2.*), l'EEPROM en deuxième argument\n";
    md += "export DDSOM_ROMS=$PWD/../rom\n";
    md += "cargo run --release -p ddsom-game --example level_maps -- cartes ../rom ../testdata/eeprom_ddsom.bin ../assets/niveaux\n";
    md += "cargo run --release -p ddsom-game --example level_maps -- planche ../assets/niveaux ../docs/niveaux.md\n";
    md += "```\n\n";
    md += "La première commande démarre le jeu (pièce, Start, personnage), saute dans chaque scène et relève la\n";
    md += "palette ; elle prend une dizaine de minutes pour les 82 scènes (numéros de scènes en arguments pour n'en\n";
    md += "refaire que quelques-unes ; `DDSOM_ROUTES=1` relève aussi la scène suivante en forçant la sortie de scène,\n";
    md += "`DDSOM_REF=1` enregistre une capture du jeu `ref_NN.png` pour comparaison). La seconde recrée la planche\n";
    md += "et réécrit ce fichier.\n\n";
    md += "Les tables elles-mêmes (blocs, bandes, tables de bandes des scènes) sont extraites sans perte dans\n";
    md += "`assets/levels.json` par l'outil `extract` : voir [assets.md](assets.md).\n\n";
    md += "## Méthode\n\n";
    md += "Le décor n'est pas reconstitué par des captures d'écran : il est relu dans la ROM, comme le font les\n";
    md += "caméras du jeu (`port/ddsom-game/src/scroll.rs`, `docs/video.md` §B1).\n\n";
    md += "* Chaque couche est une grille de *bandes* de 256x256 pixels : 16x16 cases de 16 pixels (scroll 2, carte\n";
    md += "  `0x1f9ba0`, banques `0x320f6`) ou 8x8 cases de 32 pixels (scroll 3, carte `0x2459c0`, banques `0x33a9c`).\n";
    md += "* Pour chaque scène, une table du code (`0x2ff94 + 4 s` et `0x32d7c + 4 s`) donne l'adresse d'une table de\n";
    md += "  numéros de bandes : un mot `W` (pas de ligne de la caméra), `K` lignes de `W` mots au-dessus de la ligne\n";
    md += "  d'ordonnée 0 que désigne le pointeur (ordonnées croissantes = adresses décroissantes), puis quelques\n";
    md += "  lignes sous 0. Toutes les tables se suivent en ROM ; le pointeur de chacune fixe le début de la suivante,\n";
    md += "  ce qui donne les dimensions exactes de chaque carte. Dans le scroll 2 le dernier mot de ligne est une\n";
    md += "  sentinelle (`0xfe`).\n";
    md += "* L'outil lit ces tables, compose toutes les cases (même décodage que `cps2-gfx`), les peint avec la palette\n";
    md += "  de la scène (relevée en jeu après l'ouverture) et superpose les deux couches dans l'ordre de CPS-B.\n";
    md += "* Contrôle : les bandes que la mémoire vidéo contient au début de la scène sont identiques à celles de la\n";
    md += "  table, et les images correspondent aux captures du jeu (`DDSOM_REF=1`).\n\n";
    md += "## Limites\n\n";
    md += "* Pas de scroll 1 (cases de 8 pixels : texte, interface, quelques avant-plans), ni de sprites : décors\n";
    md += "  animés, objets et ennemis sont absents. Les dessins que le jeu écrit dynamiquement dans les couches\n";
    md += "  (scènes à script particulier) ne sont pas reconstitués.\n";
    md += "* Palette : celle du début de la scène, après l'ouverture. Les fondus, éclairs et changements de palette\n";
    md += "  en cours de scène ne sont pas rendus.\n";
    md += "* Parallaxe : quand une couche est plus étroite que l'autre (fond lointain), l'image est assemblée par\n";
    md += "  tranches d'un écran (384 pixels), chacune telle que le joueur la voit à cette position, avec le\n";
    md += "  rapport de défilement déduit des largeurs ; des raccords sont visibles entre tranches. Les deux couches\n";
    md += "  sont alignées par le bas ; les couches seules sont dans `couches/NN_scroll2.png` et `NN_scroll3.png`.\n";
    md += "* Une soixantaine de scènes n'ont pas de carte propre pour le scroll 3 (leur table est celle, par défaut, de\n";
    md += "  la scène 1) : cette couche est alors un reste de mémoire masqué par le scroll 2, et elle est omise.\n";
    md += "* Les écarts verticaux entre couches sont ceux des caméras au début de la scène ; si une caméra se déplace\n";
    md += "  autrement pendant la scène (scripts de mode), l'alignement relatif n'est exact qu'au départ.\n";
    md += "* Scènes 38, 65 et 68 : la phase de jeu n'a pas été atteinte par le saut direct (palette relevée au\n";
    md += "  mieux) ; leurs couleurs sont à vérifier.\n";
    md += "* Les bandes de remplissage des tables (`0xfc`, `0xfd`) ne sont pas dessinées (jamais à l'écran).\n";
    md += "* Noms de lieux : non extraits (les cartons de titre sont des graphismes, pas du texte). Les bosses ne sont\n";
    md += "  donnés que lorsque la documentation du dépôt les identifie.\n\n";
    md += "## Index\n\n";
    md += "![planche](../assets/niveaux/planche.png)\n\n";
    md += "| Scène | Image | Taille (px) | Scroll 2 | Scroll 3 | Ordre | Scène suivante | Boss |\n|---|---|---|---|---|---|---|---|\n";
    for &n in views {
        let info = std::fs::read_to_string(dir.join("infos").join(format!("{n:02}.tsv"))).unwrap_or_default();
        let c: Vec<&str> = info.trim().split('\t').collect();
        let get = |i: usize| c.get(i).copied().unwrap_or("?");
        let name = if n >= 69 { format!("{n} (boutique)") } else { n.to_string() };
        md += &format!("| {name} | [{n:02}.png](../assets/niveaux/{n:02}.png) | {}x{} | {} | {} | {} | {} | {} |\n", get(1), get(2), get(3), get(4), get(5), get(6), boss(n));
    }
    md += "\nOrdre : couches de l'arrière vers l'avant. « Scène suivante » : scène atteinte en forçant la sortie de\n";
    md += "scène (`-` : non relevée ; la sortie dépend parfois du jeu, voir `docs/scenes.md` §5).\n";
    std::fs::write(doc, md).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("cartes") if a.len() >= 5 => {
            let scenes: Vec<u32> = if a.len() > 5 { a[5..].iter().filter_map(|s| s.parse().ok()).collect() } else { SCENES.collect() };
            cards(Path::new(&a[2]), Path::new(&a[3]), Path::new(&a[4]), &scenes);
        }
        Some("planche") if a.len() >= 4 => sheet(Path::new(&a[2]), Path::new(&a[3])),
        Some("tables") if a.len() >= 3 => {
            let m = cps2::Machine::load_ddsom(Path::new(&a[2]), None).unwrap();
            let rom = Rom { data: &m.bus.rom, prog: &m.bus.rom_dec, gfx: &m.gfx };
            for n in 0..115 {
                let dim = |c: &Layer| {
                    let g = c.grid(&rom, n);
                    format!("{}x{} bandes", g.first().map_or(0, |l| l.len()), g.len())
                };
                println!("scene {n}: scroll 2 {}, scroll 3 {}", dim(&SCROLL2), dim(&SCROLL3));
            }
        }
        _ => eprintln!("usage: level_maps cartes <roms> <eeprom> <folder> [scene...] | cartes planche <folder> <niveaux.md> | cartes tables <roms>"),
    }
}
