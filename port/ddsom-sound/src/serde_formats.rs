//! Field formats of `sound/music.json` (feature `serde`).

/// Z80 address written `"0x39e9"`.
pub mod hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u32, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{v:#x}"))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
        let s = String::deserialize(d)?;
        s.strip_prefix("0x")
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .ok_or_else(|| serde::de::Error::custom(format!("invalid address: {s}")))
    }
}

/// Track bytes written as instruction tokens (`crate::tokens`).
pub mod tokens {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &[u8], s: S) -> Result<S::Ok, S::Error> {
        let text = crate::tokens::to_text(v).ok_or_else(|| serde::ser::Error::custom("track ending inside an instruction"))?;
        s.serialize_str(&text)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        crate::tokens::from_text(&s).ok_or_else(|| serde::de::Error::custom(format!("unreadable track: {s}")))
    }
}
