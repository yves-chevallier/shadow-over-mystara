# Ressources extraites : tables, formats, vérification

L'outil `extract` (`port/extract`) lit les ROMs de l'utilisateur et écrit `assets/` ; la crate
`ddsom-assets` relit ces fichiers. Les fichiers produits dérivent des ROMs : ils restent locaux (dossier
`assets/` ignoré par git), seuls le code, les tests et cette documentation sont versionnés.

```sh
# depuis port/
cargo run --release -p extract -- ../rom ../assets            # tout
cargo run --release -p extract -- ../rom ../assets sprites levels terrain
DDSOM_ROMS=$PWD/../rom cargo test --release -p ddsom-assets   # vérification sans perte
DDSOM_ASSETS=/autre/dossier DDSOM_ROMS=... cargo test ...     # ressources hors de assets/
```

Catégories de l'outil (sans argument : toutes) : `gfx`, `palettes` (`palettes.png`,
`scene_palettes.json`), `sounds`, `sprites` (`sprites/frames.json`, `anims.json`, `tables.json`), `levels`,
`terrain`, `texts`, `music` (`sound/music.json`), `data` (`data/spawns.json` et les tables de données
`data/players.json`, `combat.json`, `enemies.json`, `boxes.json`, `scenes.json`, `misc.json`,
`dialogs.json`, `rest.json`).

Les tests relisent les fichiers et réencodent chaque table au format de la ROM : ils échouent au premier
octet différent (ROMs lues dans `DDSOM_ROMS`, sinon `rom/` à la racine ; test ignoré si un fichier manque).

| Catégorie | Fichier | Test |
|---|---|---|
| Graphismes (cellules 16x16) | `gfx/cells_NN.png` | `graphismes_sans_perte` |
| Palettes | `palettes.png` | `palettes_sans_perte` |
| Échantillons | `sounds/NNN.wav` | `sons_sans_perte` |
| Images de sprites | `sprites/frames.json` | `images_de_sprites_sans_perte` |
| Scripts d'animation | `sprites/anims.json` | `animations_sans_perte` |
| Cartes des décors | `levels.json` | `decors_sans_perte` |
| Images du scroll 1 | `levels.json` (`scroll1`) | `scroll1_sans_perte` |
| Palettes des scènes | `scene_palettes.json` | `palettes_des_scenes_sans_perte` |
| Terrain (cartes fixes, types) | `terrain.json` | `terrain_sans_perte` |
| Boîtes, marques au sol, listes d'images | `sprites/tables.json` | `tables_des_objets_sans_perte` |
| Textes (couche 8x8, bulles, noms) | `texts.json` | `textes_sans_perte` |
| Musiques et effets (pilote Z80) | `sound/music.json` | `musique_sans_perte` |
| Scripts d'apparition (vagues) | `data/spawns.json` | `apparitions_sans_perte` |
| Tables des joueurs | `data/players.json` | `joueurs_sans_perte` |
| Réactions aux coups | `data/combat.json` | `combat_sans_perte` |
| Tables des ennemis | `data/enemies.json` | `ennemis_sans_perte` |
| Tables lues par le Rust, hors des précédentes | `data/rest.json` | `restes_sans_perte` |
| Tout le programme, graphismes, échantillons | tous | `programme_reconstruit`, `graphismes_reconstruits`, `echantillons_reconstruits` (`tests/rebuild.rs`) |

Les trois premières catégories sont décrites dans le `README.md` que l'outil écrit dans `assets/`. Ce
document décrit les autres.

## Reconstruction des ROMs depuis les ressources

Le but du projet est un jeu sans ROM : `ddsom_assets::rebuild` refait, à partir du dossier d'assets, les
contenus que le moteur lit aujourd'hui dans les ROMs. Chaque fonction `put_*` de `Program` est l'inverse
d'un chargement de fichier (les encodeurs des tests sans perte y sont réunis ; les tests les appellent).

| Fonction | Reconstruit | Test |
|---|---|---|
| `gfx_region(assets/gfx)` | région graphique (0x1800000 octets, 4 plans par groupe de 8 pixels) | `graphismes_reconstruits` : identique à `cps2_gfx::rom::load_gfx` |
| `qsound_samples(assets)` | ROM d'échantillons (4 Mo) : les 342 `sounds/NNN.wav` à leur adresse (fiches de `music.json`) et la suite de chacun (`sample_tails`) ; le reste à zéro | `echantillons_reconstruits` : octets couverts identiques à `dd2.11m` + `dd2.12m` |
| `Program::load(assets)` | ROM programme, vue données et vue espace programme déchiffrée (`Image`), à leurs adresses : tous les fichiers du tableau ci-dessus | `programme_reconstruit` : octets couverts identiques à `cps2::load_program` ; aucune écriture contradictoire |

`Image::covered()` donne les zones décrites par les fichiers ; les autres octets valent zéro. Sur la ROM
programme (4 Mo, vue données), les ressources couvrent 3 084 046 octets (images, palettes, cartes de décors,
textes, tables) ; le reste est du code 68000 et des tables non extraites (voir « Lectures de la ROM
programme par le Rust »). Ne sont pas reconstruits : le code et les données du Z80 (`dd2.01`, `dd2.02` :
seules les tables du pilote, `sound/music.json`, sont extraites), le programme du DSP QSound
(`dl-1425.bin`) et le code 68000.

Application : `ddsom [dossier_roms] --assets [dossier]` (`ddsom/src/main.rs`) charge les graphismes et
les échantillons depuis les ressources (`Machine::load_ddsom_with` : `dd2.11m`...`dd2.20m` ne sont pas
lues) ; le programme, le Z80 et le DSP viennent encore des ROMs. Vérification d'une partie :

```sh
cargo run --release -p ddsom-assets --example partie_assets -- ../rom ../testdata/eeprom_ddsom.bin \
    <assets> ../scenarios/bot30k.txt 6000 partie.png
```

deux machines (jeu en Rust, 68000 surcadencé), l'une avec les ROMs, l'autre avec les ressources, rejouent le
scénario : les images (RVB) et les échantillons audio sont identiques à chaque image (bots 30k, personnage 3 et
deux joueurs : 6 000 images, 2 418 461 échantillons audio comparés).

Deux écarts avec le premier extracteur, découverts par cette comparaison : la table des échantillons du
pilote compte 345 fiches, pas 290 (les fiches 291 à 344 sont les voix des sons suivants ; 290, 293 et 305 sont
invalides) ; et une voix QSound qui ne boucle pas lit au-delà de la fin déclarée jusqu'à son arrêt par le
Z80 (`sample_tails`, voir « Musiques et effets »).

## Conventions des fichiers JSON

* Adresses ROM (provenance, et clés tant que le jeu range des adresses dans ses objets) : chaînes
  hexadécimales `"0x2b45d8"`. Toutes les autres valeurs sont des nombres décimaux.
* Une image, une entrée d'animation, une bande ou un bloc par ligne ; l'ordre des champs est celui des
  structures de `ddsom-assets` (`sprites.rs`, `levels.rs`), qui documentent chaque champ.
* Adresses de la ROM programme 68000 (4 Mo, `dd2e.03e`-`dd2e.10`), lectures de données ; les tables lues
  par le code en adressage relatif au PC le sont dans l'espace programme déchiffré (précisé ci-dessous).

## Inventaire des tables

| Table | Adresse ROM | Taille | Structure |
|---|---|---|---|
| Adresses des images | `0x293000-0x2a85fc` | 21 887 longs | adresse de l'image `i` (suit la bibliothèque de palettes) |
| Images | `0x2b45d8-0x37c3d2` | 21 887 images, 150 800 morceaux | en-tête de 10 octets puis morceaux, à la suite |
| Décalages des morceaux | `0x37c3d2-0x3ec70a` | 13 070 tables | `(dx, dy)` en mots signés, une table partagée par plusieurs images |
| Noms des familles du visualiseur | `0xefe16` | 201 x 4 octets | `pl00`..`pl0b`, `em..`, `bs..`, `ef..`, `pf..`, `ps..`, `es..`, `we..`, `bx..`, `it..`, `st10` |
| Listes de jeux par famille | `0xf0532` | 201 mots | décalage (depuis `0xf0532`) d'une liste de longs : un jeu par variante |
| Variantes par famille | `0xf06c4` | 201 longs | octet bas = 4 x (variantes - 1) |
| Jeux d'animations | `0x120000-0x1d0000` | 290 jeux, 15 740 scripts | mots signés : décalage de chaque script depuis le jeu |
| Entrées d'animation | `0x120000-0x1d0000` | 31 610 entrées | 16 octets (24 pour les personnages), + 4 si saut |
| Scroll 2 : pointeurs de scènes | `0x2ff94` (programme) | 115 longs | adresse de la ligne 0 de la table de bandes |
| Scroll 2 : tables de bandes | `0x318a2-0x320f6` | 81 tables | mot de largeur `W` puis lignes de `W` mots |
| Scroll 2 : banques de blocs | `0x320f6` | 3 longs | `0x211ba0` (6 731 blocs), `0x22c050` (5 259), `0x240900` (1 276) |
| Scroll 2 : bandes | `0x1f9ba0` | 586 x 128 octets | 8x8 numéros de blocs |
| Scroll 3 : pointeurs de scènes | `0x32d7c` (programme) | 115 longs | idem |
| Scroll 3 : tables de bandes | `0x33564-0x33a9c` | 65 tables | idem, sans sentinelle |
| Scroll 3 : banques de blocs | `0x33a9c` | 2 longs | `0x2499c0` (1 299 blocs), `0x24eaf0` (1 037) |
| Scroll 3 : bandes | `0x2459c0` | 512 x 32 octets | 4x4 numéros de blocs |
| Scroll 1 : grilles d'images | `0x1f1b60-0x1f3b60` | 64 x 128 octets (15 non nulles) | 8x8 numéros de blocs (mots) |
| Scroll 1 : blocs | `0x1f3b60-0x1f9ba0` | 385 x 64 octets | 4x4 cases (code, attributs) |
| Palettes de la page 0 par scène | `0x2ae81e` | 115 x 32 mots | déplacement dans la bibliothèque, `0xffff` = laissée |
| Palettes ajoutées par scène | `0x1a41c` | 115 longs | adresse de 3 palettes ou 0 |
| Ensembles de palettes | `0x1a3fc` | 8 longs | adresse de 32 palettes (page 0) |
| Pages 1, 2, 3 | `0x1a62c`, `0x1a6a0`, `0x1a8d4` (programme) | 3, 115, 115 longs | adresse de 32 palettes |
| Cartes de terrain fixes | `0x3f8e9c`, `0x3f9c9c` | 112 et 96 colonnes de 16 mots | mot d'attributs, terrain = bits 7-15 |
| Types de terrain | `0x3425c`, `0x34a5c` | 2 x 128 x 16 octets | 8 mots : genre, routine, sens, échelle, bord, arrêt, 0, 0 |
| Jeux de boîtes (corps, poussée, attaque) | `0x11bffa`, `0x11c11a`, `0x11c23a` | 3 x 72 longs | adresse de la boîte 0 du jeu |
| Boîtes de collision | `0x11c35a-0x123600` | 201 blocs, 3 654 boîtes | 8 octets : X, demi-largeur, Z, demi-hauteur, Y, demi-profondeur, 0, 0 |
| Boîtes de visibilité | `0x1ca538` | 128 x 8 octets | mots : dx, largeur, dy, hauteur |
| Marques au sol | `0x1e45c` | 97 x 16 octets | 8 mots : coût, code, x, attributs, dx, dy, priorité, 0 |
| Listes d'images du tir ESHL 5 | `0x9e8bc-0x9eba4` | 3 x 53 + 3 x 9 longs | adresses d'images |
| Liste d'images de 0xb48c0 | `0x17efca` | 7 longs | adresses d'images |
| Tir ESHL 5 : boîtes, listes, retournements | `0x9e55c`, `0x9e638`, `0x9e6a4` (programme), `0x9e6bc` | 15 x 4 octets, 2 x 12 mots, 256 x 2 octets | indices de boîtes ; déplacements ; (image, retournement) |

Les régions des images et des décalages sont lues d'un bout à l'autre et entièrement couvertes (pas
de trou ni de chevauchement) ; la table des adresses donne exactement la même suite d'images. Les tables
de bandes se suivent sans trou jusqu'à la table des banques.

## Images de sprites : `sprites/frames.json`

Une image (« cadre », champ `+0x34` d'un objet, voir [objets.md](objets.md)) est une liste de morceaux,
chacun un sprite matériel CPS2 de 1 à 16 x 1 à 16 cellules 16x16. Lecture : `sprites.rs` (0x1e062).

```json
{
  "table": "0x293000",
  "frames": [
    {"rom":"0x2b45d8","cost":13,"attr":6,"bank":0,"mode":12,"offsets":"0x37c3d2","pieces":[[-16,-64,1,4608], ...]},
```

| Champ | En ROM | Sens |
|---|---|---|
| `rom` | adresse de l'en-tête | identifiant ; numéro de l'image = rang dans la liste (= rang dans la table `0x293000`) |
| `cost` | mot +0 | morceaux retirés du budget de la liste de sprites (0x36a par image) |
| `attr` | mot +2 | `mode` nul : attributs communs à tous les morceaux, qui sont alors `cost` ; sinon nombre de morceaux |
| `bank` | octet +4 | banque de cellules (bits 13-14 du mot y de l'Object RAM) : cellule = `bank << 16 \| code` |
| `mode` | octet +5 | nul : attributs communs ; sinon chaque morceau a les siens (seule la nullité compte pour le jeu) |
| `offsets` | long +6 | table des `(dx, dy)` des morceaux (provenance ; les valeurs sont recopiées dans `pieces`) |
| `pieces` | +10 | `[dx, dy, code, attributs]` : `code` (mot) puis, si `mode`, attributs (mot) ; `dx`, `dy` de la table |

Unités : `dx`, `dy` en pixels depuis le point d'ancrage de l'objet (sa position à l'écran : x, et y + z
retournés), y vers le bas de l'écran. Miroir horizontal de l'objet : morceau en `-dx - 16 x largeur`, bit 5
des attributs inversé. Attributs : bits 0-4 palette (relative à la base des sprites, ou imposée par
l'objet, `+0x2a/+0x2b`), 5 miroir x, 6 miroir y, 8-11 largeur - 1, 12-15 hauteur - 1. La cellule (bx, by)
d'un morceau est `bank << 16 | (code & !0xf) + ((code + bx) & 0xf) + 16 by` (sans miroir ; `render.rs` de
`cps2-gfx`). L'ordre des morceaux est l'ordre d'écriture dans l'Object RAM : le premier passe devant.

Aperçu : `cargo run --release -p ddsom-assets --example frame -- ../assets <numéro> <palette> sortie.png`.

## Scripts d'animation : `sprites/anims.json`

Format d'une entrée et pas d'animation : `objects/anim.rs` et [objets.md](objets.md) §5.

```json
{
  "names_rom": "0xefe16", "families_rom": "0xf0532", "variants_rom": "0xf06c4",
  "families": [ {"name":"pl00","long":true,"sets":["0x12367c","0x12377c","0x1237e2"]}, ... ],
  "sets": [ {"rom":"0x12367c","long":true,"scripts":[440,1804,640, ...]}, ... ],
  "entries": [ {"rom":"0x123834","long":true,"time":32,"flags":0,"frame":1,"data":[1,1,0,20, ...]}, ... ]
}
```

* `families` : les familles du visualiseur de sprites du menu de test caché (`0xef116`..., animation
  démarrée en `0xefa40`) : nom, entrées longues (les douze `plNN`, test en `0xefa76`), un jeu par variante.
* `sets` : un jeu (tableau que reçoit `anim_start_indexed`, 0x1a32) : `scripts[i]` est le décalage en
  octets, depuis `rom`, du script `i`. La ROM ne donne pas la longueur d'un jeu : elle s'arrête au premier
  script qui suit le tableau, au jeu suivant ou à une entrée d'animation, sans les derniers décalages qui
  ne mènent à aucune entrée. 30 décalages intérieurs ne mènent à rien (gardés tels quels).
* `entries` : toutes les entrées connues, par adresse croissante. `time` durée en images ; `flags` mot de
  drapeaux (octet haut 0 : l'entrée suivante suit en mémoire ; 0xfe : fin ; autre : saut vers `next`) ;
  `frame` numéro d'image ; `data` les 8 octets recopiés en `+0x38..+0x3f` de l'objet (ombres, marque au sol,
  boîte de visibilité, priorité), 16 pour une entrée longue (`+0x38..+0x47`).
* Entrées atteintes : 25 737 depuis les jeux du visualiseur (taille certaine), plus 5 873 entrées
  isolées (`"guessed": true`) : toute adresse de la région des scripts dont le long +4 est le début d'une
  image et la durée non nulle, chaîne de suites et de sauts cohérente ; sa taille (16 ou 24 octets) est
  déduite de cette cohérence et, si les deux conviennent, de la présence d'une autre entrée juste après.
  Ces scripts sont ceux que le code démarre directement (`anim_start` 0x1a3c, `lea ...,A4` : 113 adresses)
  ou par des jeux absents du visualiseur (268 adresses de jeux dans le code).
* Ensemble : 19 230 images sur 21 887 sont employées par une entrée ; toutes les images relevées en jeu
  par `examples/creatures.rs` (1 451) le sont.

## Cartes des décors : `levels.json`

Mêmes tables que les caméras (`scroll.rs`, [video.md](video.md) §B1) et [niveaux.md](niveaux.md).

```json
{"layers": [ {"name":"scroll2","case":16,"tile_offset":6144,"palette_base":64,"fillers":[252],
   "banks_rom":"0x320f6","banks":[{"rom":"0x211ba0","blocks":[[0,0,0,0,0,0,0,0], ...]}, ...],
   "strips_rom":"0x1f9ba0","strips":[[...64 numéros de blocs...], ...],
   "scenes_rom":"0x2ff94","scenes":[0,0,1, ...],
   "tables":[{"rom":"0x318a2","rows":[[...],...],"zero":3}, ...]}, ... ]}
```

* Couche : `scroll2` (cases de 16 pixels, bandes de 16x16 cases) ou `scroll3` (cases de 32 pixels, 8x8).
  Une bande couvre 256x256 pixels.
* `banks` : blocs de 2x2 cases, 4 x (code, attributs) dans l'ordre (0,0), (0,1), (1,0), (1,1) (x puis y).
  La tuile d'une case est `code + tile_offset` (tuiles 16x16 du scroll 2 ou 32x32 du scroll 3, voir le
  README des assets) ; attributs : bits 0-4 palette (+ `palette_base` dans la mémoire de palettes), 5 et 6
  miroirs.
* `strips` : numéros de blocs d'une bande, colonne par colonne (`(cx / 2) * n + cy / 2`, n = 8 ou 4). La
  banque d'une bande est `numéro / 256` (`Layer::bank_of`) ; `Layer::case_at` donne la case (cx, cy).
* `tables` : grille des numéros de bandes, lignes dans l'ordre de la ROM (haut de la carte d'abord),
  `zero` = ligne d'ordonnée 0 que désigne le pointeur de scène (les suivantes sont sous 0). Scroll 2 : le
  dernier mot de chaque ligne est une sentinelle 0xfe (sauf dans la dernière table). Numéros 0xfc (scroll 2)
  et 0xfd (scroll 3) : remplissage, jamais à l'écran, sans bande définie (`fillers`).
* `scenes` : pour chacune des 115 scènes (0 à 114), l'indice de sa table ; plusieurs scènes partagent la
  table par défaut (celle de la scène 1, voir [niveaux.md](niveaux.md)).
* Les bandes extraites vont de 0 au plus grand numéro employé, les blocs de chaque banque de 0 au plus
  grand numéro employé par ses bandes.

Le test vérifie aussi que `Layer::case_at` redonne, pour chaque case de chaque bande, la case que lit la
caméra du jeu (`map_cell`).

### Scroll 1 : `scroll1` de `levels.json`

Images fixes de 256x256 pixels que 0x260c recopie dans la couche de cases 8x8 (écran titre, fonds de
menus, avant-plans de quelques scènes : D0 = 5 et 6 en 0x13dba, 7 et 8 en 0xfa7d6...).

```json
"scroll1": {"tile_offset":63488,"grids_rom":"0x1f1b60","grids":[[...64 numéros de blocs...], ...],
            "blocks_rom":"0x1f3b60","blocks":[[...16 x (code, attributs)...], ...]}
```

* `grids[D0]` : 8x8 numéros de blocs, colonne par colonne (`(cx / 4) * 8 + cy / 4`). Seules les 15
  premières grilles de la région sont non nulles ; le test vérifie que le reste est nul.
* `blocks` : 4x4 cases `(code, attributs)` colonne par colonne. Tuile 8x8 = `code + 0xf800` (modulo
  0x10000 : l'original ajoute `0xf8000000` au long de la case) ; attributs comme les autres couches.
* `Scroll1::case_at(image, cx, cy)` donne la case (cx, cy), 0 à 31 ; le test la compare à une
  réécriture de 0x260c (destination en colonnes de 0x80 octets). Les blocs finissent exactement là
  où commencent les bandes du scroll 2 (`0x1f9ba0`).

Le texte de la couche 1 (chiffres, chaînes, bulles) est écrit par le code depuis des descripteurs, voir
la section des textes.

## Tables des objets : `sprites/tables.json`

Tables que le code indexe avec les octets `+0x38..+0x3c` d'un objet (recopiés des entrées d'animation,
champ `data` de `anims.json`) et listes d'images lues directement par le code.

```json
{"box_tables":[{"name":"body","rom":"0x11bffa","sets":["0x11c35a", ...72...]}, ...],
 "box_blocks":[{"rom":"0x11c35a","boxes":[[0,0,0,0,0,0,0,0],[0,16,32,32,2,6,0,0], ...],"tail":[]}, ...],
 "box_end":"0x123600",
 "bounds_rom":"0x1ca538","bounds":[[256,960,512,1024], ...],
 "marks_rom":"0x1e45c","marks":[{"cost":3,"code":8137,"x":-24,"attr":528,"dx":0,"dy":0,"priority":32768,"unused":0}, ...],
 "frame_lists":[{"rom":"0x9e8bc","frames":[...numéros d'images...]}, ...],
 "shot5":{"boxes_rom":"0x9e55c","boxes":[[0,2,71,44], ...],"even_rom":"0x9e638","even":[1280, ...],
          "odd_rom":"0x9e6a4","odd":[536, ...],"mirrored_rom":"0x9e6bc","mirrored":[[32,0], ...]}}
```

* **Boîtes de collision** ([combat.md](combat.md) §3). Un objet porte un jeu (`+0x2e`) et trois
  index : corps `+0x38`, poussée `+0x39`, attaque `+0x3a` (0 : aucune boîte). `box_tables[k].sets[jeu]`
  est l'adresse de la boîte 0 du jeu ; la boîte `n` est 8 octets plus loin par index
  (`SpriteTables::hit_box`). Boîte : X (signé), demi-largeur, Z (hauteur), demi-hauteur, Y (profondeur),
  demi-profondeur, deux octets nuls. Toutes les boîtes forment une région jointive, découpée en blocs
  à chaque adresse désignée par un jeu ; un bloc s'étend jusqu'au suivant (`tail` : les 2 octets de
  bourrage `0x4e71` qui terminent 59 blocs d'attaque). Le tableau des boîtes d'attaque des tirs joueur
  (`0x121fc8`), celui des tirs ennemis (`0x1221e2`) et celui du ramassage (`0x1233c8`), indexés sans jeu,
  sont des blocs de cette région. Ces mêmes tables servent à dessiner les « ombres détaillées »
  (`sprites.rs`, 0x1ec4e : quatre quarts autour du centre de la boîte `+0x38` et `+0x3a`).
* **Boîtes de visibilité** (index `+0x3c`, 0x1ed2, [objets.md](objets.md) §7) : `(dx, largeur, dy,
  hauteur)`, l'objet est visible si `x écran + dx <= largeur` et `hauteur écran + dy <= hauteur` en non
  signé. 128 entrées (la suite de la ROM est d'une autre nature) ; les index au-delà de 65 valent tous
  `(0, 384, 0, 256)`.
* **Marques au sol** (index `+0x3b`, 0x1e3a6, `sprites.rs`) : un sprite matériel par marque ;
  97 entrées, du code suit.
* **Listes d'images** : les seules suites d'adresses d'images de la ROM hors table principale et
  entrées d'animation (recherche exhaustive dans les deux espaces). `0x9e8bc` : six listes du tir
  ESHL 5 (anneaux du serpent, zones de contact des grands ennemis, `eshl.rs` `shot5`) ; `0x17efca` :
  7 images choisies par `+0x33` du porteur (0xb48c0). Les autres images que le code impose directement
  (`move.l #image,(0x34,A0)`) sont des immédiats dans le code, pas des tables.
* **Tir ESHL 5** (`shot5`) : par `+0x2f` (rang), indices des trois boîtes et de la marque au sol
  (`boxes`, espace programme ; 15 entrées jusqu'au code, seules les 7 premières non nulles), déplacement
  de la liste des images paires et impaires (`even`, `odd`, mots relatifs à leur table, espace
  programme), et (image, retournement) des images impaires par `(+0x42 / 2) * 16 + +0x43 / 2`
  (`mirrored`, espace données).

## Textes : `texts.json`

Textes écrits dans la couche de caractères 8x8 (scroll 1, fenêtre `0x912000`) par les routines V2
(`text.rs`, [video.md](video.md)), noms des bulles (`popups.rs`) et noms terminés par `@`.

```json
{"texts":[
  {"rom":"0x13b52","kind":"text","col":2,"row":25,"attr":2,"text":"           Parental Advisory           /..."},
  {"rom":"0x14f1a","kind":"text","col":4,"row":24,"attr":2,"text":"{fc}1996 TSR, Inc. All Rights Reserved. /..."},
  {"rom":"0x1b1ec","kind":"tall","col":14,"row":14,"attr":0,"words":[506,511,532, ...]},
  {"rom":"0x1bd32","kind":"tall_at","attr":0,"words":[...]},
  {"rom":"0x19f02","kind":"box","col":3,"row":5,"size":[21,2]},
  {"rom":"0x13550","kind":"text","col":13,"row":11,"attr":0,"text":"shuukyaku demo 1 ","scanned":true}, ...],
 "tables":[{"rom":"0x11cba","program":false,"offsets":false,"entries":["0x124ca", ...]}, ...],
 "popups":{"table":"0x1a01a","offsets":[106,114, ...],"strings_rom":"0x1a084","strings":["Kobold ", ...]},
 "names":[{"table":"0xd6232","program":false,"entries":[...],"rom":"0xd6286","names":["Ogre ", ...]}, ...]}
```

Un descripteur par ligne, par adresse croissante (`Texts::by_rom`) ; `TextDesc::bytes` redonne ses octets.
Sortes (`kind`) et codes de contrôle :

| Sorte | Routines | Octets en ROM | Contrôles |
|---|---|---|---|
| `text` | `0xd5e` écrit, `0xd32` efface, `0xdca` écrit sans effacer le fond | colonne, ligne, attributs (mot), octets, `@` | `/` ligne suivante (`0xdca` : `_`, et l'espace laisse le fond) ; `0xff n` : n espaces |
| `tall` | `0x1136` | colonne, ligne, attributs, mots, `@@` | mot `t` : tuile `t` et `t + 0x10` en dessous ; `##` (0x2323) colonne sautée, `/@` ligne suivante, `//` deux lignes |
| `tall_at` | `0x1234` | attributs, mots, `@@` (position en D5, D6) | comme `tall` |
| `blocks` | `0xf38` dessine, `0xf4a` efface | colonne, ligne, attributs, mots, `@@` | bloc 2x2 `t, t+1 / t+0x10, t+0x11` ; `##` deux colonnes, `//` deux lignes, `0xffff n` n blocs vides |
| `box` | `0x1324` | colonne, ligne, largeur, hauteur | rectangle d'espaces (A4 n'avance que de 3) |

* **Texte d'octets** (`text`) : les octets sont des tuiles 8x8 dont la police suit l'ASCII (0x20 à 0x7e),
  0xfc est ©. Dans le JSON, chaque caractère ASCII imprimable s'écrit tel quel sauf `{` et `}` ; tout
  autre octet s'écrit `{xx}` en hexadécimal, de même que l'octet qui suit `0xff` (nombre d'espaces). Le `@`
  final n'est pas écrit (`texts::encode` / `texts::decode`). Ligne < 0x20 et colonne < 0x40 en
  cases ; attributs : palette de la couche 1 (bits 0-4).
* **Mots** (`tall`, `tall_at`, `blocks`) : numéros de tuiles de la grande police (p. ex. 506 = 0x1fa), les
  mots de contrôle restent dans `words`, sans le `@@` final.
* **Origine** : 219 descripteurs sont trouvés en suivant le code (adresses chargées dans A4 avant un appel
  aux routines ci-dessus, et les 20 tables de `tables` : longs, ou mots de déplacement depuis la table pour
  `0xf2968` ; `program` : lue en relatif au PC). Chaque descripteur trouvé est suivi de ceux qui lui
  succèdent en mémoire tant qu'ils sont plausibles (les appelants enchaînent les textes avec A4). 27 autres
  (`"scanned": true`) viennent d'une recherche de motifs dans toute la ROM (adresse paire, colonne < 0x40,
  ligne < 0x20, attributs < 0x20, au moins 6 caractères puis `@`) : libellés de débogage de l'attract
  (`shuukyaku demo`...), écrans de test, « INSERT 2 COINS », menu de test secret. Les descripteurs ne se
  chevauchent pas.
* **Bulles** (`popups`, 0x19fd2) : `offsets[n]` (mots, espace programme) mène depuis `table` au nom n ;
  noms à la suite à partir de `strings_rom`, terminés par `/` (non écrit), un octet par tuile (ASCII).
  53 numéros, 39 noms (ennemis : Kobold, Skeleton...).
* **Noms terminés par `@`** (`names`) : noms des boss de la barre de vie (table `0xd6232`, 21 entrées,
  région `0xd6286`, « Ogre Master Bros » n'est désigné par aucune entrée) et des personnages (`0xb20e8`,
  espace programme, 6 noms : Fighter... ; écrits une tuile par caractère par 0xb20b8).

Le test réencode chaque descripteur, chaque table, les noms des bulles et les groupes de noms.

## Musiques et effets : `sound/music.json`

Tout ce que lit le pilote son réécrit (`ddsom-sound`, [son.md](son.md)) hors échantillons PCM
(`sounds/`) : l'extraction lit la ROM Z80 avec `ddsom_sound::Rom` (adresses « linéaires » : décalage dans
`dd2.01` puis `dd2.02`), et le test aussi.

```json
{"header_ptr":"0x39e1","header":"0x9000","count":1168,"music_factor":122,"sfx_volume":127,"header_rest":[0,0],
 "songs":[null,
   {"rom":"0xa245","kind":0,"tracks":[33,201,346, ...16 déplacements...]}, ...],
 "code":[{"rom":"0xa266","ops":"step=307 vol=100 5:0 dot tie 5:0 tie 3:0 oct=5 gate=180 inst=31 ..."}, ...],
 "banks_table":"0x39e5","banks":[{"rom":"0x39e9","instruments":[[...8 octets...], ...256]}, ...],
 "samples_ptr":"0x39e3","samples_rom":"0x49e9","samples":[[...8 octets...], ...345],
 "sample_tails":["", "", "0a1b...", ...345],
 "tables":[{"name":"amplitudes","rom":"0x33e1","words":true,"values":[...]}, ...]}
```

* **Table des sons** : l'en-tête (`header`, adresse lue dans le mot `0x39e1`) donne le nombre de sons, les
  index du facteur de volume de la musique et du volume des effets (table `levels`) et 2 octets non lus ;
  suit une entrée de 4 octets par son (adresse sur 3 octets grand-boutiste, 0). `songs[n]` : définition du
  son `n` (`null` : entrée vide, 245 sur 1 168). `kind` : 0 musique (53), 1 à 0x7f effet de cette priorité
  (868), 0x80-0x8f note simple (aucune), 0x90-0x92 ADPCM (2). Musique et effet : `tracks`, 16 déplacements
  (mots grand-boutistes depuis `rom`, 0 = canal sans piste) ; note simple (10 octets) et ADPCM (7 octets) :
  `body`, voir [son.md](son.md) §5. La première définition (`0xa245`) commence sur le dernier octet nul de
  la table.
* **Pistes** (`code`) : toutes les instructions qu'atteignent les pistes des sons, en suivant les sauts
  (`jump`, débuts et sorties de boucles), regroupées en blocs d'instructions jointives (1 612 blocs,
  123 659 instructions, aucun chevauchement). `Music::track(adresse)` rend les jetons d'une piste jusqu'à
  sa fin ou son premier saut. Jetons (`music::decode_op` / `encode_op`) :

  | Jeton | Octets | Sens |
  |---|---|---|
  | `d:n` | `d << 5 \| n` (>= 0x20) | note : durée d (1-7, table `durations`), note n (1-31 + octave ; 0 silence) |
  | `triplet`, `tie`, `dot`, `up2` | 0, 1, 2, 3 | bascule triolet, bascule liaison, note pointée suivante, bascule +2 octaves |
  | `mode=f` | 4, f | drapeaux 6, 5 et 3 de la voie |
  | `step=w` | 5, mot | pas de temps (de la voie, ou de toute la musique) |
  | `gate=`, `amp=`, `inst=`, `oct=` | 6, 7, 8, 9 | tenue, amplitude (table `amplitudes`), instrument du banc, octave |
  | `mtrans=`, `trans=`, `detune=`, `glide=` | 10, 11, 12, 13 | transposition (musique, voie), accord fin, glissando |
  | `loopK=n,d` | 14 + K, n, mot | début de boucle K (0-3) : n tours, saut de d octets |
  | `exitK=f,d` | 18 + K, f, mot | sortie de la boucle K au dernier tour : drapeaux f, saut de d |
  | `jump=d` | 22, mot | saut relatif |
  | `end` | 23 | fin de piste |
  | `pan=`, `echo=`, `vol=`, `vib=`, `trem=`, `lfo=`, `lfomode=`, `bank=` | 24 à 31 | pan, écho, volume de la musique, vibrato, trémolo, vitesse et mode du LFO, banc d'instruments |

  Les déplacements des sauts sont des mots signés relatifs à l'adresse qui suit le déplacement (comme en
  ROM) ; le test vérifie que chaque cible est le début d'une instruction. Les octets de la région des sons
  (`0x9000-0x3bf2b`) non couverts par l'en-tête, les définitions et les pistes : 742 (738 trous, surtout un
  `end` isolé que rien n'atteint après un `jump`) ; la suite de la ROM est nulle.
* **Instruments** : la table `0x39e5` donne l'adresse de 2 bancs (`0x39e9`, `0x41e9`, juste après elle) de
  256 fiches de 8 octets (échantillon en mot petit-boutiste dont le bit 15 suit la variation de hauteur
  globale, accord fin, 5 index d'enveloppe : attaque, decay, tenue, chute lente, relâchement).
* **Échantillons** : 345 fiches de 8 octets à l'adresse du mot `0x39e3` (`0x49e9`) : banque, début,
  boucle, fin (mots petit-boutistes), note de référence ; les données PCM sont dans `sounds/NNN.wav`
  (342 fichiers : les fiches 290, 293 et 305 sont invalides, début après la fin, et n'ont pas de fichier).
  `sample_tails` : la suite de chaque échantillon dans la ROM d'échantillons, en hexadécimal sans les zéros
  finaux, jusqu'au prochain échantillon de la banque ou à la fin de la banque (`Music::sample_tail_end`) :
  une voix QSound qui ne boucle pas continue de lire après la fin tant que le Z80 ne l'arrête pas.
* **Tables du pilote** (`tables`, ROM fixe ; `words` : mots petit-boutistes) : `qsound_init_regs` (0xd5,
  12 octets) et `qsound_init_values` (0xe1, 12 mots), `durations` (0x1119, 24 mots), `octave_offsets`
  (0x1149, 16 octets), `semitone_octave` (0x1fc6, 108), `octave_start` (0x2032, 9), `rates` (0x203b, 108
  mots), `fine` (0x2113, 12 x 256 octets), `amplitudes` (0x33e1, 128 mots), `levels` (0x34e1, 128), `attack`
  (0x35e1, 64), `decay` (0x3661, 64), `lfo_rates` (0x36e1, 128), `vibrato` (0x37e1, 128), `tremolo`
  (0x38e1, 128).

Reste dans le code du pilote (et `ddsom-sound`) : le tableau de saut des instructions (0x1179), la
logique d'enveloppe, de LFO et de pan.

## Scripts d'apparition : `data/spawns.json`

Les « vagues d'ennemis » : un script par scène fait entrer ennemis, décors et objets au fil du défilement
(lecteur 0xf2e5c, `scene/script.rs`, [scenes.md](scenes.md) §2).

```json
{"table":"0xf2f62","scenes":["0x10cde4","0x10cde6", ...115...],
 "scripts":[{"rom":"0x10cde6","entries":[
    {"x":0,"dx":0,"z":0,"y":0,"rank":0,"players":1,"unused":0,"class":4,"f2c":0,"kind":1,"variant":0,"f2f":0,"f42":0},
    {"x":299,"call":"0x2a24","arg":299}, ...],"pad":[78,113]}, ...]}
```

* `scenes[s]` : adresse du script de la scène s (table de longs, espace données) ; 108 scripts distincts
  (les scènes 0x52 à 0x59 partagent le même), 1 706 apparitions et 18 appels.
* Apparition (0x12 octets) : abscisse de caméra de déclenchement `x`, objet créé en X = `x + dx`, Z, Y ;
  sautée si le rang du jeu (`A5+0x7f93`) dépasse `rank` ou s'il y a moins de `players` joueurs ; `class`
  (octet pair) choisit l'allocateur (table de mots `0xf2f38` : ENEMY, SET, EFFCT, BOX, SOBJ, BOSS, ITEM,
  ESHL...), `kind` et `variant` vont en `+2` et `+3` de l'objet, `f2c`, `f2f`, `f42` en `+0x2c`, `+0x2f`,
  `+0x42` (mot) ; l'octet `unused` (`+0xa`) n'est pas lu. Appel (10 octets, mot `+2` = 0xffff) : routine
  `call` avec D0 = `arg`. Le mot 0xffff final n'est pas écrit ; `pad` : mot de bourrage `0x4e71` entre
  certains scripts. Les scripts sont jointifs de `0x10cde4` à `0x114788`.

## Tables de données : `data/*.json`

Les tables de règles du jeu (personnages, réactions aux coups, ennemis, caisses, scènes) partagent un
même format générique (`ddsom_assets::data`, extraction `extract/src/data.rs`) : une table est une suite
de lignes de même forme, rangées telles quelles en ROM.

```json
{"regions":[{"name":"players","start":"0x101166","end":"0x1051ae"}],
 "tables":[
   {"name":"hp_by_level","rom":"0x325e","program":true,"format":"addr","rows":["0x101436", ...12...]},
   {"name":"hp_by_level/fighter","rom":"0x101436","format":"u16","rows":[112,112, ...]},
   {"name":"jump_speeds/fighter","rom":"0x1044fe","format":"i16*4","fields":["vx","ax","vz","az"],
    "rows":[[0,0,1280,-56], ...]}, ...]}
```

* `format` : types des colonnes, grand-boutistes : `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `addr` (long
  qui désigne une adresse de la ROM), `x8`, `x16`, `x32` (masques de bits) ; ces quatre derniers s'écrivent
  en hexadécimal. `T*n` répète `n` fois `T`. `fields` nomme
  les colonnes quand leur sens est connu. Une table d'une colonne écrit ses lignes comme des nombres.
* `program` : table lue par le code en adressage relatif au PC, donc dans l'espace programme déchiffré
  (tables rangées au milieu du code) ; sinon espace données.
* `regions` : plages que les tables couvrent exactement, sans trou ni chevauchement (la taille de chaque
  table y est l'écart jusqu'à la suivante).
* Accès : `DataFile::table(nom)`, `Table::value(ligne)`, `Table::get(ligne, colonne)`,
  `Table::field(ligne, "vx")` ; `DataFile::via("hp_by_level", personnage)` suit une table d'adresses par
  personnage comme le code (`table_of` de `init.rs`).

Le test de chaque fichier (`joueurs_sans_perte`, ...) réencode toutes les tables et les compare à la ROM
octet par octet, vérifie la couverture des régions et que toute adresse `addr` qui tombe dans une région
est le début d'une ligne d'une table du fichier.

### Joueurs : `data/players.json`

Personnages (`+3` de l'enregistrement joueur, 0 à 11) : classe = personnage / 2 (0 combattant `fighter`,
1 clerc `cleric`, 2 elfe `elf`, 3 nain `dwarf`, 4 magicien `magic_user`, 5 voleur `thief`), deux
variantes par classe. Une table par personnage est nommée `rôle/propriétaires` : la classe si ses deux
variantes la partagent, `classe.0` / `classe.1` sinon, `+` entre plusieurs classes.

**Région 0x101166-0x1051ae** (entièrement couverte), lue par les tables d'adresses (12 longs) :

| Rôle | Adresses (espace) | Lignes | Format | Sens (consommateur) |
|---|---|---|---|---|
| `level_by_scene` | `0x3208` (prog) | 120 par classe | `u8` | niveau initial selon la scène (`ram::SCENE`, 0x31f0, `init_level`) ; 115 scènes, 5 octets de plus jamais relevés |
| `hp_by_level` | `0x325e` (prog) | 24 | `u16` | PV max au niveau (0x3238) |
| `weapons_by_level` | `0x3522` (prog) | 22, 23, 11, 13, 22, 24 | `u8*4` | armes de départ `WEAPON_A`, `WEAPON_B`, deux drapeaux débloqués (0x34e4) |
| `random_stats` | `0x32d8` (prog) | 8 | `u8*8` | jeux de caractéristiques tirés au hasard, copiés en `+0x100..+0x106`, `+0x108` (0x328e) |
| `weapon_power` | `0x3590` (prog) | 40 | `u8*2` | par numéro d'arme (≤ 0x27) : puissance (`+0x109`/`+0x10a`), plafond de dégâts `+0x16e` (0x3552) |
| `unlocked_by_level` | `0x345e` (prog) | comme les armes | `u8*8` | drapeaux d'objets débloqués (liste terminée par 0, `0xff2478 + n`) (0x3432) |
| `gear_by_level` | `0x3402` (données) | 22, 23, 23, 11, 13, 24, 22, 24 | `u8*48` | équipement copié en `+0x120` : trois listes de 16 octets (8 identifiants, 8 charges ; 0x56 = rien) ; huit tables distinctes (0x3308) |
| `spells_by_level` | `0x34b4` (prog) | 24, 23, 11, 22 | `u8*16` | 10 premiers octets copiés en `+0x150` (réserve de sorts, 0x348e) ; une table nulle commune au combattant, au nain et au voleur |
| `magic_damage/0..14` | `0x10417a` + 60 n | 2 x 30 | `u8*30` | dégâts par niveau du lanceur : ligne 0 contre les ennemis, ligne 1 contre les chefs (`Damage::Table(chefs, ennemis)` de `foes.rs` ; consommateurs : `pshl_b`, `pshl_d`, `peff_e`, `peff_h`, `cleric`, `magic_user`, `elf`, `items`) |
| `jump_speeds/<classe>` | `0x1044fe`… | 4 (nain : 8) | `i16*4` | (vx, ax, vz, az) de décollage par type de saut `+0x67`, 8.8 pixels par image (`common::launch_jump`) ; le nain lit aussi `ligne + 1` |
| `walk_*` | `0x1045de`, `0x10478e`, `0x10493e`, `0x104aee`, `0x104c9e`, `0x104e4e`, `0x104ffe` (données) | 16 | `i16*2` | vitesses (vx, vy) 8.8 par combinaison des directions (bits 0-3 des touches) : normale, inutilisée (aucune lecture trouvée), lente forcée, course, rapide, accroupie, lente (`walk.rs`) |

**Tables rangées dans le code** (taille mesurée jusqu'à l'instruction suivante) :

| Nom | Adresse | Lignes x format | Sens |
|---|---|---|---|
| `shock_damage` | `0x10108` (données) | 5 x `u16` | dégâts du choc de l'arrivée d'un joueur, par nombre de joueurs vivants (masqués à 63) |
| `throw_kind` | `0x371b2` | 12 x `u8` | type de l'effet de lancer par personnage (`checks::throw_button`) |
| `special_anim` | `0x372e4` | 88 x `u8` | animation de l'attaque spéciale par identifiant de sort/objet (jusqu'à 0x3733c) |
| `ungrabbable` | `0x37588` | 32 x `u8` | non nul : type d'ennemi qui ne se laisse pas saisir (`moves::backstab`) |
| `dust_stand`, `dust_run`, `dust_jump` | `0x3faac`, `0x3fb02`, `0x3fb48` | 6 x `i16*2` | poussières (vx, vy) par classe |
| `item_sparks` | `0x3fbb8` | 42 x `i16*4` | étincelle d'objet : D0, D1, D2 de l'effet 0xb10c8, hauteur (`common::item_spark`) |
| `undead_counts` | `0x4054e` | 30 x `u8*2` | renvoi des morts-vivants : nombre de types 1 et 5 renvoyés, par niveau |
| `cure_counts`, `hold_counts`, `banish_counts` | `0x40ece`, `0x4131e`, `0x41afe` | 30, 40, 40 x `u8` | monstres touchés au plus par prière, par niveau |
| `blessing_durations`, `protection_durations` | `0x410a6`, `0x41e4c` | 25 x `u16` | durées (images) par niveau |
| `heal_amounts` | `0x416cc` (données) | 3 x `u16*24` | PV rendus par niveau (ligne 0 lue par le soin ; les deux autres suivent sans lecture relevée) |
| `heal_flicker` | `0x41792` (données) | 3 x `u16*8` | couleurs du scintillement de soin |
| `portal_speeds`, `regroup_speeds` | `0x47e78`, `0x4851c` | 12 x `u16` | vitesse par tranche de 16 pixels de distance (magicien) ; l'index n'est pas borné par le code |
| `backstab_offsets` | `0x4935a` | 23 x `i16*2` | décalage (dx, dz) du voleur derrière l'ombre, par type de l'objet lié |

Tailles incertaines : `level_by_scene` (5 octets au-delà de la scène 114), les tables du code dont l'index
n'est pas borné (`special_anim`, `portal_speeds`, `regroup_speeds`, `backstab_offsets`) s'arrêtent à
l'instruction suivante. Les nombres de lignes inégaux des tables par niveau suivent l'écart entre tables
(niveaux maximaux différents selon la classe).

Ne sont pas des données (adresses de code, restent dans le moteur) : tables de gestionnaires des actions
`0x35586` (prog), des réactions `0x3a0f4` et `0x3b820`, des sous-états `0x3541c`, des sous-actions du
combattant `0x3e806` (15 mots de déplacement vers le code, 16 pour le voleur), comportements au contact
des tirs joueur `0x267c4`.

### Combat : `data/combat.json`

Réaction d'un joueur touché (`combat/contact.rs`, 0x2749e et 0x2756c). **Région 0x1067a2-0x1069f8** (après
les fiches des caisses), avec trois mots de bourrage `0x4e71` (`pad/…`).

| Nom | Adresse | Lignes x format | Sens |
|---|---|---|---|
| `enemy_attack_reactions` | `0x1067a4` | 91 x `u8*4` | par identifiant d'attaque de l'ennemi (`+0x44`) : action du joueur au sol (`ground`), en l'air (`air`), puissance (`power`), puissance en difficulté > 5 (`power_hell`) |
| `enemy_shot_reactions` | `0x106912` | 57 x `u8*4` | idem pour les tirs ennemis |
| `shot_guard` | `0x27690` (données) | 80 x `u8` | par type de tir ennemi : 1 parable par certains équipements, 2 magie, 0 rien (jusqu'au code 0x276e0) |

Code, pas des données : comportements au contact des tirs joueur `0x267c4` (adresses de routines).

### Ennemis : `data/enemies.json`

Indice « par type » des tables de `reactions.rs` et des immunités : `+2` (type), plus 32 hors de la classe
ENEMY (chefs), soit 55 entrées (32 ennemis, 23 chefs).

| Nom | Adresse | Lignes x format | Sens |
|---|---|---|---|
| `anim_sets` | `0x3de58` (données) | 55 x `addr` | jeu d'animations de réaction par type ; une valeur < 0x10000 est le déplacement (depuis la table) d'une petite routine qui ajoute la variante `anim_variants[+0xbf]` puis prend le jeu dans son immédiat (`movea.l #jeu,A4` à `+6`) : types 13, 19, 34 et 54 |
| `anim_variants` | `0x3e28a` | 4 x `u8` | ajout au numéro d'animation selon `+0xbf` |
| `no_grab_down` | `0x3e1ee` | 64 x `u8` | non nul : le type ne reste pas au sol quand il est saisi |
| `launch_speeds`, `launch_counters` | `0x3bdcc`, `0x3bdec` | 4 x `i16*4`, 4 x `u8` | projections prédéfinies : (vx, vy, vz, gravité) 8.8 et compteur `+0x90` (0x3bda2) |
| `player_hurt_anims` | `0x3bb52` (données) | 12 x `addr` | jeu d'animations des réactions du joueur, par personnage (0x3bb16) |
| `immunities` | `0xfe78` (prog) | 55 x `x32` | masque d'immunités rangé en `+0x94` à la création (0xfe5a) ; `0xffffffff` : insensible à tout |
| `hp_boss`, `hp_enemy` | `0x458c`, `0x5c8c` (données) | 184, 224 x `u16*16` | PV initiaux (`+0x60`, `+0x62`, `+0x64`) : ligne `type x 8 + 4 x (difficulté > 5) + (joueurs - 1)`, colonne `(A5+0x7f92 >> 1) & 15` (rang) ; 23 types de chefs (0x453c), 28 types d'ennemis (0x4544). Région `hp` 0x458c-0x788c |
| `approach_tables` | `0xe93e` (données) | 65 x `addr` | par type de trajectoire `+0x79` : liste de points d'approche (0xe76c, `ai.rs`) |
| `approach/k.count`, `approach/k` | `0xea42`… | 1 x `u16`, n + 1 x `i16*2` | une liste (k = première trajectoire qui la désigne) : mot n, puis n + 1 décalages (dx, dy) autour de la cible ; région `approach` 0xe93e-0xeff2, 52 listes |

Code, pas des données : sons de coup et de mort par type `0x3e2ac` / `0x3e3a4` (62 adresses de routines
chacune, `rts` 0x3e2aa = rien), relevé par type `0x3e000`.

### Caisses : `data/boxes.json`

| Nom | Adresse | Lignes x format | Sens |
|---|---|---|---|
| `fiches` | `0x1051ae` | 122 x `addr` | fiche de chaque variété de caisse (`+3`) |
| `fiches/a`, `fiches/b` | `0x105396`, `0x1059da` | 38, 84 x `u8*10 u8*16 u8*16` | fiche de 0x2a octets (0xda082) : 10 octets copiés en `+0x60` (sorte : 0-2 caisse, 3+ coffre), `+0x61`, `+0x62` (butin du centre), `+0x69`, `+0x63..+0x68` (butin par classe de joueur) ; 16 butins tirés au hasard pour `+0x6a` ; 16 effets spéciaux tirés au hasard pour `+0x6b`. 8 octets nuls inutilisés (`pad/fiches`) suivent la fiche `0x1059a8`. Région 0x1051ae-0x1067a2 |
| `loot` | `0xda28a` (données) | 135 x `u8*2` | par numéro de butin : (type, variante) de l'ITEM créé ; s'arrête à la table d'adresses par personnage `0xda398` (taille déduite, aucun index plus grand relevé) |
| `wear_chances` | `0xdb7b8` (données) | 3 x `x16*6` | arme spéciale, bouclier, anneau (`items.rs`, `Porte`) : un masque par valeur du compteur de coups restant (0 à 5) ; l'objet tombe si le bit `rng & 15` du masque est levé |

Code, pas des données : effets spéciaux des caisses `0xda06a` (6 adresses de routines, prog).

### Scènes : `data/scenes.json`

Tables par scène (0 à 114) ou par boutique, lues par les sorties de scène, les caméras et l'interface.
Espace données sauf mention (prog). Régions couvertes : `entry` 0x21156-0x21c28, `camera1`
0x2ef22-0x2f576, `camera2` 0x32484-0x3281c, `counters` 0x51176-0x513e6, `banner` 0xf6fec-0xf71fa,
`bonus` 0x3f6c08-0x3f8e9c, `waves` 0xd4c5e-0xd5ef6.

| Nom | Adresse | Lignes x format | Sens (consommateur) |
|---|---|---|---|
| `scene_marker` | `0x21156` | 115 x `u8*2` | octet copié en `A5+0x3e` / `A5+0x40` (le second octet n'est pas lu) (`entry.rs`) |
| `start_positions` | `0x2123c` | 115 x `i16*8` | (X, Y) d'entrée des quatre joueurs |
| `start_states` | `0x2196c` | 115 x `u8*4` | octet `+0x2c` (orientation) de chaque joueur à l'entrée |
| `palette_a`, `palette_b` | `0x21b38`, `0x21bb0` | 120 x `u8` | paramètres de palette copiés en `A5+0x48` / `A5+0x49` (5 octets au-delà de la scène 114, non relevés) |
| `scene_38_durations` | `0x22c28` (prog) | 5 x `u16` | durée (images) de la scène 38 par nombre de joueurs |
| `exit_sequence` | `0xf3170` (prog) | 110 x `u8` | séquence de sortie de la scène (deux fois son numéro ; `exit.rs`) ; le code suit : les scènes ≥ 110 liraient du code (aucune relevée) |
| `route_scenes` | `0xf4a0c` (prog) | 4 x `u8` | scène suivante selon la réponse au dialogue 70 (séquence 21) |
| `blackout_palettes` | `0xf80c4` | 70 x `i8 u8*3` | fondu au noir (`blackout.rs`) : ensemble de la page 0 (négatif : aucun), puis D0 des pages 1, 2, 3 ; la scène 105 prend la ligne 69 |
| `time_step` | `0xf5a62` | 1 x `x16*2` | décréments BCD du temps : dix secondes, minute (`tally.rs`) |
| `second_bonus` | `0xf5b06` | 4 x `x8` | prime d'une seconde (BCD, seul le premier octet est lu) |
| `total_at`, `label_at`, `payout_at` (prog), `panel_at`, `panel_value_at` | `0xf5c08`, `0xf5c18`, `0xf5f0a`, `0xf5d82`, `0xf5d92` | 8 x `i16` | positions dans la couche de texte (relatives à 0x912000) par joueur (0-3), puis la disposition alternative (+4) |
| `time_bonus` | `0xf6ba0` | 31 x `x32` | prime BCD par pas de temps restant (au plus 30) |
| `hits_bonus`, `treasure_bonus` | `0xf6c1c`, `0x19c0e` | 81, 105 x `x32` | primes BCD des coups et du trésor, par indice de prime (`A5-0x557a`) ; taille jusqu'au code, la fin des tables semble porter d'autres suites de valeurs (lecture non relevée) |
| `stage_names` (prog), `lead_name_first`, `lead_clear_first` (prog) | `0xf6ed2`, `0xf6f48`, `0xf6f6a` | 22 x `addr` / `u8` | bandeau « STAGE CLEAR » : liste de tuiles du nom, colonnes vides avant le texte selon l'ordre |
| `clear_before`, `clear_after`, `stage_name/n`, `glyphs/…` | `0xf6fec`… | `u16` jusqu'au 0 final | listes de tuiles (0xf6f90) : attributs, tuiles ; `0xfffe` colonne vide ; `0xffff` puis attributs ; 0 fin. `glyphs/0xf7010` et `glyphs/0xf71d0` ne sont désignées par aucune table |
| `player_label` | `0xf7320` | 11 x `u16` | étiquette du joueur : paires (tuile haute, tuile basse), `0xffff` final |
| `shop_music`, `shop_welcome` | `0xf7ab0`, `0xf7b88` | 20 x `u8` | par boutique : déplacement dans la table des routines de musique `0xf7ac4` / indice des sons d'accueil `0xf7b9c` (routines : code) |
| `shop_of_scene` | `0xf7ad0` | 90 x `u8` | boutique de la scène (0 : aucune) ; taille jusqu'aux octets suivants, incertaine |
| `shop_exit_list` (prog), `shop_exit_lists` (prog), `shop_exit_messages/0..1` | `0xf7c14`, `0xf7c28`, `0xf7c30` | 20 x `u8`, 2 x `addr`, 2 x `u16` | messages dits en quittant la boutique (listes de numéros terminées par 0) |
| `shop_next_scene`, `shop_chained` | `0xf7d5e`, `0xf7d72` | 20 x `u8` | scène imposée après la boutique (0 : retour), boutique suivie d'une autre |
| `portraits`, `portraits_wide` | `0xf7e92`, `0xf7eb2` | 4, 3 x `x32 u16 u16` | portrait de chaque joueur : adresse dans la couche, tuile, attributs (la disposition alternative n'a que 3 entrées, du code suit) |
| `counter_scripts_a`, `counter_scripts_b` (prog) | `0x51106`, `0x5113e` | 14 x `addr` | comptoirs des scènes 0x45 à 0x52 : script de dialogue de chaque zone (`peff.rs`) |
| `counter_scripts` | `0x51176` | 100 x `u8 u8 i16` | instructions des scripts (interprète 0x514ac/0x514cc) : opération (multiple de 4, 11 opérations par la table de sauts `0x514ec`), paramètre, mot D1 (0 : ouvrir le message D1 ; 8 : saut relatif si `A5+0x5e`...) |
| `counter_zones_a`, `counter_zones_b` | `0x51306`, `0x51376` | 14 x `i16*4` | zones de comptoir : décalage X, largeur, décalage en profondeur, étendue |
| `camera_layers`, `camera_output`, `camera_max_x`, `camera_min_x` | `0x2ef22`… | 115 x `x16` / `u16` | caméra 1 : couches activées (0xbc6), valeur du port 0x400004, butées droite et gauche |
| `camera_frozen`, `camera_hold_min` | `0x2f2ba`, `0x2f332` | 120 x `u8` | caméra figée, « ne recule pas » |
| `camera_row_step`, `camera2_row_step` | `0x2f3aa`, `0x32650` | 115 x `addr` | table de bandes de la scène (son premier mot, la largeur, est le pas de bande ; tables de `levels.json`) |
| `camera2_start` | `0x32484` | 115 x `i16*2` | position de départ de la caméra 2 |
| `popup_origins` (prog), `popup_bottoms` (prog) | `0x19f6c`, `0x1a25e` | 8 x `i16`, 9 x `u16` | origine des bulles par emplacement (4 grands, 4 réduits) ; tuiles du bord bas par reste de hauteur |
| `markers`, `markers_alt` | `0x1eb48`, `0x1eb58` | 4, 3 x `x16 u16` | (x, y) des marqueurs « 1P »... ; la disposition alternative (bit 1 de `FOUR_PLAYERS`) n'a que 3 entrées |
| `door_a`, `door_b` (prog) | `0xd84e8`, `0xd8e5e` | 3 x `i16*3 u8*10` | portes de passage SET B 27 et 29 par variante : X, Z, Y, `+0x48`, `+0x46`, scène d'arrivée `+0x4f`, `+0x1b`, 6 octets non lus |
| `wave_start_kind`, `wave_end_kind` (prog) | `0xd4a0e`, `0xd4bdc` | 20 x `u8` | forme du démarrage et de la fin des générateurs de vagues, par variante (15 employées) |
| `wave_scripts` | `0xd4c5e` | 15 x `addr` | script de chaque variante du générateur de vagues SET B |
| `wave/k.timer`, `wave/k`, `wave/k.end` | `0xd4c9a`… | `u16` ; n x `i16 u16 i16*3 u8*6 u16 u16` ; `x16` | délai initial ; entrées (délai > 0 ou attente du nombre de vivants si < 0, rang minimal, X, Z, Y, joueurs minimum, routine de création, type, variante, deux paramètres, `+0x42`, position absolue) ; mot `0x8000` |
| `bonus_backdrop_alt`, `bonus_backdrop` (`.size`, `.end`) | `0x3f6c08`, `0x3f7e0e` | (lignes, colonnes), puis cases `u16*2` (code, attributs) par colonnes, puis `0xffff` | décors de la partie bonus selon `A5+0x70` (`bonus.rs`, 0xf67fa) |

Code, pas des données (documentés, restent dans le moteur) : allocateurs des classes d'apparition
`0xf2f38` (mots de déplacement vers le code), sauts des séquences de sortie `0xf31ea`, scripts de transition
`0xf81fa`, routines de musique `0xf7ac4` et d'accueil `0xf7b9c` des boutiques, table de sauts de
l'interprète des comptoirs `0x514ec`, routines de création des vagues `0xd4b92`.

### Divers : `data/misc.json`

`texts` : textes d'octets découpés en lignes pour la lecture (`TextBlock`, chaque ligne écrite comme les
textes de `texts.json`, `{xx}` pour les autres octets ; `TextBlock::bytes` les recolle).

| Nom | Adresse | Contenu | Sens |
|---|---|---|---|
| `viewer_counts` | `0x2a85fc` | 2 512 x `u8` | un octet par script du visualiseur de sprites du menu de test caché, affiché plus un (0xee952, D6 = `+0x66`) ; même ordre que les noms. Région `viewer` 0x2a85fc-0x2ae81e |
| `viewer_names` (texte) | `0x2a8fcc` | 2 512 lignes | noms des scripts du visualiseur (`pl00_000`, 8 caractères puis `0xff`), puis `0xff 0xff` |
| `demo/0..29` | `0x2b04de`… | `u16 x8*12` | entrées enregistrées des démonstrations de l'attract : en-tête (0, `0xff` x 12), enregistrements (durée en images, 3 octets d'entrées par joueur `+0x64a..+0x64c`, voir l'enregistreur 0xe05c2-0xe0700), fin (durée `0xffff`). Désignées par le long `+28` des entrées des scripts de démonstration (`modes/demo.rs`, tables 0x1f3c2...). Entre les suites : `pad/…` (`0x4e71`, une fois 0) et un enregistrement de remplissage (`filler/0x2b0850`). Région `demo` 0x2b04de-0x2b45d8 |
| `credits` (texte) | `0xd01de` | 82 lignes | générique de fin (0xd0108, pointeur posé en 0xd00ee) : octets = tuiles ASCII écrites en colonne, `/` fin de ligne (défilement), `0xff n` attributs n, `0xfe` fin |

Scripts des démonstrations (`modes/demo.rs`) : `demo_scripts` (0x1f348, prog : les deux scripts de
l'interpréteur A, alternés), puis `demo_script/k.n` (entrée n du script k ; k = 2 : interpréteur B,
0x1f756), région `demo_scripts` 0x1f3c2-0x1f818 :

| Opération | Format | Champs |
|---|---|---|
| scène (0 ; toute valeur positive pour B) | `u8 u8 u16 u8*24 addr` (32 octets) | `op`, `scene` (`A5+0x34`), `frames` (durée), 4 x (personnage `c` (négatif : absent), case `s` (`+0x15d`), `w` (`+0x10d`), `h` (`+0x10e`), objet `i` (case `+0x120 + 16 s`), `u` non lu), `inputs` (entrées enregistrées, `demo/k`) |
| son puis scène (2) | `u8 u8 u8` puis la scène sans `op` (34 octets) | `sound` (index de la table de routines 0x1f3aa), `skipped` |
| présentation (autre) | `u8*4` | `op`, `present`, `kind`, `character` |
| fin (négatif) | `x8` | suivie d'un octet de bourrage `0xff` |

Reste à relever le lecteur des entrées enregistrées (seul l'enregistreur est identifié).

### Dialogues : `data/dialogs.json`

Fenêtres de dialogue (SET 24, `0xd7056`, `families/set_b.rs` module `dialog`) et bulles (SET 10,
`families/set.rs` module `dialogue`) des trois langues de la ROM européenne : français (`fr`), anglais
(`en`), allemand (`de`). Le jeu choisit la langue par la table (le code réécrit ne lit que l'anglais).

| Langue | Région | Définitions (256) | Messages (menu de test) | Textes |
|---|---|---|---|---|
| `fr` | `0x1cd000-0x1d78d0` | `0x1cd000` | `0x1cefae` (444) | octets (`high`) |
| `en` | `0x1d78d0-0x1e78da` | `0x1d78d0` | `0x1d986c` (466) | tuiles (`tiles`) |
| `de` | `0x1e78da-0x1f1b5e` | `0x1e78da` | `0x1e9876` (444) | octets (`high`) |

* `xx/definitions` : 256 adresses, une par variante de SET 24 (0xd7056 : `+3`).
* `xx/defs@…` : définitions de 18 octets jointives (0xd7124), format `u8*4 addr i16*2 u8 i8 u16 u16` :
  colonne et ligne de la fenêtre (cases 8x8 de la couche 1), petit cadre (`+0x18` : hauteur 0xb au lieu
  de 0xf), lignes (`+0x19`), `text` (`+0x24`), X, Z (`+8`, `+0xc`), sorte (`kind`, `+0x2f`, 24 valeurs),
  `next` (`+0x1a` : positif, la fenêtre suivante est la définition qui suit, à la fermeture), attente
  (`+0x32`, 0xffff : jusqu'à une touche), paramètre (`+0x34`). Le sens de `text` dépend de la sorte
  (0xd7196, construction 0xd7466) : texte (sortes 0, 3, 6-9, 13, 14, 16-18, 21-23) ; liste de 2
  définitions, une ou plusieurs joueurs (1) ; liste de définitions indexée par `+0x23` (11) ; liste de 2
  textes (10 : texte relié, texte ; 15 : un ou plusieurs joueurs) ; base de définitions voisines (5 :
  décalage `kind5_steps` 0xd7218 ; 19 : `+0x42` x 18 ; 20 : 0 ou 36) ; rien (2, 4, 12 : textes
  construits depuis les joueurs).
* `xx/list@…` : listes d'adresses (définitions ou textes) ; `xx/messages` : tous les textes, pour le
  visualiseur de messages du menu de test (table `message_lists` 0xf2dc4, prog : français, anglais).
* Textes (`texts`, `xx/text@…`) : `tiles` = mots de la grande police (`0x1800` espace, `0x1880 +` ASCII ;
  commandes négatives suivies de deux mots, `0xfffe` d'un seul ; `0x7000-0x7009` substitutions : nombre
  de joueurs, noms...) jusqu'au mot 0 ; `high` = octets ASCII + 0x80 (`0x80` espace, `0xff n m` saut de
  ligne, `0x04 n` lettre accentuée, `0xfe n`) jusqu'à `0xfd`, puis un octet nul si la longueur est impaire.
  Un texte désigné en son milieu (suite après un saut de page) coupe celui qui le contient : chaque
  morceau va jusqu'au début du suivant.
* Couverture : les régions sont parcourues depuis les tables comme le fait le jeu ; les trous restants
  sont classés (définitions plausibles, textes, listes) ; il ne reste que deux mots de bourrage `0x4e71`
  (`xx/raw@…`).
* Bulles : `bubble_tables` (0xcc438, prog : 3 adresses, par langue `A5+0x72`), `xx/bubbles` (51 adresses
  par langue, région `bubble_lists` 0xcc444-0xcc6a8), puis pour chaque bulle `xx/bubble@…` (largeur,
  taille) et son texte `xx/bubble_text@…` (région `bubbles` 0xcc6d2-0xcdca8 ; du code sépare les deux).

* Construction des textes (module `dialog`) : `names` (0xd7604, 5 adresses) et `name@…` (noms prédéfinis :
  mot n, puis au plus n + 1 codes de caractères, `0xff` termine ; région `names` 0xd7604-0xd764a) ;
  `players_one_two`, `players_one_two_b` (0xd7d96, 0xd7da6 : texte selon le nombre de joueurs - 1,
  jetons 0x7001/0x7002), `class_titles` (0xd7db6 : titre par classe), textes `build_text@…` (tuiles :
  séparateur `0xd7e26`, titres « Fighter »..., quelques textes en tuiles japonaises) ; `name_chars`
  (0xd7e8a, 133 mots : tuile de chaque code de caractère des noms) ; `item_lists` (0xd7a20, 4 listes
  d'objets terminées par 0xff), `item_texts` (0xd7a48, prog, 5 adresses) et `item_text@…`. Région `build`
  0xd7d96-0xd801c.

* Passage de niveau (SET C 7, `families/set_c_finale_c.rs`) : `levelup_tables` (0xceaa2, prog : une
  table par classe), `levelup_range` (0xce728, prog : niveaux min et max de la classe, un
  enregistrement par niveau), `levelup/<classe>.lists` (3 listes de messages : anglais, français,
  allemand), `levelup/<classe>` (20 octets par niveau atteint : PV max `+0x60`, déplacement du message
  dans la liste pour la variante paire et impaire du personnage, octet `stats` (négatif : rien) puis 9
  minimums de `+0x151..`, nouvel et ancien objet `+0x10d`, nouvel et ancien objet d'une case `+0x140..`
  (0xff : rien)), `levelup/<classe>/<langue>` (adresses des messages) ; région `levelup`
  0xcebba-0xcf15a. Messages `levelup_text@…` (région `levelup_texts` 0xd0b90-0xd40e6 : anglais en
  tuiles, français et allemand en octets ; dans ceux-ci `~` est l'octet 0xfe, commande suivie d'un
  octet ; `0xff n m` fin de page). Cadre : `levelup_frame_offsets` et `levelup_frame_by_step` (0xce7ac,
  prog : cadre par palier), `levelup_frames` (0xce7e4, prog : adresse dans la couche, largeur),
  `levelup_names` et `levelup_name@…` (0xce9a4 : noms prédéfinis, comme `names`).

Restent à extraire ou à décoder : la sémantique complète des commandes du moteur de texte (cases
`A5+0x5c6`) et la table des tuiles latines `0xc9216` (déplacements vers des suites d'octets, noms des
joueurs en langue européenne).

## Inventaire des données de jeu restantes

Toutes les tables de l'inventaire du deuxième tour sont extraites (`data/players.json`, `combat.json`,
`enemies.json`, `boxes.json`, `scenes.json`, `misc.json`, `dialogs.json`). Les tables d'adresses de code
restent dans le moteur (listées dans le paragraphe « Code, pas des données » de chaque section). Données
relevées mais non extraites :

| Domaine | Adresse | Structure | Consommateur |
|---|---|---|---|
| Chef 11 (chaîne) | `0x1069f8`, `0x106cb8`... | lignes de 32 octets par pose `+0xaa`, trajectoires (pointeurs puis tables de 0x100 octets par orientation) ; suit la région `reactions` de `combat.json`, fin non mesurée | `families/boss_11/chain.rs` |
| Objets | `0xda398` | 12 adresses par personnage (suit la table `loot`) | `families/items.rs` |

## Palettes des scènes : `scene_palettes.json`

Les palettes ne sont pas construites par le code : chaque scène désigne des palettes de la
bibliothèque (`palettes.png`, ROM `0x253000`), par tables (`palette.rs`, [video.md](video.md) §V3).

```json
{"library":"0x253000","stages_rom":"0x2ae81e","stages":[[0,1,2, ...,-1,-1], ...],
 "extra":{"rom":"0x1a41c","program":false,"count":3,"entries":["0x0","0x28d7c0", ...]},
 "sets":{"rom":"0x1a3fc","program":false,"count":32,"entries":[...]},
 "pages":[{"rom":"0x1a62c","program":true,"count":32,"entries":[...]}, ...]}
```

* `stages[s]` : les 32 palettes de la page 0 de la scène `s` (0 à 114), en numéros de palettes de la
  bibliothèque (déplacement / 32) ; -1 : palette laissée en place (`0xffff` en ROM). 0x1a36c n'en charge
  que 29, sauf aux scènes 0x45 à 0x59 ; les entrées 29 à 31 des autres scènes sont gardées telles quelles.
* `extra` : par scène, adresse de 3 palettes copiées à la suite (0 : aucune).
* `sets` : 8 ensembles de 32 palettes (0x1a3d4, D0 = 0 à 7).
* `pages` : pages 1 (D0 = 0 à 2), 2 et 3 (par scène) : adresse de 32 palettes consécutives. Ces tables
  sont lues en adressage relatif au PC, donc dans l'espace programme déchiffré (`program`). Deux entrées
  de la page 3 valent 0 (scènes 0x58 et 0x59 : le jeu copie alors l'adresse 0 de la ROM).
* Toute adresse non nulle est dans la bibliothèque, alignée : palette `(adresse - 0x253000) / 32`
  (`ScenePalettes::library_index`).

Restent dans le code : le choix des pages et des sources de fondu (scènes 0x2f et 0x42, `0x1a296`), les
palettes d'effet (`0x3af8`, données `0x25a640`, dans la bibliothèque), l'éclair (`0x1ab56`).

## Terrain : `terrain.json`

Le numéro de terrain sous un point est `attributs >> 7` de la case du scroll 2 (donc déjà dans
`levels.json` : bits 7 à 15 des attributs des blocs), sauf aux scènes 2 et 0x14, qui lisent une carte
fixe (0x24b0, `video.rs`). Le numéro indexe une table de types (`terrain.rs`).

```json
{"maps":[{"rom":"0x3f8e9c","stage":2,"dy":-16,"columns":[[768,8963, ...16 mots...], ...]}, ...],
 "kinds":[{"rom":"0x3425c","routine":"0x33fe4","kinds":[{"genre":0,"routine":0,"flip":0,"shift":0,"edge":0,"stop":1,"unused":[0,0]}, ...]}, ...]}
```

* `maps` : colonnes de 16 pixels de large ; dans une colonne, le mot `k` couvre `(y + dy) / 16 % 16 =
  15 - k`. `FixedMap::word(x, y)` refait le calcul de 0x24b0 (colonne `(2 x) & 0xffe0` étendue en signe,
  rang `0x1e - ((y >> 3) & 0x1e)`). Carte de la scène 2 : 112 colonnes (les 16 dernières nulles, jusqu'à la
  seconde carte) ; scène 0x14 : 96 colonnes, suivies du remplissage `0xff` de la fin de la ROM.
* `kinds` : 128 types par table (la seconde suit la première ; du code suit la seconde). Les deux tables
  ne diffèrent qu'aux types 3, 4, 7, 8 et 67, 68, 71, 72 (obstacles pleins dans la première, libres
  dans la seconde) ; les types 64 à 127 répètent les types 0 à 63. Champs : voir
  `Kind` et [joueur.md](joueur.md) (collision avec le décor).

## Lectures de la ROM programme par le Rust

Mesure de ce que le code Rust (`ddsom-game`) lit comme données dans la ROM programme (0x000000-0x3fffff),
et de la part que les ressources reconstruisent (`Program`).

**Méthode.** La fonctionnalité `rom-trace` de `cps2` (et de `m68k`, qui signale dans `IN_STEP` qu'une
instruction 68000 s'exécute) compte chaque mot lu par le bus : `Cps2Bus::read_word` pour les lectures
en espace données (`rd8`, `rd16`, `rd32`, `data16`, `data32`...) et `fetch16`/`read_prog16` pour l'espace
programme (`prog8`, `prog16`, `prog32`, `rom8`, `rom16`, `rom32`, `m68k::decode` des routines de durée). Les
lectures faites par les instructions 68000 interprétées (`Cpu::step`, donc aussi les opcodes) sont ignorées.
Hors de cette fonctionnalité, rien ne change (code absent). Le journal de première lecture donne, pour les mots
non couverts, les trois premiers appelants dans `ddsom-game/src`.

```sh
cd port
CARGO_TARGET_DIR=/tmp/cible cargo build --release -p ddsom-game --example lectures_rom --features rom-trace
cargo build --release -p ddsom-assets --examples
target/release/examples/couverture_rom <assets> --octets /tmp/couvert.bin          # couverture mot par mot
DDSOM_COUVERT=/tmp/couvert.bin /tmp/cible/release/examples/lectures_rom <roms> <eeprom> lectures.txt \
    ../scenarios/bot30k.txt:12000 ...                                              # machine hybride, natives Rust
target/release/examples/couverture_rom <assets> lectures.txt [écart-hexa]          # zones non couvertes
```

**Scénarios** (15 x 12 000 images, machine hybride) : `bot30k` (guerrier), `bot_char1`, `bot_char4`, `bot_2p`,
`boss/boss5_scene58`, `boss/boss13_dragon`, `boss/boss21_scene44`, `modes/attract`, `modes/ending`,
`cov/cov_service_tests` (menu de test), `cov/cov_credits_gros`, `cov/cov_tour8_c1`, `enemy/elf_scene96`,
`scene/scene_03`, `scene_end/end_02`. Les zones lues ne sont donc qu'une partie des tables : un scénario de plus
peut en ajouter.

**Résultat.**

| Espace | Octets lus | Couverts | Non couverts |
|---|---|---|---|
| Données (lectures absolues) | 781 988 | 763 024 (97,6 %) | 18 964 en 532 zones |
| Programme (lectures relatives au PC, déchiffrées) | 41 758 | 1 332 | 40 426 dont 32 316 de chronométrage |

Octets lus et couverts, par fichier d'assets (données ; programme) : `sprites/frames.json` 380 128 ;
`sprites/anims.json` 112 880 ; `levels.json` 125 042 ; 256 ; `palettes.png` 79 072 ; `data/rest.json` 26 070 ; 106 ;
`data/misc.json` 16 466 ; `data/spawns.json` 7 482 ; `sprites/tables.json` 3 150 ; `texts.json` 2 838 ; 36 ;
`data/dialogs.json` 2 594 ; `scene_palettes.json` 2 266 ; 324 ; `data/players.json` 1 680 ; 410 ; `data/scenes.json`
1 498 ; 76 ; `data/boxes.json` 446 ; `data/enemies.json` 736 ; 116 ; `terrain.json` 508 ; `data/combat.json` 168.

**Ajoutées par cette mesure** (`data/rest.json`, `extract/src/rest.rs`, 1 497 tables, test `restes_sans_perte`) :
scripts de cases de tuiles des SET 4 et 6 (`tile_anim_entries/4` et `/6`, 53 + 32 entrées de 16 octets en
espace programme ; `tile_anim/<script>.<n>` : durée, colonnes, lignes, puis un long par case ; 0x3ec70a-0x3f5448) ;
scripts de palettes des SET 0 (`palette_script_table` 0xba57a, 79 entrées ; `palette_script/<script>.*` : en-tête,
blocs, fin / relance / saut ; `palette_script_transition` 0xba44c et `palette_script_end` 0xba4ca, espace
programme) ; sous-objets des joueurs (`overlay_body` 0x395d4, `overlay_shield` 0x39604, `overlay_anims` 0x396e8,
`overlay_palettes` 0x397e0, et les 10 tables de lignes `overlay_rows/<adresse>` de 0x1147c6 à 0x11bffa : 1 407
lignes de 8 octets, animation, dx, dz, retournement, derrière, plus un mot `nop` de bourrage) ; palettes d'aura
(`aura_offsets` 0x378aa en espace programme, `aura_b` 0x378c2, `aura_c` 0x384c2, `aura_normal` 0x390c2) ; piles
initiales des 16 tâches (`task_stacks` 0x534) ; panoramique du son (`sound_pan` 0xee04e, 48 octets).

**Zones lues et non couvertes, par sous-système** (octets ; `d` : espace données, `p` : espace programme).

| Sous-système | d | p | Rôle des zones |
|---|---|---|---|
| Chronométrage (`chemin.rs`, `timing.rs`, `objects/cost.rs`, `ai.rs`, `combat.rs`) | 0 | 32 316 | pas des données : les routines natives qui comptent encore les cycles du 68000 décodent le code d'origine (`m68k::decode`) ; disparaît avec le 68000 |
| Joueurs (`player/`) | 782 | 902 | réactions aux coups (`hit_reactions.rs` 0xf1cc4-0x1010b2 : seuils, durées d'étourdissement, lignes de poussée), combos (`combos.rs` 0xf193a) ; en p : tables de décalages des états de blessure (`hurt.rs`), des scripts de cinématiques (`scripted.rs`, `cinematics.rs`) et des routines par personnage (`characters.rs`) (h) |
| Tirs et effets des joueurs (`families/joueur/`) | 364 | 698 | trajectoires de l'anneau (`peff_ring.rs` 0x1c9db6-0x1ca034 : vitesses x, z) ; en p : déplacements des coquilles (`pshell.rs`) |
| Ennemis (`families/enemy_*`, `reactions.rs`) | 1 136 | 1 294 | tables d'attitude par niveau (octets de déplacements : squelette 0x5942a, bête 0x7473e, trog 0x5af66, ghoul, elfe, kobold, hibou, guerrier 0x77634) ; cercle de réactions 0x1c94b6 ; en p : tables d'adresses de routines et de déplacements |
| Boss (`families/boss_*`) | 240 | 694 | tables de décision (octets) des chefs 5, 7, 17, 21, vitesses du chef 15 (0x1c67b8), oscillation du dragon (0x8faa2) |
| SET : décors, portes, rideaux (`families/set*`) | 1 986 | 664 | tables par variante des scènes de SET C (`set_c_scene.rs` 0xbcfc6-0xc821c : entrées de 0x14 octets rang, difficulté, positions), finales (0xce51a), positions (0xd4198) ; en p : table du rideau (0xbcea4) |
| Effets, tirs ennemis, objets, caisses (`families/effects*`, `eshl*`, `sobj`, `items`) | 2 220 | 1 622 | rideau de la porte du SET E (`effects_e.rs` 0xadad0-0xae118 : pas du rideau puis tuiles), tables de pièces et de couleurs ; en p : types d'objets de SET (`sobj.rs`), vitesses de pièces |
| Affichage (`hud/`, `text.rs`, `video.rs`, `taches/ecran_fixe.rs`) | 2 732 | 290 | séries de colonnes de tuiles du HUD (`hud/layer.rs` 0x3f54c2-0x3f6300, 3 mots par colonne), glyphes de la barre d'état (0x17bca), chaînes terminées par `@` lues par `text.rs` (0x14e58, 0x1c552, 0xd6926, 0xe04c8), écrans fixes (0x19a1a) ; en p : origines et lettres du HUD |
| Modes, scènes, caméra, démarrage | 1 390 | 268 | scripts de démonstration (`modes/demo.rs` 0x20070), écran de démarrage (0x18faa, 0x133ba), tables de scrolls (`scroll.rs` 0x33bdc, `camera_modes.rs` 0x321fc), classement (0xfc6d8) ; en p : tables des scènes |
| Son (`sound.rs`) | 206 | 720 | listes de genres de sons (0xe6684) ; en p : code des enveloppes lu par les wrappers de son (0xe0ca8-0xede06) |
| Moteur d'objets (`sprites.rs`, `objects/`) | 4 534 | 100 | textes en sprites et listes de morceaux (`sprites.rs` 0x1d8be-0x1db32, 0x201e0), tables de vitesses des mouvements (`motion.rs` 0x1c67ce-0x1ca180 : vx, vy par indice), index d'entrées (`anim.rs` 0xb2b6a, 0x16cb10, 0x1b232c), rangs (0x1f208) |
| IA, combat, boucles de dispatch (`ai.rs`, `ai_aids.rs`, `combat/`, `dispatch.rs`) | 1 680 | 792 | pointeurs et masques d'aides à la visée (`ai_aids.rs` 0x78ae-0x16c78c, `ai.rs` 0xa4f2), tables de dégâts (`combat/damage.rs` 0x3d94) ; en p : les 10 tables de dispatch des pools d'objets (`dispatch.rs` 0x39908-0x39fe4, adresses de code) |
| Accès génériques (`vue.rs`, `mem.rs`) | 1 694 | 74 | lectures simulées (`Vue::rd8`) dont l'appelant n'est pas résolu à trois niveaux : bloc 0x000-0x400 (306 000 lectures : vecteurs ou code, (h)), 0x275a, 0x2dc3e ; pointeurs lus par `mem.rs` (0x4c6de-0x100706, nombreuses petites zones) |

**Lecture des résultats.** Dans l'espace données, les zones non couvertes sont de petites tables disséminées
(moins de 20 Ko au total, 97,6 % des octets lus sont couverts) : tables de vitesses, d'attitudes, de décisions, de
positions par variante, chaînes, listes de pièces ; chacune se décrit comme les tables de `data/*.json` (une ligne
de format et un nombre de lignes) mais demande de lire son consommateur pour en fixer les bornes. Les plus
volumineuses sont les colonnes de tuiles du HUD (`hud/layer.rs`, 1,6 Ko), les listes de morceaux et les textes en
sprites (`sprites.rs`, 1,9 Ko), les tables de vitesses des mouvements (`motion.rs`, 1,7 Ko) et les tables des
scènes de SET C (1,7 Ko). Dans l'espace programme, presque tout (80 %) est du code lu pour compter les cycles
(sans objet pour un moteur sans 68000) ; le reste (8 Ko) est fait de tables lues en relatif au PC, noyées dans le
code des gestionnaires : décalages vers des routines (des adresses de code : elles deviennent des fonctions
Rust) et quelques tables de données (vitesses des coquilles, positions, tables de décision des chefs 17 et 21).
Les lectures de `sound.rs` en espace programme (0xe0ca8-0xede06) sont des octets de code des wrappers de son lus pour les évaluer ; elles disparaîtront avec eux.

**Sans 68000** ([sans-rom.md](sans-rom.md)) : `lectures_rom` tourne sans 68000 par défaut ; plus aucune lecture de
code pour des durées (supprimées au lot 0a), et les zones lues non couvertes, mesurées sur les 269 scénarios, sont dans `data/natives.json`
(brutes, par lecteur) : le jeu sans 68000 tourne avec les seuls assets.

## Ce qui reste à extraire

* Lectures de la ROM programme par le Rust non couvertes : extraites brutes dans `data/natives.json` (2 799 zones,
  84 Ko en espace données, 28 Ko en espace programme, mesure sans 68000 sur 269 scénarios, [sans-rom.md](sans-rom.md)) ;
  reste à les décrire table par table (tableau par sous-système ci-dessus).
* Sprites : entre la table des images et les images, `0x2a85fc-0x2ae81e` (octets et noms des 2 512 scripts
  du visualiseur), les palettes des scènes et `0x2b04de-0x2b45d8` (entrées des démonstrations) sont
  extraits (`data/misc.json`). Les jeux d'animations que seul le code désigne (sans nom) ne sont pas
  listés comme jeux : leurs entrées sont dans `entries` (isolées).
  Les boîtes propres à certaines familles (données qui suivent `0x1ca938`, tables `0x121fc8`... déjà
  couvertes) et les décalages de tirs (`0xd4198`...) restent dans le code ou à inventorier.
* Animations : 35 débuts d'entrées plausibles dont la chaîne est incohérente ne sont pas extraits ; une
  paire d'entrées isolées se chevauche (`0x174c70`/`0x174c78`, l'une est un faux positif). Jeux vides :
  `bs16` (ses trois tableaux sont des copies de ceux de `bs02`, décalages relatifs à `bs02`) et `em09`
  variante 1.
* Décors : dessins écrits dynamiquement par les scripts de scène (blocs chargés par 0x26d4/0x2766 dans
  la couche, décors animés), choix des palettes de fondu (code).
* Textes :
  * **Dialogues et bulles** : extraits (`data/dialogs.json`, trois langues). Restent les tables de la
    construction des textes (noms, caractères), les messages de SET C 7 (`0xceaa2`) et la sémantique des
    commandes du moteur de texte (voir la section des dialogues).
  * **Boutique** (PEFF 43) : les scripts des comptoirs sont extraits (`counter_scripts` de
    `data/scenes.json`) ; leurs textes sont des messages du moteur de texte.
  * **Générique de fin** : extrait (`credits` de `data/misc.json`, 0xd01de-0xd06e5, routine 0xd0108).
* Données de jeu : scripts d'apparition, tables des joueurs, du combat, des ennemis, des caisses, des scènes
  extraits (`data/*.json`) ; ce qui reste est listé dans « Inventaire des données de jeu restantes »
  et, pour chaque fichier, dans le paragraphe « Code, pas des données » ou « Restent » de sa section.
