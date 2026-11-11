//! Dungeons & Dragons: Shadow over Mystara — port Rust jouable.
//!
//! ddsom [roms_dir] [--assets dir] : extracted ROMs (ddsom.zip + dl-1425.bin from qsound.zip), default
//! ../rom. With `--assets`, everything comes from the folder produced by `extract` (`--assets` alone, without a
//! folder, reads `../assets`).
//! The game runs without the 68000: only the routines rewritten in Rust (`docs/noyau.md`), on a machine without ROM
//! (`cps2::Machine::game`). It loads the typed game data (`ddsom_data::GameData`), the tile atlas, the data of the
//! Rust sound driver, the DSP tables and the sample bank: from the assets folder (PNG, WAV, JSON; no ROM file is
//! opened, docs/sans-rom.md), or extracted in memory from the ROMs.
//! Diagnostics (build with `--features original`): `DDSOM_ORIGINE=1` (original code at arcade speed), `DDSOM_INTERP=1`
//! (everything interpreted); these modes
//! read the ROMs.
//! Keys (as in MAME): 5 coin, 1 Start, arrows, Ctrl/Alt/Space/Shift = buttons 1-4
//! (or W X C V), 6 coin P2 / 2 Start P2, F2 test, P pause, Tab fast forward, F11 full screen, Esc quit;
//! rebindable in touches.cfg (`keys.rs`), Xbox gamepads included under Windows (`gamepad.rs`).
//! The EEPROM (settings, counters) is kept in nvram/ddsom.eeprom.
//! F5 saves the game to sauvegardes/rapide.txt, F9 resumes it (`save.rs`: input replay).
//! `DDSOM_ENREGISTRE=partie.txt` likewise writes the game on exit: `validate <roms> partie.txt.eeprom
//! partie.txt <frames>` replays it.

mod audio;
mod gamepad;
mod save;
mod keys;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cps2::{Board, Machine};
use minifb::{Scale, ScaleMode, Window, WindowOptions};

const FRAME_HZ: f64 = 8_000_000.0 / 512.0 / 262.0;

fn main() {
    let (romdir, assets) = arguments(std::env::args().skip(1).collect());
    let nvram = PathBuf::from("nvram/ddsom.eeprom");
    let mut keys = keys::Keys::load("touches.cfg".as_ref());
    let eeprom = std::fs::read(&nvram).ok();
    // the game: without the 68000, Rust routines only (it stops on an address that has none). Diagnostics:
    // DDSOM_ORIGINE=1: original code at arcade speed; DDSOM_INTERP=1: everything interpreted. (The hybrid mode,
    // DDSOM_HYBRIDE=1, is removed.)
    ddsom_game::refuse_hybrid();
    let original = std::env::var_os("DDSOM_ORIGINE").is_some() || std::env::var_os("DDSOM_INTERP").is_some();
    if original && !cfg!(feature = "original") {
        eprintln!("DDSOM_ORIGINE / DDSOM_INTERP: the original code needs the build with `--features original`.");
        std::process::exit(2);
    }
    let mut no_68k = !original;
    // no ROM: assets only, which requires the game without the 68000 (the 68000 program is not in the assets)
    let romdir = match (&assets, romdir) {
        (Some(_), None) if !no_68k && !PathBuf::from(DEFAULT_ROMS).is_dir() => {
            eprintln!("No ROM folder: playing without ROMs, from the assets alone (no 68000, no diagnostics).");
            no_68k = true;
            None
        }
        (Some(_), None) if no_68k => None,
        (_, r) => Some(r.unwrap_or_else(|| DEFAULT_ROMS.into())),
    };
    // the game data and what the game machine holds: from the assets folder, or extracted from the ROMs
    let loaded = match (&assets, &romdir) {
        (Some(dir), _) => extract::install_game_assets(dir),
        (None, Some(romdir)) => extract::install_game(romdir),
        (None, None) => unreachable!(),
    };
    // the original code (diagnostics) runs on a machine with the ROMs
    let originals = match (&romdir, no_68k) {
        (Some(romdir), false) => Some(cps2::Contents::from_roms(romdir, None, None)),
        _ => None,
    }
    .transpose();
    let (contents, originals) = match loaded.and_then(|c| Ok((c, originals?))) {
        Ok(c) => c,
        Err(e) => {
            match (&assets, &romdir) {
                (Some(dir), _) => eprintln!("Cannot load the assets from {}: {e}", dir.display()),
                (None, Some(romdir)) => eprintln!("Cannot load the ROMs from {}: {e}", romdir.display()),
                (None, None) => unreachable!(),
            }
            std::process::exit(1);
        }
    };
    // a fresh machine: at startup, and on each resume from a save (`save.rs`)
    match originals {
        // the game: no ROM, the Rust sound driver
        None => play(
            |eeprom| ddsom_game::install_no_68k(Machine::game(contents.clone(), eeprom)),
            &mut keys,
            eeprom.as_deref(),
            &nvram,
            no_68k,
        ),
        Some(c) => play(
            |eeprom| {
                let mut m = Machine::ddsom(c.clone(), eeprom);
                #[cfg(feature = "original")]
                if std::env::var_os("DDSOM_ORIGINE").is_some() {
                    ddsom_rc::install(&mut m);
                }
                m.sound.use_rust_driver();
                m
            },
            &mut keys,
            eeprom.as_deref(),
            &nvram,
            no_68k,
        ),
    }
}

/// Plays on the machines that `machine` builds (from the starting EEPROM): the game, or the original code.
fn play<B: Board>(
    machine: impl Fn(Option<&[u8]>) -> Machine<B>,
    keys: &mut keys::Keys,
    eeprom: Option<&[u8]>,
    nvram: &Path,
    no_68k: bool,
) {
    let mut m = machine(eeprom);

    let mut full_screen = false;
    let mut win = window(full_screen);

    let audio = audio::Audio::open(qsound::SAMPLE_RATE.round() as u32);
    if audio.is_none() {
        eprintln!("Sound unavailable (no audio output): the game runs without sound.");
    }

    let frame_time = Duration::from_secs_f64(1.0 / FRAME_HZ);
    let mut next = Instant::now();
    let mut paused = false;
    let mut game = save::Game::new(&m);
    let saved = |game: &save::Game, file: &Path| match game.saved(file) {
        Ok(()) => eprintln!("Game saved to {} (frame {}).", file.display(), game.images()),
        Err(e) => eprintln!("Cannot save to {}: {e}", file.display()),
    };
    let mut buf = vec![0u32; 384 * 224];

    while win.is_open() {
        let t = keys.lit(&win);
        if t.quit {
            break;
        }
        if t.full_screen {
            full_screen = !full_screen;
            win = window(full_screen);
        }
        if t.pause {
            paused = !paused;
        }
        if t.save {
            saved(&game, SAVE.as_ref());
        }
        if t.load {
            let start = Instant::now();
            match save::Game::load(SAVE.as_ref(), |ee| machine(Some(ee))) {
                Ok((new, p)) => {
                    eprintln!("Game resumed at frame {} ({:.1} s).", p.images(), start.elapsed().as_secs_f64());
                    (m, game) = (new, p);
                    next = Instant::now();
                }
                Err(e) => eprintln!("Reprise de {} impossible : {e}", SAVE),
            }
        }
        if paused {
            win.update();
            std::thread::sleep(Duration::from_millis(16));
            next = Instant::now();
            continue;
        }
        m.bus.hw_mut().inputs.in0 = t.in0;
        m.bus.hw_mut().inputs.in2 = t.in2;
        game.image(&m);
        let img = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())) {
            Ok(img) => img,
            // without the 68000: stop on an address with no Rust routine (diagnostic already printed by the panic)
            Err(e) if no_68k => {
                let m = e.downcast_ref::<String>().cloned().unwrap_or_default();
                eprintln!("The game without the 68000 stops: {}", m.trim_start_matches(ddsom_game::core::frame::STOP));
                break;
            }
            Err(e) => std::panic::resume_unwind(e),
        };
        for (d, p) in buf.iter_mut().zip(&img) {
            *d = (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32;
        }
        win.update_with_buffer(&buf, 384, 224).expect("display");

        let samples: Vec<i16> = m.sound.audio.drain(..).flatten().collect();
        let fast = t.fast;
        if let (Some(a), false) = (&audio, fast) {
            a.push(samples);
        }
        // pacing: 59.64 frames/s (except fast forward)
        if !fast {
            next += frame_time;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else if now - next > Duration::from_millis(200) {
                next = now;
            }
        }
    }

    if let Some(f) = std::env::var_os("DDSOM_ENREGISTRE") {
        saved(&game, f.as_ref());
    }
    if let Some(dir) = nvram.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(nvram, m.bus.hw().eeprom.to_bytes()) {
        eprintln!("Cannot save the EEPROM: {e}");
    }
}

const TITLE: &str = "Dungeons & Dragons: Shadow over Mystara (port Rust)";

/// Game window, or a borderless window covering the screen (frame stretched without distortion) in full screen.
fn window(full_screen: bool) -> Window {
    let mut win = if full_screen {
        let opts = WindowOptions { none: true, scale_mode: ScaleMode::AspectRatioStretch, ..WindowOptions::default() };
        match screen_size() {
            Some((l, h)) => Window::new(TITLE, l, h, opts),
            // unknown screen size: largest whole multiple that fits
            None => Window::new(TITLE, 384, 224, WindowOptions { scale: Scale::FitScreen, ..opts }),
        }
    } else {
        Window::new(TITLE, 384, 224, WindowOptions { scale: Scale::X2, ..WindowOptions::default() })
    }
    .expect("window");
    if full_screen {
        win.set_position(0, 0);
        win.set_cursor_visibility(false);
    }
    win
}

/// Size of the main screen in pixels (Windows: `GetSystemMetrics` from user32).
#[cfg(windows)]
fn screen_size() -> Option<(usize, usize)> {
    type GetSystemMetricsFn = unsafe extern "system" fn(index: i32) -> i32;
    // SAFETY: signature matches <winuser.h>; the library stays loaded during the calls.
    unsafe {
        let lib = libloading::Library::new("user32.dll").ok()?;
        let metric = *lib.get::<GetSystemMetricsFn>(b"GetSystemMetrics").ok()?;
        let (l, h) = (metric(0), metric(1)); // SM_CXSCREEN, SM_CYSCREEN
        (l > 0 && h > 0).then(|| (l as usize, h as usize))
    }
}

#[cfg(not(windows))]
fn screen_size() -> Option<(usize, usize)> {
    None
}

/// File of the quick save (F5 / F9).
const SAVE: &str = "sauvegardes/rapide.txt";

/// Default ROM folder.
const DEFAULT_ROMS: &str = "../rom";

/// ROM folder if given, and resources folder if `--assets` is given.
fn arguments(args: Vec<String>) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut romdir = None;
    let mut assets = None;
    let mut it = args.into_iter().peekable();
    while let Some(a) = it.next() {
        if a == "--assets" {
            assets = Some(PathBuf::from(it.next_if(|n| !n.starts_with("--")).unwrap_or_else(|| "../assets".into())));
        } else {
            romdir = Some(PathBuf::from(a));
        }
    }
    (romdir, assets)
}
