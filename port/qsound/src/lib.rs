//! Capcom QSound DL-1425 in high-level emulation.
//! Port of MAME src/devices/sound/qsoundhle.cpp (BSD-3-Clause, superctr, Valley Bell),
//! based on the disassembly of the DSP program. The integer arithmetic is reproduced exactly.

/// DSP word addresses of the tables of the DSP program (`DspTables`).
const DATA_PAN_TAB: u16 = 0x110;
const DATA_ADPCM_TAB: u16 = 0x9dc;
const DATA_FILTER_TAB: u16 = 0xd53;

/// The tables of the DSP program: name, DSP word address, size in words.
pub const DSP_TABLES: [(&str, u16, usize); 4] = [
    // pan gains: pan register (0x110 + position 0-0x20), +98 for the wet path, +196 for the right channel
    ("pan", DATA_PAN_TAB, 0x150),
    // ADPCM step size factors: 0x9dc + 8 + signed step (-8..7)
    ("adpcm", DATA_ADPCM_TAB, 16),
    // FIR filter coefficients up to the end of the 12-bit address space (95 per entry from 0xd53, and the two
    // filters of the second mode at 0xf73 and 0xfa4)
    ("filters", DATA_FILTER_TAB, 0x1000 - DATA_FILTER_TAB as usize),
    // the second filter at 0xfa4 overflows by 3 words and wraps to the start of the program
    ("filters_wrap", 0, 3),
];

const STATE_INIT2: u16 = 0x61a;
const STATE_REFRESH1: u16 = 0x039;
const STATE_REFRESH2: u16 = 0x04f;
const STATE_NORMAL1: u16 = 0x314;
const STATE_NORMAL2: u16 = 0x6b2;
const PAN_TABLE_DRY: u16 = 0;
const PAN_TABLE_WET: u16 = 98;
const PAN_TABLE_CH_OFFSET: u16 = 196;
const FILTER_ENTRY_SIZE: u16 = 95;
const DELAY_BASE_OFFSET: u16 = 0x554;
const DELAY_BASE_OFFSET2: u16 = 0x53c;

/// Default 60 MHz clock: sample rate = 60e6 / 2 / 1248.
pub const SAMPLE_RATE: f64 = 60_000_000.0 / 2.0 / 1248.0;

/// The registers of the 16 PCM voices, one array per register (so that the update of all the voices is
/// vectorised).
#[derive(Clone, Copy, Default)]
struct Voices {
    bank: [u16; 16],
    addr: [i16; 16],
    phase: [u16; 16],
    rate: [u16; 16],
    loop_len: [i16; 16],
    end_addr: [i16; 16],
    volume: [i16; 16],
    echo: [i16; 16],
}

#[derive(Clone, Copy, Default)]
struct Adpcm {
    start_addr: u16,
    end_addr: u16,
    bank: u16,
    volume: i16,
    flag: u16,
    cur_vol: i16,
    step_size: i16,
    cur_addr: u16,
}

#[derive(Clone, Copy)]
struct Fir {
    tap_count: i32,
    delay_pos: i32,
    table_pos: u16,
    taps: [i16; 95],
    delay_line: [i16; 95],
    /// The circular buffer of `apply` written twice in a row (`unrolled[k] = delay_line[k % m]` for `k < 2m`), so
    /// that the coefficients read it as one contiguous slice from `delay_pos`. Valid for `m = unrolled_len` only
    /// (0: to be rebuilt from `delay_line`).
    unrolled: [i16; 190],
    unrolled_len: usize,
}

impl Default for Fir {
    fn default() -> Self {
        Fir {
            tap_count: 0,
            delay_pos: 0,
            table_pos: 0,
            taps: [0; 95],
            delay_line: [0; 95],
            unrolled: [0; 190],
            unrolled_len: 0,
        }
    }
}

/// Sum of products (modulo 2^32 arithmetic, like the DSP: the order of the additions does not matter).
#[cfg(not(target_arch = "x86_64"))]
fn dot_product(a: &[i16], b: &[i16]) -> i32 {
    a.iter().zip(b).fold(0i32, |s, (&x, &y)| s.wrapping_add(i32::from(x) * i32::from(y)))
}

/// Sum of products (modulo 2^32 arithmetic, like the DSP: the order of the additions does not matter), by blocks of 8
/// with the SSE2 `pmaddwd` instruction (sums of pairs of products, exact modulo 2^32; SSE2 is part of x86_64, and the
/// compiler does not vectorise the plain loop by itself).
#[cfg(target_arch = "x86_64")]
fn dot_product(a: &[i16], b: &[i16]) -> i32 {
    use std::arch::x86_64::*;
    let (a8, a_rest) = a.as_chunks::<8>();
    let (b8, b_rest) = b.as_chunks::<8>();
    let rest = a_rest.iter().zip(b_rest).fold(0i32, |s, (&x, &y)| s.wrapping_add(i32::from(x) * i32::from(y)));
    // SAFETY: SSE2 is part of the x86_64 baseline; the intrinsics only compute on values (no memory access).
    unsafe {
        let load = |v: &[i16; 8]| _mm_set_epi16(v[7], v[6], v[5], v[4], v[3], v[2], v[1], v[0]);
        let mut sums = _mm_setzero_si128();
        for (x, y) in a8.iter().zip(b8) {
            sums = _mm_add_epi32(sums, _mm_madd_epi16(load(x), load(y)));
        }
        // the four lanes added together
        sums = _mm_add_epi32(sums, _mm_shuffle_epi32::<0b01_00_11_10>(sums));
        sums = _mm_add_epi32(sums, _mm_shuffle_epi32::<0b10_11_00_01>(sums));
        rest.wrapping_add(_mm_cvtsi128_si32(sums))
    }
}

impl Fir {
    fn apply(&mut self, input: i16) -> i32 {
        let m = self.tap_count - 1; // length of the circular buffer
        if m < 1 || self.delay_pos < 0 || self.delay_pos >= m {
            return self.apply_general(input);
        }
        // The first m coefficients read the buffer starting from delay_pos, walking it circularly,
        // the last one reads the input. delay_pos returns to its value, receives the input, then advances.
        let (m, p) = (m as usize, self.delay_pos as usize);
        if self.unrolled_len != m {
            for k in 0..2 * m {
                self.unrolled[k] = self.delay_line[k % m];
            }
            self.unrolled_len = m;
        }
        let mut sum = dot_product(&self.taps[..m], &self.unrolled[p..p + m]);
        sum = sum.wrapping_add(self.taps[m] as i32 * input as i32);
        self.delay_line[p] = input;
        self.unrolled[p] = input;
        self.unrolled[p + m] = input;
        self.delay_pos = if p + 1 >= m { 0 } else { p as i32 + 1 };
        0i32.wrapping_sub(sum << 2)
    }

    /// Original version, for abnormal states (uninitialized filter).
    fn apply_general(&mut self, input: i16) -> i32 {
        self.unrolled_len = 0;
        let mut output: i32 = 0;
        let mut tap = 0;
        while tap < self.tap_count - 1 {
            let p = (self.taps[tap as usize] as i32).wrapping_mul(self.delay_line[self.delay_pos as usize] as i32);
            output = output.wrapping_sub(p << 2);
            self.delay_pos += 1;
            if self.delay_pos >= self.tap_count - 1 {
                self.delay_pos = 0;
            }
            tap += 1;
        }
        let p = (self.taps[tap.max(0) as usize] as i32).wrapping_mul(input as i32);
        output = output.wrapping_sub(p << 2);
        self.delay_line[self.delay_pos.max(0) as usize] = input;
        self.delay_pos += 1;
        if self.delay_pos >= self.tap_count - 1 {
            self.delay_pos = 0;
        }
        output
    }
}

#[derive(Clone, Copy)]
struct Delay {
    delay: i16,
    volume: i16,
    write_pos: i16,
    read_pos: i16,
    line: [i16; 51],
}

impl Default for Delay {
    fn default() -> Self {
        Delay { delay: 0, volume: 0, write_pos: 0, read_pos: 0, line: [0; 51] }
    }
}

impl Delay {
    fn apply(&mut self, input: i32) -> i32 {
        self.line[self.write_pos as usize] = (input >> 16) as i16;
        self.write_pos += 1;
        if self.write_pos >= 51 {
            self.write_pos = 0;
        }
        let out = (self.line[self.read_pos as usize] as i32) * self.volume as i32;
        self.read_pos += 1;
        if self.read_pos >= 51 {
            self.read_pos = 0;
        }
        out
    }
    fn update(&mut self) {
        // C's %: remainder takes the sign of the dividend
        let n = ((self.write_pos as i32 - self.delay as i32) % 51) as i16;
        self.read_pos = if n < 0 { n + 51 } else { n };
    }
}

#[derive(Clone, Copy)]
struct Echo {
    end_pos: u16,
    feedback: i16,
    length: i16,
    last_sample: i16,
    line: [i16; 1024],
    pos: i16,
}

impl Default for Echo {
    fn default() -> Self {
        Echo { end_pos: 0, feedback: 0, length: 0, last_sample: 0, line: [0; 1024], pos: 0 }
    }
}

impl Echo {
    fn apply(&mut self, input: i32) -> i16 {
        let mut old = self.line[self.pos as usize] as i32;
        let last = self.last_sample as i32;
        self.last_sample = old as i16;
        old = (old + last) >> 1;
        let new = input.wrapping_add(old.wrapping_mul(self.feedback as i32) << 2);
        self.line[self.pos as usize] = (new >> 16) as i16;
        self.pos += 1;
        if self.pos >= self.length {
            self.pos = 0;
        }
        old as i16
    }
}

/// Register addressable by the Z80 (index in the 256-register table).
#[derive(Clone, Copy)]
enum Reg {
    VoiceBank(usize),
    VoiceAddr(usize),
    VoiceRate(usize),
    VoicePhase(usize),
    VoiceLoop(usize),
    VoiceEnd(usize),
    VoiceVolume(usize),
    VoiceEcho(usize),
    Pan(usize),
    AdpcmStart(usize),
    AdpcmEnd(usize),
    AdpcmBank(usize),
    AdpcmVolume(usize),
    AdpcmFlag(usize),
    EchoFeedback,
    EchoEnd,
    DelayUpdate,
    NextState,
    FilterTable(usize),
    AltFilterTable(usize),
    WetDelay(usize),
    DryDelay(usize),
    WetVolume(usize),
    DryVolume(usize),
}

fn register_map() -> [Option<Reg>; 256] {
    let mut m = [None; 256];
    for i in 0..16 {
        m[i << 3] = Some(Reg::VoiceBank((i + 1) % 16)); // the bank applies to the next voice
        m[(i << 3) + 1] = Some(Reg::VoiceAddr(i));
        m[(i << 3) + 2] = Some(Reg::VoiceRate(i));
        m[(i << 3) + 3] = Some(Reg::VoicePhase(i));
        m[(i << 3) + 4] = Some(Reg::VoiceLoop(i));
        m[(i << 3) + 5] = Some(Reg::VoiceEnd(i));
        m[(i << 3) + 6] = Some(Reg::VoiceVolume(i));
        m[i + 0x80] = Some(Reg::Pan(i));
        m[i + 0xba] = Some(Reg::VoiceEcho(i));
    }
    for i in 0..3 {
        m[(i << 2) + 0xca] = Some(Reg::AdpcmStart(i));
        m[(i << 2) + 0xcb] = Some(Reg::AdpcmEnd(i));
        m[(i << 2) + 0xcc] = Some(Reg::AdpcmBank(i));
        m[(i << 2) + 0xcd] = Some(Reg::AdpcmVolume(i));
        m[i + 0xd6] = Some(Reg::AdpcmFlag(i));
        m[i + 0x90] = Some(Reg::Pan(16 + i));
    }
    m[0x93] = Some(Reg::EchoFeedback);
    m[0xd9] = Some(Reg::EchoEnd);
    m[0xe2] = Some(Reg::DelayUpdate);
    m[0xe3] = Some(Reg::NextState);
    for i in 0..2 {
        m[(i << 1) + 0xda] = Some(Reg::FilterTable(i));
        m[(i << 1) + 0xde] = Some(Reg::WetDelay(i));
        m[(i << 1) + 0xe4] = Some(Reg::WetVolume(i));
        m[(i << 1) + 0xdb] = Some(Reg::AltFilterTable(i));
        m[(i << 1) + 0xdf] = Some(Reg::DryDelay(i));
        m[(i << 1) + 0xe5] = Some(Reg::DryVolume(i));
    }
    m
}

/// The tables of the DSP program (`dl-1425.bin`) that the high-level emulation reads; the rest of the program is code,
/// which the emulation replaces. The registers designate them by DSP word address.
#[derive(Clone)]
pub struct DspTables {
    pan: Vec<u16>,
    adpcm_steps: Vec<u16>,
    filters: Vec<u16>,
    filters_wrap: Vec<u16>,
}

impl DspTables {
    /// Reads the tables from the DSP program (little-endian words).
    pub fn from_program(program: &[u8]) -> Self {
        let word = |a: usize| u16::from_le_bytes([program[2 * a], program[2 * a + 1]]);
        let [pan, adpcm_steps, filters, filters_wrap] =
            DSP_TABLES.map(|(_, at, n)| (usize::from(at)..usize::from(at) + n).map(word).collect());
        DspTables { pan, adpcm_steps, filters, filters_wrap }
    }

    /// The tables of [`DSP_TABLES`] by name (`sound/qsound_dsp.json` of the assets): `table(name)` gives its words.
    /// A missing table, or one of the wrong length, is an error.
    pub fn from_tables<'a>(table: impl Fn(&str) -> Option<&'a [u16]>) -> Result<Self, String> {
        let mut words = DSP_TABLES.iter().map(|&(name, _, n)| match table(name) {
            Some(w) if w.len() == n => Ok(w.to_vec()),
            Some(w) => Err(format!("DSP table `{name}`: {} words instead of {n}", w.len())),
            None => Err(format!("DSP table `{name}` missing")),
        });
        let mut next = || words.next().unwrap();
        Ok(DspTables { pan: next()?, adpcm_steps: next()?, filters: next()?, filters_wrap: next()? })
    }

    /// Pan gain at DSP address `addr` (a pan register value, plus the wet and right channel offsets). The driver only
    /// writes pans within the table: elsewhere the DSP would read its code, here 0.
    fn pan_gain(&self, addr: u16) -> i16 {
        let i = usize::from(addr.wrapping_sub(DATA_PAN_TAB));
        self.pan.get(i).map_or(0, |&g| g as i16)
    }

    /// Step size factor of the ADPCM decoder for a signed 4-bit step.
    fn adpcm_step(&self, step: i8) -> u16 {
        self.adpcm_steps[(8 + step) as usize]
    }

    /// Filter coefficient at DSP address `addr` (a filter register value plus the tap number, on 12 bits). The driver
    /// only designates filters within the table: elsewhere the DSP would read its code, here 0.
    fn filter_tap(&self, addr: u16) -> i16 {
        let a = usize::from(addr & 0xfff);
        let tap = match a.checked_sub(usize::from(DATA_FILTER_TAB)) {
            Some(i) => self.filters.get(i),
            None => self.filters_wrap.get(a),
        };
        tap.map_or(0, |&t| t as i16)
    }
}

/// The four paths of a pan: left dry, left wet, right dry, right wet.
const PATHS: usize = 4;
/// The 19 voices (16 PCM, 3 ADPCM), padded with 5 silent ones to blocks of 8 for `dot_product`.
const MIXED: usize = 24;

/// The memory the voices play from: signed 8-bit PCM at the address `bank << 16 | address` written in the voice
/// registers. Padded with zeros to a power of two, so that an address is masked, never out of bounds (the hardware
/// reads zeros past the end).
struct SampleBank {
    bytes: Box<[u8]>,
}

impl SampleBank {
    fn new(mut bytes: Vec<u8>) -> Self {
        bytes.resize(bytes.len().next_power_of_two(), 0);
        SampleBank { bytes: bytes.into() }
    }

    /// Sample at (bank, address), as the high byte of a 16-bit value.
    #[inline]
    fn read(&self, bank: u16, address: u16) -> i16 {
        let a = ((bank & 0x7fff) as usize) << 16 | address as usize;
        ((self.bytes[a & (self.bytes.len() - 1)] as u16) << 8) as i16
    }
}

pub struct QSound {
    dsp: DspTables,
    samples: SampleBank,
    regs: [Option<Reg>; 256],
    data_latch: u16,
    /// Last stereo sample produced.
    pub out: [i16; 2],
    voice: Voices,
    adpcm: [Adpcm; 3],
    voice_pan: [u16; 19],
    /// Gains of the voices' pans on each path (`pan_gains`), read once when a pan is written.
    path_gains: [[i16; MIXED]; PATHS],
    /// Outputs of the voices (the padding stays 0).
    voice_output: [i16; MIXED],
    echo: Echo,
    filter: [Fir; 2],
    alt_filter: [Fir; 2],
    wet: [Delay; 2],
    dry: [Delay; 2],
    state: u16,
    next_state: u16,
    delay_update: u16,
    state_counter: i32,
    ready_flag: u8,
    /// Register write log (address, value), if enabled (analysis tools).
    pub log: Option<Vec<(u8, u16)>>,
}

impl QSound {
    /// `dsp_rom`: dl-1425.bin, or its tables alone (the first 0x2000 bytes, little-endian words).
    /// `samples`: sample memory (the sample ROMs, or the bank built from the WAV files).
    pub fn new(dsp_rom: &[u8], samples: Vec<u8>) -> Self {
        Self::with_tables(DspTables::from_program(dsp_rom), samples)
    }

    /// The emulation with the DSP tables alone (`DspTables`) and the sample memory.
    pub fn with_tables(dsp: DspTables, samples: Vec<u8>) -> Self {
        let mut q = QSound {
            dsp,
            samples: SampleBank::new(samples),
            regs: register_map(),
            data_latch: 0,
            out: [0; 2],
            voice: Voices::default(),
            adpcm: [Adpcm::default(); 3],
            voice_pan: [0; 19],
            path_gains: [[0; MIXED]; PATHS],
            voice_output: [0; MIXED],
            echo: Echo::default(),
            filter: [Fir::default(); 2],
            alt_filter: [Fir::default(); 2],
            wet: [Delay::default(); 2],
            dry: [Delay::default(); 2],
            state: 0,
            next_state: 0,
            delay_update: 0,
            state_counter: 0,
            ready_flag: 0,
            log: None,
        };
        q.reset();
        q
    }

    pub fn reset(&mut self) {
        self.ready_flag = 0;
        self.out = [0; 2];
        self.state = 0; // STATE_BOOT
        self.state_counter = 0;
    }

    fn read_sample(&self, bank: u16, address: u16) -> i16 {
        self.samples.read(bank, address)
    }

    /// Pan of voice `i`, and its gains.
    fn set_pan(&mut self, i: usize, pan: u16) {
        self.voice_pan[i] = pan;
        let path_gains = self.pan_gains(pan);
        for (gains, g) in self.path_gains.iter_mut().zip(path_gains) {
            gains[i] = g;
        }
    }

    /// Gains of a pan register value, in the order of `PATHS`.
    fn pan_gains(&self, pan: u16) -> [i16; PATHS] {
        let gain = |ch: u16, path: u16| self.dsp.pan_gain(pan.wrapping_add(ch * PAN_TABLE_CH_OFFSET).wrapping_add(path));
        [gain(0, PAN_TABLE_DRY), gain(0, PAN_TABLE_WET), gain(1, PAN_TABLE_DRY), gain(1, PAN_TABLE_WET)]
    }

    /// Z80 write at D000-D002.
    pub fn write(&mut self, offset: u8, data: u8) {
        match offset {
            0 => self.data_latch = (self.data_latch & 0x00ff) | (data as u16) << 8,
            1 => self.data_latch = (self.data_latch & 0xff00) | data as u16,
            2 => self.write_data(data, self.data_latch),
            _ => {}
        }
    }

    /// Z80 read at D007: 0x80 = ready.
    pub fn read(&self) -> u8 {
        self.ready_flag
    }

    fn write_data(&mut self, address: u8, d: u16) {
        if let Some(log) = self.log.as_mut() {
            log.push((address, d));
        }
        if let Some(r) = self.regs[address as usize] {
            let s = d as i16;
            match r {
                Reg::VoiceBank(i) => self.voice.bank[i] = d,
                Reg::VoiceAddr(i) => self.voice.addr[i] = s,
                Reg::VoiceRate(i) => self.voice.rate[i] = d,
                Reg::VoicePhase(i) => self.voice.phase[i] = d,
                Reg::VoiceLoop(i) => self.voice.loop_len[i] = s,
                Reg::VoiceEnd(i) => self.voice.end_addr[i] = s,
                Reg::VoiceVolume(i) => self.voice.volume[i] = s,
                Reg::VoiceEcho(i) => self.voice.echo[i] = s,
                Reg::Pan(i) => self.set_pan(i, d),
                Reg::AdpcmStart(i) => self.adpcm[i].start_addr = d,
                Reg::AdpcmEnd(i) => self.adpcm[i].end_addr = d,
                Reg::AdpcmBank(i) => self.adpcm[i].bank = d,
                Reg::AdpcmVolume(i) => self.adpcm[i].volume = s,
                Reg::AdpcmFlag(i) => self.adpcm[i].flag = d,
                Reg::EchoFeedback => self.echo.feedback = s,
                Reg::EchoEnd => self.echo.end_pos = d,
                Reg::DelayUpdate => self.delay_update = d,
                Reg::NextState => self.next_state = d,
                Reg::FilterTable(i) => self.filter[i].table_pos = d,
                Reg::AltFilterTable(i) => self.alt_filter[i].table_pos = d,
                Reg::WetDelay(i) => self.wet[i].delay = s,
                Reg::DryDelay(i) => self.dry[i].delay = s,
                Reg::WetVolume(i) => self.wet[i].volume = s,
                Reg::DryVolume(i) => self.dry[i].volume = s,
            }
        }
        self.ready_flag = 0;
    }

    /// Computes one sample (at SAMPLE_RATE).
    pub fn update_sample(&mut self) {
        match self.state {
            STATE_REFRESH1 => self.refresh_filter_1(),
            STATE_REFRESH2 => self.refresh_filter_2(),
            STATE_NORMAL1 | STATE_NORMAL2 => self.normal_update(),
            _ => self.init(),
        }
    }

    fn init(&mut self) {
        let mode2 = self.state == STATE_INIT2;
        if self.state_counter >= 2 {
            self.state_counter = 0;
            self.state = self.next_state;
            return;
        } else if self.state_counter == 1 {
            self.state_counter += 1;
            return;
        }
        self.voice = Voices::default();
        self.adpcm = [Adpcm::default(); 3];
        self.filter = [Fir::default(); 2];
        self.alt_filter = [Fir::default(); 2];
        self.wet = [Delay::default(); 2];
        self.dry = [Delay::default(); 2];
        self.echo = Echo::default();
        for i in 0..19 {
            self.set_pan(i, DATA_PAN_TAB + 0x10);
        }
        self.voice_output = [0; MIXED];
        self.voice.bank = [0x8000; 16];
        for a in self.adpcm.iter_mut() {
            a.bank = 0x8000;
        }
        if !mode2 {
            self.wet[0].delay = 0;
            self.dry[0].delay = 46;
            self.wet[1].delay = 0;
            self.dry[1].delay = 48;
            self.filter[0].table_pos = DATA_FILTER_TAB + FILTER_ENTRY_SIZE;
            self.filter[1].table_pos = DATA_FILTER_TAB + FILTER_ENTRY_SIZE * 2;
            self.echo.end_pos = DELAY_BASE_OFFSET + 6;
            self.next_state = STATE_REFRESH1;
        } else {
            self.wet[0].delay = 1;
            self.dry[0].delay = 0;
            self.wet[1].delay = 0;
            self.dry[1].delay = 0;
            self.filter[0].table_pos = 0xf73;
            self.filter[1].table_pos = 0xfa4;
            self.alt_filter[0].table_pos = 0xf73;
            self.alt_filter[1].table_pos = 0xfa4;
            self.echo.end_pos = DELAY_BASE_OFFSET2 + 6;
            self.next_state = STATE_REFRESH2;
        }
        for d in self.wet.iter_mut().chain(self.dry.iter_mut()) {
            d.volume = 0x3fff;
        }
        self.delay_update = 1;
        self.ready_flag = 0;
        self.state_counter = 1;
    }

    fn refresh_filter_1(&mut self) {
        for ch in 0..2 {
            self.filter[ch].delay_pos = 0;
            self.filter[ch].tap_count = 95;
            for i in 0..95 {
                self.filter[ch].taps[i] = self.dsp.filter_tap(self.filter[ch].table_pos.wrapping_add(i as u16));
            }
        }
        self.state = STATE_NORMAL1;
        self.next_state = STATE_NORMAL1;
    }

    fn refresh_filter_2(&mut self) {
        for ch in 0..2 {
            self.filter[ch].delay_pos = 0;
            self.filter[ch].tap_count = 45;
            for i in 0..45 {
                self.filter[ch].taps[i] = self.dsp.filter_tap(self.filter[ch].table_pos.wrapping_add(i as u16));
            }
            self.alt_filter[ch].delay_pos = 0;
            self.alt_filter[ch].tap_count = 44;
            for i in 0..44 {
                self.alt_filter[ch].taps[i] = self.dsp.filter_tap(self.alt_filter[ch].table_pos.wrapping_add(i as u16));
            }
        }
        self.state = STATE_NORMAL2;
        self.next_state = STATE_NORMAL2;
    }

    /// The 16 PCM voices for one sample: their outputs go to `voice_output[..16]`; returns the echo input.
    fn update_voices(&mut self) -> i32 {
        let v = &mut self.voice;
        // a muted voice reads its sample too (inside the bank, always): its output is null all the same
        let samples: [i16; 16] = std::array::from_fn(|i| self.samples.read(v.bank[i], v.addr[i] as u16));
        let mut echo_in = 0i32;
        let outputs = self.voice_output[..16].iter_mut().zip(samples).zip(v.volume).zip(v.echo);
        for (((out, sample), volume), echo) in outputs {
            let output = ((volume as i32 * sample as i32) >> 14) as i16;
            echo_in = echo_in.wrapping_add(((output as i32) * echo as i32) << 2);
            *out = output;
        }
        // the phase advances, muted or not (on whole arrays, so that it is vectorised)
        let wide = |a: [i16; 16]| a.map(i32::from);
        let (addr, end_addr, loop_len) = (wide(v.addr), wide(v.end_addr), wide(v.loop_len));
        let (rate, phase) = (v.rate.map(i32::from), v.phase.map(i32::from));
        for i in 0..16 {
            let mut new_phase = rate[i].wrapping_add((addr[i] << 12) | (phase[i] >> 4));
            let looped = if (new_phase >> 12) >= end_addr[i] { loop_len[i] << 12 } else { 0 };
            new_phase = new_phase.wrapping_sub(looped).clamp(-0x8000000, 0x7ffffff);
            v.addr[i] = (new_phase >> 12) as i16;
            v.phase[i] = ((new_phase << 4) & 0xffff) as u16;
        }
        echo_in
    }

    fn adpcm_update(&mut self, i: usize, mut curr_sample: i16, nibble: i32) -> i16 {
        let step: i8 = if nibble == 0 {
            let a = &mut self.adpcm[i];
            // voice muted at the end of the sample
            if a.cur_addr == a.end_addr {
                a.cur_vol = 0;
            }
            // start of playback
            if a.flag != 0 {
                curr_sample = 0;
                a.flag = 0;
                a.step_size = 10;
                a.cur_vol = a.volume;
                a.cur_addr = a.start_addr;
            }
            let a = self.adpcm[i];
            // quartet haut
            (self.read_sample(a.bank, a.cur_addr) >> 8) as i8
        } else {
            let a = self.adpcm[i];
            self.adpcm[i].cur_addr = a.cur_addr.wrapping_add(1);
            // quartet bas
            (self.read_sample(a.bank, a.cur_addr) >> 4) as i8
        };
        let step = step >> 4;
        let a = self.adpcm[i];
        let mut delta = ((1 + ((step as i32) << 1).abs()) * a.step_size as i32) >> 1;
        if step <= 0 {
            delta = -delta;
        }
        delta = (delta + curr_sample as i32).clamp(-32768, 32767);
        let tab = self.dsp.adpcm_step(step);
        let a = &mut self.adpcm[i];
        a.step_size = ((((tab as i32) * a.step_size as i32) >> 6) as i16).clamp(1, 2000);
        ((delta * a.cur_vol as i32) >> 16) as i16
    }

    fn normal_update(&mut self) {
        self.ready_flag = 0x80;
        let base = if self.state == STATE_NORMAL2 { DELAY_BASE_OFFSET2 } else { DELAY_BASE_OFFSET };
        self.echo.length = (self.echo.end_pos.wrapping_sub(base) as i16).clamp(0, 1024);

        let echo_input = self.update_voices();
        let av = (self.state_counter % 3) as usize;
        let prev = self.voice_output[16 + av];
        self.voice_output[16 + av] = self.adpcm_update(av, prev, self.state_counter / 3);

        let echo_output = self.echo.apply(echo_input) as i32;

        // the 19 voices through their pans (sums modulo 2^32: their order does not matter)
        let mut mix: [i32; PATHS] = [echo_output << 14, 0, 0, echo_output << 14];
        for (m, gains) in mix.iter_mut().zip(&self.path_gains) {
            *m = m.wrapping_sub(dot_product(&self.voice_output, gains));
        }
        for ch in 0..2 {
            let (mut dry, mut wet) = (mix[2 * ch], mix[2 * ch + 1]);
            dry = dry.clamp(-0x1fffffff, 0x1fffffff) << 2;
            wet = wet.clamp(-0x1fffffff, 0x1fffffff) << 2;
            wet = self.filter[ch].apply((wet >> 16) as i16);
            if self.state == STATE_NORMAL2 {
                dry = self.alt_filter[ch].apply((dry >> 16) as i16);
            }
            let mut output = self.wet[ch].apply(wet).wrapping_add(self.dry[ch].apply(dry));
            output = (output.wrapping_add(0x2000)) & !0x3fff;
            self.out[ch] = (output >> 14).clamp(-0x7fff, 0x7fff) as i16;
            if self.delay_update != 0 {
                self.wet[ch].update();
                self.dry[ch].update();
            }
        }
        self.delay_update = 0;
        self.state_counter += 1;
        if self.state_counter > 5 {
            self.state_counter = 0;
            self.state = self.next_state;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_product_wraps_like_the_dsp() {
        // extreme values (the pairs of -32768 * -32768 overflow an i32) and pseudo-random ones, every length
        let mut x = 0x1234_5678u32;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as i16
        };
        for len in 0..100 {
            for extreme in [false, true] {
                let a: Vec<i16> = (0..len).map(|_| if extreme { i16::MIN } else { next() }).collect();
                let b: Vec<i16> = (0..len).map(|_| if extreme { i16::MIN } else { next() }).collect();
                let plain = a.iter().zip(&b).fold(0i32, |s, (&x, &y)| s.wrapping_add(i32::from(x) * i32::from(y)));
                assert_eq!(dot_product(&a, &b), plain, "length {len}");
            }
        }
    }
}
