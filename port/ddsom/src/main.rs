//! Dungeons & Dragons: Shadow over Mystara — port Rust jouable.
//!
//! ddsom [dossier_roms] [--assets dossier] : ROMs extraites (ddsom.zip + dl-1425.bin de qsound.zip), défaut
//! ../rom. Avec `--assets`, les graphismes et les échantillons viennent du dossier produit par `extract`
//! (PNG et WAV) au lieu des ROMs graphiques et sonores (`dd2.11m`...`dd2.20m` non lues) ; `--assets` seul
//! (sans dossier) lit `../assets`.
//! `DDSOM_SANS_68K=1` : jeu sans 68000, seulement les routines réécrites en Rust (`docs/noyau.md`). Avec
//! `--assets`, il se joue alors **sans aucune ROM** : tout vient du dossier d'assets (tables du programme,
//! graphismes, données du pilote son, tables du DSP, échantillons ; `docs/sans-rom.md`), le pilote son est
//! celui en Rust, et aucun fichier ROM n'est ouvert. Sans dossier de ROMs, `--assets` choisit ce mode de
//! lui-même.
//! Touches (comme MAME) : 5 pièce, 1 Start, flèches, Ctrl/Alt/Espace/Maj = boutons 1-4
//! (ou W X C V), 6 pièce J2 / 2 Start J2, F2 test, P pause, Tab avance rapide, Échap quitter.
//! L'EEPROM (réglages, compteurs) est conservée dans nvram/ddsom.eeprom.

mod audio;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use cps2::Machine;
use minifb::{Key, KeyRepeat, Scale, Window, WindowOptions};

const FRAME_HZ: f64 = 8_000_000.0 / 512.0 / 262.0;

fn main() {
    let (romdir, assets) = arguments(std::env::args().skip(1).collect());
    let nvram = PathBuf::from("nvram/ddsom.eeprom");
    let eeprom = std::fs::read(&nvram).ok();
    // jeu réécrit en Rust (le reste en code 68000 recompilé), 68000 surcadencé : pas de
    // ralentissements. DDSOM_SANS_68K=1 : sans 68000, routines Rust seules (le jeu s'arrête sur une
    // adresse qui n'en a pas) ; DDSOM_ORIGINE=1 : code d'origine à la vitesse de l'arcade ;
    // DDSOM_INTERP=1 : tout interprété.
    let mut sans_68k = std::env::var("DDSOM_SANS_68K").is_ok_and(|v| v == "1");
    // sans ROM : assets seuls, ce qui demande le jeu sans 68000 (le programme 68000 n'est pas dans les assets)
    let romdir = match (&assets, romdir) {
        (Some(_), None) if !sans_68k && !PathBuf::from(DEFAULT_ROMS).is_dir() => {
            eprintln!("Pas de dossier de ROMs : jeu sans ROM, depuis les assets seuls (sans 68000).");
            sans_68k = true;
            None
        }
        (Some(_), None) if sans_68k => None,
        (_, r) => Some(r.unwrap_or_else(|| DEFAULT_ROMS.into())),
    };
    let loaded = match (&assets, &romdir) {
        (Some(dir), None) => ddsom_assets::rebuild::contenus(dir).map(|c| Machine::ddsom(c, eeprom.as_deref())),
        (Some(dir), Some(romdir)) => ddsom_assets::rebuild::gfx_region(&dir.join("gfx"))
            .and_then(|gfx| Ok((gfx, ddsom_assets::rebuild::qsound_samples(dir)?.bytes)))
            .and_then(|(gfx, pcm)| Machine::load_ddsom_with(romdir, eeprom.as_deref(), Some(gfx), Some(pcm))),
        (None, Some(romdir)) => Machine::load_ddsom(romdir, eeprom.as_deref()),
        (None, None) => unreachable!(),
    };
    let mut m = match loaded {
        Ok(m) => m,
        Err(e) => {
            match (&assets, &romdir) {
                (Some(dir), None) => eprintln!("Impossible de charger les assets depuis {} : {e}", dir.display()),
                (_, Some(romdir)) => eprintln!("Impossible de charger les ROMs depuis {} : {e}", romdir.display()),
                (None, None) => unreachable!(),
            }
            std::process::exit(1);
        }
    };

    if sans_68k {
        ddsom_game::install_sans_68k(&mut m);
    } else if std::env::var_os("DDSOM_ORIGINE").is_some() {
        ddsom_rc::install(&mut m);
    } else if std::env::var_os("DDSOM_INTERP").is_none() {
        ddsom_game::install(&mut m);
        m.set_overclock(16.0);
    }
    // pilote son réécrit en Rust à la place du programme du Z80 : DDSOM_SON_RUST=1, et toujours sans ROM (la région
    // du Z80 reconstruite depuis les assets n'a que les données du pilote, pas son programme)
    if std::env::var_os("DDSOM_SON_RUST").is_some() || romdir.is_none() {
        m.sound.use_rust_driver();
    }

    let mut win = Window::new(
        "Dungeons & Dragons: Shadow over Mystara (port Rust)",
        384,
        224,
        WindowOptions { scale: Scale::X2, ..WindowOptions::default() },
    )
    .expect("fenêtre");

    let audio = audio::Audio::open(qsound::SAMPLE_RATE.round() as u32);
    if audio.is_none() {
        eprintln!("Son indisponible (PulseAudio introuvable) : le jeu tourne sans son.");
    }

    let frame_time = Duration::from_secs_f64(1.0 / FRAME_HZ);
    let mut next = Instant::now();
    let mut paused = false;
    let mut buf = vec![0u32; 384 * 224];

    while win.is_open() && !win.is_key_down(Key::Escape) {
        if win.is_key_pressed(Key::P, KeyRepeat::No) {
            paused = !paused;
        }
        if paused {
            win.update();
            std::thread::sleep(Duration::from_millis(16));
            next = Instant::now();
            continue;
        }
        set_inputs(&win, &mut m);
        let img = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())) {
            Ok(img) => img,
            // sans 68000 : arrêt sur une adresse sans routine Rust (diagnostic déjà affiché par la panique)
            Err(e) if sans_68k => {
                let m = e.downcast_ref::<String>().cloned().unwrap_or_default();
                eprintln!("Le jeu sans 68000 s'arrête : {}", m.trim_start_matches(ddsom_game::sans68k::ARRET));
                break;
            }
            Err(e) => std::panic::resume_unwind(e),
        };
        for (d, p) in buf.iter_mut().zip(&img) {
            *d = (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32;
        }
        win.update_with_buffer(&buf, 384, 224).expect("affichage");

        let samples: Vec<i16> = m.sound.audio.drain(..).flat_map(|s| s).collect();
        let fast = win.is_key_down(Key::Tab);
        if let (Some(a), false) = (&audio, fast) {
            a.push(samples);
        }
        // cadence : 59,64 images/s (sauf avance rapide)
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

    if let Some(dir) = nvram.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&nvram, m.bus.eeprom.to_bytes()) {
        eprintln!("Sauvegarde de l'EEPROM impossible : {e}");
    }
}

/// Dossier des ROMs par défaut.
const DEFAULT_ROMS: &str = "../rom";

/// Dossier des ROMs s'il est donné, et dossier des ressources si `--assets` est donné.
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

/// Clavier -> ports d'entrée CPS2 (actifs à 0).
fn set_inputs(win: &Window, m: &mut Machine) {
    let k = |keys: &[Key]| keys.iter().any(|&key| win.is_key_down(key));
    let mut in0: u16 = 0xffff;
    let p1 = [
        (&[Key::Right][..], 0x01),
        (&[Key::Left][..], 0x02),
        (&[Key::Down][..], 0x04),
        (&[Key::Up][..], 0x08),
        (&[Key::LeftCtrl, Key::W][..], 0x10),
        (&[Key::LeftAlt, Key::X][..], 0x20),
        (&[Key::Space, Key::C][..], 0x40),
        (&[Key::LeftShift, Key::V][..], 0x80),
    ];
    for (keys, bit) in p1 {
        if k(keys) {
            in0 &= !bit;
        }
    }
    // joueur 2 : R F D G, boutons A S Q W -> comme MAME
    let p2 = [
        (&[Key::G][..], 0x0100),
        (&[Key::D][..], 0x0200),
        (&[Key::F][..], 0x0400),
        (&[Key::R][..], 0x0800),
        (&[Key::A][..], 0x1000),
        (&[Key::S][..], 0x2000),
        (&[Key::Q][..], 0x4000),
    ];
    for (keys, bit) in p2 {
        if k(keys) {
            in0 &= !bit;
        }
    }
    let mut in2: u16 = 0xffff;
    let sys = [
        (&[Key::F2][..], 0x0002),
        (&[Key::Key9][..], 0x0004),
        (&[Key::Key1][..], 0x0100),
        (&[Key::Key2][..], 0x0200),
        (&[Key::Key5][..], 0x1000),
        (&[Key::Key6][..], 0x2000),
    ];
    for (keys, bit) in sys {
        if k(keys) {
            in2 &= !bit;
        }
    }
    m.bus.inputs.in0 = in0;
    m.bus.inputs.in2 = in2;
}
