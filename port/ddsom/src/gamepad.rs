//! Gamepads (gilrs: evdev under Linux, Windows.Gaming.Input / XInput under Windows, IOKit under macOS), in the
//! order they were connected. Each held button gives a name usable in `touches.cfg`, with the Xbox layout: `Pad1A`
//! (bottom), `Pad1B` (right), `Pad1X` (left), `Pad1Y` (top), `Pad1LB`, `Pad1RB`, `Pad1Start`, `Pad1Back`,
//! `Pad1Up`/`Down`/`Left`/`Right` (d-pad or left stick); `Pad2...` for the second. No gamepad on the web.

pub struct Gamepads {
    #[cfg(not(target_arch = "wasm32"))]
    gilrs: Option<gilrs::Gilrs>,
}

#[cfg(not(target_arch = "wasm32"))]
const BUTTONS: [(gilrs::Button, &str); 12] = {
    use gilrs::Button::*;
    [
        (DPadUp, "Up"),
        (DPadDown, "Down"),
        (DPadLeft, "Left"),
        (DPadRight, "Right"),
        (Start, "Start"),
        (Select, "Back"),
        (LeftTrigger, "LB"),
        (RightTrigger, "RB"),
        (South, "A"),
        (East, "B"),
        (West, "X"),
        (North, "Y"),
    ]
};

/// Dead zone of the left stick (out of ±1).
#[cfg(not(target_arch = "wasm32"))]
const DEAD_ZONE: f32 = 0.4;

impl Gamepads {
    pub fn open() -> Self {
        Gamepads {
            #[cfg(not(target_arch = "wasm32"))]
            gilrs: gilrs::Gilrs::new().map_err(|e| eprintln!("Gamepads unavailable: {e}")).ok(),
        }
    }

    /// Names of the buttons held on all gamepads.
    pub fn held(&mut self) -> Vec<String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(gilrs) = &mut self.gilrs {
            return held(gilrs);
        }
        Vec::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn held(gilrs: &mut gilrs::Gilrs) -> Vec<String> {
    use gilrs::Axis;
    // the events update the state of the gamepads
    while gilrs.next_event().is_some() {}
    let mut names = Vec::new();
    for (i, (_, pad)) in gilrs.gamepads().take(2).enumerate() {
        let (x, y) = (pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY));
        // the stick, as the four directions of the d-pad (the first four buttons)
        let stick = [y > DEAD_ZONE, y < -DEAD_ZONE, x < -DEAD_ZONE, x > DEAD_ZONE];
        for (j, (button, name)) in BUTTONS.iter().enumerate() {
            if pad.is_pressed(*button) || stick.get(j).copied().unwrap_or(false) {
                names.push(format!("Pad{}{name}", i + 1));
            }
        }
    }
    names
}
