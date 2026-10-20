# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 121 scénarios, soit 2154000 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **337381646 instructions 68000 exécutées** (hors attente active), soit 156.6 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 497 exécutent encore du 68000, 693 sont entièrement natives, **1369 jamais atteintes** (95196 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 94144500.

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| K2 | Auto-test RAM (« RAM ERROR !! ») | 231450010 | 68.60 | 2 | 2 | 0 | 0 | 1912 (98.9 %), 182a (1.1 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 5961428 | 1.77 | 17 | 2 | 12 | 3 | 350 (99.9 %), 492 (0.1 %) |
| W3 | Scripts de scène (phases 16/28) | 1447745 | 0.43 | 82 | 58 | 3 | 21 | f71fa (25.8 %), f7d86 (23.9 %), f68c8 (15.8 %) |
| C0e | EFFCT : effets | 1189957 | 0.35 | 170 | 107 | 5 | 58 | b46a2 (26.3 %), b46cc (22.2 %), b03c6 (4.8 %) |
| O3 | Aides communes aux types (animations par type) | 872757 | 0.26 | 25 | 24 | 0 | 1 | 3bb9c (31.5 %), 3be02 (26.9 %), 3bb16 (7.9 %) |
| M1 | Menu TEST (service) | 521903 | 0.15 | 76 | 23 | 0 | 53 | 29948 (29.7 %), 2bb0a (21.6 %), 2ca5e (17.4 %) |
| S1 | Son : wrappers de commandes + FIFO | 364454 | 0.11 | 410 | 24 | 379 | 7 | e7e10 (45.5 %), e662a (23.9 %), e7a12 (6.9 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 296058 | 0.09 | 59 | 33 | 3 | 23 | 3769c (15.6 %), 35620 (11.4 %), 3733c (8.7 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 289845 | 0.09 | 69 | 16 | 43 | 10 | 11a36 (17.7 %), 788c (15.2 %), 1172e (15.1 %) |
| C04 | BOSS : boss | 193641 | 0.06 | 353 | 35 | 2 | 316 | 81aa4 (51.4 %), 833b6 (15.5 %), 87048 (6.9 %) |
| C1 | Crédits, pièces, start/continue | 180220 | 0.05 | 22 | 4 | 13 | 5 | e05ec (96.1 %), 14cb8 (3.0 %), e0538 (0.8 %) |
| V3 | Palettes et fondus | 98352 | 0.03 | 38 | 6 | 27 | 5 | 146c (65.9 %), 17ea (18.3 %), 1ced4 (14.2 %) |
| V2 | Texte, rectangles et blocs de tuiles | 67832 | 0.02 | 33 | 9 | 19 | 5 | f4a (73.5 %), 2860 (10.6 %), 2824 (8.1 %) |
| C08 | ESHL : tirs/attaques ennemis | 48678 | 0.01 | 127 | 35 | 1 | 91 | 9ef20 (27.5 %), a71c2 (13.6 %), a731c (13.4 %) |
| X2 | Dégâts, terrain, placement | 38118 | 0.01 | 19 | 12 | 0 | 7 | 3796 (43.4 %), 3618 (20.6 %), 3636 (18.4 %) |
| C0a | BOX : caisses/coffres (h) | 34174 | 0.01 | 7 | 4 | 0 | 3 | dabba (77.8 %), d9cea (20.1 %), da152 (2.0 %) |
| M2 | EEPROM, configuration, comptabilité | 29430 | 0.01 | 60 | 11 | 13 | 36 | 2d30a (92.5 %), 2e4ec (2.2 %), 2dc9a (1.2 %) |
| C10 | SET : décors/animations de palette | 24513 | 0.01 | 131 | 19 | 4 | 108 | d981a (23.3 %), cdedc (18.9 %), c8312 (15.6 %) |
| P5 | Code par personnage (6 × 2 variantes) | 19791 | 0.01 | 73 | 1 | 0 | 72 | 3e480 (100.0 %) |
| B1 | Scrolling / caméras par scène | 15965 | 0.00 | 42 | 1 | 10 | 31 | 33aa6 (100.0 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 15524 | 0.00 | 49 | 4 | 0 | 45 | 18082 (52.2 %), 1507a (38.8 %), 180c8 (8.9 %) |
| C0c | ITEM : objets ramassables | 14989 | 0.00 | 14 | 5 | 0 | 9 | db6c0 (97.1 %), dcddc (2.3 %), dc5fe (0.3 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 14796 | 0.00 | 3 | 1 | 1 | 1 | 1c44a (100.0 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 8046 | 0.00 | 5 | 3 | 1 | 1 | 18b8e (66.4 %), 18a3e (32.1 %), 18a0e (1.5 %) |
| P1 | Initialisation des joueurs | 6157 | 0.00 | 23 | 9 | 4 | 10 | 35c0 (53.9 %), 3432 (13.0 %), 3308 (10.8 %) |
| P3 | Comportement scénarisé des joueurs par scène | 5346 | 0.00 | 3 | 1 | 2 | 0 | 25b20 (100.0 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 4620 | 0.00 | 13 | 2 | 11 | 0 | c32 (64.1 %), c0c (35.9 %) |
| F1 | Tâche de boot, avertissements région | 4472 | 0.00 | 5 | 3 | 2 | 0 | 133dc (97.4 %), 133f6 (1.3 %), 133f0 (1.3 %) |
| U1 | RNG et files d'affichage (pop-ups) | 3570 | 0.00 | 18 | 1 | 13 | 4 | 19eb0 (100.0 %) |
| C12 | PEFF : effets du joueur | 3450 | 0.00 | 107 | 10 | 27 | 70 | 51c06 (44.6 %), 4b7a0 (19.6 %), 4dc9a (13.0 %) |
| C02 | ENEMY : ennemis | 3007 | 0.00 | 205 | 10 | 1 | 194 | 5cdc6 (43.5 %), 57c66 (21.4 %), 57c8c (13.8 %) |
| C06 | PSHL : tirs du joueur | 2967 | 0.00 | 39 | 6 | 12 | 21 | 53e48 (47.6 %), 52578 (18.9 %), 54872 (14.5 %) |
| U2 | Petites routines / tables (rôle inconnu) | 2004 | 0.00 | 17 | 6 | 10 | 1 | 1ad2a (65.9 %), 1ac68 (17.1 %), 1ac86 (15.8 %) |
| C14 | SOBJ : objets système/UI | 1699 | 0.00 | 25 | 7 | 2 | 16 | dfe26 (36.8 %), e03c6 (20.2 %), e03e4 (17.3 %) |
| K3 | Entrées vers enregistrements joueur | 792 | 0.00 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| F6 | Interpréteur de scripts de scène (démos) | 700 | 0.00 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| P6 | Réactions aux coups, saisie des coups spéciaux | 136 | 0.00 | 9 | 1 | 2 | 6 | f18fa (100.0 %) |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 16 | 0 | 0 | 16 |  |
| V4 | Construction de la liste de sprites (OBJ RAM) | 0 | 0.00 | 15 | 0 | 2 | 13 |  |
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
| 001912 | ram_test_block | K2 | 104 | 228970115 | 67.87 | 121 |
| 000350 | reset | K1 | 322 | 5952595 | 1.76 | 121 |
| 00182a | FUN_0000182a | K2 | 90 | 2479895 | 0.74 | 121 |
| 0f71fa | FUN_000f71fa | W3 | 76 | 373733 | 0.11 | 23 |
| 0f7d86 | FUN_000f7d86 | W3 | 116 | 346689 | 0.10 | 15 |
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 312819 | 0.09 | 8 |
| 03bb9c | FUN_0003bb9c | O3 | 170 | 275240 | 0.08 | 84 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 264693 | 0.08 | 8 |
| 03be02 | FUN_0003be02 | O3 | 54 | 234589 | 0.07 | 83 |
| 0f68c8 | FUN_000f68c8 | W3 | 58 | 228482 | 0.07 | 23 |
| 0e05ec | FUN_000e05ec | C1 | 426 | 173138 | 0.05 | 5 |
| 0e7e10 | FUN_000e7e10 | S1 | 58 | 165851 | 0.05 | 117 |
| 029948 | FUN_00029948 | M1 | 76 | 154899 | 0.05 | 1 |
| 02bb0a | FUN_0002bb0a | M1 | 14 | 112872 | 0.03 | 1 |
| 081aa4 | FUN_00081aa4 | C04 | 54 | 99540 | 0.03 | 17 |
| 02ca5e | FUN_0002ca5e | M1 | 72 | 91061 | 0.03 | 1 |
| 029994 | FUN_00029994 | M1 | 76 | 89025 | 0.03 | 1 |
| 0e662a | FUN_000e662a | S1 | 66 | 87240 | 0.03 | 107 |
| 0f683c | FUN_000f683c | W3 | 42 | 85433 | 0.03 | 21 |
| 0f6d94 | FUN_000f6d94 | W3 | 42 | 79084 | 0.02 | 23 |
| 03bb16 | FUN_0003bb16 | O3 | 60 | 69309 | 0.02 | 57 |
| 00146c | FUN_0000146c | V3 | 78 | 64826 | 0.02 | 121 |
| 03c2ee | FUN_0003c2ee | O3 | 14 | 63656 | 0.02 | 67 |
| 0b03c6 | FUN_000b03c6 | C0e | 102 | 57260 | 0.02 | 6 |
| 0b4686 | FUN_000b4686 | C0e | 28 | 57247 | 0.02 | 8 |
| 0f6b72 | FUN_000f6b72 | W3 | 46 | 55020 | 0.02 | 23 |
| 0f67fa | FUN_000f67fa | W3 | 66 | 52731 | 0.02 | 20 |
| 011a36 | FUN_00011a36 | E1 | 104 | 51431 | 0.02 | 9 |
| 000f4a | FUN_00000f4a | V2 | 158 | 49889 | 0.01 | 5 |
| 03de2e | FUN_0003de2e | O3 | 50 | 49050 | 0.01 | 93 |
| 0b5f8e | FUN_000b5f8e | C0e | 40 | 47104 | 0.01 | 5 |
| 03769c | FUN_0003769c | P4 | 58 | 46241 | 0.01 | 47 |
| 00788c | FUN_0000788c | E1 | 34 | 43983 | 0.01 | 96 |
| 01172e | FUN_0001172e | E1 | 92 | 43861 | 0.01 | 23 |
| 0f7916 | FUN_000f7916 | W3 | 24 | 41357 | 0.01 | 21 |
| 0ad60a | FUN_000ad60a | C0e | 52 | 41224 | 0.01 | 12 |
| 029a9a | FUN_00029a9a | M1 | 22 | 41140 | 0.01 | 1 |
| 03dd72 | FUN_0003dd72 | O3 | 14 | 35884 | 0.01 | 39 |
| 0ab2b4 | FUN_000ab2b4 | C0e | 16 | 34385 | 0.01 | 111 |
| 0b13fa | FUN_000b13fa | C0e | 32 | 33920 | 0.01 | 93 |
| 035620 | FUN_00035620 | P4 | 50 | 33816 | 0.01 | 101 |
| 0043bc | FUN_000043bc | E1 | 8 | 31772 | 0.01 | 16 |
| 0ae3bc | FUN_000ae3bc | C0e | 50 | 30386 | 0.01 | 5 |
| 0833b6 | FUN_000833b6 | C04 | 46 | 30013 | 0.01 | 1 |
| 03c63a | FUN_0003c63a | O3 | 18 | 28310 | 0.01 | 42 |
| 0abea4 | FUN_000abea4 | C0e | 36 | 28210 | 0.01 | 2 |
| 0f5cee | FUN_000f5cee | W3 | 38 | 27669 | 0.01 | 20 |
| 0079ce | FUN_000079ce | E1 | 44 | 27540 | 0.01 | 77 |
| 02d30a | FUN_0002d30a | M2 | 294 | 27225 | 0.01 | 121 |
| 03e28e | FUN_0003e28e | O3 | 1038 | 26918 | 0.01 | 93 |
| 0ad832 | FUN_000ad832 | C0e | 26 | 26660 | 0.01 | 5 |
| 0dabba | FUN_000dabba | C0a | 16 | 26600 | 0.01 | 121 |
| 00b048 | FUN_0000b048 | E1 | 74 | 26513 | 0.01 | 107 |
| 0b5f66 | FUN_000b5f66 | C0e | 40 | 26016 | 0.01 | 10 |
| 0aaa04 | FUN_000aaa04 | C0e | 60 | 25708 | 0.01 | 100 |
| 03733c | FUN_0003733c | P4 | 42 | 25621 | 0.01 | 90 |
| 0e7a12 | FUN_000e7a12 | S1 | 58 | 25024 | 0.01 | 94 |
| 036eb0 | FUN_00036eb0 | P4 | 58 | 22542 | 0.01 | 2 |
| 0392b2 | FUN_000392b2 | P4 | 50 | 22004 | 0.01 | 69 |
| 039260 | player_walk_speed | P4 | 62 | 21920 | 0.01 | 3 |
| 0b0990 | FUN_000b0990 | C0e | 80 | 21736 | 0.01 | 6 |
| 03e480 | FUN_0003e480 | P5 | 28 | 19791 | 0.01 | 82 |
| 03e130 | FUN_0003e130 | O3 | 24 | 18908 | 0.01 | 93 |
| 0f6492 | FUN_000f6492 | W3 | 14 | 18680 | 0.01 | 17 |
| 0376ee | FUN_000376ee | P4 | 150 | 18250 | 0.01 | 91 |
| 02ca46 | FUN_0002ca46 | M1 | 22 | 18225 | 0.01 | 1 |
| 0017ea | FUN_000017ea | V3 | 24 | 17978 | 0.01 | 13 |
| 0370b2 | FUN_000370b2 | P4 | 154 | 17797 | 0.01 | 80 |
| 0ec046 | FUN_000ec046 | S1 | 56 | 17712 | 0.01 | 88 |
| 037018 | FUN_00037018 | P4 | 154 | 17158 | 0.01 | 81 |
| 0e7622 | FUN_000e7622 | S1 | 58 | 16830 | 0.00 | 107 |
| 0f6b40 | FUN_000f6b40 | W3 | 38 | 16576 | 0.00 | 23 |
| 003796 | FUN_00003796 | X2 | 244 | 16548 | 0.00 | 29 |
| 0b0390 | FUN_000b0390 | C0e | 38 | 15979 | 0.00 | 6 |
| 033aa6 | FUN_00033aa6 | B1 | 126 | 15965 | 0.00 | 120 |
| 0f6f8c | FUN_000f6f8c | W3 | 96 | 15767 | 0.00 | 23 |
| 0f5d56 | FUN_000f5d56 | W3 | 44 | 15663 | 0.00 | 20 |
| 03bb84 | FUN_0003bb84 | O3 | 2 | 14800 | 0.00 | 82 |
| 01c44a | FUN_0001c44a | W1 | 16 | 14796 | 0.00 | 121 |
| 0db6c0 | FUN_000db6c0 | C0c | 248 | 14550 | 0.00 | 108 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 497 pour toutes les fonctions touchées.

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
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 45 | 3874 |
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
| U1 | RNG et files d'affichage (pop-ups) | 4 | 296 |
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
| scenarios/boss/boss10_scene65.txt | 12000 | 2249110 | 187.4 |
| scenarios/boss/boss10_scene66.txt | 12000 | 2006820 | 167.2 |
| scenarios/boss/boss11_scene42.txt | 12000 | 2462347 | 205.2 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 4104111 | 342.0 |
| scenarios/boss/boss12_scene40.txt | 12000 | 2444336 | 203.7 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 4091985 | 341.0 |
| scenarios/boss/boss13_dragon.txt | 12000 | 2434664 | 202.9 |
| scenarios/boss/boss15_scene46.txt | 12000 | 3023645 | 252.0 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 4264587 | 355.4 |
| scenarios/boss/boss17_scene13.txt | 12000 | 2475312 | 206.3 |
| scenarios/boss/boss18_scene26.txt | 12000 | 2675612 | 223.0 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 2298586 | 191.5 |
| scenarios/boss/boss19_scene32.txt | 12000 | 2138147 | 178.2 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 4122409 | 343.5 |
| scenarios/boss/boss21_scene44.txt | 12000 | 2376814 | 198.1 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 3990104 | 332.5 |
| scenarios/boss/boss5_scene58.txt | 12000 | 2108096 | 175.7 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 3929201 | 327.4 |
| scenarios/boss/boss6_scene24.txt | 12000 | 2568067 | 214.0 |
| scenarios/boss/boss8_scene64.txt | 12000 | 2113307 | 176.1 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 3294585 | 274.5 |
| scenarios/bot30k.txt | 30000 | 2000384 | 66.7 |
| scenarios/bot_2p.txt | 30000 | 1992772 | 66.4 |
| scenarios/bot_char0.txt | 30000 | 2006092 | 66.9 |
| scenarios/bot_char1.txt | 30000 | 2012216 | 67.1 |
| scenarios/bot_char2.txt | 30000 | 1986107 | 66.2 |
| scenarios/bot_char3.txt | 30000 | 2006583 | 66.9 |
| scenarios/bot_char4.txt | 30000 | 2005405 | 66.8 |
| scenarios/bot_char5.txt | 30000 | 1993859 | 66.5 |
| scenarios/cov/cov_2p_a.txt | 30000 | 5182305 | 172.7 |
| scenarios/cov/cov_2p_b.txt | 30000 | 1988012 | 66.3 |
| scenarios/cov/cov_2p_c.txt | 30000 | 11703396 | 390.1 |
| scenarios/cov/cov_3p.txt | 30000 | 5356927 | 178.6 |
| scenarios/cov/cov_4p_a.txt | 30000 | 4579703 | 152.7 |
| scenarios/cov/cov_4p_b.txt | 30000 | 2015883 | 67.2 |
| scenarios/cov/cov_4p_c.txt | 30000 | 2021473 | 67.4 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 2466761 | 82.2 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 1991632 | 66.4 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 2657703 | 88.6 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 1991632 | 66.4 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 1991632 | 66.4 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 1991613 | 66.4 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 1991632 | 66.4 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 1991632 | 66.4 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 1997898 | 66.6 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 2004537 | 66.8 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 2819629 | 94.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 3188757 | 106.3 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 1985705 | 66.2 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 8397083 | 279.9 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 2182815 | 72.8 |
| scenarios/cov/cov_service.txt | 30000 | 3566149 | 118.9 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 3283893 | 109.5 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 1988352 | 66.3 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 4949158 | 165.0 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 2926430 | 97.5 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 3226940 | 107.6 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 4633261 | 154.4 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 5733558 | 191.1 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 2112952 | 70.4 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 2056902 | 171.4 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 2000209 | 166.7 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 1985503 | 165.5 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 2052408 | 171.0 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 2084307 | 173.7 |
| scenarios/enemy/elf_scene92.txt | 12000 | 2016824 | 168.1 |
| scenarios/enemy/elf_scene94.txt | 12000 | 2003747 | 167.0 |
| scenarios/enemy/elf_scene96.txt | 12000 | 2158535 | 179.9 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 2462347 | 205.2 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 2023138 | 168.6 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 2540167 | 211.7 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 2079089 | 173.3 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 2060447 | 171.7 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 2011089 | 167.6 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 2032716 | 169.4 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 2096831 | 174.7 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 2340288 | 195.0 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 1985457 | 165.5 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 14711210 | 1225.9 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 2256240 | 188.0 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 2059821 | 171.7 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 2090162 | 174.2 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 1993157 | 166.1 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 2154549 | 179.5 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 14682427 | 1223.5 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 2130499 | 177.5 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 2451557 | 204.3 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 2041813 | 170.2 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 2249110 | 187.4 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 2229481 | 185.8 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 2041933 | 170.2 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 1985675 | 165.5 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 2016121 | 168.0 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 2008548 | 167.4 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 1988020 | 165.7 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 2475312 | 206.3 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 2111850 | 176.0 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 1994412 | 166.2 |
| scenarios/enemy/trog_scene101.txt | 12000 | 1985482 | 165.5 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 1999160 | 166.6 |
| scenarios/enemy/trog_scene104.txt | 12000 | 3179962 | 265.0 |
| scenarios/extra/mash_2p.txt | 12000 | 2049777 | 170.8 |
| scenarios/extra/mash_c0.txt | 12000 | 2050624 | 170.9 |
| scenarios/extra/mash_c0b.txt | 12000 | 2005810 | 167.2 |
| scenarios/extra/mash_c1b.txt | 12000 | 2124965 | 177.1 |
| scenarios/extra/mash_c2.txt | 12000 | 2066794 | 172.2 |
| scenarios/extra/mash_c2b.txt | 12000 | 1999409 | 166.6 |
| scenarios/extra/mash_c3.txt | 12000 | 2020392 | 168.4 |
| scenarios/extra/mash_c3b.txt | 12000 | 2035331 | 169.6 |
| scenarios/extra/mash_c3c.txt | 12000 | 2011393 | 167.6 |
| scenarios/extra/mash_c3d.txt | 12000 | 2014396 | 167.9 |
| scenarios/extra/mash_c4.txt | 12000 | 2008411 | 167.4 |
| scenarios/extra/mash_c4b.txt | 12000 | 2004859 | 167.1 |
| scenarios/extra/mash_c4c.txt | 12000 | 2011105 | 167.6 |
| scenarios/extra/mash_c5.txt | 12000 | 2033126 | 169.4 |
| scenarios/extra/mash_c5b.txt | 12000 | 2058846 | 171.6 |
| scenarios/modes/attract.txt | 12000 | 2466761 | 205.6 |
| scenarios/modes/ending.txt | 12000 | 2605934 | 217.2 |
| scenarios/modes/ending_end.txt | 12000 | 2420726 | 201.7 |
| scenarios/modes/gameover.txt | 12000 | 2005405 | 167.1 |
| scenarios/modes/screens.txt | 12000 | 2466761 | 205.6 |
