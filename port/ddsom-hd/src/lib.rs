//! High-definition packs (docs/pack-hd.md, docs/fin.md L22).
//!
//! A pack replaces the original graphics by larger images, without giving up the palettes: its images hold pens
//! (indices in a palette of 16 colours), and the colours are those of the frame being drawn ([`render::render_hd`]).
//! The pipeline:
//!
//! 1. **export** (`hd-pack export`, feature `tool`): each sprite frame of the game's frame table, assembled and
//!    coloured, as an RGBA PNG (`frames/NNNNN.png`), and the scenery of each level as one continuous image per layer
//!    (`levels/NN_scroll2.png`, `levels/NN_scroll3.png`); next to each image, its metadata ([`FrameMeta`],
//!    [`LevelMeta`]: anchor, palettes, tiles);
//! 2. retouch or upscale the images (any whole scale, the same for width and height);
//! 3. **index** ([`index_frame`], [`index_level`]; `hd-pack index`): each pixel of a retouched image goes back to the
//!    nearest colour of its original palette, as a pen: indexed PNG plus metadata, in the pack folder `hd/`;
//! 4. **load** ([`load`]): the facade reads `hd/` and draws with [`render::render_hd`].
//!
//! The pack is never versioned: it is derived from Capcom's graphics.

pub mod render;
#[cfg(feature = "tool")]
pub mod export;

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub use render::{render_hd, tile_key, HdFrame, HdPack};

/// RGB colour.
pub type Rgb = [u8; 3];
/// The 16 colours of a palette (pen 15 is transparent).
pub type Colors = [Rgb; 16];

/// Pen of a transparent pixel.
pub const CLEAR: u8 = 15;

/// Metadata of a sprite frame image (`frames/NNNNN.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FrameMeta {
    /// Frame number (`sprites/frames.json`: index in the table, then the frames outside it).
    pub frame: u32,
    /// Pixels of the image per original pixel (1 when exported).
    pub scale: usize,
    /// Size of the image in original pixels.
    pub size: [usize; 2],
    /// Position of the frame's anchor in the image, in original pixels.
    pub anchor: [i32; 2],
    /// The palettes of the frame: one per palette of its pieces, in the order of the pieces.
    pub slots: Vec<Slot>,
}

/// A palette of a frame: the piece that carries it (the game may change the palette of each piece) and its colours
/// when exported.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Slot {
    pub piece: u16,
    pub colors: Colors,
}

/// Metadata of a level image (`levels/NN_scrollL.json`): a grid of layer tiles.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LevelMeta {
    pub scene: u32,
    /// Layer: 0 to 2 (scroll 1 to 3).
    pub layer: usize,
    /// Side of the tiles in original pixels (8, 16, 32).
    pub tile: usize,
    pub scale: usize,
    /// Grid size in tiles: columns, rows.
    pub size: [usize; 2],
    /// The tiles, row after row: graphics code and attributes (bits 0-4 palette, 5 horizontal mirror, 6 vertical
    /// mirror), or nothing.
    pub cells: Vec<Option<[u16; 2]>>,
    /// Colours of the 32 palettes of the layer when exported (a tile takes palette `attributes & 0x1f`).
    pub palettes: Vec<Colors>,
}

impl LevelMeta {
    pub fn cell(&self, column: usize, row: usize) -> Option<[u16; 2]> {
        self.cells[row * self.size[0] + column]
    }
}

/// An image in memory: RGBA or one byte per pixel.
pub struct Image<P> {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<P>,
}

/// Scale of an image of `width` x `height` pixels whose original is `size`: a whole number, the same on both sides.
pub fn scale_of(width: usize, height: usize, size: [usize; 2]) -> Result<usize, String> {
    let k = width / size[0].max(1);
    if k == 0 || width != k * size[0] || height != k * size[1] {
        return Err(format!("{width} x {height} is not a whole multiple of {} x {}", size[0], size[1]));
    }
    Ok(k)
}

/// Nearest colour among `candidates` (pen, colour) in RGB distance; between equally near colours (a palette may hold
/// the same colour twice), the pen `original` if it is one of them, since another palette may tell them apart.
fn nearest(c: Rgb, candidates: &[(u8, Rgb)], original: Option<u8>) -> u8 {
    let d = |p: &Rgb| (0..3).map(|i| (c[i] as i32 - p[i] as i32).pow(2)).sum::<i32>();
    let Some(best) = candidates.iter().map(|(_, p)| d(p)).min() else { return CLEAR };
    let near = |&&(pen, p): &&(u8, Rgb)| d(&p) == best && Some(pen) == original;
    candidates.iter().find(near).or_else(|| candidates.iter().find(|(_, p)| d(p) == best)).map_or(CLEAR, |c| c.0)
}

/// The pen of the original image under pixel `i` of an image `k` times larger.
fn under(original: Option<&Image<u8>>, width: usize, k: usize, i: usize) -> Option<u8> {
    original.map(|o| o.pixels[(i / width / k) * o.width + i % width / k])
}

/// Is a pixel of a retouched image transparent?
fn transparent(p: [u8; 4]) -> bool {
    p[3] < 128
}

/// A retouched frame image back to pens: each pixel becomes `16 * slot + pen`, the nearest colour of all the slots'
/// palettes (pen 15 excluded), or pen 15 if it is transparent. `original`: the exported pens, which settle the ties.
/// Returns the scale.
pub fn index_frame(image: &Image<[u8; 4]>, meta: &FrameMeta, original: Option<&Image<u8>>) -> Result<(Image<u8>, usize), String> {
    let k = scale_of(image.width, image.height, meta.size)?;
    let candidates: Vec<(u8, Rgb)> = (meta.slots.iter().enumerate())
        .flat_map(|(s, slot)| (0..15).map(move |pen| (16 * s as u8 + pen, slot.colors[pen as usize])))
        .collect();
    let pixels = (image.pixels.iter().enumerate()).map(|(i, &p)| match transparent(p) {
        true => CLEAR,
        false => nearest([p[0], p[1], p[2]], &candidates, under(original, image.width, k, i)),
    });
    Ok((Image { width: image.width, height: image.height, pixels: pixels.collect() }, k))
}

/// A retouched level image back to pens: each pixel becomes the pen of the nearest colour of the palette of its tile
/// (pen 15 excluded), or pen 15 if it is transparent or outside every tile. `original`: the exported pens, which
/// settle the ties. Returns the scale.
pub fn index_level(image: &Image<[u8; 4]>, meta: &LevelMeta, original: Option<&Image<u8>>) -> Result<(Image<u8>, usize), String> {
    let size = [meta.size[0] * meta.tile, meta.size[1] * meta.tile];
    let k = scale_of(image.width, image.height, size)?;
    let side = meta.tile * k;
    let candidates: Vec<Vec<(u8, Rgb)>> =
        meta.palettes.iter().map(|colors| (0..15).map(|pen| (pen, colors[pen as usize])).collect()).collect();
    let mut pixels = vec![CLEAR; image.width * image.height];
    for (i, (out, &p)) in pixels.iter_mut().zip(&image.pixels).enumerate() {
        let (x, y) = (i % image.width, i / image.width);
        if let (Some([_, attr]), false) = (meta.cell(x / side, y / side), transparent(p)) {
            *out = nearest([p[0], p[1], p[2]], &candidates[(attr & 0x1f) as usize], under(original, image.width, k, i));
        }
    }
    Ok((Image { width: image.width, height: image.height, pixels }, k))
}

/// Reads a PNG as RGBA (any colour type of 8 bits or less).
pub fn read_rgba(path: &Path) -> io::Result<Image<[u8; 4]>> {
    let mut decoder = png::Decoder::new(std::fs::File::open(path)?);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width as usize, info.height as usize);
    let px = |c: &[u8]| -> [u8; 4] {
        match info.color_type {
            png::ColorType::Grayscale => [c[0], c[0], c[0], 255],
            png::ColorType::GrayscaleAlpha => [c[0], c[0], c[0], c[1]],
            png::ColorType::Rgb => [c[0], c[1], c[2], 255],
            _ => [c[0], c[1], c[2], c[3]],
        }
    };
    let n = info.color_type.samples();
    let pixels = buf[..w * h * n].chunks(n).map(px).collect();
    Ok(Image { width: w, height: h, pixels })
}

/// Writes an RGBA PNG.
pub fn write_rgba(path: &Path, image: &Image<[u8; 4]>) -> io::Result<()> {
    let mut e = png::Encoder::new(std::fs::File::create(path)?, image.width as u32, image.height as u32);
    e.set_color(png::ColorType::Rgba);
    e.write_header()?.write_image_data(image.pixels.as_flattened())?;
    Ok(())
}

/// Writes an 8-bit indexed PNG; `colors`: the colour of each value (missing ones: grey), the values with pen 15
/// transparent.
pub fn write_indexed(path: &Path, image: &Image<u8>, colors: &[Rgb]) -> io::Result<()> {
    let mut e = png::Encoder::new(std::fs::File::create(path)?, image.width as u32, image.height as u32);
    e.set_color(png::ColorType::Indexed);
    e.set_depth(png::BitDepth::Eight);
    let plte: Vec<u8> = (0..256).flat_map(|v| colors.get(v).copied().unwrap_or([(v % 16 * 17) as u8; 3])).collect();
    e.set_palette(plte);
    e.set_trns((0..256).map(|v| if v % 16 == CLEAR as usize { 0 } else { 255 }).collect::<Vec<u8>>());
    e.write_header()?.write_image_data(&image.pixels)?;
    Ok(())
}

/// Reads an 8-bit indexed (or grey) PNG: the raw values.
pub fn read_indexed(path: &Path) -> io::Result<Image<u8>> {
    let mut decoder = png::Decoder::new(std::fs::File::open(path)?);
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    if info.bit_depth != png::BitDepth::Eight || info.color_type.samples() != 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "8-bit indexed PNG expected (`hd-pack index`)"));
    }
    let (w, h) = (info.width as usize, info.height as usize);
    buf.truncate(w * h);
    Ok(Image { width: w, height: h, pixels: buf })
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    serde_json::from_slice(&std::fs::read(path)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    std::fs::write(path, serde_json::to_vec(value).map_err(io::Error::other)?)
}

/// The images of a folder (`frames/` or `levels/`): base names of the PNG that have their JSON, sorted.
pub fn images_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".png").map(String::from))
        .filter(|n| dir.join(format!("{n}.json")).is_file())
        .collect();
    names.sort();
    names
}

/// `pixels` (`width` x `height`) enlarged `k` times, each pixel repeated `k` x `k` times.
fn enlarge<P: Copy>(pixels: &[P], width: usize, k: usize) -> Vec<P> {
    if k == 1 {
        return pixels.to_vec();
    }
    pixels.chunks(width).flat_map(|row| {
        let wide: Vec<P> = row.iter().flat_map(|&p| std::iter::repeat_n(p, k)).collect();
        std::iter::repeat_n(wide, k).flatten()
    }).collect()
}

/// An image enlarged `k` times (nearest pixel): the trivial upscale, to test a pack.
pub fn upscale<P: Copy>(image: &Image<P>, k: usize) -> Image<P> {
    Image { width: image.width * k, height: image.height * k, pixels: enlarge(&image.pixels, image.width, k) }
}

/// An image enlarged twice by Scale2x (EPX), the simplest pixel-art upscaler: an edge between two equal neighbours
/// becomes a diagonal; no new colour appears. To test a pack whose images visibly differ from the original.
pub fn scale2x<P: Copy + PartialEq>(image: &Image<P>) -> Image<P> {
    let (w, h) = (image.width, image.height);
    let at = |x: isize, y: isize| image.pixels[y.clamp(0, h as isize - 1) as usize * w + x.clamp(0, w as isize - 1) as usize];
    let mut pixels = vec![image.pixels[0]; 4 * w * h];
    for y in 0..h as isize {
        for x in 0..w as isize {
            let (e, b, d, f, hh) = (at(x, y), at(x, y - 1), at(x - 1, y), at(x + 1, y), at(x, y + 1));
            let quad = if b != hh && d != f {
                [if d == b { d } else { e }, if b == f { f } else { e }, if d == hh { d } else { e }, if hh == f { f } else { e }]
            } else {
                [e; 4]
            };
            let (x, y) = (2 * x as usize, 2 * y as usize);
            for (k, p) in quad.into_iter().enumerate() {
                pixels[(y + k / 2) * 2 * w + x + k % 2] = p;
            }
        }
    }
    Image { width: 2 * w, height: 2 * h, pixels }
}

/// Loads the pack of folder `dir` (`frames/`, `levels/`: indexed PNG and their metadata). All the images are brought
/// to the largest scale found (an image whose scale does not divide it is skipped); a tile met in several level images
/// keeps its first image. Problems are reported on the standard error, the images concerned skipped.
pub fn load(dir: &Path) -> HdPack {
    let frames = read_all(&dir.join("frames"), |m: &FrameMeta| m.size);
    let levels = read_all(&dir.join("levels"), |m: &LevelMeta| {
        [m.size[0] * m.tile, m.size[1] * m.tile]
    });
    let scale = frames.iter().map(|f| f.2).chain(levels.iter().map(|l| l.2)).max().unwrap_or(1);
    let mut pack = HdPack { scale, ..Default::default() };
    let fits = |k: usize, what: &str| {
        let ok = scale % k == 0;
        if !ok {
            eprintln!("HD pack: {what} at scale {k} skipped (the pack is at scale {scale})");
        }
        ok
    };
    for (meta, image, k) in frames {
        if !fits(k, &format!("frame {}", meta.frame)) || meta.slots.is_empty() {
            continue;
        }
        let image = upscale(&image, scale / k);
        let slots = meta.slots.iter().map(|s| s.piece).collect();
        let anchor = (meta.anchor[0], meta.anchor[1]);
        pack.frames.insert(meta.frame, HdFrame { width: image.width, height: image.height, pixels: image.pixels, anchor, slots });
    }
    for (meta, image, k) in levels {
        if !fits(k, &format!("level {} layer {}", meta.scene, meta.layer + 1)) || meta.layer > 2 {
            continue;
        }
        let side = meta.tile * k;
        for row in 0..meta.size[1] {
            for column in 0..meta.size[0] {
                let Some([code, attr]) = meta.cell(column, row) else { continue };
                let key = tile_key(meta.layer, code, column);
                if pack.tiles[meta.layer].contains_key(&key) {
                    continue;
                }
                // the tile as stored: unmirrored
                let mut tile = vec![CLEAR; side * side];
                for (i, p) in tile.iter_mut().enumerate() {
                    let (x, y) = (i % side, i / side);
                    let x = if attr & 0x20 != 0 { side - 1 - x } else { x };
                    let y = if attr & 0x40 != 0 { side - 1 - y } else { y };
                    *p = image.pixels[(row * side + y) * image.width + column * side + x] & 15;
                }
                pack.tiles[meta.layer].insert(key, enlarge(&tile, side, scale / k));
            }
        }
    }
    pack
}

/// The images of `dir` with their metadata and their scale (checked against the original size, `original`).
fn read_all<M: serde::de::DeserializeOwned>(dir: &Path, original: impl Fn(&M) -> [usize; 2]) -> Vec<(M, Image<u8>, usize)> {
    let mut all = Vec::new();
    for name in images_in(dir) {
        let read = || -> Result<(M, Image<u8>, usize), String> {
            let meta: M = read_json(&dir.join(format!("{name}.json"))).map_err(|e| e.to_string())?;
            let image = read_indexed(&dir.join(format!("{name}.png"))).map_err(|e| e.to_string())?;
            let k = scale_of(image.width, image.height, original(&meta))?;
            Ok((meta, image, k))
        };
        match read() {
            Ok(m) => all.push(m),
            Err(e) => eprintln!("HD pack: {}/{name}: {e}", dir.display()),
        }
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette(seed: u8) -> Colors {
        std::array::from_fn(|i| [seed.wrapping_mul(i as u8), i as u8 * 16, 255 - i as u8])
    }

    #[test]
    fn an_enlarged_frame_goes_back_to_its_pens() {
        let mut colors = [palette(3), palette(7)];
        // pen 4 of slot 0 has the colour of pen 2: the original pens settle it
        colors[0][4] = colors[0][2];
        let meta = FrameMeta {
            frame: 0,
            scale: 1,
            size: [3, 2],
            anchor: [0, 0],
            slots: colors.iter().enumerate().map(|(i, &colors)| Slot { piece: i as u16, colors }).collect(),
        };
        let pens = Image { width: 3, height: 2, pixels: vec![0x02, 0x04, CLEAR, 0x11, 0x1e, 0x00] };
        let rgba = Image {
            width: 3,
            height: 2,
            pixels: pens.pixels.iter().map(|&v| match v % 16 {
                15 => [0; 4],
                p => {
                    let [r, g, b] = colors[v as usize / 16][p as usize];
                    [r, g, b, 255]
                }
            }).collect(),
        };
        let (indexed, k) = index_frame(&upscale(&rgba, 3), &meta, Some(&pens)).unwrap();
        assert_eq!(k, 3);
        assert_eq!(indexed.pixels, upscale(&pens, 3).pixels);
        assert!(index_frame(&upscale(&rgba, 3), &FrameMeta { size: [4, 2], ..meta }, None).is_err());
    }

    #[test]
    fn scale2x_rounds_a_diagonal() {
        let image = Image { width: 2, height: 2, pixels: vec![1, 0, 1, 1] };
        let big = scale2x(&image);
        // in the quad of the top right pixel, the corner between its two equal neighbours (left, below) takes their
        // colour, the others keep its own
        assert_eq!([big.pixels[2], big.pixels[3], big.pixels[6], big.pixels[7]], [0, 0, 1, 0]);
    }
}
