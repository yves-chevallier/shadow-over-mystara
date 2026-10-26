# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 255 scénarios, soit 4081500 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **2564 instructions 68000 exécutées** (hors attente active), soit 0.0 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 22 exécutent encore du 68000, 935 sont entièrement natives, **1602 jamais atteintes** (108460 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 477 (comptées par sous-système ci-dessous, colonne « Hors index », et par plage plus bas).

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Hors index | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| C10 | SET : décors/animations de palette | 1198 | 46.72 | 7 | 131 | 8 | 17 | 106 | d90b0 (63.7 %), cf182 (22.8 %), d5f12 (4.0 %) |
| C08 | ESHL : tirs/attaques ennemis | 775 | 30.23 | 8 | 127 | 11 | 20 | 96 | a31c2 (33.0 %), a6824 (17.6 %), a2f96 (14.9 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 207 | 8.07 | 207 | 59 | 0 | 8 | 51 |  |
| V2 | Texte, rectangles et blocs de tuiles | 206 | 8.03 | 206 | 33 | 0 | 28 | 5 |  |
| W0 | Dispatch des 10 classes d'objets | 119 | 4.64 | 0 | 10 | 1 | 9 | 0 | 39e86 (100.0 %) |
| P3 | Comportement scénarisé des joueurs par scène | 22 | 0.86 | 22 | 3 | 0 | 2 | 1 |  |
| W1 | Boucle de partie et phases 0/4/12/20 | 19 | 0.74 | 15 | 3 | 1 | 1 | 1 | 1c86e (100.0 %) |
| W3 | Scripts de scène (phases 16/28) | 12 | 0.47 | 12 | 82 | 0 | 11 | 71 |  |
| O3 | Aides communes aux types (animations par type) | 6 | 0.23 | 0 | 25 | 1 | 8 | 16 | 3e28e (100.0 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 0 | 0.00 | 0 | 17 | 0 | 14 | 3 |  |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 0 | 0.00 | 0 | 2 | 0 | 0 | 2 |  |
| K3 | Entrées vers enregistrements joueur | 0 | 0.00 | 0 | 3 | 0 | 3 | 0 |  |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 0 | 16 | 0 | 0 | 16 |  |
| V1 | Primitives vidéo (attente, couches, effacements) | 0 | 0.00 | 0 | 13 | 0 | 13 | 0 |  |
| V3 | Palettes et fondus | 0 | 0.00 | 0 | 38 | 0 | 31 | 7 |  |
| V4 | Construction de la liste de sprites (OBJ RAM) | 0 | 0.00 | 0 | 15 | 0 | 2 | 13 |  |
| B1 | Scrolling / caméras par scène | 0 | 0.00 | 0 | 42 | 0 | 11 | 31 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 0 | 22 | 0 | 22 | 0 |  |
| O2 | Pools d'objets et listes triées | 0 | 0.00 | 0 | 41 | 0 | 28 | 13 |  |
| U1 | RNG et files d'affichage (pop-ups) | 0 | 0.00 | 0 | 18 | 0 | 14 | 4 |  |
| U2 | Petites routines / tables (rôle inconnu) | 0 | 0.00 | 0 | 17 | 0 | 16 | 1 |  |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 0 | 0.00 | 0 | 69 | 0 | 59 | 10 |  |
| X1 | Collisions (boîtes, coups, poussée) | 0 | 0.00 | 0 | 39 | 0 | 0 | 39 |  |
| X2 | Dégâts, terrain, placement | 0 | 0.00 | 0 | 19 | 0 | 6 | 13 |  |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 0 | 0.00 | 0 | 1 | 0 | 0 | 1 |  |
| P1 | Initialisation des joueurs | 0 | 0.00 | 0 | 23 | 0 | 6 | 17 |  |
| P5 | Code par personnage (6 × 2 variantes) | 0 | 0.00 | 0 | 73 | 0 | 1 | 72 |  |
| P6 | Réactions aux coups, saisie des coups spéciaux | 0 | 0.00 | 0 | 9 | 0 | 2 | 7 |  |
| C06 | PSHL : tirs du joueur | 0 | 0.00 | 0 | 39 | 0 | 18 | 21 |  |
| C12 | PEFF : effets du joueur | 0 | 0.00 | 0 | 107 | 0 | 37 | 70 |  |
| C02 | ENEMY : ennemis | 0 | 0.00 | 0 | 205 | 0 | 9 | 196 |  |
| C04 | BOSS : boss | 0 | 0.00 | 0 | 353 | 0 | 11 | 342 |  |
| C0e | EFFCT : effets | 0 | 0.00 | 0 | 170 | 0 | 61 | 109 |  |
| C0a | BOX : caisses/coffres (h) | 0 | 0.00 | 0 | 7 | 0 | 1 | 6 |  |
| C0c | ITEM : objets ramassables | 0 | 0.00 | 0 | 14 | 0 | 3 | 11 |  |
| C14 | SOBJ : objets système/UI | 0 | 0.00 | 0 | 25 | 0 | 9 | 16 |  |
| C1 | Crédits, pièces, start/continue | 0 | 0.00 | 0 | 22 | 0 | 15 | 7 |  |
| M1 | Menu TEST (service) | 0 | 0.00 | 0 | 76 | 0 | 0 | 76 |  |
| M2 | EEPROM, configuration, comptabilité | 0 | 0.00 | 0 | 60 | 0 | 18 | 42 |  |
| M3 | « Secret Test » (debug) et Block Test | 0 | 0.00 | 0 | 29 | 0 | 0 | 29 |  |
| S1 | Son : wrappers de commandes + FIFO | 0 | 0.00 | 0 | 410 | 0 | 406 | 4 |  |
| F1 | Tâche de boot, avertissements région | 0 | 0.00 | 0 | 5 | 0 | 4 | 1 |  |
| F2 | Machine à modes (attract) | 0 | 0.00 | 0 | 0 | 0 | 0 | 0 |  |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 0 | 0.00 | 0 | 49 | 0 | 2 | 47 |  |
| F4 | Exécuteur d'écrans fixes (logos) | 0 | 0.00 | 0 | 5 | 0 | 1 | 4 |  |
| F5 | Curseur de sélection de personnage | 0 | 0.00 | 0 | 10 | 0 | 0 | 10 |  |
| F6 | Interpréteur de scripts de scène (démos) | 0 | 0.00 | 0 | 18 | 0 | 7 | 11 |  |
| F7 | Écrans titre / copyright | 0 | 0.00 | 0 | 5 | 0 | 1 | 4 |  |
| - | hors atlas | 0 | 0.00 | 0 | 0 | 0 | 0 | 0 |  |

## Hors de toute fonction de l'index (les 40 plages les plus gourmandes)

Code que Ghidra n'a pas découpé en fonctions (gestionnaires atteints par table, par exemple) : à désassembler depuis le listing. Plages contiguës à moins de 0x40 octets.

| Plage | Sous-système | Instr. 68000 | % |
|---|---|---:|---:|
| 036b26-036b86 | P4 | 207 | 8.07 |
| 002a24-002a34 | V2 | 206 | 8.03 |
| 023d0c-023d40 | P3 | 22 | 0.86 |
| 01c702-01c750 | W1 | 14 | 0.55 |
| 0a1a40-0a1a62 | C08 | 8 | 0.31 |
| 0d8f06-0d8f24 | C10 | 7 | 0.27 |
| 0f4c6e-0f4c76 | W3 | 6 | 0.23 |
| 0f565e-0f5666 | W3 | 6 | 0.23 |
| 01c808-01c808 | W1 | 1 | 0.04 |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 0d90b0 | FUN_000d90b0 | C10 | 270 | 759 | 29.60 | 1 |
| 0cf182 | FUN_000cf182 | C10 | 46 | 272 | 10.61 | 9 |
| 0a31c2 | FUN_000a31c2 | C08 | 34 | 253 | 9.87 | 1 |
| 0a6824 | FUN_000a6824 | C08 | 64 | 135 | 5.27 | 9 |
| 039e86 | FUN_00039e86 | W0 | 38 | 119 | 4.64 | 1 |
| 0a2f96 | FUN_000a2f96 | C08 | 34 | 114 | 4.45 | 1 |
| 0a44bc | FUN_000a44bc | C08 | 90 | 61 | 2.38 | 1 |
| 09ddee | FUN_0009ddee | C08 | 34 | 56 | 2.18 | 1 |
| 0d5f12 | FUN_000d5f12 | C10 | 30 | 48 | 1.87 | 8 |
| 0d9386 | FUN_000d9386 | C10 | 156 | 44 | 1.72 | 1 |
| 0a460a | FUN_000a460a | C08 | 36 | 32 | 1.25 | 3 |
| 0a5f38 | FUN_000a5f38 | C08 | 54 | 32 | 1.25 | 1 |
| 0a3354 | FUN_000a3354 | C08 | 24 | 30 | 1.17 | 1 |
| 0d98b8 | FUN_000d98b8 | C10 | 20 | 28 | 1.09 | 3 |
| 0cc1c2 | FUN_000cc1c2 | C10 | 62 | 25 | 0.98 | 1 |
| 0a4676 | FUN_000a4676 | C08 | 32 | 24 | 0.94 | 3 |
| 0a464e | FUN_000a464e | C08 | 40 | 18 | 0.70 | 2 |
| 0a1360 | caseD_0 | C08 | 30 | 12 | 0.47 | 2 |
| 0ce668 | FUN_000ce668 | C10 | 30 | 8 | 0.31 | 1 |
| 0d995a | FUN_000d995a | C10 | 22 | 7 | 0.27 | 1 |
| 03e28e | FUN_0003e28e | O3 | 1038 | 6 | 0.23 | 1 |
| 01c86e | FUN_0001c86e | W1 | 434 | 4 | 0.16 | 1 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 22 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 342 | 17444 |
| C10 | SET : décors/animations de palette | 106 | 10598 |
| C02 | ENEMY : ennemis | 196 | 9094 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 96 | 5952 |
| C0e | EFFCT : effets | 109 | 5404 |
| C12 | PEFF : effets du joueur | 70 | 5298 |
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

Hors appel : 960 instructions (37.44 %).

| Module | Instr. 68000 | % | Sites | Routines appelées (les plus gourmandes) |
|---|---:|---:|---:|---|
| families | 597 | 23.28 | 11 | a31c2 (253), a6824 (135), a44bc (61), a5f38 (32), a3354 (30), d98b8 (28) + 5 autres |
| player | 501 | 19.54 | 1 | cf182 (272), 36b26 (207), 23d0c (22) |
| scene | 254 | 9.91 | 2 | 2a24 (106), 2a30 (86), d5f12 (48), 2a2a (14) |
| families/boss_7 | 114 | 4.45 | 1 | a2f96 (114) |
| families/boss_15 | 74 | 2.89 | 3 | a460a (32), a4676 (24), a464e (18) |
| families/enemy_dbeast | 56 | 2.18 | 1 | 9ddee (56) |
| modes | 8 | 0.31 | 1 | ce668 (8) |

### Sites d'appel (les 60 plus gourmands)

| Site | Routine | Instr. 68000 |
|---|---|---:|
| player/mod.rs:245 | cf182 | 272 |
| player/mod.rs:245 | 36b26 | 207 |
| families/effects_c.rs:3889 | a31c2 | 138 |
| families/boss_5.rs:239 | a6824 | 135 |
| families/eshl_c.rs:312 | a31c2 | 115 |
| families/boss_7/attacks.rs:67 | a2f96 | 114 |
| scene/script.rs:107 | 2a24 | 106 |
| scene/script.rs:107 | 2a30 | 86 |
| families/set_b.rs:1815 | a44bc | 61 |
| families/enemy_dbeast/parts.rs:183 | 9ddee | 56 |
| scene/exit.rs:129 | d5f12 | 48 |
| families/boss_15/attacks.rs:404 | a460a | 32 |
| families/mod.rs:319 | a5f38 | 32 |
| families/effects_c.rs:3897 | a3354 | 30 |
| families/set_b.rs:650 | d98b8 | 28 |
| families/set_b.rs:1547 | cc1c2 | 25 |
| families/boss_15/attacks.rs:153 | a4676 | 24 |
| player/mod.rs:245 | 23d0c | 22 |
| families/boss_15/attacks.rs:309 | a464e | 18 |
| scene/script.rs:107 | 2a2a | 14 |
| families/boxes.rs:309 | a1360 | 12 |
| families/boxes.rs:309 | a1a40 | 8 |
| modes/ending.rs:1089 | ce668 | 8 |
| families/set_b.rs:784 | d995a | 7 |
| families/reactions.rs:155 | 3e480 | 6 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss10_scene65.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss10_scene66.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss11_scene42.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss12_scene40.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss13_dragon.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss15_scene46.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss17_scene13.txt | 12000 | 6 | 0.0 |
| scenarios/boss/boss18_scene26.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss19_scene32.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss21_scene44.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 3 | 0.0 |
| scenarios/boss/boss5_scene58.txt | 12000 | 15 | 0.0 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 15 | 0.0 |
| scenarios/boss/boss6_scene24.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss8_scene64.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 0 | 0.0 |
| scenarios/bot30k.txt | 30000 | 6 | 0.0 |
| scenarios/bot_2p.txt | 30000 | 4 | 0.0 |
| scenarios/bot_char0.txt | 30000 | 4 | 0.0 |
| scenarios/bot_char1.txt | 30000 | 23 | 0.0 |
| scenarios/bot_char2.txt | 30000 | 4 | 0.0 |
| scenarios/bot_char3.txt | 30000 | 4 | 0.0 |
| scenarios/bot_char4.txt | 30000 | 6 | 0.0 |
| scenarios/bot_char5.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_2p_a.txt | 30000 | 92 | 0.0 |
| scenarios/cov/cov_2p_b.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_2p_c.txt | 30000 | 91 | 0.0 |
| scenarios/cov/cov_3p.txt | 30000 | 18 | 0.0 |
| scenarios/cov/cov_4p_a.txt | 30000 | 16 | 0.0 |
| scenarios/cov/cov_4p_b.txt | 30000 | 15 | 0.0 |
| scenarios/cov/cov_4p_c.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 51 | 0.0 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 4 | 0.0 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 8 | 0.0 |
| scenarios/cov/cov_service.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_ecrans.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_reglages.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_tests.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_variante.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 16 | 0.0 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 52 | 0.0 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 407 | 0.0 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 10 | 0.0 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 15 | 0.0 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 15 | 0.0 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_diff0.txt | 30000 | 18 | 0.0 |
| scenarios/cov/covb_diff7.txt | 30000 | 61 | 0.0 |
| scenarios/cov/covb_langue4.txt | 30000 | 4 | 0.0 |
| scenarios/cov/covb_langue6.txt | 30000 | 51 | 0.0 |
| scenarios/cov/covb_long3p_02.txt | 30000 | 60 | 0.0 |
| scenarios/cov/covb_long3p_05.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_10.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_11.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_14.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_20.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_27.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_59.txt | 30000 | 17 | 0.0 |
| scenarios/cov/covb_long_02.txt | 30000 | 2 | 0.0 |
| scenarios/cov/covb_long_05.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_10.txt | 30000 | 17 | 0.0 |
| scenarios/cov/covb_long_11.txt | 30000 | 6 | 0.0 |
| scenarios/cov/covb_long_14.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_20.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_27.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_31.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_43.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_59.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_magie_c0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_magie_c4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_mortal_3p.txt | 30000 | 17 | 0.0 |
| scenarios/cov/covb_mortal_4p.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_rang_10.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_rang_14.txt | 30000 | 207 | 0.0 |
| scenarios/cov/covb_rang_39.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_rang_43.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_region_0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_region_10.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_region_2.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_region_4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_region_6.txt | 30000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/elf_scene92.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/elf_scene94.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/elf_scene96.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 6 | 0.0 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 6 | 0.0 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 15 | 0.0 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/trog_scene101.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/trog_scene104.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_2p.txt | 12000 | 6 | 0.0 |
| scenarios/extra/mash_c0.txt | 12000 | 6 | 0.0 |
| scenarios/extra/mash_c0b.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c1b.txt | 12000 | 6 | 0.0 |
| scenarios/extra/mash_c2.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c2b.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c3.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c3b.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c3c.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c3d.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c4.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c4b.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c4c.txt | 12000 | 4 | 0.0 |
| scenarios/extra/mash_c5.txt | 12000 | 6 | 0.0 |
| scenarios/extra/mash_c5b.txt | 12000 | 6 | 0.0 |
| scenarios/modes/attract.txt | 12000 | 0 | 0.0 |
| scenarios/modes/ending.txt | 12000 | 8 | 0.0 |
| scenarios/modes/ending_end.txt | 12000 | 16 | 0.0 |
| scenarios/modes/gameover.txt | 12000 | 6 | 0.0 |
| scenarios/modes/screens.txt | 12000 | 0 | 0.0 |
| scenarios/scene/scene_01.txt | 6500 | 4 | 0.0 |
| scenarios/scene/scene_02.txt | 6500 | 2 | 0.0 |
| scenarios/scene/scene_03.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_04.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_05.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_07.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_08.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_09.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_10.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_105.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_109.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_11.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_110.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_111.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_14.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_24.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_25.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_26.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_27.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_28.txt | 6500 | 25 | 0.0 |
| scenarios/scene/scene_32.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_33.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_34.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_35.txt | 6500 | 14 | 0.0 |
| scenarios/scene/scene_38.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_39.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_40.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_41.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_42.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_44.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_47.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_54.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_58.txt | 6500 | 15 | 0.0 |
| scenarios/scene/scene_59.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_64.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_65.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_66.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_67.txt | 6500 | 948 | 0.1 |
| scenarios/scene/scene_90.txt | 6500 | 0 | 0.0 |
| scenarios/scene_end/end_01.txt | 9000 | 4 | 0.0 |
| scenarios/scene_end/end_02.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_03.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_04.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_05.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_06.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_07.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_09.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_10.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_105.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_11.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_13.txt | 9000 | 6 | 0.0 |
| scenarios/scene_end/end_14.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_20.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_24.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_26.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_27.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_31.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_32.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_33.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_34.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_35.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_38.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_40.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_41.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_42.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_44.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_46.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_54.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_58.txt | 9000 | 15 | 0.0 |
| scenarios/scene_end/end_59.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_64.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_65.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_66.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_67.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/end_68.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_02.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_04.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_05.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_10.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_11.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_11_tard.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_13.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_13_tard.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_14.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_20.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_32.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_33.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_34.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_35.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_38.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_40.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_42.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_44.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_46.txt | 9000 | 0 | 0.0 |
| scenarios/scene_end/trans_67.txt | 9000 | 0 | 0.0 |
