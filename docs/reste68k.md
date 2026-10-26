# Code 68000 restant

Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.

Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue 267 scénarios, soit 4315500 images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.

* **58061 instructions 68000 exécutées** (hors attente active), soit 0.0 par image ;
* attente active, non comptée : 0 instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;
* 2559 fonctions dans `decomp/index.tsv` : 17 exécutent encore du 68000, 972 sont entièrement natives, **1570 jamais atteintes** (107182 octets : code mort, ou à exercer par de nouveaux scénarios) ;
* instructions hors de toute fonction de l'index : 43016 (comptées par sous-système ci-dessous, colonne « Hors index », et par plage plus bas).

## Par sous-système (atlas)

| ID | Sous-système | Instr. 68000 | % | Hors index | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| C0e | EFFCT : effets | 39471 | 67.98 | 27552 | 170 | 3 | 73 | 94 | aabf4 (59.2 %), aacfe (27.6 %), aa9c0 (13.2 %) |
| O3 | Aides communes aux types (animations par type) | 8940 | 15.40 | 6972 | 25 | 3 | 9 | 13 | 3be02 (97.2 %), 3de2e (2.2 %), 3e27e (0.6 %) |
| P4 | Noyau joueur (états, 26 actions génériques) | 8504 | 14.65 | 8492 | 59 | 1 | 8 | 50 | 3b8f6 (100.0 %) |
| C06 | PSHL : tirs du joueur | 480 | 0.83 | 0 | 39 | 3 | 19 | 17 | 53918 (66.2 %), 53954 (25.0 %), 539e4 (8.8 %) |
| M1 | Menu TEST (service) | 346 | 0.60 | 0 | 76 | 1 | 0 | 75 | 2b1d0 (100.0 %) |
| C12 | PEFF : effets du joueur | 246 | 0.42 | 0 | 107 | 3 | 40 | 64 | 4d3c6 (51.2 %), 4cd8e (31.7 %), 4ccac (17.1 %) |
| S1 | Son : wrappers de commandes + FIFO | 38 | 0.07 | 0 | 410 | 2 | 404 | 4 | e285c (68.4 %), e2a74 (31.6 %) |
| C10 | SET : décors/animations de palette | 36 | 0.06 | 0 | 131 | 1 | 21 | 109 | d5ef8 (100.0 %) |
| K1 | Noyau : reset, boot, VBLANK, ordonnanceur, traps, entrées, gel | 0 | 0.00 | 0 | 17 | 0 | 14 | 3 |  |
| K2 | Auto-test RAM (« RAM ERROR !! ») | 0 | 0.00 | 0 | 2 | 0 | 0 | 2 |  |
| K3 | Entrées vers enregistrements joueur | 0 | 0.00 | 0 | 3 | 0 | 3 | 0 |  |
| K4 | Exceptions et écran de dump debug | 0 | 0.00 | 0 | 16 | 0 | 0 | 16 |  |
| V1 | Primitives vidéo (attente, couches, effacements) | 0 | 0.00 | 0 | 13 | 0 | 13 | 0 |  |
| V2 | Texte, rectangles et blocs de tuiles | 0 | 0.00 | 0 | 33 | 0 | 28 | 5 |  |
| V3 | Palettes et fondus | 0 | 0.00 | 0 | 38 | 0 | 31 | 7 |  |
| V4 | Construction de la liste de sprites (OBJ RAM) | 0 | 0.00 | 0 | 15 | 0 | 2 | 13 |  |
| B1 | Scrolling / caméras par scène | 0 | 0.00 | 0 | 42 | 0 | 12 | 30 |  |
| O1 | Animation, tables de vitesse, physique, culling | 0 | 0.00 | 0 | 22 | 0 | 22 | 0 |  |
| O2 | Pools d'objets et listes triées | 0 | 0.00 | 0 | 41 | 0 | 28 | 13 |  |
| U1 | RNG et files d'affichage (pop-ups) | 0 | 0.00 | 0 | 18 | 0 | 14 | 4 |  |
| U2 | Petites routines / tables (rôle inconnu) | 0 | 0.00 | 0 | 17 | 0 | 17 | 0 |  |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 0 | 0.00 | 0 | 69 | 0 | 60 | 9 |  |
| X1 | Collisions (boîtes, coups, poussée) | 0 | 0.00 | 0 | 39 | 0 | 0 | 39 |  |
| X2 | Dégâts, terrain, placement | 0 | 0.00 | 0 | 19 | 0 | 6 | 13 |  |
| W0 | Dispatch des 10 classes d'objets | 0 | 0.00 | 0 | 10 | 0 | 10 | 0 |  |
| W1 | Boucle de partie et phases 0/4/12/20 | 0 | 0.00 | 0 | 3 | 0 | 2 | 1 |  |
| W2 | Sous-scènes de transition à fondus (phases 8/24) | 0 | 0.00 | 0 | 1 | 0 | 0 | 1 |  |
| W3 | Scripts de scène (phases 16/28) | 0 | 0.00 | 0 | 82 | 0 | 11 | 71 |  |
| P1 | Initialisation des joueurs | 0 | 0.00 | 0 | 23 | 0 | 6 | 17 |  |
| P3 | Comportement scénarisé des joueurs par scène | 0 | 0.00 | 0 | 3 | 0 | 2 | 1 |  |
| P5 | Code par personnage (6 × 2 variantes) | 0 | 0.00 | 0 | 73 | 0 | 1 | 72 |  |
| P6 | Réactions aux coups, saisie des coups spéciaux | 0 | 0.00 | 0 | 9 | 0 | 2 | 7 |  |
| C02 | ENEMY : ennemis | 0 | 0.00 | 0 | 205 | 0 | 9 | 196 |  |
| C04 | BOSS : boss | 0 | 0.00 | 0 | 353 | 0 | 11 | 342 |  |
| C08 | ESHL : tirs/attaques ennemis | 0 | 0.00 | 0 | 127 | 0 | 33 | 94 |  |
| C0a | BOX : caisses/coffres (h) | 0 | 0.00 | 0 | 7 | 0 | 1 | 6 |  |
| C0c | ITEM : objets ramassables | 0 | 0.00 | 0 | 14 | 0 | 3 | 11 |  |
| C14 | SOBJ : objets système/UI | 0 | 0.00 | 0 | 25 | 0 | 9 | 16 |  |
| C1 | Crédits, pièces, start/continue | 0 | 0.00 | 0 | 22 | 0 | 15 | 7 |  |
| M2 | EEPROM, configuration, comptabilité | 0 | 0.00 | 0 | 60 | 0 | 18 | 42 |  |
| M3 | « Secret Test » (debug) et Block Test | 0 | 0.00 | 0 | 29 | 0 | 0 | 29 |  |
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
| 0aaac0-0aab0a | C0e | 11760 | 20.25 |
| 0aabac-0aabf0 | C0e | 10410 | 17.93 |
| 03b684-03b728 | P4 | 5721 | 9.85 |
| 0aac88-0aacf4 | C0e | 5382 | 9.27 |
| 03cfea-03cff4 | O3 | 3052 | 5.26 |
| 03dd9c-03ddf4 | O3 | 1965 | 3.38 |
| 03be52-03bf54 | O3 | 1905 | 3.28 |
| 036be0-036c10 | P4 | 1809 | 3.12 |
| 03a1a6-03a1a6 | P4 | 962 | 1.66 |
| 03e0dc-03e12e | O3 | 35 | 0.06 |
| 03df34-03df56 | O3 | 15 | 0.03 |

## Par fonction (les 80 plus gourmandes)

| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |
|---|---|---|---:|---:|---:|---:|
| 0aabf4 | FUN_000aabf4 | C0e | 122 | 7056 | 12.15 | 35 |
| 0aacfe | FUN_000aacfe | C0e | 20 | 3295 | 5.68 | 35 |
| 03be02 | FUN_0003be02 | O3 | 54 | 1912 | 3.29 | 10 |
| 0aa9c0 | FUN_000aa9c0 | C0e | 68 | 1568 | 2.70 | 35 |
| 02b1d0 | FUN_0002b1d0 | M1 | 70 | 346 | 0.60 | 1 |
| 053918 | FUN_00053918 | C06 | 54 | 318 | 0.55 | 6 |
| 04d3c6 | FUN_0004d3c6 | C12 | 28 | 126 | 0.22 | 6 |
| 053954 | FUN_00053954 | C06 | 62 | 120 | 0.21 | 6 |
| 04cd8e | FUN_0004cd8e | C12 | 66 | 78 | 0.13 | 6 |
| 03de2e | FUN_0003de2e | O3 | 50 | 44 | 0.08 | 4 |
| 04ccac | FUN_0004ccac | C12 | 30 | 42 | 0.07 | 6 |
| 0539e4 | FUN_000539e4 | C06 | 36 | 42 | 0.07 | 6 |
| 0d5ef8 | FUN_000d5ef8 | C10 | 26 | 36 | 0.06 | 6 |
| 0e285c | FUN_000e285c | S1 | 136 | 26 | 0.04 | 11 |
| 03b8f6 | FUN_0003b8f6 | P4 | 38 | 12 | 0.02 | 2 |
| 03e27e | FUN_0003e27e | O3 | 12 | 12 | 0.02 | 3 |
| 0e2a74 | FUN_000e2a74 | S1 | 70 | 12 | 0.02 | 6 |

Concentration : 0 fonctions pour 90 % des instructions restantes, 0 pour 99 %, 17 pour toutes les fonctions touchées.

## Fonctions jamais atteintes, par sous-système

| ID | Sous-système | Fonctions | Octets |
|---|---|---:|---:|
| C04 | BOSS : boss | 342 | 17444 |
| C10 | SET : décors/animations de palette | 109 | 11106 |
| C02 | ENEMY : ennemis | 196 | 9094 |
| P5 | Code par personnage (6 × 2 variantes) | 72 | 7308 |
| X1 | Collisions (boîtes, coups, poussée) | 39 | 6296 |
| C08 | ESHL : tirs/attaques ennemis | 94 | 5894 |
| C12 | PEFF : effets du joueur | 64 | 5042 |
| C0e | EFFCT : effets | 94 | 4696 |
| F3 | Interface joueurs : sélection / panneaux d'état (h) | 47 | 4386 |
| M3 | « Secret Test » (debug) et Block Test | 29 | 3992 |
| P4 | Noyau joueur (états, 26 actions génériques) | 50 | 3748 |
| M1 | Menu TEST (service) | 75 | 3200 |
| B1 | Scrolling / caméras par scène | 30 | 2446 |
| W3 | Scripts de scène (phases 16/28) | 71 | 2396 |
| V4 | Construction de la liste de sprites (OBJ RAM) | 13 | 2198 |
| M2 | EEPROM, configuration, comptabilité | 42 | 2152 |
| P6 | Réactions aux coups, saisie des coups spéciaux | 7 | 1564 |
| F4 | Exécuteur d'écrans fixes (logos) | 4 | 1502 |
| C06 | PSHL : tirs du joueur | 17 | 1442 |
| C0c | ITEM : objets ramassables | 11 | 1434 |
| K4 | Exceptions et écran de dump debug | 16 | 1366 |
| C1 | Crédits, pièces, start/continue | 7 | 1090 |
| X2 | Dégâts, terrain, placement | 13 | 964 |
| F5 | Curseur de sélection de personnage | 10 | 744 |
| C14 | SOBJ : objets système/UI | 16 | 736 |
| C0a | BOX : caisses/coffres (h) | 6 | 682 |
| P1 | Initialisation des joueurs | 17 | 626 |
| F6 | Interpréteur de scripts de scène (démos) | 11 | 526 |
| O3 | Aides communes aux types (animations par type) | 13 | 480 |
| V3 | Palettes et fondus | 7 | 472 |
| E1 | Aides d'IA (cible, portées, décisions aléatoires) | 9 | 446 |
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
| W1 | Boucle de partie et phases 0/4/12/20 | 1 | 8 |

## Appels Rust qui exécutent encore du 68000

Instructions 68000 exécutées pendant un appel `call`/`call_at` (ou une de leurs enveloppes : `Ctx::call`, `Cx::call`, `call_d0`...), attribuées à l'appel le plus intérieur. **Hors appel** : code d'origine où une tâche reprend (adresse de reprise sans native), où une native continue par `tail`/`jump`, ou appelé par du 68000 lui-même hors de tout appel Rust. C'est la liste de ce qui empêche encore la boucle d'image sans 68000 (`docs/architecture.md`, point 4).

Hors appel : 667 instructions (1.15 %).

| Module | Instr. 68000 | % | Sites | Routines appelées (les plus gourmandes) |
|---|---:|---:|---:|---|
| dispatch | 38687 | 66.63 | 1 | aaac0 (38687) |
| player | 9614 | 16.56 | 1 | 3b700 (6399), 36be0 (1809), 2b1da (346), 53918 (318), 3b6f4 (240), 4d3c6 (126) + 7 autres |
| families | 8940 | 15.40 | 1 | 3cfea (8940) |
| modes | 117 | 0.20 | 1 | 1fa38 (117) |
| families/boss_0 | 18 | 0.03 | 1 | d5ef8 (18) |
| families/boss_22 | 18 | 0.03 | 1 | d5ef8 (18) |

### Sites d'appel (les 60 plus gourmands)

| Site | Routine | Instr. 68000 |
|---|---|---:|
| dispatch.rs:109 | aaac0 | 38687 |
| families/enemy_common.rs:278 | 3cfea | 8940 |
| player/mod.rs:245 | 3b700 | 6399 |
| player/mod.rs:245 | 36be0 | 1809 |
| player/mod.rs:245 | 2b1da | 346 |
| player/mod.rs:245 | 53918 | 318 |
| player/mod.rs:245 | 3b6f4 | 240 |
| player/mod.rs:245 | 4d3c6 | 126 |
| modes/demo.rs:349 | 1fa38 | 117 |
| player/mod.rs:245 | 539e4 | 84 |
| player/mod.rs:245 | 4cd8e | 78 |
| player/mod.rs:245 | 53954 | 78 |
| player/mod.rs:245 | 3b684 | 56 |
| player/mod.rs:245 | 4ccac | 42 |
| player/mod.rs:245 | e285c | 26 |
| families/boss_0/moves.rs:260 | d5ef8 | 18 |
| families/boss_22/mod.rs:429 | d5ef8 | 18 |
| player/mod.rs:245 | e2a74 | 12 |

## Scénarios

| Scénario | Images | Instr. 68000 | par image |
|---|---:|---:|---:|
| scenarios/boss/boss0_poke.txt | 12000 | 398 | 0.0 |
| scenarios/boss/boss0_poke_mort.txt | 12000 | 6 | 0.0 |
| scenarios/boss/boss10_scene65.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss10_scene66.txt | 12000 | 2278 | 0.2 |
| scenarios/boss/boss11_scene42.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss11_scene42_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss12_scene40.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss12_scene40_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss13_dragon.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss15_scene46.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss15_scene46_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss16_poke.txt | 12000 | 4909 | 0.4 |
| scenarios/boss/boss16_poke_mort.txt | 12000 | 1786 | 0.1 |
| scenarios/boss/boss17_scene13.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss18_scene26.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss18_scene26_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss19_scene32.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss19_scene32_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss21_scene44.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss21_scene44_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss2_poke.txt | 12000 | 241 | 0.0 |
| scenarios/boss/boss2_poke3p.txt | 12000 | 6 | 0.0 |
| scenarios/boss/boss2_poke3p_mort.txt | 12000 | 6 | 0.0 |
| scenarios/boss/boss5_scene58.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss5_scene58_mort.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss6_scene24.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss8_scene64.txt | 12000 | 0 | 0.0 |
| scenarios/boss/boss8_scene64_mort.txt | 12000 | 0 | 0.0 |
| scenarios/bot30k.txt | 30000 | 0 | 0.0 |
| scenarios/bot_2p.txt | 30000 | 0 | 0.0 |
| scenarios/bot_char0.txt | 30000 | 0 | 0.0 |
| scenarios/bot_char1.txt | 30000 | 0 | 0.0 |
| scenarios/bot_char2.txt | 30000 | 1142 | 0.0 |
| scenarios/bot_char3.txt | 30000 | 0 | 0.0 |
| scenarios/bot_char4.txt | 30000 | 0 | 0.0 |
| scenarios/bot_char5.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_2p_a.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_2p_b.txt | 30000 | 2438 | 0.1 |
| scenarios/cov/cov_2p_c.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_3p.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_4p_a.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_4p_b.txt | 30000 | 155 | 0.0 |
| scenarios/cov/cov_4p_c.txt | 30000 | 302 | 0.0 |
| scenarios/cov/cov_attract_coin.txt | 30000 | 1183 | 0.0 |
| scenarios/cov/cov_credits_2pieces.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_gratuit.txt | 30000 | 788 | 0.0 |
| scenarios/cov/cov_credits_gros.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_gros4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_gros_commun.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_sep2.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_sep4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_sep4_p1.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_credits_sep4_p2.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_effct_achat.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_effct_rares.txt | 30000 | 788 | 0.0 |
| scenarios/cov/cov_effct_vente.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_gameover_c4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_mortal_2p.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_mortal_c1.txt | 30000 | 414 | 0.0 |
| scenarios/cov/cov_mortal_late.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_natural_c0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_porte_annexe.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_ecrans.txt | 30000 | 1183 | 0.0 |
| scenarios/cov/cov_service_reglages.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_tests.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_service_variante.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour1_c0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour2_c1.txt | 30000 | 207 | 0.0 |
| scenarios/cov/cov_tour3_c2.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour4_c3.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour5_c4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour6_c5.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour7_c0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/cov_tour8_c1.txt | 30000 | 3472 | 0.1 |
| scenarios/cov/covb_diff0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_diff7.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_langue4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_langue6.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_02.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_05.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_10.txt | 30000 | 2474 | 0.1 |
| scenarios/cov/covb_long3p_11.txt | 30000 | 1035 | 0.0 |
| scenarios/cov/covb_long3p_14.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long3p_20.txt | 30000 | 1691 | 0.1 |
| scenarios/cov/covb_long3p_27.txt | 30000 | 3364 | 0.1 |
| scenarios/cov/covb_long3p_59.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_02.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_05.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_10.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_11.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_14.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_20.txt | 30000 | 876 | 0.0 |
| scenarios/cov/covb_long_27.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_31.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_43.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_long_59.txt | 30000 | 2489 | 0.1 |
| scenarios/cov/covb_magie_c0.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_magie_c4.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_mortal_3p.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_mortal_4p.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_rang_10.txt | 30000 | 754 | 0.0 |
| scenarios/cov/covb_rang_14.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_rang_39.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_rang_43.txt | 30000 | 0 | 0.0 |
| scenarios/cov/covb_region_0.txt | 30000 | 964 | 0.0 |
| scenarios/cov/covb_region_10.txt | 30000 | 395 | 0.0 |
| scenarios/cov/covb_region_2.txt | 30000 | 741 | 0.0 |
| scenarios/cov/covb_region_4.txt | 30000 | 395 | 0.0 |
| scenarios/cov/covb_region_6.txt | 30000 | 395 | 0.0 |
| scenarios/cov/covs_carte_dev.txt | 30000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene112.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene16.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene18.txt | 12000 | 1115 | 0.1 |
| scenarios/enemy/beetle_scene43.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/beetle_scene61.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/elf_scene92.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/elf_scene94.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/elf_scene96.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/gargoyle_scene42.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/gargoyle_scene55.txt | 12000 | 3520 | 0.3 |
| scenarios/enemy/gargoyle_scene63.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene19.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene26.txt | 12000 | 2181 | 0.2 |
| scenarios/enemy/ghoul_scene95.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ghoul_scene97.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene21.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene40.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene49.txt | 12000 | 1287 | 0.1 |
| scenarios/enemy/hellhound_scene50.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene55.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/hellhound_scene62.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene17.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene25.txt | 12000 | 1589 | 0.1 |
| scenarios/enemy/kobold_scene29.txt | 12000 | 2863 | 0.2 |
| scenarios/enemy/kobold_scene50.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene52.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene7.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/kobold_scene99.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ogre_scene65.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/ogre_scene66.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene36.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene37.txt | 12000 | 52 | 0.0 |
| scenarios/enemy/scorpion_scene38.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene39.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/scorpion_scene53.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/skeleton_scene13.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/skeleton_scene58.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/skeleton_scene64.txt | 12000 | 1604 | 0.1 |
| scenarios/enemy/trog_scene101.txt | 12000 | 292 | 0.0 |
| scenarios/enemy/trog_scene101b.txt | 12000 | 0 | 0.0 |
| scenarios/enemy/trog_scene104.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_2p.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c0.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c0b.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c1b.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c2.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c2b.txt | 12000 | 250 | 0.0 |
| scenarios/extra/mash_c3.txt | 12000 | 1509 | 0.1 |
| scenarios/extra/mash_c3b.txt | 12000 | 1474 | 0.1 |
| scenarios/extra/mash_c3c.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c3d.txt | 12000 | 1474 | 0.1 |
| scenarios/extra/mash_c4.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c4b.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c4c.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c5.txt | 12000 | 0 | 0.0 |
| scenarios/extra/mash_c5b.txt | 12000 | 0 | 0.0 |
| scenarios/modes/attract.txt | 12000 | 788 | 0.1 |
| scenarios/modes/ending.txt | 12000 | 0 | 0.0 |
| scenarios/modes/ending_end.txt | 12000 | 0 | 0.0 |
| scenarios/modes/gameover.txt | 12000 | 0 | 0.0 |
| scenarios/modes/screens.txt | 12000 | 788 | 0.1 |
| scenarios/scene/scene_01.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_02.txt | 6500 | 0 | 0.0 |
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
| scenarios/scene/scene_28.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_32.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_33.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_34.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_35.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_38.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_39.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_40.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_41.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_42.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_44.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_47.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_54.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_58.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_59.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_64.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_65.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_66.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_67.txt | 6500 | 0 | 0.0 |
| scenarios/scene/scene_90.txt | 6500 | 0 | 0.0 |
| scenarios/scene_end/end_01.txt | 9000 | 0 | 0.0 |
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
| scenarios/scene_end/end_13.txt | 9000 | 0 | 0.0 |
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
| scenarios/scene_end/end_58.txt | 9000 | 0 | 0.0 |
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
