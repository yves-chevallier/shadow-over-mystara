//! Les ressources extraites reconstruisent les ROMs octet pour octet sur les zones qu'elles décrivent
//! (région graphique, échantillons QSound, tables de la ROM programme, aux adresses d'origine).
//! Ignoré si les ressources (`DDSOM_ASSETS`, sinon ../../assets) ou les ROMs (`DDSOM_ROMS`, sinon ../../rom)
//! manquent.

use std::path::{Path, PathBuf};

use ddsom_assets::rebuild::{self, Image, Program};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn roms() -> PathBuf {
    std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| root().join("rom"))
}

fn assets() -> PathBuf {
    std::env::var_os("DDSOM_ASSETS").map(PathBuf::from).unwrap_or_else(|| root().join("assets"))
}

fn available() -> bool {
    let ok = assets().join("palettes.png").exists() && roms().join("dd2.13m").exists();
    if !ok {
        eprintln!("ressources ou ROMs absentes : test ignoré");
    }
    ok
}

fn same(name: &str, img: &Image, rom: &[u8]) {
    assert!(img.conflicts.is_empty(), "{name} : écritures contradictoires en {:#x}", img.conflicts[0]);
    if let Some(a) = rebuild::first_difference(img, rom) {
        panic!("{name} : octet {a:#x} différent ({:#04x} au lieu de {:#04x})", img.bytes[a as usize], rom[a as usize]);
    }
}

#[test]
fn programme_reconstruit() {
    if !available() {
        return;
    }
    let p = Program::load(&assets()).unwrap();
    assert!(p.missing.is_empty(), "fichiers absents : {:?}", p.missing);
    let (data, prog) = cps2::load_program(&roms()).unwrap();
    let bytes = |w: Vec<u16>| -> Vec<u8> { w.iter().flat_map(|v| v.to_be_bytes()).collect() };
    same("espace données", &p.data, &bytes(data));
    same("espace programme", &p.prog, &bytes(prog));
    // l'essentiel des tables est là : palettes (256 Ko), images (plus de 2 Mo avec les animations)
    assert!(p.data.covered_len() > 0x200000, "{}", p.data.covered_len());
}

/// Avec `data/reste.json`, la région programme est complète : les seuls octets absents (dans les deux espaces)
/// sont du code 68000 déclaré par ce fichier (docs/sans-rom.md « Couverture complète des données »).
#[test]
fn programme_complet_hors_code() {
    if !available() {
        return;
    }
    let p = Program::load(&assets()).unwrap();
    let reste = ddsom_assets::reste::Reste::load(&assets().join("data/reste.json")).unwrap();
    let mut code = vec![false; rebuild::PROGRAM_SIZE];
    for (d, f) in reste.plages_de_code().unwrap() {
        code[d as usize..f as usize].fill(true);
    }
    for (name, img) in [("espace données", &p.data), ("espace programme", &p.prog)] {
        let absent = (0..rebuild::PROGRAM_SIZE as u32).find(|&a| !img.is_covered(a) && !code[a as usize]);
        assert!(absent.is_none(), "{name} : octet {:#x} absent des assets et hors du code", absent.unwrap());
    }
}

#[test]
fn graphismes_reconstruits() {
    if !available() {
        return;
    }
    let region = rebuild::gfx_region(&assets().join("gfx")).unwrap();
    let rom = cps2_gfx::rom::load_gfx(&roms(), cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE).unwrap();
    assert!(region == rom, "région graphique différente");
}

#[test]
fn echantillons_reconstruits() {
    if !available() {
        return;
    }
    let img = rebuild::qsound_samples(&assets()).unwrap();
    let mut pcm = Vec::new();
    for name in ["dd2.11m", "dd2.12m"] {
        pcm.extend(std::fs::read(roms().join(name)).unwrap().chunks_exact(2).flat_map(|w| [w[1], w[0]]));
    }
    assert_eq!(img.bytes.len(), pcm.len());
    same("échantillons", &img, &pcm);
    assert!(img.covered_len() > 0x100000, "{}", img.covered_len());
}
