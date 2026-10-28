# Couverture des scénarios

Objectif : que les scénarios rejoués par `validate` (routines natives contre code d'origine) exécutent
l'ensemble du jeu, pas seulement le début du stage 1. Aucune donnée de ROM n'est versionnée : les
scénarios ne contiennent que des entrées et des écritures RAM (`poke`, voir `docs/rewrite-guide.md`).

## Méthode d'accès aux stages

* `FF8034` = numéro de scène (`A5+0x34`), 1 à 114 (0x72) :
  1-68 les stages (dont les boss : 24 Manticore, 26, 32 Black Dragon, 40/42 salamandres, 44, 46, 54 Tel'Arin,
  58, 64, 66-67…) et leurs scènes intermédiaires (choix de route : 14 et 38 ; scènes à décor propre 43, 47, 4b, 4d, 4f,
  50, 52 atteintes par progression naturelle), 69-82 boutiques, 83-89 écrans à graphismes non initialisés quand on
  y entre directement, 90-114 scènes de démonstration de l'attract (jouées par l'IA du mode démo).
  Un `poke8 ff8034 N` pendant la sélection du personnage (image 2400) lance la partie dans la scène N ;
  vérifié par captures d'écran pour les 114 scènes (aucun plantage, le jeu se déroule normalement).
* Saut en cours de partie : `ff8034 ← N`, `ff802c ← 8`, `ff802d ← 0`, `ff802e ← 0` (entrée de scène du jeu). Passer
  par la phase 0x10 (fin de scène) est plus fidèle mais peut attendre indéfiniment (précondition de scène).
* Boss : `+0x62` (PV) de l'objet BOSS (`FFBA1E + 0xC0 k`) ramené à 1 : le bot l'achève d'une touche, ce qui exécute
  la mort du boss et la séquence de fin de scène. Joueur invincible : `+0x62` du joueur (`FF85DE + 0x200 k`) remis à 0x70.
* Joueurs 3 et 4 : l'EEPROM d'origine déclare 2 monnayeurs (`FF8021 = 1`) ; `poke8 ff8021 3` à l'image 800 donne un
  monnayeur par joueur (Coin 1-4). Le bot lance la partie avec Start 1 (image 1100) puis les autres joueurs rejoignent.
* Boutons : Button 1 attaque, 2 saute, 3 magie / objet, 4 choix de l'objet ; Service Mode = menu TEST.

## Les scénarios (`tools/gen_scenarios.py` → `scenarios/cov/`, bot réactif `examples/autobot.rs`)

Le bot lit la RAM (joueurs, ENEMY, BOSS, ITEM, BOX) pour rejoindre l'ennemi le plus proche, s'aligner, frapper,
sauter, lancer magie et objets, ramasser objets et caisses, puis avance ; l'enregistrement est un scénario d'entrées
pur, reproduit à l'identique par `validate` (le bot tourne comme la machine B : ×16, code d'origine).
Il ne touche à aucune touche pendant l'entrée de scène (phase 8) ni la fin du compte à rebours de sélection.

| Scénario | Contenu |
|---|---|
| `cov_tour1_c0` … `cov_tour8_c1` | parcours des scènes 1-28, 29-56, 57-86, 87-114 (350 images par scène, boss achevés) ; les 6 personnages : fighter (1, 7), cleric (2, 8), elf (3), dwarf (4), magic-user (5), thief (6) |
| `cov_2p_a/b/c` | 2 joueurs (fighter+dwarf, magic-user+cleric…) sur les scènes 29-56, 1-28, 87-114 |
| `cov_3p`, `cov_4p_a/b/c` | 3 et 4 joueurs, sur chacun des tronçons |
| `cov_natural_c0` | stage 1 en jeu naturel (aucun saut), joueur invincible : combats, objets, caisses, fin de scène naturelle |
| `cov_mortal_c1`, `cov_mortal_late`, `cov_mortal_2p` | sans invincibilité : mort, écran continue (Coin + Start), reprise, sélection du personnage après continue |
| `cov_gameover_c4` | mort sans pièce : game over, retour à l'attract (qui enchaîne ensuite les scènes de démonstration 0x5a-0x72) |
| `cov_attract_coin` | attract seul 20 000 images (titre, classement, démos), puis une partie sans bot |
| `cov_service` | menu TEST (Service Mode) parcouru au hasard (seul l'écran d'entrée est atteint : 163 fonctions) |

Fin du jeu : les séquences de fin de stage et de boss sont exécutées (scènes naturelles 0x43-0x52), mais la
séquence finale (générique de fin, saisie du nom) n'a pas été identifiée avec certitude : zone non couverte.

## Couverture chiffrée (instructions 68000 distinctes, interpréteur pur, `examples/cov`)

Mesure : `tools/run_cov.py` puis `tools/cov_report.py` (dénominateur : lignes d'instruction de `listing.txt` sous
0x100000, approximatif car il contient des données mal désassemblées ; 2 559 fonctions de `decomp/index.tsv`,
fonction touchée = au moins une de ses instructions exécutée).

| Jeu de scénarios | Instructions distinctes | Fonctions touchées | % du listing |
|---|---|---|---|
| bot initial (bot30k, bot_2p, bot_char0-5) | 22 754 | 691 / 2 559 | 14,0 % |
| scénarios `cov_*` seuls | 89 659 | 1 771 / 2 559 | 54,2 % |
| union (initial + `cov_*`) | 90 017 | 1 773 / 2 559 | 54,4 % |
| **272 scénarios de la validation, 2026-10-27** (`main` b30905d) | 121 609 | 2 190 / 2 559 | – |

Les 369 fonctions jamais exécutées, par sous-système (débogage et menus de test 95, wrappers de sons 63, jeu ~200 :
boss, effets et tirs du joueur, ennemis et leurs tirs, code par personnage, décors) : [couverture-fonctions.md](couverture-fonctions.md).

Par scénario (instructions) : tours 27 700-35 600, parties à 2-4 joueurs 30 800-38 700 (maximum : `cov_4p_a`), attract
33 000, natural 22 600, mortal 13 900-23 000, service 2 900. Le bot initial plafonne à 15 000-20 500 par scénario.

## Zones non atteintes

* Blocs de 4 Ko sans aucune instruction exécutée : 0x0d000, 0x12000, 0x1b000, 0x2a000 (écran d'exception, tests M1/M3),
  0x68000, 0x6b000-0x6e000, 0x78000-0x80000, 0x8b000, 0x94000, 0x9b000-0x9c000, 0xa9000, 0xbb000-0xc8000 (en grande
  partie des données mal désassemblées), 0xd1000-0xd3000, 0xef000-0xf0000.
* Menu TEST (0x29162) et « Secret Test » (M3, 0xee16e-0xf1500) : la navigation aléatoire de `cov_service` n'a pas dépassé
  l'écran d'entrée ; il faudrait connaître la séquence d'entrées de chaque écran.
* Génération des vagues d'ennemis, boss et objets rares : couverts seulement dans la mesure où le bot les rencontre
  (350 images par scène) ; pas de joueur 3/4 sur les 90-114, pas de séquence finale, pas de saisie du nom au classement.
* 788 fonctions de `index.tsv` jamais touchées (liste produite par `cov_report.py --missed`), dont beaucoup de
  handlers de tables de sauts de types d'objets rares et du code de diagnostic.

## Validation (`DDSOM_COV=1 tools/validate_all.sh`) et écarts relevés

Les scénarios `scenarios/cov/` sont longs (une heure et plus sous charge à 2-4 joueurs) : `validate_all.sh` ne les
lance que si `DDSOM_COV=1` (images : `COV_FRAMES`, 30 000 par défaut). Les 8 scénarios historiques, les `extra/` et
les `boss/` restent VALIDES. Résultat des scénarios de couverture après la campagne « registres hérités » (première
différence de la RAM entre routines natives et code d'origine, image) :

| Scénario | Résultat | Cause identifiée |
|---|---|---|
| `cov_mortal_c1`, `cov_mortal_2p`, `cov_natural_c0`, `cov_service` | VALIDE sur 30 000 images | – |
| `cov_tour1/4/5/8`, `cov_mortal_c1` | VALIDES sur 30 000 images avec `DDSOM_ADOPT_REGS=player_overlays_update,players_update` | `player_overlays_update` : D3 (et les autres registres) relu par le créateur d'effet 0xb141a (écarts aux images 5150, 4898, 4949, 10051, 10418, 3993, 20977 sans l'adoption : mots FFD5xx-FFD9xx des objets ESHL) |
| `cov_2p_c`, `cov_3p`, `cov_attract_coin`, `cov_gameover_c4` (image 29927), `cov_2p_b`, `cov_4p_*` | écarts aux images 4192, 4326, 3362/4014 | `players_update` (DDSOM_SANS le supprime ; l'adoption des registres ne suffit pas : écarts de mémoire) |
| `cov_tour2/3/6/7`, `cov_mortal_late`, `boss/boss13_dragon` (image 9991) | images 20611, 10523, 7201, 10921, 4267 | phase de jeu qui tourne sans attendre l'image (`0xa86`, cède sans attendre) : l'original surcadencé en exécute des dizaines par image (FF2A58/FF2A38, compteur de l'image FF8054) ; le nombre d'itérations dépend de la durée exacte de chaque routine, que les natives n'imitent pas (`build_sprite_list` coûte 30 000 cycles de moins que l'original sur 3 600 appels, etc.) |
| `cov_mortal_c1`, `cov_mortal_2p` | VALIDE sur 30 000 images | – |
| `cov_attract_coin` | image 21302 (3362 et 4014 corrigés) | 21302 : tempête de pas (voir plus bas), un bouton pressé à l'image 21300 |
| `cov_3p`, `cov_4p_a`, `cov_4p_b`, `cov_2p_c` | images 9850, 4739, 4444, 6394 (2992, 3845, 3152, 3011 corrigés) | `combat_update` (supprimé par `DDSOM_SANS=combat`) ; ensuite, `cov_3p` 10652 : tempête de pas ; `cov_4p_a` 7305 : registres résiduels (`build_sprite_list`, `set_0`) ; `cov_4p_b` 12015 : `set_30` ; `cov_2p_c` 8370 : `enemy_16` (Riyakaa, ffad64), puis 15960 |
| `cov_tour4_c3`, `cov_2p_b` | images 3713, 4754 | `combat_update` (supprime l'écart) |
| `cov_natural_c0` | image 18946 (stage 1) ; 6661 avec d'autres entrées | `combat_update` (mots FF1A6A, FF8648 : drapeaux de touche du joueur 1) |
| `cov_tour1/2/3/5/6/7/8`, `cov_4p_c`, `cov_gameover_c4`, `cov_mortal_late` | images 7063, 19595, 9761, 7109, 4898, 10418, 4949, 4834, 3993, 4266 | écarts de plusieurs routines à la fois (aucune désactivation isolée ne suffit) ou à identifier |
| `cov_2p_a` | n'a pas fini en plus d'une heure | – |
| `cov_service` | écart image 1443 | non analysé (menu TEST) |

Registres hérités corrigés par cette campagne (leurs lecteurs étaient des gestionnaires 68000) : `set_0` (D0-D6,
A1-A3, A6 : D4 relu par 0xad7b0, qui additionne D4 à la position des joueurs quand `A5+0x51e` = 4),
`build_sprite_list` (D4), caméras 1 et 2 (D4 = 0x3f/0x40, 0x1f/0xe0), chargeurs de palettes de scène (0x1a296,
0x1a350-0x1a36c, 0x1a3d4, 0x1a608, 0x1a678, 0x1a8ac : D0-D6, A1-A4, A6, D7 ; relus par le menu TEST en 0x2bba0), et
des écarts de mémoire révélés au passage : `set_30` (décompte de 0x80 images), adresse 32 bits des caisses saisies
(`move.l A1,(0x86,A0)` écrit A1 étendu du signe), `eshl_52` (A1 = dernier élément créé, puis A1 = fin de la boucle des
joueurs).

Outils : `DDSOM_ADOPT_REGS=préfixe,...` (registres de l'original), `DDSOM_ADOPT_MASK=hex` (seuls les registres du
masque : bit i = Di, bit 8+i = Ai, bit 16 = drapeaux), `DDSOM_ADOPT_TRACE=1` (valeurs rendues par la native et par
l'original). Dichotomie : adopter tout, puis le masque registre par registre, puis les natives par groupes.
Corrigés dans le noyau des joueurs : (1) les gestionnaires d'origine appelés par table (`actions`, `characters::run`,
`update`, `hurt`, `scripted`) comptent sur D0 = indice d'aiguillage à octet haut nul (ils font `move.b (7,A0),D0b` puis
indexent par le mot) : un D0 quelconque faisait sauter le saut du nain (0x445ba) en 0x4c1eb et écrire dans
l'enregistrement du joueur 2 ; (2) `player_overlays_update` (0x392e4) et la jauge de chargement (0xb46) laissent
maintenant les registres de l'original (D0-D6, A0-A4 / D2.w, A0) : la boucle de partie enchaîne ces routines et les
boucles de dispatch d'objets sans rien réinitialiser, et des gestionnaires lisent ces registres (un effet recopiait D3
ou D2). **Règle** : une native appelée avant une boucle de dispatch doit reproduire les registres que l'original
laisse (`DDSOM_CHECK=1 DDSOM_CHECK_ALL=1`, lignes `REGS`), pas seulement la mémoire. Diagnostic : `DDSOM_CHECK=1
DDSOM_ADOPT_REGS=préfixe,...` (avec `DDSOM_SANS` pour les dispatch et `game_phase12`, que le mode contrôle ne sait pas
rejouer) fait adopter par ces natives les registres de l'original ; si l'écart disparaît, la cause est un registre
résiduel (`cov_4p_a` : `build_sprite_list` et `set_0`).

Deux remarques sur `validate` : (1) la pile des tâches s'étend sous FF0A00 (mots FF09B0-FF09D2 vus en attract) : la zone
ignorée commence à FF09B0 ; FF0058 (dernière tâche lancée par l'ordonnanceur, écrit en 0x96c) est ignoré ; (2) une touche
pressée pendant l'entrée de scène (phase 8) ou à la fin du compte à rebours de sélection (image où la phase 4 passe à 8)
fait diverger les deux machines avec n'importe quelle routine native active (cause : même sensibilité à la durée que
ci-dessus). Le bot évite ces instants ; les scénarios existants n'y touchent pas.
Routines fautives restantes (autres modules, à corriger par leurs auteurs) : `combat_update` d'abord ; puis
`build_sprite_list` (sprites.rs) et `set_0` (registres résiduels, D4 lu par l'effet 0xad63e), `set_30`, `enemy_16`.
Relancer ensuite les scénarios (`DDSOM_SANS=nom` pour isoler, `examples/pctrace`, `examples/watch`).

Remarques sur `validate` : (1) la pile des tâches déborde sous FF0A00 (FF09B0-FF09D2 vus en attract) : la zone
ignorée commence maintenant en FF0900 ; (2) **tempête de pas** (cause élucidée de « une touche pressée pendant l'entrée
de scène fait diverger les machines ») : un bouton pressé en phase 8 (ou à l'attract) fait passer le jeu en
avance rapide deux images plus tard : une tâche auxiliaire (table des tâches, entrée 13) remet aussitôt la tâche
principale prête, qui enchaîne ses pas de jeu sans attendre l'IRQ (`world_update` 100 à 140 fois par image au lieu
d'une, mesurable avec `examples/passages`). Le nombre de pas par image vaut alors (cycles de l'image) / (cycles d'un
pas) : il dépend du coût en cycles des natives, donc diffère forcément du code d'origine (écart de `ff806e`, le
compteur d'images, d'un pas à l'autre). Le code d'origine seul est déterministe. Ces états ne sont comparables que si
la durée des natives est exacte ; le bot évite ces instants et les scénarios de couverture doivent aussi les éviter
(aucun bouton en entrée de scène). Le moteur final devra fixer explicitement le nombre de pas de cette avance rapide.

Scénarios de boss : les écarts des variantes `_mort` (images 6211, 6652, 7111, 7741, 11040) étaient cette même tempête de
pas, déclenchée par un bouton pressé ou **maintenu** (la troncature des entrées laissait des boutons enfoncés) quand le
jeu change de scène après la mort du boss ; les écarts de `game_phase12` (FFD514, FFD574, FFD692), de `combat_update`
(FF8664) et de l'effet du boss 8 avaient disparu avec les campagnes de registres résiduels. `tools/gen_boss.py` tronque
désormais les entrées avant le bouton fautif et relâche les boutons restés enfoncés : `scenarios/boss/*_mort` et
`boss12_scene40` (entier) sont VALIDES sur 12 000 images ; `boss18_scene26` est tronqué à l'image 10 900.
(aucun bouton en entrée de scène). **Résolu** : le nombre de pas par image est maintenant plafonné à 8 (valeur de
l'arcade : 5 à 10, `ddsom_game::MAX_STEPS_PER_FRAME`) par un crochet de `cps2::Machine` appliqué à l'identique au jeu
et à la machine de référence (`install`, `install_reference` ; `docs/boucle.md` §1). Les scénarios `cov_tour2/3/6/7`,
`cov_mortal_late`, `cov_attract_coin` et `boss/boss13_dragon` sont VALIDES sur 30 000 images ; `examples/determinisme`
(deux machines d'origine à ×16 et ×32) ne montre plus d'écart que sur les mots ignorés par `validate`. Les
scénarios n'ont donc plus à éviter les boutons en entrée de scène.

## Scénarios de la machine à modes (`scenarios/modes/`)

Lancés par `tools/validate_all.sh` (12 000 images) : `attract.txt` (aucune entrée : démarrage, titres, démonstrations,
présentations de personnages, droits), `screens.txt` (écrans de droits rares, sous-modes 16 et 18 forcés par `poke`),
`gameover.txt` (sélection, partie, mort, classement), `ending.txt` et `ending_end.txt` (saut dans la scène 105, boss final
affaibli : générique de fin, phase 28 ; la seconde moitié par `poke` de l'état). Les natives réécrites de ces écrans
(`modes/`, `docs/modes.md`) y sont toutes exercées sauf les chemins d'échec d'allocation d'objets (pool plein).

## Campagne « scénarios en écart restants » (`scenarios/ko/` vidé)

Les 22 scénarios de `scenarios/cov/` sont VALIDES sur 30 000 images (`DDSOM_COV=1`), ainsi que les 132 scénarios de
`tools/validate_all.sh 30000` avec `DDSOM_SCENES=1`. Tous ceux qui restaient dans `scenarios/ko/` sont VALIDES et rangés dans `boss/` (`boss13_dragon`), `enemy/`
(`ghoul_scene19`, `ghoul_scene97`), `scene/` (`scene_05`, `07`, `08`, `11`, `110`) et `scene_end/` (`end_01`) ;
`scene/scene_67` (régression de la fusion) est corrigé. Les causes, une par une, sont dans `scenarios/ko/README.md` ;
trois enseignements généraux :

* **Appeler du 68000 qui cède la main** : une native qui appelait par `call` une routine d'origine pouvant atteindre
  `0xa40` (fondu `0x1460`, chargeurs de scène) devait utiliser `call_at(adresse, retour d'origine)`, sinon la tâche
  réveillée revenait à l'adresse sentinelle (`docs/boucle.md`). Depuis, les cessions passent par des étapes Rust
  (`docs/noyau.md`, « Appels en cours en Rust ») et `call_at` a disparu : `call` refuse une cession.
* **Registres relus par du code qui n'initialise rien** : en plus de D4 et A4 déjà rendus par le constructeur de sprites,
  les registres laissés par le reconnaisseur de combos (D0-D5, D7.b) et la copie de palette par `movem` des fenêtres de
  dialogue (D0-D6/A3) sont relus plus loin (position du sang à la mort d'un boss, nombre de lignes d'une fenêtre).
* **Comportements indéfinis de l'original** (tables lues hors limites, images de sprite sans morceau) : correctifs
  communs aux deux machines, `cps2::Machine::patches`, `docs/boucle.md` §9. `validate` compare aussi les deux banques de la
  RAM d'objets et affiche les valeurs des écarts de la GFX RAM.

Les natives sont toujours dispensées d'imiter la durée de l'original : un état où la machine de référence passe plusieurs
images dans une boucle (compteur FF806E qui avance par à-coups) est un comportement indéfini à corriger, pas à reproduire
(`examples/cyc` compare les dates des natives des deux machines).

## Bots longs (`scenarios/long/`, `DDSOM_LONG=1`)

Parties longues jouées par `examples/autobot.rs` (40 000 à 90 000 images chacune) : plusieurs joueurs, magies et
objets fréquents, routes des choix 0x14 et 0x38, boutiques, difficultés 0, 3 et 7, langue 4, joueurs mortels. Elles
sortent des chemins des scénarios de 30 000 images et ont trouvé des trous du mode sans 68000. Génération :
`tools/gen_long.py [nom...]` (écrit `<nom>.txt` et `<nom>.images`, le nombre d'images du scénario). Validation :
`DDSOM_LONG=1 tools/validate_all.sh` les ajoute (en tête de liste : chacune dure 1 à 3 minutes) ; sans la variable,
la validation habituelle ne change pas. Ciblée : `validate <roms> <eeprom> scenarios/long/<nom>.txt $(cat
scenarios/long/<nom>.images)`.

| Scénario | Images | Contenu |
|---|---|---|
| `long_2p_magies` | 90 000 | clerc et magicien, magies et objets très fréquents (scènes 1, 2, 4, 0xb, 0xd, 0xe, 0x10, boutique 0x45) |
| `long_28_scenes` | 90 000 | nain seul, 28 scènes (1, 5, 9… 109), difficulté 7, langue 4, boutique enchaînée |
| `long_3p_mortels` | 60 000 | combattant, elfe et nain mortels |
| `long_4p` | 60 000 | combattant, elfe, magicien, voleur (scènes 1-0xc, boutique 0x45) |
| `long_route_14` | 60 000 | elfe depuis le choix de route 0x14 (0x18, boutique 0x4c, 0x6a) |
| `long_route_38` | 60 000 | voleur et nain au choix de route 0x38 |
| `long_boutiques` | 40 000 | clerc, les 14 boutiques 0x45-0x52, difficulté 0 |
| `long_2p_d3_mortels` | 60 000 | combattant et magicien mortels, difficulté 3, scènes 0x1d-0x38 |
| `long_nain_mortel` | 60 000 | nain seul mortel, jeu naturel (game over) |
| `long_voleur_objets` | 60 000 | voleur, objets et magies très fréquents, scènes 1-56 |
| `long_magicien_57_86` | 60 000 | magicien, magies très fréquentes, scènes 57-86 |
| `long_2p_d7_mortels` | 60 000 | clerc et elfe mortels, difficulté 7 |
| `long_4p_route_38` | 60 000 | quatre joueurs au choix de route 0x38 |
| `long_route_14_2p` | 60 000 | combattant et clerc depuis le choix de route 0x14 |

Trous trouvés et corrigés (2026-10-27) ; chacun passait inaperçu des 272 scénarios de la validation complète :

| Bot (image) | Symptôme | Cause et correction |
|---|---|---|
| `long_2p_magies` (15 318) | arrêt : pas de native en 0x4dfe2 (`magic_user.rs`) | effets des sorts du magicien appelés par adresse sans native : vague 0x4dfe2, et du même coup orage 0x4d020, pluie 0x4d660, mur de feu 0x4d2c0 et les lunes de l'elfe 0x4e4de, 0x4e782 (relevé statique des appels par adresse de `player/`). Réécrits dans `player/spawn.rs` (`wave_bolt`, `storm_spell`, `rain_glow`, `wall_spark`, `moon`, `moon_b`), natives pour le fuzz : `fuzz_player spawn` avec `DDSOM_ENTREE`, 300 appels chacun, 0 écart |
| `long_2p_magies` (59 919) | divergence : position et angle d'un éclat du gel (PEFF type 12) | registre résiduel : le rebond du gel (0x4bda8) indexe sa table par D2, laissé par le gestionnaire précédent, le parent de l'anneau d'équipement (`peff_ring`, 0x4a370 : `move.w (table),D2w`), que le Rust n'écrivait pas. `parent_follow` laisse D0, D1.w, D2.w (et `parent_alive` D0.b) comme l'original ; le suivi recopié dans `ring1` passe par `parent_follow`. Trouvé avec `examples/registre_ecrit` |
| `long_2p_magies`, avec `DDSOM_ASSETS` (59 932) | délai d'un éclat du gel nul | glitch d'origine : le délai `move.b (0x4bd98,PC,D0.w)` prend l'octet haut de D0.w laissé par le tirage : 256 lectures de 16 octets sur ±32 Ko, souvent dans le code, absent des assets. Zones déclarées dans `tools/zones_natives.py` et ajoutées à `zones_natives.txt` (154 lignes dans le code) |
| `long_28_scenes` (86 671) | arrêt : pas de native en 0x20ae (`scene/shop.rs`) | initialisation du pool PEFF à la réouverture d'une boutique enchaînée : native `pools::INIT_PEFF` |
| `long_3p_mortels` (13 279) | 32 objets ESHL de type 0x1a manquants | glitch d'origine : 0xa17d0 (rangée d'éclats d'une caisse) charge le compte par `move.b (table),D5b` sans effacer D5 (0xa1670 le fait) : l'octet haut de D5.w laissé par la caisse s'ajoute à la boucle `dbf`, qui remplit le pool ; `eshl_creators::row_1a` gardait D5 entier |
| `long_4p` (24 848) | arrêt : pas de native en 0xe296c (`elf.rs`) | son de la lune de l'elfe qui teste la version (`A5+0x70`) : ajouté à `sound::SELON_VERSION` (seul de ce genre manquant parmi les 7 entrées `tst.w (0x70,A5)` des sons) |
| `long_2p_d3_mortels` (19 500) | compteur d'animation d'un joueur | `moves::pause` : pendant l'arrêt du jeu (`A5+0x3d`), l'action 0x1e (0x362d0) rend la main sans animer (`rts`), seule 0x1c (0x3626c) anime |

Résultat : les 14 bots finissent VALIDE en mode par défaut et avec `DDSOM_ASSETS` (assets régénérés avec les zones
ci-dessus). `validate` accepte `DDSOM_DIFF_MAX=n` pour afficher plus de 10 mots de RAM différents.

## Sorts, objets et interface des joueurs (`sorts_*`, lot « sorts », 2026-10-28)

Les bots longs précédents ne lançaient que les magies de la ligne d'équipement que le jeu donne au personnage. Les
`sorts_*` forcent chaque magie et chaque objet de chaque personnage, et exercent le curseur et les noms de l'interface.
Ils sont rangés dans `scenarios/long/` (validés avec `DDSOM_LONG=1`) et générés par `tools/gen_long.py` ; le bot
(`examples/autobot.rs`) gagne `--spells`, `--spell-period`, `--rods` et `--door`, et `tools/bot_lib.py` l'argument `late`.

**Numéros de sélection.** L'ordre réel des personnages du choix est : 0 combattant, 1 magicien, 2 clerc, 3 voleur, 4 nain,
5 elfe (octet `+3` du joueur : 0, 8, 2, 0xa, 6, 4 ; classe = `+3 >> 1`). Les commentaires et les noms des bots
`long_*` plus haut suivent un ordre erroné (« clerc » pour le 1, « magicien » pour le 4...) : les noms des `sorts_*`
sont les bons.

**Méthode des magies.** Le bouton 4 (attaque spéciale, `checks::special_attack`) lit l'identifiant de la ligne
d'équipement choisie (`+0x120 + 16 × (+0x15d)`) et la table 0x372e4 le convertit en numéro de sort (`+0x15e`, identifiant
+ 0xd pour 0 à 0x20, 0x21 + sort pour les sorts 0 à 0xb, 0x2e pour le 0xc). Le bot, au repos (état 0x0200, action hors 0xc,
0xe, 0x14, 0x22), écrit (`poke`, donc dans le scénario) l'identifiant du sort suivant dans la première ligne, une
réserve de 5 charges, la sélection 0, puis appuie trois fois sur le bouton 4. Un identifiant posé en pleine action, ou qui
n'est pas un sort du personnage (le sort 0x28 du clerc donné au magicien), mène l'original dans une table de sauts hors
limites : exception illégale et redémarrage de la machine (scène 0, RAM effacée) ; les listes de `gen_long.py` ne
contiennent que les sorts de chaque personnage, sans l'échange d'arme (0xc), qui exige un objet ramassé.

| Scénario | Images | Contenu |
|---|---|---|
| `sorts_combattant`, `sorts_voleur`, `sorts_nain` | 30 000 | objets lancés 0 à 8 (sauf 3), 9, 10 et magies partagées 13 à 20 (foudre, etc.), couteau du voleur (11) |
| `sorts_magicien` | 30 000 | sorts du magicien 0x18 à 0x24 (étincelles, éclair, gel, séisme, tempête, pluie, sorcière, malédiction, vague, portail, invocation, mur de feu, drain), et les quatre baguettes (`WEAPON_B` 0x24 à 0x27, pleines puis épuisées) |
| `sorts_clerc` | 30 000 | prières 0x24 à 0x2d (retenue, tempête, essaim, rayon, bannissement, lumière sacrée, protection, bénédiction, soins) |
| `sorts_elfe` | 30 000 | sorts 0 à 0x1c de l'elfe (lunes, protections, rayon...) |
| `sorts_select` | 12 000 | quatre joueurs, trois rejoignent en cours de partie (pièce puis Start) et manient le curseur de choix : touches tenues plus de 60 images (répétition), croisements des personnages déjà pris : `hud/select.rs` (0x153ba, 0x15444, 0x154e8, 0x158c6, 0x15958) |
| `sorts_noms_a`, `sorts_noms_b` | 40 000 | quatre joueurs mortels dont le nom commence par -2 à -7 (noms prédéfinis, 0x180c8) et des scores très différents (tous les rangs du tableau) |
| `sorts_porte_34` | 7 500 | scène 34 depuis la 1 avec le rang à 0 (les portes SET 27 n'apparaissent qu'à ce rang) : le joueur est posé devant les portes (`--door`) ; sortie de zone SET 7 |

Fonctions de la liste « jamais exécutées » (couverture-fonctions.md, P5) atteintes par ces scénarios (`examples/cov`) :
0x41aa4 et 0x41ac4 (bannissement), 0x439fc et 0x44384 (elfe), 0x474c4 et 0x474e4 (foudre), 0x47750 et 0x47770 (tempête
à gauche), 0x48392 et 0x48660 (mur de feu), 0x49d1e (voleur), 0x46044 (baguettes) : les onze. Pour F3, les branches des
noms prédéfinis de 0x180c8 (-2 à -7, rang du tableau) et la répétition du curseur de 0x158c6 / 0x15958 sont exécutées.

Trous trouvés et corrigés (chacun dans un commit séparé) :

| Bot (image) | Symptôme | Cause et correction |
|---|---|---|
| `sorts_nain` (21 306) | un second tir de type 15 (pièce lancée) créé par un ESHL 14 | `families/eshl.rs`, `shot14::attack` : la table de sauts de 0x9fbd4 n'a que deux entrées (`+7` = 0 et 2), le troisième mot mène à 0x9fb1e (fin commune des autres séries) ; la native rejouait la création pour tout `+7` ≥ 2 tant que `+0x33` restait posé. Le cas `+7` = 4 appelle `close(x, 7)` |
| `sorts_noms_b` (38 138) | les quatre joueurs de la scène 0xb ne sont plus écartés du boss 3 | `families/boss_3/body.rs`, `push_players` : la boucle de 0x818b2 / 0x819b0 appelle les formes 0x818e6 / 0x819e2 avec A1 = le joueur ; la native laissait A1 au reste de l'appel précédent (il vaut 0xff8dde en sortie) |
| `sorts_magicien` (4 593) | arrêt : pas de native en 0x535f2 | `rod_shot` appelait par adresse les tirs de baguette 0x5336a (flammes), 0x53478 (boule) et 0x535f2 (étoiles). Réécrits dans `player/spawn.rs` (`rod_row`, `rod_ball`) avec leurs natives |
| `sorts_combattant`, avec `DDSOM_ASSETS` (23 834) | un sprite d'un mot différent | lectures hors bornes du constructeur de sprites en 0x3200 et 0x1ef5c (dans le code, absent des assets) : zones ajoutées à `tools/zones_natives.py` et `port/extract/zones_natives.txt` |

Limites : le fuzz `fuzz_player spawn` ne peut pas comparer les trois tirs de baguette (l'original, depuis des états
tirés au hasard, boucle plus d'un million de blocs : un niveau absurde donne un compte de 65 536 tours) ; ils sont
validés par les scénarios (les quatre baguettes sont tirées, pleines et épuisées). Les portes de passage SET 27 et 29
(0xd8458, 0xd8dce) étaient déjà exécutées presque en entier par les scénarios (restent 0xd8316, 0xd8366, 0xd83b8, 0xd840a,
helpers de suspension) ; les boutiques et les routes 0x14 et 0x38 ne reçoivent pas de bot nouveau ici.
