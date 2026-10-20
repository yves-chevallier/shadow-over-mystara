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
| `enemy_warrior.rs` | type 25 (bras du DarkWarrior 1) |
| `enemy_skeleton/` | type 1 (Skeleton) : `stance`, `attacks`, `leaps`, `hit` |
| `enemy_elf/` | type 12 (Shadow Elf) : `stance`, `attacks`, `hit` |
| `enemy_trog/` | type 2 (Troglodyte) : `stance`, `attacks`, `hit` |
| `enemy_ghoul/` | type 5 (Ghoul) : `stance`, `attacks`, `moves`, `hit` |
| `enemy_lib.rs` | bibliothèque commune 0x103d6-0x105b6 (animation + type de coup `+0x44`, tests de côté, portes de décision `ai_gate`), utilisée par les types 8 et 18 |
| `enemy_beetle.rs` | type 9 (GiantBeetle) |
| `enemy_scorpion.rs` | type 10 (GiantScorpion) |
| `enemy_gargoyle/` | type 8 (Gargoyle) : `arrival`, `stance`, `attacks`, `hit` |
| `enemy_hellhound/` | type 6 (Hellhound) : `arrival`, `stance`, `attacks`, `hit` |
| `enemy_ogre/` | type 18 (Ogre) : `arrival`, `stance`, `attacks`, `hit` |
| `enemy_kobold/` | type 0 (Kobold) : `arrival`, `stance`, `attacks`, `moves`, `hit` |
| `enemy_goblin/` | type 15 (Goblin) : `arrival`, `stance`, `attacks`, `hit` |
| `enemy_troll/` | type 14 (Troll) : même charpente que le Goblin, avec un lancer d'objet et des sauts |
| `enemy_gnoll/` | type 13 (Axe Gnoll) : `arrival`, `stance`, `attacks`, `hit` ; mêmes familles de gestes, hache et sauts sur la cible |
| `enemy_bow/` | type 3 (Bow Gnoll, archer) : `arrival`, `stance`, `shoot`, `spawned`, `hit` |
| `enemy_golem/` | type 11 (Mud Golem) : `control`, `attacks`, `moves`, `hit` |
| `enemy_warmachine/` | type 27 (WarMachine) : `arrival`, `stance`, `attacks`, `decide`, `hit` |

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

### Type 1 (0x57cc4) « Skeleton » (h) — `enemy_skeleton/`

Squelette à épée ; la série `+0x2f` (0 ou 1) donne un squelette armé qui garde (`+0x68 = 9` : un coup encaissé
déclenche une riposte `+0xa9`). Animations 0x15eed8 (mouvements) et 0x15eef4 (coups). Variantes `+3` = 0 à 5
(arrivée debout, en posture, en chute, clignotante), table par `+0x2f` en 0x57de6 (octets `+0x6d/+0x72/+0x73/+0x68`).
Sous-états actifs : 0 arrivée, 2 debout (idée → marche → choix d'une attaque dans les tables par difficulté
0x5932a / 0x5952a, annonce 0xac3a), 4 dix coups (estoc, enchaînement, double coup, bonds `+0x14`, glissade, garde
de la série 1), 6 bonds et gardes, 8 esquives, 10 chute d'arrivée. Chaque sous-état exécute le test de zone (0xfbae,
**qui n'est pas vide** : il remet D5 à 0, lu par `tst.b D5b` en 0x5808c) puis l'ombre 0x33fe4. La réaction aux
drapeaux de coup `+0x8f` (0x59146) est un préambule des sous-états 2, 4, 6, 8 (`A6` = adresse de reprise).
Couverture : scénarios `skeleton_scene13/58/64`, fuzz (`fuzz_enemy/lot3.rs`).

### Type 12 (0x69384) « Shadow Elf » (h) — `enemy_elf/`

Elfe sombre : il frappe à l'épée (`+0x44/+0x45` = type 0x1b à 0x1d, force 4 ou 5), disparaît (`+1` bit 1) et retombe
sur sa cible. Animations 0x164174 (mouvements) et 0x164184 (coups). La création enchaîne sur l'état actif. L'état actif
appelle toujours, hors gel, 0x81aa4 (test de proximité d'un objet de scène), 0xfbae, l'ombre et la remise en liste.
**La variante `+3` est un indice de table déjà doublé** (0, 2, 4, 6). Groupes : 0 arrivée (marche, chute, bond, chute
lente), 2 debout, 4 coups (`+6` = 0 approche, 2 demi-tour, 4 et 6 coup, 8 et 10 coups répétés, 12 et 16 à 20 chute
sur la cible, 14 élan, 22 et 24 reprise de la marche), 6 coups d'après-bond, 8 coups de repli (tables 0x6a480,
0x6a73c, 0x6a93c), 10 repos. Étapes mortes (jamais atteintes, absentes du fuzz) : 12 et 14 des coups 18 et 20.
Couverture : scénarios `elf_scene92/94/96`, fuzz.

### Type 2 (0x59baa) « Troglodyte » (h) — `enemy_trog/`

Même charpente. La série 0 clignote après un coup encaissé (`flicker` : `+0xa5` phase, `+0xa6/+0xa7/+0xa8/+0xaa/+0xae`
compteurs, bit 1 de `+1`) ; la série 1 (armée) ne clignote pas. `+0xa0/+0xa1` sont des octets d'aspect. Animations
0x15faa0 et 0x15fabe. Piège : en état actif le gel `+0x53` positif **n'empêche pas** le comportement, c'est la pause
qui l'empêche (inversion par rapport aux autres types) ; la pause laisse malgré tout l'ombre et la remise en liste.
Un bond des variantes 3 et 4 teste les drapeaux de 0x1dd4 (`bpl`) : `Cx::c.cond(10)`. Couverture : scénarios
`trog_scene101/101b/104`, fuzz.

### Type 5 (0x5f792) « Ghoul » (h) — `enemy_ghoul/`

Charpente de `active_loop`. Animations 0x16191c (mouvements) et 0x161930 (coups) ; `+0xa0/+0xa1` aspect courant et
précédent. Six variantes d'arrivée (marche, saut, chute, réveil quand un joueur approche, apparition, clignotement).
Les sous-états 2, 4, 6 et 8 commencent par le préambule des drapeaux de coup `+0x8f` (0x60c88) : ici les drapeaux 1
et 2 exécutent **immédiatement** la routine de l'état touché (0x6089a, qui se termine par sa propre remise en liste,
puis la boucle de l'état actif refait zone, ombre et remise en liste) et le drapeau 3 saute dans le sous-état 6 avec
l'état 4/8. Groupes : 2 debout (marche, demi-tour, attente), 4 coups de griffe et bonds (`+6` = 0 à 16), 6 pas de
côté, bonds, deux coups, sortie d'écran, 8 demi-tours enchaînés sur une attaque, 10 patrouille. Tables par difficulté
0x60d38, 0x60f38 (+0xa) et 0x61164. Couverture : scénarios `ghoul_scene26/95` (la 19 et la 97 divergent à cause de
`combat_update` et `players_update`, voir `scenarios/ko/README.md`), fuzz.

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

### Type 9 (0x65ae8) « GiantBeetle » (h) — `enemy_beetle.rs`

Insecte qui défile sur le bord de l'écran. Variantes `+3` : 0, 1 et 3 marchent (angle 8 ou 0x18 selon la parité de
`+0x2f`), 2 entre par un bord, 4 est un nid qui engendre des pièces (ESHL 0x38, listes 0x65c78 par `+0x42`) puis dort
(sous-état 8) jusqu'à être visible. Le groupe `+0x2f` (0 à 7) choisit la trajectoire du sous-état 4 : 0-1 traversée avec
virage, 2-3 dérive latérale (`+0xb4`), 4-5 vol arrêté devant un joueur (abscisses d'arrêt `+0xb6`), 6-7 comme 4-5 avec la
morsure ; avec `+6` non nul le sous-état 4 se retourne vers un joueur (0x1376) et remonte. La morsure (0x661ce) est
déclenchée par un tirage (masques 0x6623a par difficulté) quand le joueur ciblé est dans l'état 0x200, étape 0xc. Le
sous-état 6 est le bond de la morsure, l'état touché une secousse (code `0x5c`), un recul, une chute puis un clignotement.
La recherche du joueur le plus proche dans une boîte (0x67452) est devenue `enemy_common::find_player`, la création d'une
liste d'objets de la ROM `spawn_list`. Piège : `movea.w (0x50,A0),A1` doit laisser A1 (`load_target`) : 0x1376 le lit.
Couverture : scénarios `beetle_scene16/18/43/61/112`, fuzz (code d'origine vu hors deux restes).

### Type 10 (0x6681c) « GiantScorpion » (h) — `enemy_scorpion.rs`

Variantes : 0 debout, 1 et 4 tombent d'en haut, 2 et 3 entrent en volant par un bord, 5 est le nid (liste `+0x2f`, table
0x669d6). Le `+6` d'arrivée (2, 4, 6, 8) choisit l'entrée du sous-état 0. États actifs : 2 debout (`+6` = 0 libre, 2 enfant
dont le parent `+0x5c` est surveillé, 4 enfant libre ; marche d'aller-retour animation 1/4 ; tirage d'une attaque dans la
table 0x674fa), 4 et 10 coups tirés (tables par difficulté 0x66e48 / 0x67146, geste `+0xa6` par 0x6773c, durées
0x677f8 en espace des opcodes), 6 attente, 8 charge avec retournement (animation 0x163db8, `+0x44 = 0x1f`). L'état touché
a son propre recul (pas de table de réactions) et une fuite hors de l'écran quand un joueur le saisit. Les données au
milieu du code (tables en espace de données lues par `lea (PC)` puis `(An,Dn)`, en espace des opcodes pour `(table,PC,Dn)`)
sont lues à l'exécution dans la ROM. Couverture : scénarios `scorpion_scene36-39/53`, fuzz (tout le code vu).

### Type 8 (0x63fb0) « Gargoyle » (h) — `enemy_gargoyle/`

Créature volante (variantes 0 et 1 ; la 2 saute dans des données dans l'original et n'apparaît pas). Utilise la
bibliothèque `enemy_lib` (0x103d6 : démarre une animation et copie `+0x3e` en `+0x44`, 0x1040c : pas d'animation avec
la même copie, 0x10474 : joueur en vie, 0x104a2 : zone, 0x1050c/0x10560 : sens, 0x1047a : décision d'attaque). Sous-états
actifs : 0 arrivée (vol d'entrée `+6` = 0, posé `+6` = 2 qui crée une ombre EFFCT 0x32), 2 déplacements (0 poursuite,
2 demi-tour, 4 et 6 vol de poursuite avec choix d'un geste dans les tables 0x65792.. / 0x65822.. / 0x658a2.., 8 plongeon,
10 saut), 4 coups (0 et 2 griffes avec saisie éventuelle, 4 coup bref, 6 charge latérale, 8 saut en arrière, 10 plongeon
sur la cible, 12 la créature fond sur sa cible), 6/8/10 repos, 12 saisie d'un joueur (la victime `+0x54` perd de la vie),
14 reprise du combat. Les transitions (`go::*` dans `mod.rs`) posent le long d'état et quelques drapeaux. L'état touché
utilise une table de réactions (0x6518a), un recul, une chute et une mort en plusieurs temps. `A5+0x6e`, compteur d'images,
est masqué en place par plusieurs ennemis (`andi.w #3,(0x6e,A5)`). Couverture : scénarios `gargoyle_scene42/55/63`, fuzz.

### Type 6 (0x61364) « Hellhound » (h) — `enemy_hellhound/`

Chien à huit variantes d'arrivée (`+3`) et deux profils d'IA (`+0x2f`). Charpente de `active_loop`. Sous-états : 0 arrivée
(0 hurlement, 2 demi-tour, 4 et 6 bond d'entrée, 8 chute d'en haut, 10 apparition), 2 debout (0 et 8 approche avec choix
d'un coup, 2 demi-tour, 4 attente, 6 crachat, 10 saut), 4 coups (morsures, patte, souffle de feu 16), 6 gestes d'affût,
8 gestes de repli, 10 promenade sans joueur, 12 morsure d'un joueur terrassé (`+0x50`, `+0x188`). Les tables de coups
sont celles de la ROM (0x62f80/0x62f88 par profil, 0x637fe, 0x61a00, 0x62284). Un coup fini rejoue l'approche par
`reach_boxes` (0x61aa4) ou `after_strike` (0x63790). Piège : le geste `+6` = 12 du sous-état 6 ne remet pas `+0xa1` à zéro.
Couverture : scénarios `hellhound_scene*`, fuzz (code d'origine vu hors deux restes).

### Type 18 (0x703dc) « Ogre » (h) — `enemy_ogre/`

Même cadre que le Gargoyle (bibliothèque `enemy_lib`, sous-états 0 arrivée, 2 déplacements, 4 coups, 6/8/10 repos). Le même
gestionnaire sert l'ogre (variantes 0 à 2) et sa variante négative `0xff`, seule présente dans les scènes 65 et 66 : elle
n'est pas saisissable, calcule ses PV comme un type 6 (0x453c) et meurt avec l'objet `+0x46` (0x71dde). Les gestes
« bras » créent un objet ESHL de type 0x2a (lié par `+0x42`/`+0xb0`) : `+6` = 2 et 4 le lancent variante 1 (l'ogre monte
hors de l'écran, `+0x58` prévient le bras, puis redescend sur la cible), `+6` = 8 variante 2, `leap` du sous-état 2 variante 0.
La table de choix 0x71b44 n'a que des zéros (toujours le plongeon de côté), les gestes de palier sont dans 0x71ba4/0x71c32.
L'état touché a sa table de réactions (0x715d4), une mort en cinq temps et une fuite hors de l'écran. Couverture : scénarios
`ogre_scene65/66`, fuzz.

### Type 0 (0x54914) « Kobold » (h) — `enemy_kobold/`

Charpente du Hellhound (squelette-like) à quatorze variantes d'arrivée (`+3`, états de départ 0x02000000.. posés à la
création) et quatre profils d'IA (`+0x2f`). L'aspect courant est `+0xa2` (précédent `+0xa3`), le geste à reprendre après un
demi-tour `+0xa5`/`+0xa6`. Sous-états : 0 arrivée (11 entrées `+6`), 2 debout (approche, demi-tour, attente, course), 4 coups
(19 gestes : massue, double coup, bonds, patte, souffle, pas de côté, séries), 6 et 8 gestes d'affût et de repli, 10 promenade,
état touché 0 à 8 (recul, chute, fuite). Particularités : le gel `+0x53` négatif est remis à zéro puis le sous-état s'exécute ;
`zone_shadow` est sauté quand `+6 = 0xe` ; la plupart des gestes commencent par `begin_wait` (0x557bc) et `hold` (0x5590e,
poursuite en place avec demi-tour) ; plusieurs tables de sauts ont du code juste après leur fin, que le listing omet
(0x55940, 0x55c30, 0x55c44, 0x560d2, 0x56034). Couverture : scénarios `kobold_scene*`, fuzz.

### Type 15 (0x6da20) « Goblin » (h) — `enemy_goblin/`

Combattant à l'épée de la même famille que le Troll et l'Axe Gnoll (même code de départ : paramètres d'IA `+0x68..+0x76`,
variantes d'arrivée paires, gestes choisis dans des rangées de 32 octets par niveau de difficulté `A5+0x7f92 / 2`).
Sous-états : 0 arrivée (variante `+3` = 0 marche, 2 et 4 chute, 6 course d'entrée puis coups jusqu'à être saisi), 2 debout
(approche avec boîtes de portée 0x6eb2a/0x6eb1c, demi-tour, attente, sortie d'écran), 4 gestes (provocation, séries de
coups `+6` = 4, pas de côté, charge, repos, retour à la marche), 6 (pas en profondeur, esquives, téléportation près de la
cible), 8 (suite d'une série : pause, pas en arrière, bond, rechoix), 10 promenade. L'état d'initialisation enchaîne sur le
corps de l'état actif dans la même image. Le geste choisi est mémorisé en `+0xaa` (tables 0x6ebc0 proche, 0x6edc0 loin,
0x6e904 suite). `walk_off` rejoue les remises en liste (`jmp 0x1ee1a`) de l'original. Les tables de données lues par
`lea (x,PC),A4` sont dans l'espace des données (`row_byte`), celles lues par `(x,PC,Dn)` dans l'espace des opcodes
(`react`, positions de téléportation). Couverture : fuzz seulement (aucune scène du jeu ne contient le type).

### Type 14 (0x6c4d6) « Troll » (h) — `enemy_troll/`

Le Goblin avec : trois variantes d'arrivée (0, 2, 4), un coup immédiat quand le joueur est au contact (boîte −0x20..0x20),
un test « plus aucun joueur en vie » fait sur les octets d'état des quatre joueurs (`A5+0x5de/0x7de/0x9de/0xbde`) après le
geste, des sauts à la place des esquives accroupies, un lancer d'objet (`+6` = 4, `+0xab` charges, objet créé par 0x9f346
et gardé en `+0xa6`) et une fuite à pied quand un joueur le saisit. Couverture : fuzz seulement.

### Type 13 (0x6ab52) « Axe Gnoll » (h) — `enemy_gnoll/`

Même départ de code que le Goblin et le Troll (paramètres d'IA, rangées de 32 octets 0x6c0d6 proche / 0x6c2d6 loin /
0x6be1a suite) mais armé d'une hache : six variantes d'arrivée (0 marche, 2 et 6 chute, 4 bond latéral, 8 et 10 arrivée
armée où la hache est lâchée par la clé d'animation 9 et reprise par la clé 0xb, 0xa114a), chaque coup fixe `+0x44`
(type) et `+0x45` (force), les sauts sur la cible (`+6` = 12, 16, 18, 20) recopient sa position et retombent d'en haut.
La routine 0x81aa4 est appelée à chaque image après le sous-état, avant la zone et l'ombre (toujours testées). Les
étapes 12 et plus des tables des gestes 18 et 20 n'ont pas d'appelant dans le jeu mais sont réécrites. Couverture : fuzz
seulement.

### Type 3 (0x5bda6) « Bow Gnoll » (h) — `enemy_bow/`

Archer indépendant des trois précédents. `run` appelle toujours les compteurs (0xf1bfc) et choisit une cible si `+0x50`
est nul. Sous-états : 0 arrivée (variantes 0 et 2 marche, 4 chute, 6 apparition, 8 course), 2 debout (approche, marche
latérale, boîtes de portée 0x5cdf6/0x5ce06/0x5ce14 qui lisent A1 et D5 laissés par `0xb5e0`), 4 tirs (premier geste
tiré dans la rangée 0x5d47a, second dans 0x5d67a, nombre de tirs dans 0x5d07a), 6 pas de côté et sauts, 8 repos,
10 attente, 12 demi-tour, état touché (réactions par table, chute, saisie : demi-tour vers le bord le plus proche). Un
Bow Gnoll créé par un autre objet (`+0x42`) tire en place (`spawned.rs`), sans zone ni ombre, et sort de l'écran quand
sa durée de vie `+0xa8` est écoulée. L'état à reprendre après un demi-tour est le long `+0xa0` (dont l'octet `+0xa2` est
l'étape `+6`, remis à 0). Couverture : fuzz seulement.

### Type 11 (0x67854) « Mud Golem » (h) — `enemy_golem/`

Colosse dont le code est un amas de gestes. Créé près du bord droit de la caméra (`init` n'enchaîne pas sur l'état actif),
il se tient debout (`+5` = 0 ou 2) en étant piloté par les touches du quatrième joueur (`A5+3` : croix, bouton 4 = coup,
bouton 5 = saut) ; l'IA part des sous-états 4 (coups, 11 gestes `+6`), 6 et 8 (repli : bonds, accroupi, sortie d'écran), que
la routine 0x690ce choisit dans la table 0x69146 d'après deux boîtes de portée. Étapes communes à plusieurs gestes :
`next_anim` (0x6935c), `wait` (0x69346), `turn` (0x6899a, selon `+0xa6`). Particularités : les boutons sont relus dans D0
après l'animation, qui l'écrase ; le test `cmpi.b #0x12,(0x7,A0)` de 0x68016 porte sur l'étape `+7` ; le drapeau de coup
`+0x8f` (`hit_gate`, 0x68910) redirige les sous-états 6 et 8 et l'état touché. Plusieurs états de repli (0x2020600,
0x1ed2a0) mènent à des tables trop courtes : ils sont réécrits tels quels. Couverture : fuzz seulement.

### Type 27 (0x77d12) « WarMachine » (h) — `enemy_warmachine/`

Machine de siège roulante qui s'appuie sur `enemy_lib` comme le Gargoyle et l'Ogre. `run` appelle toujours les compteurs
(0xf1bfc) puis, après le gestionnaire d'état, efface `+0x38` si `+0xad` et remet l'objet en liste si l'octet `+0` est non
nul ; l'état 0 n'enchaîne pas sur l'état actif (la variante `+3` = 0, 1 ou 2 donne `+6` = 0, 2 ou 4 par un talon). Sous-états :
0 arrivée (roule depuis le bord, ou attend un joueur dans une boîte), 2 roule vers le joueur (attente `+6` = 0, demi-tour 2, marche 4
et 6 avec délais de charge, charge 8), 4 gestes de l'arme (0 à 4 canon, 6 charge en ligne, 8 bond d'écrasement, 10 salve, 12 lancer),
6, 8 et 10 tourne sur place sans joueur, état touché 0 à 10 (mort en cinq temps `+5` = 6, fuite 8). Les décisions d'IA
(`decide.rs`) enchaînent des tirages dans des tables d'octets puis de mots (saut relatif) et aboutissent à l'un des « talons »
d'état de l'original (0x7924a à 0x792be, `go::*`) ; on relit les mêmes tables (espace des opcodes) et on exécute le talon
visé, y compris ceux qui n'entrent qu'au milieu d'un talon (0x7928e saute le `+0xad`). Couverture : fuzz seulement.

### Type 25 (0x76f0e) « DarkWarrior 1 » (h) — `enemy_warrior.rs`

Bras du guerrier sombre : le créateur (0x76ec4, non réécrit) en engendre deux, variante 0 (gauche) et 1 (droite), liés
à l'hôte `+0x46` ; si l'hôte n'est plus actif le bras passe à l'état 6. Sans IA : il se place à 0x82 pixels de l'hôte
(`+0xa8..+0xac` de l'hôte) et joue ce que l'état `+0xa2` de l'hôte (0 à 8 seulement : la table 0x76fd4 n'a que neuf
entrées) lui commande : 0 à 3 repos (ondulation de la table 0x7759e), 4 et 5 coup (`swing`, pour un seul des deux
bras, l'autre dérive), 6 à 8 sortie en scène par scénarios de déplacement (table 0x77626, `script_step`). La position
se calcule en ajoutant à la position des décalages `+0xb4/+0xb6/+0xb8` (que les vitesses `+0x14..+0x1e` intègrent,
`integrate`) ; les états de l'hôte 4 à 8 les ajoutent deux fois (une dans le sous-état, une dans `apply_motion`), comme
l'original. Les types 22, 23, 24 et 26 sont ses doubles graphiques. Couverture : fuzz seulement (507 instructions sur
515).

### Type 21 (0x756c4) « Harpy » (h) — `enemy_harpy.rs`

Créature volante à neuf variantes (ligne de huit octets par variante en 0x756e6 : orientation, durées, décalage) qui
crée à sa naissance un compagnon (ESHL 0x10, `0xa036e`) dont `+0xaa` garde le lien. `+0x2f` (0, 2 ou 4 : indexe des
tables de mots) choisit la série d'attaques, `+0x43` la forme. Sous-états : 0 vol (selon la variante), 2 attaque
(projectile `0xa0566`, coups, tir), 4 pause, 6 sortie d'écran, 8 sortie du premier plan. Le dispatch de sortie lit
D0 laissé par la table de vitesses : l'angle `+0x2d` doit rester sous 0x40. Couverture : fuzz seulement.

## 4. Non réécrit (types encore en 68000)

Aucun : les 29 types de la table 0x39a78 sont réécrits (le 17 est nul). Les types 0 Kobold, 6 Hellhound, 8 Gargoyle,
9 GiantBeetle, 10 GiantScorpion et 18 Ogre ont des scénarios (`scenarios/enemy/{kobold,hellhound,gargoyle,beetle,scorpion,ogre}_scene*.txt`) ;
les types 3 Bow Gnoll, 11 Mud Golem, 13 Axe Gnoll, 14 Troll, 15 Goblin et 27 WarMachine n'apparaissent dans aucune scène des
niveaux 1 à 66 (`scenarios/cov/`, `enemy_scan`) : ils sont validés par le fuzz seul.

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

### Méthode pour réécrire un type restant

1. `SOM_ROOT=<dépôt principal> python3 tools/handler_listing.py <début> <fin>` (bornes : adresses des gestionnaires
   voisins dans la table 0x39a78) donne le code atteignable et les tables de sauts ; lire le code de haut en bas
   en notant pour chaque routine : étapes `+7`, minuteries (`+0xa2`...), animations, tables de données, appels.
2. Écrire `families/enemy_<type>/{mod,stance,attacks,hit}.rs` sur le modèle d'`enemy_ghoul/` (le plus proche d'une
   charpente classique) en s'appuyant sur `enemy_common` ; enregistrer le module dans `families/mod.rs`.
3. Scène : `TYPES_SCENES=1 examples/types ...` sur `scenarios/cov/` (ou un parcours `autobot --plan 1:700,2:700,...` :
   les scènes 1 à 66 sont faites, aucune ne contient les types 3, 11, 13, 14, 15 et 27 ; 18 n'apparaît qu'en
   scène 66), puis une ligne dans `tools/gen_enemy.py`.
4. `DDSOM_CHECK=1 DDSOM_CHECK_ALL=1 DDSOM_SANS=game_phase12,world_update,dispatch validate ...` : les lignes `REGS`
   de l'ennemi doivent disparaître (A6/D6/D1 du préambule, A1 après `movea.w`, D0 après `andi.w`...). Hors mode
   contrôle, un écart qui subsiste avec `DDSOM_SANS=<nom de la native>` n'est pas le sien : bisecter avec les autres
   préfixes (`players_update`, `combat`...) et ranger le scénario dans `scenarios/ko/`.
5. Fuzz : une entrée par type dans `fuzz_enemy/lot3.rs`, puis `DDSOM_ENTREE=<adresse> DDSOM_VARIANTES=1 fuzz_enemy
   familles ... 3500 100` et `DDSOM_COUVERTURE=<fichier>` pour vérifier les instructions de l'original vues.
   Les premiers écarts viennent presque toujours d'un `return` dans un bras de `match` qui saute la fin commune
   du sous-état (zone, ombre) ou d'un appel oublié.

### Scénarios `scenarios/enemy/` et fuzz du troisième lot

`tools/gen_enemy.py` (comme `gen_boss.py`) entre directement dans une scène où le type apparaît (décimal ; les scènes
se trouvent avec `TYPES_SCENES=1 examples/types <roms> <eeprom> scenarios/cov/<tour>.txt 30000`, qui donne les appels
par scène `FF8034`) et fait combattre le bot, joueur invincible. **Un seul passage de scène par scénario** : un saut
de scène (`--plan a:n,b:m`) provoque la « tempête de pas » (voir `couverture.md`). `validate_all.sh` les valide à
12 000 images. Le fuzz du lot est `examples/fuzz_enemy/lot3.rs` (états `(+4,+5,+6,+7)` valides par type, décrits par
des lignes `"s4 s5 s6,.. max7"`, champs tolérés) branché dans `fuzz_enemy.rs` : `DDSOM_ENTREE=<adresse du gestionnaire>
DDSOM_VARIANTES=1 fuzz_enemy familles <roms> <eeprom> <scénario> 3500 <tours>`. Outils ajoutés : `pctrace`
(`PCTRACE_SEUL`, `PCTRACE_IMAGES`), `DDSOM_TRACE_ORIG=<nom de la native>` dans `lib.rs` (trace du code d'origine pour
les écarts d'une native en mode contrôle).
Pièges rencontrés : **0xfbae n'est pas un `rts`** (test de zone, remet D5 à 0 ; l'index du pseudo-C le donne pour
2 octets) ; les routines appelées par un sous-état reviennent à la boucle de l'état, qui exécute la suite (zone,
ombre) : un `return` dans un bras du `match` d'un sous-état saute cette suite ; une variante `+3` indexe parfois
directement une table de mots (valeurs paires) ; `bgt`/`bpl` après un `jsr` lisent les drapeaux de la routine
appelée (`Cx::c.cond`) ; une table de sauts suivie de code (`jmp (table,PC,Dn)`) a pour longueur l'adresse du
premier code moins la base ; les registres que le handler laisse (A6/D6 du préambule, A1 `movea.w (0x50,A0),A1`,
D1 `moveq`...) se corrigent avec `DDSOM_CHECK=1 DDSOM_CHECK_ALL=1` (lignes `REGS`).

### Fuzz des types du deuxième lot (4, 19, 21, 25)

`fuzz_enemy.rs` a pour ces types : `etats_enemy` (états `+4/+5/+6` valides), `etape_dbeast` / `etape_owlbear`
(étape `+7` valide selon l'état), `prep_dbeast` / `prep_owlbear` / `prep_harpy` / `prep_warrior` (compteurs courts, animation en cours
souvent égale à l'animation voulue, état à reprendre après un demi-tour, code de réaction `+0x49` < 43 entrées,
parent dans un état connu, objet au ras du sol pour les réceptions de bonds), joueurs placés dans les boîtes de portée,
redessin d'un état valide à un pas sur trois. Variables : `DDSOM_GRAINE=n` (graine), `DDSOM_PAS_ETAT=1` (trace des états
avant/après), `DDSOM_COUVERTURE=fichier`. Pièges des fuzz : `+0x2f` doit valoir 0/1 (type 19) ou 0/2/4 (type 21), le
propriétaire ne doit pas être l'objet lui-même, `+0xa2` du type 21 et `+0xa4` des types 4 et 19 ne se corrompent pas
(l'original saute alors dans des données).
