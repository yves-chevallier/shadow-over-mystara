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
instruction, 121 609 instructions distinctes, n'est pas ventilée ici). Les tailles d'`index.tsv` incluent parfois
des données mal désassemblées.

Reproduire : `cov <roms> <eeprom> <scénario> <images> <sortie.bin>` (exemple de `ddsom-game`) sur chaque scénario
(4 tâches : 45 min), `names > natives.txt`, puis `tools/couverture_fonctions.py decomp/index.tsv docs/atlas.md natives.txt
docs/code-mort.md <bitmaps>...`.

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

