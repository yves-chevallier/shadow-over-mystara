//! `hd-pack`: makes a high-definition pack (docs/pack-hd.md).
//!
//!   hd-pack export  <game> <out> [--all] [--run <scenario> <frames>]...
//!       frames/NNNNN.png (RGBA) and levels/NN_scrollL.png of the original graphics, with their metadata (.json) and
//!       their pens (.pens.png: `index` settles with them the colours that a palette holds twice).
//!       The frames are those shown while playing the scenarios, with the colours they had most often on screen;
//!       `--all` adds every other frame of the table, in grey.
//!   hd-pack upscale <dir> <out> <k> [--scale2x] [name...]
//!       Trivial upscale (nearest pixel, `k` times; with `--scale2x`, Scale2x applied until `k`, a power of 2) of the
//!       images of `dir` (all, or those named, e.g. `frames/01234`, `levels/03_scroll2`), metadata copied: to test a
//!       pack.
//!   hd-pack index   <export> <retouched> <hd>
//!       Retouched images (in `frames/`, `levels/` of <retouched>, any whole scale) back to the pens of their original
//!       palettes: indexed PNG and metadata in <hd>, the folder the game loads.
//!   hd-pack check   <game> <hd> <scenario> <frames> [--png <dir> <frame>...]
//!       Renders the scenario with the pack and without it; prints how many pixels differ from the original enlarged,
//!       and writes the listed frames (both renderings) as PNG.
//!
//! <game>: the ROM folder, or `--assets <dir>`. `DDSOM_EEPROM=<file>`: starting EEPROM (that of the scenarios).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cps2_gfx::{SCREEN_H, SCREEN_W};
use ddsom_hd::export::{self, frame_art, frame_count, GREY};
use ddsom_hd::{Colors, FrameMeta, Image, Slot};
use ddsom_game::core::scenario::Scenario;
use ddsom_game::{Game, Inputs};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("export") => export_cmd(&args[1..]),
        Some("upscale") => upscale_cmd(&args[1..]),
        Some("index") => index_cmd(&args[1..]),
        Some("check") => check_cmd(&args[1..]),
        _ => Err("usage: hd-pack export|upscale|index|check ... (see the header of hd-pack.rs or docs/pack-hd.md)".into()),
    };
    if let Err(e) = result {
        eprintln!("hd-pack: {e}");
        std::process::exit(1);
    }
}

type Result<T = ()> = std::result::Result<T, String>;

fn io<T>(r: std::io::Result<T>, what: &Path) -> Result<T> {
    r.map_err(|e| format!("{}: {e}", what.display()))
}

/// The game contents, from `<roms>` or `--assets <dir>` at the start of `args`; the rest of the arguments.
fn game_from(args: &[String]) -> Result<(cps2::GameContents, &[String])> {
    match args {
        [flag, dir, rest @ ..] if flag == "--assets" => Ok((io(extract::install_game_assets(dir.as_ref()), dir.as_ref())?, rest)),
        [roms, rest @ ..] => Ok((io(extract::install_game(roms.as_ref()), roms.as_ref())?, rest)),
        [] => Err("ROM folder or --assets <dir> expected".into()),
    }
}

/// Plays `frames` frames of `scenario` on a fresh game, calling `each` after each step.
fn play(contents: &cps2::GameContents, scenario: &Path, frames: usize, mut each: impl FnMut(usize, &Game)) -> Result {
    let eeprom = std::env::var_os("DDSOM_EEPROM").and_then(|f| std::fs::read(f).ok());
    let script = io(Scenario::load(scenario), scenario)?;
    let mut game = Game::new(contents.clone(), eeprom.as_deref());
    let mut inputs = Inputs::default();
    for f in 0..frames {
        script.apply_pokes(f, game.machine_mut());
        script.apply_inputs(f, &mut inputs);
        game.step(inputs);
        each(f, &game);
    }
    Ok(())
}

fn export_cmd(args: &[String]) -> Result {
    let (contents, rest) = game_from(args)?;
    let Some((out, mut rest)) = rest.split_first() else { return Err("output folder expected".into()) };
    let out = PathBuf::from(out);
    let data = ddsom_data::data();
    let mut all = false;
    // frame number -> how many times each set of colours of its slots was on screen
    let mut seen: HashMap<u32, HashMap<Vec<Colors>, u32>> = HashMap::new();
    while let Some((flag, more)) = rest.split_first() {
        match (flag.as_str(), more) {
            ("--all", _) => (all, rest) = (true, more),
            ("--run", [scenario, frames, more @ ..]) => {
                let frames = frames.parse().map_err(|_| format!("bad frame count {frames}"))?;
                play(&contents, scenario.as_ref(), frames, |_, game| {
                    let list = game.frame();
                    for f in list.shown_frames() {
                        let (_, slots) = export::slots(&data.frames, f.frame);
                        let colors: Vec<Colors> = slots
                            .iter()
                            .map(|&(piece, _)| {
                                let palette = list.sprites.get(f.first as usize + piece as usize).palette as usize;
                                std::array::from_fn(|i| cps2_gfx::draw::rgb(list.palette.color(16 * palette + i)))
                            })
                            .collect();
                        *seen.entry(f.frame).or_default().entry(colors).or_default() += 1;
                    }
                })?;
                rest = more;
            }
            _ => return Err(format!("unknown argument {flag}")),
        }
    }
    let frames_dir = out.join("frames");
    io(std::fs::create_dir_all(&frames_dir), &frames_dir)?;
    let mut written = 0;
    for n in 0..frame_count(&data.frames) {
        // the colours most often on screen (not those of a flash or a fade)
        let colors = match seen.get(&n).and_then(|c| c.iter().max_by_key(|(c, &n)| (n, *c))) {
            Some((c, _)) => c.clone(),
            None if all => vec![GREY; export::slots(&data.frames, n).1.len()],
            None => continue,
        };
        let Some(art) = frame_art(&data.frames, n, &contents.graphics) else { continue };
        let meta = FrameMeta {
            frame: n,
            scale: 1,
            size: [art.image.width, art.image.height],
            anchor: art.anchor,
            slots: art.slots.iter().zip(&colors).map(|(&(piece, _), &colors)| Slot { piece, colors }).collect(),
        };
        let base = frames_dir.join(format!("{n:05}"));
        io(ddsom_hd::write_rgba(&base.with_extension("png"), &export::colour(&art.image, &colors)), &base)?;
        io(ddsom_hd::write_indexed(&base.with_extension("pens.png"), &art.image, &[]), &base)?;
        io(ddsom_hd::write_json(&base.with_extension("json"), &meta), &base)?;
        written += 1;
    }
    println!("{written} frames ({} seen in play) in {}", seen.len(), frames_dir.display());

    let levels_dir = out.join("levels");
    io(std::fs::create_dir_all(&levels_dir), &levels_dir)?;
    let mut tables = HashMap::new();
    for li in [1, 2] {
        let layer = &data.levels.layers[li - 1];
        for (scene, &table) in layer.scenes.iter().enumerate() {
            // a map shared by several scenes is exported once, under the first one
            if tables.insert((li, table), scene).is_some() {
                continue;
            }
            let Some(meta) = export::level(data, scene, li) else { continue };
            let base = levels_dir.join(format!("{scene:02}_scroll{}", li + 1));
            let (image, pens) = export::level_image(&meta, &contents.graphics);
            io(ddsom_hd::write_rgba(&base.with_extension("png"), &image), &base)?;
            io(ddsom_hd::write_indexed(&base.with_extension("pens.png"), &pens, &[]), &base)?;
            io(ddsom_hd::write_json(&base.with_extension("json"), &meta), &base)?;
        }
    }
    println!("{} level images in {}", tables.len(), levels_dir.display());
    Ok(())
}

fn upscale_cmd(args: &[String]) -> Result {
    let [dir, out, k, names @ ..] = args else { return Err("upscale <dir> <out> <k> [--scale2x] [name...]".into()) };
    let k: usize = k.parse().map_err(|_| format!("bad scale {k}"))?;
    let (epx, names) = match names {
        [flag, rest @ ..] if flag == "--scale2x" => (true, rest),
        _ => (false, names),
    };
    if epx && !k.is_power_of_two() {
        return Err(format!("--scale2x: {k} is not a power of 2"));
    }
    let enlarge = |mut image: Image<[u8; 4]>| {
        if !epx {
            return ddsom_hd::upscale(&image, k);
        }
        for _ in 0..k.trailing_zeros() {
            image = ddsom_hd::scale2x(&image);
        }
        image
    };
    let (dir, out) = (PathBuf::from(dir), PathBuf::from(out));
    let mut done = 0;
    for sub in ["frames", "levels"] {
        io(std::fs::create_dir_all(out.join(sub)), &out)?;
        for name in ddsom_hd::images_in(&dir.join(sub)) {
            if !names.is_empty() && !names.iter().any(|n| *n == format!("{sub}/{name}")) {
                continue;
            }
            let (src, dst) = (dir.join(sub).join(&name), out.join(sub).join(&name));
            let image = io(ddsom_hd::read_rgba(&src.with_extension("png")), &src)?;
            io(ddsom_hd::write_rgba(&dst.with_extension("png"), &enlarge(image)), &dst)?;
            io(std::fs::copy(src.with_extension("json"), dst.with_extension("json")), &src)?;
            done += 1;
        }
    }
    println!("{done} images enlarged {k} times in {}", out.display());
    Ok(())
}

fn index_cmd(args: &[String]) -> Result {
    let [export, retouched, hd] = args else { return Err("index <export> <retouched> <hd>".into()) };
    let (export, retouched, hd) = (PathBuf::from(export), PathBuf::from(retouched), PathBuf::from(hd));
    let mut done = 0;
    for sub in ["frames", "levels"] {
        io(std::fs::create_dir_all(hd.join(sub)), &hd)?;
        let dir = retouched.join(sub);
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".png").map(String::from))
            .filter(|n| !n.ends_with(".pens"))
            .collect();
        names.sort();
        for name in names {
            let meta_path = export.join(sub).join(format!("{name}.json"));
            let src = dir.join(format!("{name}.png"));
            let dst = hd.join(sub).join(&name);
            let image = io(ddsom_hd::read_rgba(&src), &src)?;
            let pens = export.join(sub).join(format!("{name}.pens.png"));
            let original = ddsom_hd::read_indexed(&pens).ok();
            if sub == "frames" {
                let mut meta: FrameMeta = io(ddsom_hd::read_json(&meta_path), &meta_path)?;
                let (indexed, k) = ddsom_hd::index_frame(&image, &meta, original.as_ref()).map_err(|e| format!("{}: {e}", src.display()))?;
                meta.scale = k;
                let colors: Vec<_> = meta.slots.iter().flat_map(|s| s.colors).collect();
                io(ddsom_hd::write_indexed(&dst.with_extension("png"), &indexed, &colors), &dst)?;
                io(ddsom_hd::write_json(&dst.with_extension("json"), &meta), &dst)?;
            } else {
                let mut meta: ddsom_hd::LevelMeta = io(ddsom_hd::read_json(&meta_path), &meta_path)?;
                let (indexed, k) = ddsom_hd::index_level(&image, &meta, original.as_ref()).map_err(|e| format!("{}: {e}", src.display()))?;
                meta.scale = k;
                io(ddsom_hd::write_indexed(&dst.with_extension("png"), &indexed, &[]), &dst)?;
                io(ddsom_hd::write_json(&dst.with_extension("json"), &meta), &dst)?;
            }
            done += 1;
        }
    }
    println!("{done} images indexed in {}", hd.display());
    Ok(())
}

fn check_cmd(args: &[String]) -> Result {
    let (contents, rest) = game_from(args)?;
    let [hd, scenario, frames, more @ ..] = rest else { return Err("check <game> <hd> <scenario> <frames>".into()) };
    let frames: usize = frames.parse().map_err(|_| format!("bad frame count {frames}"))?;
    let (png_dir, listed): (Option<PathBuf>, Vec<usize>) = match more {
        [flag, dir, list @ ..] if flag == "--png" => (Some(dir.into()), list.iter().filter_map(|n| n.parse().ok()).collect()),
        _ => (None, Vec::new()),
    };
    let pack = ddsom_hd::load(hd.as_ref());
    let s = pack.scale;
    let tiles: usize = pack.tiles.iter().map(HashMap::len).sum();
    println!("pack {hd}: scale {s}, {} frames, {tiles} tiles", pack.frames.len());
    let (mut differing, mut drawn_frames, mut images_with_hd) = (0u64, 0u64, 0u64);
    let mut time = std::time::Duration::ZERO;
    let mut worst = Vec::new();
    play(&contents, scenario.as_ref(), frames, |f, game| {
        let list = game.frame();
        let original = cps2_gfx::render(&list, &contents.graphics);
        let t0 = std::time::Instant::now();
        let hd_image = ddsom_hd::render_hd(&list, &contents.graphics, &pack);
        time += t0.elapsed();
        let enlarged = ddsom_hd::upscale(&Image { width: SCREEN_W, height: SCREEN_H, pixels: original }, s);
        let d = enlarged.pixels.iter().zip(&hd_image).filter(|(a, b)| a != b).count() as u64;
        differing += d;
        if d > 0 {
            worst.push((d, f));
        }
        let n = list.shown_frames().filter(|sf| pack.frames.contains_key(&sf.frame)).count() as u64;
        drawn_frames += n;
        images_with_hd += (n > 0) as u64;
        if let (Some(dir), true) = (&png_dir, listed.contains(&f)) {
            let rgba = |p: &[[u8; 3]]| p.iter().map(|c| [c[0], c[1], c[2], 255]).collect();
            let (w, h) = (SCREEN_W * s, SCREEN_H * s);
            let _ = std::fs::create_dir_all(dir);
            let _ = ddsom_hd::write_rgba(&dir.join(format!("{f:05}_hd.png")), &Image { width: w, height: h, pixels: rgba(&hd_image) });
            let _ = ddsom_hd::write_rgba(&dir.join(format!("{f:05}_original.png")), &Image { width: w, height: h, pixels: rgba(&enlarged.pixels) });
        }
    })?;
    println!(
        "{frames} frames: {drawn_frames} HD sprite frames drawn ({images_with_hd} images), {differing} pixels differ \
         from the original enlarged {s} times; HD rendering {:.2} ms per image",
        time.as_secs_f64() * 1000.0 / frames.max(1) as f64
    );
    worst.sort_unstable_by(|a, b| b.cmp(a));
    if !worst.is_empty() {
        let list: Vec<String> = worst.iter().take(8).map(|(d, f)| format!("{f} ({d})")).collect();
        println!("{} frames differ; the most: {}", worst.len(), list.join(", "));
    }
    Ok(())
}
