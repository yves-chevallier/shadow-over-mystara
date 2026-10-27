//! Dungeons & Dragons: Shadow over Mystara — port Rust jouable.
//!
//! ddsom [dossier_roms] [--assets dossier] : ROMs extraites (ddsom.zip + dl-1425.bin de qsound.zip), défaut
//! ../rom. Avec `--assets`, les graphismes et les échantillons viennent du dossier produit par `extract`
//! (PNG et WAV) au lieu des ROMs graphiques et sonores (`dd2.11m`...`dd2.20m` non lues) ; `--assets` seul
//! (sans dossier) lit `../assets`.
//! Le jeu tourne sans 68000 : seulement les routines réécrites en Rust (`docs/noyau.md`). Avec `--assets`
//! sans dossier de ROMs, il se joue **sans aucune ROM** : tout vient du dossier d'assets (tables du programme,
//! graphismes, données du pilote son, tables du DSP, échantillons ; `docs/sans-rom.md`), le pilote son est
//! celui en Rust, et aucun fichier ROM n'est ouvert.
//! Diagnostic : `DDSOM_HYBRIDE=1` (routines Rust et code 68000 recompilé pour le reste, 68000 surcadencé),
//! `DDSOM_ORIGINE=1` (code d'origine à la vitesse de l'arcade), `DDSOM_INTERP=1` (tout interprété) ; ces modes
//! lisent les ROMs.
//! Touches (comme MAME) : 5 pièce, 1 Start, flèches, Ctrl/Alt/Espace/Maj = boutons 1-4
//! (ou W X C V), 6 pièce J2 / 2 Start J2, F2 test, P pause, Tab avance rapide, Échap quitter ;
//! à changer dans touches.cfg (`touches.rs`), manettes Xbox comprises sous Windows (`manette.rs`).
//! L'EEPROM (réglages, compteurs) est conservée dans nvram/ddsom.eeprom.
//! F5 sauvegarde la partie dans sauvegardes/rapide.txt, F9 la reprend (`sauvegarde.rs` : rejeu des entrées).
//! `DDSOM_ENREGISTRE=partie.txt` écrit de même la partie en quittant : `validate <roms> partie.txt.eeprom
//! partie.txt <images>` la rejoue.

mod audio;
mod manette;
mod sauvegarde;
mod touches;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cps2::Machine;
use minifb::{Scale, Window, WindowOptions};

const FRAME_HZ: f64 = 8_000_000.0 / 512.0 / 262.0;

fn main() {
    let (romdir, assets) = arguments(std::env::args().skip(1).collect());
    let nvram = PathBuf::from("nvram/ddsom.eeprom");
    let mut touches = touches::Touches::charge("touches.cfg".as_ref());
    let eeprom = std::fs::read(&nvram).ok();
    // le jeu : sans 68000, routines Rust seules (il s'arrête sur une adresse qui n'en a pas). Diagnostic :
    // DDSOM_HYBRIDE=1 : routines Rust, le reste en code 68000 recompilé, 68000 surcadencé ; DDSOM_ORIGINE=1 : code
    // d'origine à la vitesse de l'arcade ; DDSOM_INTERP=1 : tout interprété.
    let hybride = std::env::var("DDSOM_HYBRIDE").is_ok_and(|v| v == "1");
    let mut sans_68k =
        !(hybride || std::env::var_os("DDSOM_ORIGINE").is_some() || std::env::var_os("DDSOM_INTERP").is_some());
    // sans ROM : assets seuls, ce qui demande le jeu sans 68000 (le programme 68000 n'est pas dans les assets)
    let romdir = match (&assets, romdir) {
        (Some(_), None) if !sans_68k && !PathBuf::from(DEFAULT_ROMS).is_dir() => {
            eprintln!("Pas de dossier de ROMs : jeu sans ROM, depuis les assets seuls (sans 68000, pas de diagnostic).");
            sans_68k = true;
            None
        }
        (Some(_), None) if sans_68k => None,
        (_, r) => Some(r.unwrap_or_else(|| DEFAULT_ROMS.into())),
    };
    let contenus = match (&assets, &romdir) {
        (Some(dir), None) => ddsom_assets::rebuild::contenus(dir),
        (Some(dir), Some(romdir)) => ddsom_assets::rebuild::gfx_region(&dir.join("gfx"))
            .and_then(|gfx| Ok((gfx, ddsom_assets::rebuild::qsound_samples(dir)?.bytes)))
            .and_then(|(gfx, pcm)| cps2::Contenus::depuis_roms(romdir, Some(gfx), Some(pcm))),
        (None, Some(romdir)) => cps2::Contenus::depuis_roms(romdir, None, None),
        (None, None) => unreachable!(),
    };
    let contenus = match contenus {
        Ok(c) => c,
        Err(e) => {
            match (&assets, &romdir) {
                (Some(dir), None) => eprintln!("Impossible de charger les assets depuis {} : {e}", dir.display()),
                (_, Some(romdir)) => eprintln!("Impossible de charger les ROMs depuis {} : {e}", romdir.display()),
                (None, None) => unreachable!(),
            }
            std::process::exit(1);
        }
    };
    // une machine neuve : au démarrage, et à chaque reprise d'une sauvegarde (`sauvegarde.rs`)
    let machine = |eeprom: Option<&[u8]>| {
        let mut m = Machine::ddsom(contenus.clone(), eeprom);
        if sans_68k {
            ddsom_game::install_sans_68k(&mut m);
        } else if std::env::var_os("DDSOM_ORIGINE").is_some() {
            ddsom_rc::install(&mut m);
        } else if std::env::var_os("DDSOM_INTERP").is_none() {
            ddsom_game::install(&mut m);
            m.set_overclock(16.0);
        }
        // pilote son réécrit en Rust à la place du programme du Z80 : DDSOM_SON_RUST=1, et toujours sans ROM (la
        // région du Z80 reconstruite depuis les assets n'a que les données du pilote, pas son programme)
        if std::env::var_os("DDSOM_SON_RUST").is_some() || romdir.is_none() {
            m.sound.use_rust_driver();
        }
        m
    };
    let mut m = machine(eeprom.as_deref());

    let mut win = Window::new(
        "Dungeons & Dragons: Shadow over Mystara (port Rust)",
        384,
        224,
        WindowOptions { scale: Scale::X2, ..WindowOptions::default() },
    )
    .expect("fenêtre");

    let audio = audio::Audio::open(qsound::SAMPLE_RATE.round() as u32);
    if audio.is_none() {
        eprintln!("Son indisponible (pas de sortie audio) : le jeu tourne sans son.");
    }

    let frame_time = Duration::from_secs_f64(1.0 / FRAME_HZ);
    let mut next = Instant::now();
    let mut paused = false;
    let mut partie = sauvegarde::Partie::nouvelle(&m);
    let sauve = |partie: &sauvegarde::Partie, fichier: &Path| match partie.sauve(fichier) {
        Ok(()) => eprintln!("Partie sauvegardée dans {} (image {}).", fichier.display(), partie.images()),
        Err(e) => eprintln!("Sauvegarde dans {} impossible : {e}", fichier.display()),
    };
    let mut buf = vec![0u32; 384 * 224];

    while win.is_open() {
        let t = touches.lit(&win);
        if t.quitter {
            break;
        }
        if t.pause {
            paused = !paused;
        }
        if t.sauver {
            sauve(&partie, SAUVEGARDE.as_ref());
        }
        if t.charger {
            let debut = Instant::now();
            match sauvegarde::Partie::charge(SAUVEGARDE.as_ref(), |ee| machine(Some(ee))) {
                Ok((nouvelle, p)) => {
                    eprintln!("Partie reprise à l'image {} ({:.1} s).", p.images(), debut.elapsed().as_secs_f64());
                    (m, partie) = (nouvelle, p);
                    next = Instant::now();
                }
                Err(e) => eprintln!("Reprise de {} impossible : {e}", SAUVEGARDE),
            }
        }
        if paused {
            win.update();
            std::thread::sleep(Duration::from_millis(16));
            next = Instant::now();
            continue;
        }
        m.bus.inputs.in0 = t.in0;
        m.bus.inputs.in2 = t.in2;
        partie.image(&m);
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
        let fast = t.rapide;
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

    if let Some(f) = std::env::var_os("DDSOM_ENREGISTRE") {
        sauve(&partie, f.as_ref());
    }
    if let Some(dir) = nvram.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&nvram, m.bus.eeprom.to_bytes()) {
        eprintln!("Sauvegarde de l'EEPROM impossible : {e}");
    }
}

/// Fichier de la sauvegarde rapide (F5 / F9).
const SAUVEGARDE: &str = "sauvegardes/rapide.txt";

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
