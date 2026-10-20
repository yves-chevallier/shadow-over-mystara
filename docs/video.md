# Vidéo : couches, tilemaps, palettes et fondus (V1, V2, V3)

Code : `port/ddsom-game/src/video.rs` (V1, blocs de tuiles, terrain), `palette.rs` (V3), `text.rs` (texte).
Toutes les routines ci-dessous sont des natives validées (durée exacte au cycle près, sauf mention).

## Carte graphique

| Zone | Adresse | Contenu |
|---|---|---|
| Object RAM | `0x708000` | liste de sprites (voir `sprites.rs`) ; deux banques, `0x8040e1` choisit la banque écrite |
| Scroll 1 | `0x910000` | table de cases 8x8 ; la couche de texte `0x912000` en est une fenêtre |
| Scroll 2 | `0x914000` | cases 16x16 (0x4000 octets) |
| Scroll 3 | `0x918000` | cases 32x32 (0x4000 octets) |
| Palettes | `0x91c000` | quatre pages de `0x400` octets (voir plus bas) |

Registres CPS-A (`0x804100`, mots) : `+0` base de la table d'objets (valeur de `A5+0x5ba`), `+2/+4/+6`
bases des couches 1 à 3 en unités de 256 octets (`0x9100`, `0x9140`, `0x9180`). CPS-B `0x804166` : activation
des couches (image dans `A5+0x14`). `0x804122` / `0x804108` : écrits par 0x2a52 (scroll 3 / réglage).

Une **case** de couche est un long : mot haut = numéro de tuile, mot bas = attributs (palette en bits 0 à 4,
miroirs en 5 et 6...). Les cases sont rangées par colonnes pour la couche de texte (`+4` ligne suivante,
`+0x80` colonne suivante) et par lignes dans les blocs chargés par 0x260c/0x26d4/0x2766.

## V1 : primitives (`video.rs`)

| Adresse | Rôle |
|---|---|
| `0xb40` / `0xb46` | jauge de chargement : appelée par chaque attente 0xa40. Remplit les 32 premiers longs de `0x912000` avec D1 (0xb40 : `0x00200003`, espace d'attribut 3), sauf si le bit 10 de `A5+0x7fd0` est posé. Durée : 2 chemins |
| `0xb96` / `0xbac` | verrous des compteurs de pièces : `A5+0x10` et `A5+0x17` reçoivent (OU) ou perdent (ET non) les bits D0, puis les bits forcés `A5+0x18` sont réappliqués ; D1 = bits forcés |
| `0xbc6` | active les couches D0 : `A5+0x14 \|= D0`, puis (sauf si `0xff0eab` ≠ 0, écran figé) écrit `0x804166`, `0x804100` ← `A5+0x5ba`, `0x804102/04/06` ← `0x9100/9140/9180` |
| `0xbfc` | désactive les couches D0 (`A5+0x14 &= ~D0`, écrit `0x804166`) ; D0 revient complémenté |
| `0xc58` | liste d'objets vide : écrit l'entrée de fin `(0x00008000, 0x18000000)` en `0x708000`, bascule `A5+0x5bc` et `0x8040e1`, le tout deux fois (les deux banques) |
| `0xca8` `0xcc8` `0xce8` | remplissage des couches 1/2/3 : cases `(0x0020 / 0x1800 / 0x1f00, 0)`. Les trois bornes finissent à `0x91c000` |
| `0xc0c`, `0xc32` | effacement total (couches coupées, objets, trois couches) ; non natives (0xc32 attend ; 0xc0c dépasse une image) |

## V2 : terrain et blocs de tuiles (`video.rs`)

* **0x24b0 / 0x24a8** : numéro de terrain sous le point `(D0, D1)` (ou de l'objet A0 : x `+8`, y `+0x10`).
  Résultat dans D2 = bits 7 à 15 du mot d'attribut de la case. Scène 2 et scène 0x14 : carte fixe en ROM
  (`0x3f8e9c`, `0x3f9c9c`, mots, y corrigé de -/+ 0x10). Sinon : case de la couche scroll 2
  (`0x914000 + 0x3000 - (y & 0x300)*16 + 0x3c - ((y>>2) & 0x3c) + ((4 x) & 0xfc0)`), avec x et y corrigés de
  la différence des deux caméras (`A5+0x506/50a` moins `A5+0x556/55a`) quand `A5+0x36` ≠ 0. Z = numéro nul.
* **0x255a / 0x25e8** : chiffres dans la couche de caractères (une colonne `0x80` octets plus loin par
  chiffre, tuile = valeur, attribut 0). 0x255a : D0 (saturé à 99) en dizaines (A1) et unités ; 0x25e8 : D0 en
  quatre chiffres hexadécimaux. Non exercées par les scénarios (validées par le fuzz).
* **0x260c, 0x26d4, 0x2766** : chargeurs de blocs. D0 choisit une grille d'indices (mots, en ROM) ; chaque
  indice désigne un bloc de cases en ROM. Le long de chaque case reçoit un décalage de tuile.

  | Routine | Grille | Bloc | Indices (ROM) | Cases (ROM) | Décalage de tuile | Pas de ligne | Page de retour |
  |---|---|---|---|---|---|---|---|
  | 0x260c | 8x8 | 4x4 | `0x1f1b60 + 0x80 D0` | `0x1f3b60 + 64 i` | `0xf800` | `0x80` | aucune |
  | 0x26d4 | 8x8 | 2x2 | `0x1f9ba0 + 0x80 D0` | table de code `0x275a`, par tranche de 32 Ko d'indices, `+16 i` | `0x1800` | `0x40` | `0x1000` |
  | 0x2766 | 4x4 | 2x2 | `0x2459c0 + 0x20 D0` | table de code `0x27e6`, par tranche de 8 Ko d'indices, `+16 i` | `0x1f00` | `0x20` | `0x800` |

  Fin de ligne de blocs : la destination avance de `cases x pas de ligne` et revient dans sa page (A1 mod
  taille de page). Rend D0 = `0xffff`, A1 = destination finale, A2 = fin des indices. 0x2a52 (non native) appelle
  0x26d4 huit fois pour remplir `0x916000-0x917c00` (scroll 3).

## V3 : palettes (`palette.rs`)

Une couleur est `0xLRGB` : L luminosité (0 = noir, 0xf = pleine), puis rouge, vert, bleu sur 4 bits. Page k =
`0x91c000 + 0x400 k`, 32 palettes de 16 couleurs (32 octets). `A5+0x48e` : octet des pages concernées
(bit k = page k).

### Luminosité

`0x1686` pleine (OU 0xf000), `0x168c` nulle (ET 0x0fff), `0x168e` OU D0, `0x173c` remplace par D0.

### Bibliothèque de palettes

`0x253000-0x293000` : palettes de 32 octets. Par scène, `0x2ae81e + 0x40 s` donne 32 déplacements (mots) dans la
bibliothèque ; un déplacement négatif laisse la palette en place.

| Adresse | Rôle |
|---|---|
| `0x1a366` (D0), `0x1a360`, `0x1a350` | palettes de la scène D0 (29 palettes, 32 pour les scènes `0x45`-`0x59`), dans la page 0 ; 0x1a350 : dans scroll 1 (`0x910000`). Puis, si la table de code `0x1a41c` a une entrée pour la scène en cours (`A5+0x34`), 3 palettes supplémentaires |
| `0x1a3d4` | ensemble D0 : 32 palettes dont l'adresse est dans la table de code `0x1a3fc` |
| `0x1a60a`/`0x1a608` | page 1 : 32 palettes, table `0x1a62c` indexée par D0 (0x1a608 : D0 = 0) |
| `0x1a67e`/`0x1a678` | page 2 : table `0x1a6a0`, scène en cours pour 0x1a678 |
| `0x1a8b2`/`0x1a8ac` | page 3 : table `0x1a8d4` |
| `0x1a296` | tout charger pour la scène en cours (pages 0 à 3) et mettre `0x91d7fe` à 0 ; scènes `0x2f` et `0x42` : page 0 dans scroll 1 et sources du fondu `A5+0x492...` (couche 1, tampon `0xff1190`, deux palettes en ROM) |

### Fondus

Deux familles, toutes pas à pas (une image par pas, `A5+0x48f` pas restants, `A5+0x490` délai) :

1. **Fondu de luminosité** (lanceurs 0x144c/0x1460/0x146c, 0x14ba...0x1544, qui attendent par 0xa40 et ne sont
   donc pas natifs) : chaque pas ajoute D1 = ±0x1000 à toutes les couleurs des pages cochées
   (adresses dans `A5+0x492 + 4 k`). `0x1602` traite une page (512 couleurs) ; refuse si la première couleur est
   déjà à la butée (0 ou 0xf000) ou atteindrait 0. `0x164e` : 48 couleurs seulement (fin de la page 0, pour
   le fondu partiel 0x1544/0x15c8). `0x158e`/`0x15c8` parcourent les pages cochées.
2. **Fondu par composante** (lanceurs 0x1ccc2, 0x1ce30, 0x1ce9e, 0x1ced4, non natifs) : chaque pas rapproche
   d'un cran chaque composante RGB. `0x1cfc8` (page A0 vers la palette A1), `0x1d042` (vers la couleur D1),
   `0x1d00e` (vers le blanc) ; `0x1cd7c`, `0x1cdd6`, `0x1cd32` les appliquent aux pages cochées de `A5+0x48e` (la
   source de la page k est en `A5+0x492 + 4 k`). `0x1cf3c` remplit les pages cochées de blanc (`0xffffffff`) par
   `0x1cfb8` (recopie D0-D6,A1 sur 32 palettes).

## B1 : caméras et défilement du décor (`scroll.rs`)

Deux caméras sont des enregistrements de la RAM de travail : caméra 1 en `A5+0x4fe` (couche scroll 2,
cases de 16 pixels), caméra 2 en `A5+0x54e` (couche scroll 3, cases de 32 pixels). Le code d'origine les
adresse par A0. La boucle monde `0x33e20` copie la position de la caméra 1 en `+0x26/+0x2a` puis appelle
`0x2ede0` (caméra 1) et `0x32108` (caméra 2).

| Offset | Rôle |
|---|---|
| `+0x00` | actif (0 : première image de la scène, initialisation propre à la scène) |
| `+0x01` | sous-état du script de mode |
| `+0x08`, `+0x0c` | position cible x, y (le script de mode la déplace) |
| `+0x10`, `+0x14` | position déjà dessinée ; `+0x1c`, `+0x1e` : écart cible - dessiné |
| `+0x20` | mode : décalage dans la table de sauts `0x2f780` (caméra 1) / `0x3281c` (caméra 2), ~31 scripts |
| `+0x22`, `+0x24` | curseur dans la carte : décalage dans la bande, numéro de bande |
| `+0x2e`, `+0x34` | butée gauche (suit `x - 0x60` tant que `+0x3d` est nul) et butée droite de la cible |
| `+0x3c` | drapeau remis à zéro à chaque image ; `+0x3e` : défilement gelé |
| `+0x40` | pas de bande par 256 pixels de y (mot lu dans une table par scène) |

Chaque image (`camera_modes.rs`, natives `0x2ede0` et `0x32108`) : au premier appel d'une scène,
initialisation propre à la scène (tables en ROM `0x2f0ee`, `0x2f008`, `0x2ef22`, `0x2f332`, `0x2f1d4`,
`0x2f3aa`, `0x2f2ba` ; `0x32650`, `0x32484` ; chargement de blocs de tuiles pour quelques scènes) ; ensuite le
**script du mode** (`+0x20`, écrit en Rust : suivi des joueurs par fenêtre `follow`, aiguillage par scène
`stage_script` = `0x309e0` / `0x332c0`, défilements par ligne, secousses, portails...), puis la **suite
native** (`scroll::after_mode` : gel, butées de la cible ; `0x2f86e` / `0x328a8` : recopie du décor).
`0x2f86e` calcule les écarts et, tant qu'il
dépasse un pas (16 pixels, 32 pour la caméra 2), recopie une colonne (19 / 11 cases) ou une ligne (35 / 15
cases) de la carte du décor dans la couche. Ordre des tests : droite, gauche, bas, haut ; après un pas,
l'original reprend au test du même sens. Quirks conservés : le pas à gauche / en haut s'arrête si la position
dessinée devient négative (caméra 1) ou ne dépasse plus un pas (caméra 2, `bls`) ; la caméra 2 ignore une
ligne vers le haut quand `y <= 0`.

Suivi des joueurs : une fenêtre `(far, near, back)` autour de l'étendue (plus grande, plus petite valeur) d'une
grandeur des joueurs retenus ; la caméra avance de 6 pixels au plus quand le plus avancé sort de la fenêtre
(sans perdre le moins avancé) et recule quand le moins avancé est trop en arrière. Fenêtres : x joueurs
`(0xf0, 0x20, 0x70)` ou étroite `(0xf0, 0x5c, 0x5c)`, x objets `(0x160, 0x1c, 0x1c)`, profondeur
`(0x70, 0x30, 0x30)` ou basse `(0x90, 0x38, 0x38)`, objet suivi `(0x88, 0x78, 0x78)` (x : variante « à
saccades » `0x30f6a`, pas de 16 au plus).

### Carte du décor

* Caméra 1 : table d'indices de blocs en ROM `0x1f9ba0` ; chaque indice choisit un bloc de 2x2 cases (16
  octets, quatre cases `(tuile, attribut)`) dans l'une de 4 banques (`0x320f6`, quatre longs lus en espace
  données). La position `p = adresse - 0x1f9ba0` donne : banque `(p >> 16) & 0xc` (octet), indice à
  `0x1f9ba0 + (c & 0xe) + ((c >> 1) & 0x3fff0)` avec `c = p >> 2`, case du bloc `((p >> 3) & 8) + (p & 4)`.
  Les bandes de la carte (1 Ko d'indices) sont désignées par une table propre à la scène : `0x2ff94 + 4 s`
  (espace programme) donne l'adresse d'un tableau de mots ; `bande = mot[curseur]`, adresse `0x1f9ba0 + (bande << 10)`.
* Caméra 2 : mêmes principes avec la carte `0x2459c0`, 16 banques (`0x33a9c`), `c & 6`, `(c >> 1) & ~7`,
  `((p >> 2) & 8) + (p & 4)`, bandes de 256 octets (`0x32d7c + 4 s`).
* Cases écrites : `case + 0x18000000` (caméra 1) / `case + 0x1f000000` (caméra 2) : décalage de tuile. Si
  `A5+0x37` est non nul (vue « terrain » de débogage), la case de la caméra 1 est remplacée par la tuile
  `0x1bc0` ou `0x1d80` (bit 13 de l'attribut) plus le numéro de terrain.
* Écriture en colonnes de la couche : cases consécutives de 4 octets, pages de 0x40 (scroll 2) ou 0x20
  octets (scroll 3) ; en fin de page la destination passe à la colonne suivante (`+0xfc0` / `+0x7e0` modulo la couche)
  et le curseur à la bande suivante (`+0x40`).

### Décors à parallaxe

`0x33178`, `0x33196` : position (x, y en virgule fixe 16.16, `+8`, `+0xc`) augmentée d'une fraction du
défilement de l'image (`A5+0x506/0x50a` moins `A5+0x524/0x528`) : 1/4 et 1/2, ou 1/2 et 3/4. `0x331e8`, `0x3326c` :
avance de 6 ou 3 pixels par image.

### Suivi vertical des joueurs (0x31616) et mise à jour des caméras (0x33e20)

`0x33e20` (`scroll::UPDATE_CAMERAS`) est un simple enchaînement : A0 = caméra 1, cible précédente copiée
(`+0x26`, `+0x2a`), script de mode `0x2ede0`, A0 = caméra 2, `0x32108`. `0x31616` (`FOLLOW_PLAYERS_Y`, appelé
en `bsr` en `0x30476`, dans un script de mode de la caméra 1) garde l'étendue verticale des joueurs vivants (octet présent +
PV non nuls, `A5 + 0x5de + 0x200 k`) dans la fenêtre `[caméra + 0x30, caméra + 0x70]` : la caméra A0 monte
(trop haut) ou descend (trop bas) de 6 pixels par image au plus. Transcrite avec ses registres (D2, D4-D6 et
drapeaux sont relus par les scripts de mode).

### Non réécrit

Initialisation propre à chaque scène (`0x2ede0` première image, `0x32108` idem), les ~31 scripts de mode de la caméra 1
(table `0x2f780`, suivis par `0x309e0`, `0x30300`, `0x30308`, `0x305fc`, `0x30fc8`, `0x31018`, `0x3147c`,
`0x31616`...), ceux de la caméra 2, `0x2a52`, `0x33062`/`0x33290`/`0x332c0` (parallaxe avec saut calculé par scène).

## Ajouts (F3, U1, U2, F4)

* `text.rs` : `0xd32` (`text_erase`) efface un texte (mêmes descripteurs que `0xd5e`). `video.rs` : `0x19038` recopie
  la table des écrans fixes (120 octets de `0x19a1a`) en `0xff111c`.
* `palette.rs` : l'éclair (`0x1ab56` grise les palettes `0x91c800` et `0x91c3a0` après les avoir mises en
  réserve en `0xff1190`, `0x1ac24` la réserve, `0x1abe0` la remet).
* `popups.rs` : dessin des bulles (`0x19ee0` effacement, `0x19f24` cadre, `0x19fd2` texte, `0x1a20e` bord) en Rust.
* `aides.rs` : `0x1386` (direction 0 à 31 entre deux points), `0x2810` (objet saisi), `0x28d8` (tous les joueurs à
  terre : fin de partie).
* `sprites.rs` : `0x1de7c` vide les têtes de listes. **`0x1de96` n'est pas exécuté en 68000** : l'inventaire le lui attribuait à
  cause de l'`rts` en `0x1df12`, partagé par `0x1ee1a` (listes triées) comme sortie.
* Interface des joueurs : `docs/hud.md`.
Plus rien dans B1 pour les deux caméras. Restent en code d'origine, hors caméras : les routines de parallaxe
`0x33062`... (inutilisées depuis la table de la caméra 2, qui est maintenant en Rust) sauf celles que des décors
appellent encore (`0x33178`, `0x33196`, `0x331e8`, `0x3326c`, natives).
