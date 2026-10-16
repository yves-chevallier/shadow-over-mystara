//! Extrait les ressources de Shadow over Mystara depuis les ROMs de l'utilisateur.
//!
//!     extract <dossier_roms> <dossier_assets>
//!
//! Tous les formats sont sans perte : le moteur peut retrouver exactement les données d'origine.
//! Les fichiers produits dérivent des ROMs : ils restent locaux, jamais versionnés.

mod gfx;
mod palettes;
mod sounds;

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, roms, out] = &args[..] else {
        eprintln!("usage : extract <dossier_roms> <dossier_assets>");
        std::process::exit(2);
    };
    let (roms, out) = (Path::new(roms), Path::new(out));
    let run = || -> std::io::Result<()> {
        std::fs::create_dir_all(out)?;
        gfx::extract(roms, &out.join("gfx"))?;
        palettes::extract(roms, &out.join("palettes.png"))?;
        sounds::extract(roms, &out.join("sounds"))?;
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
";
