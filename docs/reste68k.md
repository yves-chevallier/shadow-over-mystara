# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 255 scénarios, soit 4081500 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **1080356 instructions 68000 exécutées** (hors attente active), soit 0.3 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 158 exécutent encore du 68000, 849 sont entièrement natives, **1552 jamais atteintes** (103352 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 615831 (comptées par sous-système ci-dessous, colonne « Hors index », et par plage plus bas).

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Hors index | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| C0e | EFFCT : effets | 443506 | 41.05 | 317085 | 170 | 59 | 7 | 104 | ae776 (19.1 %), ae298 (14.9 %), ae3ec (8.0 %) |
| C10 | SET : décors/animations de palette | 391535 | 36.24 | 76174 | 131 | 19 | 17 | 95 | c9068 (44.4 %), c919a (22.8 %), c8430 (11.0 %) |
| C04 | BOSS : boss | 200174 | 18.53 | 184177 | 353 | 37 | 8 | 308 | 81722 (17.3 %), 80d16 (17.3 %), 8a954 (7.7 %) |
| P3 | Comportement scénarisé des joueurs par scène | 24095 | 2.23 | 24095 | 3 | 0 | 2 | 1 |  |
| C08 | ESHL : tirs/attaques ennemis | 7626 | 0.71 | 4774 | 127 | 14 | 17 | 96 | a66c0 (32.3 %), a036e (21.9 %), a0566 (18.9 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 6233 | 0.58 | 6229 | 59 | 1 | 7 | 51 | 3baea (100.0 %) |
| C02 | ENEMY : ennemis | 1948 | 0.18 | 1632 | 205 | 3 | 6 | 196 | 76ec4 (96.2 %), 792f8 (2.5 %), 71e3c (1.3 %) |
| W1 | Boucle de partie et phases 0/4/12/20 | 1423 | 0.13 | 15 | 3 | 2 | 0 | 1 | 1c44a (99.7 %), 1c86e (0.3 %) |
| U2 | Petites routines / tables (rôle inconnu) | 886 | 0.08 | 832 | 17 | 3 | 13 | 1 | 1ac9a (40.7 %), 1ac72 (29.6 %), 1ac7c (29.6 %) |
| C06 | PSHL : tirs du joueur | 784 | 0.07 | 0 | 39 | 3 | 15 | 21 | 54872 (65.9 %), 54818 (31.0 %), 54700 (3.1 %) |
| S1 | Son : wrappers de commandes + FIFO | 628 | 0.06 | 600 | 410 | 2 | 404 | 4 | e27da (78.6 %), e2430 (21.4 %) |
| V2 | Texte, rectangles et blocs de tuiles | 381 | 0.04 | 206 | 33 | 3 | 25 | 5 | 2a52 (70.9 %), 2a42 (20.6 %), 2a36 (8.6 %) |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 340 | 0.03 | 0 | 69 | 1 | 58 | 10 | 10158 (100.0 %) |
| C12 | PEFF : effets du joueur | 216 | 0.02 | 0 | 107 | 2 | 35 | 70 | 4c930 (91.7 %), 4f132 (8.3 %) |
| C14 | SOBJ : objets système/UI | 156 | 0.01 | 0 | 25 | 2 | 7 | 16 | df4bc (84.6 %), df896 (15.4 %) |
| F1 | Tâche de boot, avertissements région | 152 | 0.01 | 0 | 5 | 2 | 2 | 1 | 133f6 (51.3 %), 133f0 (48.7 %) |
| W0 | Dispatch des 10 classes d'objets | 119 | 0.01 | 0 | 10 | 1 | 9 | 0 | 39e86 (100.0 %) |
| V3 | Palettes et fondus | 104 | 0.01 | 0 | 38 | 1 | 30 | 7 | 1a5e8 (100.0 %) |
| C0c | ITEM : objets ramassables | 22 | 0.00 | 0 | 14 | 1 | 2 | 11 | dd12c (100.0 %) |
| W3 | Scripts de scène (phases 16/28) | 12 | 0.00 | 12 | 82 | 0 | 11 | 71 |  |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 10 | 0.00 | 0 | 49 | 1 | 1 | 47 | 180ac (100.0 %) |
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
| 0b1d14-0b1dc4 | C0e | 75758 | 7.01 |
| 0b2f46-0b301e | C0e | 63507 | 5.88 |
| 08a75c-08acb4 | C04 | 45120 | 4.18 |
| 0b7f28-0b7fe0 | C0e | 44205 | 4.09 |
| 0b1c02-0b1c94 | C0e | 43585 | 4.03 |
| 0c8f76-0c9062 | C10 | 43303 | 4.01 |
| 07acea-07b1ca | C04 | 42819 | 3.96 |
| 07f418-07f74a | C04 | 39802 | 3.68 |
| 0b68f4-0b6ba2 | C0e | 33451 | 3.10 |
| 0c8352-0c842e | C10 | 29999 | 2.78 |
| 023e02-023f3a | P3 | 24073 | 2.23 |
| 0affca-0b00e8 | C0e | 18305 | 1.69 |
| 0ba0b0-0ba17e | C0e | 14390 | 1.33 |
| 0a8594-0a8752 | C0e | 11714 | 1.08 |
| 08b628-08b730 | C04 | 10213 | 0.95 |
| 080a90-080b08 | C04 | 9835 | 0.91 |
| 07bb66-07bbb0 | C04 | 9016 | 0.83 |
| 07bf22-07c010 | C04 | 6667 | 0.62 |
| 0b66f8-0b68aa | C0e | 6413 | 0.59 |
| 0a7232-0a72b2 | C08 | 4766 | 0.44 |
| 08b7a4-08b9c4 | C04 | 4590 | 0.42 |
| 08b48a-08b4da | C04 | 4048 | 0.37 |
| 035d08-035d50 | P4 | 3152 | 0.29 |
| 0b3df4-0b3df4 | C0e | 2420 | 0.22 |
| 07fa78-07fc32 | C04 | 2301 | 0.21 |
| 0801de-08030a | C04 | 2207 | 0.20 |
| 07fcec-07fd0a | C04 | 2058 | 0.19 |
| 07b5fe-07b898 | C04 | 1732 | 0.16 |
| 07b236-07b410 | C04 | 1684 | 0.16 |
| 0b896e-0b8a04 | C0e | 1673 | 0.15 |
| 0c8c86-0c8cd8 | C10 | 1585 | 0.15 |
| 03b36e-03b44a | P4 | 1291 | 0.12 |
| 0d40e8-0d410e | C10 | 1222 | 0.11 |
| 035c26-035c2e | P4 | 1152 | 0.11 |
| 078ad2-078b48 | C02 | 1088 | 0.10 |
| 01ad6a-01ad96 | U2 | 832 | 0.08 |
| 07fdc4-07fe78 | C04 | 694 | 0.06 |
| 0e0ad8-0e0b16 | S1 | 600 | 0.06 |
| 0ae11a-0ae210 | C0e | 561 | 0.05 |
| 071680-0716f6 | C02 | 544 | 0.05 |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 0c9068 | FUN_000c9068 | C10 | 102 | 140175 | 12.97 | 2 |
| 0c919a | FUN_000c919a | C10 | 14 | 71828 | 6.65 | 2 |
| 0c8430 | FUN_000c8430 | C10 | 2142 | 34640 | 3.21 | 2 |
| 0c9128 | FUN_000c9128 | C10 | 60 | 26845 | 2.48 | 2 |
| 0c91a8 | FUN_000c91a8 | C10 | 28 | 26544 | 2.46 | 2 |
| 0ae776 | FUN_000ae776 | C0e | 22 | 24200 | 2.24 | 242 |
| 0ae298 | FUN_000ae298 | C0e | 228 | 18819 | 1.74 | 11 |
| 0ae3ec | FUN_000ae3ec | C0e | 60 | 10172 | 0.94 | 134 |
| 0b3f6a | FUN_000b3f6a | C0e | 70 | 9251 | 0.86 | 4 |
| 0b1a7a | FUN_000b1a7a | C0e | 30 | 9212 | 0.85 | 82 |
| 0c9164 | FUN_000c9164 | C10 | 54 | 6911 | 0.64 | 2 |
| 0ab7d8 | FUN_000ab7d8 | C0e | 34 | 6535 | 0.60 | 34 |
| 0c9116 | FUN_000c9116 | C10 | 18 | 6356 | 0.59 | 2 |
| 0abb70 | FUN_000abb70 | C0e | 126 | 4978 | 0.46 | 43 |
| 0b4088 | FUN_000b4088 | C0e | 50 | 4564 | 0.42 | 7 |
| 0b3036 | FUN_000b3036 | C0e | 26 | 3750 | 0.35 | 2 |
| 0b2ef6 | FUN_000b2ef6 | C0e | 76 | 3540 | 0.33 | 2 |
| 0af346 | FUN_000af346 | C0e | 26 | 2904 | 0.27 | 242 |
| 081722 | FUN_00081722 | C04 | 26 | 2772 | 0.26 | 2 |
| 080d16 | FUN_00080d16 | C04 | 14 | 2760 | 0.26 | 1 |
| 0abec8 | FUN_000abec8 | C0e | 34 | 2736 | 0.25 | 68 |
| 0b6596 | FUN_000b6596 | C0e | 64 | 2580 | 0.24 | 64 |
| 0b68b0 | FUN_000b68b0 | C0e | 68 | 2237 | 0.21 | 1 |
| 0a8af8 | FUN_000a8af8 | C0e | 48 | 2008 | 0.19 | 46 |
| 0b0102 | FUN_000b0102 | C0e | 224 | 1671 | 0.15 | 2 |
| 0b3fd4 | FUN_000b3fd4 | C0e | 46 | 1641 | 0.15 | 3 |
| 0b378a | FUN_000b378a | C0e | 34 | 1496 | 0.14 | 22 |
| 01c44a | FUN_0001c44a | W1 | 16 | 1404 | 0.13 | 39 |
| 0b9c6a | FUN_000b9c6a | C0e | 30 | 1284 | 0.12 | 4 |
| 08a954 | FUN_0008a954 | C04 | 2 | 1231 | 0.11 | 1 |
| 07c4da | FUN_0007c4da | C04 | 50 | 1168 | 0.11 | 1 |
| 0b1764 | FUN_000b1764 | C0e | 74 | 1080 | 0.10 | 12 |
| 07f842 | FUN_0007f842 | C04 | 322 | 1003 | 0.09 | 2 |
| 08ca22 | FUN_0008ca22 | C04 | 40 | 960 | 0.09 | 1 |
| 08030e | FUN_0008030e | C04 | 34 | 929 | 0.09 | 2 |
| 0a66c0 | FUN_000a66c0 | C08 | 34 | 921 | 0.09 | 2 |
| 0b9c88 | FUN_000b9c88 | C0e | 30 | 911 | 0.08 | 2 |
| 0ba18a | FUN_000ba18a | C0e | 34 | 910 | 0.08 | 2 |
| 0b12c0 | FUN_000b12c0 | C0e | 38 | 873 | 0.08 | 32 |
| 0b7dec | FUN_000b7dec | C0e | 58 | 866 | 0.08 | 5 |
| 0a836c | FUN_000a836c | C0e | 34 | 864 | 0.08 | 6 |
| 0b17ba | FUN_000b17ba | C0e | 78 | 828 | 0.08 | 12 |
| 0d90b0 | FUN_000d90b0 | C10 | 270 | 759 | 0.07 | 1 |
| 08189e | FUN_0008189e | C04 | 20 | 750 | 0.07 | 2 |
| 0ab708 | FUN_000ab708 | C0e | 30 | 749 | 0.07 | 68 |
| 08173c | FUN_0008173c | C04 | 40 | 717 | 0.07 | 2 |
| 0b141a | FUN_000b141a | C0e | 38 | 666 | 0.06 | 37 |
| 0aaa40 | FUN_000aaa40 | C0e | 68 | 663 | 0.06 | 6 |
| 0a036e | FUN_000a036e | C08 | 114 | 624 | 0.06 | 16 |
| 0816f8 | FUN_000816f8 | C04 | 26 | 607 | 0.06 | 2 |
| 07c430 | FUN_0007c430 | C04 | 76 | 582 | 0.05 | 1 |
| 08bff0 | FUN_0008bff0 | C04 | 54 | 580 | 0.05 | 1 |
| 0b4002 | FUN_000b4002 | C0e | 38 | 568 | 0.05 | 10 |
| 0b66e4 | FUN_000b66e4 | C0e | 20 | 561 | 0.05 | 1 |
| 0a0566 | FUN_000a0566 | C08 | 72 | 540 | 0.05 | 16 |
| 0c90ce | FUN_000c90ce | C10 | 72 | 520 | 0.05 | 2 |
| 054872 | FUN_00054872 | C06 | 46 | 517 | 0.05 | 8 |
| 081764 | FUN_00081764 | C04 | 70 | 490 | 0.05 | 1 |
| 08c026 | FUN_0008c026 | C04 | 108 | 364 | 0.03 | 1 |
| 010158 | FUN_00010158 | E1 | 130 | 340 | 0.03 | 3 |
| 0b4026 | FUN_000b4026 | C0e | 42 | 328 | 0.03 | 3 |
| 076ec4 | FUN_00076ec4 | C02 | 74 | 304 | 0.03 | 13 |
| 0b4f60 | FUN_000b4f60 | C0e | 40 | 300 | 0.03 | 1 |
| 0b6c42 | FUN_000b6c42 | C0e | 44 | 297 | 0.03 | 1 |
| 0a8ae8 | FUN_000a8ae8 | C0e | 16 | 290 | 0.03 | 46 |
| 07f984 | FUN_0007f984 | C04 | 132 | 276 | 0.03 | 1 |
| 0cf182 | FUN_000cf182 | C10 | 46 | 272 | 0.03 | 9 |
| 0afe02 | FUN_000afe02 | C0e | 30 | 254 | 0.02 | 19 |
| 0a31c2 | FUN_000a31c2 | C08 | 34 | 253 | 0.02 | 1 |
| 054818 | FUN_00054818 | C06 | 42 | 243 | 0.02 | 4 |
| 0aba16 | FUN_000aba16 | C0e | 56 | 242 | 0.02 | 5 |
| 0a817c | FUN_000a817c | C0e | 78 | 240 | 0.02 | 5 |
| 0b3f40 | FUN_000b3f40 | C0e | 42 | 232 | 0.02 | 4 |
| 0b4f44 | FUN_000b4f44 | C0e | 28 | 224 | 0.02 | 5 |
| 04c930 | FUN_0004c930 | C12 | 24 | 198 | 0.02 | 11 |
| 0cc200 | FUN_000cc200 | C10 | 90 | 175 | 0.02 | 11 |
| 0c8cda | FUN_000c8cda | C10 | 28 | 170 | 0.02 | 2 |
| 0b3fb0 | FUN_000b3fb0 | C0e | 36 | 136 | 0.01 | 13 |
| 0a6824 | FUN_000a6824 | C08 | 64 | 135 | 0.01 | 9 |
| 0df4bc | FUN_000df4bc | C14 | 24 | 132 | 0.01 | 13 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 158 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 308 | 15500 |
| C02 | ENEMY : ennemis | 196 | 9094 |
| C10 | SET : décors/animations de palette | 95 | 7964 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 96 | 5952 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
| C0e | EFFCT : effets | 104 | 4874 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 47 | 4386 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| P4 | Noyau joueur (états, 26 actions génériques) | 51 | 3786 |
| M1 | Menu TEST (service) | 76 | 3270 |
| B1 | Scrolling / caméras par scène | 31 | 2630 |
| W3 | Scripts de scène (phases 16/28) | 71 | 2396 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| M2 | EEPROM, configuration, comptabilité | 42 | 2152 |
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

## Appels Rust qui exécutent encore du 68000

Instructions 68000 exécutées pendant un appel `call`/`call_at` (ou une de leurs enveloppes : `Ctx::call`, `Cx::call`, `call_d0`...), attribuées à l'appel le plus intérieur. **Hors appel** : code d'origine où une tâche reprend (adresse de reprise sans native), où une native continue par `tail`/`jump`, ou appelé par du 68000 lui-même hors de tout appel Rust. C'est la liste de ce qui empêche encore la boucle d'image sans 68000 (`docs/architecture.md`, point 4).

Hors appel : 962 instructions (0.09 %).

| Module | Instr. 68000 | % | Sites | Routines appelées (les plus gourmandes) |
|---|---:|---:|---:|---|
| lib | 1079394 | 99.91 | 1 | c8352 (388961), b1c1a (119379), b2f40 (70763), 7f41a (68029), 8a75c (67583), 7acea (64615) + 124 autres |

### Sites d'appel (les 60 plus gourmands)

| Site | Routine | Instr. 68000 |
|---|---|---:|
| lib.rs:273 | c8352 | 388961 |
| lib.rs:273 | b1c1a | 119379 |
| lib.rs:273 | b2f40 | 70763 |
| lib.rs:273 | 7f41a | 68029 |
| lib.rs:273 | 8a75c | 67583 |
| lib.rs:273 | 7acea | 64615 |
| lib.rs:273 | b7f28 | 44229 |
| lib.rs:273 | b68f4 | 34723 |
| lib.rs:273 | ae776 | 24200 |
| lib.rs:273 | 23e02 | 24073 |
| lib.rs:273 | ada96 | 19378 |
| lib.rs:273 | affca | 18398 |
| lib.rs:273 | ba0b0 | 14390 |
| lib.rs:273 | a8594 | 11714 |
| lib.rs:273 | b3f6a | 10208 |
| lib.rs:273 | ae3ec | 10172 |
| lib.rs:273 | b1a7a | 9212 |
| lib.rs:273 | b66f8 | 6975 |
| lib.rs:273 | ab7d8 | 6535 |
| lib.rs:273 | b4088 | 5580 |
| lib.rs:273 | abb70 | 4978 |
| lib.rs:273 | 35c26 | 4304 |
| lib.rs:273 | a7234 | 3274 |
| lib.rs:273 | af346 | 2904 |
| lib.rs:273 | abec8 | 2736 |
| lib.rs:273 | b6596 | 2580 |
| lib.rs:273 | a8ae8 | 2356 |
| lib.rs:273 | b896e | 2273 |
| lib.rs:273 | b3fd4 | 2088 |
| lib.rs:273 | b0102 | 1671 |
| lib.rs:273 | b378a | 1496 |
| lib.rs:273 | a727c | 1492 |
| lib.rs:273 | 1c44a | 1404 |
| lib.rs:273 | b6a74 | 1311 |
| lib.rs:273 | b9c6a | 1284 |
| lib.rs:273 | d40e8 | 1222 |
| lib.rs:273 | 78ad2 | 1096 |
| lib.rs:273 | b1764 | 1080 |
| lib.rs:273 | a66c0 | 921 |
| lib.rs:273 | b9c88 | 911 |
| lib.rs:273 | ba18a | 910 |
| lib.rs:273 | b12c0 | 873 |
| lib.rs:273 | b7dec | 866 |
| lib.rs:273 | a836c | 864 |
| lib.rs:273 | 1ad6a | 832 |
| lib.rs:273 | b17ba | 828 |
| lib.rs:273 | ab708 | 749 |
| lib.rs:273 | 3b3d8 | 671 |
| lib.rs:273 | b141a | 666 |
| lib.rs:273 | aaa40 | 663 |
| lib.rs:273 | ae11a | 660 |
| lib.rs:273 | a036e | 624 |
| lib.rs:273 | 3b3de | 602 |
| lib.rs:273 | b4002 | 568 |
| lib.rs:273 | 71680 | 548 |
| lib.rs:273 | a0566 | 540 |
| lib.rs:273 | 54872 | 517 |
| lib.rs:273 | 3626c | 425 |
| lib.rs:273 | b411e | 370 |
| lib.rs:273 | 10158 | 340 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 260 | 0.0 |
| scenarios/boss/boss10_scene66.txt | 12000 | 120 | 0.0 |
| scenarios/boss/boss11_scene42.txt | 12000 | 279 | 0.0 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 221 | 0.0 |
| scenarios/boss/boss12_scene40.txt | 12000 | 641 | 0.1 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 204 | 0.0 |
| scenarios/boss/boss13_dragon.txt | 12000 | 3535 | 0.3 |
| scenarios/boss/boss15_scene46.txt | 12000 | 3243 | 0.3 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 926 | 0.1 |
| scenarios/boss/boss17_scene13.txt | 12000 | 150 | 0.0 |
| scenarios/boss/boss18_scene26.txt | 12000 | 609 | 0.1 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 698 | 0.1 |
| scenarios/boss/boss19_scene32.txt | 12000 | 839 | 0.1 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 886 | 0.1 |
| scenarios/boss/boss21_scene44.txt | 12000 | 135 | 0.0 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 213 | 0.0 |
| scenarios/boss/boss5_scene58.txt | 12000 | 190 | 0.0 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 227 | 0.0 |
| scenarios/boss/boss6_scene24.txt | 12000 | 154 | 0.0 |
| scenarios/boss/boss8_scene64.txt | 12000 | 195 | 0.0 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 242 | 0.0 |
| scenarios/bot30k.txt | 30000 | 356 | 0.0 |
| scenarios/bot_2p.txt | 30000 | 320 | 0.0 |
| scenarios/bot_char0.txt | 30000 | 320 | 0.0 |
| scenarios/bot_char1.txt | 30000 | 493 | 0.0 |
| scenarios/bot_char2.txt | 30000 | 259 | 0.0 |
| scenarios/bot_char3.txt | 30000 | 315 | 0.0 |
| scenarios/bot_char4.txt | 30000 | 358 | 0.0 |
| scenarios/bot_char5.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_2p_a.txt | 30000 | 2427 | 0.1 |
| scenarios/cov/cov_2p_b.txt | 30000 | 274 | 0.0 |
| scenarios/cov/cov_2p_c.txt | 30000 | 4626 | 0.2 |
| scenarios/cov/cov_3p.txt | 30000 | 859 | 0.0 |
| scenarios/cov/cov_4p_a.txt | 30000 | 3774 | 0.1 |
| scenarios/cov/cov_4p_b.txt | 30000 | 146 | 0.0 |
| scenarios/cov/cov_4p_c.txt | 30000 | 142 | 0.0 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 718 | 0.0 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 1375 | 0.0 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 320 | 0.0 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 327 | 0.0 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 334 | 0.0 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 1042 | 0.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 369 | 0.0 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 259 | 0.0 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 250 | 0.0 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 1051 | 0.0 |
| scenarios/cov/cov_service.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_ecrans.txt | 30000 | 739 | 0.0 |
| scenarios/cov/cov_service_reglages.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_tests.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_variante.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 3464 | 0.1 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 112 | 0.0 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 918 | 0.0 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 5219 | 0.2 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 3210 | 0.1 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 1959 | 0.1 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 771 | 0.0 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 112 | 0.0 |
| scenarios/cov/covb_diff0.txt | 30000 | 2548 | 0.1 |
| scenarios/cov/covb_diff7.txt | 30000 | 538 | 0.0 |
| scenarios/cov/covb_langue4.txt | 30000 | 291881 | 9.7 |
| scenarios/cov/covb_langue6.txt | 30000 | 119242 | 4.0 |
| scenarios/cov/covb_long3p_02.txt | 30000 | 148005 | 4.9 |
| scenarios/cov/covb_long3p_05.txt | 30000 | 17625 | 0.6 |
| scenarios/cov/covb_long3p_10.txt | 30000 | 112 | 0.0 |
| scenarios/cov/covb_long3p_11.txt | 30000 | 354 | 0.0 |
| scenarios/cov/covb_long3p_14.txt | 30000 | 244 | 0.0 |
| scenarios/cov/covb_long3p_20.txt | 30000 | 153 | 0.0 |
| scenarios/cov/covb_long3p_27.txt | 30000 | 156 | 0.0 |
| scenarios/cov/covb_long3p_59.txt | 30000 | 857 | 0.0 |
| scenarios/cov/covb_long_02.txt | 30000 | 357 | 0.0 |
| scenarios/cov/covb_long_05.txt | 30000 | 358 | 0.0 |
| scenarios/cov/covb_long_10.txt | 30000 | 196 | 0.0 |
| scenarios/cov/covb_long_11.txt | 30000 | 141709 | 4.7 |
| scenarios/cov/covb_long_14.txt | 30000 | 295 | 0.0 |
| scenarios/cov/covb_long_20.txt | 30000 | 153 | 0.0 |
| scenarios/cov/covb_long_27.txt | 30000 | 65099 | 2.2 |
| scenarios/cov/covb_long_31.txt | 30000 | 186 | 0.0 |
| scenarios/cov/covb_long_43.txt | 30000 | 779 | 0.0 |
| scenarios/cov/covb_long_59.txt | 30000 | 120 | 0.0 |
| scenarios/cov/covb_magie_c0.txt | 30000 | 479 | 0.0 |
| scenarios/cov/covb_magie_c4.txt | 30000 | 638 | 0.0 |
| scenarios/cov/covb_mortal_3p.txt | 30000 | 289 | 0.0 |
| scenarios/cov/covb_mortal_4p.txt | 30000 | 443 | 0.0 |
| scenarios/cov/covb_rang_10.txt | 30000 | 112 | 0.0 |
| scenarios/cov/covb_rang_14.txt | 30000 | 480 | 0.0 |
| scenarios/cov/covb_rang_39.txt | 30000 | 405 | 0.0 |
| scenarios/cov/covb_rang_43.txt | 30000 | 481 | 0.0 |
| scenarios/cov/covb_region_0.txt | 30000 | 718 | 0.0 |
| scenarios/cov/covb_region_10.txt | 30000 | 718 | 0.0 |
| scenarios/cov/covb_region_2.txt | 30000 | 718 | 0.0 |
| scenarios/cov/covb_region_4.txt | 30000 | 718 | 0.0 |
| scenarios/cov/covb_region_6.txt | 30000 | 718 | 0.0 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 400 | 0.0 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 127 | 0.0 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 127 | 0.0 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 144 | 0.0 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 444 | 0.0 |
| scenarios/enemy/elf_scene92.txt | 12000 | 234 | 0.0 |
| scenarios/enemy/elf_scene94.txt | 12000 | 192 | 0.0 |
| scenarios/enemy/elf_scene96.txt | 12000 | 2859 | 0.2 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 279 | 0.0 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 114 | 0.0 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 168 | 0.0 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 2194 | 0.2 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 115 | 0.0 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 214 | 0.0 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 239 | 0.0 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 589 | 0.0 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 427 | 0.0 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 142 | 0.0 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 138 | 0.0 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 475 | 0.0 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 203 | 0.0 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 341 | 0.0 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 337 | 0.0 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 139 | 0.0 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 112 | 0.0 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 1380 | 0.1 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 3507 | 0.3 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 315 | 0.0 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 260 | 0.0 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 724 | 0.1 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 241 | 0.0 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 127 | 0.0 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 238 | 0.0 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 112 | 0.0 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 185 | 0.0 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 150 | 0.0 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 131 | 0.0 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 118 | 0.0 |
| scenarios/enemy/trog_scene101.txt | 12000 | 112 | 0.0 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 112 | 0.0 |
| scenarios/enemy/trog_scene104.txt | 12000 | 170 | 0.0 |
| scenarios/extra/mash_2p.txt | 12000 | 361 | 0.0 |
| scenarios/extra/mash_c0.txt | 12000 | 313 | 0.0 |
| scenarios/extra/mash_c0b.txt | 12000 | 320 | 0.0 |
| scenarios/extra/mash_c1b.txt | 12000 | 582 | 0.0 |
| scenarios/extra/mash_c2.txt | 12000 | 422 | 0.0 |
| scenarios/extra/mash_c2b.txt | 12000 | 271 | 0.0 |
| scenarios/extra/mash_c3.txt | 12000 | 281 | 0.0 |
| scenarios/extra/mash_c3b.txt | 12000 | 281 | 0.0 |
| scenarios/extra/mash_c3c.txt | 12000 | 315 | 0.0 |
| scenarios/extra/mash_c3d.txt | 12000 | 281 | 0.0 |
| scenarios/extra/mash_c4.txt | 12000 | 328 | 0.0 |
| scenarios/extra/mash_c4b.txt | 12000 | 320 | 0.0 |
| scenarios/extra/mash_c4c.txt | 12000 | 320 | 0.0 |
| scenarios/extra/mash_c5.txt | 12000 | 454 | 0.0 |
| scenarios/extra/mash_c5b.txt | 12000 | 345 | 0.0 |
| scenarios/modes/attract.txt | 12000 | 718 | 0.1 |
| scenarios/modes/ending.txt | 12000 | 1488 | 0.1 |
| scenarios/modes/ending_end.txt | 12000 | 1513 | 0.1 |
| scenarios/modes/gameover.txt | 12000 | 358 | 0.0 |
| scenarios/modes/screens.txt | 12000 | 718 | 0.1 |
| scenarios/scene/scene_01.txt | 6500 | 259 | 0.0 |
| scenarios/scene/scene_02.txt | 6500 | 234 | 0.0 |
| scenarios/scene/scene_03.txt | 6500 | 153 | 0.0 |
| scenarios/scene/scene_04.txt | 6500 | 2198 | 0.3 |
| scenarios/scene/scene_05.txt | 6500 | 194 | 0.0 |
| scenarios/scene/scene_07.txt | 6500 | 1757 | 0.3 |
| scenarios/scene/scene_08.txt | 6500 | 979 | 0.2 |
| scenarios/scene/scene_09.txt | 6500 | 1942 | 0.3 |
| scenarios/scene/scene_10.txt | 6500 | 112 | 0.0 |
| scenarios/scene/scene_105.txt | 6500 | 728 | 0.1 |
| scenarios/scene/scene_109.txt | 6500 | 219 | 0.0 |
| scenarios/scene/scene_11.txt | 6500 | 41423 | 6.4 |
| scenarios/scene/scene_110.txt | 6500 | 122 | 0.0 |
| scenarios/scene/scene_111.txt | 6500 | 127 | 0.0 |
| scenarios/scene/scene_14.txt | 6500 | 234 | 0.0 |
| scenarios/scene/scene_24.txt | 6500 | 140 | 0.0 |
| scenarios/scene/scene_25.txt | 6500 | 370 | 0.1 |
| scenarios/scene/scene_26.txt | 6500 | 163 | 0.0 |
| scenarios/scene/scene_27.txt | 6500 | 419 | 0.1 |
| scenarios/scene/scene_28.txt | 6500 | 145 | 0.0 |
| scenarios/scene/scene_32.txt | 6500 | 826 | 0.1 |
| scenarios/scene/scene_33.txt | 6500 | 127 | 0.0 |
| scenarios/scene/scene_34.txt | 6500 | 122 | 0.0 |
| scenarios/scene/scene_35.txt | 6500 | 180 | 0.0 |
| scenarios/scene/scene_38.txt | 6500 | 119 | 0.0 |
| scenarios/scene/scene_39.txt | 6500 | 112 | 0.0 |
| scenarios/scene/scene_40.txt | 6500 | 191 | 0.0 |
| scenarios/scene/scene_41.txt | 6500 | 3195 | 0.5 |
| scenarios/scene/scene_42.txt | 6500 | 199 | 0.0 |
| scenarios/scene/scene_44.txt | 6500 | 127 | 0.0 |
| scenarios/scene/scene_47.txt | 6500 | 112 | 0.0 |
| scenarios/scene/scene_54.txt | 6500 | 144 | 0.0 |
| scenarios/scene/scene_58.txt | 6500 | 131 | 0.0 |
| scenarios/scene/scene_59.txt | 6500 | 120 | 0.0 |
| scenarios/scene/scene_64.txt | 6500 | 146 | 0.0 |
| scenarios/scene/scene_65.txt | 6500 | 212 | 0.0 |
| scenarios/scene/scene_66.txt | 6500 | 144 | 0.0 |
| scenarios/scene/scene_67.txt | 6500 | 1075 | 0.2 |
| scenarios/scene/scene_90.txt | 6500 | 248 | 0.0 |
| scenarios/scene_end/end_01.txt | 9000 | 294 | 0.0 |
| scenarios/scene_end/end_02.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_03.txt | 9000 | 134 | 0.0 |
| scenarios/scene_end/end_04.txt | 9000 | 545 | 0.1 |
| scenarios/scene_end/end_05.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_06.txt | 9000 | 1755 | 0.2 |
| scenarios/scene_end/end_07.txt | 9000 | 2524 | 0.3 |
| scenarios/scene_end/end_09.txt | 9000 | 118 | 0.0 |
| scenarios/scene_end/end_10.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_105.txt | 9000 | 1102 | 0.1 |
| scenarios/scene_end/end_11.txt | 9000 | 354 | 0.0 |
| scenarios/scene_end/end_13.txt | 9000 | 118 | 0.0 |
| scenarios/scene_end/end_14.txt | 9000 | 234 | 0.0 |
| scenarios/scene_end/end_20.txt | 9000 | 153 | 0.0 |
| scenarios/scene_end/end_24.txt | 9000 | 119 | 0.0 |
| scenarios/scene_end/end_26.txt | 9000 | 123 | 0.0 |
| scenarios/scene_end/end_27.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_31.txt | 9000 | 160 | 0.0 |
| scenarios/scene_end/end_32.txt | 9000 | 826 | 0.1 |
| scenarios/scene_end/end_33.txt | 9000 | 127 | 0.0 |
| scenarios/scene_end/end_34.txt | 9000 | 147 | 0.0 |
| scenarios/scene_end/end_35.txt | 9000 | 142 | 0.0 |
| scenarios/scene_end/end_38.txt | 9000 | 119 | 0.0 |
| scenarios/scene_end/end_40.txt | 9000 | 217 | 0.0 |
| scenarios/scene_end/end_41.txt | 9000 | 917 | 0.1 |
| scenarios/scene_end/end_42.txt | 9000 | 183 | 0.0 |
| scenarios/scene_end/end_44.txt | 9000 | 119 | 0.0 |
| scenarios/scene_end/end_46.txt | 9000 | 1550 | 0.2 |
| scenarios/scene_end/end_54.txt | 9000 | 147 | 0.0 |
| scenarios/scene_end/end_58.txt | 9000 | 166 | 0.0 |
| scenarios/scene_end/end_59.txt | 9000 | 234 | 0.0 |
| scenarios/scene_end/end_64.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_65.txt | 9000 | 196 | 0.0 |
| scenarios/scene_end/end_66.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_67.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/end_68.txt | 9000 | 151 | 0.0 |
| scenarios/scene_end/trans_02.txt | 9000 | 126 | 0.0 |
| scenarios/scene_end/trans_04.txt | 9000 | 392 | 0.0 |
| scenarios/scene_end/trans_05.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/trans_10.txt | 9000 | 385 | 0.0 |
| scenarios/scene_end/trans_11.txt | 9000 | 354 | 0.0 |
| scenarios/scene_end/trans_11_tard.txt | 9000 | 354 | 0.0 |
| scenarios/scene_end/trans_13.txt | 9000 | 45362 | 5.0 |
| scenarios/scene_end/trans_13_tard.txt | 9000 | 31870 | 3.5 |
| scenarios/scene_end/trans_14.txt | 9000 | 241 | 0.0 |
| scenarios/scene_end/trans_20.txt | 9000 | 160 | 0.0 |
| scenarios/scene_end/trans_32.txt | 9000 | 1566 | 0.2 |
| scenarios/scene_end/trans_33.txt | 9000 | 44368 | 4.9 |
| scenarios/scene_end/trans_34.txt | 9000 | 115 | 0.0 |
| scenarios/scene_end/trans_35.txt | 9000 | 175 | 0.0 |
| scenarios/scene_end/trans_38.txt | 9000 | 119 | 0.0 |
| scenarios/scene_end/trans_40.txt | 9000 | 112 | 0.0 |
| scenarios/scene_end/trans_42.txt | 9000 | 119 | 0.0 |
| scenarios/scene_end/trans_44.txt | 9000 | 119 | 0.0 |
| scenarios/scene_end/trans_46.txt | 9000 | 1366 | 0.2 |
| scenarios/scene_end/trans_67.txt | 9000 | 122 | 0.0 |
