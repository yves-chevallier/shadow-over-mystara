//! Extrait les ressources de Shadow over Mystara depuis les ROMs de l'utilisateur.
//!
//!     extract <dossier_roms> <dossier_assets> [gfx|palettes|sounds|sprites|levels...]
//!
//! Sans catégorie, tout est extrait. Tous les formats sont sans perte : le moteur peut retrouver
//! exactement les données d'origine (`cargo test -p ddsom-assets`, voir docs/assets.md).
//! Les fichiers produits dérivent des ROMs : ils restent locaux, jamais versionnés.

mod gfx;
mod levels;
mod palettes;
mod rom;
mod sounds;
mod sprites;

use std::path::Path;

const CATEGORIES: [&str; 5] = ["gfx", "palettes", "sounds", "sprites", "levels"];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 || args[3..].iter().any(|c| !CATEGORIES.contains(&c.as_str())) {
        eprintln!("usage : extract <dossier_roms> <dossier_assets> [{}...]", CATEGORIES.join("|"));
        std::process::exit(2);
    }
    let (roms, out) = (Path::new(&args[1]), Path::new(&args[2]));
    let wanted = |c: &str| args.len() == 3 || args[3..].iter().any(|a| a == c);
    let run = || -> std::io::Result<()> {
        std::fs::create_dir_all(out)?;
        if wanted("gfx") {
            gfx::extract(roms, &out.join("gfx"))?;
        }
        if wanted("palettes") {
            palettes::extract(roms, &out.join("palettes.png"))?;
        }
        if wanted("sounds") {
            sounds::extract(roms, &out.join("sounds"))?;
        }
        if wanted("sprites") {
            sprites::extract(roms, &out.join("sprites"))?;
        }
        if wanted("levels") {
            levels::extract(roms, &out.join("levels.json"))?;
        }
        std::fs::write(out.join("README.md"), README)?;
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("extraction impossible : {e}");
        std::process::exit(1);
    }
}

const README: &str = "\
# Ressources extraites de Shadow over Mystara

Générées par `extract` à partir des ROMs : ne pas versionner.

## gfx/cells_NN.png

Toute la ROM graphique, en cellules de 16x16 pixels dans l'ordre de la ROM : la cellule `c` est dans
`cells_{c / 4096}.png`, colonne `c % 64`, ligne `(c / 64) % 64`. PNG indexé 4 bits : la valeur est
l'indice de couleur dans la palette choisie par le jeu (15 = transparent).

Les autres formes de tuiles sont des lectures différentes des mêmes octets :
- tuile 8x8 `t` (couche 1) : demi-cellule, cellule `0x10000 + t / 2`, lignes `8 * (t % 2)..`,
  colonnes `8 * (colonne de la carte % 2)..` ;
- tuile 32x32 `t` (couche 3) : cellules `0x10000 + 4 t ..`, la ligne `y` de la tuile est la ligne
  `2 * (y % 8)` (moitié gauche) et `2 * (y % 8) + 1` (moitié droite) de la cellule `y / 8`.

## palettes.png

La bibliothèque de palettes du jeu (ROM programme 0x253000) : une palette de 16 couleurs par ligne.
Rouge, vert, bleu et luminosité sur 4 bits, stockés multipliés par 17 dans R, G, B et A.
Couleur affichée : `composante * 0x11 * (0x0f + 2 * luminosité) / 0x2d`.

## sounds/NNN.wav

Les échantillons QSound (table du pilote son, ROM Z80 0x49e8), PCM 8 bits à 24 038 Hz (vitesse de
lecture 1.0 de la puce). Le bloc `smpl` donne la boucle et la note de référence (60 = do central).

## sprites/frames.json, sprites/anims.json

Définitions des sprites : les 21 887 images (assemblages de cellules, table ROM 0x293000) et les scripts
d'animation (jeux et entrées). JSON, adresses ROM en hexadécimal ; format détaillé dans docs/assets.md.

## levels.json

Cartes des décors (scroll 2 et scroll 3) : blocs de 2x2 cases, bandes de 256x256 pixels et tables de
bandes des 115 scènes. Format détaillé dans docs/assets.md.
";
