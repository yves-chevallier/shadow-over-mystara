# ENEMY (C02) : les ennemis (`families/enemy*.rs`)

Pool d'objets de 22 emplacements de 0xC0 octets (A5+0x299e, compteur de places libres A5+0x7e62), table de
gestionnaires 0x39a78 (29 types, le 17 est nul), boucle de dispatch 0x39a52. Cadre commun (`Cx`, `handlers!`,
contrat A0/D7) : `docs/familles-eshl-set-sobj.md` ; vue d'ensemble du code : `docs/atlas.md` §7 étape 4.3.
Les noms de rôle (h) sont des hypothèses tirées du comportement et des noms de debug du jeu (atlas, annexe A).

## 1. Fichiers

| Fichier | Contenu |
|---|---|
| `enemy_common.rs` | champs `f::*`, états `st::*`, adresses `rom::*`, allocation de sous-objets (`alloc`), libération (`free`), tirage (`random`), pause (`paused`, `suspended`), boucles communes `active_loop` / `hit_loop`, réaction aux coups par table (`react`), aides `add8`/`add16`/`set_dw` |
| `enemy.rs` | type 20 (ManScorpion), type 28 (Nagpa) |
| `enemy_double.rs` | « doubles » qui recopient un maître : types 22 (Chimera), 23 (Spectre), 24 (Manticore), 26 (DarkWarrior 2) |
| `enemy_part.rs` | partie attachée à un hôte : type 7 (Wyvern, tentacule) |
| `enemy_riyakaa.rs` | type 16 (Riyakaa), ennemi à IA complète |

## 2. Charpente commune

Chaque gestionnaire commence par `jsr 0x29ea` (hook de pause : Z levé = objet non suspendu ; sinon retour
immédiat), puis, pour les ennemis à IA, `jsr 0xf1bfc` (compteurs de fin de gestionnaire : gel `+0x53`
négatif remis à zéro, compte à rebours `+0x89`). L'état principal `+4` indexe une table de sauts :

| `+4` | Rôle |
|---|---|
| 0 | création : paramètres d'IA, PV initiaux (`0x4544`), immunités (`0xfe5a`), premier jeu d'animations |
| 2 | actif : sous-état `+5`, sous-sous-états `+6`, `+7` (octets pairs, indices de tables de sauts) |
| 4 | touché (posé par `combat.md` §4 : `+4 = 4`, `+0x49` = code de réaction, `+0x4a` = orientation) |
| 6 | libération (`0x21f4`) |

Les états se posent aussi par des longs (`+4..+7`) : `0x02020000` = actif, sous-état 0 (« marche »),
`0x04020600` = touché, sous-état 2... Un octet `+0x53` positif est un gel (coup encaissé) qui bloque tout
sauf la décrémentation. `active_loop` / `hit_loop` factorisent la fin commune des états 2 et 4 : gel, sous-état,
test de zone `0xfbae` et ombre `0x33fe4` (dès que le sous-état n'est pas 0), puis remise en liste triée
(`reinsert`, 0x1ee1a). Le sous-état 8 de la variante 8 du type 16 abandonne l'image sans passer par cette fin
(l'original dépile son adresse de retour) : `active_loop` accepte pour cela un sous-état qui rend vrai.

La réaction aux coups (état 4, sous-état 0 ou 6) est partagée par tous les ennemis : une table de longs indexée par
`+0x49` (0x6fe22 et 0x70018 pour le type 16, mêmes routines `0x3bb9c`..`0x3dab0`) mène à des routines
communes qui jouent le recul, la chute ou la mort. Ces routines ne sont pas encore réécrites : `react` les
appelle (68000). Elles s'exécutent sur l'ennemi courant (`A0`) et modifient ses sous-états.

Tirage pseudo-aléatoire : `random` (0x19a88). Cibles : `0x424e` (choix aléatoire d'un joueur, Z = aucun),
`0xb5e0` (joueur dans une boîte devant l'ennemi), `0xa4b4`/`0xa4c2` (décision par masque), voir `combat.md` §6.

## 3. Types réécrits

### Type 20 (0x74fc0) « ManScorpion » (h) — `enemy.rs`

Ennemi composite greffé sur un parent de type 19 (`+0x50`). La création engendre : deux halos (ESHL type 11,
`+0xa4/+0xa6`), une ombre (EFFCT 0x20, `+0xb4`), deux flammes (ITEM type 6, `+0xa8/+0xaa`) et, variante 1, deux bras
(ENEMY type 19, `+0xac/+0xae`, masque `+0xb0`). Variantes 0 et 1 volent vers le bord gauche en jetant des
étincelles (`0xb10c8`) ; la variante 2 vole vers le bord droit et lâche ses bras. L'état 4 (touché) gèle les parties,
recule de 15 images puis déroule une mort en plusieurs temps (`+6` = 0, 2, 4, 6, 8, 10) ; l'état 6 libère les parties
et rend ses compteurs au parent et à l'hôte (`+0x46`). Conditions de création : au moins deux places d'ESHL et d'ITEM
libres (A5+0x7e64, 0x7e6e) et, soit moins de deux places d'ENEMY libres (0x7e62), soit une place d'EFFCT libre (0x7e68). Animations 0x169fb0.
Couverture : scénarios (7 914 appels), fuzz (code d'origine entièrement vu hors deux octets de table).

### Type 28 (0x79300) « Nagpa » (h) — `enemy.rs`

Personnage caché qui apparaît quand un joueur vivant passe à moins de 0x30 (variante 0) ou 0x50 (variante 1) pixels
(test `0x7f19c`), joue son animation d'apparition, puis redevient caché quand plus personne n'est à portée. L'octet
`0xff2064` (`A5-0x5f9c`) note qu'il est visible (bit 1 variante 0, bit 0 variante 1). Sous-états : 0 attente,
2 apparition, 4 visible, 6 disparition. Animations 0x16c7fc. Couverture : fuzz seulement (code vu en entier).

### Type 7 (0x63b2a) « Wyvern » (h) — `enemy_part.rs`

Tentacule d'un Displacer Beast (hôte `+0x46`, type 0x13 ; un hôte de type 0xe ne choisit que les jeux d'animations
0x17dbb0.., pas ceux du Displacer Beast 0x1824d4..). Il se colle à l'hôte avec un décalage par variante (table
0x63f34 : X inversé quand l'hôte regarde à gauche) et lit l'état `+0xa2` de l'hôte, converti en sous-état par la
table d'octets 0x63fa8 (espace de données). Sous-états : 0 attente (variante 0 : animation de repos à durée
aléatoire 0x63c7e), 2 début d'attaque, 4 coup (compte à rebours `+0xa4` de 360 images), 6 retour, 8 coup spécial,
10 recul. La variante 1 n'est qu'un accompagnement (animation 3). Il prévient l'hôte de la fin d'un geste par `+0xa3`
de l'hôte. L'état 4 resynchronise les PV (0x7fff). Un test sur son propre type (`cmpi.b #0xe,(2,A0)`, 7 ici) rend
la branche `0xe50fe` morte : conservée telle quelle. Couverture : fuzz.

### Doubles graphiques (types 22, 23, 24, 26) — `enemy_double.rs`

Ces ennemis n'ont pas de logique de combat : ils recopient leur maître (octet actif, drapeaux, position,
orientation, propriétaire) et choisissent leur animation d'après les sous-états du maître. Le choix est une table de
décision (`choose_*`) rendue en `Play` : rien, animation par `0x10428` (jeu choisi d'après `+0x33` du maître, puis
`+0x44 = +0x3e`), animation du coup par `0x10450`, ou, pour la Chimère, relance d'animation sur demande (`+0xb6`)
et tir ESHL 0x31.

| Type | Adresse | Maître | Particularités |
|---|---|---|---|
| 24 | 0x76afc | type 25, `+0xb2` | un pixel plus bas ; jeux d'animations 0x16c2da..0x16c75a |
| 23 | 0x76592 | idem | deux jeux (normal / blessé, `+0xaf`) ; suivi de vie `+0x62/+0x64`, délai `+0xb4`, EFFCT 0x3f à la mort |
| 22 | 0x75f42 | idem | comme 23, un pixel plus bas ; gestes relancés `+0xb6`, souffle ESHL 0x31 |
| 26 | 0x77736 | type 25, `+0xb0` | rejoue les déplacements du maître (`0x1d90`, `0x1da6`, `0x1d78`) d'après `+0xb8..+0xbb` ; EFFCT 0x32 à la mort |

Les créateurs de ces doubles (type 25, `0x76ec4`) ne sont pas réécrits.

### Type 16 (0x6f1c0) « Riyakaa » (h) — `enemy_riyakaa.rs`

Ennemi à IA complète (humanoïde à griffes). Animations 0x1665d6 (mouvements) et 0x1665f8 (coups). Sous-états de
l'état actif : 0 arrivée selon la variante `+3` (0/4 course, 2 chute, 6 bond, 8 approche, 10 apparition retardée),
2 choix d'une attaque (le joueur le plus proche, saisie par un joueur → état 4/8), 4 coups de patte (haut, milieu,
bas, bond, course, attente de cible, demi-tour), 6 coups spéciaux (cri, tour autour de la cible, saut avec
coup), 8 se jette sur un joueur, 10 repos. L'état touché : sous-état 0/6 réaction commune par table, 2 recul puis
contre-attaque (masques 0x6ff70 par difficulté `A5+0x7f92`), 4 relevé, 8 saisie par un joueur (fuite). Les
décisions lisent les paramètres d'IA `+0x68..+0x81` posés à la création (masques `+0x72/+0x73`).
Couverture : scénarios (jusqu'à 3 088 appels), fuzz ; non atteintes par le fuzz (examinées à la main) : l'attaque
choisie dans `70178` quand un joueur est tout près, le cri quand `0x5cdc6` rend Z, le retour de saisie.

## 4. Non réécrit (types encore en 68000)

0 Kobold, 1 Skeleton, 2 Troglodyte, 3 Bow Gnoll, 4 Owlbear, 5 Ghoul, 6 Hellhound, 8 Gargoyle, 9 GiantBeetle,
10 GiantScorpion, 11 mud golem, 12 Shadow Elf, 13 Axe Gnoll, 14 Troll, 15 Goblin, 18 Ogre, 19 DisplacerBeast,
21 Harpy, 25 DarkWarrior 1, 27 WarMachine. Parmi eux, seuls 4 (4 474 appels), 19 (76 491) et 16 (3 088, réécrit)
apparaissent dans les scénarios.

## 5. Validation

* `tools/validate_all.sh 30000` : les 8 scénarios + 4 « mash » doivent être VALIDE ; type 20 : 7 914 appels,
  type 16 : appels dans plusieurs scénarios.
* `fuzz familles` (groupe `familles` d'`examples/fuzz.rs`, ENEMY ajouté) : crée un ennemi neuf du type (variante,
  états `+4/+5/+6` pris dans les tables que l'original accepte, maître/hôte/cible fabriqués), puis 60 appels
  comparés à l'original ; les compteurs de places libres sont recalculés. `DDSOM_ETATS=1` active les états tirés,
  `DDSOM_VARIANTES=n` les variantes, `DDSOM_COUVERTURE=fichier` écrit les instructions de l'original exécutées.
* Pièges rencontrés : la table 0x63fa8 se lit dans l'espace de données alors que 0x63f34 (adressage `(tbl,PC,Dn)`)
  se lit dans l'espace des opcodes ; `+0x24/+0x26` sont les liens des listes triées (ne pas les corrompre dans le
  fuzz) ; les routines de réaction communes exigent des sous-états de départ valides ; un gestionnaire qui crée
  des objets (`alloc`) doit respecter l'ordre des allocations (l'emplacement obtenu dépend de l'ordre).
