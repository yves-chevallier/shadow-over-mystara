//! Arrays of addresses stored in the program (often in the code itself, read PC-relative): one address per index.

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};
use crate::RomAddr;

/// Array of ROM addresses at `rom`, one long per index.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Pointers {
    #[serde(with = "hex")]
    pub rom: u32,
    #[serde(with = "hex_vec")]
    pub targets: Vec<u32>,
}

impl Pointers {
    /// Address of index `i` (`None` outside the array: the original would read beyond it).
    pub fn get(&self, i: i32) -> Option<RomAddr> {
        usize::try_from(i).ok().and_then(|i| self.targets.get(i)).map(|&a| RomAddr(a))
    }
}
