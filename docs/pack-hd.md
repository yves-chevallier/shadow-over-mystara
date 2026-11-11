# Pack HD : remplacer les graphismes par des images plus grandes

Un pack HD remplace les images des sprites et les tuiles des décors par des images retouchées ou agrandies (×2, ×4…),
sans perdre les palettes : le pack garde des **index de palette** (encres 0 à 14, 15 transparente), et les couleurs
sont celles de l'image en cours. Variantes de palette, flashs et fondus continuent donc de marcher sur les images HD.

Le pack est dérivé des graphismes de Capcom : **il ne se versionne jamais** (`hd/` et `hd-*/` sont dans
`.gitignore`), pas plus que les exports.

Outil : `hd-pack` (crate `port/ddsom-hd`), à construire une fois :

```sh
cd port
cargo build --release -p ddsom-hd        # binaire port/target/release/hd-pack
```

## 1. Exporter

```sh
DDSOM_EEPROM=testdata/eeprom_ddsom.bin \
  port/target/release/hd-pack export rom export --run scenarios/bot30k.txt 12000 [--run autre.txt N]... [--all]
```

(`rom` : le dossier des ROMs, ou `--assets <dossier>` pour jouer depuis les assets.)

L'outil joue les scénarios donnés (sans fenêtre, ~1 s pour 12 000 images) et note chaque image de sprite montrée avec
les couleurs qu'elle avait le plus souvent à l'écran (pas celles d'un flash ou d'un fondu). Il écrit :

* `export/frames/NNNNN.png` : chaque image de la table des images (`sprites/frames.json`, numéro = index) vue en jeu,
  assemblée à partir de ses morceaux, non retournée, en RGBA (fond transparent). `--all` ajoute toutes les autres
  (21 887 en tout), en gris, faute de connaître leurs couleurs ;
* `export/levels/NN_scroll2.png`, `NN_scroll3.png` : le décor complet de la scène `NN` sur la couche 2 (tuiles de 16)
  et la couche 3 (tuiles de 32), une grande image continue, avec les palettes que la scène charge ; une carte
  partagée par plusieurs scènes n'est écrite qu'une fois (sous la première) : 146 images ;
* à côté de chaque image : `.json` (métadonnées : ancre et palettes d'une image de sprite ; grille des tuiles et
  palettes d'un niveau) et `.pens.png` (les encres d'origine, qui servent au retour aux index). Ne pas les modifier.

## 2. Retoucher

Copier les PNG voulus (pas les `.json` ni les `.pens.png`) dans un dossier de travail, par exemple
`retouche/frames/00012.png`, `retouche/levels/00_scroll2.png`, et les agrandir ou les retoucher avec n'importe quel
outil (agrandisseur de pixel art, retouche à la main…). Règles :

* l'image agrandie doit avoir **exactement** `k` fois la taille de l'export, `k` entier, le même en largeur et en
  hauteur ;
* ne pas déplacer le dessin : l'ancre d'une image de sprite et la grille des tuiles d'un niveau sont fixes ;
* les couleurs reviennent à la plus proche des 15 couleurs de la palette d'origine (de la tuile pour un niveau, de
  l'un des morceaux pour un sprite) : une teinte nouvelle devient la plus proche ; un pixel d'alpha < 128 est
  transparent ;
* une tuile de décor est remplacée **partout où elle sert** (tous les niveaux, toutes les palettes) : la retouche
  d'une tuile répétée vaut pour toutes ses copies, et si la même tuile est retouchée différemment dans deux images de
  niveau, la première chargée (ordre des noms) l'emporte.

Pour essayer sans outil externe : `hd-pack upscale export retouche 2 [noms...]` agrandit au plus proche (image
identique à l'original, agrandie) ; `hd-pack upscale export retouche 4 --scale2x [noms...]` applique Scale2x deux
fois (diagonales adoucies, sans couleur nouvelle). Les noms sont `frames/NNNNN` ou `levels/NN_scrollL` (sans eux :
tout).

## 3. Revenir aux index

```sh
port/target/release/hd-pack index export retouche hd
```

Chaque image de `retouche/frames` et `retouche/levels` est ramenée aux encres de sa palette d'origine : PNG indexé
8 bits (sprites : `16 × palette + encre`, une « palette » par palette distincte des morceaux ; niveaux : encre) et ses
métadonnées, dans `hd/`. Quand une palette contient deux fois la même couleur, l'encre d'origine (`.pens.png`) départage.

## 4. Installer et jouer

Le dossier `hd/` se place dans le dossier d'où l'on lance le jeu (la racine du dépôt avec `play.sh`), ou se désigne
par `DDSOM_HD=<dossier>`. Au démarrage, `ddsom` écrit `HD pack hd: scale 4, 53 frames, 551 tiles (F8: on/off).`
**F8** coupe et remet le pack (comparaison instantanée, même en pause). Sans dossier `hd/`, rien ne change.

Toutes les images sont ramenées à la plus grande échelle du pack (une image ×2 dans un pack ×4 est agrandie au plus
proche ; une échelle qui ne divise pas la plus grande est ignorée, avec un message). L'image du jeu fait alors
`384 k × 224 k` et la fenêtre la montre comme d'habitude (échelle `F6`, filtres `F7`) : pour une netteté parfaite,
une fenêtre à un multiple de `k` (plein écran 1920 × 1200 : 5× ; un pack ×4 y est réduit au plus proche, ou lissé
avec `F7`).

## 5. Vérifier

```sh
DDSOM_EEPROM=testdata/eeprom_ddsom.bin \
  port/target/release/hd-pack check rom hd scenarios/bot30k.txt 12000 [--png captures 4500 6000]
```

rejoue le scénario avec et sans le pack, compte les images de sprites remplacées et les pixels qui diffèrent de
l'original agrandi (0 attendu pour un pack fait par `upscale … 2` sans retouche), et écrit les images demandées
(`NNNNN_hd.png`, `NNNNN_original.png`).

## Ce que le pack remplace, et ses limites

* **Sprites** : une image de la table des images, en entier, partout où le jeu la montre (`DrawList::shown_frames`),
  à son ancre, retournée comme elle, avec la priorité de ses entrées et la palette de chacun de ses morceaux
  (palette imposée par l'objet comprise). Les sprites qui ne sont pas des images de la table (ombres au sol,
  quelques effets tracés morceau par morceau) gardent leurs graphismes d'origine, agrandis.
* **Décors** : les tuiles des couches 2 et 3, reconnues par leur numéro de graphisme ; défilement et défilement par
  ligne déplacent les tuiles HD par pixels d'origine entiers. La couche 1 (tuiles de 8 : texte, interface, quelques
  avant-plans) n'est pas exportée (le format et le rendu la prennent en charge, clé `numéro × 2 + parité de la
  colonne`).
* **Composition** : celle du matériel, pixel à pixel à `k` fois la résolution (`ddsom_hd::render_hd`, calqué sur
  `cps2_gfx::render`) : couches dans leur ordre, sprites de la dernière entrée à la première, masqués par les couches
  selon leur priorité et par les sprites déjà tracés. Un pack « identité » (export complet, `upscale 2`, `index`)
  donne exactement l'original agrandi : bot30k, 12 000 images, 122 051 images de sprites remplacées, **0 pixel
  différent**.
* Une image HD ne peut pas déborder du cadre de l'image d'origine (la taille est `k` fois celle de l'export), ni
  ajouter de couleur à la palette (16 couleurs, dont la transparente).
* Les positions restent celles de l'arcade : un sprite ou un décor se déplace par pas de `k` pixels HD.
* Coût : rendu sur le processeur, ~1,7 ms par image à ×2 et ~6,7 ms à ×4 (bot30k), plus l'envoi de la texture.
