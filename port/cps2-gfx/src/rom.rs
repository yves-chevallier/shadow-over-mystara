//! Chargement de la région graphique CPS2 (ROM_LOAD64_WORD + désentrelacement « unshuffle »).

use std::io;
use std::path::Path;

/// Puces graphiques de ddsom : (fichier, offset dans la région). Chaque puce fournit un mot
/// (2 octets) tous les 8 octets, à l'offset indiqué (0, 2, 4 ou 6).
pub const DDSOM_GFX: &[(&str, usize)] = &[
    ("dd2.13m", 0x0000000),
    ("dd2.15m", 0x0000002),
    ("dd2.17m", 0x0000004),
    ("dd2.19m", 0x0000006),
    ("dd2.14m", 0x1000000),
    ("dd2.16m", 0x1000002),
    ("dd2.18m", 0x1000004),
    ("dd2.20m", 0x1000006),
];
pub const DDSOM_GFX_SIZE: usize = 0x1800000;

/// Construit la région graphique telle que le matériel la voit.
pub fn load_gfx(romdir: &Path, chips: &[(&str, usize)], size: usize) -> io::Result<Vec<u8>> {
    let mut region = vec![0u8; size];
    for &(name, offset) in chips {
        let data = std::fs::read(romdir.join(name))?;
        for (i, w) in data.chunks_exact(2).enumerate() {
            let dst = offset + 8 * i;
            region[dst] = w[0];
            region[dst + 1] = w[1];
        }
    }
    const BANK: usize = 0x200000;
    for bank in region.chunks_exact_mut(BANK) {
        unshuffle(bank);
    }
    Ok(region)
}

/// cps2_state::unshuffle, sur des unités de 8 octets.
fn unshuffle(buf: &mut [u8]) {
    let len = buf.len() / 8;
    if len == 2 {
        return;
    }
    let half = len / 2;
    let (a, b) = buf.split_at_mut(half * 8);
    unshuffle(a);
    unshuffle(b);
    // échange de la 2e moitié de a avec la 1re moitié de b
    let quarter = half / 2;
    a[quarter * 8..].swap_with_slice(&mut b[..quarter * 8]);
}
