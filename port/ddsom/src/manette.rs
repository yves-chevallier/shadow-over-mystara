//! Manettes XInput (Xbox) sous Windows : `xinput1_4.dll` chargée à l'exécution. Chaque bouton tenu donne un
//! nom utilisable dans `touches.cfg` : `Pad1A`, `Pad1B`, `Pad1X`, `Pad1Y`, `Pad1LB`, `Pad1RB`, `Pad1Start`,
//! `Pad1Back`, `Pad1Haut`/`Bas`/`Gauche`/`Droite` (croix ou stick gauche) ; `Pad2...` pour la deuxième.
//! Ailleurs (Linux, WSL), aucune manette.

pub struct Manettes {
    #[cfg(windows)]
    xinput: Option<xinput::XInput>,
}

impl Manettes {
    pub fn ouvre() -> Self {
        Manettes {
            #[cfg(windows)]
            xinput: xinput::XInput::ouvre(),
        }
    }

    /// Noms des boutons tenus sur toutes les manettes.
    pub fn tenus(&mut self) -> Vec<String> {
        #[cfg(windows)]
        if let Some(x) = &mut self.xinput {
            return x.tenus();
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

    const BOUTONS: [(u16, &str); 12] = [
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
    /// Zone morte du stick gauche (sur ±32767).
    const ZONE_MORTE: i16 = 12_000;

    pub struct XInput {
        _lib: libloading::Library,
        get_state: GetStateFn,
        /// Manette absente : prochaine interrogation (XInputGetState est lent sur une manette débranchée).
        absente: [Option<Instant>; 2],
    }

    impl XInput {
        pub fn ouvre() -> Option<Self> {
            // SAFETY : signature conforme à <xinput.h> ; la bibliothèque est gardée avec le pointeur.
            unsafe {
                let lib = libloading::Library::new("xinput1_4.dll")
                    .or_else(|_| libloading::Library::new("xinput9_1_0.dll"))
                    .ok()?;
                let get_state = *lib.get::<GetStateFn>(b"XInputGetState").ok()?;
                Some(XInput { _lib: lib, get_state, absente: [None; 2] })
            }
        }

        pub fn tenus(&mut self) -> Vec<String> {
            let mut noms = Vec::new();
            for i in 0..2 {
                if self.absente[i].is_some_and(|t| Instant::now() < t) {
                    continue;
                }
                let mut s = State::default();
                // SAFETY : `s` a la taille de XINPUT_STATE.
                if unsafe { (self.get_state)(i as u32, &mut s) } != 0 {
                    self.absente[i] = Some(Instant::now() + Duration::from_secs(1));
                    continue;
                }
                self.absente[i] = None;
                let mut b = s.buttons;
                b |= if s.ly > ZONE_MORTE { 0x0001 } else if s.ly < -ZONE_MORTE { 0x0002 } else { 0 };
                b |= if s.lx < -ZONE_MORTE { 0x0004 } else if s.lx > ZONE_MORTE { 0x0008 } else { 0 };
                for (bit, nom) in BOUTONS {
                    if b & bit != 0 {
                        noms.push(format!("Pad{}{nom}", i + 1));
                    }
                }
            }
            noms
        }
    }
}
