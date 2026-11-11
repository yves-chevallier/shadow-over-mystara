//! Configurable keys: `touches.cfg` (created with the default keys if missing) maps each action to a list of key
//! names (`Left`, `LeftCtrl`, `A`, `Key1`, `NumPad0`...) or gamepad buttons (`Pad1A`, `Pad1Up`...: `gamepad.rs`).
//! The former French names (`j1.haut`, `sauver`, `Pad1Haut`...) are still read; a file that uses them is rewritten
//! with the English names. Letters and digits follow the keyboard layout (on AZERTY, `Key1` needs Shift);
//! `DDSOM_TOUCHES=1` prints the name of the pressed keys to fill in the file.

use std::path::Path;

use macroquad::input::get_keys_down;

use crate::gamepad::Gamepads;

/// What an action does: a CPS2 input port bit (active low), or a command of the application.
#[derive(Clone, Copy, PartialEq)]
pub enum Port {
    In0(u16),
    In2(u16),
    /// Held: fast forward.
    Fast,
    Quit,
    /// Acts once per press.
    Command(Command),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Command {
    Pause,
    Save,
    Load,
    FullScreen,
    /// Next scale mode (whole multiples, fit).
    Scale,
    /// Next filter (none, smooth, CRT).
    Filter,
}

/// Actions: name in the file, former French name (still read), effect, default keys (as in MAME).
const ACTIONS: &[(&str, &str, Port, &str)] = &[
    ("p1.right", "j1.droite", Port::In0(0x01), "Right Pad1Right"),
    ("p1.left", "j1.gauche", Port::In0(0x02), "Left Pad1Left"),
    ("p1.down", "j1.bas", Port::In0(0x04), "Down Pad1Down"),
    ("p1.up", "j1.haut", Port::In0(0x08), "Up Pad1Up"),
    ("p1.button1", "j1.bouton1", Port::In0(0x10), "LeftCtrl W Pad1X"),
    ("p1.button2", "j1.bouton2", Port::In0(0x20), "LeftAlt X Pad1Y"),
    ("p1.button3", "j1.bouton3", Port::In0(0x40), "Space C Pad1B"),
    ("p1.button4", "j1.bouton4", Port::In0(0x80), "LeftShift V Pad1A"),
    ("p2.right", "j2.droite", Port::In0(0x0100), "G Pad2Right"),
    ("p2.left", "j2.gauche", Port::In0(0x0200), "D Pad2Left"),
    ("p2.down", "j2.bas", Port::In0(0x0400), "F Pad2Down"),
    ("p2.up", "j2.haut", Port::In0(0x0800), "R Pad2Up"),
    ("p2.button1", "j2.bouton1", Port::In0(0x1000), "A Pad2X"),
    ("p2.button2", "j2.bouton2", Port::In0(0x2000), "S Pad2Y"),
    ("p2.button3", "j2.bouton3", Port::In0(0x4000), "Q Pad2B"),
    ("p2.button4", "j2.bouton4", Port::In0(0x8000), "Pad2A"),
    ("test", "test", Port::In2(0x0002), "F2"),
    ("service", "service", Port::In2(0x0004), "Key9"),
    ("p1.start", "j1.start", Port::In2(0x0100), "Key1 Pad1Start"),
    ("p2.start", "j2.start", Port::In2(0x0200), "Key2 Pad2Start"),
    ("p1.coin", "j1.piece", Port::In2(0x1000), "Key5 Pad1Back"),
    ("p2.coin", "j2.piece", Port::In2(0x2000), "Key6 Pad2Back"),
    ("pause", "pause", Port::Command(Command::Pause), "P"),
    ("save", "sauver", Port::Command(Command::Save), "F5"),
    ("load", "charger", Port::Command(Command::Load), "F9"),
    ("fast", "rapide", Port::Fast, "Tab"),
    ("fullscreen", "plein_ecran", Port::Command(Command::FullScreen), "F11"),
    ("scale", "", Port::Command(Command::Scale), "F6"),
    ("filter", "", Port::Command(Command::Filter), "F7"),
    ("quit", "quitter", Port::Quit, "Escape"),
];

/// Keys of each action, by name.
pub struct Keys {
    actions: Vec<(Port, Vec<String>)>,
    gamepads: Gamepads,
    trace: bool,
    before: Vec<String>,
    /// Commands held on the previous reading (they act on press).
    held_before: Vec<Command>,
}

/// Keyboard and gamepad reading for one displayed frame.
pub struct State {
    /// The inputs of the game (CPS2 ports, active low).
    pub inputs: ddsom_game::Inputs,
    /// Commands just pressed.
    pub commands: Vec<Command>,
    pub fast: bool,
    pub quit: bool,
}

impl Keys {
    /// Reads `file` (lines `action = keys...`, `#` comment); writes it with the defaults if missing, rewrites it
    /// with the English names if it uses the former French ones.
    pub fn load(file: &Path) -> Self {
        let mut actions: Vec<(Port, Vec<String>)> =
            ACTIONS.iter().map(|&(_, _, port, keys)| (port, names(keys))).collect();
        let mut rewrite = true;
        if let Ok(text) = std::fs::read_to_string(file) {
            rewrite = false;
            for (n, line) in text.lines().enumerate() {
                let line = line.split('#').next().unwrap_or("").trim();
                if line.is_empty() {
                    continue;
                }
                let Some((name, keys)) = line.split_once('=') else {
                    eprintln!("{}:{}: 'action = keys' expected", file.display(), n + 1);
                    continue;
                };
                let name = name.trim();
                match ACTIONS.iter().position(|a| a.0 == name || a.1 == name) {
                    Some(i) => {
                        let keys = names(keys);
                        rewrite |= ACTIONS[i].0 != name || keys.iter().any(|k| french_pad_button(k).is_some());
                        actions[i].1 = keys.iter().map(|k| french_pad_button(k).unwrap_or_else(|| key_name(k))).collect();
                    }
                    None => eprintln!("{}:{}: unknown action '{name}'", file.display(), n + 1),
                }
            }
            if rewrite {
                eprintln!("{}: former French names, rewritten with the English names.", file.display());
            }
        }
        if rewrite {
            if let Err(e) = std::fs::write(file, contents(&actions)) {
                eprintln!("Cannot write {}: {e}", file.display());
            }
        }
        let trace = std::env::var("DDSOM_TOUCHES").is_ok_and(|v| v == "1");
        Keys { actions, gamepads: Gamepads::open(), trace, before: Vec::new(), held_before: Vec::new() }
    }

    /// State of the actions from the held keys and buttons.
    pub fn read(&mut self) -> State {
        let mut held: Vec<String> = get_keys_down().iter().map(|k| key_name(&format!("{k:?}"))).collect();
        held.extend(self.gamepads.held());
        if self.trace && held != self.before {
            let fresh: Vec<_> = held.iter().filter(|k| !self.before.contains(k)).collect();
            if !fresh.is_empty() {
                eprintln!("keys: {fresh:?}");
            }
            self.before = held.clone();
        }
        let mut state =
            State { inputs: ddsom_game::Inputs::default(), commands: Vec::new(), fast: false, quit: false };
        let mut commands = Vec::new();
        for (port, keys) in &self.actions {
            if keys.iter().any(|k| held.contains(k)) {
                match *port {
                    Port::In0(bit) => state.inputs.in0 &= !bit,
                    Port::In2(bit) => state.inputs.in2 &= !bit,
                    Port::Fast => state.fast = true,
                    Port::Quit => state.quit = true,
                    Port::Command(c) => commands.push(c),
                }
            }
        }
        state.commands = commands.iter().copied().filter(|c| !self.held_before.contains(c)).collect();
        self.held_before = commands;
        state
    }
}

fn names(keys: &str) -> Vec<String> {
    keys.split_whitespace().map(String::from).collect()
}

/// The name of a key in `touches.cfg`, from the name of a macroquad `KeyCode` (or a name already in that form):
/// the names of the former minifb window, so that existing files keep working.
fn key_name(key: &str) -> String {
    let renamed = match key {
        "LeftControl" => "LeftCtrl",
        "RightControl" => "RightCtrl",
        "GraveAccent" => "Backquote",
        "KpDecimal" => "NumPadDot",
        "KpDivide" => "NumPadSlash",
        "KpMultiply" => "NumPadAsterisk",
        "KpSubtract" => "NumPadMinus",
        "KpAdd" => "NumPadPlus",
        "KpEnter" => "NumPadEnter",
        _ => match key.strip_prefix("Kp") {
            Some(digit) if digit.len() == 1 => return format!("NumPad{digit}"),
            _ => key,
        },
    };
    renamed.to_string()
}

/// `Pad1Haut` → `Pad1Up`... (former French names of the gamepad directions).
fn french_pad_button(key: &str) -> Option<String> {
    let (pad, button) = key.split_at_checked(4)?;
    let english = match button {
        "Haut" => "Up",
        "Bas" => "Down",
        "Gauche" => "Left",
        "Droite" => "Right",
        _ => return None,
    };
    (pad.starts_with("Pad")).then(|| format!("{pad}{english}"))
}

/// Contents of the file: an explanation, then one line per action.
fn contents(actions: &[(Port, Vec<String>)]) -> String {
    let mut s = String::from(
        "# ddsom port keys: action = key names separated by spaces (empty = none).\n\
         # Keys: Left Right Up Down, LeftCtrl RightCtrl LeftAlt LeftShift Space Enter Tab Escape,\n\
         # A..Z, Key0..Key9, NumPad0..NumPad9, F1..F12... Letters and digits follow the keyboard layout.\n\
         # Gamepads: Pad1A Pad1B Pad1X Pad1Y Pad1LB Pad1RB Pad1Start Pad1Back,\n\
         # Pad1Up Pad1Down Pad1Left Pad1Right (d-pad or left stick); Pad2... for the second.\n\
         # DDSOM_TOUCHES=1 prints the name of the pressed keys and buttons.\n\n",
    );
    for ((name, ..), (_, keys)) in ACTIONS.iter().zip(actions) {
        s += &format!("{name} = {}\n", keys.join(" "));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::input::KeyCode;

    #[test]
    fn former_names_are_read_and_rewritten_in_english() {
        let dir = std::env::temp_dir().join(format!("ddsom_keys_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("touches.cfg");
        std::fs::write(&file, "j1.haut = Z Pad1Haut\nquitter = Escape Q\nscale = F4\n").unwrap();
        let keys = Keys::load(&file);
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("p1.up = Z Pad1Up\n"));
        assert!(text.contains("quit = Escape Q\n"));
        assert!(text.contains("scale = F4\n"));
        assert!(text.contains("p1.down = Down Pad1Down\n"));
        assert!(!text.contains("j1.") && !text.contains("Haut"));
        assert_eq!(keys.actions[3].1, ["Z", "Pad1Up"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn macroquad_key_names() {
        assert_eq!(key_name(&format!("{:?}", KeyCode::LeftControl)), "LeftCtrl");
        assert_eq!(key_name(&format!("{:?}", KeyCode::Kp7)), "NumPad7");
        assert_eq!(key_name(&format!("{:?}", KeyCode::Key7)), "Key7");
        assert_eq!(key_name(&format!("{:?}", KeyCode::F11)), "F11");
    }
}
