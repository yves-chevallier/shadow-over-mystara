# Déroulement des scènes (W2, W3, F6)

Code : `port/ddsom-game/src/scene/` (`mod.rs` : contexte et variables communes, `script.rs` : lecteur de
script d'apparition, `entry.rs` : phase 8, `intermission.rs` : phase 24, `exit.rs` : phase 16). Les
tables restent dans la ROM et sont lues à l'exécution. `A5` = 0xFF8000. Les rôles marqués (h) sont des
hypothèses.

## 1. Vue d'ensemble

Une partie enchaîne des *scènes* (`A5+0x34`, 1 à 114 : niveaux, boss, boutiques, démonstrations de
l'attract). La boucle de partie (`docs/boucle.md`) appelle chaque image le gestionnaire de la phase
(`A5+0x2c`, pas de 4) ; chaque phase fait un pas par image. Les phases qui orchestrent les scènes :

| Phase | Adresse | Rôle | Module |
|---|---|---|---|
| 8 | `0x20c38` | entrée de scène : initialisation (étape 0), puis séquence d'ouverture de la scène (étape 2) | `entry.rs` |
| 12 | `0x1c6a0` | le jeu ; le lecteur de script d'apparition (`0xf2e5c`) tourne à chaque image | `script.rs`, `game.rs` |
| 16 | `0xf3142` | sortie de scène : écrans de fin, choix de la scène suivante | `exit.rs` |
| 24 | `0x21c2a` | séquences scriptées (arrivée d'un boss, défilement forcé) | `intermission.rs` |
| 28 | `0xfb292` | fin du jeu (générique) : **non réécrite** (états de 0xfb2aa à 0x100000) | – |

La phase 4 (sélection du personnage, F3/F5) et les phases 0, 12, 20 (W1) sont décrites ailleurs
(`docs/boucle.md`, `docs/familles/set_c.md`).

Toutes les séquences sont des machines à états en RAM : l'étape (`A5+0x2d`) et la sous-étape
(`A5+0x2e`, pas de 2 en phase 8 et 24, de 4 en phase 16) disent où l'on en est ; des minuteries et des
pointeurs d'objets sont gardés dans des variables temporaires (mots à `A5-0x65d6…`, adresse notée par
son déplacement 16 bits dans le code). Une phase ne cède jamais la main à l'ordonnanceur, **sauf** les
états finaux de la phase 16 (voir §5).

## 2. Lecteur de script d'apparition (`0xf2e5c`)

Un script par scène (table de longs `0xf2f62`) fait entrer ennemis, décors et objets au fil du
défilement. Entrées triées par abscisse de caméra croissante (`A5+0x506`, ou `A5+0x556` si `A5+0x36`) :

| Entrée | Taille | Contenu |
|---|---|---|
| apparition | 0x12 | `+0` abscisse de déclenchement (mot signé), `+2` décalage X, `+4` Z, `+6` Y, `+8` rang maximal, `+9` nombre de joueurs minimal, `+0xb` classe (octet pair, index dans la table de mots `0xf2f38` des allocateurs : ENEMY, SET, EFFCT, BOX, SOBJ, BOSS, ITEM, ESHL...), `+0xc` → `+0x2c`, `+0xd` type, `+0xe` variante, `+0xf` → `+0x2f`, `+0x10` → mot `+0x42` de l'objet |
| appel | 10 | `+0` abscisse, `+2` = 0xffff, `+4` adresse d'une routine (long), `+8` son argument D0 (mot) |

Un mot `+0` égal à 0xffff termine le script. L'objet créé reçoit `X = abscisse + décalage`. Une
apparition dont le rang (`A5+0x7f93`) dépasse `+8`, ou qui exige plus de joueurs que `A5+0x35`, est
sautée ; si le pool de la classe est plein la lecture s'arrête et reprend à la même entrée à l'image
suivante. État (octet `A5-0x55cc`) : 0 charge le script, 2 lit, 4 puis 6 : fini. Curseur : long
`A5-0x55d0`. `0xf2e56` remet l'état à 0 (à chaque entrée de scène).

C'est le « spawner des vagues d'ennemis » que l'atlas ne localisait pas (§8).

## 3. Phase 8 : entrée de scène (`0x20c38`)

**Étape 0** (`A5+0x2d` = 0) : variables remises à zéro, octets de palette de la scène (`A5+0x48/0x49`
d'après les tables `0x21b38/0x21bb0`), appels d'initialisation (palettes `0x1a296`, caméras, pools
`0x2062`, interface joueurs `0x1507a`...), joueurs remis à neuf, `A5+0x44` = 1 dans les scènes 7 à 9
(cinématique), `A5+0x35` = nombre de joueurs vivants, puis placement des joueurs d'après les tables
`0x2123c` (4 couples X, Y par scène, consommés dans l'ordre des joueurs en jeu) et `0x2196c`.

**Étape 2** : à chaque image la *séquence d'ouverture* de la scène, puis `0xf2e5c` (script), puis le
saut de niveau de débogage (`game::debug_scene_skip`, inactif en production). La séquence passe la
phase à 12 (`end_scene`) quand elle est finie. Elle verrouille la scène (`A5+0x33`) et les joueurs
(`A5+0x4a`) le temps de l'ouverture. Séquences (indice par la table `0x20e6e` du code d'origine,
remplacée par `entry::sequence`) :

| Scènes | Séquence | Contenu |
|---|---|---|
| 0, 3, 6, 12, 13, 15-19, 21-23, 29-31, 37, 46, 48-53, 55-57, 60-63, 68-89 | `quick` | fondu d'ouverture, musique, phase 12 aussitôt (boutiques 69-82 comprises : leur logique est dans les objets) |
| 25, 106-108 | `quick_with_music` | comme `quick`, musique une seule fois (`A5+0x67`) |
| 90-104 | `white_start` | fondu depuis le blanc, objet système 6 (variante 23) : démonstrations de l'attract |
| 109, 112-114 / 110 | `quick_with_sobj` / `_flag` | fondu, objet système, musique (110 lève `A5+0x59`) |
| 1 | `stage_1` | carton de titre, dialogue 0, trois ennemis (type 20) entrent par la droite |
| 2, 14, 4 | `stage_2`, `title_card`, `scene_4` | carton de titre, effets de scène à intervalles |
| 5, 20 / 39, 41, 43, 45 | `wait_players_10/6` | attend que les joueurs atteignent l'état 10 / 6 (la 41 lance `0xdfa4a`) |
| 7, 9 / 8 / 10 / 11 | `boss_cave`, `scene_8`, `scene_10`, `scene_11` | effets, boss de type 13, dialogue 95, défilement forcé |
| 24, 26, 27, 28 | `scene_24/26/27/28` | dialogues (31-32, 34, 37, 140) avant le boss |
| 32, 40, 42, 44, 111 | `scene_32`, `boss_gate`, `scene_44` | boss d'ouverture (types 0x13, 12, 11, 0x11) |
| 33, 36 / 34 / 35 / 59 / 38 / 67 | `plain_fade` ... | fondu simple, carton, verrou d'entrées |
| 47 | `scene_47` | fondu à un niveau par pas (`A5+0x48f`) |
| 54, 58, 64, 65 | `scene_54/58/64/65` | boss ou effet d'ouverture, dialogues (43, 75, 77, 81) |
| 66 (dernier niveau) | `scene_66` | dialogues 82, 83, 84, défilement forcé, quatre effets 0x50 |
| 105 | `scene_105` | effet 0x24, dialogues 86, 87, 42, deux fins selon `A5+0x5c` |

Un *dialogue* est un objet SET créé par `0xc8312` (D0 = numéro) ; l'octet `+0x22` de l'objet passe à 1
quand il est terminé. Les séquences attendent cet octet.

## 4. Phase 24 (`0x21c2a`)

Deux séquences scriptées (étape 0 et 2) ; chaque image appelle ensuite `0x2aec`, la mise à jour du monde
et les sprites. Étape 0 : un boss (type 9) entre (`A5+0x4ff` 8 puis 12), message 4, la caméra arrive en
800, le boss s'avance, message 6, retour au jeu (phase 12, horloge `A5+0x38` = 0x04595900). Étape 2 :
transition imposée (`A5+0x51e` 76 puis 92), défilement de la caméra jusqu'à 2408, retour au jeu.

## 5. Phase 16 : sortie de scène (`0xf3142`)

Bloc d'état `A5-0x55ca` : `+0` état (0 : initialisation, 2 : séquence), `+1` numéro de séquence (deux
fois l'indice dans la table de mots `0xf31ea`, d'après la table d'octets `0xf3170` indexée par la
scène), `+2` état de la séquence (pas de 4), `+4` minuterie, `+0x10/0x12/0x16` objets (messages,
effets), `+0x14` scène suivante (séquence 21). 36 séquences, une par fin de scène ; ~70 % de leur code
est commun :

```
état 0       efface le monde (0xf7ee0), dessine l'image de fin (0xf7916)         Clear
état 4       attend la minuterie, fait sortir les joueurs (état 0x000c0000)      WalkWait
états suivants  écrans partagés (decompte, bonus, changement de zone)            Shared
dernier état prépare la scène suivante et cède la main                           Leave
```

Chaque image la séquence exécute l'état courant puis une fin d'image (`Update`) qui met le monde à jour,
sauf à l'état final. **États partagés** (restés en 68000) : `0xf590e` (décompte des points, sans
cession), `0xf5938` et `0xf6492` (écrans de bonus, avec cessions), `0xf792e`, `0xf7ecc` (changement
de zone : plusieurs cessions) ; quand une séquence atteint l'un d'eux, la native cède la main à la
séquence d'origine (`Ex::delegate` : `PC` ← début de la séquence, sans `rts`), qui continue exactement
au même endroit. **États finaux** (`Leave`) : `A5+0x42` ← 0, fondu (`0x168c`), joueurs et sprites remis
à zéro (`0x1de7c`), éventuellement la scène suivante (`A5+0x34`), `A5+0x2c` ← 0x08000000 (phase 8),
puis cession de quelques images à l'ordonnanceur (`0xa40`) ; la native s'arrête là en empilant
l'adresse de reprise d'origine, où d'autres natives (`RESUME_*`) finissent par `0xc0c` et `0xe0882`.
C'est la technique générale pour une routine qui cède : **découper aux points de cession** et rendre
chaque morceau accessible à l'adresse de reprise du code d'origine.

Séquences réécrites : 0 à 5, 7 à 19, 21 à 26, 28, 30, 32, 34, 35. Cédées à l'original : 6, 20, 27, 29,
31, 33 (aiguillage par saut), correspondant aux scènes 7, 35 et 111, 54, 59, 65, 67.

| Scènes | Séquence | Scène suivante |
|---|---|---|
| 0 et ~80 autres (défaut) | 0 | scène + 1 |
| 6 / 9 | 5 / 7 | scène + 1 (la 6 : 10 si le compteur d'images `A5+0x54` dépasse 0x1500) |
| 24 | 13 | 106 |
| 26 | 14 | 27 |
| 38 (choix de route) | 21 | 39, 41 ou 43 selon le dialogue 70 (table `0xf4a0c`) |
| 41 | 23 | 44 |
| 46 | 26 | 38 |
| 58 | 28 | 59 |
| 64 | 30 | 65 |
| 66 | 32 | 105 |
| 1, 2, 4, 5, 10, 11, 14, 20, 31-34, 40, 42, 44, 68, 105 | 1-4, 8-12, 15-19, 22, 25, 34, 35 | choisie par les états partagés (changement de zone) |

### Particularités fidèlement reproduites

* L'aiguillage d'origine appelle l'état par `jsr (A1)` puis compare `(2,A0)` à l'état final : quand l'état
  a lui-même mis le monde à jour (états finaux qui attendent la fin du fondu), `A0` pointe un objet et la
  comparaison lit l'octet `+2` de cet objet ; la fin d'image a alors lieu une seconde fois
  (`Ex::a0_clobbered`). Sans cela, la mise à jour du monde est omise et les deux machines divergent.
* L'état final de la phase 16 empile, sous l'adresse de reprise, l'adresse de retour de l'aiguillage
  (`entrée + 10`) : le mot de pile de la tâche principale (table des tâches, comparé par `validate`) en
  dépend, et la reprise finit par le code d'origine de l'aiguillage.
* La séquence 2 commence par décrémenter sa minuterie (mot qui part de 0) : un état d'origine commence au
  milieu d'une instruction pour le désassembleur linéaire ; vérifier chaque état par un désassemblage
  depuis son adresse exacte (`objdump --start-address`).

## 6. Couverture et validation

* `scenarios/scene/scene_NN.txt` (`tools/gen_scene.py`, bot réactif) : entrée directe dans la scène NN
  puis 6 500 images ; ils exercent la phase 8 de chaque séquence d'ouverture et le lecteur de script.
* `scenarios/scene_end/end_NN.txt` (`tools/gen_scene_end.py`) : entrée dans la scène NN, puis la phase 16
  forcée (`ff802c` ← 16) sans aucune touche ; une scène par séquence de sortie, 9 000 images. Ils ne sont
  lancés par `tools/validate_all.sh` que si `DDSOM_SCENES=1`.
* Les scénarios de `scenarios/boss/` et `bot*` atteignent des fins de scène naturelles (4 388 appels de
  la phase 16, 26 741 de la phase 8, 273 737 du lecteur de script sur les 8 scénarios historiques).
* Écarts connus, **indépendants de ce module** (identiques avec `DDSOM_SANS=scene_`) : voir
  `scenarios/ko/README.md` (scene_05, 07, 08, 11, 110 et end_01).
* Non exercé : la phase 24 (aucun scénario n'y entre), les séquences cédées à l'original
  (6, 20, 27, 29, 31, 33 : seul l'aiguillage est testé), la phase 28 (non réécrite) et la saisie du
  nom et le générique de fin.
