//! Dungeons & Dragons: Shadow over Mystara — port Rust jouable.
//!
//! ddsom [dossier_roms] : ROMs extraites (ddsom.zip + dl-1425.bin de qsound.zip), défaut ../rom.
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
    let romdir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "../rom".into()));
    let nvram = PathBuf::from("nvram/ddsom.eeprom");
    let eeprom = std::fs::read(&nvram).ok();
    let mut m = match Machine::load_ddsom(&romdir, eeprom.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Impossible de charger les ROMs depuis {} : {e}", romdir.display());
            std::process::exit(1);
        }
    };

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
        let img = m.run_frame();
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
