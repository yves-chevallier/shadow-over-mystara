# Ressources extraites : tables, formats, vérification

L'outil `extract` (`port/extract`) lit les ROMs de l'utilisateur et écrit `assets/` ; la crate
`ddsom-assets` relit ces fichiers. Les fichiers produits dérivent des ROMs : ils restent locaux (dossier
`assets/` ignoré par git), seuls le code, les tests et cette documentation sont versionnés.

```sh
# depuis port/
cargo run --release -p extract -- ../rom ../assets            # tout
cargo run --release -p extract -- ../rom ../assets sprites levels
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

Les trois premières catégories sont décrites dans le `README.md` que l'outil écrit dans `assets/`. Ce
document décrit les définitions de sprites et les cartes.

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

## Ce qui reste à extraire

* Sprites : les tables de cadres lues directement par le code (listes d'adresses d'images, p. ex.
  `0x9e8b8`, décors SET), les octets entre la table des images et les images (`0x2a85fc-0x2b45d8`, rôle
  inconnu), les boîtes de collision et de visibilité (`0x1ca538`), les ombres (`0x11bffa`, `0x11c23a`), les
  marques au sol (`0x1e45c`). Les jeux d'animations que seul le code désigne (sans nom) ne sont pas
  listés comme jeux : leurs entrées sont dans `entries` (isolées).
* Animations : 35 débuts d'entrées plausibles dont la chaîne est incohérente ne sont pas extraits ; une
  paire d'entrées isolées se chevauche (`0x174c70`/`0x174c78`, l'une est un faux positif). Jeux vides :
  `bs16` (ses trois tableaux sont des copies de ceux de `bs02`, décalages relatifs à `bs02`) et `em09`
  variante 1.
* Décors : scroll 1 (cases 8x8, texte et avant-plans), dessins écrits dynamiquement par les scripts de
  scène, palettes des scènes (relevées en jeu par `examples/cartes.rs`), cartes de terrain et de collision
  (`0x3f8e9c`, `0x3f9c9c`).
* Données de jeu (vitesses, IA, vagues d'ennemis), textes, musiques (séquences du pilote Z80).
