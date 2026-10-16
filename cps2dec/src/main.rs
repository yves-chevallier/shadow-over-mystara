//! Déchiffrement des opcodes 68000 CPS2.
//! Port de MAME src/mame/capcom/cps2crypt.cpp et cps2_state::init_cps2crypt
//! (BSD-3-Clause, Paul Leaman, Andreas Naive, Nicola Salmoria, Charles MacDonald).
//!
//! Usage : cps2dec <prog_plain.bin> <jeu.key> <sortie.bin>
//! L'entrée est l'image programme big-endian (ROM_LOAD16_WORD_SWAP déjà appliqué).
//! La sortie est l'image vue par le CPU lors des lectures d'opcodes.

mod sboxes;
use sboxes::*;

pub struct Sbox {
    pub table: [u8; 64],
    pub inputs: [i32; 6], // -1 : pas d'entrée, seulement la clé
    pub outputs: [i32; 2],
}

#[derive(Clone, Copy)]
struct OptSbox {
    input_lookup: [u8; 256],
    output: [u8; 64],
}

impl OptSbox {
    fn new(s: &Sbox) -> Self {
        let mut o = OptSbox { input_lookup: [0; 256], output: [0; 64] };
        for i in 0..256u32 {
            let mut res = 0u8;
            for (k, &inp) in s.inputs.iter().enumerate() {
                if inp >= 0 {
                    res |= (((i >> inp) & 1) as u8) << k;
                }
            }
            o.input_lookup[i as usize] = res;
        }
        for i in 0..64 {
            let t = s.table[i];
            let mut v = 0u8;
            if t & 1 != 0 {
                v |= 1 << s.outputs[0];
            }
            if t & 2 != 0 {
                v |= 1 << s.outputs[1];
            }
            o.output[i] = v;
        }
        o
    }

    fn f(&self, x: u8, key: u32) -> u8 {
        self.output[(self.input_lookup[x as usize] ^ (key & 0x3f) as u8) as usize]
    }
}

const FN1_GROUP_A: [u32; 8] = [10, 4, 6, 7, 2, 13, 15, 14];
const FN1_GROUP_B: [u32; 8] = [0, 1, 3, 5, 8, 9, 11, 12];
const FN2_GROUP_A: [u32; 8] = [6, 0, 2, 13, 1, 4, 14, 7];
const FN2_GROUP_B: [u32; 8] = [3, 5, 9, 10, 8, 15, 12, 11];

fn bit(v: u32, n: u32) -> u32 {
    (v >> n) & 1
}

fn round(x: u8, boxes: &[OptSbox; 4], key: u32) -> u8 {
    boxes[0].f(x, key) | boxes[1].f(x, key >> 6) | boxes[2].f(x, key >> 12) | boxes[3].f(x, key >> 18)
}

fn feistel(val: u16, a: &[u32; 8], b: &[u32; 8], boxes: &[[OptSbox; 4]; 4], key: &[u32; 4]) -> u16 {
    let v = val as u32;
    let gather = |g: &[u32; 8]| (0..8).fold(0u8, |acc, i| acc | ((bit(v, g[i]) as u8) << i));
    let mut l = gather(b);
    let mut r = gather(a);
    l ^= round(r, &boxes[0], key[0]);
    r ^= round(l, &boxes[1], key[1]);
    l ^= round(r, &boxes[2], key[2]);
    r ^= round(l, &boxes[3], key[3]);
    let mut out = 0u16;
    for i in 0..8 {
        out |= (bit(l as u32, i as u32) as u16) << a[i];
        out |= (bit(r as u32, i as u32) as u16) << b[i];
    }
    out
}

fn expand(bits: &[u32; 96], src: &[u32; 2]) -> [u32; 4] {
    let mut d = [0u32; 4];
    for (i, &b) in bits.iter().enumerate() {
        d[i / 24] |= bit(src[(b / 32) as usize], b % 32) << (i % 24);
    }
    d
}

const KEY1_BITS: [u32; 96] = [
    33, 58, 49, 36, 0, 31, 22, 30, 3, 16, 5, 53, 10, 41, 23, 19, 27, 39, 43, 6, 34, 12, 61, 21,
    48, 13, 32, 35, 6, 42, 43, 14, 21, 41, 52, 25, 18, 47, 46, 37, 57, 53, 20, 8, 55, 54, 59, 60,
    27, 33, 35, 18, 8, 15, 63, 1, 50, 44, 16, 46, 5, 4, 45, 51, 38, 25, 13, 11, 62, 29, 48, 2,
    59, 61, 62, 56, 51, 57, 54, 9, 24, 63, 22, 7, 26, 42, 45, 40, 23, 14, 2, 31, 52, 28, 44, 17,
];

const KEY2_BITS: [u32; 96] = [
    34, 9, 32, 24, 44, 54, 38, 61, 47, 13, 28, 7, 29, 58, 18, 1, 20, 60, 15, 6, 11, 43, 39, 19,
    63, 23, 16, 62, 54, 40, 31, 3, 56, 61, 17, 25, 47, 38, 55, 57, 5, 4, 15, 42, 22, 7, 2, 19,
    46, 37, 29, 39, 12, 30, 49, 57, 31, 41, 26, 27, 24, 36, 11, 63, 33, 16, 56, 62, 48, 60, 59, 32,
    12, 30, 53, 48, 10, 0, 50, 35, 3, 59, 14, 49, 51, 45, 44, 2, 21, 33, 55, 52, 23, 28, 8, 26,
];

const SUBKEY_BITS: [u32; 64] = [
    5, 10, 14, 9, 4, 0, 15, 6, 1, 8, 3, 2, 12, 7, 13, 11, 5, 12, 7, 2, 13, 11, 9, 14, 4, 1, 6, 10,
    8, 0, 15, 3, 4, 10, 2, 0, 6, 9, 12, 1, 11, 7, 15, 8, 13, 5, 14, 3, 14, 11, 12, 7, 4, 5, 2, 10,
    1, 15, 0, 9, 8, 6, 13, 3,
];

fn opt(boxes: &[[Sbox; 4]; 4]) -> [[OptSbox; 4]; 4] {
    boxes.each_ref().map(|r| r.each_ref().map(OptSbox::new))
}

/// Décodage du fichier .key (cps2_state::init_cps2crypt) : clé maître 64 bits et plage chiffrée en mots.
fn parse_key(raw: &[u8]) -> ([u32; 2], u32, u32) {
    let mut d = [0u16; 10];
    for b in 0..160 {
        let bitpos = (317 - b) % 160;
        if (raw[bitpos / 8] >> ((bitpos ^ 7) % 8)) & 1 != 0 {
            d[b / 16] |= 0x8000 >> (b % 16);
        }
    }
    let key = [(d[0] as u32) << 16 | d[1] as u32, (d[2] as u32) << 16 | d[3] as u32];
    let (lower, upper) = if d[9] == 0xffff {
        (0xff0000, 0xffffff)
    } else {
        (0, ((((!d[9] as u32) & 0x3ff) << 14) | 0x3fff) + 1)
    };
    eprintln!(
        "clé {:08x} {:08x}  plage chiffrée {:06x}-{:06x}  watchdog {:04x} {:04x} {:04x}",
        key[0], key[1], lower, upper, d[6], d[5], d[4]
    );
    (key, lower / 2, upper / 2)
}

fn decrypt(rom: &[u16], key: [u32; 2], lower: u32, upper: u32) -> Vec<u16> {
    let sb1 = opt(&[FN1_R1_BOXES, FN1_R2_BOXES, FN1_R3_BOXES, FN1_R4_BOXES]);
    let sb2 = opt(&[FN2_R1_BOXES, FN2_R2_BOXES, FN2_R3_BOXES, FN2_R4_BOXES]);

    let mut k1 = expand(&KEY1_BITS, &key);
    // bits supplémentaires pour les s-boxes à moins de 6 entrées
    k1[0] ^= bit(k1[0], 1) << 4;
    k1[0] ^= bit(k1[0], 2) << 5;
    k1[0] ^= bit(k1[0], 8) << 11;
    k1[1] ^= bit(k1[1], 0) << 5;
    k1[1] ^= bit(k1[1], 8) << 11;
    k1[2] ^= bit(k1[2], 1) << 5;
    k1[2] ^= bit(k1[2], 8) << 11;

    let mut dec = rom.to_vec();
    for i in 0..0x10000u32 {
        let seed = feistel(i as u16, &FN1_GROUP_A, &FN1_GROUP_B, &sb1, &k1) as u32;
        let mut sub = [0u32; 2];
        for (j, &b) in SUBKEY_BITS.iter().enumerate() {
            sub[j / 32] |= bit(seed, b) << (j % 32);
        }
        sub[0] ^= key[0];
        sub[1] ^= key[1];
        let mut k2 = expand(&KEY2_BITS, &sub);
        k2[0] ^= bit(k2[0], 0) << 5;
        k2[0] ^= bit(k2[0], 6) << 11;
        k2[1] ^= bit(k2[1], 0) << 5;
        k2[1] ^= bit(k2[1], 1) << 4;
        k2[2] ^= bit(k2[2], 2) << 5;
        k2[2] ^= bit(k2[2], 3) << 4;
        k2[2] ^= bit(k2[2], 7) << 11;
        k2[3] ^= bit(k2[3], 1) << 5;

        let mut a = i;
        while (a as usize) < rom.len() {
            if a >= lower && a <= upper {
                dec[a as usize] = feistel(rom[a as usize], &FN2_GROUP_A, &FN2_GROUP_B, &sb2, &k2);
            }
            a += 0x10000;
        }
    }
    dec
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("usage : {} <prog_plain.bin> <jeu.key> <sortie.bin>", args[0]);
        std::process::exit(2);
    }
    let raw = std::fs::read(&args[1]).expect("lecture image programme");
    let keyfile = std::fs::read(&args[2]).expect("lecture clé");
    assert!(keyfile.len() >= 20, "fichier .key trop court");

    let rom: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    let (key, lower, upper) = parse_key(&keyfile);
    let dec = decrypt(&rom, key, lower, upper);
    let out: Vec<u8> = dec.iter().flat_map(|w| w.to_be_bytes()).collect();
    std::fs::write(&args[3], out).expect("écriture sortie");
}
