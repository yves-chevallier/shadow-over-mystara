//! EEPROM série 93C46 en mode 16 bits (64 mots), d'après MAME machine/eepromser.cpp.
//! Les écritures sont instantanées (pas de délai de programmation).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    InReset,
    WaitForStartBit,
    WaitForCommand,
    ReadingData,
    WaitForData,
    WaitForCompletion,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Command {
    Read,
    Write,
    Erase,
    Lock,
    Unlock,
    WriteAll,
    EraseAll,
}

const ADDRESS_BITS: u32 = 6;
const DATA_BITS: u32 = 16;

#[derive(Clone)]
pub struct Eeprom93c46 {
    pub data: [u16; 64],
    state: State,
    locked: bool,
    cs: bool,
    clk: bool,
    di: bool,
    bits: u32,
    accum: u32,
    shift: u32,
    command: Command,
    address: u32,
}

impl Eeprom93c46 {
    pub fn new(data: [u16; 64]) -> Self {
        Eeprom93c46 {
            data,
            state: State::InReset,
            locked: true,
            cs: false,
            clk: false,
            di: false,
            bits: 0,
            accum: 0,
            shift: 0,
            command: Command::Read,
            address: 0,
        }
    }

    /// Contenu au format du fichier nvram de MAME (mots petit-boutistes, ordre natif x86).
    pub fn from_bytes(b: &[u8]) -> Self {
        let mut d = [0xffffu16; 64];
        for (i, w) in b.chunks_exact(2).take(64).enumerate() {
            d[i] = u16::from_le_bytes([w[0], w[1]]);
        }
        Self::new(d)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.data.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// Ligne DO (1 par défaut : sortie en haute impédance avec tirage).
    pub fn do_read(&self) -> bool {
        if self.state == State::ReadingData {
            self.shift & 0x8000_0000 != 0
        } else {
            true
        }
    }

    /// Écriture des lignes DI, CLK et CS (dans cet ordre, comme le port EEPROMOUT de MAME).
    pub fn write_lines(&mut self, di: bool, clk: bool, cs: bool) {
        self.di = di;
        if clk != self.clk {
            self.clk = clk;
            if clk {
                self.clock_rising();
            }
        }
        if cs != self.cs {
            self.cs = cs;
            if cs {
                if self.state == State::InReset {
                    self.state = State::WaitForStartBit;
                }
            } else {
                self.state = State::InReset;
            }
        }
    }

    fn clock_rising(&mut self) {
        match self.state {
            State::WaitForStartBit => {
                if self.di {
                    self.accum = 0;
                    self.bits = 0;
                    self.state = State::WaitForCommand;
                }
            }
            State::WaitForCommand => {
                self.accum = (self.accum << 1) | self.di as u32;
                self.bits += 1;
                if self.bits == 2 + ADDRESS_BITS {
                    self.execute_command();
                }
            }
            State::ReadingData => {
                let i = self.bits;
                self.bits += 1;
                if i % DATA_BITS == 0 && i == 0 {
                    let a = (self.address + self.bits / DATA_BITS) & ((1 << ADDRESS_BITS) - 1);
                    self.shift = (self.data[a as usize] as u32) << (32 - DATA_BITS);
                } else {
                    self.shift = (self.shift << 1) | 1;
                }
            }
            State::WaitForData => {
                self.shift = (self.shift << 1) | self.di as u32;
                self.bits += 1;
                if self.bits == DATA_BITS {
                    self.execute_write();
                }
            }
            State::InReset | State::WaitForCompletion => {}
        }
    }

    fn execute_command(&mut self) {
        self.address = self.accum & ((1 << ADDRESS_BITS) - 1);
        self.command = match self.accum >> ADDRESS_BITS {
            0 => {
                let c = match self.address >> (ADDRESS_BITS - 2) {
                    0 => Command::Lock,
                    1 => Command::WriteAll,
                    2 => Command::EraseAll,
                    _ => Command::Unlock,
                };
                self.address = 0;
                c
            }
            1 => Command::Write,
            2 => Command::Read,
            _ => Command::Erase,
        };
        self.bits = 0;
        match self.command {
            Command::Read => {
                self.shift = 0;
                self.state = State::ReadingData;
            }
            Command::Write | Command::WriteAll => {
                self.shift = 0;
                self.state = State::WaitForData;
            }
            Command::Erase => {
                if self.locked {
                    self.state = State::InReset;
                } else {
                    self.data[self.address as usize] = 0xffff;
                    self.state = State::WaitForCompletion;
                }
            }
            Command::EraseAll => {
                if self.locked {
                    self.state = State::InReset;
                } else {
                    self.data = [0xffff; 64];
                    self.state = State::WaitForCompletion;
                }
            }
            Command::Lock => {
                self.locked = true;
                self.state = State::InReset;
            }
            Command::Unlock => {
                self.locked = false;
                self.state = State::InReset;
            }
        }
    }

    fn execute_write(&mut self) {
        if self.locked {
            self.state = State::InReset;
            return;
        }
        let v = self.shift as u16;
        match self.command {
            Command::Write => self.data[self.address as usize] = v,
            // WRITEALL suppose un ERASEALL préalable : ET avec le contenu
            _ => self.data.iter_mut().for_each(|d| *d &= v),
        }
        self.state = State::WaitForCompletion;
    }
}
