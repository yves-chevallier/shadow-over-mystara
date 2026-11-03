//! An image of a ROM zone: its bytes and the bytes that the resources describe (the sample bank, and the program
//! images that `extract::rebuild` writes for the lossless tests).

/// Bytes of a ROM zone, with the list of bytes that the resources describe.
pub struct Image {
    pub bytes: Vec<u8>,
    known: Vec<u64>,
    /// Addresses where two writes contradict each other (should stay empty).
    pub conflicts: Vec<u32>,
}

impl Image {
    pub fn new(size: usize) -> Self {
        Image { bytes: vec![0; size], known: vec![0; size.div_ceil(64)], conflicts: Vec::new() }
    }

    /// Writes `data` at address `at`.
    pub fn put(&mut self, at: u32, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            let a = at as usize + i;
            if a >= self.bytes.len() {
                panic!("write outside the image at {a:#x}");
            }
            let known = self.known[a / 64] >> (a % 64) & 1 != 0;
            if known && self.bytes[a] != b {
                self.conflicts.push(a as u32);
            }
            self.bytes[a] = b;
            self.known[a / 64] |= 1 << (a % 64);
        }
    }

    pub fn put16(&mut self, at: u32, v: u16) {
        self.put(at, &v.to_be_bytes());
    }

    pub fn put32(&mut self, at: u32, v: u32) {
        self.put(at, &v.to_be_bytes());
    }

    pub fn put_words(&mut self, at: u32, words: &[u16]) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        self.put(at, &bytes);
    }

    pub fn is_covered(&self, a: u32) -> bool {
        let a = a as usize;
        a < self.bytes.len() && self.known[a / 64] >> (a % 64) & 1 != 0
    }

    /// Covered zones `(start, end exclusive)`, sorted, non-overlapping.
    pub fn covered(&self) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        for a in 0..self.bytes.len() as u32 {
            if self.is_covered(a) {
                match out.last_mut() {
                    Some(r) if r.1 == a => r.1 += 1,
                    _ => out.push((a, a + 1)),
                }
            }
        }
        out
    }

    /// Covered bytes.
    pub fn covered_len(&self) -> usize {
        self.known.iter().map(|w| w.count_ones() as usize).sum()
    }
}
