//! Configurable keys: `touches.cfg` (created with the default keys if missing) maps each
//! action to a list of minifb key names (`Left`, `LeftCtrl`, `A`, `Key1`, `NumPad0`...) or gamepad
//! buttons (`Pad1A`, `Pad1Haut`... : `gamepad.rs`).
//! Letters and digits follow the keyboard layout (on AZERTY, `Key1` needs Shift);
//! `DDSOM_TOUCHES=1` prints the name of the pressed keys to fill in the file.

use std::path::Path;

use minifb::Window;

use crate::gamepad::Gamepads;

/// CPS2 input port touched by an action (active low).
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

/// Actions: name in the file, effect, default keys (as in MAME).
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

/// Keys of each action, by minifb name.
pub struct Keys {
    actions: Vec<(Port, Vec<String>)>,
    gamepads: Gamepads,
    trace: bool,
    before: Vec<String>,
    /// Pause, save, load, full screen held on the previous frame (they act on press).
    before_press: [bool; 4],
}

/// Keyboard reading for one frame.
pub struct State {
    /// The inputs of the game (CPS2 ports, active low).
    pub inputs: ddsom_game::Inputs,
    /// Pause, save, load, full screen have just been pressed.
    pub pause: bool,
    pub save: bool,
    pub load: bool,
    pub full_screen: bool,
    pub fast: bool,
    pub quit: bool,
}

impl Keys {
    /// Reads `file` (lines `action = keys...`, `#` comment); writes it with the defaults if missing.
    pub fn load(file: &Path) -> Self {
        let mut actions: Vec<(Port, Vec<String>)> =
            ACTIONS.iter().map(|&(_, port, keys)| (port, names(keys))).collect();
        match std::fs::read_to_string(file) {
            Ok(text) => {
                if !text.contains("Pad1") {
                    eprintln!("{} names no gamepad button: delete it to recreate it.", file.display());
                }
                for (n, line) in text.lines().enumerate() {
                    let line = line.split('#').next().unwrap_or("").trim();
                    if line.is_empty() {
                        continue;
                    }
                    let Some((name, keys)) = line.split_once('=') else {
                        eprintln!("{}:{}: 'action = keys' expected", file.display(), n + 1);
                        continue;
                    };
                    match ACTIONS.iter().position(|a| a.0 == name.trim()) {
                        Some(i) => actions[i].1 = names(keys),
                        None => eprintln!("{}:{}: unknown action '{}'", file.display(), n + 1, name.trim()),
                    }
                }
            }
            Err(_) => {
                if let Err(e) = std::fs::write(file, defaults()) {
                    eprintln!("Cannot write {}: {e}", file.display());
                }
            }
        }
        let trace = std::env::var("DDSOM_TOUCHES").is_ok_and(|v| v == "1");
        Keys { actions, gamepads: Gamepads::opens(), trace, before: Vec::new(), before_press: [false; 4] }
    }

    /// State of the actions from the held keys and buttons.
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
            inputs: ddsom_game::Inputs::default(),
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
                    Port::In0(bit) => e.inputs.in0 &= !bit,
                    Port::In2(bit) => e.inputs.in2 &= !bit,
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

/// Contents of the file created when it is missing.
fn defaults() -> String {
    let mut s = String::from(
        "# ddsom port keys: action = minifb key names separated by spaces (empty = none).\n\
         # Noms : Left Right Up Down, LeftCtrl RightCtrl LeftAlt LeftShift Space Enter Tab Escape,\n\
         # A..Z, Key0..Key9, NumPad0..NumPad9, F1..F12... Letters and digits follow the keyboard layout.\n\
         # Manettes Xbox (Windows) : Pad1A Pad1B Pad1X Pad1Y Pad1LB Pad1RB Pad1Start Pad1Back,\n\
         # Pad1Haut Pad1Bas Pad1Gauche Pad1Droite (d-pad or left stick); Pad2... for the second.\n\
         # DDSOM_TOUCHES=1 prints the name of the pressed keys and buttons.\n\n",
    );
    for (name, _, keys) in ACTIONS {
        s += &format!("{name} = {keys}\n");
    }
    s
}
