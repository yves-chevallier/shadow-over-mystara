//! Sound output. The game gives its samples frame by frame (stereo, `SAMPLE_RATE`, about 24 038 Hz) to a queue; an
//! output thread takes them at the device rate (48 kHz), resampled linearly. The game is paced by the clock of the
//! computer, the device by its own: the resampling ratio is nudged (at most 0.5 %, inaudible) by the queue's fill,
//! so that the latency stays near `TARGET` instead of drifting. The queue starts (and restarts after running dry)
//! once it holds `TARGET` of sound (the latency, plus about 30 ms in the device).
//! Devices: PulseAudio under Linux, waveOut under Windows (libraries loaded at run time: no build dependency;
//! macroquad's own audio only plays whole sounds). Without output, the game runs without sound.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ddsom_game::core::interface::SAMPLE_RATE;

/// Rate of the device.
const OUTPUT_RATE: u32 = 48_000;
/// Sound kept ahead in the queue, in seconds.
const TARGET: f64 = 0.04;
/// Beyond this (after a stall), the queue is cut back to `TARGET`.
const LIMIT: f64 = 0.2;
/// Largest change of the resampling ratio.
const NUDGE: f64 = 0.005;

pub struct Audio {
    queue: Arc<Mutex<Resampler>>,
}

impl Audio {
    /// Opens the sound output, in its thread: the game never waits for the device (a sound server can take long to
    /// answer); without one, the game runs without sound.
    pub fn open() -> Self {
        let queue = Arc::new(Mutex::new(Resampler::new()));
        let output = queue.clone();
        std::thread::spawn(move || {
            if let Err(e) = device::output_thread(OUTPUT_RATE, output) {
                eprintln!("Sound unavailable ({e}): the game runs without sound.");
            }
        });
        Audio { queue }
    }

    /// Queues the sound of one frame.
    pub fn push(&self, samples: &[[i16; 2]]) {
        let mut q = self.queue.lock().unwrap();
        q.input.extend(samples);
        let (target, limit) = (seconds(TARGET), seconds(LIMIT));
        if q.input.len() > limit {
            let excess = q.input.len() - target;
            q.input.drain(..excess);
        }
    }
}

/// Number of game samples in `s` seconds.
fn seconds(s: f64) -> usize {
    (s * SAMPLE_RATE) as usize
}

/// Game samples not yet played, and the position of the next output sample between the first two.
struct Resampler {
    input: VecDeque<[i16; 2]>,
    position: f64,
    /// Waiting for `TARGET` of sound (at the start, or after running dry).
    priming: bool,
}

impl Resampler {
    fn new() -> Self {
        Resampler { input: VecDeque::new(), position: 0.0, priming: true }
    }

    /// Fills `out` (interleaved stereo at `rate`); silence while the queue is priming.
    fn fill(&mut self, out: &mut [i16], rate: u32) {
        let target = seconds(TARGET);
        self.priming &= self.input.len() < target;
        let fullness = (self.input.len() as f64 - target as f64) / target as f64;
        let step = SAMPLE_RATE / rate as f64 * (1.0 + NUDGE * fullness.clamp(-1.0, 1.0));
        for frame in out.as_chunks_mut::<2>().0 {
            if self.priming || self.input.len() < 2 {
                self.priming = true;
                frame.fill(0);
                continue;
            }
            let (a, b, t) = (self.input[0], self.input[1], self.position);
            for c in 0..2 {
                frame[c] = (a[c] as f64 + (b[c] as f64 - a[c] as f64) * t).round() as i16;
            }
            self.position += step;
            let whole = self.position as usize;
            self.input.drain(..whole.min(self.input.len()));
            self.position -= whole as f64;
        }
    }
}

#[cfg(unix)]
mod device {
    use super::*;
    use std::ffi::{c_char, c_int, c_void, CString};

    #[repr(C)]
    struct SampleSpec {
        format: c_int, // PA_SAMPLE_S16LE = 3
        rate: u32,
        channels: u8,
    }

    /// `pa_buffer_attr` (u32::MAX: the server's choice).
    #[repr(C)]
    struct BufferAttr {
        maxlength: u32,
        tlength: u32,
        prebuf: u32,
        minreq: u32,
        fragsize: u32,
    }

    type NewFn = unsafe extern "C" fn(
        server: *const c_char,
        name: *const c_char,
        dir: c_int,
        dev: *const c_char,
        stream_name: *const c_char,
        ss: *const SampleSpec,
        map: *const c_void,
        attr: *const BufferAttr,
        error: *mut c_int,
    ) -> *mut c_void;
    type WriteFn = unsafe extern "C" fn(s: *mut c_void, data: *const c_void, bytes: usize, error: *mut c_int) -> c_int;

    /// Samples written at once (10 ms), and sound held by the server (bytes: 30 ms).
    const CHUNK: usize = 480;
    const SERVER_BYTES: u32 = OUTPUT_RATE * 4 * 30 / 1000;

    pub fn output_thread(rate: u32, queue: Arc<Mutex<Resampler>>) -> Result<(), &'static str> {
        // SAFETY: signatures match <pulse/simple.h>; the library stays loaded for the whole thread.
        unsafe {
            let lib = libloading::Library::new("libpulse-simple.so.0").map_err(|_| "no libpulse-simple.so.0")?;
            let (Ok(new), Ok(write)) = (lib.get::<NewFn>(b"pa_simple_new"), lib.get::<WriteFn>(b"pa_simple_write")) else {
                return Err("libpulse-simple.so.0 incomplete");
            };
            let name = CString::new("ddsom").unwrap();
            let stream = CString::new("game").unwrap();
            let spec = SampleSpec { format: 3, rate, channels: 2 };
            let attr =
                BufferAttr { maxlength: u32::MAX, tlength: SERVER_BYTES, prebuf: u32::MAX, minreq: u32::MAX, fragsize: u32::MAX };
            let mut err = 0;
            let s = new(std::ptr::null(), name.as_ptr(), 1, std::ptr::null(), stream.as_ptr(), &spec,
                        std::ptr::null(), &attr, &mut err);
            if s.is_null() {
                return Err("no PulseAudio server");
            }
            let mut buf = vec![0i16; CHUNK * 2];
            loop {
                queue.lock().unwrap().fill(&mut buf, rate);
                // blocks while the server holds enough: this paces the thread
                if write(s, buf.as_ptr() as *const c_void, buf.len() * 2, &mut err) < 0 {
                    return Err("PulseAudio stream lost");
                }
            }
        }
    }
}

#[cfg(windows)]
mod device {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    struct WaveFormat {
        tag: u16, // WAVE_FORMAT_PCM = 1
        channels: u16,
        rate: u32,
        bytes_per_sec: u32,
        block_align: u16,
        bits: u16,
        extra: u16,
    }

    #[repr(C)]
    struct WaveHdr {
        data: *mut c_void,
        len: u32,
        recorded: u32,
        user: usize,
        flags: u32, // WHDR_DONE = 1
        loops: u32,
        next: *mut WaveHdr,
        reserved: usize,
    }

    type OpenFn = unsafe extern "system" fn(*mut *mut c_void, u32, *const WaveFormat, usize, usize, u32) -> u32;
    type HdrFn = unsafe extern "system" fn(*mut c_void, *mut WaveHdr, u32) -> u32;

    /// Blocks queued in the device, of `CHUNK` samples (20 ms) each.
    const BLOCKS: usize = 3;
    const CHUNK: usize = 960;

    pub fn output_thread(rate: u32, queue: Arc<Mutex<Resampler>>) -> Result<(), &'static str> {
        // SAFETY: signatures match <mmeapi.h>; the library stays loaded for the whole thread, and each header
        // and its samples stay in place until the device has given it back (WHDR_DONE).
        unsafe {
            let lib = libloading::Library::new("winmm.dll").map_err(|_| "no winmm.dll")?;
            let (Ok(open), Ok(prepare), Ok(write), Ok(unprepare)) = (
                lib.get::<OpenFn>(b"waveOutOpen"),
                lib.get::<HdrFn>(b"waveOutPrepareHeader"),
                lib.get::<HdrFn>(b"waveOutWrite"),
                lib.get::<HdrFn>(b"waveOutUnprepareHeader"),
            ) else {
                return Err("winmm.dll incomplete");
            };
            let fmt = WaveFormat {
                tag: 1,
                channels: 2,
                rate,
                bytes_per_sec: rate * 4,
                block_align: 4,
                bits: 16,
                extra: 0,
            };
            let mut dev = std::ptr::null_mut();
            if open(&mut dev, u32::MAX /* WAVE_MAPPER */, &fmt, 0, 0, 0) != 0 {
                return Err("no waveOut device");
            }
            let size = std::mem::size_of::<WaveHdr>() as u32;
            let mut hdrs: Box<[WaveHdr]> = (0..BLOCKS)
                .map(|_| WaveHdr {
                    data: std::ptr::null_mut(),
                    len: 0,
                    recorded: 0,
                    user: 0,
                    flags: 1,
                    loops: 0,
                    next: std::ptr::null_mut(),
                    reserved: 0,
                })
                .collect();
            let mut bufs: Vec<Vec<i16>> = vec![vec![0; CHUNK * 2]; BLOCKS];
            for i in (0..BLOCKS).cycle() {
                let h = &mut hdrs[i] as *mut WaveHdr;
                while std::ptr::read_volatile(&(*h).flags) & 1 == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                unprepare(dev, h, size);
                queue.lock().unwrap().fill(&mut bufs[i], rate);
                (*h).data = bufs[i].as_mut_ptr() as *mut c_void;
                (*h).len = (bufs[i].len() * 2) as u32;
                (*h).flags = 0;
                if prepare(dev, h, size) != 0 || write(dev, h, size) != 0 {
                    return Err("waveOut device lost");
                }
            }
        }
        unreachable!("endless cycle")
    }
}

/// No sound output on the web (yet) nor elsewhere.
#[cfg(not(any(unix, windows)))]
mod device {
    use super::*;

    pub fn output_thread(_rate: u32, _queue: Arc<Mutex<Resampler>>) -> Result<(), &'static str> {
        Err("no sound output on this platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A steady game keeps the queue near `TARGET`, whatever the small difference of the two clocks.
    #[test]
    fn latency_stays_near_the_target() {
        for clock in [0.998, 1.0, 1.002] {
            let mut r = Resampler::new();
            let frame = vec![[1000i16, -1000]; (SAMPLE_RATE / 59.637) as usize];
            let mut out = vec![0i16; 480 * 2];
            let mut produced = 0.0;
            for chunk in 0..6000 {
                // 10 ms of output per chunk; the game produces at its own (slightly different) clock
                while produced < (chunk as f64 + 1.0) * 0.01 * clock {
                    r.input.extend(&frame);
                    produced += 1.0 / 59.637;
                }
                r.fill(&mut out, OUTPUT_RATE);
            }
            let fill = r.input.len() as f64 / SAMPLE_RATE;
            assert!((fill - TARGET).abs() < 0.03, "clock {clock}: {fill} s queued");
            assert!(out.chunks(2).all(|f| f == [1000, -1000]));
        }
    }
}
