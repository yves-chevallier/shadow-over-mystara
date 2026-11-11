//! The game as a library (docs/fin.md, L20): one interface, no window and no output device.
//!
//! * [`Game::step`] runs one frame with the inputs of the players;
//! * [`Game::frame`] gives the frame to draw ([`DrawList`]: palette, scroll layers, sprites, and which frame of the
//!   frame table each group of sprites shows), for any renderer: the CPS2 renderer (`cps2_gfx::render`) draws the
//!   exact image of the hardware from it;
//! * [`Game::sound`] gives the sound of the frame, mixed (stereo samples at [`SAMPLE_RATE`]);
//! * [`Game::eeprom`] gives the EEPROM to keep (settings, counters) between two sessions.
//!
//! The application (window, keyboard, audio output, saves) talks to the game only through it. The state of the game
//! (work RAM, video and sound hardware, task table) is the one `validate` compares; the interface only reads it.

use cps2::{Cps2Bus, GameContents, Machine};

use crate::world::World;

pub use cps2::Inputs;
pub use cps2_gfx::draw::{DrawList, SpriteFrame};
pub use crate::core::frame::STOP;

/// Sample rate of [`Game::sound`] (QSound: 24 kHz).
pub const SAMPLE_RATE: f64 = qsound::SAMPLE_RATE;

/// The game, on the machine of a [`Board`]: the game itself ([`World`]), or the original code on the 68000
/// (`ddsom --features original`, a diagnostic: its sprites carry no frame).
pub struct Game<B: Board = World> {
    machine: Machine<B>,
    /// Frames of the object table latched at the last vblank (shown by the next frame), and of the one shown by the
    /// last frame: the frames follow their object table (`cps2::Machine::shown`).
    latched_frames: Vec<SpriteFrame>,
    shown_frames: Vec<SpriteFrame>,
}

/// What a game runs on: a CPS2 board whose sprite list may tell which frames it shows.
pub trait Board: cps2::Board {
    /// Frames shown by the object table that the hardware latches at vblank.
    fn latched_frames(&self) -> &[SpriteFrame];
}

impl Board for World {
    fn latched_frames(&self) -> &[SpriteFrame] {
        &self.sprite_frames[self.hw.objram_bank as usize & 1]
    }
}

impl Board for Cps2Bus {
    fn latched_frames(&self) -> &[SpriteFrame] {
        &[]
    }
}

impl Game {
    /// The game on its data (`extract::install_game` from the ROMs, `extract::install_game_assets` from the assets,
    /// which also install the typed game data), from a saved EEPROM (128 bytes) or a blank one.
    pub fn new(contents: GameContents, eeprom: Option<&[u8]>) -> Game {
        Game::on(crate::install_no_68k(Machine::game(contents, eeprom)))
    }
}

impl<B: Board> Game<B> {
    /// The game on a machine already built (the original code: `ddsom --features original`; the tools).
    pub fn on(mut machine: Machine<B>) -> Self {
        // the frame is drawn from `frame`, by the caller
        machine.rendering = false;
        Game { machine, latched_frames: Vec::new(), shown_frames: Vec::new() }
    }

    /// Runs one frame with `inputs` (the ports stay as given until the next step).
    pub fn step(&mut self, inputs: Inputs) {
        self.machine.bus.hw_mut().inputs = inputs;
        self.machine.sound.audio.clear();
        std::mem::swap(&mut self.shown_frames, &mut self.latched_frames);
        self.machine.run_frame();
        self.latched_frames.clear();
        self.latched_frames.extend_from_slice(self.machine.bus.latched_frames());
    }

    /// The frame shown by the last step.
    pub fn frame(&self) -> DrawList<'_> {
        DrawList::from_video(&self.machine.shown()).with_frames(&self.shown_frames)
    }

    /// The sound of the last step: stereo samples (left, right) at [`SAMPLE_RATE`].
    pub fn sound(&self) -> &[[i16; 2]] {
        &self.machine.sound.audio
    }

    /// Contents of the EEPROM (128 bytes), to give back to [`Game::new`].
    pub fn eeprom(&self) -> Vec<u8> {
        self.machine.bus.hw().eeprom.to_bytes()
    }

    /// Frames run since the start.
    pub fn frames(&self) -> u64 {
        self.machine.frame
    }

    /// The machine, for the tools (state hashes, RAM writes of the scenarios): an application does not need it.
    pub fn machine(&self) -> &Machine<B> {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut Machine<B> {
        &mut self.machine
    }
}
