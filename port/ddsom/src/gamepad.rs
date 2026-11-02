//! XInput (Xbox) gamepads under Windows: `xinput1_4.dll` loaded at run time. Each held button gives a
//! name usable in `keys.cfg`: `Pad1A`, `Pad1B`, `Pad1X`, `Pad1Y`, `Pad1LB`, `Pad1RB`, `Pad1Start`,
//! `Pad1Back`, `Pad1Haut`/`Bas`/`Gauche`/`Droite` (d-pad or left stick); `Pad2...` for the second.
//! Elsewhere (Linux, WSL), no gamepad.

pub struct Gamepads {
    #[cfg(windows)]
    xinput: Option<xinput::XInput>,
}

impl Gamepads {
    pub fn opens() -> Self {
        Gamepads {
            #[cfg(windows)]
            xinput: xinput::XInput::opens(),
        }
    }

    /// Names of the buttons held on all gamepads.
    pub fn held(&mut self) -> Vec<String> {
        #[cfg(windows)]
        if let Some(x) = &mut self.xinput {
            return x.held();
        }
        Vec::new()
    }
}

#[cfg(windows)]
mod xinput {
    use std::time::{Duration, Instant};

    #[repr(C)]
    #[derive(Default)]
    struct State {
        packet: u32,
        buttons: u16,
        left_trigger: u8,
        right_trigger: u8,
        lx: i16,
        ly: i16,
        rx: i16,
        ry: i16,
    }

    type GetStateFn = unsafe extern "system" fn(index: u32, state: *mut State) -> u32;

    const BUTTONS: [(u16, &str); 12] = [
        (0x0001, "Haut"),
        (0x0002, "Bas"),
        (0x0004, "Gauche"),
        (0x0008, "Droite"),
        (0x0010, "Start"),
        (0x0020, "Back"),
        (0x0100, "LB"),
        (0x0200, "RB"),
        (0x1000, "A"),
        (0x2000, "B"),
        (0x4000, "X"),
        (0x8000, "Y"),
    ];
    /// Dead zone of the left stick (out of ±32767).
    const DEAD_ZONE: i16 = 12_000;

    pub struct XInput {
        _lib: libloading::Library,
        get_state: GetStateFn,
        /// Gamepad absent: next poll (XInputGetState is slow on an unplugged gamepad).
        absent: [Option<Instant>; 2],
    }

    impl XInput {
        pub fn opens() -> Option<Self> {
            // SAFETY: signature matches <xinput.h>; the library is kept with the pointer.
            unsafe {
                let lib = libloading::Library::new("xinput1_4.dll")
                    .or_else(|_| libloading::Library::new("xinput9_1_0.dll"))
                    .ok()?;
                let get_state = *lib.get::<GetStateFn>(b"XInputGetState").ok()?;
                Some(XInput { _lib: lib, get_state, absent: [None; 2] })
            }
        }

        pub fn held(&mut self) -> Vec<String> {
            let mut names = Vec::new();
            for i in 0..2 {
                if self.absent[i].is_some_and(|t| Instant::now() < t) {
                    continue;
                }
                let mut s = State::default();
            // SAFETY: `s` has the size of XINPUT_STATE.
                if unsafe { (self.get_state)(i as u32, &mut s) } != 0 {
                    self.absent[i] = Some(Instant::now() + Duration::from_secs(1));
                    continue;
                }
                self.absent[i] = None;
                let mut b = s.buttons;
                b |= if s.ly > DEAD_ZONE { 0x0001 } else if s.ly < -DEAD_ZONE { 0x0002 } else { 0 };
                b |= if s.lx < -DEAD_ZONE { 0x0004 } else if s.lx > DEAD_ZONE { 0x0008 } else { 0 };
                for (bit, name) in BUTTONS {
                    if b & bit != 0 {
                        names.push(format!("Pad{}{name}", i + 1));
                    }
                }
            }
            names
        }
    }
}
