# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 200 scénarios, soit 2851500 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **92987040 instructions 68000 exécutées** (hors attente active), soit 32.6 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 316 exécutent encore du 68000, 774 sont entièrement natives, **1469 jamais atteintes** (100634 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 90948957 (comptées par sous-système ci-dessous, colonne « Hors index », et par plage plus bas).

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Hors index | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| C0e | EFFCT : effets | 91112876 | 97.98 | 89324025 | 170 | 111 | 5 | 54 | b46a2 (27.5 %), b46cc (23.3 %), b03c6 (5.2 %) |
| C0c | ITEM : objets ramassables | 782747 | 0.84 | 782295 | 14 | 4 | 1 | 9 | dcddc (76.5 %), dc5fe (11.9 %), dcb9a (6.6 %) |
| C04 | BOSS : boss | 749598 | 0.81 | 693341 | 353 | 46 | 5 | 302 | 930a4 (23.1 %), 830b2 (18.5 %), 93158 (17.2 %) |
| C0a | BOX : caisses/coffres (h) | 67603 | 0.07 | 58677 | 7 | 3 | 1 | 3 | d9cea (88.3 %), da152 (11.3 %), da04e (0.4 %) |
| W3 | Scripts de scène (phases 16/28) | 60449 | 0.07 | 48775 | 82 | 24 | 9 | 49 | fa270 (82.6 %), f9e38 (5.6 %), f9744 (2.4 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 32920 | 0.04 | 10290 | 49 | 4 | 0 | 45 | 18082 (59.8 %), 1507a (34.0 %), 180c8 (6.1 %) |
| C08 | ESHL : tirs/attaques ennemis | 26134 | 0.03 | 5836 | 127 | 27 | 2 | 98 | a7b0a (19.7 %), a7570 (19.5 %), a4e84 (11.6 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 23059 | 0.02 | 15 | 3 | 2 | 0 | 1 | 1c44a (100.0 %), 1c86e (0.0 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 22863 | 0.02 | 16710 | 59 | 8 | 5 | 46 | 37882 (40.6 %), 3b960 (25.7 %), 3a98e (15.7 %) |
| B1 | Scrolling / caméras par scène | 19561 | 0.02 | 0 | 42 | 1 | 10 | 31 | 33aa6 (100.0 %) |
| C10 | SET : décors/animations de palette | 15190 | 0.02 | 665 | 131 | 19 | 7 | 105 | d9778 (24.0 %), d6182 (16.9 %), ce00a (15.9 %) |
| C1 | Crédits, pièces, start/continue | 13944 | 0.01 | 2939 | 22 | 3 | 13 | 6 | 14cb8 (80.8 %), e0538 (16.7 %), e05d6 (2.6 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 13292 | 0.01 | 0 | 69 | 4 | 55 | 10 | bad6 (93.9 %), 103ba (2.2 %), fffc (2.0 %) |
| F1 | Tâche de boot, avertissements région | 7384 | 0.01 | 0 | 5 | 3 | 2 | 0 | 133dc (98.5 %), 133f6 (0.8 %), 133f0 (0.7 %) |
| V1 | Primitives vidéo (attente, couches, effacements) | 7130 | 0.01 | 0 | 13 | 2 | 11 | 0 | c32 (63.8 %), c0c (36.2 %) |
| C02 | ENEMY : ennemis | 6200 | 0.01 | 1936 | 205 | 10 | 1 | 194 | 5cdc6 (50.7 %), 57c66 (20.0 %), 57c8c (11.0 %) |
| V2 | Texte, rectangles et blocs de tuiles | 5688 | 0.01 | 192 | 33 | 4 | 24 | 5 | 27ee (98.2 %), 2a52 (1.1 %), 2a42 (0.7 %) |
| U1 | RNG et files d'affichage (pop-ups) | 4658 | 0.01 | 0 | 18 | 1 | 13 | 4 | 19eb0 (100.0 %) |
| U2 | Petites routines / tables (rôle inconnu) | 3396 | 0.00 | 832 | 17 | 6 | 10 | 1 | 1ad2a (72.1 %), 1ac68 (13.5 %), 1ac86 (13.2 %) |
| C06 | PSHL : tirs du joueur | 3062 | 0.00 | 0 | 39 | 6 | 12 | 21 | 53e48 (47.7 %), 52578 (18.3 %), 54872 (15.4 %) |
| C14 | SOBJ : objets système/UI | 2517 | 0.00 | 59 | 25 | 7 | 2 | 16 | dfe26 (34.8 %), df688 (22.6 %), e03c6 (16.8 %) |
| S1 | Son : wrappers de commandes + FIFO | 2396 | 0.00 | 2348 | 410 | 3 | 401 | 6 | e27da (45.8 %), edee4 (45.8 %), e2430 (8.3 %) |
| F6 | Interpréteur de scripts de scène (démos) | 1184 | 0.00 | 0 | 18 | 1 | 6 | 11 | 20de6 (100.0 %) |
| K3 | Entrées vers enregistrements joueur | 972 | 0.00 | 0 | 3 | 1 | 2 | 0 | 2bdc (100.0 %) |
| M2 | EEPROM, configuration, comptabilité | 873 | 0.00 | 0 | 60 | 4 | 16 | 40 | 2e630 (53.8 %), 2e67c (26.9 %), 2e696 (12.8 %) |
| C12 | PEFF : effets du joueur | 863 | 0.00 | 0 | 107 | 7 | 30 | 70 | 4cfbc (35.2 %), 4b8d0 (21.9 %), 4c930 (16.7 %) |
| P1 | Initialisation des joueurs | 196 | 0.00 | 0 | 23 | 1 | 5 | 17 | 3552 (100.0 %) |
| W0 | Dispatch des 10 classes d'objets | 119 | 0.00 | 0 | 10 | 1 | 9 | 0 | 39e86 (100.0 %) |
| V3 | Palettes et fondus | 104 | 0.00 | 0 | 38 | 1 | 30 | 7 | 1a5e8 (100.0 %) |
| F4 | Exécuteur d'écrans fixes (logos) | 34 | 0.00 | 0 | 5 | 1 | 1 | 3 | 18b8e (100.0 %) |
| P3 | Comportement scénarisé des joueurs par scène | 22 | 0.00 | 22 | 3 | 0 | 2 | 1 |  |
| O3 | Aides communes aux types (animations par type) | 6 | 0.00 | 0 | 25 | 1 | 8 | 16 | 3e28e (100.0 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 0 | 0.00 | 0 | 17 | 0 | 14 | 3 |  |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 0 | 0.00 | 0 | 2 | 0 | 0 | 2 |  |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 0 | 16 | 0 | 0 | 16 |  |
| V4 | Construction de la liste de sprites (OBJ RAM) | 0 | 0.00 | 0 | 15 | 0 | 2 | 13 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 0 | 22 | 0 | 22 | 0 |  |
| O2 | Pools d'objets et listes triées | 0 | 0.00 | 0 | 41 | 0 | 28 | 13 |  |
| X1 | Collisions (boîtes, coups, poussée) | 0 | 0.00 | 0 | 39 | 0 | 0 | 39 |  |
| X2 | Dégâts, terrain, placement | 0 | 0.00 | 0 | 19 | 0 | 6 | 13 |  |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 0 | 0.00 | 0 | 1 | 0 | 0 | 1 |  |
| P5 | Code par personnage (6 × 2 variantes) | 0 | 0.00 | 0 | 73 | 0 | 1 | 72 |  |
| P6 | Réactions aux coups, saisie des coups spéciaux | 0 | 0.00 | 0 | 9 | 0 | 2 | 7 |  |
| M1 | Menu TEST (service) | 0 | 0.00 | 0 | 76 | 0 | 0 | 76 |  |
| M3 | « Secret Test » (debug) et Block Test | 0 | 0.00 | 0 | 29 | 0 | 0 | 29 |  |
| F2 | Machine à modes (attract) | 0 | 0.00 | 0 | 0 | 0 | 0 | 0 |  |
| F5 | Curseur de sélection de personnage | 0 | 0.00 | 0 | 10 | 0 | 0 | 10 |  |
| F7 | Écrans titre / copyright | 0 | 0.00 | 0 | 5 | 0 | 1 | 4 |  |
| - | hors atlas | 0 | 0.00 | 0 | 0 | 0 | 0 | 0 |  |

## Hors de toute fonction de l'index (les 40 plages les plus gourmandes)

Code que Ghidra n'a pas découpé en fonctions (gestionnaires atteints par table, par exemple) : à désassembler depuis le listing. Plages contiguës à moins de 0x40 octets.

| Plage | Sous-système | Instr. 68000 | % |
|---|---|---:|---:|
| 0b9d7e-0b9eb8 | C0e | 37424156 | 40.25 |
| 0afa46-0afac8 | C0e | 15396926 | 16.56 |
| 0aeff0-0af2b6 | C0e | 8577065 | 9.22 |
| 0abccc-0abdfa | C0e | 3511902 | 3.78 |
| 0b6212-0b64aa | C0e | 3044064 | 3.27 |
| 0af3f2-0af4be | C0e | 2189994 | 2.36 |
| 0b9f36-0b9fe2 | C0e | 1807524 | 1.94 |
| 0b1bb6-0b1c02 | C0e | 1780398 | 1.91 |
| 0b2f46-0b372e | C0e | 1713031 | 1.84 |
| 0b5fb6-0b615a | C0e | 1665035 | 1.79 |
| 0ad600-0ad7f6 | C0e | 1432575 | 1.54 |
| 0b46ec-0b492e | C0e | 1312169 | 1.41 |
| 0ad866-0adaca | C0e | 1016371 | 1.09 |
| 0b6d48-0b7790 | C0e | 1006709 | 1.08 |
| 0b5dbe-0b5f60 | C0e | 775732 | 0.83 |
| 0b9a6a-0b9c62 | C0e | 674600 | 0.73 |
| 0b1abc-0b1b60 | C0e | 584698 | 0.63 |
| 0b1818-0b1a74 | C0e | 436651 | 0.47 |
| 0b40f8-0b417c | C0e | 434803 | 0.47 |
| 0ab414-0ab4dc | C0e | 394812 | 0.42 |
| 0b4b66-0b4bac | C0e | 368440 | 0.40 |
| 0b45da-0b4682 | C0e | 344420 | 0.37 |
| 081b3a-0820b0 | C04 | 316473 | 0.34 |
| 0ae52a-0ae6fa | C0e | 307178 | 0.33 |
| 0db18a-0db29e | C0c | 292308 | 0.31 |
| 0b9956-0b9a24 | C0e | 291345 | 0.31 |
| 0ad476-0ad530 | C0e | 284780 | 0.31 |
| 0db31a-0db38a | C0c | 245363 | 0.26 |
| 0aed4c-0aef4c | C0e | 228488 | 0.25 |
| 0b802a-0b81d4 | C0e | 217275 | 0.23 |
| 0abe4e-0abe52 | C0e | 214848 | 0.23 |
| 0db0c2-0db134 | C0c | 166038 | 0.18 |
| 0ab2d4-0ab3d0 | C0e | 156427 | 0.17 |
| 0859f8-085cc2 | C04 | 148861 | 0.16 |
| 0b4ac0-0b4aec | C0e | 130872 | 0.14 |
| 0b2dea-0b2eec | C0e | 122314 | 0.13 |
| 0ac90a-0ac94a | C0e | 121768 | 0.13 |
| 0b24d6-0b2802 | C0e | 120580 | 0.13 |
| 0abeea-0abef6 | C0e | 99530 | 0.11 |
| 0afe20-0b0032 | C0e | 97283 | 0.10 |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 492635 | 0.53 | 11 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 416845 | 0.45 | 11 |
| 0b03c6 | FUN_000b03c6 | C0e | 102 | 93632 | 0.10 | 9 |
| 0b5f8e | FUN_000b5f8e | C0e | 40 | 78080 | 0.08 | 12 |
| 0ae3bc | FUN_000ae3bc | C0e | 50 | 72646 | 0.08 | 11 |
| 0abea4 | FUN_000abea4 | C0e | 36 | 69790 | 0.08 | 3 |
| 0b4686 | FUN_000b4686 | C0e | 28 | 57316 | 0.06 | 11 |
| 0b0990 | FUN_000b0990 | C0e | 80 | 46379 | 0.05 | 9 |
| 0ad60a | FUN_000ad60a | C0e | 52 | 42176 | 0.05 | 13 |
| 0ab2b4 | FUN_000ab2b4 | C0e | 16 | 41170 | 0.04 | 158 |
| 0b13fa | FUN_000b13fa | C0e | 32 | 39870 | 0.04 | 124 |
| 0ad832 | FUN_000ad832 | C0e | 26 | 35772 | 0.04 | 8 |
| 0aaa04 | FUN_000aaa04 | C0e | 60 | 33110 | 0.04 | 143 |
| 0b5f66 | FUN_000b5f66 | C0e | 40 | 30976 | 0.03 | 18 |
| 0b0390 | FUN_000b0390 | C0e | 38 | 28899 | 0.03 | 9 |
| 01c44a | FUN_0001c44a | W1 | 16 | 23040 | 0.02 | 200 |
| 033aa6 | FUN_00033aa6 | B1 | 126 | 19561 | 0.02 | 196 |
| 0ae776 | FUN_000ae776 | C0e | 22 | 19200 | 0.02 | 192 |
| 0ae298 | FUN_000ae298 | C0e | 228 | 18076 | 0.02 | 10 |
| 0b40ba | FUN_000b40ba | C0e | 62 | 16236 | 0.02 | 6 |
| 0b4af6 | FUN_000b4af6 | C0e | 96 | 15512 | 0.02 | 6 |
| 018082 | FUN_00018082 | F3 | 42 | 13534 | 0.01 | 200 |
| 0930a4 | FUN_000930a4 | C04 | 62 | 12976 | 0.01 | 3 |
| 0b3556 | FUN_000b3556 | C0e | 26 | 12770 | 0.01 | 10 |
| 00bad6 | FUN_0000bad6 | E1 | 1104 | 12479 | 0.01 | 88 |
| 0830b2 | FUN_000830b2 | C04 | 28 | 10428 | 0.01 | 3 |
| 093158 | FUN_00093158 | C04 | 42 | 9688 | 0.01 | 3 |
| 0fa270 | FUN_000fa270 | W3 | 38 | 9648 | 0.01 | 3 |
| 014cb8 | FUN_00014cb8 | C1 | 188 | 8888 | 0.01 | 200 |
| 0d9cea | FUN_000d9cea | C0a | 78 | 7884 | 0.01 | 3 |
| 01507a | FUN_0001507a | F3 | 50 | 7704 | 0.01 | 196 |
| 0133dc | FUN_000133dc | F1 | 20 | 7272 | 0.01 | 200 |
| 0ae3ec | FUN_000ae3ec | C0e | 60 | 7164 | 0.01 | 106 |
| 0b3f6a | FUN_000b3f6a | C0e | 70 | 6935 | 0.01 | 3 |
| 0b4552 | FUN_000b4552 | C0e | 88 | 5975 | 0.01 | 3 |
| 0b20b8 | FUN_000b20b8 | C0e | 50 | 5664 | 0.01 | 27 |
| 0b1a7a | FUN_000b1a7a | C0e | 30 | 5551 | 0.01 | 57 |
| 0027ee | FUN_000027ee | V2 | 34 | 5395 | 0.01 | 92 |
| 0ab7d8 | FUN_000ab7d8 | C0e | 34 | 5039 | 0.01 | 26 |
| 0b230e | FUN_000b230e | C0e | 38 | 4984 | 0.01 | 27 |
| 0abb70 | FUN_000abb70 | C0e | 126 | 4742 | 0.01 | 41 |
| 019eb0 | FUN_00019eb0 | U1 | 48 | 4658 | 0.01 | 79 |
| 000c32 | FUN_00000c32 | V1 | 38 | 4550 | 0.00 | 200 |
| 0b0102 | FUN_000b0102 | C0e | 224 | 4381 | 0.00 | 8 |
| 0aabf4 | FUN_000aabf4 | C0e | 122 | 4380 | 0.00 | 22 |
| 0ae748 | FUN_000ae748 | C0e | 14 | 4025 | 0.00 | 5 |
| 0a7b0a | FUN_000a7b0a | C08 | 92 | 4000 | 0.00 | 7 |
| 0a7570 | FUN_000a7570 | C08 | 72 | 3952 | 0.00 | 6 |
| 0b6596 | FUN_000b6596 | C0e | 64 | 3930 | 0.00 | 43 |
| 087ccc | FUN_00087ccc | C04 | 50 | 3816 | 0.00 | 2 |
| 0d9778 | FUN_000d9778 | C10 | 20 | 3479 | 0.00 | 106 |
| 0b222e | FUN_000b222e | C0e | 18 | 3444 | 0.00 | 27 |
| 0b4088 | FUN_000b4088 | C0e | 50 | 3410 | 0.00 | 6 |
| 0afe02 | FUN_000afe02 | C0e | 30 | 3283 | 0.00 | 9 |
| 0b2212 | FUN_000b2212 | C0e | 28 | 3232 | 0.00 | 27 |
| 0b212c | FUN_000b212c | C0e | 114 | 3044 | 0.00 | 27 |
| 083094 | FUN_00083094 | C04 | 30 | 2982 | 0.00 | 3 |
| 087c9a | FUN_00087c9a | C04 | 50 | 2765 | 0.00 | 2 |
| 000c0c | FUN_00000c0c | V1 | 38 | 2580 | 0.00 | 200 |
| 0b1ff8 | FUN_000b1ff8 | C0e | 52 | 2542 | 0.00 | 27 |
| 0abec8 | FUN_000abec8 | C0e | 34 | 2520 | 0.00 | 50 |
| 037882 | FUN_00037882 | P4 | 40 | 2497 | 0.00 | 38 |
| 0d6182 | FUN_000d6182 | C10 | 18 | 2448 | 0.00 | 6 |
| 0a4e84 | FUN_000a4e84 | C08 | 38 | 2346 | 0.00 | 5 |
| 081722 | FUN_00081722 | C04 | 26 | 2310 | 0.00 | 1 |
| 0ce00a | FUN_000ce00a | C10 | 26 | 2310 | 0.00 | 195 |
| 0af346 | FUN_000af346 | C0e | 26 | 2304 | 0.00 | 192 |
| 0b9aa0 | FUN_000b9aa0 | C0e | 26 | 2227 | 0.00 | 7 |
| 05cdc6 | FUN_0005cdc6 | C02 | 26 | 2163 | 0.00 | 20 |
| 0af2c0 | FUN_000af2c0 | C0e | 66 | 2080 | 0.00 | 19 |
| 0ba18a | FUN_000ba18a | C0e | 34 | 2080 | 0.00 | 1 |
| 085cf2 | FUN_00085cf2 | C04 | 2 | 1978 | 0.00 | 2 |
| 01ad2a | FUN_0001ad2a | U2 | 32 | 1848 | 0.00 | 7 |
| 0e0538 | FUN_000e0538 | C1 | 86 | 1835 | 0.00 | 196 |
| 0b202c | FUN_000b202c | C0e | 76 | 1736 | 0.00 | 27 |
| 0af302 | FUN_000af302 | C0e | 68 | 1710 | 0.00 | 2 |
| 0cc0ca | FUN_000cc0ca | C10 | 194 | 1698 | 0.00 | 10 |
| 08603e | FUN_0008603e | C04 | 122 | 1676 | 0.00 | 2 |
| 0b3fd4 | FUN_000b3fd4 | C0e | 46 | 1641 | 0.00 | 3 |
| 03b960 | FUN_0003b960 | P4 | 12 | 1584 | 0.00 | 5 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 316 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 302 | 15166 |
| C10 | SET : décors/animations de palette | 105 | 10508 |
| C02 | ENEMY : ennemis | 194 | 9052 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 98 | 6076 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 45 | 3874 |
| P4 | Noyau joueur (états, 26 actions génériques) | 46 | 3512 |
| M1 | Menu TEST (service) | 76 | 3270 |
| B1 | Scrolling / caméras par scène | 31 | 2630 |
| C0e | EFFCT : effets | 54 | 2502 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| M2 | EEPROM, configuration, comptabilité | 40 | 2102 |
| W3 | Scripts de scène (phases 16/28) | 49 | 1800 |
| C06 | PSHL : tirs du joueur | 21 | 1726 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 7 | 1564 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C0c | ITEM : objets ramassables | 9 | 1254 |
| F4 | Exécuteur d'écrans fixes (logos) | 3 | 1044 |
| X2 | Dégâts, terrain, placement | 13 | 964 |
| C1 | Crédits, pièces, start/continue | 6 | 902 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| C14 | SOBJ : objets système/UI | 16 | 736 |
| P1 | Initialisation des joueurs | 17 | 626 |
| O3 | Aides communes aux types (animations par type) | 16 | 596 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 10 | 508 |
| V3 | Palettes et fondus | 7 | 472 |
| O2 | Pools d'objets et listes triées | 13 | 342 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 3 | 318 |
| S1 | Son : wrappers de commandes + FIFO | 6 | 310 |
| U1 | RNG et files d'affichage (pop-ups) | 4 | 296 |
| C0a | BOX : caisses/coffres (h) | 3 | 264 |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 2 | 194 |
| F7 | Écrans titre / copyright | 4 | 108 |
| V2 | Texte, rectangles et blocs de tuiles | 5 | 96 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 46 |
| P3 | Comportement scénarisé des joueurs par scène | 1 | 30 |
| U2 | Petites routines / tables (rôle inconnu) | 1 | 10 |
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 197085 | 16.4 |
| scenarios/boss/boss10_scene66.txt | 12000 | 6943 | 0.6 |
| scenarios/boss/boss11_scene42.txt | 12000 | 147435 | 12.3 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 1606685 | 133.9 |
| scenarios/boss/boss12_scene40.txt | 12000 | 51582 | 4.3 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 1522113 | 126.8 |
| scenarios/boss/boss13_dragon.txt | 12000 | 384761 | 32.1 |
| scenarios/boss/boss15_scene46.txt | 12000 | 592705 | 49.4 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 1641097 | 136.8 |
| scenarios/boss/boss17_scene13.txt | 12000 | 359758 | 30.0 |
| scenarios/boss/boss18_scene26.txt | 12000 | 612116 | 51.0 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 220884 | 18.4 |
| scenarios/boss/boss19_scene32.txt | 12000 | 127108 | 10.6 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 1667528 | 139.0 |
| scenarios/boss/boss21_scene44.txt | 12000 | 313168 | 26.1 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 1555100 | 129.6 |
| scenarios/boss/boss5_scene58.txt | 12000 | 11942 | 1.0 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 1506654 | 125.6 |
| scenarios/boss/boss6_scene24.txt | 12000 | 526845 | 43.9 |
| scenarios/boss/boss8_scene64.txt | 12000 | 21148 | 1.8 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 915259 | 76.3 |
| scenarios/bot30k.txt | 30000 | 6922 | 0.2 |
| scenarios/bot_2p.txt | 30000 | 6285 | 0.2 |
| scenarios/bot_char0.txt | 30000 | 7525 | 0.3 |
| scenarios/bot_char1.txt | 30000 | 10199 | 0.3 |
| scenarios/bot_char2.txt | 30000 | 925 | 0.0 |
| scenarios/bot_char3.txt | 30000 | 8684 | 0.3 |
| scenarios/bot_char4.txt | 30000 | 8153 | 0.3 |
| scenarios/bot_char5.txt | 30000 | 5929 | 0.2 |
| scenarios/cov/cov_2p_a.txt | 30000 | 2447953 | 81.6 |
| scenarios/cov/cov_2p_b.txt | 30000 | 1092 | 0.0 |
| scenarios/cov/cov_2p_c.txt | 30000 | 8879665 | 296.0 |
| scenarios/cov/cov_3p.txt | 30000 | 2228964 | 74.3 |
| scenarios/cov/cov_4p_a.txt | 30000 | 1799849 | 60.0 |
| scenarios/cov/cov_4p_b.txt | 30000 | 1874 | 0.1 |
| scenarios/cov/cov_4p_c.txt | 30000 | 23029 | 0.8 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 402537 | 13.4 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 6076 | 0.2 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 578634 | 19.3 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 6076 | 0.2 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 6076 | 0.2 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 6076 | 0.2 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 6076 | 0.2 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 6076 | 0.2 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 11598 | 0.4 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 17544 | 0.6 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 509388 | 17.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 33910 | 1.1 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 737 | 0.0 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 5891327 | 196.4 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 45218 | 1.5 |
| scenarios/cov/cov_service.txt | 30000 | 213 | 0.0 |
| scenarios/cov/cov_service_ecrans.txt | 30000 | 409319 | 13.6 |
| scenarios/cov/cov_service_reglages.txt | 30000 | 213 | 0.0 |
| scenarios/cov/cov_service_tests.txt | 30000 | 213 | 0.0 |
| scenarios/cov/cov_service_variante.txt | 30000 | 416 | 0.0 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 934539 | 31.2 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 2975 | 0.1 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 2101129 | 70.0 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 684981 | 22.8 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 908863 | 30.3 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 2074432 | 69.1 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 2762853 | 92.1 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 97766 | 3.3 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 4776 | 0.4 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 1894 | 0.2 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 767 | 0.1 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 56275 | 4.7 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 24875 | 2.1 |
| scenarios/enemy/elf_scene92.txt | 12000 | 1999 | 0.2 |
| scenarios/enemy/elf_scene94.txt | 12000 | 4228 | 0.4 |
| scenarios/enemy/elf_scene96.txt | 12000 | 122208 | 10.2 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 147435 | 12.3 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 34930 | 2.9 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 554264 | 46.2 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 14898 | 1.2 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 70014 | 5.8 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 2581 | 0.2 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 25623 | 2.1 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 59252 | 4.9 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 50238 | 4.2 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 642 | 0.1 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 12052093 | 1004.3 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 186812 | 15.6 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 57241 | 4.8 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 38617 | 3.2 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 6260 | 0.5 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 15060 | 1.3 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 12038605 | 1003.2 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 111595 | 9.3 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 359025 | 29.9 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 28965 | 2.4 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 197085 | 16.4 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 176887 | 14.7 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 13463 | 1.1 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 878 | 0.1 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 11668 | 1.0 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 11435 | 1.0 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 1906 | 0.2 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 359758 | 30.0 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 14296 | 1.2 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 725 | 0.1 |
| scenarios/enemy/trog_scene101.txt | 12000 | 646 | 0.1 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 2819 | 0.2 |
| scenarios/enemy/trog_scene104.txt | 12000 | 1102008 | 91.8 |
| scenarios/extra/mash_2p.txt | 12000 | 12462 | 1.0 |
| scenarios/extra/mash_c0.txt | 12000 | 8332 | 0.7 |
| scenarios/extra/mash_c0b.txt | 12000 | 7067 | 0.6 |
| scenarios/extra/mash_c1b.txt | 12000 | 76210 | 6.4 |
| scenarios/extra/mash_c2.txt | 12000 | 67356 | 5.6 |
| scenarios/extra/mash_c2b.txt | 12000 | 1018 | 0.1 |
| scenarios/extra/mash_c3.txt | 12000 | 1870 | 0.2 |
| scenarios/extra/mash_c3b.txt | 12000 | 860 | 0.1 |
| scenarios/extra/mash_c3c.txt | 12000 | 6559 | 0.5 |
| scenarios/extra/mash_c3d.txt | 12000 | 840 | 0.1 |
| scenarios/extra/mash_c4.txt | 12000 | 7451 | 0.6 |
| scenarios/extra/mash_c4b.txt | 12000 | 6628 | 0.6 |
| scenarios/extra/mash_c4c.txt | 12000 | 7192 | 0.6 |
| scenarios/extra/mash_c5.txt | 12000 | 17105 | 1.4 |
| scenarios/extra/mash_c5b.txt | 12000 | 28382 | 2.4 |
| scenarios/modes/attract.txt | 12000 | 402537 | 33.5 |
| scenarios/modes/ending.txt | 12000 | 459472 | 38.3 |
| scenarios/modes/ending_end.txt | 12000 | 273581 | 22.8 |
| scenarios/modes/gameover.txt | 12000 | 8153 | 0.7 |
| scenarios/modes/screens.txt | 12000 | 402537 | 33.5 |
| scenarios/scene/scene_01.txt | 6500 | 2415 | 0.4 |
| scenarios/scene/scene_02.txt | 6500 | 8201 | 1.3 |
| scenarios/scene/scene_03.txt | 6500 | 6724 | 1.0 |
| scenarios/scene/scene_04.txt | 6500 | 8227 | 1.3 |
| scenarios/scene/scene_05.txt | 6500 | 2944 | 0.5 |
| scenarios/scene/scene_07.txt | 6500 | 223138 | 34.3 |
| scenarios/scene/scene_08.txt | 6500 | 224150 | 34.5 |
| scenarios/scene/scene_09.txt | 6500 | 238395 | 36.7 |
| scenarios/scene/scene_10.txt | 6500 | 502 | 0.1 |
| scenarios/scene/scene_105.txt | 6500 | 218557 | 33.6 |
| scenarios/scene/scene_109.txt | 6500 | 21336 | 3.3 |
| scenarios/scene/scene_11.txt | 6500 | 133197 | 20.5 |
| scenarios/scene/scene_110.txt | 6500 | 1450 | 0.2 |
| scenarios/scene/scene_111.txt | 6500 | 107473 | 16.5 |
| scenarios/scene/scene_14.txt | 6500 | 27526 | 4.2 |
| scenarios/scene/scene_24.txt | 6500 | 246851 | 38.0 |
| scenarios/scene/scene_25.txt | 6500 | 28533 | 4.4 |
| scenarios/scene/scene_26.txt | 6500 | 238165 | 36.6 |
| scenarios/scene/scene_27.txt | 6500 | 16089 | 2.5 |
| scenarios/scene/scene_28.txt | 6500 | 28105 | 4.3 |
| scenarios/scene/scene_32.txt | 6500 | 120550 | 18.5 |
| scenarios/scene/scene_33.txt | 6500 | 532 | 0.1 |
| scenarios/scene/scene_34.txt | 6500 | 1449 | 0.2 |
| scenarios/scene/scene_35.txt | 6500 | 1348508 | 207.5 |
| scenarios/scene/scene_38.txt | 6500 | 13620 | 2.1 |
| scenarios/scene/scene_39.txt | 6500 | 2227 | 0.3 |
| scenarios/scene/scene_40.txt | 6500 | 31008 | 4.8 |
| scenarios/scene/scene_41.txt | 6500 | 164722 | 25.3 |
| scenarios/scene/scene_42.txt | 6500 | 106007 | 16.3 |
| scenarios/scene/scene_44.txt | 6500 | 107473 | 16.5 |
| scenarios/scene/scene_47.txt | 6500 | 1883749 | 289.8 |
| scenarios/scene/scene_54.txt | 6500 | 23912 | 3.7 |
| scenarios/scene/scene_58.txt | 6500 | 3593 | 0.6 |
| scenarios/scene/scene_59.txt | 6500 | 13959 | 2.1 |
| scenarios/scene/scene_64.txt | 6500 | 7623 | 1.2 |
| scenarios/scene/scene_65.txt | 6500 | 31128 | 4.8 |
| scenarios/scene/scene_66.txt | 6500 | 69646 | 10.7 |
| scenarios/scene/scene_67.txt | 6500 | 25485 | 3.9 |
| scenarios/scene/scene_90.txt | 6500 | 5627 | 0.9 |
| scenarios/scene_end/end_01.txt | 9000 | 2783 | 0.3 |
| scenarios/scene_end/end_02.txt | 9000 | 516 | 0.1 |
| scenarios/scene_end/end_03.txt | 9000 | 707 | 0.1 |
| scenarios/scene_end/end_04.txt | 9000 | 1006 | 0.1 |
| scenarios/scene_end/end_05.txt | 9000 | 507 | 0.1 |
| scenarios/scene_end/end_06.txt | 9000 | 268373 | 29.8 |
| scenarios/scene_end/end_07.txt | 9000 | 316839 | 35.2 |
| scenarios/scene_end/end_09.txt | 9000 | 147264 | 16.4 |
| scenarios/scene_end/end_10.txt | 9000 | 502 | 0.1 |
| scenarios/scene_end/end_105.txt | 9000 | 399203 | 44.4 |
| scenarios/scene_end/end_11.txt | 9000 | 88086 | 9.8 |
| scenarios/scene_end/end_13.txt | 9000 | 19956 | 2.2 |
| scenarios/scene_end/end_14.txt | 9000 | 27526 | 3.1 |
| scenarios/scene_end/end_20.txt | 9000 | 42016 | 4.7 |
| scenarios/scene_end/end_24.txt | 9000 | 381335 | 42.4 |
| scenarios/scene_end/end_26.txt | 9000 | 399558 | 44.4 |
| scenarios/scene_end/end_27.txt | 9000 | 507 | 0.1 |
| scenarios/scene_end/end_31.txt | 9000 | 1203984 | 133.8 |
| scenarios/scene_end/end_32.txt | 9000 | 117298 | 13.0 |
| scenarios/scene_end/end_33.txt | 9000 | 532 | 0.1 |
| scenarios/scene_end/end_34.txt | 9000 | 1398081 | 155.3 |
| scenarios/scene_end/end_35.txt | 9000 | 2157212 | 239.7 |
| scenarios/scene_end/end_38.txt | 9000 | 132425 | 14.7 |
| scenarios/scene_end/end_40.txt | 9000 | 2586 | 0.3 |
| scenarios/scene_end/end_41.txt | 9000 | 129825 | 14.4 |
| scenarios/scene_end/end_42.txt | 9000 | 109907 | 12.2 |
| scenarios/scene_end/end_44.txt | 9000 | 118008 | 13.1 |
| scenarios/scene_end/end_46.txt | 9000 | 387632 | 43.1 |
| scenarios/scene_end/end_54.txt | 9000 | 83896 | 9.3 |
| scenarios/scene_end/end_58.txt | 9000 | 1203420 | 133.7 |
| scenarios/scene_end/end_59.txt | 9000 | 691 | 0.1 |
| scenarios/scene_end/end_64.txt | 9000 | 507 | 0.1 |
| scenarios/scene_end/end_65.txt | 9000 | 971 | 0.1 |
| scenarios/scene_end/end_66.txt | 9000 | 536 | 0.1 |
| scenarios/scene_end/end_67.txt | 9000 | 11381 | 1.3 |
| scenarios/scene_end/end_68.txt | 9000 | 1398085 | 155.3 |
