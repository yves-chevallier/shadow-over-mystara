//! Dungeons & Dragons: Shadow over Mystara — port Rust jouable.
//!
//! ddsom [roms_dir] [--assets dir] : extracted ROMs (ddsom.zip + dl-1425.bin from qsound.zip), default
//! ../rom. With `--assets`, everything comes from the folder produced by `extract` (`--assets` alone, without a
//! folder, reads `../assets`).
//! The game runs without the 68000: only the routines rewritten in Rust (`docs/noyau.md`), on a machine without ROM.
//! The application talks to it only through `ddsom_game::Game` (one step per frame with the inputs, the frame to draw,
//! the mixed sound, the EEPROM); the frame is drawn by the CPS2 renderer (`cps2_gfx::render`) and shown in a macroquad
//! window (`view.rs`: whole or fitted scale, filters), the sound played by `audio.rs`.
//! It loads the typed game data (`ddsom_data::GameData`), the tile atlas, the data of the Rust sound driver, the DSP
//! tables and the sample bank: from the assets folder (PNG, WAV, JSON; no ROM file is opened, docs/sans-rom.md), or
//! extracted in memory from the ROMs (`extract::install_game*`).
//! Diagnostics (build with `--features original`): `DDSOM_ORIGINE=1` (original code at arcade speed), `DDSOM_INTERP=1`
//! (everything interpreted); these modes
//! read the ROMs.
//! Keys (as in MAME): 5 coin, 1 Start, arrows, Ctrl/Alt/Space/Shift = buttons 1-4
//! (or W X C V), 6 coin P2 / 2 Start P2, F2 test, P pause, Tab fast forward, F11 full screen, F6 scale (whole
//! multiples or fit), F7 filter (none, smooth, CRT), F8 HD pack on/off, Esc quit; rebindable in touches.cfg (`keys.rs`), gamepads
//! included (`gamepad.rs`).
//! The EEPROM (settings, counters) is kept in nvram/ddsom.eeprom.
//! HD pack (docs/pack-hd.md): the folder `hd/` (or `DDSOM_HD=<dir>`), if present, replaces the sprite frames and
//! layer tiles it holds by its larger images (`ddsom_hd::render_hd`); F8 switches it off and on.
//! F5 saves the game to sauvegardes/rapide.txt, F9 resumes it (`save.rs`: input replay).
//! `DDSOM_ENREGISTRE=partie.txt` likewise writes the game on exit: `validate <roms> partie.txt.eeprom
//! partie.txt <frames>` replays it.

mod audio;
mod gamepad;
mod keys;
mod save;
mod view;

use std::path::{Path, PathBuf};

use cps2_gfx::{TileAtlas, SCREEN_H, SCREEN_W};
use ddsom_game::core::interface::Board;
use ddsom_game::Game;
use keys::Command;
use macroquad::prelude::*;

const FRAME_HZ: f64 = 8_000_000.0 / 512.0 / 262.0;

fn main() {
    let (romdir, assets) = arguments(std::env::args().skip(1).collect());
    let nvram = PathBuf::from("nvram/ddsom.eeprom");
    let keys = keys::Keys::load("touches.cfg".as_ref());
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
    // a fresh game: at startup, and on each resume from a save (`save.rs`)
    macroquad::Window::from_config(window_conf(), async move {
        match originals {
            // the game: no ROM, the Rust sound driver
            None => {
                let new_game = |eeprom: Option<&[u8]>| Game::new(contents.clone(), eeprom);
                play(new_game, &contents.graphics, keys, eeprom, &nvram, no_68k).await
            }
            Some(c) => {
                let new_game = |eeprom: Option<&[u8]>| {
                    let mut m = cps2::Machine::ddsom(c.clone(), eeprom);
                    #[cfg(feature = "original")]
                    if std::env::var_os("DDSOM_ORIGINE").is_some() {
                        ddsom_rc::install(&mut m);
                    }
                    m.sound.use_rust_driver();
                    Game::on(m)
                };
                play(new_game, &c.graphics, keys, eeprom, &nvram, no_68k).await
            }
        }
    });
}

/// The window: twice the frame (once for `DDSOM_IDENTITY`), resizable, synchronised with the display.
fn window_conf() -> Conf {
    let scale = if std::env::var_os("DDSOM_IDENTITY").is_some() { 1 } else { 2 };
    Conf {
        window_title: "Dungeons & Dragons: Shadow over Mystara (port Rust)".into(),
        window_width: (SCREEN_W * scale) as i32,
        window_height: (SCREEN_H * scale) as i32,
        window_resizable: true,
        platform: miniquad::conf::Platform { swap_interval: Some(1), ..Default::default() },
        ..Default::default()
    }
}

/// Plays the games that `new_game` builds (from the starting EEPROM): the game, or the original code. `graphics`: the
/// tiles that the frames designate. One pass of the loop per displayed frame (the display's refresh rate); the game
/// steps at its own rate (`FRAME_HZ`), from the clock.
async fn play<B: Board>(
    new_game: impl Fn(Option<&[u8]>) -> Game<B>,
    graphics: &TileAtlas,
    mut keys: keys::Keys,
    eeprom: Option<Vec<u8>>,
    nvram: &Path,
    no_68k: bool,
) {
    prevent_quit();
    let mut game = new_game(eeprom.as_deref());
    let mut screen = view::Screen::new();
    let mut identity = std::env::var("DDSOM_IDENTITY").ok().map(|n| Identity::new(n.parse().unwrap_or(600)));
    // the HD pack (not with DDSOM_IDENTITY, which checks the CPS2 image)
    let pack = if identity.is_some() { None } else { load_pack() };
    let mut hd = pack.is_some();

    let audio = audio::Audio::open();

    let mut clock = Clock::new();
    let mut paused = false;
    let mut full_screen = false;
    let mut session = save::Session::new(&game);
    let mut redraw = false;

    loop {
        let t = keys.read();
        if t.quit || is_quit_requested() {
            break;
        }
        for command in &t.commands {
            match command {
                Command::FullScreen => {
                    full_screen = !full_screen;
                    set_fullscreen(full_screen);
                    show_mouse(!full_screen);
                }
                Command::Scale => screen.next_scale(),
                Command::Filter => screen.next_filter(),
                Command::Hd => {
                    hd = !hd && pack.is_some();
                    redraw = true;
                }
                Command::Pause => paused = !paused,
                Command::Save => saved(&session, SAVE.as_ref()),
                Command::Load => {
                    let start = get_time();
                    match save::Session::load(SAVE.as_ref(), |ee| new_game(Some(ee))) {
                        Ok((new, s)) => {
                            eprintln!("Game resumed at frame {} ({:.1} s).", s.images(), get_time() - start);
                            (game, session) = (new, s);
                        }
                        Err(e) => eprintln!("Cannot resume from {SAVE}: {e}"),
                    }
                }
            }
        }
        if paused || t.commands.contains(&Command::Load) {
            clock.restart();
        }
        let steps = match (paused, t.fast) {
            (true, _) => 0,
            (false, true) => FAST_STEPS,
            (false, false) => clock.due(),
        };
        for _ in 0..steps {
            session.frame(t.inputs);
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| game.step(t.inputs))) {
                Ok(()) => {}
                // without the 68000: stop on an address with no Rust routine (diagnostic already printed by the panic)
                Err(e) if no_68k => {
                    let m = e.downcast_ref::<String>().cloned().unwrap_or_default();
                    eprintln!("The game without the 68000 stops: {}", m.trim_start_matches(ddsom_game::core::interface::STOP));
                    return finish(&game, &session, nvram);
                }
                Err(e) => std::panic::resume_unwind(e),
            }
            if !t.fast {
                audio.push(game.sound());
            }
        }
        let image = (steps > 0 || redraw).then(|| match (&pack, hd) {
            (Some(pack), true) => (ddsom_hd::render_hd(&game.frame(), graphics, pack), pack.scale),
            _ => (cps2_gfx::render(&game.frame(), graphics), 1),
        });
        redraw = false;
        if let Some((image, scale)) = &image {
            screen.show(image, *scale);
        }
        screen.draw();
        if let (Some(check), Some((image, _))) = (&mut identity, &image) {
            if check.compare(image, &screen, game.frame_count()) {
                return;
            }
        }
        if steps == 0 {
            clock.idle(paused);
        }
        next_frame().await;
    }
    finish(&game, &session, nvram);
}

/// The HD pack of `DDSOM_HD`, or of `hd/` if it exists; nothing if there is none or it holds no image.
fn load_pack() -> Option<ddsom_hd::HdPack> {
    let dir = std::env::var_os("DDSOM_HD").map_or_else(|| PathBuf::from("hd"), PathBuf::from);
    if !dir.is_dir() {
        return None;
    }
    let pack = ddsom_hd::load(&dir);
    let tiles: usize = pack.tiles.iter().map(|t| t.len()).sum();
    eprintln!("HD pack {}: scale {}, {} frames, {tiles} tiles (F8: on/off).", dir.display(), pack.scale, pack.frames.len());
    (!pack.is_empty()).then_some(pack)
}

fn saved(session: &save::Session, file: &Path) {
    match session.saved(file) {
        Ok(()) => eprintln!("Game saved to {} (frame {}).", file.display(), session.images()),
        Err(e) => eprintln!("Cannot save to {}: {e}", file.display()),
    }
}

/// On exit: the game recorded if `DDSOM_ENREGISTRE` names a file, the EEPROM saved.
fn finish<B: Board>(game: &Game<B>, session: &save::Session, nvram: &Path) {
    if let Some(f) = std::env::var_os("DDSOM_ENREGISTRE") {
        saved(session, f.as_ref());
    }
    if let Some(dir) = nvram.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(nvram, game.eeprom()) {
        eprintln!("Cannot save the EEPROM: {e}");
    }
}

/// Frames of the game run per displayed frame in fast forward.
const FAST_STEPS: usize = 8;

/// The pace of the game: `FRAME_HZ` frames per second of the clock, whatever the display's refresh rate.
struct Clock {
    /// Time of the next frame of the game.
    next: f64,
}

impl Clock {
    fn new() -> Self {
        Clock { next: get_time() }
    }

    /// Starts again from now (after a pause or a resume).
    fn restart(&mut self) {
        self.next = get_time();
    }

    /// Number of frames of the game due by now.
    fn due(&mut self) -> usize {
        let now = get_time();
        if now - self.next > 0.2 {
            // far behind (stalled window, slow machine): skip rather than catch up
            self.next = now;
        }
        let mut n = 0;
        while self.next <= now {
            self.next += 1.0 / FRAME_HZ;
            n += 1;
        }
        n
    }

    /// Nothing was due: waits for the next frame of the game (a display without vertical synchronisation, as with
    /// software OpenGL, would otherwise turn the loop as fast as it can). The browser paces its own loop.
    fn idle(&self, paused: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let wait = if paused { 1.0 / FRAME_HZ } else { self.next - get_time() };
            std::thread::sleep(std::time::Duration::from_secs_f64(wait.clamp(0.0, 0.05)));
        }
        #[cfg(target_arch = "wasm32")]
        let _ = paused;
    }
}

/// `DDSOM_IDENTITY=N`: the window (scale 1, no filter) is read back after each frame of the game up to frame `N`
/// and compared with the image of the CPS2 renderer; the result is printed and the application quits.
struct Identity {
    last: u64,
    frames: u64,
    differing: u64,
}

impl Identity {
    fn new(last: u64) -> Self {
        Identity { last, frames: 0, differing: 0 }
    }

    /// Compares the window with `image`; true once the last frame is checked.
    fn compare(&mut self, image: &[[u8; 3]], screen: &view::Screen, frame: u64) -> bool {
        let (x0, y0, s) = screen.placement();
        assert!(s == 1.0 && screen.filter == view::Filter::None, "DDSOM_IDENTITY: scale 1 without filter only");
        let window = get_screen_data();
        let (w, h) = (window.width as usize, window.height as usize);
        let (x0, y0) = (x0 as usize, y0 as usize);
        for (i, p) in image.iter().enumerate() {
            let (x, y) = (x0 + i % SCREEN_W, y0 + i / SCREEN_W);
            // the read back starts with the bottom row
            let at = ((h - 1 - y) * w + x) * 4;
            self.differing += u64::from(window.bytes[at..at + 3] != p[..]);
        }
        self.frames += 1;
        if frame < self.last {
            return false;
        }
        println!(
            "DDSOM_IDENTITY: {} frames read back ({w} x {h} window, frame at {x0},{y0}, scale 1): \
             {} pixels differ from cps2_gfx::render",
            self.frames, self.differing
        );
        true
    }
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
