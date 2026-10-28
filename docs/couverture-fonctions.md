# Fonctions d'origine jamais exécutées par les scénarios

Mesure du 2026-10-27 sur `main` b30905d : les 272 scénarios de `tools/validate_all.sh` (avec `DDSOM_SCENES=1
DDSOM_COV=1`, chacun avec son nombre d'images de validation), rejoués par l'exemple `cov` (code d'origine seul,
interprété, 68000 ×16 comme la machine de référence de `validate`). Une fonction de `decomp/index.tsv` compte comme
exécutée si au moins une de ses instructions l'est. Les sous-systèmes sont les zones de [atlas.md](atlas.md) ;
« native » : la fonction est le point d'entrée d'une routine Rust (`examples/names`) ; « code-mort.md » : son adresse
figure dans [code-mort.md](code-mort.md).

**Pourquoi.** Le jeu sans 68000 n'exécute que du Rust : chaque fonction de cette liste est réécrite (en native ou
dans le corps d'une autre), mais aucun scénario ne l'a comparée au code d'origine. Un écart y passerait inaperçu,
y compris en jouant depuis les ROMs. Ce sont les cibles des fuzz différentiels par routine (sur le modèle de
`fuzz_porte`, `fuzz_familles`) et des scénarios ciblés.

**Limite.** Couverture par fonction : une fonction exécutée peut avoir des branches jamais prises (la couverture par
instruction est ventilée plus bas, « Branches des fonctions exécutées »). Les tailles d'`index.tsv` incluent parfois
des données mal désassemblées.

Reproduire : `cov <roms> <eeprom> <scénario> <images> <sortie.bin>` (exemple de `ddsom-game`) sur chaque scénario
(4 tâches : 45 min), `names > natives.txt`, puis `tools/couverture_fonctions.py decomp/index.tsv docs/atlas.md natives.txt
docs/code-mort.md <bitmaps>...`.

## Branches des fonctions exécutées

Même mesure, au niveau des instructions (`tools/couverture_branches.py`, zones de débogage et de test K4, M1, M2, M3
écartées) : pour chaque fonction exécutée, les instructions jamais exécutées et les sauts conditionnels (`bcc`,
`dbcc`) pris d'un seul côté (cible ou instruction suivante jamais exécutée ; approximation : un côté peut avoir été
atteint par un autre chemin). Une ligne par fonction, avec l'adresse et le côté manquant de chaque saut :
[couverture-branches.tsv](couverture-branches.tsv).

| Zone | Sous-système | Fonctions exécutées | Instructions | Jamais exécutées | Sauts pris d'un seul côté |
|---|---|---:|---:|---:|---:|
| S1 | Son : wrappers de commandes + FIFO | 347 | 9863 | 552 (6 %) | 472 |
| C04 | BOSS : boss | 308 | 4447 | 426 (10 %) | 101 |
| C10 | SET : décors/animations de palette | 116 | 2560 | 399 (16 %) | 52 |
| C1 | Crédits, pièces, start/continue | 22 | 1152 | 372 (32 %) | 29 |
| P5 | Code par personnage (6 × 2 variantes) | 53 | 1273 | 188 (15 %) | 48 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 49 | 1062 | 154 (15 %) | 23 |
| C12 | PEFF : effets du joueur | 68 | 1065 | 106 (10 %) | 19 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 14 | 643 | 104 (16 %) | 7 |
| C02 | ENEMY : ennemis | 179 | 2281 | 81 (4 %) | 24 |
| P4 | Noyau joueur (états, 26 actions génériques) | 57 | 1070 | 74 (7 %) | 16 |
| C08 | ESHL : tirs/attaques ennemis | 102 | 1600 | 73 (5 %) | 28 |
| C0c | ITEM : objets ramassables | 14 | 506 | 66 (13 %) | 20 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 9 | 525 | 62 (12 %) | 9 |
| C0e | EFFCT : effets | 161 | 2028 | 57 (3 %) | 29 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 1800 | 46 (3 %) | 18 |
| O2 | Pools d'objets et listes triées | 41 | 538 | 43 (8 %) | 11 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 64 | 1544 | 42 (3 %) | 15 |
| X2 | Dégâts, terrain, placement | 19 | 351 | 33 (9 %) | 9 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 17 | 363 | 28 (8 %) | 5 |
| B1 | Scrolling / caméras par scène | 41 | 1049 | 27 (3 %) | 3 |
| C06 | PSHL : tirs du joueur | 30 | 462 | 26 (6 %) | 15 |
| V2 | Texte, rectangles et blocs de tuiles | 26 | 650 | 19 (3 %) | 11 |
| W1 | Boucle de partie et phases 0/4/12/20 | 3 | 82 | 19 (23 %) | 1 |
| O3 | Aides communes aux types (animations par type) | 24 | 344 | 19 (6 %) | 4 |
| W3 | Scripts de scène (phases 16/28) | 82 | 749 | 16 (2 %) | 6 |
| V3 | Palettes et fondus | 38 | 565 | 15 (3 %) | 8 |
| U1 | RNG et files d'affichage (pop-ups) | 18 | 234 | 9 (4 %) | 5 |
| F5 | Curseur de sélection de personnage | 8 | 191 | 9 (5 %) | 4 |
| F1 | Tâche de boot, avertissements région | 5 | 106 | 7 (7 %) | 4 |
| O1 | Animation, tables de vitesse, physique, culling | 22 | 325 | 6 (2 %) | 0 |
| F4 | Exécuteur d'écrans fixes (logos) | 4 | 228 | 6 (3 %) | 1 |
| V1 | Primitives vidéo (attente, couches, effacements) | 13 | 99 | 4 (4 %) | 1 |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 2 | 54 | 4 (7 %) | 2 |
| C14 | SOBJ : objets système/UI | 24 | 223 | 4 (2 %) | 4 |
| F6 | Interpréteur de scripts de scène (démos) | 18 | 230 | 3 (1 %) | 2 |
| F7 | Écrans titre / copyright | 5 | 47 | 3 (6 %) | 1 |
| C0a | BOX : caisses/coffres (h) | 7 | 164 | 1 (1 %) | 1 |
| K3 | Entrées vers enregistrements joueur | 3 | 77 | 0 (0 %) | 0 |
| P1 | Initialisation des joueurs | 23 | 245 | 0 (0 %) | 0 |
| U2 | Petites routines / tables (rôle inconnu) | 15 | 150 | 0 (0 %) | 0 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 12 | 0 (0 %) | 0 |
| P3 | Comportement scénarisé des joueurs par scène | 3 | 41 | 0 (0 %) | 0 |
| W0 | Dispatch des 10 classes d'objets | 10 | 120 | 0 (0 %) | 0 |

### Les 50 fonctions les plus trouées

| Fonction | Zone | Native | Instructions | Jamais exécutées | Sauts pris d'un seul côté |
|---|---|---|---:|---:|---:|
| `0xc8430` | C10 |  | 272 | 138 | 3 |
| `0x148be` | C1 | CREDIT_TEXT | 175 | 128 | 4 |
| `0x46044` | P5 |  | 82 | 70 | 4 |
| `0xe05ec` | C1 |  | 97 | 69 | 5 |
| `0x13c2c` | C1 |  | 258 | 54 | 2 |
| `0x180c8` | F3 |  | 106 | 52 | 8 |
| `0x1ea6c` | V4 |  | 62 | 48 | 4 |
| `0x8ec02` | C04 |  | 302 | 46 | 7 |
| `0x1ec4e` | V4 |  | 44 | 41 | 1 |
| `0xdbad8` | C0c |  | 119 | 41 | 8 |
| `0xca524` | C10 |  | 45 | 39 | 1 |
| `0x143da` | C1 | credits_lire_monnayeurs | 86 | 38 | 2 |
| `0xe2430` | S1 | son_selon_version | 35 | 33 | 1 |
| `0xedf80` | S1 |  | 35 | 32 | 1 |
| `0xf232e` | P6 | enemy_reacts_to_player_hit | 110 | 29 | 4 |
| `0x376ee` | P4 |  | 33 | 28 | 2 |
| `0x936ee` | C04 |  | 77 | 28 | 2 |
| `0xf1b22` | P6 |  | 56 | 28 | 2 |
| `0x0bad6` | E1 | butin_special | 298 | 27 | 6 |
| `0x14546` | C1 |  | 34 | 27 | 2 |
| `0x1ef6c` | O2 | list_reinsert_foreground | 93 | 27 | 3 |
| `0x4f9be` | C12 |  | 76 | 26 | 2 |
| `0xa1ba6` | C08 |  | 58 | 26 | 4 |
| `0x158c6` | F3 |  | 44 | 24 | 3 |
| `0x7f9fe` | C04 |  | 30 | 23 | 1 |
| `0x9b30a` | C04 |  | 30 | 23 | 1 |
| `0x9b384` | C04 |  | 30 | 23 | 1 |
| `0x77032` | C02 |  | 46 | 21 | 3 |
| `0x309e0` | B1 |  | 152 | 20 | 0 |
| `0x47390` | P5 |  | 85 | 20 | 10 |
| `0x4d7ac` | C12 |  | 54 | 20 | 1 |
| `0xcbb3c` | C10 |  | 22 | 20 | 1 |
| `0xd8316` | C10 |  | 24 | 20 | 1 |
| `0xd8366` | C10 |  | 24 | 20 | 1 |
| `0xd83b8` | C10 |  | 24 | 20 | 1 |
| `0x153ba` | F3 |  | 44 | 19 | 2 |
| `0x154e8` | F3 |  | 46 | 19 | 2 |
| `0x1c86e` | W1 | world_update | 74 | 19 | 1 |
| `0x639fe` | C02 |  | 23 | 19 | 1 |
| `0x87048` | C04 | boss_seuil_pv | 23 | 19 | 1 |
| `0x87b86` | C04 |  | 23 | 19 | 1 |
| `0x89788` | C04 |  | 23 | 19 | 1 |
| `0x8a2c6` | C04 |  | 23 | 19 | 1 |
| `0xe2a74` | S1 | son_selon_version | 21 | 19 | 1 |
| `0x03f28` | X2 |  | 24 | 18 | 1 |
| `0x1422e` | C1 |  | 24 | 18 | 1 |
| `0x14652` | C1 | DIGITS | 123 | 18 | 4 |
| `0x47104` | P5 |  | 80 | 18 | 7 |
| `0x3a98e` | P4 |  | 30 | 17 | 1 |
| `0xd764a` | C10 |  | 32 | 17 | 1 |

## 2190 fonctions exécutées sur 2559 ; 369 jamais exécutées

| Zone | Sous-système | Fonctions | Jamais exécutées | dont entrées de natives |
|---|---|---:|---:|---:|
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 17 | 0 | 0 |
| V1 | Primitives vidéo (attente, couches, effacements) | 13 | 0 | 0 |
| V2 | Texte, rectangles et blocs de tuiles | 33 | 7 | 2 |
| V3 | Palettes et fondus | 38 | 0 | 0 |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 2 | 0 | 0 |
| O1 | Animation, tables de vitesse, physique, culling | 22 | 0 | 0 |
| O2 | Pools d'objets et listes triées | 41 | 0 | 0 |
| K3 | Entrées vers enregistrements joueur | 3 | 0 | 0 |
| P1 | Initialisation des joueurs | 23 | 0 | 0 |
| X2 | Dégâts, terrain, placement | 19 | 0 | 0 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 69 | 5 | 1 |
| F1 | Tâche de boot, avertissements région | 5 | 0 | 0 |
| C1 | Crédits, pièces, start/continue | 22 | 0 | 0 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 49 | 0 | 0 |
| F4 | Exécuteur d'écrans fixes (logos) | 5 | 1 | 0 |
| U1 | RNG et files d'affichage (pop-ups) | 18 | 0 | 0 |
| U2 | Petites routines / tables (rôle inconnu) | 17 | 2 | 2 |
| W1 | Boucle de partie et phases 0/4/12/20 | 3 | 0 | 0 |
| F5 | Curseur de sélection de personnage | 10 | 2 | 0 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 15 | 1 | 0 |
| F6 | Interpréteur de scripts de scène (démos) | 18 | 0 | 0 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 0 | 0 |
| P3 | Comportement scénarisé des joueurs par scène | 3 | 0 | 0 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 0 | 0 |
| K4 | Exceptions et écran de dump debug | 16 | 16 | 0 |
| M1 | Menu TEST (service) | 76 | 26 | 0 |
| M2 | EEPROM, configuration, comptabilité | 60 | 24 | 2 |
| B1 | Scrolling / caméras par scène | 42 | 1 | 1 |
| P4 | Noyau joueur (états, 26 actions génériques) | 59 | 2 | 0 |
| W0 | Dispatch des 10 classes d'objets | 10 | 0 | 0 |
| O3 | Aides communes aux types (animations par type) | 25 | 1 | 0 |
| P5 | Code par personnage (6 × 2 variantes) | 73 | 20 | 0 |
| C12 | PEFF : effets du joueur | 107 | 39 | 13 |
| C06 | PSHL : tirs du joueur | 39 | 9 | 4 |
| C02 | ENEMY : ennemis | 205 | 26 | 0 |
| C04 | BOSS : boss | 353 | 45 | 2 |
| C08 | ESHL : tirs/attaques ennemis | 127 | 25 | 1 |
| C0e | EFFCT : effets | 170 | 9 | 6 |
| C10 | SET : décors/animations de palette | 131 | 15 | 0 |
| C0a | BOX : caisses/coffres (h) | 7 | 0 | 0 |
| C0c | ITEM : objets ramassables | 14 | 0 | 0 |
| C14 | SOBJ : objets système/UI | 25 | 1 | 0 |
| S1 | Son : wrappers de commandes + FIFO | 410 | 63 | 61 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 29 | 0 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 9 | 0 | 0 |
| W3 | Scripts de scène (phases 16/28) | 82 | 0 | 0 |
| F7 | Écrans titre / copyright | 5 | 0 | 0 |

### V2 : Texte, rectangles et blocs de tuiles (7 sur 33)

* `0x00d18` FUN_00000d18 (4 o)
* `0x00fe8` FUN_00000fe8 (8 o)
* `0x011bc` FUN_000011bc (4 o)
* `0x012a0` FUN_000012a0 (30 o)
* `0x012c4` FUN_000012c4 (50 o)
* `0x0255a` FUN_0000255a (44 o) : native `video_chiffres_decimaux`
* `0x025e8` FUN_000025e8 (36 o) : native `video_chiffres_hexa`

### E1 : Aides d'IA (cible, portées, décisions aléatoires) (5 sur 69)

* `0x07024` FUN_00007024 (10 o)
* `0x0ad7c` FUN_0000ad7c (52 o) : code-mort.md
* `0x0ffda` FUN_0000ffda (26 o) : code-mort.md
* `0x101da` FUN_000101da (62 o) : native `palette_quatre_blocs_69`
* `0x10298` FUN_00010298 (34 o) : code-mort.md

### F4 : Exécuteur d'écrans fixes (logos) (1 sur 5)

* `0x196ae` FUN_000196ae (666 o)

### U2 : Petites routines / tables (rôle inconnu) (2 sur 17)

* `0x1aad4` FUN_0001aad4 (26 o) : native `palette_eclair_c`
* `0x1aaee` FUN_0001aaee (26 o) : native `palette_eclair_d`

### F5 : Curseur de sélection de personnage (2 sur 10)

* `0x1d848` FUN_0001d848 (34 o)
* `0x1d888` FUN_0001d888 (46 o)

### V4 : Construction de la liste de sprites (OBJ RAM) (1 sur 15)

* `0x1ed22` FUN_0001ed22 (174 o)

### K4 : Exceptions et écran de dump debug (16 sur 16)

* `0x286ea` thunk_FUN_000286f2 (4 o)
* `0x286f2` FUN_000286f2 (238 o)
* `0x28800` FUN_00028800 (98 o)
* `0x28864` FUN_00028864 (122 o)
* `0x288de` FUN_000288de (76 o)
* `0x28964` FUN_00028964 (40 o)
* `0x2898c` FUN_0002898c (40 o)
* `0x289dc` FUN_000289dc (364 o)
* `0x28b50` FUN_00028b50 (8 o)
* `0x28b58` FUN_00028b58 (10 o)
* `0x28b62` FUN_00028b62 (8 o)
* `0x28b6a` FUN_00028b6a (28 o)
* `0x28b86` FUN_00028b86 (28 o)
* `0x28ba2` FUN_00028ba2 (80 o)
* `0x28bf2` FUN_00028bf2 (70 o)
* `0x28c38` FUN_00028c38 (152 o)

### M1 : Menu TEST (service) (26 sur 76)

* `0x296dc` FUN_000296dc (28 o)
* `0x29c20` FUN_00029c20 (18 o)
* `0x29c32` FUN_00029c32 (24 o)
* `0x29c68` FUN_00029c68 (40 o)
* `0x2b066` FUN_0002b066 (46 o)
* `0x2b094` FUN_0002b094 (40 o)
* `0x2b0bc` FUN_0002b0bc (34 o)
* `0x2b0de` FUN_0002b0de (72 o)
* `0x2b126` FUN_0002b126 (68 o)
* `0x2b16a` FUN_0002b16a (18 o)
* `0x2b17c` FUN_0002b17c (18 o)
* `0x2b18e` FUN_0002b18e (66 o)
* `0x2b216` FUN_0002b216 (96 o)
* `0x2b276` FUN_0002b276 (72 o)
* `0x2b2be` FUN_0002b2be (42 o)
* `0x2b2e8` FUN_0002b2e8 (30 o)
* `0x2b306` FUN_0002b306 (30 o)
* `0x2b444` FUN_0002b444 (6 o)
* `0x2cf26` FUN_0002cf26 (368 o)
* `0x2d096` FUN_0002d096 (56 o)
* `0x2d0d6` FUN_0002d0d6 (48 o)
* `0x2d106` FUN_0002d106 (8 o)
* `0x2d10e` FUN_0002d10e (6 o)
* `0x2d114` FUN_0002d114 (8 o)
* `0x2d11c` FUN_0002d11c (8 o)
* `0x2d124` FUN_0002d124 (30 o)

### M2 : EEPROM, configuration, comptabilité (24 sur 60)

* `0x2da4a` FUN_0002da4a (70 o)
* `0x2da90` FUN_0002da90 (58 o)
* `0x2daf4` FUN_0002daf4 (42 o) : native `compta_departs`
* `0x2dca6` FUN_0002dca6 (80 o)
* `0x2dcd2` FUN_0002dcd2 (36 o)
* `0x2dd14` FUN_0002dd14 (32 o)
* `0x2de2a` FUN_0002de2a (66 o)
* `0x2e082` FUN_0002e082 (66 o)
* `0x2e204` FUN_0002e204 (88 o)
* `0x2e49a` FUN_0002e49a (66 o) : native `compta_complete_departs`
* `0x2e4dc` FUN_0002e4dc (16 o)
* `0x2e620` FUN_0002e620 (16 o)
* `0x2e77c` FUN_0002e77c (26 o)
* `0x2e8e4` FUN_0002e8e4 (54 o)
* `0x2e93e` FUN_0002e93e (46 o)
* `0x2e96c` FUN_0002e96c (46 o)
* `0x2e99a` FUN_0002e99a (46 o)
* `0x2e9c8` FUN_0002e9c8 (46 o)
* `0x2e9f6` FUN_0002e9f6 (46 o)
* `0x2ea52` FUN_0002ea52 (64 o)
* `0x2ea92` FUN_0002ea92 (88 o)
* `0x2eaea` FUN_0002eaea (32 o)
* `0x2ebc4` FUN_0002ebc4 (28 o)
* `0x2ebe0` FUN_0002ebe0 (28 o)

### B1 : Scrolling / caméras par scène (1 sur 42)

* `0x33196` FUN_00033196 (36 o) : native `decor_parallaxe_trois_quarts`

### P4 : Noyau joueur (états, 26 actions génériques) (2 sur 59)

* `0x397a8` FUN_000397a8 (56 o)
* `0x3b8c4` FUN_0003b8c4 (12 o)

### O3 : Aides communes aux types (animations par type) (1 sur 25)

* `0x3cfaa` FUN_0003cfaa (38 o)

### P5 : Code par personnage (6 × 2 variantes) (20 sur 73)

* `0x3fb8c` FUN_0003fb8c (44 o)
* `0x410d8` FUN_000410d8 (196 o)
* `0x41aa4` FUN_00041aa4 (32 o)
* `0x41ac4` FUN_00041ac4 (58 o)
* `0x41c30` FUN_00041c30 (32 o)
* `0x41c50` FUN_00041c50 (248 o)
* `0x439fc` FUN_000439fc (264 o)
* `0x44270` FUN_00044270 (68 o)
* `0x44384` FUN_00044384 (240 o)
* `0x46f00` FUN_00046f00 (46 o)
* `0x474c4` FUN_000474c4 (32 o)
* `0x474e4` FUN_000474e4 (280 o)
* `0x475fc` FUN_000475fc (56 o)
* `0x47718` FUN_00047718 (56 o)
* `0x47750` FUN_00047750 (32 o)
* `0x47770` FUN_00047770 (282 o)
* `0x48392` FUN_00048392 (232 o)
* `0x48660` FUN_00048660 (228 o)
* `0x49cf0` FUN_00049cf0 (46 o)
* `0x49d1e` FUN_00049d1e (40 o)

### C12 : PEFF : effets du joueur (39 sur 107)

* `0x4aeb2` FUN_0004aeb2 (30 o) : native `spawn_holy_light`
* `0x4aed0` FUN_0004aed0 (68 o) : native `cree_effets_5`
* `0x4c26c` FUN_0004c26c (72 o) : native `spawn_banish`
* `0x4c4a0` FUN_0004c4a0 (238 o)
* `0x4cdd0` FUN_0004cdd0 (60 o) : native `spawn_lightning`
* `0x4cfee` FUN_0004cfee (50 o) : native `spawn_portal_sparks`, code-mort.md
* `0x4d020` FUN_0004d020 (24 o)
* `0x4d038` FUN_0004d038 (44 o)
* `0x4d064` FUN_0004d064 (40 o) : native `spawn_portal_burst`, code-mort.md
* `0x4d2c0` FUN_0004d2c0 (62 o)
* `0x4d660` FUN_0004d660 (34 o)
* `0x4dfe2` FUN_0004dfe2 (86 o)
* `0x4e1b8` FUN_0004e1b8 (316 o)
* `0x4e4de` FUN_0004e4de (28 o)
* `0x4e782` FUN_0004e782 (26 o)
* `0x4ec36` FUN_0004ec36 (48 o)
* `0x4ec66` FUN_0004ec66 (28 o) : native `spawn_storm`
* `0x4ef1a` FUN_0004ef1a (208 o)
* `0x4f004` FUN_0004f004 (34 o) : native `spawn_lightning_target`
* `0x4fefe` FUN_0004fefe (32 o) : native `spawn_cursed_weapon`, code-mort.md
* `0x5009e` FUN_0005009e (144 o) : native `spawn_dying_shards`, code-mort.md
* `0x50324` FUN_00050324 (36 o) : native `spawn_wall_target`
* `0x5085a` FUN_0005085a (58 o)
* `0x50894` FUN_00050894 (104 o)
* `0x508fc` FUN_000508fc (78 o)
* `0x5094a` FUN_0005094a (100 o)
* `0x509ae` FUN_000509ae (142 o)
* `0x50a3c` FUN_00050a3c (42 o)
* `0x50a66` FUN_00050a66 (106 o)
* `0x50ad0` FUN_00050ad0 (50 o)
* `0x50b02` FUN_00050b02 (40 o)
* `0x50b2a` FUN_00050b2a (56 o)
* `0x50b62` FUN_00050b62 (40 o)
* `0x50d9c` FUN_00050d9c (20 o)
* `0x514ac` FUN_000514ac (32 o)
* `0x514cc` FUN_000514cc (32 o)
* `0x51872` FUN_00051872 (66 o) : native `spawn_portal_glow`, code-mort.md
* `0x519b0` FUN_000519b0 (16 o)
* `0x51aa4` FUN_00051aa4 (50 o) : native `spawn_storm_cloud`, code-mort.md

### C06 : PSHL : tirs du joueur (9 sur 39)

* `0x52002` FUN_00052002 (76 o) : native `spawn_thrown_e`
* `0x52a9e` FUN_00052a9e (144 o)
* `0x5308c` FUN_0005308c (48 o) : native `spawn_thrown_g`
* `0x5336a` FUN_0005336a (138 o)
* `0x53478` FUN_00053478 (64 o)
* `0x535f2` FUN_000535f2 (138 o)
* `0x54274` FUN_00054274 (122 o) : native `spawn_rain`, code-mort.md
* `0x54522` FUN_00054522 (26 o) : native `spawn_fireball`
* `0x5453c` FUN_0005453c (36 o)

### C02 : ENEMY : ennemis (26 sur 205)

* `0x5f2c6` FUN_0005f2c6 (36 o)
* `0x63ec0` FUN_00063ec0 (18 o)
* `0x63f64` FUN_00063f64 (32 o)
* `0x65ae0` FUN_00065ae0 (8 o)
* `0x666bc` FUN_000666bc (10 o)
* `0x67ea4` FUN_00067ea4 (92 o)
* `0x689e6` FUN_000689e6 (50 o)
* `0x690ce` FUN_000690ce (120 o)
* `0x6ab3c` FUN_0006ab3c (2 o)
* `0x6c032` FUN_0006c032 (14 o)
* `0x6c040` FUN_0006c040 (56 o)
* `0x6c078` FUN_0006c078 (40 o)
* `0x6c0a0` FUN_0006c0a0 (6 o)
* `0x6c0a6` FUN_0006c0a6 (48 o)
* `0x6d57c` FUN_0006d57c (14 o)
* `0x6d58a` FUN_0006d58a (56 o)
* `0x6d5c2` FUN_0006d5c2 (40 o)
* `0x6d5ea` FUN_0006d5ea (6 o)
* `0x6d5f0` FUN_0006d5f0 (48 o)
* `0x6eb1c` FUN_0006eb1c (14 o)
* `0x6eb2a` FUN_0006eb2a (56 o)
* `0x6eb62` FUN_0006eb62 (40 o)
* `0x6eb8a` FUN_0006eb8a (6 o)
* `0x6eb90` FUN_0006eb90 (48 o)
* `0x71d98` FUN_00071d98 (46 o)
* `0x791d0` FUN_000791d0 (16 o)

### C04 : BOSS : boss (45 sur 353)

* `0x7c02e` FUN_0007c02e (18 o)
* `0x7c396` FUN_0007c396 (26 o)
* `0x7c5a0` FUN_0007c5a0 (8 o)
* `0x800e0` FUN_000800e0 (36 o)
* `0x80130` FUN_00080130 (42 o)
* `0x801c2` FUN_000801c2 (8 o)
* `0x801ca` FUN_000801ca (20 o)
* `0x8069a` FUN_0008069a (26 o)
* `0x80cb0` FUN_00080cb0 (18 o)
* `0x80e16` thunk_FUN_000edb82 (6 o)
* `0x81810` FUN_00081810 (102 o)
* `0x818b2` FUN_000818b2 (52 o)
* `0x818e6` FUN_000818e6 (48 o) : native `boss_3_pousse_devant`, code-mort.md
* `0x81916` FUN_00081916 (58 o)
* `0x819b0` FUN_000819b0 (50 o)
* `0x819e2` FUN_000819e2 (40 o) : native `boss_3_pousse_cote`, code-mort.md
* `0x81a0a` FUN_00081a0a (58 o)
* `0x83414` FUN_00083414 (8 o)
* `0x83dde` FUN_00083dde (32 o)
* `0x87e20` FUN_00087e20 (126 o)
* `0x89760` FUN_00089760 (40 o)
* `0x8bc0c` FUN_0008bc0c (50 o)
* `0x8bcd2` FUN_0008bcd2 (54 o)
* `0x8be88` FUN_0008be88 (60 o)
* `0x8c1a0` FUN_0008c1a0 (108 o)
* `0x8c450` FUN_0008c450 (38 o)
* `0x8c576` FUN_0008c576 (34 o)
* `0x8c698` FUN_0008c698 (8 o)
* `0x8c6a0` FUN_0008c6a0 (50 o)
* `0x8c8d2` FUN_0008c8d2 (76 o)
* `0x8c952` FUN_0008c952 (14 o)
* `0x8c960` FUN_0008c960 (40 o)
* `0x8c988` FUN_0008c988 (34 o)
* `0x8c9ba` FUN_0008c9ba (20 o)
* `0x8eb1a` FUN_0008eb1a (14 o)
* `0x90798` FUN_00090798 (14 o)
* `0x9300c` FUN_0009300c (18 o)
* `0x95272` FUN_00095272 (66 o)
* `0x9ad94` FUN_0009ad94 (70 o)
* `0x9d24e` FUN_0009d24e (6 o)
* `0x9d254` FUN_0009d254 (44 o)
* `0x9d8e0` FUN_0009d8e0 (72 o)
* `0x9d9d0` FUN_0009d9d0 (48 o)
* `0x9da00` FUN_0009da00 (92 o)
* `0x9da5c` FUN_0009da5c (98 o)

### C08 : ESHL : tirs/attaques ennemis (25 sur 127)

* `0x9f346` FUN_0009f346 (48 o) : code-mort.md
* `0x9f60c` FUN_0009f60c (68 o)
* `0x9fd90` FUN_0009fd90 (110 o)
* `0xa0294` FUN_000a0294 (76 o)
* `0xa02e0` FUN_000a02e0 (110 o)
* `0xa09ca` FUN_000a09ca (60 o) : code-mort.md
* `0xa0ae0` FUN_000a0ae0 (64 o) : code-mort.md
* `0xa114a` FUN_000a114a (54 o) : code-mort.md
* `0xa2048` FUN_000a2048 (24 o)
* `0xa2d22` FUN_000a2d22 (40 o) : code-mort.md
* `0xa2e40` FUN_000a2e40 (40 o)
* `0xa2e74` FUN_000a2e74 (26 o)
* `0xa2e8e` FUN_000a2e8e (70 o)
* `0xa3ac4` FUN_000a3ac4 (32 o)
* `0xa3f4e` FUN_000a3f4e (134 o)
* `0xa4054` FUN_000a4054 (20 o)
* `0xa462c` FUN_000a462c (34 o) : native `eshl_lie_34_3`
* `0xa4ae4` FUN_000a4ae4 (24 o)
* `0xa4b8e` FUN_000a4b8e (46 o)
* `0xa5ab4` FUN_000a5ab4 (34 o)
* `0xa5c56` FUN_000a5c56 (38 o)
* `0xa63b2` FUN_000a63b2 (38 o)
* `0xa64e4` FUN_000a64e4 (86 o)
* `0xa6588` FUN_000a6588 (86 o)
* `0xa661a` FUN_000a661a (86 o)

### C0e : EFFCT : effets (9 sur 170)

* `0xaa8d6` FUN_000aa8d6 (18 o)
* `0xaadba` FUN_000aadba (112 o)
* `0xb3e4e` FUN_000b3e4e (56 o) : native `cree_cris`
* `0xb3ec6` FUN_000b3ec6 (58 o) : native `cree_cris_b`
* `0xb4f1c` FUN_000b4f1c (40 o) : native `cree_sorts_c`
* `0xb8d80` FUN_000b8d80 (66 o)
* `0xb96c0` FUN_000b96c0 (28 o) : native `cree_eclat_lance`
* `0xb97a2` FUN_000b97a2 (24 o) : native `cree_eclat_pose`
* `0xb97ba` FUN_000b97ba (30 o) : native `cree_eclat_pose_b`

### C10 : SET : décors/animations de palette (15 sur 131)

* `0xbacec` FUN_000bacec (10 o)
* `0xc8a10` FUN_000c8a10 (114 o)
* `0xcc2f4` FUN_000cc2f4 (94 o)
* `0xcc548` FUN_000cc548 (6 o)
* `0xcffd0` FUN_000cffd0 (12 o)
* `0xd0870` FUN_000d0870 (78 o)
* `0xd76b4` FUN_000d76b4 (94 o)
* `0xd7a38` FUN_000d7a38 (18 o)
* `0xd8836` FUN_000d8836 (226 o)
* `0xd8918` FUN_000d8918 (110 o)
* `0xd8a70` FUN_000d8a70 (146 o)
* `0xd8c4c` FUN_000d8c4c (150 o)
* `0xd8f30` FUN_000d8f30 (228 o)
* `0xd9014` FUN_000d9014 (86 o)
* `0xd9762` FUN_000d9762 (14 o)

### C14 : SOBJ : objets système/UI (1 sur 25)

* `0xe038a` FUN_000e038a (60 o)

### S1 : Son : wrappers de commandes + FIFO (63 sur 410)

* `0xe089e` FUN_000e089e (22 o)
* `0xe19f8` FUN_000e19f8 (14 o) : native `son_wrapper`
* `0xe296c` FUN_000e296c (136 o)
* `0xe2afa` FUN_000e2afa (64 o) : native `son_wrapper`
* `0xe2b3a` FUN_000e2b3a (64 o) : native `son_wrapper`
* `0xe2b7a` FUN_000e2b7a (64 o) : native `son_wrapper`
* `0xe2bfa` FUN_000e2bfa (64 o) : native `son_wrapper`
* `0xe2c7a` FUN_000e2c7a (64 o) : native `son_wrapper`
* `0xe2cba` FUN_000e2cba (64 o) : native `son_wrapper`
* `0xe2cfa` FUN_000e2cfa (64 o) : native `son_wrapper`
* `0xe2dfa` FUN_000e2dfa (64 o) : native `son_wrapper`
* `0xe2efa` FUN_000e2efa (64 o) : native `son_wrapper`
* `0xe2fba` FUN_000e2fba (64 o) : native `son_wrapper`
* `0xe30ba` FUN_000e30ba (64 o) : native `son_wrapper`
* `0xe313a` FUN_000e313a (64 o) : native `son_wrapper`
* `0xe317a` FUN_000e317a (64 o) : native `son_wrapper`
* `0xe31fa` FUN_000e31fa (64 o) : native `son_wrapper`
* `0xe323a` FUN_000e323a (64 o) : native `son_wrapper`
* `0xe385e` caseD_8 (112 o) : native `son_wrapper`
* `0xe4188` FUN_000e4188 (106 o) : native `son_wrapper`
* `0xe46de` FUN_000e46de (112 o) : native `son_wrapper`
* `0xe4752` FUN_000e4752 (106 o) : native `son_wrapper`
* `0xe47bc` FUN_000e47bc (112 o) : native `son_wrapper`
* `0xe55a2` FUN_000e55a2 (92 o) : native `son_wrapper`
* `0xe55fe` FUN_000e55fe (52 o) : native `son_wrapper`
* `0xe5632` FUN_000e5632 (112 o) : native `son_wrapper`
* `0xe64ce` FUN_000e64ce (112 o) : native `son_wrapper`
* `0xe83e2` FUN_000e83e2 (110 o) : native `son_wrapper`
* `0xe8454` FUN_000e8454 (110 o) : native `son_wrapper`
* `0xe8676` FUN_000e8676 (14 o) : native `son_wrapper`
* `0xe876a` FUN_000e876a (138 o) : native `son_wrapper`
* `0xe8ef2` FUN_000e8ef2 (44 o) : native `son_wrapper`
* `0xe8fc6` FUN_000e8fc6 (106 o) : native `son_wrapper`
* `0xe910c` FUN_000e910c (106 o) : native `son_wrapper`
* `0xe9240` FUN_000e9240 (134 o) : native `son_wrapper`
* `0xe92c6` FUN_000e92c6 (106 o) : native `son_wrapper`
* `0xe9330` FUN_000e9330 (138 o) : native `son_wrapper`
* `0xe93ba` FUN_000e93ba (106 o) : native `son_wrapper`
* `0xe9424` FUN_000e9424 (106 o) : native `son_wrapper`
* `0xe948e` FUN_000e948e (106 o) : native `son_wrapper`
* `0xe974a` FUN_000e974a (106 o) : native `son_wrapper`
* `0xe98fa` FUN_000e98fa (110 o) : native `son_wrapper`
* `0xe996c` FUN_000e996c (92 o) : native `son_wrapper`
* `0xe99c8` FUN_000e99c8 (106 o) : native `son_wrapper`
* `0xe9a32` FUN_000e9a32 (86 o) : native `son_wrapper`
* `0xe9a88` FUN_000e9a88 (92 o) : native `son_wrapper`
* `0xe9d3e` FUN_000e9d3e (106 o) : native `son_wrapper`
* `0xe9da8` FUN_000e9da8 (140 o) : native `son_wrapper`
* `0xe9e34` FUN_000e9e34 (106 o) : native `son_wrapper`
* `0xe9e9e` FUN_000e9e9e (86 o) : native `son_wrapper`
* `0xe9f5e` FUN_000e9f5e (106 o) : native `son_wrapper`
* `0xe9fc8` FUN_000e9fc8 (106 o) : native `son_wrapper`
* `0xea032` FUN_000ea032 (106 o) : native `son_wrapper`
* `0xea09c` FUN_000ea09c (106 o) : native `son_wrapper`
* `0xea106` FUN_000ea106 (96 o) : native `son_wrapper`
* `0xea166` FUN_000ea166 (106 o) : native `son_wrapper`
* `0xea50c` FUN_000ea50c (110 o) : native `son_wrapper`
* `0xea5f0` FUN_000ea5f0 (110 o) : native `son_wrapper`
* `0xeab50` FUN_000eab50 (92 o) : native `son_wrapper`
* `0xeae86` FUN_000eae86 (110 o) : native `son_wrapper`
* `0xebc4c` FUN_000ebc4c (110 o) : native `son_wrapper`
* `0xedb82` FUN_000edb82 (110 o) : native `son_wrapper`
* `0xedbf4` FUN_000edbf4 (110 o) : native `son_wrapper`

### M3 : « Secret Test » (debug) et Block Test (29 sur 29)

* `0xee16e` FUN_000ee16e (2730 o)
* `0xeeef0` FUN_000eeef0 (18 o)
* `0xef116` FUN_000ef116 (44 o)
* `0xef6b0` FUN_000ef6b0 (170 o)
* `0xef75a` FUN_000ef75a (14 o)
* `0xef768` FUN_000ef768 (34 o)
* `0xef78a` FUN_000ef78a (56 o)
* `0xef7c2` FUN_000ef7c2 (18 o)
* `0xef7d4` FUN_000ef7d4 (38 o)
* `0xef7fa` FUN_000ef7fa (14 o)
* `0xef808` FUN_000ef808 (14 o)
* `0xef816` FUN_000ef816 (66 o)
* `0xef858` FUN_000ef858 (28 o)
* `0xef874` FUN_000ef874 (18 o)
* `0xef886` FUN_000ef886 (28 o)
* `0xef8aa` FUN_000ef8aa (52 o)
* `0xef8de` FUN_000ef8de (70 o)
* `0xef924` FUN_000ef924 (86 o)
* `0xef97a` FUN_000ef97a (72 o)
* `0xef9c2` FUN_000ef9c2 (8 o)
* `0xef9ca` FUN_000ef9ca (10 o)
* `0xef9d4` FUN_000ef9d4 (10 o)
* `0xef9de` FUN_000ef9de (14 o)
* `0xefa40` FUN_000efa40 (90 o)
* `0xefb8e` FUN_000efb8e (100 o)
* `0xefbf2` caseD_86 (36 o)
* `0xf2954` FUN_000f2954 (20 o)
* `0xf2970` FUN_000f2970 (116 o)
* `0xf29fa` FUN_000f29fa (18 o)


## Validation par fuzz

Lot « fuzz » (branche `wt-fuzz`, 2026-10-27/28) : le code d'origine que les 272 scénarios n'exécutent pas (tableaux
ci-dessus) est exécuté par un fuzz différentiel, l'original et la native Rust partant du même état, et comparé.

### Méthode : `examples/fuzz_couv`

`fuzz_couv <roms> <eeprom> <scénario> <images> <cibles> <sortie.bin>` déroule un scénario sur la machine de
référence (code d'origine seul, 68000 ×16). Aux appels d'une native ciblée (zone de l'atlas, adresse ou préfixe de
nom), l'état réel devient une *base* ; des copies perturbées en sont exécutées par l'original (interprété, couverture
relevée) et par la native, puis comparées : RAM (hors piles mortes et RAM du noyau, la table des tâches Rust étant
reconstruite depuis FF0064 puis comparée par `taches::table::ecarts`), mémoire vidéo, registres du contrat, PC.

* **Perturbations ciblées.** La base est d'abord exécutée par l'original avec un bus qui journalise ses lectures de
  RAM : seuls les octets, mots et longs *lus* sont réécrits. Les lectures faites juste avant un saut conditionnel pris
  d'un seul côté par les scénarios (`FUZZ_UNION`, union des bitmaps de `cov`) sont « chaudes » : la valeur immédiate
  comparée et ses voisines, le bit testé ; celles qui précèdent un aiguillage par table de sauts reçoivent les index
  valides de la table. Les autres valeurs sont tirées du *domaine* du champ (valeurs vues dans le même champ d'un objet
  du même pool et du même type, ou à la même adresse), plus rarement quelconques. Une copie qui atteint une instruction
  nouvelle entre au corpus et sera de nouveau perturbée.
* **États exclus** (aucun code ne les produit) : RAM du noyau et pile privée des wrappers de son, têtes et liens des
  listes triées, octets d'existence et de type des objets ; un essai dont l'original sort du code (PC impair, hors des
  débuts d'instruction du listing, table de sauts lue au-delà de ses entrées) est écarté (« hors code »).
* **Entrées jamais appelées** (`FUZZ_SUBSTITUTS=1`) : une native de la zone jamais appelée est essayée depuis l'état
  d'une voisine (un gestionnaire d'objet sur l'objet A0 d'un gestionnaire du même pool, dont le type est changé et
  l'état souvent remis à zéro ; un wrapper de son avec A0 sur un joueur). Ces bases sont marquées `SUBST` : leurs
  registres d'entrée sont ceux d'un autre appel.
* **Code des tâches** : des « natives de test » (`hud::NATIVES_DE_TEST` 0x15216, `credits` 0x1422e, `tache_credits`
  0x13c2c, `modes::demo` 0xe05ec) présentent au fuzz des sous-programmes réécrits dans le corps d'une tâche ; le jeu
  ne les appelle pas par adresse. `FUZZ_FOCUS=zone` avec la cible `tout` fuzze toutes les natives dont la base passe
  par la zone (code atteint par des natives d'autres zones : collisions X1, interface F3...).
* **Tri des écarts** : chaque écart est réduit (perturbations retirées une à une tant que l'écart persiste) et
  étiqueté : `HORS` (une valeur hors du domaine du champ : état que le jeu ne produit pas), `REG` (registres seuls,
  sans effet tant qu'aucun appelant ne les relit), `SUBST`, `NEUF` (l'original passe par du code jamais exécuté par
  les scénarios). Les écarts *plausibles* (ni HORS, ni REG, ni SUBST) ont tous été analysés.
* **Diagnostic** : la fonctionnalité `surveille` de `ddsom-game` (`FUZZ_SURVEILLE=adresse[-fin]`) donne la pile
  d'appels Rust de chaque écriture de la native dans la plage, et les écritures de l'original avec leur PC ;
  `FUZZ_PC=adresse,...` montre les registres de l'original à ces PC.

Bilan : `tools/couverture_fuzz.py <listing> <index.tsv> <atlas.md> <union des scénarios> <bitmaps de fuzz_couv>...
[--detail ZONE]` ; la sortie `<sortie>.identiques` de `fuzz_couv` ne garde que les essais sans écart.

Par zone : 6 à 16 scénarios choisis pour couvrir les sauts de la frontière de la zone, un appel retenu sur 8 (les
4 premiers toujours, 400 au plus par native), 32 essais par appel retenu. Une zone dure de 2 à 15 minutes sur 6
cœurs.

### Écarts trouvés et corrigés

Tous sur des chemins que les scénarios n'exercent pas ; chacun corrigé dans un commit séparé (`git log wt-fuzz`).

| Zone | Routine | Écart | Correction |
|---|---|---|---|
| C04 | 0x833b6 oscillation des boss | D0 (décalage) non rendu ; `subq.w #1` + `bgt` depuis 0x8000 (dépassement) | D0 rendu ; comparaison signée exacte de l'ancienne valeur |
| C04 | 0x87048 seuil de PV des boss | D1.w (moitié des PV) non rendu | D1.w rendu |
| C04 | 0x81ae0 riposte du boss 3 | D0 (`andi.l #0xf`) non rendu ; index de table `(d8,PC,D1.w)` non étendu en signe | idem |
| C04 | 0x7c430 hauteur du bond du boss 1 | mot haut de D3 pris dans le reste du registre Rust, nul dans l'original (laissé par la direction 0x137e) | D3 = mot bas seul |
| C04 | 0x7aeda traversée du boss 1 | index des tables de vitesses et d'animations calculés sur 32 bits au lieu d'un mot étendu en signe | `wrapping_mul` sur un mot, extension de signe |
| C04 | boss 21, bond étape 2 (0x99e52) | pas d'animation avec recopie de la sorte de coup `+0x44` (0x1040c) au lieu de 0x1a56 | `anim_step` seul |
| V2 | 0x1386 direction | `add.l #0x8000,D4` : la retenue dans le mot haut de D4 n'était pas rendue | idem |
| E1 / C04 | 0x10298 script de palette (réveil du boss 16, variante 1) | table lue dans l'espace programme (opcodes déchiffrés) au lieu de l'espace des données (`lea (d,PC),A2` puis `movem.l (A2)`) : couleurs fausses | `data32` (corrigé aussi, en parallèle, par le lot IA) |
| C12 | PEFF 12 (gel), éclat qui rebondit | table d'angles du bord bas en 0x4be8c au lieu de 0x4be88 (`lea` de 0x4be5e) | adresse corrigée |
| C10 | 0xd40e8 création des tremplins | mot haut de D5 (compteur `moveq #12`) laissé au reste | D5 long |
| C08 | ESHL 8 (0x9ee00) et 13 (0x9f712) | sans propriétaire (`+0x50` nul), l'original passe l'effet à l'état 6 ; la native le laissait tel quel | `move.l #0x6000000,(4,A0)` |
| C0c | ITEM 3, échange d'équipement (0xdbc2e → 0xdc28a) | l'objet expulsé est tiré de la table par D0.w, dont l'octet haut (classe × 8) n'est nul qu'avec un vrai propriétaire ; index non étendu en signe | index D0.w complet passé à `objet_expulse` |
| P4 | 0x3b3aa joueur gelé, chute hors de l'état 4 | variante de l'effet 0xb6ba8 prise dans un reste Rust de D0 au lieu du D0 laissé par le mouvement 0x1da6 (octet bas nul) | D0 recalculé |

Les écarts d'outillage (états impossibles, registres non relus) ne sont pas corrigés : voir ci-dessous.

### Écarts restants (états que le jeu ne produit pas)

* **HORS** : valeurs hors du domaine du champ. Les plus fréquents : octet `+0x15e` des joueurs ≥ 0x80 (P5, ses
  valeurs sont 0 à 0x25), orientation `+0x42` des anneaux du boss 11 hors de 0 à 31, rang `A5+0x7f92` ≥ 0x8000 (index
  négatifs : boss 0, 6, 9, 21), personnage `+3` des joueurs à 6 (P4, surcouches d'armes), pointeurs impairs. Les
  natives qui détectent ces états paniquent avec un message (« indice hors de la table de sauts », « pool plein :
  l'original lit un mot à une adresse impaire », réaction 0x3ddf6 « pas 0xc hors de la table d'origine »).
* **REG** : registres de sortie seuls (contrats `regs::ALL` des natives des scripts SET, CCR des poussées du boss 3,
  CCR de `ia_boite`) : aucun appelant Rust ne les relit.
* **SUBST** : wrappers de son appelés depuis l'état d'un autre wrapper imbriqué (la sauvegarde de l'original sur la
  pile privée, plus profonde que dans le jeu, recouvre les têtes de la file : écart assumé de `sound::jouer_wrapper`) ;
  gestionnaires de boss dont les champs viennent d'un autre type.
* **Natives de haut niveau** (`world_update`, `game_phase12`, `scene_phase_*`, `players_update`) : une perturbation
  profonde y fait diverger un gestionnaire ; les cas plausibles retombent sur les écarts du tableau ci-dessus ou sur
  ceux des boucles de dispatch (registres laissés par le gestionnaire précédent, `DDSOM_RESTES`).
* **Retours non standard** (`select_phase_4`, `ia_atterrissage_par_type`) : la native revient plus haut (cession,
  épilogue étranger) ; le cadre du fuzz ne les compare pas.
* À trancher : la table des sous-états de la réaction des boss 15 (0x929b8) a six entrées, dont 8 (`rts`) et 0xa
  (0x92ce0 : retour au combat, état 0x02020000) que la native ignore (`_ => {}`) ; même motif pour le boss 19
  (0x9101a). Aucun écrivain de `+5` = 8 ou 0xa dans l'état 4 n'a été trouvé (les valeurs viennent d'autres états du
  même objet) : état probablement impossible, non corrigé. Animation des surcouches d'armes pour le personnage 6
  (P4, `+3` = 6 hors des six personnages).

### Couverture obtenue

Union de tous les essais (environ 900 exécutions de `fuzz_couv`, une par scénario et par zone) : instructions, côtés de
sauts et fonctions du code d'origine que les 272 scénarios n'exécutent pas, exécutés par le fuzz ; « sans écart » :
des essais où l'original et la native sont identiques (relevé seulement pour les dernières séries, minorant).

| Zone | Instructions jamais exécutées par les scénarios | exécutées par le fuzz | dont sans écart (essais tracés) | Sauts pris d'un seul côté | autre côté pris par le fuzz | dont sans écart | Fonctions jamais exécutées | exécutées par le fuzz | dont sans écart |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| S1 | 552 | 520 | 316 | 472 | 471 | 266 | 63 | 59 | 46 |
| C04 | 426 | 328 | 297 | 101 | 76 | 69 | 45 | 37 | 36 |
| C10 | 399 | 213 | 209 | 52 | 28 | 27 | 15 | 9 | 9 |
| C1 | 372 | 347 | 347 | 29 | 24 | 24 | 0 | 0 | 0 |
| P5 | 188 | 74 | 74 | 48 | 23 | 23 | 20 | 9 | 9 |
| F3 | 154 | 55 | 55 | 23 | 12 | 12 | 0 | 0 | 0 |
| C12 | 106 | 82 | 66 | 19 | 17 | 14 | 39 | 29 | 25 |
| V4 | 104 | 102 | 102 | 7 | 6 | 6 | 1 | 1 | 1 |
| C02 | 81 | 76 | 63 | 24 | 21 | 18 | 26 | 20 | 18 |
| P4 | 74 | 45 | 32 | 16 | 13 | 13 | 2 | 2 | 2 |
| C08 | 73 | 56 | 27 | 28 | 22 | 15 | 25 | 18 | 18 |
| C0c | 66 | 51 | 31 | 20 | 13 | 7 | 0 | 0 | 0 |
| P6 | 62 | 34 | 34 | 9 | 6 | 6 | 0 | 0 | 0 |
| C0e | 57 | 21 | 12 | 29 | 17 | 11 | 9 | 5 | 5 |
| X1 | 46 | 20 | 20 | 18 | 7 | 7 | 0 | 0 | 0 |
| O2 | 43 | 37 | 37 | 11 | 8 | 8 | 0 | 0 | 0 |
| E1 | 42 | 33 | 33 | 15 | 13 | 13 | 5 | 3 | 3 |
| X2 | 33 | 29 | 29 | 9 | 7 | 7 | 0 | 0 | 0 |
| K1 | 28 | 0 | 0 | 5 | 0 | 0 | 0 | 0 | 0 |
| B1 | 27 | 2 | 2 | 3 | 3 | 3 | 1 | 1 | 1 |
| C06 | 26 | 26 | 26 | 15 | 15 | 15 | 9 | 8 | 8 |
| V2 | 19 | 12 | 12 | 11 | 7 | 7 | 7 | 3 | 3 |
| W1 | 19 | 19 | 19 | 1 | 1 | 1 | 0 | 0 | 0 |
| O3 | 19 | 10 | 10 | 4 | 4 | 4 | 1 | 1 | 1 |
| W3 | 16 | 11 | 11 | 6 | 6 | 6 | 0 | 0 | 0 |
| V3 | 15 | 11 | 11 | 8 | 5 | 5 | 0 | 0 | 0 |
| U1 | 9 | 8 | 8 | 5 | 5 | 5 | 0 | 0 | 0 |
| F5 | 9 | 5 | 0 | 4 | 1 | 0 | 2 | 0 | 0 |
| F1 | 7 | 0 | 0 | 4 | 0 | 0 | 0 | 0 | 0 |
| O1 | 6 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| F4 | 6 | 0 | 0 | 1 | 0 | 0 | 1 | 0 | 0 |
| V1 | 4 | 4 | 4 | 1 | 1 | 1 | 0 | 0 | 0 |
| K2 | 4 | 0 | 0 | 2 | 0 | 0 | 0 | 0 | 0 |
| C14 | 4 | 4 | 4 | 4 | 4 | 4 | 1 | 1 | 1 |
| F6 | 3 | 3 | 3 | 2 | 2 | 2 | 0 | 0 | 0 |
| F7 | 3 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| C0a | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 |
| K3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| U2 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 1 | 1 |
| W2 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| P3 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| W0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| total | 3103 | 2238 | 1894 | 1008 | 838 | 599 | 274 | 207 | 187 |

Restent (zones de jeu) :

* **Code du corps des tâches** sans native de test : K1 (noyau), F1, F4, F7, O1, K2 (auto-test RAM) ; F3 (interface
  joueurs) n'est couvert qu'à 55/154 par l'emplacement 0x15216 (choix du personnage et reprise : il faut des
  enchaînements d'entrées).
* **C10** : les styles de fenêtres de 0xc8430 (76 instructions) ne sont choisis que par des enregistrements de la ROM que
  les scénarios n'affichent pas (le fuzz ne fabrique pas d'enregistrement) ; 0xca524 (39), 0xcc200, 0xd764a.
* **P5** : sorts et objets jamais lancés (0x41aa4 bannissement du clerc, 0x474c4 foudre, 0x47750 tempête,
  0x48392 mur de feu, 0x46044 baguettes du magicien) : réécrits, atteignables en jeu, mais il faut le sort ou l'objet
  en main ; les bots longs (`scenarios/long`, lot « trous ») ne les couvrent pas non plus.
* Fonctions jamais exécutées, ni par les scénarios ni par le fuzz :
  * S1 : 0xe089e 0xe19f8 0xe8676 0xe8ef2 
  * C04 : 0x800e0 0x80130 0x801c2 0x801ca 0x89760 0x8bcd2 0x8c576 0x9ad94 
  * C10 : 0xbacec 0xc8a10 0xcc548 0xcffd0 0xd76b4 0xd7a38 
  * P5 : 0x41aa4 0x41ac4 0x439fc 0x44384 0x474c4 0x474e4 0x47750 0x47770 0x48392 0x48660 0x49d1e 
  * C12 : 0x4ef1a 0x508fc 0x5094a 0x509ae 0x50a3c 0x50a66 0x50ad0 0x50b02 0x50b2a 0x50b62 
  * C02 : 0x5f2c6 0x65ae0 0x666bc 0x689e6 0x6d5ea 0x791d0 
  * C08 : 0x9f346 0xa09ca 0xa0ae0 0xa2048 0xa3ac4 0xa4b8e 0xa63b2 
  * C0e : 0xb3ec6 0xb8d80 0xb96c0 0xb97a2 
  * V2 : 0x00d18 0x00fe8 0x011bc 0x012c4 
  * E1 : 0x07024 0x0ffda 
  * F5 : 0x1d848 0x1d888 
  * F4 : 0x196ae 
  * U2 : 0x1aaee 
  * C06 : 0x52a9e 

  (aucun appelant direct dans `decomp/index.tsv` pour celles de S1 et C04 : atteintes, si elles le sont, par des tables
  que ni les scénarios ni le fuzz ne choisissent ; voir aussi les entrées « code-mort.md » des listes ci-dessus.)
* Débogage et tests (K4, M1, M2, M3) : non traités par ce lot (le groupe `m2` de `examples/fuzz` couvre l'EEPROM).
