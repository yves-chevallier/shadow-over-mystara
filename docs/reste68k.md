# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 40 scénarios, soit 894000 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **433802958 instructions 68000 exécutées** (hors attente active), soit 485.2 par image ;
* attente active, non comptée : 12981467934 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 640 exécutent encore du 68000, 625 sont entièrement natives, **1294 jamais atteintes** (88182 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 206950649.

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| K2 | Auto-test RAM (« RAM ERROR !! ») | 76512400 | 17.64 | 2 | 2 | 0 | 0 | 1912 (98.9 %), 182a (1.1 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 41142056 | 9.48 | 49 | 49 | 0 | 0 | 15216 (25.4 %), 156fa (23.6 %), 17814 (11.5 %) |
| O2 | Pools d'objets et listes triées | 39713187 | 9.15 | 41 | 2 | 26 | 13 | 1ee1a (94.1 %), 1ef6c (5.9 %) |
| B1 | Scrolling / caméras par scène | 32041505 | 7.39 | 42 | 19 | 4 | 19 | 33fe4 (37.7 %), 31616 (13.8 %), 33e20 (8.2 %) |
| C0e | EFFCT : effets | 11628651 | 2.68 | 170 | 108 | 0 | 62 | b9eba (63.7 %), b9ee6 (23.9 %), ba094 (5.4 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 10405693 | 2.40 | 69 | 47 | 15 | 7 | fbb0 (70.6 %), e76c (13.0 %), fbae (2.5 %) |
| V2 | Texte, rectangles et blocs de tuiles | 3645315 | 0.84 | 33 | 15 | 12 | 6 | 28d8 (58.5 %), 1386 (20.9 %), 2810 (14.4 %) |
| V4 | Construction de la liste de sprites (OBJ RAM) | 2623161 | 0.60 | 15 | 2 | 0 | 13 | 1de96 (99.8 %), 1de7c (0.2 %) |
| S1 | Son : wrappers de commandes + FIFO | 2197602 | 0.51 | 410 | 24 | 380 | 6 | edf80 (93.7 %), e7e10 (3.0 %), e662a (1.3 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 1970720 | 0.45 | 17 | 2 | 12 | 3 | 350 (99.9 %), 492 (0.1 %) |
| C04 | BOSS : boss | 1043020 | 0.24 | 353 | 32 | 1 | 320 | 7f19c (88.3 %), 81aa4 (6.7 %), 930a4 (1.2 %) |
| C02 | ENEMY : ennemis | 955997 | 0.22 | 205 | 59 | 0 | 146 | 67452 (62.7 %), 66740 (8.3 %), 661ce (3.9 %) |
| W3 | Scripts de scène (phases 16/28) | 891444 | 0.21 | 82 | 57 | 3 | 22 | f71fa (31.4 %), f68c8 (20.0 %), f7d86 (11.0 %) |
| M1 | Menu TEST (service) | 521903 | 0.12 | 76 | 23 | 0 | 53 | 29948 (29.7 %), 2bb0a (21.6 %), 2ca5e (17.4 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 452098 | 0.10 | 59 | 32 | 2 | 25 | 3525c (77.4 %), 39260 (4.8 %), 35620 (3.5 %) |
| O3 | Aides communes aux types (animations par type) | 306393 | 0.07 | 25 | 23 | 0 | 2 | 3bb9c (32.6 %), 3be02 (26.0 %), 3bb16 (9.4 %) |
| U2 | Petites routines / tables (rôle inconnu) | 224458 | 0.05 | 17 | 10 | 0 | 7 | 1ab56 (94.6 %), 1abe0 (2.4 %), 1ac24 (2.4 %) |
| U1 | RNG et files d'affichage (pop-ups) | 172532 | 0.04 | 18 | 5 | 13 | 0 | 19fd2 (55.8 %), 1a20e (28.7 %), 19f24 (9.6 %) |
| C1 | Crédits, pièces, start/continue | 141201 | 0.03 | 22 | 4 | 13 | 5 | e05ec (97.8 %), 14cb8 (1.2 %), e0538 (0.9 %) |
| C10 | SET : décors/animations de palette | 77754 | 0.02 | 131 | 19 | 3 | 109 | cc18e (87.1 %), c8312 (3.3 %), d9778 (1.7 %) |
| P3 | Comportement scénarisé des joueurs par scène | 40719 | 0.01 | 3 | 2 | 1 | 0 | 25260 (88.7 %), 25b20 (11.3 %) |
| V3 | Palettes et fondus | 40434 | 0.01 | 38 | 6 | 27 | 5 | 146c (61.3 %), 17ea (23.5 %), 1ced4 (13.2 %) |
| X2 | Dégâts, terrain, placement | 22282 | 0.01 | 19 | 10 | 0 | 9 | 3618 (35.2 %), 3636 (31.5 %), 3796 (16.1 %) |
| C08 | ESHL : tirs/attaques ennemis | 13029 | 0.00 | 127 | 24 | 1 | 102 | 9ef20 (57.3 %), a7438 (20.1 %), a4e84 (5.3 %) |
| M2 | EEPROM, configuration, comptabilité | 10225 | 0.00 | 60 | 11 | 13 | 36 | 2d30a (88.0 %), 2d5c6 (3.1 %), 2e4ec (3.0 %) |
| C0a | BOX : caisses/coffres (h) | 8774 | 0.00 | 7 | 3 | 0 | 4 | dabba (95.7 %), da152 (4.1 %), da04e (0.2 %) |
| C12 | PEFF : effets du joueur | 8466 | 0.00 | 107 | 9 | 26 | 72 | 517ba (81.1 %), 51c06 (8.9 %), 4b7a0 (4.1 %) |
| C14 | SOBJ : objets système/UI | 7251 | 0.00 | 25 | 8 | 0 | 17 | ddeaa (56.4 %), dfa4a (27.9 %), dfe26 (6.0 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 6175 | 0.00 | 5 | 4 | 0 | 1 | 18b8e (43.1 %), 19038 (35.0 %), 18a3e (20.9 %) |
| P5 | Code par personnage (6 × 2 variantes) | 6079 | 0.00 | 73 | 1 | 0 | 72 | 3e480 (100.0 %) |
| P1 | Initialisation des joueurs | 5961 | 0.00 | 23 | 9 | 4 | 10 | 35c0 (55.7 %), 3432 (12.9 %), 3308 (10.3 %) |
| C0c | ITEM : objets ramassables | 5358 | 0.00 | 14 | 5 | 0 | 9 | db6c0 (92.0 %), dcddc (6.5 %), dc5fe (0.7 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 4608 | 0.00 | 3 | 1 | 1 | 1 | 1c44a (100.0 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 1740 | 0.00 | 13 | 2 | 11 | 0 | c32 (54.0 %), c0c (46.0 %) |
| C06 | PSHL : tirs du joueur | 1622 | 0.00 | 39 | 5 | 12 | 22 | 53e48 (44.0 %), 54872 (24.4 %), 52578 (16.8 %) |
| F1 | Tâche de boot, avertissements région | 1474 | 0.00 | 5 | 3 | 2 | 0 | 133dc (97.7 %), 133f6 (1.2 %), 133f0 (1.1 %) |
| K3 | Entrées vers enregistrements joueur | 657 | 0.00 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| F6 | Interpréteur de scripts de scène (démos) | 348 | 0.00 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| P6 | Réactions aux coups, saisie des coups spéciaux | 17 | 0.00 | 9 | 1 | 2 | 6 | f18fa (100.0 %) |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 16 | 0 | 0 | 16 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 22 | 0 | 22 | 0 |  |
| X1 | Collisions (boîtes, coups, poussée) | 0 | 0.00 | 39 | 0 | 0 | 39 |  |
| W0 | Dispatch des 10 classes d'objets | 0 | 0.00 | 10 | 0 | 10 | 0 |  |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 0 | 0.00 | 1 | 0 | 0 | 1 |  |
| M3 | « Secret Test » (debug) et Block Test | 0 | 0.00 | 29 | 0 | 0 | 29 |  |
| F2 | Machine à modes (attract) | 0 | 0.00 | 0 | 0 | 0 | 0 |  |
| F5 | Curseur de sélection de personnage | 0 | 0.00 | 10 | 0 | 0 | 10 |  |
| F7 | Écrans titre / copyright | 0 | 0.00 | 5 | 0 | 1 | 4 |  |
| - | hors atlas | 0 | 0.00 | 0 | 0 | 0 | 0 |  |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 001912 | ram_test_block | K2 | 104 | 75692600 | 17.45 | 40 |
| 01ee1a | caseD_1ee1a | O2 | 338 | 37361021 | 8.61 | 40 |
| 033fe4 | FUN_00033fe4 | B1 | 90 | 12074096 | 2.78 | 39 |
| 015216 | FUN_00015216 | F3 | 62 | 10457126 | 2.41 | 39 |
| 0156fa | FUN_000156fa | F3 | 110 | 9716093 | 2.24 | 39 |
| 0b9eba | FUN_000b9eba | C0e | 44 | 7412280 | 1.71 | 3 |
| 00fbb0 | FUN_0000fbb0 | E1 | 148 | 7348993 | 1.69 | 29 |
| 017814 | FUN_00017814 | F3 | 92 | 4714953 | 1.09 | 39 |
| 0175a8 | FUN_000175a8 | F3 | 46 | 4514782 | 1.04 | 39 |
| 031616 | FUN_00031616 | B1 | 228 | 4428573 | 1.02 | 22 |
| 01728e | FUN_0001728e | F3 | 38 | 4236818 | 0.98 | 39 |
| 0b9ee6 | FUN_000b9ee6 | C0e | 70 | 2774668 | 0.64 | 3 |
| 033e20 | FUN_00033e20 | B1 | 32 | 2633724 | 0.61 | 39 |
| 01de96 | FUN_0001de96 | V4 | 126 | 2617729 | 0.60 | 40 |
| 030308 | FUN_00030308 | B1 | 126 | 2377259 | 0.55 | 11 |
| 01ef6c | FUN_0001ef6c | O2 | 338 | 2352166 | 0.54 | 15 |
| 0309e0 | FUN_000309e0 | B1 | 1014 | 2295990 | 0.53 | 39 |
| 0028d8 | FUN_000028d8 | V2 | 274 | 2133892 | 0.49 | 39 |
| 0edf80 | FUN_000edf80 | S1 | 196 | 2058828 | 0.47 | 40 |
| 016c1a | FUN_00016c1a | F3 | 628 | 2014529 | 0.46 | 39 |
| 000350 | reset | K1 | 322 | 1967800 | 0.45 | 40 |
| 031018 | FUN_00031018 | B1 | 174 | 1842133 | 0.42 | 30 |
| 0155b2 | FUN_000155b2 | F3 | 70 | 1822060 | 0.42 | 32 |
| 00e76c | FUN_0000e76c | E1 | 142 | 1348479 | 0.31 | 21 |
| 0332c0 | FUN_000332c0 | B1 | 16 | 1274320 | 0.29 | 39 |
| 033062 | caseD_f7 | B1 | 32 | 1135377 | 0.26 | 19 |
| 030fc8 | FUN_00030fc8 | B1 | 60 | 988104 | 0.23 | 30 |
| 07f19c | FUN_0007f19c | C04 | 92 | 920648 | 0.21 | 8 |
| 02ede0 | FUN_0002ede0 | B1 | 2360 | 905366 | 0.21 | 39 |
| 032108 | FUN_00032108 | B1 | 1616 | 897701 | 0.21 | 39 |
| 00182a | FUN_0000182a | K2 | 90 | 819800 | 0.19 | 40 |
| 001386 | FUN_00001386 | V2 | 134 | 760146 | 0.18 | 25 |
| 0177a0 | FUN_000177a0 | F3 | 46 | 754208 | 0.17 | 13 |
| 0ba094 | FUN_000ba094 | C0e | 28 | 628794 | 0.14 | 3 |
| 067452 | FUN_00067452 | C02 | 120 | 599627 | 0.14 | 9 |
| 017a94 | FUN_00017a94 | F3 | 76 | 529938 | 0.12 | 39 |
| 002810 | FUN_00002810 | V2 | 20 | 525454 | 0.12 | 26 |
| 01757e | FUN_0001757e | F3 | 22 | 440818 | 0.10 | 39 |
| 016b42 | FUN_00016b42 | F3 | 148 | 402102 | 0.09 | 36 |
| 0305fc | FUN_000305fc | B1 | 34 | 379464 | 0.09 | 20 |
| 03525c | FUN_0003525c | P4 | 44 | 349771 | 0.08 | 39 |
| 03403c | FUN_0003403c | B1 | 90 | 286436 | 0.07 | 26 |
| 0f71fa | FUN_000f71fa | W3 | 76 | 280183 | 0.06 | 12 |
| 00fbae | FUN_0000fbae | E1 | 2 | 265250 | 0.06 | 29 |
| 007f0c | FUN_00007f0c | E1 | 220 | 262671 | 0.06 | 23 |
| 033290 | FUN_00033290 | B1 | 38 | 229030 | 0.05 | 12 |
| 0170ca | FUN_000170ca | F3 | 22 | 227109 | 0.05 | 39 |
| 01ab56 | FUN_0001ab56 | U2 | 138 | 212298 | 0.05 | 7 |
| 00eff2 | FUN_0000eff2 | E1 | 120 | 198791 | 0.05 | 11 |
| 03147c | FUN_0003147c | B1 | 220 | 198620 | 0.05 | 9 |
| 0f68c8 | FUN_000f68c8 | W3 | 58 | 178021 | 0.04 | 12 |
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 176735 | 0.04 | 6 |
| 0173ee | FUN_000173ee | F3 | 18 | 166625 | 0.04 | 14 |
| 00f0e8 | FUN_0000f0e8 | E1 | 48 | 164582 | 0.04 | 23 |
| 029948 | FUN_00029948 | M1 | 76 | 154899 | 0.04 | 1 |
| 015444 | FUN_00015444 | F3 | 164 | 150296 | 0.03 | 5 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 149545 | 0.03 | 6 |
| 010428 | FUN_00010428 | E1 | 40 | 148689 | 0.03 | 2 |
| 017b40 | FUN_00017b40 | F3 | 86 | 140208 | 0.03 | 39 |
| 0e05ec | FUN_000e05ec | C1 | 426 | 138046 | 0.03 | 4 |
| 016aec | FUN_00016aec | F3 | 126 | 137887 | 0.03 | 36 |
| 017236 | FUN_00017236 | F3 | 40 | 131290 | 0.03 | 39 |
| 000d5e | FUN_00000d5e | V2 | 108 | 118510 | 0.03 | 35 |
| 02bb0a | FUN_0002bb0a | M1 | 14 | 112872 | 0.03 | 1 |
| 00e886 | FUN_0000e886 | E1 | 242 | 111380 | 0.03 | 8 |
| 017034 | FUN_00017034 | F3 | 72 | 110380 | 0.03 | 39 |
| 03bb9c | FUN_0003bb9c | O3 | 170 | 100036 | 0.02 | 24 |
| 0f7d86 | FUN_000f7d86 | W3 | 116 | 98283 | 0.02 | 8 |
| 019fd2 | FUN_00019fd2 | U1 | 72 | 96302 | 0.02 | 25 |
| 02ca5e | FUN_0002ca5e | M1 | 72 | 91061 | 0.02 | 1 |
| 029994 | FUN_00029994 | M1 | 76 | 89025 | 0.02 | 1 |
| 011654 | FUN_00011654 | E1 | 58 | 82625 | 0.02 | 4 |
| 0153ba | FUN_000153ba | F3 | 138 | 80994 | 0.02 | 3 |
| 03be02 | FUN_0003be02 | O3 | 54 | 79615 | 0.02 | 22 |
| 066740 | FUN_00066740 | C02 | 144 | 79488 | 0.02 | 8 |
| 0168a2 | FUN_000168a2 | F3 | 92 | 77355 | 0.02 | 39 |
| 01161c | FUN_0001161c | E1 | 56 | 72031 | 0.02 | 4 |
| 081aa4 | FUN_00081aa4 | C04 | 54 | 69532 | 0.02 | 12 |
| 0115e2 | FUN_000115e2 | E1 | 58 | 68322 | 0.02 | 4 |
| 0cc18e | FUN_000cc18e | C10 | 52 | 67738 | 0.02 | 11 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 640 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 320 | 16344 |
| C10 | SET : décors/animations de palette | 109 | 11018 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| C02 | ENEMY : ennemis | 146 | 6622 |
| C08 | ESHL : tirs/attaques ennemis | 102 | 6340 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C12 | PEFF : effets du joueur | 72 | 5440 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| C0e | EFFCT : effets | 62 | 2904 |
| M1 | Menu TEST (service) | 53 | 2386 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| P4 | Noyau joueur (états, 26 actions génériques) | 25 | 1998 |
| C06 | PSHL : tirs du joueur | 22 | 1752 |
| M2 | EEPROM, configuration, comptabilité | 36 | 1696 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 6 | 1500 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C0c | ITEM : objets ramassables | 9 | 1254 |
| B1 | Scrolling / caméras par scène | 19 | 888 |
| C14 | SOBJ : objets système/UI | 17 | 760 |
| W3 | Scripts de scène (phases 16/28) | 22 | 756 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| F4 | Exécuteur d'écrans fixes (logos) | 1 | 666 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| X2 | Dégâts, terrain, placement | 9 | 520 |
| C1 | Crédits, pièces, start/continue | 5 | 476 |
| O2 | Pools d'objets et listes triées | 13 | 342 |
| C0a | BOX : caisses/coffres (h) | 4 | 342 |
| V3 | Palettes et fondus | 5 | 330 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 3 | 318 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 7 | 290 |
| P1 | Initialisation des joueurs | 10 | 178 |
| U2 | Petites routines / tables (rôle inconnu) | 7 | 150 |
| S1 | Son : wrappers de commandes + FIFO | 6 | 132 |
| V2 | Texte, rectangles et blocs de tuiles | 6 | 108 |
| F7 | Écrans titre / copyright | 4 | 108 |
| O3 | Aides communes aux types (animations par type) | 2 | 80 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 46 |
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/bot30k.txt | 30000 | 493104751 | 16436.8 |
| scenarios/bot_2p.txt | 30000 | 492113382 | 16403.8 |
| scenarios/bot_char0.txt | 30000 | 492642476 | 16421.4 |
| scenarios/bot_char3.txt | 30000 | 492917095 | 16430.6 |
| scenarios/cov/cov_2p_a.txt | 30000 | 657885961 | 21929.5 |
| scenarios/cov/cov_2p_b.txt | 30000 | 99323730 | 3310.8 |
| scenarios/cov/cov_2p_c.txt | 30000 | 600489358 | 20016.3 |
| scenarios/cov/cov_3p.txt | 30000 | 758222403 | 25274.1 |
| scenarios/cov/cov_4p_a.txt | 30000 | 812999994 | 27100.0 |
| scenarios/cov/cov_4p_b.txt | 30000 | 101938661 | 3398.0 |
| scenarios/cov/cov_4p_c.txt | 30000 | 59804418 | 1993.5 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 69007741 | 2300.3 |
| scenarios/cov/cov_credits_2pieces.txt | 6000 | 104984129 | 17497.4 |
| scenarios/cov/cov_credits_gratuit.txt | 6000 | 102153672 | 17025.6 |
| scenarios/cov/cov_credits_gros.txt | 6000 | 104978763 | 17496.5 |
| scenarios/cov/cov_credits_gros4.txt | 6000 | 104913679 | 17485.6 |
| scenarios/cov/cov_credits_gros_commun.txt | 6000 | 104956112 | 17492.7 |
| scenarios/cov/cov_credits_sep2.txt | 6000 | 104979743 | 17496.6 |
| scenarios/cov/cov_credits_sep4.txt | 6000 | 104915478 | 17485.9 |
| scenarios/cov/cov_credits_sep4_p1.txt | 6000 | 101859438 | 16976.6 |
| scenarios/cov/cov_credits_sep4_p2.txt | 6000 | 101769723 | 16961.6 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 511392236 | 17046.4 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 476280857 | 15876.0 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 99252948 | 3308.4 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 317591583 | 10586.4 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 506700906 | 16890.0 |
| scenarios/cov/cov_service.txt | 30000 | 471617398 | 15720.6 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 728513716 | 24283.8 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 43538492 | 1451.3 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 686980865 | 22899.4 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 535750592 | 17858.4 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 728288673 | 24276.3 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 718788305 | 23959.6 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 654176924 | 21805.9 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 52281443 | 1742.7 |
| scenarios/modes/attract.txt | 12000 | 69007741 | 5750.6 |
| scenarios/modes/ending.txt | 12000 | 233962208 | 19496.9 |
| scenarios/modes/ending_end.txt | 12000 | 233211672 | 19434.3 |
| scenarios/modes/gameover.txt | 12000 | 212965885 | 17747.2 |
| scenarios/modes/screens.txt | 12000 | 69007741 | 5750.6 |
