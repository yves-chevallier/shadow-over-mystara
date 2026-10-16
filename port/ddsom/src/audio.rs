//! Sortie audio par PulseAudio (libpulse-simple chargée dynamiquement : aucune dépendance de compilation).
//! Un fil dédié écrit les échantillons ; s'il n'y a pas de serveur, le jeu tourne sans son.

use std::ffi::{c_char, c_int, c_void, CString};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

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

pub struct Audio {
    tx: SyncSender<Vec<i16>>,
}

impl Audio {
    /// Ouvre un flux stéréo 16 bits à `rate` Hz. None si PulseAudio est indisponible.
    pub fn open(rate: u32) -> Option<Self> {
        let (tx, rx) = sync_channel::<Vec<i16>>(8);
        let (ok_tx, ok_rx) = sync_channel::<bool>(1);
        std::thread::spawn(move || audio_thread(rate, rx, ok_tx));
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

fn audio_thread(rate: u32, rx: Receiver<Vec<i16>>, ok: SyncSender<bool>) {
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
