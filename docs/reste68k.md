# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 121 scénarios, soit 2154000 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **1133046796 instructions 68000 exécutées** (hors attente active), soit 526.0 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 627 exécutent encore du 68000, 624 sont entièrement natives, **1308 jamais atteintes** (89024 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 486898706.

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---|
| K2 | Auto-test RAM (« RAM ERROR !! ») | 231450010 | 20.43 | 2 | 2 | 0 | 0 | 1912 (98.9 %), 182a (1.1 %) |
| O2 | Pools d'objets et listes triées | 121621125 | 10.73 | 41 | 2 | 26 | 13 | 1ee1a (95.1 %), 1ef6c (4.9 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 98641976 | 8.71 | 49 | 49 | 0 | 0 | 15216 (25.4 %), 156fa (20.5 %), 1728e (12.6 %) |
| B1 | Scrolling / caméras par scène | 86631516 | 7.65 | 42 | 19 | 4 | 19 | 33fe4 (39.9 %), 31616 (12.1 %), 33e20 (7.8 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 36253572 | 3.20 | 69 | 48 | 15 | 6 | fbb0 (69.5 %), e76c (13.1 %), 7f0c (2.6 %) |
| C0e | EFFCT : effets | 34566175 | 3.05 | 170 | 112 | 0 | 58 | b9eba (64.1 %), b9ee6 (26.3 %), ba094 (4.5 %) |
| V2 | Texte, rectangles et blocs de tuiles | 10746071 | 0.95 | 33 | 16 | 12 | 5 | 28d8 (58.1 %), 1386 (21.7 %), 2810 (14.0 %) |
| V4 | Construction de la liste de sprites (OBJ RAM) | 7829035 | 0.69 | 15 | 2 | 0 | 13 | 1de96 (99.9 %), 1de7c (0.1 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 5961428 | 0.53 | 17 | 2 | 12 | 3 | 350 (99.9 %), 492 (0.1 %) |
| S1 | Son : wrappers de commandes + FIFO | 5834825 | 0.51 | 410 | 25 | 379 | 6 | edf80 (93.8 %), e7e10 (2.8 %), e662a (1.5 %) |
| W3 | Scripts de scène (phases 16/28) | 1447437 | 0.13 | 82 | 58 | 3 | 21 | f71fa (25.8 %), f7d86 (24.0 %), f68c8 (15.8 %) |
| C04 | BOSS : boss | 1289039 | 0.11 | 353 | 36 | 1 | 316 | 7f19c (85.0 %), 81aa4 (7.7 %), 833b6 (2.3 %) |
| O3 | Aides communes aux types (animations par type) | 872757 | 0.08 | 25 | 24 | 0 | 1 | 3bb9c (31.5 %), 3be02 (26.9 %), 3bb16 (7.9 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 807082 | 0.07 | 59 | 34 | 2 | 23 | 3525c (63.3 %), 3769c (5.7 %), 35620 (4.2 %) |
| U2 | Petites routines / tables (rôle inconnu) | 523029 | 0.05 | 17 | 12 | 0 | 5 | 1ab56 (94.7 %), 1abe0 (2.4 %), 1ac24 (2.4 %) |
| M1 | Menu TEST (service) | 521903 | 0.05 | 76 | 23 | 0 | 53 | 29948 (29.7 %), 2bb0a (21.6 %), 2ca5e (17.4 %) |
| U1 | RNG et files d'affichage (pop-ups) | 402560 | 0.04 | 18 | 5 | 13 | 0 | 19fd2 (55.6 %), 1a20e (28.9 %), 19f24 (9.9 %) |
| C1 | Crédits, pièces, start/continue | 180220 | 0.02 | 22 | 4 | 13 | 5 | e05ec (96.1 %), 14cb8 (3.0 %), e0538 (0.8 %) |
| C10 | SET : décors/animations de palette | 142773 | 0.01 | 131 | 20 | 3 | 108 | cc18e (82.8 %), d981a (4.0 %), cdedc (3.2 %) |
| V3 | Palettes et fondus | 98352 | 0.01 | 38 | 6 | 27 | 5 | 146c (65.9 %), 17ea (18.3 %), 1ced4 (14.2 %) |
| C08 | ESHL : tirs/attaques ennemis | 48678 | 0.00 | 127 | 35 | 1 | 91 | 9ef20 (27.5 %), a71c2 (13.6 %), a731c (13.4 %) |
| P3 | Comportement scénarisé des joueurs par scène | 46233 | 0.00 | 3 | 2 | 1 | 0 | 25260 (88.4 %), 25b20 (11.6 %) |
| X2 | Dégâts, terrain, placement | 38118 | 0.00 | 19 | 12 | 0 | 7 | 3796 (43.4 %), 3618 (20.6 %), 3636 (18.4 %) |
| C0a | BOX : caisses/coffres (h) | 34174 | 0.00 | 7 | 4 | 0 | 3 | dabba (77.8 %), d9cea (20.1 %), da152 (2.0 %) |
| M2 | EEPROM, configuration, comptabilité | 29430 | 0.00 | 60 | 11 | 13 | 36 | 2d30a (92.5 %), 2e4ec (2.2 %), 2dc9a (1.2 %) |
| C12 | PEFF : effets du joueur | 26874 | 0.00 | 107 | 11 | 26 | 70 | 517ba (87.2 %), 51c06 (5.7 %), 4b7a0 (2.5 %) |
| P5 | Code par personnage (6 × 2 variantes) | 19791 | 0.00 | 73 | 1 | 0 | 72 | 3e480 (100.0 %) |
| C14 | SOBJ : objets système/UI | 16691 | 0.00 | 25 | 9 | 0 | 16 | ddeaa (49.1 %), dfa4a (40.7 %), dfe26 (3.7 %) |
| C0c | ITEM : objets ramassables | 14989 | 0.00 | 14 | 5 | 0 | 9 | db6c0 (97.1 %), dcddc (2.3 %), dc5fe (0.3 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 14796 | 0.00 | 3 | 1 | 1 | 1 | 1c44a (100.0 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 14580 | 0.00 | 5 | 4 | 0 | 1 | 19038 (44.8 %), 18b8e (36.6 %), 18a3e (17.7 %) |
| P1 | Initialisation des joueurs | 6157 | 0.00 | 23 | 9 | 4 | 10 | 35c0 (53.9 %), 3432 (13.0 %), 3308 (10.8 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 4620 | 0.00 | 13 | 2 | 11 | 0 | c32 (64.1 %), c0c (35.9 %) |
| F1 | Tâche de boot, avertissements région | 4472 | 0.00 | 5 | 3 | 2 | 0 | 133dc (97.4 %), 133f6 (1.3 %), 133f0 (1.3 %) |
| C02 | ENEMY : ennemis | 3007 | 0.00 | 205 | 10 | 0 | 195 | 5cdc6 (43.5 %), 57c66 (21.4 %), 57c8c (13.8 %) |
| C06 | PSHL : tirs du joueur | 2967 | 0.00 | 39 | 6 | 12 | 21 | 53e48 (47.6 %), 52578 (18.9 %), 54872 (14.5 %) |
| K3 | Entrées vers enregistrements joueur | 792 | 0.00 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| F6 | Interpréteur de scripts de scène (démos) | 700 | 0.00 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| P6 | Réactions aux coups, saisie des coups spéciaux | 136 | 0.00 | 9 | 1 | 2 | 6 | f18fa (100.0 %) |
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
| 001912 | ram_test_block | K2 | 104 | 228970115 | 20.21 | 121 |
| 01ee1a | caseD_1ee1a | O2 | 338 | 115695211 | 10.21 | 121 |
| 033fe4 | FUN_00033fe4 | B1 | 90 | 34552326 | 3.05 | 120 |
| 00fbb0 | FUN_0000fbb0 | E1 | 148 | 25197280 | 2.22 | 106 |
| 015216 | FUN_00015216 | F3 | 62 | 25101916 | 2.22 | 120 |
| 0b9eba | FUN_000b9eba | C0e | 44 | 22161012 | 1.96 | 5 |
| 0156fa | FUN_000156fa | F3 | 110 | 20182431 | 1.78 | 120 |
| 01728e | FUN_0001728e | F3 | 38 | 12446120 | 1.10 | 120 |
| 017814 | FUN_00017814 | F3 | 92 | 12342810 | 1.09 | 120 |
| 0175a8 | FUN_000175a8 | F3 | 46 | 11744021 | 1.04 | 120 |
| 031616 | FUN_00031616 | B1 | 228 | 10481118 | 0.93 | 65 |
| 0b9ee6 | FUN_000b9ee6 | C0e | 70 | 9093468 | 0.80 | 5 |
| 01de96 | FUN_0001de96 | V4 | 126 | 7818724 | 0.69 | 121 |
| 033e20 | FUN_00033e20 | B1 | 32 | 6739740 | 0.59 | 120 |
| 0028d8 | FUN_000028d8 | V2 | 274 | 6247722 | 0.55 | 120 |
| 030308 | FUN_00030308 | B1 | 126 | 6137976 | 0.54 | 13 |
| 0309e0 | FUN_000309e0 | B1 | 1014 | 5980364 | 0.53 | 118 |
| 000350 | reset | K1 | 322 | 5952595 | 0.53 | 121 |
| 01ef6c | FUN_0001ef6c | O2 | 338 | 5925914 | 0.52 | 26 |
| 0155b2 | FUN_000155b2 | F3 | 70 | 5709858 | 0.50 | 114 |
| 031018 | FUN_00031018 | B1 | 174 | 5539792 | 0.49 | 60 |
| 0edf80 | FUN_000edf80 | S1 | 196 | 5470371 | 0.48 | 121 |
| 00e76c | FUN_0000e76c | E1 | 142 | 4732254 | 0.42 | 91 |
| 016c1a | FUN_00016c1a | F3 | 628 | 4192799 | 0.37 | 120 |
| 033062 | caseD_f7 | B1 | 32 | 3267594 | 0.29 | 52 |
| 030fc8 | FUN_00030fc8 | B1 | 60 | 3114527 | 0.27 | 58 |
| 0332c0 | FUN_000332c0 | B1 | 16 | 3027535 | 0.27 | 120 |
| 00182a | FUN_0000182a | K2 | 90 | 2479895 | 0.22 | 121 |
| 001386 | FUN_00001386 | V2 | 134 | 2336407 | 0.21 | 101 |
| 02ede0 | FUN_0002ede0 | B1 | 2360 | 2282338 | 0.20 | 120 |
| 032108 | FUN_00032108 | B1 | 1616 | 2272273 | 0.20 | 120 |
| 0177a0 | FUN_000177a0 | F3 | 46 | 1994374 | 0.18 | 35 |
| 0ba094 | FUN_000ba094 | C0e | 28 | 1559646 | 0.14 | 5 |
| 002810 | FUN_00002810 | V2 | 20 | 1500870 | 0.13 | 94 |
| 0305fc | FUN_000305fc | B1 | 34 | 1158504 | 0.10 | 39 |
| 07f19c | FUN_0007f19c | C04 | 92 | 1095398 | 0.10 | 12 |
| 016b42 | FUN_00016b42 | F3 | 148 | 1014055 | 0.09 | 117 |
| 007f0c | FUN_00007f0c | E1 | 220 | 949228 | 0.08 | 93 |
| 017a94 | FUN_00017a94 | F3 | 76 | 918576 | 0.08 | 120 |
| 00fbae | FUN_0000fbae | E1 | 2 | 888871 | 0.08 | 106 |
| 00eff2 | FUN_0000eff2 | E1 | 120 | 842230 | 0.07 | 16 |
| 03147c | FUN_0003147c | B1 | 220 | 778010 | 0.07 | 12 |
| 03403c | FUN_0003403c | B1 | 90 | 753602 | 0.07 | 104 |
| 01757e | FUN_0001757e | F3 | 22 | 706648 | 0.06 | 120 |
| 010428 | FUN_00010428 | E1 | 40 | 663957 | 0.06 | 3 |
| 00f0e8 | FUN_0000f0e8 | E1 | 48 | 551610 | 0.05 | 96 |
| 03525c | FUN_0003525c | P4 | 44 | 511024 | 0.05 | 120 |
| 01ab56 | FUN_0001ab56 | U2 | 138 | 495357 | 0.04 | 17 |
| 0b10c8 | FUN_000b10c8 | C0e | 82 | 454082 | 0.04 | 108 |
| 00e886 | FUN_0000e886 | E1 | 242 | 419271 | 0.04 | 15 |
| 000d5e | FUN_00000d5e | V2 | 108 | 392776 | 0.03 | 116 |
| 0f71fa | FUN_000f71fa | W3 | 76 | 373733 | 0.03 | 23 |
| 016aec | FUN_00016aec | F3 | 126 | 367702 | 0.03 | 117 |
| 0f7d86 | FUN_000f7d86 | W3 | 116 | 346689 | 0.03 | 15 |
| 0170ca | FUN_000170ca | F3 | 22 | 332812 | 0.03 | 120 |
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 312819 | 0.03 | 8 |
| 03bb9c | FUN_0003bb9c | O3 | 170 | 275240 | 0.02 | 84 |
| 0173ee | FUN_000173ee | F3 | 18 | 274490 | 0.02 | 36 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 264693 | 0.02 | 8 |
| 0115e2 | FUN_000115e2 | E1 | 58 | 253028 | 0.02 | 9 |
| 03be02 | FUN_0003be02 | O3 | 54 | 234589 | 0.02 | 83 |
| 033290 | FUN_00033290 | B1 | 38 | 230940 | 0.02 | 13 |
| 0f68c8 | FUN_000f68c8 | W3 | 58 | 228174 | 0.02 | 23 |
| 00a470 | FUN_0000a470 | E1 | 28 | 227968 | 0.02 | 96 |
| 019fd2 | FUN_00019fd2 | U1 | 72 | 223651 | 0.02 | 74 |
| 030300 | FUN_00030300 | B1 | 8 | 215490 | 0.02 | 48 |
| 011654 | FUN_00011654 | E1 | 58 | 203293 | 0.02 | 9 |
| 017b40 | FUN_00017b40 | F3 | 86 | 201720 | 0.02 | 120 |
| 007fe8 | FUN_00007fe8 | E1 | 4 | 197544 | 0.02 | 93 |
| 017236 | FUN_00017236 | F3 | 40 | 179340 | 0.02 | 120 |
| 01161c | FUN_0001161c | E1 | 56 | 176028 | 0.02 | 9 |
| 0e05ec | FUN_000e05ec | C1 | 426 | 173138 | 0.02 | 5 |
| 0e7e10 | FUN_000e7e10 | S1 | 58 | 165851 | 0.01 | 117 |
| 029948 | FUN_00029948 | M1 | 76 | 154899 | 0.01 | 1 |
| 015444 | FUN_00015444 | F3 | 164 | 150476 | 0.01 | 6 |
| 017034 | FUN_00017034 | F3 | 72 | 145852 | 0.01 | 120 |
| 00fda2 | FUN_0000fda2 | E1 | 146 | 139785 | 0.01 | 11 |
| 00137e | FUN_0000137e | V2 | 8 | 132648 | 0.01 | 101 |
| 007ef4 | FUN_00007ef4 | E1 | 24 | 132434 | 0.01 | 25 |
| 0cc18e | FUN_000cc18e | C10 | 52 | 118260 | 0.01 | 12 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 627 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 316 | 16240 |
| C10 | SET : décors/animations de palette | 108 | 10956 |
| C02 | ENEMY : ennemis | 195 | 9172 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 91 | 5572 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| C0e | EFFCT : effets | 58 | 2722 |
| M1 | Menu TEST (service) | 53 | 2386 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| P4 | Noyau joueur (états, 26 actions génériques) | 23 | 1876 |
| C06 | PSHL : tirs du joueur | 21 | 1726 |
| M2 | EEPROM, configuration, comptabilité | 36 | 1696 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 6 | 1500 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C0c | ITEM : objets ramassables | 9 | 1254 |
| B1 | Scrolling / caméras par scène | 19 | 888 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| C14 | SOBJ : objets système/UI | 16 | 736 |
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
| scenarios/boss/boss10_scene65.txt | 12000 | 9079019 | 756.6 |
| scenarios/boss/boss10_scene66.txt | 12000 | 3030879 | 252.6 |
| scenarios/boss/boss11_scene42.txt | 12000 | 10577368 | 881.4 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 10770230 | 897.5 |
| scenarios/boss/boss12_scene40.txt | 12000 | 10482579 | 873.5 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 8694278 | 724.5 |
| scenarios/boss/boss13_dragon.txt | 12000 | 8301289 | 691.8 |
| scenarios/boss/boss15_scene46.txt | 12000 | 8983689 | 748.6 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 8289295 | 690.8 |
| scenarios/boss/boss17_scene13.txt | 12000 | 8125294 | 677.1 |
| scenarios/boss/boss18_scene26.txt | 12000 | 8535710 | 711.3 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 6825708 | 568.8 |
| scenarios/boss/boss19_scene32.txt | 12000 | 8696867 | 724.7 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 8787821 | 732.3 |
| scenarios/boss/boss21_scene44.txt | 12000 | 8508685 | 709.1 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 10250453 | 854.2 |
| scenarios/boss/boss5_scene58.txt | 12000 | 9352952 | 779.4 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 10847202 | 903.9 |
| scenarios/boss/boss6_scene24.txt | 12000 | 8382936 | 698.6 |
| scenarios/boss/boss8_scene64.txt | 12000 | 9766021 | 813.8 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 9622319 | 801.9 |
| scenarios/bot30k.txt | 30000 | 6359712 | 212.0 |
| scenarios/bot_2p.txt | 30000 | 5277033 | 175.9 |
| scenarios/bot_char0.txt | 30000 | 5907916 | 196.9 |
| scenarios/bot_char1.txt | 30000 | 8782738 | 292.8 |
| scenarios/bot_char2.txt | 30000 | 4139881 | 138.0 |
| scenarios/bot_char3.txt | 30000 | 6145507 | 204.9 |
| scenarios/bot_char4.txt | 30000 | 5644436 | 188.1 |
| scenarios/bot_char5.txt | 30000 | 5600313 | 186.7 |
| scenarios/cov/cov_2p_a.txt | 30000 | 22290730 | 743.0 |
| scenarios/cov/cov_2p_b.txt | 30000 | 2936077 | 97.9 |
| scenarios/cov/cov_2p_c.txt | 30000 | 37450978 | 1248.4 |
| scenarios/cov/cov_3p.txt | 30000 | 29195863 | 973.2 |
| scenarios/cov/cov_4p_a.txt | 30000 | 28426244 | 947.5 |
| scenarios/cov/cov_4p_b.txt | 30000 | 3828815 | 127.6 |
| scenarios/cov/cov_4p_c.txt | 30000 | 3115973 | 103.9 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 4612642 | 153.8 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 8590295 | 286.3 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 10562271 | 352.1 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 8587656 | 286.3 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 8618248 | 287.3 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 8555457 | 285.2 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 8587656 | 286.3 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 8618248 | 287.3 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 9726157 | 324.2 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 12076899 | 402.6 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 15786483 | 526.2 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 16458353 | 548.6 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 3246002 | 108.2 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 20267322 | 675.6 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 19259728 | 642.0 |
| scenarios/cov/cov_service.txt | 30000 | 3666902 | 122.2 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 19116090 | 637.2 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 2284750 | 76.2 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 21017676 | 700.6 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 13641224 | 454.7 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 18934529 | 631.2 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 19595151 | 653.2 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 22232033 | 741.1 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 2698259 | 89.9 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 10314168 | 859.5 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 6970610 | 580.9 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 2263375 | 188.6 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 8928654 | 744.1 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 8681606 | 723.5 |
| scenarios/enemy/elf_scene92.txt | 12000 | 7197672 | 599.8 |
| scenarios/enemy/elf_scene94.txt | 12000 | 10479104 | 873.3 |
| scenarios/enemy/elf_scene96.txt | 12000 | 9411335 | 784.3 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 10577368 | 881.4 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 2825721 | 235.5 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 14436816 | 1203.1 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 8400868 | 700.1 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 2925043 | 243.8 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 7261678 | 605.1 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 7140534 | 595.0 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 6880221 | 573.4 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 10047032 | 837.3 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 2273381 | 189.4 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 33317977 | 2776.5 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 10695888 | 891.3 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 4248818 | 354.1 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 7406155 | 617.2 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 2404475 | 200.4 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 8863178 | 738.6 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 33193536 | 2766.1 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 9281018 | 773.4 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 8560582 | 713.4 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 7252629 | 604.4 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 9079019 | 756.6 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 9175002 | 764.6 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 8261248 | 688.4 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 2914575 | 242.9 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 7752404 | 646.0 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 8701514 | 725.1 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 10121097 | 843.4 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 8125294 | 677.1 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 9455916 | 788.0 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 2880676 | 240.1 |
| scenarios/enemy/trog_scene101.txt | 12000 | 2278444 | 189.9 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 6778428 | 564.9 |
| scenarios/enemy/trog_scene104.txt | 12000 | 13606299 | 1133.9 |
| scenarios/extra/mash_2p.txt | 12000 | 7212628 | 601.1 |
| scenarios/extra/mash_c0.txt | 12000 | 7745138 | 645.4 |
| scenarios/extra/mash_c0b.txt | 12000 | 4954841 | 412.9 |
| scenarios/extra/mash_c1b.txt | 12000 | 8050709 | 670.9 |
| scenarios/extra/mash_c2.txt | 12000 | 5160118 | 430.0 |
| scenarios/extra/mash_c2b.txt | 12000 | 4214499 | 351.2 |
| scenarios/extra/mash_c3.txt | 12000 | 7671939 | 639.3 |
| scenarios/extra/mash_c3b.txt | 12000 | 7880724 | 656.7 |
| scenarios/extra/mash_c3c.txt | 12000 | 5582833 | 465.2 |
| scenarios/extra/mash_c3d.txt | 12000 | 5500344 | 458.4 |
| scenarios/extra/mash_c4.txt | 12000 | 5713932 | 476.2 |
| scenarios/extra/mash_c4b.txt | 12000 | 5240181 | 436.7 |
| scenarios/extra/mash_c4c.txt | 12000 | 5595771 | 466.3 |
| scenarios/extra/mash_c5.txt | 12000 | 7288989 | 607.4 |
| scenarios/extra/mash_c5b.txt | 12000 | 8172657 | 681.1 |
| scenarios/modes/attract.txt | 12000 | 4612642 | 384.4 |
| scenarios/modes/ending.txt | 12000 | 7071968 | 589.3 |
| scenarios/modes/ending_end.txt | 12000 | 6051606 | 504.3 |
| scenarios/modes/gameover.txt | 12000 | 5590436 | 465.9 |
| scenarios/modes/screens.txt | 12000 | 4612642 | 384.4 |
