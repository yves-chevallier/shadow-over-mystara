//! Sortie audio : PulseAudio sous Linux, waveOut sous Windows (bibliothèques chargées dynamiquement : aucune
//! dépendance de compilation). Un fil dédié écrit les échantillons ; sans sortie, le jeu tourne sans son.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

pub struct Audio {
    tx: SyncSender<Vec<i16>>,
}

impl Audio {
    /// Ouvre un flux stéréo 16 bits à `rate` Hz. None sans sortie audio.
    pub fn open(rate: u32) -> Option<Self> {
        let (tx, rx) = sync_channel::<Vec<i16>>(8);
        let (ok_tx, ok_rx) = sync_channel::<bool>(1);
        std::thread::spawn(move || sortie::audio_thread(rate, rx, ok_tx));
        if ok_rx.recv().unwrap_or(false) { Some(Audio { tx }) } else { None }
    }

    /// Envoie un bloc d'échantillons entrelacés ; abandonné si le tampon est plein (retard).
    pub fn push(&self, samples: Vec<i16>) {
        match self.tx.try_send(samples) {
            Ok(()) | Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

#[cfg(unix)]
mod sortie {
    use super::*;
    use std::ffi::{c_char, c_int, c_void, CString};

    #[repr(C)]
    struct SampleSpec {
        format: c_int, // PA_SAMPLE_S16LE = 3
        rate: u32,
        channels: u8,
    }

    type NewFn = unsafe extern "C" fn(
        server: *const c_char,
        name: *const c_char,
        dir: c_int,
        dev: *const c_char,
        stream_name: *const c_char,
        ss: *const SampleSpec,
        map: *const c_void,
        attr: *const c_void,
        error: *mut c_int,
    ) -> *mut c_void;
    type WriteFn = unsafe extern "C" fn(s: *mut c_void, data: *const c_void, bytes: usize, error: *mut c_int) -> c_int;

    pub fn audio_thread(rate: u32, rx: Receiver<Vec<i16>>, ok: SyncSender<bool>) {
        // SAFETY : signatures conformes à <pulse/simple.h> ; la bibliothèque reste chargée tout le fil.
        unsafe {
            let Ok(lib) = libloading::Library::new("libpulse-simple.so.0") else {
                let _ = ok.send(false);
                return;
            };
            let (Ok(new), Ok(write)) = (lib.get::<NewFn>(b"pa_simple_new"), lib.get::<WriteFn>(b"pa_simple_write")) else {
                let _ = ok.send(false);
                return;
            };
            let name = CString::new("ddsom").unwrap();
            let stream = CString::new("jeu").unwrap();
            let spec = SampleSpec { format: 3, rate, channels: 2 };
            let mut err = 0;
            let s = new(std::ptr::null(), name.as_ptr(), 1, std::ptr::null(), stream.as_ptr(), &spec,
                        std::ptr::null(), std::ptr::null(), &mut err);
            if s.is_null() {
                let _ = ok.send(false);
                return;
            }
            let _ = ok.send(true);
            while let Ok(buf) = rx.recv() {
                if write(s, buf.as_ptr() as *const c_void, buf.len() * 2, &mut err) < 0 {
                    break;
                }
            }
        }
    }
}

#[cfg(windows)]
mod sortie {
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

    /// Blocs en file dans le périphérique (un par image, environ 17 ms chacun).
    const BLOCS: usize = 4;

    pub fn audio_thread(rate: u32, rx: Receiver<Vec<i16>>, ok: SyncSender<bool>) {
        // SAFETY : signatures conformes à <mmeapi.h> ; la bibliothèque reste chargée tout le fil, et chaque en-tête
        // et ses échantillons restent en place tant que le périphérique ne l'a pas rendu (WHDR_DONE).
        unsafe {
            let Ok(lib) = libloading::Library::new("winmm.dll") else {
                let _ = ok.send(false);
                return;
            };
            let (Ok(open), Ok(prepare), Ok(write), Ok(unprepare)) = (
                lib.get::<OpenFn>(b"waveOutOpen"),
                lib.get::<HdrFn>(b"waveOutPrepareHeader"),
                lib.get::<HdrFn>(b"waveOutWrite"),
                lib.get::<HdrFn>(b"waveOutUnprepareHeader"),
            ) else {
                let _ = ok.send(false);
                return;
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
                let _ = ok.send(false);
                return;
            }
            let _ = ok.send(true);
            let size = std::mem::size_of::<WaveHdr>() as u32;
            let mut hdrs: Box<[WaveHdr]> = (0..BLOCS)
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
            let mut bufs: Vec<Vec<i16>> = vec![Vec::new(); BLOCS];
            let mut i = 0;
            while let Ok(buf) = rx.recv() {
                let h = &mut hdrs[i] as *mut WaveHdr;
                while std::ptr::read_volatile(&(*h).flags) & 1 == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                unprepare(dev, h, size);
                bufs[i] = buf;
                (*h).data = bufs[i].as_mut_ptr() as *mut c_void;
                (*h).len = (bufs[i].len() * 2) as u32;
                (*h).flags = 0;
                if prepare(dev, h, size) != 0 || write(dev, h, size) != 0 {
                    break;
                }
                i = (i + 1) % BLOCS;
            }
        }
    }
}
