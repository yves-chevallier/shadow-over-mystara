# Ressources extraites : tables, formats, vérification

L'outil `extract` (`port/extract`) lit les ROMs de l'utilisateur et écrit `assets/` ; la crate
`ddsom-assets` relit ces fichiers. Les fichiers produits dérivent des ROMs : ils restent locaux (dossier
`assets/` ignoré par git), seuls le code, les tests et cette documentation sont versionnés.

```sh
# depuis port/
cargo run --release -p extract -- ../rom ../assets            # tout
cargo run --release -p extract -- ../rom ../assets sprites levels terrain
DDSOM_ROMS=$PWD/../rom cargo test --release -p ddsom-assets   # vérification sans perte
```

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

Les trois premières catégories sont décrites dans le `README.md` que l'outil écrit dans `assets/`. Ce
document décrit les autres.

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
 "samples_ptr":"0x39e3","samples_rom":"0x49e9","samples":[[...8 octets...], ...290],
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
* **Échantillons** : 290 fiches de 8 octets à l'adresse du mot `0x39e3` (`0x49e9`) : banque, début,
  boucle, fin (mots petit-boutistes), note de référence ; les données PCM sont dans `sounds/`.
* **Tables du pilote** (`tables`, ROM fixe ; `words` : mots petit-boutistes) : `qsound_init_regs` (0xd5,
  12 octets) et `qsound_init_values` (0xe1, 12 mots), `durations` (0x1119, 24 mots), `octave_offsets`
  (0x1149, 16 octets), `semitone_octave` (0x1fc6, 108), `octave_start` (0x2032, 9), `rates` (0x203b, 108
  mots), `fine` (0x2113, 12 x 256 octets), `amplitudes` (0x33e1, 128 mots), `levels` (0x34e1, 128), `attack`
  (0x35e1, 64), `decay` (0x3661, 64), `lfo_rates` (0x36e1, 128), `vibrato` (0x37e1, 128), `tremolo`
  (0x38e1, 128).

Reste dans le code du pilote (et `ddsom-sound`) : le tableau de saut des instructions (0x1179), la
logique d'enveloppe, de LFO et de pan.

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

## Ce qui reste à extraire

* Sprites : entre la table des images et les images, `0x2a85fc-0x2a8fcc` (octets, rôle inconnu) puis
  `0x2a8fcc-0x2ae81e` : 2 462 noms de scripts du visualiseur (`pl00_000`, 8 caractères terminés par
  `0xff`), puis les palettes des scènes et `0x2b04de-0x2b45d8` (inconnu). Les jeux d'animations que seul
  le code désigne (sans nom) ne sont pas listés comme jeux : leurs entrées sont dans `entries` (isolées).
  Les boîtes propres à certaines familles (données qui suivent `0x1ca938`, tables `0x121fc8`... déjà
  couvertes) et les décalages de tirs (`0xd4198`...) restent dans le code ou à inventorier.
* Animations : 35 débuts d'entrées plausibles dont la chaîne est incohérente ne sont pas extraits ; une
  paire d'entrées isolées se chevauche (`0x174c70`/`0x174c78`, l'une est un faux positif). Jeux vides :
  `bs16` (ses trois tableaux sont des copies de ceux de `bs02`, décalages relatifs à `bs02`) et `em09`
  variante 1.
* Décors : dessins écrits dynamiquement par les scripts de scène (blocs chargés par 0x26d4/0x2766 dans
  la couche, décors animés), choix des palettes de fondu (code).
* Textes non extraits :
  * **Dialogues et messages** (fenêtres de dialogue SET 24, `0xd7056` ; bulles SET 10 ; messages numérotés
    créés par `0xc8312`) : le texte est une liste de jetons mots (tuiles de la grande police, négatifs =
    commandes, `0x7000-0x7009` = substitutions : nombre de joueurs, noms, objets) composée dans le tampon
    `0xff2072` et affichée par le moteur de texte (cases `A5+0x5c6`). Définitions des fenêtres : table
    `0x1d78d0` (18 octets : colonne, ligne, indicateurs, texte, position, sorte `+0x2f`) ; tables
    `0xd7218`, `0xd7a48`, `0xd7e8a`, `0xd7604`, `0xd7db6`, `0xd7d96`, `0xd7da6`, `0xd7988` ; messages de
    SET C 7 : `0xceaa2` ; bulles SET 10 : `0xcc438`. Le moteur de texte et la table des messages restent à
    décoder (format des jetons, police) avant extraction.
  * **Boutique** (PEFF 43, interprète `0x514ac` / `0x514cc`) : scripts d'instructions de 4 octets
    (opération multiple de 4, paramètre, mot `D1`), 11 opérations par la table de sauts `0x514ec`
    (0 : ouvrir le message `D1` ; 8 : saut relatif de `D1` octets si `A5+0x5e` ; achats, paiement BCD,
    inventaire...). Scripts désignés par les zones de comptoir (`0x51306` / `0x51376`, champs `+0x1c` /
    `+0x58`) ; les textes eux-mêmes sont des messages du moteur de texte.
  * **Générique de fin** (`0xd01e0-0xd0692`, noms du staff, lignes séparées par `/`, codes `0xff n`) :
    routine d'affichage non relevée.
* Données de jeu (vitesses, IA, vagues d'ennemis) : voir l'inventaire ci-dessous.
