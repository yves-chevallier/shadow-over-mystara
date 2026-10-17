# ESHL 40 à 63 (`families/eshl_c.rs`)

Tirs et attaques des ennemis, types 40 à 63 de la classe ESHL. Cadre commun et contrat des
gestionnaires : `docs/familles-eshl-set-sobj.md`. Les noms de rôle marqués (h) sont des hypothèses
tirées du comportement. Champs : `+4` état principal, `+5` / `+6` / `+7` sous-états, `+3` variante,
`+0x2c` orientation, `+0x2e` jeu de boîtes, `+0x44` identifiant d'attaque, `+0x45` puissance,
`+0x50` propriétaire, `+0x5c` créateur (pour les types qui en ont un, l'objet est créé par une
fonction de création placée dans la ROM entre deux gestionnaires, non réécrite). Les tables sont lues
dans la ROM (jamais recopiées) : `(tbl,PC,Dn)` en espace programme (`rom16/rom8`), registres et
absolus en espace de données (`data16/data32/data8`).

Aucun de ces types n'est exercé par les 8 scénarios : ils ne sont validés que par `fuzz familles`
(voir la couverture en fin de document).

Fonctions de création **non** réécrites (appelées par d'autres objets) : 0xa3354 (type 41),
0xa31c2 (type 40, appelée aussi par le type 41, via `x.call`), 0xa3b4a / 0xa3b66 (type 45), 0xa44bc
(type 51), 0xa460a-0xa4696 (type 52, sauf 0xa4696 réécrite dans `spawn_followers`), 0xa4d6a (type 53,
appelée par le type 63 : réécrite en ligne), 0xa4e84 (type 54), 0xa5c7c / 0xa5c9a (type 59), 0xa5f38
(type 60), 0xa63b2 (type 61), 0xa66c0 et 0xa6824 (types 62, 63).

Aucun gestionnaire ne cède la main à l'ordonnanceur (pas de `trap`).

## Types

| Type | Adresse | Module | Rôle |
|---|---|---|---|
| 40 | 0xa31e4 | `shot40` | projectile jeté depuis son créateur, animation au hasard, 90 images, palette cyclique |
| 41 | 0xa336c | `shot41` | rayon visant la cible du boss, recharge une palette, crée 3 tirs de type 40 au sol |
| 42 | 0xa3568 | `shot42` | attaque collée à un ennemi qui lance une rafale de 5 éclats (type 43) |
| 43 | 0xa3762 | `shot43` | éclat de l'attaque 42, direction `+0x2d`, détruit au contact ou hors fenêtre |
| 44 | 0xa380a | `beam44` | faisceau interpolé entre deux objets liés (variantes 0 et 1) |
| 45 | 0xa3b88 | `debris45` | débris projetés par un objet (variantes 0 et 1), retenus à l'écran |
| 46 | 0xa3e72 | `scroller46` | objet défilant de 4 pixels par image vers la gauche |
| 47 | 0xa3ebe | `spawn47` | apparition invisible décalée au hasard autour de la caméra, puis animation |
| 48 | 0xa4088 | `lob48` | projectile lancé en cloche par un lanceur (`+0x5c`) |
| 49 | 0xa418e | `strike49` | coup posé devant l'ennemi créateur (`+0x42`) |
| 50 | 0xa4266 | `bullet50` | balle d'un ennemi (2 variantes), phases d'animation par table |
| 51 | 0xa4522 | `drop51` | projectile tombant du haut de l'écran |
| 52 | 0xa46c6 | `finale52` | éléments de l'attaque finale d'un boss (4 variantes), blessent les joueurs |
| 53 | 0xa4d82 | `aimed53` | projectile visant un joueur (direction par 0x1376) |
| 54 | 0xa4eaa | `burst54` | gerbe de projectiles (table de 18 / 20 octets par variante) |
| 55 | 0xa5358 | `fall55` | projectile jeté en diagonale du haut de l'écran |
| 56 | 0xa5466 | `pickup56` | objets à ramasser (12 variantes en 6 types) |
| 57 | 0xa593c | `seeker57` | projectile à tête chercheuse qui explose en 16 éclats (type 58) |
| 58 | 0xa5ad6 | `shard58` | éclat de l'explosion 57 |
| 59 | 0xa5cc2 | `ring59` | anneau d'effet d'un créateur (2 sous-types) |
| 60 | 0xa5f5c | `platform60` | plateforme mobile suivant le défilement vertical (4 variantes) |
| 61 | 0xa63d8 | `marker61` | marqueur collé au créateur (3 tables de décalages) |
| 62 | 0xa66ea | `meteor62` | météore tombant près de la caméra |
| 63 | 0xa6864 | `hand63` | main d'un boss qui le suit et attaque |

## Détail

### 40 (0xa31e4) et 41 (0xa336c)
Le type 41 (rayon) lit la cible du créateur de son créateur (`+0x5c` → `+0x5c`, champs `+0xb0/+0xb2`
de la cible), calcule la direction (0x137e), prend la vitesse de Z dans la table 0xa3492 par tranche
de 32 pixels de distance en X, recopie 32 octets de la ROM 0x25a6e0 vers la RAM palette 0x91c2e0,
tombe (MOVE_XYZ) puis, à Z <= 0, appelle la fonction de création 0xa31c2 (trois tirs de type 40 de
variantes 2, 1, 0 liés à lui). Le type 40 : état 0 copie la position du créateur (décalage X 0 / -10 /
+10 selon la variante), tire une animation dans la table 0xa329e (16 octets) ; état 2 : palette cyclique
(table 0xa31ba, tous les 5 pas), 90 images (`+0x42`), clignotement par parité de variante contre
`A5+0x6e`, mort si le propriétaire est dans l'état 0x406. Tables : 0xa329e, 0xa31ba, 0xa3492.

### 42 (0xa3568) et 43 (0xa3762)
42 : pause (0x29ea), colle à l'hôte `+0x42` (32 pixels devant, 64 en hauteur), sous-états `+5`
(0 attente / 2 combat / 4 retombée) et `+6` (0 départ, 2 attente du déclencheur `+0x59`, 4 rafale de 5
éclats toutes les 8 images, 6 pause de 20 images, 8 récupération). Angle `+0x5e` et pas `+0x5f`
(modulo 32). Crée des tirs de type 43 (0x2b). Tables : 0xa3600 (animation par `+0x33` de l'hôte).
43 : table de vitesses 0x40, animation `0x10 + table[0xa37ba + dir/2]`, mort si `+0x57 & 0x28` ou hors de
la fenêtre `[-0x50, 0x1d0]` de 0x104a2.

### 44 (0xa380a) et 45 (0xa3b88)
44 : faisceau entre le créateur A1 (`+0x46`) et le créateur de celui-ci A2 ; position = A2 + t(A1-A2)/16
pendant 16 images (`+0x14` = t, Z mesuré 0x50 plus haut) ; compte ses faisceaux dans `+0x42` du créateur.
Variante 1 : posée sur A2, animation selon `+0x2f` de A1. Le code à 0xa39e6 du listing est inatteignable.
45 : variante 0, vitesse tirée dans la table 0xa3e30 (8 lignes de 4 mots), rebond (table 0xa3c94),
retenue à l'écran (0xfbb0) ; variante 1, éclat posé sur la source (décalages 0xa3dc2 / 0xa3dc4 lus en
chevauchement), qui tombe quand `+0xa4` de la source n'est plus 4.

### 46 à 51
46 : animation = variante, X -= 4 par image, libéré à `A5+0x506 - 0x50`. 47 : table de décalages
0xa3fd4 / 0xa3ff4 (X) et 0xa4014 / 0xa4034 (Y), délais 0xa4068. 48 : monte/descend le long de VZ,
compte ses retombées dans `+0x1d` du lanceur, libère l'emplacement `+8+variante` du lanceur ; appelle
0x54872 (création d'un tir joueur). 49 : posé à 0x6e pixels devant l'hôte, anime jusqu'à ce que l'hôte
passe à `+7 = 6`. 50 : phases d'animation `+0x5a` / durée `+0x5b` (table 0xa44aa), retenue (0xfbae) et
0x33fe4 à chaque image. 51 : retombée depuis `A5+0x50a + 0x100`, appelle 0xab7d8 et 0x2860 au sol.

### 52 (0xa46c6)
Quatre variantes liées au boss `+0x46` : 0 (état boss 0x201 : boule + trois éléments), 1 (0x214 :
rayon), 2 (0x215 : voile plein écran, verrou d'écran `A5+0x43`), 3 (voile de 256 images). Collisions :
`overlap` (tables 0xa4cd2 par `+0x3e`, pointeurs 0xa4cba par personnage et ligne `+0x38`),
`fall_check` (zones sûres 0xa4bca pour l'étage 0x2e, rectangles 0xa4bd4 / 0xa4bdc et octet 0xff2064
pour l'étage 0x69), `hurt` (état du joueur 0x02042000, PV - PV max / 2 pour l'attaque 0x2c).

### 53 à 55
53 : direction vers la cible du propriétaire du créateur (0x1376), tables 0xa4e16 (animation,
orientation). 54 : 17 gerbes numérotées, lignes de 18 octets (0xa4fe6) ou 20 octets (0xa51b4, avec
animation). 55 : jeté depuis `+0x58/+0x5a` (VX ±0x800), 48 images au sol dont 32 de clignotement.

### 56 à 58
56 : 12 variantes (6 types × palette 0x14 / 0x13), ramassage au contact (±8 en X, ±6 en profondeur,
au sol), 4 familles de comportement. Table de délais 0xa55b2. 57 : suivi du `+0x50` du créateur,
direction rafraîchie toutes les 20 images, 360 images maximum, 16 éclats. 58 : lignes de 8 octets
(0xa5bd6 : durée, sous-type, indice de vitesse, animation, orientation, table de vitesses).

### 59 à 63
59 : anneau (sous-type 0, suit le propriétaire) ou satellite (sous-type 1, table 0xa5db2). 60 :
cycle d'attente / descente / repos / remontée, couche de dessin `+0x3b` selon Z, détection de
joueur par 0x7f19c, variantes 1 et 3 ; la variante 3 pose `A5+0x51e`. 61 : décalages par
variante du créateur (tables 0xa6538 / 0xa65de / 0xa6670, 20 octets par variante). 62 : position
tirée autour de la caméra, retard par la table 0xa679e, effets d'impact 0xb9c88, 0xab7d8, 0x2860,
0xeafdc. 63 : main du boss (`+0xac` du boss : phase), suit le boss de 2 pixels par image, tirs au
sort par les tables 0xa6b0e (délais) et 0xa6c54 (masques indexés par `FFFF92`), crée un tir de type
53 en fin d'attaque.

## Couverture

Scénarios : aucun des types 40 à 63 n'est appelé (seuls ESHL 1, 2, 11, 13, 14, 23 le sont).

Fuzz (`fuzz familles`, `DDSOM_LIEN=1 DDSOM_PAS=200 DDSOM_VARIANTES=n DDSOM_ENTREE=<adresse sans 0x>`,
150 trajectoires environ par type) : 0 écart sur les 24 types, de 200 à 12 000 appels chacun selon le
type. Les variantes hors table (par exemple `+3` = 2 pour les types 44, 45 qui n'en ont que deux) font
sortir l'original des rails : elles ne sont pas des écarts à corriger et ont été exclues du compte.
Le fuzz a reçu trois ajouts **additifs** (activés par `DDSOM_LIEN`, `DDSOM_PAS`) : le créateur `+0x5c`
et des compteurs `+0x42/+0x4c` de l'objet, des chaînes de liens `+0x46/+0x50/+0x5c` non nulles chez les
propriétaires possibles, des états d'attaque de boss (`+0xa0` = 0x201 / 0x214 / 0x215 / 0x204, `+0xac`,
`+0xa4`, `+0x1c`, `+0xae`), l'étage `A5+0x34`, l'octet `0xff2064` et les drapeaux `+0x58/59/5a` de l'objet.

Instructions de l'original **jamais exécutées** par le fuzz (code non validé), hors fonctions de création
appelées d'ailleurs :
* 40 : état 2 fin de vie de la boule (0xa32e8) ; vu seulement avec 200 appels.
* 41 : bandes de distance élevées du choix de la vitesse de Z (0xa3414-0xa3478).
* 42 : ses états de combat sont vus ; reste le tir d'un éclat quand l'allocation réussit sur 0xa3706-0xa3760
  (partiel) et les chaînes `+6` 2 → 4 (0xa3626-0xa36ce).
* 44 : variante 1 profonde (0xa39c6-0xa3a2a), code mort de 0xa39e6 (inatteignable).
* 45 : miroir de `+0x2f` (0xa3cd6-0xa3cdc), 0xa3d40.
* 47 : certaines combinaisons de position de la caméra (0xa3f6e-0xa3fd2, 0xa4036).
* 49 : sortie de l'animation avec l'hôte étourdi (0xa422e-0xa4260).
* 50 : rien d'autre que la fonction de création 0xa44bc ; 51 : la fonction 0xa460a-0xa46c0.
* 52 : variante 0 sous-types 1 à 3 (0xa47d6-0xa47fc), la fin de la variante 2 (0xa498c-0xa49e4), la
  mort des joueurs (0xa4b32-0xa4bbc, étage 0x2e/0x69 avec joueurs vivants), 0xa4c48 (inversions X),
  0xa4d32-0xa4d40 (PV) ; le reste de 0xa4d50-0xa4d80 est une création.
* 53 : création 0xa4e84. 55 : atterrissage (0xa5412-0xa5460) hors de portée du fuzz.
* 56 : quelques tests de bord (0xa5494, 0xa54a2, 0xa5674...). 57, 58, 59, 61, 62 : quelques lignes
  (fin de vie du type 57, création 0xa5c7c, 0xa5e5e-0xa5e8a de l'anneau 59, 0xa66c0, 0xa6824).
* 60 : retour et descente selon les sous-types 1 et 3 en partie, 0xa63a2-0xa63d6 (création).
* 63 : tir au sort d'attaque (0xa68e0-0xa692a) et fin d'attaque 0xa69d8-0xa6a0c.

Registres : chaque gestionnaire ne garantit que A0 et D7 (contrat de la boucle de dispatch). D0-D2 et
A1-A4 en sortie ne sont pas reconstitués comme dans l'original.
