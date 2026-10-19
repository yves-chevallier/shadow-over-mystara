# BOSS (C04) : les boss (`families/boss_*.rs`)

Pool de 3 objets de 0xC0 octets (FFBA1E, compteur de places libres inutilisé), table de gestionnaires 0x39b12 (23
entrées, dispatch 0x39aec). Même cadre que les ennemis (`docs/familles/enemy.md` §2, `Cx`, `handlers!`, contrat A0/D7 :
`docs/familles-eshl-set-sobj.md`) : `+4` état (0 création, 2 actif, 4 touché, 6 libération), `+5`, `+6`, `+7` sous-états,
octets pairs qui indexent des tables de sauts. `boss_common.rs` réexporte `enemy_common` (champs, états, `alloc`,
`suspended`, `random`...) et ajoute les variables de scène des boss (`var::*`, `leader`, `scroll_x`).
Les noms de rôle (h) sont des hypothèses tirées du comportement et des noms de debug du jeu (atlas, annexe A).

## 1. Qui apparaît où

Relevé par `examples/boss_scan.rs` (saute de scène en scène sur la machine d'origine) et `examples/boss_seen.rs`
(rejoue un scénario et note la première image de chaque boss) :

| Type | Handler | Octets | Nom (h) | Scènes |
|---|---|---|---|---|
| 13 | 0x8f59c | 1 352 | dragon noir en vol | 7, 9 (visible aussi pendant 6 et 12 : l'objet survit au changement de scène) |
| 17 | 0x952b4 | 9 016 | Tel'Arin (h) | 13, 44 (variante 1) — réécrit |
| 6 | 0x8430e | 5 866 | Manticore | 24 — réécrit |
| 18 | 0x975ec | 7 822 | Lich (h) | 26 |
| 14 / 19 | 0x8fae4 | 6 732 | dragons vert / noir (h) | 32 (19, variante 0), 65-66 (19, variante 1) — réécrit |
| 12 / 11 | 0x8d59e | 8 190 | salamandres de feu / de glace | 40 / 42 |
| 21 | 0x9947a | 6 704 | compagnon de Tel'Arin (h) | 44 — réécrit |
| 15 | 0x91530 | 7 470 | dragon rouge | 46 |
| 5 | 0x8341c | 3 826 | Ezerhorden | 58 — réécrit |
| 8 | 0x87fc0 | 10 140 | DarkWarrior 2 | 64 |
| 10 | 0x8ca4a | 2 900 | Nagpa | 65, 66 — réécrit |

Les types 0 à 4 (Ogre, Displacer Beast, Man Scorpion, Harpy, Chimera) n'ont été vus dans aucune scène 1-68 ni dans les
scénarios `cov_*` : le jeu les joue comme ENEMY (types 4, 19, 20, 21, 22...). Les types 14 et 19, 15 et 20, 11 et 12 partagent un handler.

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

## 3. Validation

* `scenarios/boss/boss17_scene13.txt` : VALIDE sur 12 000 images (9 259 appels du boss, qui reste en vie : variante 0, PV intacts).
* `scenarios/boss/boss5_scene58.txt`, `boss21_scene44.txt`, `boss19_scene32.txt` : VALIDES sur 12 000 images (2 460, 9 266 et 9 044 appels).
  Leurs variantes `_mort` (PV ramenés à 1 après 1 500 images, `--weaken` d'`autobot`) tuent le boss mais divergent peu après sa mort,
  par des routines d'autres modules (décompte de fin de scène, `combat_update`, `game_phase12`) : `scenarios/ko/`, qui sont VALIDES jusqu'à l'image
  de l'écart.
* `scenarios/boss/` : un scénario par boss (entrée directe dans la scène par `poke8 ff8034`, bot `autobot`, PV du boss
  intacts), générés par `tools/gen_boss.py` ; `tools/validate_all.sh` les lance sur 12 000 images.
* `scenarios/ko/boss13_dragon.txt` : le dragon diverge dès l'image 3100 (fraction de X du joueur 1), avec ou sans son natif
  (`DDSOM_SANS=boss_13`) ; chaque native seule, et toutes les paires, sont VALIDES ; toutes natives ensemble divergent. Hypothèse :
  la machine d'origine B finit en retard certaines images de cette scène chargée (tirs, sprites) alors que A, plus rapide,
  les finit à temps. Contrôle appel par appel (`DDSOM_CHECK=1 DDSOM_CHECK_ALL=1`) : VALIDE.
* `examples/boss_trace.rs <roms> <eeprom> <scénario> <images>` : trace les changements d'état et de PV de chaque boss (quelles phases,
  dont la mort, un scénario exerce).
* `examples/fuzz_boss.rs <roms> <eeprom> <scénario> <chauffe> <tours>` (modèle `fuzz_enemy.rs`) : objet neuf dans le pool BOSS,
  états tirés dans les tables de sauts (`DDSOM_ETATS=1`), meneur de la caméra, joueurs, caméra et drapeaux perturbés à chaque pas,
  chaque appel comparé à l'original ; `DDSOM_COUVERTURE=fichier` écrit les instructions vues (`DDSOM_GRAINE`, `DDSOM_ENTREE`, `DDSOM_PAS`).
  Exemple : `DDSOM_ETATS=1 fuzz_boss $DDSOM_ROMS $DDSOM_EEPROM scenarios/bot_char0.txt 4500 300`.
