# ENEMY (C02) : les ennemis (`families/enemy*.rs`)

Pool d'objets de 22 emplacements de 0xC0 octets (A5+0x299e, compteur de places libres A5+0x7e62), table de
gestionnaires 0x39a78 (29 types, le 17 est nul), boucle de dispatch 0x39a52. Cadre commun (`Cx`, `handlers!`,
contrat A0/D7) : `docs/familles-eshl-set-sobj.md` ; vue d'ensemble du code : `docs/atlas.md` §7 étape 4.3.
Les noms de rôle (h) sont des hypothèses tirées du comportement et des noms de debug du jeu (atlas, annexe A).

## 1. Fichiers

| Fichier | Contenu |
|---|---|
| `enemy_common.rs` | champs `f::*`, états `st::*`, adresses `rom::*`, allocation de sous-objets (`alloc`), libération (`free`), tirage (`random`), pause (`paused`, `suspended`), boucles communes `active_loop` / `hit_loop` / `hit_loop_at`, réaction aux coups par table (`react`), aides d'IA (`target_in_box`, `target_in_box_far`, `pick_target`, `grabbed`, `target`, `needs_turn`), aides `add8`/`add16`/`set_dw` |
| `enemy.rs` | type 20 (ManScorpion), type 28 (Nagpa) |
| `enemy_double.rs` | « doubles » qui recopient un maître : types 22 (Chimera), 23 (Spectre), 24 (Manticore), 26 (DarkWarrior 2) |
| `enemy_part.rs` | partie attachée à un hôte : type 7 (Wyvern, tentacule) |
| `enemy_riyakaa.rs` | type 16 (Riyakaa), ennemi à IA complète |
| `enemy_dbeast/` | type 19 (Displacer Beast et ses pièces), un sous-module par groupe de comportements : `parts`, `stance`, `attacks`, `attacks_b`, `moves`, `hit` |
| `enemy_owlbear/` | type 4 (Owlbear) : `stance`, `attacks`, `moves`, `hit` |
| `enemy_harpy.rs` | type 21 (créature volante « Harpy ») |

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

### Type 19 (0x71e44) « Displacer Beast » (h) — `enemy_dbeast/`

Un seul gestionnaire pour quatorze variantes (`+3`) : la bête (0 à 3, liées à leur parent `+0x42`) et des pièces
lâchées (4 à 13 : projectiles, bonds, ombre). `+0x50` est la cible (un joueur), `+0x2f` le profil d'IA (0 ou 1 :
la table de sauts de la création n'a que deux entrées), `+0xbf` un décalage de série d'animations tiré à la création
(tables 0x74f56/0x74f7a/0x74f8a), `+0xbc` le rang de la pièce (−1 : pas une pièce). Animations 0x166fb6
(mouvements) et 0x167054 (coups) ; `+0xa5` est l'animation voulue, `+0xa2` celle en cours (on ne relance pas la même,
`Replay` dans `attacks.rs` décompte les répétitions `+0xae`). Groupes de l'état actif (`+5`) :

| `+5` | Fichier | Rôle |
|---|---|---|
| 0 | `parts` | pièces : membres suivant le parent, projectiles, bonds en X ou en profondeur, glissades (`+6` = 0 à 22) |
| 2 | `stance` | la bête debout : poursuite (0, 8), demi-tour (2), attente (4), approche (6) ; `engage` teste la portée (coup court, coup long, saisie) |
| 4 | `attacks` | dix-sept coups (`+6` = 0 à 36) ; étapes communes `+7` = 0 (préparation) et 2 (regard) puis étapes propres |
| 6 | `attacks_b` | seconde série : petits bonds, bond de côté, rugissement |
| 8, 10, 12 | `moves` | charges et sauts vers un joueur, entrée en scène (chute), sortie par le haut |

L'état touché (`hit`) : 0 et 6 réaction commune par table (`0x740c0`, `0x742fa`), 2 recul d'une pièce, 4 chute, 8 fuite
ou mort. Piège : plusieurs tables de sauts ont du code **dans** la table (une entrée désigne le code qui la suit :
`0x72efe`, `0x73cb8`) et `0x72bbc` (code après le `jsr` de la table de `stance`, absent du listing) ; les tables
d'octets lues par `lea ...(PC)` se lisent en espace de données, celles lues par `(adresse,PC,Dn)` en espace
des opcodes. Couverture : scénarios (jusqu'à 129 000 appels), fuzz (2 126 instructions, 43 non atteintes dont une
dizaine de restes de tables).

### Type 4 (0x5d87a) « Owlbear » (h) — `enemy_owlbear/`

Même charpente que le Riyakaa. Animations 0x160e26 (mouvements) et 0x160e44 (coups). Variantes 0 (arrivée debout),
1 (arrivée en marchant, `+0xa0` = durée), 2 (bond d'entrée). Sous-états actifs : 0 arrivée (compte à rebours `+0xaa`,
tirage de la « chance d'attaquer » `0x5f2c6`), 2 debout (poursuite, demi-tour, attente, approche ; `engage` →
`reach` tire un coup court ou une attaque dans les tables par difficulté `0x5eec6` / `0x5f0c6`), 4 coups (dix, `+6` =
0 à 18), 6 bonds de l'après-arrivée, 8 charges (`+6` = 0 à 6), 10 repos. L'état touché a un octet `+0xa5` qui
supprime le test de zone (`hit_loop_at`). Couverture : scénarios (4 474 appels), fuzz.

### Type 21 (0x756c4) « Harpy » (h) — `enemy_harpy.rs`

Créature volante à neuf variantes (ligne de huit octets par variante en 0x756e6 : orientation, durées, décalage) qui
crée à sa naissance un compagnon (ESHL 0x10, `0xa036e`) dont `+0xaa` garde le lien. `+0x2f` (0, 2 ou 4 : indexe des
tables de mots) choisit la série d'attaques, `+0x43` la forme. Sous-états : 0 vol (selon la variante), 2 attaque
(projectile `0xa0566`, coups, tir), 4 pause, 6 sortie d'écran, 8 sortie du premier plan. Le dispatch de sortie lit
D0 laissé par la table de vitesses : l'angle `+0x2d` doit rester sous 0x40. Couverture : fuzz seulement.

## 4. Non réécrit (types encore en 68000)

0 Kobold, 1 Skeleton, 2 Troglodyte, 3 Bow Gnoll, 5 Ghoul, 6 Hellhound, 8 Gargoyle, 9 GiantBeetle,
10 GiantScorpion, 11 mud golem, 12 Shadow Elf, 13 Axe Gnoll, 14 Troll, 15 Goblin, 18 Ogre, 25 DarkWarrior 1,
27 WarMachine. Aucun n'apparaît dans les scénarios (seuls 4, 19 et 16, réécrits, y figurent).

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

### Fuzz des types du deuxième lot (4, 19, 21)

`fuzz_enemy.rs` a pour ces types : `etats_enemy` (états `+4/+5/+6` valides), `etape_dbeast` / `etape_owlbear`
(étape `+7` valide selon l'état), `prep_dbeast` / `prep_owlbear` / `prep_harpy` (compteurs courts, animation en cours
souvent égale à l'animation voulue, état à reprendre après un demi-tour, code de réaction `+0x49` < 43 entrées,
parent dans un état connu, objet au ras du sol pour les réceptions de bonds), joueurs placés dans les boîtes de portée,
redessin d'un état valide à un pas sur trois. Variables : `DDSOM_GRAINE=n` (graine), `DDSOM_PAS_ETAT=1` (trace des états
avant/après), `DDSOM_COUVERTURE=fichier`. Pièges des fuzz : `+0x2f` doit valoir 0/1 (type 19) ou 0/2/4 (type 21), le
propriétaire ne doit pas être l'objet lui-même, `+0xa2` du type 21 et `+0xa4` des types 4 et 19 ne se corrompent pas
(l'original saute alors dans des données).
