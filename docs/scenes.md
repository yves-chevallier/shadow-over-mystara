# Déroulement des scènes (W2, W3, F6)

Code : `port/ddsom-game/src/scene/` (`mod.rs` : contexte et variables communes, `script.rs` : lecteur de
script d'apparition, `entry.rs` : phase 8, `intermission.rs` : phase 24, `exit.rs` : phase 16, ses états partagés
`tally.rs`, `bonus.rs`, `characters.rs`, `shop.rs`, `blackout.rs`, les scripts de transition `transition/`, et leurs aides
`tiles.rs` (couche de caractères), `screen.rs`). Les
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
| 28 | `0xfb292` | fin du jeu (générique) | `modes/ending.rs`, voir `docs/modes.md` |

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
état 0       variables de transition (0xf7ee0) et bloc de la boutique (0xf7916) à zéro   Clear
état 4       attend la minuterie, fait sortir les joueurs (état 0x000c0000)             WalkWait
états suivants  états partagés (décompte, bonus, intermède, boutique, fondu au noir)    Shared
dernier état prépare la scène suivante et cède la main                                  Leave
```

Chaque image la séquence exécute l'état courant puis une fin d'image (`Update`) qui met le monde à jour,
sauf à l'état final. L'aiguillage d'origine appelle l'état par `jsr (A1)` (entrée + 4) puis compare
`(2,A0)` au dernier état (entrée + 10) ; les séquences 0, 6, 20, 29, 31 et 33 sautent à l'état (`jmp (A1)`)
et leurs états finissent eux-mêmes l'image (`Update::Nothing`, pas d'adresse de retour empilée). La
séquence 27 devient la 29 quand `A5+0x66` est posé (son aiguillage suit ce test, entrée + 10).

**États partagés** (modules `tally`, `bonus`, `characters`, `shop`, `blackout`) : appelés comme par
l'aiguillage (son adresse de retour, entrée + 10, empilée), ils cèdent souvent la main à l'ordonnanceur
(attentes `0xa40`, fondus `0x1460`/`0x144c`, `0xb64`) ; la suite est une native posée à l'adresse de
reprise d'origine, qui finit par revenir à l'aiguillage, où une native (`DISPATCH_TAILS`, entrée + 10)
fait la fin d'image. Les registres qui servent d'entrée implicite sont reproduits (D1 de l'aiguillage
relu comme `+0x2f` d'un effet par 0xb1a7a, A6 et D1 gardés à travers les cessions de l'intermède).

| Adresse | Module | Rôle | Sous-états (octet `+3`, pas de 2) |
|---|---|---|---|
| `0xf590e` | `tally` | décompte de fin de niveau | 0 fondu (tâche 0x14de) puis bandeau « STAGE CLEAR » + nom dessiné hors champ (`0x910020`) ; 2 le bandeau glisse depuis la colonne 48 ; 4 temps restant (ou « TIME OVER » et prime du trésor en grands chiffres) ; 8 prime de temps : de 5:00 au temps restant par pas de dix secondes, prime du pas en table `0xf6ba0` ; 10 prime des secondes (unités × taux BCD `0xf5b06`) ; 12 total ; 14 total par joueur ; 16-18 trésor (`+0x18a`, prime `0x19c0e`, objet SET 19 qui l'emporte) ; 20-22 meilleur nombre de coups (`+0x190`, prime `0xf6c1c`) ; 24 versement chiffre par chiffre (0x19ae6) ; 26 fin |
| `0xf5938` | `bonus` | écran de bonus (joueurs aux longs `+0x118`/`+0x11c` à -1) | 0 fondu au noir, caméras de côté ; 2 décor chargé, effets ; 4 cinq SET triés par adresse, le dernier mène le jeu, compte à rebours 90 s ; 6 compte à rebours (repart de 20 s quand le score du meneur change) ; 8 fondu ; 10 caméras rendues ; 12 fondu d'ouverture |
| `0xf6492` | `characters` | intermède propre à certains personnages (masque par scène `0xf672e`, bit = octet `+3` / 2) | 0 fondu, écran vidé ; 2 joueur suivant concerné ; 4 trois SET (types 5, 6, 7) créés dès qu'il y a de la place ; 6 ; 8 240 images ; 10 fin |
| `0xf792e` | `shop` | boutique (scène 69 + numéro, table `0xf7ad0`) ; bloc `A5-0x5560` (voir le module) | 0 ouverture, tâche des portraits `0xf7dfe` ; 2 attente ; 4 fondu, accueil (`0xf7b88`, correctif des scènes 109-114) ; 6 messages ; 8 messages de sortie, retour à la scène de retour ou boutique enchaînée |
| `0xf7ecc` | `blackout` | fondu au noir et préparation de la scène suivante | 0 noir, remises à zéro (la scène 105 garde une caméra) ; 2 palettes (table `0xf80c4`) ; 4 script de transition de la scène (table `0xf81fa`, variables `A5-0x5520`, module `transition/`, ci-dessous) |

**États finaux** (`Leave`) : `A5+0x42` ← 0, fondu (`0x168c`, sauf séquence 31), joueurs et sprites remis
à zéro (`0x1de7c`), éventuellement la scène suivante (`A5+0x34`), `A5+0x2c` ← 0x08000000 (phase 8),
puis cession de quelques images à l'ordonnanceur (`0xa40`) ; la native s'arrête là en empilant
l'adresse de reprise d'origine, où d'autres natives (`RESUME_*`) finissent par `0xc0c` et `0xe0882`.
C'est la technique générale pour une routine qui cède : **découper aux points de cession** et rendre
chaque morceau accessible à l'adresse de reprise du code d'origine.

Toutes les séquences et tous les scripts de transition (`0xf8200-0xfa300`) sont réécrits : la phase 16 n'exécute plus
aucune instruction 68000 (hors sous-programmes du moteur appelés par `call`).

| Scènes | Séquence | Scène suivante |
|---|---|---|
| 0 et ~80 autres (défaut) | 0 | scène + 1 |
| 7 | 6 | scène + 1 |
| 6 / 9 | 5 / 7 | scène + 1 (la 6 : 10 si le compteur d'images `A5+0x54` dépasse 0x1500) |
| 24 | 13 | 106 |
| 26 | 14 | 27 |
| 35, 111 | 20 | 33 |
| 38 (choix de route) | 21 | 39, 41 ou 43 selon le dialogue 70 (table `0xf4a0c`) |
| 41 | 23 | 44 |
| 46 | 26 | 38 |
| 54 | 27 | 55 (si `A5+0x66` : comme la 29) |
| 58 | 28 | 59 |
| 59 | 29 | scène + 1 |
| 64 | 30 | 65 |
| 65 | 31 | scène + 1 |
| 66 | 32 | 105 |
| 67 | 33 | 38 |
| 1, 2, 4, 5, 10, 11, 14, 20, 31-34, 40, 42, 44, 68, 105 | 1-4, 8-12, 15-19, 22, 25, 34, 35 | choisie par les états partagés (script de transition) |

### Particularités fidèlement reproduites

* L'aiguillage d'origine appelle l'état par `jsr (A1)` puis compare `(2,A0)` à l'état final : quand l'état
  a lui-même mis le monde à jour (états finaux qui attendent la fin du fondu), `A0` pointe un objet et la
  comparaison lit l'octet `+2` de cet objet ; la fin d'image a alors lieu une seconde fois
  (`Ex::a0_clobbered`). Sans cela, la mise à jour du monde est omise et les deux machines divergent.
* L'état final de la phase 16 empile, sous l'adresse de reprise, l'adresse de retour de l'aiguillage
  (`entrée + 10`) : le mot de pile de la tâche principale (table des tâches, comparé par `validate`) en
  dépend ; la reprise finit par la native de fin d'aiguillage (`DISPATCH_TAILS`).
* La séquence 2 commence par décrémenter sa minuterie (mot qui part de 0) : un état d'origine commence au
  milieu d'une instruction pour le désassembleur linéaire ; vérifier chaque état par un désassemblage
  depuis son adresse exacte (`objdump --start-address`).

### Scripts de transition (`transition/`, table `0xf81fa`)

Dernier sous-état de `blackout` : chaque image, le script de la scène qui se termine (la 105 prend celui de la 69) fait
un pas de l'intermède entre deux niveaux (décors fixes, dialogues, objets système animés, défilements, fondus), puis
fixe la scène suivante (`A5+0x34`) et fait passer la sortie de scène à son état suivant (`0xfa0e6` : `+2` du bloc
désigné par A0, une image cédée). Variables `A5-0x5520` (A6) : `+0` état (pas de 2), `+1` sous-état, `+2` minuterie,
`+4` message ouvert (mot) ou scène choisie (octet), `+6` drapeau ou choix, `+8` signal échangé avec les objets système
du script (ils reçoivent l'adresse des variables en `+0x4c`). Un dialogue est un objet SET de `0xc8312` : `+0x22`
passe à 1 quand il est fini, `+0x23` est la réponse choisie (qui indexe une table d'octets de la ROM).

Presque chaque pas cède la main (`jsr 0xa40`, fondus `0x144c`/`0x1460`/`0x1ccc2`/`0x1ce30`/`0x1ce9e`, et les
sous-programmes du script `0xfa102`-`0xfa1c2` qui finissent par l'un d'eux) : comme pour les états partagés, chaque
adresse de reprise d'origine (instruction qui suit le `jsr`/`bsr`) est une native (137 en tout), le sous-programme qui
cède est appelé avec l'adresse de retour d'origine empilée (`bsr`, `Ret::At`) ou en appel terminal (`bra`,
`Ret::Caller` : retour à l'aiguillage de la sortie de scène), de sorte que la pile de la tâche est celle de l'original.
Une boucle de création qui échoue (pool plein) cède une image et reprend à son début.

| Scènes | Script | Contenu | Scène suivante |
|---|---|---|---|
| 1 | `0xf8286` | dialogue 96 | 2 |
| 2 | `0xf82d8` | dialogues 9 à 13, objet SET 8, plans 2 et 3 | réponse au 13 (table `0xf847c`) |
| 4, 10 | `0xf875e` | deux textes défilants (`A5+0x5c6`, table par région `A5+0x72`), dialogues 15, 92, 16, objets système 0 et 1, décor en huit blocs | 11 |
| 5, 14, 20 | `0xf847e` | dialogues 93 et 94, texte défilant ; un bouton abrège les attentes | 108 après la 5, sinon 24 |
| 11 | `0xf8cd8` | s'il reste du temps (`A5+0x38`) : objet système 2, dialogues 17 et 18 | 13 |
| 13 | `0xf8e92` | objet système 3 ; temps restant : objets SET 10 et 11 ; temps écoulé : fondu par composante, objet SET 9, fondu vers des palettes fixes ; dialogue 30 ; une question (136, 137 ou 138 selon les postures des joueurs et le temps) | réponse (tables `0xf92e4`, `0xf932e`, `0xf937a`), 14 sinon |
| 27 | `0xf943e` | aucun pas | choix gardé en `+4` (table `0xf9454`) |
| 31 | `0xf9516` | dialogue 141, durée restante et prime (table `0xf95ba` par dizaine de secondes) versée tranche par tranche à chaque joueur en jeu (`0x19ae6`) | 31 |
| 33 | `0xf9878` | objet système 0, dialogues 39 et 40 ; la réponse fixe `A5+0x59` | 34 |
| 32, 34, 35, 67 | `0xf99e4` | dialogues 59 à 62, fondu vers une couleur (`0x1ce30`) | inchangée (la sortie continue) |
| 38, 40, 42, 44 | `0xf9be2` | objets système 4 et 5, dialogues 97, 73, 98, 74 ; décor 540, ou 551 selon les postures des joueurs | 47 |
| 46 | `0xf9e4c` | palettes de la scène 7, objets SET 12 et 21, dialogue 69, la caméra 2 descend d'un demi-pixel par image | 46 |
| 69 (105) | `0xf9fbc` | fin du jeu : le plan 1 clignote 180 images, fondus, décor final, caméras placées | inchangée |
| autres | `0xfa0e6` | aucun pas | octet bas de D0, deux fois la scène (h : la boutique qui suit fixe la vraie scène) |

Particularités reproduites : la reprise du dialogue 40 (scène 33) retourne à la création de l'objet système de l'état 0
(`bra 0xf98be`) ; celle du dialogue 59 (scène 32) ne recharge pas D0 et rouvre le message 1 (D0 de l'attente) ; la
recherche du décor 551 (scène 38) continue après un joueur dans la posture 11 (`move.w` remet Z à 0 avant le `beq`) ;
la prime de la scène 31 est commune (le même long est décompté pour chaque joueur).

## 6. Couverture et validation

* `scenarios/scene/scene_NN.txt` (`tools/gen_scene.py`, bot réactif) : entrée directe dans la scène NN
  puis 6 500 images ; ils exercent la phase 8 de chaque séquence d'ouverture et le lecteur de script.
* `scenarios/scene_end/end_NN.txt` (`tools/gen_scene_end.py`) : entrée dans la scène NN, puis la phase 16
  forcée (`ff802c` ← 16) sans aucune touche ; une scène par séquence de sortie, 9 000 images. Ils ne sont
  lancés par `tools/validate_all.sh` que si `DDSOM_SCENES=1`.
* `scenarios/scene_end/trans_NN.txt` (`tools/gen_scene_end.py --transitions`) : les scripts de transition que les sorties
  forcées n'atteignent pas (leurs séquences attendent la mort d'un boss ou une minuterie jamais armée) ; la sortie est
  forcée directement à l'état de fondu au noir de la séquence 18 (bloc `A5-0x55ca` = état 2, séquence 0x24, étape 20), le
  script de la scène joue jusqu'au bout puis la scène suivante commence. `trans_13_tard` met l'horloge à zéro (branche
  « temps écoulé » de la scène 13). Les scripts 1, 31, 27, 33 et 69 sont exercés par `end_01`, `end_31`, `cov_tour5_c4`,
  `cov_tour6_c5` et `modes/ending`.
* Mesure (`examples/reste68k`, 66 scénarios : `scene_end/` dont `trans_*`, `cov_tour1` à `8`, `cov_service`,
  `modes/ending`) : 288 979 instructions 68000 dans les scripts de transition (`0xf8200-0xfa300`) avant, **0** après ;
  tout W3 (`0xf2e00-0xfa300`, `0xfb200-0x100000`) : 288 986 → 3 (l'aiguillage d'origine de la séquence 32, relancé une
  fois par `seq32_route`/`seq32_clear` dans `cov_tour3_c2`).
* Les scénarios de `scenarios/boss/` et `bot*` atteignent des fins de scène naturelles (4 388 appels de
  la phase 16, 26 741 de la phase 8, 273 737 du lecteur de script sur les 8 scénarios historiques).
* Les scénarios `scene_05`, `07`, `08`, `11`, `67`, `110` et `end_01`, longtemps en écart (`scenarios/ko/README.md`),
  sont VALIDES : leurs écarts venaient d'autres modules (harpie, fenêtres de dialogue, appel de fondu de la porte 29,
  registres du constructeur de sprites) et de comportements indéfinis de l'original (`docs/boucle.md` §9).
* Non exercé : la phase 24 (aucun scénario n'y entre), la phase 28 (réécrite : `docs/modes.md`), la saisie du nom et le
  générique de fin, et quelques reprises des états partagés (boutique enchaînée, retentatives de création d'objets de
  l'intermède, fin de l'écran de bonus de la scène 1).
