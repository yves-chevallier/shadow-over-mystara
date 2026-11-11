# Dungeons & Dragons: Shadow over Mystara — port Rust

Le jeu d'arcade de Capcom (CPS2, 1996, révision Europe 960619) réécrit en Rust : il se joue sans émulateur, sans
code 68000 et sans ROM, à partir de ressources (images PNG, sons WAV, tables JSON) que vous extrayez **de vos
propres ROMs**. Le jeu se comporte comme l'original, image par image : c'est vérifié en continu contre le code
d'origine ([docs/moteur.md](docs/moteur.md)).

Ce dépôt ne contient ni ROM ni rien qui en soit tiré : il faut posséder les ROMs du jeu.

## 1. Ce qu'il faut

* **Les ROMs**, au format du jeu `ddsom` de MAME : `ddsom.zip`, et `qsound.zip` (programme de la puce son,
  `dl-1425.bin`). Fichiers attendus une fois décompressés :

  | Rôle | Fichiers |
  |---|---|
  | programme 68000 (chiffré) et sa clé | `dd2e.03e` `dd2e.04e` `dd2e.05e` `dd2e.06e` `dd2e.07` `dd2e.08` `dd2e.09` `dd2e.10`, `ddsom.key` |
  | graphismes | `dd2.13m` à `dd2.20m` |
  | son : programme du Z80, échantillons | `dd2.01` `dd2.02`, `dd2.11m` `dd2.12m` |
  | puce QSound (`qsound.zip`) | `dl-1425.bin` |

* **Rust** : <https://rustup.rs> (Linux, macOS, Windows ; compilé ici avec Rust 1.98 stable).
* **Sous Linux**, pour compiler : un éditeur de liens, `pkg-config` et `libudev` (manettes) :

      sudo apt install build-essential pkg-config libudev-dev      # Debian, Ubuntu

  Pour jouer : une session graphique (X11 ou Wayland par XWayland, OpenGL ; WSLg convient) et PulseAudio (ou
  PipeWire) pour le son. Sans sortie son, le jeu tourne sans son.

## 2. Préparer les ROMs

Copier `ddsom.zip` et `qsound.zip` à la racine du dépôt, puis :

    mkdir -p rom
    unzip -o ddsom.zip -d rom
    unzip -o qsound.zip -d rom

(`./play.sh` fait ces deux décompressions lui-même s'il manque `rom/ddsom.key` ou `rom/dl-1425.bin`.)

## 3. Extraire les ressources

    cargo run --release --manifest-path port/Cargo.toml -p extract -- rom assets

Le premier lancement compile l'extracteur (quelques minutes), l'extraction elle-même prend une vingtaine de secondes.
Elle écrit `assets/` : graphismes (`gfx/cells_NN.png`), palettes, sons (`sounds/NNN.wav`), musique, sprites,
décors, textes et tables de jeu (`data/*.json`), environ 37 Mo ; leur format est décrit dans `assets/README.md` et
[docs/assets.md](docs/assets.md). L'extraction est sans perte. Une fois `assets/` produit, le jeu n'a plus besoin des
ROMs.

Après une mise à jour du dépôt qui change le format des ressources, relancer la même commande (elle réécrit
`assets/`).

## 4. Jouer

Sans ROM, depuis les ressources :

    cargo build --release --manifest-path port/Cargo.toml -p ddsom
    port/target/release/ddsom --assets assets

Avec les ROMs (les données sont alors extraites en mémoire à chaque lancement) :

    ./play.sh

`play.sh` compile et lance `port/target/release/ddsom rom`. Les deux façons jouent le même jeu.

**Windows** : `play.bat` (double-clic, PowerShell `.\play.bat`, ou depuis un partage `\\wsl.localhost\...`). Il
demande Rust pour Windows (MSVC) ; s'il n'y a pas encore de dossier `assets\` à jour, il décompresse `ddsom.zip` et
`qsound.zip` (posés à la racine) dans `rom\` et lance l'extraction une fois, puis compile et joue sans ROM (son par
waveOut).

Le jeu lit et écrit ses fichiers dans le **dossier courant** : lancez-le depuis la racine du dépôt (`play.sh` et
`play.bat` le font).

| Fichier | Contenu |
|---|---|
| `touches.cfg` | les touches (créé au premier lancement) |
| `nvram/ddsom.eeprom` | l'EEPROM du jeu (réglages du menu TEST, compteurs), écrite en quittant |
| `sauvegardes/rapide.txt` (et `.eeprom`) | la sauvegarde rapide |
| `hd/` | le pack HD, s'il existe (facultatif) |

## 5. Touches

| Action | Joueur 1 | Joueur 2 |
|---|---|---|
| Pièce | `5` | `6` |
| Start | `1` | `2` |
| Directions | flèches | `R` `F` `D` `G` (haut, bas, gauche, droite) |
| Bouton 1 (attaque) | `Ctrl` gauche ou `W` | `A` |
| Bouton 2 (saut) | `Alt` gauche ou `X` | `S` |
| Bouton 3 (sélection d'objet) | `Espace` ou `C` | `Q` |
| Bouton 4 (utiliser l'objet) | `Maj` gauche ou `V` | — |

| Touche | Effet |
|---|---|
| `F2` | menu TEST de la borne (réglages, enregistrés dans l'EEPROM) ; `9` : service (crédit) |
| `P` | pause |
| `Tab` (maintenue) | avance rapide (sans son) |
| `F5` / `F9` | sauvegarde rapide / reprise |
| `F6` | échelle : plus grand multiple entier qui tient dans la fenêtre, ou ajustée à la fenêtre |
| `F7` | filtre : aucun, lissé, CRT (lignes de balayage) |
| `F8` | pack HD marche / arrêt |
| `F11` | plein écran |
| `Échap` | quitter (enregistre l'EEPROM) |

La fenêtre (deux fois la taille de l'image au départ) se redimensionne librement.

Les touches viennent de la disposition du clavier : en AZERTY, `1` à `6` demandent `Maj` ; changez-les dans
`touches.cfg` si besoin.

### `touches.cfg`

Une ligne par action : `action = touches`, séparées par des espaces (vide : aucune), `#` commente. Actions :
`p1.up` `p1.down` `p1.left` `p1.right` `p1.button1`..`p1.button4` `p1.start` `p1.coin` (et `p2.*`), `test`,
`service`, `pause`, `save`, `load`, `fast`, `fullscreen`, `scale`, `filter`, `hd`, `quit`. Touches : `Left` `Right`
`Up` `Down`, `LeftCtrl` `RightCtrl` `LeftAlt` `LeftShift` `Space` `Enter` `Tab` `Escape`, `A`..`Z`, `Key0`..`Key9`,
`NumPad0`..`NumPad9`, `F1`..`F12`... Exemple :

    p1.button1 = LeftCtrl W Pad1X
    p1.coin = Key5 NumPad5 Pad1Back

Pour connaître le nom d'une touche : lancer le jeu avec `DDSOM_TOUCHES=1` (par exemple
`DDSOM_TOUCHES=1 ./play.sh`), il affiche le nom de chaque touche ou bouton pressé. Un fichier d'une version
précédente, aux noms français (`j1.haut`, `sauver`, `Pad1Haut`...), est encore lu, puis réécrit avec les noms
anglais (ses commentaires sont alors perdus).

### Manettes

Deux manettes au plus (Linux, Windows, macOS), dans l'ordre de branchement, disposition Xbox :

| Bouton | Action par défaut |
|---|---|
| croix ou stick gauche | directions |
| `X` (gauche) / `Y` (haut) / `B` (droite) / `A` (bas) | boutons 1 / 2 / 3 / 4 |
| `Start` / `Back` | Start / pièce |

Dans `touches.cfg` : `Pad1A` `Pad1B` `Pad1X` `Pad1Y` `Pad1LB` `Pad1RB` `Pad1Start` `Pad1Back` `Pad1Up` `Pad1Down`
`Pad1Left` `Pad1Right`, et `Pad2...` pour la seconde.

## 6. Sauvegardes et EEPROM

* **Sauvegarde rapide** : `F5` écrit `sauvegardes/rapide.txt`, `F9` reprend la partie à cet endroit. Le jeu
  rejoue sans affichage toutes les entrées depuis le démarrage (quelques secondes pour une longue partie) : la
  sauvegarde reste valable d'une version à l'autre tant que le jeu se comporte de la même façon.
* **EEPROM** : comme la borne, le jeu garde ses réglages (difficulté, pièces et crédits... dans le menu TEST, `F2`)
  et ses compteurs dans `nvram/ddsom.eeprom`, écrit en quittant (`Échap` ou fermeture de la fenêtre). Supprimer ce
  fichier rend les réglages d'usine.

## 7. Pack HD (facultatif)

Un dossier `hd/` à la racine (ou `DDSOM_HD=<dossier>`) remplace des images de sprites et des tuiles de décor par des
images agrandies ou retouchées, en gardant palettes, flashs et fondus ; `F8` le coupe. Il se fabrique avec l'outil
`hd-pack` à partir de vos ROMs : voir [docs/pack-hd.md](docs/pack-hd.md).

## 8. Ce qui n'est jamais versionné

Tout ce qui vient des ROMs (œuvre de Capcom) reste chez vous ; `.gitignore` l'exclut :

* les ROMs (`*.zip`, `rom/`) et les images programme (`prog_*.bin`) ;
* les ressources extraites (`assets/`), le pack HD et ses exports (`hd/`, `hd-*/`) ;
* vos fichiers de jeu (`touches.cfg`, `nvram/`, `sauvegardes/`) ;
* les produits de l'analyse (`listing.txt`, `ghidra_proj/`, `decomp/`, `testdata/`).

## Pour aller plus loin

* [docs/README.md](docs/README.md) : index de la documentation.
* [docs/moteur.md](docs/moteur.md) : le guide du moteur (couches, boucle d'image, tâches, données, validation).
* [docs/retro-ingenierie.md](docs/retro-ingenierie.md) : le matériel CPS2, le chiffrement, Ghidra et MAME.
* [docs/sans-rom.md](docs/sans-rom.md) : comment le jeu se passe des ROMs.
