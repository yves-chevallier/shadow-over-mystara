//! Touches configurables : `keys.cfg` (créé avec les touches par défaut s'il manque) associe à chaque
//! action une liste de noms de touches minifb (`Left`, `LeftCtrl`, `A`, `Key1`, `NumPad0`...) ou de boutons
//! de manette (`Pad1A`, `Pad1Haut`... : `gamepad.rs`).
//! Les lettres et les chiffres suivent la disposition du clavier (en AZERTY, `Key1` demande Maj) ;
//! `DDSOM_TOUCHES=1` affiche le nom des touches pressées pour remplir le fichier.

use std::path::Path;

use minifb::Window;

use crate::gamepad::Gamepads;

/// Port d'entrée CPS2 touché par une action (actif à 0).
#[derive(Clone, Copy)]
pub enum Port {
    In0(u16),
    In2(u16),
    Pause,
    Save,
    Load,
    Fast,
    FullScreen,
    Quit,
}

/// Actions : nom dans le fichier, effet, touches par défaut (comme MAME).
const ACTIONS: &[(&str, Port, &str)] = &[
    ("j1.droite", Port::In0(0x01), "Right Pad1Droite"),
    ("j1.gauche", Port::In0(0x02), "Left Pad1Gauche"),
    ("j1.bas", Port::In0(0x04), "Down Pad1Bas"),
    ("j1.haut", Port::In0(0x08), "Up Pad1Haut"),
    ("j1.bouton1", Port::In0(0x10), "LeftCtrl W Pad1X"),
    ("j1.bouton2", Port::In0(0x20), "LeftAlt X Pad1Y"),
    ("j1.bouton3", Port::In0(0x40), "Space C Pad1B"),
    ("j1.bouton4", Port::In0(0x80), "LeftShift V Pad1A"),
    ("j2.droite", Port::In0(0x0100), "G Pad2Droite"),
    ("j2.gauche", Port::In0(0x0200), "D Pad2Gauche"),
    ("j2.bas", Port::In0(0x0400), "F Pad2Bas"),
    ("j2.haut", Port::In0(0x0800), "R Pad2Haut"),
    ("j2.bouton1", Port::In0(0x1000), "A Pad2X"),
    ("j2.bouton2", Port::In0(0x2000), "S Pad2Y"),
    ("j2.bouton3", Port::In0(0x4000), "Q Pad2B"),
    ("j2.bouton4", Port::In0(0x8000), "Pad2A"),
    ("test", Port::In2(0x0002), "F2"),
    ("service", Port::In2(0x0004), "Key9"),
    ("j1.start", Port::In2(0x0100), "Key1 Pad1Start"),
    ("j2.start", Port::In2(0x0200), "Key2 Pad2Start"),
    ("j1.piece", Port::In2(0x1000), "Key5 Pad1Back"),
    ("j2.piece", Port::In2(0x2000), "Key6 Pad2Back"),
    ("pause", Port::Pause, "P"),
    ("sauver", Port::Save, "F5"),
    ("charger", Port::Load, "F9"),
    ("rapide", Port::Fast, "Tab"),
    ("plein_ecran", Port::FullScreen, "F11"),
    ("quitter", Port::Quit, "Escape"),
];

/// Touches de chaque action, par nom minifb.
pub struct Keys {
    actions: Vec<(Port, Vec<String>)>,
    gamepads: Gamepads,
    trace: bool,
    before: Vec<String>,
    /// Pause, sauver, charger, plein écran tenus à l'image précédente (ils agissent à l'appui).
    before_press: [bool; 4],
}

/// Lecture du clavier pour une image.
pub struct State {
    /// Ports d'entrée CPS2 (actifs à 0).
    pub in0: u16,
    pub in2: u16,
    /// Pause, sauver, charger, plein écran viennent d'être pressés.
    pub pause: bool,
    pub save: bool,
    pub load: bool,
    pub full_screen: bool,
    pub fast: bool,
    pub quit: bool,
}

impl Keys {
    /// Lit `file` (lignes `action = keys...`, `#` commentaire) ; l'écrit avec les défauts s'il manque.
    pub fn charge(file: &Path) -> Self {
        let mut actions: Vec<(Port, Vec<String>)> =
            ACTIONS.iter().map(|&(_, port, keys)| (port, names(keys))).collect();
        match std::fs::read_to_string(file) {
            Ok(text) => {
                if !text.contains("Pad1") {
                    eprintln!("{} ne nomme aucun bouton de manette : supprimez-le pour le recréer.", file.display());
                }
                for (n, ligne) in text.lines().enumerate() {
                    let ligne = ligne.split('#').next().unwrap_or("").trim();
                    if ligne.is_empty() {
                        continue;
                    }
                    let Some((name, keys)) = ligne.split_once('=') else {
                        eprintln!("{}:{} : « action = touches » attendu", file.display(), n + 1);
                        continue;
                    };
                    match ACTIONS.iter().position(|a| a.0 == name.trim()) {
                        Some(i) => actions[i].1 = names(keys),
                        None => eprintln!("{}:{} : action inconnue « {} »", file.display(), n + 1, name.trim()),
                    }
                }
            }
            Err(_) => {
                if let Err(e) = std::fs::write(file, defaults()) {
                    eprintln!("Écriture de {} impossible : {e}", file.display());
                }
            }
        }
        let trace = std::env::var("DDSOM_TOUCHES").is_ok_and(|v| v == "1");
        Keys { actions, gamepads: Gamepads::opens(), trace, before: Vec::new(), before_press: [false; 4] }
    }

    /// État des actions d'après les touches et les boutons tenus.
    pub fn lit(&mut self, win: &Window) -> State {
        let mut low: Vec<String> = win.get_keys().iter().map(|k| format!("{k:?}")).collect();
        low.extend(self.gamepads.held());
        if self.trace && low != self.before {
            let fresh: Vec<_> = low.iter().filter(|k| !self.before.contains(k)).collect();
            if !fresh.is_empty() {
                eprintln!("touches : {fresh:?}");
            }
            self.before = low.clone();
        }
        let mut e = State {
            in0: 0xffff,
            in2: 0xffff,
            pause: false,
            save: false,
            load: false,
            full_screen: false,
            fast: false,
            quit: false,
        };
        let mut press = [false; 4];
        for (port, keys) in &self.actions {
            if keys.iter().any(|k| low.contains(k)) {
                match *port {
                    Port::In0(bit) => e.in0 &= !bit,
                    Port::In2(bit) => e.in2 &= !bit,
                    Port::Pause => press[0] = true,
                    Port::Save => press[1] = true,
                    Port::Load => press[2] = true,
                    Port::FullScreen => press[3] = true,
                    Port::Fast => e.fast = true,
                    Port::Quit => e.quit = true,
                }
            }
        }
        [e.pause, e.save, e.load, e.full_screen] = std::array::from_fn(|i| press[i] && !self.before_press[i]);
        self.before_press = press;
        e
    }
}

fn names(keys: &str) -> Vec<String> {
    keys.split_whitespace().map(String::from).collect()
}

/// Contenu du fichier créé quand il manque.
fn defaults() -> String {
    let mut s = String::from(
        "# Touches du port ddsom : action = noms de touches minifb séparés par des espaces (vide = aucune).\n\
         # Noms : Left Right Up Down, LeftCtrl RightCtrl LeftAlt LeftShift Space Enter Tab Escape,\n\
         # A..Z, Key0..Key9, NumPad0..NumPad9, F1..F12... Lettres et chiffres suivent la disposition du clavier.\n\
         # Manettes Xbox (Windows) : Pad1A Pad1B Pad1X Pad1Y Pad1LB Pad1RB Pad1Start Pad1Back,\n\
         # Pad1Haut Pad1Bas Pad1Gauche Pad1Droite (croix ou stick gauche) ; Pad2... pour la deuxième.\n\
         # DDSOM_TOUCHES=1 affiche le nom des touches et boutons pressés.\n\n",
    );
    for (name, _, keys) in ACTIONS {
        s += &format!("{name} = {keys}\n");
    }
    s
}
