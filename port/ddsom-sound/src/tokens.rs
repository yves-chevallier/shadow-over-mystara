//! Text form of the track bytes (`sound/music.json`): one space-separated token per instruction.
//!
//! * note (byte >= 0x20): `duration:note`, duration 1 to 7 (bits 7-5), note 0 to 31 (bits 4-0, 0 = rest);
//! * instruction (byte < 0x20): its name (`OPS`), followed by `=` and its comma-separated parameters:
//!   bytes in decimal, `step` as a word, jump offsets (`loopK`, `exitK`, `jump`) as a signed word
//!   relative to the address following the offset.

/// Names of instructions 0 to 31 (docs/son.md §6, `track::op`).
pub const OPS: [&str; 32] = [
    "triplet", "tie", "dot", "up2", "mode", "step", "gate", "amp", "inst", "oct", "mtrans", "trans", "detune", "glide",
    "loop0", "loop1", "loop2", "loop3", "exit0", "exit1", "exit2", "exit3", "jump", "end", "pan", "echo", "vol", "vib",
    "trem", "lfo", "lfomode", "bank",
];

/// Parameter bytes of instruction `op` (< 0x20).
pub fn param_len(op: u8) -> usize {
    match op {
        0..=3 | 23 => 0,
        5 | 22 => 2,
        14..=21 => 3,
        _ => 1,
    }
}

/// Token of the instruction starting at `bytes[0]` (and its length), `None` if bytes are missing.
pub fn decode(bytes: &[u8]) -> Option<(String, usize)> {
    let b = *bytes.first()?;
    if b >= 0x20 {
        return Some((format!("{}:{}", b >> 5, b & 0x1f), 1));
    }
    let n = param_len(b);
    let p = bytes.get(1..1 + n)?;
    let be = |i: usize| u16::from_be_bytes([p[i], p[i + 1]]);
    let name = OPS[usize::from(b)];
    let tok = match b {
        0..=3 | 23 => name.to_string(),
        5 => format!("{name}={}", be(0)),
        22 => format!("{name}={}", be(0) as i16),
        14..=21 => format!("{name}={},{}", p[0], be(1) as i16),
        _ => format!("{name}={}", p[0]),
    };
    Some((tok, 1 + n))
}

/// Bytes of a token.
pub fn encode(tok: &str) -> Option<Vec<u8>> {
    if let Some((d, n)) = tok.split_once(':') {
        let (d, n): (u8, u8) = (d.parse().ok()?, n.parse().ok()?);
        return ((1..=7).contains(&d) && n < 32).then_some(vec![d << 5 | n]);
    }
    let (name, args) = tok.split_once('=').unwrap_or((tok, ""));
    let op = OPS.iter().position(|&o| o == name)? as u8;
    let args: Vec<i32> = if args.is_empty() { Vec::new() } else { args.split(',').map(|a| a.parse().ok()).collect::<Option<_>>()? };
    let mut out = vec![op];
    match (op, args.as_slice()) {
        (0..=3 | 23, []) => {}
        (5 | 22, [v]) => out.extend((*v as u16).to_be_bytes()),
        (14..=21, [a, v]) => {
            out.push(*a as u8);
            out.extend((*v as u16).to_be_bytes());
        }
        (_, [v]) if param_len(op) == 1 => out.push(*v as u8),
        _ => return None,
    }
    Some(out)
}

/// Tokens of a run of instructions, `None` if the last one is cut.
pub fn to_text(bytes: &[u8]) -> Option<String> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let (tok, n) = decode(&bytes[at..])?;
        tokens.push(tok);
        at += n;
    }
    Some(tokens.join(" "))
}

/// Bytes of a run of tokens, `None` if one is unreadable.
pub fn from_text(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for tok in text.split(' ') {
        bytes.extend(encode(tok)?);
    }
    Some(bytes)
}
