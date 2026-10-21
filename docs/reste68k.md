# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 200 scénarios, soit 2851500 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **91232723 instructions 68000 exécutées** (hors attente active), soit 32.0 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 203 exécutent encore du 68000, 842 sont entièrement natives, **1514 jamais atteintes** (104058 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 89406203 (comptées par sous-système ci-dessous, colonne « Hors index », et par plage plus bas).

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Hors index | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| C0e | EFFCT : effets | 91110834 | 99.87 | 89324025 | 170 | 111 | 5 | 54 | b46a2 (27.6 %), b46cc (23.3 %), b03c6 (5.2 %) |
| W3 | Scripts de scène (phases 16/28) | 60449 | 0.07 | 48775 | 82 | 24 | 9 | 49 | fa270 (82.6 %), f9e38 (5.6 %), f9744 (2.4 %) |
| C04 | BOSS : boss | 23692 | 0.03 | 18233 | 353 | 14 | 8 | 331 | 81722 (42.3 %), 7f842 (18.3 %), 81764 (9.0 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 12734 | 0.01 | 0 | 69 | 2 | 57 | 10 | bad6 (98.0 %), 10158 (2.0 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 9374 | 0.01 | 7754 | 59 | 2 | 7 | 50 | 3b960 (97.8 %), 3b952 (2.2 %) |
| C08 | ESHL : tirs/attaques ennemis | 5985 | 0.01 | 4058 | 127 | 12 | 17 | 98 | a66c0 (47.8 %), a31c2 (13.1 %), a036e (10.8 %) |
| C10 | SET : décors/animations de palette | 1927 | 0.00 | 665 | 131 | 9 | 17 | 105 | d90b0 (60.1 %), cf182 (14.8 %), cc200 (13.3 %) |
| C02 | ENEMY : ennemis | 1900 | 0.00 | 1632 | 205 | 3 | 6 | 196 | 76ec4 (95.5 %), 792f8 (3.0 %), 71e3c (1.5 %) |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 1738 | 0.00 | 0 | 49 | 2 | 0 | 47 | 1507a (99.4 %), 180ac (0.6 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 1063 | 0.00 | 15 | 3 | 2 | 0 | 1 | 1c44a (99.6 %), 1c86e (0.4 %) |
| U2 | Petites routines / tables (rôle inconnu) | 864 | 0.00 | 832 | 17 | 3 | 13 | 1 | 1ac7c (43.8 %), 1ac9a (37.5 %), 1ac72 (18.8 %) |
| C06 | PSHL : tirs du joueur | 740 | 0.00 | 0 | 39 | 3 | 15 | 21 | 54872 (63.9 %), 54818 (32.8 %), 54700 (3.2 %) |
| C14 | SOBJ : objets système/UI | 557 | 0.00 | 0 | 25 | 3 | 6 | 16 | e03c6 (74.1 %), df4bc (21.5 %), df896 (4.3 %) |
| V2 | Texte, rectangles et blocs de tuiles | 293 | 0.00 | 192 | 33 | 3 | 25 | 5 | 2a52 (61.4 %), 2a42 (35.6 %), 2a36 (3.0 %) |
| C12 | PEFF : effets du joueur | 162 | 0.00 | 0 | 107 | 2 | 35 | 70 | 4c930 (88.9 %), 4f132 (11.1 %) |
| W0 | Dispatch des 10 classes d'objets | 119 | 0.00 | 0 | 10 | 1 | 9 | 0 | 39e86 (100.0 %) |
| F1 | Tâche de boot, avertissements région | 112 | 0.00 | 0 | 5 | 2 | 2 | 1 | 133f6 (51.8 %), 133f0 (48.2 %) |
| V3 | Palettes et fondus | 104 | 0.00 | 0 | 38 | 1 | 30 | 7 | 1a5e8 (100.0 %) |
| S1 | Son : wrappers de commandes + FIFO | 26 | 0.00 | 0 | 410 | 2 | 404 | 4 | e27da (84.6 %), e2430 (15.4 %) |
| P3 | Comportement scénarisé des joueurs par scène | 22 | 0.00 | 22 | 3 | 0 | 2 | 1 |  |
| C0c | ITEM : objets ramassables | 22 | 0.00 | 0 | 14 | 1 | 2 | 11 | dd12c (100.0 %) |
| O3 | Aides communes aux types (animations par type) | 6 | 0.00 | 0 | 25 | 1 | 8 | 16 | 3e28e (100.0 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 0 | 0.00 | 0 | 17 | 0 | 14 | 3 |  |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 0 | 0.00 | 0 | 2 | 0 | 0 | 2 |  |
| K3 | Entrées vers enregistrements joueur | 0 | 0.00 | 0 | 3 | 0 | 3 | 0 |  |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 0 | 16 | 0 | 0 | 16 |  |
| V1 | Primitives vidéo (attente, couches, effacements) | 0 | 0.00 | 0 | 13 | 0 | 13 | 0 |  |
| V4 | Construction de la liste de sprites (OBJ RAM) | 0 | 0.00 | 0 | 15 | 0 | 2 | 13 |  |
| B1 | Scrolling / caméras par scène | 0 | 0.00 | 0 | 42 | 0 | 11 | 31 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 0 | 22 | 0 | 22 | 0 |  |
| O2 | Pools d'objets et listes triées | 0 | 0.00 | 0 | 41 | 0 | 28 | 13 |  |
| U1 | RNG et files d'affichage (pop-ups) | 0 | 0.00 | 0 | 18 | 0 | 14 | 4 |  |
| X1 | Collisions (boîtes, coups, poussée) | 0 | 0.00 | 0 | 39 | 0 | 0 | 39 |  |
| X2 | Dégâts, terrain, placement | 0 | 0.00 | 0 | 19 | 0 | 6 | 13 |  |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 0 | 0.00 | 0 | 1 | 0 | 0 | 1 |  |
| P1 | Initialisation des joueurs | 0 | 0.00 | 0 | 23 | 0 | 6 | 17 |  |
| P5 | Code par personnage (6 × 2 variantes) | 0 | 0.00 | 0 | 73 | 0 | 1 | 72 |  |
| P6 | Réactions aux coups, saisie des coups spéciaux | 0 | 0.00 | 0 | 9 | 0 | 2 | 7 |  |
| C0a | BOX : caisses/coffres (h) | 0 | 0.00 | 0 | 7 | 0 | 1 | 6 |  |
| C1 | Crédits, pièces, start/continue | 0 | 0.00 | 0 | 22 | 0 | 15 | 7 |  |
| M1 | Menu TEST (service) | 0 | 0.00 | 0 | 76 | 0 | 0 | 76 |  |
| M2 | EEPROM, configuration, comptabilité | 0 | 0.00 | 0 | 60 | 0 | 18 | 42 |  |
| M3 | « Secret Test » (debug) et Block Test | 0 | 0.00 | 0 | 29 | 0 | 0 | 29 |  |
| F2 | Machine à modes (attract) | 0 | 0.00 | 0 | 0 | 0 | 0 | 0 |  |
| F4 | Exécuteur d'écrans fixes (logos) | 0 | 0.00 | 0 | 5 | 0 | 1 | 4 |  |
| F5 | Curseur de sélection de personnage | 0 | 0.00 | 0 | 10 | 0 | 0 | 10 |  |
| F6 | Interpréteur de scripts de scène (démos) | 0 | 0.00 | 0 | 18 | 0 | 7 | 11 |  |
| F7 | Écrans titre / copyright | 0 | 0.00 | 0 | 5 | 0 | 1 | 4 |  |
| - | hors atlas | 0 | 0.00 | 0 | 0 | 0 | 0 | 0 |  |

## Hors de toute fonction de l'index (les 40 plages les plus gourmandes)

Code que Ghidra n'a pas découpé en fonctions (gestionnaires atteints par table, par exemple) : à désassembler depuis le listing. Plages contiguës à moins de 0x40 octets.

| Plage | Sous-système | Instr. 68000 | % |
|---|---|---:|---:|
| 0b9d7e-0b9eb8 | C0e | 37424156 | 41.02 |
| 0afa46-0afac8 | C0e | 15396926 | 16.88 |
| 0aeff0-0af2b6 | C0e | 8577065 | 9.40 |
| 0abccc-0abdfa | C0e | 3511902 | 3.85 |
| 0b6212-0b64aa | C0e | 3044064 | 3.34 |
| 0af3f2-0af4be | C0e | 2189994 | 2.40 |
| 0b9f36-0b9fe2 | C0e | 1807524 | 1.98 |
| 0b1bb6-0b1c02 | C0e | 1780398 | 1.95 |
| 0b2f46-0b372e | C0e | 1713031 | 1.88 |
| 0b5fb6-0b615a | C0e | 1665035 | 1.83 |
| 0ad600-0ad7f6 | C0e | 1432575 | 1.57 |
| 0b46ec-0b492e | C0e | 1312169 | 1.44 |
| 0ad866-0adaca | C0e | 1016371 | 1.11 |
| 0b6d48-0b7790 | C0e | 1006709 | 1.10 |
| 0b5dbe-0b5f60 | C0e | 775732 | 0.85 |
| 0b9a6a-0b9c62 | C0e | 674600 | 0.74 |
| 0b1abc-0b1b60 | C0e | 584698 | 0.64 |
| 0b1818-0b1a74 | C0e | 436651 | 0.48 |
| 0b40f8-0b417c | C0e | 434803 | 0.48 |
| 0ab414-0ab4dc | C0e | 394812 | 0.43 |
| 0b4b66-0b4bac | C0e | 368440 | 0.40 |
| 0b45da-0b4682 | C0e | 344420 | 0.38 |
| 0ae52a-0ae6fa | C0e | 307178 | 0.34 |
| 0b9956-0b9a24 | C0e | 291345 | 0.32 |
| 0ad476-0ad530 | C0e | 284780 | 0.31 |
| 0aed4c-0aef4c | C0e | 228488 | 0.25 |
| 0b802a-0b81d4 | C0e | 217275 | 0.24 |
| 0abe4e-0abe52 | C0e | 214848 | 0.24 |
| 0ab2d4-0ab3d0 | C0e | 156427 | 0.17 |
| 0b4ac0-0b4aec | C0e | 130872 | 0.14 |
| 0b2dea-0b2eec | C0e | 122314 | 0.13 |
| 0ac90a-0ac94a | C0e | 121768 | 0.13 |
| 0b24d6-0b2802 | C0e | 120580 | 0.13 |
| 0abeea-0abef6 | C0e | 99530 | 0.11 |
| 0afe20-0b0032 | C0e | 97283 | 0.11 |
| 0b1f72-0b1fdc | C0e | 88836 | 0.10 |
| 0b7e26-0b7f0e | C0e | 71657 | 0.08 |
| 0b1dc6-0b1e4a | C0e | 63084 | 0.07 |
| 0b44a0-0b4544 | C0e | 62192 | 0.07 |
| 0b37ac-0b38f2 | C0e | 62071 | 0.07 |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 0b46a2 | FUN_000b46a2 | C0e | 42 | 492635 | 0.54 | 11 |
| 0b46cc | FUN_000b46cc | C0e | 32 | 416845 | 0.46 | 11 |
| 0b03c6 | FUN_000b03c6 | C0e | 102 | 93632 | 0.10 | 9 |
| 0b5f8e | FUN_000b5f8e | C0e | 40 | 78080 | 0.09 | 12 |
| 0ae3bc | FUN_000ae3bc | C0e | 50 | 72646 | 0.08 | 11 |
| 0abea4 | FUN_000abea4 | C0e | 36 | 69790 | 0.08 | 3 |
| 0b4686 | FUN_000b4686 | C0e | 28 | 57316 | 0.06 | 11 |
| 0b0990 | FUN_000b0990 | C0e | 80 | 46379 | 0.05 | 9 |
| 0ad60a | FUN_000ad60a | C0e | 52 | 42176 | 0.05 | 13 |
| 0ab2b4 | FUN_000ab2b4 | C0e | 16 | 41170 | 0.05 | 158 |
| 0b13fa | FUN_000b13fa | C0e | 32 | 39870 | 0.04 | 124 |
| 0ad832 | FUN_000ad832 | C0e | 26 | 35772 | 0.04 | 8 |
| 0aaa04 | FUN_000aaa04 | C0e | 60 | 33110 | 0.04 | 143 |
| 0b5f66 | FUN_000b5f66 | C0e | 40 | 30976 | 0.03 | 18 |
| 0b0390 | FUN_000b0390 | C0e | 38 | 28899 | 0.03 | 9 |
| 0ae776 | FUN_000ae776 | C0e | 22 | 19200 | 0.02 | 192 |
| 0ae298 | FUN_000ae298 | C0e | 228 | 18076 | 0.02 | 10 |
| 0b40ba | FUN_000b40ba | C0e | 62 | 16236 | 0.02 | 6 |
| 0b4af6 | FUN_000b4af6 | C0e | 96 | 15512 | 0.02 | 6 |
| 0b3556 | FUN_000b3556 | C0e | 26 | 12770 | 0.01 | 10 |
| 00bad6 | FUN_0000bad6 | E1 | 1104 | 12479 | 0.01 | 88 |
| 0fa270 | FUN_000fa270 | W3 | 38 | 9648 | 0.01 | 3 |
| 0ae3ec | FUN_000ae3ec | C0e | 60 | 7164 | 0.01 | 106 |
| 0b3f6a | FUN_000b3f6a | C0e | 70 | 6935 | 0.01 | 3 |
| 0b4552 | FUN_000b4552 | C0e | 88 | 5975 | 0.01 | 3 |
| 0b20b8 | FUN_000b20b8 | C0e | 50 | 5664 | 0.01 | 27 |
| 0b1a7a | FUN_000b1a7a | C0e | 30 | 5551 | 0.01 | 57 |
| 0ab7d8 | FUN_000ab7d8 | C0e | 34 | 5039 | 0.01 | 26 |
| 0b230e | FUN_000b230e | C0e | 38 | 4984 | 0.01 | 27 |
| 0abb70 | FUN_000abb70 | C0e | 126 | 4742 | 0.01 | 41 |
| 0b0102 | FUN_000b0102 | C0e | 224 | 4381 | 0.00 | 8 |
| 0aabf4 | FUN_000aabf4 | C0e | 122 | 4380 | 0.00 | 22 |
| 0ae748 | FUN_000ae748 | C0e | 14 | 4025 | 0.00 | 5 |
| 0b222e | FUN_000b222e | C0e | 18 | 3444 | 0.00 | 27 |
| 0b4088 | FUN_000b4088 | C0e | 50 | 3410 | 0.00 | 6 |
| 0afe02 | FUN_000afe02 | C0e | 30 | 3283 | 0.00 | 9 |
| 0b2212 | FUN_000b2212 | C0e | 28 | 3232 | 0.00 | 27 |
| 0b212c | FUN_000b212c | C0e | 114 | 3044 | 0.00 | 27 |
| 0b1ff8 | FUN_000b1ff8 | C0e | 52 | 2542 | 0.00 | 27 |
| 0abec8 | FUN_000abec8 | C0e | 34 | 2520 | 0.00 | 50 |
| 081722 | FUN_00081722 | C04 | 26 | 2310 | 0.00 | 1 |
| 0af346 | FUN_000af346 | C0e | 26 | 2304 | 0.00 | 192 |
| 0b9aa0 | FUN_000b9aa0 | C0e | 26 | 2227 | 0.00 | 7 |
| 0af2c0 | FUN_000af2c0 | C0e | 66 | 2080 | 0.00 | 19 |
| 0ba18a | FUN_000ba18a | C0e | 34 | 2080 | 0.00 | 1 |
| 0b6596 | FUN_000b6596 | C0e | 64 | 1965 | 0.00 | 43 |
| 0b202c | FUN_000b202c | C0e | 76 | 1736 | 0.00 | 27 |
| 01507a | FUN_0001507a | F3 | 50 | 1728 | 0.00 | 192 |
| 0af302 | FUN_000af302 | C0e | 68 | 1710 | 0.00 | 2 |
| 0b3fd4 | FUN_000b3fd4 | C0e | 46 | 1641 | 0.00 | 3 |
| 03b960 | FUN_0003b960 | P4 | 12 | 1584 | 0.00 | 5 |
| 0b378a | FUN_000b378a | C0e | 34 | 1496 | 0.00 | 22 |
| 0a8af8 | FUN_000a8af8 | C0e | 48 | 1340 | 0.00 | 31 |
| 0b2078 | FUN_000b2078 | C0e | 60 | 1302 | 0.00 | 27 |
| 0b4bb2 | FUN_000b4bb2 | C0e | 26 | 1300 | 0.00 | 3 |
| 0b9c6a | FUN_000b9c6a | C0e | 30 | 1284 | 0.00 | 4 |
| 01c44a | FUN_0001c44a | W1 | 16 | 1044 | 0.00 | 29 |
| 07f842 | FUN_0007f842 | C04 | 322 | 999 | 0.00 | 1 |
| 0b2208 | FUN_000b2208 | C0e | 10 | 992 | 0.00 | 27 |
| 0b3734 | FUN_000b3734 | C0e | 46 | 975 | 0.00 | 5 |
| 0b2ef6 | FUN_000b2ef6 | C0e | 76 | 949 | 0.00 | 1 |
| 0a66c0 | FUN_000a66c0 | C08 | 34 | 921 | 0.00 | 2 |
| 0b9c88 | FUN_000b9c88 | C0e | 30 | 911 | 0.00 | 2 |
| 0ad84c | FUN_000ad84c | C0e | 26 | 868 | 0.00 | 2 |
| 0b7dec | FUN_000b7dec | C0e | 58 | 866 | 0.00 | 5 |
| 0a836c | FUN_000a836c | C0e | 34 | 864 | 0.00 | 6 |
| 0b24b6 | FUN_000b24b6 | C0e | 32 | 864 | 0.00 | 2 |
| 0ba03c | FUN_000ba03c | C0e | 64 | 836 | 0.00 | 6 |
| 0b219e | FUN_000b219e | C0e | 50 | 806 | 0.00 | 27 |
| 0b2348 | FUN_000b2348 | C0e | 42 | 774 | 0.00 | 27 |
| 0b52b0 | FUN_000b52b0 | C0e | 94 | 768 | 0.00 | 1 |
| 0d90b0 | FUN_000d90b0 | C10 | 270 | 759 | 0.00 | 1 |
| 0b4934 | FUN_000b4934 | C0e | 82 | 750 | 0.00 | 2 |
| 0f9e38 | FUN_000f9e38 | W3 | 20 | 650 | 0.00 | 3 |
| 0aaa40 | FUN_000aaa40 | C0e | 68 | 646 | 0.00 | 5 |
| 0ab708 | FUN_000ab708 | C0e | 30 | 595 | 0.00 | 58 |
| 0b5206 | FUN_000b5206 | C0e | 30 | 567 | 0.00 | 5 |
| 0b21d8 | FUN_000b21d8 | C0e | 28 | 558 | 0.00 | 27 |
| 0b141a | FUN_000b141a | C0e | 38 | 549 | 0.00 | 28 |
| 0b12c0 | FUN_000b12c0 | C0e | 38 | 522 | 0.00 | 24 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 203 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 331 | 16518 |
| C10 | SET : décors/animations de palette | 105 | 10508 |
| C02 | ENEMY : ennemis | 196 | 9094 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 98 | 6076 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 47 | 4386 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| P4 | Noyau joueur (états, 26 actions génériques) | 50 | 3774 |
| M1 | Menu TEST (service) | 76 | 3270 |
| B1 | Scrolling / caméras par scène | 31 | 2630 |
| C0e | EFFCT : effets | 54 | 2502 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| M2 | EEPROM, configuration, comptabilité | 42 | 2152 |
| W3 | Scripts de scène (phases 16/28) | 49 | 1800 |
| C06 | PSHL : tirs du joueur | 21 | 1726 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 7 | 1564 |
| F4 | Exécuteur d'écrans fixes (logos) | 4 | 1502 |
| C0c | ITEM : objets ramassables | 11 | 1434 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C1 | Crédits, pièces, start/continue | 7 | 1090 |
| X2 | Dégâts, terrain, placement | 13 | 964 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| C14 | SOBJ : objets système/UI | 16 | 736 |
| C0a | BOX : caisses/coffres (h) | 6 | 682 |
| P1 | Initialisation des joueurs | 17 | 626 |
| O3 | Aides communes aux types (animations par type) | 16 | 596 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 10 | 508 |
| V3 | Palettes et fondus | 7 | 472 |
| O2 | Pools d'objets et listes triées | 13 | 342 |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 3 | 318 |
| U1 | RNG et files d'affichage (pop-ups) | 4 | 296 |
| S1 | Son : wrappers de commandes + FIFO | 4 | 252 |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 2 | 194 |
| F7 | Écrans titre / copyright | 4 | 108 |
| V2 | Texte, rectangles et blocs de tuiles | 5 | 96 |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 1 | 46 |
| P3 | Comportement scénarisé des joueurs par scène | 1 | 30 |
| F1 | Tâche de boot, avertissements région | 1 | 20 |
| U2 | Petites routines / tables (rôle inconnu) | 1 | 10 |
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 196132 | 16.3 |
| scenarios/boss/boss10_scene66.txt | 12000 | 6486 | 0.5 |
| scenarios/boss/boss11_scene42.txt | 12000 | 145766 | 12.1 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 1605895 | 133.8 |
| scenarios/boss/boss12_scene40.txt | 12000 | 18022 | 1.5 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 1521085 | 126.8 |
| scenarios/boss/boss13_dragon.txt | 12000 | 382086 | 31.8 |
| scenarios/boss/boss15_scene46.txt | 12000 | 587862 | 49.0 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 1640406 | 136.7 |
| scenarios/boss/boss17_scene13.txt | 12000 | 358622 | 29.9 |
| scenarios/boss/boss18_scene26.txt | 12000 | 590048 | 49.2 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 220259 | 18.4 |
| scenarios/boss/boss19_scene32.txt | 12000 | 124056 | 10.3 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 1654516 | 137.9 |
| scenarios/boss/boss21_scene44.txt | 12000 | 310604 | 25.9 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 1532490 | 127.7 |
| scenarios/boss/boss5_scene58.txt | 12000 | 11426 | 1.0 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 1505964 | 125.5 |
| scenarios/boss/boss6_scene24.txt | 12000 | 504936 | 42.1 |
| scenarios/boss/boss8_scene64.txt | 12000 | 18622 | 1.6 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 909247 | 75.8 |
| scenarios/bot30k.txt | 30000 | 6358 | 0.2 |
| scenarios/bot_2p.txt | 30000 | 5768 | 0.2 |
| scenarios/bot_char0.txt | 30000 | 6982 | 0.2 |
| scenarios/bot_char1.txt | 30000 | 9499 | 0.3 |
| scenarios/bot_char2.txt | 30000 | 517 | 0.0 |
| scenarios/bot_char3.txt | 30000 | 8158 | 0.3 |
| scenarios/bot_char4.txt | 30000 | 7604 | 0.3 |
| scenarios/bot_char5.txt | 30000 | 5430 | 0.2 |
| scenarios/cov/cov_2p_a.txt | 30000 | 2419459 | 80.6 |
| scenarios/cov/cov_2p_b.txt | 30000 | 590 | 0.0 |
| scenarios/cov/cov_2p_c.txt | 30000 | 8773121 | 292.4 |
| scenarios/cov/cov_3p.txt | 30000 | 2213362 | 73.8 |
| scenarios/cov/cov_4p_a.txt | 30000 | 1708215 | 56.9 |
| scenarios/cov/cov_4p_b.txt | 30000 | 1359 | 0.0 |
| scenarios/cov/cov_4p_c.txt | 30000 | 17857 | 0.6 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 388074 | 12.9 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 5609 | 0.2 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 563984 | 18.8 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 5609 | 0.2 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 5609 | 0.2 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 5609 | 0.2 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 5609 | 0.2 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 5609 | 0.2 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 11120 | 0.4 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 17055 | 0.6 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 493949 | 16.5 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 32895 | 1.1 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 343 | 0.0 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 5889818 | 196.3 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 30057 | 1.0 |
| scenarios/cov/cov_service.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_ecrans.txt | 30000 | 394643 | 13.2 |
| scenarios/cov/cov_service_reglages.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_tests.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_variante.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 872689 | 29.1 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 2586 | 0.1 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 2074623 | 69.2 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 535236 | 17.8 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 830348 | 27.7 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 2030565 | 67.7 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 2748019 | 91.6 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 97248 | 3.2 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 3936 | 0.3 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 1469 | 0.1 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 364 | 0.0 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 55627 | 4.6 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 3715 | 0.3 |
| scenarios/enemy/elf_scene92.txt | 12000 | 1606 | 0.1 |
| scenarios/enemy/elf_scene94.txt | 12000 | 2158 | 0.2 |
| scenarios/enemy/elf_scene96.txt | 12000 | 121739 | 10.1 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 145766 | 12.1 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 34519 | 2.9 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 553725 | 46.1 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 13011 | 1.1 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 69589 | 5.8 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 2138 | 0.2 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 3076 | 0.3 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 10648 | 0.9 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 48415 | 4.0 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 224 | 0.0 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 12051703 | 1004.3 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 186181 | 15.5 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 12161 | 1.0 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 37741 | 3.1 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 561 | 0.0 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 14647 | 1.2 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 12038215 | 1003.2 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 105893 | 8.8 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 356602 | 29.7 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 7018 | 0.6 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 196132 | 16.3 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 176238 | 14.7 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 12603 | 1.1 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 466 | 0.0 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 11050 | 0.9 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 4848 | 0.4 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 1261 | 0.1 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 358622 | 29.9 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 13804 | 1.2 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 277 | 0.0 |
| scenarios/enemy/trog_scene101.txt | 12000 | 265 | 0.0 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 2174 | 0.2 |
| scenarios/enemy/trog_scene104.txt | 12000 | 709069 | 59.1 |
| scenarios/extra/mash_2p.txt | 12000 | 11916 | 1.0 |
| scenarios/extra/mash_c0.txt | 12000 | 7837 | 0.7 |
| scenarios/extra/mash_c0b.txt | 12000 | 6528 | 0.5 |
| scenarios/extra/mash_c1b.txt | 12000 | 69068 | 5.8 |
| scenarios/extra/mash_c2.txt | 12000 | 66827 | 5.6 |
| scenarios/extra/mash_c2b.txt | 12000 | 513 | 0.0 |
| scenarios/extra/mash_c3.txt | 12000 | 1446 | 0.1 |
| scenarios/extra/mash_c3b.txt | 12000 | 466 | 0.0 |
| scenarios/extra/mash_c3c.txt | 12000 | 6026 | 0.5 |
| scenarios/extra/mash_c3d.txt | 12000 | 446 | 0.0 |
| scenarios/extra/mash_c4.txt | 12000 | 6876 | 0.6 |
| scenarios/extra/mash_c4b.txt | 12000 | 6066 | 0.5 |
| scenarios/extra/mash_c4c.txt | 12000 | 6672 | 0.6 |
| scenarios/extra/mash_c5.txt | 12000 | 16376 | 1.4 |
| scenarios/extra/mash_c5b.txt | 12000 | 27645 | 2.3 |
| scenarios/modes/attract.txt | 12000 | 388074 | 32.3 |
| scenarios/modes/ending.txt | 12000 | 451249 | 37.6 |
| scenarios/modes/ending_end.txt | 12000 | 265056 | 22.1 |
| scenarios/modes/gameover.txt | 12000 | 7604 | 0.6 |
| scenarios/modes/screens.txt | 12000 | 388074 | 32.3 |
| scenarios/scene/scene_01.txt | 6500 | 1970 | 0.3 |
| scenarios/scene/scene_02.txt | 6500 | 7511 | 1.2 |
| scenarios/scene/scene_03.txt | 6500 | 2754 | 0.4 |
| scenarios/scene/scene_04.txt | 6500 | 7067 | 1.1 |
| scenarios/scene/scene_05.txt | 6500 | 2475 | 0.4 |
| scenarios/scene/scene_07.txt | 6500 | 221951 | 34.1 |
| scenarios/scene/scene_08.txt | 6500 | 223241 | 34.3 |
| scenarios/scene/scene_09.txt | 6500 | 236892 | 36.4 |
| scenarios/scene/scene_10.txt | 6500 | 121 | 0.0 |
| scenarios/scene/scene_105.txt | 6500 | 218084 | 33.6 |
| scenarios/scene/scene_109.txt | 6500 | 228 | 0.0 |
| scenarios/scene/scene_11.txt | 6500 | 132233 | 20.3 |
| scenarios/scene/scene_110.txt | 6500 | 522 | 0.1 |
| scenarios/scene/scene_111.txt | 6500 | 105873 | 16.3 |
| scenarios/scene/scene_14.txt | 6500 | 6379 | 1.0 |
| scenarios/scene/scene_24.txt | 6500 | 225063 | 34.6 |
| scenarios/scene/scene_25.txt | 6500 | 4024 | 0.6 |
| scenarios/scene/scene_26.txt | 6500 | 237583 | 36.6 |
| scenarios/scene/scene_27.txt | 6500 | 12490 | 1.9 |
| scenarios/scene/scene_28.txt | 6500 | 26846 | 4.1 |
| scenarios/scene/scene_32.txt | 6500 | 118935 | 18.3 |
| scenarios/scene/scene_33.txt | 6500 | 136 | 0.0 |
| scenarios/scene/scene_34.txt | 6500 | 522 | 0.1 |
| scenarios/scene/scene_35.txt | 6500 | 1347821 | 207.4 |
| scenarios/scene/scene_38.txt | 6500 | 13161 | 2.0 |
| scenarios/scene/scene_39.txt | 6500 | 1841 | 0.3 |
| scenarios/scene/scene_40.txt | 6500 | 8641 | 1.3 |
| scenarios/scene/scene_41.txt | 6500 | 163868 | 25.2 |
| scenarios/scene/scene_42.txt | 6500 | 104912 | 16.1 |
| scenarios/scene/scene_44.txt | 6500 | 105873 | 16.3 |
| scenarios/scene/scene_47.txt | 6500 | 1883363 | 289.7 |
| scenarios/scene/scene_54.txt | 6500 | 23416 | 3.6 |
| scenarios/scene/scene_58.txt | 6500 | 3142 | 0.5 |
| scenarios/scene/scene_59.txt | 6500 | 13504 | 2.1 |
| scenarios/scene/scene_64.txt | 6500 | 6583 | 1.0 |
| scenarios/scene/scene_65.txt | 6500 | 30289 | 4.7 |
| scenarios/scene/scene_66.txt | 6500 | 69154 | 10.6 |
| scenarios/scene/scene_67.txt | 6500 | 24971 | 3.8 |
| scenarios/scene/scene_90.txt | 6500 | 5184 | 0.8 |
| scenarios/scene_end/end_01.txt | 9000 | 2277 | 0.3 |
| scenarios/scene_end/end_02.txt | 9000 | 121 | 0.0 |
| scenarios/scene_end/end_03.txt | 9000 | 229 | 0.0 |
| scenarios/scene_end/end_04.txt | 9000 | 554 | 0.1 |
| scenarios/scene_end/end_05.txt | 9000 | 121 | 0.0 |
| scenarios/scene_end/end_06.txt | 9000 | 266457 | 29.6 |
| scenarios/scene_end/end_07.txt | 9000 | 314984 | 35.0 |
| scenarios/scene_end/end_09.txt | 9000 | 146795 | 16.3 |
| scenarios/scene_end/end_10.txt | 9000 | 121 | 0.0 |
| scenarios/scene_end/end_105.txt | 9000 | 398730 | 44.3 |
| scenarios/scene_end/end_11.txt | 9000 | 87684 | 9.7 |
| scenarios/scene_end/end_13.txt | 9000 | 19072 | 2.1 |
| scenarios/scene_end/end_14.txt | 9000 | 6379 | 0.7 |
| scenarios/scene_end/end_20.txt | 9000 | 162 | 0.0 |
| scenarios/scene_end/end_24.txt | 9000 | 380905 | 42.3 |
| scenarios/scene_end/end_26.txt | 9000 | 399049 | 44.3 |
| scenarios/scene_end/end_27.txt | 9000 | 121 | 0.0 |
| scenarios/scene_end/end_31.txt | 9000 | 1188677 | 132.1 |
| scenarios/scene_end/end_32.txt | 9000 | 116804 | 13.0 |
| scenarios/scene_end/end_33.txt | 9000 | 136 | 0.0 |
| scenarios/scene_end/end_34.txt | 9000 | 1397599 | 155.3 |
| scenarios/scene_end/end_35.txt | 9000 | 2156764 | 239.6 |
| scenarios/scene_end/end_38.txt | 9000 | 131966 | 14.7 |
| scenarios/scene_end/end_40.txt | 9000 | 833 | 0.1 |
| scenarios/scene_end/end_41.txt | 9000 | 129333 | 14.4 |
| scenarios/scene_end/end_42.txt | 9000 | 108341 | 12.0 |
| scenarios/scene_end/end_44.txt | 9000 | 117581 | 13.1 |
| scenarios/scene_end/end_46.txt | 9000 | 387101 | 43.0 |
| scenarios/scene_end/end_54.txt | 9000 | 83341 | 9.3 |
| scenarios/scene_end/end_58.txt | 9000 | 1202839 | 133.6 |
| scenarios/scene_end/end_59.txt | 9000 | 243 | 0.0 |
| scenarios/scene_end/end_64.txt | 9000 | 121 | 0.0 |
| scenarios/scene_end/end_65.txt | 9000 | 205 | 0.0 |
| scenarios/scene_end/end_66.txt | 9000 | 121 | 0.0 |
| scenarios/scene_end/end_67.txt | 9000 | 10995 | 1.2 |
| scenarios/scene_end/end_68.txt | 9000 | 1397603 | 155.3 |
