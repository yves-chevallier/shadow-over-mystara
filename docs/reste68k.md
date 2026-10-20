# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 159 scénarios, soit 2318000 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **775852477 instructions 68000 exécutées** (hors attente active), soit 334.7 par image ;
* attente active, non comptée : 36417688167 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 630 exécutent encore du 68000, 641 sont entièrement natives, **1288 jamais atteintes** (90128 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 178817105.

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| K2 | Auto-test RAM (« RAM ERROR !! ») | 304136790 | 39.20 | 2 | 2 | 0 | 0 | 1912 (98.9 %), 182a (1.1 %) |
| O2 | Pools d'objets et listes triées | 116034187 | 14.96 | 41 | 2 | 26 | 13 | 1ee1a (95.4 %), 1ef6c (4.6 %) |
| B1 | Scrolling / caméras par scène | 93915164 | 12.10 | 42 | 19 | 4 | 19 | 33fe4 (37.9 %), 31616 (11.7 %), 30308 (9.2 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 38673598 | 4.98 | 69 | 48 | 15 | 6 | fbb0 (68.2 %), e76c (13.2 %), 7f0c (2.6 %) |
| C0e | EFFCT : effets | 14184046 | 1.83 | 170 | 112 | 0 | 58 | b9eba (58.5 %), b9ee6 (20.4 %), ba094 (5.3 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 7833612 | 1.01 | 17 | 2 | 12 | 3 | 350 (99.9 %), 492 (0.1 %) |
| V4 | Construction de la liste de sprites (OBJ RAM) | 7529228 | 0.97 | 15 | 1 | 1 | 13 | 1de96 (100.0 %) |
| S1 | Son : wrappers de commandes + FIFO | 6445960 | 0.83 | 410 | 25 | 379 | 6 | edf80 (94.7 %), e7e10 (2.4 %), e662a (1.2 %) |
| C02 | ENEMY : ennemis | 1637557 | 0.21 | 205 | 66 | 0 | 139 | 67452 (37.2 %), 63a98 (8.9 %), 71dde (8.2 %) |
| W3 | Scripts de scène (phases 16/28) | 1637088 | 0.21 | 82 | 66 | 3 | 13 | f7d86 (28.5 %), f71fa (23.9 %), f68c8 (12.3 %) |
| C04 | BOSS : boss | 1616384 | 0.21 | 353 | 36 | 1 | 316 | 7f19c (87.1 %), 81aa4 (7.1 %), 833b6 (1.9 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 1033853 | 0.13 | 59 | 33 | 2 | 24 | 3525c (56.1 %), 39260 (13.7 %), 392b2 (7.9 %) |
| O3 | Aides communes aux types (animations par type) | 779026 | 0.10 | 25 | 24 | 0 | 1 | 3bb9c (31.2 %), 3be02 (27.4 %), 3bb16 (8.1 %) |
| M1 | Menu TEST (service) | 521903 | 0.07 | 76 | 23 | 0 | 53 | 29948 (29.7 %), 2bb0a (21.6 %), 2ca5e (17.4 %) |
| C10 | SET : décors/animations de palette | 188658 | 0.02 | 131 | 21 | 3 | 107 | cc18e (86.5 %), cdedc (3.3 %), c8312 (2.3 %) |
| C1 | Crédits, pièces, start/continue | 181988 | 0.02 | 22 | 4 | 13 | 5 | e05ec (95.1 %), 14cb8 (3.8 %), e0538 (0.9 %) |
| X2 | Dégâts, terrain, placement | 123221 | 0.02 | 19 | 12 | 0 | 7 | 3636 (49.9 %), 3618 (33.2 %), 3796 (11.4 %) |
| V3 | Palettes et fondus | 122001 | 0.02 | 38 | 6 | 27 | 5 | 146c (67.9 %), 17ea (15.5 %), 1ced4 (15.0 %) |
| V2 | Texte, rectangles et blocs de tuiles | 71145 | 0.01 | 33 | 9 | 19 | 5 | f4a (70.1 %), 2860 (15.1 %), 2824 (7.8 %) |
| C08 | ESHL : tirs/attaques ennemis | 65303 | 0.01 | 127 | 36 | 1 | 90 | 9ef20 (20.5 %), a71c2 (17.9 %), a731c (13.5 %) |
| P3 | Comportement scénarisé des joueurs par scène | 50800 | 0.01 | 3 | 2 | 1 | 0 | 25260 (86.8 %), 25b20 (13.2 %) |
| C0a | BOX : caisses/coffres (h) | 40628 | 0.01 | 7 | 4 | 0 | 3 | dabba (84.2 %), d9cea (13.8 %), da152 (2.0 %) |
| M2 | EEPROM, configuration, comptabilité | 38398 | 0.00 | 60 | 11 | 13 | 36 | 2d30a (93.2 %), 2e4ec (2.1 %), 2dc9a (1.2 %) |
| P1 | Initialisation des joueurs | 27528 | 0.00 | 23 | 9 | 4 | 10 | 35c0 (89.6 %), 3432 (2.9 %), 3308 (2.4 %) |
| C12 | PEFF : effets du joueur | 25823 | 0.00 | 107 | 11 | 26 | 70 | 517ba (90.7 %), 51c06 (3.4 %), 4b7a0 (1.6 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 18968 | 0.00 | 3 | 2 | 0 | 1 | 1c44a (99.6 %), 1c86e (0.4 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 18889 | 0.00 | 49 | 4 | 0 | 45 | 18082 (56.4 %), 1507a (36.2 %), 180c8 (7.3 %) |
| C14 | SOBJ : objets système/UI | 18354 | 0.00 | 25 | 9 | 0 | 16 | ddeaa (53.9 %), dfa4a (35.4 %), dfe26 (4.4 %) |
| P5 | Code par personnage (6 × 2 variantes) | 18165 | 0.00 | 73 | 1 | 0 | 72 | 3e480 (100.0 %) |
| C0c | ITEM : objets ramassables | 15075 | 0.00 | 14 | 5 | 0 | 9 | db6c0 (97.1 %), dcddc (2.3 %), dc5fe (0.3 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 8046 | 0.00 | 5 | 3 | 1 | 1 | 18b8e (66.4 %), 18a3e (32.1 %), 18a0e (1.5 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 5870 | 0.00 | 13 | 2 | 11 | 0 | c32 (63.4 %), c0c (36.6 %) |
| F1 | Tâche de boot, avertissements région | 5840 | 0.00 | 5 | 3 | 2 | 0 | 133dc (98.0 %), 133f6 (1.0 %), 133f0 (1.0 %) |
| U1 | RNG et files d'affichage (pop-ups) | 4386 | 0.00 | 18 | 1 | 13 | 4 | 19eb0 (100.0 %) |
| U2 | Petites routines / tables (rôle inconnu) | 2112 | 0.00 | 17 | 6 | 10 | 1 | 1ad2a (75.0 %), 1ac68 (12.1 %), 1ac86 (11.6 %) |
| C06 | PSHL : tirs du joueur | 2026 | 0.00 | 39 | 6 | 12 | 21 | 53e48 (48.7 %), 54872 (21.7 %), 52578 (16.6 %) |
| W0 | Dispatch des 10 classes d'objets | 2023 | 0.00 | 10 | 1 | 9 | 0 | 39e86 (100.0 %) |
| K3 | Entrées vers enregistrements joueur | 954 | 0.00 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| F6 | Interpréteur de scripts de scène (démos) | 656 | 0.00 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| P6 | Réactions aux coups, saisie des coups spéciaux | 119 | 0.00 | 9 | 1 | 2 | 6 | f18fa (100.0 %) |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 16 | 0 | 0 | 16 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 22 | 0 | 22 | 0 |  |
| X1 | Collisions (boîtes, coups, poussée) | 0 | 0.00 | 39 | 0 | 0 | 39 |  |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 0 | 0.00 | 1 | 0 | 0 | 1 |  |
| M3 | « Secret Test » (debug) et Block Test | 0 | 0.00 | 29 | 0 | 0 | 29 |  |
| F2 | Machine à modes (attract) | 0 | 0.00 | 0 | 0 | 0 | 0 |  |
| F5 | Curseur de sélection de personnage | 0 | 0.00 | 10 | 0 | 0 | 10 |  |
| F7 | Écrans titre / copyright | 0 | 0.00 | 5 | 0 | 1 | 4 |  |
| - | hors atlas | 0 | 0.00 | 0 | 0 | 0 | 0 |  |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 001912 | ram_test_block | K2 | 104 | 300878085 | 38.78 | 159 |
| 01ee1a | caseD_1ee1a | O2 | 338 | 110666503 | 14.26 | 159 |
| 033fe4 | FUN_00033fe4 | B1 | 90 | 35616752 | 4.59 | 155 |
| 00fbb0 | FUN_0000fbb0 | E1 | 148 | 26368208 | 3.40 | 123 |
| 031616 | FUN_00031616 | B1 | 228 | 10969588 | 1.41 | 88 |
| 030308 | FUN_00030308 | B1 | 126 | 8678911 | 1.12 | 14 |
| 0b9eba | FUN_000b9eba | C0e | 44 | 8296688 | 1.07 | 4 |
| 000350 | reset | K1 | 322 | 7822005 | 1.01 | 159 |
| 01de96 | FUN_0001de96 | V4 | 126 | 7529228 | 0.97 | 159 |
| 033e20 | FUN_00033e20 | B1 | 32 | 7385682 | 0.95 | 158 |
| 0309e0 | FUN_000309e0 | B1 | 1014 | 6131010 | 0.79 | 155 |
| 0edf80 | FUN_000edf80 | S1 | 196 | 6101946 | 0.79 | 159 |
| 031018 | FUN_00031018 | B1 | 174 | 5889836 | 0.76 | 70 |
| 01ef6c | FUN_0001ef6c | O2 | 338 | 5367684 | 0.69 | 42 |
| 00e76c | FUN_0000e76c | E1 | 142 | 5091099 | 0.66 | 101 |
| 033062 | caseD_f7 | B1 | 32 | 3706749 | 0.48 | 74 |
| 0332c0 | FUN_000332c0 | B1 | 16 | 3478345 | 0.45 | 158 |
| 00182a | FUN_0000182a | K2 | 90 | 3258705 | 0.42 | 159 |
| 030fc8 | FUN_00030fc8 | B1 | 60 | 3195531 | 0.41 | 65 |
| 0b9ee6 | FUN_000b9ee6 | C0e | 70 | 2897968 | 0.37 | 4 |
| 02ede0 | FUN_0002ede0 | B1 | 2360 | 2501012 | 0.32 | 158 |
| 032108 | FUN_00032108 | B1 | 1616 | 2490162 | 0.32 | 158 |
| 07f19c | FUN_0007f19c | C04 | 92 | 1408125 | 0.18 | 15 |
| 03147c | FUN_0003147c | B1 | 220 | 1206450 | 0.16 | 15 |
| 0305fc | FUN_000305fc | B1 | 34 | 1168552 | 0.15 | 40 |
| 007f0c | FUN_00007f0c | E1 | 220 | 1016002 | 0.13 | 102 |
| 00fbae | FUN_0000fbae | E1 | 2 | 914265 | 0.12 | 120 |
| 0ba094 | FUN_000ba094 | C0e | 28 | 753072 | 0.10 | 4 |
| 03403c | FUN_0003403c | B1 | 90 | 722712 | 0.09 | 124 |
| 010428 | FUN_00010428 | E1 | 40 | 663957 | 0.09 | 3 |
| 00eff2 | FUN_0000eff2 | E1 | 120 | 625550 | 0.08 | 16 |
| 067452 | FUN_00067452 | C02 | 120 | 608451 | 0.08 | 10 |
| 00f0e8 | FUN_0000f0e8 | E1 | 48 | 598978 | 0.08 | 111 |
| 00e886 | FUN_0000e886 | E1 | 242 | 584527 | 0.08 | 22 |
| 03525c | FUN_0003525c | P4 | 44 | 580355 | 0.07 | 158 |
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 492635 | 0.06 | 11 |
| 0f7d86 | FUN_000f7d86 | W3 | 116 | 467321 | 0.06 | 19 |
| 0b10c8 | FUN_000b10c8 | C0e | 82 | 453561 | 0.06 | 116 |
| 033290 | FUN_00033290 | B1 | 38 | 431600 | 0.06 | 17 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 416845 | 0.05 | 11 |
| 0f71fa | FUN_000f71fa | W3 | 76 | 391147 | 0.05 | 25 |
| 0115e2 | FUN_000115e2 | E1 | 58 | 307224 | 0.04 | 10 |
| 0104a2 | FUN_000104a2 | E1 | 58 | 256901 | 0.03 | 36 |
| 00fda2 | FUN_0000fda2 | E1 | 146 | 255431 | 0.03 | 13 |
| 00a470 | FUN_0000a470 | E1 | 28 | 249664 | 0.03 | 105 |
| 03bb9c | FUN_0003bb9c | O3 | 170 | 243289 | 0.03 | 84 |
| 011654 | FUN_00011654 | E1 | 58 | 239448 | 0.03 | 10 |
| 007fe8 | FUN_00007fe8 | E1 | 4 | 227432 | 0.03 | 107 |
| 03be02 | FUN_0003be02 | O3 | 54 | 213276 | 0.03 | 82 |
| 01161c | FUN_0001161c | E1 | 56 | 206335 | 0.03 | 10 |
| 030300 | FUN_00030300 | B1 | 8 | 202746 | 0.03 | 50 |
| 0f68c8 | FUN_000f68c8 | W3 | 58 | 200628 | 0.03 | 25 |
| 0e05ec | FUN_000e05ec | C1 | 426 | 173138 | 0.02 | 5 |
| 007ef4 | FUN_00007ef4 | E1 | 24 | 169769 | 0.02 | 35 |
| 0103e0 | FUN_000103e0 | E1 | 44 | 167424 | 0.02 | 32 |
| 0cc18e | FUN_000cc18e | C10 | 52 | 163217 | 0.02 | 18 |
| 029948 | FUN_00029948 | M1 | 76 | 154899 | 0.02 | 1 |
| 0e7e10 | FUN_000e7e10 | S1 | 58 | 154014 | 0.02 | 113 |
| 063a98 | FUN_00063a98 | C02 | 70 | 146296 | 0.02 | 14 |
| 039260 | player_walk_speed | P4 | 62 | 141376 | 0.02 | 5 |
| 071dde | FUN_00071dde | C02 | 38 | 134126 | 0.02 | 7 |
| 081aa4 | FUN_00081aa4 | C04 | 54 | 114564 | 0.01 | 18 |
| 02bb0a | FUN_0002bb0a | M1 | 14 | 112872 | 0.01 | 1 |
| 00e914 | FUN_0000e914 | E1 | 42 | 110912 | 0.01 | 31 |
| 0b03c6 | FUN_000b03c6 | C0e | 102 | 97020 | 0.01 | 8 |
| 0f683c | FUN_000f683c | W3 | 42 | 96978 | 0.01 | 26 |
| 0aa96c | FUN_000aa96c | C0e | 84 | 96116 | 0.01 | 106 |
| 065ab6 | FUN_00065ab6 | C02 | 18 | 92072 | 0.01 | 10 |
| 02ca5e | FUN_0002ca5e | M1 | 72 | 91061 | 0.01 | 1 |
| 029994 | FUN_00029994 | M1 | 76 | 89025 | 0.01 | 1 |
| 0331ba | FUN_000331ba | B1 | 16 | 84132 | 0.01 | 12 |
| 00146c | FUN_0000146c | V3 | 78 | 82816 | 0.01 | 159 |
| 0392b2 | FUN_000392b2 | P4 | 50 | 82120 | 0.01 | 73 |
| 066740 | FUN_00066740 | C02 | 144 | 82028 | 0.01 | 9 |
| 0f6d94 | FUN_000f6d94 | W3 | 42 | 80615 | 0.01 | 24 |
| 0b5f8e | FUN_000b5f8e | C0e | 40 | 78080 | 0.01 | 12 |
| 057bee | FUN_00057bee | C02 | 70 | 77354 | 0.01 | 10 |
| 0e662a | FUN_000e662a | S1 | 66 | 77220 | 0.01 | 105 |
| 0abea4 | FUN_000abea4 | C0e | 36 | 69790 | 0.01 | 3 |
| 0f67fa | FUN_000f67fa | W3 | 66 | 65286 | 0.01 | 25 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 630 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 316 | 16240 |
| C10 | SET : décors/animations de palette | 107 | 10934 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| C02 | ENEMY : ennemis | 139 | 6466 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 90 | 5538 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 45 | 3874 |
| C0e | EFFCT : effets | 58 | 2718 |
| M1 | Menu TEST (service) | 53 | 2386 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| P4 | Noyau joueur (états, 26 actions génériques) | 24 | 1934 |
| C06 | PSHL : tirs du joueur | 21 | 1726 |
| M2 | EEPROM, configuration, comptabilité | 36 | 1696 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 6 | 1500 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C0c | ITEM : objets ramassables | 9 | 1254 |
| B1 | Scrolling / caméras par scène | 19 | 888 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| C14 | SOBJ : objets système/UI | 16 | 736 |
| F4 | Exécuteur d'écrans fixes (logos) | 1 | 666 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| X2 | Dégâts, terrain, placement | 7 | 506 |
| W3 | Scripts de scène (phases 16/28) | 13 | 476 |
| C1 | Crédits, pièces, start/continue | 5 | 476 |
| O2 | Pools d'objets et listes triées | 13 | 342 |
| V3 | Palettes et fondus | 5 | 330 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 3 | 318 |
| U1 | RNG et files d'affichage (pop-ups) | 4 | 296 |
| C0a | BOX : caisses/coffres (h) | 3 | 264 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 6 | 220 |
| P1 | Initialisation des joueurs | 10 | 178 |
| S1 | Son : wrappers de commandes + FIFO | 6 | 132 |
| F7 | Écrans titre / copyright | 4 | 108 |
| V2 | Texte, rectangles et blocs de tuiles | 5 | 96 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 46 |
| O3 | Aides communes aux types (animations par type) | 1 | 38 |
| U2 | Petites routines / tables (rôle inconnu) | 1 | 10 |
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 247362135 | 20613.5 |
| scenarios/boss/boss10_scene66.txt | 12000 | 109264771 | 9105.4 |
| scenarios/boss/boss11_scene42.txt | 12000 | 195371717 | 16281.0 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 217726586 | 18143.9 |
| scenarios/boss/boss12_scene40.txt | 12000 | 195543219 | 16295.3 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 216445787 | 18037.1 |
| scenarios/boss/boss15_scene46.txt | 12000 | 186732090 | 15561.0 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 209094848 | 17424.6 |
| scenarios/boss/boss17_scene13.txt | 12000 | 201844304 | 16820.4 |
| scenarios/boss/boss18_scene26.txt | 12000 | 214925824 | 17910.5 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 241293563 | 20107.8 |
| scenarios/boss/boss19_scene32.txt | 12000 | 202702622 | 16891.9 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 225696001 | 18808.0 |
| scenarios/boss/boss21_scene44.txt | 12000 | 214331704 | 17861.0 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 237694863 | 19807.9 |
| scenarios/boss/boss5_scene58.txt | 12000 | 211516471 | 17626.4 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 229372626 | 19114.4 |
| scenarios/boss/boss6_scene24.txt | 12000 | 207698700 | 17308.2 |
| scenarios/boss/boss8_scene64.txt | 12000 | 224043393 | 18670.3 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 240299988 | 20025.0 |
| scenarios/bot30k.txt | 30000 | 490429524 | 16347.7 |
| scenarios/bot_2p.txt | 30000 | 490079811 | 16336.0 |
| scenarios/bot_char0.txt | 30000 | 490327845 | 16344.3 |
| scenarios/bot_char1.txt | 30000 | 491685825 | 16389.5 |
| scenarios/bot_char2.txt | 30000 | 121824527 | 4060.8 |
| scenarios/bot_char3.txt | 30000 | 490573490 | 16352.4 |
| scenarios/bot_char4.txt | 30000 | 490238419 | 16341.3 |
| scenarios/bot_char5.txt | 30000 | 490210154 | 16340.3 |
| scenarios/cov/cov_2p_a.txt | 30000 | 651079913 | 21702.7 |
| scenarios/cov/cov_2p_b.txt | 30000 | 98818258 | 3293.9 |
| scenarios/cov/cov_2p_c.txt | 30000 | 593038078 | 19767.9 |
| scenarios/cov/cov_3p.txt | 30000 | 743188267 | 24772.9 |
| scenarios/cov/cov_4p_a.txt | 30000 | 799690491 | 26656.3 |
| scenarios/cov/cov_4p_b.txt | 30000 | 101100855 | 3370.0 |
| scenarios/cov/cov_4p_c.txt | 30000 | 59086012 | 1969.5 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 68097272 | 2269.9 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 477837015 | 15927.9 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 314464284 | 10482.1 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 477731325 | 15924.4 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 477094939 | 15903.2 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 477723232 | 15924.1 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 477837039 | 15927.9 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 477281174 | 15909.4 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 473879034 | 15796.0 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 473073468 | 15769.1 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 503399303 | 16780.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 468739816 | 15624.7 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 98442348 | 3281.4 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 312309915 | 10410.3 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 497224549 | 16574.2 |
| scenarios/cov/cov_service.txt | 30000 | 471617337 | 15720.6 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 719341308 | 23978.0 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 43331177 | 1444.4 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 677747920 | 22591.6 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 530663647 | 17688.8 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 719103780 | 23970.1 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 710639639 | 23688.0 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 644842040 | 21494.7 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 51887069 | 1729.6 |
| scenarios/enemy/elf_scene92.txt | 12000 | 186181596 | 15515.1 |
| scenarios/enemy/elf_scene94.txt | 12000 | 189385939 | 15782.2 |
| scenarios/enemy/elf_scene96.txt | 12000 | 186204775 | 15517.1 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 84246083 | 7020.5 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 186397412 | 15533.1 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 201844304 | 16820.4 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 211564149 | 17630.3 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 97938142 | 8161.5 |
| scenarios/enemy/trog_scene101.txt | 12000 | 42555974 | 3546.3 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 186175204 | 15514.6 |
| scenarios/enemy/trog_scene104.txt | 12000 | 191401545 | 15950.1 |
| scenarios/extra/mash_2p.txt | 12000 | 211771921 | 17647.7 |
| scenarios/extra/mash_c0.txt | 12000 | 211857145 | 17654.8 |
| scenarios/extra/mash_c0b.txt | 12000 | 210669474 | 17555.8 |
| scenarios/extra/mash_c1b.txt | 12000 | 211615027 | 17634.6 |
| scenarios/extra/mash_c2.txt | 12000 | 210718025 | 17559.8 |
| scenarios/extra/mash_c2b.txt | 12000 | 124156154 | 10346.3 |
| scenarios/extra/mash_c3.txt | 12000 | 211876398 | 17656.4 |
| scenarios/extra/mash_c3b.txt | 12000 | 211932594 | 17661.0 |
| scenarios/extra/mash_c3c.txt | 12000 | 210973942 | 17581.2 |
| scenarios/extra/mash_c3d.txt | 12000 | 210954889 | 17579.6 |
| scenarios/extra/mash_c4.txt | 12000 | 210951854 | 17579.3 |
| scenarios/extra/mash_c4b.txt | 12000 | 210710146 | 17559.2 |
| scenarios/extra/mash_c4c.txt | 12000 | 210942128 | 17578.5 |
| scenarios/extra/mash_c5.txt | 12000 | 212006502 | 17667.2 |
| scenarios/extra/mash_c5b.txt | 12000 | 212293942 | 17691.2 |
| scenarios/modes/attract.txt | 12000 | 68097272 | 5674.8 |
| scenarios/modes/ending.txt | 12000 | 232017193 | 19334.8 |
| scenarios/modes/ending_end.txt | 12000 | 231296046 | 19274.7 |
| scenarios/modes/gameover.txt | 12000 | 210921211 | 17576.8 |
| scenarios/modes/screens.txt | 12000 | 68097272 | 5674.8 |
| scenarios/scene/scene_01.txt | 6500 | 124785419 | 19197.8 |
| scenarios/scene/scene_02.txt | 6500 | 99913816 | 15371.4 |
| scenarios/scene/scene_03.txt | 6500 | 99251770 | 15269.5 |
| scenarios/scene/scene_04.txt | 6500 | 122724869 | 18880.7 |
| scenarios/scene/scene_09.txt | 6500 | 104310125 | 16047.7 |
| scenarios/scene/scene_10.txt | 6500 | 105498904 | 16230.6 |
| scenarios/scene/scene_105.txt | 6500 | 129046104 | 19853.2 |
| scenarios/scene/scene_109.txt | 6500 | 99135840 | 15251.7 |
| scenarios/scene/scene_111.txt | 6500 | 126791062 | 19506.3 |
| scenarios/scene/scene_14.txt | 6500 | 99057620 | 15239.6 |
| scenarios/scene/scene_24.txt | 6500 | 120434392 | 18528.4 |
| scenarios/scene/scene_25.txt | 6500 | 100206184 | 15416.3 |
| scenarios/scene/scene_26.txt | 6500 | 120935248 | 18605.4 |
| scenarios/scene/scene_27.txt | 6500 | 112858174 | 17362.8 |
| scenarios/scene/scene_28.txt | 6500 | 118311730 | 18201.8 |
| scenarios/scene/scene_32.txt | 6500 | 116286414 | 17890.2 |
| scenarios/scene/scene_33.txt | 6500 | 99534719 | 15313.0 |
| scenarios/scene/scene_34.txt | 6500 | 44153825 | 6792.9 |
| scenarios/scene/scene_35.txt | 6500 | 101121297 | 15557.1 |
| scenarios/scene/scene_38.txt | 6500 | 109573510 | 16857.5 |
| scenarios/scene/scene_39.txt | 6500 | 109691850 | 16875.7 |
| scenarios/scene/scene_40.txt | 6500 | 107272635 | 16503.5 |
| scenarios/scene/scene_41.txt | 6500 | 136635784 | 21020.9 |
| scenarios/scene/scene_42.txt | 6500 | 107095230 | 16476.2 |
| scenarios/scene/scene_44.txt | 6500 | 126791062 | 19506.3 |
| scenarios/scene/scene_47.txt | 6500 | 103289239 | 15890.7 |
| scenarios/scene/scene_54.txt | 6500 | 123898852 | 19061.4 |
| scenarios/scene/scene_58.txt | 6500 | 124116498 | 19094.8 |
| scenarios/scene/scene_59.txt | 6500 | 99667846 | 15333.5 |
| scenarios/scene/scene_64.txt | 6500 | 136448337 | 20992.1 |
| scenarios/scene/scene_65.txt | 6500 | 160055940 | 24624.0 |
| scenarios/scene/scene_66.txt | 6500 | 142974852 | 21996.1 |
| scenarios/scene/scene_67.txt | 6500 | 100085509 | 15397.8 |
| scenarios/scene/scene_90.txt | 6500 | 99770793 | 15349.4 |
| scenarios/scene_end/end_02.txt | 9000 | 139201091 | 15466.8 |
| scenarios/scene_end/end_03.txt | 9000 | 161747352 | 17971.9 |
| scenarios/scene_end/end_04.txt | 9000 | 161806069 | 17978.5 |
| scenarios/scene_end/end_05.txt | 9000 | 149214023 | 16579.3 |
| scenarios/scene_end/end_06.txt | 9000 | 155832597 | 17314.7 |
| scenarios/scene_end/end_07.txt | 9000 | 149308118 | 16589.8 |
| scenarios/scene_end/end_09.txt | 9000 | 149433229 | 16603.7 |
| scenarios/scene_end/end_10.txt | 9000 | 144765486 | 16085.1 |
| scenarios/scene_end/end_105.txt | 9000 | 339172793 | 37685.9 |
| scenarios/scene_end/end_11.txt | 9000 | 342662760 | 38073.6 |
| scenarios/scene_end/end_13.txt | 9000 | 139798674 | 15533.2 |
| scenarios/scene_end/end_14.txt | 9000 | 138392112 | 15376.9 |
| scenarios/scene_end/end_20.txt | 9000 | 149923116 | 16658.1 |
| scenarios/scene_end/end_24.txt | 9000 | 160208504 | 17800.9 |
| scenarios/scene_end/end_26.txt | 9000 | 160112733 | 17790.3 |
| scenarios/scene_end/end_27.txt | 9000 | 151440043 | 16826.7 |
| scenarios/scene_end/end_31.txt | 9000 | 153152958 | 17017.0 |
| scenarios/scene_end/end_32.txt | 9000 | 155356478 | 17261.8 |
| scenarios/scene_end/end_33.txt | 9000 | 138963253 | 15440.4 |
| scenarios/scene_end/end_34.txt | 9000 | 140169048 | 15574.3 |
| scenarios/scene_end/end_35.txt | 9000 | 141087822 | 15676.4 |
| scenarios/scene_end/end_38.txt | 9000 | 149115342 | 16568.4 |
| scenarios/scene_end/end_40.txt | 9000 | 147760350 | 16417.8 |
| scenarios/scene_end/end_41.txt | 9000 | 176266751 | 19585.2 |
| scenarios/scene_end/end_42.txt | 9000 | 147286435 | 16365.2 |
| scenarios/scene_end/end_44.txt | 9000 | 166525560 | 18502.8 |
| scenarios/scene_end/end_46.txt | 9000 | 139153050 | 15461.5 |
| scenarios/scene_end/end_54.txt | 9000 | 172915301 | 19212.8 |
| scenarios/scene_end/end_58.txt | 9000 | 178499055 | 19833.2 |
| scenarios/scene_end/end_59.txt | 9000 | 138869825 | 15430.0 |
| scenarios/scene_end/end_64.txt | 9000 | 340011773 | 37779.1 |
| scenarios/scene_end/end_65.txt | 9000 | 332342718 | 36927.0 |
| scenarios/scene_end/end_66.txt | 9000 | 339862689 | 37762.5 |
| scenarios/scene_end/end_67.txt | 9000 | 138936191 | 15437.4 |
| scenarios/scene_end/end_68.txt | 9000 | 140126327 | 15569.6 |
