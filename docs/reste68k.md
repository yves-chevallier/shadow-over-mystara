# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 90 scénarios, soit 1782000 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **696364878 instructions 68000 exécutées** (hors attente active), soit 390.8 par image ;
* attente active, non comptée : 26592033160 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 659 exécutent encore du 68000, 639 sont entièrement natives, **1261 jamais atteintes** (86636 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 386170362.

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| K2 | Auto-test RAM (« RAM ERROR !! ») | 172152900 | 24.72 | 2 | 2 | 0 | 0 | 1912 (98.9 %), 182a (1.1 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 79100666 | 11.36 | 49 | 49 | 0 | 0 | 15216 (25.4 %), 156fa (20.9 %), 17814 (12.1 %) |
| B1 | Scrolling / caméras par scène | 29117853 | 4.18 | 42 | 15 | 8 | 19 | 30308 (21.1 %), 31018 (16.9 %), 309e0 (16.2 %) |
| V2 | Texte, rectangles et blocs de tuiles | 8051680 | 1.16 | 33 | 16 | 12 | 5 | 28d8 (58.3 %), 1386 (22.8 %), 2810 (12.9 %) |
| S1 | Son : wrappers de commandes + FIFO | 4784122 | 0.69 | 410 | 24 | 380 | 6 | edf80 (94.2 %), e7e10 (2.8 %), e662a (1.3 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 4434120 | 0.64 | 17 | 2 | 12 | 3 | 350 (99.9 %), 492 (0.1 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 4216786 | 0.61 | 69 | 44 | 19 | 6 | 7f0c (17.2 %), 10428 (15.7 %), eff2 (11.2 %) |
| C0e | EFFCT : effets | 1479184 | 0.21 | 170 | 107 | 3 | 60 | b46a2 (21.1 %), b10c8 (20.2 %), b46cc (17.9 %) |
| W3 | Scripts de scène (phases 16/28) | 1418711 | 0.20 | 82 | 58 | 3 | 21 | f71fa (26.3 %), f7d86 (24.4 %), f68c8 (14.1 %) |
| C02 | ENEMY : ennemis | 1180324 | 0.17 | 205 | 62 | 0 | 143 | 67452 (50.8 %), 71dde (8.3 %), 66740 (6.7 %) |
| C04 | BOSS : boss | 1120032 | 0.16 | 353 | 36 | 1 | 316 | 7f19c (83.7 %), 81aa4 (8.0 %), 833b6 (2.7 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 683750 | 0.10 | 59 | 33 | 2 | 24 | 3525c (65.4 %), 3769c (5.9 %), 35620 (4.1 %) |
| O3 | Aides communes aux types (animations par type) | 599148 | 0.09 | 25 | 24 | 0 | 1 | 3bb9c (31.5 %), 3be02 (27.4 %), 3bb16 (8.3 %) |
| M1 | Menu TEST (service) | 521903 | 0.07 | 76 | 23 | 0 | 53 | 29948 (29.7 %), 2bb0a (21.6 %), 2ca5e (17.4 %) |
| U2 | Petites routines / tables (rôle inconnu) | 472962 | 0.07 | 17 | 12 | 0 | 5 | 1ab56 (94.8 %), 1abe0 (2.4 %), 1ac24 (2.4 %) |
| U1 | RNG et files d'affichage (pop-ups) | 234540 | 0.03 | 18 | 5 | 13 | 0 | 19fd2 (56.4 %), 1a20e (27.7 %), 19f24 (9.9 %) |
| C1 | Crédits, pièces, start/continue | 178784 | 0.03 | 22 | 4 | 13 | 5 | e05ec (96.8 %), 14cb8 (2.2 %), e0538 (0.8 %) |
| C10 | SET : décors/animations de palette | 83628 | 0.01 | 131 | 19 | 3 | 109 | cc18e (81.0 %), c8312 (4.4 %), cdedc (4.0 %) |
| V3 | Palettes et fondus | 83015 | 0.01 | 38 | 6 | 27 | 5 | 146c (64.5 %), 17ea (21.1 %), 1ced4 (12.8 %) |
| C08 | ESHL : tirs/attaques ennemis | 44962 | 0.01 | 127 | 35 | 1 | 91 | 9ef20 (29.8 %), a71c2 (14.8 %), a731c (14.5 %) |
| P3 | Comportement scénarisé des joueurs par scène | 40719 | 0.01 | 3 | 2 | 1 | 0 | 25260 (88.7 %), 25b20 (11.3 %) |
| X2 | Dégâts, terrain, placement | 34156 | 0.00 | 19 | 12 | 0 | 7 | 3796 (38.6 %), 3618 (23.0 %), 3636 (20.5 %) |
| C12 | PEFF : effets du joueur | 25728 | 0.00 | 107 | 11 | 26 | 70 | 517ba (91.0 %), 51c06 (3.3 %), 4b7a0 (1.6 %) |
| C0a | BOX : caisses/coffres (h) | 25476 | 0.00 | 7 | 4 | 0 | 3 | dabba (80.1 %), d9cea (17.9 %), da152 (1.9 %) |
| M2 | EEPROM, configuration, comptabilité | 22114 | 0.00 | 60 | 11 | 13 | 36 | 2d30a (91.6 %), 2e4ec (2.3 %), 2d5c6 (1.4 %) |
| P5 | Code par personnage (6 × 2 variantes) | 13945 | 0.00 | 73 | 1 | 0 | 72 | 3e480 (100.0 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 12906 | 0.00 | 5 | 4 | 0 | 1 | 18b8e (41.4 %), 19038 (37.7 %), 18a3e (20.0 %) |
| C14 | SOBJ : objets système/UI | 12700 | 0.00 | 25 | 8 | 0 | 17 | ddeaa (54.0 %), dfa4a (34.4 %), dfe26 (4.6 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 11448 | 0.00 | 3 | 1 | 1 | 1 | 1c44a (100.0 %) |
| C0c | ITEM : objets ramassables | 11095 | 0.00 | 14 | 5 | 0 | 9 | db6c0 (96.0 %), dcddc (3.1 %), dc5fe (0.4 %) |
| V4 | Construction de la liste de sprites (OBJ RAM) | 8743 | 0.00 | 15 | 1 | 1 | 13 | 1de7c (100.0 %) |
| P1 | Initialisation des joueurs | 6157 | 0.00 | 23 | 9 | 4 | 10 | 35c0 (53.9 %), 3432 (13.0 %), 3308 (10.8 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 3680 | 0.00 | 13 | 2 | 11 | 0 | c32 (63.6 %), c0c (36.4 %) |
| F1 | Tâche de boot, avertissements région | 3356 | 0.00 | 5 | 3 | 2 | 0 | 133dc (96.5 %), 133f6 (1.8 %), 133f0 (1.7 %) |
| C06 | PSHL : tirs du joueur | 1982 | 0.00 | 39 | 6 | 12 | 21 | 53e48 (49.7 %), 54872 (20.0 %), 52578 (17.0 %) |
| K3 | Entrées vers enregistrements joueur | 774 | 0.00 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| F6 | Interpréteur de scripts de scène (démos) | 348 | 0.00 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| P6 | Réactions aux coups, saisie des coups spéciaux | 119 | 0.00 | 9 | 1 | 2 | 6 | f18fa (100.0 %) |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 16 | 0 | 0 | 16 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 22 | 0 | 22 | 0 |  |
| O2 | Pools d'objets et listes triées | 0 | 0.00 | 41 | 0 | 28 | 13 |  |
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
| 001912 | ram_test_block | K2 | 104 | 170308350 | 24.46 | 90 |
| 015216 | FUN_00015216 | F3 | 62 | 20118584 | 2.89 | 89 |
| 0156fa | FUN_000156fa | F3 | 110 | 16549508 | 2.38 | 89 |
| 017814 | FUN_00017814 | F3 | 92 | 9609037 | 1.38 | 89 |
| 01728e | FUN_0001728e | F3 | 38 | 9597791 | 1.38 | 89 |
| 0175a8 | FUN_000175a8 | F3 | 46 | 9255899 | 1.33 | 89 |
| 030308 | FUN_00030308 | B1 | 126 | 6137976 | 0.88 | 13 |
| 031018 | FUN_00031018 | B1 | 174 | 4912728 | 0.71 | 55 |
| 0309e0 | FUN_000309e0 | B1 | 1014 | 4706600 | 0.68 | 87 |
| 0028d8 | FUN_000028d8 | V2 | 274 | 4692281 | 0.67 | 89 |
| 0edf80 | FUN_000edf80 | S1 | 196 | 4505478 | 0.65 | 90 |
| 000350 | reset | K1 | 322 | 4427550 | 0.64 | 90 |
| 0155b2 | FUN_000155b2 | F3 | 70 | 4215162 | 0.61 | 83 |
| 016c1a | FUN_00016c1a | F3 | 628 | 3447032 | 0.50 | 89 |
| 030fc8 | FUN_00030fc8 | B1 | 60 | 2753023 | 0.40 | 53 |
| 033062 | caseD_f7 | B1 | 32 | 2341386 | 0.34 | 38 |
| 0332c0 | FUN_000332c0 | B1 | 16 | 2242070 | 0.32 | 89 |
| 0177a0 | FUN_000177a0 | F3 | 46 | 1994374 | 0.29 | 35 |
| 00182a | FUN_0000182a | K2 | 90 | 1844550 | 0.26 | 90 |
| 001386 | FUN_00001386 | V2 | 134 | 1835997 | 0.26 | 74 |
| 02ede0 | FUN_0002ede0 | B1 | 2360 | 1809040 | 0.26 | 89 |
| 032108 | FUN_00032108 | B1 | 1616 | 1799946 | 0.26 | 89 |
| 0305fc | FUN_000305fc | B1 | 34 | 1158504 | 0.17 | 39 |
| 002810 | FUN_00002810 | V2 | 20 | 1035661 | 0.15 | 66 |
| 07f19c | FUN_0007f19c | C04 | 92 | 937057 | 0.13 | 11 |
| 017a94 | FUN_00017a94 | F3 | 76 | 881544 | 0.13 | 89 |
| 03147c | FUN_0003147c | B1 | 220 | 778010 | 0.11 | 12 |
| 016b42 | FUN_00016b42 | F3 | 148 | 738967 | 0.11 | 86 |
| 007f0c | FUN_00007f0c | E1 | 220 | 726486 | 0.10 | 70 |
| 010428 | FUN_00010428 | E1 | 40 | 663957 | 0.10 | 3 |
| 01757e | FUN_0001757e | F3 | 22 | 630992 | 0.09 | 89 |
| 067452 | FUN_00067452 | C02 | 120 | 599627 | 0.09 | 9 |
| 00eff2 | FUN_0000eff2 | E1 | 120 | 473721 | 0.07 | 14 |
| 01ab56 | FUN_0001ab56 | U2 | 138 | 448178 | 0.06 | 15 |
| 03525c | FUN_0003525c | P4 | 44 | 447146 | 0.06 | 89 |
| 00e886 | FUN_0000e886 | E1 | 242 | 419271 | 0.06 | 15 |
| 0f71fa | FUN_000f71fa | W3 | 76 | 373733 | 0.05 | 23 |
| 0f7d86 | FUN_000f7d86 | W3 | 116 | 346689 | 0.05 | 15 |
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 312819 | 0.04 | 8 |
| 0b10c8 | FUN_000b10c8 | C0e | 82 | 298779 | 0.04 | 82 |
| 0170ca | FUN_000170ca | F3 | 22 | 293264 | 0.04 | 89 |
| 000d5e | FUN_00000d5e | V2 | 108 | 287810 | 0.04 | 85 |
| 0173ee | FUN_000173ee | F3 | 18 | 274490 | 0.04 | 36 |
| 016aec | FUN_00016aec | F3 | 126 | 264957 | 0.04 | 86 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 264693 | 0.04 | 8 |
| 0115e2 | FUN_000115e2 | E1 | 58 | 237943 | 0.03 | 7 |
| 033290 | FUN_00033290 | B1 | 38 | 230940 | 0.03 | 13 |
| 0f68c8 | FUN_000f68c8 | W3 | 58 | 200330 | 0.03 | 23 |
| 011654 | FUN_00011654 | E1 | 58 | 193676 | 0.03 | 7 |
| 03bb9c | FUN_0003bb9c | O3 | 170 | 188607 | 0.03 | 63 |
| 00a470 | FUN_0000a470 | E1 | 28 | 182472 | 0.03 | 73 |
| 017b40 | FUN_00017b40 | F3 | 86 | 178760 | 0.03 | 89 |
| 0e05ec | FUN_000e05ec | C1 | 426 | 173138 | 0.02 | 5 |
| 01161c | FUN_0001161c | E1 | 56 | 167118 | 0.02 | 7 |
| 03be02 | FUN_0003be02 | O3 | 54 | 164300 | 0.02 | 61 |
| 017236 | FUN_00017236 | F3 | 40 | 163528 | 0.02 | 89 |
| 029948 | FUN_00029948 | M1 | 76 | 154899 | 0.02 | 1 |
| 0104a2 | FUN_000104a2 | E1 | 58 | 152788 | 0.02 | 22 |
| 007fe8 | FUN_00007fe8 | E1 | 4 | 151480 | 0.02 | 70 |
| 015444 | FUN_00015444 | F3 | 164 | 150476 | 0.02 | 6 |
| 030300 | FUN_00030300 | B1 | 8 | 149762 | 0.02 | 43 |
| 00fda2 | FUN_0000fda2 | E1 | 146 | 135871 | 0.02 | 9 |
| 017034 | FUN_00017034 | F3 | 72 | 132912 | 0.02 | 89 |
| 0e7e10 | FUN_000e7e10 | S1 | 58 | 132354 | 0.02 | 88 |
| 019fd2 | FUN_00019fd2 | U1 | 72 | 132171 | 0.02 | 51 |
| 02bb0a | FUN_0002bb0a | M1 | 14 | 112872 | 0.02 | 1 |
| 0168a2 | FUN_000168a2 | F3 | 92 | 107966 | 0.02 | 89 |
| 00137e | FUN_0000137e | V2 | 8 | 103684 | 0.01 | 74 |
| 071dde | FUN_00071dde | C02 | 38 | 98118 | 0.01 | 5 |
| 007ef4 | FUN_00007ef4 | E1 | 24 | 96192 | 0.01 | 22 |
| 0103e0 | FUN_000103e0 | E1 | 44 | 94932 | 0.01 | 21 |
| 02ca5e | FUN_0002ca5e | M1 | 72 | 91061 | 0.01 | 1 |
| 081aa4 | FUN_00081aa4 | C04 | 54 | 89504 | 0.01 | 16 |
| 029994 | FUN_00029994 | M1 | 76 | 89025 | 0.01 | 1 |
| 0f683c | FUN_000f683c | W3 | 42 | 85433 | 0.01 | 21 |
| 0153ba | FUN_000153ba | F3 | 138 | 80994 | 0.01 | 3 |
| 066740 | FUN_00066740 | C02 | 144 | 79488 | 0.01 | 8 |
| 0f6d94 | FUN_000f6d94 | W3 | 42 | 79084 | 0.01 | 23 |
| 0aa96c | FUN_000aa96c | C0e | 84 | 77194 | 0.01 | 81 |
| 00e914 | FUN_0000e914 | E1 | 42 | 74328 | 0.01 | 24 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 659 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 316 | 16240 |
| C10 | SET : décors/animations de palette | 109 | 11018 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| C02 | ENEMY : ennemis | 143 | 6570 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 91 | 5572 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| C0e | EFFCT : effets | 60 | 2792 |
| M1 | Menu TEST (service) | 53 | 2386 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| P4 | Noyau joueur (états, 26 actions génériques) | 24 | 1934 |
| C06 | PSHL : tirs du joueur | 21 | 1726 |
| M2 | EEPROM, configuration, comptabilité | 36 | 1696 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 6 | 1500 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C0c | ITEM : objets ramassables | 9 | 1254 |
| B1 | Scrolling / caméras par scène | 19 | 888 |
| C14 | SOBJ : objets système/UI | 17 | 760 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| W3 | Scripts de scène (phases 16/28) | 21 | 730 |
| F4 | Exécuteur d'écrans fixes (logos) | 1 | 666 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| X2 | Dégâts, terrain, placement | 7 | 506 |
| C1 | Crédits, pièces, start/continue | 5 | 476 |
| O2 | Pools d'objets et listes triées | 13 | 342 |
| V3 | Palettes et fondus | 5 | 330 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 3 | 318 |
| C0a | BOX : caisses/coffres (h) | 3 | 264 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 6 | 220 |
| P1 | Initialisation des joueurs | 10 | 178 |
| S1 | Son : wrappers de commandes + FIFO | 6 | 132 |
| U2 | Petites routines / tables (rôle inconnu) | 5 | 114 |
| F7 | Écrans titre / copyright | 4 | 108 |
| V2 | Texte, rectangles et blocs de tuiles | 5 | 96 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 46 |
| O3 | Aides communes aux types (animations par type) | 1 | 38 |
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 248141609 | 20678.5 |
| scenarios/boss/boss10_scene66.txt | 12000 | 109711172 | 9142.6 |
| scenarios/boss/boss11_scene42.txt | 12000 | 194758604 | 16229.9 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 219009645 | 18250.8 |
| scenarios/boss/boss12_scene40.txt | 12000 | 194878208 | 16239.9 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 217752452 | 18146.0 |
| scenarios/boss/boss15_scene46.txt | 12000 | 188356116 | 15696.3 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 210646505 | 17553.9 |
| scenarios/boss/boss17_scene13.txt | 12000 | 203528173 | 16960.7 |
| scenarios/boss/boss18_scene26.txt | 12000 | 216565522 | 18047.1 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 243613728 | 20301.1 |
| scenarios/boss/boss19_scene32.txt | 12000 | 203965115 | 16997.1 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 227076463 | 18923.0 |
| scenarios/boss/boss21_scene44.txt | 12000 | 215889508 | 17990.8 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 239336238 | 19944.7 |
| scenarios/boss/boss5_scene58.txt | 12000 | 212228271 | 17685.7 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 230911385 | 19242.6 |
| scenarios/boss/boss6_scene24.txt | 12000 | 209603446 | 17467.0 |
| scenarios/boss/boss8_scene64.txt | 12000 | 224103510 | 18675.3 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 241559230 | 20129.9 |
| scenarios/bot30k.txt | 30000 | 491924913 | 16397.5 |
| scenarios/bot_2p.txt | 30000 | 491284082 | 16376.1 |
| scenarios/bot_char0.txt | 30000 | 491513664 | 16383.8 |
| scenarios/bot_char1.txt | 30000 | 493778950 | 16459.3 |
| scenarios/bot_char2.txt | 30000 | 122659359 | 4088.6 |
| scenarios/bot_char3.txt | 30000 | 491591089 | 16386.4 |
| scenarios/bot_char4.txt | 30000 | 491179850 | 16372.7 |
| scenarios/bot_char5.txt | 30000 | 491216025 | 16373.9 |
| scenarios/cov/cov_2p_a.txt | 30000 | 651574788 | 21719.2 |
| scenarios/cov/cov_2p_b.txt | 30000 | 99093834 | 3303.1 |
| scenarios/cov/cov_2p_c.txt | 30000 | 585921116 | 19530.7 |
| scenarios/cov/cov_3p.txt | 30000 | 754671751 | 25155.7 |
| scenarios/cov/cov_4p_a.txt | 30000 | 807126326 | 26904.2 |
| scenarios/cov/cov_4p_b.txt | 30000 | 101585129 | 3386.2 |
| scenarios/cov/cov_4p_c.txt | 30000 | 59600360 | 1986.7 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 68011723 | 2267.1 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 481151527 | 16038.4 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 316177334 | 10539.2 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 481043194 | 16034.8 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 480411350 | 16013.7 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 481007220 | 16033.6 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 481148908 | 16038.3 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 480597592 | 16019.9 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 477813555 | 15927.1 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 478746668 | 15958.2 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 508379655 | 16946.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 472685091 | 15756.2 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 99038980 | 3301.3 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 312315881 | 10410.5 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 501744374 | 16724.8 |
| scenarios/cov/cov_service.txt | 30000 | 471606684 | 15720.2 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 725030013 | 24167.7 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 43482263 | 1449.4 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 684307965 | 22810.3 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 531982226 | 17732.7 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 724983601 | 24166.1 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 715140378 | 23838.0 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 651178594 | 21706.0 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 52185619 | 1739.5 |
| scenarios/enemy/elf_scene92.txt | 12000 | 188770676 | 15730.9 |
| scenarios/enemy/elf_scene94.txt | 12000 | 192158746 | 16013.2 |
| scenarios/enemy/elf_scene96.txt | 12000 | 186747627 | 15562.3 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 84728995 | 7060.7 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 189036700 | 15753.1 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 203528173 | 16960.7 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 212288270 | 17690.7 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 98279717 | 8190.0 |
| scenarios/enemy/trog_scene101.txt | 12000 | 42674401 | 3556.2 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 188872539 | 15739.4 |
| scenarios/enemy/trog_scene104.txt | 12000 | 189550811 | 15795.9 |
| scenarios/extra/mash_2p.txt | 12000 | 213121743 | 17760.1 |
| scenarios/extra/mash_c0.txt | 12000 | 213622649 | 17801.9 |
| scenarios/extra/mash_c0b.txt | 12000 | 211534523 | 17627.9 |
| scenarios/extra/mash_c1b.txt | 12000 | 213256460 | 17771.4 |
| scenarios/extra/mash_c2.txt | 12000 | 211908328 | 17659.0 |
| scenarios/extra/mash_c2b.txt | 12000 | 125067102 | 10422.3 |
| scenarios/extra/mash_c3.txt | 12000 | 213659584 | 17805.0 |
| scenarios/extra/mash_c3b.txt | 12000 | 213540825 | 17795.1 |
| scenarios/extra/mash_c3c.txt | 12000 | 211967093 | 17663.9 |
| scenarios/extra/mash_c3d.txt | 12000 | 211872481 | 17656.0 |
| scenarios/extra/mash_c4.txt | 12000 | 212228636 | 17685.7 |
| scenarios/extra/mash_c4b.txt | 12000 | 211766974 | 17647.2 |
| scenarios/extra/mash_c4c.txt | 12000 | 211952100 | 17662.7 |
| scenarios/extra/mash_c5.txt | 12000 | 212559175 | 17713.3 |
| scenarios/extra/mash_c5b.txt | 12000 | 213605379 | 17800.4 |
| scenarios/modes/attract.txt | 12000 | 68011723 | 5667.6 |
| scenarios/modes/ending.txt | 12000 | 233100025 | 19425.0 |
| scenarios/modes/ending_end.txt | 12000 | 232585694 | 19382.1 |
| scenarios/modes/gameover.txt | 12000 | 211862635 | 17655.2 |
| scenarios/modes/screens.txt | 12000 | 68011723 | 5667.6 |
