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
