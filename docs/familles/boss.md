# BOSS (C04) : les boss (`families/boss_*.rs`)

Pool de 3 objets de 0xC0 octets (FFBA1E, compteur de places libres inutilisé), table de gestionnaires 0x39b12 (23
entrées, dispatch 0x39aec). Même cadre que les ennemis (`docs/familles/enemy.md` §2, `Cx`, `handlers!`, contrat A0/D7 :
`docs/familles-eshl-set-sobj.md`) : `+4` état (0 création, 2 actif, 4 touché, 6 libération), `+5`, `+6`, `+7` sous-états,
octets pairs qui indexent des tables de sauts. `boss_common.rs` réexporte `enemy_common` (champs, états, `alloc`,
`suspended`, `random`...) et ajoute les variables de scène des boss (`var::*`, `leader`, `scroll_x`).
Les noms de rôle (h) sont des hypothèses tirées du comportement et des noms de debug du jeu (atlas, annexe A).

## 0. Recensement des tables et des entrées encore en 68000

Les 23 entrées de la table BOSS 0x39b12 (index = type) et les 29 de la table ENEMY 0x39a78 sont toutes couvertes par une native
Rust ; il ne reste plus aucun gestionnaire de ces deux classes en 68000.

| Type BOSS | Entrée | Native | Type BOSS | Entrée | Native |
|---|---|---|---|---|---|
| 0 | 0x794ae | `boss_0/` | 12 | 0x8d59e | `boss_11/` (même routine que 11) |
| 1 | 0x7acea | `boss_1/` | 13 | 0x8f59c | `boss_dragon.rs` |
| 2 | 0x7c5a8 | `boss_22/` (avec `two.rs`) | 14 | 0x8fae4 | `boss_19/` (même routine que 19) |
| 3 | 0x7f41a | `boss_3/` | 15 | 0x91530 | `boss_15/` |
| 4 | 0x81b3a | `boss_4/` | 16 | 0x9325e | `boss_16/` |
| 5 | 0x8341c | `boss_5.rs` | 17 | 0x952b4 | `boss_17/` |
| 6 | 0x8430e | `boss_6/` | 18 | 0x975ec | `boss_18/` |
| 7 | 0x859f8 | `boss_7/` | 19 | 0x8fae4 | `boss_19/` |
| 8 | 0x87fc0 | `boss_8/` | 20 | 0x91530 | `boss_15/` (même routine que 15) |
| 9 | 0x8a75c | `boss_9/` | 21 | 0x9947a | `boss_21/` |
| 10 | 0x8ca4a | `boss_10.rs` | 22 | 0x9aeaa | `boss_22/` |
| 11 | 0x8d59e | `boss_11/` | | | |

Les types 0, 2, 16 et 22 étaient les derniers en 68000 : ils n'apparaissent dans aucune des scènes que les parcours atteignent
(`boss_scan` ne reste que 500 images par scène) mais le script d'apparition de la scène 20 crée le type 0, celui de la scène 14 le
type 16, et la fin de la scène 10 (action de fin 9 du contrôleur de scène, qui crée le SET 17 de l'arrivée du boss) le type 2, qui
crée à son tour le type 22 : ce sont de vrais boss (aucun n'est du code mort). Ils sont exercés par des scénarios qui les créent
à la main (`autobot --spawn`, `tools/gen_boss.py`, voir §3).

ENEMY (0x39a78) : types 0 Kobold à 28 Nagpa, le 17 vaut 0x39aec (la boucle de dispatch BOSS : jamais créé) ; chaque type a sa
native (`enemy_*`), sans table de variantes en 68000. Les sous-tables des gestionnaires (réactions aux coups par code `+0x49`, tables
de décision, jeux d'animations) sont lues en ROM par le Rust ; les réactions communes (0x3bb9c-0x3dab0) sont dans `reactions.rs`,
les seules routines de réaction propres à un boss sont réécrites avec lui (0x4c : tremblement, 0x7a442, 0x7bc6a...).

Appels qui exécutaient du 68000 depuis les gestionnaires de BOSS et d'ENEMY (docs/reste68k.md) : les six petits créateurs d'ESHL
0xa6824 (boss 5), 0xa2f96 (boss 7), 0xa460a, 0xa464e, 0xa4676 (boss 15) et 0x9ddee (ennemi dbeast) sont désormais des fonctions
Rust (`enemy_common.rs`, `eshl_pair_3f`, `eshl_burst_27`, `eshl_shot_34`, `eshl_thrown_by_beast`) ; plus aucune instruction
d'origine n'est exécutée dans ces modules. Les bibliothèques d'IA et les sous-routines d'effets qu'ils appellent encore par
adresse (0x10112, 0xe3d32, 0xed836, 0xb59a6...) sont comptées avec leur famille (EFFCT, ESHL, SET).

## 1. Qui apparaît où

Relevé par `examples/boss_scan.rs` (saute de scène en scène sur la machine d'origine) et `examples/boss_seen.rs`
(rejoue un scénario et note la première image de chaque boss) :

| Type | Handler | Octets | Nom (h) | Scènes |
|---|---|---|---|---|
| 13 | 0x8f59c | 1 352 | dragon noir en vol | 7, 9 (visible aussi pendant 6 et 12 : l'objet survit au changement de scène) |
| 17 | 0x952b4 | 9 016 | Tel'Arin (h) | 13, 44 (variante 1) — réécrit |
| 6 | 0x8430e | 5 866 | Manticore | 24 — réécrit |
| 18 | 0x975ec | 7 822 | Lich (h) | 26 — réécrit |
| 14 / 19 | 0x8fae4 | 6 732 | dragons vert / noir (h) | 32 (19, variante 0), 65-66 (19, variante 1) — réécrit |
| 12 / 11 | 0x8d59e | 8 190 | salamandres de feu / de glace | 40 / 42 — réécrits |
| 21 | 0x9947a | 6 704 | compagnon de Tel'Arin (h) | 44 — réécrit |
| 15 / 20 | 0x91530 | 7 470 | dragon rouge | 46 — réécrit |
| 5 | 0x8341c | 3 826 | Ezerhorden | 58 — réécrit |
| 8 | 0x87fc0 | 10 140 | DarkWarrior 2 | 64 — réécrit |
| 7 | 0x859f8 | 9 672 | DarkWarrior 1 | démonstration de l'attract (pose), et en partie dans `cov_2p_c`, `cov_tour4_c3` — réécrit |
| 10 | 0x8ca4a | 2 900 | Nagpa | 65, 66 — réécrit |
| 4 | 0x81b3a | 6 370 | Chimera | scénarios `trog_scene104`, `cov_2p_c`, `cov_tour4_c3` — réécrit |
| 1 | 0x7acea | 6 334 | fauve bondissant (nom de debug « Displacer Beast ») | 27 (`covb_long_27`) — réécrit |
| 3 | 0x7f41a | 10 016 | créature volante (« Harpy ») | 11 (`scene_11`, `covb_long_11`) — réécrit |
| 9 | 0x8a75c | 8 942 | engin de guerre roulant et son équipage (h) | 2 (`covb_long3p_02`) — réécrit |

Les types 14 et 19, 15 et 20, 11 et 12 partagent un handler. Les quatre derniers, vus seulement par les scénarios qui les créent à
la main (les parcours de `boss_scan` ne durent pas assez pour déclencher leur apparition) :

| Type | Handler | Octets | Nom (h) | Scène |
|---|---|---|---|---|
| 0 | 0x794ae | 6 100 | créature qui se dédouble (une seconde moitié, variante 1, lui est liée) | 20 (script d'apparition, `x` = 736) |
| 16 | 0x9325e | 8 300 | boss à charges et prises | 14 (script d'apparition, `x` = 1024) |
| 2 | 0x7c5a8 | 11 900 | boss de fin de la scène 10 (action de fin 9 : SET 17 d'arrivée du boss) | 10 |
| 22 | 0x9aeaa | 9 000 | partenaire du type 2 (créé par lui à trois joueurs, ou à deux joueurs sur une difficulté d'au moins 0x17) | 10 |

## 2. Réécrits

### Type 13 (0x8f59c) : dragon noir en vol — `boss_dragon.rs`

Ce n'est pas un adversaire : il accompagne la caméra (`A5+0x506`) à `scroll - 0x80`, flotte (`bob` : tous les 16 images,
quatre vitesses tirées à `4 × (tirage & 7)` octets dans la table 0x8faa2, qui se chevauchent comme dans l'original,
puis leurs opposées), et déclenche des étapes quand la caméra dépasse les seuils 0xf00, 0x1e00, 0x2d00, 0x3c00 de la
table 0x8f97a (`next_stop`, compteur `+0xa7` ; la lecture au-delà du 4e seuil lit des opcodes). Création : au moins
deux places ESHL libres et un meneur de la caméra (`A5+0x46`), deux tirs ESHL 0x14 (variantes 1 puis 0, `+0x50` = le
dragon) et le jeu d'animations 0x17d7d2. Sous-états `+6` de l'état actif (`+5` = 0 ; `+5` = 2 libère) :

| `+6` | Rôle |
|---|---|
| 0 | prépare l'étape (efface les bits `+0x42`, appelle 0xdf4bc) |
| 2 | attend le seuil suivant ; si le bit 0 de `+0x42` est levé : fige le jeu (`A5+0x3d/0x3c/0x33` = 1, `+0x16a` des joueurs), ouvre un message (0xc8312, type 0xe, lien `+0xb0`) et lance la cinématique (0xe0d8c) |
| 4 | attend la fin du message (octet `+0x22` du message), rend la main aux joueurs, repart vers le meneur |
| 6 | s'approche du meneur (`+0x58` du meneur ajouté à X ; 0x1e92 teste l'arrivée) |
| 8, 12 | suit le meneur dans la fenêtre `[x - 0xe0, x - 0x60]` pendant `+0xa2` images |
| 10 | idem puis tient la position tant que le bit 0 de `+0xa5` |
| 14 | sort : retient la vitesse du meneur moins le défilement, ou -8, ou -0x200 si `0xff0ec6` ; hors de ±0x200 de la caméra : fin de cinématique (0xe0c5c), retour à l'état 2/0 |

L'octet `0xff0ec6` (`A5-0x713a`) force le départ. Les états 4 et 6 de `+4` appellent la libération d'ENEMY `0x21f4` (l'original
ne libère pas le boss par `0x2234` : ils ne sont jamais atteints). Couverture : fuzz `fuzz_boss` (300 instructions sur 301 en
300 tours, toutes en 600), `DDSOM_CHECK=1 DDSOM_CHECK_ALL=1` sur le scénario (12 000 images : VALIDE).

### Type 17 (0x952b4) : boss des scènes 13 et 44 — `boss_17/`

Combattant à trois variantes (`+3`) : 0 est le boss de la scène 13, 1 et 2 des apparitions annexes (scène 44). Animations
0x18076e (mouvements), 0x1807a2 (coups), 0x1807c0 (recul) ; `+0xa0` est le jeu courant (0x30 à 0x34 : séquences
spéciales sans ombre ni test de zone). Fichiers : `mod.rs` (boucle, création, champs `g::*`), `intro.rs` (`+5` = 0 : arrivée,
entrée, mort et cinématique de fin, variantes 1 et 2), `fight.rs` (`+5` = 2 : attente, demi-tour, marche, charge),
`decide.rs` (changements d'état et tirages dans les tables de la ROM : choix d'attaque selon la distance, difficulté,
nombre de joueurs), `attacks.rs` (`+5` = 4 : neuf attaques `+6` = 0 à 16), `hit.rs` (état touché : réactions par la table
0x96a0c dont une seule, 0x96ab8, est propre au boss, recul, relevé, mort).

Particularités : (1) la boucle `run` rejoue la logique dans la même image tant que le compte `+0xba` du ralenti (`+0xbb` non
nul) reste positif ; (2) la phase `+0xaa` (0 à 4) monte quand les PV passent sous les seuils de 0x10ee2 (variante 0) / 0x110e2
(105b6) et règle le choix des attaques ; (3) `+0xad` non nul rend invulnérable (`+0x38` remis à zéro) ; (4) le long d'état
`+4..+7` est posé d'un coup (`0x0202_0400` = actif, combat, marche) ; (5) les tirs ESHL 0x20/0x21, les effets EFFCT
0x31/0x32/0x25/0x27/0x10/0x3b/0x3c et les bulles de dialogue (0xc8312, octet `+0x22` du message = terminé) sont créés
par le boss ; (6) l'original passe D2/D3 (restes du calcul de terrain 0x24b0) comme vitesse à l'effet 0x1d créé par 0x103ba à la
mort des variantes 1 et 2 : le natif `video_terrain` laisse donc D3 et D4 comme l'original.

### Type 6 (0x8430e) : Manticore (h), scène 24 — `boss_6/`

Même gabarit que le type 17 (états en longs, tables de décision de la ROM, bibliothèque d'IA 0x103d6..), sans ralenti ni
phases de cinématique de fin. Animations 0x177874 (sol), 0x17788c (vol), 0x1778a4 (recul). Groupes (`+5`) :
0 `intro` (bond d'arrivée depuis la droite, atterrissage, attente de la caméra `A5+0x2c` = 0xc), 2 `fight` (attente, demi-tour,
marche, charge `+6` = 8 vers la caméra ou vers la position mémorisée de la cible si `+0xb2`, tir en bond `+6` = 10 avec un
ESHL 0x2a), 4 `attacks` (rugissement, deux piqués avec battements d'ailes, bond sur la cible, vol bas avec tir variante 2),
6 à 12 attente sans joueur, 14 reprise ; état touché dans `hit` (table de réactions 0x8527a dont la seule routine propre,
0x85326, est l'étourdissement ; mort en cinq temps). Les variantes `+3` non nulles sont des parties liées à l'hôte `+0x46`
qui meurent avec lui (`guard`, 0x859b2). La phase `+0xaa` (0 à 2) monte par 0x105b6 d'après la table de PV 0x10de2.
Couverture : fuzz 1 183 instructions sur 1 247 (le reste : tables de choix d'attaque très improbables), contrôle appel par
appel VALIDE sur le scénario ; `scenarios/ko/boss6_scene24.txt` est VALIDE sur 12 000 images avec `DDSOM_SANS=combat_update`.

### Type 10 (0x8ca4a) : Nagpa (h), scènes 65 et 66 — `boss_10.rs`

Le plus court des boss, même gabarit. À sa création il joue l'ouverture (`+5` = 0) quand la scène l'autorise (`A5+0x2e` = 6) :
il crée un ennemi de type 18 (variante 0xff) et un boss de type 19 (variante 1) liés à lui par `+0x46`. Combat (`+5` = 2) :
attente, demi-tour, marche vers un joueur, puis saut sur une position tirée autour de la caméra (`+6` = 6) ; selon la
distance, tir (`+5` = 4) : ESHL 0x21 lâché en cinématique (`A5+0x51e` = 4). Choix de la suite (`next_action`, 0x8d526) :
table de 32 octets (étapes 0, 4 ou 6). Touché : table 0x8d176 dont l'étourdissement 0x8d294 (compteur de coups `+0x84`),
relevé, mort en cinq temps. Sources de piège : `0x43ae` reçoit D2 en entrée et le modifie (le saut ajoute le D2 qu'il rend si
aucun joueur n'est trouvé) ; le natif `ai_pick_target_any` a une durée estimée qui diffère de l'original de 10 à 20 cycles
(sans effet sur l'état). Couverture : fuzz 573 instructions sur 578.

### Type 5 (0x8341c) : Ezerhorden, scène 58 — `boss_5.rs`

Gabarit ancien : pas de hook de pause ni de longs d'état ; `+5` (multiples de 4) et `+6` sont posés champ par champ, et
`+0xa8` mémorise l'état avant et après chaque passage. Animations 0x175da6 (corps), 0x175dc4 (attaques), 0x175e0c
(chute, mort), mémorisées en `+0xa4` (numéro) et `+0xa5` (jeu) pour ne pas relancer la même. `next_action` (0x83e3e) choisit
le groupe suivant d'après la position de la cible : une ligne de la table 0x83f4e (distance en Y, puis en X devant le
boss, plus un tirage), deux octets par entrée (angle et groupe, durée et demi-tour).

| `+5` | Rôle |
|---|---|
| 0 | arrivée : attend la scène (`A5+0x2e` = 4), cinématique (`A5+0x2c` = 0xc), crée un SET 0x73 |
| 4 | marche lente (`+0xa0` images, selon la difficulté) |
| 8, 0x14 | déplacement (tables de vitesses 0x8369c / 0x836fa), sens selon `+0x2c` |
| 0xc | saisie d'un joueur (`+0x50`) par 0xfd64 : lui retire 1 PV tous les 4 images (0x10 à la prise), le relâche à 1 PV |
| 0x10 | attaque simple |
| 0x18 | poursuite par angle (`+0x2d`, 32 directions) puis saisie à portée |
| état 4 | touché : recul puis reprise (ou poursuite), chute en tourbillon si `+0x49` = 0xa8 (`+6` = 0 à 0x10 : lancée, vol par la table de vecteurs 0x1c94b6, retombée, rebond), mort (`+5` = 6) |

Particularités : la reprise après un coup sans effet (`+0x49` = 0x38) rappelle la boucle de l'état actif depuis l'état
touché (et revient à la fin de l'état touché : zone et ombre) ; les `+6` hors tables (6 impair ou trop grand) sautent dans le
code de l'original à des adresses sans sens, la porte `etats_boss` du fuzz s'en tient aux valeurs des tables.
`addq8` (boss_common) reproduit les drapeaux de `addq.b` avant les appels sonores qui rendent les drapeaux reçus.
Couverture : fuzz 585 instructions sur 594 (reste : l'appel 0x9b6 de l'ouverture, qui ne revient pas en fuzz).

### Type 21 (0x9947a) : compagnon de Tel'Arin (h), scène 44 — `boss_21/`

Cousin du type 17 (70 % d'instructions identiques) : mêmes groupes (`+5` = 0 intro, 2 combat, 4 attaques, 6 à 10 attente,
état 4 touché), même ralenti (`+0xbb`), mêmes bibliothèques d'IA ; tables (marches, choix d'attaque, seuils de phase 0x112e2,
budgets `+0xb2..4` initialisés à 8, 8, 3) et attaques propres. Il est lié à un hôte (`+0x46`, le Tel'Arin de la variante 1) :
quand l'hôte est mort (état 0x0406) il quitte la scène. Animations 0x1836ec (mouvements), 0x18371c (coups), 0x18373a (recul).
`mod.rs` (boucle, ralenti, création), `intro.rs` (arrivée par la droite, chute d'entrée), `fight.rs` (attente, demi-tour,
marche, charge), `decide.rs` (changements d'état et tirages à deux niveaux dans les tables de la ROM), `attacks.rs` (huit
attaques : coup simple, coup à effet, bond, saut sur la cible, rayon ESHL 0x20 (`+0xb2` restants), tir ESHL 0x21 en
cinématique (`+0xb3`), lâcher d'objets (`+0xb4`), tourbillon qui lance le ralenti), `hit.rs` (réactions, recul, relevé, mort).
Couverture : fuzz 1 342 instructions sur 1 383 (les choix d'attaque à portée très courte et quelques cases des tables).

### Types 14 et 19 (0x8fae4) : dragons, scènes 32, 65 et 66 — `boss_19/`

Une seule routine pour deux types (noms de debug : 14 « Green Dragon », 19 « Black Dragon ») ; le type 14 n'apparaît dans aucun
scénario (fuzz seulement). Le type 19 de variante 0 est le boss de la scène 32 ; celui de variante 1 est le dragon que Nagpa
(`boss_10`) fait venir dans les scènes 65 et 66 (`+0x46` = Nagpa : il disparaît quand Nagpa n'a plus de PV). Trois jeux
d'animations par type (`+0xa0` = 0, 2, 4 : `set_anim`, qui ne relance pas l'animation déjà en cours) ; le dragon flotte autour
de l'altitude 0x20 quand `+0xa4` est levé (`hover`). `mod.rs` (boucle, flottement), `intro.rs` (arrivée selon type et variante :
bulles de texte, entrée par la droite, attente de la caméra `A5+0x2c` = 0xc), `fight.rs` (marche par tirage et sondes de
portée, demi-tour, riposte spéciale quand les PV passent un quart de `+0x60`), `attacks.rs` (six attaques : bond arrière, charge,
souffle qui suit un joueur, saut périlleux, saut en avant, grand saut avec retombée de l'autre côté de l'écran), `hit.rs` (table
de réactions 0x909a0 : coup ordinaire, couché, chancelant, tremblement 0xa8 qui mène à la mort ; reprise du souffle si on
le touche pendant l'attaque 4 ; mort propre à chaque type).

Particularités de l'original reproduites : (1) la boucle de choix de cible du souffle (0x904aa, 0x904ee, 0x90524) détruit son
compteur `D0` en appelant 0x9059e et compare à -1 : seule 0x90524 trouve vraiment une cible ; (2) `move.w (-0x7f92,A0)` (0x90366,
0x903f4) lit à A0 - 0x7f92 au lieu du compteur d'images `A5+0x6e` ; (3) les sondes 0x91348 et 0x91390 sans cible écrivent l'état
« attente » et rendent vrai (Z levé par le `move.l`), ce qui envoie l'appelant vers un choix d'attaque ; (4) 0x9132e rend toujours
faux ; (5) `0x910b6` tire une attaque puis écrase l'état par « debout » ; (6) les tables à registre d'adresse (0x91096, 0x9110c,
0x90556) sont des lectures de données en clair, pas des lectures du programme déchiffré.
Couverture : fuzz 1 366 instructions sur 1 398 (restes : code mort et branches improbables).

### Types 15 et 20 (0x91530) : dragon rouge, scène 46 — `boss_15/`

Même famille que les dragons de `boss_19` (mêmes tirages de cible 0x43ae, mêmes sondes `0xb5e0`, même boucle de relevé) avec
d'autres tables ; le type 20 (jamais vu dans les scénarios) est une variante plus petite (PV, vitesses). `+0xa8/aa/ac` mémorise la
position à la fin de chaque image et la rétablit au début : un coup ne déplace pas le dragon. Armure (`hit.rs`) : pendant les
`+0xb5` (4) premiers coups de chaque période de 64 images (`+0xb4`), un coup reçu en état actif est ignoré (reprise de l'état de
l'image précédente `+0xb0`). `mod.rs` (boucle, flottement par table de sinus 0x1c67b6, prise d'un joueur), `intro.rs` (chute du
ciel, puis combat dans la même image), `fight.rs` (surveillance et tirage d'attaque selon la boîte de portée et le quart de PV,
riposte spéciale, bond), `attacks.rs` (dix attaques sur des étapes communes de montée, descente et pause), `hit.rs`.
Particularités : `0xb3fb0` relit `D1b` laissé par `set_anim` (numéro du jeu d'animations) ; `0xfd8c` relâche le joueur désigné par
`A1`, chargé au début de la prise ; l'étape 4 de l'arrivée enchaîne sur le groupe 2 sans rendre la main. Couverture : fuzz 1 303
instructions sur 1 369, VALIDE (les natives `ai_pick_target_any` et `son_enfiler_direct` désactivées : leurs écarts connus de durée et de
drapeau X ne viennent pas du boss).

### Type 18 (0x975ec) : Lich (h), scène 26 — `boss_18/`

De la famille des boss 17 et 21 (même charpente, tirages à deux niveaux dans les tables de la ROM, `decide.rs`), sans ralenti ;
particularité : l'état touché `hit.rs` a une routine de réaction par code de coup `+0x49` (table 0x986d6 de mots, neuf routines
distinctes) : recul sur place (animations 0 et 1), projections selon une ligne de vitesses de 0x993ac (codes 4 à 0x54, `hit_c` à
`hit_g`), soulevé avec bouffées d'effets (0x9c), rebonds jusqu'à la mort (0xa8) ; le recul `+5` = 2 interrompt la poursuite
magique. Animations 0x181ae0 (mouvements), 0x181af2 (coups), 0x181b10 (réactions). `mod.rs` (boucle, création), `intro.rs`
(quatre temps minutés avec sons), `fight.rs` (attente, demi-tour qui continue d'avancer, marche, charge, reprise), `decide.rs`,
`attacks.rs` (onze attaques : coup, pas de côté, geste à suite tirée, poursuite magique, bond arrière, boule ESHL 0x20, rayon ESHL
0x21, onde de choc qui renverse les joueurs, téléportation, pluie de 16 tirs 0x23, salve de tirs 0x24), `hit.rs`.
Couverture : fuzz 1 538 instructions sur 1 642 (VALIDE, natives `ai_pick_target_any` et `son_enfiler` désactivées : écarts connus
de durée et de drapeau X). Le scénario `boss18_scene26` (tronqué à l'image 10 900 par `gen_boss.py`) est VALIDE sur 12 000 images
(7 125 appels) ; le bot tue le Lich à l'image 10 541, la mort et la fin de scène sont donc exercées.

### Type 8 (0x87fc0) : DarkWarrior 2, scène 64 — `boss_8/`

Même charpente que les 17, 18 et 21 (états posés d'un coup en longs, sauts par tables relatives au PC), avec trois
particularités. 1) Un ralenti : quand ses PV passent sous le seuil `+0xb6` (moitié ou trois quarts des PV selon le nombre de
joueurs, `+0xad`) il met `+0xb9` à 1 ; le compteur `+0xba` (2, 1, 0) fait alors rejouer le gestionnaire une image sur trois
(`mod.rs`, boucle de `run`). 2) Ses réactions aux coups sont les routines communes des ennemis (table de longs 0x89330, indexée
par `+0x49`, appelées par `jsr (A1)`) : `hit.rs` ne réécrit que les groupes autour (projeté, chute, mort). 3) Le choix de
l'attaque suivante se fait dans des tables de 0x200 octets de la ROM (lignes de 32 octets selon la difficulté, 0x89866 à
0x8a2c6 : devant, loin, ralenti, après une attaque), lues à l'exécution.

| `+5` | Fichier | Rôle |
|---|---|---|
| 0 | `fight` | arrivée : attente, apparition, saut, atterrissage avec sept bouffées d'effets |
| 2 | `fight` | combat : marche (`+6` = 0, 8) qui regarde à portée, demi-tour (2), arrêt (4), approche lente (6) |
| 4 | `attacks` | dix attaques (`+6` = 0 à 18) : coup, fente (2 et 18), couteaux (4 et 6, `pick_new`), combo, charge, bond, pluie de couteaux (14), disparition (16) |
| 6, 8 | `moves` | esquives après une attaque (`aids::after_attack`) ou un coup : disparition et réapparition, pas de côté, bond, repli |
| 10, 12, 14 | `fight`, `mod` | attente sans joueur, debout, relevé |
| état 4 | `hit` | réaction commune, projeté (rebonds puis riposte tirée par `0x894cc`), chute, mort (effets avant ou arrière, clignotement) |

`aids.rs` regroupe les aides de l'original (0x8972e, 0x89760, 0x8a2c6, 0x8a368, 0x8a404, 0x8a432, 0x8a466, 0x8a49a, 0x8a4e4
à 0x8a666, 0x89788, 0x89804). Les registres hérités (D6 et A2 de la boucle sur les quatre joueurs, D0, D1) sont reproduits.
Animations : 0x1790f6 (mouvements), 0x179116 (coups). Fuzz : 257 000 appels sans écart (couverture 1 471 instructions sur
1 493 ; restent un cas inatteignable de `fight` — le nombre de joueurs en vie qui devient nul pendant l'étape —, deux sorties
du tirage « loin » de 0x8a432 et le choix de la troisième attaque en ralenti). Le scénario `boss8_scene64` est VALIDE sur
12 000 images (8 305 appels), le boss vivant (602 PV) ; `boss8_scene64_mort` (mort à l'image 6 134) est VALIDE sur 12 000 images.

### Type 7 (0x859f8) : DarkWarrior 1 — `boss_7/`

Le frère du type 8 : même charpente, mêmes champs (`g::*`), mêmes aides (`aids.rs` : 0x86fee, 0x87020, 0x870a2, 0x870b4,
0x87926, 0x87b86, 0x87c6c, 0x87c9a, 0x87ccc, 0x87cfe, 0x87d48, 0x87da2, 0x87e20, 0x87e9e, 0x87eca), tables et constantes
propres. Différences : hors partie (`A5+0x30` nul, démonstration de l'attract) il garde la pose (état `0x02000200`, groupe 0
étape 2) ; son arrivée est une simple chute ; le ralenti compte 3, 2, 1, 0 ; une créature d'appoint (SET 2/0x3f) paraît au
bout de 0xf0 images ; l'aire de jeu est absolue (x de 0x45f à 0x68f, y de 0x168 à 0x19f) ; le seuil des PV est la routine commune
0x87048. Neuf attaques (`+6` = 0 à 16) : coup, combo, couteaux (4 et 8), coup plongeant (6, sur la position retenue
`+0xb0/+0xb2`), charge, bond, pluie de couteaux (14), disparition. Groupe 8 : marche courte, repli, saut ailleurs, bond en
arrière (qui finit sur le combat, 0x86a7c, et non sur le choix d'une attaque). Animations 0x17809c (mouvements), 0x1780ba
(coups). Validation : `cov_2p_c`, `cov_tour4_c3` (1 326 appels, contrôle appel par appel sans écart), fuzz `fuzz_boss`
(`DDSOM_ENTREE=859f8 DDSOM_ETATS=1`, 3 × 300 tours : seuls les écarts connus du drapeau X, communs avec le type 8).

### Type 4 (0x81b3a) : Chimère — `boss_4/`

Charpente des boss à IA (bibliothèque 0x103d6..0x105b6, états posés en longs par `decide.rs`, 0x8328e..0x833ac). À la
création, trois doubles ENEMY (types 22, 23, 24 de `enemy_double`) liés par `+0xb4`, `+0xb6`, `+0xb8` ; les deux premiers ont
leurs propres PV (ceux du boss moins 0xfa ou 0x12c). Le premier double rejoue un geste quand le boss pose son `+0xb6`, et son
`+0x33` dit au boss quand souffler ; son `+0xaf` (occupé) fait remplacer les souffles par une autre attaque. Hors partie
(`A5+0x30` nul), le boss attend (groupe 10). Groupes : arrivée par le haut de l'écran (`fight::intro`), combat (`fight` :
debout, demi-tour, marches rapide et lente, charge vers le milieu de l'écran ou vers la cible, bond en arrière), attente sans
joueur, retour au combat (0x82ea0, aussi appelé par les réactions communes, table 0x3e000). Pendant la marche, quatre boîtes
devant le boss (0x83094 et 0x830b2 avec le geste de l'IA 0x1047a, 0xb5e0, 0x7ef4) mènent à `decide` : tirage de 0 à 15 dans une
table d'octets de la ROM choisie par la phase `+0xaa` (et le geste), puis saut vers un état. Sept attaques (`attacks.rs`) : cri,
envol puis piqué sur la cible, envol puis traversée de l'écran, bond par-dessus la cible (élan selon la distance), souffle en
vol (ESHL 0x32 en éventail, table d'angles 0x8295c), geste du double, souffle au sol (ESHL 0x32/1). En vol, battements d'ailes
(0x11a36, sur les marques `+0x33` à bit 7), vent des ailes (0x115e2), oscillation (0x833b6). État 4 (`hit.rs`) : réactions
(table de longs 0x82c8a ; le code 0x4c, 0x82d36, tremblement sur place, est propre au boss), recul puis traversée en riposte,
à terre, mort. Animations 0x17557e (corps), 0x175596 (vol, attaques), 0x1755ae (coups reçus). Validation : `trog_scene104`,
`cov_2p_c`, `cov_tour4_c3` (contrôle appel par appel), fuzz `fuzz_boss` (`DDSOM_ENTREE=81b3a DDSOM_ETATS=1`).

### Type 1 (0x7acea) : fauve bondissant, scène 27 — `boss_1/`

Cousin de la Chimère (`boss_4`, même charpente et mêmes réglages) qui se déplace par bonds : une marche (`+6` = 4 ou 6) est une
suite de sauts vers des points tirés près de la cible (tables 0x7c400 / 0x7c410, angle par 0x137e) dont la hauteur dépend de la
distance (`hop_height`, 0x7c430 : la boucle relit le mot haut de D3 laissé par 0x137e, les registres sont donc reproduits). Les
décisions sont des tirages dans des tables de la ROM qui désignent l'adresse de l'état à poser (`go` : 0x7c24a..0x7c372).

| `+5` | Fichier | Rôle |
|---|---|---|
| 0 | `moves` | arrivée en cinématique : huit sauts d'un bord à l'autre (table de vitesses 0x7af94), chute du haut de l'écran |
| 2 | `moves` | attente, demi-tour, marche par bonds, grand bond (vers la cible ou le milieu), bond en arrière et glissade |
| 4 | `moves` | quatre coups (enchaînés `+0xae` fois si le coup porte), saisie (0xfc44/0xfd64), bond en ciseaux avec un double ENEMY 0x1a |
| 6 à 10 | `moves` | attente sans joueur |
| 12 | `moves` | joueur saisi (0xfda2) : morsures de 4 PV (jamais en dessous de 1), puis relâché (0xfd8c) |
| 14, état 4 | `hit` | réactions (table 0x7bbbe, seule 0x7bc6a est propre : tremblement), recul, à terre et bond, mort, retour au combat 0x7bece (aussi appelé par les réactions communes, table 0x3e000) |

### Type 3 (0x7f41a) : créature volante, scène 11 — `boss_3/`

Le boss vole (`+0xc` = altitude) : il plane, s'approche, pique, et choisit ses attaques dans des tables de 32 octets par
difficulté lues en données (0x80e62 cible proche, 0x81062 à distance, 0x81262 en vol, 0x814f8 après une attaque). `+0xa2` est le
mode vol / sol, recalculé d'après les seizièmes de PV (0x81722, table 0x81712). Il s'enregistre comme obstacle (`A5-0x5fb2`) : son
corps repousse joueurs et objets selon `+0xa4` (remis à zéro à chaque image) ; les deux formes 0x818e6 (devant) et 0x819e2 (de côté)
sont des natives (`body.rs`) appelées par `boss_repousse` (0x81aa4) et par les attaques (cris, vol de côté). Les sous-routines qui
« sautent » dans 0x7f5ae (`move.l #0x020a0000,(4,A0) ; rts`) rendent vrai à leur appelant : sans cible, l'état d'attente est posé
puis souvent écrasé par l'appelant, comme dans l'original.

| `+5` | Fichier | Rôle |
|---|---|---|
| 0 | `fight` | arrivée (variante 0) : descente du haut de l'écran en cinématique, puis combat dans la même image |
| 2 | `fight` | vol stationnaire, demi-tour, approche (`+6` = 4, 8), ligne droite (6), montée (10), altitude tenue (12), descente (14), vol bas (16) |
| 4 | `attacks` | seize attaques : griffes, bond et plongée, cris qui repoussent, rase-mottes, ruée, vol de côté, enlèvement d'un joueur |
| 6, 8 | `fight` | ronde vers des points de passage par quart d'écran (0x81876), saut d'envol |
| 10, 12, 14 | `fight` | attente sans joueur, joueur emporté et lâché de haut, riposte (0xffda) |
| état 4 | `hit` | réactions communes (table 0x80b0a), recul en vol, à terre et riposte (0x80cf6), mort (0x300 en `A5-0x557c`) |

La riposte 0x81ae0 (appelée par la réaction commune 0x3e22e) est aussi une native.

### Type 9 (0x8a75c) : engin de guerre roulant, scène 2 — `boss_9/`

Pas d'IA de combat : l'engin roule d'un bord à l'autre de l'écran (caméra `A5+0x556`), s'arrête à une abscisse tirée (0x8bcd2),
et donne des ordres à ses parties (ESHL 0xd, 0xe créées à l'initialisation : `+0xa9` bit 0 tir, bits 3 et 4 bombardement ; les
parties les effacent). Son équipage (ENEMY 0x13 : conducteur `+0xac`, quatre servants `+0xae..+0xb4`) est recompté chaque image
(`+0xb6` bits, `+0xb7` nombre × 4) et remplacé au demi-tour ; la vitesse dépend de l'équipage (tables 0x8c092 / 0x8c0ba). Les coups
ne font que le secouer et cumulent des avaries par huitièmes de PV (`+0xa8`, 0x8b9ea) qui interdisent certains tirs. Il attend
d'avoir 10 places ESHL et 5 ENEMY libres pour se créer. Toutes les tables par difficulté sont lues en données.

| État, `+5` | Fichier | Rôle |
|---|---|---|
| 2, 0 | `moves` | équipage, dialogue (0xc8312), traversée de l'écran, marche arrière |
| 2, 2 et 4 | `moves` | passages (`+6` = 0), approche de l'arrêt (2), arrêts : tir (4, 16), tir en se déplaçant (6), bombardements (8, 20), bombardement puis tir (10, 22), pauses (12, 14, 24, 26 : l'original décompte l'octet fort du mot, elles finissent aussitôt), salves (18) |
| 2, 6 | `moves` | attente sans joueur |
| 4, 0 | `hit` | coup : secousse, recul, reprise de l'état mémorisé `+0x8a` |
| 4, 2 | `hit` | détruit (0x900 en `A5-0x557c`) : sortie de l'écran, l'équipage saute, dialogue à la caméra 0x500, chute, libération |

Le code 0x8b258..0x8b326 n'est référencé par aucune table (code mort, non réécrit).

### Types 11 et 12 (0x8d59e) : salamandres, scènes 42 et 40 — `boss_11/`

Le boss est la tête d'un serpent de douze anneaux. Les anneaux sont des objets ESHL de type 5 (`eshl_05`, 0x9e458 : zone de
contact qui recopie la visibilité de la tête et choisit son cadre de sprite) créés à l'initialisation par 0x9e426 (`+0x2f` = rang
0 à 11, `+0x46` = la tête). Chaque image, `chain::update` replace tous les anneaux d'après les tables de la ROM (lignes de 32
octets de 0x1069f8 choisies par la pose `+0xaa`, qui copie l'image courante `+0x3e` de l'animation ; trajectoires de
0x106cb8), puis recopie la position de l'anneau suivi (rang `+0xa8`) dans la sauvegarde `+0xa2..+0xa6` que la tête reprend
au début de l'image suivante. Les poses (`pose_0` à `pose_6`) règlent l'orientation `+0x42` et l'inclinaison `+0x43` de chaque
anneau ; un anneau manquant (`find_prev`, `find_next`) met le boss en état 0x04060000 (mort). L'initialisation se prolonge dans
l'état actif (pas de `rts`).

| `+5` | Fichier | Rôle |
|---|---|---|
| 0 | `behave` | arrivée à droite de l'écran, chute, rebond, première marche |
| 2, 8, 10, 12 | `behave` | poursuite (pas, cap, animation de marche), puis tests dans l'ordre : demi-tour, coup devant, attaque lointaine, attaque proche, hors écran |
| 4 | `attacks` | huit attaques (`+6` = 0 à 14) : coup de queue, crachat (11 : trois tirs, 12 : deux), bond, élan, charge sur un joueur, approche suivant un tracé, crachat chercheur (12 seulement), grand bond |
| 6 | `behave` | attente sans joueur |
| état 4 | `hit` | six routines de réaction (table de longs 0x8e3dc), étourdi, relevé, mort |

Particularités de l'original reproduites : l'initialisation tombe dans l'état actif ; `0x8d62e` compare à `(8,A2)` au lieu de `(8,A0)` ;
la lecture de la table de tracé réutilise D0w=0 (`0x8e178`) ; `0x8e0b6` charge trois fois la même table. La lecture
`move.w (-0x7f92,A0)` de l'étourdissement lit A0-0x7f92 (et non A5). Fuzz : 287 000 appels sans écart (couverture 2 060 - 64
instructions ; restent la sous-routine 0x8ed7e, inutilisée car toutes les lignes de la ROM ont `+4` = 1, des signes d'écart rares et le
second débordement du recadrage). Les registres de sortie A2, A4 sont reproduits (le 68000 les relit après le handler) ;
D3, D4 et D5 restent différents (non lus).

Un défaut de `eshl_05` (families/eshl.rs) a été corrigé en passant : pour les images impaires, l'original relit l'octet d'index 0
de la table 0x9e6bc (D0 remis à zéro avant la lecture) et non celui de la ligne.

### Type 0 (0x794ae) : créature qui se dédouble, scène 20 — `boss_0/`

Même charpente que le fauve bondissant (`boss_1/`) : états en longs, tables de décision de la ROM, bibliothèque d'IA 0x103d6.
Deux variantes (`+3`) : la moitié principale (0) et la seconde moitié (1), créée par la première quand ses PV passent sous la moitié
(`transform`, 0x7aaf4) et liée à elle par `+0xba` ; elles ne meurent qu'ensemble (la première à tomber ne fait que 0x19ad6, la
dernière prévient la scène). Jeux d'animations 0x171a52 (corps), 0x171a7e (entrées en scène, attaques), 0x171a94 (coups reçus).
`mod.rs` (création, aiguillage, décisions posées d'un coup et leurs tables), `moves.rs`, `hit.rs`.

| `+5` | Rôle |
|---|---|
| 0 | entrées en scène : arrivée par un bord (`+6` = 0), transformation avec création de la seconde moitié (2), arrivée de la seconde moitié (4) |
| 2 | combat : attente (0), demi-tour (2), course (4 et 6), charge (8) |
| 4 | sept attaques (`+6` = 0 à 12) : trois coups, bond sur la cible, saut en arc, trois coups enchaînés, projectile (ESHL 0x43) |
| 6 à 10 | attente tant qu'aucun joueur n'est en vie |
| état 4 | réactions (table 0x7a396, la seule routine propre est 0x7a442, tremblement), recul, à terre, relevé (0x7a5ac), mort |

Les tirages de décision lisent les tables d'octets de la ROM (0x7a8e8, 0x7a932, 0x7a9ae.., 0x7aa88..) puis un saut par table de mots
(0x7a8f8, 0x7a942, 0x7a9de, 0x7aac8) : `go` les exécute par adresse. Fuzz `fuzz_boss` (`DDSOM_ENTREE=794ae`) : 1 259 instructions
sur 1 306 vues (les entrées de tables qu'aucun octet de la ROM ne sélectionne dans les tirages joués), sans écart du gestionnaire.

### Type 16 (0x9325e) : boss de la scène 14 — `boss_16/`

Gabarit ancien comme Ezerhorden : `+5`, `+6`, `+7` posés champ par champ, trois jeux d'animations (0x17efe8 mouvements, 0x17f002
attaques, 0x17f046 coups reçus) lancés par `anim_a`, `anim_b`, `anim_c` qui mémorisent le jeu et le numéro (`+0xa0`, `+0xa1`). Variantes
0 et 1 (la caméra choisit), les autres vont droit au combat. Le réveil est une pose accroupie qui se retourne vers le joueur le
plus proche (`proximity`, 0x9351a, dont le test vertical compare la position du joueur avec elle-même : toujours nul).

| `+5` | Rôle |
|---|---|
| 0 | réveil (variante 0 : tourne vers le joueur ; variante 1 : script de palette 0x10298 au rythme de l'animation) |
| 2 | combat : attente, demi-tour, marche vers la cible (`+6` = 4 et 8), bond (6) ; les boîtes de portée choisissent une attaque (table 0x94a24) |
| 4 | huit attaques (`+6` = 0 à 14) : ramper et bondir en saisissant, charge, flamme, deux souffles, rugissement, double saut |
| 6, 8 | course vers un point de l'écran (table 0x951b0) puis saut sur place ; le groupe 8 y mène après une attaque |
| 10 | pause |
| 12 | joueur saisi (`+0x50`, `+0x188` levé) : morsures de 0x10 PV puis de 1 PV, relâché à 1 PV |
| état 4 | réactions propres (0x9416e recul au sol, 0x941f8 projection, 0x942da gerbes d'effets, 0x94154 mort forcée), recul, à terre, mort (deux séquences, 0x946fc et 0x947c6 pour le code de coup 0x60) |

Pièges : les routines de réaction sont des sous-états à part entière (la gerbe 0x942da enchaîne sur 0x9416e à ses étapes 4 et 6) ; la
seconde table d'attaques (0x94c24) est écrasée avant d'être lue (deux `lea` à la suite en 0x949f0), donc jamais utilisée ; A1 doit
désigner la proie quand `0xedd96` (qui sauve A1 sur sa pile privée) est appelé. Fuzz : 1 089 instructions sur 1 109 vues.

### Types 2 et 22 (0x7c5a8, 0x9aeaa) : boss de la scène 10 et son partenaire — `boss_22/`

Le type 22 est une copie modifiée du type 2 (74 % d'instructions identiques) : un seul module `boss_22/` sert les deux, le type
étant lu dans `+2` (`two`) et les jeux d'animations, tables de la ROM et quelques constantes réunis dans `Kit` (`TWO_KIT`, `PART_KIT`).
Seconde forme (`+0xb5`) après un rugissement (`+0xbf`), états posés d'un coup, mêmes groupes pour les deux types :

| `+5` | Rôle |
|---|---|
| 0 | arrivée (type 22 : tombe du haut de l'écran ; type 2, `two.rs` : grand bond par la droite) |
| 2 | combat : attente (0 et 8), demi-tour (2), élan (4), bond vers la cible (6) |
| 4 | tir (0, 6), souffle (2, 8), morsure (4), attaque de la seconde forme, charge, piétinement (14), plongeon (20), plongeon vertical (22) ; le type 2 y ajoute `+6` = 10 (bond-écrasement, 16 étapes) et 12 (rotation et rugissement), `two.rs` |
| 6, 8 | course vers un point de l'écran, pause, grands bonds (de face et de côté) |
| 10, 12, 14 | pause, joueur saisi (morsures de 4 PV), transformation en seconde forme |
| état 4 | réactions communes (`reactions.rs`), recul, à terre, mort : le type 22 passe le relais à son hôte (`+0x42`), la dernière moitié à tomber annonce la scène (type 2) |

Le type 2 crée le contrôleur de scène SET 2 (variante 0x3e) quand son compte à rebours `+0xbc` s'écoule, et un type 22 à la
création (3 joueurs, ou 2 joueurs et difficulté `A5+0x7f92` d'au moins 0x17) lié par `+0x42`/`+0xac`. Écarts du type 2 avec le 22 :
création et arrivée, `+0xbb` (0x46/0x50 permutés), breath rendant vulnérable, vitesse verticale du grand bond (0x500 au lieu de 0x700),
`next_attack` qui saute les joueurs absents au lieu de s'arrêter, rangées de chances de riposte de 4 octets, partenaire testé par `+0xac`,
`+0xbf` remis à zéro au relevé, `(A0)` non réécrit à la transformation. Les trois routines de poussière 0x7f2c4, 0x7f328 et 0x7f3a0
sont réécrites en Rust (`dust_line`, `dust_trail`), de même que les tests de portée 0x7f19c et 0x7f16c. Quirks conservés :
`andi.w #3,(0x6e,A5)` réduit le compteur de la RAM de travail ; la vitesse de projection de la mort du 22 est la position de la
caméra (+0xc0), 0x180 étant écrasé ; `subq.b #1,(0xa0)` agit sur l'octet haut du compteur (déjà nul). Fuzz : 2 035 instructions sur 2 140
(type 2) et 1 474 sur 1 522 (type 22), sans écart du gestionnaire.

## 3. Validation

* `scenarios/boss/boss17_scene13.txt` : VALIDE sur 12 000 images (9 259 appels du boss, qui reste en vie : variante 0, PV intacts).
* `scenarios/boss/boss5_scene58.txt`, `boss21_scene44.txt`, `boss19_scene32.txt`, `boss15_scene46.txt`, `boss18_scene26.txt`, `boss8_scene64.txt`, `boss11_scene42.txt`, `boss12_scene40.txt` : VALIDES sur 12 000 images (2 460, 9 266, 9 044, 8 963, 7 125, 8 305, 9 123 et 9 139 appels ;
  `boss18_scene26` est tronqué à l'image 10 900 par `gen_boss.py`, voir `scenarios/ko/README.md`).
  Leurs variantes `_mort` (PV ramenés à 1 après 1 500 images, `--weaken` d'`autobot`) tuent le boss et exercent sa mort et la fin
  de scène : VALIDES sur 12 000 images, tronquées par `gen_boss.py` avant les boutons qui déclenchent l'avance rapide.
* `scenarios/boss/` : un scénario par boss (entrée directe dans la scène par `poke8 ff8034`, bot `autobot`, PV du boss
  intacts), générés par `tools/gen_boss.py` ; `tools/validate_all.sh` les lance sur 12 000 images.
* `scenarios/ko/boss13_dragon.txt` : le dragon diverge dès l'image 3100 (fraction de X du joueur 1), avec ou sans son natif
  (`DDSOM_SANS=boss_13`) ; chaque native seule, et toutes les paires, sont VALIDES ; toutes natives ensemble divergent. Hypothèse :
  la machine d'origine B finit en retard certaines images de cette scène chargée (tirs, sprites) alors que A, plus rapide,
  les finit à temps. Contrôle appel par appel (`DDSOM_CHECK=1 DDSOM_CHECK_ALL=1`) : VALIDE.
* Types 0, 16, 2 et 22 (jamais créés par les parcours) : `scenarios/boss/boss0_poke{,_mort}.txt`, `boss16_poke{,_mort}.txt`,
  `boss2_poke.txt`, `boss2_poke3p{,_mort}.txt`. `tools/gen_boss.py` ajoute le 7e élément `type:variante:image[:joueurs]` des spécifications :
  `autobot --spawn boss:type:variante:image` écrit en RAM (`poke`) un objet BOSS neuf à 0x60 pixels à droite du joueur 1 comme le fait
  `creatures spawn`, et le bot le combat (variante `_mort` : PV ramenés à 1, séquences de mort et de fin de scène). Les scénarios à
  trois joueurs exercent aussi le type 22 (créé par le type 2).
* `examples/boss_trace.rs <roms> <eeprom> <scénario> <images>` : trace les changements d'état et de PV de chaque boss (quelles phases,
  dont la mort, un scénario exerce).
* `examples/fuzz_boss.rs <roms> <eeprom> <scénario> <chauffe> <tours>` (modèle `fuzz_enemy.rs`) : objet neuf dans le pool BOSS,
  états tirés dans les tables de sauts (`DDSOM_ETATS=1`), meneur de la caméra, joueurs, caméra et drapeaux perturbés à chaque pas,
  chaque appel comparé à l'original ; `DDSOM_COUVERTURE=fichier` écrit les instructions vues (`DDSOM_GRAINE`, `DDSOM_ENTREE`, `DDSOM_PAS`).
  Exemple : `DDSOM_ETATS=1 fuzz_boss $DDSOM_ROMS $DDSOM_EEPROM scenarios/bot_char0.txt 4500 300`.
  `DDSOM_SEULS=boss_` n'affiche que les écarts des routines dont le nom commence ainsi (les écarts connus du drapeau X des
  natives d'IA ne masquent plus les autres) ; chaque écart nomme la routine fautive.
* Types 1, 3 et 9 : `covb_long_27`, `scene_11` et `covb_long_11`, `covb_long3p_02` VALIDES ; fuzz (`boss5_scene58`, 3 000 images
  de chauffe, 2 × 300 tours par type, `DDSOM_ETATS=1`) sans écart des gestionnaires. Instructions d'origine jamais vues (scénarios
  et fuzz) : 78 sur 1 370 (type 1), 76 sur 1 732 (type 3), 29 sur 1 384 (type 9), dont les données mal désassemblées ; le reste
  est relu à la main.


## Sous-routines communes appelées par le code d'origine des boss (`boss_common.rs`)

* `0x81aa4` (`boss_repousse`) : repousse l'objet A0 (ennemi, effet) hors du corps du boss enregistré en `A5-0x5fb2`
  (mot négatif = adresse), si ce boss est actif, de classe 3 et a une forme `+0xa4` non nulle ; la routine de la forme
  (table 0x81ad4 : 1 = 0x818e6, 2 = 0x819e2, natives de `boss_3/body.rs`) déplace l'objet en X. Rend D0 = 1 (Z = 0) si
  l'objet a été déplacé, A0 intact, A1 = A6 = l'objet. Appelée chaque image par les ennemis (elfe, gnoll...).
* `0x833b6` (`boss_oscillation`) : oscillation verticale (`+0xc` += long de la table 0x833e4 par `+0x33 & 0x7f`,
  négatif quand le compte à rebours `+0xa8` est sous 36 ; période 72).
* `0x87048` (`boss_seuil_pv`) : selon `A5+0x5a` (0 : rien, 3 : toujours) et la forme `+0xad` (2 ou 3), pose `+0x43`
  ou, si la moitié des PV `+0x62` dépasse 72, retient ce seuil en `+0xb6` et pose `+0x42`.

Validées par les scénarios (`boss_repousse` dans presque tous les scénarios d'ennemis, `boss_oscillation` dans
`enemy/trog_scene104`, `boss_seuil_pv` dans les scénarios de boss) ; les deux formes du corps sont exercées par le fuzz du boss 3.
