//! Capcom QSound DL-1425 en émulation haut niveau.
//! Port de MAME src/devices/sound/qsoundhle.cpp (BSD-3-Clause, superctr, Valley Bell),
//! fondé sur le désassemblage du programme du DSP. L'arithmétique entière est reproduite à l'identique.

const DATA_PAN_TAB: u16 = 0x110;
const DATA_ADPCM_TAB: u16 = 0x9dc;
const DATA_FILTER_TAB: u16 = 0xd53;
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

/// Horloge par défaut 60 MHz : fréquence d'échantillonnage = 60e6 / 2 / 1248.
pub const SAMPLE_RATE: f64 = 60_000_000.0 / 2.0 / 1248.0;

#[derive(Clone, Copy, Default)]
struct Voice {
    bank: u16,
    addr: i16,
    phase: u16,
    rate: u16,
    loop_len: i16,
    end_addr: i16,
    volume: i16,
    echo: i16,
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
}

impl Default for Fir {
    fn default() -> Self {
        Fir { tap_count: 0, delay_pos: 0, table_pos: 0, taps: [0; 95], delay_line: [0; 95] }
    }
}

impl Fir {
    fn apply(&mut self, input: i16) -> i32 {
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
        // % du C : reste du signe du dividende
        let n = ((self.write_pos as i32 - self.delay as i32) % 51) as i16;
        self.read_pos = if n < 0 { n + 51 } else { n };
    }
}

#[derive(Clone)]
struct Echo {
    end_pos: u16,
    feedback: i16,
    length: i16,
    last_sample: i16,
    line: Vec<i16>,
    pos: i16,
}

impl Default for Echo {
    fn default() -> Self {
        Echo { end_pos: 0, feedback: 0, length: 0, last_sample: 0, line: vec![0; 1024], pos: 0 }
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

/// Registre adressable par le Z80 (index dans la table de 256 registres).
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
        m[i << 3] = Some(Reg::VoiceBank((i + 1) % 16)); // la banque s'applique à la voie suivante
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

pub struct QSound {
    dsp_rom: Vec<u16>,
    samples: Vec<u8>,
    sample_mask: usize,
    regs: [Option<Reg>; 256],
    data_latch: u16,
    /// Dernier échantillon stéréo produit.
    pub out: [i16; 2],
    voice: [Voice; 16],
    adpcm: [Adpcm; 3],
    voice_pan: [u16; 19],
    voice_output: [i16; 19],
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
}

impl QSound {
    /// `dsp_rom` : dl-1425.bin (seuls les 0x2000 premiers octets, mots petit-boutistes).
    /// `samples` : ROM d'échantillons (taille puissance de 2).
    pub fn new(dsp_rom: &[u8], samples: Vec<u8>) -> Self {
        let rom = dsp_rom[..0x2000].chunks_exact(2).map(|w| u16::from_le_bytes([w[0], w[1]])).collect();
        let sample_mask = samples.len().next_power_of_two() - 1;
        let mut q = QSound {
            dsp_rom: rom,
            samples,
            sample_mask,
            regs: register_map(),
            data_latch: 0,
            out: [0; 2],
            voice: [Voice::default(); 16],
            adpcm: [Adpcm::default(); 3],
            voice_pan: [0; 19],
            voice_output: [0; 19],
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

    fn dsp(&self, addr: u16) -> u16 {
        self.dsp_rom[(addr & 0xfff) as usize]
    }

    fn read_sample(&self, bank: u16, address: u16) -> i16 {
        let a = (((bank & 0x7fff) as usize) << 16 | address as usize) & self.sample_mask;
        let b = *self.samples.get(a).unwrap_or(&0);
        ((b as u16) << 8) as i16
    }

    /// Écriture Z80 en D000-D002.
    pub fn write(&mut self, offset: u8, data: u8) {
        match offset {
            0 => self.data_latch = (self.data_latch & 0x00ff) | (data as u16) << 8,
            1 => self.data_latch = (self.data_latch & 0xff00) | data as u16,
            2 => self.write_data(data, self.data_latch),
            _ => {}
        }
    }

    /// Lecture Z80 en D007 : 0x80 = prêt.
    pub fn read(&self) -> u8 {
        self.ready_flag
    }

    fn write_data(&mut self, address: u8, d: u16) {
        if let Some(r) = self.regs[address as usize] {
            let s = d as i16;
            match r {
                Reg::VoiceBank(i) => self.voice[i].bank = d,
                Reg::VoiceAddr(i) => self.voice[i].addr = s,
                Reg::VoiceRate(i) => self.voice[i].rate = d,
                Reg::VoicePhase(i) => self.voice[i].phase = d,
                Reg::VoiceLoop(i) => self.voice[i].loop_len = s,
                Reg::VoiceEnd(i) => self.voice[i].end_addr = s,
                Reg::VoiceVolume(i) => self.voice[i].volume = s,
                Reg::VoiceEcho(i) => self.voice[i].echo = s,
                Reg::Pan(i) => self.voice_pan[i] = d,
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

    /// Calcule un échantillon (à SAMPLE_RATE).
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
        self.voice = [Voice::default(); 16];
        self.adpcm = [Adpcm::default(); 3];
        self.filter = [Fir::default(); 2];
        self.alt_filter = [Fir::default(); 2];
        self.wet = [Delay::default(); 2];
        self.dry = [Delay::default(); 2];
        self.echo = Echo::default();
        self.voice_pan = [DATA_PAN_TAB + 0x10; 19];
        self.voice_output = [0; 19];
        for v in self.voice.iter_mut() {
            v.bank = 0x8000;
        }
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
                self.filter[ch].taps[i] = self.dsp(self.filter[ch].table_pos.wrapping_add(i as u16)) as i16;
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
                self.filter[ch].taps[i] = self.dsp(self.filter[ch].table_pos.wrapping_add(i as u16)) as i16;
            }
            self.alt_filter[ch].delay_pos = 0;
            self.alt_filter[ch].tap_count = 44;
            for i in 0..44 {
                self.alt_filter[ch].taps[i] = self.dsp(self.alt_filter[ch].table_pos.wrapping_add(i as u16)) as i16;
            }
        }
        self.state = STATE_NORMAL2;
        self.next_state = STATE_NORMAL2;
    }

    fn voice_update(&mut self, i: usize, echo_out: &mut i32) -> i16 {
        let v = self.voice[i];
        let output = ((v.volume as i32 * self.read_sample(v.bank, v.addr as u16) as i32) >> 14) as i16;
        *echo_out = echo_out.wrapping_add(((output as i32) * v.echo as i32) << 2);
        let mut new_phase = (v.rate as i32).wrapping_add(((v.addr as i32) << 12) | (v.phase >> 4) as i32);
        if (new_phase >> 12) >= v.end_addr as i32 {
            new_phase = new_phase.wrapping_sub((v.loop_len as i32) << 12);
        }
        new_phase = new_phase.clamp(-0x8000000, 0x7ffffff);
        let vm = &mut self.voice[i];
        vm.addr = (new_phase >> 12) as i16;
        vm.phase = ((new_phase << 4) & 0xffff) as u16;
        output
    }

    fn adpcm_update(&mut self, i: usize, mut curr_sample: i16, nibble: i32) -> i16 {
        let step: i8 = if nibble == 0 {
            let a = &mut self.adpcm[i];
            // voie muette en fin d'échantillon
            if a.cur_addr == a.end_addr {
                a.cur_vol = 0;
            }
            // démarrage de la lecture
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
        let tab = self.dsp((DATA_ADPCM_TAB as i32 + 8 + step as i32) as u16);
        let a = &mut self.adpcm[i];
        a.step_size = ((((tab as i32) * a.step_size as i32) >> 6) as i16).clamp(1, 2000);
        ((delta * a.cur_vol as i32) >> 16) as i16
    }

    fn normal_update(&mut self) {
        self.ready_flag = 0x80;
        let base = if self.state == STATE_NORMAL2 { DELAY_BASE_OFFSET2 } else { DELAY_BASE_OFFSET };
        self.echo.length = (self.echo.end_pos.wrapping_sub(base) as i16).clamp(0, 1024);

        let mut echo_input = 0i32;
        for i in 0..16 {
            self.voice_output[i] = self.voice_update(i, &mut echo_input);
        }
        let av = (self.state_counter % 3) as usize;
        let prev = self.voice_output[16 + av];
        self.voice_output[16 + av] = self.adpcm_update(av, prev, self.state_counter / 3);

        let echo_output = self.echo.apply(echo_input) as i32;

        for ch in 0..2 {
            let mut wet: i32 = if ch == 1 { echo_output << 14 } else { 0 };
            let mut dry: i32 = if ch == 0 { echo_output << 14 } else { 0 };
            for i in 0..19 {
                let pan = self.voice_pan[i].wrapping_add(ch as u16 * PAN_TABLE_CH_OFFSET);
                let vo = self.voice_output[i] as i32;
                dry = dry.wrapping_sub(vo * self.dsp(pan.wrapping_add(PAN_TABLE_DRY)) as i16 as i32);
                wet = wet.wrapping_sub(vo * self.dsp(pan.wrapping_add(PAN_TABLE_WET)) as i16 as i32);
            }
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
