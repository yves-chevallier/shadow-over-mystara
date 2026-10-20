# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 121 scénarios, soit 2154000 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **773102500 instructions 68000 exécutées** (hors attente active), soit 358.9 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 555 exécutent encore du 68000, 684 sont entièrement natives, **1320 jamais atteintes** (91026 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 420145132.

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| K2 | Auto-test RAM (« RAM ERROR !! ») | 231450010 | 29.94 | 2 | 2 | 0 | 0 | 1912 (98.9 %), 182a (1.1 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 98641976 | 12.76 | 49 | 49 | 0 | 0 | 15216 (25.4 %), 156fa (20.5 %), 1728e (12.6 %) |
| V2 | Texte, rectangles et blocs de tuiles | 10746071 | 1.39 | 33 | 16 | 12 | 5 | 28d8 (58.1 %), 1386 (21.7 %), 2810 (14.0 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 5961428 | 0.77 | 17 | 2 | 12 | 3 | 350 (99.9 %), 492 (0.1 %) |
| W3 | Scripts de scène (phases 16/28) | 1447441 | 0.19 | 82 | 58 | 3 | 21 | f71fa (25.8 %), f7d86 (24.0 %), f68c8 (15.8 %) |
| C0e | EFFCT : effets | 1189957 | 0.15 | 170 | 107 | 5 | 58 | b46a2 (26.3 %), b46cc (22.2 %), b03c6 (4.8 %) |
| O3 | Aides communes aux types (animations par type) | 872757 | 0.11 | 25 | 24 | 0 | 1 | 3bb9c (31.5 %), 3be02 (26.9 %), 3bb16 (7.9 %) |
| M1 | Menu TEST (service) | 521903 | 0.07 | 76 | 23 | 0 | 53 | 29948 (29.7 %), 2bb0a (21.6 %), 2ca5e (17.4 %) |
| U1 | RNG et files d'affichage (pop-ups) | 402560 | 0.05 | 18 | 5 | 13 | 0 | 19fd2 (55.6 %), 1a20e (28.9 %), 19f24 (9.9 %) |
| S1 | Son : wrappers de commandes + FIFO | 364454 | 0.05 | 410 | 24 | 379 | 7 | e7e10 (45.5 %), e662a (23.9 %), e7a12 (6.9 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 296058 | 0.04 | 59 | 33 | 3 | 23 | 3769c (15.6 %), 35620 (11.4 %), 3733c (8.7 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 289845 | 0.04 | 69 | 16 | 43 | 10 | 11a36 (17.7 %), 788c (15.2 %), 1172e (15.1 %) |
| C04 | BOSS : boss | 193641 | 0.03 | 353 | 35 | 2 | 316 | 81aa4 (51.4 %), 833b6 (15.5 %), 87048 (6.9 %) |
| C1 | Crédits, pièces, start/continue | 180220 | 0.02 | 22 | 4 | 13 | 5 | e05ec (96.1 %), 14cb8 (3.0 %), e0538 (0.8 %) |
| V3 | Palettes et fondus | 98352 | 0.01 | 38 | 6 | 27 | 5 | 146c (65.9 %), 17ea (18.3 %), 1ced4 (14.2 %) |
| C08 | ESHL : tirs/attaques ennemis | 48678 | 0.01 | 127 | 35 | 1 | 91 | 9ef20 (27.5 %), a71c2 (13.6 %), a731c (13.4 %) |
| X2 | Dégâts, terrain, placement | 38118 | 0.00 | 19 | 12 | 0 | 7 | 3796 (43.4 %), 3618 (20.6 %), 3636 (18.4 %) |
| C0a | BOX : caisses/coffres (h) | 34174 | 0.00 | 7 | 4 | 0 | 3 | dabba (77.8 %), d9cea (20.1 %), da152 (2.0 %) |
| M2 | EEPROM, configuration, comptabilité | 29430 | 0.00 | 60 | 11 | 13 | 36 | 2d30a (92.5 %), 2e4ec (2.2 %), 2dc9a (1.2 %) |
| C10 | SET : décors/animations de palette | 24513 | 0.00 | 131 | 19 | 4 | 108 | d981a (23.3 %), cdedc (18.9 %), c8312 (15.6 %) |
| P5 | Code par personnage (6 × 2 variantes) | 19791 | 0.00 | 73 | 1 | 0 | 72 | 3e480 (100.0 %) |
| B1 | Scrolling / caméras par scène | 15965 | 0.00 | 42 | 1 | 10 | 31 | 33aa6 (100.0 %) |
| C0c | ITEM : objets ramassables | 14989 | 0.00 | 14 | 5 | 0 | 9 | db6c0 (97.1 %), dcddc (2.3 %), dc5fe (0.3 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 14796 | 0.00 | 3 | 1 | 1 | 1 | 1c44a (100.0 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 14580 | 0.00 | 5 | 4 | 0 | 1 | 19038 (44.8 %), 18b8e (36.6 %), 18a3e (17.7 %) |
| V4 | Construction de la liste de sprites (OBJ RAM) | 10311 | 0.00 | 15 | 1 | 1 | 13 | 1de7c (100.0 %) |
| P1 | Initialisation des joueurs | 6157 | 0.00 | 23 | 9 | 4 | 10 | 35c0 (53.9 %), 3432 (13.0 %), 3308 (10.8 %) |
| P3 | Comportement scénarisé des joueurs par scène | 5346 | 0.00 | 3 | 1 | 2 | 0 | 25b20 (100.0 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 4620 | 0.00 | 13 | 2 | 11 | 0 | c32 (64.1 %), c0c (35.9 %) |
| F1 | Tâche de boot, avertissements région | 4472 | 0.00 | 5 | 3 | 2 | 0 | 133dc (97.4 %), 133f6 (1.3 %), 133f0 (1.3 %) |
| C12 | PEFF : effets du joueur | 3450 | 0.00 | 107 | 10 | 27 | 70 | 51c06 (44.6 %), 4b7a0 (19.6 %), 4dc9a (13.0 %) |
| C02 | ENEMY : ennemis | 3007 | 0.00 | 205 | 10 | 1 | 194 | 5cdc6 (43.5 %), 57c66 (21.4 %), 57c8c (13.8 %) |
| C06 | PSHL : tirs du joueur | 2967 | 0.00 | 39 | 6 | 12 | 21 | 53e48 (47.6 %), 52578 (18.9 %), 54872 (14.5 %) |
| U2 | Petites routines / tables (rôle inconnu) | 2004 | 0.00 | 17 | 6 | 10 | 1 | 1ad2a (65.9 %), 1ac68 (17.1 %), 1ac86 (15.8 %) |
| C14 | SOBJ : objets système/UI | 1699 | 0.00 | 25 | 7 | 2 | 16 | dfe26 (36.8 %), e03c6 (20.2 %), e03e4 (17.3 %) |
| K3 | Entrées vers enregistrements joueur | 792 | 0.00 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| F6 | Interpréteur de scripts de scène (démos) | 700 | 0.00 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| P6 | Réactions aux coups, saisie des coups spéciaux | 136 | 0.00 | 9 | 1 | 2 | 6 | f18fa (100.0 %) |
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
| 001912 | ram_test_block | K2 | 104 | 228970115 | 29.62 | 121 |
| 015216 | FUN_00015216 | F3 | 62 | 25101916 | 3.25 | 120 |
| 0156fa | FUN_000156fa | F3 | 110 | 20182431 | 2.61 | 120 |
| 01728e | FUN_0001728e | F3 | 38 | 12446120 | 1.61 | 120 |
| 017814 | FUN_00017814 | F3 | 92 | 12342810 | 1.60 | 120 |
| 0175a8 | FUN_000175a8 | F3 | 46 | 11744021 | 1.52 | 120 |
| 0028d8 | FUN_000028d8 | V2 | 274 | 6247722 | 0.81 | 120 |
| 000350 | reset | K1 | 322 | 5952595 | 0.77 | 121 |
| 0155b2 | FUN_000155b2 | F3 | 70 | 5709858 | 0.74 | 114 |
| 016c1a | FUN_00016c1a | F3 | 628 | 4192799 | 0.54 | 120 |
| 00182a | FUN_0000182a | K2 | 90 | 2479895 | 0.32 | 121 |
| 001386 | FUN_00001386 | V2 | 134 | 2336407 | 0.30 | 101 |
| 0177a0 | FUN_000177a0 | F3 | 46 | 1994374 | 0.26 | 35 |
| 002810 | FUN_00002810 | V2 | 20 | 1500870 | 0.19 | 94 |
| 016b42 | FUN_00016b42 | F3 | 148 | 1014055 | 0.13 | 117 |
| 017a94 | FUN_00017a94 | F3 | 76 | 918576 | 0.12 | 120 |
| 01757e | FUN_0001757e | F3 | 22 | 706648 | 0.09 | 120 |
| 000d5e | FUN_00000d5e | V2 | 108 | 392776 | 0.05 | 116 |
| 0f71fa | FUN_000f71fa | W3 | 76 | 373733 | 0.05 | 23 |
| 016aec | FUN_00016aec | F3 | 126 | 367702 | 0.05 | 117 |
| 0f7d86 | FUN_000f7d86 | W3 | 116 | 346689 | 0.04 | 15 |
| 0170ca | FUN_000170ca | F3 | 22 | 332812 | 0.04 | 120 |
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 312819 | 0.04 | 8 |
| 03bb9c | FUN_0003bb9c | O3 | 170 | 275240 | 0.04 | 84 |
| 0173ee | FUN_000173ee | F3 | 18 | 274490 | 0.04 | 36 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 264693 | 0.03 | 8 |
| 03be02 | FUN_0003be02 | O3 | 54 | 234589 | 0.03 | 83 |
| 0f68c8 | FUN_000f68c8 | W3 | 58 | 228178 | 0.03 | 23 |
| 019fd2 | FUN_00019fd2 | U1 | 72 | 223651 | 0.03 | 74 |
| 017b40 | FUN_00017b40 | F3 | 86 | 201720 | 0.03 | 120 |
| 017236 | FUN_00017236 | F3 | 40 | 179340 | 0.02 | 120 |
| 0e05ec | FUN_000e05ec | C1 | 426 | 173138 | 0.02 | 5 |
| 0e7e10 | FUN_000e7e10 | S1 | 58 | 165851 | 0.02 | 117 |
| 029948 | FUN_00029948 | M1 | 76 | 154899 | 0.02 | 1 |
| 015444 | FUN_00015444 | F3 | 164 | 150476 | 0.02 | 6 |
| 017034 | FUN_00017034 | F3 | 72 | 145852 | 0.02 | 120 |
| 00137e | FUN_0000137e | V2 | 8 | 132648 | 0.02 | 101 |
| 01a20e | FUN_0001a20e | U1 | 118 | 116351 | 0.02 | 74 |
| 02bb0a | FUN_0002bb0a | M1 | 14 | 112872 | 0.01 | 1 |
| 0168a2 | FUN_000168a2 | F3 | 92 | 109105 | 0.01 | 120 |
| 081aa4 | FUN_00081aa4 | C04 | 54 | 99540 | 0.01 | 17 |
| 02ca5e | FUN_0002ca5e | M1 | 72 | 91061 | 0.01 | 1 |
| 029994 | FUN_00029994 | M1 | 76 | 89025 | 0.01 | 1 |
| 0e662a | FUN_000e662a | S1 | 66 | 87240 | 0.01 | 107 |
| 0f683c | FUN_000f683c | W3 | 42 | 85433 | 0.01 | 21 |
| 0153ba | FUN_000153ba | F3 | 138 | 80994 | 0.01 | 3 |
| 0f6d94 | FUN_000f6d94 | W3 | 42 | 79084 | 0.01 | 23 |
| 03bb16 | FUN_0003bb16 | O3 | 60 | 69309 | 0.01 | 57 |
| 0027ee | FUN_000027ee | V2 | 34 | 69164 | 0.01 | 80 |
| 015958 | FUN_00015958 | F3 | 146 | 67500 | 0.01 | 6 |
| 00146c | FUN_0000146c | V3 | 78 | 64826 | 0.01 | 121 |
| 03c2ee | FUN_0003c2ee | O3 | 14 | 63656 | 0.01 | 67 |
| 0b03c6 | FUN_000b03c6 | C0e | 102 | 57260 | 0.01 | 6 |
| 0b4686 | FUN_000b4686 | C0e | 28 | 57247 | 0.01 | 8 |
| 0f6b72 | FUN_000f6b72 | W3 | 46 | 55020 | 0.01 | 23 |
| 0f67fa | FUN_000f67fa | W3 | 66 | 52731 | 0.01 | 20 |
| 011a36 | FUN_00011a36 | E1 | 104 | 51431 | 0.01 | 9 |
| 000f4a | FUN_00000f4a | V2 | 158 | 49889 | 0.01 | 5 |
| 03de2e | FUN_0003de2e | O3 | 50 | 49050 | 0.01 | 93 |
| 0b5f8e | FUN_000b5f8e | C0e | 40 | 47104 | 0.01 | 5 |
| 03769c | FUN_0003769c | P4 | 58 | 46241 | 0.01 | 47 |
| 00788c | FUN_0000788c | E1 | 34 | 43983 | 0.01 | 96 |
| 01172e | FUN_0001172e | E1 | 92 | 43861 | 0.01 | 23 |
| 0f7916 | FUN_000f7916 | W3 | 24 | 41357 | 0.01 | 21 |
| 0ad60a | FUN_000ad60a | C0e | 52 | 41224 | 0.01 | 12 |
| 029a9a | FUN_00029a9a | M1 | 22 | 41140 | 0.01 | 1 |
| 019f24 | FUN_00019f24 | U1 | 72 | 39956 | 0.01 | 74 |
| 01725e | FUN_0001725e | F3 | 48 | 39870 | 0.01 | 120 |
| 016c86 | FUN_00016c86 | F3 | 208 | 35960 | 0.00 | 117 |
| 03dd72 | FUN_0003dd72 | O3 | 14 | 35884 | 0.00 | 39 |
| 017176 | FUN_00017176 | F3 | 46 | 34927 | 0.00 | 120 |
| 0ab2b4 | FUN_000ab2b4 | C0e | 16 | 34385 | 0.00 | 111 |
| 0b13fa | FUN_000b13fa | C0e | 32 | 33920 | 0.00 | 93 |
| 035620 | FUN_00035620 | P4 | 50 | 33816 | 0.00 | 101 |
| 0043bc | FUN_000043bc | E1 | 8 | 31772 | 0.00 | 16 |
| 0ae3bc | FUN_000ae3bc | C0e | 50 | 30386 | 0.00 | 5 |
| 0833b6 | FUN_000833b6 | C04 | 46 | 30013 | 0.00 | 1 |
| 03c63a | FUN_0003c63a | O3 | 18 | 28310 | 0.00 | 42 |
| 0abea4 | FUN_000abea4 | C0e | 36 | 28210 | 0.00 | 2 |
| 0f5cee | FUN_000f5cee | W3 | 38 | 27669 | 0.00 | 20 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 555 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 316 | 16240 |
| C10 | SET : décors/animations de palette | 108 | 10956 |
| C02 | ENEMY : ennemis | 194 | 9052 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 91 | 5572 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| C0e | EFFCT : effets | 58 | 2722 |
| B1 | Scrolling / caméras par scène | 31 | 2630 |
| M1 | Menu TEST (service) | 53 | 2386 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| P4 | Noyau joueur (états, 26 actions génériques) | 23 | 1876 |
| C06 | PSHL : tirs du joueur | 21 | 1726 |
| M2 | EEPROM, configuration, comptabilité | 36 | 1696 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 6 | 1500 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C0c | ITEM : objets ramassables | 9 | 1254 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| C14 | SOBJ : objets système/UI | 16 | 736 |
| W3 | Scripts de scène (phases 16/28) | 21 | 730 |
| F4 | Exécuteur d'écrans fixes (logos) | 1 | 666 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 10 | 508 |
| X2 | Dégâts, terrain, placement | 7 | 506 |
| C1 | Crédits, pièces, start/continue | 5 | 476 |
| O2 | Pools d'objets et listes triées | 13 | 342 |
| V3 | Palettes et fondus | 5 | 330 |
| S1 | Son : wrappers de commandes + FIFO | 7 | 328 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 3 | 318 |
| C0a | BOX : caisses/coffres (h) | 3 | 264 |
| P1 | Initialisation des joueurs | 10 | 178 |
| F7 | Écrans titre / copyright | 4 | 108 |
| V2 | Texte, rectangles et blocs de tuiles | 5 | 96 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 46 |
| O3 | Aides communes aux types (animations par type) | 1 | 38 |
| U2 | Petites routines / tables (rôle inconnu) | 1 | 10 |
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 5807314 | 483.9 |
| scenarios/boss/boss10_scene66.txt | 12000 | 2737145 | 228.1 |
| scenarios/boss/boss11_scene42.txt | 12000 | 6044593 | 503.7 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 6972048 | 581.0 |
| scenarios/boss/boss12_scene40.txt | 12000 | 5976347 | 498.0 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 6956155 | 579.7 |
| scenarios/boss/boss13_dragon.txt | 12000 | 5975939 | 498.0 |
| scenarios/boss/boss15_scene46.txt | 12000 | 6553979 | 546.2 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 6946337 | 578.9 |
| scenarios/boss/boss17_scene13.txt | 12000 | 6015207 | 501.3 |
| scenarios/boss/boss18_scene26.txt | 12000 | 6228721 | 519.1 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 5569211 | 464.1 |
| scenarios/boss/boss19_scene32.txt | 12000 | 5718711 | 476.6 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 7019297 | 584.9 |
| scenarios/boss/boss21_scene44.txt | 12000 | 5891602 | 491.0 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 6877606 | 573.1 |
| scenarios/boss/boss5_scene58.txt | 12000 | 5765536 | 480.5 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 6895797 | 574.6 |
| scenarios/boss/boss6_scene24.txt | 12000 | 6066507 | 505.5 |
| scenarios/boss/boss8_scene64.txt | 12000 | 5739484 | 478.3 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 6412872 | 534.4 |
| scenarios/bot30k.txt | 30000 | 4843867 | 161.5 |
| scenarios/bot_2p.txt | 30000 | 4164197 | 138.8 |
| scenarios/bot_char0.txt | 30000 | 4473857 | 149.1 |
| scenarios/bot_char1.txt | 30000 | 6253079 | 208.4 |
| scenarios/bot_char2.txt | 30000 | 3430716 | 114.4 |
| scenarios/bot_char3.txt | 30000 | 4506000 | 150.2 |
| scenarios/bot_char4.txt | 30000 | 4188309 | 139.6 |
| scenarios/bot_char5.txt | 30000 | 4226948 | 140.9 |
| scenarios/cov/cov_2p_a.txt | 30000 | 12533577 | 417.8 |
| scenarios/cov/cov_2p_b.txt | 30000 | 2556684 | 85.2 |
| scenarios/cov/cov_2p_c.txt | 30000 | 19706539 | 656.9 |
| scenarios/cov/cov_3p.txt | 30000 | 21838116 | 727.9 |
| scenarios/cov/cov_4p_a.txt | 30000 | 19441504 | 648.1 |
| scenarios/cov/cov_4p_b.txt | 30000 | 2960381 | 98.7 |
| scenarios/cov/cov_4p_c.txt | 30000 | 2782089 | 92.7 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 3425915 | 114.2 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 6587982 | 219.6 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 6993333 | 233.1 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 6585343 | 219.5 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 6615935 | 220.5 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 6553144 | 218.4 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 6585343 | 219.5 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 6615935 | 220.5 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 7446875 | 248.2 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 9500993 | 316.7 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 11273565 | 375.8 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 11136540 | 371.2 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 2875394 | 95.8 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 13935603 | 464.5 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 12175877 | 405.9 |
| scenarios/cov/cov_service.txt | 30000 | 3566210 | 118.9 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 13276551 | 442.6 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 2205684 | 73.5 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 14904382 | 496.8 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 8357848 | 278.6 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 13242777 | 441.4 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 13448367 | 448.3 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 15753640 | 525.1 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 2528557 | 84.3 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 5607587 | 467.3 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 5499664 | 458.3 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 2201766 | 183.5 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 5545515 | 462.1 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 5627730 | 469.0 |
| scenarios/enemy/elf_scene92.txt | 12000 | 5510924 | 459.2 |
| scenarios/enemy/elf_scene94.txt | 12000 | 5493183 | 457.8 |
| scenarios/enemy/elf_scene96.txt | 12000 | 5729161 | 477.4 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 6044593 | 503.7 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 2516370 | 209.7 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 6029155 | 502.4 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 5727800 | 477.3 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 2708838 | 225.7 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 5531788 | 461.0 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 5528868 | 460.7 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 5638726 | 469.9 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 5858521 | 488.2 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 2189493 | 182.5 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 18216913 | 1518.1 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 5921775 | 493.5 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 3301501 | 275.1 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 5613127 | 467.8 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 2260096 | 188.3 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 5775518 | 481.3 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 18176125 | 1514.7 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 5717331 | 476.4 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 5969027 | 497.4 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 5544196 | 462.0 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 5807314 | 483.9 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 5803498 | 483.6 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 5582158 | 465.2 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 2348453 | 195.7 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 5506501 | 458.9 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 5512200 | 459.4 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 5480453 | 456.7 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 6015207 | 501.3 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 5796729 | 483.1 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 2610093 | 217.5 |
| scenarios/enemy/trog_scene101.txt | 12000 | 2187055 | 182.3 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 5500555 | 458.4 |
| scenarios/enemy/trog_scene104.txt | 12000 | 6687797 | 557.3 |
| scenarios/extra/mash_2p.txt | 12000 | 5196037 | 433.0 |
| scenarios/extra/mash_c0.txt | 12000 | 5627168 | 468.9 |
| scenarios/extra/mash_c0b.txt | 12000 | 3865095 | 322.1 |
| scenarios/extra/mash_c1b.txt | 12000 | 5662839 | 471.9 |
| scenarios/extra/mash_c2.txt | 12000 | 4166057 | 347.2 |
| scenarios/extra/mash_c2b.txt | 12000 | 3497520 | 291.5 |
| scenarios/extra/mash_c3.txt | 12000 | 5573023 | 464.4 |
| scenarios/extra/mash_c3b.txt | 12000 | 5612668 | 467.7 |
| scenarios/extra/mash_c3c.txt | 12000 | 4247481 | 354.0 |
| scenarios/extra/mash_c3d.txt | 12000 | 4159706 | 346.6 |
| scenarios/extra/mash_c4.txt | 12000 | 4447849 | 370.7 |
| scenarios/extra/mash_c4b.txt | 12000 | 4110250 | 342.5 |
| scenarios/extra/mash_c4c.txt | 12000 | 4260859 | 355.1 |
| scenarios/extra/mash_c5.txt | 12000 | 4833819 | 402.8 |
| scenarios/extra/mash_c5b.txt | 12000 | 5607668 | 467.3 |
| scenarios/modes/attract.txt | 12000 | 3425915 | 285.5 |
| scenarios/modes/ending.txt | 12000 | 4708835 | 392.4 |
| scenarios/modes/ending_end.txt | 12000 | 4494119 | 374.5 |
| scenarios/modes/gameover.txt | 12000 | 4188309 | 349.0 |
| scenarios/modes/screens.txt | 12000 | 3425915 | 285.5 |
