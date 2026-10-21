# Niveaux : images complètes des décors

Une image par scène jouable (1 à 68) et par boutique (69 à 82) : tout le décor de la scène (scroll 2 et
scroll 3 superposés dans l'ordre du matériel), **sans sprites ni interface**. `planche.png` en donne un
aperçu (vignette et numéro de chaque scène).

Les images sont tirées des ROMs (œuvre protégée) : elles ne sont **jamais versionnées**. Elles sont écrites
dans `assets/niveaux/` (dossier ignoré par git) ; les liens ci-dessous ne fonctionnent qu'après les avoir
générées.

## Régénérer

```sh
# depuis port/ ; les ROMs sont lues dans DDSOM_ROMS (dossier des fichiers dd2.*), l'EEPROM en deuxième argument
export DDSOM_ROMS=$PWD/../rom
cargo run --release -p ddsom-game --example cartes -- cartes ../rom ../testdata/eeprom_ddsom.bin ../assets/niveaux
cargo run --release -p ddsom-game --example cartes -- planche ../assets/niveaux ../docs/niveaux.md
```

La première commande démarre le jeu (pièce, Start, personnage), saute dans chaque scène et relève la
palette ; elle prend une dizaine de minutes pour les 82 scènes (numéros de scènes en arguments pour n'en
refaire que quelques-unes ; `DDSOM_ROUTES=1` relève aussi la scène suivante en forçant la sortie de scène,
`DDSOM_REF=1` enregistre une capture du jeu `ref_NN.png` pour comparaison). La seconde recrée la planche
et réécrit ce fichier.

Les tables elles-mêmes (blocs, bandes, tables de bandes des scènes) sont extraites sans perte dans
`assets/levels.json` par l'outil `extract` : voir [assets.md](assets.md).

## Méthode

Le décor n'est pas reconstitué par des captures d'écran : il est relu dans la ROM, comme le font les
caméras du jeu (`port/ddsom-game/src/scroll.rs`, `docs/video.md` §B1).

* Chaque couche est une grille de *bandes* de 256x256 pixels : 16x16 cases de 16 pixels (scroll 2, carte
  `0x1f9ba0`, banques `0x320f6`) ou 8x8 cases de 32 pixels (scroll 3, carte `0x2459c0`, banques `0x33a9c`).
* Pour chaque scène, une table du code (`0x2ff94 + 4 s` et `0x32d7c + 4 s`) donne l'adresse d'une table de
  numéros de bandes : un mot `W` (pas de ligne de la caméra), `K` lignes de `W` mots au-dessus de la ligne
  d'ordonnée 0 que désigne le pointeur (ordonnées croissantes = adresses décroissantes), puis quelques
  lignes sous 0. Toutes les tables se suivent en ROM ; le pointeur de chacune fixe le début de la suivante,
  ce qui donne les dimensions exactes de chaque carte. Dans le scroll 2 le dernier mot de ligne est une
  sentinelle (`0xfe`).
* L'outil lit ces tables, compose toutes les cases (même décodage que `cps2-gfx`), les peint avec la palette
  de la scène (relevée en jeu après l'ouverture) et superpose les deux couches dans l'ordre de CPS-B.
* Contrôle : les bandes que la mémoire vidéo contient au début de la scène sont identiques à celles de la
  table, et les images correspondent aux captures du jeu (`DDSOM_REF=1`).

## Limites

* Pas de scroll 1 (cases de 8 pixels : texte, interface, quelques avant-plans), ni de sprites : décors
  animés, objets et ennemis sont absents. Les dessins que le jeu écrit dynamiquement dans les couches
  (scènes à script particulier) ne sont pas reconstitués.
* Palette : celle du début de la scène, après l'ouverture. Les fondus, éclairs et changements de palette
  en cours de scène ne sont pas rendus.
* Parallaxe : quand une couche est plus étroite que l'autre (fond lointain), l'image est assemblée par
  tranches d'un écran (384 pixels), chacune telle que le joueur la voit à cette position, avec le
  rapport de défilement déduit des largeurs ; des raccords sont visibles entre tranches. Les deux couches
  sont alignées par le bas ; les couches seules sont dans `couches/NN_scroll2.png` et `NN_scroll3.png`.
* Une soixantaine de scènes n'ont pas de carte propre pour le scroll 3 (leur table est celle, par défaut, de
  la scène 1) : cette couche est alors un reste de mémoire masqué par le scroll 2, et elle est omise.
* Les écarts verticaux entre couches sont ceux des caméras au début de la scène ; si une caméra se déplace
  autrement pendant la scène (scripts de mode), l'alignement relatif n'est exact qu'au départ.
* Scènes 38, 65 et 68 : la phase de jeu n'a pas été atteinte par le saut direct (palette relevée au
  mieux) ; leurs couleurs sont à vérifier.
* Les bandes de remplissage des tables (`0xfc`, `0xfd`) ne sont pas dessinées (jamais à l'écran).
* Noms de lieux : non extraits (les cartons de titre sont des graphismes, pas du texte). Les bosses ne sont
  donnés que lorsque la documentation du dépôt les identifie.

## Index

![planche](../assets/niveaux/planche.png)

| Scène | Image | Taille (px) | Scroll 2 | Scroll 3 | Ordre | Scène suivante | Boss |
|---|---|---|---|---|---|---|---|
| 1 | [01.png](../assets/niveaux/01.png) | 1024x1024 | 1024x1280 | 512x1024 | 3>2 | 2 |  |
| 2 | [02.png](../assets/niveaux/02.png) | 1536x280 | 1792x512 | 1792x512 | 2>3 | - |  |
| 3 | [03.png](../assets/niveaux/03.png) | 384x232 | 512x512 | 512x512 | 3>2 | 4 |  |
| 4 | [04.png](../assets/niveaux/04.png) | 3584x568 | 3584x768 | 512x512 | 3>2 | - |  |
| 5 | [05.png](../assets/niveaux/05.png) | 1536x746 | 1536x1024 | 1024x512 | 3>2 | - |  |
| 6 | [06.png](../assets/niveaux/06.png) | 512x72 | 512x512 | 512x1024 | 2 | 7 |  |
| 7 | [07.png](../assets/niveaux/07.png) | 7680x512 | 7680x768 | 7680x512 | 3>2 | 8 |  |
| 8 | [08.png](../assets/niveaux/08.png) | 512x496 | 512x512 | 512x256 | 3>2 | 9 |  |
| 9 | [09.png](../assets/niveaux/09.png) | 7680x512 | 7680x768 | 7680x512 | 3>2 | 10 |  |
| 10 | [10.png](../assets/niveaux/10.png) | 1280x304 | 1280x768 | 512x1024 | 2 | - |  |
| 11 | [11.png](../assets/niveaux/11.png) | 768x464 | 768x768 | 768x256 | 2>3 | - |  |
| 12 | [12.png](../assets/niveaux/12.png) | 512x256 | 512x512 | 256x256 | 3>2 | 13 |  |
| 13 | [13.png](../assets/niveaux/13.png) | 384x1024 | 512x512 | 512x1024 | 3>2 | - |  |
| 14 | [14.png](../assets/niveaux/14.png) | 1536x685 | 1536x1024 | 1024x512 | 3>2 | - |  |
| 15 | [15.png](../assets/niveaux/15.png) | 1280x224 | 1280x512 | 512x1024 | 2 | 16 |  |
| 16 | [16.png](../assets/niveaux/16.png) | 1280x224 | 1280x512 | 512x1024 | 2 | 17 |  |
| 17 | [17.png](../assets/niveaux/17.png) | 416x240 | 512x512 | 512x1024 | 2 | 18 |  |
| 18 | [18.png](../assets/niveaux/18.png) | 416x240 | 512x512 | 512x1024 | 2 | 19 |  |
| 19 | [19.png](../assets/niveaux/19.png) | 416x240 | 512x512 | 512x1024 | 2 | 20 |  |
| 20 | [20.png](../assets/niveaux/20.png) | 1280x264 | 1024x512 | 1280x256 | 2>3 | - |  |
| 21 | [21.png](../assets/niveaux/21.png) | 384x232 | 512x512 | 512x512 | 3>2 | 22 |  |
| 22 | [22.png](../assets/niveaux/22.png) | 384x224 | 512x512 | 512x512 | 3>2 | 23 |  |
| 23 | [23.png](../assets/niveaux/23.png) | 384x232 | 512x512 | 512x512 | 3>2 | 24 |  |
| 24 | [24.png](../assets/niveaux/24.png) | 512x256 | 512x512 | 512x1024 | 2 | - | Manticore |
| 25 | [25.png](../assets/niveaux/25.png) | 1280x1696 | 1280x512 | 768x1792 | 3>2 | 26 |  |
| 26 | [26.png](../assets/niveaux/26.png) | 944x240 | 1024x512 | 512x512 | 2>3 | - |  |
| 27 | [27.png](../assets/niveaux/27.png) | 768x512 | 768x768 | 768x256 | 3>2 | - |  |
| 28 | [28.png](../assets/niveaux/28.png) | 768x512 | 512x768 | 768x256 | 3>2 | 29 |  |
| 29 | [29.png](../assets/niveaux/29.png) | 768x512 | 512x768 | 768x256 | 3>2 | 30 |  |
| 30 | [30.png](../assets/niveaux/30.png) | 768x512 | 512x768 | 768x256 | 3>2 | 31 |  |
| 31 | [31.png](../assets/niveaux/31.png) | 768x512 | 512x768 | 768x256 | 3>2 | - |  |
| 32 | [32.png](../assets/niveaux/32.png) | 768x512 | 512x768 | 768x256 | 3>2 | - | Black Dragon |
| 33 | [33.png](../assets/niveaux/33.png) | 1536x504 | 1536x768 | 1024x512 | 3>2 | - |  |
| 34 | [34.png](../assets/niveaux/34.png) | 768x240 | 512x512 | 768x256 | 3>2 | 67 |  |
| 35 | [35.png](../assets/niveaux/35.png) | 1280x474 | 1280x768 | 768x256 | 3>2 | - |  |
| 36 | [36.png](../assets/niveaux/36.png) | 512x256 | 512x512 | 512x256 | 2>3 | 37 |  |
| 37 | [37.png](../assets/niveaux/37.png) | 512x256 | 512x512 | 512x256 | 2>3 | 38 |  |
| 38 | [38.png](../assets/niveaux/38.png) | 512x240 | 512x512 | 512x1024 | 2 | 40 |  |
| 39 | [39.png](../assets/niveaux/39.png) | 1792x512 | 1792x768 | 768x512 | 3>2 | 40 |  |
| 40 | [40.png](../assets/niveaux/40.png) | 384x256 | 512x512 | 512x256 | 3>2 | - | Salamandre (givre) |
| 41 | [41.png](../assets/niveaux/41.png) | 512x496 | 512x512 | 512x256 | 3>2 | 44 |  |
| 42 | [42.png](../assets/niveaux/42.png) | 384x256 | 512x512 | 512x256 | 3>2 | - | Salamandre (flamme) |
| 43 | [43.png](../assets/niveaux/43.png) | 1792x512 | 1792x768 | 768x512 | 3>2 | 44 |  |
| 44 | [44.png](../assets/niveaux/44.png) | 768x463 | 768x512 | 768x512 | 3>2 | - |  |
| 45 | [45.png](../assets/niveaux/45.png) | 1280x304 | 1280x512 | 768x256 | 3>2 | 46 |  |
| 46 | [46.png](../assets/niveaux/46.png) | 1536x2048 | 768x512 | 1536x2048 | 3>2 | - |  |
| 47 | [47.png](../assets/niveaux/47.png) | 1280x512 | 1280x768 | 1280x512 | 3>2 | 48 |  |
| 48 | [48.png](../assets/niveaux/48.png) | 576x240 | 768x512 | 512x1024 | 2 | 49 |  |
| 49 | [49.png](../assets/niveaux/49.png) | 576x496 | 768x768 | 512x1024 | 2 | 50 |  |
| 50 | [50.png](../assets/niveaux/50.png) | 1280x512 | 1280x768 | 1280x512 | 3>2 | 51 |  |
| 51 | [51.png](../assets/niveaux/51.png) | 576x240 | 768x512 | 512x1024 | 2 | 52 |  |
| 52 | [52.png](../assets/niveaux/52.png) | 512x512 | 512x768 | 512x1024 | 2 | 53 |  |
| 53 | [53.png](../assets/niveaux/53.png) | 576x240 | 768x512 | 512x1024 | 2 | 54 |  |
| 54 | [54.png](../assets/niveaux/54.png) | 416x192 | 512x512 | 512x512 | 2>3 | 55 | Tel'Arin |
| 55 | [55.png](../assets/niveaux/55.png) | 1280x512 | 1280x768 | 768x512 | 3>2 | 56 |  |
| 56 | [56.png](../assets/niveaux/56.png) | 512x256 | 512x512 | 512x256 | 3>2 | 57 |  |
| 57 | [57.png](../assets/niveaux/57.png) | 512x256 | 512x512 | 512x256 | 3>2 | 58 |  |
| 58 | [58.png](../assets/niveaux/58.png) | 384x208 | 512x512 | 512x512 | 2>3 | - |  |
| 59 | [59.png](../assets/niveaux/59.png) | 416x432 | 512x512 | 512x512 | 2>3 | 61 |  |
| 60 | [60.png](../assets/niveaux/60.png) | 2032x512 | 2048x768 | 512x1024 | 2 | 61 |  |
| 61 | [61.png](../assets/niveaux/61.png) | 715x384 | 512x768 | 768x256 | 3>2 | 62 |  |
| 62 | [62.png](../assets/niveaux/62.png) | 715x368 | 512x768 | 768x256 | 3>2 | 63 |  |
| 63 | [63.png](../assets/niveaux/63.png) | 512x512 | 512x768 | 512x256 | 3>2 | 64 |  |
| 64 | [64.png](../assets/niveaux/64.png) | 384x208 | 512x512 | 512x512 | 2>3 | - |  |
| 65 | [65.png](../assets/niveaux/65.png) | 512x1736 | 512x512 | 512x256 | 3>2 | - |  |
| 66 | [66.png](../assets/niveaux/66.png) | 768x1960 | 768x768 | 512x2048 | 3>2 | - |  |
| 67 | [67.png](../assets/niveaux/67.png) | 512x4352 | 512x4608 | 512x1280 | 3>2 | - |  |
| 68 | [68.png](../assets/niveaux/68.png) | 384x208 | 512x512 | 512x1024 | 2 | - |  |
| 69 (boutique) | [69.png](../assets/niveaux/69.png) | 1005x224 | 512x256 | 1024x512 | 3>2 | 71 |  |
| 70 (boutique) | [70.png](../assets/niveaux/70.png) | 384x192 | 256x256 | 512x512 | 3>2 | 71 |  |
| 71 (boutique) | [71.png](../assets/niveaux/71.png) | 1005x224 | 512x256 | 1024x512 | 3>2 | 72 |  |
| 72 (boutique) | [72.png](../assets/niveaux/72.png) | 1005x224 | 512x256 | 1024x512 | 3>2 | 73 |  |
| 73 (boutique) | [73.png](../assets/niveaux/73.png) | 384x704 | 256x768 | 512x512 | 3>2 | 74 |  |
| 74 (boutique) | [74.png](../assets/niveaux/74.png) | 384x192 | 256x256 | 512x512 | 3>2 | 75 |  |
| 75 (boutique) | [75.png](../assets/niveaux/75.png) | 384x192 | 256x256 | 512x512 | 3>2 | 76 |  |
| 76 (boutique) | [76.png](../assets/niveaux/76.png) | 1005x224 | 512x256 | 1024x512 | 3>2 | 77 |  |
| 77 (boutique) | [77.png](../assets/niveaux/77.png) | 384x192 | 256x256 | 512x512 | 3>2 | 78 |  |
| 78 (boutique) | [78.png](../assets/niveaux/78.png) | 1005x224 | 512x256 | 1024x512 | 3>2 | 79 |  |
| 79 (boutique) | [79.png](../assets/niveaux/79.png) | 384x192 | 256x256 | 512x512 | 3>2 | 80 |  |
| 80 (boutique) | [80.png](../assets/niveaux/80.png) | 1005x224 | 512x256 | 1024x512 | 3>2 | 81 |  |
| 81 (boutique) | [81.png](../assets/niveaux/81.png) | 1008x224 | 512x256 | 1024x512 | 3>2 | 82 |  |
| 82 (boutique) | [82.png](../assets/niveaux/82.png) | 1008x736 | 512x256 | 1024x1024 | 3>2 | 83 |  |

Ordre : couches de l'arrière vers l'avant. « Scène suivante » : scène atteinte en forçant la sortie de
scène (`-` : non relevée ; la sortie dépend parfois du jeu, voir `docs/scenes.md` §5).
